//! Discarding: lines and a mode change by patch, and whole files — a tracked file's
//! unstaged change restored from the index, an untracked file deleted
//! (`docs/prd/staging-and-commit.md` R3.3, R3.5). Destructive (R1.5): what is discarded was
//! never hashed into any object, so nothing in the repository can bring it back, and no
//! backup is made (L2). Each operation takes a [`Confirmed`] by value, is on the guard's
//! destructive-operation roster, derives every target from the [`Consequence`] the token
//! carries, and re-reads the repository the moment before it runs, refusing — writing
//! nothing — when anything that `Consequence` names moved (R1.4,
//! [`Error::ChangedSinceConfirmed`]).
//!
//! Each `Consequence` is computed here, from the repository as it is, by
//! [`discard_lines_consequence`] and [`discard_files_consequence`] — never by a caller —
//! and refused before one is offered where no honest prompt could be shown: a selection of
//! nothing (no zero-line prompt, phase 01's QA item 18), no file at all, a conflicted path
//! (R3.11), a submodule (R3.10), a repository nested in the working tree (R3.5), and staged
//! changes, which are never discarded (R3.6): a path with no unstaged change is refused.
//!
//! **Lines.** The patch is the model's, from the exact diff the user confirmed — the
//! unstaged or untracked diff, inverted, never applied with `-R` — given to
//! `git apply --whitespace=nowarn -` on the working tree, which cleans the file on read and
//! smudges it on write, so a git-form patch lands on a CRLF or filtered file and the file
//! keeps its own form. The diff handed to [`discard_lines`] must be the one confirmed: its
//! path and both its ids are checked against the `Consequence`, which is what the patch's
//! targets come from. Every line of a new file is not discarded by patch — that deletes the
//! file — but by [`discard_files`], whose prompt says the file is deleted
//! ([`Refusal::WholeFileOnly`]; phase 02's carry-forward).
//!
//! **Files.** Tracked files go by `git restore --worktree` from a pathspec file, untracked
//! ones by `git clean -f -- <paths>` — exactly the paths `git status` listed, each a file:
//! git lists untracked files one per file (`crate::reads::status`), the one directory it
//! still lists whole is a nested repository, which is refused, so `-d` is never passed and
//! a file added beside a deleted one is never taken. `git clean` reads no pathspec file, so
//! its paths go on `argv`, split into invocations of at most [`CLEAN_ARGUMENT_BYTES`] each,
//! all after the one re-check made before the first (R3.5). The two verbs are two writes:
//! where the second fails, the first has happened, and the failure says which.
//!
//! **The re-check** compares the index entry, read with gix from the index file now, and the
//! working-tree file, hashed as its bytes are now — so any byte changed since the
//! confirmation refuses, a line ending included, which git's form would not show (phase 01's
//! QA item 25) — and, for lines, the file's git form too, since that is what the patch was
//! built against (R3.7). Between the re-check and git's run is a window no check closes; it
//! is the user's own `git checkout -p`'s window too.

use std::ffi::OsString;

use cairn_model::{
    AskpassToken, ChangeStatus, Confirmed, Consequence, DiffContent, DiscardedFile, FileDiff,
    FileLoss, PatchAction, RepoPath, Selection,
};

use super::fresh_state::{
    IndexNow, IndexSide, OnDisk, git_form, holds_a_repository, index_side, on_disk,
};
use super::local_write::{PATHSPEC_FILE, locks_around, locks_now, run};
use super::stage::{checked_pathspec_file, patch_of, refused};
use super::{GitBinary, Invalidated, Performed};
use crate::{CancelSignal, ContentOptions, Error, Refusal, Repository, WorkingTreeDiff};

/// How many bytes of `argv` one `git clean` is given for its paths: each path's bytes, its
/// NUL and its pointer. Half of Linux's smallest `ARG_MAX` (32 pages, 128 KiB), a sixteenth
/// of macOS's 1 MiB and a thirty-second of the 2 MiB measured on the machine this was built
/// on, where `git clean` took 1.5 MiB of 100-byte paths and the kernel refused 1.9 MiB with
/// `E2BIG` (`progress.md`, phase 03): the rest of `ARG_MAX` is the environment's and the
/// other arguments'. A path is at most `PATH_MAX`, 4 KiB, so one always fits.
pub const CLEAN_ARGUMENT_BYTES: usize = 64 * 1024;

