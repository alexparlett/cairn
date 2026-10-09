//! Which commits of a Show Lost Commits walk no ref reaches (`docs/prd/staging-and-commit.md`
//! R11.1): those `git rev-list <reflog ids> --not --branches --remotes --tags HEAD` lists,
//! drawn dimmed. Decided as the walk goes, never by a walk of its own before the first page.
//!
//! A commit pulled off the walk is reached when it is a tip a ref names or the parent of a
//! commit pulled before it that a ref reaches; otherwise it is lost, so far. The walk
//! yields the newest committed first, so a commit's children come off it before the commit
//! itself — except where a child is dated older than its parent, as a clock set wrong
//! dates one. There the parent can come off first and be taken for lost; when its child
//! a ref reaches comes off later, the parent and every commit taken for lost only through
//! it are reached after all, and each that was already handed on as a row is named
//! ([`Reach::take_reached`]), so the page that carries it draws that row as any other.
//!
//! What is kept: the commits a ref reaches that are not yet pulled (the walk's own frontier,
//! no more), and every commit taken for lost. Once every reflog tip not known to be reached
//! has come off the walk and none is lost, no commit after it can be: the tracking stops
//! and frees what it held.

use std::collections::{HashMap, HashSet};

use cairn_model::Oid;

/// A commit taken for lost: its parents, which are lost through it, and its row once it is
/// handed on.
#[derive(Debug)]
struct Lost {
    parents: Vec<Oid>,
    row: Option<usize>,
}

#[derive(Debug, Default)]
pub(super) struct Reach {
    /// Commits a ref reaches that have not come off the walk yet.
    frontier: HashSet<Oid>,
    /// Commits that came off the walk with no ref known to reach them.
    lost: HashMap<Oid, Lost>,
    /// Reflog tips no ref is known to reach that have not come off the walk yet.
    pending: HashSet<Oid>,
    /// Rows handed on as lost that a ref reaches after all.
    reached: Vec<(usize, Oid)>,
    /// Every commit from here on is reached: nothing is tracked.
    settled: bool,
}

impl Reach {
    /// For a walk from `refs`, the tips refs name, and `reflogs`, the tips only reflogs do.
    pub(super) fn new(refs: &[Oid], reflogs: &[Oid]) -> Self {
        let frontier: HashSet<Oid> = refs.iter().copied().collect();
        let pending = reflogs
            .iter()
            .filter(|tip| !frontier.contains(tip))
            .copied()
            .collect();
        let mut reach = Self {
            frontier,
            pending,
            ..Self::default()
        };
        reach.settle();
        reach
    }

    /// `id` came off the walk, with `parents`.
    pub(super) fn pulled(&mut self, id: Oid, parents: &[Oid]) {
        if self.settled {
            return;
        }
        self.pending.remove(&id);
        if self.frontier.remove(&id) {
            for parent in parents {
                self.reach(*parent);
            }
        } else {
            self.lost.insert(
                id,
                Lost {
                    parents: parents.to_vec(),
                    row: None,
                },
            );
        }
        self.settle();
    }

    /// `id` is handed on as row `row`: whether it is lost now.
    pub(super) fn handed(&mut self, id: &Oid, row: usize) -> bool {
        match self.lost.get_mut(id) {
            Some(lost) => {
                lost.row = Some(row);
                true
            }
            None => false,
        }
    }

    /// Whether `id` came off the walk and no ref is known to reach it.
    pub(super) fn is_lost(&self, id: &Oid) -> bool {
        self.lost.contains_key(id)
    }

    /// The rows handed on as lost that a ref reaches after all, among the first `drawn` rows
    /// — those a page has carried already. Every other is forgotten: its row reads whether
    /// it is lost when it is carried.
    pub(super) fn take_reached(&mut self, drawn: usize) -> Vec<(usize, Oid)> {
        let mut reached = std::mem::take(&mut self.reached);
        reached.retain(|(row, _)| *row < drawn);
        reached
    }

