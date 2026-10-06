//! Lane assigner tests.

mod histories;

use cairn_model::LaneAssigner;
use histories::{
    History, assert_every_parent_edge_is_drawn, assert_rows_are_well_formed, assign,
    branch_and_merge, corpus, criss_cross, delivers_a_parent_before_its_child, describe,
    lane_outlives_the_one_to_its_left, linear, links_delivered_backwards, literal, multiple_roots,
    octopus_merge, random_history, skewed, skewed_across_a_busy_span,
};

// --- Lanes and edges ---

/// Caught by: not freeing a commit's lane on arrival.
#[test]
fn linear_history_stays_in_one_lane() {
    let history = linear();
    let rows = assign(&history);
    assert_eq!(
        describe(&rows),
        [
            "c3 lane=0 [out 0>0]",
            "c2 lane=0 [in 0>0, out 0>0]",
            "c1 lane=0 [in 0>0]",
        ]
    );
    assert_every_parent_edge_is_drawn(&history, &rows);
    assert_rows_are_well_formed(&rows);
}

/// Caught by: dropping the deduplication of parent reservations.
#[test]
fn a_branch_and_merge_opens_one_lane_and_closes_it() {
    let history = branch_and_merge();
    let rows = assign(&history);
    assert_eq!(
        describe(&rows),
        [
            "m lane=0 [out 0>0, out 0>1]",
            "a lane=0 [pass 1, in 0>0, out 0>0]",
            "b lane=1 [pass 0, in 1>1, out 1>0]",
            "base lane=0 [in 0>0]",
        ]
    );
    assert_every_parent_edge_is_drawn(&history, &rows);
    assert_rows_are_well_formed(&rows);
}

/// Caught by: handling only a merge's first two parents, or dropping reservation deduplication.
#[test]
fn an_octopus_merge_draws_a_line_to_every_parent() {
    let history = octopus_merge();
    let rows = assign(&history);
    assert_eq!(
        describe(&rows),
        [
            "o lane=0 [out 0>0, out 0>1, out 0>2]",
            "p1 lane=0 [pass 1, pass 2, in 0>0, out 0>0]",
            "p2 lane=1 [pass 0, pass 2, in 1>1, out 1>0]",
            "p3 lane=2 [pass 0, in 2>2, out 2>0]",
            "base lane=0 [in 0>0]",
        ]
    );
    assert_every_parent_edge_is_drawn(&history, &rows);
    assert_rows_are_well_formed(&rows);
}

/// Caught by: a commit with no reservation taking a lane still in use.
#[test]
fn criss_cross_merges_keep_both_shared_parents_on_one_lane_each() {
    let history = criss_cross();
    let rows = assign(&history);
    assert_eq!(
        describe(&rows),
        [
            "m1 lane=0 [out 0>0, out 0>1]",
            "m2 lane=2 [pass 0, pass 1, out 2>0, out 2>1]",
            "a lane=0 [pass 1, in 0>0, out 0>0]",
            "b lane=1 [pass 0, in 1>1, out 1>0]",
            "base lane=0 [in 0>0]",
        ]
    );
    assert_every_parent_edge_is_drawn(&history, &rows);
    assert_rows_are_well_formed(&rows);
}

/// Caught by: skipping a commit that arrives with no lane reserved.
#[test]
fn multiple_roots_each_get_their_own_lane() {
    let history = multiple_roots();
    let rows = assign(&history);
    assert_eq!(
        describe(&rows),
        [
            "a2 lane=0 [out 0>0]",
            "b2 lane=1 [pass 0, out 1>1]",
            "a1 lane=0 [pass 1, in 0>0]",
            "b1 lane=1 [in 1>1]",
        ]
    );
    assert_every_parent_edge_is_drawn(&history, &rows);
    assert_rows_are_well_formed(&rows);
}

