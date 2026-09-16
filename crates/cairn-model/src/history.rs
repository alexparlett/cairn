//! A history list as something that draws one needs it.
//!
//! The graph vocabulary in [`crate::graph`] deliberately knows nothing about
//! commits, and [`CommitSummary`] deliberately knows nothing about drawing.
//! A list needs both for the same entry on the same line, and pairing them by
//! index at the call site is how the two halves drift apart.
//!
//! A row is a **list entry**, not by definition a commit (`docs/prd/history-graph.md`,
//! R6). The working-tree row Sourcetree shows above the first commit sits *in*
//! the graph — it occupies a lane and lines pass it — so it is laid out by the
//! engine like any other row, and reaches a view as a row whose content is not
//! a commit. That variant belongs to `refs-and-status`; this packet emits only
//! commits and deliberately invents none of its fields.

use crate::{CommitSummary, GraphRow, Oid};

/// What a row is *about*.
///
/// One variant today, which is the point: a consumer reads a row by matching
/// here, so the entry that is not a commit arrives as a variant rather than as
/// a nullable commit, a flag, or a second list beside this one.
///
/// Deliberately **not** `#[non_exhaustive]`. A wildcard arm in a view is a row
/// silently not drawn; a compile error is a decision someone has to make. The
/// cost of the next variant is paid by whoever adds it, in the places that must
/// choose what to render — which is where R6.2 says the cost belongs. Fields a
/// row later carries (ref labels, ahead/behind, on-HEAD) are additive by R6.3
/// and cost consumers nothing; only a new *kind* of row breaks a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowContent {
    /// A commit, summarised as a list needs it.
    Commit(CommitSummary),
    /// Stand-in for content that is not a commit, compiled only into this
    /// crate's own tests.
    ///
    /// It exists so A9 is decidable while the real non-commit row is still
    /// someone else's to write: without it every test would build a commit row,
    /// and collapsing `RowContent` back into a `commit: CommitSummary` field
    /// would leave the suite green. It is invisible to every other crate — an
    /// integration test and every consumer see exactly one variant — so it
    /// cannot become the shape `refs-and-status` is forced to adopt.
    #[cfg(test)]
    NotACommit,
}

/// A row's stable identity: what a selection survives on, and what a detail
/// pane is opened from (R6.1).
///
/// Not an [`Oid`], and not an index. An index moves when rows arrive above it,
/// and an id assumes every row is a commit — the working-tree row has no object
/// id at all. So identity is a sum in step with [`RowContent`]: each kind of row
/// says what identifies it, and a holder of one only needs it to compare equal
/// to itself. `Copy` because a view holds one per selection and passes it into
/// an event handler; `Hash` because a keyed list reconciles rows by it. NOT
/// ordered: an ordering would assert that one kind of row sorts before another,
/// which is not a fact the model knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowId {
    /// A commit row, identified by the commit.
    Commit(Oid),
    /// The identity of [`RowContent::NotACommit`] — see there. Test-only, and
    /// carries no `Oid`, which is what makes "identity does not assume an id"
    /// something a test can decide rather than assert by comment.
    #[cfg(test)]
    NotACommit,
}

/// One line of history: what the row is about, and where its node and lines sit.
///
/// The two halves describe the same entry. Nothing enforces that structurally —
/// both are plain data — so the pairing is made once, by whoever built the row,
/// and read back through [`HistoryRow::id`] rather than from one half or the
/// other at random.
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

    /// `id()` reads the content half, and says so decisively: a row built with
    /// two different ids would let an `id()` that read the graph half pass a
    /// test where both halves agree.
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

    /// A9's first half. A row's content is expressible as something that is not
    /// a commit — pinned by construction, since a row of that kind can be built
    /// here at all and carries a `GraphRow` like any other — and such a row has
    /// an identity, which is the part `id()` actually decides: one that holds no
    /// `Oid`, because the row it stands in for (the working tree) will not have
    /// one.
    ///
    /// This is the test that fails if the shape collapses back to
    /// `HistoryRow { commit: CommitSummary, graph }`: there is nowhere to put
    /// this row at all, and nowhere for its identity to come from. It does not
    /// reach a consumer — no other crate ever sees this variant — so A9's second
    /// half, that consumers match rather than assume, is carried by the
    /// exhaustive matches at those call sites and not by this test.
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
        // Identity is per row kind, not per `Oid`: a row carrying no object id
        // still compares equal to itself and unequal to a commit row, which is
        // all a selection needs of it.
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

    /// Pins a shape, not a result: a row can be read only by matching on its
    /// content, exhaustively and without a wildcard. It stops compiling if
    /// `RowContent` becomes a bare commit field.
    ///
    /// `cairn-app` writes the one-arm form of this match, because the second
    /// variant is test-only — this is what that match becomes when a real
    /// variant lands, and the reason landing one is a compile error at every
    /// consumer rather than a row silently not drawn.
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
