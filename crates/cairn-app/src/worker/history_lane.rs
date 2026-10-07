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
//! differ: the open is then made once more, from them (the decision phase 05 handed on).

use std::sync::Arc;
use std::sync::mpsc::Sender;

use cairn_git::{Error, HistoryCursor, HistoryRequest, HistorySession, Repository};
use cairn_model::{RefsSnapshot, RowsPage};

use super::epoch::{Epoch, Epochs, Superseded};
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
            Page::Open { rows } => {
                // A different scroll: drop the open walk first.
                self.session = None;
                self.cursor = None;
                self.walked_from = None;
                rows
            }
            Page::More { rows } => rows,
        };
        let cancel = answering.epochs.watch(epoch);
        if self.session.is_none() {
            match self.cursor.clone() {
                // Cold restart from the last good page, from the snapshot it began from.
                Some(at) => match repo.history_session(&HistoryRequest::resume(at, rows)) {
                    Ok(session) => self.session = Some(session),
                    Err(error) => {
                        self.failed(error, epoch, answering);
                        return;
                    }
                },
                None => {
                    self.open(repo, rows, epoch, answering);
                    return;
                }
            }
        }
        match self.next_page(rows, &cancel) {
            Ok(answer) => answering.outbox.send(Some(epoch), answer),
            // Superseded mid-page: the session keeps its rows. Nothing is sent.
            Err(Error::Cancelled { .. }) => {}
            Err(error) => self.failed(error, epoch, answering),
        }
    }

    /// A new walk from every ref, and its first page.
    fn open(
        &mut self,
        repo: &'repo Repository,
        rows: usize,
        epoch: Epoch,
        answering: &Answering<'_>,
    ) {
        let cancel = answering.epochs.watch(epoch);
        let (snapshot, read_now) = match self.latest.clone() {
            Some(snapshot) => (snapshot, false),
            None => match repo.refs(&cancel) {
                Ok(read) => (Arc::new(read.snapshot), true),
                Err(Error::RefsCancelled) => return,
                Err(error) => {
                    self.failed(error, epoch, answering);
                    return;
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
            Ok(answer) => {
                answering.outbox.send(Some(epoch), answer);
                return;
            }
            Err(Error::Cancelled { .. }) => return,
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
                            Ok(answer) => {
                                answering.outbox.send(Some(epoch), answer);
                                return;
                            }
                            Err(Error::Cancelled { .. }) => return,
                            Err(again) => again,
                        }
                    }
                    Err(Error::RefsCancelled) => return,
                    // The same refs, or none to read: the open's own failure stands.
                    Ok(_) | Err(_) => error,
                }
            }
        };
        self.failed(error, epoch, answering);
    }

    /// Opens the walk from `snapshot` and reads its first page.
    fn first_page(
        &mut self,
        repo: &'repo Repository,
        snapshot: &Arc<RefsSnapshot>,
        rows: usize,
        cancel: &Superseded,
    ) -> Result<Update, Error> {
        self.session = Some(repo.history_session(&HistoryRequest::from_refs(snapshot, rows))?);
        self.walked_from = Some(Arc::clone(snapshot));
        self.next_page(rows, cancel)
    }

    /// The next page of the walk open. Any failure but a cancel drops the session, so the
    /// next request cold-restarts from the last good cursor.
    fn next_page(&mut self, rows: usize, cancel: &Superseded) -> Result<Update, Error> {
        let Some(session) = self.session.as_mut() else {
            return Ok(Update::Rows {
                rows: RowsPage::new(),
                complete: true,
            });
        };
        match session.next_page(rows, cancel) {
            Ok(page) => {
                let complete = page.cursor.is_none();
                self.cursor = page.cursor;
                Ok(Update::Rows {
                    rows: page.rows,
                    complete,
                })
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

    /// The decision phase 05 handed on: a ref whose commit has gone since the refresh read
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
        lane.page(&repo, Page::Open { rows: 3 }, epoch, &answering);
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
