//! An operation git has begun in a repository and not finished — a merge, a rebase, a
//! cherry-pick, a revert or `git am` — as git records it in the git directory
//! (`docs/prd/staging-and-commit.md` R6.9, L25). The engine reads it; the commit box draws
//! it; a commit or an amend is refused during any of them but a merge, whose commit is the
//! merge commit.

/// What git is in the middle of, as `git status` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationInProgress {
    /// `MERGE_HEAD` exists: a commit now is the merge commit, its parents `HEAD` and
    /// `MERGE_HEAD`. `message` is git's `MERGE_MSG`, as it stands, when there is one — what
    /// the commit box fills an empty draft with (R10.8); git's own comment lines in it are
    /// git's, and the commit's cleanup decides what becomes of them.
    Merge { message: Option<String> },
    /// A rebase stopped part way (`rebase-merge/` or `rebase-apply/`).
    Rebase,
    /// `git am` stopped part way (`rebase-apply/applying`).
    ApplyingPatches,
    /// A cherry-pick stopped part way (`CHERRY_PICK_HEAD`, or the sequencer's next pick).
    CherryPick,
    /// A revert stopped part way (`REVERT_HEAD`, or the sequencer's next revert).
    Revert,
}

impl OperationInProgress {
    /// Whether a commit or an amend is refused while it runs: every one but a merge (L25).
    pub fn refuses_commit(&self) -> bool {
        match self {
            Self::Merge { .. } => false,
            Self::Rebase | Self::ApplyingPatches | Self::CherryPick | Self::Revert => true,
        }
    }

    /// Whether an amend is refused while it runs: every one, a merge included — git refuses
    /// to amend in the middle of a merge (R6.3).
    pub fn refuses_amend(&self) -> bool {
        match self {
            Self::Merge { .. }
            | Self::Rebase
            | Self::ApplyingPatches
            | Self::CherryPick
            | Self::Revert => true,
        }
    }

    /// Its name in a sentence: "a rebase is in progress".
    pub fn name(&self) -> &'static str {
        match self {
            Self::Merge { .. } => "a merge",
            Self::Rebase => "a rebase",
            Self::ApplyingPatches => "git am",
            Self::CherryPick => "a cherry-pick",
            Self::Revert => "a revert",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L25: a merge is committed, never amended; everything else refuses both. Caught by: a
    /// merge refused (no merge commit could be made), or a rebase let through.
    #[test]
    fn only_a_merge_lets_a_commit_through_and_nothing_lets_an_amend() {
        let merge = OperationInProgress::Merge { message: None };
        assert!(!merge.refuses_commit());
        assert!(merge.refuses_amend());
        for other in [
            OperationInProgress::Rebase,
            OperationInProgress::ApplyingPatches,
            OperationInProgress::CherryPick,
            OperationInProgress::Revert,
        ] {
            assert!(other.refuses_commit(), "{other:?}");
            assert!(other.refuses_amend(), "{other:?}");
        }
    }

    #[test]
    fn each_is_named_as_a_sentence_names_it() {
        assert_eq!(
            [
                OperationInProgress::Merge { message: None }.name(),
                OperationInProgress::Rebase.name(),
                OperationInProgress::ApplyingPatches.name(),
                OperationInProgress::CherryPick.name(),
                OperationInProgress::Revert.name(),
            ],
            ["a merge", "a rebase", "git am", "a cherry-pick", "a revert"]
        );
    }
}
