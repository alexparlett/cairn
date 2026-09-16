//! The worker that owns a repository, and the two values that reach the view.
//!
//! D3: one [`SharedRepository`] per open repository, one worker thread for it,
//! `to_worker` called once at the top of that thread. See
//! `docs/systems/history-graph.md`, "The worker boundary".

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use cairn_git::{Error, HistoryCursor, HistoryRequest, HistorySession, SharedRepository};

use super::epoch::{Epoch, Epochs};
use super::request::{Request, Update};
use super::wake::{Wake, Woken};

/// One worker thread per open repository — not one per core.
///
/// Structural, not a throughput budget: R2.5's live walk borrows one thread's
/// handle for the life of a scroll, and there is exactly one scroll, so a second
/// worker would have nothing to do while holding an object cache. A second kind
/// of work gets its own worker, which costs this one almost nothing — concurrent
/// walks scale 2.1x, 4.2x and 7.9x at 2, 4 and 8 threads with single-walk time
/// flat (`measures_concurrent_walks_against_a_named_repository`).
pub const WORKERS_PER_REPOSITORY: usize = 1;

// The sign, not the enforcement: `incoming` is a single-consumer receiver moved
// into one closure, so a loop of workers does not compile at any count. A second
// worker needs a routing decision about which one owns the live walk.
const _: () = assert!(
    WORKERS_PER_REPOSITORY == 1,
    "serve() owns one repository handle and one live walk per thread; more \
     workers per repository need a routing decision, not a bigger constant"
);

/// Starts the worker that will own the repository containing `path`, returning
/// before anything touches a disk: opening happens on the worker, since
/// discovering a repository walks up the filesystem reading config. A path
/// outside a repository arrives as an [`Update::Failed`]; the only failure here
/// is the operating system refusing a thread. Drive [`Updates`] from exactly one
/// task.
pub fn open(path: impl AsRef<Path>) -> Result<(RepositoryHandle, Updates), OpenError> {
    let path = path.as_ref().to_owned();

    let (jobs, incoming) = channel::<(Epoch, Request)>();
    let (outgoing, inbox) = channel::<Envelope>();
    let wake = Wake::new();
    let epochs = Epochs::new();

    let outbox = Outbox {
        updates: outgoing,
        wake: Arc::clone(&wake),
    };
    let worker_epochs = epochs.clone();
    let worker_wake = Arc::clone(&wake);
    let opening = path.clone();

    // Detached: joining is waiting. The thread ends when the last
    // `RepositoryHandle` drops and its channel closes.
    std::thread::Builder::new()
        .name("cairn-repository".to_owned())
        .spawn(move || {
            // One sender lives on this thread and `exit` owns it. A second copy
            // would outlive `exit`, leaving the UI task an empty channel rather
            // than a closed one when `WorkerExit::drop` wakes it.
            let exit = WorkerExit {
                outbox: Some(outbox),
                wake: worker_wake,
            };
            let Some(outbox) = exit.outbox.as_ref() else {
                return;
            };
            match SharedRepository::discover(&opening) {
                Ok(shared) => serve(shared, incoming, outbox, worker_epochs),
                Err(source) => outbox.send(
                    // No epoch: failing to open answers no request and must not
                    // be filtered by one. The message names the path (R5.2).
                    None,
                    Update::Failed {
                        message: source.to_string(),
                    },
                ),
            }
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
    ))
}

/// The view's only way to reach a repository. One method, returning
/// immediately, and no receiving end of anything: the type-level half of "the UI
/// thread never waits on repository work". The guard covers what a type cannot —
/// that nobody reached past this type to a repository directly.
#[derive(Debug, Clone)]
pub struct RepositoryHandle {
    jobs: Sender<(Epoch, Request)>,
    epochs: Epochs,
}

impl RepositoryHandle {
    /// Asks for something, superseding whatever was in flight. The channel is
    /// unbounded, so the send cannot block; the epoch returned is how a caller
    /// recognises the answer to *this* request.
    pub fn submit(&self, request: Request) -> Epoch {
        let epoch = self.epochs.bump();
        // A failed send means the worker is gone and has already said so.
        let _ = self.jobs.send((epoch, request));
        epoch
    }
}

/// Everything the workers have to say, in arrival order. Driven from exactly
/// one task; `next` is a future, so the UI thread yields between updates.
#[derive(Debug)]
pub struct Updates {
    inbox: Receiver<Envelope>,
    wake: Arc<Wake>,
    epochs: Epochs,
}

