//! The repository worker pool: the threads that touch the repository, and the
//! values that cross back to the window.
//!
//! Three threads per open repository, started by the first, each with its own
//! [`Outbox`] so the update stream ends only when every one of them has gone:
//!
//! - `cairn-repository` (this file): opens the channel and finds `git` before
//!   the repository, serves the history walk, forwards operations, and on its
//!   way out stops the other two.
//! - `cairn-operations` (`operations.rs`): runs `git` verbs, so a fetch blocks
//!   neither the walk nor the window.
//! - `cairn-askpass` (`askpass.rs`): accepts the helper's questions and waits
//!   on the window for each answer.
//!
//! Queries carry an epoch and are superseded by the next; operations carry
//! none, so a scroll cannot cancel a fetch and a fetch cannot cancel a scroll.

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use cairn_git::{Error, HistoryCursor, HistoryRequest, HistorySession, SharedRepository};

use super::askpass::{AcceptorStop, Reply, serve_prompts};
use super::epoch::{Epoch, Epochs};
use super::operations::{FetchControl, Operation, serve_operations};
use super::request::{Request, Update};
use super::startup::{Backend, Startup};
use super::wake::{Wake, Woken};

pub const WORKERS_PER_REPOSITORY: usize = 1;

// `incoming` is moved into one closure, so a second worker would not compile anyway.
const _: () = assert!(
    WORKERS_PER_REPOSITORY == 1,
    "serve() owns one repository handle and one live walk per thread; more \
     workers per repository need a routing decision, not a bigger constant"
);

/// What the window does with a credential prompt's answer; see [`Reply`].
/// A plain callback, never a struct field: it holds the channel a secret
/// travels down, and application state must not keep one.
pub type Replier = Rc<dyn Fn(Reply)>;

/// Returns before touching a disk; open failures arrive as [`Update::Failed`], and
/// so does a `git` that is missing or older than Cairn requires, checked first.
/// Drive [`Updates`] from exactly one task.
pub fn open(path: impl AsRef<Path>) -> Result<(RepositoryHandle, Updates, Replier), OpenError> {
    open_with(path, Startup::of_this_process())
}

