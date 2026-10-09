//! `git check-ref-format --branch <name>`: whether git takes `name` as a new branch's name
//! (`docs/prd/staging-and-commit.md` R11.3; the user's decision, 2026-10-09: Create Branch
//! refuses a name inline, before git runs, by git's own rules). git answers a name it takes by
//! printing the branch it names, and refuses one with its reason on stderr — the reason the
//! dialog shows, as the user's own `git branch` would say it.
//!
//! Query plumbing: it reads configuration, and for `@{-N}` the reflog, and writes nothing,
//! takes no lock and runs nothing (`checking_a_branch_name_writes_nothing`,
//! `crates/cairn-git/tests/diff/branch.rs`). The name is the argument after `--branch` and is
//! read by git as the name whatever it begins with.

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

const VERB: [&str; 2] = ["check-ref-format", "--branch"];

/// A branch name and its newline, with room to spare.
const CEILING: usize = 64 * 1024;

/// What git says of `name` as a branch's name: `Ok` with the name it takes it as, or `Err`
/// with its reason, the first line of what it wrote, `fatal: ` left off. Cancelled through
/// `cancel`, as any read is.
pub(crate) fn branch_name(
    git: &GitBinary,
    repo: &Repository,
    name: &str,
    cancel: &impl Cancel,
) -> Result<Result<String, String>, Error> {
    let answer = git
        .read_invocation()
        .in_repository(repo)
        .args(VERB)
        .arg(name)
        .start()?
        .collect(cancel, CEILING, |_| {});
    match answer {
        Ok(output) => {
            let printed = String::from_utf8_lossy(output.stdout());
            Ok(Ok(printed.trim_end_matches('\n').to_owned()))
        }
        Err(Error::GitFailed { stderr, .. }) => {
            let first = stderr.lines().next().unwrap_or_default();
            Ok(Err(first
                .strip_prefix("fatal: ")
                .unwrap_or(first)
                .to_owned()))
        }
        Err(other) => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: another verb, or a name read as an option of git's.
    #[test]
    fn the_read_is_check_ref_format_of_a_branch() {
        assert_eq!(VERB, ["check-ref-format", "--branch"]);
    }
}
