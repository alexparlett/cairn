//! View state for the history list.

use cairn_model::HistoryRow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Being opened, or the first rows are on their way.
    Loading,
    /// Opened, with no commits to show.
    Empty,
    Ready,
    /// Display text. Rows already on screen stay on screen.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    status: Status,
    lanes: usize,
    complete: bool,
    in_flight: bool,
    /// No worker is left to answer.
    ended: bool,
    loaded: usize,
}

impl Progress {
    /// Starts with a request already in flight.
    pub fn opening() -> Self {
        Self {
            status: Status::Loading,
            lanes: 1,
            complete: false,
            in_flight: true,
            ended: false,
            loaded: 0,
        }
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// The widest lane seen in any page so far; only grows.
    pub fn lanes(&self) -> usize {
        self.lanes
    }

    /// How many rows have arrived, without reading (and subscribing to) the row vector.
    pub fn loaded(&self) -> usize {
        self.loaded
    }

    pub fn has_rows(&self) -> bool {
        self.loaded > 0
    }

    pub fn complete(&self) -> bool {
        self.complete
    }

    /// False while a page is in flight, while the history is complete, and once the stream has
    /// ended. A failed page is asked for again: only the list's next visibility change calls
    /// this, so a failure cannot loop.
    pub fn wants_more(&self) -> bool {
        !self.complete && !self.in_flight && !self.ended
    }

    pub fn asked(&mut self) {
        self.in_flight = true;
    }

    /// `loaded` is the total rows held once the page has been added.
    pub fn received(&mut self, widest_lane: usize, complete: bool, loaded: usize) {
        self.in_flight = false;
        self.lanes = self.lanes.max(widest_lane);
        self.complete = complete;
        self.loaded = loaded;
        self.status = match (loaded, complete) {
            // Ran out with nothing in it: empty. Nothing handed over yet: still loading.
            (0, true) => Status::Empty,
            (0, false) => Status::Loading,
            _ => Status::Ready,
        };
    }

    pub fn failed(&mut self, message: String) {
        self.in_flight = false;
        self.status = Status::Failed(message);
    }

    /// Does not overwrite an existing failure's message.
    pub fn stream_ended(&mut self, message: &str) {
        self.in_flight = false;
        self.ended = true;
        if !matches!(self.status, Status::Failed(_)) {
            self.status = Status::Failed(message.to_owned());
        }
    }
}

/// Counts every lane a segment names, not just node lanes: a passing line's commit
/// may be on another page.
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

    /// Caught by: a second request going out over the first.
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

    /// Caught by: a transient failure ending the scroll for the session.
    #[test]
    fn a_failed_page_is_asked_for_again_and_keeps_its_sentence_until_rows_arrive() {
        let mut progress = Progress::opening();
        progress.received(1, false, 40);
        progress.failed("failed to read commit abc".to_owned());
        assert_eq!(
            progress.status(),
            &Status::Failed("failed to read commit abc".to_owned())
        );
        assert!(progress.wants_more(), "a failed page can never be retried");

        progress.asked();
        assert!(!progress.wants_more(), "a retry went out over a retry");
        assert_eq!(
            progress.status(),
            &Status::Failed("failed to read commit abc".to_owned()),
            "asking again unsaid the failure before anything worked"
        );

        progress.received(1, false, 80);
        assert_eq!(progress.status(), &Status::Ready);
        assert!(progress.wants_more());
    }

    /// Caught by: asking a worker that has gone, which can never answer.
    #[test]
    fn a_stream_that_has_ended_stops_asking_for_good() {
        let mut named = Progress::opening();
        named.received(1, false, 40);
        named.failed("the repository worker stopped unexpectedly".to_owned());
        named.stream_ended("the repository worker has stopped");
        assert!(!named.wants_more());

        let mut silent = Progress::opening();
        silent.received(1, false, 40);
        silent.stream_ended("the repository worker has stopped");
        assert!(!silent.wants_more());
    }

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

    /// Caught by: measuring nodes only.
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
