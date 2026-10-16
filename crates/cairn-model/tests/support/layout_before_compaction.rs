//! The lane assigner as it stood before rows were compacted (`f34631c`'s
//! `crates/cairn-model/src/lane_assignment.rs`), kept verbatim as the oracle the
//! compact rows are checked against: every row it retains carries every edge crossing
//! it, which is what the drawn edges must still be. Only the names changed — its row
//! is `RowBeforeCompaction` and the assigner `AssignerBeforeCompaction`, so neither
//! is mistaken for the model's — and the module header and `GraphRow`'s definition
//! were brought in from `graph.rs`.
//!
//! Test code only: `crates/cairn-model/tests/` includes it as a module, and
//! `crates/cairn-git/tests/` and `crates/cairn-ui/tests/` include it by path. Never
//! edit the algorithm below; a change to the layout is a change to what C15 compares
//! against, and belongs in the model's assigner with its own tests.

#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};

use cairn_model::{EdgeSegment, Lane, Oid};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowBeforeCompaction {
    pub id: Oid,
    pub lane: Lane,
    /// Every line crossing this row, in no meaningful order.
    pub edges: Vec<EdgeSegment>,
}

#[derive(Debug)]
pub struct AssignerBeforeCompaction {
    /// One slot per lane; `Some(id)` is a line descending that lane waiting for `id`.
    lanes: Vec<Option<Oid>>,
    /// Absolute row number and lane of every commit still inside the window.
    laid_out: HashMap<Oid, (usize, Lane)>,
    /// Ids whose rows have left the window, oldest first.
    gone: VecDeque<Oid>,
    gone_count: HashMap<Oid, usize>,
    /// Never longer than `window`, even mid-push.
    rows: VecDeque<RowBeforeCompaction>,
    /// Absolute row number of `rows.front()`.
    first_row: usize,
    window: usize,
}

impl Default for AssignerBeforeCompaction {
    fn default() -> Self {
        Self::with_window(Self::DEFAULT_WINDOW)
    }
}

impl AssignerBeforeCompaction {
    pub const DEFAULT_WINDOW: usize = 1024;

    pub fn new() -> Self {
        Self::default()
    }

    /// Zero is raised to one.
    pub fn with_window(window: usize) -> Self {
        Self {
            lanes: Vec::new(),
            laid_out: HashMap::new(),
            gone: VecDeque::new(),
            gone_count: HashMap::new(),
            rows: VecDeque::new(),
            first_row: 0,
            window: window.max(1),
        }
    }

    pub fn window(&self) -> usize {
        self.window
    }

    /// How many ids past the window are still recognised as already laid out.
    pub fn remembered(&self) -> usize {
        self.window.saturating_mul(Self::REMEMBERED_PER_ROW)
    }

    const REMEMBERED_PER_ROW: usize = 16;

    /// Lays out a whole walk, returning every row, including those the window made final.
    pub fn assign_all<I>(commits: I) -> Vec<RowBeforeCompaction>
    where
        I: IntoIterator<Item = (Oid, Vec<Oid>)>,
    {
        let mut assigner = Self::new();
        let mut rows = Vec::new();
        for (id, parents) in commits {
            if let Some(final_row) = assigner.push(id, parents) {
                rows.push(final_row);
            }
        }
        rows.extend(assigner.into_rows());
        rows
    }

