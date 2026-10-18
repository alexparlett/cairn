//! The commit walks the history routes run: gitoxide's own traversal in commit-time or graph
//! order, and git's date order (`date_order`), over the parents git shows.
//!
//! A shallow clone's boundary commits — the ones its `shallow` file lists — name parents in
//! their objects that the clone cut off. git reads the file as grafts: each such commit has
//! NO parents, so `git log --format=%P` prints nothing for it, `git log --graph` draws it
//! as a root, and a parent the clone does have is still shown when another commit reaches
//! it. gix's `rev_walk` treats the file differently (`gix::revision::walk::Platform::selected`
//! in 0.87.1): the next time a cut-off parent's id comes up it is skipped, whichever commit
//! names it, so a parent the clone HAS, reached through a sibling branch, vanishes from the
//! walk; and the boundary commit's `parent_ids` still name what the clone lacks, which would
//! hold a lane of the graph open for a commit that never arrives. A commit the graft file
//! grafts is shown with the graft's parents, and gix reads no graft file (`crate::grafts`).
//!
//! So the walk here is `gix::traverse::commit::Simple` itself, reading every commit through
//! [`Grafted`], which hands a grafted commit over with its parent lines replaced by the
//! graft's: git's graft, applied where the object is read, so the walk neither lists nor
//! follows a parent git would not. A replace ref is applied by gix's object database itself.
//! The commit-graph is read only where git reads one ([`commit_graph`]): its parents are
//! the commits' own, which a graft or a replace ref contradicts.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Write as _;
use std::rc::Rc;

use gix::objs::FindExt as _;

use crate::grafts::Grafts;
use crate::replacements::Replacements;
use crate::{Cancel, Error};

use super::HistoryOrder;
use super::date_order::{self, DateOrder};

/// gitoxide's traversal over [`Grafted`] reads, yielding detached walk entries.
pub(crate) type SimpleWalk<'repo> =
    gix::traverse::commit::Simple<Seeded<'repo>, fn(&gix::oid) -> bool>;

/// A walk in one of the history's orders.
pub(crate) enum CommitWalk<'repo> {
    /// Commit-time or graph order: gitoxide's traversal.
    Simple(Box<SimpleWalk<'repo>>),
    /// git's date order.
    DateOrder(Box<DateOrder<'repo>>),
}

/// One step of a [`CommitWalk`].
#[derive(Debug)]
pub(crate) enum Step {
    Commit(gix::traverse::commit::Info),
    End,
    /// `cancel` fired within the step; the walk is as it was before it, and the next step
    /// carries on.
    Cancelled,
}

impl CommitWalk<'_> {
    /// The next commit. gitoxide's traversal polls nothing; git's date order polls `cancel`
    /// between the commits it reads to know a parent's children (`date_order`).
    pub(crate) fn next(&mut self, cancel: &impl Cancel) -> Result<Step, Error> {
        match self {
            Self::Simple(walk) => match walk.next() {
                None => Ok(Step::End),
                Some(Ok(info)) => Ok(Step::Commit(info)),
                Some(Err(source)) => Err(Error::Walk {
                    source: Box::new(source),
                }),
            },
            Self::DateOrder(walk) => Ok(match walk.next(cancel)? {
                date_order::Popped::Commit { info, .. } => Step::Commit(info),
                date_order::Popped::End => Step::End,
                date_order::Popped::Cancelled => Step::Cancelled,
            }),
        }
    }
}

/// A walk from `tips` in `order`, with the repository's commit-graph where git would use one;
/// `None` when `cancel` fired while the walk opened, nothing kept.
///
/// In commit-time order gitoxide reads every tip's commit for its committer date before
/// the walk's first step (`Simple::sorting`), in one call that polls nothing — a read per
/// tip, and a walk from every ref has one tip per ref. So the tips are read here instead,
/// `cancel` polled before each but the first (a walk from one tip polls as it always did,
/// once per commit laid out), and gitoxide's own read of each tip is then answered from
/// the dates read ([`Seeded`]), costing no object read. Graph order reads no tip to open.
/// Date order's opening pass reads every tip and polls between commits (`date_order`).
pub(crate) fn open<'repo>(
    repo: &'repo gix::Repository,
    tips: &[gix::hash::ObjectId],
    order: HistoryOrder,
    cancel: &impl Cancel,
) -> Result<Option<CommitWalk<'repo>>, Error> {
    open_dated(repo, tips, order, HashMap::new(), cancel)
}

