//! Create Branch's checkout (`docs/prd/staging-and-commit.md` R11.3; the user's decision,
//! 2026-10-09): a branch created at a commit and checked out, with the working tree's changes
//! kept — "Don't change", `git checkout -q -b <name> <id> --`, which git refuses, writing
//! nothing, where a change or an untracked file would be overwritten — or discarded —
//! "Discard", `git checkout -q -f -b <name> <id> --`, destructive and sealed: its
//! [`Consequence::CheckoutDiscarding`] names every staged and unstaged change to a tracked file
//! it throws away and every untracked file the commit's tree overwrites, and is computed again
//! and compared before git runs (R1.4). It is the one operation that discards a staged change
//! (R1.5's stated exception).
//!
//! The name is `-b`'s value, which git reads as the name whatever it begins with; the commit is
//! its full id, and the `--` after it says it is no path. Checking out is this packet's only
//! through Create Branch; checking out a branch is branch-ops'.
//!
//! The untracked files are read whatever `status.showUntrackedFiles` says
//! (`reads::untracked_paths`), and every proper prefix of each is looked up in the commit's
//! tree: a file the tree holds at the path is overwritten; untracked files under a directory
//! the tree holds a file at are deleted with that directory; a repository nested in the working
//! tree where the tree holds anything is deleted whole — each named in the `Consequence` with
//! what is under it (phase 10's QA, items 1-3).
//!
//! What the seal does not count: an ignored file or directory at a path the commit holds a
//! file at, which git deletes on any checkout, kept or discarding, as the user's own `git
//! checkout` does — a whole ignored directory included; and a submodule's or a conflicted
//! path's state, which the discard refuses before any prompt. A directory or nested repository
//! named lost is compared again by the count and the bytes of what is under it, so an edit that
//! keeps a file's size inside it is not seen by the re-check.

use std::collections::BTreeMap;
use std::ffi::OsString;

use cairn_model::{
    AskpassToken, ChangeLoss, ChangedKind, Confirmed, Consequence, LostChange, Oid, RemovedKind,
    RepoPath, StagedChange, StatusEntry, UnstagedChange, WorkingTreeStatus,
};

use super::fresh_state::{IndexNow, IndexSide, on_disk};
use super::local_write::{locks_around, locks_now, run};
use super::{GitBinary, Invalidated, Performed};
use crate::object_id::{model_id, object_id};
use crate::{Cancel, CheckoutRefusal, Error, Refusal, Repository};

/// Creates the branch `name` at `commit` and checks it out, keeping the working tree's changes:
/// `git checkout -q -b <name> <id> --`, a write. git refuses — creating no branch — where a
/// change or an untracked file would be overwritten, and its words are the failure's
/// ([`Error::GitFailed`]). Invalidates the refs, the index and the working tree.
pub fn create_branch_and_checkout(
    git: &GitBinary,
    repo: &Repository,
    name: &str,
    commit: Oid,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let before = locks_now(repo);
    run(
        git,
        repo,
        token,
        &checkout_arguments(name, commit, false),
        None,
    )?;
    Ok(Performed::new(
        format!(
            "created and checked out branch {name} at {}",
            commit.short()
        ),
        checkout_invalidates(),
    )
    .with_locks(locks_around(repo, before)))
}

