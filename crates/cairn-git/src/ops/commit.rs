//! Commit and amend (`docs/prd/staging-and-commit.md` R6.1, R6.2, R6.4, R6.5, R6.8, R6.9,
//! R6.11): `git commit -q -F -` and `git commit -q --amend -F -`, the message on stdin, byte
//! for byte.
//!
//! **The message is git's to clean.** No `--cleanup` is passed, so `commit.cleanup` and git's
//! default for a message given with `-F` (`whitespace`: trailing spaces and blank lines go,
//! `#` lines stay) decide what is stored, exactly as the user's `git commit -F <file>` would
//! store it — compared byte for byte under every value on git 2.30.9, 2.32.7 and the host's
//! (`crates/cairn-git/tests/diff/commit.rs`). The message never reaches `argv`, so neither
//! the process table nor the command log holds it. A non-UTF-8 `i18n.commitEncoding` — as
//! `git config` answers it (`crate::reads::commit_encoding`, R6.11), a linked worktree's
//! `includeIf` included — is refused before git runs: git stores the bytes it is given under
//! that encoding's name, and a draft is UTF-8 (git-write-verbs.md §5).
//!
//! **Hooks run as git runs them.** `--no-verify` — which skips `pre-commit` and `commit-msg`
//! and nothing else — is passed only for [`Hooks::Skip`], the skip a failed commit or amend
//! offers (R10.5). No `--literal-pathspecs` either: a commit takes no pathspec, and git would
//! export the mode to every hook it runs (`GIT_LITERAL_PATHSPECS=1`), where a `pre-commit`
//! hook's own `git diff -- '*.rs'` would then match nothing. `-q` leaves out the summary git
//! prints once the commit is made — nothing Cairn reads, and a line an orphaned commit could
//! die writing.
//!
//! **The identity is git's** (R6.8, L26): no author or committer is passed and none is read,
//! so git uses the configuration and the inherited identity variables a terminal's commit
//! would; with none, git's own error is the failure.
//!
//! **An operation in progress** (R6.9, L25): during a merge the commit is the merge commit, as
//! git makes it from `MERGE_HEAD`; during a single cherry-pick or revert it concludes it, as
//! `git commit` does — the picked commit's author kept, `CHERRY_PICK_HEAD` or `REVERT_HEAD`
//! removed by git; during a rebase, `git am` or a sequence of picks or reverts — and, for an
//! amend, during any of them — it is refused before git runs, naming git's command to continue
//! or abort it. A detached `HEAD` refuses nothing: the commit is made on no branch.
//!
//! **An amend is read at the press** (R6.4, rules 2 and 3 of the redesign): what it costs
//! ([`super::amend_consequence`]) is read in the job that runs it. One git logs and no remote
//! has is recoverable from Show Lost Commits, so [`amend_unconfirmed`] runs it at once, taking
//! no token — it is not destructive (R1.5) and not on the destructive-operation roster; any
//! other ends without running git, answering its `Consequence`
//! ([`AmendAnswer::NeedsConfirming`]) for the confirmation dialog, and runs through [`amend`]
//! with the token the dialog builds. [`amend`] re-computes that `Consequence` before git runs
//! and refuses, writing nothing, when anything in it moved — `HEAD`, whether a remote has it,
//! the reflog setting — ([`Error::AmendChangedSinceConfirmed`], R1.4), and records the prompt
//! the user accepted. Between the re-check and git's run is a window no check closes, as for a
//! discard.
//!
//! **Made is read from `HEAD`** (R4.7, and phase 12's QA item #10). After git is reaped `HEAD`
//! is read again: a commit is made when `HEAD` is a commit whose first parent is the `HEAD` it
//! ran on (none, on an unborn branch), an amend when `HEAD`'s parents are the replaced commit's.
//! git's exit 0 with `HEAD` anything else is [`Error::CommitUnconfirmed`] — something else moved
//! `HEAD` while it ran, and for an amend, git may have amended whatever `HEAD` had become; a
//! commit or amend the user cancelled that git had already made is reported made, one it had not
//! is [`Error::GitCancelled`]; and one git made and then failed after — a `die` past the ref
//! update — is [`Error::MadeButGitFailed`], made, with git's words.
//!
//! **The token-free amend's window.** [`amend_unconfirmed`] decides the amend is recoverable from
//! what it reads at the press, and git then runs the hooks before it updates the ref: in that
//! window a push of `HEAD`, or a checkout, from a terminal changes what git amends or whether a
//! remote has it, and the amend is reported made all the same. The replaced commit stays in the
//! reflog git writes, so Show Lost Commits still finds it; no check after the run says the
//! commit was published meanwhile (stated in `docs/systems/staging.md`, "Residuals").
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