/// Caught by: giving a first parent the lowest free lane rather than the commit's own.
#[test]
fn a_branch_keeps_its_lane_when_the_one_to_its_left_falls_empty() {
    let history = lane_outlives_the_one_to_its_left();
    let rows = assign(&history);
    assert_eq!(
        describe(&rows),
        [
            "t1 lane=0 [out 0>0]",
            "t2 lane=1 [pass 0, out 1>1]",
            "a lane=0 [pass 1, in 0>0]",
            "b lane=1 [in 1>1, out 1>1]",
            "c lane=1 [in 1>1]",
        ]
    );
    assert_every_parent_edge_is_drawn(&history, &rows);
    assert_rows_are_well_formed(&rows);
}

// --- Skew ---

/// Caught by: reserving a lane for an already-laid-out parent, or not repainting the rows between.
#[test]
fn skewed_history_places_every_commit_and_draws_every_parent_edge() {
    let history = skewed();
    assert!(
        delivers_a_parent_before_its_child(&history),
        "the fixture must actually hand a parent over before its child, \
         or this test passes on the ordinary path"
    );

    let rows = assign(&history);
    assert_eq!(
        describe(&rows),
        [
            "p lane=0 [out 0>0, out 0>2~]",
            "t lane=1 [pass 0, out 1>0, pass 2~]",
            "c lane=1 [pass 0, in 2>1~]",
            "base lane=0 [in 0>0]",
        ]
    );
    assert_every_parent_edge_is_drawn(&history, &rows);
    assert_rows_are_well_formed(&rows);

    let backwards: Vec<_> = rows
        .iter()
        .flat_map(|row| &row.edges)
        .filter(|edge| edge.out_of_order)
        .collect();
    assert_eq!(
        backwards.len(),
        3,
        "the c->p line is one segment per row it crosses, and only that line \
         is flagged"
    );
}

/// Caught by: a connecting line down a lane already in use, or a partial repaint.
#[test]
fn a_line_to_a_parent_delivered_early_runs_down_a_lane_that_is_free_throughout() {
    let history = skewed_across_a_busy_span();
    assert_eq!(
        links_delivered_backwards(&history),
        [(2, 5)],
        "the fixture must hand `p` over four rows before its child `c`"
    );

    let rows = assign(&history);
    assert_eq!(
        describe(&rows),
        [
            "t0 lane=0 [out 0>0]",
            "t1 lane=1 [pass 0, out 1>1]",
            "p lane=2 [pass 0, pass 1, out 2>2, out 2>3~]",
            "a lane=0 [pass 1, pass 2, in 0>0, out 0>0, pass 3~]",
            "b lane=1 [pass 0, pass 2, in 1>1, out 1>0, pass 3~]",
            "c lane=1 [pass 0, pass 2, in 3>1~]",
            "e lane=2 [pass 0, in 2>2]",
            "f lane=0 [in 0>0]",
        ]
    );
    assert_every_parent_edge_is_drawn(&history, &rows);
    assert_rows_are_well_formed(&rows);
}

#[test]
fn generated_skewed_histories_are_all_placed_and_all_connected() {
    let mut saw_skew = false;
    for seed in 1..80u64 {
        for len in [1usize, 2, 7, 25] {
            let history = random_history(seed, len, true);
            saw_skew |= delivers_a_parent_before_its_child(&history);
            let rows = assign(&history);
            assert_every_parent_edge_is_drawn(&history, &rows);
            assert_rows_are_well_formed(&rows);
        }
    }
    assert!(
        saw_skew,
        "the generator never produced an out-of-order walk"
    );
}

// --- Stability ---