/// What `git apply` is given for a discard: the patch on stdin, on the working tree.
const APPLY_WORKTREE: [&str; 3] = ["apply", "--whitespace=nowarn", "-"];

/// The `Consequence` of discarding `selection` of `diff` — the path's unstaged or untracked
/// diff, as the engine answered it — read from the repository now (module docs): the index
/// entry the lines go back to, the file's git form and its bytes, the selection, and the mode
/// change when the selection holds it.
///
/// Refused before any prompt ([`Error::Refused`]) when nothing is selected, when the change
/// is one only a file verb takes, or when the selection is every line of a new file (which
/// [`discard_files_consequence`] confirms as a deletion); [`Error::ChangedSinceRead`] when the
/// index or the file is no longer the diff's; [`Error::ContentReadsDisagree`] when the file
/// changed while it was read.
pub fn discard_lines_consequence(
    git: &GitBinary,
    repo: &Repository,
    diff: &FileDiff,
    selection: Selection,
) -> Result<Consequence, Error> {
    let path = &diff.file.new_path;
    match &diff.content {
        DiffContent::Submodule { .. } => return Err(refused(path, Refusal::Submodule)),
        DiffContent::Conflicted => return Err(refused(path, Refusal::Conflicted)),
        DiffContent::Text { .. }
        | DiffContent::ModeChangeOnly
        | DiffContent::Binary { .. }
        | DiffContent::TooLarge { .. }
        | DiffContent::LfsPointer { .. }
        | DiffContent::Unsupported { .. } => {}
    }
    let mode = match (
        selection.holds_mode(),
        diff.file.old_mode,
        diff.file.new_mode,
    ) {
        (true, Some(old), Some(new)) if old != new => Some((old, new)),
        _ => None,
    };
    let no_lines = selection.removed().next().is_none() && selection.added().next().is_none();
    if no_lines && mode.is_none() {
        return Err(refused(path, Refusal::NothingSelected));
    }
    if diff.file.status == ChangeStatus::Added
        && diff
            .text()
            .is_some_and(|text| selection.holds_every_change(text))
    {
        return Err(refused(path, Refusal::WholeFileOnly));
    }
    patch_of(PatchAction::Discard, diff, &selection)?;
    match index_side(repo, path)? {
        IndexSide::Conflicted => return Err(refused(path, Refusal::Conflicted)),
        side if !side.is_old_side(diff.file.old_id.as_ref(), diff.file.old_mode) => {
            return Err(Error::ChangedSinceRead {
                path: path.to_string(),
            });
        }
        IndexSide::Absent | IndexSide::Entry { .. } => {}
    }
    let disk = on_disk(repo, path)?;
    let form = git_form(git, repo, path, &disk)?;
    if on_disk(repo, path)? != disk {
        return Err(moved_while_read(path));
    }
    let (Some(on_disk_id), Some(form)) = (disk.id().copied(), form) else {
        return Err(Error::ChangedSinceRead {
            path: path.to_string(),
        });
    };
    if diff.file.new_id.as_ref() != Some(&form) {
        return Err(Error::ChangedSinceRead {
            path: path.to_string(),
        });
    }
    Ok(Consequence::DiscardLines {
        path: path.clone(),
        index: diff.file.old_id,
        working_tree: form,
        on_disk: on_disk_id,
        selection,
        mode,
    })
}

