//! The history thread, `cairn-history`: the history lane of one repository — the walk held
//! open across pages, the cursor a cold restart resumes from, and a find's pages — on a thread
//! of its own, with its own thread-local repository handle, so an opening pass that reads the
//! whole history holds up nothing but the history: the refs, the filters and the freeing of old
//! answers are the repository thread's.
//!
//! An open walks from the snapshot the last refresh read on the repository thread
//! (`refs.rs`). Two cases read their own, in the history lane, and answer it there before the
//! first page: no refresh has read one (or the last open failed, which forgets it), and a walk
//! that fails to open from it — a ref's commit deleted and pruned between the refresh and the
//! open — when the refs read again differ: the open is then made once more, from them.
//!
//! **Cancellation.** An open's opening pass and its first page run under the walk's own number
//! (`QueryLane::Walk`), which only the next open moves — a reopen asked because the refs moved,
//! Show Lost Commits toggled — or a close stops: a find, or a find's stop, asked while a walk
//! opens leaves the pass running, and is served once the first page is out. Every later page —
//! a scroll's or a find's — runs under the history lane's number of the query that asked it, so
//! the next press, scroll or stop supersedes it. The pass tells the window how far it has read
//! once it has run 250 ms, then at most once per 100 ms (`reading.rs`).
//!
//! A find pages the same walk forward, a page at a time, until a page holds the row looked
//! for or the walk ends; [`serve_history`] walks one of its pages whenever no other job is
//! waiting. Every page — a scroll's or a find's — is answered under the number of the walk it
//! belongs to, not the query's: a page laid out before its find was superseded is still the
//! walk's next page, and the window appends it.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use cairn_git::{
    Error, HistoryCursor, HistoryRequest, HistorySession, Repository, SharedRepository,
};
use cairn_model::{Oid, RefsSnapshot, RowsPage};

use crate::worker::epoch::{Epoch, Epochs, QueryLane, Superseded};
use crate::worker::history::reading::{Pacing, ReadingPace};
use crate::worker::history::refs::WalkedRefs;
use crate::worker::pool::Outbox;
use crate::worker::request::Update;
use crate::worker::routing::{HistoryJob, Page};

pub(crate) struct HistoryLane<'repo> {
    session: Option<HistorySession<'repo>>,
    /// `None` means the scroll has not started.
    cursor: Option<HistoryCursor>,
    /// The refs the walk opens from, shared with the repository thread that reads them.
    refs: WalkedRefs,
    /// The walk lane's number for the walk open: what its pages are answered under, and what
    /// cancels its opening pass.
    walk: Option<Epoch>,
    /// Whether the walk open — and the next one opened, should it fail — is Show Lost
    /// Commits': the open that began it said so.
    lost: bool,
    /// When the window is told how far an opening pass has read.
    pacing: Pacing,
}

/// What a page answered: whether it held the row a find looks for, and whether the walk
/// ended with it.
#[derive(Debug, Clone, Copy)]
struct Paged {
    found: bool,
    complete: bool,
}

/// A find in progress: the commit whose row is looked for — a commit's own, or a stash's
/// stash commit — and how many rows each of its pages walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Finding {
    pub target: Oid,
    pub rows: usize,
}

/// Where a lane's answers go, and how it is cancelled.
pub(crate) struct Answering<'a> {
    pub epochs: &'a Epochs,
    pub outbox: &'a Outbox,
}

/// What the history thread is handed to serve with: the epochs it is cancelled by, its queue,
/// its outbox, and the refs it shares with the repository thread.
pub(crate) struct Serving<'a> {
    pub epochs: &'a Epochs,
    pub jobs: &'a Receiver<HistoryJob>,
    pub outbox: &'a Outbox,
    pub refs: WalkedRefs,
}

