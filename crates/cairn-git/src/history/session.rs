//! A history walk held open across pages.

use std::collections::VecDeque;
use std::sync::Arc;

use cairn_model::{GraphRow, LaneAssigner, RowsPage};

use super::seeds::Decoration;
use super::stream::{Next, Stream};
use super::{
    HistoryCursor, HistoryOrder, HistoryPage, HistoryRequest, Laid, Pending, Resolved, lay_out,
    starting_points,
};
use crate::{Cancel, Error, Repository};

/// Rows are handed out only once the assigner has made them final.
pub struct HistorySession<'repo> {
    repo: &'repo gix::Repository,
    /// `None` until the first page opens the walk, under that page's cancel.
    stream: Option<Stream<'repo>>,
    assigner: LaneAssigner,
    /// Rows made final, each with what it draws read, and not yet handed to the caller.
    ready: VecDeque<(GraphRow, Laid)>,
    /// Entries laid out and still inside the window, oldest first; the front is the next row
    /// to leave.
    pending: VecDeque<Pending>,
    tips: super::Tips,
    decoration: Arc<Decoration>,
    order: HistoryOrder,
    window: usize,
    lookahead: usize,
    /// Rows earlier pages covered: laid out for lane numbering, never decoded or handed back.
    skip: usize,
    /// Position in the walk of the next row to leave the assigner.
    next_row: usize,
    /// Rows laid out, including the replayed prefix.
    walked: usize,
    /// Commit objects read over the session's life.
    decoded: usize,
    /// Rows handed to the caller over the session's life.
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
    /// The request's limit is ignored; [`HistorySession::next_page`] takes one. The walk is
    /// opened by the first page, under its cancel: opening reads every tip, and a walk from
    /// every ref has one per ref (`walk::open`).
    pub fn history_session(&self, request: &HistoryRequest) -> Result<HistorySession<'_>, Error> {
        let Resolved {
            tips,
            decoration,
            order,
            window,
            lookahead,
            skip,
        } = starting_points(self, request)?;

        Ok(HistorySession {
            repo: self.inner(),
            stream: None,
            // The first row handed out carries a snapshot, so it draws without the prefix.
            assigner: LaneAssigner::with_window(window).drawn_from(skip),
            ready: VecDeque::new(),
            pending: VecDeque::new(),
            tips,
            decoration,
            order,
            window,
            lookahead,
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
    /// `cancel` is polled once per row laid out, per commit looked ahead at, and — on the
    /// page that opens the walk — per tip read but the first; on cancellation the work done
    /// stays in the session, except a cancelled open, which the next page begins again. `walked` and `decoded` count this
    /// call only. Any error other than [`Error::Cancelled`] poisons the session.
    pub fn next_page(&mut self, limit: usize, cancel: &impl Cancel) -> Result<HistoryPage, Error> {
        let walked_before = self.walked;
        let decoded_before = self.decoded;

        if self.stream.is_none() {
            let Some(walk) = super::walk::open(self.repo, &self.tips, self.order, cancel)? else {
                // Nothing kept: the next page opens it again.
                return Err(Error::Cancelled { walked: 0 });
            };
            self.stream = Some(Stream::new(
                walk,
                Arc::clone(&self.decoration),
                self.lookahead,
            ));
        }

        while self.ready.len() < limit && !self.exhausted {
            if cancel.is_cancelled() || !self.pull(cancel)? {
                return Err(Error::Cancelled {
                    walked: self.walked - walked_before,
                });
            }
        }

        let take = limit.min(self.ready.len());
        let mut rows = RowsPage::new();
        for (graph, laid) in self.ready.drain(..take) {
            laid.push_to(&mut rows, graph, &self.decoration);
        }
        self.delivered += rows.len();
        let more = !self.exhausted || !self.ready.is_empty();

        Ok(HistoryPage {
            rows,
            cursor: more.then(|| self.cursor()),
            walked: self.walked - walked_before,
            decoded: self.decoded - decoded_before,
        })
    }

    /// Where a fresh query would start to continue this scroll.
    pub fn cursor(&self) -> HistoryCursor {
        HistoryCursor {
            tips: Arc::clone(&self.tips),
            decoration: Arc::clone(&self.decoration),
            order: self.order,
            window: self.window,
            lookahead: self.lookahead,
            walked: self.skip + self.delivered,
        }
    }

    pub fn delivered(&self) -> usize {
        self.delivered
    }

    /// Commits pulled off the walk over the session's life, those looked ahead at for a
    /// stash's base included.
    pub fn commits_walked(&self) -> usize {
        self.stream.as_ref().map_or(0, Stream::pulled)
    }

    /// The walk reached the end and every row it produced has been handed out.
    pub fn is_exhausted(&self) -> bool {
        self.exhausted && self.ready.is_empty()
    }

    /// Lays out the stream's next entry, or notices the end. `false` when `cancel` fired
    /// while the stream looked ahead.
    fn pull(&mut self, cancel: &impl Cancel) -> Result<bool, Error> {
        let Some(stream) = self.stream.as_mut() else {
            // Unreachable: `next_page` opens the walk before it pulls.
            return Ok(false);
        };
        let entry = match stream.next(cancel)? {
            Next::Entry(entry) => entry,
            Next::Cancelled => return Ok(false),
            Next::End => {
                self.exhausted = true;
                // No later row is coming: the window's rows are final.
                let rest = std::mem::take(&mut self.assigner).into_rows();
                for graph in rest {
                    self.place(graph)?;
                }
                return Ok(true);
            }
        };
        if self.walked >= self.skip {
            self.pending.push_back(Pending::of(&entry));
        }
        self.walked += 1;

        if let Some(graph) = lay_out(&mut self.assigner, &self.decoration, entry) {
            self.place(graph)?;
        }
        Ok(true)
    }

    /// Reads what a final row draws and pairs the two, skipping the replayed prefix.
    fn place(&mut self, graph: GraphRow) -> Result<(), Error> {
        let position = self.next_row;
        self.next_row += 1;
        if position < self.skip {
            // Replayed for the lanes; never read from the object database.
            return Ok(());
        }
        let Some(entry) = self.pending.pop_front() else {
            // Unreachable: one entry is pushed per row past the prefix.
            return Ok(());
        };
        let laid = Laid::read(self.repo, entry)?;
        if matches!(laid, Laid::Commit { .. }) {
            self.decoded += 1;
        }
        self.ready.push_back((graph, laid));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CancelSignal, SharedRepository};
    use std::cell::Cell;

    /// Fires after a chosen number of polls: a known commit, not a race.
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

        let mut paged = cairn_model::History::new();
        paged.append(first.rows).unwrap();
        paged.append(second.rows).unwrap();
        let mut whole = cairn_model::History::new();
        whole.append(one_page.rows).unwrap();
        assert_eq!(paged.len(), whole.len(), "row counts differ");
        for (from_session, from_cursor) in paged.rows().zip(whole.rows()) {
            assert_eq!(from_session.id(), from_cursor.id(), "different commits");
            assert_eq!(
                from_session.lane(),
                from_cursor.lane(),
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
        // Priming costs 2 extra commits on page one only.
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
            !head.rows.ids().any(|id| next.rows.ids().next() == Some(id)),
            "the cold restart repeated a row the first session had handed out"
        );
        // Only rows handed out are decoded.
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
