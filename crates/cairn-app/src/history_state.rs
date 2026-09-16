//! View state, graph width, and whether another page is worth asking for.
//!
//! A plain value, so R4.3 — a still-loading list and an empty repository looking
//! identical — is decided by a test rather than a screenshot; which sentence
//! each state shows is [`crate::status_text`]'s. It also keeps the window from
//! reading the row vector to decide what to draw.

use cairn_model::HistoryRow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Being opened, or the first rows are on their way. Never rendered the same
    /// as [`Status::Empty`] (R4.3), which is why this is an enum and not an
    /// empty vector.
    Loading,
    /// Opened, with no commits to show.
    Empty,
    Ready,
    /// Display text. Rows already on screen stay on screen.
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
    /// A request is already on its way, so it is loading and another would only
    /// supersede this one.
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

    /// The widest lane seen in any page so far. Only grows, so a row already
    /// drawn never has to move.
    pub fn lanes(&self) -> usize {
        self.lanes
    }

    /// How many rows have arrived, without reading the row vector — which would
    /// subscribe the window to every page.
    pub fn loaded(&self) -> usize {
        self.loaded
    }

    pub fn has_rows(&self) -> bool {
        self.loaded > 0
    }

    pub fn complete(&self) -> bool {
        self.complete
    }

    /// False while a page is in flight, while the history is complete, and after
    /// a failure. The debounce the worker boundary needs: every `submit`
    /// supersedes, so asking again throws away the page being built.
    pub fn wants_more(&self) -> bool {
        !self.complete && !self.in_flight && !matches!(self.status, Status::Failed(_))
    }

    pub fn asked(&mut self) {
        self.in_flight = true;
    }

    /// Folds in a page. `loaded` is the total rows held once the page has been
    /// added, counted by the caller, which owns them.
    pub fn received(&mut self, widest_lane: usize, complete: bool, loaded: usize) {
        self.in_flight = false;
        self.lanes = self.lanes.max(widest_lane);
        self.complete = complete;
        self.loaded = loaded;
        self.status = match (loaded, complete) {
            // R4.3: a history that ran out with nothing in it is empty; one
            // that has handed over nothing yet is still loading.
            (0, true) => Status::Empty,
            (0, false) => Status::Loading,
            _ => Status::Ready,
        };
    }

    pub fn failed(&mut self, message: String) {
        self.in_flight = false;
        self.status = Status::Failed(message);
    }

    /// The stream ended with nobody saying why. Only overwrites a status that is
    /// not already a failure: replacing a named cause with a generic line loses
    /// the only sentence that had one.
    pub fn stream_ended(&mut self, message: &str) {
        self.in_flight = false;
        if !matches!(self.status, Status::Failed(_)) {
            self.status = Status::Failed(message.to_owned());
        }
    }
}

/// How many lane columns `rows` needs. Every lane a row names counts, not just
/// its own commit's: a passing line runs in a lane whose commit may be nowhere
/// in this page, and a narrower column clips it.
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

    /// R4.3: different values, so a view rendering them the same ignored a
    /// distinction it was handed.
    #[test]
    fn loading_and_an_empty_repository_are_different_states() {
        let mut empty = Progress::opening();
        assert_eq!(empty.status(), &Status::Loading);
        empty.received(widest_lane(&[]), true, 0);
        assert_eq!(empty.status(), &Status::Empty);
        assert_ne!(Status::Empty, Status::Loading);
    }

    /// Caught by: calling it empty, which flashes "no commits" mid-scroll.
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

    /// Caught by: letting a second request go out over the first, which throws
    /// away the page being built.
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

    /// Caught by: asking on, which loops a broken repository forever.
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

    /// A failure after rows arrived is a banner, not a blank window, decided
    /// without reading the row vector.
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

    /// Caught by: overwriting a named cause with the generic line.
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

    /// Caught by: narrowing, which moves every subject sideways mid-scroll.
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

    /// Caught by: measuring nodes only. An out-of-order line runs down a lane
    /// chosen for being free, which no commit in the page need sit in.
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

    /// Caught by: a zero-wide graph on a repository's first page.
    #[test]
    fn an_empty_page_still_reserves_a_column() {
        assert_eq!(widest_lane(&[]), 1);
    }
}
