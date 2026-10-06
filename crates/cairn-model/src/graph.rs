//! Lanes, edge segments and graph rows.

use crate::{LaneSnapshot, Oid};

/// A vertical track, numbered from the left.
///
/// Held in 32 bits, so a lane change a history keeps for every row is 16 bytes rather than
/// 24: a lane is one of a row's open lines, and four billion of them is no history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lane(u32);

impl Lane {
    /// An index past `u32::MAX` is held as `u32::MAX`: no window holds that many lines.
    pub fn new(index: usize) -> Self {
        Self(u32::try_from(index).unwrap_or(u32::MAX))
    }

    pub fn index(self) -> usize {
        // `usize` is at least 32 bits on every target Cairn builds for.
        usize::try_from(self.0).unwrap_or(usize::MAX)
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
    changes: Box<[LaneChange]>,
    /// The lines crossing into this row, on every [`LaneAssigner::snapshot_every`]th row and
    /// on the first a reader keeps.
    ///
    /// [`LaneAssigner::snapshot_every`]: crate::LaneAssigner::snapshot_every
    snapshot: Option<Box<LaneSnapshot>>,
}

impl GraphRow {
    /// A row drawn on its own: nothing crosses it but the lines its own changes name.
    pub fn new(id: Oid, lane: Lane, changes: Vec<LaneChange>) -> Self {
        Self {
            id,
            lane,
            changes: changes.into_boxed_slice(),
            snapshot: Some(Box::default()),
        }
    }

    /// A row the lane assigner made final.
    pub(crate) fn laid_out(
        id: Oid,
        lane: Lane,
        changes: Box<[LaneChange]>,
        snapshot: Option<Box<LaneSnapshot>>,
    ) -> Self {
        Self {
            id,
            lane,
            changes,
            snapshot,
        }
    }

    pub fn changes(&self) -> &[LaneChange] {
        &self.changes
    }

    pub fn has_snapshot(&self) -> bool {
        self.snapshot.is_some()
    }

    /// Heap bytes the row holds beyond its own struct, by capacity: its changes and its
    /// snapshot. Allocator overhead is not counted.
    pub fn heap_bytes(&self) -> usize {
        size_of_val::<[LaneChange]>(&self.changes)
            + self.snapshot().map_or(0, |snapshot| {
                size_of::<LaneSnapshot>() + snapshot.heap_bytes()
            })
    }

    pub(crate) fn snapshot(&self) -> Option<&LaneSnapshot> {
        self.snapshot.as_deref()
    }

    /// Its changes and its snapshot, to be copied into a page.
    pub(crate) fn into_parts(self) -> (Oid, Lane, Box<[LaneChange]>, Option<Box<LaneSnapshot>>) {
        (self.id, self.lane, self.changes, self.snapshot)
    }

    /// One more than the highest lane this row names — its node, its changes and, on a row
    /// with a snapshot, every line crossing into it. Over every row from the first, that is
    /// the width of everything they draw.
    pub fn lanes_named(&self) -> usize {
        lanes_named(
            self.lane,
            &self.changes,
            self.snapshot().and_then(LaneSnapshot::highest_lane),
        )
    }
}

/// One more than the highest lane a row names: its node, its changes and the highest line
/// crossing into it, when it carries a snapshot.
pub(crate) fn lanes_named(lane: Lane, changes: &[LaneChange], crossing: Option<Lane>) -> usize {
    let own = changes
        .iter()
        .map(|change| change.lane())
        .fold(lane, Lane::max);
    crossing.map_or(own, |lane| own.max(lane)).index() + 1
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

    /// Caught by: counting a row's changes by length of something else, or its snapshot not
    /// at all.
    #[test]
    fn a_rows_heap_bytes_are_its_changes_and_its_snapshot() {
        let id = Oid::from_bytes(&[1; 20]).unwrap();
        let changes = vec![
            LaneChange::Ends(Lane::new(0)),
            LaneChange::Starts(Lane::new(0)),
            LaneChange::Starts(Lane::new(70)),
        ];
        let mut row = GraphRow::new(id, Lane::new(0), changes);
        let empty = LaneSnapshot::default();
        assert_eq!(empty.heap_bytes(), 0);
        assert_eq!(
            row.heap_bytes(),
            3 * size_of::<LaneChange>() + size_of::<LaneSnapshot>()
        );
        row.snapshot = None;
        assert_eq!(row.heap_bytes(), 3 * size_of::<LaneChange>());
    }

    /// Caught by: a lane held in a `usize` again, which makes every lane change a history
    /// keeps 24 bytes rather than 16.
    #[test]
    fn a_lane_change_is_sixteen_bytes_and_a_lane_keeps_its_index() {
        assert_eq!(size_of::<Lane>(), 4);
        assert_eq!(size_of::<LaneChange>(), 16);
        for index in [0, 1, 63, 64, 130, 65_535, 1 << 20] {
            assert_eq!(Lane::new(index).index(), index);
        }
        assert_eq!(Lane::new(usize::MAX).index(), u32::MAX as usize);
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
