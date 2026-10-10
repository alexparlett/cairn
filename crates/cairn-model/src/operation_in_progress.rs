//! An operation git has begun in a repository and not finished — a merge, a single cherry-pick
//! or revert, a sequence of them, a rebase or `git am` — as git records it in the git directory
//! (`docs/prd/staging-and-commit.md` R6.9, L25). The engine reads it; the commit box draws it.
//! A commit concludes a merge, a single cherry-pick and a single revert, as `git commit` does;
//! a rebase, `git am` and a sequence refuse a commit and an amend, since continuing one is that
//! operation's own, and are named with git's command to continue or abort it. An amend is
//! refused during every one of them, as git refuses it.

use crate::Oid;

/// What git is in the middle of, as `git status` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationInProgress {
    /// `MERGE_HEAD` exists: a commit now is the merge commit, its parents `HEAD` and
    /// `MERGE_HEAD`.
    Merge,
    /// One cherry-pick stopped part way: `CHERRY_PICK_HEAD`, no sequence of picks. A commit
    /// concludes it, keeping the picked commit's author, and git removes `CHERRY_PICK_HEAD`.
    /// `picked` is the commit `CHERRY_PICK_HEAD` names, when it holds an id.
    CherryPick { picked: Option<Oid> },
    /// One revert stopped part way: `REVERT_HEAD`, no sequence. A commit concludes it, and git
    /// removes `REVERT_HEAD`. `reverted` is the commit `REVERT_HEAD` names, when it holds an id.
    Revert { reverted: Option<Oid> },
    /// A sequence of cherry-picks stopped part way (`sequencer/`, its next command a pick):
    /// continued or aborted with `git cherry-pick`, never concluded by a commit.
    CherryPickSequence,
    /// A sequence of reverts stopped part way (`sequencer/`, its next command a revert).
    RevertSequence,
    /// A rebase stopped part way (`rebase-merge/` or `rebase-apply/`).
    Rebase,
    /// `git am` stopped part way (`rebase-apply/applying`).
    ApplyingPatches,
}

impl OperationInProgress {
    /// Whether a commit is refused while it runs: a rebase, `git am` and a sequence of picks or
    /// reverts. A merge, a single cherry-pick and a single revert are concluded by the commit.
    pub fn refuses_commit(&self) -> bool {
        match self {
            Self::Merge | Self::CherryPick { .. } | Self::Revert { .. } => false,
            Self::CherryPickSequence
            | Self::RevertSequence
            | Self::Rebase
            | Self::ApplyingPatches => true,
        }
    }

    /// Whether an amend is refused while it runs: every one — git refuses to amend in the middle
    /// of a merge, a cherry-pick or a revert, and an amend is no way to continue a sequence
    /// (R6.3).
    pub fn refuses_amend(&self) -> bool {
        match self {
            Self::Merge
            | Self::CherryPick { .. }
            | Self::Revert { .. }
            | Self::CherryPickSequence
            | Self::RevertSequence
            | Self::Rebase
            | Self::ApplyingPatches => true,
        }
    }

    /// Whether a commit concludes it, so the message git prepared for it (`MERGE_MSG`) is the
    /// draft's: a merge, a single cherry-pick, a single revert.
    pub fn concluded_by_commit(&self) -> bool {
        !self.refuses_commit()
    }

    /// Its name in a sentence: "a rebase is in progress".
    pub fn name(&self) -> &'static str {
        match self {
            Self::Merge => "a merge",
            Self::CherryPick { .. } | Self::CherryPickSequence => "a cherry-pick",
            Self::Revert { .. } | Self::RevertSequence => "a revert",
            Self::Rebase => "a rebase",
            Self::ApplyingPatches => "git am",
        }
    }

    /// git's own command for it, whose `--continue` and `--abort` continue or abort it: what a
    /// refusal names, so the person knows what to run.
    pub fn git_command(&self) -> &'static str {
        match self {
            Self::Merge => "git merge",
            Self::CherryPick { .. } | Self::CherryPickSequence => "git cherry-pick",
            Self::Revert { .. } | Self::RevertSequence => "git revert",
            Self::Rebase => "git rebase",
            Self::ApplyingPatches => "git am",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every() -> [OperationInProgress; 7] {
        let id = Oid::from_bytes(&[0xab; 20]).ok();
        [
            OperationInProgress::Merge,
            OperationInProgress::CherryPick { picked: id },
            OperationInProgress::Revert { reverted: id },
            OperationInProgress::CherryPickSequence,
            OperationInProgress::RevertSequence,
            OperationInProgress::Rebase,
            OperationInProgress::ApplyingPatches,
        ]
    }

    /// L25 as the redesign narrowed it (C3): a merge, a single cherry-pick and a single revert
    /// are concluded by a commit; a sequence, a rebase and `git am` refuse it; nothing lets an
    /// amend through. Caught by: a single pick refused (Fork and git conclude it), a sequence
    /// concluded (its next picks would be left to the terminal unsaid), or an amend let through.
    #[test]
    fn a_commit_concludes_a_merge_or_a_single_pick_and_nothing_lets_an_amend() {
        let concluded: Vec<bool> = every().iter().map(|op| op.concluded_by_commit()).collect();
        assert_eq!(
            concluded,
            [true, true, true, false, false, false, false],
            "{:?}",
            every()
        );
        for operation in every() {
            assert_eq!(operation.refuses_commit(), !operation.concluded_by_commit());
            assert!(operation.refuses_amend(), "{operation:?}");
        }
    }

    /// Each is named as git's status names it, with the command that continues or aborts it.
    #[test]
    fn each_is_named_with_gits_own_command() {
        let named: Vec<(&str, &str)> = every()
            .iter()
            .map(|op| (op.name(), op.git_command()))
            .collect();
        assert_eq!(
            named,
            [
                ("a merge", "git merge"),
                ("a cherry-pick", "git cherry-pick"),
                ("a revert", "git revert"),
                ("a cherry-pick", "git cherry-pick"),
                ("a revert", "git revert"),
                ("a rebase", "git rebase"),
                ("git am", "git am"),
            ]
        );
    }
}
