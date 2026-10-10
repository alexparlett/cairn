//! The operation a repository is in the middle of, as git records it
//! (`docs/prd/staging-and-commit.md` R6.9, L25): what `git status` reads to say "You are
//! currently rebasing", and what decides whether a commit concludes it or is refused — and the
//! message git prepared for the commit that concludes one, cleaned as git's editor cleans it
//! (R6.10).
//!
//! gix has `Repository::state`, and it is not used: it checks in another order than git
//! (a cherry-pick ahead of a merge, a bisect ahead of a revert) and never reads the
//! sequencer, so a cherry-pick sequence whose conflict was resolved and committed — no
//! `CHERRY_PICK_HEAD` left, the next picks still in `sequencer/todo` — reads as nothing
//! while `git status` says a cherry-pick is in progress. So this reads the files git's
//! `wt_status_get_state` reads (`wt-status.c`, the same at v2.30.0 and v2.56.0), in the
//! per-worktree git directory:
//!
//! - **a merge**: `MERGE_HEAD` exists; its message is `MERGE_MSG`;
//! - **a rebase or `git am`**: `rebase-apply/` (`applying` inside it for `git am`) or
//!   `rebase-merge/`, checked whether or not a merge is in progress too, as git does — a
//!   rebase replaying a merge stops with `MERGE_HEAD` written;
//! - **a sequence of picks or reverts**: the sequencer's todo list starting with a pick or a
//!   revert (`sequencer_get_last_command`) — what `git cherry-pick A B` or `git revert A..B`
//!   leaves when it stops, `CHERRY_PICK_HEAD` or `REVERT_HEAD` beside it or not;
//! - **a single cherry-pick**: `CHERRY_PICK_HEAD`, when neither a merge nor a rebase holds and
//!   no sequence does — what `git cherry-pick <commit>` leaves when it stops, git touching no
//!   sequencer state for one commit (`sequencer_pick_revisions`); its message is `MERGE_MSG`;
//! - **a single revert**: `REVERT_HEAD`, no sequence; its message is `MERGE_MSG`.
//!
//! git can report two at once; the one answered is the one that decides a commit: a rebase,
//! `git am` or a sequence — each of which refuses it — ahead of a single cherry-pick or revert,
//! ahead of a merge. A bisect is not an operation a commit is refused for, and is not reported.
//! git reads `CHERRY_PICK_HEAD` and `REVERT_HEAD` as refs; here the file existing is enough, and
//! the commit it names is read where it holds an id, for the box to name.
//!
//! A commit concludes a merge, a single cherry-pick or a single revert as `git commit` does — the
//! picked commit's author kept, `CHERRY_PICK_HEAD` or `REVERT_HEAD` removed by git
//! (`tests/diff/commit.rs`, on git 2.30.9, 2.32.7 and the host's). A detached `HEAD` is no
//! operation: the refs snapshot reports it (`HeadState::Detached`), and a commit there is made
//! on no branch, refused by nothing (C8).

use std::path::Path;

use cairn_model::{Oid, OperationInProgress};

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

impl Repository {
    /// What git is in the middle of here, or `None` (module docs). Reads a handful of paths
    /// in the git directory; a file that cannot be read is taken as absent.
    pub fn operation_in_progress(&self) -> Option<OperationInProgress> {
        in_progress(self.git_dir())
    }

    /// The message git prepared for the commit that concludes a merge, a cherry-pick or a revert
    /// — `MERGE_MSG` — cleaned of git's commentary as `git stripspace --strip-comments` cleans
    /// it, git reading the comment character (R6.10, `crate::reads::stripspace`); `None` where
    /// git prepared none. The commit box fills an empty draft with it, so what it shows is what
    /// is committed.
    pub fn prepared_message(
        &self,
        git: &GitBinary,
        cancel: &impl Cancel,
    ) -> Result<Option<String>, Error> {
        let Ok(written) = std::fs::read(self.git_dir().join("MERGE_MSG")) else {
            return Ok(None);
        };
        let cleaned = crate::reads::stripspace(git, self, &written, cancel)?;
        Ok(Some(String::from_utf8_lossy(&cleaned).into_owned()))
    }
}

fn in_progress(git_dir: &Path) -> Option<OperationInProgress> {
    let exists = |name: &str| git_dir.join(name).exists();
    let merge = exists("MERGE_HEAD");
    let rebase_apply = git_dir.join("rebase-apply").is_dir();
    let applying = rebase_apply && exists("rebase-apply/applying");
    let rebase = (rebase_apply && !applying) || git_dir.join("rebase-merge").is_dir();
    let sequence = next_sequencer_command(git_dir);
    let cherry_pick = !merge && !rebase_apply && !rebase && exists("CHERRY_PICK_HEAD");
    if rebase {
        Some(OperationInProgress::Rebase)
    } else if applying {
        Some(OperationInProgress::ApplyingPatches)
    } else if sequence == Some(Sequenced::Pick) {
        Some(OperationInProgress::CherryPickSequence)
    } else if sequence == Some(Sequenced::Revert) {
        Some(OperationInProgress::RevertSequence)
    } else if cherry_pick {
        Some(OperationInProgress::CherryPick {
            picked: named(git_dir, "CHERRY_PICK_HEAD"),
        })
    } else if exists("REVERT_HEAD") {
        Some(OperationInProgress::Revert {
            reverted: named(git_dir, "REVERT_HEAD"),
        })
    } else if merge {
        Some(OperationInProgress::Merge)
    } else {
        None
    }
}

