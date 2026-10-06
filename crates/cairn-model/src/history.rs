//! Row content and row identity.

use crate::{CommitSummary, GraphRow, Oid};

/// Not `#[non_exhaustive]`: consumers match every variant, with no wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowContent {
    Commit(CommitSummary),
    /// Test-only second row kind; the shape tests below need one.
    #[cfg(test)]
    NotACommit,
}

/// A row's stable identity: not an index, and not an [`Oid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowId {
    Commit(Oid),
    /// Test-only, and carries no `Oid`.
    #[cfg(test)]
    NotACommit,
}

/// Nothing enforces that `content` and `graph` describe the same entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub content: RowContent,
    pub graph: GraphRow,
}

impl HistoryRow {
    pub fn id(&self) -> RowId {
        match &self.content {
            RowContent::Commit(commit) => RowId::Commit(commit.id),
            #[cfg(test)]
            RowContent::NotACommit => RowId::NotACommit,
        }
    }
}

/// A history's rows are drawn by deriving each one's edges from the graph rows around it
/// ([`crate::row_edges`]).
impl AsRef<GraphRow> for HistoryRow {
    fn as_ref(&self) -> &GraphRow {
        &self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lane, LaneChange};

    fn commit(id: Oid) -> CommitSummary {
        CommitSummary {
            id,
            parents: Vec::new(),
            summary: "first".to_owned(),
            author_name: "A".to_owned(),
            author_email: "a@example.com".to_owned(),
            author_time: 0,
        }
    }

    fn graph(id: Oid) -> GraphRow {
        GraphRow::new(id, Lane::new(0), vec![LaneChange::Starts(Lane::new(0))])
    }

    #[test]
    fn a_row_takes_its_identity_from_its_content() {
        let id = Oid::parse("0123456789abcdef0123456789abcdef01234567").unwrap();
        let other = Oid::parse("fedcba9876543210fedcba9876543210fedcba98").unwrap();
        let row = HistoryRow {
            content: RowContent::Commit(commit(id)),
            graph: graph(other),
        };
        assert_eq!(
            row.id(),
            RowId::Commit(id),
            "the row took its identity from the graph half"
        );
        assert_ne!(row.id(), RowId::Commit(other));
    }

    /// Caught by: collapsing the row shape to a bare commit field.
    #[test]
    fn a_row_can_be_about_something_that_is_not_a_commit() {
        let row = HistoryRow {
            content: RowContent::NotACommit,
            graph: graph(Oid::parse("0123456789abcdef0123456789abcdef01234567").unwrap()),
        };

        let id = row.id();
        assert_eq!(id, RowId::NotACommit);
        assert!(
            !matches!(id, RowId::Commit(_)),
            "identity assumed the row was a commit"
        );
        // Identity is per row kind, not per `Oid`.
        assert_eq!(id, row.id());
        assert_ne!(
            id,
            HistoryRow {
                content: RowContent::Commit(commit(
                    Oid::parse("0123456789abcdef0123456789abcdef01234567").unwrap()
                )),
                graph: graph(Oid::parse("0123456789abcdef0123456789abcdef01234567").unwrap()),
            }
            .id(),
        );
    }

    /// Caught by: a history row handing the derivation some graph row other than its own.
    #[test]
    fn a_history_row_draws_from_its_own_graph_row() {
        let walk = (0..5u8).map(|n| {
            let id = Oid::from_bytes(&[n + 1; 20]).unwrap();
            let parent = Oid::from_bytes(&[n + 2; 20]).unwrap();
            (id, if n < 4 { vec![parent] } else { Vec::new() })
        });
        let graphs = crate::LaneAssigner::with_window(2)
            .with_snapshot_every(2)
            .assign_each(walk);
        let rows: Vec<HistoryRow> = graphs
            .iter()
            .map(|graph| HistoryRow {
                content: RowContent::Commit(commit(graph.id)),
                graph: graph.clone(),
            })
            .collect();
        for index in 0..rows.len() {
            let drawn = crate::row_edges(&rows, index);
            assert!(drawn.is_some(), "row {index} drew nothing");
            assert_eq!(drawn, crate::row_edges(&graphs, index), "row {index}");
        }
    }

    /// Pins a shape: stops compiling if `RowContent` becomes a bare commit field.
    #[test]
    fn a_consumer_reads_a_row_by_matching_on_its_content() {
        let id = Oid::parse("0123456789abcdef0123456789abcdef01234567").unwrap();
        let rows = [
            HistoryRow {
                content: RowContent::Commit(commit(id)),
                graph: graph(id),
            },
            HistoryRow {
                content: RowContent::NotACommit,
                graph: graph(id),
            },
        ];

        let drawn: Vec<String> = rows
            .iter()
            .map(|row| match &row.content {
                RowContent::Commit(commit) => commit.summary.clone(),
                RowContent::NotACommit => "not a commit".to_owned(),
            })
            .collect();

        assert_eq!(drawn, vec!["first".to_owned(), "not a commit".to_owned()]);
    }
}
