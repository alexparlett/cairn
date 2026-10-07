//! The history lane's state on the repository thread: the walk held open across pages, the
//! cursor a cold restart resumes from, and the refs it walks from (refs-and-status R4.1,
//! R10, R11.2).
//!
//! The refs are read here, on the thread that owns the walk. A refresh reads them in the
//! refs lane and compares what they would draw with the snapshot the open walk began from
//! (`RefsSnapshot::walks_as`): the answer, [`Update::Refs`], says whether the history must be
//! reopened, and the window — which alone numbers the history lane — asks for the reopen.
//! A refresh never touches the walk itself, so a page being walked is never cancelled by
//! one, and a refresh that finds nothing the graph draws changed leaves the history alone.
//!
//! An open walks from the snapshot the last refresh read. Two cases read their own, in the
//! history lane, and answer it there before the first page: no refresh has read one (or the
//! last open failed, which forgets it), and a walk that fails to open from it — a ref's
//! commit deleted and pruned between the refresh and the open — when the refs read again
//! differ: the open is then made once more, from them.
//!
//! A find (R8.5) pages the same walk forward, a page at a time, until a page holds the row
//! looked for or the walk ends; `pool::serve` walks one of its pages whenever no other job is
//! waiting. Every page — a scroll's or a find's — is answered under the number of the walk it
//! belongs to (`QueryLane::Walk`), not the query's: a page laid out before its find was
//! superseded is still the walk's next page, and the window appends it.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use cairn_git::{Error, HistoryCursor, HistoryRequest, HistorySession, Repository};
use cairn_model::{Oid, RefsSnapshot, RowsPage};

use super::epoch::{Epoch, Epochs, QueryLane, Superseded};
use super::pool::Outbox;
use super::refresh_lane::RefreshJob;
use super::request::{Refreshed, Update};
use super::routing::Page;

#[derive(Default)]
pub(super) struct HistoryLane<'repo> {
    session: Option<HistorySession<'repo>>,
    /// `None` means the scroll has not started.
    cursor: Option<HistoryCursor>,
    /// The snapshot the open walk began from: what a refresh compares with.
    walked_from: Option<Arc<RefsSnapshot>>,
    /// The snapshot the last refresh read: what the next open walks from.
    latest: Option<Arc<RefsSnapshot>>,
    /// The walk lane's number for the walk open: what its pages are answered under.
    walk: Option<Epoch>,
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
pub(super) struct Finding {
    pub target: Oid,
    pub rows: usize,
}

/// Where a lane's answers go, and how it is cancelled.
pub(super) struct Answering<'a> {
    pub epochs: &'a Epochs,
    pub outbox: &'a Outbox,
}

impl<'repo> HistoryLane<'repo> {
    /// A refresh's first read (R10.1): the refs as they stand, under the refs lane's
    /// `epoch`, which also cancels it. Answered as [`Update::Refs`] — `reopen` when what the
    /// history draws differs from the snapshot its walk began from, or no walk has begun —
    /// and handed on to the refresh thread for ahead/behind, under `ahead_behind`. A snapshot
    /// equal to the one held is answered as that one, the new read freed here.
    pub(super) fn refresh(
        &mut self,
        repo: &Repository,
        epoch: Epoch,
        ahead_behind: Epoch,
        refresh: &Sender<RefreshJob>,
        answering: &Answering<'_>,
    ) {
        let read = match repo.refs(&answering.epochs.watch(epoch)) {
            Ok(read) => read,
            // Superseded by the next refresh: not a failure.
            Err(Error::RefsCancelled) => return,
            Err(error) => {
                answering.outbox.send(
                    Some(epoch),
                    Update::RefreshFailed {
                        what: Refreshed::Refs,
                        message: error.to_string(),
                    },
                );
                return;
            }
        };
        let snapshot = match &self.latest {
            Some(held) if **held == read.snapshot => Arc::clone(held),
            _ => Arc::new(read.snapshot),
        };
        // A failed send means the refresh thread has gone with the repository.
        let _ = refresh.send(RefreshJob::AheadBehind {
            epoch: ahead_behind,
            snapshot: Arc::clone(&snapshot),
        });
        let reopen = !self
            .walked_from
            .as_ref()
            .is_some_and(|walked| walked.walks_as(&snapshot));
        self.latest = Some(Arc::clone(&snapshot));
        answering
            .outbox
            .send(Some(epoch), Update::Refs { snapshot, reopen });
    }

    /// Answers one page under the history lane's `epoch`, which also cancels it.
    pub(super) fn page(
        &mut self,
        repo: &'repo Repository,
        page: Page,
        epoch: Epoch,
        answering: &Answering<'_>,
    ) {
        let rows = match page {
            Page::Open { rows, walk } => {
                // A different scroll: drop the open walk first.
                self.replace_walk(walk);
                rows
            }
            Page::More { rows } => rows,
            // `pool::serve` walks a find a page at a time (`find_page`), and a stop asks nothing.
            Page::Find { .. } | Page::Stop => return,
        };
        self.answer(repo, rows, None, epoch, answering);
    }

