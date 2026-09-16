//! A history list as something that draws one needs it.
//!
//! [`crate::graph`] knows nothing about commits and [`CommitSummary`] knows
//! nothing about drawing; a list needs both for the same entry, and pairing
//! them by index at the call site is how the two halves drift apart.
//!
//! A row is a **list entry**, not by definition a commit
//! (`docs/prd/history-graph.md`, R6): the working-tree row occupies a lane and
//! has lines pass it, so it is laid out like any other. That variant belongs to
//! `refs-and-status`; this packet emits only commits.

use crate::{CommitSummary, GraphRow, Oid};

/// What a row is *about*.
///
/// A consumer reads a row by matching here, so the entry that is not a commit
/// arrives as a variant rather than as a nullable commit or a flag.
///
/// Deliberately **not** `#[non_exhaustive]`: a wildcard arm in a view is a row
/// silently not drawn, and R6.2 puts the cost of the next variant on whoever
/// adds it. Added FIELDS are additive by R6.3; only a new kind of row breaks a
/// match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowContent {
    /// A commit, summarised as a list needs it.
    Commit(CommitSummary),
    /// Stand-in for content that is not a commit, compiled only into this
    /// crate's own tests.
    ///
    /// It exists so A9 is decidable before the real non-commit row is written:
    /// without it, collapsing `RowContent` back into a `commit: CommitSummary`
    /// field would leave the suite green. Invisible to every other crate, so it
    /// cannot become the shape `refs-and-status` is forced to adopt.
    #[cfg(test)]
    NotACommit,
}

/// A row's stable identity: what a selection survives on, and what a detail
/// pane is opened from (R6.1).
///
/// Not an [`Oid`], and not an index: an index moves when rows arrive above it,
/// and an id assumes every row is a commit, which the working-tree row is not.
/// So identity is a sum in step with [`RowContent`]. Deliberately NOT ordered —
/// that one kind of row sorts before another is not a fact the model knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowId {
    /// A commit row, identified by the commit.
    Commit(Oid),
    /// The identity of [`RowContent::NotACommit`] — see there. Test-only, and
    /// carries no `Oid`, which is what makes "identity does not assume an id"
    /// something a test can decide.
    #[cfg(test)]
    NotACommit,
}

/// One line of history: what the row is about, and where its node and lines sit.
///
/// Nothing structurally enforces that the two halves describe the same entry,
/// so the pairing is made once, by whoever built the row, and read back through
/// [`HistoryRow::id`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub content: RowContent,
    pub graph: GraphRow,
}

impl HistoryRow {
    /// This row's identity, derived from its content rather than stored beside
    /// it: two fields that must agree are two fields that can disagree.
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

    /// Caught by: reading the identity off the graph half. The row is built
    /// with a different id in each half, so agreeing halves cannot hide it.
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

    /// A9's first half: a row can be about something that is not a commit, and
    /// its identity then holds no `Oid`.
    ///
    /// Caught by: collapsing the shape back to
    /// `HistoryRow { commit: CommitSummary, graph }` — there is then nowhere to
    /// put this row and nowhere for its identity to come from. A9's second half
    /// (consumers match rather than assume) is carried by the exhaustive matches
    /// at the call sites, not here.
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
        // Identity is per row kind, not per `Oid`: equal to itself, unequal to
        // a commit row, which is all a selection needs of it.
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

    /// Pins a shape, not a result: a row is read only by matching on its
    /// content, exhaustively and without a wildcard. Caught by: `RowContent`
    /// becoming a bare commit field — this stops compiling. `cairn-app` writes
    /// the one-arm form of the same match, so a real second variant is a compile
    /// error there rather than a row silently not drawn.
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
