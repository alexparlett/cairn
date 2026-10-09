//! Commit and amend (`docs/prd/staging-and-commit.md` R6.1, R6.2, R6.5, R6.8, R6.9): `git
//! commit -q -F -` and `git commit -q --amend -F -`, the message on stdin, byte for byte.
//!
//! **The message is git's to clean.** No `--cleanup` is passed, so `commit.cleanup` and git's
//! default for a message given with `-F` (`whitespace`: trailing spaces and blank lines go,
//! `#` lines stay) decide what is stored, exactly as the user's `git commit -F <file>` would
//! store it — compared byte for byte under every value on git 2.30.9, 2.32.7 and the host's
//! (`crates/cairn-git/tests/diff/commit.rs`). The message never reaches `argv`, so neither
//! the process table nor the command log holds it. A non-UTF-8 `i18n.commitEncoding` is
//! refused before git runs: git stores the bytes it is given under that encoding's name, and
//! a draft is UTF-8 (git-write-verbs.md §5).
//!
//! **Hooks run as git runs them.** `--no-verify` — which skips `pre-commit` and `commit-msg`
//! and nothing else — is passed only for [`Hooks::Skip`], the hook failure's skip (R10.5). No
//! `--literal-pathspecs` either: a commit takes no pathspec, and git would export the mode to
//! every hook it runs (`GIT_LITERAL_PATHSPECS=1`), where a `pre-commit` hook's own `git diff
//! -- '*.rs'` would then match nothing. `-q` leaves out the summary git prints once the
//! commit is made — nothing Cairn reads, and a line an orphaned commit could die writing.
//!
//! **The identity is git's** (R6.8, L26): no author or committer is passed and none is read,
//! so git uses the configuration and the inherited identity variables a terminal's commit
//! would; with none, git's own error is the failure.
//!
//! **An operation in progress** (R6.9, L25): during a merge the commit is the merge commit, as
//! git makes it from `MERGE_HEAD`; during a rebase, `git am`, a cherry-pick or a revert — and,
//! for an amend, a merge too — it is refused before git runs, naming the operation.
//!
//! **Amend is destructive** (R1.5): it takes the [`Confirmed`] built from
//! [`super::amend_consequence`] by value, re-computes that `Consequence` before git runs and
//! refuses, writing nothing, when anything in it moved — `HEAD`, whether a remote has it, the
//! reflog setting — ([`Error::AmendChangedSinceConfirmed`], R1.4), and records the prompt the
//! user accepted. Between the re-check and git's run is a window no check closes, as for a
//! discard.
//!
//! **While it runs** a commit is the one write that can be cancelled (R4.3): once git is
//! running, [`CommitWatch::running`] is handed a [`CommitCancel`], which ends git's process
//! group as a fetch's cancel does; before then, [`CommitWatch::cancel`] is polled through the
//! checks, and a commit cancelled there writes nothing
//! ([`Error::CommitCancelledBeforeRunning`]). git's output — a hook's lines, on stdout and
//! stderr alike — is handed to [`CommitWatch::output`] line by line as it arrives (R6.5); a
//! failure carries it too, stdout's tail ahead of stderr's, since git says "nothing to commit"
//! and "would make it empty" on stdout.

use cairn_model::{AskpassToken, Confirmed, Consequence};

use super::amend::amend_consequence;
use super::local_write::{locks_around, locks_now};
use super::{GitBinary, Invalidated, Performed, WriteAuthority};
use crate::diff::git_config::last_value;
use crate::process::KillHandle;
use crate::{Cancel, CancelSignal, CommitRefusal, Error, Repository};

/// Whether the commit runs the `pre-commit` and `commit-msg` hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hooks {
    /// As git runs them: the default.
    Run,
    /// `--no-verify`: only from the hook failure's skip (R10.5).
    Skip,
}

/// What a running commit tells its caller, and how it is cancelled.
pub struct CommitWatch<'a> {
    /// Polled through the checks before git runs; a cancel there writes nothing.
    pub cancel: &'a dyn Cancel,
    /// Called once, as git starts, with what ends it from another thread.
    pub running: &'a mut dyn FnMut(CommitCancel),
    /// Each line git or a hook writes, stdout's and stderr's alike, as it arrives.
    pub output: &'a mut dyn FnMut(&str),
}

/// Ends a running commit and everything it started — its hooks, a signing program — with
/// `SIGTERM` to the process group, then `SIGKILL` after the grace period. Never blocks.
#[derive(Debug, Clone)]
pub struct CommitCancel(KillHandle);

impl CommitCancel {
    pub fn cancel(&self) {
        self.0.kill();
    }
}

impl std::fmt::Debug for CommitWatch<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommitWatch")
            .field("cancelled", &self.cancel.is_cancelled())
            .finish_non_exhaustive()
    }
}

/// How much of git's stdout a failure keeps: the end of it, which is where git says why.
const STDOUT_TAIL: usize = 64 * 1024;