/// [`open`] with the environment `git` is searched on and run with, and the
/// helper it is pointed at; what a test hands in.
pub(super) fn open_with(
    path: impl AsRef<Path>,
    startup: Startup,
) -> Result<(RepositoryHandle, Updates, Replier), OpenError> {
    let path = path.as_ref().to_owned();

    let (jobs, incoming) = channel::<(Option<Epoch>, Request)>();
    let (outgoing, inbox) = channel::<Envelope>();
    let (answers, answered) = channel::<Reply>();
    let wake = Wake::new();
    let epochs = Epochs::new();

    // One sender per thread, so the stream ends when the last thread does.
    let outbox = Outbox {
        updates: outgoing.clone(),
        wake: Arc::clone(&wake),
    };
    let operations_outbox = Outbox {
        updates: outgoing.clone(),
        wake: Arc::clone(&wake),
    };
    let acceptor_outbox = Outbox {
        updates: outgoing,
        wake: Arc::clone(&wake),
    };
    let worker_epochs = epochs.clone();
    let worker_wake = Arc::clone(&wake);
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
            // Once, before anything else: the channel, then git around it. A missing or
            // too-old git is reported with the version Cairn needs, never worked around
            // (D1). Off the UI thread, since finding out means running `git --version`.
            let backend = match Backend::open(&startup) {
                Ok(backend) => backend,
                Err(error) => {
                    outbox.send(
                        None,
                        Update::Failed {
                            message: error.to_string(),
                        },
                    );
                    return;
                }
            };
            let shared = match SharedRepository::discover(&opening) {
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
            let threads = Threads::start(
                backend,
                Arc::clone(&shared),
                answered,
                operations_outbox,
                acceptor_outbox,
                &worker_wake,
            );
            serve(&shared, incoming, outbox, worker_epochs, &threads);
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
            epochs: epochs.clone(),
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

/// The two threads the repository thread starts, and how it reaches them.
struct Threads {
    operations: Option<Sender<Operation>>,
    control: FetchControl,
    acceptor: Option<AcceptorStop>,
}

impl Threads {
    fn start(
        backend: Backend,
        shared: Arc<SharedRepository>,
        answered: Receiver<Reply>,
        operations_outbox: Outbox,
        acceptor_outbox: Outbox,
        wake: &Arc<Wake>,
    ) -> Self {
        let (operations, queued) = channel::<Operation>();
        let control = FetchControl::default();
        let Backend {
            git,
            channel,
            prompting,
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
            // A thread that could not start leaves nothing to stop; `exit` went with the closure.
            let _ = started;
            stop
        });

        let running = control.clone();
        let exit = WorkerExit {
            outbox: Some(operations_outbox),
            wake: Arc::clone(wake),
        };
        let _ = std::thread::Builder::new()
            .name("cairn-operations".to_owned())
            .spawn(move || {
                if let Some(outbox) = exit.outbox.as_ref() {
                    serve_operations(
                        &git,
                        &shared,
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
            operations: Some(operations),
            control,
            acceptor,
        }
    }

    fn perform(&self, operation: Operation) {
        if let Some(operations) = &self.operations {
            let _ = operations.send(operation);
        }
    }

    /// Ends both threads; see [`Threads::drop`], which does the work so that
    /// a panic in `serve` stops them too.
    fn stop(self) {
        drop(self);
    }
}

impl Drop for Threads {
    /// A fetch in flight is killed, the operations queue closed, and the
    /// acceptor woken to see it should stop. Runs once the window has let go
    /// (the acceptor's answering end is gone by then, so a prompt still
    /// waiting is refused rather than left hanging) and on unwinding alike.
    fn drop(&mut self) {
        self.control.cancel();
        self.operations = None;
        if let Some(acceptor) = &self.acceptor {
            acceptor.stop();
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepositoryHandle {
    jobs: Sender<(Option<Epoch>, Request)>,
    epochs: Epochs,
}

impl RepositoryHandle {
    /// Never blocks. A query supersedes whatever query was in flight and is
    /// numbered; an operation is queued behind nothing and supersedes
    /// nothing, and the epoch returned is simply the current one.
    pub fn submit(&self, request: Request) -> Epoch {
        let epoch = if request.is_query() {
            self.epochs.bump()
        } else {
            self.epochs.current()
        };
        let numbered = request.is_query().then_some(epoch);
        // A failed send means the worker is gone and has already said so.
        let _ = self.jobs.send((numbered, request));
        epoch
    }

    /// [`Self::submit`] as the plain callback the window takes.
    pub fn into_submitter(self) -> Rc<dyn Fn(Request)> {
        Rc::new(move |request| {
            self.submit(request);
        })
    }
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
#[derive(Debug)]
pub(super) struct Outbox {
    updates: Sender<Envelope>,
    wake: Arc<Wake>,
}

impl Outbox {
    pub(super) fn send(&self, epoch: Option<Epoch>, update: Update) {
        if self.updates.send(Envelope { epoch, update }).is_ok() {
            self.wake.signal();
        }
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
        // Close the channel, then wake: a task woken while a sender lives sees an empty
        // channel and parks again.
        self.outbox = None;
        self.wake.signal();
    }
}

fn serve(
    shared: &SharedRepository,
    jobs: Receiver<(Option<Epoch>, Request)>,
    outbox: &Outbox,
    epochs: Epochs,
    threads: &Threads,
) {
    let repo = shared.to_worker();
    // Once, outside the loop: `scroll` borrows `repo` across turns, so moving this inside
    // fails to compile.
    let mut scroll: Option<HistorySession<'_>> = None;
    let mut cursor: Option<HistoryCursor> = None;

    while let Ok((epoch, request)) = jobs.recv() {
        if epochs.is_stopping() {
            break;
        }
        if let Some(epoch) = epoch
            && !epochs.is_current(epoch)
        {
            continue; // Superseded before it was picked up; never started.
        }

        let rows = match request {
            Request::OpenHistory { rows } => {
                // A different scroll: drop the open walk first. After a fetch this is
                // also what honours `Invalidated::refs`: the new walk starts from the
                // refs as they are now.
                scroll = None;
                cursor = None;
                rows
            }
            Request::MoreHistory { rows } => rows,
            Request::ListRemotes => {
                match repo.remotes() {
                    Ok(remotes) => outbox.send(None, Update::Remotes { remotes }),
                    Err(error) => outbox.send(
                        None,
                        Update::Failed {
                            message: error.to_string(),
                        },
                    ),
                }
                continue;
            }
            Request::Fetch { remote } => {
                threads.perform(Operation::Fetch { remote });
                continue;
            }
            Request::CancelFetch => {
                threads.control.cancel();
                continue;
            }
        };
        let Some(epoch) = epoch else {
            continue; // Every query is numbered; `submit` guarantees it.
        };

        if scroll.is_none() {
            // Cold restart from the last good page; `None` means the scroll has not started.
            let request = match cursor.clone() {
                Some(at) => HistoryRequest::resume(at, rows),
                None => HistoryRequest::from_head(rows),
            };
            match repo.history_session(&request) {
                Ok(session) => scroll = Some(session),
                Err(error) => {
                    outbox.send(Some(epoch), no_walk(error));
                    continue;
                }
            }
        }

        let Some(session) = scroll.as_mut() else {
            continue;
        };
        match session.next_page(rows, &epochs.watch(epoch)) {
            Ok(page) => {
                let complete = page.cursor.is_none();
                cursor = page.cursor;
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
                scroll = None;
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
    use std::path::PathBuf;
    use std::task::Waker;
    use std::time::Instant;

    use crate::worker::fetch_tests::{UnbornRepository, block_on, woken_by};

    /// The Cairn checkout itself, opened through the real boundary.
    fn cairn() -> (RepositoryHandle, Updates) {
        match open(env!("CARGO_MANIFEST_DIR")) {
            Ok((handle, updates, _)) => (handle, updates),
            Err(error) => panic!("opening the Cairn checkout: {error}"),
        }
    }

    /// Arrives as an update, not as an error from `open`.
    #[test]
    fn opening_a_path_outside_a_repository_is_reported_and_names_the_path() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        let (_handle, mut updates, _) = match open(&outside) {
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

    /// `open` succeeds for a path with no repository above it.
    #[test]
    fn opening_returns_before_the_repository_is_found() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        assert!(
            open(&outside).is_ok(),
            "open() decided there was no repository, so it looked — on the caller's thread"
        );
    }

    /// Caught by: deleting the unborn arm at the call site while `no_walk` stays correct.
    #[test]
    fn a_freshly_initialised_repository_reaches_the_view_as_an_empty_history() {
        let fixture = UnbornRepository::new("cairn-unborn-head");
        let (handle, mut updates, _) = match open(&fixture.path) {
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
        let before = updates.epochs.current();
        let submit = handle.into_submitter();
        submit(Request::OpenHistory { rows: 2 });
        // Checked first: waiting for rows that were never asked for would hang.
        assert!(
            updates.epochs.current() > before,
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

    /// Borrows this checkout's objects through `alternates`, so its `HEAD` can be broken and mended.
    struct BorrowedRepository {
        fixture: UnbornRepository,
    }

    impl BorrowedRepository {
        fn new(name: &str) -> Self {
            let fixture = UnbornRepository::new(name);
            let checkout = match SharedRepository::discover(env!("CARGO_MANIFEST_DIR")) {
                Ok(shared) => shared.git_dir().to_owned(),
                Err(error) => panic!("opening the Cairn checkout: {error}"),
            };
            // A linked worktree keeps its objects in the common directory.
            let common = match std::fs::read_to_string(checkout.join("commondir")) {
                Ok(relative) => checkout.join(relative.trim()),
                Err(_) => checkout,
            };
            let objects = match common.join("objects").canonicalize() {
                Ok(objects) => objects,
                Err(error) => panic!("finding the checkout's objects: {error}"),
            };
            let alternates = fixture.path.join(".git/objects/info/alternates");
            if let Err(error) = std::fs::write(&alternates, format!("{}\n", objects.display())) {
                panic!("writing {}: {error}", alternates.display());
            }
            Self { fixture }
        }

        fn point_main_at(&self, id: &str) {
            let main = self.fixture.path.join(".git/refs/heads/main");
            if let Err(error) = std::fs::write(&main, format!("{id}\n")) {
                panic!("writing {}: {error}", main.display());
            }
        }
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
        let (handle, mut updates, _) = match open(&fixture.fixture.path) {
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
            let epoch = handle.epochs.bump();
            for _ in 0..batch {
                let queued = handle.jobs.send((
                    Some(epoch),
                    Request::OpenHistory {
                        rows: whole_history,
                    },
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
            updates: sender,
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

        let stale = epochs.bump();
        outbox.send(
            Some(stale),
            Update::Failed {
                message: "stale".to_owned(),
            },
        );
        let fresh = epochs.bump();
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

    /// The negative for the test above: dropping everything must fail it.
    #[test]
    fn an_answer_to_the_current_request_is_returned() {
        let (epochs, outbox, mut updates) = inbox_only();
        let mine = epochs.bump();
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
        epochs.bump();
        outbox.send(
            None,
            Update::WorkerLost {
                message: "gone".to_owned(),
            },
        );
        epochs.bump();
        epochs.bump();

        match block_on(updates.next()) {
            Some(Update::WorkerLost { message }) => assert_eq!(message, "gone"),
            other => panic!("expected the worker notice, got {other:?}"),
        }
    }

    #[test]
    fn nothing_is_rendered_once_the_pool_is_stopping() {
        let (epochs, outbox, mut updates) = inbox_only();
        let mine = epochs.bump();
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
            updates: updates_tx,
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
                updates: updates_tx,
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
        let (_handle, mut updates, _) = match open(&outside) {
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

    #[test]
    fn submitting_returns_immediately_even_with_nobody_serving() {
        let (jobs, incoming) = channel::<(Option<Epoch>, Request)>();
        drop(incoming);
        let epochs = Epochs::new();
        let handle = RepositoryHandle {
            jobs,
            epochs: epochs.clone(),
        };
        let started = Instant::now();
        let epoch = handle.submit(Request::OpenHistory { rows: 1_000_000 });
        assert!(
            started.elapsed() < std::time::Duration::from_millis(50),
            "submitting waited for {:?}",
            started.elapsed()
        );
        assert_eq!(epochs.current(), epoch, "the request was not numbered");
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
