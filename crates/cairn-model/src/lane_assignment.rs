//! [`LaneAssigner`]: commits in walk order in, laid-out [`GraphRow`]s out. Pure
//! — no repository, no clock, no I/O.
//!
//! Lane indices are final from emission; edge lists inside the window are not,
//! since a parent arriving before its child adds an upward line flagged
//! [`EdgeSegment::out_of_order`] and is placed, never rejected. A row
//! [`LaneAssigner::push`] returns is final and forgotten; a line spanning more
//! than the window is not drawn at all. Blind spot past `window + remembered`:
//! `docs/systems/history-graph.md`, "The lane assigner".

use std::collections::{HashMap, VecDeque};

use crate::{EdgeSegment, GraphRow, Lane, Oid};

/// Lays commits out in lanes, one at a time, in walk order. Retained state is
/// `window x open lanes` plus a fixed number of ids, never anything that grows
/// with the walk (R1.3).
#[derive(Debug)]
pub struct LaneAssigner {
    /// One slot per lane number. `Some(id)` is a line descending that lane
    /// waiting for `id`. Slots are reused, never renumbered.
    lanes: Vec<Option<Oid>>,
    /// Where each commit still inside the window landed, by *absolute* row
    /// number. Every retained row is indexed, not just lane-openers; the entry
    /// goes when the row leaves the window, which is what makes it final.
    laid_out: HashMap<Oid, (usize, Lane)>,
    /// Commits whose rows have left the window, oldest at the front, counted so
    /// a walk repeating an id stays total. Answers "already laid out?": without
    /// it a child naming a gone parent reserves a lane nothing ever frees,
    /// painting a line down every row after it.
    gone: VecDeque<Oid>,
    gone_count: HashMap<Oid, usize>,
    /// The most recent rows, oldest at the front. Never longer than `window`,
    /// including mid-push: room is made before the new row goes in.
    rows: VecDeque<GraphRow>,
    /// Absolute row number of `rows.front()`, keeping `laid_out` indices
    /// meaningful after eviction.
    first_row: usize,
    window: usize,
}

impl Default for LaneAssigner {
    fn default() -> Self {
        Self::with_window(Self::DEFAULT_WINDOW)
    }
}

impl LaneAssigner {
    /// A load budget, not a memory mitigation: skew is local, and Git Graph and
    /// lazygit bound the same thing at 300 rows. Layout costs 264 B per row at
    /// p99 (`docs/research/history-graph/scroll-memory-model.md`, Part D).
    pub const DEFAULT_WINDOW: usize = 1024;

    pub fn new() -> Self {
        Self::default()
    }

    /// Holds at most `window` rows; zero is raised to one. Widening is not
    /// linear — 66 ms at 1024, 749 ms at 4096, 23.3 s at 16384
    /// (`docs/research/history-graph/scroll-memory-model.md`).
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

    /// How many rows this assigner holds before it starts making them final.
    pub fn window(&self) -> usize {
        self.window
    }

    /// How many commits past the window are still recognised by id, so no lane
    /// is reserved for one already gone. Ids are cheap — about 1 MB against
    /// 9 MB of rows at the default window.
    pub fn remembered(&self) -> usize {
        self.window.saturating_mul(Self::REMEMBERED_PER_ROW)
    }

    const REMEMBERED_PER_ROW: usize = 16;

    /// Lays out every commit; `commits` yields `(id, parent_ids)` in walk
    /// order. Returns every row in walk order, those the window made final
    /// carrying whatever segments they had when they left.
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

