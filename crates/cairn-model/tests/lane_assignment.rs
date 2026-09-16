//! Acceptance tests for the lane assigner (`docs/prd/history-graph.md`, R1).
//!
//! A1 is the fixture table below, pinned to exact lanes *and* edges: a
//! lane-count assertion passes on a layout that drew the wrong lines. A2 is
//! `skewed_history_...`, A3 `lane_indices_never_change_...`. Every test names
//! the change to the assigner it catches.

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

/// `p` at row 0, its child `c` at row 2 — what committer-date sorting does to
/// rebased history (`docs/research/history-graph/gix-revwalk-ordering.md`,
/// finding 2). `t` between them gives the line a row to repaint.
fn skewed() -> History {
    literal(&[
        ("p", &["base"]),
        ("t", &["base"]),
        ("c", &["p"]),
        ("base", &[]),
    ])
}

/// The same reversal four rows apart, lanes 0, 1 and 2 busy throughout, so the
/// connecting line's lane is forced. `skewed()` decides neither: a partial
/// repaint looks like a full one there, and its free lane is also the widest.
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

// --- A1: lanes and edges for the fixture set -------------------------------

/// Caught by: not freeing a commit's lane on arrival — linear history would
/// walk right, one lane per commit.
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

/// Caught by: dropping the deduplication of parent reservations — `base` gets
/// two incoming lines rather than one converging merge.
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

/// Caught by: handling only a merge's first two parents, or dropping the
/// deduplication of parent reservations. A lane-count assertion on `o` misses
/// both: `o` still occupies exactly one lane.
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

/// Caught by: letting a commit with no reservation take a lane still in use —
/// `m2` would draw over the line descending to `a`, lane count still right.
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

/// Caught by: skipping a commit that arrives with no lane reserved — `b2`, the
/// second root's tip, would lose its row.
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

/// Caught by: giving a commit's first parent the lowest free lane rather than
/// the commit's own — `c` jumps left out of lane 1. The only test that fails on
/// that change.
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

// --- A2: the skew regression ------------------------------------------------

/// Caught by: reserving a lane for an already-laid-out parent instead of
/// connecting upward (A2 — the `c`→`p` line descends into a lane that never
/// fills); or connecting upward without repainting the rows between, which
/// drops `t`'s `pass 2~`.
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

/// Caught by: running the connecting line down a lane already in use, or
/// repainting only part of the span — neither of which `skewed()`'s single
/// intermediate row can see.
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

/// The same totality over generated histories, in any walk order.
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

// --- A3: stability ----------------------------------------------------------

/// A3: N commits and N+M agree on the first N lane indices exactly, and an edge
/// list may only grow, by segments of a line to a parent already on screen —
/// the one thing R1.2 permits. Caught by: renumbering lanes as commits arrive,
/// or drawing the late-joining line by rewriting existing segments.
#[test]
fn lane_indices_never_change_when_more_commits_are_assigned() {
    let mut cases = corpus();
    for seed in 1..40u64 {
        cases.push(("generated", random_history(seed, 20, true)));
        // Control: with nothing delivered backwards, no row may be repainted
        // however wide the graph. These reach lane 8.
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
                    // Derived from the walk, not from the assigner's own flag:
                    // a row may be repainted only on the span of a line to an
                    // early-delivered parent whose child the longer run added.
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

// --- R1.5 and arrival-order defences ---------------------------------------

/// A function of its input alone: same commits in, same rows out.
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

/// Streaming one at a time matches feeding them all at once. Rows arrive two
/// ways — handed back as the window makes them final, and left inside it at the
/// end — and together they must be the whole walk, in order.
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

/// Input a repository can really produce: an empty walk, a repeated commit, a
/// parent named twice, a parent not in the walk. None may panic.
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