/// Discards the lines `confirmed` names, from `diff` — which must be the diff its
/// `Consequence` was computed from ([`Refusal::NotWhatWasConfirmed`] otherwise) — with
/// `git apply --whitespace=nowarn -` on the working tree (R3.3).
///
/// Re-reads the index entry, the file's bytes and its git form first, and refuses with
/// [`Error::ChangedSinceConfirmed`], writing nothing, when any differs from the
/// `Consequence` (R1.4). The [`Performed`] records the prompt the user accepted.
pub fn discard_lines(
    git: &GitBinary,
    repo: &Repository,
    diff: &FileDiff,
    confirmed: Confirmed,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let (path, patch) = match confirmed.consequence() {
        Consequence::DiscardLines {
            path,
            index,
            working_tree,
            on_disk: disk_id,
            selection,
            mode: _,
        } => {
            let confirmed_diff = diff.file.new_path == *path
                && diff.file.old_id == *index
                && diff.file.new_id.as_ref() == Some(working_tree);
            if !confirmed_diff {
                return Err(refused(path, Refusal::NotWhatWasConfirmed));
            }
            let changed = || Error::ChangedSinceConfirmed {
                path: path.to_string(),
            };
            let side = index_side(repo, path)?;
            if !side.is_old_side(index.as_ref(), diff.file.old_mode) {
                return Err(changed());
            }
            let disk = on_disk(repo, path)?;
            if disk.id() != Some(disk_id) {
                return Err(changed());
            }
            if git_form(git, repo, path, &disk)?.as_ref() != Some(working_tree) {
                return Err(changed());
            }
            (
                path.clone(),
                patch_of(PatchAction::Discard, diff, selection)?,
            )
        }
        Consequence::DiscardFiles { .. }
        | Consequence::Amend { .. }
        | Consequence::RemoveLock { .. } => {
            return Err(refused(&diff.file.new_path, Refusal::NotWhatWasConfirmed));
        }
    };
    let before = locks_now(repo);
    run(
        git,
        repo,
        token,
        &APPLY_WORKTREE.map(OsString::from),
        Some(patch.as_bytes().to_vec()),
    )?;
    Ok(Performed::destructive(
        format!("discarded lines of {path}"),
        confirmed,
        Invalidated::working_tree(),
    )
    .with_locks(locks_around(repo, before)))
}

/// The `Consequence` of discarding the changes in `paths` — rows of Local Changes' Unstaged
/// list, as `git status` listed them — read from the repository now: for each tracked file
/// the index entry it is restored from, its bytes (or that it is deleted, and comes back),
/// and how many lines its unstaged diff changes (`None` for a change that is not text); for
/// each untracked file its bytes' id and size.
///
/// Refused before any prompt ([`Error::Refused`]) for a conflicted path, a submodule, a
/// nested repository or any other directory, a special file, and a path with no unstaged
/// change; [`Error::NoPaths`] for no path at all; [`Error::ChangedSinceRead`] for an
/// untracked file that is gone; [`Error::ContentReadsDisagree`] for a file that changed
/// while it was read. Each path is counted once, in the order given.
pub fn discard_files_consequence(
    git: &GitBinary,
    repo: &Repository,
    paths: &[RepoPath],
) -> Result<Consequence, Error> {
    if paths.is_empty() {
        return Err(Error::NoPaths);
    }
    let index = IndexNow::read(repo)?;
    let mut files: Vec<DiscardedFile> = Vec::with_capacity(paths.len());
    for path in paths {
        if files.iter().any(|file| &file.path == path) {
            continue;
        }
        crate::reads::work_tree_relative(path)?;
        if path.as_bytes().ends_with(b"/") {
            return Err(directory_refusal(repo, path)?);
        }
        let loss = match index.side(path)? {
            IndexSide::Conflicted => return Err(refused(path, Refusal::Conflicted)),
            IndexSide::Entry {
                mode: Some(cairn_model::FileMode::Submodule),
                ..
            } => return Err(refused(path, Refusal::Submodule)),
            IndexSide::Entry { id, .. } => {
                let disk = on_disk(repo, path)?;
                if let Some(refusal) = not_a_file(repo, path, &disk)? {
                    return Err(refusal);
                }
                let lines = unstaged_lines(git, repo, path)?;
                if on_disk(repo, path)? != disk {
                    return Err(moved_while_read(path));
                }
                FileLoss::Modified {
                    index: id,
                    working_tree: disk.id().copied(),
                    lines,
                }
            }
            IndexSide::Absent => {
                let disk = on_disk(repo, path)?;
                if let Some(refusal) = not_a_file(repo, path, &disk)? {
                    return Err(refusal);
                }
                match (disk.id(), disk.bytes()) {
                    (Some(id), Some(bytes)) => FileLoss::Untracked {
                        working_tree: *id,
                        bytes,
                    },
                    _ => {
                        return Err(Error::ChangedSinceRead {
                            path: path.to_string(),
                        });
                    }
                }
            }
        };
        files.push(DiscardedFile {
            path: path.clone(),
            loss,
        });
    }
    Ok(Consequence::DiscardFiles { files })
}