use cairn_model::{AskpassToken, Confirmed, Consequence, Oid, ScrubbedLines};

use super::amend::amend_consequence;
use super::local_write::{locks_around, locks_now};
use super::{GitBinary, Invalidated, Performed, WriteAuthority};
use crate::object_id::{model_id, object_id};
use crate::process::KillHandle;
use crate::{Cancel, CancelSignal, CommitRefusal, Error, Repository};

/// Whether the commit runs the `pre-commit` and `commit-msg` hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hooks {
    /// As git runs them: the default.
    Run,
    /// `--no-verify`: only from the skip a failed commit or amend offers (R10.5).
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

/// What an amend asked at the press came to (R6.4): run, being recoverable, or not run, its
/// `Consequence` answered for the confirmation dialog.
#[derive(Debug)]
pub enum AmendAnswer {
    /// git logs it and no remote has `HEAD`: amended at once, asking nothing.
    Amended(Performed),
    /// A remote has `HEAD`, or git will keep no reflog: no git ran. Confirm this and run
    /// [`amend`] with the token.
    NeedsConfirming(Consequence),
}

/// Commits what is staged with `message` (module docs). Refused before git runs
/// ([`Error::CommitRefused`]) for a non-UTF-8 `i18n.commitEncoding` and during a rebase,
/// `git am` or a sequence of picks or reverts; git's own failure — a hook's, an empty message,
/// nothing staged, no identity — is [`Error::GitFailed`] with its output.
pub fn commit(
    git: &GitBinary,
    repo: &Repository,
    message: &str,
    hooks: Hooks,
    token: Option<&AskpassToken>,
    watch: CommitWatch<'_>,
) -> Result<Performed, Error> {
    utf8_messages(git, repo, watch.cancel)?;
    if let Some(operation) = repo.operation_in_progress()
        && operation.refuses_commit()
    {
        return Err(Error::CommitRefused {
            why: CommitRefusal::InProgress(operation),
        });
    }
    let before = locks_now(repo);
    let made = Made::Commit {
        on: head_commit(repo)?,
    };
    run(
        git,
        repo,
        &arguments(false, hooks),
        message,
        token,
        watch,
        &made,
    )?;
    Ok(Performed::new("committed", everything_a_commit_moves())
        .with_locks(locks_around(repo, before)))
}

/// Amends `HEAD` with `message` and what is staged, at the press (module docs): what it costs is
/// read first, and it runs only where it is recoverable — git logs it and no remote has `HEAD` —
/// taking no token, since nothing it replaces is lost (R1.5). Otherwise nothing runs and the
/// `Consequence` is answered, for the dialog. Refused as [`amend`] is.
pub fn amend_unconfirmed(
    git: &GitBinary,
    repo: &Repository,
    message: &str,
    hooks: Hooks,
    token: Option<&AskpassToken>,
    watch: CommitWatch<'_>,
) -> Result<AmendAnswer, Error> {
    utf8_messages(git, repo, watch.cancel)?;
    let consequence = cost_now(git, repo, watch.cancel)?;
    let replaced = match &consequence {
        Consequence::Amend { commit, .. } if !consequence.needs_confirming() => *commit,
        Consequence::Amend { .. }
        | Consequence::DiscardLines { .. }
        | Consequence::DiscardFiles { .. }
        | Consequence::RemoveLock { .. }
        | Consequence::CheckoutDiscarding { .. } => {
            return Ok(AmendAnswer::NeedsConfirming(consequence));
        }
    };
    let before = locks_now(repo);
    let made = Made::amend_of(repo, replaced)?;
    run(
        git,
        repo,
        &arguments(true, hooks),
        message,
        token,
        watch,
        &made,
    )?;
    Ok(AmendAnswer::Amended(
        Performed::new(
            format!("amended {}", replaced.short().as_str()),
            everything_a_commit_moves(),
        )
        .with_locks(locks_around(repo, before)),
    ))
}

