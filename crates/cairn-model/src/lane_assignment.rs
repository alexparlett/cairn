//! Turning a walk of commits into laid-out graph rows.
//!
//! Feed commits in the order the walk produces them — newest first, normally —
//! and read the rows back. The assigner is pure: no repository, no clock, no
//! I/O, nothing but ids and parent ids in and [`GraphRow`]s out.
//!
//! # Arrival order
//!
//! A lane is reserved for a commit when its *child* is laid out. A commit can
//! still arrive with no lane reserved for it: git puts no ordering guarantee on
//! committer dates, so a rebase, an import or two machines with unsynchronised
//! clocks routinely deliver a parent before its child. That is not corruption
//! and the assigner never rejects it. Such a commit opens a lane of its own,
//! and when its child does turn up the line joining the two is drawn *upward*,
//! through a lane that was free on every row in between, with every segment
//! flagged [`EdgeSegment::out_of_order`].
//!
//! Drawing that line means adding segments to rows that were already laid out,
//! which is why the assigner keeps them. Lane *numbers* never change — a row's lane is final the moment it is
//! emitted — but a row's edge list can grow. That is the whole of the
//! stability contract, and it is why [`LaneAssigner::push`] returns nothing:
//! the rows are read back from the assigner, not collected as they are made.

use std::collections::HashMap;

use crate::{EdgeSegment, GraphRow, Lane, Oid};

/// Lays commits out in lanes, one commit at a time, in walk order.
///
/// Lane bookkeeping costs one slot per lane open at once. The assigner also
/// keeps the rows it has emitted and an index into them, both of which grow
/// with the walk: drawing the line to a parent that was handed over early
/// means adding segments to rows already laid out, and a row cannot be
/// repainted after it has been dropped. A caller that streams an unbounded
/// history therefore bounds the assigner by loading a window of history into
/// one, not by expecting the assigner to forget.
#[derive(Debug, Default)]
pub struct LaneAssigner {
    /// One slot per lane number. `Some(id)` means a line is descending that
    /// lane waiting for `id` to arrive; `None` means the slot is free for the
    /// next commit that needs one. Slots are reused but never renumbered.
    lanes: Vec<Option<Oid>>,
    /// Where each commit already laid out landed. A commit whose child has
    /// not been seen yet is found here when that child finally arrives, and
    /// the line between them is drawn upward. Both commits that opened their
    /// own lane and commits that had one reserved can gain a child this way,
    /// so every row is indexed, not just the unreserved ones.
    laid_out: HashMap<Oid, (usize, Lane)>,
    rows: Vec<GraphRow>,
}