/// Discards the changes in the files `confirmed` names (R3.5): each tracked file restored
/// from the index with `git restore --worktree`, from a pathspec file, then each untracked
/// file deleted with `git clean -f --`, its paths split past [`CLEAN_ARGUMENT_BYTES`].
///
/// Re-reads every file and its index entry first — all of them, before the first write —
/// and refuses with [`Error::ChangedSinceConfirmed`], writing nothing, when any differs from
/// the `Consequence` (R1.4). The [`Performed`] records the prompt the user accepted.
pub fn discard_files(
    git: &GitBinary,
    repo: &Repository,
    confirmed: Confirmed,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let (tracked, untracked) = match confirmed.consequence() {
        Consequence::DiscardFiles { files } => {
            if files.is_empty() {
                return Err(Error::NoPaths);
            }
            let index = IndexNow::read(repo)?;
            for file in files {
                recheck(repo, &index, file)?;
            }
            let (tracked, untracked): (Vec<&DiscardedFile>, Vec<&DiscardedFile>) =
                files.iter().partition(|file| match file.loss {
                    FileLoss::Modified { .. } => true,
                    FileLoss::Untracked { .. } => false,
                });
            (
                tracked
                    .into_iter()
                    .map(|file| file.path.clone())
                    .collect::<Vec<_>>(),
                untracked
                    .into_iter()
                    .map(|file| file.path.clone())
                    .collect::<Vec<_>>(),
            )
        }
        Consequence::DiscardLines { path, .. } => {
            return Err(refused(path, Refusal::NotWhatWasConfirmed));
        }
        Consequence::Amend { .. } | Consequence::RemoveLock { .. } => {
            return Err(refused(&RepoPath::new(""), Refusal::NotWhatWasConfirmed));
        }
    };
    let before = locks_now(repo);
    if !tracked.is_empty() {
        let mut arguments = vec![OsString::from("restore"), OsString::from("--worktree")];
        arguments.extend(PATHSPEC_FILE.map(OsString::from));
        run(
            git,
            repo,
            token,
            &arguments,
            Some(checked_pathspec_file(&tracked)?),
        )?;
    }
    for batch in clean_batches(&untracked) {
        let mut arguments: Vec<OsString> = ["clean", "-f", "--"].map(OsString::from).into();
        arguments.extend(batch.iter().map(|path| {
            use std::os::unix::ffi::OsStringExt as _;
            OsString::from_vec(path.as_bytes().to_vec())
        }));
        run(git, repo, token, &arguments, None)?;
    }
    let count = tracked.len() + untracked.len();
    Ok(Performed::destructive(
        format!(
            "discarded the changes in {count} {}",
            if count == 1 { "file" } else { "files" }
        ),
        confirmed,
        Invalidated::working_tree().and(Invalidated::index()),
    )
    .with_locks(locks_around(repo, before)))
}

/// One file of a discard re-read against what was confirmed of it (R1.4): a tracked file's
/// index entry and bytes, an untracked file's absence from the index, bytes and size.
fn recheck(repo: &Repository, index: &IndexNow, file: &DiscardedFile) -> Result<(), Error> {
    let path = &file.path;
    let side = index.side(path)?;
    let disk = on_disk(repo, path)?;
    let unchanged = match &file.loss {
        FileLoss::Modified {
            index,
            working_tree,
            lines: _,
        } => {
            let entry = matches!(&side, IndexSide::Entry { id, mode, .. }
                if id == index && *mode != Some(cairn_model::FileMode::Submodule));
            let file_now = match working_tree {
                Some(id) => disk.id() == Some(id),
                None => disk == OnDisk::Absent,
            };
            entry && file_now
        }
        FileLoss::Untracked {
            working_tree,
            bytes,
        } => {
            side == IndexSide::Absent
                && disk.id() == Some(working_tree)
                && disk.bytes() == Some(*bytes)
        }
    };
    if unchanged {
        Ok(())
    } else {
        Err(Error::ChangedSinceConfirmed {
            path: path.to_string(),
        })
    }
}

