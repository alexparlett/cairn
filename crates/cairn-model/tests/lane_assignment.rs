//! Lane assigner tests.

mod histories;

use cairn_model::LaneAssigner;
use histories::{
    History, assert_every_parent_edge_is_drawn, assert_rows_are_well_formed, assign,
    delivers_a_parent_before_its_child, describe, links_delivered_backwards, literal,
    random_history,
};

fn linear() -> History {
    literal(&[("c3", &["c2"]), ("c2", &["c1"]), ("c1", &[])])
}

fn branch_and_merge() -> History {
    literal(&[
        ("m", &["a", "b"]),
        ("a", &["base"]),
        ("b", &["base"]),
        ("base", &[]),
    ])
}

fn octopus_merge() -> History {
    literal(&[
        ("o", &["p1", "p2", "p3"]),
        ("p1", &["base"]),
        ("p2", &["base"]),
        ("p3", &["base"]),
        ("base", &[]),
    ])
}

fn criss_cross() -> History {
    literal(&[
        ("m1", &["a", "b"]),
        ("m2", &["a", "b"]),
        ("a", &["base"]),
        ("b", &["base"]),
        ("base", &[]),
    ])
}

/// `b` sits in lane 1 with lane 0 already empty: a branch must not hop left.
fn lane_outlives_the_one_to_its_left() -> History {
    literal(&[
        ("t1", &["a"]),
        ("t2", &["b"]),
        ("a", &[]),
        ("b", &["c"]),
        ("c", &[]),
    ])
}

fn multiple_roots() -> History {
    literal(&[("a2", &["a1"]), ("b2", &["b1"]), ("a1", &[]), ("b1", &[])])
}

/// `p` at row 0, its child `c` at row 2, `t` between them.
fn skewed() -> History {
    literal(&[
        ("p", &["base"]),
        ("t", &["base"]),
        ("c", &["p"]),
        ("base", &[]),
    ])
}

/// The same reversal four rows apart, with lanes 0, 1 and 2 busy throughout.
fn skewed_across_a_busy_span() -> History {
    literal(&[
        ("t0", &["a"]),
        ("t1", &["b"]),
        ("p", &["e"]),
        ("a", &["f"]),
        ("b", &["f"]),
        ("c", &["p"]),
        ("e", &[]),
        ("f", &[]),
    ])
}

fn corpus() -> Vec<(&'static str, History)> {
    vec![
        ("linear", linear()),
        ("branch and merge", branch_and_merge()),
        ("octopus merge", octopus_merge()),
        ("criss-cross", criss_cross()),
        ("multiple roots", multiple_roots()),
        (
            "lane outlives its left neighbour",
            lane_outlives_the_one_to_its_left(),
        ),
        ("skewed", skewed()),
        ("skewed across a busy span", skewed_across_a_busy_span()),
    ]
}

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
        assert_eq!(describe(&streamed), describe(&assign(&history)), "{name}");
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
