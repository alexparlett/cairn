//! Whether a name can be a new branch's, as Create Branch asks before git runs
//! (`docs/prd/staging-and-commit.md` R11.3; the user's decision, 2026-10-09; the refusal typed
//! by the review's M3, 2026-10-10). The engine answers what is true of the name; the view
//! words it.

/// What the engine answered of a name typed for a new branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchName {
    /// git takes it, and no local branch has it or clashes with it.
    Free,
    /// It cannot be created, and why.
    Refused(NameRefusal),
}

/// Why a name cannot be a new branch's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameRefusal {
    /// It holds `@{`, which git reads as the start of a reflog selector (the user's decision F,
    /// 2026-10-09): refused before git is asked.
    AtBrace,
    /// git does not take it as a branch's name (`git check-ref-format --branch`): git's reason,
    /// its first line, `fatal: ` left off.
    Invalid { reason: String },
    /// A local branch has it already.
    Taken,
    /// A folder holds it: a branch already sits at a folder on the way to it — `leaf` for
    /// `leaf/child` — so git cannot make a ref under that branch. `branch` is the branch's full
    /// ref, `refs/heads/leaf`.
    InsideABranch { branch: String },
    /// It holds a branch: branches already sit under it as a folder — `folder/inner` for
    /// `folder` — so git cannot make a ref of that folder. `branch` is one such branch's full
    /// ref, `refs/heads/folder/inner`.
    HoldsABranch { branch: String },
}

impl BranchName {
    /// Why it cannot be created, or `None` when it can.
    pub fn refusal(&self) -> Option<&NameRefusal> {
        match self {
            Self::Free => None,
            Self::Refused(why) => Some(why),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a free name answered with a refusal, or a refusal lost.
    #[test]
    fn a_refused_name_says_why_and_a_free_one_nothing() {
        assert_eq!(BranchName::Free.refusal(), None);
        assert_eq!(
            BranchName::Refused(NameRefusal::Taken).refusal(),
            Some(&NameRefusal::Taken)
        );
    }
}
