//! The worker that owns a repository, and the two values that reach the view.
//!
//! Decision D3, made concrete: one [`SharedRepository`] per open repository,
//! one worker thread per repository, and that thread calls `to_worker` exactly
//! once — at its start, for its whole life. Per-request conversion compiles and
//! passes every test while throwing away the object cache and the pack snapshot
//! the split exists to keep, so [`serve`] is written to make the single call
//! visible in one place.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use cairn_git::{Error, HistoryCursor, HistoryRequest, HistorySession, SharedRepository};

use super::epoch::{Epoch, Epochs};
use super::request::{Request, Update};
use super::wake::{Wake, Woken};

/// One worker thread per open repository — not one per core.
///
/// **O3, decided by measurement, and the measurement did not say what the
/// packet expected.** The harness is
/// `measures_concurrent_walks_against_a_named_repository` in `cairn-git`; the
/// numbers and their caveats are in `docs/work/history-graph/progress.md`.
/// Concurrent walks of one repository scale nearly linearly — 2.1x, 4.2x and
/// 7.9x the rows per second at 2, 4 and 8 threads, with the time for any single
/// walk flat within noise — so "gix already parallelises internally, more
/// threads would oversubscribe" is not what happens here. The reason for one is
/// structural instead:
///
/// - R2.5's live walk borrows one worker's repository handle and stays on that
///   thread for the life of a scroll. A second worker cannot serve the *next
///   page* of the same scroll. Splitting one scroll across workers is not a
///   tuning question; it is not expressible.
/// - There is exactly one scroll. A second worker would have nothing to do,
///   and idle threads holding object caches are not free.
///
/// So workers are pinned to a purpose rather than fed from an anonymous queue,
/// and the measurement says that pinning a *second* purpose to a *second*
/// worker will cost the first one almost nothing. A second kind of work —
/// fetch, when `docs/prd/credential-prompts.md` lands — gets its own worker for
/// that reason, and because it blocks mid-operation on a UI dialog: a fetch
/// sharing this queue would stall the graph behind a password prompt.
pub const WORKERS_PER_REPOSITORY: usize = 1;

// Raising the number above is not enough to raise the number of workers, and
// the compiler is the reason rather than this assertion: `incoming` is a
// single-consumer receiver moved into one closure, so `for _ in
// 0..WORKERS_PER_REPOSITORY { spawn(move || ...) }` does not compile at any
// count — not even at one, because the compiler cannot know a loop runs once.
// A second worker therefore needs a routing decision about which one owns the
// live walk, not a bigger number. This assertion is the sign that says so.
const _: () = assert!(
    WORKERS_PER_REPOSITORY == 1,
    "serve() owns one repository handle and one live walk per thread; more \
     workers per repository need a routing decision, not a bigger constant"
);

