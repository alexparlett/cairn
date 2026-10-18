//! `git log --date-order`'s walk: no commit before all of its children, otherwise newest
//! committer date first, and every tie in the order git breaks it.
//!
//! This is git's incremental topological walk in `revision.c` (v2.30.9 `:3304-3533`, v2.56.0
//! `:3739-4007`) with the topological queue ordered by commit date, as `--date-order` orders
//! it (`REV_SORT_BY_COMMIT_DATE`), each step below citing the function it is. git takes this
//! path where generation numbers are enabled and otherwise walks every reachable commit
//! first (`limit_list`) and sorts the whole list (`sort_in_topological_order`, `commit.c`);
//! a commit then becomes ready on both paths at the same moment, when its last child is
//! emitted, in the same parent loop, so both give one order. Here every commit read
//! without the commit-graph has an infinite generation, which makes the opening pass walk
//! everything reachable before the first commit, as git's whole-list path does, and with a
//! graph only as deep as the lowest generation it is about to emit.
//!
//! - **Tips** (`prepare_revision_walk`, v2.30.9 `:3535`, v2.56.0 `:4016`): each once, in the
//!   order given, a missing one or one that is no commit dropped, then sorted newest
//!   committer date first by a stable sort (`commit_list_sort_by_date`, v2.30.9
//!   `revision.c:3570`, v2.56.0 `:4050`), so equal dates keep the order given: the refs' and
//!   `HEAD`, then each stash, then each reflog id (Show Lost Commits).
//! - **The queues** (`init_topo_walk`, v2.30.9 `:3413`, v2.56.0 `:3880`): the in-degree queue
//!   ordered by generation then date (`compare_commits_by_gen_then_commit_date`, v2.30.9
//!   `commit.c:731`, v2.56.0 `:902`), the topological queue by date
//!   (`compare_commits_by_commit_date`, v2.30.9 `commit.c:751`, v2.56.0 `:923`), each popping
//!   ties in the order they were put (`queue`).
//! - **The in-degree walk** (`compute_indegrees_to_depth` and `indegree_walk_step`, v2.30.9
//!   `:3390`, `:3360`; v2.56.0 `:3852`, `:3817`): pops every queued commit down to a
//!   generation, each parent's in-degree going 0 to 2 or up by one per parent entry
//!   (v2.30.9 `:3381`, v2.56.0 `:3843`). git's explore walk beside it only settles which
//!   commits a negative tip hides; a history walk has no negative tip, and the explore walk
//!   then reads exactly the commits the in-degree walk reads, so it is left out.
//! - **Emission** (`next_topo_commit` and `expand_topo_walk`, v2.30.9 `:3483`, `:3497`;
//!   v2.56.0 `:3955`, `:3969`): pop the topological queue, zero the commit's in-degree, and
//!   for each parent in the commit's order, every entry, decrement its in-degree and put it
//!   when it reaches 1 (v2.30.9 `:3526-3528`, v2.56.0 `:4000-4002`).
//!
//! A stash is a tip whose one parent is its base, as `git replace --graft <stash>
//! <stash>^1` makes it for git: its commit's own parents are never read, from its object or
//! the commit-graph. It is seeded only when the refs reach its base, which the in-degree walk
//! decides — a base it reached once it has walked down to the base's generation has an
//! in-degree — and it is decided no sooner than the walk needs it: when the walk has gone that
//! deep already, when the stash would pop before the commit about to be emitted, or before its
//! base's in-degree is counted down. Until then it holds the place among git's tips it was
//! given (`Queue::reserve`), so when it is seeded it pops where git's walk pops it. So a stash
//! made on an old commit costs the first page nothing where a commit-graph spares the walk.
//! With Show Lost Commits' reflog ids among the tips, the stashes are decided before those ids
//! join the walk, since the ids may reach a base the refs do not, and the ids walk deep
//! anyway.

mod commits;
mod pace;
mod queue;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use cairn_model::Oid;

use crate::object_id::{model_id, object_id};
use crate::{Cancel, Error, Repository};

use super::walk::{self, Grafted};
use commits::{Commits, Mark, Node, date, generation};
use pace::Pace;
use queue::Queue;

pub(crate) use pace::REPORT_EVERY;

/// What a walk starts from, each list in the order git is given it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Tips<'a> {
    /// The commits refs name, then `HEAD`'s.
    pub(crate) refs: &'a [gix::ObjectId],
    /// Each stash's commit and its base, in `stash@{n}` order.
    pub(crate) stashes: &'a [(gix::ObjectId, gix::ObjectId)],
    /// Ids only reflogs name: Show Lost Commits' tips.
    pub(crate) reflogs: &'a [gix::ObjectId],
}

/// What [`DateOrder::next`] found.
#[derive(Debug)]
pub(crate) enum Popped {
    /// The next commit, with its parents as the walk read them, and, for a stash, its index
    /// among the stashes the walk was given.
    Commit {
        info: gix::traverse::commit::Info,
        stash: Option<usize>,
    },
    End,
    /// `cancel` fired; the walk is as it was, and the next call carries on.
    Cancelled,
}

/// git's date-order walk, held open across calls.
pub(crate) struct DateOrder<'repo> {
    commits: Commits<'repo>,
    /// `info->indegree_queue`: by generation, then committer date.
    indegree_queue: Queue<(u32, u64), gix::ObjectId>,
    /// `info->topo_queue`: by committer date.
    topo_queue: Queue<u64, gix::ObjectId>,
    /// `info->min_generation`: every queued commit at or above it has been walked.
    min_generation: u32,
    /// The stashes seeded, by commit, each with its index among those given.
    stashes: HashMap<gix::ObjectId, usize>,
    /// The stashes not yet decided, the first the one git's queue would pop first.
    pending: Vec<Pending>,
    /// Commits the in-degree walk has popped: git's `count_indegree_walked`.
    walked: usize,
}

