//! What the window knows about the history besides the rows themselves.
//!
//! The rows live in one place — the reactive vector the list reads — so this
//! holds everything derived from them that the view needs and the list does
//! not: which of the four states the view is in, how wide the graph column has
//! to be, and whether asking for another page would achieve anything.
//!
//! It is a plain value, deliberately: the bug R4.3 exists to prevent — a
//! still-loading list and an empty repository looking identical — is a bug in a
//! state machine, and a state machine that is a value can be decided by a test
//! instead of by a screenshot. Which SENTENCE each state is shown as is decided
//! in [`crate::status_text`], for the same reason.
//!
//! It also means the window never reads the row vector to decide what to draw.
//! The window still re-renders when a page arrives — it reads this value, and
//! this value changes — but what that costs is a title bar and a header, not a
//! pass over the history.

use cairn_model::HistoryRow;

/// Which of the four things the view is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The repository is being opened, or its first rows are on their way.
    /// **Never the same rendering as [`Status::Empty`]** — that is R4.3, and it
    /// is the whole reason this is an enum rather than an empty vector.
    Loading,
    /// The repository opened and has no commits to show.
    Empty,
    /// There are rows.
    Ready,
    /// Something went wrong, and this is the sentence to show. Rows already on
    /// screen stay on screen: a page that failed does not unsay the pages that
    /// worked.
    Failed(String),
}

/// Everything about the history except its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    status: Status,
    lanes: usize,
    complete: bool,
    in_flight: bool,
    loaded: usize,
}