/// What creating `branch` at `at` and checking it out with `git checkout -f` would lose, read
/// now: every tracked path with a staged or an unstaged change, and every untracked file at a
/// path `at`'s tree holds; the rest of the untracked files counted as kept. Refused before any
/// prompt during a merge, a rebase, `git am`, a cherry-pick or a revert, for a conflicted path
/// or a submodule with changes, and when nothing is changed. Runs `git status` and two numstat
/// reads; `cancel` stops them.
pub fn checkout_discarding_consequence(
    git: &GitBinary,
    repo: &Repository,
    branch: &str,
    at: Oid,
    cancel: &impl Cancel,
) -> Result<Consequence, Error> {
    if let Some(operation) = repo.operation_in_progress() {
        return Err(Error::CheckoutRefused {
            why: CheckoutRefusal::InProgress(operation),
        });
    }
    let head = head_commit(repo)?;
    let entries = match repo.status(git, cancel)? {
        WorkingTreeStatus::Listed(entries) => entries,
        WorkingTreeStatus::IndexUnreadable(_) | WorkingTreeStatus::NoWorkingTree => {
            return Err(Error::CheckoutRefused {
                why: CheckoutRefusal::NothingToDiscard,
            });
        }
    };
    let lines = crate::reads::change_lines(git, repo, head, cancel)?;
    let index = IndexNow::read(repo)?;
    let target = target_tree(repo, at)?;
    // By path, so the prompt and the re-check read them in one order.
    let mut kinds: BTreeMap<RepoPath, ChangedKind> = BTreeMap::new();
    for entry in entries {
        match entry {
            StatusEntry::Conflicted(conflict) => {
                return Err(Error::Refused {
                    path: conflict.path.to_string(),
                    why: Refusal::Conflicted,
                });
            }
            StatusEntry::Changed(changed) => {
                if changed.submodule.is_some() {
                    return Err(Error::Refused {
                        path: changed.path.to_string(),
                        why: Refusal::Submodule,
                    });
                }
                for (path, kind) in kinds_of(&changed) {
                    kinds.entry(path).or_insert(kind);
                }
            }
            // Read again below, whatever the display setting hides.
            StatusEntry::Untracked(_) => {}
        }
    }
    if kinds.is_empty() {
        return Err(Error::CheckoutRefused {
            why: CheckoutRefusal::NothingToDiscard,
        });
    }
    let mut changes = Vec::with_capacity(kinds.len());
    for (path, kind) in kinds {
        if cancel.is_cancelled() {
            return Err(Error::ConsequenceCancelled);
        }
        let index_id = match index.side(&path)? {
            IndexSide::Entry { id, .. } => Some(id),
            IndexSide::Absent => None,
            IndexSide::Conflicted => {
                return Err(Error::Refused {
                    path: path.to_string(),
                    why: Refusal::Conflicted,
                });
            }
        };
        let disk = on_disk(repo, &path)?;
        let counted = lines.get(&path).copied().unwrap_or(Some(0));
        changes.push(LostChange {
            loss: ChangeLoss::Changed {
                kind,
                index: index_id,
                working_tree: disk.id().copied(),
                executable: disk.executable(),
                lines: counted,
            },
            path,
        });
    }
    let (lost, kept_untracked) = untracked_losses(git, repo, &target, cancel)?;
    changes.extend(lost);
    // By path, a tracked change before an untracked loss at the same path.
    changes.sort_by(|one, other| one.path.cmp(&other.path));
    Ok(Consequence::CheckoutDiscarding {
        branch: branch.to_owned(),
        at,
        head,
        changes,
        kept_untracked,
    })
}

/// Creates the branch the confirmation names at the commit it names and checks it out with
/// `git checkout -q -f -b <name> <id> --`, discarding exactly what the confirmation names:
/// the consequence is computed again first, and any difference — a file edited, `HEAD` moved,
/// an untracked file added in the way — refuses with [`Error::ChangedSinceConfirmed`], writing
/// nothing (R1.4). Every target is read from the confirmation, never from a parameter beside
/// it. Invalidates the refs, the index and the working tree.
pub fn create_branch_discarding(
    git: &GitBinary,
    repo: &Repository,
    confirmed: Confirmed,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let (branch, at) = match confirmed.consequence() {
        Consequence::CheckoutDiscarding { branch, at, .. } => (branch.clone(), *at),
        Consequence::DiscardLines { .. }
        | Consequence::DiscardFiles { .. }
        | Consequence::Amend { .. }
        | Consequence::RemoveLock { .. } => {
            return Err(Error::CheckoutRefused {
                why: CheckoutRefusal::NotWhatWasConfirmed,
            });
        }
    };
    let now = checkout_discarding_consequence(git, repo, &branch, at, &crate::CancelSignal::new())?;
    if let Some(moved) = first_difference(confirmed.consequence(), &now) {
        return Err(Error::ChangedSinceConfirmed { path: moved });
    }
    let before = locks_now(repo);
    run(
        git,
        repo,
        token,
        &checkout_arguments(&branch, at, true),
        None,
    )?;
    Ok(Performed::destructive(
        format!(
            "created and checked out branch {branch} at {}, discarding the changes",
            at.short()
        ),
        confirmed,
        checkout_invalidates(),
    )
    .with_locks(locks_around(repo, before)))
}