/// [`open`], with the committer dates of some tips known already — read with their ids, as
/// Show Lost Commits' reflog tips are (`reflogs::reflog_tips`), so no tip is read twice. A
/// tip whose date is not known is taken from the commit-graph where the graph holds it, as
/// git's own walk takes it (`fill_commit_in_graph`), and read from its object otherwise.
/// Date order reads its tips itself, each once, and takes no known dates.
pub(crate) fn open_dated<'repo>(
    repo: &'repo gix::Repository,
    tips: &[gix::hash::ObjectId],
    order: HistoryOrder,
    known: HashMap<gix::hash::ObjectId, i64>,
    cancel: &impl Cancel,
) -> Result<Option<CommitWalk<'repo>>, Error> {
    let walk_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Walk { source };
    let sorting = match order.sorting() {
        Some(sorting) => sorting,
        None => {
            let tips = date_order::Tips {
                refs: tips,
                stashes: &[],
                reflogs: &[],
            };
            return Ok(
                DateOrder::open(repo, tips, cancel, &mut |_| {}, date_order::REPORT_EVERY)?
                    .map(|walk| CommitWalk::DateOrder(Box::new(walk))),
            );
        }
    };
    let substitutions = Substitutions::read(repo)?;
    let graph = commit_graph(repo, &substitutions)?;
    let objects = Grafted::new(&repo.objects, substitutions);
    let mut dates = known;
    if matches!(order, HistoryOrder::CommitTime) {
        dates.reserve(tips.len());
        let mut buffer = Vec::new();
        for (read, tip) in tips.iter().enumerate() {
            if read > 0 && cancel.is_cancelled() {
                return Ok(None);
            }
            if dates.contains_key(tip) {
                continue;
            }
            if let Some(date) = graph.as_ref().and_then(|graph| graph_date(graph, tip)) {
                dates.insert(*tip, date);
                continue;
            }
            let commit = objects
                .find_commit_iter(tip, &mut buffer)
                .map_err(|e| walk_error(Box::new(e)))?;
            let committer = commit.committer().map_err(|e| walk_error(Box::new(e)))?;
            dates.insert(*tip, committer.seconds());
        }
    }
    let dates = Rc::new(RefCell::new(dates));
    let seeded = Seeded {
        objects,
        dates: Rc::clone(&dates),
    };
    let walk = gix::traverse::commit::Simple::new(tips.iter().copied(), seeded)
        .sorting(sorting)
        .map_err(|e| walk_error(Box::new(e)))?;
    // Every later read is the object's own.
    drop(dates.take());
    Ok(Some(CommitWalk::Simple(Box::new(walk.commit_graph(graph)))))
}

/// The repository's commit-graph, where git would read one: none where git's
/// `commit_graph_compatible` refuses it — a replace ref git reads, a graft, a shallow
/// repository (v2.30.9 `commit-graph.c:201-217`, v2.56.0
/// `commit-graph.c:223-239`) — or where `core.commitGraph` is off. The graph's parents are
/// each commit's own, which a replace ref or a graft contradicts, and a generation computed
/// over parents git no longer walks would misorder a walk bounded by it. As gix's own walk
/// decides it, an invalid `core.commitGraph` is an error, and a graph that will not open is
/// walked without, from the objects themselves.
pub(crate) fn commit_graph(
    repo: &gix::Repository,
    substitutions: &Substitutions,
) -> Result<Option<gix::commitgraph::Graph>, Error> {
    if !substitutions.grafts.is_empty() || !substitutions.replacements.is_empty() {
        return Ok(None);
    }
    match repo.commit_graph_if_enabled() {
        Ok(graph) => Ok(graph),
        Err(gix::repository::commit_graph_if_enabled::Error::OpenCommitGraph(_)) => Ok(None),
        Err(other) => Err(Error::Walk {
            source: Box::new(other),
        }),
    }
}

/// `id`'s committer date as the commit-graph holds it, when it holds `id`.
pub(crate) fn graph_date(graph: &gix::commitgraph::Graph, id: &gix::hash::oid) -> Option<i64> {
    let commit = graph.commit_by_id(id)?;
    i64::try_from(commit.committer_timestamp()).ok()
}

/// [`Grafted`] reads, except that while a walk opens, a tip whose committer date [`open`]
/// has read is answered with a commit holding that date and nothing else: all gitoxide
/// reads of a tip to order it (`add_to_queue`). Its every later read — its parents, when
/// the walk reaches it — is the stored object, the dates being cleared once the walk is
/// open.
pub(crate) struct Seeded<'repo> {
    objects: Grafted<'repo>,
    dates: Rc<RefCell<HashMap<gix::hash::ObjectId, i64>>>,
}

