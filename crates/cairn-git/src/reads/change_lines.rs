//! How many lines each tracked path's staged and unstaged changes hold together: `git
//! diff-index --cached --numstat -z <HEAD>` and `git diff-files --numstat -z`, for Create
//! Branch's discard, whose prompt counts what it loses as the discard prompts count
//! (`docs/prd/staging-and-commit.md` R11.3, R1.2).
//!
//! Query plumbing over the index and the working tree, as `working_tree_patch` reads them: it
//! writes nothing (a read runs with `GIT_OPTIONAL_LOCKS=0`, and the diff plumbing never writes
//! the index), takes no rename detection, and passes neither `--textconv` nor `--ext-diff`.
//! `diff-files` reads the working tree through the paths' clean filters, as the user's own
//! `git diff` does. An unborn branch's staged changes are counted against the empty tree.

use std::collections::HashMap;

use cairn_model::{Oid, RepoPath};

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// The most a numstat may print: a record per changed path, each a path and two counts.
const CEILING: usize = 64 * 1024 * 1024;

/// Each changed path's lines, staged and unstaged together; `None` for a path either side of
/// which is not text.
pub(crate) fn change_lines(
    git: &GitBinary,
    repo: &Repository,
    head: Option<Oid>,
    cancel: &impl Cancel,
) -> Result<HashMap<RepoPath, Option<usize>>, Error> {
    let base = match head {
        Some(head) => head.to_string(),
        None => repo.inner().object_hash().empty_tree().to_string(),
    };
    let staged = numstat(
        git,
        repo,
        &["diff-index", "--cached", "--numstat", "-z", &base],
        cancel,
    )?;
    let unstaged = numstat(git, repo, &["diff-files", "--numstat", "-z"], cancel)?;
    let mut lines: HashMap<RepoPath, Option<usize>> = HashMap::new();
    for (path, counted) in staged.into_iter().chain(unstaged) {
        let total = lines.entry(path).or_insert(Some(0));
        *total = match (*total, counted) {
            (Some(so_far), Some(more)) => Some(so_far + more),
            _ => None,
        };
    }
    Ok(lines)
}

fn numstat(
    git: &GitBinary,
    repo: &Repository,
    arguments: &[&str],
    cancel: &impl Cancel,
) -> Result<Vec<(RepoPath, Option<usize>)>, Error> {
    let output = git
        .read_invocation()
        .in_repository(repo)
        .args(arguments)
        .start()?
        .collect(cancel, CEILING, |_| {})?;
    parse(output.stdout()).ok_or_else(|| Error::UnexpectedGitOutput {
        arguments: arguments.join(" "),
        record: String::from_utf8_lossy(output.stdout())
            .chars()
            .take(200)
            .collect(),
    })
}

/// `--numstat -z` without rename detection: `<added>\t<removed>\t<path>\0` per path, `-` for
/// each count of a path that is not text.
fn parse(printed: &[u8]) -> Option<Vec<(RepoPath, Option<usize>)>> {
    let mut records = Vec::new();
    for record in printed
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let (added, removed, path) = (fields.next()?, fields.next()?, fields.next()?);
        let count = |field: &[u8]| -> Option<Option<usize>> {
            if field == b"-" {
                return Some(None);
            }
            std::str::from_utf8(field).ok()?.parse().ok().map(Some)
        };
        let lines = match (count(added)?, count(removed)?) {
            (Some(added), Some(removed)) => Some(added + removed),
            _ => None,
        };
        records.push((RepoPath::new(path.to_vec()), lines));
    }
    Some(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a binary path counted as zero lines, a path with a tab cut at it, or a
    /// record that is not numstat's taken for one.
    #[test]
    fn a_numstat_is_each_paths_lines_and_binary_is_none() {
        let printed = b"3\t1\ta.rs\x00-\t-\timage.png\x000\t0\tname\twith tab\x00";
        assert_eq!(
            parse(printed),
            Some(vec![
                (RepoPath::new("a.rs"), Some(4)),
                (RepoPath::new("image.png"), None),
                (RepoPath::new("name\twith tab"), Some(0)),
            ])
        );
        assert_eq!(parse(b"x\t1\ta\x00"), None);
        assert_eq!(parse(b""), Some(Vec::new()));
    }
}
