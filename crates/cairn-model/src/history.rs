//! A history list as something that draws one needs it.
//!
//! The graph vocabulary in [`crate::graph`] deliberately knows nothing about
//! commits, and [`CommitSummary`] deliberately knows nothing about drawing.
//! A list needs both for the same commit on the same line, and pairing them by
//! index at the call site is how the two halves drift apart.

use crate::{CommitSummary, GraphRow, Oid};

/// One line of history: the commit, and where its node and lines sit.
///
/// The two halves describe the same commit. Nothing enforces that structurally
/// — both are plain data — so the pairing is made once, by whoever built the
/// row, and read back through [`HistoryRow::id`] rather than from one half or
/// the other at random.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub commit: CommitSummary,
    pub graph: GraphRow,
}

impl HistoryRow {
    /// The commit this row is about.
    pub fn id(&self) -> &Oid {
        &self.commit.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeSegment, Lane};

    /// `id()` reads the commit half, and says so decisively: a row built with
    /// two different ids would let an `id()` that read the graph half pass a
    /// test where both halves agree.
    #[test]
    fn a_row_takes_its_identity_from_the_commit() {
        let id = Oid::parse("0123456789abcdef0123456789abcdef01234567").unwrap();
        let other = Oid::parse("fedcba9876543210fedcba9876543210fedcba98").unwrap();
        let row = HistoryRow {
            commit: CommitSummary {
                id,
                parents: Vec::new(),
                summary: "first".to_owned(),
                author_name: "A".to_owned(),
                author_email: "a@example.com".to_owned(),
                author_time: 0,
            },
            graph: GraphRow {
                id: other,
                lane: Lane::new(0),
                edges: vec![EdgeSegment::passing(Lane::new(0))],
            },
        };
        assert_eq!(
            row.id(),
            &id,
            "the row took its identity from the graph half"
        );
        assert_ne!(row.id(), &other);
    }
}