/// Amends `HEAD` with `message` and what is staged (module docs), once the `Consequence`
/// `confirmed` carries is still what the repository says. Refused as [`commit`] is, and for
/// any operation in progress or an unborn branch; [`Error::AmendChangedSinceConfirmed`] when
/// what was confirmed moved. git's failure with `HEAD` unmoved — a hook refused it — is
/// [`Error::AmendNotMade`], the token handed back unspent for the skip.
pub fn amend(
    git: &GitBinary,
    repo: &Repository,
    confirmed: Confirmed,
    message: &str,
    hooks: Hooks,
    token: Option<&AskpassToken>,
    watch: CommitWatch<'_>,
) -> Result<Performed, Error> {
    let replaced = match confirmed.consequence() {
        Consequence::Amend { commit, .. } => *commit,
        Consequence::DiscardLines { .. }
        | Consequence::DiscardFiles { .. }
        | Consequence::RemoveLock { .. }
        | Consequence::CheckoutDiscarding { .. } => {
            return Err(Error::CommitRefused {
                why: CommitRefusal::NotWhatWasConfirmed,
            });
        }
    };
    utf8_messages(git, repo, watch.cancel)?;
    if &cost_now(git, repo, watch.cancel)? != confirmed.consequence() {
        return Err(Error::AmendChangedSinceConfirmed);
    }
    let before = locks_now(repo);
    let made = Made::amend_of(repo, replaced)?;
    match run(
        git,
        repo,
        &arguments(true, hooks),
        message,
        token,
        watch,
        &made,
    ) {
        Ok(()) => {}
        // git failed, and `HEAD` is still the commit confirmed: nothing the confirmation names
        // moved, so it is handed back unspent, for the skip (R1.1's option (a)).
        Err(failure @ Error::GitFailed { .. })
            if head_commit(repo).ok().flatten() == Some(replaced) =>
        {
            return Err(Error::AmendNotMade {
                failure: Box::new(failure),
                unspent: Box::new(confirmed),
            });
        }
        Err(other) => return Err(other),
    }
    Ok(Performed::destructive(
        format!("amended {}", replaced.short().as_str()),
        confirmed,
        everything_a_commit_moves(),
    )
    .with_locks(locks_around(repo, before)))
}

