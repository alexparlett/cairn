//! Literal parent maps to lay out, and the checks every layout must satisfy.
//!
//! Labels are hex-encoded into object ids, so a failure message reads back to
//! the label that produced it.

use std::collections::{HashMap, HashSet};

use cairn_model::{EdgeKind, GraphRow, Lane, LaneAssigner, Oid};

/// `(commit label, parent labels)` in walk order, newest first unless a fixture
/// is deliberately skewed.
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

/// A label as an object id: its bytes in hex, padded to SHA-1 width, and
/// reversible. `unwrap` is denied here — `clippy.toml`'s carve-out reaches
/// `#[cfg(test)]` code only, which an integration test crate is not.
pub fn oid(label: &str) -> Oid {
    let mut hex: String = label.bytes().map(|b| format!("{b:02x}")).collect();
    assert!(hex.len() <= 40, "label {label:?} is too long to encode");
    while hex.len() < 40 {
        hex.push('0');
    }
    Oid::parse(&hex).unwrap_or_else(|e| panic!("label {label:?} encoded to a bad id: {e}"))
}

pub fn label_of(id: &Oid) -> String {
    // Zero padding is where the label ends.
    let label: Vec<u8> = id
        .as_bytes()
        .iter()
        .copied()
        .take_while(|&byte| byte != 0)
        .collect();
    String::from_utf8_lossy(&label).into_owned()
}

pub fn assign(history: &History) -> Vec<GraphRow> {
    LaneAssigner::assign_all(
        history
            .iter()
            .map(|(id, parents)| (oid(id), parents.iter().map(|p| oid(p)).collect())),
    )
}

/// One line per row. `pass 2` crosses lane 2, `in 1>0` runs from the top edge
/// in lane 1 to the node in lane 0, `out 0>1` from the node to the bottom edge,
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

/// The lane carrying a continuous line from row `top`'s node to row
/// `bottom`'s, if one is drawn.
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

/// Every commit gets a row, every parent link is drawable end to end, and no
/// line is drawn that is not a parent link. The last clause is what stops a
/// spurious line hiding behind a satisfied continuity check.
pub fn assert_every_parent_edge_is_drawn(history: &History, rows: &[GraphRow]) {
    assert_eq!(rows.len(), history.len(), "one row per commit");
    assert_the_picture_joins_up(rows);
    let row_of: HashMap<Oid, usize> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.id, index))
        .collect();
    assert_eq!(
        row_of.len(),
        rows.len(),
        "a commit is laid out at most once"
    );

    let mut links = 0usize;
    for (id, parents) in history {
        let child = row_of[&oid(id)];
        for parent in distinct_parents(parents) {
            links += 1;
            let Some(&ancestor) = row_of.get(&oid(parent)) else {
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

/// Lane numbering is dense from zero, a passing line stays in its lane, a line
/// into the row ends at its node and a line out of it starts there.
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

/// What leaves the bottom of one row is exactly what enters the top of the
/// next, no lane carries two lines at once, and nothing enters the first row.
/// This is what makes a fabricated segment of any kind visible: an edge count
/// bounds only the lines leaving a node, so a spurious `Passing` slips past.
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
        // A passing line and a line out of the node share a lane when a branch
        // merges into the line already descending it.
        leaving_the_row_above = passing_out.union(&from_the_node).copied().collect();
    }
}

/// A deterministic generator: `cairn-model` takes no dependency, tests
/// included.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        // xorshift64*.
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

/// A random history of `len` commits. Commit `i` is newer than `j` for `i < j`
/// and parents are always older, so index order is a valid walk. `skew` then
/// swaps adjacent entries, so some parents arrive before their children.
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

/// Every `(parent row, child row)` pair the walk delivers backwards, derived
/// from walk order alone: which rows are *entitled* to a repaint, without
/// asking the assigner what it flagged.
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

/// True when the walk hands a commit over before one of its children.
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