/// `checkout -q [-f] -b <name> <id> --`.
fn checkout_arguments(name: &str, commit: Oid, discarding: bool) -> Vec<OsString> {
    let mut arguments = vec![OsString::from("checkout"), OsString::from("-q")];
    if discarding {
        arguments.push(OsString::from("-f"));
    }
    arguments.extend([
        OsString::from("-b"),
        OsString::from(name),
        OsString::from(commit.to_string()),
        OsString::from("--"),
    ]);
    arguments
}

fn checkout_invalidates() -> Invalidated {
    Invalidated::refs()
        .and(Invalidated::index())
        .and(Invalidated::working_tree())
}

/// The paths a status entry's changes touch, each with how it differs from `HEAD`: a rename's
/// source too, which the checkout puts back.
fn kinds_of(changed: &cairn_model::ChangedEntry) -> Vec<(RepoPath, ChangedKind)> {
    let mut kinds = Vec::with_capacity(2);
    let own = match (&changed.staged, &changed.unstaged) {
        (Some(StagedChange::Added), _)
        | (Some(StagedChange::Renamed { .. } | StagedChange::Copied { .. }), _)
        | (
            None,
            Some(
                UnstagedChange::IntentToAdd
                | UnstagedChange::Renamed { .. }
                | UnstagedChange::Copied { .. },
            ),
        ) => ChangedKind::Added,
        (Some(StagedChange::Deleted), _) | (_, Some(UnstagedChange::Deleted)) => {
            ChangedKind::Deleted
        }
        (Some(StagedChange::Modified | StagedChange::TypeChanged), _)
        | (None, Some(UnstagedChange::Modified | UnstagedChange::TypeChanged))
        | (None, None) => ChangedKind::Modified,
    };
    kinds.push((changed.path.clone(), own));
    for from in [
        match &changed.staged {
            Some(StagedChange::Renamed { from, .. }) => Some(from),
            Some(
                StagedChange::Added
                | StagedChange::Modified
                | StagedChange::Deleted
                | StagedChange::TypeChanged
                | StagedChange::Copied { .. },
            )
            | None => None,
        },
        match &changed.unstaged {
            Some(UnstagedChange::Renamed { from, .. }) => Some(from),
            Some(
                UnstagedChange::Modified
                | UnstagedChange::Deleted
                | UnstagedChange::TypeChanged
                | UnstagedChange::IntentToAdd
                | UnstagedChange::Copied { .. },
            )
            | None => None,
        },
    ]
    .into_iter()
    .flatten()
    {
        kinds.push((from.clone(), ChangedKind::Deleted));
    }
    kinds
}

/// `HEAD`'s commit; `None` on an unborn branch.
fn head_commit(repo: &Repository) -> Result<Option<Oid>, Error> {
    match repo.inner().head_id() {
        Ok(id) => Ok(Some(model_id(&id)?)),
        Err(_) => Ok(None),
    }
}

/// `at`'s tree.
fn target_tree(repo: &Repository, at: Oid) -> Result<gix::Tree<'_>, Error> {
    let read = |source: Box<dyn std::error::Error + Send + Sync>| Error::ReadCommit {
        id: at.to_string(),
        source,
    };
    repo.inner()
        .find_commit(object_id(&at)?)
        .map_err(|source| read(Box::new(source)))?
        .tree()
        .map_err(|source| read(Box::new(source)))
}

