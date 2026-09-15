//! The worker that owns a repository, and the two values that reach the view.
//!
//! Decision D3, made concrete: one [`SharedRepository`] per open repository,
//! one worker thread per repository, and that thread calls `to_worker` exactly
//! once — at its start, for its whole life. Per-request conversion throws away
//! the object cache and the pack snapshot the split exists to keep, so [`serve`]
//! is written so that the borrow checker refuses it rather than leaving a reader
//! to notice — see the note above `drop(shared)` there for which line carries
//! that and which does not.

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
    let worker_wake = Arc::clone(&wake);
    let opening = path.clone();

    // Deliberately detached: the handle to join on is never kept, because
    // joining is waiting and nothing on this side of the boundary may wait. The
    // thread ends when the last `RepositoryHandle` drops and its channel closes.
    std::thread::Builder::new()
        .name("cairn-repository".to_owned())
        .spawn(move || {
            // ONE sender lives on this thread, and `exit` owns it. The work
            // below borrows it. A second copy — a clone captured by this
            // closure, say — would outlive `exit` by the ordinary drop rules
            // and so still be alive when `WorkerExit::drop` wakes the UI task,
            // which would then see an empty channel rather than a closed one
            // and park with nothing coming. Keeping it to one makes that
            // unrepresentable rather than a thing to get right.
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

/// The one way anything reaches the window. Deliberately NOT `Clone`: exactly
/// one lives on a worker thread, owned by its [`WorkerExit`], so there is no
/// second sender to keep the channel open past the wake that announces the
/// thread is gone.
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
    outbox: &Outbox,
    epochs: Epochs,
) {
    let repo = shared.to_worker();
    // The conversion happens once, and the compiler is what makes that true
    // rather than merely intended: moving `to_worker()` into the loop below —
    // which would silently rebuild the object cache and the pack snapshot on
    // every request — does not compile. Two errors stand in the way and only
    // one of them is load-bearing: the move past `drop(shared)` is reported
    // first (`error[E0382]`), and with that out of the way the borrow `scroll`
    // holds still refuses it — a `HistorySession<'_>` borrows `repo` and
    // outlives a turn of the loop, so `repo` may not be replaced while it lives
    // (`error[E0597]: repo does not live long enough`). The second is the one
    // that would still be there if this function were rearranged.
    //
    // `drop(shared)` is not that guarantee and must not be read as it —
    // deleting this line leaves the crate compiling, clippy clean and every
    // test green. What it does is narrower and still worth keeping: it says
    // the shared handle has no further use on this thread, so a SECOND
    // `to_worker()` next to the first — one that did not feed `scroll`, and
    // that the borrow therefore would not catch — is a use-after-move.
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
        woken_by(&waker, future)
    }

    /// [`block_on`] with the waker supplied, for the one test whose subject is
    /// what happens when waking the window is itself what goes wrong.
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

    /// The epoch reaches the ENGINE, not only the answer — D3's strong form, at
    /// the one line that makes it true.
    ///
    /// `epoch.rs` decides that `Superseded` flips, and `cairn-git`'s
    /// `cancelling_a_session_stops_the_walk_and_keeps_its_progress` decides
    /// that `next_page` honours a cancel signal it is handed. Neither of them
    /// sees the call site. Handing `next_page` a fresh `CancelSignal` instead
    /// of `epochs.watch(epoch)` leaves every other test in this crate green
    /// while a superseded walk runs to completion on a core and has its answer
    /// thrown away — exactly the weak form the epoch exists to replace. The
    /// tests that look like this one run on [`inbox_only`], which has no
    /// repository behind it and so decides the epoch FILTER alone.
    ///
    /// What a superseded request DID is read off the next page, because that is
    /// where its fates differ:
    ///
    /// - **stopped mid-walk** — its session is young, so the next page starts
    ///   again at the row the scroll opened on;
    /// - **never picked up**, superseded before the worker reached it — the
    ///   previous scroll's session survives untouched, so the next page carries
    ///   on past the rows that one already handed out;
    /// - **ran to completion** — its session is exhausted, so the next page is
    ///   empty.
    ///
    /// Only the first is a stop, and only a cancel signal wired to the epoch
    /// can produce it — with one caveat worth stating rather than leaving to be
    /// discovered: a request that FAILED would also leave the next page
    /// starting at the row the scroll opened on, because `serve` drops a
    /// poisoned session and the cold restart goes back to `HEAD`, and its
    /// `Update::Failed` is filtered out by epoch before anyone sees it. That is
    /// a fourth fate, not a way for the broken version to pass: the mutation
    /// this test exists to catch raises no error. A repository that started
    /// failing would redden every other test in this file first.
    ///
    /// Getting there means superseding a request while its walk is running, and
    /// a walk of this repository is microseconds long — far too short to aim a
    /// `sleep` at, which a first version of this test proved by failing 20 runs
    /// in 25 on a loaded machine. So the worker is given a BATCH of requests
    /// under one epoch instead. It works through them without ever returning to
    /// `recv`, the batch outlasts the wait below by orders of magnitude on any
    /// machine, and so the supersession lands inside a walk rather than inside a
    /// park.
    ///
    /// What is left racing is the gap BETWEEN two requests of the batch — an
    /// answer sent, a wake signalled, the next request received. That gap holds
    /// two syscalls, so on a loaded machine it is not a sliver: measured under
    /// sixteen CPU burners, roughly two rounds in three land in it. It is a
    /// retry rather than a verdict, because landing there shows a request that
    /// ran to completion, which is also what the broken version shows. Rounds
    /// are therefore cheap and many, and the failure is that EVERY one of them
    /// saw a walk finish after being superseded — which is what the broken
    /// version does every time.
    #[test]
    fn superseding_a_request_stops_the_walk_that_is_serving_it() {
        // Larger than this repository can ever answer, so every request below
        // is one that would otherwise walk the whole history.
        let whole_history = 1_000_000;
        // Queued work, not a queued number: what matters is that the worker is
        // still busy when the supersession arrives.
        let batch = 2_000;
        let (handle, mut updates) = cairn();

        // One ordinary round trip. It names the row a scroll opens on, and it
        // measures what an answer costs on THIS machine right now — everything
        // below is scaled to that, because a fixed number of microseconds means
        // nothing on a loaded one.
        let started = Instant::now();
        handle.submit(Request::OpenHistory { rows: 2 });
        let opened_on = match block_on(updates.next()) {
            Some(Update::Rows { rows, .. }) if !rows.is_empty() => *rows[0].id(),
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
            // Posted under ONE epoch, which `submit` cannot do because it bumps
            // per call — every request but the last would be superseded before
            // it was picked up. The worker's own loop does not know the
            // difference: same channel, same epoch test, same walk.
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
                    Some(row) if row.id() == &opened_on => return,
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

    /// The same notice, through the wiring that has to install it.
    ///
    /// The test above builds its own [`WorkerExit`] on its own thread, so it
    /// decides that the guard works when something puts it there — not that
    /// [`open`] does. Deleting `WorkerExit` from `open`'s closure and ending
    /// the closure with `drop(outbox); worker_wake.signal();` keeps clean
    /// shutdown working and every other test in this crate green, while a panic
    /// inside [`serve`] unwinds past that signal: no notice, and no wake for
    /// whoever was already parked on the stream when the worker died. Measured
    /// against that mutation, this test goes red both ways, and which one it
    /// lands on is a race with the unwind — three times in four it spent the
    /// whole ten seconds below, which is the phase-03 deadlock returned, and
    /// once the reader had not yet parked and saw the stream simply end,
    /// without ever being told why. Both are failures; only one of them hangs.
    ///
    /// The panic is raised where one is still reachable on a real worker. The
    /// engine is written not to panic — `unwrap` and its friends are denied
    /// outside tests, and a commit it cannot read comes back as an
    /// `Error::ReadCommit` rather than as an unwind — but
    /// `Wake::signal` calls the window's waker ON THE WORKER THREAD, so a waker
    /// that gives way is a panic inside `serve`, raised by the very line that
    /// answers a request. It gives way exactly once: a second panic while the
    /// notice was being sent would abort the process instead of deciding
    /// anything.
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
                // this reaches must already be able to see that the waker has
                // given way, and it must be woken whether it does or not,
                // because a test that hangs on its own waker decides nothing.
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

        // Everything that can park runs over there, so the bound below bounds
        // the whole test rather than one step of it — and a failure comes back
        // as a value rather than as a panic the hook would silence.
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

            // Ask again until an answer's wake actually runs the waker: a
            // signal that arrives before this thread has parked only latches,
            // and latching raises nothing. Which arrives first once it does —
            // the answer the worker died delivering or the notice of the death
            // — is a race with the unwind, and both are the same news here.
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

        // A worker that dies without announcing itself parks whoever drives
        // `Updates` forever, and that has to come back as a red test rather
        // than as a suite that never finishes.
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

    /// The negative for the two tests above: a clean exit raises no alarm.
    /// Without this, a notice sent unconditionally would also pass.
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

    /// Implemented for [`Outbox`] by hand, and for everything `Clone` by
    /// blanket impl — so the two overlap exactly when `Outbox` is `Clone`, and
    /// an overlap is `error[E0119]`.
    ///
    /// That puts the strongest available tier under "one sender per worker
    /// thread" — but it decides the `Clone`, not the rule. What it cannot say,
    /// stated here rather than left implied: `Sender<Envelope>` is `Clone` and
    /// `Outbox`'s fields are visible throughout this module, so a second sender
    /// written by hand — `Outbox { updates: outgoing.clone(), .. }` beside the
    /// first — still compiles and still passes every test here. That one is a
    /// review obligation, and the reason to keep it a short one: the whole
    /// point of the derive being absent is that nobody reaches for `.clone()`
    /// and finds it there.
    ///
    /// Commit 500f6eb removed a second sender from the worker closure because a
    /// copy outliving [`WorkerExit`] holds the update channel open past the wake
    /// that announces the thread is gone, and the waiting task then sees an
    /// empty channel rather than a closed one and parks forever. No test sees
    /// that: the race is narrow enough that
    /// [`a_failed_open_ends_the_stream_rather_than_leaving_it_open`] passed
    /// against the broken code, and being not-`Clone` was the whole of the fix
    /// with nothing reading it.
    trait OneSenderPerWorker {}
    impl OneSenderPerWorker for Outbox {}
    impl<T: Clone> OneSenderPerWorker for T {}

    /// The bound above, demanded of [`Outbox`]. The assertion is the
    /// compilation, not the body: deriving `Clone` on `Outbox` makes the two
    /// impls of [`OneSenderPerWorker`] overlap, and the test build stops with
    /// "conflicting implementations of trait `OneSenderPerWorker` for type
    /// `Outbox`" rather than this failing. Verified by deriving one. The gate
    /// compiles tests twice over — `cargo clippy --all-targets` and the test
    /// run — so a `Clone` cannot reach it, though `cargo build` alone would
    /// still succeed.
    #[test]
    fn a_worker_thread_cannot_be_given_a_second_sender() {
        fn one_sender_only<T: OneSenderPerWorker>() {}
        one_sender_only::<Outbox>();
    }

    /// The stream must END after a failed open, not just report the failure.
    ///
    /// This pins the contract, and the contract alone: the bug behind it — a
    /// second sender outliving the notice that the worker was gone — was a race
    /// narrow enough that this test passed against the broken code too. What
    /// decides that one is the type, and
    /// [`a_worker_thread_cannot_be_given_a_second_sender`] is where the type is
    /// held to it: [`Outbox`] is not `Clone`, so a second sender on a worker
    /// thread cannot be written.
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