    /// The rows still inside the window, oldest first.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &RowBeforeCompaction> {
        self.rows.iter()
    }

    fn rows_laid_out(&self) -> usize {
        self.first_row + self.rows.len()
    }

    /// The rows still inside the window.
    pub fn into_rows(self) -> Vec<RowBeforeCompaction> {
        self.rows.into()
    }

    /// `parents` is in git's order. Returns the row this push forced out of the window.
    pub fn push(&mut self, id: Oid, parents: Vec<Oid>) -> Option<RowBeforeCompaction> {
        // Evict first, or the repaint below reaches one row past the window.
        let finalised = self.evict_past_the_window();
        let row_index = self.rows_laid_out();

        // Consume every lane waiting for this commit, not just the first.
        let reserved: Vec<usize> = self
            .lanes
            .iter()
            .enumerate()
            .filter(|(_, slot)| **slot == Some(id))
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
            placed.push(*parent);

            if let Some(lane) = self.lanes.iter().position(|slot| *slot == Some(*parent)) {
                // Another branch already reserved a lane for this parent.
                edges.push(EdgeSegment::out_of_commit(Lane::new(own), Lane::new(lane)));
            } else if self.laid_out.contains_key(parent) {
                // Already laid out above: connect once this row exists.
                late_parents.push(*parent);
            } else if self.gone_count.contains_key(parent) {
                // Already gone: a lane reserved for it would never be freed.
            } else {
                // Still to come; the first parent takes this commit's own lane when it is free.
                let lane = if self.lanes[own].is_none() {
                    own
                } else {
                    self.free_slot()
                };
                self.lanes[lane] = Some(*parent);
                edges.push(EdgeSegment::out_of_commit(Lane::new(own), Lane::new(lane)));
            }
        }

        self.rows.push_back(RowBeforeCompaction {
            id,
            lane: Lane::new(own),
            edges,
        });
        self.laid_out.insert(id, (row_index, Lane::new(own)));
        for parent in late_parents {
            self.connect_upward(&parent, row_index);
        }
        finalised
    }

    fn evict_past_the_window(&mut self) -> Option<RowBeforeCompaction> {
        if self.rows.len() < self.window {
            return None;
        }
        let row = self.rows.pop_front()?;
        // Only drop the index entry for *this* row: a walk can repeat an id.
        if self.laid_out.get(&row.id).map(|&(index, _)| index) == Some(self.first_row) {
            self.laid_out.remove(&row.id);
        }
        self.remember_gone(row.id);
        self.first_row += 1;
        Some(row)
    }

    /// Records `id` as gone, forgetting the oldest past [`AssignerBeforeCompaction::remembered`].
    fn remember_gone(&mut self, id: Oid) {
        *self.gone_count.entry(id).or_insert(0) += 1;
        self.gone.push_back(id);
        while self.gone.len() > self.remembered() {
            let Some(oldest) = self.gone.pop_front() else {
                break;
            };
            if let Some(count) = self.gone_count.get_mut(&oldest) {
                *count -= 1;
                if *count == 0 {
                    self.gone_count.remove(&oldest);
                }
            }
        }
    }

    /// The lowest-numbered free slot, adding one on the right if all are taken.
    fn free_slot(&mut self) -> usize {
        match self.lanes.iter().position(Option::is_none) {
            Some(lane) => lane,
            None => {
                self.lanes.push(None);
                self.lanes.len() - 1
            }
        }
    }

    /// Where a row sits in the window, or `None` if it has already left it.
    fn offset_of(&self, row_index: usize) -> Option<usize> {
        row_index
            .checked_sub(self.first_row)
            .filter(|&offset| offset < self.rows.len())
    }

    /// Draws the line up to an earlier parent, repainting the rows between.
    fn connect_upward(&mut self, parent: &Oid, child_row: usize) {
        let Some(&(parent_row, parent_lane)) = self.laid_out.get(parent) else {
            return;
        };
        if parent_row >= child_row {
            return;
        }
        let (Some(parent_offset), Some(child_offset)) =
            (self.offset_of(parent_row), self.offset_of(child_row))
        else {
            return;
        };
        let lane = self.free_lane_across(parent_offset, child_offset);
        let Some(child_lane) = self.rows.get(child_offset).map(|row| row.lane) else {
            return;
        };

        if let Some(row) = self.rows.get_mut(parent_offset) {
            row.edges
                .push(EdgeSegment::out_of_commit(parent_lane, lane).marked_out_of_order());
        }
        for row in self.rows.range_mut(parent_offset + 1..child_offset) {
            row.edges
                .push(EdgeSegment::passing(lane).marked_out_of_order());
        }
        if let Some(row) = self.rows.get_mut(child_offset) {
            row.edges
                .push(EdgeSegment::into_commit(lane, child_lane).marked_out_of_order());
        }
    }

    /// The lowest lane free on every row from offset `first` to `last` inclusive.
    fn free_lane_across(&self, first: usize, last: usize) -> Lane {
        let mut taken: Vec<bool> = Vec::new();
        for row in self.rows.range(first..=last) {
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