/// Serves the history lane until told to stop, or until every sender has gone. A find in
/// progress walks one page whenever no other job is waiting, so the next page, find or stop —
/// whose number supersedes it — is taken up between its pages.
pub(crate) fn serve_history(shared: &SharedRepository, serving: &Serving<'_>) {
    let epochs = serving.epochs;
    let repo = shared.to_worker();
    // Once, outside the loop: the scroll's session borrows `repo` across turns.
    let mut lane = HistoryLane::new(serving.refs.clone());
    let answering = Answering {
        epochs,
        outbox: serving.outbox,
    };
    let mut finding: Option<(Epoch, Finding)> = None;
    loop {
        let job = match finding {
            Some((epoch, find)) => match serving.jobs.try_recv() {
                Ok(job) => job,
                Err(TryRecvError::Empty) => {
                    finding = (!epochs.is_stopping()
                        && epochs.is_current(epoch)
                        && lane.find_page(&repo, find, epoch, &answering))
                    .then_some((epoch, find));
                    continue;
                }
                Err(TryRecvError::Disconnected) => break,
            },
            None => match serving.jobs.recv() {
                Ok(job) => job,
                Err(_) => break,
            },
        };
        if epochs.is_stopping() {
            break;
        }
        let HistoryJob::Page { epoch, page } = job else {
            break;
        };
        match page {
            // Whatever the history lane has asked since — a find, a stop — an open whose walk is
            // still the window's runs: its pass is superseded by the next open alone.
            Page::Open { rows, walk, lost } => {
                if epochs.is_current(walk) {
                    lane.open_walk(&repo, rows, walk, lost, &answering);
                }
            }
            Page::More { rows } => {
                if epochs.is_current(epoch) {
                    lane.more(&repo, rows, epoch, &answering);
                }
            }
            Page::Find { target, rows } => {
                if epochs.is_current(epoch) {
                    finding = Some((epoch, Finding { target, rows }));
                }
            }
            // Its number superseded the find; there is nothing to walk.
            Page::Stop => finding = None,
        }
    }
}

impl<'repo> HistoryLane<'repo> {
    pub(crate) fn new(refs: WalkedRefs) -> Self {
        Self {
            session: None,
            cursor: None,
            refs,
            walk: None,
            lost: false,
            pacing: Pacing::default(),
        }
    }

    /// Drops the walk open and opens the one numbered `walk` — Show Lost Commits' when `lost` —
    /// answering its first page; its opening pass and that page run under `walk`'s number.
    pub(crate) fn open_walk(
        &mut self,
        repo: &'repo Repository,
        rows: usize,
        walk: Epoch,
        lost: bool,
        answering: &Answering<'_>,
    ) {
        self.session = None;
        self.cursor = None;
        self.walk = Some(walk);
        self.lost = lost;
        self.open(repo, rows, None, answering);
    }

    /// The walk's next `rows` rows, a scroll's, under the history lane's `epoch`, which also
    /// cancels them.
    pub(crate) fn more(
        &mut self,
        repo: &'repo Repository,
        rows: usize,
        epoch: Epoch,
        answering: &Answering<'_>,
    ) {
        self.answer(repo, rows, None, epoch, answering);
    }

    /// One page of `find`, under the history lane's `epoch`, which also cancels it: answered
    /// as a scroll's page is. Says whether to walk another — not when the page held the row,
    /// the walk ended, the find was superseded or the walk failed.
    pub(crate) fn find_page(
        &mut self,
        repo: &'repo Repository,
        find: Finding,
        epoch: Epoch,
        answering: &Answering<'_>,
    ) -> bool {
        self.answer(repo, find.rows, Some(find.target), epoch, answering)
            .is_some_and(|paged| !paged.found && !paged.complete)
    }

    /// The walk's number, or — when no open has numbered one — the walk lane's now.
    fn walk(&mut self, answering: &Answering<'_>) -> Epoch {
        *self
            .walk
            .get_or_insert_with(|| answering.epochs.current(QueryLane::Walk))
    }

    /// Answers the walk's next `rows` rows under the history lane's `epoch`, which also
    /// cancels them: from the walk held, one restarted cold from the last good page's cursor
    /// — its opening pass under the walk's number — or, with neither, one opened now. Says
    /// what the page answered, and whether it held `target`'s row; `None` when nothing was
    /// answered — superseded mid-page (the session keeps its rows), or failed, which is said.
    fn answer(
        &mut self,
        repo: &'repo Repository,
        rows: usize,
        target: Option<Oid>,
        epoch: Epoch,
        answering: &Answering<'_>,
    ) -> Option<Paged> {
        if self.session.is_none() {
            match self.cursor.clone() {
                // Cold restart from the last good page, from the snapshot it began from.
                Some(at) => match repo.history_session(&HistoryRequest::resume(at, rows)) {
                    Ok(session) => self.session = Some(session),
                    Err(error) => {
                        self.failed(error, epoch, answering);
                        return None;
                    }
                },
                None => return self.open(repo, rows, target, answering),
            }
            // Rows are drawn already: the strip says a walk replacing them reads, and this one
            // replaces nothing, so the window is told no count.
            match self.open_pass(answering, false) {
                Ok(()) => {}
                Err(Error::Cancelled { .. }) => return None,
                Err(error) => {
                    self.failed(error, epoch, answering);
                    return None;
                }
            }
        }
        let cancel = answering.epochs.watch(epoch);
        match self.next_page(rows, &cancel) {
            Ok((page, complete)) => Some(self.send_rows(page, complete, target, answering)),
            // Superseded mid-page: the session keeps its rows. Nothing is sent.
            Err(Error::Cancelled { .. }) => None,
            Err(error) => {
                self.failed(error, epoch, answering);
                None
            }
        }
    }