/// What `tree` holds at `parts`: nothing, a tree, or something else (a file, a link, a
/// submodule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    Nothing,
    Tree,
    File,
}

fn held(tree: &gix::Tree<'_>, parts: &[&[u8]]) -> Result<Held, Error> {
    let entry = tree
        .lookup_entry(parts.iter().copied())
        .map_err(|source| Error::ReadCommit {
            id: tree.id.to_string(),
            source: Box::new(source),
        })?;
    Ok(match entry {
        None => Held::Nothing,
        Some(entry) if entry.mode().is_tree() => Held::Tree,
        Some(_) => Held::File,
    })
}

/// Every untracked path `git checkout -f` of `tree` would lose, and how many it keeps (phase
/// 10's QA, items 1-3): the untracked paths read whatever `status.showUntrackedFiles` says; a
/// file at a path the tree holds is overwritten; everything under a directory the tree holds a
/// file at — any directory on the way, the deepest file-held one taken — is deleted with that
/// directory, named once; and a nested repository at, or under, a path the tree holds is
/// deleted whole, its own `.git` among it.
fn untracked_losses(
    git: &GitBinary,
    repo: &Repository,
    tree: &gix::Tree<'_>,
    cancel: &impl Cancel,
) -> Result<(Vec<LostChange>, usize), Error> {
    let mut overwritten = Vec::new();
    // By path, and whether it is a nested repository rather than a directory.
    let mut removed: BTreeMap<(RepoPath, bool), (usize, u64)> = BTreeMap::new();
    let mut kept = 0usize;
    for path in crate::reads::untracked_paths(git, repo, cancel)? {
        if cancel.is_cancelled() {
            return Err(Error::ConsequenceCancelled);
        }
        let nested = path.as_bytes().ends_with(b"/");
        let trimmed = path
            .as_bytes()
            .strip_suffix(b"/")
            .unwrap_or(path.as_bytes());
        let parts: Vec<&[u8]> = trimmed.split(|byte| *byte == b'/').collect();
        // The first directory on the way the tree holds a file at.
        let mut blocked_at = None;
        let mut whole = Held::Nothing;
        for end in 1..=parts.len() {
            let here = held(tree, parts.get(..end).unwrap_or_default())?;
            if end < parts.len() {
                match here {
                    Held::Tree => continue,
                    Held::File => blocked_at = Some(end),
                    Held::Nothing => {}
                }
                break;
            }
            whole = here;
        }
        let (count, bytes) = if nested {
            content_under(repo, trimmed)?
        } else {
            let disk = on_disk(repo, &RepoPath::new(trimmed.to_vec()))?;
            (1, disk.bytes().unwrap_or(0))
        };
        match (blocked_at, nested, whole) {
            (Some(end), _, _) => {
                let directory = RepoPath::new(parts.get(..end).unwrap_or_default().join(&b'/'));
                let tally = removed.entry((directory, false)).or_default();
                tally.0 += count;
                tally.1 = tally.1.saturating_add(bytes);
            }
            (None, true, Held::Tree | Held::File) => {
                let tally = removed
                    .entry((RepoPath::new(trimmed.to_vec()), true))
                    .or_default();
                tally.0 += count;
                tally.1 = tally.1.saturating_add(bytes);
            }
            (None, false, Held::Tree | Held::File) => {
                let disk = on_disk(repo, &path)?;
                match (disk.id(), disk.bytes()) {
                    (Some(id), Some(bytes)) => overwritten.push(LostChange {
                        path,
                        loss: ChangeLoss::Overwritten {
                            working_tree: *id,
                            executable: disk.executable(),
                            bytes,
                        },
                    }),
                    _ => kept += 1,
                }
            }
            (None, _, Held::Nothing) => kept += 1,
        }
    }
    let mut lost = overwritten;
    lost.extend(
        removed
            .into_iter()
            .map(|((path, repository), (files, bytes))| LostChange {
                path,
                loss: ChangeLoss::Removed {
                    kind: if repository {
                        RemovedKind::Repository
                    } else {
                        RemovedKind::Directory
                    },
                    files,
                    bytes,
                },
            }),
    );
    Ok((lost, kept))
}