    /// `id` is reached: so is every commit taken for lost through it.
    fn reach(&mut self, id: Oid) {
        let mut reaching = vec![id];
        while let Some(id) = reaching.pop() {
            self.pending.remove(&id);
            match self.lost.remove(&id) {
                Some(lost) => {
                    if let Some(row) = lost.row {
                        self.reached.push((row, id));
                    }
                    reaching.extend(lost.parents);
                }
                None => {
                    self.frontier.insert(id);
                }
            }
        }
    }

    /// Stops tracking once nothing more can be lost: no reflog tip left to come off the walk
    /// unreached, and none lost.
    fn settle(&mut self) {
        if self.pending.is_empty() && self.lost.is_empty() {
            self.settled = true;
            self.frontier = HashSet::new();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    /// Pulls `walk` — each commit with its parents — handing each on as the next row, and
    /// says which rows were lost as they were handed on.
    fn walk(reach: &mut Reach, walk: &[(u8, &[u8])]) -> Vec<bool> {
        let mut lost = Vec::new();
        for (row, (id, parents)) in walk.iter().enumerate() {
            let parents: Vec<Oid> = parents.iter().map(|n| oid(*n)).collect();
            reach.pulled(oid(*id), &parents);
            lost.push(reach.handed(&oid(*id), row));
        }
        lost
    }

    /// main: 1 <- 2 <- 3; an amend replaced 3 with 4 (parent 2), and a reset left 5 (parent
    /// 4) behind: the reflog tips are 3, 4 and 5, of which a ref reaches 4. Caught by: a lost
    /// commit's parent a ref reaches taken for lost, or a reflog tip a ref also names.
    #[test]
    fn a_commit_is_lost_exactly_when_only_lost_commits_and_reflogs_reach_it() {
        let mut reach = Reach::new(&[oid(4)], &[oid(3), oid(4), oid(5)]);
        // Newest first: 5, 4, 3, 2, 1.
        let lost = walk(
            &mut reach,
            &[(5, &[4]), (4, &[2]), (3, &[2]), (2, &[1]), (1, &[])],
        );
        assert_eq!(lost, [true, false, true, false, false]);
        assert!(reach.take_reached(5).is_empty());
        assert!(!reach.settled, "two lost commits are still lost");
    }

    /// Clock skew: 9 (reached, from a ref) names 8 as its parent but is dated older, so 8
    /// comes off first through lost 7, then 6 through 8: both are taken for lost and handed
    /// on, then 9 comes off and both are reached after all — each named with its row, and
    /// only once the rows have been carried. Caught by: a reached row never corrected, a
    /// correction lost for a row a later page carries, or one naming a row not carried yet.
    #[test]
    fn a_parent_taken_for_lost_before_its_reached_child_is_reached_after_all() {
        let mut reach = Reach::new(&[oid(9)], &[oid(7)]);
        let lost = walk(&mut reach, &[(7, &[8]), (8, &[6]), (6, &[]), (9, &[8])]);
        assert_eq!(lost, [true, true, true, false]);
        assert!(!reach.is_lost(&oid(8)) && !reach.is_lost(&oid(6)));
        assert!(reach.is_lost(&oid(7)), "7 itself is still lost");
        let mut reached = reach.take_reached(2);
        reached.sort();
        assert_eq!(reached, [(1, oid(8))], "row 2 was not carried yet");
        assert!(reach.take_reached(4).is_empty(), "each said once");
    }

    /// Every reflog tip a ref reaches, and nothing lost: the tracking stops, and from then on
    /// nothing is lost or kept. Caught by: the frontier kept for the whole walk, or a commit
    /// after the last reflog tip taken for lost.
    #[test]
    fn once_nothing_can_be_lost_nothing_is_tracked() {
        let mut reach = Reach::new(&[oid(3)], &[oid(2)]);
        assert!(!reach.settled);
        let lost = walk(&mut reach, &[(3, &[2]), (2, &[1])]);
        assert_eq!(lost, [false, false]);
        assert!(reach.settled && reach.frontier.is_empty());
        assert!(walk(&mut reach, &[(1, &[])]).iter().all(|lost| !lost));
        let none = Reach::new(&[oid(1)], &[oid(1)]);
        assert!(none.settled, "a reflog tip a ref names is no candidate");
    }
}
