//! The shape of a history graph, as something that draws it needs it.
//!
//! A *row* is one commit's line in the list. A *lane* is a vertical track the
//! connecting lines run in, numbered from the left. An *edge segment* is one
//! piece of line crossing one row. Nothing here knows what a commit is: given
//! these values a renderer can draw the graph having never seen git.

use crate::Oid;

/// A vertical track in the graph, numbered from the left starting at zero.
///
/// A lane number is assigned once and never renumbered, however many more
/// commits are loaded afterwards, so it is safe to turn straight into an x
/// position. Copy because it is a `usize` wearing a name — passing it by
/// reference would cost more than copying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lane(usize);

impl Lane {
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    /// How many lanes in from the left this one sits. Lane numbers are dense
    /// from zero but a number is not a promise that the lane is occupied on
    /// any given row: a lane is only drawn where a segment names it.
    pub fn index(self) -> usize {
        self.0
    }
}

/// What a segment touches in the row it crosses.
///
/// The three cases are the whole geometric vocabulary: a line either passes a
/// row by, ends at its commit, or starts at its commit. Genealogy is not in
/// here — which end is the parent is a question for the commit data, not for
/// the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// Crosses the row without touching its commit: draw a straight line from
    /// the top edge to the bottom edge, in one lane.
    Passing,
    /// Arrives from the row above and stops at this row's commit: draw from
    /// the top edge in lane `from` to the node in lane `to`.
    IntoCommit,
    /// Leaves this row's commit and continues below: draw from the node in
    /// lane `from` to the bottom edge in lane `to`.
    OutOfCommit,
}

/// One piece of a connecting line, clipped to a single row.
///
/// Rows are self-contained on purpose: to draw row 400 you need row 400 and
/// nothing else, which is what makes a virtualised list possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeSegment {
    /// Lane the line occupies where it meets the top edge of the row, or where
    /// it leaves this row's commit.
    pub from: Lane,
    /// Lane the line occupies where it meets the bottom edge of the row, or
    /// where it reaches this row's commit.
    pub to: Lane,
    pub kind: EdgeKind,
    /// True when the line this segment belongs to joins a commit to a parent
    /// drawn *above* it instead of below: the walk handed Cairn the parent
    /// first, which committer-date skew makes ordinary. The geometry is
    /// unchanged and the line is complete; the flag exists so a renderer can
    /// mark the reversal rather than silently drawing time running backwards.
    pub out_of_order: bool,
}

impl EdgeSegment {
    /// A line crossing the row untouched. It stays in one lane because lanes
    /// are never renumbered.
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

    /// The same segment, flagged as part of a line that runs backwards on
    /// screen. See [`EdgeSegment::out_of_order`].
    pub fn marked_out_of_order(self) -> Self {
        Self {
            out_of_order: true,
            ..self
        }
    }
}

/// One commit's line in the history graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    pub id: Oid,
    /// Lane the commit's node sits in. Fixed for the life of the row.
    pub lane: Lane,
    /// Every line crossing this row, including those that begin or end at its
    /// commit. A renderer draws all of them and should not read anything into
    /// their order; the order is deterministic only so that tests can pin it.
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
