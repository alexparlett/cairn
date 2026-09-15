//! A walk kept alive for the length of a scroll.
//!
//! [`Repository::history`] resumes by replaying: page *k* walks `k x limit`
//! commits, which is correct by construction and is why two pages of `n`
//! describe the same commits as one page of `2n` — but it does not reach the
//! sizes the packet promises (measured 1.29-2.1 s for one page at depth 500k,
//! and 54-87 minutes of CPU to page there at 100 rows a page). A
//! [`HistorySession`] holds gitoxide's walk open instead, so every page after
//! the first costs O(limit). That is requirement R2.5; the cursor is not
//! deleted, it becomes the cold-restart path for when no session exists.
//!
//! The session cannot be moved between threads and does not try: gitoxide's
//! `Walk` borrows the repository, so the session borrows one too, and the
//! worker that owns a repository handle for its whole life is exactly the place
//! such a borrow can live (design decision D3).

use std::collections::VecDeque;

use cairn_model::{GraphRow, HistoryRow, LaneAssigner, Oid};

use super::{HistoryCursor, HistoryOrder, HistoryPage, HistoryRequest, Resolved, starting_points};
use crate::{Cancel, Error, Repository};

/// A walk held open across pages.
///
/// Rows are handed out only once the lane assigner has made them final — once
/// they have left its window and can no longer be repainted (R1.2). The cost of
/// that is a one-off prime of `window` commits at the start of a scroll; the
/// benefit is that a row never changes after the view has drawn it, so no
/// repaint protocol is needed between here and the view.
pub struct HistorySession<'repo> {
    walk: gix::revision::Walk<'repo>,
    assigner: LaneAssigner,
    /// Rows made final and not yet handed to the caller.
    ready: VecDeque<HistoryRow>,
    /// Ids and parent ids for commits walked but whose row is still inside the
    /// window, oldest first. A row leaving the assigner is always the oldest one
    /// it held, so the front of this queue is always that row's commit.
    ///
    /// Ids, not summaries: the commit object is read when the row is handed
    /// out, not when it is walked. That is the difference between a first page
    /// of 64 rows reading 64 objects and reading 1,088 — one per commit the
    /// window is primed with, most of which a scroll that stops never asks for.
    pending: VecDeque<(Oid, Vec<Oid>)>,
    tips: Vec<Oid>,
    order: HistoryOrder,
    window: usize,
    /// Rows earlier pages already covered, when this session was started from a
    /// cursor. They are walked and laid out — lane numbering depends on them —
    /// but neither decoded nor handed back.
    skip: usize,
    /// Position in the walk of the next row to leave the assigner.
    next_row: usize,
    /// Commits pulled off the walk, including the replayed prefix.
    walked: usize,
    /// Commit objects read, over the session's whole life.
    decoded: usize,
    /// Rows handed to the caller, over the session's whole life.
    delivered: usize,
    exhausted: bool,
}

impl std::fmt::Debug for HistorySession<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistorySession")
            .field("order", &self.order)
            .field("window", &self.window)
            .field("walked", &self.walked)
            .field("delivered", &self.delivered)
            .field("ready", &self.ready.len())
            .field("exhausted", &self.exhausted)
            .finish_non_exhaustive()
    }
}

impl Repository {
    /// Open a walk and keep it open, so paging through it costs O(limit).
    ///
    /// The session borrows this repository, which is what stops it being sent
    /// to another thread. Start one from [`HistoryRequest::resume`] to pick up
    /// where a cold cursor left off: the replayed prefix is walked once, when
    /// the session first needs it, and never again.
    ///
    /// The request's limit is not used — [`HistorySession::next_page`] carries
    /// it, because a scroll changes how much it asks for as the window resizes.
    pub fn history_session(&self, request: &HistoryRequest) -> Result<HistorySession<'_>, Error> {
        let Resolved {
            tips,
            order,
            window,
            skip,
        } = starting_points(self, request)?;

        let mut object_ids = Vec::with_capacity(tips.len());
        for tip in &tips {
            object_ids.push(super::object_id(tip)?);
        }
        let walk = self
            .inner()
            .rev_walk(object_ids)
            .sorting(order.sorting())
            .all()
            .map_err(|source| Error::Walk {
                source: Box::new(source),
            })?;

        Ok(HistorySession {
            walk,
            assigner: LaneAssigner::with_window(window),
            ready: VecDeque::new(),
            pending: VecDeque::new(),
            tips,
            order,
            window,
            skip,
            next_row: 0,
            walked: 0,
            decoded: 0,
            delivered: 0,
            exhausted: false,
        })
    }
}