impl std::fmt::Debug for DateOrder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DateOrder")
            .field("read", &self.commits.read_count())
            .field("walked", &self.walked)
            .field("ready", &self.topo_queue.len())
            .field("min_generation", &self.min_generation)
            .finish_non_exhaustive()
    }
}

/// A stash whose base the walk has not yet decided the refs reach.
#[derive(Debug, Clone, Copy)]
struct Pending {
    stash: gix::ObjectId,
    base: gix::ObjectId,
    /// Its index among the stashes given.
    index: usize,
    /// Its committer date, which orders it among the tips.
    date: u64,
    /// The base's generation, down to which the in-degree walk must go to decide it.
    generation: u32,
    /// The place among the topological queue's puts it takes as a tip, once tips are put.
    place: Option<u64>,
}

impl Pending {
    /// Its key as the topological queue orders it: the greater pops first.
    fn order(&self) -> (u64, std::cmp::Reverse<u64>) {
        (self.date, std::cmp::Reverse(self.place.unwrap_or(0)))
    }
}

/// What one tip given to the walk is, in `revs->commits`.
#[derive(Debug, Clone, Copy)]
enum Given {
    Commit(gix::ObjectId),
    /// A stash's commit, pending or seeded.
    Stash(gix::ObjectId),
}

/// Whether a run of the in-degree walk went as deep as it was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Depth {
    Reached,
    Cancelled,
}

/// The count of commits read, given to `report` at most once per `every`.
struct Progress<'a> {
    pace: Pace,
    report: &'a mut dyn FnMut(usize),
}

impl<'repo> DateOrder<'repo> {
    /// The walk from `tips`, its opening pass run: `None` when `cancel` fired, polled before
    /// each tip and before each commit the in-degree walk pops, nothing kept.
    /// How many commits the pass has read goes to `report`, at most once per `every`, the
    /// first once `every` has passed.
    pub(crate) fn open(
        repo: &'repo gix::Repository,
        tips: Tips<'_>,
        cancel: &impl Cancel,
        report: &mut dyn FnMut(usize),
        every: Duration,
    ) -> Result<Option<Self>, Error> {
        let mut progress = Progress {
            pace: Pace::starting(Instant::now(), every),
            report,
        };
        let substitutions = walk::Substitutions::read(repo)?;
        let graph = walk::commit_graph(repo, &substitutions)?;
        let mut walk = Self {
            // Each commit is read once and kept, so an object cache would only be filled.
            commits: Commits::new(
                Grafted::uncached(&repo.objects, substitutions),
                graph,
                repo.object_hash(),
            ),
            indegree_queue: Queue::new(),
            topo_queue: Queue::new(),
            min_generation: commits::INFINITE,
            stashes: HashMap::new(),
            pending: Vec::new(),
            walked: 0,
        };
        // `prepare_revision_walk`'s `SEEN`: a tip already given keeps its first place.
        let mut seen = HashSet::new();
        // `revs->commits`, in the order given, sorted below.
        let mut given: Vec<(Given, u64)> = Vec::new();

        // `init_topo_walk`: each tip read, queued, its in-degree 1, and `min_generation`
        // lowered to its generation; then in-degrees walked down to it.
        let mut cutoff = commits::INFINITE;
        for tip in tips.refs {
            if cancel.is_cancelled() {
                return Ok(None);
            }
            if !seen.insert(*tip) {
                continue;
            }
            if let Some((date, generation)) = walk.begin(tip)? {
                given.push((Given::Commit(*tip), date));
                cutoff = cutoff.min(generation);
            }
        }
        if walk.compute_indegrees_to_depth(cutoff, cancel, Some(&mut progress))? == Depth::Cancelled
        {
            return Ok(None);
        }
        walk.min_generation = cutoff;

        // Each stash read — its date, and its base — and held pending.
        let mut stash_tips = Vec::new();
        for (index, (stash, base)) in tips.stashes.iter().enumerate() {
            if cancel.is_cancelled() {
                return Ok(None);
            }
            if !seen.insert(*stash) {
                continue;
            }
            stash_tips.push(*stash);
            let Some(generation) = walk.commits.read(base)?.map(|node| generation(node)) else {
                continue;
            };
            let Some(date) = walk.commits.date_of(stash)? else {
                continue;
            };
            walk.pending.push(Pending {
                stash: *stash,
                base: *base,
                index,
                date: date as u64,
                generation,
                place: None,
            });
            given.push((Given::Stash(*stash), date as u64));
        }
        // Decided now: each stash the walk has gone deep enough for already, so a base that is
        // itself a tip is never put on the queue before a stash on it — and, with reflog ids
        // to walk, every stash.
        let eager = !tips.reflogs.is_empty();
        let mut index = 0;
        while let Some(pending) = walk.pending.get(index).copied() {
            if eager || pending.generation >= walk.min_generation {
                if walk.decide(index, cancel, Some(&mut progress))? == Depth::Cancelled {
                    return Ok(None);
                }
            } else {
                index += 1;
            }
        }
        // A stash the refs did not reach is no tip: a reflog id naming its commit is one.
        for stash in &stash_tips {
            if !walk.stashes.contains_key(stash) && !walk.is_pending(stash) {
                seen.remove(stash);
            }
        }
        given.retain(|(tip, _)| match tip {
            Given::Commit(_) => true,
            Given::Stash(stash) => walk.stashes.contains_key(stash) || walk.is_pending(stash),
        });

        // The reflogs' ids join the in-degree walk as tips of their own; a commit the refs
        // already reach keeps the in-degree they gave it, which counts it a tip as git's
        // single pass would (its 1, plus one per child reached).
        for tip in tips.reflogs {
            if cancel.is_cancelled() {
                return Ok(None);
            }
            if !seen.insert(*tip) {
                continue;
            }
            if let Some((date, generation)) = walk.begin(tip)? {
                given.push((Given::Commit(*tip), date));
                cutoff = cutoff.min(generation);
            }
        }
        let cutoff = cutoff.min(walk.min_generation);
        if walk.compute_indegrees_to_depth(cutoff, cancel, Some(&mut progress))? == Depth::Cancelled
        {
            return Ok(None);
        }
        walk.min_generation = cutoff;

        // `commit_list_sort_by_date`: stable, so equal dates keep the order given; then every
        // tip no other tip reaches put on the topological queue in that order, and a stash
        // still pending given the place it would take.
        given.sort_by_key(|&(_, date)| std::cmp::Reverse(date));
        for (tip, date) in given {
            let id = match tip {
                Given::Commit(id) => id,
                Given::Stash(stash) => {
                    if let Some(pending) = walk.pending.iter_mut().find(|p| p.stash == stash) {
                        pending.place = Some(walk.topo_queue.reserve());
                        continue;
                    }
                    stash
                }
            };
            if walk
                .commits
                .get(&id)
                .is_some_and(|node| node.data.indegree == 1)
            {
                walk.topo_queue.put(date, id);
            }
        }
        walk.pending
            .sort_by_key(|pending| std::cmp::Reverse(pending.order()));
        Ok(Some(walk))
    }

