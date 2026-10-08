//! Staging and unstaging: lines and a mode change by patch, and whole files
//! (`docs/prd/staging-and-commit.md` R3.1, R3.2, R3.4). Not destructive (R1.5): what they
//! move between the index and `HEAD` is in the object store either way, and a wrong one is
//! undone by its opposite, so none takes a [`cairn_model::Confirmed`].
//!
//! **Lines** are a patch the model emits from the exact diff the user selected in
//! ([`cairn_model::action_patch`]) — the unstaged or untracked diff for a stage, the staged
//! diff for an unstage, inverted, so nothing is ever applied with `-R` — given to
//! `git apply --cached --whitespace=nowarn -` on stdin. `--whitespace=nowarn` overrides the
//! user's `apply.whitespace`, which would otherwise strip what was staged (`fix`) or refuse it
//! (`error`): what is staged is what was selected, byte for byte (C5). Never `--recount`,
//! `--3way`, `--unidiff-zero`, `-C` or `--allow-overlap`: the patch's counts and context are
//! exact, and a patch that does not apply as it is must not be made to. `apply.ignoreWhitespace`
//! is left to the user: measured on git 2.30.9, 2.32.7 and 2.56.0 over 400 selections, it
//! changes nothing a fresh patch stages — it matters only where the index moved under a
//! patch's context, which the stale check refuses first (`progress.md`, phase 03).
//!
//! **The stale check** (R3.7, L17f): before git runs, the index entry is read again with
//! gix and must be the side the patch was built from — for a stage, the diff's old id and
//! mode (or, for a new file, no entry or an intent-to-add one); for an unstage, the staged
//! diff's new id and mode at its path. Otherwise the patch is not applied: `git apply` would
//! land it at an offset on lines the user never selected (spike E6), so the answer is
//! [`Error::ChangedSinceRead`], naming the path, and nothing is written.
//!
//! **Files** go by git's own verbs, every path in a pathspec file on stdin
//! (`--pathspec-from-file=- --pathspec-file-nul`): stage with `git add`, which also stages a
//! deletion and marks a conflicted path resolved (R3.11); unstage with `git reset -q`, which
//! works on an unborn branch where `git restore --staged` does not ([`UnstageTo::Head`]);
//! out of an amend, against `HEAD`'s parent ([`UnstageTo::Commit`]); and out of a root
//! commit's amend, which has no parent to reset to, with `git rm --cached -q`
//! ([`UnstageTo::Nothing`]). A staged rename is unstaged whole by naming both its paths;
//! one path alone unstages that side alone, as `git reset -- <path>` does.
//!
//! Every verb runs with git's global `--literal-pathspecs` (`super::local_write`), and its
//! [`Performed`] declares the index invalid and carries the lock files around it (R3.8).

use std::ffi::OsString;

use cairn_model::{AskpassToken, DiffContent, FileDiff, Oid, PatchAction, RepoPath, Selection};

use super::fresh_state::{IndexSide, index_side};
use super::local_write::{PATHSPEC_FILE, locks_around, locks_now, pathspec_file, run};
use super::{GitBinary, Invalidated, Performed};
use crate::{Error, Refusal, Repository};

/// What `git apply` is given for a patch into the index: the patch on stdin.
const APPLY_CACHED: [&str; 4] = ["apply", "--cached", "--whitespace=nowarn", "-"];

/// Stages `selection` of `diff` — the path's unstaged or untracked diff, as the engine
/// answered it — into the index (R3.1).
///
/// Refused before git runs ([`Error::Refused`]) when nothing is selected, when the change is
/// one only a file verb takes whole, or when the path is conflicted; [`Error::ChangedSinceRead`]
/// when the index entry is no longer the diff's old side (module docs).
pub fn stage_lines(
    git: &GitBinary,
    repo: &Repository,
    diff: &FileDiff,
    selection: &Selection,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let path = &diff.file.new_path;
    let patch = patch_of(PatchAction::Stage, diff, selection)?;
    let side = index_side(repo, path)?;
    if side == IndexSide::Conflicted {
        return Err(refused(path, Refusal::Conflicted));
    }
    if !side.is_old_side(diff.file.old_id.as_ref(), diff.file.old_mode) {
        return Err(Error::ChangedSinceRead {
            path: path.to_string(),
        });
    }
    apply_cached(git, repo, token, patch).map(|locks| {
        Performed::new(format!("staged lines of {path}"), Invalidated::index()).with_locks(locks)
    })
}