impl HistorySession<'_> {
    /// The next `limit` rows, or fewer when the history ran out.
    ///
    /// `cancel` is polled once per commit visited. When it fires, **the work
    /// already done stays in the session**: the rows walked so far are held,
    /// not discarded, so a superseded request leaves no half-consumed walk for
    /// the next one to read (R2.5). The error says how many commits *this call*
    /// had walked, and the next call carries on from there.
    ///
    /// The returned [`HistoryPage::walked`] and [`HistoryPage::decoded`] count
    /// this call only. `decoded` equals the rows returned and never exceeds
    /// them, because a commit object is read when its row is handed out rather
    /// than when it is walked: priming the assigner's window costs a walk step
    /// per commit and no object at all, which matters most on the first page of
    /// a scroll and on a scroll that stops after one.
    ///
    /// **Any error other than [`Error::Cancelled`] poisons the session.** A
    /// commit that cannot be read, or a walk that fails, can lose a row the
    /// assigner had already made final, so the counts stop meaning what they
    /// say. Drop the session and start another from the last good
    /// [`Self::cursor`] — which is what `cairn-app`'s worker does, and is why
    /// the cursor is worth keeping past R2.5.
    pub fn next_page(&mut self, limit: usize, cancel: &impl Cancel) -> Result<HistoryPage, Error> {
        let walked_before = self.walked;
        let decoded_before = self.decoded;

        while self.ready.len() < limit && !self.exhausted {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled {
                    walked: self.walked - walked_before,
                });
            }
            self.pull()?;
        }

        let take = limit.min(self.ready.len());
        let rows: Vec<HistoryRow> = self.ready.drain(..take).collect();
        self.delivered += rows.len();
        let more = !self.exhausted || !self.ready.is_empty();

        Ok(HistoryPage {
            rows,
            cursor: more.then(|| self.cursor()),
            walked: self.walked - walked_before,
            decoded: self.decoded - decoded_before,
        })
    }

    /// Where a fresh query would have to start to continue this scroll.
    ///
    /// The cold-restart path: hand this to [`HistoryRequest::resume`] when the
    /// session is gone — the repository was reopened, the worker was replaced —
    /// and paging carries on, at the replay cost this session exists to avoid.
    pub fn cursor(&self) -> HistoryCursor {
        HistoryCursor {
            tips: self.tips.clone(),
            order: self.order,
            window: self.window,
            walked: self.skip + self.delivered,
        }
    }

    /// Rows this session has handed out.
    pub fn delivered(&self) -> usize {
        self.delivered
    }

    /// Whether the walk has reached the end of the history and every row it
    /// produced has been handed out.
    pub fn is_exhausted(&self) -> bool {
        self.exhausted && self.ready.is_empty()
    }

    /// Pull one commit off the walk and lay it out, or notice the end.
    fn pull(&mut self) -> Result<(), Error> {
        let Some(next) = self.walk.next() else {
            self.exhausted = true;
            // The window is holding rows that no later commit can now repaint,
            // because no later commit is coming. They are final; hand them on.
            let rest = std::mem::take(&mut self.assigner).into_rows();
            for graph in rest {
                self.place(graph)?;
            }
            return Ok(());
        };
        let info = next.map_err(|source| Error::Walk {
            source: Box::new(source),
        })?;

        let id = super::model_id(&info.id)?;
        let mut parents = Vec::with_capacity(info.parent_ids.len());
        for parent in info.parent_ids.iter() {
            parents.push(super::model_id(parent)?);
        }
        if self.walked >= self.skip {
            self.pending.push_back((id, parents.clone()));
        }
        self.walked += 1;

        if let Some(graph) = self.assigner.push(id, parents) {
            self.place(graph)?;
        }
        Ok(())
    }

    /// Read the commit for a row the assigner has made final, and pair the two
    /// — unless the row belongs to the prefix an earlier page already covered.
    ///
    /// This is the one place a commit object is read, and it happens per row
    /// HANDED OUT rather than per commit walked.
    fn place(&mut self, graph: GraphRow) -> Result<(), Error> {
        let position = self.next_row;
        self.next_row += 1;
        if position < self.skip {
            // Replayed to get the lanes right; not this session's to hand out,
            // and deliberately never read from the object database.
            return Ok(());
        }
        let Some((id, parents)) = self.pending.pop_front() else {
            // Unreachable: an id is pushed for every commit walked past the
            // prefix, and rows leave the assigner in walk order, so the front of
            // the queue is this row's commit. Dropping the row is the safe
            // reading of an impossible state — a panic here would cost a user
            // their window.
            return Ok(());
        };
        let commit = super::summary_of_commit(self.walk.repo, &id, &parents)?;
        self.decoded += 1;
        self.ready.push_back(HistoryRow { commit, graph });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CancelSignal, SharedRepository};
    use std::cell::Cell;

    /// A cancel signal that fires after a chosen number of polls, so a test can
    /// stop a walk at a known commit rather than by racing it.
    struct StopsAfter {
        polls: Cell<usize>,
        limit: usize,
    }

    impl StopsAfter {
        fn new(limit: usize) -> Self {
            Self {
                polls: Cell::new(0),
                limit,
            }
        }
    }

    impl Cancel for StopsAfter {
        fn is_cancelled(&self) -> bool {
            let seen = self.polls.get();
            self.polls.set(seen + 1);
            seen >= self.limit
        }
    }

    fn cairn() -> Repository {
        Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap()
    }

    #[test]
    fn a_session_pages_the_same_rows_the_cursor_path_does() {
        let repo = cairn();
        // Small window, so rows leave it on a repository of this size.
        let request = HistoryRequest::from_head(6).with_window(2);

        let one_page = repo.history(&request, &CancelSignal::new()).unwrap();

        let mut session = repo.history_session(&request).unwrap();
        let first = session.next_page(3, &CancelSignal::new()).unwrap();
        let second = session.next_page(3, &CancelSignal::new()).unwrap();

        let paged: Vec<_> = first.rows.iter().chain(second.rows.iter()).collect();
        assert_eq!(paged.len(), one_page.rows.len(), "row counts differ");
        for (from_session, from_cursor) in paged.iter().zip(one_page.rows.iter()) {
            assert_eq!(from_session.id(), from_cursor.id(), "different commits");
            assert_eq!(
                from_session.graph.lane, from_cursor.graph.lane,
                "the same commit landed in different lanes"
            );
        }
    }

    #[test]
    fn paging_a_session_walks_only_what_the_page_needs() {
        let repo = cairn();
        let request = HistoryRequest::from_head(4).with_window(2);
        let mut session = repo.history_session(&request).unwrap();

        let first = session.next_page(2, &CancelSignal::new()).unwrap();
        let second = session.next_page(2, &CancelSignal::new()).unwrap();

        assert_eq!(first.rows.len(), 2);
        assert_eq!(second.rows.len(), 2);
        // The point of the whole session: the second page does not replay the
        // first. With a window of 2, priming costs 2 extra commits on page one
        // and nothing afterwards.
        assert_eq!(
            second.walked, 2,
            "page two walked {} commits; it should walk only what it returns",
            second.walked
        );
        assert!(
            first.walked >= 4,
            "page one should have primed the window as well as filled itself"
        );
    }

    #[test]
    fn cancelling_stops_the_walk_and_keeps_what_it_had() {
        let repo = cairn();
        let request = HistoryRequest::from_head(1).with_window(1);
        let mut session = repo.history_session(&request).unwrap();

        // Fires on the fourth poll, so at most four commits are walked.
        let err = session.next_page(50, &StopsAfter::new(3)).unwrap_err();
        let Error::Cancelled { walked } = err else {
            panic!("expected Cancelled, got {err:?}");
        };
        assert!(walked <= 4, "the walk ran on to {walked} commits");
        assert!(walked > 0, "the walk stopped before it started");

        // The work is not lost: the next call continues rather than restarting.
        let resumed = session.next_page(2, &CancelSignal::new()).unwrap();
        assert_eq!(resumed.rows.len(), 2);
        assert!(
            resumed.walked < 2 + walked,
            "resuming re-walked what the cancelled call had already done"
        );
    }

    #[test]
    fn a_session_resumed_from_a_cursor_continues_where_it_stopped() {
        let repo = cairn();
        let request = HistoryRequest::from_head(4).with_window(2);

        let mut first = repo.history_session(&request).unwrap();
        let head = first.next_page(2, &CancelSignal::new()).unwrap();
        let cursor = first.cursor();
        drop(first);

        let behind = cursor.rows_behind();
        assert_eq!(behind, 2, "the cursor should stand two rows into the walk");

        let mut cold = repo
            .history_session(&HistoryRequest::resume(cursor, 2))
            .unwrap();
        let next = cold.next_page(2, &CancelSignal::new()).unwrap();

        assert_eq!(next.rows.len(), 2);
        assert!(
            !head.rows.iter().any(|r| r.id() == next.rows[0].id()),
            "the cold restart repeated a row the first session had handed out"
        );
        // Nothing is read from the object database except the rows handed out:
        // not the replayed prefix, and not the commits the window is primed
        // with. The walk pays a step for each of those and no object read.
        assert_eq!(
            next.decoded,
            next.rows.len(),
            "walked {} and decoded {} to return {} rows",
            next.walked,
            next.decoded,
            next.rows.len()
        );
        assert!(
            next.walked > next.decoded + behind,
            "the window was primed without walking for it"
        );
    }

    #[test]
    fn a_session_ends_and_says_so() {
        let repo = cairn();
        let request = HistoryRequest::from_head(1).with_window(4);
        let mut session = repo.history_session(&request).unwrap();

        let mut rows = 0usize;
        for _ in 0..10_000 {
            let page = session.next_page(64, &CancelSignal::new()).unwrap();
            rows += page.rows.len();
            if page.cursor.is_none() {
                break;
            }
        }
        assert!(session.is_exhausted(), "the session never reached the root");
        assert!(rows > 0, "an exhausted session returned no rows at all");
        assert!(
            session.pending.is_empty(),
            "{} summaries were decoded for rows that never came out",
            session.pending.len()
        );
        assert_eq!(rows, session.delivered());
    }

    #[test]
    fn a_session_runs_on_a_worker_handle_from_a_shared_repository() {
        let shared = SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let rows = std::thread::spawn(move || {
            let repo = shared.to_worker();
            let mut session = repo
                .history_session(&HistoryRequest::from_head(2).with_window(1))
                .unwrap();
            session
                .next_page(2, &CancelSignal::new())
                .unwrap()
                .rows
                .len()
        })
        .join()
        .unwrap();
        assert_eq!(rows, 2);
    }
}