    /// The rows still inside the window, oldest first; rows already made final
    /// are not here.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &GraphRow> {
        self.rows.iter()
    }

    /// Total rows laid out, including those already made final.
    fn rows_laid_out(&self) -> usize {
        self.first_row + self.rows.len()
    }

    /// The rows still inside the window.
    pub fn into_rows(self) -> Vec<GraphRow> {
        self.rows.into()
    }

    /// Lays out one commit; `parents` is in git's order, so `parents[0]` is the
    /// first parent. Returns the row this push forced out of the window, final.
    pub fn push(&mut self, id: Oid, parents: Vec<Oid>) -> Option<GraphRow> {
        // Evict first: evicting after would leave the doomed row reachable for
        // the repaint below, so the window would reach one row further back
        // than it says.
        let finalised = self.evict_past_the_window();
        let row_index = self.rows_laid_out();

        // At most one lane waits for a given commit; consuming all of them
        // keeps this total if that ever stops holding.
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
                // Already on screen above: connect once this row exists, since
                // the line repaints the rows between.
                late_parents.push(*parent);
            } else if self.gone_count.contains_key(parent) {
                // Gone before its child arrived: the line has nowhere to start,
                // and no lane is reserved — the parent never comes round again
                // and the reservation would descend forever.
            } else {
                // Still to come. The first parent needing a fresh lane takes
                // this commit's own, keeping a linear history one lane wide.
                let lane = if self.lanes[own].is_none() {
                    own
                } else {
                    self.free_slot()
                };
                self.lanes[lane] = Some(*parent);
                edges.push(EdgeSegment::out_of_commit(Lane::new(own), Lane::new(lane)));
            }
        }

        self.rows.push_back(GraphRow {
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

    /// Drops the oldest row when the window is full, handing it back as final.
    /// One push adds one row, so at most one leaves.
    fn evict_past_the_window(&mut self) -> Option<GraphRow> {
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

    /// Records `id` as gone, forgetting the oldest past
    /// [`LaneAssigner::remembered`].
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

    /// Draws the line up from `child_row` to a parent laid out earlier,
    /// repainting the rows between. Their lane numbers are untouched; they only
    /// gain segments.
    fn connect_upward(&mut self, parent: &Oid, child_row: usize) {
        let Some(&(parent_row, parent_lane)) = self.laid_out.get(parent) else {
            return;
        };
        if parent_row >= child_row {
            return;
        }
        // Both rows are inside the window; the guards keep this total rather
        // than trusting it.
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

    /// The lowest lane unoccupied on every row from offset `first` to `last`
    /// inclusive.
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

    fn id(n: usize) -> Oid {
        let hex = format!("{n:040x}");
        Oid::parse(&hex).unwrap()
    }

    /// `branches` independent chains, interleaved, each commit naming the
    /// previous of its own chain.
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

    /// What the assigner holds, in R1.3's terms. `gone`/`gone_count` are
    /// included as the one structure bounded by a loop, not by construction.
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

    /// Caught by: eviction stopping (R1.3). The unbounded control run is what
    /// stops this passing on a history that never filled a window.
    #[test]
    fn retained_state_is_bounded_by_the_window_times_the_open_lanes() {
        let branches = 16;
        let history = wide_history(branches, 400);
        let window = 32;

        let mut bounded = LaneAssigner::with_window(window);
        let mut unbounded = LaneAssigner::with_window(usize::MAX);
        let mut peak_rows = 0;
        let mut peak_segments = 0;
        let mut peak_remembered = 0;
        for (id, parents) in &history {
            bounded.push(*id, parents.clone());
            unbounded.push(*id, parents.clone());

            let (rows, indexed, segments, widest) = retained(&bounded);
            assert!(rows <= window, "window overrun: {rows} rows held");
            assert!(indexed <= window, "index overrun: {indexed} commits held");
            // Pinned against the history's own width: a bound derived from the
            // retained rows would move with the defect.
            assert!(
                bounded.lanes.len() <= branches,
                "{} lanes open on a {branches}-branch history",
                bounded.lanes.len()
            );
            assert!(
                widest <= branches,
                "a segment reached lane {widest} on a {branches}-branch history"
            );
            assert!(
                segments <= rows * (branches + 1),
                "retained {segments} segments over {rows} rows"
            );
            // Bounded by a loop, not construction. Both structures: they trim
            // together and only one is the queue.
            assert!(
                bounded.gone.len() <= bounded.remembered(),
                "{} remembered ids past a limit of {}",
                bounded.gone.len(),
                bounded.remembered()
            );
            assert!(
                bounded.gone_count.len() <= bounded.remembered(),
                "{} counted ids past a limit of {}",
                bounded.gone_count.len(),
                bounded.remembered()
            );
            peak_remembered = peak_remembered.max(bounded.gone.len());
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
        // Without this the bounds above hold vacuously and neutering
        // `remember_gone`'s trim still passes.
        assert_eq!(
            peak_remembered,
            bounded.remembered(),
            "the remembered-id limit was never actually reached, so the bound on it decided \
             nothing"
        );
        assert!(
            history.len() > bounded.remembered(),
            "the history must outlast the remembered ids, or nothing is ever forgotten"
        );
    }

    /// R1.2's finality half: a row `push` hands back never changes.
    #[test]
    fn a_row_that_leaves_the_window_never_changes_again() {
        // `late` at row 0, its child after the window has passed it: the
        // joining line would repaint a row nobody holds.
        let window = 4;
        let mut assigner = LaneAssigner::with_window(window);
        let late = id(900);
        let child = id(901);

        let first = assigner.push(late, vec![]);
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

        // Too late: still laid out, but no line to a row that is gone.
        assigner.push(child, vec![late]);
        let rows: Vec<GraphRow> = assigner.rows().cloned().collect();
        let child_row = rows
            .iter()
            .find(|row| row.id == child)
            .expect("the child is laid out");
        assert!(
            child_row.edges.iter().all(|edge| !edge.out_of_order),
            "a line was drawn back to a row the assigner no longer holds: {child_row:?}"
        );
        // Asserting on `evicted` would decide nothing: the test owns that copy.
        assert!(
            rows.iter()
                .flat_map(|row| &row.edges)
                .all(|edge| !edge.out_of_order),
            "a backward line was drawn although its top end had left the window"
        );
    }

    /// `pairs` of parent-then-child separated by `gap` fillers, so the parent's
    /// row has left a smaller window by the time the child arrives.
    fn skew_past_the_window(pairs: usize, gap: usize) -> Vec<(Oid, Vec<Oid>)> {
        let mut commits = Vec::new();
        for n in 0..pairs {
            commits.push((id(n), Vec::new()));
            for filler in 0..gap {
                commits.push((id(1_000_000 + n * gap + filler), Vec::new()));
            }
            commits.push((id(2_000_000 + n), vec![id(n)]));
        }
        commits
    }

    /// Caught by: reserving a lane for a parent that has already left the
    /// window (R1.3). One lane leaks per event, so what is pinned is that lanes
    /// stay flat however long the walk.
    #[test]
    fn a_parent_that_went_past_before_its_child_does_not_leak_a_lane() {
        let window = 4;
        let gap = window * 2;
        let mut widths = Vec::new();
        for pairs in [2usize, 8, 32, 128] {
            let mut assigner = LaneAssigner::with_window(window);
            let history = skew_past_the_window(pairs, gap);
            let mut rows = Vec::new();
            for (id, parents) in &history {
                if let Some(row) = assigner.push(*id, parents.clone()) {
                    rows.push(row);
                }
            }
            rows.extend(assigner.rows().cloned());

            assert_eq!(rows.len(), history.len(), "a commit was lost");
            let widest = rows
                .iter()
                .flat_map(|row| {
                    std::iter::once(row.lane.index()).chain(
                        row.edges
                            .iter()
                            .flat_map(|e| [e.from.index(), e.to.index()]),
                    )
                })
                .max()
                .unwrap_or(0);
            let busiest = rows.iter().map(|row| row.edges.len()).max().unwrap_or(0);
            widths.push((assigner.lanes.len(), widest, busiest));
        }
        assert!(
            widths.windows(2).all(|pair| pair[0] == pair[1]),
            "lanes grew with the length of the walk: {widths:?}"
        );
    }

    /// Within the remembered ids the same history leaks nothing: remembering
    /// makes the difference, not the shape of the history.
    #[test]
    fn a_parent_beyond_the_remembered_ids_is_the_only_case_that_cannot_be_told_apart() {
        let mut assigner = LaneAssigner::with_window(2);
        assert_eq!(assigner.remembered(), 32);
        // A gap inside the remembered range: recognised, so no lane is kept.
        for (id, parents) in skew_past_the_window(4, 8) {
            assigner.push(id, parents);
        }
        assert!(
            assigner.lanes.iter().all(Option::is_none),
            "a lane was left reserved for a commit that had already gone: {:?}",
            assigner.lanes
        );
    }

    /// The blind spot R1.3 accepts: a parent further back than `remembered()`
    /// reserves a lane that never frees. Pins graceful degradation, not
    /// closure — every commit placed, walk order kept, one leak per event.
    #[test]
    fn a_parent_beyond_the_remembered_ids_is_placed_anyway_and_costs_one_lane() {
        let window = 2;
        let mut assigner = LaneAssigner::with_window(window);
        let events = 3;
        // `remembered()` is 32 here, so every parent falls beyond it.
        let gap = assigner.remembered() + 8;
        let history = skew_past_the_window(events, gap);

        let mut rows = Vec::new();
        for (id, parents) in &history {
            rows.extend(assigner.push(*id, parents.clone()));
        }
        // Read before draining: after `mem::take` the lanes are a fresh
        // `Default` and decide nothing.
        let reserved = assigner.lanes.iter().filter(|slot| slot.is_some()).count();
        rows.extend(std::mem::take(&mut assigner).into_rows());

        assert_eq!(
            rows.len(),
            history.len(),
            "a commit was dropped rather than placed"
        );
        // R1.4: total over arrival order — placed, not rejected.
        for ((walked, _), row) in history.iter().zip(&rows) {
            assert_eq!(*walked, row.id, "rows came back out of walk order");
        }
        // One leaked reservation per event, not per row.
        assert!(
            reserved <= events,
            "{reserved} lanes reserved after {events} unrecognised parents: the leak compounds"
        );
    }

    /// Caught by: `assign_all` dropping the rows the window made final. Every
    /// acceptance fixture is shorter than [`LaneAssigner::DEFAULT_WINDOW`], so
    /// all of them would still pass.
    #[test]
    fn assign_all_returns_every_row_of_a_walk_longer_than_the_window() {
        let length = LaneAssigner::DEFAULT_WINDOW + 500;
        let commits: Vec<(Oid, Vec<Oid>)> = (0..length)
            .map(|n| {
                let parents = if n + 1 < length {
                    vec![id(n + 1)]
                } else {
                    Vec::new()
                };
                (id(n), parents)
            })
            .collect();

        let rows = LaneAssigner::assign_all(commits);
        assert_eq!(rows.len(), length, "rows went missing past the window");
        for (n, row) in rows.iter().enumerate() {
            assert_eq!(row.id, id(n), "row {n} is out of walk order");
            assert_eq!(row.lane.index(), 0, "a linear history left lane 0");
        }
    }

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

    /// What laying a row out costs as more lanes stay open: a parent is found
    /// by scanning lane slots, and every open lane adds a passing segment per
    /// row. A measurement, not an assertion; it cannot say which term mattered.
    ///
    /// `cargo test --release -p cairn-model --lib -- --ignored --nocapture
    /// measures_layout_cost_against_lane_count`
    #[test]
    #[ignore = "a measurement, not an assertion"]
    fn measures_layout_cost_against_lane_count() {
        const COMMITS: usize = 50_000;
        const RUNS: usize = 3;
        let mut per_commit = Vec::new();
        for branches in [1usize, 2, 8, 32, 200] {
            let mut samples = Vec::new();
            // Run 0 is the warm-up and is thrown away.
            for run in 0..=RUNS {
                let history = wide_history(branches, COMMITS / branches);
                let laid_out = history.len();
                let started = std::time::Instant::now();
                let rows = LaneAssigner::assign_all(history);
                let elapsed = started.elapsed();
                assert_eq!(rows.len(), laid_out, "a commit was lost");
                if run > 0 {
                    samples.push(elapsed.as_secs_f64() * 1e9 / laid_out as f64);
                }
            }
            samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let median = samples.get(samples.len() / 2).copied().unwrap_or(f64::NAN);
            per_commit.push((branches, median));
            eprintln!("  {branches:>3} lanes\t{median:.0} ns/commit (median of {RUNS})");
        }
        let base = per_commit.first().map_or(1.0, |&(_, nanos)| nanos);
        for (branches, nanos) in per_commit {
            eprintln!("  {branches:>3} lanes\t{:.2}x one lane", nanos / base);
        }
    }
}