/// Commits what is staged with `message` (module docs). Refused before git runs
/// ([`Error::CommitRefused`]) for a non-UTF-8 `i18n.commitEncoding` and during a rebase,
/// `git am`, a cherry-pick or a revert; git's own failure — a hook's, an empty message,
/// nothing staged, no identity — is [`Error::GitFailed`] with its output.
pub fn commit(
    git: &GitBinary,
    repo: &Repository,
    message: &str,
    hooks: Hooks,
    token: Option<&AskpassToken>,
    watch: CommitWatch<'_>,
) -> Result<Performed, Error> {
    utf8_messages(repo)?;
    if let Some(operation) = repo.operation_in_progress()
        && operation.refuses_commit()
    {
        return Err(Error::CommitRefused {
            why: CommitRefusal::InProgress(operation),
        });
    }
    let before = locks_now(repo);
    run(git, repo, &arguments(false, hooks), message, token, watch)?;
    Ok(Performed::new("committed", everything_a_commit_moves())
        .with_locks(locks_around(repo, before)))
}

/// Amends `HEAD` with `message` and what is staged (module docs), once the `Consequence`
/// `confirmed` carries is still what the repository says. Refused as [`commit`] is, and for
/// a merge in progress or an unborn branch; [`Error::AmendChangedSinceConfirmed`] when what
/// was confirmed moved.
pub fn amend(
    git: &GitBinary,
    repo: &Repository,
    confirmed: Confirmed,
    message: &str,
    hooks: Hooks,
    token: Option<&AskpassToken>,
    watch: CommitWatch<'_>,
) -> Result<Performed, Error> {
    let short = match confirmed.consequence() {
        Consequence::Amend { commit, .. } => commit.short().as_str().to_owned(),
        Consequence::DiscardLines { .. }
        | Consequence::DiscardFiles { .. }
        | Consequence::RemoveLock { .. } => {
            return Err(Error::CommitRefused {
                why: CommitRefusal::NotWhatWasConfirmed,
            });
        }
    };
    utf8_messages(repo)?;
    let now = amend_consequence(repo, &Polled(watch.cancel)).map_err(|error| {
        if watch.cancel.is_cancelled() {
            Error::CommitCancelledBeforeRunning
        } else {
            error
        }
    })?;
    if &now != confirmed.consequence() {
        return Err(Error::AmendChangedSinceConfirmed);
    }
    let before = locks_now(repo);
    run(git, repo, &arguments(true, hooks), message, token, watch)?;
    Ok(Performed::destructive(
        format!("amended {short}"),
        confirmed,
        everything_a_commit_moves(),
    )
    .with_locks(locks_around(repo, before)))
}

/// A ref moved, the index was written (a hook may stage), and objects were added.
fn everything_a_commit_moves() -> Invalidated {
    Invalidated::refs()
        .and(Invalidated::index())
        .and(Invalidated::objects())
}

/// `git commit -q [--amend] [--no-verify] -F -`.
fn arguments(amend: bool, hooks: Hooks) -> Vec<&'static str> {
    let mut arguments = vec!["commit", "-q"];
    if amend {
        arguments.push("--amend");
    }
    match hooks {
        Hooks::Run => {}
        Hooks::Skip => arguments.push("--no-verify"),
    }
    arguments.extend(["-F", "-"]);
    arguments
}

/// Refuses a non-UTF-8 `i18n.commitEncoding` as git reads the name (`is_encoding_utf8` in
/// git's `utf8.c`: `UTF-8` or `UTF8` in any case, an optional `-` after `utf`).
fn utf8_messages(repo: &Repository) -> Result<(), Error> {
    let config = repo.inner().config_snapshot();
    match last_value(config.plumbing(), "i18n", None, "commitEncoding") {
        None => Ok(()),
        Some(Some(name)) if names_utf8(&name) => Ok(()),
        Some(value) => Err(Error::CommitRefused {
            why: CommitRefusal::CommitEncoding {
                encoding: value.map_or_else(String::new, |name| name.to_string()),
            },
        }),
    }
}

fn names_utf8(name: &[u8]) -> bool {
    let Some(rest) = name
        .get(..3)
        .filter(|prefix| prefix.eq_ignore_ascii_case(b"utf"))
        .map(|_| &name[3..])
    else {
        return false;
    };
    let rest = rest.strip_prefix(b"-").unwrap_or(rest);
    rest == b"8"
}

