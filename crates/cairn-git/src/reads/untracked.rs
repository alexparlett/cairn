//! Every untracked path, whatever `status.showUntrackedFiles` says: `git ls-files --others
//! --exclude-standard -z`, for Create Branch's discard, which must name every untracked file a
//! forced checkout deletes or overwrites (`docs/prd/staging-and-commit.md` R11.3; phase 10's QA,
//! items 1-3). Each untracked file is listed, and a repository nested in the working tree as its
//! directory, with a trailing `/`, git not looking inside it.
//!
//! Query plumbing: it reads the index and the working tree and writes nothing — no index
//! refresh, no lock (`checking_a_branch_name_writes_nothing`'s sibling,
//! `a_discarding_checkout_names_every_loss_and_then_loses_exactly_those`, reads it on a fixture
//! it then compares). Ignored files are not listed, as `--exclude-standard` reads the user's
//! ignore files.

use cairn_model::RepoPath;

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

const ARGUMENTS: [&str; 4] = ["ls-files", "--others", "--exclude-standard", "-z"];

/// Room for a path per untracked file of a very large working tree.
const CEILING: usize = 256 * 1024 * 1024;

/// Every untracked path in `repo`'s working tree, relative to its top.
pub(crate) fn untracked_paths(
    git: &GitBinary,
    repo: &Repository,
    cancel: &impl Cancel,
) -> Result<Vec<RepoPath>, Error> {
    let output = git
        .read_invocation()
        .in_repository(repo)
        .args(ARGUMENTS)
        .start()?
        .collect(cancel, CEILING, |_| {})?;
    Ok(output
        .stdout()
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| RepoPath::new(path.to_vec()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: `--directory` (an untracked directory listed as one path), or the display
    /// setting read (`status`, which honours `showUntrackedFiles=no`).
    #[test]
    fn the_read_lists_every_untracked_file_from_ls_files() {
        assert_eq!(
            ARGUMENTS,
            ["ls-files", "--others", "--exclude-standard", "-z"]
        );
    }
}