impl Updates {
    /// The next update worth rendering, or `None` once every worker has gone.
    /// Drops superseded updates (R3.2). An epochless update — a dying worker —
    /// is kept.
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
    /// Stops the walk where it stands rather than letting a worker finish a
    /// page of somebody's ten-year monorepo first.
    fn drop(&mut self) {
        self.epochs.stop();
    }
}

/// Why a repository could not be opened: display text and nothing else.
/// Hand-written rather than wrapping `cairn_git::Error`, so the view needs none
/// of the engine's vocabulary.
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

/// One update and the request it answers, or `None` for news about the worker
/// itself.
#[derive(Debug)]
struct Envelope {
    epoch: Option<Epoch>,
    update: Update,
}

/// The one way anything reaches the window. Not `Clone`: exactly one lives on a
/// worker thread, owned by its [`WorkerExit`], so no second sender keeps the
/// channel open past the wake announcing the thread is gone.
#[derive(Debug)]
struct Outbox {
    updates: Sender<Envelope>,
    wake: Arc<Wake>,
}

impl Outbox {
    fn send(&self, epoch: Option<Epoch>, update: Update) {
        if self.updates.send(Envelope { epoch, update }).is_ok() {
            self.wake.signal();
        }
    }
}

/// Announces a worker's death, however it died. A `Drop`, which unwinding runs,
/// carrying the last sender into the update channel and dropping it before
/// waking: closing a channel does not wake a task parked on [`Wake`].
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
        // Order matters: close the channel, then wake. A task woken while a
        // sender is still alive sees an empty channel and parks again.
        self.outbox = None;
        self.wake.signal();
    }
}