impl gix::objs::Find for Seeded<'_> {
    fn try_find<'a>(
        &self,
        id: &gix::oid,
        buffer: &'a mut Vec<u8>,
    ) -> Result<Option<gix::objs::Data<'a>>, gix::objs::find::Error> {
        let date = {
            let dates = self.dates.borrow();
            // Empty once the walk is open: every read but the open's.
            if dates.is_empty() {
                None
            } else {
                dates.get(id).copied()
            }
        };
        let Some(seconds) = date else {
            return self.objects.try_find(id, buffer);
        };
        let kind = id.kind();
        buffer.clear();
        let written = writeln!(
            buffer,
            "tree {}\nauthor a <a> {seconds} +0000\ncommitter a <a> {seconds} +0000\n",
            kind.null()
        );
        if let Err(error) = written {
            return Err(Box::new(error));
        }
        Ok(Some(gix::objs::Data {
            kind: gix::objs::Kind::Commit,
            object_hash: kind,
            data: buffer.as_slice(),
        }))
    }
}

/// A walk from `tip` that leaves out every commit any of `hidden` reaches — `git rev-list
/// <tip> ^<hidden>...` — over the commits git's walk sees ([`Grafted`]). Every read goes
/// through [`Polled`], which fails once `cancel` says so: the frontier gix paints before a
/// hiding walk's first commit is one long call, and a read that fails is what stops it.
/// So no commit-graph is used here, though git would use one: a commit read from the
/// graph is not read through `objects`, and that paint could not be stopped. `reads`
/// counts every object read.
pub(crate) fn hiding<'a, C: Cancel>(
    repo: &'a gix::Repository,
    tip: gix::hash::ObjectId,
    hidden: impl IntoIterator<Item = gix::hash::ObjectId>,
    cancel: &'a C,
    reads: &'a Cell<usize>,
) -> Result<HidingWalk<'a, C>, Error> {
    let walk_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Walk { source };
    let objects = Polled {
        objects: Grafted::new(&repo.objects, Substitutions::read(repo)?),
        cancel,
        reads,
    };
    gix::traverse::commit::Simple::new([tip], objects)
        .hide(hidden)
        .map_err(|e| walk_error(Box::new(e)))
}

/// Whether any of `hidden` reaches `tip`, answered from the commit-graph alone, as git's
/// `repo_is_descendant_of` answers it with generation numbers: a walk down from the hidden tips
/// through the graph's parents that never descends below `tip`'s generation, since a commit of
/// a lower generation cannot reach it. No object is read, `cancel` is polled before each
/// commit the walk visits, and what it visited is counted in `visited`. `None` — answer by the
/// object walk ([`hiding`]) instead — where git would read no graph ([`commit_graph`]: a
/// replace ref, a graft or a shallow repository), no graph is enabled or opens, or the graph
/// does not hold `tip` and every hidden tip. `Some(Err(..))` once cancelled, or on a graph
/// that does not parse.
pub(crate) fn reaches_through_graph<C: Cancel>(
    repo: &gix::Repository,
    tip: gix::hash::ObjectId,
    hidden: &[gix::hash::ObjectId],
    cancel: &C,
    visited: &Cell<usize>,
) -> Option<Result<bool, Error>> {
    let substitutions = Substitutions::read(repo).ok()?;
    let graph = commit_graph(repo, &substitutions).ok().flatten()?;
    let target = graph.lookup(tip)?;
    let floor = graph.commit_at(target).generation();
    let starts: Vec<gix::commitgraph::Position> = hidden
        .iter()
        .map(|id| graph.lookup(id))
        .collect::<Option<_>>()?;
    let mut seen = vec![false; graph.num_commits() as usize];
    let mut stack = starts;
    while let Some(at) = stack.pop() {
        let Some(mark) = seen.get_mut(at.0 as usize) else {
            continue;
        };
        if std::mem::replace(mark, true) {
            continue;
        }
        if cancel.is_cancelled() {
            return Some(Err(Error::Cancelled {
                walked: visited.get(),
            }));
        }
        visited.set(visited.get() + 1);
        if at == target {
            return Some(Ok(true));
        }
        let commit = graph.commit_at(at);
        // A commit below the tip's generation cannot reach it (generations only fall along a
        // parent edge), so nothing under it is walked.
        if commit.generation() < floor {
            continue;
        }
        for parent in commit.iter_parents() {
            match parent {
                Ok(parent) => stack.push(parent),
                Err(source) => {
                    return Some(Err(Error::Walk {
                        source: Box::new(source),
                    }));
                }
            }
        }
    }
    Some(Ok(false))
}

