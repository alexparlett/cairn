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
//! stderr alike — is read by the runner as whole lines, each scrubbed as it is split, and handed
//! to [`CommitWatch::output`] a read at a time as it arrives (R6.5, R4.10); a failure carries
//! the runner's tail of both, in the order the lines arrived, since git says "nothing to
//! commit" and "would make it empty" on stdout; the command log records the same.

use cairn_model::{AskpassToken, Confirmed, Consequence, ScrubbedLines};

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
    /// The lines git or a hook writes, stdout's and stderr's alike, as they arrive: each call
    /// the lines one read of one pipe completed, whole and scrubbed, never empty
    /// (staging-and-commit phase 05's QA item 3 — a caller that sends each call on sends one
    /// message a read, not a line; R4.10).
    pub output: &'a mut dyn FnMut(&ScrubbedLines),
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
        | Consequence::RemoveLock { .. }
        | Consequence::CheckoutDiscarding { .. } => {
            return Err(Error::CommitRefused {
                why: CommitRefusal::NotWhatWasConfirmed,
            });
        }
    };
    utf8_messages(repo)?;
    let now = amend_consequence(repo, &watch.cancel).map_err(|error| {
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
    // Both pipes' lines, as the runner splits and scrubs them, to the one output.
    invocation
        .lines(&CancelSignal::new(), |lines| (watch.output)(lines))
        .map(|_| ())
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

    /// C9 for commit and amend, against a `git` that records what it is given and runs the
    /// real one (phase 05's QA item 2): the argv each actually runs, the message on stdin byte
    /// for byte, a write's environment, and the message nowhere in the command log. Caught by:
    /// the message given with `-m` (which stores the same bytes under the default cleanup),
    /// `--literal-pathspecs`, `--no-verify` without the skip, or a `--cleanup` passed.
    #[test]
    fn commit_and_amend_run_git_commit_with_the_message_on_stdin_and_nowhere_else() {
        use super::super::recording_stub::RecordingStub;
        let stub = RecordingStub::new();
        stub.config("user.name", "Commit Ter");
        stub.config("user.email", "committer@example.com");
        let (git, repo) = (stub.git_binary(), stub.repository());
        let message = "a subject never on argv\n\nand a body\n";
        let cancel = CancelSignal::new();
        let (mut running, mut output) = (|_: CommitCancel| {}, |_: &ScrubbedLines| {});
        stub.forget();
        commit(
            &git,
            &repo,
            message,
            Hooks::Run,
            None,
            CommitWatch {
                cancel: &cancel,
                running: &mut running,
                output: &mut output,
            },
        )
        .unwrap();
        let consequence = amend_consequence(&repo, &cancel).unwrap();
        amend(
            &git,
            &repo,
            Confirmed::by_user(consequence),
            message,
            Hooks::Skip,
            None,
            CommitWatch {
                cancel: &cancel,
                running: &mut running,
                output: &mut output,
            },
        )
        .unwrap();
        let recorded = stub.recorded();
        let verbs: Vec<Vec<String>> = recorded
            .iter()
            .map(|record| record.arguments_after_location())
            .collect();
        assert_eq!(
            verbs,
            [
                vec!["commit", "-q", "-F", "-"],
                vec!["commit", "-q", "--amend", "--no-verify", "-F", "-"],
            ]
        );
        for record in &recorded {
            assert_eq!(record.stdin, message.as_bytes(), "{record:?}");
            record.assert_a_write();
        }
        let logged = repo.processes().log();
        assert!(logged.len() >= 2, "{logged:?}");
        for entry in &logged {
            assert!(
                !entry
                    .arguments
                    .iter()
                    .any(|argument| argument.contains("never on argv")),
                "the message reached the command log: {entry:?}"
            );
        }
    }

    /// Runs a commit of the fixture's staged change through `run`'s line stream, handing back the
    /// outcome and every line `output` was handed, in order. A hook written a moment ago can be
    /// refused with "Text file busy" while another test's fork still holds it open: retried then.
    fn commit_seen(
        stub: &super::super::recording_stub::RecordingStub,
        repo: &Repository,
    ) -> (Result<Performed, Error>, Vec<String>) {
        let git = stub.git_binary();
        for _ in 0..20 {
            let cancel = CancelSignal::new();
            let mut seen = Vec::new();
            let mut running = |_: CommitCancel| {};
            let mut output = |lines: &ScrubbedLines| seen.extend(lines.lines().map(str::to_owned));
            let outcome = commit(
                &git,
                repo,
                "subject",
                Hooks::Run,
                None,
                CommitWatch {
                    cancel: &cancel,
                    running: &mut running,
                    output: &mut output,
                },
            );
            match &outcome {
                Err(Error::GitFailed { stderr, .. }) if stderr.contains("Text file busy") => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                _ => return (outcome, seen),
            }
        }
        panic!("the hook stayed busy");
    }

    /// C31 for a commit (R4.10, R6.5): a hook's output reaches the commit's output and its
    /// failure as the runner's whole lines — a URL's userinfo written across two writes never
    /// handed on, kept or recorded; a multi-byte character whole; a `\r` redraw split as a line
    /// of its own — and the failure keeps what was handed on, in order. git runs a hook with its
    /// stdout sent to git's stderr, so every line here, the hook's "stdout" ones included,
    /// travels git's stderr; git's own stdout is the next test's, and `\r` on stdout is
    /// `pipes.rs`'s `lines_end_at_either_terminator_across_reads`. Caught by: a scrub after the
    /// cut, or a failure built from another text.
    #[test]
    fn a_hooks_output_reaches_the_commit_as_whole_scrubbed_lines() {
        use std::os::unix::fs::PermissionsExt as _;
        let stub = super::super::recording_stub::RecordingStub::new();
        stub.config("user.name", "Commit Ter");
        stub.config("user.email", "committer@example.com");
        stub.write(
            ".git/hooks/pre-commit",
            "#!/bin/sh\n\
             PATH=/usr/bin:/bin\n\
             printf 'denied by https://u:SEC'\n\
             sleep 0.1\n\
             printf 'RET@host/r\\n'\n\
             printf 'caf\\303\\251 closed\\n' >&2\n\
             printf 'redraw 1\\rredraw 2\\n'\n\
             exit 1\n",
        );
        let hook = stub.repository().git_dir().join("hooks/pre-commit");
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        let repo = stub.repository();
        let (outcome, seen) = commit_seen(&stub, &repo);
        for line in [
            "denied by https://host/r",
            "café closed",
            "redraw 1",
            "redraw 2",
        ] {
            assert!(
                seen.iter().any(|said| said == line),
                "{line:?} not handed on: {seen:?}"
            );
        }
        assert!(seen.iter().all(|line| !line.contains("SEC")), "{seen:?}");
        let Err(Error::GitFailed { stderr, .. }) = outcome else {
            panic!("the hook's failure was not the commit's: {outcome:?}");
        };
        assert_eq!(
            stderr.lines().collect::<Vec<_>>(),
            seen.iter().map(String::as_str).collect::<Vec<_>>(),
            "the failure keeps other than what was handed on"
        );
        let logged = format!("{:?}", repo.processes().log());
        assert!(
            !logged.contains("SEC") && logged.contains("café closed"),
            "{logged}"
        );
    }

    /// R6.5's other stream: git's own words on stdout — "no changes added to commit" — reach the
    /// commit's output and its failure as lines, as stderr's do. Caught by: stdout read as bytes
    /// and dropped, or kept apart from the failure.
    #[test]
    fn gits_stdout_reaches_the_commits_output_and_failure_as_lines() {
        let stub = super::super::recording_stub::RecordingStub::new();
        stub.config("user.name", "Commit Ter");
        stub.config("user.email", "committer@example.com");
        let repo = stub.repository();
        let (made, _) = commit_seen(&stub, &repo);
        assert!(made.is_ok(), "{made:?}");
        let (outcome, seen) = commit_seen(&stub, &repo);
        assert!(
            seen.iter()
                .any(|line| line.starts_with("no changes added to commit")),
            "{seen:?}"
        );
        let Err(Error::GitFailed { stderr, .. }) = outcome else {
            panic!("nothing staged, yet {outcome:?}");
        };
        assert!(stderr.contains("no changes added to commit"), "{stderr}");
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
}
