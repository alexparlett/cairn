//! How every local write of `docs/prd/staging-and-commit.md` R3 runs: one write invocation,
//! git's global `--literal-pathspecs` ahead of the verb, its input on stdin, and the lock
//! files listed before it starts and after it is over (R3.8).
//!
//! `--literal-pathspecs` is git's global option, given on `argv` before the verb — never the
//! `GIT_LITERAL_PATHSPECS` variable, since an invocation's environment is built only in
//! `crate::process`, and the environment twin refuses one set anywhere else — so a path
//! named `*.rs` or `:(top)x` is that path and never a pattern (R3). git exports the mode to
//! the hooks it runs under these verbs (`GIT_LITERAL_PATHSPECS=1` in a `post-index-change` or
//! `post-checkout` hook's environment): the residual R3 states.
//!
//! A local write is not cancelled: only a commit or an amend is (R4.3), so the signal the
//! runner polls is one nobody holds. Its stdout is drained and dropped — `git clean`'s
//! "Removing" lines are prose — and its stderr travels on its failure.

use std::ffi::OsString;
use std::path::PathBuf;

use cairn_model::AskpassToken;

use super::stranded_locks::stranded_locks;
use super::{GitBinary, Locks, WriteAuthority};
use crate::{CancelSignal, Error, Repository};

/// git's global option, before the verb, on every local write.
pub(super) const LITERAL_PATHSPECS: &str = "--literal-pathspecs";

/// Runs `git --literal-pathspecs <arguments>` in `repo` as a write, `input` on its stdin
/// when there is one, with `token` as the operation's askpass authorisation. A failure is
/// [`Error::GitFailed`], carrying git's stderr and the lock files present.
pub(super) fn run(
    git: &GitBinary,
    repo: &Repository,
    token: Option<&AskpassToken>,
    arguments: &[OsString],
    input: Option<Vec<u8>>,
) -> Result<(), Error> {
    let mut command = git
        .write_invocation(WriteAuthority::new())
        .in_repository(repo)
        .arg(LITERAL_PATHSPECS)
        .args(arguments);
    if let Some(input) = input {
        command = command.input(input);
    }
    if let Some(token) = token {
        command = command.authorized_by(token);
    }
    command
        .start()?
        .finish(&CancelSignal::new(), |_| {}, |_| {})
        .map(|_| ())
}

/// Every `*.lock` under the repository's git directories now.
pub(super) fn locks_now(repo: &Repository) -> Vec<PathBuf> {
    stranded_locks(repo.git_dir(), repo.inner().common_dir())
}

/// The lock files around a write: those listed before it, and those now.
pub(super) fn locks_around(repo: &Repository, before: Vec<PathBuf>) -> Locks {
    Locks {
        before,
        after: locks_now(repo),
    }
}

/// Paths as a pathspec file git reads with `--pathspec-from-file=- --pathspec-file-nul`:
/// each path's bytes, NUL-terminated, so a path with a newline or a quote is that path.
pub(super) fn pathspec_file<'a>(
    paths: impl IntoIterator<Item = &'a cairn_model::RepoPath>,
) -> Vec<u8> {
    let mut file = Vec::new();
    for path in paths {
        file.extend_from_slice(path.as_bytes());
        file.push(0);
    }
    file
}

/// What every verb that takes many paths reads them with: the pathspec file on stdin,
/// NUL-separated (git 2.25, and 2.26 for `rm`; inside the 2.30 floor).
pub(super) const PATHSPEC_FILE: [&str; 2] = ["--pathspec-from-file=-", "--pathspec-file-nul"];

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::RepoPath;

    /// Caught by: a separator git reads as part of a path, or a path's own bytes changed.
    #[test]
    fn a_pathspec_file_is_each_paths_bytes_nul_terminated() {
        let paths = [
            RepoPath::new("a b"),
            RepoPath::new("new\nline"),
            RepoPath::new(b"\xff".to_vec()),
        ];
        assert_eq!(pathspec_file(&paths), b"a b\0new\nline\0\xff\0".to_vec());
        assert_eq!(pathspec_file(&[]), Vec::<u8>::new());
    }
}