impl Progress {
    /// The state a window starts in: a request is already on its way, so it is
    /// loading and another request would only supersede this one.
    pub fn opening() -> Self {
        Self {
            status: Status::Loading,
            lanes: 1,
            complete: false,
            in_flight: true,
            loaded: 0,
        }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// How many lanes the graph column reserves: the widest lane seen in any
    /// page so far. It only grows, so a row already drawn never has to move.
    pub fn lanes(&self) -> usize {
        self.lanes
    }

    /// How many rows have arrived, and whether the history has run out.
    ///
    /// The window shows this and uses it to tell a failure that left rows on
    /// screen from one that left nothing — both without reading the row vector,
    /// which would subscribe the whole window to every page.
    pub fn loaded(&self) -> usize {
        self.loaded
    }

    /// Whether any row has ever arrived.
    pub fn has_rows(&self) -> bool {
        self.loaded > 0
    }

    /// Whether the history ran out.
    pub fn complete(&self) -> bool {
        self.complete
    }

    /// Whether asking for another page now would achieve anything.
    ///
    /// The list asks every time a row near the end is visible, which is many
    /// times per scroll; this is what makes that cheap. It is also the debounce
    /// the worker boundary needs — every `submit` supersedes the request in
    /// flight, so asking again before an answer arrives throws away the page
    /// that was being built.
    pub fn wants_more(&self) -> bool {
        !self.complete && !self.in_flight && !matches!(self.status, Status::Failed(_))
    }

    /// Record that a page has been asked for.
    pub fn asked(&mut self) {
        self.in_flight = true;
    }

    /// Fold in a page.
    ///
    /// `widest_lane` is how many lane columns the page needs (see
    /// [`widest_lane`]), and `loaded` is how many rows are held in total once
    /// the page has been added — the caller owns the rows, so it is the caller
    /// that counts them.
    pub fn received(&mut self, widest_lane: usize, complete: bool, loaded: usize) {
        self.in_flight = false;
        self.lanes = self.lanes.max(widest_lane);
        self.complete = complete;
        self.loaded = loaded;
        self.status = match (loaded, complete) {
            // The distinction R4.3 is about. A repository whose history ran out
            // with nothing in it is empty; one that has handed over nothing YET
            // is still loading.
            (0, true) => Status::Empty,
            (0, false) => Status::Loading,
            _ => Status::Ready,
        };
    }

    /// Record a failure, from a worker or from failing to start one.
    pub fn failed(&mut self, message: String) {
        self.in_flight = false;
        self.status = Status::Failed(message);
    }

    /// The stream of updates ended without anyone saying why.
    ///
    /// Only overwrites a status that is not already a failure: a worker
    /// announces its own death before its channel closes, and replacing that
    /// with a generic line loses the only sentence that named a cause.
    pub fn stream_ended(&mut self, message: &str) {
        self.in_flight = false;
        if !matches!(self.status, Status::Failed(_)) {
            self.status = Status::Failed(message.to_owned());
        }
    }
}

/// How many lane columns `rows` needs.
///
/// Every lane a row NAMES counts, not just the lane its own commit sits in: a
/// line passing a row runs in a lane whose commit may be nowhere in this page,
/// and a column too narrow to hold it would clip the line rather than draw it.
pub fn widest_lane(rows: &[HistoryRow]) -> usize {
    rows.iter()
        .map(|row| {
            let node = row.graph.lane.index();
            row.graph
                .edges
                .iter()
                .map(|edge| edge.from.index().max(edge.to.index()))
                .fold(node, usize::max)
                + 1
        })
        .max()
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{CommitSummary, EdgeSegment, GraphRow, Lane, Oid, RowContent};

    fn oid(n: u8) -> Oid {
        let mut bytes = [0u8; 20];
        bytes[19] = n;
        Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
    }

    fn row(n: u8, lane: usize, edges: Vec<EdgeSegment>) -> HistoryRow {
        HistoryRow {
            content: RowContent::Commit(CommitSummary {
                id: oid(n),
                parents: Vec::new(),
                summary: "s".to_owned(),
                author_name: "a".to_owned(),
                author_email: "a@example.com".to_owned(),
                author_time: 0,
            }),
            graph: GraphRow {
                id: oid(n),
                lane: Lane::new(lane),
                edges,
            },
        }
    }

    /// R4.3, decided rather than asserted by comment: the two states a reader
    /// must be able to tell apart are different values, so a view that renders
    /// them the same is a view that ignored a distinction it was handed.
    #[test]
    fn loading_and_an_empty_repository_are_different_states() {
        let mut empty = Progress::opening();
        assert_eq!(empty.status(), &Status::Loading);
        empty.received(widest_lane(&[]), true, 0);
        assert_eq!(empty.status(), &Status::Empty);
        assert_ne!(Status::Empty, Status::Loading);
    }

    /// A page that arrived with nothing in it but more to come is still
    /// loading, not empty — the state that would otherwise flash "no commits"
    /// at a reader in the middle of a scroll.
    #[test]
    fn an_empty_page_with_more_to_come_is_still_loading() {
        let mut progress = Progress::opening();
        progress.received(widest_lane(&[]), false, 0);
        assert_eq!(progress.status(), &Status::Loading);
        assert!(progress.wants_more(), "a loading view stopped asking");
    }

    #[test]
    fn rows_arriving_make_the_view_ready() {
        let mut progress = Progress::opening();
        let page = vec![row(1, 0, Vec::new())];
        progress.received(widest_lane(&page), false, page.len());
        assert_eq!(progress.status(), &Status::Ready);
        assert!(progress.has_rows());
        assert!(
            progress.wants_more(),
            "a page that said more was coming stopped asking"
        );
    }

    /// The debounce the worker boundary needs: every `submit` supersedes, so a
    /// second request before the first is answered throws away the page being
    /// built. Asking is only worth it when nothing is in flight.
    #[test]
    fn only_one_page_is_asked_for_at_a_time() {
        let mut progress = Progress::opening();
        assert!(!progress.wants_more(), "the opening request is already out");

        progress.received(1, false, 1);
        assert!(progress.wants_more());

        progress.asked();
        assert!(
            !progress.wants_more(),
            "a second request went out over the first"
        );
    }

    #[test]
    fn a_complete_history_stops_asking() {
        let mut progress = Progress::opening();
        progress.received(1, true, 1);
        assert_eq!(progress.status(), &Status::Ready);
        assert!(progress.complete());
        assert!(
            !progress.wants_more(),
            "a history that said it had run out was asked for more"
        );
    }

    /// A failure stops the asking too, or a broken repository turns into a loop
    /// that asks a worker the same failing question forever.
    #[test]
    fn a_failure_stops_asking_and_keeps_its_sentence() {
        let mut progress = Progress::opening();
        progress.failed("no git repository at /tmp/nowhere".to_owned());
        assert_eq!(
            progress.status(),
            &Status::Failed("no git repository at /tmp/nowhere".to_owned())
        );
        assert!(!progress.wants_more());
    }

    /// A failure after rows arrived is a banner, not a blank window — and the
    /// window can tell the two apart without reading the row vector.
    #[test]
    fn a_failure_does_not_unsay_the_pages_that_worked() {
        let mut progress = Progress::opening();
        progress.received(1, false, 40);
        progress.failed("failed to read commit abc".to_owned());
        assert!(progress.has_rows(), "the rows already drawn were disowned");
        assert_eq!(progress.loaded(), 40);

        let mut never_started = Progress::opening();
        never_started.failed("no git repository at /tmp/nowhere".to_owned());
        assert!(!never_started.has_rows());
    }

    /// The stream ending is only worth reporting when nobody said anything
    /// better. A worker announces its own death first, and overwriting that
    /// loses the only sentence that named a cause.
    #[test]
    fn the_stream_ending_does_not_overwrite_a_named_cause() {
        let mut named = Progress::opening();
        named.failed("the repository worker stopped unexpectedly".to_owned());
        named.stream_ended("the repository worker has stopped");
        assert_eq!(
            named.status(),
            &Status::Failed("the repository worker stopped unexpectedly".to_owned())
        );

        let mut silent = Progress::opening();
        silent.stream_ended("the repository worker has stopped");
        assert_eq!(
            silent.status(),
            &Status::Failed("the repository worker has stopped".to_owned())
        );
    }

    /// The graph column only ever widens. A column that narrowed would move
    /// every subject on screen sideways as the reader scrolled.
    #[test]
    fn the_graph_column_only_grows() {
        let mut progress = Progress::opening();
        let wide = vec![row(1, 4, Vec::new())];
        progress.received(widest_lane(&wide), false, 1);
        assert_eq!(progress.lanes(), 5);

        let narrow = vec![row(2, 0, Vec::new())];
        progress.received(widest_lane(&narrow), false, 2);
        assert_eq!(progress.lanes(), 5, "the column narrowed under the reader");
    }

    /// A lane a line merely PASSES through counts. It is the case a node-only
    /// measurement gets wrong, and it is not hypothetical: an out-of-order line
    /// is run down a lane chosen for being free, which can be a lane no commit
    /// in the page sits in.
    #[test]
    fn a_lane_only_a_passing_line_uses_still_gets_a_column() {
        let rows = vec![row(
            1,
            0,
            vec![
                EdgeSegment::passing(Lane::new(0)),
                EdgeSegment::passing(Lane::new(6)),
            ],
        )];
        assert_eq!(widest_lane(&rows), 7);
    }

    /// A page with no rows still needs a column, or the first page of a
    /// repository would draw a zero-wide graph.
    #[test]
    fn an_empty_page_still_reserves_a_column() {
        assert_eq!(widest_lane(&[]), 1);
    }
}