/// See [`hiding`].
pub(crate) type HidingWalk<'a, C> =
    gix::traverse::commit::Simple<Polled<'a, C>, fn(&gix::oid) -> bool>;

/// [`Grafted`] reads that fail once `cancel` says so, each counted in `reads`.
pub(crate) struct Polled<'a, C> {
    objects: Grafted<'a>,
    cancel: &'a C,
    reads: &'a Cell<usize>,
}

/// What a read fails with once its walk is cancelled.
#[derive(Debug, thiserror::Error)]
#[error("the walk was cancelled")]
struct WalkCancelled;

impl<C: Cancel> gix::objs::Find for Polled<'_, C> {
    fn try_find<'a>(
        &self,
        id: &gix::oid,
        buffer: &'a mut Vec<u8>,
    ) -> Result<Option<gix::objs::Data<'a>>, gix::objs::find::Error> {
        if self.cancel.is_cancelled() {
            return Err(Box::new(WalkCancelled));
        }
        self.reads.set(self.reads.get() + 1);
        self.objects.try_find(id, buffer)
    }
}

/// The object database as git's walk sees it: an object a replace ref replaces reads as its
/// replacement (`crate::replacements`), and a grafted commit — a shallow clone's boundary,
/// or one the graft file names — reads with the graft's parents in place of its own (none at
/// a shallow boundary), the graft found by the commit's own id, as git's
/// `parse_commit_buffer` looks it up over the replacement's bytes. Every other object is the
/// stored bytes. Its handle is its own, told to read no replacement of gitoxide's (which
/// reads `core.useReplaceRefs` the wrong way round).
pub(crate) struct Grafted<'repo> {
    objects: gix::OdbHandle,
    grafts: Grafts,
    replacements: Replacements,
    /// The repository whose objects these are: a walk is held no longer than it.
    repo: std::marker::PhantomData<&'repo gix::OdbHandle>,
}

impl<'repo> Grafted<'repo> {
    pub(crate) fn new(objects: &'repo gix::OdbHandle, substitutions: Substitutions) -> Self {
        let mut objects = objects.clone();
        objects.ignore_replacements = true;
        let Substitutions {
            grafts,
            replacements,
        } = substitutions;
        Self {
            objects,
            grafts,
            replacements,
            repo: std::marker::PhantomData,
        }
    }

    /// [`Grafted::new`], keeping no object cache: for a walk that reads each commit once and
    /// keeps what it read, where filling a cache is only a cost.
    pub(crate) fn uncached(objects: &'repo gix::OdbHandle, substitutions: Substitutions) -> Self {
        let mut grafted = Self::new(objects, substitutions);
        grafted.objects.unset_object_cache();
        grafted
    }
}

/// What git reads in place of what is stored, read once for one walk: the grafts and the
/// replace refs.
pub(crate) struct Substitutions {
    pub(crate) grafts: Grafts,
    pub(crate) replacements: Replacements,
}

impl Substitutions {
    pub(crate) fn read(repo: &gix::Repository) -> Result<Self, Error> {
        let walk_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Walk { source };
        Ok(Self {
            grafts: Grafts::read(repo).map_err(|e| walk_error(Box::new(e)))?,
            replacements: Replacements::read(repo).map_err(walk_error)?,
        })
    }
}

impl gix::objs::Find for Grafted<'_> {
    fn try_find<'a>(
        &self,
        id: &gix::oid,
        buffer: &'a mut Vec<u8>,
    ) -> Result<Option<gix::objs::Data<'a>>, gix::objs::find::Error> {
        let stored = self.replacements.of(id).map_err(Box::new)?;
        let Some(parents) = self.grafts.of(id) else {
            return self.objects.try_find(&stored, buffer);
        };
        let (kind, object_hash, bytes) = match self.objects.try_find(&stored, buffer)? {
            None => return Ok(None),
            Some(found) if found.kind == gix::objs::Kind::Commit => (
                found.kind,
                found.object_hash,
                with_parents(found.data, parents),
            ),
            Some(found) => (found.kind, found.object_hash, found.data.to_vec()),
        };
        *buffer = bytes;
        Ok(Some(gix::objs::Data {
            kind,
            object_hash,
            data: buffer.as_slice(),
        }))
    }
}

