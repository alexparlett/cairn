//! Which commits of a Show Lost Commits walk no ref reaches: those
//! `git rev-list <reflog ids> --not --branches --remotes --tags HEAD` lists, drawn dimmed.
//! Decided as the walk goes, never by a walk of its own before the first page.
//!
//! The walk is git's date order, which yields no commit before every child it walks
//! (`date_order`). So when a commit comes off it, every child that could make it reached has
//! come off already: it is reached when it is a tip a ref names or the parent of a commit
//! that came off reached, and lost otherwise — a verdict that is final the moment it is
//! given, whatever the commits' dates say.
//!
//! What is kept: the commits a ref reaches that have not come off yet (the walk's own
//! frontier, no more), and likewise the parents of lost commits. Once every reflog tip not
//! known to be reached has come off the walk and no lost commit's parent is still to come,
//! no commit after it can be lost: the tracking stops and frees what it held.

use std::collections::HashSet;

use cairn_model::Oid;

#[derive(Debug, Default)]
pub(super) struct Reach {
    /// Commits a ref reaches that have not come off the walk yet.
    frontier: HashSet<Oid>,
    /// Parents of lost commits that have not come off the walk yet and no ref is known to
    /// reach.
    below_lost: HashSet<Oid>,
    /// Reflog tips no ref is known to reach that have not come off the walk yet.
    pending: HashSet<Oid>,
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

    /// `id` came off the walk, with `parents`, after every child of it the walk yields:
    /// whether no ref reaches it.
    pub(super) fn pulled(&mut self, id: Oid, parents: &[Oid]) -> bool {
        if self.settled {
            return false;
        }
        self.pending.remove(&id);
        self.below_lost.remove(&id);
        let lost = !self.frontier.remove(&id);
        for parent in parents {
            if lost {
                if !self.frontier.contains(parent) {
                    self.below_lost.insert(*parent);
                }
            } else {
                self.below_lost.remove(parent);
                self.frontier.insert(*parent);
            }
        }
        self.settle();
        lost
    }

    /// `id` came off the walk as a stash's row, which is neither reached nor lost: a reflog
    /// tip naming the stash's commit is no longer to come.
    pub(super) fn passed(&mut self, id: &Oid) {
        if !self.settled {
            self.pending.remove(id);
            self.settle();
        }
    }

    /// Stops tracking once nothing more can be lost: no reflog tip left to come off the walk
    /// unreached, and no lost commit's parent still to come.
    fn settle(&mut self) {
        if self.pending.is_empty() && self.below_lost.is_empty() {
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

    /// Pulls `walk` — each commit with its parents, children first — and says which were
    /// lost as each came off.
    fn walk(reach: &mut Reach, walk: &[(u8, &[u8])]) -> Vec<bool> {
        walk.iter()
            .map(|(id, parents)| {
                let parents: Vec<Oid> = parents.iter().map(|n| oid(*n)).collect();
                reach.pulled(oid(*id), &parents)
            })
            .collect()
    }

    /// main: 1 <- 2 <- 3; an amend replaced 3 with 4 (parent 2), and a reset left 5 (parent
    /// 4) behind: the reflog tips are 3, 4 and 5, of which a ref reaches 4. Caught by: a lost
    /// commit's parent a ref reaches taken for lost, or a reflog tip a ref also names.
    #[test]
    fn a_commit_is_lost_exactly_when_only_lost_commits_and_reflogs_reach_it() {
        let mut reach = Reach::new(&[oid(4)], &[oid(3), oid(4), oid(5)]);
        let lost = walk(
            &mut reach,
            &[(5, &[4]), (4, &[2]), (3, &[2]), (2, &[1]), (1, &[])],
        );
        assert_eq!(lost, [true, false, true, false, false]);
        assert!(reach.settled, "nothing is left that could be lost");
    }

    /// A lost commit's parents are lost unless a ref reaches them too, and the tracking waits
    /// for each: 7 (a reflog tip) names 8, which names 6 and 4; 9 (a ref) names 6 alone, and
    /// each commit comes off after its children. Caught by: a lost commit's parent taken for
    /// reached, a parent a ref reaches taken for lost through its lost child, or the tracking
    /// stopped while a lost commit's parent was still to come.
    #[test]
    fn a_lost_commits_parents_are_lost_unless_a_ref_reaches_them() {
        let mut reach = Reach::new(&[oid(9)], &[oid(7)]);
        assert_eq!(walk(&mut reach, &[(7, &[8]), (8, &[6, 4])]), [true, true]);
        assert!(!reach.settled, "6 and 4 are still to come, lost through 8");
        assert_eq!(walk(&mut reach, &[(9, &[6]), (6, &[5])]), [false, false]);
        assert!(!reach.settled, "4 is still to come, lost through 8 alone");
        assert_eq!(walk(&mut reach, &[(4, &[])]), [true]);
        assert!(reach.settled, "nothing lost is left to come");
        assert_eq!(walk(&mut reach, &[(5, &[])]), [false]);
    }

    /// Every reflog tip a ref reaches, and nothing lost: the tracking stops, and from then on
    /// nothing is lost or kept. A stash's row clears a reflog tip naming its commit. Caught by:
    /// the frontier kept for the whole walk, a commit after the last reflog tip taken for
    /// lost, or a stash's commit left pending for the whole walk.
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
        let mut stash = Reach::new(&[oid(1)], &[oid(8)]);
        stash.passed(&oid(8));
        assert!(stash.settled, "the stash's row was the reflog tip's commit");
    }
}