/// Unstages `selection` of `diff` — the path's staged diff, as the engine answered it, a
/// rename or a copy paired as `git diff --cached` pairs it — out of the index (R3.2). The
/// lines go back at the path the change has now: unstaging lines of a rename never moves
/// the path, whichever of its rows asked (R2.6).
///
/// Refused as [`stage_lines`] is; [`Error::ChangedSinceRead`] when the index entry at the
/// path is no longer the staged diff's new side.
pub fn unstage_lines(
    git: &GitBinary,
    repo: &Repository,
    diff: &FileDiff,
    selection: &Selection,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let path = &diff.file.new_path;
    let patch = patch_of(PatchAction::Unstage, diff, selection)?;
    let side = index_side(repo, path)?;
    if side == IndexSide::Conflicted {
        return Err(refused(path, Refusal::Conflicted));
    }
    let current = match (&side, &diff.file.new_id) {
        (
            IndexSide::Entry {
                id,
                mode,
                intent_to_add: false,
            },
            Some(new_id),
        ) => id == new_id && *mode == diff.file.new_mode,
        (IndexSide::Absent | IndexSide::Conflicted | IndexSide::Entry { .. }, _) => false,
    };
    if !current {
        return Err(Error::ChangedSinceRead {
            path: path.to_string(),
        });
    }
    apply_cached(git, repo, token, patch).map(|locks| {
        Performed::new(format!("unstaged lines of {path}"), Invalidated::index()).with_locks(locks)
    })
}

/// The patch `action` applies for `selection` of `diff`, or the refusal that stands in for an
/// empty one: nothing selected, or part of a change only a file verb takes (R2.3).
pub(super) fn patch_of(
    action: PatchAction,
    diff: &FileDiff,
    selection: &Selection,
) -> Result<cairn_model::Patch, Error> {
    let path = &diff.file.new_path;
    match &diff.content {
        DiffContent::Conflicted => return Err(refused(path, Refusal::Conflicted)),
        DiffContent::Text { .. }
        | DiffContent::ModeChangeOnly
        | DiffContent::Binary { .. }
        | DiffContent::TooLarge { .. }
        | DiffContent::LfsPointer { .. }
        | DiffContent::Submodule { .. }
        | DiffContent::Unsupported { .. } => {}
    }
    if selection.is_empty() {
        return Err(refused(path, Refusal::NothingSelected));
    }
    let patch = cairn_model::action_patch(action, diff, selection);
    if patch.is_empty() {
        return Err(refused(path, Refusal::WholeFileOnly));
    }
    Ok(patch)
}

pub(super) fn refused(path: &RepoPath, why: Refusal) -> Error {
    Error::Refused {
        path: path.to_string(),
        why,
    }
}

/// `git apply --cached --whitespace=nowarn -`, the patch on stdin, and the locks around it.
fn apply_cached(
    git: &GitBinary,
    repo: &Repository,
    token: Option<&AskpassToken>,
    patch: cairn_model::Patch,
) -> Result<super::Locks, Error> {
    let before = locks_now(repo);
    run(
        git,
        repo,
        token,
        &APPLY_CACHED.map(OsString::from),
        Some(patch.as_bytes().to_vec()),
    )?;
    Ok(locks_around(repo, before))
}

