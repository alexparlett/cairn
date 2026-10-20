//! Which rows of a walk from refs `HEAD` reaches: those `git rev-list HEAD` lists. The rest are
//! drawn greyed, as Fork draws a commit `HEAD` cannot reach. Decided as the walk goes, never by
//! a walk of its own.
//!
//! The walk is git's date order, which yields no commit before every child it walks
//! (`date_order`). So when a commit comes off it, every child that could make `HEAD` reach it
//! has come off already: `HEAD` reaches it when it is `HEAD`'s commit or a parent of a commit
//! `HEAD` reaches that came off before it — a verdict final the moment it is given, whatever
//! the commits' dates say. The parents are the ones the walk read, so a graft, a replace ref
//! and a shallow boundary count as `git rev-list` counts them.
//!
//! What is kept: the commits `HEAD` reaches that have not come off yet — the frontier of
//! `HEAD`'s own history, no more — so a row costs a lookup and an insert per parent, whatever
//! the history's length. A stash's row is never reached: `git rev-list HEAD` lists no stash
//! commit, and the walk yields a stash commit something reaches as that commit's own row.

use std::collections::HashSet;

use cairn_model::Oid;

#[derive(Debug, Default)]
pub(super) struct HeadReach {
    /// Commits `HEAD` reaches that have not come off the walk yet.
    frontier: HashSet<Oid>,
}

impl HeadReach {
    /// For a walk whose `HEAD` names `head`; `None` — an unborn `HEAD`, or a detached one naming
    /// what is not a commit — reaches nothing.
    pub(super) fn new(head: Option<Oid>) -> Self {
        Self {
            frontier: head.into_iter().collect(),
        }
    }

    /// `id` came off the walk, with `parents`, after every child of it the walk yields: whether
    /// `HEAD` reaches it.
    pub(super) fn pulled(&mut self, id: &Oid, parents: &[Oid]) -> bool {
        let reached = self.frontier.remove(id);
        if reached {
            self.frontier.extend(parents.iter().copied());
        }
        reached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap_or_else(|error| panic!("{error}"))
    }

    /// Pulls `walk` — each commit with its parents, children first — and says which `HEAD`
    /// reached as each came off.
    fn walk(reach: &mut HeadReach, walk: &[(u8, &[u8])]) -> Vec<bool> {
        walk.iter()
            .map(|(id, parents)| {
                let parents: Vec<Oid> = parents.iter().map(|n| oid(*n)).collect();
                reach.pulled(&oid(*id), &parents)
            })
            .collect()
    }

    /// `HEAD` on 5, a merge of 3 and 4; 6 a side branch's tip on 2; 3, 4 and 2 down to the
    /// root, 1. Caught by: a parent taken for reached through an unreached child (6 marking 2
    /// alone would do), a merge's second parent left unreached, or the frontier never let go
    /// of.
    #[test]
    fn head_reaches_its_commit_and_every_parent_of_what_it_reaches() {
        let mut reach = HeadReach::new(Some(oid(5)));
        let reached = walk(
            &mut reach,
            &[
                (6, &[2]),
                (5, &[3, 4]),
                (4, &[2]),
                (3, &[2]),
                (2, &[1]),
                (1, &[]),
            ],
        );
        assert_eq!(reached, [false, true, true, true, true, true]);
        assert!(reach.frontier.is_empty(), "the root let go of the frontier");
    }

    /// A commit only an unreached commit names stays unreached, however many unreached children
    /// it has. Caught by: marking each parent of every row pulled.
    #[test]
    fn a_commit_only_unreached_commits_name_is_unreached() {
        let mut reach = HeadReach::new(Some(oid(1)));
        assert_eq!(
            walk(&mut reach, &[(9, &[8]), (7, &[8]), (8, &[1]), (1, &[])]),
            [false, false, false, true]
        );
    }

    /// An unborn `HEAD` reaches nothing.
    #[test]
    fn an_unborn_head_reaches_nothing() {
        let mut reach = HeadReach::new(None);
        assert_eq!(walk(&mut reach, &[(2, &[1]), (1, &[])]), [false, false]);
    }
}
