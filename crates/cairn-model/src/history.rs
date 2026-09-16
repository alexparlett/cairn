//! Row content, row identity, and the pairing of the two with a graph row.
//!
//! A row is a list entry, not by definition a commit (R6): the working-tree row
//! lays out like any other. That variant belongs to `refs-and-status`; this
//! packet emits only commits.

use crate::{CommitSummary, GraphRow, Oid};

/// What a row is about. Deliberately not `#[non_exhaustive]`: a wildcard arm is
/// a row silently not drawn, so R6.2 puts the cost of a new variant on whoever
/// adds it. New fields stay additive (R6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowContent {
    Commit(CommitSummary),
    /// Test-only stand-in making A9 decidable: without it, collapsing
    /// `RowContent` into a `commit: CommitSummary` field would leave the suite
    /// green. Invisible to every other crate.
    #[cfg(test)]
    NotACommit,
}

/// A row's stable identity, survived by a selection and opened from by a detail
/// pane (R6.1). Not an index (it moves as rows arrive above) and not an
/// [`Oid`] (the working-tree row has none). Deliberately not ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowId {
    Commit(Oid),
    /// Test-only, and carries no `Oid` — what makes "identity does not assume
    /// an id" decidable.
    #[cfg(test)]
    NotACommit,
}

/// One line of history. Nothing enforces that the two halves describe the same
/// entry; the builder pairs them once and [`HistoryRow::id`] reads it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub content: RowContent,
    pub graph: GraphRow,
}

impl HistoryRow {
    /// Derived from `content`, never stored beside it: two fields that must
    /// agree can disagree.
    pub fn id(&self) -> RowId {
        match &self.content {
            RowContent::Commit(commit) => RowId::Commit(commit.id),
            #[cfg(test)]
            RowContent::NotACommit => RowId::NotACommit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeSegment, Lane};

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
        GraphRow {
            id,
            lane: Lane::new(0),
            edges: vec![EdgeSegment::passing(Lane::new(0))],
        }
    }

    /// Caught by: reading the identity off the graph half; the halves carry
    /// different ids.
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

    /// Caught by: collapsing the shape to `HistoryRow { commit, graph }`, which
    /// leaves nowhere to put this row. A9's second half — consumers match
    /// rather than assume — is carried by the call sites, not here.
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

    /// Pins a shape, not a result. Caught by: `RowContent` becoming a bare
    /// commit field, which stops this compiling. `cairn-app` writes the same
    /// match without a wildcard.
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