/// What amending costs now, read through `cancel` — a cancel while it reads is the commit
/// cancelled before git ran.
fn cost_now(git: &GitBinary, repo: &Repository, cancel: &dyn Cancel) -> Result<Consequence, Error> {
    amend_consequence(git, repo, &cancel).map_err(|error| {
        if cancel.is_cancelled() {
            Error::CommitCancelledBeforeRunning
        } else {
            error
        }
    })
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

/// Refuses a non-UTF-8 `i18n.commitEncoding`, as `git config` answers it, as git reads the
/// name (`is_encoding_utf8` in git's `utf8.c`: `UTF-8` or `UTF8` in any case, an optional `-`
/// after `utf`). A cancel while it reads is the commit cancelled before git ran.
fn utf8_messages(git: &GitBinary, repo: &Repository, cancel: &dyn Cancel) -> Result<(), Error> {
    let encoding = crate::reads::commit_encoding(git, repo, &cancel).map_err(|error| {
        if cancel.is_cancelled() {
            Error::CommitCancelledBeforeRunning
        } else {
            error
        }
    })?;
    match encoding {
        None => Ok(()),
        Some(name) if names_utf8(&name) => Ok(()),
        Some(name) => Err(Error::CommitRefused {
            why: CommitRefusal::CommitEncoding {
                encoding: String::from_utf8_lossy(&name).into_owned(),
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

/// `HEAD`'s commit; `None` on an unborn branch.
fn head_commit(repo: &Repository) -> Result<Option<Oid>, Error> {
    match repo.inner().head_id() {
        Ok(id) => Ok(Some(model_id(&id)?)),
        Err(_) => Ok(None),
    }
}

/// `commit`'s parents as the object names them — not as a shallow clone's boundary hides them,
/// since an amend of the boundary commit writes the parents the object holds.
fn parents_of(repo: &Repository, commit: &Oid) -> Result<Vec<Oid>, Error> {
    let found = repo
        .inner()
        .find_commit(object_id(commit)?)
        .map_err(|source| Error::ReadCommit {
            id: commit.to_string(),
            source: Box::new(source),
        })?;
    found
        .parent_ids()
        .map(|parent| model_id(&parent.detach()))
        .collect()
}

/// What `HEAD` is once the commit or amend was made (module docs).
#[derive(Debug)]
enum Made {
    /// A commit whose first parent is `on` — none on an unborn branch.
    Commit { on: Option<Oid> },
    /// An amend of `replaced`: a commit with `replaced`'s parents.
    Amend { replaced: Oid, parents: Vec<Oid> },
}

impl Made {
    fn amend_of(repo: &Repository, replaced: Oid) -> Result<Self, Error> {
        Ok(Self::Amend {
            replaced,
            parents: parents_of(repo, &replaced)?,
        })
    }

    /// Whether `HEAD` now is what this made; `None` where `HEAD` or its commit cannot be read.
    /// `exited` is git's word that it finished: an amend may then leave `HEAD` the very commit
    /// it replaced — the same tree, message, parents and author within one second make the
    /// same object — where after a cancel `HEAD` unmoved is an amend not made.
    fn holds(&self, repo: &Repository, exited: bool) -> Option<bool> {
        let Some(now) = head_commit(repo).ok()? else {
            return Some(false);
        };
        let parents = parents_of(repo, &now).ok()?;
        Some(match self {
            Self::Commit { on } => Some(now) != *on && parents.first() == on.as_ref(),
            Self::Amend {
                replaced,
                parents: was,
            } => parents == *was && (exited || now != *replaced),
        })
    }
}

/// Runs the commit, its message on stdin, watched as [`CommitWatch`] says, and reads `HEAD`
/// after the reap to say whether `made` holds (module docs).
fn run(
    git: &GitBinary,
    repo: &Repository,
    arguments: &[&str],
    message: &str,
    token: Option<&AskpassToken>,
    watch: CommitWatch<'_>,
    made: &Made,
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
    let outcome = invocation.lines(&CancelSignal::new(), |lines| (watch.output)(lines));
    let verb = match made {
        Made::Commit { .. } => "committed",
        Made::Amend { .. } => "amended",
    };
    match outcome {
        // git's word, unless `HEAD` says otherwise; one that cannot be read is git's word.
        Ok(_) => match made.holds(repo, true) {
            Some(false) => Err(Error::CommitUnconfirmed { verb }),
            Some(true) | None => Ok(()),
        },
        // Cancelled after git had made it: made. One that cannot be read is cancelled.
        Err(Error::GitCancelled { .. }) if made.holds(repo, false) == Some(true) => Ok(()),
        // Made, and then git failed (a `die` after the ref update): made, git's words kept. Read
        // as after a cancel — `HEAD` unmoved is not made — so a failed amend whose `HEAD` is
        // still the replaced commit stays git's failure, and hands its token back.
        Err(failure @ Error::GitFailed { .. }) if made.holds(repo, false) == Some(true) => {
            Err(Error::MadeButGitFailed {
                verb,
                failure: Box::new(failure),
            })
        }
        Err(other) => Err(other),
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
        let consequence = amend_consequence(&git, &repo, &cancel).unwrap();
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
        let (commits, reads): (Vec<_>, Vec<_>) = recorded.iter().partition(|record| {
            record
                .arguments_after_location()
                .first()
                .map(String::as_str)
                == Some("commit")
        });
        let verbs: Vec<Vec<String>> = commits
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
        for record in &commits {
            assert_eq!(record.stdin, message.as_bytes(), "{record:?}");
            record.assert_a_write();
        }
        // What each asks first is git's configuration (R6.11), read as reads, the message on no
        // stdin of theirs.
        assert!(!reads.is_empty());
        for record in &reads {
            assert_eq!(
                record
                    .arguments_after_location()
                    .first()
                    .map(String::as_str),
                Some("config"),
                "{record:?}"
            );
            assert!(record.stdin.is_empty(), "{record:?}");
            record.assert_a_read();
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
