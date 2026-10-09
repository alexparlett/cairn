//! The repository worker pool: the threads that touch the repository, and the
//! values that cross back to the window.
//!
//! Six threads per open repository, started by the first, each with its own
//! [`Outbox`] so the update stream ends only when every one of them has gone:
//!
//! - `cairn-repository` (this file): takes the `git` the application found as
//!   it started ([`Discovery`]), opens the repository and its askpass channel,
//!   serves the history lane and a refresh's refs (`history_lane.rs`), the
//!   filters and the command log, forwards each operation to its write lane,
//!   and on its way out closes the repository — every `git` in it ended and
//!   reaped, up to `CLOSE_BOUND`, once the write the local lane is running has
//!   ended — and stops the other five.
//! - `cairn-diff` (`diff_lane.rs`): the changes and file-diff lanes, so a
//!   page and a diff never queue behind each other.
//! - `cairn-refresh` (`refresh_lane.rs`): status and ahead/behind, so neither
//!   a page nor a diff queues behind a slow status.
//! - `cairn-network` (`network_lane.rs`): the network lane, where fetch runs,
//!   so it blocks neither the walk nor the window.
//! - `cairn-local` (`local_lane.rs`): the local write lane, where every write
//!   to the index, the working tree and a local ref runs, one at a time, in the
//!   order asked; reached directly, so a write never waits behind a page.
//! - `cairn-askpass` (`askpass.rs`): accepts the helper's questions and waits
//!   on the window for each answer.
//!
//! Which thread serves a request is the routing table in `routing.rs`, applied
//! as the request is submitted. Queries carry an epoch numbered in their lane
//! (`epoch.rs`) and are superseded by the next in that lane — a changes query
//! the file diff's too; operations carry none, so a scroll, a diff and a fetch
//! cannot cancel one another.

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use cairn_git::{CLOSE_BOUND, SharedRepository};
use cairn_model::{Disclosure, RefsSnapshot};

use super::askpass::{AcceptorStop, Reply, STOP_DEADLINE, serve_prompts};
use super::diff_lane::{DiffJob, Serving, serve_diffs};
use super::discovery::Discovery;
use super::epoch::{Epoch, Epochs, QueryLane};
use super::history_lane::{Answering, Finding, HistoryLane};
use super::local_lane::{LaneState, Local, LocalJob, serve_local_lane};
use super::network_lane::{FetchControl, Lane, Operation, serve_network_lane};
use super::refresh_lane::{RefreshJob, Refreshing, serve_refreshes};
use super::request::{MOST_LANES, Request, Update};
use super::routing::{Page, RepositoryJob, Routed, route};
use super::startup::{Backend, Startup};
use super::wake::{Wake, Woken};

/// What the window does with a credential prompt's answer; see [`Reply`].
/// A plain callback, never a struct field: it holds the channel a secret
/// travels down, and application state must not keep one.
pub type Replier = Rc<dyn Fn(Reply)>;

/// How long the window may wait on a close it asked for before a second
/// request closes it anyway: past the longest an honest close takes —
/// `CLOSE_BOUND` for the reaps, then up to the acceptor's stop deadline — so
/// only a worker that has stopped answering is abandoned, and the window can
/// always be closed. The one close that may honestly take longer is one that
/// waits on a local write — a commit in its hooks (staging-and-commit R4.9) —
/// which the window says it is finishing; a second request past this closes
/// the window then too, leaving the write to finish or not, and a lock it
/// strands is named the next time the repository opens.
pub const CLOSE_PATIENCE: Duration = Duration::from_secs(5);

const _: () = assert!(
    CLOSE_PATIENCE.as_millis() > CLOSE_BOUND.as_millis() + STOP_DEADLINE.as_millis(),
    "the window must not give up on a close before an honest one can finish"
);

/// Returns before touching a disk; open failures arrive as [`Update::Failed`], and
/// so does a `git` that is missing or older than Cairn requires, checked first —
/// once per application, by `git` ([`Discovery`]), never per repository.
/// Drive [`Updates`] from exactly one task.
pub fn open(
    path: impl AsRef<Path>,
    git: &Discovery,
) -> Result<(RepositoryHandle, Updates, Replier), OpenError> {
    let path = path.as_ref().to_owned();
    let discovery = git.clone();

    let (jobs, incoming) = channel::<(Option<Epoch>, RepositoryJob)>();
    // Created here, on the caller's thread, so a diff is routed straight to the diff
    // thread rather than forwarded by the repository thread behind a page.
    let (diff_jobs, diff_incoming) = channel::<DiffJob>();
    // Likewise the refresh thread's: a status is asked of it directly.
    let (refresh_jobs, refresh_incoming) = channel::<RefreshJob>();
    // And the local lane's: a write is sent to it directly (staging-and-commit R4.1).
    let (local_jobs, local_incoming) = channel::<LocalJob>();
    let lane = LaneState::default();
    let (outgoing, inbox) = channel::<Envelope>();
    let (answers, answered) = channel::<Reply>();
    let wake = Wake::new();
    let epochs = Epochs::new();
    // Held by the handle too, so a cancel reaches the fetch without queueing behind a page.
    let control = FetchControl::default();

    // One sender per thread, so the stream ends when the last thread does.
    let outbox = Outbox {
        updates: Some(outgoing.clone()),
        wake: Arc::clone(&wake),
    };
    let network_outbox = Outbox {
        updates: Some(outgoing.clone()),
        wake: Arc::clone(&wake),
    };
    let acceptor_outbox = Outbox {
        updates: Some(outgoing.clone()),
        wake: Arc::clone(&wake),
    };
    let diff = DiffThread {
        jobs: diff_incoming,
        stop: diff_jobs.clone(),
        startup: git.startup().clone(),
        outbox: Outbox {
            updates: Some(outgoing.clone()),
            wake: Arc::clone(&wake),
        },
        epochs: epochs.clone(),
    };
    let refresh = RefreshThread {
        jobs: refresh_incoming,
        queue: refresh_jobs.clone(),
        outbox: Outbox {
            updates: Some(outgoing.clone()),
            wake: Arc::clone(&wake),
        },
        epochs: epochs.clone(),
        lane: lane.clone(),
    };
    let local = LocalThread {
        jobs: local_incoming,
        stop: local_jobs.clone(),
        outbox: Outbox {
            updates: Some(outgoing),
            wake: Arc::clone(&wake),
        },
        lane: lane.clone(),
    };
    let worker_epochs = epochs.clone();
    let worker_wake = Arc::clone(&wake);
    let worker_control = control.clone();
    let opening = path.clone();

    // Detached; ends when the last `RepositoryHandle` drops.
    std::thread::Builder::new()
        .name("cairn-repository".to_owned())
        .spawn(move || {
            // The only sender on this thread, owned by `exit`: a second copy would outlive it and
            // leave the UI task an open, empty channel.
            let exit = WorkerExit {
                outbox: Some(outbox),
                wake: Arc::clone(&worker_wake),
            };
            let Some(outbox) = exit.outbox.as_ref() else {
                return;
            };
            // Before anything else: the git the application found as it started,
            // waiting for that answer if it is still being found. A missing or too-old
            // git is reported with the version Cairn needs, never worked around (D1).
            let git = match discovery.git() {
                Ok(git) => git.clone(),
                Err(message) => {
                    outbox.send(
                        None,
                        Update::Failed {
                            message: message.to_owned(),
                        },
                    );
                    return;
                }
            };
            // As the git found above would find it: a bare repository planted in a
            // working tree is refused here, as that git refuses it under
            // `safe.bareRepository = explicit`, since every git run in it afterwards
            // is given its git directory and checks nothing.
            let startup = discovery.startup();
            let shared =
                match SharedRepository::discover_for(&opening, &git, |name| startup.parent(name)) {
                    Ok(shared) => Arc::new(shared),
                    Err(source) => {
                        // No epoch: failing to open answers no request.
                        outbox.send(
                            None,
                            Update::Failed {
                                message: source.to_string(),
                            },
                        );
                        return;
                    }
                };
            outbox.send(
                None,
                Update::Opened {
                    name: repository_name(&shared),
                },
            );
            // This repository's channel, and git pointed at it.
            let backend = Backend::open(discovery.startup(), &git);
            let threads = Threads::start(
                backend,
                Arc::clone(&shared),
                answered,
                (network_outbox, acceptor_outbox),
                (diff, refresh, local),
                &worker_wake,
                worker_control,
            );
            serve(&shared, incoming, outbox, worker_epochs, &threads);
            // However serve ended — a close, every handle gone — the repository closes.
            threads.stop();
        })
        .map_err(|source| OpenError {
            message: format!(
                "could not start a repository worker for {}: {source}",
                path.display()
            ),
        })?;

    Ok((
        RepositoryHandle {
            jobs,
            diff: diff_jobs,
            refresh: refresh_jobs,
            local: local_jobs,
            lane,
            epochs: epochs.clone(),
            control,
        },
        Updates {
            inbox,
            wake,
            epochs,
        },
        Rc::new(move |answer| {
            // A failed send means the acceptor is gone; the prompt it served is refused.
            let _ = answers.send(answer);
        }),
    ))
}