    /// Lets go of the walk open for the one an open numbered `walk` begins, without walking
    /// it: the open was superseded before it started, and what superseded it pages the new
    /// walk from its start.
    pub(super) fn replace_walk(&mut self, walk: Epoch) {
        self.session = None;
        self.cursor = None;
        self.walked_from = None;
        self.walk = Some(walk);
    }

    /// One page of `find`, under the history lane's `epoch`, which also cancels it: answered
    /// as a scroll's page is. Says whether to walk another — not when the page held the row,
    /// the walk ended, the find was superseded or the walk failed.
    pub(super) fn find_page(
        &mut self,
        repo: &'repo Repository,
        find: Finding,
        epoch: Epoch,
        answering: &Answering<'_>,
    ) -> bool {
        self.answer(repo, find.rows, Some(find.target), epoch, answering)
            .is_some_and(|paged| !paged.found && !paged.complete)
    }

    /// Answers the walk's next `rows` rows under the history lane's `epoch`, which also
    /// cancels the walk: from the walk held, one restarted cold from the last good page's
    /// cursor, or — with neither — one opened now. Says what the page answered, and whether
    /// it held `target`'s row; `None` when nothing was answered — superseded mid-page (the
    /// session keeps its rows), or failed, which is said.
    fn answer(
        &mut self,
        repo: &'repo Repository,
        rows: usize,
        target: Option<Oid>,
        epoch: Epoch,
        answering: &Answering<'_>,
    ) -> Option<Paged> {
        let cancel = answering.epochs.watch(epoch);
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
                None => return self.open(repo, rows, target, epoch, answering),
            }
        }
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
        &self,
        rows: RowsPage,
        complete: bool,
        target: Option<Oid>,
        answering: &Answering<'_>,
    ) -> Paged {
        let found = target.is_some_and(|target| rows.ids().any(|id| id == target));
        answering.outbox.send(
            self.walk
                .or_else(|| Some(answering.epochs.current(QueryLane::Walk))),
            Update::Rows { rows, complete },
        );
        Paged { found, complete }
    }

    /// A new walk from every ref, and its first page.
    fn open(
        &mut self,
        repo: &'repo Repository,
        rows: usize,
        target: Option<Oid>,
        epoch: Epoch,
        answering: &Answering<'_>,
    ) -> Option<Paged> {
        if self.walk.is_none() {
            self.walk = Some(answering.epochs.current(QueryLane::Walk));
        }
        let cancel = answering.epochs.watch(epoch);
        let (snapshot, read_now) = match self.latest.clone() {
            Some(snapshot) => (snapshot, false),
            None => match repo.refs(&cancel) {
                Ok(read) => (Arc::new(read.snapshot), true),
                Err(Error::RefsCancelled) => return None,
                Err(error) => {
                    self.failed(error, epoch, answering);
                    return None;
                }
            },
        };
        if read_now {
            self.latest = Some(Arc::clone(&snapshot));
            answering.outbox.send(
                Some(epoch),
                Update::Refs {
                    snapshot: Arc::clone(&snapshot),
                    reopen: false,
                },
            );
        }
        let error = match self.first_page(repo, &snapshot, rows, &cancel) {
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
                        self.latest = Some(Arc::clone(&fresh));
                        answering.outbox.send(
                            Some(epoch),
                            Update::Refs {
                                snapshot: Arc::clone(&fresh),
                                reopen: false,
                            },
                        );
                        match self.first_page(repo, &fresh, rows, &cancel) {
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
        self.failed(error, epoch, answering);
        None
    }

    /// Opens the walk from `snapshot` and reads its first page.
    fn first_page(
        &mut self,
        repo: &'repo Repository,
        snapshot: &Arc<RefsSnapshot>,
        rows: usize,
        cancel: &Superseded,
    ) -> Result<(RowsPage, bool), Error> {
        self.session = Some(repo.history_session(&HistoryRequest::from_refs(snapshot, rows))?);
        self.walked_from = Some(Arc::clone(snapshot));
        self.next_page(rows, cancel)
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

    /// The walk failed: it is forgotten with the refs it began from, so the next open reads
    /// them afresh and the next refresh reopens.
    fn failed(&mut self, error: Error, epoch: Epoch, answering: &Answering<'_>) {
        self.session = None;
        if self.cursor.is_none() {
            self.walked_from = None;
            self.latest = None;
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

    /// The cancel a scroll and a find share (R8.5): a page asked under a number already
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
        let mut lane = HistoryLane::default();
        let opened = epochs.bump(QueryLane::History);
        let walk = epochs.bump(QueryLane::Walk);
        lane.page(&repo, Page::Open { rows: 2, walk }, opened, &answering);
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
        lane.page(&repo, Page::More { rows: 1_000_000 }, stale, &answering);
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

        lane.page(&repo, Page::More { rows: 2 }, current, &answering);
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
        let mut lane = HistoryLane {
            latest: Some(Arc::new(stale)),
            ..HistoryLane::default()
        };
        let epoch = epochs.bump(QueryLane::History);
        let walk = epochs.bump(QueryLane::Walk);
        lane.page(&repo, Page::Open { rows: 3, walk }, epoch, &answering);
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
            lane.walked_from.as_deref(),
            Some(&fresh),
            "the walk is not the one from the refs read again"
        );
    }
}
