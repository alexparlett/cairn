//! The changes query: which commits to compare, read by gix; what changed between them,
//! answered by `git diff-tree` (decision E); and the answer put in a total order.
//!
//! A root commit is shown as the user's own `git log` and `git show` show it: with its
//! whole content when `log.showRoot` is true, which is git's default, and with no diff at
//! all when it is false — and so is a shallow clone's boundary commit, which git shows as a
//! root. Plumbing reads `log.showRoot` no more than it reads `diff.renames`, so it is read
//! here; a comparison of two commits is `git diff`'s answer, which does not read it.

use cairn_model::{ChangeSet, ChangedFile, CommitDetails, Oid, RepoPath};

use crate::object_id::{model_id, object_id};
use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

use super::git_config::{invalid, last_value, parse_bool};
use super::renames::Configured;
use super::submodules::Hiding;
use super::{ChangesRequest, Subject};
use crate::reads::{Detection, Submodules};

pub(super) fn changes(
    git: &GitBinary,
    repo: &Repository,
    request: &ChangesRequest,
    cancel: &impl Cancel,
) -> Result<ChangeSet, Error> {
    let inner = repo.inner();
    let (old, new, details) = subject(inner, request)?;

    let search = Configured::read(inner)?.search(git.version());
    // Read before anything is answered: `git log` refuses a value it does not accept
    // whatever the commit changed.
    let hiding = Hiding::read(inner, repo.workdir().is_some())?;
    // `log.showRoot` is read for every commit, as `git log` reads it — a value git refuses
    // is refused whether or not the commit is a root — and never for a comparison.
    let root_hidden = match &details {
        Some(details) => !shows_root_diff(inner)? && details.parents.is_empty(),
        None => false,
    };
    if root_hidden {
        if cancel.is_cancelled() {
            return Err(Error::ChangesCancelled { changed: 0 });
        }
        return Ok(ChangeSet {
            files: Vec::new(),
            renames: search.outcome(&[]),
            details,
        });
    }
    let first = match hiding {
        Hiding::EveryGitlink => Submodules::HideEvery,
        Hiding::Nothing | Hiding::GitlinksExcept(_) => Submodules::AsListed,
    };
    let detection = search.detection();
    let mut files = crate::reads::changes(git, repo, &old, &new, detection, first, cancel)?;
    if matches!(hiding, Hiding::GitlinksExcept(_)) {
        files = hide_submodules(git, repo, (&old, &new), detection, &hiding, files, cancel)?;
    }

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

/// What a request compares: the two commits `git` is given — the one commit's first parent
/// or, for a root commit or a shallow clone's boundary, the empty tree — and, for one
/// commit, its details. Each id is read as a commit, so a missing one is
/// [`Error::ReadCommit`] and git is never handed a tree or a blob as a commit.
pub(super) fn subject(
    inner: &gix::Repository,
    request: &ChangesRequest,
) -> Result<(Oid, Oid, Option<CommitDetails>), Error> {
    // Each id is read as a commit here first, so a missing one is `ReadCommit` — the same
    // answer whichever side it is on — and git is never handed a tree or a blob as a commit.
    Ok(match &request.subject {
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
    })
}

/// The answer with the gitlinks `hiding` hides taken out, as the user's `git log` never
/// queued them (`diff/submodules.rs`). With detection off a gitlink can only be its own
/// row, so dropping those rows is exact. With detection on, a hidden gitlink still counted
/// against `diff.renameLimit` in this answer, so the query is asked again with each hidden
/// path excluded by a pathspec — unless one of them is also a directory another changed
/// path sits under (a submodule replaced by a directory of the same name), which a
/// pathspec cannot exclude without the paths under it; then the rows are dropped, and the
/// residual is stated in `docs/systems/diff.md`.
fn hide_submodules(
    git: &GitBinary,
    repo: &Repository,
    (old, new): (&Oid, &Oid),
    detection: Detection,
    hiding: &Hiding,
    files: Vec<ChangedFile>,
    cancel: &impl Cancel,
) -> Result<Vec<ChangedFile>, Error> {
    let mut hidden: Vec<RepoPath> = files
        .iter()
        .flat_map(|file| hiding.hidden_paths(file))
        .cloned()
        .collect();
    if hidden.is_empty() {
        return Ok(files);
    }
    hidden.sort();
    hidden.dedup();
    let encloses_another = hidden.iter().any(|path| {
        let mut directory = path.as_bytes().to_vec();
        directory.push(b'/');
        files.iter().any(|file| {
            file.old_path.as_bytes().starts_with(&directory)
                || file.new_path.as_bytes().starts_with(&directory)
        })
    });
    if detection == Detection::Off || encloses_another {
        return Ok(files
            .into_iter()
            .filter(|file| hiding.hidden_paths(file).is_empty())
            .collect());
    }
    crate::reads::changes(
        git,
        repo,
        old,
        new,
        detection,
        Submodules::Excluding(&hidden),
        cancel,
    )
}

/// `log.showRoot`, as `git log` reads it (`git_config_bool`): true unless the user set it
/// false, and [`Error::InvalidConfig`] for a value git refuses.
fn shows_root_diff(repo: &gix::Repository) -> Result<bool, Error> {
    let config = repo.config_snapshot();
    match last_value(config.plumbing(), "log", None, "showRoot") {
        None => Ok(true),
        Some(value) => parse_bool(value.as_ref().map(|value| value.as_slice()))
            .ok_or_else(|| invalid("log.showRoot", value)),
    }
}

fn find_commit<'repo>(repo: &'repo gix::Repository, id: &Oid) -> Result<gix::Commit<'repo>, Error> {
    repo.find_commit(object_id(id)?)
        .map_err(|source| Error::ReadCommit {
            id: id.to_string(),
            source: Box::new(source),
        })
}