/// The files under the working tree's `relative` directory and their bytes, links not
/// followed: what a nested repository deleted whole holds.
fn content_under(repo: &Repository, relative: &[u8]) -> Result<(usize, u64), Error> {
    let top = repo.workdir().unwrap_or(repo.git_dir());
    let root = top.join(gix::path::from_byte_slice(relative));
    let mut pending = vec![root.clone()];
    let (mut files, mut bytes) = (0usize, 0u64);
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|source| Error::ReadWorkingTree {
            path: String::from_utf8_lossy(relative).into_owned(),
            source,
        })?;
        for entry in entries.flatten() {
            let Ok(metadata) = entry.path().symlink_metadata() else {
                continue;
            };
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                files += 1;
                bytes = bytes.saturating_add(metadata.len());
            }
        }
    }
    Ok((files, bytes))
}

/// The first thing `now` differs from `confirmed` in, named for the refusal: a path, or
/// `HEAD`.
fn first_difference(confirmed: &Consequence, now: &Consequence) -> Option<String> {
    if confirmed == now {
        return None;
    }
    match (confirmed, now) {
        (
            Consequence::CheckoutDiscarding {
                head: was,
                changes: confirmed_changes,
                ..
            },
            Consequence::CheckoutDiscarding {
                head: is,
                changes: now_changes,
                ..
            },
        ) => {
            if was != is {
                return Some("HEAD".to_owned());
            }
            let moved = confirmed_changes
                .iter()
                .zip(now_changes.iter())
                .find(|(was, is)| was != is)
                .map(|(was, _)| was.path.to_string())
                .or_else(|| {
                    let longer = if confirmed_changes.len() > now_changes.len() {
                        confirmed_changes
                    } else {
                        now_changes
                    };
                    longer
                        .get(confirmed_changes.len().min(now_changes.len()))
                        .map(|change| change.path.to_string())
                });
            Some(moved.unwrap_or_else(|| "the untracked files".to_owned()))
        }
        _ => Some("HEAD".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::recording_stub::RecordingStub;

    /// R11.3's argv, kept and discarding: `checkout -q -b`, `-f` only when discarding, the name
    /// as `-b`'s value, the commit by its full id, `--` after it; a write's environment.
    /// Caught by: `-f` on the kept checkout, a name or a commit read as a path, a short id.
    #[test]
    fn the_checkouts_run_as_r11_names_them() {
        let id = Oid::parse("07da224c7ec04501dfb451be161fa962effe1dc1").unwrap();
        let strings = |arguments: Vec<OsString>| -> Vec<String> {
            arguments
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            strings(checkout_arguments("topic", id, false)),
            [
                "checkout",
                "-q",
                "-b",
                "topic",
                id.to_string().as_str(),
                "--"
            ]
        );
        assert_eq!(
            strings(checkout_arguments("-x", id, true)),
            [
                "checkout",
                "-q",
                "-f",
                "-b",
                "-x",
                id.to_string().as_str(),
                "--"
            ]
        );
        let stub = RecordingStub::new();
        let (git, repo) = (stub.git_binary(), stub.repository());
        let head = model_id(&repo.inner().head_id().unwrap()).unwrap();
        // The stub's working tree has a staged and an unstaged edit on file.txt: git refuses
        // nothing here, since the checkout is of `HEAD` itself.
        create_branch_and_checkout(&git, &repo, "kept", head, None).unwrap();
        let recorded = stub.recorded();
        let checkout = recorded
            .iter()
            .find(|record| {
                record
                    .arguments_after_location()
                    .contains(&"checkout".to_owned())
            })
            .unwrap();
        assert_eq!(
            checkout.arguments_after_location(),
            [
                "--literal-pathspecs",
                "checkout",
                "-q",
                "-b",
                "kept",
                head.to_string().as_str(),
                "--"
            ]
        );
        checkout.assert_a_write();
    }
}