/// The paths of one `git clean` each, in order: as many as fit [`CLEAN_ARGUMENT_BYTES`],
/// and never none.
pub(crate) fn clean_batches(paths: &[RepoPath]) -> Vec<&[RepoPath]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut used = 0usize;
    for (at, path) in paths.iter().enumerate() {
        let cost = path.as_bytes().len() + 1 + std::mem::size_of::<usize>();
        if at > start && used + cost > CLEAN_ARGUMENT_BYTES {
            batches.push(&paths[start..at]);
            start = at;
            used = 0;
        }
        used += cost;
    }
    if start < paths.len() {
        batches.push(&paths[start..]);
    }
    batches
}

/// How many lines of `path`'s unstaged diff a discard takes back, read as the diff is drawn:
/// `None` for a change that is not text (binary, an LFS pointer, past the load-anyway
/// ceiling, unreadable), and [`Refusal::NoUnstagedChange`] where git's diff of it is empty.
fn unstaged_lines(
    git: &GitBinary,
    repo: &Repository,
    path: &RepoPath,
) -> Result<Option<usize>, Error> {
    let options = ContentOptions {
        load_anyway: true,
        ..ContentOptions::default()
    };
    let Some(diff) = repo.working_tree_diff(
        git,
        path,
        WorkingTreeDiff::Unstaged,
        &options,
        &CancelSignal::new(),
    )?
    else {
        return Err(refused(path, Refusal::NoUnstagedChange));
    };
    match &diff.content {
        DiffContent::Text { text, .. } => Ok(Some(Selection::with_every_change(text).len())),
        DiffContent::ModeChangeOnly => Ok(Some(0)),
        DiffContent::Binary { .. }
        | DiffContent::TooLarge { .. }
        | DiffContent::LfsPointer { .. }
        | DiffContent::Unsupported { .. } => Ok(None),
        DiffContent::Submodule { .. } => Err(refused(path, Refusal::Submodule)),
        DiffContent::Conflicted => Err(refused(path, Refusal::Conflicted)),
    }
}

/// The refusal for a path that is not one file to discard, if it is not: a directory — a
/// nested repository, or any other — or a special file.
fn not_a_file(repo: &Repository, path: &RepoPath, disk: &OnDisk) -> Result<Option<Error>, Error> {
    match disk {
        OnDisk::Directory => directory_refusal(repo, path).map(Some),
        OnDisk::Other => Ok(Some(refused(path, Refusal::NotAFile))),
        OnDisk::Changing => Ok(Some(moved_while_read(path))),
        OnDisk::Absent | OnDisk::File { .. } | OnDisk::Symlink { .. } => Ok(None),
    }
}

fn directory_refusal(repo: &Repository, path: &RepoPath) -> Result<Error, Error> {
    let why = if holds_a_repository(repo, path)? {
        Refusal::NestedRepository
    } else {
        Refusal::NotAFile
    };
    Ok(refused(path, why))
}

