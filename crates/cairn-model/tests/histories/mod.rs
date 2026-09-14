//! Literal parent maps to lay out, and the checks every layout must satisfy.
//!
//! The assigner never sees a repository, so neither do its tests: a history
//! here is a list of `(commit, parents)` in the order a walk would hand them
//! over. Labels are turned into object ids by hex-encoding them, so a failure
//! message can be read back to the label that produced it.

use std::collections::{HashMap, HashSet};

use cairn_model::{EdgeKind, GraphRow, Lane, LaneAssigner, Oid};

/// `(commit label, parent labels)` in walk order — newest first, except where
/// a fixture is deliberately skewed.
pub type History = Vec<(String, Vec<String>)>;

pub fn literal(rows: &[(&str, &[&str])]) -> History {
    rows.iter()
        .map(|(id, parents)| {
            (
                (*id).to_owned(),
                parents.iter().map(|p| (*p).to_owned()).collect(),
            )
        })
        .collect()
}

/// A label as an object id: the label's bytes in hex, padded out to SHA-1
/// width. Reversible, so failures name the commit a human wrote down.
///
/// `unwrap` is unavailable here — `clippy.toml`'s carve-out only reaches
/// `#[cfg(test)]` code, and an integration test crate is not that — so the
/// failure path names what went wrong instead.
pub fn oid(label: &str) -> Oid {
    let mut hex: String = label.bytes().map(|b| format!("{b:02x}")).collect();
    assert!(hex.len() <= 40, "label {label:?} is too long to encode");
    while hex.len() < 40 {
        hex.push('0');
    }
    Oid::parse(&hex).unwrap_or_else(|e| panic!("label {label:?} encoded to a bad id: {e}"))
}

pub fn label_of(id: &Oid) -> String {
    let mut bytes = Vec::new();
    for pair in id.as_str().as_bytes().chunks(2) {
        let byte = pair.iter().fold(0u8, |value, digit| {
            let nibble = char::from(*digit).to_digit(16).unwrap_or(0) as u8;
            value.wrapping_mul(16).wrapping_add(nibble)
        });
        if byte == 0 {
            break; // The padding starts here: the label is over.
        }
        bytes.push(byte);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn assign(history: &History) -> Vec<GraphRow> {
    LaneAssigner::assign_all(
        history
            .iter()
            .map(|(id, parents)| (oid(id), parents.iter().map(|p| oid(p)).collect())),
    )
}

/// One readable line per row: the commit, its lane, and every segment crossing
/// it. `pass 2` is a line crossing lane 2, `in 1>0` runs from the top edge in
/// lane 1 to the node in lane 0, `out 0>1` from the node to the bottom edge,
/// and a trailing `~` marks a segment flagged out of order.
pub fn describe(rows: &[GraphRow]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            let segments: Vec<String> = row
                .edges
                .iter()
                .map(|edge| {
                    let (from, to) = (edge.from.index(), edge.to.index());
                    let body = match edge.kind {
                        EdgeKind::Passing => format!("pass {from}"),
                        EdgeKind::IntoCommit => format!("in {from}>{to}"),
                        EdgeKind::OutOfCommit => format!("out {from}>{to}"),
                    };
                    if edge.out_of_order {
                        format!("{body}~")
                    } else {
                        body
                    }
                })
                .collect();
            format!(
                "{} lane={} [{}]",
                label_of(&row.id),
                row.lane.index(),
                segments.join(", ")
            )
        })
        .collect()
}

fn distinct_parents(parents: &[String]) -> Vec<&String> {
    let mut seen = Vec::new();
    for parent in parents {
        if !seen.contains(&parent) {
            seen.push(parent);
        }
    }
    seen
}

