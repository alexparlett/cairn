//! The repository worker pool: the threads that touch the repository, and the
//! values that cross back to the window.
//!
//! Four threads per open repository, started by the first, each with its own
//! [`Outbox`] so the update stream ends only when every one of them has gone:
//!
//! - `cairn-repository` (this file): takes the `git` the application found as
//!   it started ([`Discovery`]), opens the repository and its askpass channel,
//!   serves the history lane and the command log, forwards each operation to
//!   its write lane, and on its way out closes the repository — every `git`
//!   in it ended and reaped, up to `CLOSE_BOUND` — and stops the other three.
//! - `cairn-diff` (`diff_lane.rs`): the changes and file-diff lanes, so a
//!   page and a diff never queue behind each other.
//! - `cairn-network` (`network_lane.rs`): the network lane, where fetch runs,
//!   so it blocks neither the walk nor the window.
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
use std::time::Duration;

use cairn_git::{
    CLOSE_BOUND, Error, HistoryCursor, HistoryRequest, HistorySession, Repository, SharedRepository,
};

use super::askpass::{AcceptorStop, Reply, STOP_DEADLINE, serve_prompts};
use super::diff_lane::{DiffJob, Serving, serve_diffs};
use super::discovery::Discovery;
use super::epoch::{Epoch, Epochs};
use super::network_lane::{FetchControl, Lane, Operation, serve_network_lane};
use super::request::{Request, Update};
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
/// always be closed. It does not bound the network lane's ref scans, which
/// run before and after a fetch and cannot be cancelled: a repository with a
/// great many refs can hold the stream's end past it.
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
            updates: Some(outgoing),
            wake: Arc::clone(&wake),
        },
        epochs: epochs.clone(),
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
            // This repository's channel, and git pointed at it.
            let backend = Backend::open(discovery.startup(), &git);
            let threads = Threads::start(
                backend,
                Arc::clone(&shared),
                answered,
                (network_outbox, acceptor_outbox),
                diff,
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

/// The three threads the repository thread starts, how it reaches them, and
/// the repository they run `git` in, which stopping them closes.
struct Threads {
    /// The network lane's queue; one lane per [`Lane`], one lane so far.
    network: Option<Sender<Operation>>,
    /// Told to stop as the repository closes: the handles hold the diff
    /// thread's queue open, so it is not ended by this thread going.
    diff: Sender<DiffJob>,
    control: FetchControl,
    acceptor: Option<AcceptorStop>,
    shared: Arc<SharedRepository>,
}

impl Threads {
    fn start(
        backend: Backend,
        shared: Arc<SharedRepository>,
        answered: Receiver<Reply>,
        (network_outbox, acceptor_outbox): (Outbox, Outbox),
        diff: DiffThread,
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
            control,
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
    /// the network lane's queue is closed;
    /// then every `git` running in the repository — a fetch in flight, one
    /// mid-spawn, one dropped to a reaper — is ended the way a cancel ends
    /// it, and this waits up to `CLOSE_BOUND` for their reaps, here on the
    /// repository thread. One that enters the registry afterwards — a fetch
    /// already forwarded to the lane's queue when the close came, which the
    /// lane takes up after it — is ended as it enters. (A fetch still on this
    /// thread's own queue is never forwarded: `serve` stopped with the
    /// epochs.) The registry is the one authority here: a fetch
    /// is ended by it, not by its cancel, so nothing it misses is ended by
    /// accident. Last, the acceptor is woken to see it should stop. Runs once
    /// the window has asked to close or let go (the acceptor's answering end
    /// may still be held then: a prompt still waiting is withdrawn by the
    /// window when its fetch's outcome arrives) and on unwinding alike.
    fn drop(&mut self) {
        // First, so the diff thread takes up nothing more; what it is running is
        // ended with the rest below, or by its epoch if the close stopped them.
        let _ = self.diff.send(DiffJob::Stop);
        self.network = None;
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
    /// and the close behind them is reached at once; the waiting it starts is
    /// the repository thread's.
    pub fn submit(&self, request: Request) -> Option<Epoch> {
        // Numbered before it is routed, so what it supersedes is cancelled now,
        // not when a thread gets to it.
        let epoch = request.lane().map(|lane| self.epochs.bump(lane));
        // A failed send means the worker is gone and has already said so.
        match route(request) {
            Routed::CancelFetch => self.control.cancel(),
            Routed::Repository(job) => {
                if matches!(job, RepositoryJob::Close) {
                    self.epochs.stop();
                }
                let _ = self.jobs.send((epoch, job));
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
    let handle = RepositoryHandle {
        jobs,
        diff,
        epochs: Epochs::new(),
        control: FetchControl::default(),
    };
    (handle, move || {
        let repository = incoming
            .try_iter()
            .map(|(_, job)| unroute(Routed::Repository(job)));
        let diffs = diff_incoming.try_iter().filter_map(|job| match job {
            DiffJob::Query { query, .. } => Some(unroute(Routed::Diff(*query))),
            DiffJob::Stop => None,
        });
        repository.chain(diffs).collect()
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
    /// Epochless updates are never dropped.
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
    let mut scroll = Scroll::default();

    while let Ok((epoch, job)) = jobs.recv() {
        if epochs.is_stopping() {
            break;
        }
        match job {
            RepositoryJob::History(page) => match epoch {
                Some(epoch) if epochs.is_current(epoch) => {
                    scroll.page(&repo, page, epoch, &epochs, outbox);
                }
                // Superseded before it was picked up, and never started. (Every page is
                // numbered: `submit` numbers what is in a lane.)
                _ => {}
            },
            RepositoryJob::ListRemotes => outbox.send(
                None,
                Update::Remotes {
                    remotes: repo.remotes(),
                },
            ),
            // A value git refuses sends nothing: the views keep git's default, and every
            // diff asked fails with the configuration's error, as the user's `git diff` does.
            RepositoryJob::ConfiguredContext => {
                if let Ok(context) = repo.configured_context() {
                    outbox.send(None, Update::ConfiguredContext { context });
                }
            }
            RepositoryJob::Fetch { remote } => {
                threads.perform(Operation::Fetch { remote }, outbox);
            }
            RepositoryJob::CommandLog => outbox.send(
                None,
                Update::CommandLog {
                    records: shared.command_log(),
                },
            ),
            // Freed here, off the UI thread, which is the whole of the job.
            RepositoryJob::Retire(retired) => drop(retired),
            // The epochs were stopped as it was sent; the closing is the caller's.
            RepositoryJob::Close => break,
        }
    }
}

/// The history lane's state: the live walk, and the cursor a cold restart resumes from.
#[derive(Default)]
struct Scroll<'repo> {
    session: Option<HistorySession<'repo>>,
    /// `None` means the scroll has not started.
    cursor: Option<HistoryCursor>,
}

impl<'repo> Scroll<'repo> {
    /// Answers one page under `epoch`, which is also what cancels it.
    fn page(
        &mut self,
        repo: &'repo Repository,
        page: Page,
        epoch: Epoch,
        epochs: &Epochs,
        outbox: &Outbox,
    ) {
        let rows = match page {
            Page::Open { rows } => {
                // A different scroll: drop the open walk first. After a fetch this is
                // also what honours `Invalidated::refs`: the new walk starts from the
                // refs as they are now.
                self.session = None;
                self.cursor = None;
                rows
            }
            Page::More { rows } => rows,
        };

        if self.session.is_none() {
            // Cold restart from the last good page.
            let request = match self.cursor.clone() {
                Some(at) => HistoryRequest::resume(at, rows),
                None => HistoryRequest::from_head(rows),
            };
            match repo.history_session(&request) {
                Ok(session) => self.session = Some(session),
                Err(error) => {
                    outbox.send(Some(epoch), no_walk(error));
                    return;
                }
            }
        }

        let Some(session) = self.session.as_mut() else {
            return;
        };
        match session.next_page(rows, &epochs.watch(epoch)) {
            Ok(page) => {
                let complete = page.cursor.is_none();
                self.cursor = page.cursor;
                outbox.send(
                    Some(epoch),
                    Update::Rows {
                        rows: page.rows,
                        complete,
                    },
                );
            }
            Err(Error::Cancelled { .. }) => {
                // Superseded mid-page: the session keeps its rows. Nothing is sent.
            }
            Err(error) => {
                // Drop the session; the next request cold-restarts from the last good cursor.
                self.session = None;
                outbox.send(
                    Some(epoch),
                    Update::Failed {
                        message: error.to_string(),
                    },
                );
            }
        }
    }
}

/// `Error::UnbornHead` becomes a complete, empty page rather than a failure.
fn no_walk(error: Error) -> Update {
    match error {
        Error::UnbornHead { .. } => Update::Rows {
            rows: Vec::new(),
            complete: true,
        },
        other => Update::Failed {
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::task::{Context, Poll, Waker};
    use std::time::Instant;

    use crate::worker::epoch::QueryLane;
    use crate::worker::fetch_tests::{BorrowedRepository, UnbornRepository, block_on, woken_by};

    /// The Cairn checkout itself, opened through the real boundary.
    fn cairn() -> (RepositoryHandle, Updates) {
        match open_with(env!("CARGO_MANIFEST_DIR"), Startup::of_this_process()) {
            Ok((handle, updates, _)) => (handle, updates),
            Err(error) => panic!("opening the Cairn checkout: {error}"),
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
        handle.submit(Request::OpenHistory { rows: 8 });
        match block_on(updates.next()) {
            Some(Update::Rows { rows, complete }) => assert!(rows.is_empty() && complete),
            other => panic!("without the setting the repository opens, got {other:?}"),
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

    /// Caught by: deleting the unborn arm at the call site while `no_walk` stays correct.
    #[test]
    fn a_freshly_initialised_repository_reaches_the_view_as_an_empty_history() {
        let fixture = UnbornRepository::new("cairn-unborn-head");
        let (handle, mut updates, _) = match open_with(&fixture.path, Startup::of_this_process()) {
            Ok(opened) => opened,
            Err(error) => panic!("starting the worker: {error}"),
        };
        handle.submit(Request::OpenHistory { rows: 8 });

        match block_on(updates.next()) {
            Some(Update::Rows { rows, complete }) => {
                assert!(rows.is_empty(), "an unborn HEAD produced rows: {rows:?}");
                assert!(complete, "an empty history said more was coming");
            }
            other => panic!("expected an empty complete page, got {other:?}"),
        }
        drop(handle);
    }

    #[test]
    fn a_repository_with_no_commits_yet_arrives_as_an_empty_history() {
        let answer = no_walk(Error::UnbornHead {
            path: PathBuf::from("/tmp/fresh"),
        });
        assert_eq!(
            answer,
            Update::Rows {
                rows: Vec::new(),
                complete: true
            },
            "an unborn HEAD was reported as a failure"
        );
    }

    #[test]
    fn every_other_failure_to_open_a_walk_keeps_its_sentence() {
        let answer = no_walk(Error::NotARepository {
            path: PathBuf::from("/tmp/nowhere"),
        });
        match answer {
            Update::Failed { message } => assert!(
                message.contains("/tmp/nowhere"),
                "the message does not name the path: {message}"
            ),
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    #[test]
    fn a_request_through_the_submitter_is_answered_with_rows() {
        let (handle, mut updates) = cairn();
        let before = updates.epochs.current(QueryLane::History);
        let submit = handle.into_submitter();
        submit(Request::OpenHistory { rows: 2 });
        // Checked first: waiting for rows that were never asked for would hang.
        assert!(
            updates.epochs.current(QueryLane::History) > before,
            "the submitter did not submit"
        );
        match block_on(updates.next()) {
            Some(Update::Rows { rows, .. }) => assert_eq!(rows.len(), 2),
            other => panic!("expected rows, got {other:?}"),
        }
        drop(submit);
    }

    #[test]
    fn a_request_is_answered_with_rows() {
        let (handle, mut updates) = cairn();
        handle.submit(Request::OpenHistory { rows: 3 });
        match block_on(updates.next()) {
            Some(Update::Rows { rows, .. }) => assert_eq!(rows.len(), 3),
            other => panic!("expected rows, got {other:?}"),
        }
        drop(handle);
    }

    /// Caught by: a failure leaving the worker unable to answer the request after it.
    #[test]
    fn a_failed_request_is_answered_when_it_is_asked_again() {
        let (checkout, mut checkout_updates) = cairn();
        checkout.submit(Request::OpenHistory { rows: 3 });
        let expected: Vec<String> = match block_on(checkout_updates.next()) {
            Some(Update::Rows { rows, .. }) if rows.len() == 3 => {
                rows.iter().map(|row| row.graph.id.to_string()).collect()
            }
            other => panic!("expected three rows of this checkout, got {other:?}"),
        };

        let fixture = BorrowedRepository::new("cairn-retried-request");
        fixture.point_main_at(&"1".repeat(40));
        let (handle, mut updates, _) =
            match open_with(&fixture.fixture.path, Startup::of_this_process()) {
                Ok(opened) => opened,
                Err(error) => panic!("starting the worker: {error}"),
            };

        handle.submit(Request::OpenHistory { rows: 3 });
        match block_on(updates.next()) {
            Some(Update::Failed { .. }) => {}
            other => panic!("expected a missing HEAD commit to fail, got {other:?}"),
        }

        fixture.point_main_at(&expected[0]);
        handle.submit(Request::MoreHistory { rows: 3 });
        match block_on(updates.next()) {
            Some(Update::Rows { rows, .. }) => assert_eq!(
                rows.iter()
                    .map(|row| row.graph.id.to_string())
                    .collect::<Vec<_>>(),
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
        handle.submit(Request::OpenHistory { rows: 4 });
        let first = match block_on(updates.next()) {
            Some(Update::Rows { rows, .. }) => rows,
            other => panic!("expected rows, got {other:?}"),
        };
        handle.submit(Request::MoreHistory { rows: 4 });
        let second = match block_on(updates.next()) {
            Some(Update::Rows { rows, .. }) => rows,
            other => panic!("expected rows, got {other:?}"),
        };

        assert_eq!(first.len(), 4);
        assert_eq!(second.len(), 4);
        let repeated = first
            .iter()
            .any(|a| second.iter().any(|b| a.id() == b.id()));
        assert!(!repeated, "the second page repeated a row from the first");
    }

    /// Caught by: handing `next_page` a fresh `CancelSignal` instead of `epochs.watch(epoch)`.
    /// A supersession landing between two batched requests looks like completion, so a round
    /// that sees one retries; the failure is every round seeing one.
    #[test]
    fn superseding_a_request_stops_the_walk_that_is_serving_it() {
        // Larger than this repository, so every request below would walk the whole history.
        let whole_history = 1_000_000;
        // Queued work, so the worker is still busy when the supersession arrives.
        let batch = 2_000;
        let (handle, mut updates) = cairn();

        // One ordinary round trip, to name the opening row and time an answer on this machine.
        let started = Instant::now();
        handle.submit(Request::OpenHistory { rows: 2 });
        let opened_on = match block_on(updates.next()) {
            Some(Update::Rows { rows, .. }) if !rows.is_empty() => rows[0].id(),
            other => panic!("expected the first page of a scroll, got {other:?}"),
        };
        let answer = started.elapsed();
        let wait = (answer * 2).clamp(
            std::time::Duration::from_millis(1),
            std::time::Duration::from_millis(20),
        );

        let mut ran_on = 0usize;
        let mut never_started = 0usize;
        for _ in 0..40 {
            // Posted under one epoch, which `submit` cannot do.
            let epoch = handle.epochs.bump(QueryLane::History);
            for _ in 0..batch {
                let queued = handle.jobs.send((
                    Some(epoch),
                    RepositoryJob::History(Page::Open {
                        rows: whole_history,
                    }),
                ));
                assert!(queued.is_ok(), "the worker went away mid-batch");
            }
            std::thread::sleep(wait);
            // Supersedes the whole batch.
            handle.submit(Request::MoreHistory { rows: 2 });

            match block_on(updates.next()) {
                Some(Update::Rows { rows, .. }) => match rows.first() {
                    Some(row) if row.id() == opened_on => return,
                    Some(_) => never_started += 1,
                    None => ran_on += 1,
                },
                other => panic!("expected the next page, got {other:?}"),
            }
        }

        panic!(
            "no supersession ever stopped a walk: {ran_on} rounds ran to the end of the \
             history anyway and {never_started} never started one, over a batch of {batch} \
             requests superseded after {wait:?} (one answer took {answer:?}). A walk that \
             finishes after being superseded means the engine was handed a cancel signal \
             that is not the epoch — see `serve`."
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
                    asking.submit(Request::OpenHistory { rows: 1 });
                }
                match woken_by(&waker, updates.next()) {
                    Some(Update::Rows { .. }) => {}
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

    #[test]
    fn submitting_returns_immediately_even_with_nobody_serving() {
        let (jobs, incoming) = channel::<(Option<Epoch>, RepositoryJob)>();
        let (diff, diff_incoming) = channel::<DiffJob>();
        drop((incoming, diff_incoming));
        let epochs = Epochs::new();
        let handle = RepositoryHandle {
            jobs,
            diff,
            epochs: epochs.clone(),
            control: FetchControl::default(),
        };
        let started = Instant::now();
        let epoch = handle.submit(Request::OpenHistory { rows: 1_000_000 });
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
