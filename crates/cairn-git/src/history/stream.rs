//! The walk with each stash's row merged into it (PRD R4.2): what both history routes lay
//! out, entry by entry.
//!
//! A stash's row goes where its commit's date falls among the walk's commits — newest
//! committed first, as the walk orders them — but never after the commit it was made on,
//! its base, and only when the walk reaches that base (Fork's rule): a stash whose base no
//! ref reaches has no row, and nothing of it is walked. The walk is seeded from refs alone,
//! so how the stream knows the base is reached when the stash's date comes up is by looking
//! ahead in the walk itself:
//!
//! - When a stash's date comes up, the commits the walk has already yielded but not yet
//!   handed on are searched for its base, and up to [`LOOKAHEAD`] more are pulled off the
//!   walk to find it — stopping early once the walk passes the base's own date without
//!   meeting it. Found: the stash's row goes here, at its date. Not found: the stash is
//!   held.
//! - Whenever the next commit to hand on is a held stash's base — or the base of a stash
//!   whose date has not come up yet, as when a stash is dated older than its base by clock
//!   skew — the stash's row goes directly above it.
//! - A stash still waiting when the walk ends had a base the walk never reached: no row.
//!
//! So a stash's row is exact — there if and only if the walk reaches its base — and costs
//! no walk of its own: the commits looked ahead at are the ones the stream hands on next.
//! A stash whose base is reached only far below its date (more than [`LOOKAHEAD`] commits,
//! or past a skewed date) is drawn directly above its base rather than at its date.
//!
//! A stash commit the walk reaches itself, through some ref, is that commit's row, never a
//! stash's: the stream drops the stash when the walk hands the commit on, or meets it while
//! looking ahead. In graph order, which carries no dates, every stash goes directly above
//! its base.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use cairn_model::Oid;

use super::seeds::Decoration;
use super::walk::CommitWalk;
use crate::{Cancel, Error};

/// How many commits the stream pulls ahead of the one it hands on next, looking for a
/// stash's base when the stash's date comes up.
pub(super) const LOOKAHEAD: usize = 4096;

/// One entry of the stream, in the order the lane assigner lays them out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Entry {
    Commit {
        id: Oid,
        parents: Vec<Oid>,
    },
    /// The stash at this index of the decoration's stashes.
    Stash(usize),
}

/// What [`Stream::next`] found.
#[derive(Debug)]
pub(super) enum Next {
    Entry(Entry),
    End,
    /// `cancel` fired while the stream looked ahead; nothing was lost, and the next call
    /// carries on.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// Its date has not come up.
    Waiting,
    /// Its date came up before the stream knew its base was reached: it goes directly above
    /// its base, if the walk reaches it.
    Held,
    /// Its row is laid out, or it has none.
    Done,
}

/// A commit pulled off the walk and not yet handed on.
#[derive(Debug)]
struct Walked {
    id: Oid,
    parents: Vec<Oid>,
    /// Its committer date; `None` in graph order.
    time: Option<i64>,
}

pub(super) struct Stream<'repo> {
    walk: CommitWalk<'repo>,
    decoration: Arc<Decoration>,
    lookahead: usize,
    ahead: VecDeque<Walked>,
    exhausted: bool,
    /// One per stash of the decoration.
    places: Vec<Place>,
    /// Stashes by base, each list newest committed first: what goes directly above it.
    by_base: HashMap<Oid, Vec<usize>>,
    /// Stashes by their own commit, to drop one the walk reaches as a commit.
    by_commit: HashMap<Oid, usize>,
    /// Stashes newest committed first, and the first whose date has not come up.
    dated: Vec<usize>,
    next_dated: usize,
    /// Commits pulled off the walk.
    pulled: usize,
}

impl std::fmt::Debug for Stream<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("ahead", &self.ahead.len())
            .field("exhausted", &self.exhausted)
            .field("places", &self.places)
            .finish_non_exhaustive()
    }
}

impl<'repo> Stream<'repo> {
    pub(super) fn new(
        walk: CommitWalk<'repo>,
        decoration: Arc<Decoration>,
        lookahead: usize,
    ) -> Self {
        let stashes = decoration.stashes();
        let mut dated: Vec<usize> = (0..stashes.len()).collect();
        // Newest committed first; among equal dates, `stash@{0}` first.
        dated.sort_by_key(|&at| {
            stashes
                .get(at)
                .map(|stash| (std::cmp::Reverse(stash.committed), stash.index))
        });
        let mut by_base: HashMap<Oid, Vec<usize>> = HashMap::new();
        let mut by_commit = HashMap::new();
        for &at in &dated {
            if let Some(stash) = stashes.get(at) {
                by_base.entry(stash.base).or_default().push(at);
                by_commit.insert(stash.id, at);
            }
        }
        Self {
            walk,
            places: vec![Place::Waiting; stashes.len()],
            decoration,
            lookahead,
            ahead: VecDeque::new(),
            exhausted: false,
            by_base,
            by_commit,
            dated,
            next_dated: 0,
            pulled: 0,
        }
    }

