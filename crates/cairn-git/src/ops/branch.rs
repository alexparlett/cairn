//! `Create Branch Here…` (`docs/prd/staging-and-commit.md` R11.3): a branch put on a commit —
//! the way back to a commit Show Lost Commits draws — as `git branch -- <name> <commit>`.
//!
//! The name follows `--`, so a name beginning with `-` is a name git judges and never an
//! option; the commit is given by its full id, never a revision expression. git refuses a
//! name that is taken or not a valid branch name, and its reason travels on the failure
//! ([`Error::GitFailed`]'s stderr), as the user's own `git branch` says it. Not destructive:
//! without `-f` git never moves a branch that exists, so it takes no `Confirmed`.

use std::ffi::OsString;

use cairn_model::{AskpassToken, Oid};

use super::local_write::{locks_around, locks_now, run};
use super::{GitBinary, Invalidated, Performed};
use crate::{Error, Repository};

/// Creates the branch `name` at `commit` in `repo`: `git branch -- <name> <commit>`, a write,
/// with `token` as the operation's askpass authorisation. Invalidates the refs.
pub fn create_branch(
    git: &GitBinary,
    repo: &Repository,
    name: &str,
    commit: Oid,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let before = locks_now(repo);
    run(git, repo, token, &branch_arguments(name, commit), None)?;
    Ok(Performed::new(
        format!("created branch {name} at {}", commit.short()),
        Invalidated::refs(),
    )
    .with_locks(locks_around(repo, before)))
}

/// `branch -- <name> <commit>`.
fn branch_arguments(name: &str, commit: Oid) -> Vec<OsString> {
    vec![
        OsString::from("branch"),
        OsString::from("--"),
        OsString::from(name),
        OsString::from(commit.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::recording_stub::RecordingStub;

    /// C9's rule for a write, and R11.3's argv: `--literal-pathspecs` ahead of the verb, the
    /// name after `--` whatever it begins with, the commit by its full id, nothing on stdin,
    /// and a write's environment. Caught by: a name read as an option, a short or symbolic
    /// commit, a missing `--`, or the verb run as a read.
    #[test]
    fn a_branch_is_created_by_git_branch_after_a_double_dash() {
        let stub = RecordingStub::new();
        let (git, repo) = (stub.git_binary(), stub.repository());
        let head = crate::object_id::model_id(&repo.inner().head_id().unwrap()).unwrap();
        create_branch(&git, &repo, "recovered", head, None).unwrap();
        let refused = create_branch(&git, &repo, "-f", head, None);
        assert!(
            matches!(refused, Err(Error::GitFailed { .. })),
            "git refuses `-f` as a branch name: {refused:?}"
        );
        let recorded = stub.recorded();
        let verbs: Vec<Vec<String>> = recorded
            .iter()
            .map(|record| record.arguments_after_location())
            .collect();
        let id = head.to_string();
        assert_eq!(
            verbs,
            [
                vec![
                    "--literal-pathspecs",
                    "branch",
                    "--",
                    "recovered",
                    id.as_str()
                ],
                vec!["--literal-pathspecs", "branch", "--", "-f", id.as_str()],
            ]
        );
        for record in &recorded {
            record.assert_a_write();
            assert!(record.stdin.is_empty(), "{record:?}");
        }
    }
}