    /// The next commit in git's date order. `cancel` is polled before each commit the
    /// in-degree walk pops to know the next commit's parents' children; a cancelled step
    /// changes nothing.
    pub(crate) fn next(&mut self, cancel: &impl Cancel) -> Result<Popped, Error> {
        // A pending stash git's queue would pop before the head, or with nothing left, is
        // decided first: seeded, it is the next commit.
        while let Some(first) = self.pending.first() {
            let before_head = match self.topo_queue.peek_place() {
                None => true,
                Some((&date, place)) => first.order() > (date, std::cmp::Reverse(place)),
            };
            if !before_head {
                break;
            }
            if self.decide(0, cancel, None)? == Depth::Cancelled {
                return Ok(Popped::Cancelled);
            }
        }
        let Some((_, &next)) = self.topo_queue.peek() else {
            return Ok(Popped::End);
        };
        let parents = self
            .commits
            .get(&next)
            .map(|node| node.parents.clone())
            .unwrap_or_default();
        // `expand_topo_walk` lowers `min_generation` to each parent's generation in turn
        // and walks in-degrees down to it before decrementing that parent. Walking down to
        // the lowest of them first reaches the same in-degrees by the time each parent is
        // decremented — no commit at or above a parent's generation can be a child of one
        // decremented before it — and lets a cancelled walk leave the commit unpopped.
        let lowest = parents
            .iter()
            .filter_map(|parent| self.commits.get(parent))
            .map(generation)
            .min();
        if let Some(lowest) = lowest
            && lowest < self.min_generation
        {
            if self.compute_indegrees_to_depth(lowest, cancel, None)? == Depth::Cancelled {
                return Ok(Popped::Cancelled);
            }
            self.min_generation = lowest;
        }
        // A stash pending on a parent is decided before that parent is counted down, so a
        // seeded stash keeps its base from being ready before it. The walk is already down
        // to the parent's generation, so this reads nothing more and cannot be cancelled;
        // and the stash pops after the head, or it would have been decided above.
        for parent in &parents {
            while let Some(index) = self.pending.iter().position(|p| p.base == *parent) {
                if self.decide(index, cancel, None)? == Depth::Cancelled {
                    return Ok(Popped::Cancelled);
                }
            }
        }

        // `next_topo_commit`.
        self.topo_queue.pop();
        let Some(node) = self.commits.get_mut(&next) else {
            return Err(broken("an emitted commit was never read"));
        };
        node.data.indegree = 0;
        let info = gix::traverse::commit::Info {
            id: next,
            parent_ids: node.parents.clone(),
            generation: node.generation,
            commit_time: Some(node.commit_time),
        };
        // `expand_topo_walk`: each parent in the commit's order, every entry; a parent that
        // could not be read was never counted, and is skipped as git skips one it cannot
        // parse.
        for parent in &parents {
            let Some(node) = self.commits.get_mut(parent) else {
                continue;
            };
            if node.data.indegree < 2 {
                return Err(broken("a parent's children were not all counted"));
            }
            node.data.indegree -= 1;
            if node.data.indegree == 1 {
                let date = date(node);
                self.topo_queue.put(date, *parent);
            }
        }
        Ok(Popped::Commit {
            info,
            stash: self.stashes.get(&next).copied(),
        })
    }

    /// Commits read, from the commit-graph or from their objects.
    pub(crate) fn read(&self) -> usize {
        self.commits.read_count()
    }

    /// Commits read from the commit-graph.
    pub(crate) fn read_from_graph(&self) -> usize {
        self.commits.read_from_graph()
    }

    /// Commits the in-degree walk has popped.
    pub(crate) fn walked(&self) -> usize {
        self.walked
    }

    /// The bytes the walk's commits hold.
    pub(crate) fn retained(&self) -> usize {
        self.commits.retained()
    }