/// What the title bar calls `shared`: the last component of its working tree — of its git
/// directory, when it is bare — which discovery has already made absolute and free of `.`
/// and `..`, so a repository opened at `.`, `..`, a subdirectory or its `.git` is named by its
/// own folder (`an_open_names_the_repositorys_folder_whatever_path_it_was_opened_at`).
fn repository_name(shared: &SharedRepository) -> String {
    let root = shared.workdir().unwrap_or_else(|| shared.git_dir());
    root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// [`open`] with a `git` found afresh from `startup` — the environment it is
/// searched on and run with, and the helper it is pointed at; what a test
/// hands in.
#[cfg(test)]
pub(super) fn open_with(
    path: impl AsRef<Path>,
    startup: Startup,
) -> Result<(RepositoryHandle, Updates, Replier), OpenError> {
    open(path, &Discovery::new(startup))
}

/// What the diff thread is started with: its queue, which `open` made so the
/// handle reaches it directly, a sender of the repository thread's own to
/// stop it with, how the launching environment is read (which opening the
/// repository again needs), its outbox and the epochs it is cancelled by.
struct DiffThread {
    jobs: Receiver<DiffJob>,
    stop: Sender<DiffJob>,
    startup: Startup,
    outbox: Outbox,
    epochs: Epochs,
}

/// What the refresh thread is started with: its queue, which `open` made so the handle
/// reaches it directly, a sender of the repository thread's own — to hand it ahead/behind
/// and to stop it with — its outbox and the epochs it is cancelled by.
struct RefreshThread {
    jobs: Receiver<RefreshJob>,
    queue: Sender<RefreshJob>,
    outbox: Outbox,
    epochs: Epochs,
    /// The local lane's state: a status read while a write runs is dropped, and none starts
    /// while a commit runs.
    lane: LaneState,
}

/// What the local lane is started with: its queue, which `open` made so the handle reaches it
/// directly, a sender of the repository thread's own to stop it with, its outbox and the state
/// it shares with the handle and the refresh thread.
struct LocalThread {
    jobs: Receiver<LocalJob>,
    stop: Sender<LocalJob>,
    outbox: Outbox,
    lane: LaneState,
}

/// The five threads the repository thread starts, how it reaches them, and
/// the repository they run `git` in, which stopping them closes.
struct Threads {
    /// The network lane's queue; one lane per [`Lane`], one lane so far.
    network: Option<Sender<Operation>>,
    /// Told to stop as the repository closes: the handles hold the diff
    /// thread's queue open, so it is not ended by this thread going.
    diff: Sender<DiffJob>,
    /// The refresh thread's queue: ahead/behind is handed on here, and the
    /// thread is told to stop as the repository closes, as the diff thread is.
    refresh: Sender<RefreshJob>,
    control: FetchControl,
    /// The local lane's queue, to stop it with as the repository closes.
    local: Option<Sender<LocalJob>>,
    /// The local lane, waited for as the repository closes: the write it runs is never ended
    /// by a close (staging-and-commit R4.9).
    local_thread: Option<JoinHandle<()>>,
    lane: LaneState,
    acceptor: Option<AcceptorStop>,
    shared: Arc<SharedRepository>,
}

impl Threads {
    fn start(
        backend: Backend,
        shared: Arc<SharedRepository>,
        answered: Receiver<Reply>,
        (network_outbox, acceptor_outbox): (Outbox, Outbox),
        (diff, refresh, local): (DiffThread, RefreshThread, LocalThread),
        wake: &Arc<Wake>,
        control: FetchControl,
    ) -> Self {
        let (network, queued) = channel::<Operation>();
        let Backend {
            git,
            channel,
            mut prompting,
        } = backend;

        let acceptor = channel.as_ref().map(|channel| {
            let stop = AcceptorStop::new(channel.socket_path().to_owned());
            let exit = WorkerExit {
                outbox: Some(acceptor_outbox),
                wake: Arc::clone(wake),
            };
            let channel = Arc::clone(channel);
            let serving = stop.clone();
            let started = std::thread::Builder::new()
                .name("cairn-askpass".to_owned())
                .spawn(move || {
                    if let Some(outbox) = exit.outbox.as_ref() {
                        serve_prompts(channel, answered, outbox, &serving);
                    }
                    // The channel (and with it the socket) goes before the stream ends.
                    drop(exit);
                });
            if let Err(error) = started {
                // Nothing accepts, so no prompt can be answered: say so on the fetch that needs one.
                prompting = Err(format!("the askpass thread could not be started: {error}"));
                // And nothing will leave a loop it never entered: a stop is not to wait for it.
                stop.acknowledge();
            }
            stop
        });

        let DiffThread {
            jobs: diff_jobs,
            stop: diff_stop,
            startup: diff_startup,
            outbox: diff_outbox,
            epochs,
        } = diff;
        let diff_exit = WorkerExit {
            outbox: Some(diff_outbox),
            wake: Arc::clone(wake),
        };
        let diff_shared = Arc::clone(&shared);
        let diff_git = git.clone();
        // A thread that cannot be started drops its exit and its queue, which ends its
        // part of the stream; a diff asked of it is then never answered, which the
        // window's own state reports as waiting.
        let _ = std::thread::Builder::new()
            .name("cairn-diff".to_owned())
            .spawn(move || {
                if let Some(outbox) = diff_exit.outbox.as_ref() {
                    let serving = Serving {
                        git: &diff_git,
                        startup: &diff_startup,
                        epochs: &epochs,
                        jobs: &diff_jobs,
                        outbox,
                    };
                    serve_diffs(&diff_shared, &serving);
                }
                drop(diff_exit);
            });

        let RefreshThread {
            jobs: refresh_jobs,
            queue: refresh_queue,
            outbox: refresh_outbox,
            epochs: refresh_epochs,
            lane: refresh_lane,
        } = refresh;
        let refresh_exit = WorkerExit {
            outbox: Some(refresh_outbox),
            wake: Arc::clone(wake),
        };
        let refresh_shared = Arc::clone(&shared);
        let refresh_git = git.clone();
        // As the diff thread: one that cannot be started ends its part of the stream, and
        // a refresh asked of it is never answered.
        let _ = std::thread::Builder::new()
            .name("cairn-refresh".to_owned())
            .spawn(move || {
                if let Some(outbox) = refresh_exit.outbox.as_ref() {
                    let serving = Refreshing {
                        git: &refresh_git,
                        epochs: &refresh_epochs,
                        lane: &refresh_lane,
                        jobs: &refresh_jobs,
                        outbox,
                    };
                    serve_refreshes(&refresh_shared, &serving);
                }
                drop(refresh_exit);
            });

        let LocalThread {
            jobs: local_jobs,
            stop: local_stop,
            outbox: local_outbox,
            lane,
        } = local;
        let local_exit = WorkerExit {
            outbox: Some(local_outbox),
            wake: Arc::clone(wake),
        };
        let local_shared = Arc::clone(&shared);
        let local_git = git.clone();
        let local_channel = channel.clone();
        let local_prompting = prompting.clone();
        let local_lane = lane.clone();
        // As the diff thread: one that cannot be started ends its part of the stream, and a
        // write asked of it is never answered.
        let local_thread = std::thread::Builder::new()
            .name("cairn-local".to_owned())
            .spawn(move || {
                if let Some(outbox) = local_exit.outbox.as_ref() {
                    let serving = Local {
                        git: &local_git,
                        channel: local_channel.as_ref(),
                        prompting: &local_prompting,
                        lane: &local_lane,
                        jobs: &local_jobs,
                        outbox,
                    };
                    serve_local_lane(&local_shared, &serving);
                }
                // This thread may hold the last reference to the channel: let it go first.
                drop(local_channel);
                drop(local_exit);
            })
            .ok();

        let running = control.clone();
        let exit = WorkerExit {
            outbox: Some(network_outbox),
            wake: Arc::clone(wake),
        };
        let lane_shared = Arc::clone(&shared);
        let _ = std::thread::Builder::new()
            .name("cairn-network".to_owned())
            .spawn(move || {
                if let Some(outbox) = exit.outbox.as_ref() {
                    serve_network_lane(
                        &git,
                        &lane_shared,
                        channel.as_ref(),
                        &prompting,
                        &running,
                        &queued,
                        outbox,
                    );
                }
                // This thread may hold the last reference to the channel: let it go first.
                drop(channel);
                drop(exit);
            });

        Self {
            network: Some(network),
            diff: diff_stop,
            refresh: refresh_queue,
            control,
            local: Some(local_stop),
            local_thread,
            lane,
            acceptor,
            shared,
        }
    }

    /// Forwards an operation to its lane. A second copy of one already
    /// running or waiting there is refused, with the reason sent to the
    /// window, never dropped in silence (PRD R7.2).
    fn perform(&self, operation: Operation, outbox: &Outbox) {
        match operation.lane() {
            Lane::Network => {
                let Operation::Fetch { remote } = &operation;
                if let Err(refusal) = self.control.arm(remote) {
                    outbox.send(
                        None,
                        Update::FetchRefused {
                            remote: remote.clone(),
                            reason: refusal.to_string(),
                        },
                    );
                    return;
                }
                if let Some(network) = &self.network {
                    let _ = network.send(operation);
                }
            }
        }
    }

    /// Closes the repository and ends both threads; see [`Threads::drop`],
    /// which does the work so that a panic in `serve` does it too.
    fn stop(self) {
        drop(self);
    }
}

impl Drop for Threads {
    /// Closes the repository (PRD R6.3): the diff thread is told to stop and
    /// the network lane's queue is closed; the fetch in flight, if any, is
    /// cancelled at once (`FetchControl`, as `CancelFetch` does), since the
    /// local lane is waited for next and a fetch must not run on for as long
    /// as a commit's hooks do; then the local lane is marked closing and
    /// joined, so the write it is running finishes and is never ended by a
    /// close (staging-and-commit R4.9); then every `git` still running in the
    /// repository — a fetch mid-spawn, one dropped to a reaper, a read — is
    /// ended the way a cancel ends it, and this waits up to `CLOSE_BOUND` for
    /// their reaps, here on the repository thread. One that enters the
    /// registry afterwards — a fetch already forwarded to the lane's queue when
    /// the close came, which the lane takes up after it — is ended as it
    /// enters. (A fetch still on this thread's own queue is never forwarded:
    /// `serve` stopped with the epochs.) Past the fetch's cancel, the registry
    /// is the one authority: what the cancel misses is ended by it. Last, the
    /// acceptor is woken to see it should stop. Runs once the window has asked
    /// to close or let go (the acceptor's answering end may still be held then:
    /// a prompt still waiting is withdrawn by the window when its fetch's or
    /// write's outcome arrives) and on unwinding alike.
    fn drop(&mut self) {
        // First, so the diff thread takes up nothing more; what it is running is
        // ended with the rest below, or by its epoch if the close stopped them.
        let _ = self.diff.send(DiffJob::Stop);
        let _ = self.refresh.send(RefreshJob::Stop);
        self.network = None;
        // Ended now, not after the local lane is waited for below: a fetch is ended by a
        // close as soon as it is asked, whatever write is still running.
        self.control.cancel();
        // The local lane starts nothing more, and the write it is running — a commit in its
        // hooks, say — is waited for, never ended (staging-and-commit R4.9): the window says
        // which, and a second close after its patience closes it anyway. Waited for here, on
        // the repository thread; the acceptor still answers a prompt the write is waiting on.
        self.lane.close();
        if let Some(local) = self.local.take() {
            let _ = local.send(LocalJob::Stop);
        }
        if let Some(local) = self.local_thread.take() {
            let _ = local.join();
        }
        // How many outlived the bound is told to nobody: the window is closing, and
        // each has had `SIGKILL`, which waiting longer cannot improve on.
        let _ = self.shared.end_invocations(CLOSE_BOUND);
        if let Some(acceptor) = &self.acceptor {
            acceptor.stop();
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepositoryHandle {
    /// The repository thread's queue: the history lane and the operations.
    jobs: Sender<(Option<Epoch>, RepositoryJob)>,
    /// The diff thread's queue: the changes and file-diff lanes.
    diff: Sender<DiffJob>,
    /// The refresh thread's queue: a refresh's status.
    refresh: Sender<RefreshJob>,
    /// The local lane's queue: a write.
    local: Sender<LocalJob>,
    /// What the local lane shares: a refresh asked while a commit runs is kept back, and a
    /// cancel reaches the write it names.
    lane: LaneState,
    epochs: Epochs,
    control: FetchControl,
}

impl RepositoryHandle {
    /// Never blocks. A query supersedes what its lane has in flight — a changes
    /// query the file diff's too — is numbered, and goes straight to the
    /// thread the routing table names for its lane; its epoch is returned. An
    /// operation is queued behind nothing and supersedes nothing, and returns
    /// `None`. A cancel does not queue at all: it reaches the fetch directly,
    /// ahead of any page the repository thread is walking, through a lock the
    /// network lane holds only for an assignment and a kill that only tries
    /// for the child's — a review obligation, since this runs on the UI
    /// thread. A close stops the epochs first, an atomic store, so the page
    /// being walked and the diff being read are abandoned at their next poll
    /// and the close behind them is reached at once, and marks the local lane
    /// closing, one bounded lock, so no write asked after it starts; the waiting
    /// it starts is the repository thread's. A refresh is numbered in its two lanes and
    /// sent to two threads — its refs to the repository thread, its status to
    /// the refresh thread under the status lane's number, which no refresh moves
    /// (a running status is never superseded, R10.3 as amended) — and returns its
    /// refs' epoch.
    pub fn submit(&self, request: Request) -> Option<Epoch> {
        // Kept back while a commit runs, and so numbered nowhere: the commit's ending says to
        // read everything again (staging-and-commit R4.6).
        if matches!(request, Request::Refresh | Request::RefreshStatus) && self.lane.defer_refresh()
        {
            return None;
        }
        // Numbered before it is routed, so what it supersedes is cancelled now,
        // not when a thread gets to it.
        let lanes = request.lanes();
        let mut numbered = [None; MOST_LANES];
        for (slot, lane) in numbered.iter_mut().zip(lanes) {
            *slot = Some(self.epochs.bump(*lane));
        }
        let epoch_of = |lane: QueryLane| {
            lanes
                .iter()
                .position(|asked| *asked == lane)
                .and_then(|at| numbered.get(at).copied().flatten())
        };
        let epoch = numbered[0];
        // A failed send means the worker is gone and has already said so.
        match route(request) {
            Routed::Refresh => {
                if let (Some(refs), Some(ahead_behind)) =
                    (epoch_of(QueryLane::Refs), epoch_of(QueryLane::AheadBehind))
                {
                    let status = self.epochs.current(QueryLane::Status);
                    let _ = self
                        .jobs
                        .send((Some(refs), RepositoryJob::Refs { ahead_behind }));
                    let _ = self.refresh.send(RefreshJob::Status { epoch: status });
                }
            }
            Routed::OpenHistory { rows, lost } => {
                if let (Some(epoch), Some(walk)) = (epoch, epoch_of(QueryLane::Walk)) {
                    let _ = self.jobs.send((
                        Some(epoch),
                        RepositoryJob::History(Page::Open { rows, walk, lost }),
                    ));
                }
            }
            Routed::CancelFetch => self.control.cancel(),
            Routed::Write { id, write } => {
                // A write ends the amend read in flight or queued (staging-and-commit R10.3): it
                // is read again over the status the write's ending reads, and a stage queued
                // behind a stale one would wait on its walk for nothing.
                self.epochs.bump(QueryLane::Amending);
                let _ = self.local.send(LocalJob::Write {
                    id,
                    write: Box::new(write),
                });
            }
            Routed::DiscardConsequence { asked, paths } => {
                if let Some(epoch) = epoch {
                    let _ = self.local.send(LocalJob::Consequence {
                        asked,
                        paths,
                        cancel: self.epochs.watch(epoch),
                    });
                }
            }
            Routed::DiscardLinesConsequence {
                asked,
                diff,
                selection,
            } => {
                if let Some(epoch) = epoch {
                    let _ = self.local.send(LocalJob::LinesConsequence {
                        asked,
                        diff,
                        selection,
                        cancel: self.epochs.watch(epoch),
                    });
                }
            }
            // Numbered above: the count in flight is superseded, and nothing is sent.
            Routed::StopCounting => {}
            Routed::CommitReads => {
                if let Some(epoch) = epoch {
                    let _ = self.local.send(LocalJob::CommitReads {
                        epoch,
                        cancel: self.epochs.watch(epoch),
                    });
                }
            }
            Routed::Amending { status } => {
                if let Some(epoch) = epoch {
                    let _ = self.local.send(LocalJob::Amending {
                        status,
                        epoch,
                        cancel: self.epochs.watch(epoch),
                    });
                }
            }
            Routed::CheckBranchName { name } => {
                if let Some(epoch) = epoch {
                    let _ = self.local.send(LocalJob::BranchName {
                        name,
                        epoch,
                        cancel: self.epochs.watch(epoch),
                    });
                }
            }
            Routed::CheckoutConsequence { asked, name, at } => {
                if let Some(epoch) = epoch {
                    let _ = self.local.send(LocalJob::CheckoutConsequence {
                        asked,
                        name,
                        at,
                        cancel: self.epochs.watch(epoch),
                    });
                }
            }
            // Numbered above: the amend read in flight is superseded, and nothing is sent.
            Routed::StopAmending => {}
            Routed::CancelWrite(id) => self.lane.cancel(id),
            Routed::RefreshStatus => {
                let status = self.epochs.current(QueryLane::Status);
                let _ = self.refresh.send(RefreshJob::Status { epoch: status });
            }
            Routed::Repository(job) => {
                if matches!(job, RepositoryJob::Close) {
                    self.epochs.stop();
                    // No write asked from now on is started, whatever the repository thread
                    // is still busy with (R4.9): one bounded lock.
                    self.lane.close();
                }
                let _ = self.jobs.send((epoch, job));
            }
            Routed::ConfiguredContext => {
                let _ = self.diff.send(DiffJob::ConfiguredContext);
            }
            Routed::Diff(query) => {
                if let Some(epoch) = epoch {
                    let _ = self.diff.send(DiffJob::Query {
                        epoch,
                        query: Box::new(query),
                    });
                }
            }
        }
        epoch
    }

    /// [`Self::submit`] as the plain callback the window takes.
    pub fn into_submitter(self) -> Rc<dyn Fn(Request)> {
        Rc::new(move |request| {
            self.submit(request);
        })
    }
}

/// A handle with no worker behind it, and what has been sent through it
/// since last asked — the repository thread's queue, then the diff
/// thread's: for a test of the window's side alone.
#[cfg(test)]
pub fn idle_handle() -> (RepositoryHandle, impl Fn() -> Vec<Request>) {
    use super::routing::unroute;

    let (jobs, incoming) = channel::<(Option<Epoch>, RepositoryJob)>();
    let (diff, diff_incoming) = channel::<DiffJob>();
    // A refresh's status is read back as a status alone: a refresh's own is read back, from
    // the repository thread's queue.
    let (refresh, refresh_incoming) = channel::<RefreshJob>();
    let (local, local_incoming) = channel::<LocalJob>();
    let handle = RepositoryHandle {
        jobs,
        diff,
        refresh,
        local,
        lane: LaneState::default(),
        epochs: Epochs::new(),
        control: FetchControl::default(),
    };
    (handle, move || {
        let repository = incoming
            .try_iter()
            .map(|(_, job)| unroute(Routed::Repository(job)));
        let diffs = diff_incoming.try_iter().filter_map(|job| match job {
            DiffJob::Query { query, .. } => Some(unroute(Routed::Diff(*query))),
            DiffJob::ConfiguredContext => Some(unroute(Routed::ConfiguredContext)),
            DiffJob::Stop => None,
        });
        let asked: Vec<Request> = repository.chain(diffs).collect();
        // Only a status asked alone: a refresh's own was read back from its refs above.
        let statuses = refresh_incoming.try_iter().count();
        let refreshes = asked
            .iter()
            .filter(|request| matches!(request, Request::Refresh))
            .count();
        let alone = std::iter::repeat_with(|| Request::RefreshStatus)
            .take(statuses.saturating_sub(refreshes));
        let writes = local_incoming.try_iter().filter_map(|job| match job {
            LocalJob::Write { id, write } => Some(unroute(Routed::Write { id, write: *write })),
            LocalJob::Consequence { asked, paths, .. } => {
                Some(unroute(Routed::DiscardConsequence { asked, paths }))
            }
            LocalJob::LinesConsequence {
                asked,
                diff,
                selection,
                ..
            } => Some(unroute(Routed::DiscardLinesConsequence {
                asked,
                diff,
                selection,
            })),
            LocalJob::CommitReads { .. } => Some(unroute(Routed::CommitReads)),
            LocalJob::Amending { status, .. } => Some(unroute(Routed::Amending { status })),
            LocalJob::BranchName { name, .. } => Some(unroute(Routed::CheckBranchName { name })),
            LocalJob::CheckoutConsequence {
                asked, name, at, ..
            } => Some(unroute(Routed::CheckoutConsequence { asked, name, at })),
            LocalJob::Stop => None,
        });
        asked.into_iter().chain(alone).chain(writes).collect()
    })
}

/// Driven from exactly one task.
#[derive(Debug)]
pub struct Updates {
    inbox: Receiver<Envelope>,
    wake: Arc<Wake>,
    epochs: Epochs,
}

impl Updates {
    /// The next update worth rendering, or `None` once every worker has gone.
    /// Epochless updates are never dropped. A superseded answer is not rendered: one holding
    /// something large comes back as [`Update::Superseded`], so the window hands it to a
    /// worker to free rather than freeing it here, on the task the UI thread drives; the
    /// rest is dropped.
    pub async fn next(&mut self) -> Option<Update> {
        loop {
            match self.inbox.try_recv() {
                Ok(Envelope {
                    epoch: None,
                    update,
                }) => return Some(update),
                Ok(Envelope {
                    epoch: Some(epoch),
                    update,
                }) => {
                    if self.epochs.is_current(epoch) {
                        return Some(update);
                    }
                    if let Some(retired) = update.into_retired() {
                        return Some(Update::Superseded(retired));
                    }
                }
                Err(TryRecvError::Empty) => Woken(&self.wake).await,
                Err(TryRecvError::Disconnected) => return None,
            }
        }
    }
}

impl Drop for Updates {
    /// Stops any walk in progress.
    fn drop(&mut self) {
        self.epochs.stop();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenError {
    message: String,
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for OpenError {}

/// `epoch` is `None` for news about the worker itself.
#[derive(Debug)]
pub(super) struct Envelope {
    epoch: Option<Epoch>,
    update: Update,
}

#[cfg(test)]
impl Envelope {
    /// What a thread sent, unwrapped, for a test of that thread alone.
    pub(super) fn opened(self) -> (Option<Epoch>, Update) {
        (self.epoch, self.update)
    }
}

/// Not `Clone`: a second sender would hold the channel open past the exit wake.
///
/// Closing one wakes the task driving [`Updates`], whichever thread drops it and
/// however it got there: the stream ends when the LAST sender closes, and a
/// sender that closed silently while the task was parked would leave it parked
/// for good. That is what happened on the worker's early exits — an outbox
/// captured by the repository thread's closure outlived the [`WorkerExit`] that
/// had already signalled, and was dropped after it with no wake of its own
/// (`an_outbox_dropped_anywhere_ends_the_stream`).
#[derive(Debug)]
pub(super) struct Outbox {
    /// `None` only inside `drop`, so the channel is closed before the wake.
    updates: Option<Sender<Envelope>>,
    wake: Arc<Wake>,
}

impl Outbox {
    pub(super) fn send(&self, epoch: Option<Epoch>, update: Update) {
        if let Some(updates) = &self.updates
            && updates.send(Envelope { epoch, update }).is_ok()
        {
            self.wake.signal();
        }
    }

    /// One with its receiving end handed back bare, for a test of one thread
    /// alone; drop the receiver for a stream nobody drives.
    #[cfg(test)]
    pub(super) fn watched() -> (Self, Receiver<Envelope>) {
        let (updates, inbox) = channel::<Envelope>();
        (
            Self {
                updates: Some(updates),
                wake: Wake::new(),
            },
            inbox,
        )
    }
}

impl Drop for Outbox {
    /// Close the channel, then wake: a task woken while a sender lives sees an
    /// empty channel and parks again, so the order is the whole point.
    fn drop(&mut self) {
        self.updates = None;
        self.wake.signal();
    }
}

/// Announces a worker's death from `Drop`, so unwinding runs it.
#[derive(Debug)]
struct WorkerExit {
    outbox: Option<Outbox>,
    wake: Arc<Wake>,
}

impl Drop for WorkerExit {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(outbox) = &self.outbox
        {
            outbox.send(
                None,
                Update::WorkerLost {
                    message: "the repository worker stopped unexpectedly; \
                              reopen the repository to carry on"
                        .to_owned(),
                },
            );
        }
        // Dropping the outbox closes the channel and then wakes; the wake here is for a
        // worker whose outbox was already taken.
        self.outbox = None;
        self.wake.signal();
    }
}

fn serve(
    shared: &SharedRepository,
    jobs: Receiver<(Option<Epoch>, RepositoryJob)>,
    outbox: &Outbox,
    epochs: Epochs,
    threads: &Threads,
) {
    let repo = shared.to_worker();
    // Once, outside the loop: the scroll's session borrows `repo` across turns, so moving
    // this inside fails to compile.
    let mut history = HistoryLane::default();
    let answering = Answering {
        epochs: &epochs,
        outbox,
    };

    // A find in progress (R8.5): one page of it is walked whenever no other job is waiting, so
    // a filter, a refresh or a free asked meanwhile is served between its pages, and the
    // next page or find — whose number supersedes it — ends it.
    let mut finding: Option<(Epoch, Finding)> = None;
    loop {
        let next = match finding {
            Some((epoch, find)) => match jobs.try_recv() {
                Ok(job) => job,
                Err(TryRecvError::Empty) => {
                    finding = (!epochs.is_stopping()
                        && epochs.is_current(epoch)
                        && history.find_page(&repo, find, epoch, &answering))
                    .then_some((epoch, find));
                    continue;
                }
                Err(TryRecvError::Disconnected) => break,
            },
            None => match jobs.recv() {
                Ok(job) => job,
                Err(_) => break,
            },
        };
        let (epoch, job) = next;
        if epochs.is_stopping() {
            break;
        }
        match job {
            RepositoryJob::History(Page::Find { target, rows }) => match epoch {
                Some(epoch) if epochs.is_current(epoch) => {
                    finding = Some((epoch, Finding { target, rows }));
                }
                // Superseded before it was picked up.
                _ => {}
            },
            // Its number superseded the find; there is nothing to walk.
            RepositoryJob::History(Page::Stop) => finding = None,
            // An open whose walk is still the window's, superseded in the history lane by a
            // find or a stop asked straight after it: the walk it replaces is let go of all
            // the same, so what is asked next pages the new walk, never the old one.
            RepositoryJob::History(Page::Open { walk, lost, .. })
                if epochs.is_current(walk) && !epoch.is_some_and(|e| epochs.is_current(e)) =>
            {
                history.replace_walk(walk, lost);
            }
            RepositoryJob::History(page) => match epoch {
                Some(epoch) if epochs.is_current(epoch) => {
                    history.page(&repo, page, epoch, &answering);
                }
                // Superseded before it was picked up, and never started. (Every page is
                // numbered: `submit` numbers what is in a lane.)
                _ => {}
            },
            // Kept back while a commit runs: its ending reads everything again (R4.6).
            RepositoryJob::Refs { .. } if threads.lane.defer_refresh() => {}
            RepositoryJob::Refs { ahead_behind } => match epoch {
                Some(epoch) if epochs.is_current(epoch) => {
                    history.refresh(&repo, epoch, ahead_behind, &threads.refresh, &answering);
                }
                // Superseded by the next refresh before it was picked up.
                _ => {}
            },
            RepositoryJob::FilterRefs {
                refs,
                text,
                disclosure,
            } => {
                if let Some(epoch) = epoch {
                    sidebar_rows(refs, text, &disclosure, epoch, &epochs, outbox);
                }
            }
            RepositoryJob::ListRemotes => outbox.send(
                None,
                Update::Remotes {
                    remotes: repo.remotes(),
                },
            ),
            RepositoryJob::Fetch { remote } => {
                threads.perform(Operation::Fetch { remote }, outbox);
            }
            RepositoryJob::CommandLog => outbox.send(
                None,
                Update::CommandLog {
                    records: shared.command_log(),
                },
            ),
            RepositoryJob::Filter { of, files, text } => {
                if let Some(epoch) = epoch {
                    filter_files(of, &files, text, epoch, &epochs, outbox);
                }
            }
            RepositoryJob::FilterLocalChanges { changes, text } => {
                if let Some(epoch) = epoch {
                    filter_local_changes(changes, text, epoch, &epochs, outbox);
                }
            }
            // Freed here, off the UI thread, which is the whole of the job.
            RepositoryJob::Retire(retired) => drop(retired),
            // The epochs were stopped as it was sent; the closing is the caller's.
            RepositoryJob::Close => break,
        }
    }
}

/// The sidebar's rows for `refs` (R8.1-R8.3), answered while `epoch` is current: one
/// superseded before it started, or by the next ask while it runs, stops within a few
/// thousand refs and sends nothing. The snapshot is the window's own, shared, and handed back
/// with the rows so the window keeps them together.
pub(super) fn sidebar_rows(
    refs: Arc<RefsSnapshot>,
    text: String,
    disclosure: &Disclosure,
    epoch: Epoch,
    epochs: &Epochs,
    outbox: &Outbox,
) {
    if !epochs.is_current(epoch) {
        return;
    }
    if let Some(rows) = refs.sidebar_rows(&text, disclosure, || epochs.is_current(epoch)) {
        outbox.send(Some(epoch), Update::FilteredRefs { refs, text, rows });
    }
}

/// Which of `files`' files hold `text` (the Changes tab's filter, R5.4), answered while
/// `epoch` is current: one superseded before it started, or by a keystroke while it runs,
/// stops at the next few thousand files and sends nothing. The change set is the window's
/// own, shared; if the window let go of it meanwhile, it is freed here, off the UI thread.
fn filter_files(
    of: super::request::Comparison,
    files: &cairn_model::ChangeSet,
    text: String,
    epoch: Epoch,
    epochs: &Epochs,
    outbox: &Outbox,
) {
    if !epochs.is_current(epoch) {
        return;
    }
    if let Some(matched) = files.files_matching(&text, || epochs.is_current(epoch)) {
        outbox.send(
            Some(epoch),
            Update::FilteredFiles {
                of,
                text,
                files: matched,
            },
        );
    }
}

/// Which rows of Local Changes' two lists hold `text` (refs-and-status R9), answered while
/// `epoch` is current: one superseded before it started, or by a keystroke or a newer status
/// while it runs, stops within a few thousand paths and sends nothing. The lists are the
/// window's own, shared, and handed back with the rows so the window keeps the rows only for
/// the lists they index.
pub(super) fn filter_local_changes(
    changes: Arc<cairn_model::LocalChanges>,
    text: String,
    epoch: Epoch,
    epochs: &Epochs,
    outbox: &Outbox,
) {
    if !epochs.is_current(epoch) {
        return;
    }
    if let Some(rows) = changes.matching(&text, || epochs.is_current(epoch)) {
        outbox.send(
            Some(epoch),
            Update::FilteredLocalChanges {
                changes,
                text,
                rows,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::{LocalWrite, OperationId, Retired};
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::task::{Context, Poll, Waker};
    use std::time::Instant;

    use cairn_git::{CancelSignal, HistoryRequest, Repository};

    use crate::worker::fetch_tests::{BorrowedRepository, UnbornRepository, block_on, woken_by};
    use crate::worker::routing::Page;

    /// The next update but an open's own refs, which come before its first page.
    fn after_refs(updates: &mut Updates) -> Option<Update> {
        loop {
            match block_on(updates.next()) {
                Some(Update::Refs { reopen: false, .. }) => {}
                other => return other,
            }
        }
    }

    /// The Cairn checkout itself, opened through the real boundary.
    fn cairn() -> (RepositoryHandle, Updates) {
        match open_with(env!("CARGO_MANIFEST_DIR"), Startup::of_this_process()) {
            Ok((handle, mut updates, _)) => {
                crate::worker::fetch_tests::opened_as(&mut updates);
                (handle, updates)
            }
            Err(error) => panic!("opening the Cairn checkout: {error}"),
        }
    }

    /// QC1, R7.1: the title bar names the repository's own folder whatever path it was opened
    /// at — its working tree, `<it>/.`, `<it>/sub/..`, a subdirectory, its `.git` — as Fork
    /// names a repository by its folder. Caught by: naming the path as given (`.`, `..`,
    /// `sub`, `.git`), unresolved.
    #[test]
    fn an_open_names_the_repositorys_folder_whatever_path_it_was_opened_at() {
        let fixture = UnbornRepository::new("cairn-named-by-its-folder");
        let sub = fixture.path.join("sub");
        std::fs::create_dir_all(sub.join("deeper"))
            .unwrap_or_else(|error| panic!("making {}: {error}", sub.display()));
        for at in [
            fixture.path.clone(),
            fixture.path.join("."),
            sub.join(".."),
            sub.clone(),
            sub.join("deeper"),
            fixture.path.join(".git"),
        ] {
            let (_handle, mut updates, _) = match open_with(&at, Startup::of_this_process()) {
                Ok(opened) => opened,
                Err(error) => panic!("starting the worker: {error}"),
            };
            assert_eq!(
                crate::worker::fetch_tests::opened_as(&mut updates),
                "cairn-named-by-its-folder",
                "opened at {}",
                at.display()
            );
        }
        // Relative to where the application was launched, as a command line names it: the
        // tests run in this crate's directory, inside the Cairn checkout.
        let checkout = match cairn_git::SharedRepository::discover(env!("CARGO_MANIFEST_DIR")) {
            Ok(shared) => shared.workdir().map(Path::to_owned),
            Err(error) => panic!("opening the Cairn checkout: {error}"),
        };
        let folder = checkout
            .and_then(|root| std::fs::canonicalize(root).ok())
            .and_then(|root| {
                root.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| panic!("the Cairn checkout has no folder name"));
        for at in [".", "src", "./src/..", "../.."] {
            let (_handle, mut updates, _) = match open_with(at, Startup::of_this_process()) {
                Ok(opened) => opened,
                Err(error) => panic!("starting the worker: {error}"),
            };
            assert_eq!(
                crate::worker::fetch_tests::opened_as(&mut updates),
                folder,
                "opened at {at}"
            );
        }
    }

    /// Arrives as an update, not as an error from `open`.
    #[test]
    fn opening_a_path_outside_a_repository_is_reported_and_names_the_path() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        let (_handle, mut updates, _) = match open_with(&outside, Startup::of_this_process()) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        match block_on(updates.next()) {
            Some(Update::Failed { message }) => assert!(
                message.contains(&outside.display().to_string()),
                "the message does not name the path: {message}"
            ),
            other => panic!("expected the open to be reported, got {other:?}"),
        }
    }

    /// A git that cannot be found is reported with the required version, and nothing follows:
    /// the repository is not opened behind the refusal.
    #[test]
    fn a_missing_git_is_refused_naming_the_version_and_nothing_is_served() {
        let (_handle, mut updates, _) = match open_with(
            env!("CARGO_MANIFEST_DIR"),
            Startup::new(|_| None, PathBuf::from("/nonexistent/cairn-askpass")),
        ) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        match block_on(updates.next()) {
            Some(Update::Failed { message }) => {
                assert!(message.contains("2.30.0"), "no required version: {message}");
                assert!(message.contains("PATH is unset"), "no cause: {message}");
            }
            other => panic!("expected the refusal, got {other:?}"),
        }
        assert!(
            block_on(updates.next()).is_none(),
            "the worker went on to serve the repository after refusing git"
        );
    }

    /// A bare repository planted inside a working tree — its configuration pointing its
    /// working tree at the enclosing one, so that it would be read and its programs run —
    /// is refused as it is opened when the launching environment's configuration says
    /// `safe.bareRepository = explicit`, as that environment's own `git` refuses it, and
    /// nothing is served behind the refusal; without the setting it opens, as git does.
    /// Caught by: opening through `SharedRepository::discover`'s process environment
    /// rather than the launch's, or not at all through the git found.
    #[test]
    fn a_planted_bare_repository_is_refused_as_the_launchs_git_refuses_it() {
        let version = match cairn_git::ops::GitBinary::discover(&cairn_git::ops::Askpass::new(
            "/nonexistent/cairn-askpass",
            None,
        )) {
            Ok(git) => git.version(),
            Err(error) => panic!("finding git: {error}"),
        };
        if version.minor < 38 && version.major == 2 {
            eprintln!(
                "SKIPPED a_planted_bare_repository_is_refused_as_the_launchs_git_refuses_it: \
                 git {version} has no safe.bareRepository"
            );
            return;
        }
        let work = UnbornRepository::new("cairn-planted-bare");
        let planted = work.path.join("evil.git");
        for inside in ["objects/info", "objects/pack", "refs/heads", "refs/tags"] {
            std::fs::create_dir_all(planted.join(inside))
                .unwrap_or_else(|error| panic!("building the planted repository: {error}"));
        }
        std::fs::write(planted.join("HEAD"), "ref: refs/heads/main\n")
            .unwrap_or_else(|error| panic!("writing HEAD: {error}"));
        std::fs::write(
            planted.join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tworktree = ..\n",
        )
        .unwrap_or_else(|error| panic!("writing config: {error}"));
        let home = work.path.join("home");
        std::fs::create_dir_all(&home).unwrap_or_else(|error| panic!("a home: {error}"));
        let path = std::env::var_os("PATH");
        let launch = |explicit: bool| {
            let (path, home) = (path.clone(), home.clone().into_os_string());
            let global = if explicit {
                let file = work.path.join("explicit.gitconfig");
                std::fs::write(&file, "[safe]\n\tbareRepository = explicit\n")
                    .unwrap_or_else(|error| panic!("writing the setting: {error}"));
                file.into_os_string()
            } else {
                OsString::from("/dev/null")
            };
            Startup::new(
                move |name| match name {
                    "PATH" => path.clone(),
                    "HOME" => Some(home.clone()),
                    "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
                    "GIT_CONFIG_GLOBAL" => Some(global.clone()),
                    _ => None,
                },
                PathBuf::from("/nonexistent/cairn-askpass"),
            )
        };

        let (_handle, mut updates, _) = match open_with(&planted, launch(true)) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        match block_on(updates.next()) {
            Some(Update::Failed { message }) => assert!(
                message.contains("safe.bareRepository"),
                "the refusal does not say why: {message}"
            ),
            other => panic!("expected the refusal, got {other:?}"),
        }
        assert!(
            block_on(updates.next()).is_none(),
            "the worker went on to serve the repository after refusing it"
        );

        let (handle, mut updates, _) = match open_with(&planted, launch(false)) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        crate::worker::fetch_tests::opened_as(&mut updates);
        handle.submit(Request::OpenHistory {
            rows: 8,
            lost: false,
        });
        match after_refs(&mut updates) {
            Some(Update::Rows { rows, complete }) => assert!(rows.is_empty() && complete),
            other => panic!("without the setting the repository opens, got {other:?}"),
        }
        drop(handle);
    }

    /// A repository the launching environment's own `git` refuses for dubious ownership
    /// is refused as it is opened, in git's words, and nothing is served behind the
    /// refusal; with `safe.directory = *` it opens, as git does. The launch tells git to
    /// take every path as someone else's (`GIT_TEST_ASSUME_DIFFERENT_OWNER`, which git
    /// reads from 2.30.4 and Cairn reads as git does), so no second user is needed.
    /// Caught by: the open made with reduced trust and shown, as it was before the user
    /// decided to refuse it, or through `SharedRepository::discover`'s process environment
    /// rather than the launch's.
    #[test]
    fn a_repository_git_refuses_for_its_ownership_is_refused_as_git_refuses_it() {
        let version = match cairn_git::ops::GitBinary::discover(&cairn_git::ops::Askpass::new(
            "/nonexistent/cairn-askpass",
            None,
        )) {
            Ok(git) => git.version(),
            Err(error) => panic!("finding git: {error}"),
        };
        let reads_the_variable = version.major > 2
            || version.minor >= 36
            || matches!(
                (version.minor, version.patch),
                (30, 4..) | (31, 3..) | (32, 2..) | (33, 3..) | (34, 3..) | (35, 3..)
            );
        if !reads_the_variable {
            eprintln!(
                "SKIPPED a_repository_git_refuses_for_its_ownership_is_refused_as_git_refuses_it: \
                 git {version} does not read GIT_TEST_ASSUME_DIFFERENT_OWNER"
            );
            return;
        }
        let work = UnbornRepository::new("cairn-dubious-ownership");
        let path = std::env::var_os("PATH");
        let launch = |star: bool| {
            let path = path.clone();
            let parameters = star.then(|| OsString::from("'safe.directory=*'"));
            Startup::new(
                move |name| match name {
                    "PATH" => path.clone(),
                    "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
                    "GIT_CONFIG_GLOBAL" => Some(OsString::from("/dev/null")),
                    "GIT_TEST_ASSUME_DIFFERENT_OWNER" => Some(OsString::from("1")),
                    "GIT_CONFIG_PARAMETERS" => parameters.clone(),
                    _ => None,
                },
                PathBuf::from("/nonexistent/cairn-askpass"),
            )
        };

        let (_handle, mut updates, _) = match open_with(&work.path, launch(false)) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        match block_on(updates.next()) {
            Some(Update::Failed { message }) => assert!(
                message.contains("dubious ownership") && message.contains("safe.directory"),
                "the refusal does not say why, in git's words: {message}"
            ),
            other => panic!("expected the refusal, got {other:?}"),
        }
        assert!(
            block_on(updates.next()).is_none(),
            "the worker went on to serve the repository after refusing it"
        );

        if version.major == 2 && version.minor < 38 {
            // Before 2.38 git reads no `safe.directory` from the command line.
            return;
        }
        let (handle, mut updates, _) = match open_with(&work.path, launch(true)) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        crate::worker::fetch_tests::opened_as(&mut updates);
        handle.submit(Request::OpenHistory {
            rows: 8,
            lost: false,
        });
        match after_refs(&mut updates) {
            Some(Update::Rows { rows, complete }) => assert!(rows.is_empty() && complete),
            other => panic!("with safe.directory = * the repository opens, got {other:?}"),
        }
        drop(handle);
    }

    /// `open` succeeds for a path with no repository above it.
    #[test]
    fn opening_returns_before_the_repository_is_found() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        assert!(
            open_with(&outside, Startup::of_this_process()).is_ok(),
            "open() decided there was no repository, so it looked — on the caller's thread"
        );
    }

    /// Phase 05's hand-off: a repository with no ref and an unborn `HEAD` has a snapshot
    /// that seeds nothing, and its walk answers the empty, complete page the window draws as
    /// an empty history — what `from_head`'s unborn arm answered before the window walked
    /// from every ref. Caught by: an empty seed set failing the open, or a page that says
    /// more is coming.
    #[test]
    fn a_freshly_initialised_repository_reaches_the_view_as_an_empty_history() {
        let fixture = UnbornRepository::new("cairn-unborn-head");
        let (handle, mut updates, _) = match open_with(&fixture.path, Startup::of_this_process()) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        assert_eq!(
            crate::worker::fetch_tests::opened_as(&mut updates),
            "cairn-unborn-head"
        );
        handle.submit(Request::OpenHistory {
            rows: 8,
            lost: false,
        });

        match block_on(updates.next()) {
            Some(Update::Refs { snapshot, reopen }) => {
                assert!(!reopen);
                assert!(snapshot.refs.is_empty(), "{snapshot:?}");
                assert!(
                    matches!(snapshot.head, cairn_model::HeadState::Unborn(_)),
                    "{snapshot:?}"
                );
            }
            other => panic!("expected the open's own refs, got {other:?}"),
        }
        match after_refs(&mut updates) {
            Some(Update::Rows { rows, complete }) => {
                assert!(rows.is_empty(), "an unborn HEAD produced rows: {rows:?}");
                assert!(complete, "an empty history said more was coming");
            }
            other => panic!("expected an empty complete page, got {other:?}"),
        }
        drop(handle);
    }

    #[test]
    fn a_request_through_the_submitter_is_answered_with_rows() {
        let (handle, mut updates) = cairn();
        let before = updates.epochs.current(QueryLane::History);
        let submit = handle.into_submitter();
        submit(Request::OpenHistory {
            rows: 2,
            lost: false,
        });
        // Checked first: waiting for rows that were never asked for would hang.
        assert!(
            updates.epochs.current(QueryLane::History) > before,
            "the submitter did not submit"
        );
        match after_refs(&mut updates) {
            Some(Update::Rows { rows, .. }) => assert_eq!(rows.len(), 2),
            other => panic!("expected rows, got {other:?}"),
        }
        drop(submit);
    }

    #[test]
    fn a_request_is_answered_with_rows() {
        let (handle, mut updates) = cairn();
        handle.submit(Request::OpenHistory {
            rows: 3,
            lost: false,
        });
        match after_refs(&mut updates) {
            Some(Update::Rows { rows, .. }) => assert_eq!(rows.len(), 3),
            other => panic!("expected rows, got {other:?}"),
        }
        drop(handle);
    }

    /// Caught by: a failure leaving the worker unable to answer the request after it. The
    /// failure is a walk that cannot open — a `shallow` file that is not one — mended by
    /// removing it.
    #[test]
    fn a_failed_request_is_answered_when_it_is_asked_again() {
        let (checkout, mut checkout_updates) = cairn();
        checkout.submit(Request::OpenHistory {
            rows: 1,
            lost: false,
        });
        let tip = match after_refs(&mut checkout_updates) {
            Some(Update::Rows { rows, .. }) if rows.len() == 1 => rows.ids().next(),
            other => panic!("expected a row of this checkout, got {other:?}"),
        };
        let Some(tip) = tip else {
            panic!("the checkout has no first row");
        };
        let expected: Vec<String> =
            match Repository::discover(env!("CARGO_MANIFEST_DIR")).and_then(|repo| {
                repo.history(
                    &HistoryRequest::from_commits([tip], 3),
                    &CancelSignal::new(),
                )
            }) {
                Ok(page) => page.rows.ids().map(|id| id.to_string()).collect(),
                Err(error) => panic!("walking the checkout from {tip}: {error}"),
            };

        let fixture = BorrowedRepository::new("cairn-retried-request");
        fixture.point_main_at(&tip.to_string());
        let shallow = fixture.fixture.path.join(".git/shallow");
        std::fs::write(&shallow, "not a commit id\n")
            .unwrap_or_else(|error| panic!("writing {}: {error}", shallow.display()));
        let (handle, mut updates, _) =
            match open_with(&fixture.fixture.path, Startup::of_this_process()) {
                Ok(opened) => opened,
                Err(error) => panic!("starting the worker: {error}"),
            };
        crate::worker::fetch_tests::opened_as(&mut updates);

        handle.submit(Request::OpenHistory {
            rows: 3,
            lost: false,
        });
        match after_refs(&mut updates) {
            Some(Update::Failed { .. }) => {}
            other => panic!("expected a walk that cannot open to fail, got {other:?}"),
        }

        std::fs::remove_file(&shallow)
            .unwrap_or_else(|error| panic!("removing {}: {error}", shallow.display()));
        handle.submit(Request::MoreHistory { rows: 3 });
        match after_refs(&mut updates) {
            Some(Update::Rows { rows, .. }) => assert_eq!(
                rows.ids().map(|id| id.to_string()).collect::<Vec<_>>(),
                expected,
                "the retry did not deliver the history from the top"
            ),
            other => panic!("expected the retry to deliver rows, got {other:?}"),
        }
        drop(handle);
    }

    #[test]
    fn paging_continues_the_same_walk_rather_than_replaying_it() {
        let (handle, mut updates) = cairn();
        handle.submit(Request::OpenHistory {
            rows: 4,
            lost: false,
        });
        let first = match after_refs(&mut updates) {
            Some(Update::Rows { rows, .. }) => rows,
            other => panic!("expected rows, got {other:?}"),
        };
        handle.submit(Request::MoreHistory { rows: 4 });
        let second = match after_refs(&mut updates) {
            Some(Update::Rows { rows, .. }) => rows,
            other => panic!("expected rows, got {other:?}"),
        };

        assert_eq!(first.len(), 4);
        assert_eq!(second.len(), 4);
        let repeated = first.ids().any(|a| second.ids().any(|b| a == b));
        assert!(!repeated, "the second page repeated a row from the first");
    }

    /// The cancel a scroll and a find share, through the real boundary: one queued page
    /// asking for the whole of a long line of commits, superseded while it walks by a page
    /// asking for two, stops where it is, sends nothing, and the two rows answered are the
    /// walk's third and fourth — the walk taken up where the first page left it. The line is
    /// written for the test (not this checkout's history, #61), long enough that walking it
    /// takes far longer than the moment the supersession comes after; a round whose walk
    /// finished first (a slow machine) is tried again. Caught by: handing `next_page` a fresh
    /// `CancelSignal` instead of the epoch, or watching the walk lane's number rather than the
    /// page's — every round's walk then runs to the end. A page starting at the walk's first
    /// row is a reopened walk, a failure outright.
    #[test]
    fn superseding_a_request_stops_the_walk_that_is_serving_it() {
        let line = crate::worker::written_repository::WrittenRepository::linear(
            "cairn-superseded-walk",
            20_000,
        );
        let (handle, mut updates) = match open_with(line.path(), Startup::of_this_process()) {
            Ok((handle, mut updates, _)) => {
                crate::worker::fetch_tests::opened_as(&mut updates);
                (handle, updates)
            }
            Err(error) => panic!("opening the line: {error}"),
        };
        let whole = line.commits.len();
        let first_page = |updates: &mut Updates| match after_refs(updates) {
            Some(Update::Rows { rows, .. }) => rows.ids().collect::<Vec<_>>(),
            other => panic!("expected the first page of a scroll, got {other:?}"),
        };
        let queue_the_rest = |handle: &RepositoryHandle| {
            // Posted under its own number, as `submit` would, but held to one page of the rest.
            let epoch = handle.epochs.bump(QueryLane::History);
            let queued = handle.jobs.send((
                Some(epoch),
                RepositoryJob::History(Page::More { rows: whole }),
            ));
            assert!(queued.is_ok(), "the worker went away");
        };

        // Unsuperseded, for how long walking the rest takes on this machine.
        handle.submit(Request::OpenHistory {
            rows: 2,
            lost: false,
        });
        assert_eq!(first_page(&mut updates), line.commits[..2]);
        let started = Instant::now();
        queue_the_rest(&handle);
        match after_refs(&mut updates) {
            Some(Update::Rows {
                rows,
                complete: true,
            }) => assert_eq!(rows.len(), whole - 2),
            other => panic!("expected the rest of the line, got {other:?}"),
        }
        let rest = started.elapsed();
        let wait = rest / 5;

        let mut ran_on = 0usize;
        for _ in 0..20 {
            handle.submit(Request::OpenHistory {
                rows: 2,
                lost: false,
            });
            assert_eq!(first_page(&mut updates), line.commits[..2]);
            queue_the_rest(&handle);
            std::thread::sleep(wait);
            handle.submit(Request::MoreHistory { rows: 2 });
            match after_refs(&mut updates) {
                Some(Update::Rows { rows, .. }) if rows.len() == 2 => {
                    let ids: Vec<cairn_model::Oid> = rows.ids().collect();
                    assert_ne!(
                        ids.first(),
                        line.commits.first(),
                        "the next page began the walk again"
                    );
                    assert_eq!(
                        ids,
                        line.commits[2..4],
                        "the walk was not taken up where it was"
                    );
                    return;
                }
                Some(Update::Rows { rows, .. }) if rows.len() == whole - 2 => {
                    // The walk finished before the supersession: its page, then the empty one.
                    ran_on += 1;
                    match after_refs(&mut updates) {
                        Some(Update::Rows {
                            rows,
                            complete: true,
                        }) if rows.is_empty() => {}
                        other => panic!("expected the empty end of the walk, got {other:?}"),
                    }
                }
                other => panic!("expected the next page, got {other:?}"),
            }
        }

        panic!(
            "no supersession ever stopped a walk: {ran_on} rounds ran to the end of the line \
             anyway, superseded after {wait:?} (the whole rest took {rest:?}). A walk that \
             finishes after being superseded means the engine was handed a cancel signal that \
             is not the page's epoch — see `HistoryLane::answer`."
        );
    }

    /// The receiving half alone, with no repository behind it.
    fn inbox_only() -> (Epochs, Outbox, Updates) {
        let (sender, inbox) = channel::<Envelope>();
        let wake = Wake::new();
        let epochs = Epochs::new();
        let outbox = Outbox {
            updates: Some(sender),
            wake: Arc::clone(&wake),
        };
        (
            epochs.clone(),
            outbox,
            Updates {
                inbox,
                wake,
                epochs,
            },
        )
    }

    /// The stale answer is posted by hand, then superseded.
    #[test]
    fn an_answer_to_a_superseded_request_is_never_returned() {
        let (epochs, outbox, mut updates) = inbox_only();

        let stale = epochs.bump(QueryLane::History);
        outbox.send(
            Some(stale),
            Update::Failed {
                message: "stale".to_owned(),
            },
        );
        let fresh = epochs.bump(QueryLane::History);
        outbox.send(
            Some(fresh),
            Update::Failed {
                message: "fresh".to_owned(),
            },
        );

        match block_on(updates.next()) {
            Some(Update::Failed { message }) => assert_eq!(
                message, "fresh",
                "a superseded request's answer reached the view"
            ),
            other => panic!("expected the fresh answer, got {other:?}"),
        }
    }

    /// C8, "a superseded answer is never drawn", per lane: an answer that finished after
    /// its query was superseded is dropped on arrival — a file diff's by a newer file diff
    /// or by a changes query — while an answer in a lane nothing superseded arrives, though
    /// other lanes moved on after it. Posted by hand under real epochs, so the answer has
    /// certainly finished first. Mutations that redden it: `Updates::next` passing an
    /// update without asking whether its epoch is current; one counter for every lane.
    #[test]
    fn an_answer_superseded_in_its_lane_is_dropped_on_arrival() {
        let (epochs, outbox, mut updates) = inbox_only();
        let failed = |message: &str| Update::Failed {
            message: message.to_owned(),
        };

        let arrive = |updates: &mut Updates| {
            let mut arrived = Vec::new();
            // Everything posted is in the channel already; the stream is not ended, so the
            // first empty poll is where this round stops.
            let waker = std::task::Waker::noop();
            let mut cx = Context::from_waker(waker);
            loop {
                let mut next = std::pin::pin!(updates.next());
                match next.as_mut().poll(&mut cx) {
                    Poll::Ready(Some(Update::Failed { message })) => arrived.push(message),
                    Poll::Ready(other) => panic!("{other:?}"),
                    Poll::Pending => return arrived,
                }
            }
        };

        // A file diff, then a changes query, each answered before the other's answer is read.
        let page = epochs.bump(QueryLane::History);
        let file = epochs.bump(QueryLane::FileDiff);
        let changes = epochs.bump(QueryLane::Changes);
        outbox.send(Some(file), failed("a file diff a changes query superseded"));
        outbox.send(Some(page), failed("page"));
        outbox.send(Some(changes), failed("changes"));
        assert_eq!(arrive(&mut updates), ["page", "changes"]);

        // Two file diffs; the page and the changes query, asked before them, still current.
        let first = epochs.bump(QueryLane::FileDiff);
        let second = epochs.bump(QueryLane::FileDiff);
        outbox.send(Some(first), failed("a file diff a file diff superseded"));
        outbox.send(Some(second), failed("the file diff asked last"));
        outbox.send(Some(changes), failed("changes, again"));
        outbox.send(Some(page), failed("page, again"));
        assert_eq!(
            arrive(&mut updates),
            ["the file diff asked last", "changes, again", "page, again"]
        );
    }

    /// R1 (phase 07 QA): a superseded answer that holds something large — a file's
    /// prepared diff, a change set, Expand All's batch — is not dropped here, on the task
    /// the UI thread drives, but handed back as `Update::Superseded` for the window to send
    /// to a worker to free; one holding nothing worth a worker is still dropped, and the
    /// current answer still arrives after it. Caught by: a stale answer dropped in place,
    /// or handed back drawn as if current.
    #[test]
    fn a_superseded_answer_comes_back_to_be_freed_on_a_worker() {
        use cairn_model::{
            ChangeSet, ChangeStatus, ChangedFile, DiffContent, FileDiff, Oid, RenameDetection,
            RepoPath, ShownDiff,
        };

        use crate::worker::request::{Comparison, DiffOptions, FileQuery, FileTarget};

        let (epochs, outbox, mut updates) = inbox_only();
        let of = Comparison::Commit(Oid::from_bytes(&[1; 20]).unwrap());
        let file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("a.txt"),
            new_path: RepoPath::from("a.txt"),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        };
        let diff = FileDiff {
            file: file.clone(),
            content: DiffContent::ModeChangeOnly,
        };
        let shown = ShownDiff::new(diff.clone(), DiffOptions::default().context);
        let query = FileQuery {
            target: FileTarget::Committed { of, file },
            options: DiffOptions::default(),
        };
        let changes = ChangeSet {
            files: Vec::new(),
            details: None,
            renames: RenameDetection::default(),
        };

        let stale_file = epochs.bump(QueryLane::FileDiff);
        let stale_changes = epochs.bump(QueryLane::Changes);
        outbox.send(
            Some(stale_file),
            Update::FileDiff {
                query: query.clone(),
                diff: Some(Box::new(shown.clone())),
            },
        );
        outbox.send(
            Some(stale_file),
            Update::Expanded {
                of,
                options: DiffOptions::default(),
                files: vec![crate::worker::ExpandedFile {
                    file: crate::worker::OpenedFile {
                        index: 0,
                        load_anyway: false,
                    },
                    by_all: true,
                    outcome: Ok(Box::new(shown.clone())),
                }],
                all: None,
            },
        );
        outbox.send(
            Some(stale_file),
            Update::Failed {
                message: "nothing to free".to_owned(),
            },
        );
        outbox.send(
            Some(stale_changes),
            Update::Changes {
                of,
                changes: changes.clone(),
            },
        );
        let fresh = epochs.bump(QueryLane::Changes);
        outbox.send(
            Some(fresh),
            Update::Changes {
                of,
                changes: changes.clone(),
            },
        );

        match block_on(updates.next()) {
            Some(Update::Superseded(retired)) => {
                assert_eq!(retired.shown(), std::slice::from_ref(&shown))
            }
            other => panic!("expected the stale file diff handed back, got {other:?}"),
        }
        match block_on(updates.next()) {
            Some(Update::Superseded(retired)) => {
                assert_eq!(retired.shown(), std::slice::from_ref(&shown))
            }
            other => panic!("expected the stale page handed back, got {other:?}"),
        }
        match block_on(updates.next()) {
            Some(Update::Superseded(retired)) => {
                assert_eq!(retired.changes(), Some(&changes));
            }
            other => panic!("expected the stale change set handed back, got {other:?}"),
        }
        match block_on(updates.next()) {
            Some(Update::Changes { .. }) => {}
            other => panic!("expected the current change set, got {other:?}"),
        }
    }

    /// R11.3: a refresh's superseded answers — a refs snapshot, ahead/behind, a status — come
    /// back to be freed on a worker, never dropped on the task the UI thread drives; the
    /// current refresh's arrive as they are. Posted by hand under real epochs. Caught by:
    /// `Update::into_retired` dropping any of the three.
    #[test]
    fn a_superseded_refresh_comes_back_to_be_freed_on_a_worker() {
        use cairn_model::{
            AheadBehind, HeadState, RefName, RefsSnapshot, StatusEntry, WorkingTreeStatus,
        };

        let (epochs, outbox, mut updates) = inbox_only();
        let snapshot = Arc::new(RefsSnapshot {
            refs: Vec::new(),
            head: HeadState::Unborn(RefName::new("refs/heads/main")),
            stashes: Vec::new(),
            unreadable: 0,
        });
        let counts = vec![(
            RefName::new("refs/heads/main"),
            AheadBehind {
                ahead: 1,
                behind: 2,
            },
        )];
        let status = WorkingTreeStatus::Listed(vec![StatusEntry::Untracked(
            cairn_model::RepoPath::from("new.txt"),
        )]);
        let send = |epochs: &Epochs, outbox: &Outbox| {
            let refs = epochs.bump(QueryLane::Refs);
            let ahead_behind = epochs.bump(QueryLane::AheadBehind);
            let read = epochs.bump(QueryLane::Status);
            outbox.send(
                Some(refs),
                Update::Refs {
                    snapshot: Arc::clone(&snapshot),
                    reopen: true,
                },
            );
            outbox.send(
                Some(ahead_behind),
                Update::AheadBehind {
                    counts: counts.clone(),
                },
            );
            outbox.send(
                Some(read),
                Update::Status {
                    changes: Arc::new(cairn_model::LocalChanges::new(status.clone())),
                },
            );
        };
        send(&epochs, &outbox);
        send(&epochs, &outbox);

        let retired: Vec<Retired> = (0..3)
            .map(|_| match block_on(updates.next()) {
                Some(Update::Superseded(retired)) => retired,
                other => panic!("expected a superseded answer handed back, got {other:?}"),
            })
            .collect();
        assert_eq!(retired[0].refs_snapshot(), Some(&*snapshot));
        assert_eq!(retired[1].counts(), counts.as_slice());
        assert_eq!(retired[2].working_tree_status(), Some(&status));
        for _ in 0..3 {
            match block_on(updates.next()) {
                Some(Update::Refs { .. } | Update::AheadBehind { .. } | Update::Status { .. }) => {}
                other => panic!("expected the current refresh's answer, got {other:?}"),
            }
        }
    }

    /// The negative for the test above: dropping everything must fail it.
    #[test]
    fn an_answer_to_the_current_request_is_returned() {
        let (epochs, outbox, mut updates) = inbox_only();
        let mine = epochs.bump(QueryLane::History);
        outbox.send(
            Some(mine),
            Update::Failed {
                message: "mine".to_owned(),
            },
        );
        match block_on(updates.next()) {
            Some(Update::Failed { message }) => assert_eq!(message, "mine"),
            other => panic!("expected the current answer, got {other:?}"),
        }
    }

    #[test]
    fn a_worker_dying_is_reported_whatever_the_epoch_is() {
        let (epochs, outbox, mut updates) = inbox_only();
        for lane in QueryLane::ALL {
            epochs.bump(lane);
        }
        outbox.send(
            None,
            Update::WorkerLost {
                message: "gone".to_owned(),
            },
        );
        for lane in QueryLane::ALL {
            epochs.bump(lane);
        }

        match block_on(updates.next()) {
            Some(Update::WorkerLost { message }) => assert_eq!(message, "gone"),
            other => panic!("expected the worker notice, got {other:?}"),
        }
    }

    #[test]
    fn nothing_is_rendered_once_the_pool_is_stopping() {
        let (epochs, outbox, mut updates) = inbox_only();
        let mine = epochs.bump(QueryLane::History);
        epochs.stop();
        outbox.send(
            Some(mine),
            Update::Failed {
                message: "too late".to_owned(),
            },
        );
        outbox.send(
            None,
            Update::WorkerLost {
                message: "gone".to_owned(),
            },
        );
        match block_on(updates.next()) {
            Some(Update::WorkerLost { .. }) => {}
            other => panic!("a stopping pool still rendered {other:?}"),
        }
    }

    /// Drives a real panic through the `Drop` notice.
    #[test]
    fn a_panicking_worker_says_so_instead_of_disappearing() {
        let (updates_tx, inbox) = channel::<Envelope>();
        let wake = Wake::new();
        let outbox = Outbox {
            updates: Some(updates_tx),
            wake: Arc::clone(&wake),
        };

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let died = std::thread::spawn(move || {
            let _exit = WorkerExit {
                outbox: Some(outbox),
                wake,
            };
            panic!("a pack file went missing mid-walk");
        })
        .join();
        std::panic::set_hook(previous);

        assert!(died.is_err(), "the worker was supposed to panic");
        match inbox.try_recv() {
            Ok(Envelope {
                epoch: None,
                update: Update::WorkerLost { message },
            }) => assert!(!message.is_empty(), "an empty notice tells nobody anything"),
            other => panic!("a panicking worker announced {other:?}"),
        }
    }

    /// Caught by: `open` not installing a [`WorkerExit`] around [`serve`].
    /// The waker panics exactly once; a second panic while sending the notice would abort.
    #[test]
    fn a_panic_inside_a_real_worker_is_announced_by_the_pool_that_opened_it() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct GivesWayOnce {
            waiting: std::thread::Thread,
            spent: AtomicBool,
        }

        impl std::task::Wake for GivesWayOnce {
            fn wake(self: Arc<Self>) {
                self.wake_by_ref();
            }

            fn wake_by_ref(self: &Arc<Self>) {
                // `spent` first, then the unpark, then the panic: the waiting thread must be woken either way.
                let first = !self.spent.swap(true, Ordering::SeqCst);
                self.waiting.unpark();
                if first {
                    panic!("the window's waker gave way mid-answer");
                }
            }
        }

        let (handle, updates) = cairn();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        // Everything that can park runs over there, so the timeout below bounds the whole test.
        let (told, verdict) = channel::<Result<String, String>>();
        let asking = handle.clone();
        std::thread::spawn(move || {
            let mut updates = updates;
            let fragile = Arc::new(GivesWayOnce {
                waiting: std::thread::current(),
                spent: AtomicBool::new(false),
            });
            let waker = Waker::from(Arc::clone(&fragile));
            let mut asked = 0;

            // Ask until an answer's wake runs the waker: a signal before this thread parks only latches.
            let announced = loop {
                if !fragile.spent.load(Ordering::SeqCst) {
                    if asked >= 100 {
                        break Err(format!("{asked} answers arrived without ever waking"));
                    }
                    asked += 1;
                    asking.submit(Request::OpenHistory {
                        rows: 1,
                        lost: false,
                    });
                }
                match woken_by(&waker, updates.next()) {
                    Some(Update::Rows { .. } | Update::Refs { .. }) => {}
                    Some(Update::WorkerLost { message }) => break Ok(message),
                    other => {
                        break Err(format!(
                            "a worker that panicked inside `serve` announced {other:?}"
                        ));
                    }
                }
            };
            let _ = told.send(
                announced.and_then(|message| match block_on(updates.next()) {
                    None => Ok(message),
                    left => Err(format!(
                        "the stream outlived the worker that panicked inside it: {left:?}"
                    )),
                }),
            );
        });

        // Redden rather than hang the suite.
        let waited = verdict.recv_timeout(std::time::Duration::from_secs(10));
        std::panic::set_hook(previous);

        match waited {
            Ok(Ok(message)) => {
                assert!(!message.is_empty(), "an empty notice tells nobody anything");
            }
            Ok(Err(why)) => panic!("{why}"),
            Err(_) => panic!(
                "ten seconds after a panic inside `serve`, the stream had neither announced \
                 the worker nor ended: whoever drives `Updates` is parked with nothing coming"
            ),
        }
        drop(handle);
    }

    /// The negative for the two above.
    #[test]
    fn a_worker_that_exits_cleanly_raises_no_alarm() {
        let (updates_tx, inbox) = channel::<Envelope>();
        let wake = Wake::new();
        drop(WorkerExit {
            outbox: Some(Outbox {
                updates: Some(updates_tx),
                wake: Arc::clone(&wake),
            }),
            wake,
        });
        match inbox.try_recv() {
            Err(TryRecvError::Disconnected) => {}
            other => panic!("a clean shutdown left {other:?} behind"),
        }
    }

    /// Overlaps the blanket impl, a compile error, exactly when `Outbox` is `Clone`.
    trait OneSenderPerWorker {}
    impl OneSenderPerWorker for Outbox {}
    impl<T: Clone> OneSenderPerWorker for T {}

    /// Passes by compiling: deriving `Clone` on [`Outbox`] stops the test build.
    #[test]
    fn a_worker_thread_cannot_be_given_a_second_sender() {
        fn one_sender_only<T: OneSenderPerWorker>() {}
        one_sender_only::<Outbox>();
    }

    /// The stream must end after a failed open, not merely report the failure.
    #[test]
    fn a_failed_open_ends_the_stream_rather_than_leaving_it_open() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        let (_handle, mut updates, _) = match open_with(&outside, Startup::of_this_process()) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        match block_on(updates.next()) {
            Some(Update::Failed { .. }) => {}
            other => panic!("expected the open to be reported, got {other:?}"),
        }
        assert!(
            block_on(updates.next()).is_none(),
            "the stream stayed open after the worker that failed to open had gone"
        );
    }

    /// Closing a channel does not wake a `Wake`.
    #[test]
    fn a_worker_ending_in_silence_still_wakes_the_waiting_task() {
        let (_epochs, outbox, mut updates) = inbox_only();
        let wake = Arc::clone(&updates.wake);
        let exit = WorkerExit {
            outbox: Some(outbox),
            wake,
        };
        std::thread::spawn(move || drop(exit));
        assert!(
            block_on(updates.next()).is_none(),
            "the stream did not end when its last sender went"
        );
    }

    /// Issue #21, the mechanism: on the worker's early exits (`git` missing, no repository)
    /// two outboxes were dropped as closure captures AFTER the `WorkerExit` had signalled,
    /// so a task parked between the two never woke. The outbox itself must wake on drop,
    /// with no `WorkerExit` around it. Single-threaded on purpose: the task is parked
    /// (one poll, `Pending`) BEFORE the drop, so the old code fails on the wake count
    /// every time rather than only when the scheduler let the driver park first.
    #[test]
    fn an_outbox_dropped_anywhere_ends_the_stream() {
        struct Woke(std::sync::atomic::AtomicUsize);
        impl std::task::Wake for Woke {
            fn wake(self: Arc<Self>) {
                self.wake_by_ref();
            }
            fn wake_by_ref(self: &Arc<Self>) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let (_epochs, outbox, mut updates) = inbox_only();
        let woke = Arc::new(Woke(std::sync::atomic::AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&woke));
        let mut cx = Context::from_waker(&waker);
        let mut next = std::pin::pin!(updates.next());
        assert!(
            next.as_mut().poll(&mut cx).is_pending(),
            "an empty stream with a live sender was ready"
        );
        drop(outbox);
        assert_eq!(
            woke.0.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the last outbox closed without waking the task parked on the stream"
        );
        assert!(
            matches!(next.as_mut().poll(&mut cx), Poll::Ready(None)),
            "woken, the task did not see the stream end"
        );
    }

    /// A close stops the epochs as it is submitted, on the caller's thread, so a page
    /// being walked is abandoned at its next poll and the close behind it is reached at
    /// once — and it is still queued, for a worker that is between requests. Caught by:
    /// a close that only queues, which waits behind whatever page is being walked.
    #[test]
    fn a_close_stops_the_epochs_as_it_is_submitted_and_is_queued() {
        let (handle, asked) = idle_handle();
        assert!(!handle.epochs.is_stopping());
        handle.submit(Request::Close);
        assert!(
            handle.epochs.is_stopping(),
            "the close left the walk in progress to finish its page"
        );
        assert_eq!(asked(), vec![Request::Close], "the close was not queued");
    }

    /// Staging-and-commit R4.9: a close marks the local lane closing as it is submitted, so a
    /// write asked after it is never started — even while the repository thread, which marks
    /// it again as it closes, is still busy with what was queued before. Caught by: marking
    /// the lane only in `Threads::drop`, which leaves an idle lane to start a write asked
    /// between the close and the repository thread reaching it.
    /// Phase 09's QA item 9: asking a write supersedes the amend read in flight or queued, so
    /// a stage never waits on a stale amend's walk — its ending reads status, over which the
    /// window asks again; nothing else a write is asked beside is superseded. Caught by: a
    /// write that leaves the amend read current.
    #[test]
    fn a_write_supersedes_the_amend_read_and_nothing_else() {
        let (handle, _asked) = idle_handle();
        let status = Arc::new(cairn_model::LocalChanges::new(
            cairn_model::WorkingTreeStatus::Listed(Vec::new()),
        ));
        let amending = handle.submit(Request::Amending { status });
        let reads = handle.submit(Request::CommitReads);
        let (Some(amending), Some(reads)) = (amending, reads) else {
            panic!("the reads were numbered in no lane");
        };
        handle.submit(Request::Write {
            id: OperationId::next(),
            write: LocalWrite::StageFiles {
                paths: vec![cairn_model::RepoPath::from("a")],
            },
        });
        assert!(
            !handle.epochs.is_current(amending),
            "a stage left the amend read current"
        );
        assert!(
            handle.epochs.is_current(reads),
            "a stage superseded the box's reads"
        );
    }

    #[test]
    fn a_close_marks_the_local_lane_closing_as_it_is_submitted() {
        let (handle, _asked) = idle_handle();
        assert!(!handle.lane.is_closing());
        handle.submit(Request::Close);
        assert!(
            handle.lane.is_closing(),
            "a write asked after the close could still start"
        );
    }

    #[test]
    fn submitting_returns_immediately_even_with_nobody_serving() {
        let (jobs, incoming) = channel::<(Option<Epoch>, RepositoryJob)>();
        let (diff, diff_incoming) = channel::<DiffJob>();
        let (refresh, refresh_incoming) = channel::<RefreshJob>();
        let (local, local_incoming) = channel::<LocalJob>();
        drop((incoming, diff_incoming, refresh_incoming, local_incoming));
        let epochs = Epochs::new();
        let handle = RepositoryHandle {
            jobs,
            diff,
            refresh,
            local,
            lane: LaneState::default(),
            epochs: epochs.clone(),
            control: FetchControl::default(),
        };
        let started = Instant::now();
        let epoch = handle.submit(Request::OpenHistory {
            rows: 1_000_000,
            lost: false,
        });
        let changes = handle.submit(Request::Changes {
            of: crate::worker::Comparison::Commit(cairn_model::Oid::from_bytes(&[3; 20]).unwrap()),
        });
        assert!(
            started.elapsed() < std::time::Duration::from_millis(50),
            "submitting waited for {:?}",
            started.elapsed()
        );
        assert_eq!(
            Some(epochs.current(QueryLane::History)),
            epoch,
            "the page was not numbered"
        );
        assert_eq!(
            Some(epochs.current(QueryLane::Changes)),
            changes,
            "the diff was not numbered"
        );
        assert_eq!(
            handle.submit(Request::ListRemotes),
            None,
            "an operation was numbered"
        );
        let refreshed = handle.submit(Request::Refresh);
        assert_eq!(refreshed, Some(epochs.current(QueryLane::Refs)));
        assert!(
            started.elapsed() < std::time::Duration::from_millis(50),
            "a refresh waited for {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn the_stream_ends_when_every_worker_has_gone() {
        let (handle, mut updates) = cairn();
        drop(handle);
        assert!(
            block_on(updates.next()).is_none(),
            "the stream outlived its workers"
        );
    }
}
