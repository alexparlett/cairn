//! `git stash show`: what a stash changed, as git lists it — the third porcelain exception a
//! read runs, accepted by the user on 2026-10-07 (refs-and-status R6.2, Q1).
//!
//! Why porcelain. A stash made with `--include-untracked` keeps its untracked files in a
//! third parent, apart from its tracked changes; with `stash.showIncludeUntracked` set (git
//! 2.32 and later), `git stash show` diffs the commit the stash was made on against the
//! tracked tree and those untracked files *together*, in one diff — so rename and copy
//! detection pairs across them: a tracked file deleted beside an untracked file of the same
//! content is `R100 a b`. No plumbing can diff one tree against the union of two without
//! writing a tree or an index, and two plumbing diffs merged print `D a` and `A b` where git
//! prints the rename. So the list is asked of `git stash show` itself, and git reads
//! `stash.showIncludeUntracked`, `diff.renames`, `diff.renameLimit` and the submodule
//! settings exactly as the user's own `git stash show` does — on every version, git 2.30 and
//! 2.31 ignoring the setting as they ignore it for the user.
//!
//! What it runs: `git stash show --raw -z --no-abbrev --no-color --no-ext-diff --no-textconv
//! --no-relative --end-of-options <stash commit>`, built here and nowhere else. The raw form
//! prints no patch, so no textconv or external diff could run even without the flags that
//! refuse them; `--no-color` and `--no-relative` set back the presentation settings
//! porcelain reads and plumbing does not (`color.ui`, `diff.relative`), and an explicit
//! output format makes git ignore `stash.showStat` and `stash.showPatch`. `git stash show`
//! takes no lock, reads no index (its untracked half is merged into an index held in
//! memory) and writes nothing: `a_stash_read_writes_nothing` holds the git directory
//! byte-identical after it. It needs a working tree, as every `git stash` does; a stash in
//! a bare repository is refused by git, and the answer is [`Error::GitFailed`]. Run as a
//! read, with `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1`: in a partial clone, the
//! blobs rename detection compares may be the promisor's alone, and git 2.44 and later then
//! fails rather than fetching them, as [`super::changes`] does. Git before 2.44 ignores
//! `GIT_NO_LAZY_FETCH`, so there this read may lazy-fetch them — a pack written and the
//! network reached — where the setting pairs an untracked file by an inexact rename
//! (reproduced on 2.32.7 in a `blob:none` clone; git 2.30 and 2.31 list no untracked file,
//! so have none to pair). The floor's residual (root `CLAUDE.md`, the environment
//! invariant).

use cairn_model::{ChangedFile, Oid};

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

use super::changes::RawRecords;

/// What the stash commit `stash` changed against the commit it was made on — with its
/// untracked files where the user's `stash.showIncludeUntracked` says so — as `git stash
/// show --raw` lists it, in git's order. `cancel` is polled while git runs: a superseded
/// query ends the process and answers [`Error::ChangesCancelled`].
pub(crate) fn stash_changes(
    git: &GitBinary,
    repo: &Repository,
    stash: &Oid,
    cancel: &impl Cancel,
) -> Result<Vec<ChangedFile>, Error> {
    if cancel.is_cancelled() {
        return Err(Error::ChangesCancelled { changed: 0 });
    }
    let arguments = arguments(stash);
    let mut records = RawRecords::default();
    let outcome = git
        .read_invocation()
        .in_repository(repo)
        .args(&arguments)
        .start()?
        .records(cancel, |record| records.push(record), |_| {});
    match outcome {
        Ok(_) => records.finish(&arguments.join(" ")),
        Err(Error::GitReadCancelled { .. }) => Err(Error::ChangesCancelled {
            changed: records.len(),
        }),
        Err(other) => Err(other),
    }
}

/// The read's arguments: the one porcelain `stash` verb a read may name, `show`, in its raw
/// form, and the stash commit after `--end-of-options`.
fn arguments(stash: &Oid) -> Vec<String> {
    let mut arguments: Vec<String> = [
        "stash",
        "show",
        "--raw",
        "-z",
        "--no-abbrev",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--no-relative",
        "--end-of-options",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    arguments.push(stash.to_string());
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancelSignal;
    use crate::process::stub_git::{StubGit, discover_retrying, printed_environment};

    const BLOB: &str = "78981922613b2afb6025042ff6bd878ac1994e85";

    /// The read as git receives it, through the runner: a stub `git` prints its argv and its
    /// environment beside it and answers one added file. The argv is `stash show` in raw
    /// form after where the repository is, and the environment a read's — optional locks
    /// off, no lazy fetch, no askpass token. Caught by: the read built as a write (an index
    /// refresh, a token), or anything but `stash show` run.
    #[test]
    fn the_stub_git_is_asked_stash_show_with_a_reads_environment() {
        let stub = StubGit::with_git_from(|directory| {
            let argv = directory.join("argv").display().to_string();
            let env = directory.join("env").display().to_string();
            format!(
                "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                 printf '%s\\n' \"$@\" > '{argv}'\n\
                 /usr/bin/env > '{env}'\n\
                 printf ':000000 100644 {zero} {BLOB} A\\0new.txt\\0'",
                zero = "0".repeat(40)
            )
        });
        let environment = stub.environment();
        let path = environment
            .get("PATH")
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| panic!("a PATH"));
        let git = discover_retrying(environment).unwrap_or_else(|e| panic!("{e}"));
        let repo =
            Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap_or_else(|e| panic!("{e}"));
        let stash = Oid::from_bytes(&[0xab; 20]).unwrap_or_else(|_| unreachable!());
        let files = stash_changes(&git, &repo, &stash, &CancelSignal::new())
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].new_path.to_string(), "new.txt");

        let argv = std::fs::read_to_string(stub.directory().join("argv"))
            .unwrap_or_else(|e| panic!("{e}"));
        let words: Vec<&str> = argv
            .lines()
            .filter(|word| !word.starts_with("--git-dir=") && !word.starts_with("--work-tree="))
            .collect();
        assert_eq!(words, arguments(&stash));
        let printed = printed_environment(
            &std::fs::read_to_string(stub.directory().join("env"))
                .unwrap_or_else(|e| panic!("{e}")),
        );
        let expected: std::collections::BTreeMap<String, String> = [
            ("GIT_ASKPASS", StubGit::HELPER),
            ("GIT_EDITOR", "false"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_SEQUENCE_EDITOR", "false"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("PATH", path.as_str()),
            ("SSH_ASKPASS", StubGit::HELPER),
            ("SSH_ASKPASS_REQUIRE", "force"),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
        assert_eq!(printed, expected);
    }

    /// The argv the read is built with, in full: `stash show` and nothing else of `stash`,
    /// the raw form, every presentation and program setting refused, and the stash after
    /// `--end-of-options`. Caught by: a patch format (which could run textconv), a missing
    /// refusal, or the id where an option could be read.
    #[test]
    fn the_read_is_stash_show_in_raw_form_and_nothing_else() {
        let stash = Oid::from_bytes(&[0xab; 20]).unwrap_or_else(|_| unreachable!());
        assert_eq!(
            arguments(&stash),
            [
                "stash",
                "show",
                "--raw",
                "-z",
                "--no-abbrev",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--no-relative",
                "--end-of-options",
                "abababababababababababababababababababab",
            ]
        );
    }
}