    /// Commits pulled off the walk so far, those looked ahead at included.
    pub(super) fn pulled(&self) -> usize {
        self.pulled
    }

    /// The next entry. The caller polls `cancel` before asking for one; the stream polls it
    /// again before each commit it pulls to look ahead for a stash's base.
    pub(super) fn next(&mut self, cancel: &impl Cancel) -> Result<Next, Error> {
        if self.ahead.is_empty() && !self.exhausted {
            self.pull()?;
        }
        let Some(front) = self.ahead.front() else {
            // The walk is over: a stash still waiting had a base it never reached.
            return Ok(Next::End);
        };
        let (front_id, front_time) = (front.id, front.time);

        // A stash made on the commit about to be handed on: directly above it.
        if let Some(stash) = self.above(&front_id) {
            return Ok(Next::Entry(Entry::Stash(stash)));
        }

        // Each stash whose date is newer than the commit about to be handed on goes here,
        // newest first, if its base is reached; one that is not is held.
        while let Some(stash) = self.due(front_time) {
            match self.look_ahead(stash, cancel)? {
                Ahead::Reached => {
                    self.finish(stash);
                    return Ok(Next::Entry(Entry::Stash(stash)));
                }
                Ahead::WalkedItself => self.finish(stash),
                Ahead::NotYet => self.hold(stash),
                Ahead::Cancelled => return Ok(Next::Cancelled),
            }
        }

        let Some(walked) = self.ahead.pop_front() else {
            return Ok(Next::End);
        };
        if let Some(&stash) = (!self.by_commit.is_empty())
            .then(|| self.by_commit.get(&walked.id))
            .flatten()
        {
            // The walk reaches the stash commit itself: that is a commit's row.
            self.finish(stash);
        }
        Ok(Next::Entry(Entry::Commit {
            id: walked.id,
            parents: walked.parents,
        }))
    }

    /// The newest stash on `base` not yet placed, marked placed.
    fn above(&mut self, base: &Oid) -> Option<usize> {
        if self.by_base.is_empty() {
            return None;
        }
        let waiting = self.by_base.get(base)?;
        let stash = waiting
            .iter()
            .copied()
            .find(|&at| self.places.get(at) != Some(&Place::Done))?;
        self.finish(stash);
        Some(stash)
    }

    /// The newest stash still waiting whose date is newer than `time`, the date of the
    /// commit about to be handed on.
    fn due(&mut self, time: Option<i64>) -> Option<usize> {
        let time = time?;
        while let Some(&at) = self.dated.get(self.next_dated) {
            if self.places.get(at) != Some(&Place::Waiting) {
                self.next_dated += 1;
                continue;
            }
            let committed = self.decoration.stashes().get(at)?.committed;
            return (committed > time).then_some(at);
        }
        None
    }

    /// Whether stash `at`'s base is among the commits pulled and not yet handed on, pulling
    /// up to the lookahead more to find it.
    fn look_ahead(&mut self, at: usize, cancel: &impl Cancel) -> Result<Ahead, Error> {
        let Some(stash) = self.decoration.stashes().get(at) else {
            return Ok(Ahead::NotYet);
        };
        let (id, base, base_committed) = (stash.id, stash.base, stash.base_committed);
        for walked in &self.ahead {
            if walked.id == id {
                return Ok(Ahead::WalkedItself);
            }
            if walked.id == base {
                return Ok(Ahead::Reached);
            }
        }
        while self.ahead.len() < self.lookahead && !self.exhausted {
            if cancel.is_cancelled() {
                return Ok(Ahead::Cancelled);
            }
            let Some(walked) = self.pull()? else {
                break;
            };
            if walked.id == id {
                return Ok(Ahead::WalkedItself);
            }
            if walked.id == base {
                return Ok(Ahead::Reached);
            }
            if let (Some(time), Some(base_time)) = (walked.time, base_committed)
                && time < base_time
            {
                // Past the base's date without meeting it: it is reached far below, if at all.
                break;
            }
        }
        Ok(Ahead::NotYet)
    }

    fn finish(&mut self, at: usize) {
        if let Some(place) = self.places.get_mut(at) {
            *place = Place::Done;
        }
    }

    fn hold(&mut self, at: usize) {
        if let Some(place) = self.places.get_mut(at) {
            *place = Place::Held;
        }
    }

    /// Pulls one commit off the walk to the back of what is ahead, or notes the walk's end.
    fn pull(&mut self) -> Result<Option<&Walked>, Error> {
        let Some(next) = self.walk.next() else {
            self.exhausted = true;
            return Ok(None);
        };
        let info = next.map_err(|source| Error::Walk {
            source: Box::new(source),
        })?;
        let walked = Walked {
            id: crate::object_id::model_id(&info.id)?,
            parents: super::parents_of(&info)?,
            time: info.commit_time,
        };
        self.pulled += 1;
        self.ahead.push_back(walked);
        Ok(self.ahead.back())
    }
}

/// What looking ahead for a stash's base found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ahead {
    Reached,
    /// The stash commit itself is a commit the walk reaches.
    WalkedItself,
    NotYet,
    Cancelled,
}
