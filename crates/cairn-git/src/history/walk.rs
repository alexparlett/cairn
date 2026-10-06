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

use std::cell::Cell;

use crate::shallow::ShallowBoundary;
use crate::{Cancel, Error};

use super::HistoryOrder;

/// gitoxide's traversal over [`Grafted`] reads, yielding detached walk entries.
pub(super) type CommitWalk<'repo> =
    gix::traverse::commit::Simple<Grafted<'repo>, fn(&gix::oid) -> bool>;

/// A walk from `tips` in `order`, with the repository's commit-graph where git would use one.
pub(super) fn open<'repo>(
    repo: &'repo gix::Repository,
    tips: &[gix::hash::ObjectId],
    order: HistoryOrder,
) -> Result<CommitWalk<'repo>, Error> {
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
    gix::traverse::commit::Simple::new(tips.iter().copied(), objects)
        .sorting(order.sorting())
        .map(|walk| walk.commit_graph(graph))
        .map_err(|e| walk_error(Box::new(e)))
}

/// A walk from `tip` that leaves out every commit `hidden` reaches — `git rev-list
/// <tip> ^<hidden>` — over the commits git's walk sees ([`Grafted`]). Every read goes
/// through [`Polled`], which fails once `cancel` says so: the frontier gix paints before a
/// hiding walk's first commit is one long call, and a read that fails is what stops it.
/// So no commit-graph is used here, though git would use one: a commit read from the
/// graph is not read through `objects`, and that paint could not be stopped. `reads`
/// counts every object read.
pub(crate) fn hiding<'a, C: Cancel>(
    repo: &'a gix::Repository,
    tip: gix::hash::ObjectId,
    hidden: gix::hash::ObjectId,
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
        .hide([hidden])
        .map_err(|e| walk_error(Box::new(e)))
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
pub(super) struct Grafted<'repo> {
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
}
