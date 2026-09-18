//! The tree walk behind a changes query, and gix's changes turned into `ChangedFile`s.

use std::convert::Infallible;
use std::ops::ControlFlow;

use cairn_model::{ChangeStatus, ChangedFile, FileMode, Oid, RepoPath, Similarity};
use gix::object::tree::diff::Change;
use gix::objs::tree::{EntryKind, EntryMode};

use crate::object_id::{model_id, object_id};
use crate::{Cancel, Error, Repository};

use super::{ChangeSet, ChangesRequest, RenameDetection, Subject};

pub(super) fn changes(
    repo: &Repository,
    cache: &mut gix::diff::blob::Platform,
    request: &ChangesRequest,
    cancel: &impl Cancel,
) -> Result<ChangeSet, Error> {
    let inner = repo.inner();
    let (old_tree, new_tree, details) = match &request.subject {
        Subject::Commit(id) => {
            let commit = find_commit(inner, id)?;
            let details = crate::commit::details_of(&commit, id)?;
            let new_tree = tree_of_commit(&commit, id)?;
            // A root commit is compared with the empty tree (L5), which makes its diff the
            // whole of its content rather than nothing at all.
            let old_tree = match details.parents.first() {
                Some(parent) => tree_of_commit(&find_commit(inner, parent)?, parent)?,
                None => inner.empty_tree(),
            };
            (old_tree, new_tree, Some(details))
        }
        Subject::Between { old, new } => (
            tree_of_commit(&find_commit(inner, old)?, old)?,
            tree_of_commit(&find_commit(inner, new)?, new)?,
            None,
        ),
    };

    // `changes()` reads `diff.renames` and `diff.renameLimit` from the user's config and
    // falls back to git's own defaults, which is R2.2 in one call.
    let mut platform = old_tree.changes().map_err(|source| Error::TreeDiff {
        source: Box::new(source),
    })?;

    let mut files = Vec::new();
    let mut cancelled = false;
    let mut failure = None;
    let outcome = platform.for_each_to_obtain_tree_with_cache(&new_tree, cache, |change| {
        if cancel.is_cancelled() {
            cancelled = true;
            return Ok::<_, Infallible>(ControlFlow::Break(()));
        }
        match changed_file(&change) {
            // A directory is emitted as a change of its own beside its entries; only the
            // entries are files.
            Ok(None) => {}
            Ok(Some(file)) => files.push(file),
            Err(error) => {
                failure = Some(error);
                return Ok(ControlFlow::Break(()));
            }
        }
        Ok(ControlFlow::Continue(()))
    });

    // Both checks come before the call's own error: a break is reported as a failure by
    // gix, and the reason it broke is ours to name.
    if cancelled {
        return Err(Error::ChangesCancelled {
            changed: files.len(),
        });
    }
    if let Some(error) = failure {
        return Err(error);
    }
    let outcome = outcome.map_err(|source| Error::TreeDiff {
        source: Box::new(source),
    })?;

    repair_copies(&old_tree, &mut files)?;

    // gix emits modifications in traversal order and rename pairs as the tracker finds
    // them, so the answer is sorted here. The key is total — a destination path, then the
    // source it came from — so the list cannot shuffle between two runs of one query.
    files.sort_by(|left, right| {
        left.new_path
            .cmp(&right.new_path)
            .then_with(|| left.old_path.cmp(&right.old_path))
    });

    Ok(ChangeSet {
        files,
        details,
        renames: rename_detection(outcome.as_ref()),
    })
}

/// Puts a copy's source back where git has it.
///
/// gix looks for a copy among the files MODIFIED by the same change, and when it finds one
/// it reports the source as it is AFTER the change and stops reporting that file as
/// modified at all. git reports the source as it was BEFORE — which is the version
/// `git apply --cached` finds in the index, so it is the version a patch has to be built
/// against — and still lists the file as modified. Both are put right here, by reading the
/// source path out of the old tree: one lookup per copy, and nothing at all when copies are
/// not configured.
///
/// The similarity percentage stays gix's, and gix measured it against the other version of
/// the source; see `docs/systems/diff.md`.
fn repair_copies(old_tree: &gix::Tree<'_>, files: &mut Vec<ChangedFile>) -> Result<(), Error> {
    if !files.iter().any(ChangedFile::is_copy) {
        return Ok(());
    }

    let mut swallowed: Vec<ChangedFile> = Vec::new();
    for index in 0..files.len() {
        if !files[index].is_copy() {
            continue;
        }
        let source_path = files[index].old_path.clone();
        let Some((before_mode, before_id)) = entry_of(old_tree, &source_path)? else {
            // The source is not in the old tree at all, which the set of modified files
            // cannot contain; leave gix's answer as it stands.
            continue;
        };
        let after_mode = files[index].old_mode;
        let after_id = files[index].old_id;
        files[index].old_mode = Some(before_mode);
        files[index].old_id = Some(before_id);
        if files[index].new_id == Some(before_id) {
            // Byte-identical to the source as it was, which is what `C100` means.
            files[index].status = ChangeStatus::Copied(Similarity::from_percent(100));
        }

        let moved = after_id != Some(before_id) || after_mode != Some(before_mode);
        let already_listed = files
            .iter()
            .any(|file| file.new_path == source_path && !file.is_copy());
        let recovered = swallowed
            .iter()
            .any(|file: &ChangedFile| file.new_path == source_path);
        if moved && !already_listed && !recovered {
            let (Some(after_mode), Some(after_id)) = (after_mode, after_id) else {
                continue;
            };
            swallowed.push(ChangedFile {
                status: if is_type_change(before_mode, after_mode) {
                    ChangeStatus::TypeChanged
                } else {
                    ChangeStatus::Modified
                },
                old_path: source_path.clone(),
                new_path: source_path,
                old_mode: Some(before_mode),
                new_mode: Some(after_mode),
                old_id: Some(before_id),
                new_id: Some(after_id),
            });
        }
    }
    files.append(&mut swallowed);
    Ok(())
}