/// One worker's whole life. `to_worker` is called once, before the loop, and the
/// walk borrowing it lives in this frame — which is how a scroll keeps
/// gitoxide's walk open across requests without it ever being `Send`.
fn serve(
    shared: SharedRepository,
    jobs: Receiver<(Epoch, Request)>,
    outbox: &Outbox,
    epochs: Epochs,
) {
    let repo = shared.to_worker();
    // Once, and the borrow checker makes it true: `scroll` holds a
    // `HistorySession<'_>` borrowing `repo` across a turn of the loop, so moving
    // `to_worker()` into the loop fails with `error[E0597]`.
    //
    // `drop(shared)` is not that guarantee: deleting it leaves the crate
    // compiling and every test green. It adds only that a second `to_worker()`
    // beside the first, which no borrow would catch, becomes a use-after-move.
    drop(shared);
    let mut scroll: Option<HistorySession<'_>> = None;
    let mut cursor: Option<HistoryCursor> = None;

    while let Ok((epoch, request)) = jobs.recv() {
        if epochs.is_stopping() {
            break;
        }
        if !epochs.is_current(epoch) {
            continue; // Superseded before it was picked up; never started.
        }

        let rows = match request {
            Request::OpenHistory { rows } => {
                // A different scroll: drop the open walk before building
                // another.
                scroll = None;
                cursor = None;
                rows
            }
            Request::MoreHistory { rows } => rows,
        };

        if scroll.is_none() {
            // R2.5's cold-restart path: replay to where the last good page left
            // off. `None` means the scroll has not started.
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
                // Superseded mid-page. The session keeps its laid-out rows, so
                // the replacing request carries on from there. Nothing is sent.
            }
            Err(error) => {
                // The walk's position is no longer trustworthy; the last good
                // page's cursor is. Dropping the session cold-restarts there.
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

/// `Error::UnbornHead` is not a failure: an empty history crosses as a complete
/// page of no rows, so the window says "no commits yet" rather than showing an
/// error banner (R4.3). Every other failure crosses as display text.
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
    use std::task::{Context, Poll, Waker};
    use std::time::Instant;

    /// Drives a future on this thread: the tests need the boundary's contract,
    /// not Freya's scheduler. On `std::task::Wake`: the workspace forbids
    /// `unsafe`.
    fn block_on<F: Future>(future: F) -> F::Output {
        struct Unpark(std::thread::Thread);
        impl std::task::Wake for Unpark {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
        woken_by(&waker, future)
    }

    /// [`block_on`] with the waker supplied, for the one test whose subject is
    /// waking the window going wrong.
    fn woken_by<F: Future>(waker: &Waker, future: F) -> F::Output {
        let mut cx = Context::from_waker(waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }

    /// The Cairn checkout itself, opened through the real boundary.
    fn cairn() -> (RepositoryHandle, Updates) {
        match open(env!("CARGO_MANIFEST_DIR")) {
            Ok(pair) => pair,
            Err(error) => panic!("opening the Cairn checkout: {error}"),
        }
    }

    /// R5.2: a path outside a repository fails with a message naming it, and
    /// arrives as an update rather than an error from `open`.
    #[test]
    fn opening_a_path_outside_a_repository_is_reported_and_names_the_path() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        let (_handle, mut updates) = match open(&outside) {
            Ok(pair) => pair,
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

    /// Pinned by the one thing a test can see from here: `open` succeeds for a
    /// path with no repository anywhere above it.
    #[test]
    fn opening_returns_before_the_repository_is_found() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        assert!(
            open(&outside).is_ok(),
            "open() decided there was no repository, so it looked — on the caller's thread"
        );
    }

    /// Built with `std::fs`, not `git init`:
    /// `only_the_ops_module_mutates_a_repository` scans this crate's test code
    /// too. These four directories and two files are what `gix` discovers as a
    /// repository whose `HEAD` points at a branch that does not exist yet.
    struct UnbornRepository {
        path: PathBuf,
    }

    impl UnbornRepository {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(name);
            let _ = std::fs::remove_dir_all(&path);
            let dot = path.join(".git");
            for inside in ["objects/info", "objects/pack", "refs/heads", "refs/tags"] {
                if let Err(error) = std::fs::create_dir_all(dot.join(inside)) {
                    panic!("building {}: {error}", dot.join(inside).display());
                }
            }
            if let Err(error) = std::fs::write(dot.join("HEAD"), "ref: refs/heads/main\n") {
                panic!("writing HEAD: {error}");
            }
            if let Err(error) = std::fs::write(
                dot.join("config"),
                "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
            ) {
                panic!("writing config: {error}");
            }
            Self { path }
        }
    }

    impl Drop for UnbornRepository {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// The whole path, not the mapping alone. Caught by: deleting the arm from
    /// the call site while `no_walk` stays correct, which turns a freshly
    /// initialised repository back into a red error banner.
    #[test]
    fn a_freshly_initialised_repository_reaches_the_view_as_an_empty_history() {
        let fixture = UnbornRepository::new("cairn-unborn-head");
        let (handle, mut updates) = match open(&fixture.path) {
            Ok(pair) => pair,
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

    /// Empty, not broken: R4.3 cannot be drawn if an empty repository arrives as
    /// an error sentence beside every other kind of failure.
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

    /// Every other failure still arrives as the text it will be shown as,
    /// including R5.2's.
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
    fn a_request_is_answered_with_rows() {
        let (handle, mut updates) = cairn();
        handle.submit(Request::OpenHistory { rows: 3 });
        match block_on(updates.next()) {
            Some(Update::Rows { rows, .. }) => assert_eq!(rows.len(), 3),
            other => panic!("expected rows, got {other:?}"),
        }
        drop(handle);
    }

    /// R2.5 across the boundary: paging costs the page, not the pages before it.
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

    /// The epoch reaches the engine, not only the answer (D3). Caught by:
    /// handing `next_page` a fresh `CancelSignal` instead of
    /// `epochs.watch(epoch)`, which leaves every other test in this crate green
    /// while a superseded walk runs to completion and has its answer thrown
    /// away. `epoch.rs` decides that `Superseded` flips and `cairn-git`'s
    /// `cancelling_a_session_stops_the_walk_and_keeps_its_progress` that
    /// `next_page` honours a signal; neither sees this call site, and the
    /// [`inbox_only`] tests decide the epoch filter alone.
    ///
    /// A superseded request's fate is read off the next page: stopped mid-walk
    /// starts again at the row the scroll opened on; never picked up carries on
    /// past the rows already handed out; ran to completion gives an empty page.
    /// Only the first is a stop. A walk of this repository is microseconds long,
    /// so the worker gets a batch under one epoch and the supersession lands
    /// inside a walk rather than a park; the gap between two requests of the
    /// batch still looks like completion, so a round landing there is a retry and
    /// the failure is that every round saw one.
    #[test]
    fn superseding_a_request_stops_the_walk_that_is_serving_it() {
        // Larger than this repository can answer, so every request below would
        // otherwise walk the whole history.
        let whole_history = 1_000_000;
        // Queued work, not a queued number: the worker must still be busy when
        // the supersession arrives.
        let batch = 2_000;
        let (handle, mut updates) = cairn();

        // One ordinary round trip: names the row a scroll opens on, and measures
        // what an answer costs on this machine. Everything below scales to it.
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
            // Posted under one epoch, which `submit` cannot do: it bumps per
            // call. The worker's loop cannot tell the difference.
            let epoch = handle.epochs.bump();
            for _ in 0..batch {
                let queued = handle.jobs.send((
                    epoch,
                    Request::OpenHistory {
                        rows: whole_history,
                    },
                ));
                assert!(queued.is_ok(), "the worker went away mid-batch");
            }
            std::thread::sleep(wait);
            // Supersedes the whole batch: the one being walked stops, the rest
            // are dropped unpicked.
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

    /// The receiving half alone, with no repository behind it: enough to decide
    /// what reaches the view, without racing a walk.
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

    /// R3.2, without racing a walk: the stale answer is posted by hand and then
    /// superseded.
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

    /// The negative for the test above: without it, dropping everything passes.
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

    /// News about a worker answers no request, so it survives a supersession.
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

    /// Shutting down stops answers as well as walks: the epoch that was current
    /// is not once the window is closing.
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

    /// A worker that panics must announce itself, or the window waits forever.
    /// The notice is a `Drop`, so this drives a real panic through one.
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

    /// The same notice through the wiring that installs it: the test above
    /// places a [`WorkerExit`] by hand, deciding that the guard works and not
    /// that [`open`] installs one. Caught by: deleting `WorkerExit` from
    /// `open`'s closure and ending it with `drop(outbox); worker_wake.signal();`
    /// — clean shutdown keeps working and every other test stays green while a
    /// panic inside [`serve`] unwinds past that signal. This goes red two ways,
    /// racing the unwind: the ten-second hang below, or a stream that ends
    /// without saying why.
    ///
    /// `Wake::signal` calls the window's waker on the worker thread, so a waker
    /// that gives way is a panic inside `serve` raised by the line answering a
    /// request. It gives way exactly once: a second panic while the notice was
    /// being sent would abort the process.
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
                // `spent` first, then the unpark, then the panic: the thread
                // this reaches must see the waker gave way, and must be woken
                // either way — a test hanging on its own waker decides nothing.
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

        // Everything that can park runs over there, so the bound below bounds the
        // whole test, and a failure returns as a value the hook cannot silence.
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

            // Ask again until an answer's wake runs the waker: a signal arriving
            // before this thread parks only latches, and latching raises
            // nothing.
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

        // A worker dying unannounced parks whoever drives `Updates` forever;
        // that must redden rather than hang the suite.
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

    /// The negative for the two above: without it, a notice sent
    /// unconditionally passes.
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

    /// Implemented for [`Outbox`] by hand and for everything `Clone` by blanket
    /// impl, so the two overlap exactly when `Outbox` is `Clone` and an overlap
    /// is `error[E0119]`.
    ///
    /// It decides the `Clone`, not the rule. Residual review obligation:
    /// `Sender<Envelope>` is `Clone` and `Outbox`'s fields are visible
    /// throughout this module, so a second sender written by hand compiles and
    /// passes every test here. A copy outliving [`WorkerExit`] holds the update
    /// channel open past the wake announcing the thread is gone and parks the
    /// waiting task forever — no test sees that, the race being narrow enough
    /// that [`a_failed_open_ends_the_stream_rather_than_leaving_it_open`] passed
    /// against the broken code.
    trait OneSenderPerWorker {}
    impl OneSenderPerWorker for Outbox {}
    impl<T: Clone> OneSenderPerWorker for T {}

    /// The assertion is the compilation, not the body: deriving `Clone` on
    /// [`Outbox`] makes the two [`OneSenderPerWorker`] impls overlap and the test
    /// build stops rather than this failing. The gate compiles tests twice —
    /// clippy and the test run — so a `Clone` cannot reach it, though
    /// `cargo build` alone would still succeed.
    #[test]
    fn a_worker_thread_cannot_be_given_a_second_sender() {
        fn one_sender_only<T: OneSenderPerWorker>() {}
        one_sender_only::<Outbox>();
    }

    /// The stream must end after a failed open, not merely report the failure.
    /// The contract alone: a second sender outliving the notice is a race narrow
    /// enough that this passed against the broken code too. What decides that is
    /// the type, held to it by
    /// [`a_worker_thread_cannot_be_given_a_second_sender`].
    #[test]
    fn a_failed_open_ends_the_stream_rather_than_leaving_it_open() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        let (_handle, mut updates) = match open(&outside) {
            Ok(pair) => pair,
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

    /// The hang this boundary exists to prevent: a worker ending without sending
    /// anything must still wake the parked task, since closing a channel does
    /// not wake a `Wake`.
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

    /// `submit` returns whether or not a worker is listening.
    #[test]
    fn submitting_returns_immediately_even_with_nobody_serving() {
        let (jobs, incoming) = channel::<(Epoch, Request)>();
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
