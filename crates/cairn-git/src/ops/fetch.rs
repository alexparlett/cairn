//! Fetching from a remote: the first operation to run a `git` verb, and the
//! first that can ask the user for a credential.
//!
//! **Not destructive, and deliberately takes no [`cairn_model::Confirmed`].**
//! With the refspecs git gives a clone (`+refs/heads/*:refs/remotes/<remote>/*`)
//! a fetch adds objects and moves remote-tracking refs; it touches no local
//! branch, no index and no working tree, and every ref it moves is in the
//! reflog wherever the repository keeps one (a bare repository logs nothing
//! by default; the old tip's commits survive until `gc` either way). The one
//! deletion it performs is the user's own: `fetch.prune` (or
//! `remote.<name>.prune`, which overrides it) is honoured exactly as
//! `git fetch` honours it, so a remote-tracking ref the remote has already
//! deleted is removed here too — its commits survive until `gc`, and the
//! ref was never local work. Nothing is passed for prune, so git reads that
//! configuration itself, fresh, on every run (decided on issue #17).
//!
//! What is never done, whatever the configuration: pruning a local tag.
//! Tags have no reflog, so `--no-prune-tags` is always passed (in git since
//! 2.17, within [`super::GitVersion::MINIMUM`]) and `fetch.pruneTags` and
//! `remote.<name>.pruneTags` are ignored — a user who set them gets stale
//! tags left standing rather than removed without a word. That flag only
//! withholds the tag refspec git would add, so a remote whose OWN refspecs
//! write `refs/tags/` is refused while pruning is on, before any process
//! starts (`super::refspec_policy`). The same check refuses a remote whose
//! refspecs would write local branches — a mirror clone, `remote.<name>.mirror`,
//! `+refs/*:refs/*`, any `refs/heads/*` destination — with the setting quoted
//! in [`Error::FetchRefused`]; a fetch that overwrites local branches, with
//! no reflog in a bare repository, is a fetch from a terminal, not from a
//! button. Push, the next operation on this backend, is the one that needs
//! the token whatever the configuration (L8); pruning that says what will
//! go, as a confirmed operation, is issue #17's remainder.
//!
//! What it invalidates: `refs` (remote-tracking refs moved) and `objects`
//! (new objects arrived), declared on the [`Performed`] per the cache contract
//! in the [`super`] module docs, and honoured by the worker in `cairn-app` by
//! reopening its history walk from `HEAD`.
//!
//! Progress is git's own: `--progress` makes it write its meters to stderr
//! even with no terminal, and [`FetchInProgress::finish`] hands each redrawn
//! line to the caller as it arrives. Cancelling ends the process
//! ([`FetchCancel`]): `SIGTERM`, which git cleans its lock files up on, then
//! `SIGKILL` if it is still there after the grace period (`ProcessKill::kill`
//! has the numbers). Whatever `*.lock` files are under the git directory
//! afterwards — a `SIGKILL` that landed mid ref-update, a lock from an
//! earlier crash, or one another git holds this instant, which a listing
//! cannot tell apart — travel on the [`Error::GitCancelled`] so the banner
//! can name them and say when acting on them is safe (`super::stranded_locks`).

use std::path::PathBuf;

use cairn_model::AskpassToken;

use super::{GitBinary, Invalidated, Performed, refspec_policy, stranded_locks};
use crate::ops::cli::{ProcessKill, Running};
use crate::{Error, Repository};

/// Starts `git fetch --progress --no-prune-tags <remote>` in `repo`, with
/// `token` as the operation's askpass authorisation when there is a channel
/// to answer on; without one, a prompt fails closed. Returns as soon as the
/// process is running; [`FetchInProgress::finish`] waits for it. A remote
/// whose configuration would have the fetch write local branches, or delete
/// local tags under pruning, is [`Error::FetchRefused`] and no process
/// starts (module docs).
///
/// `remote` is a configured remote name or a URL, as `git fetch` takes it;
/// it follows `--end-of-options`, so a name beginning with `-` is a remote and
/// never an option.
pub fn fetch(
    git: &GitBinary,
    repo: &Repository,
    remote: &str,
    token: Option<&AskpassToken>,
) -> Result<FetchInProgress, Error> {
    refspec_policy::check(repo.git_dir(), remote)?;
    let mut command = git
        .command()
        .in_repository(repo)
        .args(ARGUMENTS)
        .arg(remote);
    if let Some(token) = token {
        command = command.authorized_by(token);
    }
    Ok(FetchInProgress {
        running: command.stream()?,
        remote: remote.to_owned(),
        git_dir: repo.git_dir().to_owned(),
        common_dir: repo.inner().common_dir().to_owned(),
    })
}