/// Caught by: renumbering lanes as commits arrive, or rewriting existing segments.
#[test]
fn lane_indices_never_change_when_more_commits_are_assigned() {
    let mut cases = corpus();
    for seed in 1..40u64 {
        cases.push(("generated", random_history(seed, 20, true)));
        // Control: nothing is delivered backwards, so no row may be repainted.
        cases.push(("generated in order", random_history(seed, 20, false)));
    }

    let mut saw_a_gained_segment = false;
    for (name, history) in &cases {
        let whole = assign(history);
        for prefix_len in 0..=history.len() {
            let prefix: History = history[..prefix_len].to_vec();
            let partial = assign(&prefix);
            assert_eq!(partial.len(), prefix_len, "{name}: one row per commit");

            for (index, (before, after)) in partial.iter().zip(whole.iter()).enumerate() {
                assert_eq!(
                    before.id, after.id,
                    "{name}: row {index} is a different commit"
                );
                assert_eq!(
                    before.lane,
                    after.lane,
                    "{name}: row {index} moved lane when {} more commits were assigned",
                    history.len() - prefix_len
                );
                assert!(
                    after.edges.starts_with(&before.edges),
                    "{name}: row {index} had an edge rewritten, not added\nwas:  {:?}\nnow:  {:?}",
                    before.edges,
                    after.edges
                );
                let backwards = links_delivered_backwards(history);
                for gained in &after.edges[before.edges.len()..] {
                    saw_a_gained_segment = true;
                    // Entitlement comes from walk order, not from the assigner's own flag.
                    let entitled = backwards.iter().any(|&(parent, child)| {
                        parent <= index && index <= child && child >= prefix_len
                    });
                    assert!(
                        entitled,
                        "{name}: row {index} gained {gained:?}, but no parent delivered \
                         early spans it. Backward links: {backwards:?}"
                    );
                    assert!(
                        gained.out_of_order,
                        "{name}: row {index} gained {gained:?} unflagged, so a renderer \
                         cannot tell the line runs backwards"
                    );
                }
            }
        }
    }
    assert!(
        saw_a_gained_segment,
        "no case ever repainted a row, so the edge half of A3 decided nothing"
    );
}

// --- Arrival order ---

#[test]
fn assignment_is_deterministic() {
    for (name, history) in corpus() {
        assert_eq!(
            describe(&assign(&history)),
            describe(&assign(&history)),
            "{name}"
        );
    }
}

/// Rows handed back by `push` plus those left in the window must be the whole walk, in order.
#[test]
fn pushing_one_at_a_time_matches_assigning_the_whole_walk() {
    for (name, history) in corpus() {
        let mut assigner = LaneAssigner::new();
        let mut streamed = Vec::new();
        for (id, parents) in &history {
            if let Some(finalised) = assigner.push(
                histories::oid(id),
                parents.iter().map(|p| histories::oid(p)).collect(),
            ) {
                streamed.push(finalised);
            }
        }
        streamed.extend(assigner.into_rows());
        assert_eq!(
            describe(&histories::drawn(&streamed)),
            describe(&assign(&history)),
            "{name}"
        );
    }
}

#[test]
fn malformed_input_is_placed_rather_than_rejected() {
    assert!(assign(&literal(&[])).is_empty());

    let repeated = literal(&[("a", &["b"]), ("b", &[]), ("b", &[])]);
    assert_eq!(
        describe(&assign(&repeated)),
        ["a lane=0 [out 0>0]", "b lane=0 [in 0>0]", "b lane=0 []",],
        "the second delivery of a commit gets its own row and draws nothing twice"
    );

    let doubled_parent = literal(&[("a", &["b", "b"]), ("b", &[])]);
    let rows = assign(&doubled_parent);
    assert_eq!(
        describe(&rows),
        ["a lane=0 [out 0>0]", "b lane=0 [in 0>0]"],
        "one line, not two, for a parent named twice"
    );

    let dangling = literal(&[("a", &["missing"])]);
    let rows = assign(&dangling);
    assert_eq!(describe(&rows), ["a lane=0 [out 0>0]"]);
    assert_every_parent_edge_is_drawn(&dangling, &rows);
}

// --- Compact rows (C15): every row draws what the assigner drew before compaction ---