/// The lane carrying a continuous line from the node on row `top` to the node
/// on row `bottom`, if one is drawn at all.
fn connecting_lane(rows: &[GraphRow], top: usize, bottom: usize) -> Option<Lane> {
    rows[top]
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::OutOfCommit && edge.from == rows[top].lane)
        .map(|edge| edge.to)
        .find(|&lane| {
            let crosses_every_row_between = rows[top + 1..bottom].iter().all(|row| {
                row.edges
                    .iter()
                    .any(|edge| edge.kind == EdgeKind::Passing && edge.from == lane)
            });
            let reaches_the_node = rows[bottom].edges.iter().any(|edge| {
                edge.kind == EdgeKind::IntoCommit
                    && edge.from == lane
                    && edge.to == rows[bottom].lane
            });
            crosses_every_row_between && reaches_the_node
        })
}

/// Every commit gets a row, every parent link is a line a renderer can
/// actually draw end to end, and no line is drawn that is not a parent link.
///
/// The second half is what stops a layout passing by drawing extra edges: the
/// count of segments leaving a node has to equal the number of parent links,
/// so a spurious line cannot hide behind a satisfied continuity check.
pub fn assert_every_parent_edge_is_drawn(history: &History, rows: &[GraphRow]) {
    assert_eq!(rows.len(), history.len(), "one row per commit");
    assert_the_picture_joins_up(rows);
    let row_of: HashMap<&str, usize> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.id.as_str(), index))
        .collect();
    assert_eq!(
        row_of.len(),
        rows.len(),
        "a commit is laid out at most once"
    );

    let mut links = 0usize;
    for (id, parents) in history {
        let child = row_of[oid(id).as_str()];
        for parent in distinct_parents(parents) {
            links += 1;
            let Some(&ancestor) = row_of.get(oid(parent).as_str()) else {
                continue; // A parent outside the walk: the line runs off the end.
            };
            assert_ne!(child, ancestor, "{id} cannot be its own parent");
            let (top, bottom) = (child.min(ancestor), child.max(ancestor));
            assert!(
                connecting_lane(rows, top, bottom).is_some(),
                "no line joins {id} to its parent {parent}\n{}",
                describe(rows).join("\n")
            );
        }
    }

    let drawn = rows
        .iter()
        .flat_map(|row| &row.edges)
        .filter(|edge| edge.kind == EdgeKind::OutOfCommit)
        .count();
    assert_eq!(
        drawn,
        links,
        "a line leaves a node for something that is not its parent\n{}",
        describe(rows).join("\n")
    );
}

/// The structural contract of a row: lane numbering is dense from zero, a
/// passing line stays in its lane, a line into the row ends at the row's node
/// and a line out of it starts there.
pub fn assert_rows_are_well_formed(rows: &[GraphRow]) {
    if rows.is_empty() {
        return;
    }
    let mut ever_used: HashSet<usize> = HashSet::new();
    for row in rows {
        ever_used.insert(row.lane.index());
        for edge in &row.edges {
            ever_used.insert(edge.from.index());
            ever_used.insert(edge.to.index());
            match edge.kind {
                EdgeKind::Passing => assert_eq!(
                    edge.from,
                    edge.to,
                    "a passing line changed lane on row {}",
                    label_of(&row.id)
                ),
                EdgeKind::IntoCommit => assert_eq!(
                    edge.to,
                    row.lane,
                    "a line into row {} misses its node",
                    label_of(&row.id)
                ),
                EdgeKind::OutOfCommit => assert_eq!(
                    edge.from,
                    row.lane,
                    "a line out of row {} does not start at its node",
                    label_of(&row.id)
                ),
            }
        }
    }
    let widest = ever_used.iter().copied().max().unwrap_or(0);
    for lane in 0..=widest {
        assert!(
            ever_used.contains(&lane),
            "lane {lane} is never used but lane {widest} is: the numbering has a hole"
        );
    }
}

