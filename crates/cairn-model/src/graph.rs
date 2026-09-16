//! Lanes, edge segments and graph rows.
//!
//! Geometric vocabulary only: nothing here names genealogy, so a renderer draws
//! a row without knowing what a line means.

use crate::Oid;

/// A vertical track, numbered from the left. Assigned once and never renumbered
/// as more commits load, so it converts straight to an x position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lane(usize);

impl Lane {
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// Dense from zero; a lane is drawn only on rows whose segments name it.
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// Top edge to bottom edge, in one lane.
    Passing,
    /// Top edge in `from` to this row's node in `to`.
    IntoCommit,
    /// This row's node in `from` to the bottom edge in `to`.
    OutOfCommit,
}

/// One piece of a connecting line, clipped to a single row. Drawing a row needs
/// that row alone, which is what a virtualised list requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeSegment {
    pub from: Lane,
    pub to: Lane,
    pub kind: EdgeKind,
    /// True when the line joins a commit to a parent drawn *above* it, which
    /// date skew makes ordinary. Not a promise the other end is visible — see
    /// `docs/systems/history-graph.md`.
    pub out_of_order: bool,
}

impl EdgeSegment {
    pub fn passing(lane: Lane) -> Self {
        Self {
            from: lane,
            to: lane,
            kind: EdgeKind::Passing,
            out_of_order: false,
        }
    }

    pub fn into_commit(from: Lane, to: Lane) -> Self {
        Self {
            from,
            to,
            kind: EdgeKind::IntoCommit,
            out_of_order: false,
        }
    }

    pub fn out_of_commit(from: Lane, to: Lane) -> Self {
        Self {
            from,
            to,
            kind: EdgeKind::OutOfCommit,
            out_of_order: false,
        }
    }

    pub fn marked_out_of_order(self) -> Self {
        Self {
            out_of_order: true,
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    pub id: Oid,
    /// The node's lane, fixed for the life of the row.
    pub lane: Lane,
    /// Every line crossing this row. Order is deterministic for tests only;
    /// read nothing into it.
    pub edges: Vec<EdgeSegment>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_passing_segment_stays_in_one_lane() {
        let segment = EdgeSegment::passing(Lane::new(3));
        assert_eq!(segment.from, segment.to);
        assert_eq!(segment.from.index(), 3);
        assert_eq!(segment.kind, EdgeKind::Passing);
        assert!(!segment.out_of_order);
    }

    #[test]
    fn marking_out_of_order_changes_nothing_else() {
        let plain = EdgeSegment::out_of_commit(Lane::new(0), Lane::new(2));
        let marked = plain.marked_out_of_order();
        assert!(!plain.out_of_order);
        assert!(marked.out_of_order);
        assert_eq!(
            (marked.from, marked.to, marked.kind),
            (plain.from, plain.to, plain.kind)
        );
    }
}