/// A commit object's bytes with every `parent` line of its header replaced by one per id of
/// `parents`, in their order, after the `tree` line, and nothing else touched. The header
/// ends at the first empty line; a `parent` line can only be in it, and a continuation line
/// of a multi-line header (a signature) starts with a space.
fn with_parents(commit: &[u8], parents: &[gix::ObjectId]) -> Vec<u8> {
    let mut out = Vec::with_capacity(commit.len() + parents.len() * 72);
    let mut header = true;
    for line in commit.split_inclusive(|byte| *byte == b'\n') {
        if header && line == b"\n" {
            header = false;
        }
        if header && line.starts_with(b"parent ") {
            continue;
        }
        out.extend_from_slice(line);
        if header && line.starts_with(b"tree ") {
            for parent in parents {
                out.extend_from_slice(b"parent ");
                out.extend_from_slice(parent.to_hex().to_string().as_bytes());
                out.push(b'\n');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a parent line left in, a line of the message dropped because it reads
    /// like one, or a signature's continuation lines disturbed.
    #[test]
    fn only_the_headers_parent_lines_are_removed() {
        let commit = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
parent 1111111111111111111111111111111111111111\n\
parent 2222222222222222222222222222222222222222\n\
author A <a@example.com> 1600000000 +0000\n\
committer C <c@example.com> 1600000000 +0000\n\
gpgsig -----BEGIN PGP SIGNATURE-----\n \n parent of nothing\n -----END PGP SIGNATURE-----\n\
\n\
subject\n\
\n\
parent 3333333333333333333333333333333333333333\n";
        let expected = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
author A <a@example.com> 1600000000 +0000\n\
committer C <c@example.com> 1600000000 +0000\n\
gpgsig -----BEGIN PGP SIGNATURE-----\n \n parent of nothing\n -----END PGP SIGNATURE-----\n\
\n\
subject\n\
\n\
parent 3333333333333333333333333333333333333333\n";
        assert_eq!(
            String::from_utf8_lossy(&with_parents(commit, &[])),
            String::from_utf8_lossy(expected)
        );
        let root = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\nauthor A <a@example.com> 1 +0000\n\nmsg";
        assert_eq!(
            with_parents(root, &[]),
            root.to_vec(),
            "a root is untouched"
        );
    }

    /// A graft's parents go in after the `tree` line, in the graft's order, in place of the
    /// commit's own. Caught by: the commit's parents kept beside the graft's, the graft's put
    /// after `author` where no reader looks for a parent, or their order changed.
    #[test]
    fn a_grafts_parents_replace_the_commits_own() {
        let commit = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
parent 1111111111111111111111111111111111111111\n\
author A <a@example.com> 1600000000 +0000\n\
committer C <c@example.com> 1600000000 +0000\n\
\n\
subject\n";
        let graft = |hex: &str| gix::ObjectId::from_hex(hex.as_bytes()).unwrap();
        let parents = [
            graft("3333333333333333333333333333333333333333"),
            graft("2222222222222222222222222222222222222222"),
        ];
        let expected = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\
parent 3333333333333333333333333333333333333333\n\
parent 2222222222222222222222222222222222222222\n\
author A <a@example.com> 1600000000 +0000\n\
committer C <c@example.com> 1600000000 +0000\n\
\n\
subject\n";
        assert_eq!(
            String::from_utf8_lossy(&with_parents(commit, &parents)),
            String::from_utf8_lossy(expected)
        );
    }

    /// A repository of its own under the temporary directory, removed when dropped.
    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn git(dir: &std::path::Path, args: &[&str]) -> String {
        // The git Cairn would find, so the fixture is made by the binary it runs.
        let program = crate::ops::GitBinary::discover(&crate::ops::Askpass::new(
            "/nonexistent/cairn-askpass",
            None,
        ))
        .unwrap();
        let output = std::process::Command::new(program.path())
            .args(["-c", "user.name=A", "-c", "user.email=a@example.com"])
            .args(["-c", "maintenance.auto=false", "-c", "gc.auto=0"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// The graph answers alone exactly when it holds the tip and every hidden tip — true where
    /// a hidden tip reaches the tip, false where none does — and steps aside (`None`) when it
    /// holds the tip but not a hidden tip, or not the tip, so the object walk answers. Caught
    /// by: hidden tips the graph lacks skipped rather than handing over (a pushed `HEAD` called
    /// unpublished), or the graph route never taken.
    #[test]
    fn the_graph_answers_only_for_commits_it_holds_all_of() {
        let dir = std::env::temp_dir().join(format!("cairn-graph-reach-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let scratch = Scratch(dir);
        let at = scratch.0.as_path();
        git(at, &["init", "-q", "-b", "main"]);
        let mut commits = Vec::new();
        for n in 0..4 {
            git(
                at,
                &["commit", "-q", "--allow-empty", "-m", &format!("c{n}")],
            );
            commits.push(git(at, &["rev-parse", "HEAD"]));
        }
        git(at, &["checkout", "-q", "-b", "side", &commits[1]]);
        git(at, &["commit", "-q", "--allow-empty", "-m", "side"]);
        let side = git(at, &["rev-parse", "HEAD"]);
        git(at, &["commit-graph", "write", "--reachable"]);
        git(
            at,
            &["commit", "-q", "--allow-empty", "-m", "outside the graph"],
        );
        let outside = git(at, &["rev-parse", "HEAD"]);
        let repo = crate::SharedRepository::discover(at).unwrap().to_worker();
        let id = |hex: &str| gix::hash::ObjectId::from_hex(hex.as_bytes()).unwrap();
        let ask = |tip: &str, hidden: &[&str]| {
            let hidden: Vec<_> = hidden.iter().map(|hex| id(hex)).collect();
            reaches_through_graph(
                repo.inner(),
                id(tip),
                &hidden,
                &crate::CancelSignal::new(),
                &Cell::new(0),
            )
            .map(|answer| answer.unwrap())
        };
        assert_eq!(ask(&commits[0], &[&commits[3]]), Some(true));
        assert_eq!(ask(&commits[3], &[&commits[0]]), Some(false));
        assert_eq!(ask(&side, &[&commits[3]]), Some(false), "a side branch");
        assert_eq!(ask(&commits[1], &[&commits[3], &side]), Some(true));
        assert_eq!(
            ask(&side, &[&commits[3], &outside]),
            None,
            "a hidden tip the graph does not hold"
        );
        assert_eq!(
            ask(&outside, &[&commits[3]]),
            None,
            "a tip it does not hold"
        );
    }

    /// Where git would not read the commit-graph — a replace ref, a graft — the graph does not
    /// answer, and the object walk, which reads the replaced and grafted parents, answers as
    /// git does: `c3`'s parent replaced by `side` cuts `c1` and `c2` off it. Caught by: the
    /// graph read whatever git reads, which still has `c3` reach `c1`.
    #[test]
    fn the_graph_does_not_answer_where_git_would_not_read_it() {
        let dir = std::env::temp_dir().join(format!("cairn-graph-subst-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let scratch = Scratch(dir);
        let at = scratch.0.as_path();
        git(at, &["init", "-q", "-b", "main"]);
        let mut commits = Vec::new();
        for n in 0..4 {
            git(
                at,
                &["commit", "-q", "--allow-empty", "-m", &format!("c{n}")],
            );
            commits.push(git(at, &["rev-parse", "HEAD"]));
        }
        git(at, &["checkout", "-q", "-b", "side", &commits[0]]);
        git(at, &["commit", "-q", "--allow-empty", "-m", "side"]);
        let side = git(at, &["rev-parse", "HEAD"]);
        git(at, &["commit-graph", "write", "--reachable"]);
        let id = |hex: &str| gix::hash::ObjectId::from_hex(hex.as_bytes()).unwrap();
        let reaches = |hidden: &str| {
            let repo = crate::SharedRepository::discover(at).unwrap().to_worker();
            let cancel = crate::CancelSignal::new();
            let reads = Cell::new(0);
            let graph = reaches_through_graph(
                repo.inner(),
                id(&commits[1]),
                &[id(hidden)],
                &cancel,
                &reads,
            )
            .map(|answer| answer.unwrap());
            let walked = hiding(repo.inner(), id(&commits[1]), [id(hidden)], &cancel, &reads)
                .unwrap()
                .count();
            (graph, walked == 0)
        };
        assert_eq!(reaches(&commits[3]), (Some(true), true), "no substitution");

        git(at, &["replace", "--graft", &commits[3], &side]);
        assert_eq!(reaches(&commits[3]), (None, false), "a replace ref");
        git(at, &["replace", "-d", &commits[3]]);

        std::fs::write(
            at.join(".git/info/grafts"),
            format!("{} {side}\n", commits[3]),
        )
        .unwrap();
        assert_eq!(reaches(&commits[3]), (None, false), "a graft");
    }
}