mod layout_before_compaction;

use cairn_model::{EdgeKind, GraphRow, LaneChange, Oid, RowEdges, row_edges};
use layout_before_compaction::{AssignerBeforeCompaction, RowBeforeCompaction};

fn walk(history: &History) -> Vec<(Oid, Vec<Oid>)> {
    history
        .iter()
        .map(|(id, parents)| {
            (
                histories::oid(id),
                parents.iter().map(|p| histories::oid(p)).collect(),
            )
        })
        .collect()
}

/// The rows the assigner retained before compaction, made final as it made them.
fn before_compaction(history: &History, window: usize) -> Vec<RowBeforeCompaction> {
    let mut assigner = AssignerBeforeCompaction::with_window(window);
    let mut rows = Vec::new();
    for (id, parents) in walk(history) {
        rows.extend(assigner.push(id, parents));
    }
    rows.extend(assigner.into_rows());
    rows
}

fn compacted(history: &History, window: usize, every: usize) -> Vec<GraphRow> {
    LaneAssigner::with_window(window)
        .with_snapshot_every(every)
        .assign_each(walk(history))
}

/// A child whose two parents were both delivered before it, in the reverse of its parent
/// order: the two out-of-order lines cross `p2`'s row in the child's parent order, not in
/// the order their tops were laid out.
fn two_late_parents_in_reverse() -> History {
    literal(&[
        ("p1", &["base"]),
        ("p2", &["base"]),
        ("t", &["base"]),
        ("c", &["p2", "p1"]),
        ("base", &[]),
    ])
}

/// What the equivalence runs exercised, so a run that never reached a case cannot pass it.
#[derive(Debug, Default)]
struct Exercised {
    rows: usize,
    repainted: usize,
    /// A snapshot carrying an out-of-order line that started above it.
    snapshot_with_a_late_line: usize,
    /// A late line started between a row and its snapshot, and crosses the row.
    late_start_advanced_through: usize,
    /// The furthest any row was derived from its snapshot.
    furthest_advance: usize,
}

/// Every row's derived edges are the edges the old assigner retained for it, in its order.
fn assert_draws_what_it_drew(
    name: &str,
    before: &[RowBeforeCompaction],
    after: &[GraphRow],
    seen: &mut Exercised,
) {
    assert_eq!(after.len(), before.len(), "{name}: a row was lost");
    for (index, (old, new)) in before.iter().zip(after).enumerate() {
        assert_eq!(new.id, old.id, "{name}: row {index} is a different commit");
        assert_eq!(
            row_edges(after, index),
            Some(RowEdges {
                lane: old.lane,
                edges: old.edges.clone(),
            }),
            "{name}: row {index} draws other edges than the assigner retained for it"
        );

        seen.rows += 1;
        let late_crossing = old
            .edges
            .iter()
            .any(|edge| edge.out_of_order && edge.kind != EdgeKind::OutOfCommit);
        if old.edges.iter().any(|edge| edge.out_of_order) {
            seen.repainted += 1;
        }
        if new.has_snapshot() && late_crossing {
            seen.snapshot_with_a_late_line += 1;
        }
        let snapshot_row = (0..=index)
            .rev()
            .find(|&above| after[above].has_snapshot())
            .unwrap_or(index);
        seen.furthest_advance = seen.furthest_advance.max(index - snapshot_row);
        let started_since = (snapshot_row..index).any(|above| {
            after[above].changes().iter().any(|change| {
                matches!(*change, LaneChange::StartsLate { rows, .. }
                    if above + rows as usize >= index)
            })
        });
        if started_since {
            seen.late_start_advanced_through += 1;
        }
    }
}