    /// `tip` read and begun as `init_topo_walk` begins a tip: queued for the in-degree walk
    /// and its in-degree 1. A commit an earlier tip already reached keeps its in-degree and
    /// its place in the queue. Its date and generation, or `None` when it cannot be read.
    fn begin(&mut self, tip: &gix::ObjectId) -> Result<Option<(u64, u32)>, Error> {
        let Some(node) = self.commits.read(tip)? else {
            return Ok(None);
        };
        let (date, generation) = (date(node), generation(node));
        if node.data.indegree == 0 {
            node.data.indegree = 1;
        }
        if !node.data.queued {
            node.data.queued = true;
            self.indegree_queue.put((generation, date), *tip);
        }
        Ok(Some((date, generation)))
    }

    fn is_pending(&self, stash: &gix::ObjectId) -> bool {
        self.pending.iter().any(|pending| pending.stash == *stash)
    }

    /// Decides the pending stash at `index`: the in-degree walk taken down to its base's
    /// generation, then the stash seeded as a tip whose one parent is its base when the refs
    /// reach the base — the walk gave it an in-degree, and it is no stash's own commit — and
    /// do not reach the stash's commit itself, which is then that commit's row; dropped
    /// otherwise. A stash seeded once tips are on the queue is put at the place it took.
    /// Cancelled, nothing is decided.
    fn decide(
        &mut self,
        index: usize,
        cancel: &impl Cancel,
        progress: Option<&mut Progress<'_>>,
    ) -> Result<Depth, Error> {
        let Some(pending) = self.pending.get(index).copied() else {
            return Ok(Depth::Reached);
        };
        if pending.generation < self.min_generation {
            if self.compute_indegrees_to_depth(pending.generation, cancel, progress)?
                == Depth::Cancelled
            {
                return Ok(Depth::Cancelled);
            }
            self.min_generation = pending.generation;
        }
        self.pending.remove(index);
        let Some(base) = self.commits.get(&pending.base) else {
            return Ok(Depth::Reached);
        };
        let reached = base.data.indegree != 0 && !self.stashes.contains_key(&pending.base);
        let base_generation = base.generation;
        let walked_itself = self
            .commits
            .get(&pending.stash)
            .is_some_and(|node| node.data.queued || node.data.indegree != 0);
        if !reached || walked_itself {
            return Ok(Depth::Reached);
        }
        let mut parents = gix::traverse::commit::ParentIds::new();
        parents.push(pending.base);
        self.commits.insert(
            pending.stash,
            Node {
                parents,
                commit_time: pending.date as i64,
                // Any generation above the base's keeps the walk's bound; none where the base
                // has none.
                generation: base_generation.map(|generation| generation.saturating_add(1)),
                // Its in-degree step is the one below: the base counts it as a child.
                data: Mark {
                    indegree: 1,
                    queued: true,
                },
            },
        );
        if let Some(base) = self.commits.get_mut(&pending.base) {
            base.data.indegree += 1;
        }
        self.stashes.insert(pending.stash, pending.index);
        if let Some(place) = pending.place {
            self.topo_queue.put_at(pending.date, place, pending.stash);
        }
        Ok(Depth::Reached)
    }

    /// `compute_indegrees_to_depth`: every queued commit at or above `cutoff` popped and its
    /// parents counted, `cancel` polled before each; how many commits were read so far goes
    /// to `progress` on its pace.
    fn compute_indegrees_to_depth(
        &mut self,
        cutoff: u32,
        cancel: &impl Cancel,
        mut progress: Option<&mut Progress<'_>>,
    ) -> Result<Depth, Error> {
        while let Some((&(generation, _), _)) = self.indegree_queue.peek() {
            if generation < cutoff {
                break;
            }
            if cancel.is_cancelled() {
                return Ok(Depth::Cancelled);
            }
            self.indegree_walk_step()?;
            if let Some(progress) = progress.as_deref_mut()
                && self.walked.is_multiple_of(256)
                && progress.pace.due(Instant::now())
            {
                (progress.report)(self.commits.read_count());
            }
        }
        Ok(Depth::Reached)
    }

    /// `indegree_walk_step`: the in-degree queue's head popped, and each of its parents, every
    /// entry, counted — 0 to 2, or up by one — and queued once. A parent that cannot be read
    /// is skipped here and at emission alike (git v2.56.0 stops counting the commit's later
    /// parents there; both skip it at emission).
    fn indegree_walk_step(&mut self) -> Result<(), Error> {
        let Some((_, commit)) = self.indegree_queue.pop() else {
            return Ok(());
        };
        let Some(parents) = self.commits.get(&commit).map(|node| node.parents.clone()) else {
            return Ok(());
        };
        self.walked += 1;
        for parent in &parents {
            let Some(node) = self.commits.read(parent)? else {
                continue;
            };
            node.data.indegree = if node.data.indegree == 0 {
                2
            } else {
                node.data.indegree + 1
            };
            if !node.data.queued {
                node.data.queued = true;
                let key = (generation(node), date(node));
                self.indegree_queue.put(key, *parent);
            }
        }
        Ok(())
    }
}

/// What a walk whose accounting broke fails with: never expected, and an error rather than a
/// panic.
#[derive(Debug, thiserror::Error)]
#[error("the date-order walk's in-degrees broke: {0}")]
struct Broken(&'static str);

fn broken(what: &'static str) -> Error {
    Error::Walk {
        source: Box::new(Broken(what)),
    }
}

/// What a date-order walk starts from, each list in the order git is given it: git keeps that
/// order among tips whose dates tie.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DateOrderTips {
    refs: Vec<Oid>,
    stashes: Vec<(Oid, Oid)>,
    reflogs: Vec<Oid>,
}