impl LaneAssigner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lay out every commit in one go. `commits` yields `(id, parent_ids)` in
    /// walk order.
    pub fn assign_all<I>(commits: I) -> Vec<GraphRow>
    where
        I: IntoIterator<Item = (Oid, Vec<Oid>)>,
    {
        let mut assigner = Self::new();
        for (id, parents) in commits {
            assigner.push(id, parents);
        }
        assigner.into_rows()
    }

    /// The rows laid out so far. Lane numbers here are final; edge lists may
    /// still gain segments when a later commit turns out to be the child of a
    /// commit already in this list.
    pub fn rows(&self) -> &[GraphRow] {
        &self.rows
    }

    pub fn into_rows(self) -> Vec<GraphRow> {
        self.rows
    }

    /// Lay out one commit. `parents` is its parent ids in git's own order, so
    /// `parents[0]` is the first parent.
    pub fn push(&mut self, id: Oid, parents: Vec<Oid>) {
        let row_index = self.rows.len();

        // Lanes already waiting for this commit. At most one can exist, and no
        // caller can change that: a parent joins an existing reservation before
        // it makes a new one, and every reservation is cleared when its commit
        // arrives. Consuming all of them is a structural guard that keeps the
        // code total if that ever stops holding, not a case to be produced.
        let reserved: Vec<usize> = self
            .lanes
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.as_ref() == Some(&id))
            .map(|(lane, _)| lane)
            .collect();

        let own = match reserved.first() {
            Some(&lane) => lane,
            None => self.free_slot(),
        };

        let mut edges = Vec::new();
        for (lane, slot) in self.lanes.iter().enumerate() {
            if slot.is_some() && !reserved.contains(&lane) {
                edges.push(EdgeSegment::passing(Lane::new(lane)));
            }
        }
        for &lane in &reserved {
            edges.push(EdgeSegment::into_commit(Lane::new(lane), Lane::new(own)));
            self.lanes[lane] = None;
        }

        let mut late_parents = Vec::new();
        let mut placed = Vec::new();
        for parent in &parents {
            if placed.contains(parent) {
                // A commit may name the same parent twice; one line is enough.
                continue;
            }
            placed.push(parent.clone());

            if let Some(lane) = self
                .lanes
                .iter()
                .position(|slot| slot.as_ref() == Some(parent))
            {
                // Another branch already reserved a lane for this parent: join
                // it, rather than opening a second lane for the same commit.
                edges.push(EdgeSegment::out_of_commit(Lane::new(own), Lane::new(lane)));
            } else if self.laid_out.contains_key(parent) {
                // The parent is already on screen, above: connect once the row
                // exists, because the connection repaints the rows in between.
                late_parents.push(parent.clone());
            } else {
                // The first parent that needs a fresh lane continues straight
                // down in this commit's own lane, which is what keeps a linear
                // history one lane wide.
                let lane = if self.lanes[own].is_none() {
                    own
                } else {
                    self.free_slot()
                };
                self.lanes[lane] = Some(parent.clone());
                edges.push(EdgeSegment::out_of_commit(Lane::new(own), Lane::new(lane)));
            }
        }

        self.rows.push(GraphRow {
            id: id.clone(),
            lane: Lane::new(own),
            edges,
        });
        self.laid_out.insert(id, (row_index, Lane::new(own)));
        for parent in late_parents {
            self.connect_upward(&parent, row_index);
        }
    }

    /// The lowest-numbered free slot, adding one on the right if every slot is
    /// taken.
    fn free_slot(&mut self) -> usize {
        match self.lanes.iter().position(Option::is_none) {
            Some(lane) => lane,
            None => {
                self.lanes.push(None);
                self.lanes.len() - 1
            }
        }
    }

    /// Draw the line from the commit at `child_row` up to a parent that was
    /// laid out earlier, repainting the rows in between so the line is
    /// continuous. Lane numbers of those rows are untouched; they only gain
    /// segments.
    fn connect_upward(&mut self, parent: &Oid, child_row: usize) {
        let Some(&(parent_row, parent_lane)) = self.laid_out.get(parent) else {
            return;
        };
        if parent_row >= child_row {
            return;
        }
        let lane = self.free_lane_across(parent_row, child_row);
        let child_lane = self.rows[child_row].lane;

        self.rows[parent_row]
            .edges
            .push(EdgeSegment::out_of_commit(parent_lane, lane).marked_out_of_order());
        for row in self.rows[parent_row + 1..child_row].iter_mut() {
            row.edges
                .push(EdgeSegment::passing(lane).marked_out_of_order());
        }
        self.rows[child_row]
            .edges
            .push(EdgeSegment::into_commit(lane, child_lane).marked_out_of_order());
    }

    /// The lowest-numbered lane that is unoccupied on every row from `first` to
    /// `last` inclusive, so a line can be run down it without crossing
    /// anything already drawn.
    fn free_lane_across(&self, first: usize, last: usize) -> Lane {
        let mut taken: Vec<bool> = Vec::new();
        for row in &self.rows[first..=last] {
            let mut mark = |lane: Lane| {
                if taken.len() <= lane.index() {
                    taken.resize(lane.index() + 1, false);
                }
                taken[lane.index()] = true;
            };
            mark(row.lane);
            for edge in &row.edges {
                mark(edge.from);
                mark(edge.to);
            }
        }
        Lane::new(
            taken
                .iter()
                .position(|occupied| !occupied)
                .unwrap_or(taken.len()),
        )
    }
}