/// Stages every one of `paths` whole with `git add`, from a pathspec file (R3.4): a
/// modification, a new file, a deletion, a submodule's new commit, and a conflicted path —
/// which `git add` marks resolved (R3.11). Paths are work-tree-relative, as `git status`
/// lists them; an empty list is [`Error::NoPaths`].
pub fn stage_files(
    git: &GitBinary,
    repo: &Repository,
    paths: &[RepoPath],
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let input = checked_pathspec_file(paths)?;
    let before = locks_now(repo);
    let mut arguments = vec![OsString::from("add")];
    arguments.extend(PATHSPEC_FILE.map(OsString::from));
    run(git, repo, token, &arguments, Some(input))?;
    Ok(Performed::new(
        format!("staged {}", files(paths.len())),
        Invalidated::index(),
    )
    .with_locks(locks_around(repo, before)))
}

/// What a whole-file unstage puts the index entries back to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnstageTo {
    /// `HEAD`'s entries, or none on an unborn branch: `git reset -q`.
    Head,
    /// A commit's entries: `HEAD`'s parent, for a file unstaged out of an amend (R6.3) —
    /// `git reset -q <commit>`, the commit given by its id, which is never an option.
    Commit(Oid),
    /// No entry at all: a file unstaged out of the amend of a root commit, which has no
    /// parent to reset to — `git rm --cached -q`, which git refuses where the staged
    /// content differs from both the file and `HEAD`.
    Nothing,
}

/// Unstages every one of `paths` whole, back to `to`, from a pathspec file (R3.4). A staged
/// rename is unstaged whole by naming both its paths. An empty list is [`Error::NoPaths`].
pub fn unstage_files(
    git: &GitBinary,
    repo: &Repository,
    paths: &[RepoPath],
    to: &UnstageTo,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let input = checked_pathspec_file(paths)?;
    let before = locks_now(repo);
    run(git, repo, token, &unstage_arguments(to), Some(input))?;
    Ok(Performed::new(
        format!("unstaged {}", files(paths.len())),
        Invalidated::index(),
    )
    .with_locks(locks_around(repo, before)))
}

fn unstage_arguments(to: &UnstageTo) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = match to {
        UnstageTo::Head | UnstageTo::Commit(_) => vec!["reset".into(), "-q".into()],
        UnstageTo::Nothing => vec!["rm".into(), "--cached".into(), "-q".into()],
    };
    arguments.extend(PATHSPEC_FILE.map(OsString::from));
    if let UnstageTo::Commit(commit) = to {
        arguments.push(commit.to_string().into());
    }
    arguments
}

/// The pathspec file for `paths`, each checked work-tree-relative first: an absolute path or
/// one with a `.` or `..` component could name a file outside the working tree.
pub(super) fn checked_pathspec_file(paths: &[RepoPath]) -> Result<Vec<u8>, Error> {
    if paths.is_empty() {
        return Err(Error::NoPaths);
    }
    for path in paths {
        crate::reads::work_tree_relative(path)?;
    }
    Ok(pathspec_file(paths))
}

fn files(count: usize) -> String {
    if count == 1 {
        "1 file".to_owned()
    } else {
        format!("{count} files")
    }
}

