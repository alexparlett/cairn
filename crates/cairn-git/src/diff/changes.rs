//! The changes query: which commits to compare, read by gix; what changed between them,
//! answered by `git diff-tree` (decision E); and the answer put in a total order.

use cairn_model::Oid;

use crate::object_id::{model_id, object_id};
use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

use super::renames::Configured;
use super::{ChangeSet, ChangesRequest, Subject};

pub(super) fn changes(
    git: &GitBinary,
    repo: &Repository,
    request: &ChangesRequest,
    cancel: &impl Cancel,
) -> Result<ChangeSet, Error> {
    let inner = repo.inner();
    // Each id is read as a commit here first, so a missing one is `ReadCommit` — the same
    // answer whichever side it is on — and git is never handed a tree or a blob as a commit.
    let (old, new, details) = match &request.subject {
        Subject::Commit(id) => {
            let commit = find_commit(inner, id)?;
            let details = crate::commit::details_of(&commit, id)?;
            // A root commit is compared with the empty tree (L5), which makes its diff the
            // whole of its content rather than nothing at all — and so is a shallow
            // clone's boundary commit, whose parents the clone does not have and which
            // its details list none of, as git's own `git log` shows it (`details_of`).
            // A merge is compared with its first parent, like any other commit.
            let old = match details.parents.first() {
                Some(parent) => *parent,
                None => model_id(&gix::ObjectId::empty_tree(inner.object_hash()))?,
            };
            (old, *id, Some(details))
        }
        Subject::Between { old, new } => {
            find_commit(inner, old)?;
            find_commit(inner, new)?;
            (*old, *new, None)
        }
    };

    let search = Configured::read(inner)?.search(git.version());
    let mut files = crate::reads::changes(git, repo, &old, &new, search.detection(), cancel)?;

    // git lists paths in its own tree order, with a pair under its destination. The answer
    // is sorted here by a key that is total — a destination path, then the source it came
    // from — so the list cannot shuffle between two runs of one query.
    files.sort_by(|left, right| {
        left.new_path
            .cmp(&right.new_path)
            .then_with(|| left.old_path.cmp(&right.old_path))
    });

    Ok(ChangeSet {
        renames: search.outcome(&files),
        files,
        details,
    })
}

fn find_commit<'repo>(repo: &'repo gix::Repository, id: &Oid) -> Result<gix::Commit<'repo>, Error> {
    repo.find_commit(object_id(id)?)
        .map_err(|source| Error::ReadCommit {
            id: id.to_string(),
            source: Box::new(source),
        })
}