#[test]
fn every_row_draws_the_edges_the_assigner_retained_before_compaction() {
    let mut cases = corpus();
    cases.push(("two late parents in reverse", two_late_parents_in_reverse()));
    for seed in 1..160u64 {
        for len in [1usize, 2, 7, 25, 150] {
            cases.push(("generated", random_history(seed, len, true)));
            cases.push(("generated in order", random_history(seed, len, false)));
        }
    }

    let mut seen = Exercised::default();
    for (name, history) in &cases {
        for window in [1usize, 2, 3, 5, 1024] {
            let before = before_compaction(history, window);
            for every in [1usize, 2, 3, 7, LaneAssigner::SNAPSHOT_EVERY] {
                let after = compacted(history, window, every);
                assert_draws_what_it_drew(
                    &format!("{name} (window {window}, a snapshot every {every})"),
                    &before,
                    &after,
                    &mut seen,
                );
            }
        }
    }

    assert!(seen.repainted > 0, "no row was repainted: {seen:?}");
    assert!(
        seen.snapshot_with_a_late_line > 0,
        "no snapshot carried an out-of-order line: {seen:?}"
    );
    assert!(
        seen.late_start_advanced_through > 0,
        "no derivation advanced through a late line's start: {seen:?}"
    );
    assert_eq!(
        seen.furthest_advance,
        LaneAssigner::SNAPSHOT_EVERY - 1,
        "no row was drawn from the furthest a snapshot can be above it: {seen:?}"
    );
}

/// Caught by: drawing a row's out-of-order lines in the order their tops were laid out.
#[test]
fn two_late_lines_cross_a_row_in_their_childs_parent_order() {
    let history = two_late_parents_in_reverse();
    let before = before_compaction(&history, 1024);
    let rows = compacted(&history, 1024, LaneAssigner::SNAPSHOT_EVERY);
    assert_eq!(
        describe(&histories::drawn(&rows)),
        [
            "p1 lane=0 [out 0>0, out 0>3~]",
            "p2 lane=1 [pass 0, out 1>0, out 1>2~, pass 3~]",
            "t lane=1 [pass 0, out 1>0, pass 2~, pass 3~]",
            "c lane=1 [pass 0, in 2>1~, in 3>1~]",
            "base lane=0 [in 0>0]",
        ]
    );
    let mut seen = Exercised::default();
    assert_draws_what_it_drew("two late parents in reverse", &before, &rows, &mut seen);
}

/// Caught by: a snapshot on the wrong row, or none on the first row a reader keeps.
#[test]
fn a_snapshot_falls_every_k_rows_and_on_the_first_row_kept() {
    let history = random_history(7, 60, true);
    for every in [1usize, 3, 7, 64] {
        for kept in [0usize, 1, 5, 21, 59] {
            let rows = LaneAssigner::with_window(4)
                .with_snapshot_every(every)
                .drawn_from(kept)
                .assign_each(walk(&history));
            let carrying: Vec<usize> = (0..rows.len())
                .filter(|&index| rows[index].has_snapshot())
                .collect();
            let expected: Vec<usize> = (0..rows.len())
                .filter(|&index| index.is_multiple_of(every) || index == kept)
                .collect();
            assert_eq!(
                carrying, expected,
                "a snapshot every {every}, kept from {kept}"
            );
        }
    }
}

/// Caught by: a snapshot that misses a line crossing into the first row kept.
#[test]
fn rows_kept_from_any_row_on_draw_without_the_rows_above_them() {
    let mut drew_a_line_from_above = false;
    for seed in 1..60u64 {
        let history = random_history(seed, 40, true);
        for window in [1usize, 3, 1024] {
            let before = before_compaction(&history, window);
            for kept in [1usize, 9, 33] {
                let rows = LaneAssigner::with_window(window)
                    .with_snapshot_every(16)
                    .drawn_from(kept)
                    .assign_each(walk(&history));
                let kept_rows = &rows[kept..];
                for (offset, old) in before[kept..].iter().enumerate() {
                    assert_eq!(
                        row_edges(kept_rows, offset).map(|row| row.edges),
                        Some(old.edges.clone()),
                        "seed {seed}, window {window}: row {} drawn from row {kept}",
                        kept + offset
                    );
                }
                drew_a_line_from_above |= before[kept]
                    .edges
                    .iter()
                    .any(|edge| edge.kind != EdgeKind::OutOfCommit);
            }
        }
    }
    assert!(
        drew_a_line_from_above,
        "no first kept row had a line crossing into it, so its snapshot decided nothing"
    );
}