/// Runs the commit, its message on stdin, watched as [`CommitWatch`] says.
fn run(
    git: &GitBinary,
    repo: &Repository,
    arguments: &[&str],
    message: &str,
    token: Option<&AskpassToken>,
    watch: CommitWatch<'_>,
) -> Result<(), Error> {
    if watch.cancel.is_cancelled() {
        return Err(Error::CommitCancelledBeforeRunning);
    }
    let mut command = git
        .write_invocation(WriteAuthority::new())
        .in_repository(repo)
        .args(arguments)
        .input(message.as_bytes().to_vec());
    if let Some(token) = token {
        command = command.authorized_by(token);
    }
    let invocation = command.start()?;
    (watch.running)(CommitCancel(invocation.kill_handle()));
    // Both pipes' lines go to the one output, each on the thread that drives the runner.
    let output = std::cell::RefCell::new(watch.output);
    let mut stdout = Lines::default();
    let outcome = invocation.finish(
        &CancelSignal::new(),
        |chunk| stdout.push(chunk, &mut |line| (output.borrow_mut())(line)),
        |line| (output.borrow_mut())(line),
    );
    stdout.finish(&mut |line| (output.borrow_mut())(line));
    match outcome {
        Ok(_) => Ok(()),
        Err(Error::GitFailed {
            arguments,
            status,
            stderr,
            present_locks,
        }) => Err(Error::GitFailed {
            arguments,
            status,
            stderr: [stdout.tail(), stderr]
                .into_iter()
                .filter(|part| !part.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            present_locks,
        }),
        Err(other) => Err(other),
    }
}

/// A `&dyn Cancel` where a walk wants a sized one.
struct Polled<'a>(&'a dyn Cancel);

impl Cancel for Polled<'_> {
    fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

/// stdout split into lines as it arrives, and the tail of it kept for a failure.
#[derive(Default)]
struct Lines {
    partial: Vec<u8>,
    tail: Vec<u8>,
}

impl Lines {
    fn push(&mut self, chunk: &[u8], output: &mut dyn FnMut(&str)) {
        self.tail.extend_from_slice(chunk);
        if self.tail.len() > STDOUT_TAIL {
            let excess = self.tail.len() - STDOUT_TAIL;
            self.tail.drain(..excess);
        }
        self.partial.extend_from_slice(chunk);
        while let Some(end) = self.partial.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.partial.drain(..=end).collect();
            let text = String::from_utf8_lossy(&line[..end]);
            if !text.trim().is_empty() {
                output(&text);
            }
        }
    }

    fn finish(&mut self, output: &mut dyn FnMut(&str)) {
        let text = String::from_utf8_lossy(&self.partial).into_owned();
        self.partial.clear();
        if !text.trim().is_empty() {
            output(&text);
        }
    }

    fn tail(&self) -> String {
        String::from_utf8_lossy(&self.tail).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a cleanup chosen for git, `--no-verify` without the skip, the message on
    /// argv, or `--literal-pathspecs`, which the hooks would inherit.
    #[test]
    fn the_arguments_are_git_commit_q_and_the_message_on_stdin() {
        assert_eq!(arguments(false, Hooks::Run), ["commit", "-q", "-F", "-"]);
        assert_eq!(
            arguments(false, Hooks::Skip),
            ["commit", "-q", "--no-verify", "-F", "-"]
        );
        assert_eq!(
            arguments(true, Hooks::Run),
            ["commit", "-q", "--amend", "-F", "-"]
        );
        assert_eq!(
            arguments(true, Hooks::Skip),
            ["commit", "-q", "--amend", "--no-verify", "-F", "-"]
        );
    }

    /// git's `is_encoding_utf8`. Caught by: a case-sensitive match, `utf-8` refused, or
    /// `UTF-16` taken for UTF-8.
    #[test]
    fn utf8_is_named_as_git_names_it() {
        for name in ["UTF-8", "utf-8", "utf8", "Utf8", "uTf-8"] {
            assert!(names_utf8(name.as_bytes()), "{name}");
        }
        for name in [
            "",
            "utf",
            "UTF-16",
            "latin1",
            "ISO-8859-1",
            "utf--8",
            "utf-8 ",
        ] {
            assert!(!names_utf8(name.as_bytes()), "{name}");
        }
    }

    /// stdout's lines reach the output as they complete, a last line without its newline
    /// on finish, blank ones dropped; the tail keeps the end of a long answer. Caught by: a
    /// line split at a chunk's edge, or the tail growing without bound.
    #[test]
    fn stdout_is_handed_on_by_line_and_its_tail_is_bounded() {
        let mut seen = Vec::new();
        let mut lines = Lines::default();
        let mut output = |line: &str| seen.push(line.to_owned());
        lines.push(b"On branch ma", &mut output);
        lines.push(b"in\n\nnothing to com", &mut output);
        lines.push(b"mit\nlast", &mut output);
        lines.finish(&mut output);
        assert_eq!(seen, ["On branch main", "nothing to commit", "last"]);
        let mut lines = Lines::default();
        lines.push(&vec![b'x'; STDOUT_TAIL * 2], &mut |_| {});
        lines.push(b"\nend\n", &mut |_| {});
        assert_eq!(lines.tail().len(), STDOUT_TAIL);
        assert!(lines.tail().ends_with("\nend\n"));
    }
}