/// Start the worker that will own the repository containing `path`.
///
/// Returns the two halves of the boundary: a [`RepositoryHandle`] to hold, and
/// the [`Updates`] stream to drive from exactly one task. Both come back before
/// anything has touched a disk — **opening the repository happens on the worker**,
/// because discovering one walks up the filesystem reading config, and doing
/// that on the UI thread is the very thing this module exists to stop. A path
/// that is not inside a repository therefore arrives as an
/// [`Update::Failed`] rather than as an error from here, and the only way this
/// call fails is that the operating system refused a thread.
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
    let worker_outbox = outbox.clone();
    let worker_wake = Arc::clone(&wake);
    let opening = path.clone();

    // Deliberately detached: the handle to join on is never kept, because
    // joining is waiting and nothing on this side of the boundary may wait. The
    // thread ends when the last `RepositoryHandle` drops and its channel closes.
    std::thread::Builder::new()
        .name("cairn-repository".to_owned())
        .spawn(move || {
            let _exit = WorkerExit {
                outbox: Some(worker_outbox),
                wake: worker_wake,
            };
            match SharedRepository::discover(&opening) {
                Ok(shared) => serve(shared, incoming, outbox, worker_epochs),
                Err(source) => outbox.send(
                    // No epoch: failing to open is not an answer to a request,
                    // and must not be filtered out by one. The engine's message
                    // already names the path — "no git repository at /tmp/x" —
                    // which is what R5.2 asks a failed open to say.
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

/// The view's only way to reach a repository.
///
/// It has exactly one method, it returns immediately, and the type carries no
/// receiving end of anything: there is nothing here to wait on, and nothing to
/// wait for. That is the type-level half of "the UI thread never waits on
/// repository work" — the guard covers what a type cannot say, which is that
/// nobody reached past this type to a repository directly.
#[derive(Debug, Clone)]
pub struct RepositoryHandle {
    jobs: Sender<(Epoch, Request)>,
    epochs: Epochs,
}

impl RepositoryHandle {
    /// Ask for something, superseding whatever was in flight.
    ///
    /// Returns immediately. The channel is unbounded, so the send cannot block
    /// even when a worker is busy; the epoch it returns is how a caller
    /// recognises the answer to *this* request.
    pub fn submit(&self, request: Request) -> Epoch {
        let epoch = self.epochs.bump();
        // A failed send means the worker is gone. It has already said so —
        // `WorkerLost` on a death, and the update stream ending either way —
        // so there is nobody left to tell and nothing here can wait to find out.
        let _ = self.jobs.send((epoch, request));
        epoch
    }
}

/// Everything the workers have to say, in arrival order.
///
/// Owned by the application root and driven from one task. `next` is a future,
/// so the UI thread yields to its event loop between updates rather than
/// holding it.
#[derive(Debug)]
pub struct Updates {
    inbox: Receiver<Envelope>,
    wake: Arc<Wake>,
    epochs: Epochs,
}

impl Updates {
    /// The next update worth rendering, or `None` because every worker has
    /// gone.
    ///
    /// R3.2 lives here: an update belonging to a superseded request is dropped
    /// rather than returned, so nothing stale can reach the view. An update
    /// carrying no epoch — a worker dying — is never dropped, because it is not
    /// an answer to anything.
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
    /// The window is going. Stop the walk where it stands rather than letting a
    /// worker finish a page of somebody's ten-year monorepo first.
    fn drop(&mut self) {
        self.epochs.stop();
    }
}

/// Why a repository could not be opened.
///
/// Carries the sentence to show, and nothing else. Hand-written rather than
/// wrapping `cairn_git::Error`: the view must be able to say what went wrong
/// without linking the engine's vocabulary.
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

#[derive(Debug, Clone)]
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

/// Announces a worker's death, however it died.
///
/// A pool that silently loses a thread degrades into a window that waits
/// forever — the exact failure this boundary exists to prevent — so the notice
/// is a `Drop`, which unwinding runs, rather than something the worker loop
/// would have to reach.
///
/// It carries the LAST sender into the update channel, and drops it before
/// waking: closing a channel does not wake a task parked on [`Wake`], so a
/// worker that ended without sending anything — a clean shutdown, or a death
/// the panic branch did not cover — would otherwise leave the UI task asleep
/// forever with nothing coming. That is the hang, arrived at from the other
/// side.
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
        // Order matters: close the channel, THEN wake. A task woken while a
        // sender is still alive sees an empty channel and parks again.
        self.outbox = None;
        self.wake.signal();
    }
}

/// One worker's whole life.
///
/// `to_worker` is called once, here, before the loop. The repository handle and
/// the walk that borrows it both live in this stack frame, which is what lets a
/// scroll keep gitoxide's walk open across requests: the walk is not `Send` and
/// never has to be.
fn serve(
    shared: SharedRepository,
    jobs: Receiver<(Epoch, Request)>,
    outbox: Outbox,
    epochs: Epochs,
) {
    let repo = shared.to_worker();
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
                // A different scroll. The open walk is not wanted, so it goes
                // before another is built rather than being left to age.
                scroll = None;
                cursor = None;
                rows
            }
            Request::MoreHistory { rows } => rows,
        };

        if scroll.is_none() {
            // The cold-restart path R2.5 keeps: no live walk, so replay to
            // where the last good page left off. `None` means the scroll has
            // not started yet.
            let request = match cursor.clone() {
                Some(at) => HistoryRequest::resume(at, rows),
                None => HistoryRequest::from_head(rows),
            };
            match repo.history_session(&request) {
                Ok(session) => scroll = Some(session),
                Err(error) => {
                    outbox.send(
                        Some(epoch),
                        Update::Failed {
                            message: error.to_string(),
                        },
                    );
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
                // Superseded mid-page. The session keeps every row it had
                // already laid out, so the request that replaced this one
                // carries on from there instead of re-walking: no work is lost,
                // and no half-consumed walk is left for the next request to
                // read. Nothing is sent, because nobody wants this answer.
            }
            Err(error) => {
                // The walk failed, so its position is no longer trustworthy —
                // but the cursor from the last good page is. Dropping the
                // session makes the next request cold-restart from there.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{Context, Poll, Waker};
    use std::time::Instant;

    /// Drive a future to completion on this thread.
    ///
    /// The tests need the boundary's contract, not Freya's scheduler, so they
    /// bring their own one-task executor. `std::task::Wake` keeps it safe —
    /// this workspace forbids `unsafe`.
    fn block_on<F: Future>(future: F) -> F::Output {
        struct Unpark(std::thread::Thread);
        impl std::task::Wake for Unpark {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }

    /// O3's measurement — "how many workers should one repository have?" — lives
    /// with the type the question is about:
    /// `measures_concurrent_walks_against_a_named_repository` in
    /// `crates/cairn-git/src/repository.rs`. Keeping it there means running it
    /// does not link a window.
    fn cairn() -> (RepositoryHandle, Updates) {
        match open(env!("CARGO_MANIFEST_DIR")) {
            Ok(pair) => pair,
            Err(error) => panic!("opening the Cairn checkout: {error}"),
        }
    }

    /// R5.2 in miniature: a path that is not in a repository fails with a
    /// message that names it, not with an empty window. It arrives as an update
    /// rather than as an error from `open`, because opening reads the
    /// filesystem and so happens on the worker.
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

    /// Nothing about opening happens before `open` returns — the filesystem walk
    /// that discovers a repository is the worker's, not the UI thread's. Pinned
    /// by the one thing a test can see from here: `open` succeeds for a path
    /// that has no repository anywhere above it.
    #[test]
    fn opening_returns_before_the_repository_is_found() {
        let outside = std::env::temp_dir().join("cairn-not-a-repository");
        assert!(
            open(&outside).is_ok(),
            "open() decided there was no repository, so it looked — on the caller's thread"
        );
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

    /// R2.5 across the boundary: paging costs the page, not the pages before
    /// it, because the worker keeps one walk open for the whole scroll.
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

    /// The receiving half on its own, with no repository behind it: enough to
    /// decide what reaches the view and what does not, without racing a walk.
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

    /// R3.2. A response belonging to a superseded request never reaches the
    /// view — pinned without racing the walk, by posting the stale answer by
    /// hand and then superseding it.
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

    /// The negative for the test above: with no supersession, the same answer
    /// does arrive. Without this, dropping everything would also pass.
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

    /// News about a worker is not an answer to a request, so it survives a
    /// supersession that would have dropped one.
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

    /// Shutting down stops answers as well as walks: an update tagged with the
    /// epoch that was current is no longer current once the window is closing.
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

    /// The QA brief's question: what happens when a worker panics? It must
    /// announce itself, or the window waits forever. The notice is a `Drop`, so
    /// this drives a real panic through the real guard.
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

    /// The negative for the test above: a clean exit raises no alarm. Without
    /// The negative for the test above: a clean exit raises no alarm. Without
    /// this, a notice sent unconditionally would also pass.
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

    /// The hang this boundary exists to prevent, approached from the side that
    /// actually produced it: a worker that ends WITHOUT sending anything must
    /// still wake the task parked on the update stream, or that task sleeps
    /// forever with nothing coming. Closing a channel does not wake a `Wake`.
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

    /// Nothing on the view's side of the boundary may wait, so `submit` has to
    /// return whether or not a worker is listening.
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