/// Against a `git` that records what it was given and runs the real one (C9's argv,
/// environment and stdin for each verb here, since the runner is crate-private). What the
/// verbs do to a repository is pinned against real git alone in
/// `crates/cairn-git/tests/diff/write_verbs.rs`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorkingTreeDiff;
    use crate::ops::recording_stub::RecordingStub;

    fn strings(arguments: &[OsString]) -> Vec<String> {
        arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }

    /// Caught by: `restore --staged` (which fails on an unborn branch), `-R`, a pathspec on
    /// argv, or the commit spelled as a revision expression rather than its id.
    #[test]
    fn unstaging_resets_or_removes_from_a_pathspec_file() {
        assert_eq!(
            strings(&unstage_arguments(&UnstageTo::Head)),
            [
                "reset",
                "-q",
                "--pathspec-from-file=-",
                "--pathspec-file-nul"
            ]
        );
        let parent = Oid::parse("07da224c7ec04501dfb451be161fa962effe1dc1").unwrap();
        assert_eq!(
            strings(&unstage_arguments(&UnstageTo::Commit(parent))),
            [
                "reset",
                "-q",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
                "07da224c7ec04501dfb451be161fa962effe1dc1"
            ]
        );
        assert_eq!(
            strings(&unstage_arguments(&UnstageTo::Nothing)),
            [
                "rm",
                "--cached",
                "-q",
                "--pathspec-from-file=-",
                "--pathspec-file-nul"
            ]
        );
    }

    /// C9 for the file verbs: `--literal-pathspecs` before the verb on argv, the paths in a
    /// NUL-separated pathspec file on stdin and nowhere on argv, and the write's environment
    /// — no `GIT_LITERAL_PATHSPECS`, no read pin.
    #[test]
    fn the_file_verbs_run_as_r3_names_them() {
        let stub = RecordingStub::new();
        let (git, repo) = (stub.git_binary(), stub.repository());
        let paths = [RepoPath::new("new.txt"), RepoPath::new("file.txt")];
        stage_files(&git, &repo, &paths, None).unwrap();
        unstage_files(&git, &repo, &paths, &UnstageTo::Head, None).unwrap();
        stage_files(&git, &repo, &paths[..1], None).unwrap();
        unstage_files(&git, &repo, &paths[..1], &UnstageTo::Nothing, None).unwrap();
        let recorded = stub.recorded();
        let verbs: Vec<Vec<String>> = recorded
            .iter()
            .map(|record| record.arguments_after_location())
            .collect();
        let add = [
            "--literal-pathspecs",
            "add",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ];
        let reset = [
            "--literal-pathspecs",
            "reset",
            "-q",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ];
        let rm = [
            "--literal-pathspecs",
            "rm",
            "--cached",
            "-q",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ];
        assert_eq!(
            verbs,
            [add.to_vec(), reset.to_vec(), add.to_vec(), rm.to_vec()]
        );
        let stdins: Vec<&[u8]> = recorded
            .iter()
            .map(|record| record.stdin.as_slice())
            .collect();
        assert_eq!(
            stdins,
            [
                b"new.txt\0file.txt\0".as_slice(),
                b"new.txt\0file.txt\0",
                b"new.txt\0",
                b"new.txt\0"
            ]
        );
        for record in &recorded {
            record.assert_a_write();
        }
    }

    /// C9 for the patch verbs: `git --literal-pathspecs apply --cached --whitespace=nowarn -`,
    /// never `-R`, `--recount`, `--3way` or `--unidiff-zero`, the model's patch on stdin, run
    /// as a write; and the stale check reads the index with gix, so no read runs beside it.
    #[test]
    fn the_patch_verbs_run_as_r3_names_them() {
        let stub = RecordingStub::new();
        let (git, repo) = (stub.git_binary(), stub.repository());
        let unstaged = stub.diff("file.txt", WorkingTreeDiff::Unstaged);
        let to_stage = RecordingStub::first_change(&unstaged);
        let staged = stub.diff("file.txt", WorkingTreeDiff::Staged);
        let to_unstage = RecordingStub::first_change(&staged);
        stage_lines(&git, &repo, &unstaged, &to_stage, None).unwrap();
        let recorded = stub.recorded();
        stub.forget();
        let staged = stub.diff("file.txt", WorkingTreeDiff::Staged);
        unstage_lines(&git, &repo, &staged, &to_unstage, None).unwrap();
        let recorded = [recorded, stub.recorded()].concat();
        assert_eq!(recorded.len(), 2, "{recorded:?}");
        for (record, patch) in recorded.iter().zip([
            cairn_model::action_patch(PatchAction::Stage, &unstaged, &to_stage),
            cairn_model::action_patch(PatchAction::Unstage, &staged, &to_unstage),
        ]) {
            assert_eq!(
                record.arguments_after_location(),
                [
                    "--literal-pathspecs",
                    "apply",
                    "--cached",
                    "--whitespace=nowarn",
                    "-"
                ]
            );
            assert_eq!(
                record.stdin,
                patch.as_bytes(),
                "the patch on stdin is not the model's"
            );
            record.assert_a_write();
        }
    }
}
