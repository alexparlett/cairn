//! The walk with each stash's row in it: what both history routes lay out, entry by entry.
//!
//! In git's date order — the history's — a stash is a tip of the walk itself, whose one
//! parent is its base (`date_order`): the walk seeds it only when the refs reach its base,
//! and yields its row where `git log --date-order` puts it, so the stream hands that row on
//! as it comes. A stash whose base no ref reaches has no row, and nothing of it is walked.
//!
//! The other orders walk the refs alone, and carry no dates the stream could place a stash
//! by: there each stash whose base the walk reaches goes directly above that base, newest
//! committed first, and a stash commit the walk reaches itself, through some ref, is that
//! commit's row, never a stash's.
//!
//! With Show Lost Commits, which always walks in date order, each commit's row says whether
//! no ref reaches it, decided as it comes off the walk (`reach`).

use std::collections::HashMap;
use std::sync::Arc;

use cairn_model::Oid;

use super::reach::Reach;
use super::seeds::Decoration;
use super::walk::{CommitWalk, Grafted, Step};
use crate::{Cancel, Error};

/// One entry of the stream, in the order the lane assigner lays them out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Entry {
    Commit {
        id: Oid,
        parents: Vec<Oid>,
        /// No ref reaches it: only a reflog does (Show Lost Commits).
        lost: bool,
    },
    /// The stash at this index of the decoration's stashes.
    Stash(usize),
}

/// What [`Stream::next`] found.
#[derive(Debug)]
pub(super) enum Next {
    Entry(Entry),
    End,
    /// `cancel` fired within the walk's step; nothing was lost, and the next call carries on.
    Cancelled,
}

pub(super) struct Stream<'repo> {
    walk: CommitWalk<'repo>,
    /// The repository's objects under the substitutions the walk was opened with: what each
    /// row's commit is read through.
    objects: Grafted<'repo>,
    /// Whether the walk yields the stashes' rows itself (date order).
    stashes_walked: bool,
    /// Stashes by base, each list newest committed first: what goes directly above it, in an
    /// order whose walk yields no stash.
    by_base: HashMap<Oid, Vec<usize>>,
    /// Stashes by their own commit, to drop one the walk reaches as a commit.
    by_commit: HashMap<Oid, usize>,
    /// One per stash of the decoration: its row is laid out, or it has none.
    placed: Vec<bool>,
    /// A commit pulled off the walk and held back while the stashes on it go first.
    held: Option<Entry>,
    /// Commits pulled off the walk.
    pulled: usize,
    /// Which commits no ref reaches, when the walk has tips only reflogs name (Show Lost
    /// Commits); `None` when every commit it walks is reached.
    reach: Option<Reach>,
    decoration: Arc<Decoration>,
}

impl std::fmt::Debug for Stream<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("stashes_walked", &self.stashes_walked)
            .field("pulled", &self.pulled)
            .field("placed", &self.placed)
            .finish_non_exhaustive()
    }
}

impl<'repo> Stream<'repo> {
    /// `stashes_walked` when `walk` yields the stashes' rows itself; `reach` decides which
    /// commits no ref reaches, for a walk with tips only reflogs name.
    pub(super) fn new(
        walk: CommitWalk<'repo>,
        objects: Grafted<'repo>,
        decoration: Arc<Decoration>,
        stashes_walked: bool,
        reach: Option<Reach>,
    ) -> Self {
        let stashes = decoration.stashes();
        let mut by_base: HashMap<Oid, Vec<usize>> = HashMap::new();
        let mut by_commit = HashMap::new();
        if !stashes_walked {
            let mut dated: Vec<usize> = (0..stashes.len()).collect();
            // Newest committed first; among equal dates, `stash@{0}` first.
            dated.sort_by_key(|&at| {
                stashes
                    .get(at)
                    .map(|stash| (std::cmp::Reverse(stash.committed), stash.index))
            });
            for at in dated {
                if let Some(stash) = stashes.get(at) {
                    by_base.entry(stash.base).or_default().push(at);
                    by_commit.insert(stash.id, at);
                }
            }
        }
        Self {
            walk,
            objects,
            stashes_walked,
            by_base,
            by_commit,
            placed: vec![false; stashes.len()],
            held: None,
            pulled: 0,
            reach,
            decoration,
        }
    }

    /// The repository's objects as the walk reads them.
    pub(super) fn objects(&self) -> &Grafted<'repo> {
        &self.objects
    }

    /// Commits pulled off the walk so far.
    pub(super) fn pulled(&self) -> usize {
        self.pulled
    }

    /// The next entry: each is the next row. The caller polls `cancel` before asking for
    /// one, and the walk polls it within a step that reads commits to know the next one
    /// (date order).
    pub(super) fn next(&mut self, cancel: &impl Cancel) -> Result<Next, Error> {
        if let Some(held) = self.held.take() {
            return Ok(Next::Entry(self.above(held)));
        }
        let info = match self.walk.next(cancel)? {
            Step::Commit(info) => info,
            Step::Stash(at) => {
                if let Some(place) = self.placed.get_mut(at) {
                    *place = true;
                }
                if let (Some(reach), Some(stash)) =
                    (self.reach.as_mut(), self.decoration.stashes().get(at))
                {
                    reach.passed(&stash.id);
                }
                return Ok(Next::Entry(Entry::Stash(at)));
            }
            Step::End => return Ok(Next::End),
            Step::Cancelled => return Ok(Next::Cancelled),
        };
        let id = crate::object_id::model_id(&info.id)?;
        let parents = super::parents_of(&info)?;
        self.pulled += 1;
        let lost = self
            .reach
            .as_mut()
            .is_some_and(|reach| reach.pulled(id, &parents));
        if let Some(&stash) = self.by_commit.get(&id) {
            // The walk reaches the stash commit itself: that is a commit's row.
            if let Some(place) = self.placed.get_mut(stash) {
                *place = true;
            }
        }
        Ok(Next::Entry(self.above(Entry::Commit { id, parents, lost })))
    }

    /// `commit`, or — in an order whose walk yields no stash — the newest stash on it not yet
    /// placed, `commit` held back for the next call.
    fn above(&mut self, commit: Entry) -> Entry {
        let Entry::Commit { id, .. } = &commit else {
            return commit;
        };
        let next = self.by_base.get(id).and_then(|stashes| {
            stashes
                .iter()
                .copied()
                .find(|&at| self.placed.get(at) == Some(&false))
        });
        match next {
            Some(stash) => {
                if let Some(place) = self.placed.get_mut(stash) {
                    *place = true;
                }
                self.held = Some(commit);
                Entry::Stash(stash)
            }
            None => commit,
        }
    }
}
