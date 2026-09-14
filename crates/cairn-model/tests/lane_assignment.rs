//! Acceptance tests for the lane assigner (`docs/prd/history-graph.md`, R1).
//!
//! A1 is the fixture table below: linear history, a branch and a merge, an
//! octopus merge, criss-cross merges and multiple roots, each pinned to its
//! exact lanes *and* edges. A lane-count-only assertion would pass on a layout
//! that drew the wrong lines, so every expectation is a full row description.
//! A2 is `skewed_history_...`. A3 is `lane_indices_never_change_...`.
//!
//! Each fixture test says, in its doc comment, which change to the assigner it
//! would catch, and every one of those was checked by making that change and
//! watching the test fail. A test that cannot name one is decoration.

mod histories;

use cairn_model::LaneAssigner;
use histories::{
    History, assert_every_parent_edge_is_drawn, assert_rows_are_well_formed, assign,
    delivers_a_parent_before_its_child, describe, literal, random_history,
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

/// `b` is reserved in lane 1 while lane 0 has already fallen empty. A branch
/// must not hop left into the gap.
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

/// The walk hands `p` over at row 0 and its child `c` only at row 2 — a parent
/// before its child, which is what committer-date sorting does to rebased,
/// cherry-picked or imported history (evidence record
/// `docs/research/history-graph/gix-revwalk-ordering.md`, finding 2). `t` sits
/// between them so the connecting line has a row to be repainted onto.
fn skewed() -> History {
    literal(&[
        ("p", &["base"]),
        ("t", &["base"]),
        ("c", &["p"]),
        ("base", &[]),
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
    ]
}

// --- A1: lanes and edges for the fixture set -------------------------------

/// Caught by: not freeing a commit's lane when the commit arrives. The first
/// parent would never be able to continue in it, and linear history would walk
/// right, one lane per commit. (Verified by mutation: the change fails this
/// test.)
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

/// Caught by: dropping the deduplication of parent reservations (verified by
/// mutation). `b`'s parent
/// `base` already has lane 0 reserved by `a`; without the check `b` would open
/// a second lane for the same commit and `base` would be drawn with two
/// incoming lines instead of one converging merge.
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

/// Caught by: handling only the first two parents of a merge, or dropping the
/// deduplication of parent reservations (verified by mutation). The third
/// segment `out 0>2` would vanish and `p3` would never get lane 2 — a defect a
/// lane-count assertion on `o` alone would miss, because `o` still occupies
/// exactly one lane.
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

/// Caught by: letting a commit with no reserved lane take a lane that is still
/// open. `m2` arrives while lanes 0 and 1 are carrying `a` and `b`; if it took
/// lane 0 it would be drawn on top of the line descending to `a`, and the
/// lane count for the fixture would still be right. (Verified by mutation.)
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

/// Caught by: rejecting or skipping a commit that arrives with no lane
/// reserved for it. `b2` is the second root's tip and nothing reserved a lane
/// for it, so it would lose its row entirely. (Verified by mutation.)
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

/// Caught by: choosing the lowest free lane for a commit's first parent
/// instead of the commit's own lane. `b` sits in lane 1 with lane 0 free, so
/// its parent `c` would jump left to lane 0 and the branch would appear to
/// change track for no reason. (Verified by mutation: the change fails this
/// test and no other.)
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

/// A2. The walk delivers `p` before its child `c`; every commit still gets a
/// row and every parent edge is drawn, end to end.
///
/// Caught by: reserving a lane for an already-laid-out parent instead of
/// connecting upward (the `c`→`p` line would descend into a lane that never
/// fills, and `p`'s row would keep no link to `c`); or connecting upward
/// without repainting the rows in between, which would drop `t`'s `pass 2~`
/// and leave the line broken across the row it has to cross.
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

/// The same totality over generated histories: whatever order the walk uses,
/// every commit is placed and every parent edge is drawable.
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

/// A3. Laying out the first N commits and laying out N+M must agree on the
/// first N rows' lane indices exactly. An edge list may only grow, and only by
/// segments belonging to a line that joins a commit to a parent already on
/// screen — the one thing R1.2 permits.
///
/// Caught by: renumbering or compacting lanes as commits arrive (any lane
/// index would move), or drawing the late-joining line by rewriting existing
/// segments rather than adding new ones (the prefix check would fail).
#[test]
fn lane_indices_never_change_when_more_commits_are_assigned() {
    let mut cases = corpus();
    for seed in 1..40u64 {
        cases.push(("generated", random_history(seed, 20, true)));
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
                for gained in &after.edges[before.edges.len()..] {
                    saw_a_gained_segment = true;
                    assert!(
                        gained.out_of_order,
                        "{name}: row {index} gained {gained:?}, which is not a segment for a \
                         parent that arrived late"
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

/// The assigner is a function of its input alone: same commits in, same rows
/// out, every time.
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

/// Feeding commits one at a time must match feeding them all at once, because
/// `cairn-git` will stream them.
#[test]
fn pushing_one_at_a_time_matches_assigning_the_whole_walk() {
    for (name, history) in corpus() {
        let mut assigner = LaneAssigner::new();
        for (id, parents) in &history {
            assigner.push(
                histories::oid(id),
                parents.iter().map(|p| histories::oid(p)).collect(),
            );
        }
        assert_eq!(
            describe(assigner.rows()),
            describe(&assign(&history)),
            "{name}"
        );
    }
}

/// Input a repository can really produce and a naive assigner would trip over:
/// an empty walk, a commit repeated, the same parent named twice, and a parent
/// that is not in the walk at all. None of these may panic.
#[test]
fn malformed_input_is_placed_rather_than_rejected() {
    assert!(assign(&literal(&[])).is_empty());

    let repeated = literal(&[("a", &["b"]), ("b", &[]), ("b", &[])]);
    assert_eq!(assign(&repeated).len(), 3);

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
