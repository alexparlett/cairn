//! Lanes, edge segments and graph rows.

use crate::{LaneSnapshot, Oid};

/// A vertical track, numbered from the left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lane(usize);

impl Lane {
    pub fn new(index: usize) -> Self {
        Self(index)
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeSegment {
    pub from: Lane,
    pub to: Lane,
    pub kind: EdgeKind,
    /// True when the line joins a commit to a parent drawn *above* it.
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

/// What happens to the lanes at one row: a line ends at its commit, or leaves it. Every
/// other line crossing the row is derived ([`crate::row_edges`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LaneChange {
    /// The line descending this lane ends at the row's commit.
    Ends(Lane),
    /// A line leaves the row's commit down this lane, to a parent below: a lane it opens,
    /// or one already descending to that parent, which it joins.
    Starts(Lane),
    /// A line leaves the row's commit down `lane` to a child `rows` rows below, which names
    /// this commit as a parent although the walk delivered this one first; it is drawn out
    /// of order. `order` ranks it among the lines ending at that child.
    StartsLate { lane: Lane, rows: u32, order: u32 },
}

impl LaneChange {
    pub fn lane(self) -> Lane {
        match self {
            Self::Ends(lane) | Self::Starts(lane) | Self::StartsLate { lane, .. } => lane,
        }
    }
}

/// A row as the lane assigner keeps it: its commit, its lane and only the lane changes at
/// it. The lines crossing it are derived ([`crate::row_edges`]) from the nearest row above
/// that carries a [`LaneSnapshot`], advanced through the changes between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    pub id: Oid,
    pub lane: Lane,
    /// In the order the edges they derive are drawn: ends by lane, starts in parent order,
    /// late starts as their children arrived.
    pub changes: Box<[LaneChange]>,
    /// The lines crossing into this row, on every [`LaneAssigner::snapshot_every`]th row and
    /// on the first a reader keeps.
    ///
    /// [`LaneAssigner::snapshot_every`]: crate::LaneAssigner::snapshot_every
    pub snapshot: Option<Box<LaneSnapshot>>,
    /// Every line crossing this row, in no meaningful order.
    pub edges: Vec<EdgeSegment>,
}

impl GraphRow {
    /// A row drawn on its own: nothing crosses it but the lines its own changes name.
    pub fn new(id: Oid, lane: Lane, changes: Vec<LaneChange>) -> Self {
        Self {
            id,
            lane,
            changes: changes.into_boxed_slice(),
            snapshot: Some(Box::default()),
            edges: Vec::new(),
        }
    }

    pub fn changes(&self) -> &[LaneChange] {
        &self.changes
    }

    pub fn has_snapshot(&self) -> bool {
        self.snapshot.is_some()
    }

    pub(crate) fn snapshot(&self) -> Option<&LaneSnapshot> {
        self.snapshot.as_deref()
    }

    /// One more than the highest lane this row names — its node, its changes and, on a row
    /// with a snapshot, every line crossing into it. Over every row from the first, that is
    /// the width of everything they draw.
    pub fn lanes_named(&self) -> usize {
        let own = self
            .changes
            .iter()
            .map(|change| change.lane())
            .fold(self.lane, Lane::max);
        let crossing = self.snapshot().and_then(LaneSnapshot::highest_lane);
        crossing.map_or(own, |lane| own.max(lane)).index() + 1
    }
}

impl AsRef<GraphRow> for GraphRow {
    fn as_ref(&self) -> &GraphRow {
        self
    }
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
