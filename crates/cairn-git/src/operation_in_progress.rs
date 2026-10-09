//! The operation a repository is in the middle of, as git records it
//! (`docs/prd/staging-and-commit.md` R6.9, L25): what `git status` reads to say "You are
//! currently rebasing", and what decides whether a commit is the merge commit or refused.
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
//! - **a cherry-pick**: `CHERRY_PICK_HEAD`, when neither of the above holds; or the
//!   sequencer's todo list starting with a pick;
//! - **a revert**: `REVERT_HEAD`, or the sequencer's todo list starting with a revert.
//!
//! git can report two at once; the one answered is the one that decides a commit: a rebase,
//! `git am`, a cherry-pick or a revert — each of which refuses it (L25) — ahead of a merge.
//! A bisect is not an operation a commit is refused for, and is not reported. git reads
//! `CHERRY_PICK_HEAD` and `REVERT_HEAD` as refs, which a file that holds no id fails; here
//! the file existing is enough, so a damaged one refuses a commit git would also refuse to
//! continue.

use std::path::Path;

use cairn_model::OperationInProgress;

use crate::Repository;

impl Repository {
    /// What git is in the middle of here, or `None` (module docs). Reads a handful of paths
    /// in the git directory and, for a merge, its message; a file that cannot be read is
    /// taken as absent.
    pub fn operation_in_progress(&self) -> Option<OperationInProgress> {
        in_progress(self.git_dir())
    }
}

fn in_progress(git_dir: &Path) -> Option<OperationInProgress> {
    let exists = |name: &str| git_dir.join(name).exists();
    let merge = exists("MERGE_HEAD");
    let rebase_apply = git_dir.join("rebase-apply").is_dir();
    let applying = rebase_apply && exists("rebase-apply/applying");
    let rebase = (rebase_apply && !applying) || git_dir.join("rebase-merge").is_dir();
    let sequencer = next_sequencer_command(git_dir);
    let cherry_pick = (!merge && !rebase_apply && !rebase && exists("CHERRY_PICK_HEAD"))
        || sequencer == Some(Sequenced::Pick);
    let revert = exists("REVERT_HEAD") || sequencer == Some(Sequenced::Revert);
    if rebase {
        Some(OperationInProgress::Rebase)
    } else if applying {
        Some(OperationInProgress::ApplyingPatches)
    } else if cherry_pick {
        Some(OperationInProgress::CherryPick)
    } else if revert {
        Some(OperationInProgress::Revert)
    } else if merge {
        Some(OperationInProgress::Merge {
            message: std::fs::read(git_dir.join("MERGE_MSG"))
                .ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
        })
    } else {
        None
    }
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
    /// a refusing operation answers ahead of a merge, and `CHERRY_PICK_HEAD` beside a merge
    /// is the merge's, as `git status` reads it. The real states are made by real git in
    /// `tests/diff/commit.rs`; this holds the combinations git rarely leaves. Caught by: a
    /// merge answered while a rebase replays it, or gix's order.
    #[test]
    fn a_refusing_operation_answers_ahead_of_a_merge() {
        type Case<'a> = (
            &'a str,
            &'a [(&'a str, &'a str)],
            Option<OperationInProgress>,
        );
        let cases: [Case<'_>; 9] = [
            ("none", &[], None),
            (
                "merge",
                &[("MERGE_HEAD", "x\n"), ("MERGE_MSG", "Merge x\n")],
                Some(OperationInProgress::Merge {
                    message: Some("Merge x\n".to_owned()),
                }),
            ),
            (
                "merge-in-rebase",
                &[("MERGE_HEAD", "x\n"), ("rebase-merge/", "")],
                Some(OperationInProgress::Rebase),
            ),
            (
                "apply-rebase",
                &[("rebase-apply/", "")],
                Some(OperationInProgress::Rebase),
            ),
            (
                "am",
                &[("rebase-apply/applying", "")],
                Some(OperationInProgress::ApplyingPatches),
            ),
            (
                "pick-beside-merge",
                &[("MERGE_HEAD", "x\n"), ("CHERRY_PICK_HEAD", "y\n")],
                Some(OperationInProgress::Merge { message: None }),
            ),
            (
                "sequenced-pick",
                &[("sequencer/todo", "\n  pick abc subject\n")],
                Some(OperationInProgress::CherryPick),
            ),
            (
                "sequenced-revert",
                &[("sequencer/todo", "revert\tabc subject\n")],
                Some(OperationInProgress::Revert),
            ),
            (
                "sequenced-other",
                &[("sequencer/todo", "picked abc\n")],
                None,
            ),
        ];
        for (name, files, expected) in cases {
            let root = holding(name, files);
            assert_eq!(in_progress(&root), expected, "{name}");
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}
