//! The commit walk both history routes run: gitoxide's own traversal, over the parents git
//! shows.
//!
//! A shallow clone's boundary commits — the ones its `shallow` file lists — name parents in
//! their objects that the clone cut off. git reads the file as grafts: each such commit has
//! NO parents, so `git log --format=%P` prints nothing for it, `git log --graph` draws it
//! as a root, and a parent the clone does have is still shown when another commit reaches
//! it. gix's `rev_walk` treats the file differently (`gix::revision::walk::Platform::selected`
//! in 0.87.1): the next time a cut-off parent's id comes up it is skipped, whichever commit
//! names it, so a parent the clone HAS, reached through a sibling branch, vanishes from the
//! walk; and the boundary commit's `parent_ids` still name what the clone lacks, which would
//! hold a lane of the graph open for a commit that never arrives.
//!
//! So the walk here is `gix::traverse::commit::Simple` itself, reading every commit through
//! [`Grafted`], which hands a boundary commit over with its parent lines removed: git's
//! graft, applied where the object is read, so the walk neither lists nor follows a parent
//! git would not. git also never reads a commit-graph in a shallow repository
//! (`commit_graph_compatible` in git's `commit-graph.c`), and a graph written before the
//! repository became shallow would name the cut-off parents, so neither does this.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Write as _;
use std::rc::Rc;

use gix::objs::FindExt as _;

use crate::shallow::ShallowBoundary;
use crate::{Cancel, Error};

use super::HistoryOrder;

/// gitoxide's traversal over [`Grafted`] reads, yielding detached walk entries.
pub(crate) type CommitWalk<'repo> =
    gix::traverse::commit::Simple<Seeded<'repo>, fn(&gix::oid) -> bool>;

/// A walk from `tips` in `order`, with the repository's commit-graph where git would use one;
/// `None` when `cancel` fired while the tips were read, nothing kept.
///
/// In commit-time order gitoxide reads every tip's commit for its committer date before
/// the walk's first step (`Simple::sorting`), in one call that polls nothing — a read per
/// tip, and a walk from every ref has one tip per ref. So the tips are read here instead,
/// `cancel` polled before each but the first (a walk from one tip polls as it always did,
/// once per commit laid out), and gitoxide's own read of each tip is then answered from
/// the dates read ([`Seeded`]), costing no object read. Graph order reads no tip to open.
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
/// git's own walk takes it (`fill_commit_in_graph`), and read from its object otherwise
/// (staging-and-commit phase 10's decision A).
pub(crate) fn open_dated<'repo>(
    repo: &'repo gix::Repository,
    tips: &[gix::hash::ObjectId],
    order: HistoryOrder,
    known: HashMap<gix::hash::ObjectId, i64>,
    cancel: &impl Cancel,
) -> Result<Option<CommitWalk<'repo>>, Error> {
    let walk_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Walk { source };
    let boundary = ShallowBoundary::read(repo).map_err(|e| walk_error(Box::new(e)))?;
    // As gix's own walk decides it: an invalid `core.commitGraph` is an error, and a graph
    // that will not open is walked without, from the objects themselves.
    let graph = if boundary.is_shallow() {
        None
    } else {
        match repo.commit_graph_if_enabled() {
            Ok(graph) => graph,
            Err(gix::repository::commit_graph_if_enabled::Error::OpenCommitGraph(_)) => None,
            Err(other) => return Err(walk_error(Box::new(other))),
        }
    };
    let objects = Grafted {
        objects: &repo.objects,
        boundary,
    };
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
        .sorting(order.sorting())
        .map_err(|e| walk_error(Box::new(e)))?;
    // Every later read is the object's own.
    drop(dates.take());
    Ok(Some(walk.commit_graph(graph)))
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
    let boundary = ShallowBoundary::read(repo).map_err(|e| walk_error(Box::new(e)))?;
    let objects = Polled {
        objects: Grafted {
            objects: &repo.objects,
            boundary,
        },
        cancel,
        reads,
    };
    gix::traverse::commit::Simple::new([tip], objects)
        .hide(hidden)
        .map_err(|e| walk_error(Box::new(e)))
}

/// Whether any of `hidden` reaches `tip`, answered from the commit-graph alone, as git's
/// `repo_is_descendant_of` answers it with generation numbers (staging-and-commit phase 05's QA
/// item 4): a walk down from the hidden tips through the graph's parents that never descends
/// below `tip`'s generation, since a commit of a lower generation cannot reach it. No object is
/// read, `cancel` is polled before each commit the walk visits, and what it visited is counted
/// in `visited`. `None` — answer by the object walk ([`hiding`]) instead — where the repository
/// is shallow, no graph is enabled or opens, or the graph does not hold `tip` and every hidden
/// tip. `Some(Err(..))` once cancelled, or on a graph that does not parse.
pub(crate) fn reaches_through_graph<C: Cancel>(
    repo: &gix::Repository,
    tip: gix::hash::ObjectId,
    hidden: &[gix::hash::ObjectId],
    cancel: &C,
    visited: &Cell<usize>,
) -> Option<Result<bool, Error>> {
    if repo.is_shallow() {
        return None;
    }
    let graph = repo.commit_graph_if_enabled().ok().flatten()?;
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

/// The object database as git's walk sees it in a shallow clone: a boundary commit reads
/// with no parents. Every other object, and every other commit, is the stored bytes.
pub(crate) struct Grafted<'repo> {
    objects: &'repo gix::OdbHandle,
    boundary: ShallowBoundary,
}

impl gix::objs::Find for Grafted<'_> {
    fn try_find<'a>(
        &self,
        id: &gix::oid,
        buffer: &'a mut Vec<u8>,
    ) -> Result<Option<gix::objs::Data<'a>>, gix::objs::find::Error> {
        if !self.boundary.cuts(id) {
            return self.objects.try_find(id, buffer);
        }
        let (kind, object_hash, bytes) = match self.objects.try_find(id, buffer)? {
            None => return Ok(None),
            Some(found) if found.kind == gix::objs::Kind::Commit => {
                (found.kind, found.object_hash, without_parents(found.data))
            }
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

/// A commit object's bytes with every `parent` line of its header removed, and nothing
/// else touched. The header ends at the first empty line; a `parent` line can only be in
/// it, and a continuation line of a multi-line header (a signature) starts with a space.
fn without_parents(commit: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(commit.len());
    let mut header = true;
    for line in commit.split_inclusive(|byte| *byte == b'\n') {
        if header && line == b"\n" {
            header = false;
        }
        if header && line.starts_with(b"parent ") {
            continue;
        }
        out.extend_from_slice(line);
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
            String::from_utf8_lossy(&without_parents(commit)),
            String::from_utf8_lossy(expected)
        );
        let root = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\nauthor A <a@example.com> 1 +0000\n\nmsg";
        assert_eq!(without_parents(root), root.to_vec(), "a root is untouched");
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

    /// Phase 11's QA (TC1): the graph answers alone exactly when it holds the tip and every
    /// hidden tip — true where a hidden tip reaches the tip, false where none does — and steps
    /// aside (`None`) when it holds the tip but not a hidden tip, or not the tip, so the object
    /// walk answers. Caught by: hidden tips the graph lacks skipped rather than handing over (a
    /// pushed `HEAD` called unpublished), or the graph route never taken.
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
}