impl DateOrderTips {
    /// The commits refs name, in the order `git log --branches --remotes --tags HEAD` takes
    /// them: local branches, remote-tracking refs and tags, each in refname order, then
    /// `HEAD`'s.
    pub fn new(refs: impl IntoIterator<Item = Oid>) -> Self {
        Self {
            refs: refs.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Each stash, its commit and its base, in `stash@{n}` order: a tip after the refs whose
    /// one parent is its base, seeded only when the refs reach that base.
    pub fn with_stashes(mut self, stashes: impl IntoIterator<Item = (Oid, Oid)>) -> Self {
        self.stashes = stashes.into_iter().collect();
        self
    }

    /// Show Lost Commits' ids, which only reflogs name: tips after the stashes.
    pub fn with_reflog_tips(mut self, ids: impl IntoIterator<Item = Oid>) -> Self {
        self.reflogs = ids.into_iter().collect();
        self
    }
}

/// One commit of a [`DateOrderWalk`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateOrdered {
    pub id: Oid,
    /// As git shows them; a stash's is its base alone.
    pub parents: Vec<Oid>,
    /// For a stash, its index among the stashes the walk was given.
    pub stash: Option<usize>,
}

/// git's date-order walk over a repository, its opening pass run.
pub struct DateOrderWalk<'repo> {
    walk: DateOrder<'repo>,
}

impl std::fmt::Debug for DateOrderWalk<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.walk.fmt(f)
    }
}

impl Repository {
    /// The commits `tips` reach in `git log --date-order`'s order, its opening pass run:
    /// without a commit-graph git can use, every commit reachable is read before the first
    /// is known, as git's own walk reads them. `cancel` is polled before each tip and before
    /// each commit the pass reads parents for; a cancelled open is
    /// [`Error::Cancelled`] and keeps nothing. How many commits the pass has read goes to
    /// `read`, at most once per 100 ms, the first once it has run that long.
    pub fn date_order(
        &self,
        tips: &DateOrderTips,
        cancel: &impl Cancel,
        read: &mut dyn FnMut(usize),
    ) -> Result<DateOrderWalk<'_>, Error> {
        open_public(self, tips, cancel, read, REPORT_EVERY)
    }
}

fn open_public<'repo>(
    repo: &'repo Repository,
    tips: &DateOrderTips,
    cancel: &impl Cancel,
    read: &mut dyn FnMut(usize),
    every: Duration,
) -> Result<DateOrderWalk<'repo>, Error> {
    let ids = |oids: &[Oid]| -> Result<Vec<gix::ObjectId>, Error> {
        oids.iter().map(object_id).collect()
    };
    let refs = ids(&tips.refs)?;
    let reflogs = ids(&tips.reflogs)?;
    let stashes = tips
        .stashes
        .iter()
        .map(|(stash, base)| Ok((object_id(stash)?, object_id(base)?)))
        .collect::<Result<Vec<_>, Error>>()?;
    let tips = Tips {
        refs: &refs,
        stashes: &stashes,
        reflogs: &reflogs,
    };
    match DateOrder::open(repo.inner(), tips, cancel, read, every)? {
        Some(walk) => Ok(DateOrderWalk { walk }),
        None => Err(Error::Cancelled { walked: 0 }),
    }
}

