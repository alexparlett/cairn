//! Whether a name can be a new branch's, as Create Branch asks before git runs
//! (`docs/prd/staging-and-commit.md` R11.3; the user's decision, 2026-10-09).

/// What the engine answered of a name typed for a new branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchName {
    /// git takes it, and no local branch has it.
    Free,
    /// git takes it, and a local branch has it already.
    Taken,
    /// git does not take it as a branch's name: its reason, as git words it.
    Refused { reason: String },
}

impl BranchName {
    /// Why `name` cannot be created, as the dialog says it beside its button — Fork's words for
    /// a name taken ("Branch test already exists"), git's for a name it refuses — or `None`
    /// when it can.
    pub fn refusal(&self, name: &str) -> Option<String> {
        match self {
            Self::Free => None,
            Self::Taken => Some(format!("Branch {name} already exists")),
            Self::Refused { reason } => Some(reason.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a free name refused, a taken one not said in Fork's words, or git's reason
    /// rewritten.
    #[test]
    fn a_name_is_refused_in_forks_words_when_taken_and_gits_when_invalid() {
        assert_eq!(BranchName::Free.refusal("topic"), None);
        assert_eq!(
            BranchName::Taken.refusal("test").as_deref(),
            Some("Branch test already exists")
        );
        let refused = BranchName::Refused {
            reason: "'a..b' is not a valid branch name".to_owned(),
        };
        assert_eq!(
            refused.refusal("a..b").as_deref(),
            Some("'a..b' is not a valid branch name")
        );
    }
}