/// The mode and id one path had in a tree, or `None` when it was not in it.
fn entry_of(
    tree: &gix::Tree<'_>,
    path: &RepoPath,
) -> Result<Option<(FileMode, cairn_model::Oid)>, Error> {
    let components = path
        .as_bytes()
        .split(|byte| *byte == b'/')
        .map(gix::bstr::BString::from);
    let found = tree
        .lookup_entry(components)
        .map_err(|source| Error::TreeDiff {
            source: Box::new(source),
        })?;
    let Some(entry) = found else {
        return Ok(None);
    };
    let Some(mode) = file_mode(entry.mode()) else {
        return Ok(None);
    };
    Ok(Some((mode, model_id(&entry.id().detach())?)))
}

fn rename_detection(outcome: Option<&gix::diff::rewrites::Outcome>) -> RenameDetection {
    let Some(outcome) = outcome else {
        return RenameDetection::default();
    };
    RenameDetection {
        enabled: true,
        copies: outcome.options.copies.is_some(),
        limit: outcome.options.limit,
        similarity_checks: outcome.num_similarity_checks,
        renames_skipped_for_limit: outcome
            .num_similarity_checks_skipped_for_rename_tracking_due_to_limit,
        copies_skipped_for_limit: outcome
            .num_similarity_checks_skipped_for_copy_tracking_due_to_limit,
    }
}

fn find_commit<'repo>(repo: &'repo gix::Repository, id: &Oid) -> Result<gix::Commit<'repo>, Error> {
    repo.find_commit(object_id(id)?)
        .map_err(|source| Error::ReadCommit {
            id: id.to_string(),
            source: Box::new(source),
        })
}

fn tree_of_commit<'repo>(commit: &gix::Commit<'repo>, id: &Oid) -> Result<gix::Tree<'repo>, Error> {
    commit.tree().map_err(|source| Error::ReadCommit {
        id: id.to_string(),
        source: Box::new(source),
    })
}

/// `None` for a directory: gix reports a whole directory added or deleted as its own
/// change beside the entries under it, and a file list wants the entries.
fn changed_file(change: &Change<'_, '_, '_>) -> Result<Option<ChangedFile>, Error> {
    Ok(match change {
        Change::Addition {
            location,
            entry_mode,
            id,
            ..
        } => {
            let Some(mode) = file_mode(*entry_mode) else {
                return Ok(None);
            };
            let path = path_of(location);
            Some(ChangedFile {
                status: ChangeStatus::Added,
                old_path: path.clone(),
                new_path: path,
                old_mode: None,
                new_mode: Some(mode),
                old_id: None,
                new_id: Some(model_id(id)?),
            })
        }
        Change::Deletion {
            location,
            entry_mode,
            id,
            ..
        } => {
            let Some(mode) = file_mode(*entry_mode) else {
                return Ok(None);
            };
            let path = path_of(location);
            Some(ChangedFile {
                status: ChangeStatus::Deleted,
                old_path: path.clone(),
                new_path: path,
                old_mode: Some(mode),
                new_mode: None,
                old_id: Some(model_id(id)?),
                new_id: None,
            })
        }
        Change::Modification {
            location,
            previous_entry_mode,
            previous_id,
            entry_mode,
            id,
        } => {
            let (Some(old_mode), Some(new_mode)) =
                (file_mode(*previous_entry_mode), file_mode(*entry_mode))
            else {
                return Ok(None);
            };
            let path = path_of(location);
            Some(ChangedFile {
                status: if is_type_change(old_mode, new_mode) {
                    ChangeStatus::TypeChanged
                } else {
                    ChangeStatus::Modified
                },
                old_path: path.clone(),
                new_path: path,
                old_mode: Some(old_mode),
                new_mode: Some(new_mode),
                old_id: Some(model_id(previous_id)?),
                new_id: Some(model_id(id)?),
            })
        }
        Change::Rewrite {
            source_location,
            source_entry_mode,
            source_id,
            diff,
            entry_mode,
            location,
            id,
            copy,
            ..
        } => {
            let (Some(old_mode), Some(new_mode)) =
                (file_mode(*source_entry_mode), file_mode(*entry_mode))
            else {
                return Ok(None);
            };
            let similarity = similarity_of(diff.as_ref());
            Some(ChangedFile {
                status: if *copy {
                    ChangeStatus::Copied(similarity)
                } else {
                    ChangeStatus::Renamed(similarity)
                },
                old_path: path_of(source_location),
                new_path: path_of(location),
                old_mode: Some(old_mode),
                new_mode: Some(new_mode),
                old_id: Some(model_id(source_id)?),
                new_id: Some(model_id(id)?),
            })
        }
    })
}