/// Caught by: deriving a row from rows that carry no snapshot, which can only guess.
#[test]
fn a_row_with_no_snapshot_within_reach_draws_nothing_rather_than_guessing() {
    let history = random_history(3, 30, true);
    let rows = compacted(&history, 1024, 16);
    assert!(row_edges(&rows, rows.len()).is_none(), "a row past the end");
    assert!(
        row_edges(&rows[1..], 0).is_none(),
        "a slice that starts below its snapshot"
    );
    assert!(row_edges(&rows[1..], 15).is_some(), "row 16 carries one");
    assert!(
        row_edges(&rows[1..], 20).is_some(),
        "row 21 is below row 16's"
    );

    // The interval is held to the furthest a reader looks.
    let long = linear_history(LaneAssigner::MAX_SNAPSHOT_EVERY + 2);
    let rows = compacted(&long, 1024, usize::MAX);
    let carrying: Vec<usize> = (0..rows.len())
        .filter(|&index| rows[index].has_snapshot())
        .collect();
    assert_eq!(carrying, [0, LaneAssigner::MAX_SNAPSHOT_EVERY]);
    assert!(row_edges(&rows, LaneAssigner::MAX_SNAPSHOT_EVERY - 1).is_some());
}

fn linear_history(len: usize) -> History {
    (0..len)
        .map(|n| {
            let parents = if n + 1 < len {
                vec![format!("l{}", n + 1)]
            } else {
                Vec::new()
            };
            (format!("l{n}"), parents)
        })
        .collect()
}

/// Caught by: a lane only a line passing through names going uncounted on rows kept from
/// below where it opened.
#[test]
fn the_lanes_kept_rows_name_are_the_lanes_their_edges_reach() {
    // `missing` is never walked, so lane 1 runs past every row below `a`.
    let mut cases = vec![literal(&[
        ("a", &["b", "missing"]),
        ("b", &["c"]),
        ("c", &[]),
    ])];
    cases.extend((1..60u64).map(|seed| random_history(seed, 40, true)));
    let mut a_lane_only_passing_named = false;
    for (case, history) in cases.iter().enumerate() {
        let before = before_compaction(history, 4);
        for kept in [0usize, 1, 7, 20]
            .into_iter()
            .filter(|&kept| kept < history.len())
        {
            let after = LaneAssigner::with_window(4)
                .with_snapshot_every(8)
                .drawn_from(kept)
                .assign_each(walk(history));
            let reached = before[kept..]
                .iter()
                .flat_map(|row| {
                    std::iter::once(row.lane.index()).chain(
                        row.edges
                            .iter()
                            .flat_map(|edge| [edge.from.index(), edge.to.index()]),
                    )
                })
                .max()
                .map_or(0, |lane| lane + 1);
            let named = after[kept..]
                .iter()
                .map(GraphRow::lanes_named)
                .max()
                .unwrap_or(0);
            assert_eq!(named, reached, "case {case}, kept from {kept}");

            let by_changes = after[kept..]
                .iter()
                .flat_map(|row| {
                    std::iter::once(row.lane.index())
                        .chain(row.changes().iter().map(|change| change.lane().index()))
                })
                .max()
                .map_or(0, |lane| lane + 1);
            a_lane_only_passing_named |= by_changes < reached;
        }
    }
    assert!(
        a_lane_only_passing_named,
        "no kept rows had a lane only a passing line named, so the snapshot's part decided \
         nothing"
    );
}
