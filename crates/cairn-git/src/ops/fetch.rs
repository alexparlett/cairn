//! Fetching from a remote: the first operation to run a `git` verb, and the
//! first that can ask the user for a credential.
//!
//! **Not destructive, and deliberately takes no [`cairn_model::Confirmed`].**
//! A fetch adds objects and moves remote-tracking refs; it touches no local
//! branch, no index and no working tree, and every ref it moves is in the
//! reflog. There is nothing to warn the user about before it runs, so the
//! omission of the token is the decision, not an oversight — the next
//! operation on this backend, push, is the one that needs it (L8).
//!
//! What it invalidates: `refs` (remote-tracking refs moved) and `objects`
//! (new objects arrived), declared on the [`Performed`] per the cache contract
//! in the [`super`] module docs, and honoured by the worker in `cairn-app` by
//! reopening its history walk from `HEAD`.
//!
//! Progress is git's own: `--progress` makes it write its meters to stderr
//! even with no terminal, and [`FetchInProgress::finish`] hands each redrawn
//! line to the caller as it arrives. Cancelling kills the process
//! ([`FetchCancel`]); a fetch has nothing to save, and git's next invocation
//! clears any lock it left.

use cairn_model::AskpassToken;

use super::{GitBinary, Invalidated, Performed};
use crate::ops::cli::{ProcessKill, Running};
use crate::{Error, Repository};

/// Starts `git fetch --progress <remote>` in `repo`, with `token` as the
/// operation's askpass authorisation when there is a channel to answer on;
/// without one, a prompt fails closed. Returns as soon as the process is
/// running; [`FetchInProgress::finish`] waits for it.
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
    let mut command =
        git.command()
            .in_repository(repo)
            .args(["fetch", "--progress", "--end-of-options", remote]);
    if let Some(token) = token {
        command = command.authorized_by(token);
    }
    Ok(FetchInProgress {
        running: command.stream()?,
        remote: remote.to_owned(),
    })
}

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
    /// `objects` invalid; a cancelled fetch is [`Error::GitCancelled`], and
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
