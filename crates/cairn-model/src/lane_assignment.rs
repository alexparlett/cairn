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
//! which is why the assigner keeps them. Lane *numbers* never change — a row's
//! lane is final the moment it is emitted — but a row's edge list can grow
//! while the assigner still holds the row.
//!
//! # The window
//!
//! The assigner holds only the most recent [`LaneAssigner::window`] rows. A row
//! pushed out of that window is **final**: [`LaneAssigner::push`] hands it back,
//! the assigner forgets it, and nothing can repaint it afterwards. That is what
//! makes retained state proportional to the window rather than to the length of
//! the walk, and it is why the window lives here rather than in the caller — a
//! window imposed from outside cannot stop the assigner reaching back past it.
//!
//! The cost is stated rather than hidden: a parent whose child has already left
//! the window opens its own lane and the joining line is simply not drawn. The
//! line would have had to start on a row nobody holds any more. A window wide
//! enough for the skew a repository actually contains never reaches this case;
//! a window narrower than the skew loses lines, not commits.

use std::collections::{HashMap, VecDeque};

use crate::{EdgeSegment, GraphRow, Lane, Oid};

/// Lays commits out in lanes, one commit at a time, in walk order.
///
/// Retained state is bounded by the window: at most `window` rows, an index
/// over exactly those rows, and one slot per lane open at once. Each retained
/// row carries at most one segment per lane open across it, so the whole of
/// what the assigner holds is `window x open lanes` — never anything that
/// grows with the number of commits walked.
#[derive(Debug)]
pub struct LaneAssigner {
    /// One slot per lane number. `Some(id)` means a line is descending that
    /// lane waiting for `id` to arrive; `None` means the slot is free for the
    /// next commit that needs one. Slots are reused but never renumbered.
    lanes: Vec<Option<Oid>>,
    /// Where each commit still inside the window landed, by *absolute* row
    /// number. A commit whose child has not been seen yet is found here when
    /// that child finally arrives, and the line between them is drawn upward.
    /// Both commits that opened their own lane and commits that had one
    /// reserved can gain a child this way, so every retained row is indexed,
    /// not just the unreserved ones. An entry is dropped when its row leaves
    /// the window, which is what makes that row final.
    laid_out: HashMap<Oid, (usize, Lane)>,
    /// The window: the most recent rows, oldest at the front. Never longer
    /// than `window`, including while a row is being laid out — room is made
    /// before the new row goes in, not after.
    rows: VecDeque<GraphRow>,
    /// Absolute row number of `rows.front()`, so an index kept in `laid_out`
    /// stays meaningful after rows have been dropped from the front.
    first_row: usize,
    window: usize,
}

impl Default for LaneAssigner {
    fn default() -> Self {
        Self::with_window(Self::DEFAULT_WINDOW)
    }
}

impl LaneAssigner {
    /// How many rows [`LaneAssigner::new`] keeps.
    ///
    /// Committer-date skew is local: a rebase, an import or a clock a few
    /// minutes out reorders neighbours, not halves of a history. A thousand
    /// rows is far wider than any skew observed in the evidence record, and it
    /// bounds what a busy history retains to megabytes rather than gigabytes —
    /// a 200-branch history measured at 361 segments per row costs about 9 MB
    /// here against 5.4 GB unbounded across 500k rows. A caller that knows its
    /// repository better sets its own with [`LaneAssigner::with_window`].
    pub const DEFAULT_WINDOW: usize = 1024;

    pub fn new() -> Self {
        Self::default()
    }

    /// An assigner holding at most `window` rows. A window of zero is raised to
    /// one: the row being laid out has to exist while it is laid out.
    pub fn with_window(window: usize) -> Self {
        Self {
            lanes: Vec::new(),
            laid_out: HashMap::new(),
            rows: VecDeque::new(),
            first_row: 0,
            window: window.max(1),
        }
    }

    /// How many rows this assigner holds before it starts making them final.
    pub fn window(&self) -> usize {
        self.window
    }

    /// Lay out every commit in one go. `commits` yields `(id, parent_ids)` in
    /// walk order.
    ///
    /// Every row is returned, in walk order — the ones the window made final
    /// along the way and the ones still inside it at the end. Rows that left
    /// the window carry whatever segments they had when they left, which is
    /// the difference between this and an assigner that could hold the whole
    /// walk.
    pub fn assign_all<I>(commits: I) -> Vec<GraphRow>
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