    /// A page of the walk open, answered under the walk's number; whether it held
    /// `target`'s row is read off it first.
    fn send_rows(
        &mut self,
        rows: RowsPage,
        complete: bool,
        target: Option<Oid>,
        answering: &Answering<'_>,
    ) -> Paged {
        let found = target.is_some_and(|target| rows.ids().any(|id| id == target));
        let walk = self.walk(answering);
        answering
            .outbox
            .send(Some(walk), Update::Rows { rows, complete });
        Paged { found, complete }
    }

    /// A new walk from every ref, and its first page, under the walk's number: what it opens
    /// from is read off the shared refs, or read now.
    fn open(
        &mut self,
        repo: &'repo Repository,
        rows: usize,
        target: Option<Oid>,
        answering: &Answering<'_>,
    ) -> Option<Paged> {
        let walk = self.walk(answering);
        let cancel = answering.epochs.watch(walk);
        let (snapshot, read_now) = match self.refs.opening() {
            Some(snapshot) => (snapshot, false),
            None => match repo.refs(&cancel) {
                Ok(read) => (Arc::new(read.snapshot), true),
                Err(Error::RefsCancelled) => return None,
                Err(error) => {
                    self.failed(error, walk, answering);
                    return None;
                }
            },
        };
        if read_now {
            self.refs.read_for_open(&snapshot);
            answering.outbox.send(
                Some(walk),
                Update::Refs {
                    snapshot: Arc::clone(&snapshot),
                    reopen: false,
                },
            );
        }
        let error = match self.first_page(repo, &snapshot, rows, &cancel, answering) {
            Ok((page, complete)) => return Some(self.send_rows(page, complete, target, answering)),
            Err(Error::Cancelled { .. }) => return None,
            Err(error) if read_now => error,
            Err(error) => {
                // A ref's commit may have gone since the refresh read it (deleted and
                // pruned): read the refs again and, when they draw something else, open
                // from them instead — once.
                match repo.refs(&cancel) {
                    Ok(read) if !read.snapshot.walks_as(&snapshot) => {
                        let fresh = Arc::new(read.snapshot);
                        self.refs.read_for_open(&fresh);
                        answering.outbox.send(
                            Some(walk),
                            Update::Refs {
                                snapshot: Arc::clone(&fresh),
                                reopen: false,
                            },
                        );
                        match self.first_page(repo, &fresh, rows, &cancel, answering) {
                            Ok((page, complete)) => {
                                return Some(self.send_rows(page, complete, target, answering));
                            }
                            Err(Error::Cancelled { .. }) => return None,
                            Err(again) => again,
                        }
                    }
                    Err(Error::RefsCancelled) => return None,
                    // The same refs, or none to read: the open's own failure stands.
                    Ok(_) | Err(_) => error,
                }
            }
        };
        self.failed(error, walk, answering);
        None
    }

    /// Opens the walk from `snapshot` — its opening pass — and reads its first page, both
    /// under `cancel`, the walk's.
    fn first_page(
        &mut self,
        repo: &'repo Repository,
        snapshot: &Arc<RefsSnapshot>,
        rows: usize,
        cancel: &Superseded,
        answering: &Answering<'_>,
    ) -> Result<(RowsPage, bool), Error> {
        let request = HistoryRequest::from_refs(snapshot, rows);
        let request = if self.lost {
            request.with_lost_commits()
        } else {
            request
        };
        self.session = Some(repo.history_session(&request)?);
        self.open_pass(answering, true)?;
        self.next_page(rows, cancel)
    }