/// Everything before the remote. Nothing for prune, so git applies the
/// user's `fetch.prune`; `--no-prune-tags` always, so no tag is ever pruned
/// (module docs).
const ARGUMENTS: [&str; 4] = ["fetch", "--progress", "--no-prune-tags", "--end-of-options"];

/// A fetch that has been started; see [`fetch`].
#[derive(Debug)]
pub struct FetchInProgress {
    running: Running,
    remote: String,
    /// Where a cancel looks for what it stranded; paths rather than the
    /// repository, which belongs to the thread that opened it.
    git_dir: PathBuf,
    common_dir: PathBuf,
}

impl FetchInProgress {
    /// A handle that cancels this fetch from another thread.
    pub fn canceller(&self) -> FetchCancel {
        FetchCancel(self.running.killer())
    }

    /// Streams git's progress to `progress`, one line per redraw, until the
    /// process exits. Success is a [`Performed`] declaring `refs` and
    /// `objects` invalid — the declaration is what a fetch CAN change; whether
    /// a ref actually moved is for the caller to see, which the worker does by
    /// comparing [`crate::Repository::ref_tips`] before and after, on every
    /// outcome, since a failed or killed fetch may have updated some refs
    /// before it stopped. A cancelled fetch is [`Error::GitCancelled`],
    /// carrying every `*.lock` left under the git directory once git is gone;
    /// anything git refused is [`Error::GitFailed`] carrying its stderr —
    /// which is where "could not read Username ...: terminal prompts
    /// disabled" arrives when no credential could be had.
    pub fn finish(self, progress: impl FnMut(&str)) -> Result<Performed, Error> {
        match self.running.finish(progress) {
            Ok(_) => Ok(Performed::new(
                format!("fetched {}", self.remote),
                Invalidated::refs().and(Invalidated::objects()),
            )),
            // Searched after the reap, so a lock git removed on its way out is not
            // reported; the runner knows a process, not a repository.
            Err(Error::GitCancelled { arguments, .. }) => Err(Error::GitCancelled {
                arguments,
                stranded_locks: stranded_locks::stranded_locks(&self.git_dir, &self.common_dir),
            }),
            Err(error) => Err(error),
        }
    }
}

/// Cancels a running fetch by ending the `git` process (`SIGTERM`, then
/// `SIGKILL` after the grace period). Cloneable and `Send`, so the thread
/// waiting in [`FetchInProgress::finish`] need not be the one deciding to
/// stop; the waiter still reaps the process, and never blocks the caller.
#[derive(Debug, Clone)]
pub struct FetchCancel(ProcessKill);

impl FetchCancel {
    pub fn cancel(&self) {
        self.0.kill();
    }
}

/// Against a stub `git`, since the runner is crate-private: what the process is
/// actually given. The behaviour the arguments buy is pinned end to end by the
/// pruning tests in `tests/fetch.rs`.
#[cfg(all(test, unix))]
mod tests {
    use super::super::stub_git::{StubGit, discover_retrying};
    use super::*;

    /// Nothing for prune — git reads `fetch.prune` itself — and `--no-prune-tags`
    /// always, before `--end-of-options` and the remote.
    #[test]
    fn the_arguments_leave_prune_to_git_forbid_pruning_tags_and_end_the_options() {
        let stub = StubGit::with_git(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
             printf '%s\\n' \"$@\" >&2",
        );
        let git = discover_retrying(stub.environment()).unwrap();
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let mut seen = Vec::new();
        fetch(&git, &repo, "-origin", None)
            .unwrap()
            .finish(|line| seen.push(line.to_owned()))
            .unwrap();
        assert_eq!(
            seen,
            [
                "fetch",
                "--progress",
                "--no-prune-tags",
                "--end-of-options",
                "-origin",
            ]
        );
    }
}