fn path_of(location: &gix::bstr::BStr) -> RepoPath {
    RepoPath::new(location.to_vec())
}

/// `None` for a tree, which is the one mode a changed *file* never has.
fn file_mode(mode: EntryMode) -> Option<FileMode> {
    match mode.kind() {
        EntryKind::Blob => Some(FileMode::Regular),
        EntryKind::BlobExecutable => Some(FileMode::Executable),
        EntryKind::Link => Some(FileMode::Symlink),
        EntryKind::Commit => Some(FileMode::Submodule),
        EntryKind::Tree => None,
    }
}

/// git's `T`: the entry stopped being one kind of thing and became another. Gaining or
/// losing the executable bit is not one — git calls that a modification, and so does R1.1.
fn is_type_change(old: FileMode, new: FileMode) -> bool {
    fn kind(mode: FileMode) -> u8 {
        match mode {
            FileMode::Regular | FileMode::Executable => 0,
            FileMode::Symlink => 1,
            FileMode::Submodule => 2,
        }
    }
    kind(old) != kind(new)
}

/// gix reports no similarity for a pair it matched by identity, because it ran no
/// comparison to find one: identical content is a hundred percent alike.
fn similarity_of(stats: Option<&gix::diff::blob::DiffLineStats>) -> Similarity {
    let Some(stats) = stats else {
        return Similarity::from_percent(100);
    };
    // git truncates rather than rounds: `similarity index 99%` means "not quite identical".
    let percent = (stats.similarity * 100.0).floor().clamp(0.0, 100.0);
    Similarity::from_percent(percent as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: treating the executable bit as a type change, which would show `T` where
    /// git shows `M` and, through `ChangedFile::mode_changed`, is a different patch header.
    #[test]
    fn only_a_change_of_kind_is_a_type_change() {
        use FileMode::{Executable, Regular, Submodule, Symlink};
        assert!(!is_type_change(Regular, Executable));
        assert!(!is_type_change(Executable, Regular));
        assert!(!is_type_change(Regular, Regular));
        assert!(is_type_change(Regular, Symlink));
        assert!(is_type_change(Symlink, Executable));
        assert!(is_type_change(Regular, Submodule));
        assert!(is_type_change(Submodule, Symlink));
    }

    #[test]
    fn a_tree_is_not_a_changed_file_and_every_other_mode_is() {
        assert_eq!(file_mode(EntryKind::Tree.into()), None);
        assert_eq!(file_mode(EntryKind::Blob.into()), Some(FileMode::Regular));
        assert_eq!(
            file_mode(EntryKind::BlobExecutable.into()),
            Some(FileMode::Executable)
        );
        assert_eq!(file_mode(EntryKind::Link.into()), Some(FileMode::Symlink));
        assert_eq!(
            file_mode(EntryKind::Commit.into()),
            Some(FileMode::Submodule)
        );
    }

    /// Caught by: rounding, which turns a 99.6% match into `similarity index 100%` — the
    /// number git reserves for content that is byte-identical.
    #[test]
    fn a_similarity_reads_the_way_git_spells_one() {
        let stats = |similarity| gix::diff::blob::DiffLineStats {
            removals: 0,
            insertions: 0,
            before: 0,
            after: 0,
            similarity,
        };
        assert_eq!(similarity_of(None).percent(), 100, "an identity match");
        assert_eq!(similarity_of(Some(&stats(1.0))).percent(), 100);
        assert_eq!(similarity_of(Some(&stats(0.996))).percent(), 99);
        assert_eq!(similarity_of(Some(&stats(0.5))).percent(), 50);
        assert_eq!(similarity_of(Some(&stats(0.0))).percent(), 0);
        assert_eq!(similarity_of(Some(&stats(-1.0))).percent(), 0, "clamped");
    }

    /// Caught by: reporting a cut-short search as complete, which would leave a view
    /// claiming a file was added when git would have called it a rename.
    #[test]
    fn a_search_the_limit_stopped_says_so() {
        let mut detection = RenameDetection {
            enabled: true,
            copies: false,
            limit: 1000,
            similarity_checks: 12,
            renames_skipped_for_limit: 0,
            copies_skipped_for_limit: 0,
        };
        assert!(!detection.was_cut_short());
        detection.renames_skipped_for_limit = 1;
        assert!(detection.was_cut_short());
        detection.renames_skipped_for_limit = 0;
        detection.copies_skipped_for_limit = 1;
        assert!(detection.was_cut_short());
        assert!(
            !RenameDetection::default().was_cut_short(),
            "detection that never ran cannot have been cut short"
        );
    }
}