    /// The session's opening pass, under the walk's number — the window told how far it has
    /// read on the pace `reading.rs` sets when `told`, as for a walk its first page will open.
    /// Any failure but a cancel drops the session.
    fn open_pass(&mut self, answering: &Answering<'_>, told: bool) -> Result<(), Error> {
        let walk = self.walk(answering);
        let cancel = answering.epochs.watch(walk);
        let mut pace = ReadingPace::starting(Instant::now(), self.pacing);
        let Some(session) = self.session.as_mut() else {
            return Ok(());
        };
        let opened = session.open_walk(&cancel, self.pacing.every, &mut |commits| {
            if told && pace.due(Instant::now()) {
                answering
                    .outbox
                    .send(Some(walk), Update::Reading { commits });
            }
        });
        if let Err(error) = &opened
            && !matches!(error, Error::Cancelled { .. })
        {
            self.session = None;
        }
        opened
    }

    /// The next page of the walk open. Any failure but a cancel drops the session, so the
    /// next request cold-restarts from the last good cursor.
    fn next_page(&mut self, rows: usize, cancel: &Superseded) -> Result<(RowsPage, bool), Error> {
        let Some(session) = self.session.as_mut() else {
            return Ok((RowsPage::new(), true));
        };
        match session.next_page(rows, cancel) {
            Ok(page) => {
                let complete = page.cursor.is_none();
                self.cursor = page.cursor;
                Ok((page.rows, complete))
            }
            Err(error) => {
                if !matches!(error, Error::Cancelled { .. }) {
                    self.session = None;
                }
                Err(error)
            }
        }
    }