/// The commit a pseudo-ref file names, when its first line is one id.
fn named(git_dir: &Path, name: &str) -> Option<Oid> {
    let text = std::fs::read_to_string(git_dir.join(name)).ok()?;
    Oid::parse(text.lines().next()?.trim()).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sequenced {
    Pick,
    Revert,
}

/// The first command of `sequencer/todo`, as git's `sequencer_get_last_command` reads it:
/// leading whitespace skipped, then `pick` (or `p`) or `revert`, followed by a space or a
/// tab.
fn next_sequencer_command(git_dir: &Path) -> Option<Sequenced> {
    let todo = std::fs::read(git_dir.join("sequencer/todo")).ok()?;
    let start = todo
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))?;
    let line = &todo[start..];
    let command = |word: &[u8]| {
        line.strip_prefix(word)
            .is_some_and(|rest| matches!(rest.first(), Some(b' ' | b'\t')))
    };
    if command(b"pick") || command(b"p") {
        Some(Sequenced::Pick)
    } else if command(b"revert") {
        Some(Sequenced::Revert)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A git directory holding exactly `files` (a trailing `/` makes a directory).
    fn holding(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("cairn-in-progress-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        for (path, content) in files {
            let at = root.join(path);
            if path.ends_with('/') {
                std::fs::create_dir_all(&at).unwrap();
            } else {
                std::fs::create_dir_all(at.parent().unwrap()).unwrap();
                std::fs::write(&at, content).unwrap();
            }
        }
        root
    }

    /// The precedence git's own checks imply, each case a git directory as git leaves it:
    /// a refusing operation answers ahead of a merge or a single pick, a sequence ahead of the
    /// single pick it stopped on, and `CHERRY_PICK_HEAD` beside a merge is the merge's, as `git
    /// status` reads it. The real states are made by real git in `tests/diff/commit.rs`; this
    /// holds the combinations git rarely leaves. Caught by: a merge answered while a rebase
    /// replays it, a sequence's stopped pick concluded as a single one, or gix's order.
    #[test]
    fn a_refusing_operation_answers_ahead_of_one_a_commit_concludes() {
        const PICKED: &str = "1234567890abcdef1234567890abcdef12345678";
        let picked = Oid::parse(PICKED).ok();
        let head = format!("{PICKED}\n");
        type Case<'a> = (
            &'a str,
            Vec<(&'a str, &'a str)>,
            Option<OperationInProgress>,
        );
        let cases: Vec<Case<'_>> = vec![
            ("none", vec![], None),
            (
                "merge",
                vec![("MERGE_HEAD", "x\n"), ("MERGE_MSG", "Merge x\n")],
                Some(OperationInProgress::Merge),
            ),
            (
                "merge-in-rebase",
                vec![("MERGE_HEAD", "x\n"), ("rebase-merge/", "")],
                Some(OperationInProgress::Rebase),
            ),
            (
                "apply-rebase",
                vec![("rebase-apply/", "")],
                Some(OperationInProgress::Rebase),
            ),
            (
                "am",
                vec![("rebase-apply/applying", "")],
                Some(OperationInProgress::ApplyingPatches),
            ),
            (
                "pick-beside-merge",
                vec![("MERGE_HEAD", "x\n"), ("CHERRY_PICK_HEAD", "y\n")],
                Some(OperationInProgress::Merge),
            ),
            (
                "single-pick",
                vec![("CHERRY_PICK_HEAD", head.as_str())],
                Some(OperationInProgress::CherryPick { picked }),
            ),
            (
                "single-pick-unreadable",
                vec![("CHERRY_PICK_HEAD", "not an id\n")],
                Some(OperationInProgress::CherryPick { picked: None }),
            ),
            (
                "single-revert",
                vec![("REVERT_HEAD", head.as_str())],
                Some(OperationInProgress::Revert { reverted: picked }),
            ),
            (
                "sequenced-pick",
                vec![
                    ("CHERRY_PICK_HEAD", head.as_str()),
                    ("sequencer/todo", "\n  pick abc subject\n"),
                ],
                Some(OperationInProgress::CherryPickSequence),
            ),
            (
                "sequenced-pick-resolved",
                vec![("sequencer/todo", "p abc subject\n")],
                Some(OperationInProgress::CherryPickSequence),
            ),
            (
                "sequenced-revert",
                vec![("sequencer/todo", "revert\tabc subject\n")],
                Some(OperationInProgress::RevertSequence),
            ),
            (
                "sequenced-other",
                vec![("sequencer/todo", "picked abc\n")],
                None,
            ),
        ];
        for (name, files, expected) in cases {
            let root = holding(name, &files);
            assert_eq!(in_progress(&root), expected, "{name}");
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}