    /// The rows still inside the window, oldest first. Lane numbers here are
    /// final; edge lists may still gain segments. Rows already made final are
    /// not here — [`LaneAssigner::push`] handed those back.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &GraphRow> {
        self.rows.iter()
    }

    /// How many rows have been laid out in total, including the ones the window
    /// has already made final.
    fn rows_laid_out(&self) -> usize {
        self.first_row + self.rows.len()
    }

    /// Consume the assigner and take the rows still inside the window.
    pub fn into_rows(self) -> Vec<GraphRow> {
        self.rows.into()
    }

    /// Lay out one commit. `parents` is its parent ids in git's own order, so
    /// `parents[0]` is the first parent.
    ///
    /// Returns the row this push forced out of the window, if it forced one
    /// out. That row is final: it will never gain another segment, and the
    /// assigner no longer holds it.
    pub fn push(&mut self, id: Oid, parents: Vec<Oid>) -> Option<GraphRow> {
        // Make room *before* laying the row out, not after. Evicting afterwards
        // would leave the row about to be dropped still reachable for the
        // repaint below, so the window would quietly reach one row further back
        // than it says it does.
        let finalised = self.evict_past_the_window();
        let row_index = self.rows_laid_out();

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
                // Either a parent still to come, or one whose row has already
                // left the window — in which case there is nothing to draw a
                // line back to and it opens a lane of its own. The first parent
                // that needs a fresh lane continues straight down in this
                // commit's own lane, which is what keeps a linear history one
                // lane wide.
                let lane = if self.lanes[own].is_none() {
                    own
                } else {
                    self.free_slot()
                };
                self.lanes[lane] = Some(parent.clone());
                edges.push(EdgeSegment::out_of_commit(Lane::new(own), Lane::new(lane)));
            }
        }

        self.rows.push_back(GraphRow {
            id: id.clone(),
            lane: Lane::new(own),
            edges,
        });
        self.laid_out.insert(id, (row_index, Lane::new(own)));
        for parent in late_parents {
            self.connect_upward(&parent, row_index);
        }
        finalised
    }

    /// Drop the oldest row if the window has no room for another, and hand it
    /// back as final. One push adds one row, so at most one row leaves per
    /// push.
    fn evict_past_the_window(&mut self) -> Option<GraphRow> {
        if self.rows.len() < self.window {
            return None;
        }
        let row = self.rows.pop_front()?;
        // Only drop the index entry that points at *this* row: a walk can hand
        // over the same id twice, and the later row is the one still held.
        if self.laid_out.get(&row.id).map(|&(index, _)| index) == Some(self.first_row) {
            self.laid_out.remove(&row.id);
        }
        self.first_row += 1;
        Some(row)
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

    /// Where a row sits in the window, or `None` if it has already left it.
    fn offset_of(&self, row_index: usize) -> Option<usize> {
        row_index
            .checked_sub(self.first_row)
            .filter(|&offset| offset < self.rows.len())
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
        // Both rows are inside the window: `laid_out` only holds rows that are,
        // and the child was pushed a moment ago. The guards keep the code total
        // rather than trusting that.
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

    /// The lowest-numbered lane that is unoccupied on every row from window
    /// offset `first` to `last` inclusive, so a line can be run down it without
    /// crossing anything already drawn.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A commit id from a small integer, so a generated history reads back.
    fn id(n: usize) -> Oid {
        let hex = format!("{n:040x}");
        Oid::parse(&hex).unwrap()
    }

    /// A history wide enough to keep many lanes open at once and long enough to
    /// dwarf any window under test: `branches` independent chains, interleaved,
    /// each commit naming the previous commit of its own chain.
    fn wide_history(branches: usize, length: usize) -> Vec<(Oid, Vec<Oid>)> {
        let mut commits = Vec::new();
        for step in 0..length {
            for branch in 0..branches {
                let here = step * branches + branch;
                let parent = here + branches;
                if step + 1 < length {
                    commits.push((id(here), vec![id(parent)]));
                } else {
                    commits.push((id(here), Vec::new()));
                }
            }
        }
        commits
    }

    /// The whole of what the assigner holds, in the terms R1.3 states it in.
    fn retained(assigner: &LaneAssigner) -> (usize, usize, usize, usize) {
        let segments: usize = assigner.rows.iter().map(|row| row.edges.len()).sum();
        let widest = assigner
            .rows
            .iter()
            .flat_map(|row| {
                std::iter::once(row.lane.index()).chain(
                    row.edges
                        .iter()
                        .flat_map(|e| [e.from.index(), e.to.index()]),
                )
            })
            .max()
            .map_or(0, |lane| lane + 1);
        (
            assigner.rows.len(),
            assigner.laid_out.len(),
            segments,
            widest,
        )
    }

    /// R1.3, the bound the window exists to meet: retained state is
    /// `window x open lanes`, never anything that grows with the walk.
    ///
    /// The control run is the point — an unbounded assigner over the same
    /// history retains every row, so this test fails the moment eviction stops
    /// happening rather than passing on a history that never filled a window.
    #[test]
    fn retained_state_is_bounded_by_the_window_times_the_open_lanes() {
        let branches = 16;
        let history = wide_history(branches, 400);
        let window = 32;

        let mut bounded = LaneAssigner::with_window(window);
        let mut unbounded = LaneAssigner::with_window(usize::MAX);
        let mut peak_rows = 0;
        let mut peak_segments = 0;
        for (id, parents) in &history {
            bounded.push(id.clone(), parents.clone());
            unbounded.push(id.clone(), parents.clone());

            let (rows, indexed, segments, widest) = retained(&bounded);
            assert!(rows <= window, "window overrun: {rows} rows held");
            assert!(indexed <= window, "index overrun: {indexed} commits held");
            // One segment per lane open across the row, plus the one leaving
            // the row's own node. Anything above that is a row holding more
            // than the lanes crossing it can account for.
            assert!(
                segments <= rows * (widest + 1),
                "retained {segments} segments over {rows} rows {widest} lanes wide"
            );
            peak_rows = peak_rows.max(rows);
            peak_segments = peak_segments.max(segments);
        }

        assert_eq!(peak_rows, window, "the window was never actually filled");
        assert!(
            peak_segments > window,
            "the history never opened a second lane, so the bound decided nothing"
        );
        assert_eq!(
            unbounded.rows.len(),
            history.len(),
            "the control run must retain everything, or the bound proves nothing"
        );
        assert!(
            history.len() > window * 8,
            "the history must dwarf the window"
        );
    }

    /// The finality half of R1.2: a row handed back by `push` is the row, and
    /// nothing that happens afterwards can change it.
    #[test]
    fn a_row_that_leaves_the_window_never_changes_again() {
        // `late` is delivered at row 0 and its child `child` only after the
        // window has moved past it, so the joining line would have to repaint a
        // row nobody holds.
        let window = 4;
        let mut assigner = LaneAssigner::with_window(window);
        let late = id(900);
        let child = id(901);

        let first = assigner.push(late.clone(), vec![]);
        assert!(
            first.is_none(),
            "nothing leaves the window on the first push"
        );

        let mut finals = Vec::new();
        for filler in 0..window {
            if let Some(row) = assigner.push(id(filler), vec![]) {
                finals.push(row);
            }
        }
        assert_eq!(finals.len(), 1, "exactly one row left the window");
        let evicted = finals[0].clone();
        assert_eq!(evicted.id, late, "the oldest row is the one made final");

        // The child turns up too late. It must still be laid out, and it must
        // not pretend to draw a line to a row that is gone.
        assigner.push(child.clone(), vec![late.clone()]);
        let rows: Vec<GraphRow> = assigner.rows().cloned().collect();
        let child_row = rows
            .iter()
            .find(|row| row.id == child)
            .expect("the child is laid out");
        assert!(
            child_row.edges.iter().all(|edge| !edge.out_of_order),
            "a line was drawn back to a row the assigner no longer holds: {child_row:?}"
        );
        assert_eq!(
            evicted, finals[0],
            "the row handed back changed after it was handed back"
        );
    }

    /// A window of zero would drop the row being laid out; it is raised to one.
    #[test]
    fn a_zero_window_still_lays_out_one_row() {
        let mut assigner = LaneAssigner::with_window(0);
        assert_eq!(assigner.window(), 1);
        assert!(assigner.push(id(1), vec![id(2)]).is_none());
        assert_eq!(assigner.rows().len(), 1);
        let evicted = assigner.push(id(2), vec![]).expect("row 0 is now final");
        assert_eq!(evicted.id, id(1));
        assert_eq!(
            assigner.rows().len(),
            1,
            "the window holds one row at a time"
        );
    }
}