    /// The walk failed, said under `epoch`: it is forgotten with the refs it began from, so
    /// the next open reads them afresh and the next refresh reopens.
    fn failed(&mut self, error: Error, epoch: Epoch, answering: &Answering<'_>) {
        self.session = None;
        if self.cursor.is_none() {
            self.refs.forget();
        }
        answering.outbox.send(
            Some(epoch),
            Update::Failed {
                message: error.to_string(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use cairn_model::{Oid, RefName, RefTarget};

    use super::*;
    use crate::worker::Refreshable;
    use crate::worker::epoch::QueryLane;

    /// The cancel a scroll and a find share: a page asked under a number already
    /// superseded stops at its first commit and sends nothing, the walk keeping its place, and
    /// the next page asked under the current number takes the walk up where the last one sent
    /// left it. Driven on the lane itself, over a line of commits written for it (not this
    /// checkout's history, #61) and longer than the lane assigner's window
    /// (`LaneAssigner::DEFAULT_WINDOW`), so its first page does not walk it to its end; nothing
    /// is timed. Caught by: the walk handed a fresh `CancelSignal` rather than the epoch (the
    /// stale page walks the whole line), the walk watched under the walk lane's number rather
    /// than the page's (the same), or a stale page that drops the walk.
    #[test]
    fn a_page_asked_under_a_superseded_number_walks_nothing_and_the_next_takes_the_walk_up() {
        let line = crate::worker::written_repository::WrittenRepository::linear(
            "cairn-superseded-page",
            cairn_model::LaneAssigner::DEFAULT_WINDOW + 64,
        );
        let repo = Repository::discover(line.path())
            .unwrap_or_else(|error| panic!("opening the line: {error}"));
        let epochs = Epochs::new();
        let (outbox, sent) = Outbox::watched();
        let answering = Answering {
            epochs: &epochs,
            outbox: &outbox,
        };
        let mut lane = HistoryLane::new(WalkedRefs::default());
        epochs.bump(QueryLane::History);
        let walk = epochs.bump(QueryLane::Walk);
        lane.open_walk(&repo, 2, walk, false, &answering);
        let first: Vec<Update> = sent.try_iter().map(|sent| sent.opened().1).collect();
        let first_rows: Vec<Oid> = first
            .iter()
            .filter_map(|update| match update {
                Update::Rows { rows, .. } => Some(rows.ids().collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(first_rows, line.commits[..2]);

        let stale = epochs.bump(QueryLane::History);
        let current = epochs.bump(QueryLane::History);
        lane.more(&repo, 1_000_000, stale, &answering);
        let after_stale: Vec<(Option<Epoch>, Update)> =
            sent.try_iter().map(|sent| sent.opened()).collect();
        assert!(
            after_stale.is_empty(),
            "a page asked under a superseded number sent {after_stale:?}"
        );
        assert!(
            lane.session.is_some(),
            "the superseded page dropped the walk"
        );

        lane.more(&repo, 2, current, &answering);
        let next: Vec<(Option<Epoch>, Update)> =
            sent.try_iter().map(|sent| sent.opened()).collect();
        match next.as_slice() {
            [(epoch, Update::Rows { rows, complete })] => {
                assert_eq!(
                    *epoch,
                    Some(walk),
                    "a page not answered under its walk's number"
                );
                assert_eq!(rows.ids().collect::<Vec<_>>(), line.commits[2..4]);
                assert!(!complete, "the walk ended early");
            }
            other => panic!("expected the next two rows, got {other:?}"),
        }
    }

    /// An opening pass is superseded by the next walk alone: an open whose walk lane has moved
    /// on — a reopen asked after it — ends its pass at the first commit it would read and sends
    /// nothing, in a sliver of the time the same pass takes when its walk is current, which is
    /// read first; one whose history lane alone moved on — a find and its stop asked after it —
    /// runs its pass and sends its first page. Over a line long enough that its pass takes a
    /// measurable while. Caught by: the pass handed a fresh `CancelSignal` or a number nothing
    /// moves (the superseded open runs the whole pass), or the open sent under the history lane's
    /// number (its first page dropped as superseded).
    #[test]
    fn an_opening_pass_is_superseded_by_the_next_walk_alone() {
        let line = crate::worker::written_repository::WrittenRepository::packed(
            "cairn-pass-superseded",
            100_000,
        );
        let repo = Repository::discover(line.path())
            .unwrap_or_else(|error| panic!("opening the line: {error}"));
        let epochs = Epochs::new();
        let (outbox, sent) = Outbox::watched();
        let answering = Answering {
            epochs: &epochs,
            outbox: &outbox,
        };
        let mut lane = HistoryLane::new(WalkedRefs::default());

        epochs.bump(QueryLane::History);
        let walk = epochs.bump(QueryLane::Walk);
        // A find and its stop, asked after the open.
        epochs.bump(QueryLane::History);
        epochs.bump(QueryLane::History);
        let started = Instant::now();
        lane.open_walk(&repo, 3, walk, false, &answering);
        let pass = started.elapsed();
        let answered: Vec<(Option<Epoch>, Update)> =
            sent.try_iter().map(|sent| sent.opened()).collect();
        let first = answered.iter().find_map(|(epoch, update)| match update {
            Update::Rows { rows, .. } => Some((*epoch, rows.ids().collect::<Vec<_>>())),
            _ => None,
        });
        assert_eq!(first, Some((Some(walk), line.commits[..3].to_vec())));

        epochs.bump(QueryLane::History);
        let superseded = epochs.bump(QueryLane::Walk);
        // The reopen asked after it.
        epochs.bump(QueryLane::Walk);
        let started = Instant::now();
        lane.open_walk(&repo, 3, superseded, false, &answering);
        let stopped = started.elapsed();
        let after: Vec<(Option<Epoch>, Update)> =
            sent.try_iter().map(|sent| sent.opened()).collect();
        assert!(after.is_empty(), "a superseded open sent {after:?}");
        assert!(
            stopped < pass / 10,
            "a superseded open took {stopped:?} of a {pass:?} pass"
        );
    }

    /// A cold restart — the walk dropped by a failed page, the next resumed from the last good
    /// page's cursor — runs its opening pass again but tells the window no count: its rows are
    /// drawn already, and the strip says a walk replacing them reads. Here on a pace with no
    /// hold-back and no interval, on which the open before it told its counts. Caught by: a cold
    /// restart that tells its pass's counts (the strip drawn over rows nothing is replacing).
    #[test]
    fn a_cold_restart_tells_the_window_no_count() {
        let line = crate::worker::written_repository::WrittenRepository::packed(
            "cairn-cold-restart",
            20_000,
        );
        let repo = Repository::discover(line.path())
            .unwrap_or_else(|error| panic!("opening the line: {error}"));
        let epochs = Epochs::new();
        let (outbox, sent) = Outbox::watched();
        let answering = Answering {
            epochs: &epochs,
            outbox: &outbox,
        };
        let mut lane = HistoryLane::new(WalkedRefs::default());
        lane.pacing = Pacing {
            after: std::time::Duration::ZERO,
            every: std::time::Duration::ZERO,
        };
        let walk = epochs.bump(QueryLane::Walk);
        lane.open_walk(&repo, 3, walk, false, &answering);
        let opening: Vec<Update> = sent.try_iter().map(|sent| sent.opened().1).collect();
        assert!(
            opening
                .iter()
                .any(|update| matches!(update, Update::Reading { .. })),
            "the open told no count"
        );

        // As a failed page leaves it: the walk dropped, its cursor kept.
        lane.session = None;
        let more = epochs.bump(QueryLane::History);
        lane.more(&repo, 2, more, &answering);
        let resumed: Vec<Update> = sent.try_iter().map(|sent| sent.opened().1).collect();
        assert!(
            !resumed
                .iter()
                .any(|update| matches!(update, Update::Reading { .. })),
            "a cold restart told its count"
        );
        let rows: Vec<Oid> = resumed
            .iter()
            .filter_map(|update| match update {
                Update::Rows { rows, .. } => Some(rows.ids().collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(
            rows,
            line.commits[3..5],
            "the restart did not take the walk up"
        );
    }

    /// The window is told how far an opening pass has read, under the walk's number and before
    /// its first page, the count rising and never past the commits in the line — here on a pace
    /// with no hold-back and no interval, so every count the walk reports is told; a pass shorter
    /// than its hold-back tells nothing. Caught by: the walk's
    /// count dropped on its way to the window (none told), told under another number than the
    /// walk's (dropped as superseded), or told on the window's pace before the pass has run its
    /// hold-back.
    #[test]
    fn the_window_is_told_how_far_an_opening_pass_has_read() {
        let line = crate::worker::written_repository::WrittenRepository::packed(
            "cairn-pass-counted",
            20_000,
        );
        let repo = Repository::discover(line.path())
            .unwrap_or_else(|error| panic!("opening the line: {error}"));
        let epochs = Epochs::new();
        let (outbox, sent) = Outbox::watched();
        let answering = Answering {
            epochs: &epochs,
            outbox: &outbox,
        };
        let mut lane = HistoryLane::new(WalkedRefs::default());
        lane.pacing = Pacing {
            after: std::time::Duration::ZERO,
            every: std::time::Duration::ZERO,
        };
        let walk = epochs.bump(QueryLane::Walk);
        lane.open_walk(&repo, 3, walk, false, &answering);
        let answered: Vec<(Option<Epoch>, Update)> =
            sent.try_iter().map(|sent| sent.opened()).collect();
        let counts: Vec<usize> = answered
            .iter()
            .take_while(|(_, update)| !matches!(update, Update::Rows { .. }))
            .filter_map(|(epoch, update)| match update {
                Update::Reading { commits } => {
                    assert_eq!(*epoch, Some(walk), "a count told under another number");
                    Some(*commits)
                }
                _ => None,
            })
            .collect();
        assert!(counts.len() >= 4, "{counts:?}");
        assert!(
            counts.windows(2).all(|pair| pair[0] < pair[1]),
            "{counts:?}"
        );
        assert!(counts.iter().all(|&count| count <= line.commits.len()));
        assert!(
            answered
                .iter()
                .any(|(_, update)| matches!(update, Update::Rows { .. })),
            "no first page"
        );

        // Every count the walk reports, but a hold-back longer than the pass.
        let mut paced = HistoryLane::new(WalkedRefs::default());
        paced.pacing = Pacing {
            after: std::time::Duration::from_secs(3600),
            every: std::time::Duration::ZERO,
        };
        let walk = epochs.bump(QueryLane::Walk);
        paced.open_walk(&repo, 3, walk, false, &answering);
        let told = sent
            .try_iter()
            .map(|sent| sent.opened().1)
            .filter(|update| matches!(update, Update::Reading { .. }))
            .count();
        assert_eq!(told, 0, "a pass shorter than the hold-back told its count");
    }

    /// An open asked as Show Lost Commits' walks from the
    /// reflog too and carries the commits only it reaches as lost; and the open after it,
    /// asked plainly, walks from the refs alone. `main` moved back two commits, `HEAD`'s log
    /// naming the newest as an entry's old id only. Caught by: the open's `lost` dropped
    /// between the window and the walk, or kept for the plain open after it.
    #[test]
    fn an_open_asked_as_show_lost_commits_walks_the_reflog_and_the_next_plain_open_does_not() {
        let line =
            crate::worker::written_repository::WrittenRepository::linear("cairn-lost-open", 6);
        let [newest, second, kept, ..] = line.commits[..] else {
            panic!("six commits: {:?}", line.commits);
        };
        let git_dir = line.path().join(".git");
        let write = |path: std::path::PathBuf, text: String| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
            }
            std::fs::write(&path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        };
        write(git_dir.join("refs/heads/main"), format!("{kept}\n"));
        write(
            git_dir.join("logs/HEAD"),
            format!(
                "{newest} {kept} Ada <ada@example.com> 1600000100 +0000\treset: moving to HEAD~2\n"
            ),
        );
        let repo = Repository::discover(line.path())
            .unwrap_or_else(|error| panic!("opening the line: {error}"));
        let epochs = Epochs::new();
        let (outbox, sent) = Outbox::watched();
        let answering = Answering {
            epochs: &epochs,
            outbox: &outbox,
        };
        let mut lane = HistoryLane::new(WalkedRefs::default());
        let mut open = |lost: bool| -> Vec<(Oid, bool)> {
            epochs.bump(QueryLane::History);
            let walk = epochs.bump(QueryLane::Walk);
            lane.open_walk(&repo, 64, walk, lost, &answering);
            sent.try_iter()
                .filter_map(|sent| match sent.opened().1 {
                    Update::Rows { rows, .. } => Some(
                        rows.ids()
                            .enumerate()
                            .map(|(at, id)| (id, rows.is_lost(at)))
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .flatten()
                .collect()
        };
        let mut expected: Vec<(Oid, bool)> = vec![(newest, true), (second, true)];
        expected.extend(line.commits[2..].iter().map(|id| (*id, false)));
        assert_eq!(open(true), expected, "Show Lost Commits' walk");
        assert_eq!(open(false), expected[2..], "the plain walk after it");
    }

    /// A stale tip: a ref whose commit has gone since the refresh read
    /// it — deleted and pruned in between — fails the walk's open from that snapshot; the
    /// open reads the refs again, says so in the history lane, and opens once more from
    /// them, so the first page arrives rather than a failure. The stale snapshot is the
    /// fixture's own refs read, with `topic` naming a commit no object store holds. Caught
    /// by: the open's failure sent as it was (a history that stays failed until the next
    /// refresh), or a retry from the same stale snapshot.
    #[test]
    fn an_open_from_refs_gone_stale_reads_them_again_and_opens_from_those() {
        let fixture = Refreshable::new("cairn-stale-tip");
        let repo = match Repository::discover(fixture.path()) {
            Ok(repo) => repo,
            Err(error) => panic!("opening the fixture: {error}"),
        };
        let fresh = match repo.refs(&cairn_git::CancelSignal::new()) {
            Ok(read) => read.snapshot,
            Err(error) => panic!("reading the fixture's refs: {error}"),
        };
        let gone = Oid::from_bytes(&[0x11; 20]).unwrap_or_else(|error| panic!("{error}"));
        let mut stale = fresh.clone();
        let Some(topic) = stale
            .refs
            .iter_mut()
            .find(|listed| listed.name == RefName::new("refs/heads/topic"))
        else {
            panic!("the fixture has no topic: {stale:?}");
        };
        topic.target = RefTarget::Commit(gone);

        let epochs = Epochs::new();
        let (outbox, sent) = Outbox::watched();
        let answering = Answering {
            epochs: &epochs,
            outbox: &outbox,
        };
        let mut lane = HistoryLane::new(WalkedRefs::holding(stale));
        epochs.bump(QueryLane::History);
        let walk = epochs.bump(QueryLane::Walk);
        lane.open_walk(&repo, 3, walk, false, &answering);
        drop(outbox);

        let answers: Vec<Update> = sent.try_iter().map(|sent| sent.opened().1).collect();
        match answers.as_slice() {
            [Update::Refs { snapshot, reopen }, Update::Rows { rows, .. }] => {
                assert_eq!(**snapshot, fresh, "the refs read again are not the refs");
                assert!(!reopen, "the open's own refs said to reopen");
                assert_eq!(rows.len(), 3);
                assert!(rows.ids().all(|id| id != gone));
            }
            other => panic!("expected the refs read again and the first page, got {other:?}"),
        }
        assert_eq!(
            lane.refs.walked_from().as_deref(),
            Some(&fresh),
            "the walk is not the one from the refs read again"
        );
    }
}