impl DateOrderWalk<'_> {
    /// The next commit, `None` at the end. `cancel` is polled before each commit read to
    /// know its parents' children; a cancelled call is [`Error::Cancelled`] and changes
    /// nothing, so the next carries on.
    pub fn next(&mut self, cancel: &impl Cancel) -> Result<Option<DateOrdered>, Error> {
        match self.walk.next(cancel)? {
            Popped::Commit { info, stash } => Ok(Some(DateOrdered {
                id: model_id(&info.id)?,
                parents: info
                    .parent_ids
                    .iter()
                    .map(|parent| model_id(parent))
                    .collect::<Result<_, _>>()?,
                stash,
            })),
            Popped::End => Ok(None),
            Popped::Cancelled => Err(Error::Cancelled {
                walked: self.walk.walked(),
            }),
        }
    }

    /// Commits read so far, from the commit-graph or from their objects.
    pub fn commits_read(&self) -> usize {
        self.walk.read()
    }

    /// Commits read so far from the commit-graph: none where git would not read one.
    pub fn read_from_graph(&self) -> usize {
        self.walk.read_from_graph()
    }

    /// Commits whose parents the walk has counted so far.
    pub fn walked(&self) -> usize {
        self.walk.walked()
    }

    /// The bytes the walk keeps for the commits it has read.
    pub fn retained_bytes(&self) -> usize {
        self.walk.retained()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::CancelSignal;

    /// A repository of its own under the temporary directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cairn-date-order-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            git(&dir, &["init", "-q", "-b", "main"], None);
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `git` in `dir`, isolated from the machine's configuration, fed `input` on stdin: the
    /// git Cairn would find, so the fixture is made by the binary it runs.
    fn git(dir: &Path, args: &[&str], input: Option<&str>) -> String {
        use std::io::Write as _;
        let program = crate::ops::GitBinary::discover(&crate::ops::Askpass::new(
            "/nonexistent/cairn-askpass",
            None,
        ))
        .unwrap();
        let mut child = std::process::Command::new(program.path())
            .args(["-c", "maintenance.auto=false", "-c", "gc.auto=0"])
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "A")
            .env("GIT_AUTHOR_EMAIL", "a@example.com")
            .env("GIT_COMMITTER_NAME", "C")
            .env("GIT_COMMITTER_EMAIL", "c@example.com")
            .env("GIT_AUTHOR_DATE", "1500000000 +0000")
            .env("GIT_COMMITTER_DATE", "1500000000 +0000")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.unwrap_or("").as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn id(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).unwrap()
    }

    /// A `git fast-import` stream of `count` commits on `main`, one a second, a branch
    /// `side` forking every tenth and merged back ten later, so the walk has merges to count.
    fn imported(name: &str, count: usize) -> Scratch {
        let scratch = Scratch::new(name);
        let mut stream = String::new();
        let mut side_open = false;
        for n in 1..=count {
            let when = 1_500_000_000 + n;
            stream.push_str(&format!(
                "commit refs/heads/main\nmark :{n}\ncommitter C <c@example.com> {when} +0000\n\
                 data 0\n"
            ));
            if n > 1 {
                stream.push_str(&format!("from :{}\n", n - 1));
            }
            if n % 10 == 0 && side_open {
                stream.push_str("merge refs/heads/side\n");
                side_open = false;
            }
            stream.push('\n');
            if n % 10 == 5 {
                stream.push_str(&format!(
                    "commit refs/heads/side\ncommitter C <c@example.com> {when} +0000\n\
                     data 0\nfrom :{n}\n\n"
                ));
                side_open = true;
            }
        }
        git(scratch.path(), &["fast-import", "--quiet"], Some(&stream));
        scratch
    }

    fn worker(at: &Path) -> Repository {
        crate::SharedRepository::discover(at).unwrap().to_worker()
    }

    fn head(at: &Path) -> gix::ObjectId {
        id(&git(at, &["rev-parse", "HEAD"], None))
    }

    /// Every commit of the walk, in order.
    fn walked(walk: &mut DateOrder<'_>, cancel: &impl Cancel) -> Vec<gix::ObjectId> {
        let mut ids = Vec::new();
        loop {
            match walk.next(cancel).unwrap() {
                Popped::Commit { info, .. } => ids.push(info.id),
                Popped::End => return ids,
                Popped::Cancelled => panic!("an uncancelled walk was cancelled"),
            }
        }
    }

    fn refs_only(refs: &[gix::ObjectId]) -> Tips<'_> {
        Tips {
            refs,
            stashes: &[],
            reflogs: &[],
        }
    }

    /// A cancel that answers `true` on its `at`th poll, and counts every poll; with `once`, it
    /// answers `false` again after that poll.
    struct CancelAt {
        at: usize,
        once: bool,
        polls: Cell<usize>,
    }

    impl CancelAt {
        fn new(at: usize, once: bool) -> Self {
            Self {
                at,
                once,
                polls: Cell::new(0),
            }
        }
    }

    impl Cancel for CancelAt {
        fn is_cancelled(&self) -> bool {
            let polls = self.polls.get() + 1;
            self.polls.set(polls);
            if self.once {
                polls == self.at
            } else {
                polls >= self.at
            }
        }
    }

    /// A merge's parents are queued in the commit's own order, so at one date the first parent
    /// is shown first, git's breadth-first interleaving at equal times: the merge names its
    /// parents `y`, `z`, `x` — neither sorted by name nor by id — all four commits and the root
    /// dated the same second. Caught by: parents pushed in reverse, or sorted.
    #[test]
    fn parents_are_queued_in_gits_order() {
        let scratch = Scratch::new("octopus");
        let at = scratch.path();
        git(at, &["commit", "-q", "--allow-empty", "-m", "root"], None);
        let root = git(at, &["rev-parse", "HEAD"], None);
        let tree = git(at, &["rev-parse", "HEAD^{tree}"], None);
        let child = |name: &str| git(at, &["commit-tree", "-p", &root, "-m", name, &tree], None);
        let (x, y, z) = (child("x"), child("y"), child("z"));
        let merge = git(
            at,
            &[
                "commit-tree",
                "-p",
                &y,
                "-p",
                &z,
                "-p",
                &x,
                "-m",
                "merge",
                &tree,
            ],
            None,
        );
        let repo = worker(at);
        let tips = [id(&merge)];
        let mut walk = DateOrder::open(
            repo.inner(),
            refs_only(&tips),
            &CancelSignal::new(),
            &mut |_| {},
            REPORT_EVERY,
        )
        .unwrap()
        .unwrap();
        let order = walked(&mut walk, &CancelSignal::new());
        assert_eq!(order, [&merge, &y, &z, &x, &root].map(|hex| id(hex)));
        // git itself, as the oracle.
        let git_order: Vec<gix::ObjectId> = git(at, &["rev-list", "--date-order", &merge], None)
            .lines()
            .map(id)
            .collect();
        assert_eq!(order, git_order);
    }

    /// The opening pass polls before each commit it reads parents for, so it stops within one
    /// commit's work: it is polled at least once per commit the in-degree walk pops, and a cancel
    /// on any poll ends the open there — no walk handed back, and no poll after the one that
    /// said stop. Caught by: a pass that polls only per tip or per page, or carries on past a
    /// cancel.
    #[test]
    fn an_opening_pass_stops_when_superseded() {
        let scratch = imported("superseded", 400);
        let repo = worker(scratch.path());
        let tips = [head(scratch.path())];
        let counting = CancelAt::new(usize::MAX, false);
        let walk = DateOrder::open(
            repo.inner(),
            refs_only(&tips),
            &counting,
            &mut |_| {},
            REPORT_EVERY,
        )
        .unwrap()
        .unwrap();
        // Without a commit-graph the pass walks everything reachable before the first commit.
        let commits = git(scratch.path(), &["rev-list", "--count", "HEAD"], None);
        assert_eq!(walk.walked().to_string(), commits);
        assert!(
            counting.polls.get() >= walk.walked(),
            "polled {} times for {} commits walked",
            counting.polls.get(),
            walk.walked()
        );
        for at in [1, 2, 37, 200, walk.walked()] {
            let cancel = CancelAt::new(at, false);
            let opened = DateOrder::open(
                repo.inner(),
                refs_only(&tips),
                &cancel,
                &mut |_| {},
                REPORT_EVERY,
            )
            .unwrap();
            assert!(opened.is_none(), "cancelled on poll {at}, and opened");
            assert_eq!(cancel.polls.get(), at, "polled on after the cancel");
        }
    }

    /// A step that must walk in-degrees deeper to know its commit's parents' children — with a
    /// commit-graph, a step at a time — and is cancelled within that walk changes nothing: the
    /// walk carries on to the same order an uncancelled one gives. Caught by: the commit popped
    /// or a parent decremented before the in-degree walk finished.
    #[test]
    fn a_cancelled_step_leaves_the_walk_as_it_was() {
        let scratch = imported("cancelled-step", 120);
        git(
            scratch.path(),
            &["commit-graph", "write", "--reachable"],
            None,
        );
        let repo = worker(scratch.path());
        let tips = [head(scratch.path())];
        let open = || {
            DateOrder::open(
                repo.inner(),
                refs_only(&tips),
                &CancelSignal::new(),
                &mut |_| {},
                REPORT_EVERY,
            )
            .unwrap()
            .unwrap()
        };
        let expected = walked(&mut open(), &CancelSignal::new());
        let mut walk = open();
        assert!(
            walk.walked() < expected.len(),
            "the graph's generations bound the opening pass: it walked {} of {}",
            walk.walked(),
            expected.len()
        );
        let mut order = Vec::new();
        let mut cancelled = 0;
        let mut poll = 1;
        loop {
            // Each call is cancelled on its first poll, if it polls; asked again, it goes on.
            let cancel = CancelAt::new(poll, true);
            match walk.next(&cancel).unwrap() {
                Popped::Commit { info, .. } => order.push(info.id),
                Popped::End => break,
                Popped::Cancelled => {
                    cancelled += 1;
                    poll = 2;
                    continue;
                }
            }
            poll = 1;
        }
        assert!(cancelled > 10, "only {cancelled} steps were cancelled");
        assert_eq!(order, expected);
    }

    /// The opening pass hands the count of commits it has read to its listener as it goes, the
    /// count rising and never past what it read; at most once per interval (`pace`), here
    /// none, so every chance is taken. Caught by: no count, or one that runs ahead of the reads.
    #[test]
    fn an_opening_pass_reports_how_far_it_has_read() {
        let scratch = imported("reports", 1500);
        let repo = worker(scratch.path());
        let tips = [head(scratch.path())];
        let mut counts = Vec::new();
        let walk = DateOrder::open(
            repo.inner(),
            refs_only(&tips),
            &CancelSignal::new(),
            &mut |read| counts.push(read),
            Duration::ZERO,
        )
        .unwrap()
        .unwrap();
        assert!(counts.len() >= 4, "{counts:?}");
        assert!(
            counts.windows(2).all(|pair| pair[0] < pair[1]),
            "{counts:?}"
        );
        assert!(
            counts.iter().all(|&count| count <= walk.read()),
            "{counts:?}"
        );

        // At the production interval, a pass this short says nothing at all within 100 ms of
        // starting.
        let started = Instant::now();
        let mut reported = Vec::new();
        let _walk = DateOrder::open(
            repo.inner(),
            refs_only(&tips),
            &CancelSignal::new(),
            &mut |_| reported.push(started.elapsed()),
            REPORT_EVERY,
        )
        .unwrap()
        .unwrap();
        assert!(
            reported.iter().all(|&at| at >= REPORT_EVERY),
            "a count within the first interval: {reported:?}"
        );
    }

    /// Reporter. The opening pass from every ref of `CAIRN_BENCH_REPO` — refs/heads, refs/remotes
    /// and refs/tags each in refname order, then `HEAD`, as `git log --branches --remotes --tags
    /// HEAD` takes them — to its first 64 rows, seven times: each run's wall time, the median,
    /// the commits read and walked, and what the walk retains per commit read. With
    /// `CAIRN_BENCH_ORACLE` naming a file of git's own `--date-order` ids for those tips, every
    /// row is compared with it. Run with `--release`; the repository is only read.
    #[test]
    #[ignore = "needs a repository named by CAIRN_BENCH_REPO"]
    fn measures_the_opening_pass_against_a_named_repository() {
        let path = std::env::var("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO");
        let repo = Repository::discover(&path).unwrap();
        let tips = every_ref_in_gits_order(repo.inner());
        eprintln!("repository {path}: {} tips", tips.len());
        let mut times = Vec::new();
        for run in 0..7 {
            let rss_before = resident_kib();
            let started = Instant::now();
            let mut walk = DateOrder::open(
                repo.inner(),
                refs_only(&tips),
                &CancelSignal::new(),
                &mut |_| {},
                REPORT_EVERY,
            )
            .unwrap()
            .unwrap();
            let opened = started.elapsed();
            for _ in 0..64 {
                match walk.next(&CancelSignal::new()).unwrap() {
                    Popped::Commit { .. } => {}
                    Popped::End | Popped::Cancelled => break,
                }
            }
            let page = started.elapsed();
            times.push(page);
            let retained = walk.retained();
            eprintln!(
                "  run {run}: opened in {opened:?}, 64 rows at {page:?}; read {} ({} from the \
                 commit-graph), walked {}; retains {:.1} MiB, {} B per commit read; resident \
                 {} KiB -> {} KiB",
                walk.read(),
                walk.read_from_graph(),
                walk.walked(),
                retained as f64 / (1024.0 * 1024.0),
                retained / walk.read().max(1),
                rss_before,
                resident_kib(),
            );
        }
        times.sort();
        eprintln!("  median of seven, to the 64th row: {:?}", times[3]);

        if let Ok(oracle) = std::env::var("CAIRN_BENCH_ORACLE") {
            let expected: Vec<gix::ObjectId> = std::fs::read_to_string(&oracle)
                .unwrap()
                .lines()
                .map(id)
                .collect();
            let started = Instant::now();
            let mut walk = DateOrder::open(
                repo.inner(),
                refs_only(&tips),
                &CancelSignal::new(),
                &mut |_| {},
                REPORT_EVERY,
            )
            .unwrap()
            .unwrap();
            let order = walked(&mut walk, &CancelSignal::new());
            eprintln!(
                "  the whole walk: {} rows in {:?}; retains {:.1} MiB, {} B per commit; peak \
                 resident {} KiB",
                order.len(),
                started.elapsed(),
                walk.retained() as f64 / (1024.0 * 1024.0),
                walk.retained() / order.len().max(1),
                peak_resident_kib(),
            );
            let first_difference = order
                .iter()
                .zip(&expected)
                .position(|(ours, git)| ours != git);
            eprintln!(
                "  against git's {} rows: first difference at {first_difference:?}",
                expected.len()
            );
            assert_eq!(first_difference, None);
            assert_eq!(order.len(), expected.len());
        }
    }

    /// Reporter. What deciding a stash's base costs the first page of 64 rows: from `HEAD` of
    /// `CAIRN_BENCH_REPO` alone, then with each stash `CAIRN_BENCH_STASHES` names
    /// (`stash:base` pairs of ids, comma-separated) — any commit stands in for a stash, its date
    /// deciding how soon the walk meets it. Run with `--release`; the repository is only read.
    #[test]
    #[ignore = "needs a repository named by CAIRN_BENCH_REPO"]
    fn measures_what_deciding_a_stashs_base_costs() {
        let path = std::env::var("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO");
        let stashes = std::env::var("CAIRN_BENCH_STASHES").expect("set CAIRN_BENCH_STASHES");
        let repo = Repository::discover(&path).unwrap();
        let refs = [repo.inner().head_id().unwrap().detach()];
        let mut cases: Vec<(String, Vec<(gix::ObjectId, gix::ObjectId)>)> =
            vec![("HEAD alone".to_owned(), Vec::new())];
        for pair in stashes.split(',') {
            let (stash, base) = pair.split_once(':').expect("stash:base");
            cases.push((
                format!("a stash {stash} on {base}"),
                vec![(id(stash), id(base))],
            ));
        }
        for (label, stashes) in cases {
            let mut times = Vec::new();
            let (mut opened_walked, mut page_walked) = (0, 0);
            for _ in 0..7 {
                let started = Instant::now();
                let tips = Tips {
                    refs: &refs,
                    stashes: &stashes,
                    reflogs: &[],
                };
                let mut walk = DateOrder::open(
                    repo.inner(),
                    tips,
                    &CancelSignal::new(),
                    &mut |_| {},
                    REPORT_EVERY,
                )
                .unwrap()
                .unwrap();
                opened_walked = walk.walked();
                for _ in 0..64 {
                    match walk.next(&CancelSignal::new()).unwrap() {
                        Popped::Commit { .. } => {}
                        Popped::End | Popped::Cancelled => break,
                    }
                }
                times.push(started.elapsed());
                page_walked = walk.walked();
            }
            times.sort();
            eprintln!(
                "  {label}: 64 rows in {:?} (median of seven); walked {opened_walked} to open, \
                 {page_walked} by the 64th row",
                times[3]
            );
        }
    }

    /// The commits of every local branch, remote-tracking ref and tag, each kind in refname byte
    /// order, then `HEAD`'s, each peeled to a commit; a ref naming no commit is left out.
    fn every_ref_in_gits_order(repo: &gix::Repository) -> Vec<gix::ObjectId> {
        let mut tips = Vec::new();
        let platform = repo.references().unwrap();
        for prefix in ["refs/heads/", "refs/remotes/", "refs/tags/"] {
            let mut named: Vec<(Vec<u8>, gix::ObjectId)> = platform
                .prefixed(prefix)
                .unwrap()
                .filter_map(Result::ok)
                .filter_map(|mut reference| {
                    let name = reference.name().as_bstr().to_vec();
                    let id = reference.peel_to_id().ok()?.detach();
                    Some((name, id))
                })
                .collect();
            named.sort();
            tips.extend(named.into_iter().map(|(_, id)| id));
        }
        if let Ok(head) = repo.head_id() {
            tips.push(head.detach());
        }
        tips.retain(|tip| {
            repo.find_header(*tip)
                .is_ok_and(|header| header.kind() == gix::object::Kind::Commit)
        });
        tips
    }

    fn status_kib(field: &str) -> u64 {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|status| {
                status
                    .lines()
                    .find(|line| line.starts_with(field))
                    .and_then(|line| line.split_whitespace().nth(1)?.parse().ok())
            })
            .unwrap_or(0)
    }

    fn resident_kib() -> u64 {
        status_kib("VmRSS:")
    }

    fn peak_resident_kib() -> u64 {
        status_kib("VmHWM:")
    }
}