/// The picture has to join up: what leaves the bottom of one row is exactly
/// what enters the top of the next, no lane carries two lines at once, and
/// nothing enters the top of the very first row.
///
/// This is the check that makes a fabricated segment of ANY kind visible. An
/// edge count alone only bounds the lines that leave a node, so a spurious
/// `Passing` or `IntoCommit` — a line drawn from nowhere — slips past it; a
/// line that appears without a matching line above it cannot.
pub fn assert_the_picture_joins_up(rows: &[GraphRow]) {
    let mut leaving_the_row_above: HashSet<usize> = HashSet::new();
    for row in rows {
        let mut entering: HashSet<usize> = HashSet::new();
        for edge in &row.edges {
            if edge.kind == EdgeKind::OutOfCommit {
                continue;
            }
            assert!(
                entering.insert(edge.from.index()),
                "two lines enter row {} in lane {}\n{}",
                label_of(&row.id),
                edge.from.index(),
                describe(rows).join("\n")
            );
        }
        assert_eq!(
            entering,
            leaving_the_row_above,
            "row {} is entered by lines that did not leave the row above it\n{}",
            label_of(&row.id),
            describe(rows).join("\n")
        );

        let mut passing_out: HashSet<usize> = HashSet::new();
        let mut from_the_node: HashSet<usize> = HashSet::new();
        for edge in &row.edges {
            let (claimed, lane) = match edge.kind {
                EdgeKind::IntoCommit => continue,
                EdgeKind::Passing => (&mut passing_out, edge.to.index()),
                EdgeKind::OutOfCommit => (&mut from_the_node, edge.to.index()),
            };
            assert!(
                claimed.insert(lane),
                "two lines of the same kind leave row {} in lane {}\n{}",
                label_of(&row.id),
                lane,
                describe(rows).join("\n")
            );
        }
        // A passing line and a line out of the node may share a lane: that is
        // a branch merging into the line already descending it.
        leaving_the_row_above = passing_out.union(&from_the_node).copied().collect();
    }
}

/// A deterministic generator, so a property test is reproducible without a
/// dependency. `cairn-model` depends on nothing, tests included.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        // xorshift64*, chosen because it is five lines and needs no crate.
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next() % bound as u64) as usize
        }
    }
}

/// A random history of `len` commits. Commit `i` is newer than commit `j` for
/// `i < j`, and parents are always older, so index order is a valid walk. When
/// `skew` is set the walk order is then scrambled by adjacent swaps, which is
/// exactly what committer-date sorting does to a rebased or imported history:
/// it hands some parents over before their children.
pub fn random_history(seed: u64, len: usize, skew: bool) -> History {
    let mut rng = Rng::new(seed);
    let mut commits: Vec<(String, Vec<String>)> = Vec::with_capacity(len);
    for index in 0..len {
        let older = len - index - 1;
        let parent_count = match rng.below(10) {
            0 => 0,
            1..=2 if older >= 2 => 2,
            3 if older >= 3 => 3,
            _ => 1,
        };
        let mut parents = Vec::new();
        for _ in 0..parent_count.min(older) {
            let parent = format!("c{}", index + 1 + rng.below(older));
            if !parents.contains(&parent) {
                parents.push(parent);
            }
        }
        commits.push((format!("c{index}"), parents));
    }
    if skew {
        for position in 0..commits.len().saturating_sub(1) {
            if rng.below(3) == 0 {
                commits.swap(position, position + 1);
            }
        }
    }
    commits
}

/// Every `(parent row, child row)` pair the walk delivers backwards: the
/// parent handed over first and the child only later. Derived from the walk
/// order alone, so a test can decide which rows are *entitled* to be repainted
/// without asking the assigner what it flagged.
pub fn links_delivered_backwards(history: &History) -> Vec<(usize, usize)> {
    let position: HashMap<&str, usize> = history
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (id.as_str(), index))
        .collect();
    let mut links = Vec::new();
    for (child, (_, parents)) in history.iter().enumerate() {
        for parent in parents {
            if let Some(&at) = position.get(parent.as_str())
                && at < child
            {
                links.push((at, child));
            }
        }
    }
    links
}

/// True when the walk hands a commit over before something that lists it as a
/// parent — the case the assigner has to be total over.
pub fn delivers_a_parent_before_its_child(history: &History) -> bool {
    let position: HashMap<&str, usize> = history
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (id.as_str(), index))
        .collect();
    history.iter().enumerate().any(|(child, (_, parents))| {
        parents
            .iter()
            .any(|parent| position.get(parent.as_str()).is_some_and(|&at| at < child))
    })
}