fn moved_while_read(path: &RepoPath) -> Error {
    Error::ContentReadsDisagree {
        path: path.to_string(),
        detail: "the file changed while its discard was being read".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::recording_stub::RecordingStub;

    fn path_of(length: usize, n: usize) -> RepoPath {
        RepoPath::new(format!("{n:0length$}"))
    }

    /// R3.5's bound: every path in exactly one batch, in order, no batch past the bound
    /// unless it is one path, and a list under the bound one batch. Caught by: dropping the
    /// last batch, an off-by-one at the boundary, or an empty batch.
    #[test]
    fn a_list_past_the_bound_is_split_and_every_path_kept_in_order() {
        let cost = |path: &RepoPath| path.as_bytes().len() + 1 + std::mem::size_of::<usize>();
        let few: Vec<RepoPath> = (0..10).map(|n| path_of(20, n)).collect();
        assert_eq!(clean_batches(&few), vec![few.as_slice()]);
        let many: Vec<RepoPath> = (0..2_000).map(|n| path_of(100, n)).collect();
        let batches = clean_batches(&many);
        assert!(batches.len() >= 3, "{} batches", batches.len());
        assert_eq!(
            batches.concat(),
            many,
            "a path was lost, doubled or reordered"
        );
        for batch in &batches {
            assert!(!batch.is_empty());
            let used: usize = batch.iter().map(cost).sum();
            assert!(used <= CLEAN_ARGUMENT_BYTES, "{used} bytes in one batch");
        }
        let exact = CLEAN_ARGUMENT_BYTES / cost(&path_of(100, 0));
        assert_eq!(clean_batches(&many[..exact]).len(), 1);
        assert_eq!(clean_batches(&many[..exact + 1]).len(), 2);
        assert!(clean_batches(&[]).is_empty());
    }

    /// C9 for a discard of lines: its re-check's git form asked of `git hash-object --path`
    /// as a read, then `git --literal-pathspecs apply --whitespace=nowarn -` as a write —
    /// no `--cached`, no `-R` — with the model's inverted patch on stdin.
    #[test]
    fn a_discard_of_lines_runs_as_r3_names_it() {
        let stub = RecordingStub::new();
        let (git, repo) = (stub.git_binary(), stub.repository());
        let diff = stub.diff("file.txt", WorkingTreeDiff::Unstaged);
        let selection = RecordingStub::first_change(&diff);
        let consequence = discard_lines_consequence(&git, &repo, &diff, selection.clone()).unwrap();
        stub.forget();
        discard_lines(&git, &repo, &diff, Confirmed::by_user(consequence), None).unwrap();
        let recorded = stub.recorded();
        assert_eq!(recorded.len(), 2, "{recorded:?}");
        assert_eq!(
            recorded[0].arguments_after_location(),
            ["hash-object", "--path=file.txt", "--", "file.txt"]
        );
        recorded[0].assert_a_read();
        assert_eq!(
            recorded[1].arguments_after_location(),
            ["--literal-pathspecs", "apply", "--whitespace=nowarn", "-"]
        );
        assert_eq!(
            recorded[1].stdin,
            cairn_model::action_patch(PatchAction::Discard, &diff, &selection).as_bytes()
        );
        recorded[1].assert_a_write();
    }

    /// C9 for a discard of files: `git --literal-pathspecs restore --worktree` from a
    /// pathspec file for the tracked file, then `git --literal-pathspecs clean -f -- <path>`
    /// for the untracked one — no `-d`, no second `-f` — each a write, and nothing read in
    /// between: the re-check reads the index with gix and hashes the bytes itself.
    #[test]
    fn a_discard_of_files_runs_as_r3_names_it() {
        let stub = RecordingStub::new();
        let (git, repo) = (stub.git_binary(), stub.repository());
        let consequence = discard_files_consequence(
            &git,
            &repo,
            &[RepoPath::new("file.txt"), RepoPath::new("new.txt")],
        )
        .unwrap();
        stub.forget();
        discard_files(&git, &repo, Confirmed::by_user(consequence), None).unwrap();
        let recorded = stub.recorded();
        let verbs: Vec<Vec<String>> = recorded
            .iter()
            .map(|record| record.arguments_after_location())
            .collect();
        assert_eq!(
            verbs,
            [
                vec![
                    "--literal-pathspecs",
                    "restore",
                    "--worktree",
                    "--pathspec-from-file=-",
                    "--pathspec-file-nul"
                ],
                vec!["--literal-pathspecs", "clean", "-f", "--", "new.txt"],
            ]
        );
        assert_eq!(recorded[0].stdin, b"file.txt\0");
        assert_eq!(recorded[1].stdin, b"");
        for record in &recorded {
            record.assert_a_write();
        }
    }
}
