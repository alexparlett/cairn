//! Which of the hooks a commit's skip passes over git would run
//! (`docs/prd/staging-and-commit.md` R6.6, R10.5): `--no-verify` skips `pre-commit` and
//! `commit-msg` and nothing else, so the commit box offers to skip hooks only where one of
//! those two would run.

/// The two hooks `--no-verify` skips, each `true` where git would run it: its file exists in
/// the hooks directory git resolves and is executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommitHooks {
    pub pre_commit: bool,
    pub commit_msg: bool,
}

impl CommitHooks {
    /// Whether skipping hooks would skip anything: the one condition on which the skip is
    /// offered.
    pub fn skippable(&self) -> bool {
        self.pre_commit || self.commit_msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a skip offered with no hook to skip, or withheld with one.
    #[test]
    fn the_skip_is_offered_exactly_where_either_hook_would_run() {
        let hooks = |pre_commit, commit_msg| CommitHooks {
            pre_commit,
            commit_msg,
        };
        assert!(!hooks(false, false).skippable());
        assert!(hooks(true, false).skippable());
        assert!(hooks(false, true).skippable());
        assert!(hooks(true, true).skippable());
        assert_eq!(CommitHooks::default(), hooks(false, false));
    }
}
