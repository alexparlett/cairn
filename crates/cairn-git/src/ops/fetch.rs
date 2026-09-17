//! Fetching from a remote: the first operation to run a `git` verb, and the
//! first that can ask the user for a credential.
//!
//! **Not destructive, and deliberately takes no [`cairn_model::Confirmed`].**
//! With the default refspec (`+refs/heads/*:refs/remotes/<remote>/*`) a fetch
//! adds objects and moves remote-tracking refs; it touches no local branch,
//! no index and no working tree, and every ref it moves is in the reflog.
//! What could make a plain `git fetch` DELETE a ref is the user's
//! configuration — `fetch.prune` and `remote.<name>.prune` remove
//! remote-tracking refs gone from the remote, `fetch.pruneTags` and
//! `remote.<name>.pruneTags` remove even purely local tags — so this
//! operation always passes `--no-prune --no-prune-tags` (both in git since
//! 2.17, so within [`super::GitVersion::MINIMUM`]): no configuration makes
//! Cairn's fetch delete a ref, and a user who set those on purpose gets a
//! fetch that leaves stale refs standing rather than one that removes them
//! without a word. `--no-prune` alone is what decides it — git prunes tags
//! only when it is pruning at all — so the second flag is belt and braces
//! against a `--prune` ever being added to this invocation, and is pinned as
//! an argument rather than as behaviour. Pruning as a named operation that
//! says what will go is the remainder of issue #17.
//!
//! What the flags cannot cover is a configured refspec that OVERWRITES a
//! local ref, which no prune flag touches: a destination under `refs/heads/`
//! (a `--mirror` clone, or a bare repository set up to take the remote's
//! branches as its own; git refuses only a branch that is checked out, and
//! in a bare repository `core.logAllRefUpdates` is off by default, so the
//! old tip goes to no reflog), or a forced tag refspec
//! (`+refs/tags/*:refs/tags/*`) in any clone at all, since git never reflogs
//! a tag. This operation runs `git fetch` as the shell would and inspects no
//! refspec; deciding that needs the remote model that reads them, and is
//! left on issue #17 rather than approximated here. Push, the next operation
//! on this backend, is the one that needs the token whatever the
//! configuration (L8).
//!
//! What it invalidates: `refs` (remote-tracking refs moved) and `objects`
//! (new objects arrived), declared on the [`Performed`] per the cache contract
//! in the [`super`] module docs, and honoured by the worker in `cairn-app` by
//! reopening its history walk from `HEAD`.
//!
//! Progress is git's own: `--progress` makes it write its meters to stderr
//! even with no terminal, and [`FetchInProgress::finish`] hands each redrawn
//! line to the caller as it arrives. Cancelling kills the process
//! ([`FetchCancel`]); see `ProcessKill::kill` for what a kill can leave
//! behind — a stale `*.lock` if it lands mid ref-update, which git does not
//! clean up after `SIGKILL`.

use cairn_model::AskpassToken;

use super::{GitBinary, Invalidated, Performed};
use crate::ops::cli::{ProcessKill, Running};
use crate::{Error, Repository};

/// Starts `git fetch --progress --no-prune --no-prune-tags <remote>` in
/// `repo`, with `token` as the operation's askpass authorisation when there
/// is a channel to answer on; without one, a prompt fails closed. Returns as
/// soon as the process is running; [`FetchInProgress::finish`] waits for it.
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
    })
}

/// Everything before the remote. The two `--no-prune` flags are what keeps
/// "deletes nothing" true under the user's configuration (module docs).
const ARGUMENTS: [&str; 5] = [
    "fetch",
    "--progress",
    "--no-prune",
    "--no-prune-tags",
    "--end-of-options",
];

/// A fetch that has been started; see [`fetch`].
#[derive(Debug)]
pub struct FetchInProgress {
    running: Running,
    remote: String,
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
    /// before it stopped. A cancelled fetch is [`Error::GitCancelled`], and
    /// anything git refused is [`Error::GitFailed`] carrying its stderr — which
    /// is where "could not read Username ...: terminal prompts disabled"
    /// arrives when no credential could be had.
    pub fn finish(self, progress: impl FnMut(&str)) -> Result<Performed, Error> {
        self.running.finish(progress)?;
        Ok(Performed::new(
            format!("fetched {}", self.remote),
            Invalidated::refs().and(Invalidated::objects()),
        ))
    }
}

/// Cancels a running fetch by killing the `git` process. Cloneable and
/// `Send`, so the thread waiting in [`FetchInProgress::finish`] need not be
/// the one deciding to stop; the waiter still reaps the process.
#[derive(Debug, Clone)]
pub struct FetchCancel(ProcessKill);

impl FetchCancel {
    pub fn cancel(&self) {
        self.0.kill();
    }
}

/// Against a stub `git`, since the runner is crate-private: what the process is
/// actually given. The behaviour the flags buy is pinned end to end by
/// `a_fetch_never_prunes_however_the_repository_is_configured` in
/// `tests/fetch.rs`.
#[cfg(all(test, unix))]
mod tests {
    use super::super::stub_git::{StubGit, discover_retrying};
    use super::*;

    #[test]
    fn the_arguments_forbid_pruning_and_end_the_options_before_the_remote() {
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
                "--no-prune",
                "--no-prune-tags",
                "--end-of-options",
                "-origin",
            ]
        );
    }
}
