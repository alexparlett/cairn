//! Repository mutations.
//!
//! This module is the only place in Cairn that writes to a repository, and the
//! guard suite pins that: a mutating gitoxide call or a `git` subprocess
//! anywhere else fails the gate.
//!
//! Operations that can destroy work a user cannot recover from `git reflog`
//! alone — force push, hard reset, branch deletion, history rewrites — take a
//! [`cairn_model::Confirmed`] by value. The token cannot be forged, so the type
//! system, not a review, is what keeps a destructive path from being reached
//! without a prompt.
//!
//! # How a mutation runs
//!
//! Every write goes through the `git` binary rather than gitoxide (design
//! decision D1): a mutation must run the user's hooks, filters and credential
//! helpers and honour their configuration, and gix runs none of them. The
//! pieces, each its own module:
//!
//! - [`GitBinary`] finds `git` on `PATH` once at startup and refuses one older
//!   than [`GitVersion::MINIMUM`], loudly, with the required version in the
//!   message. Nothing degrades silently.
//! - [`GitEnvironment`] is the environment every invocation runs with. It is
//!   built from a spelled-out roster, never inherited wholesale, and it always
//!   sets `GIT_TERMINAL_PROMPT=0` and `SSH_ASKPASS_REQUIRE=force` and points
//!   `GIT_ASKPASS` and `SSH_ASKPASS` at Cairn's helper ([`Askpass`]). It is
//!   also the only place a `std::process::Command` is built, so no process
//!   exists without it; an invocation that may prompt is given its askpass
//!   token there too, per invocation, because that is what the token is.
//! - `GitCommand` (crate-private, like `Output`) adds the arguments and runs
//!   the process with standard input closed — to completion, or streaming its
//!   stderr and killable from another thread, which is what a long fetch
//!   needs. Nothing outside `ops` can run a raw verb: the public surface is
//!   named operations ([`fetch`] today), so the confirmation seal cannot be
//!   routed around through the runner.
//!
//! # Output policy
//!
//! Where git offers a machine-readable form, use it and nothing else: `-z` for
//! anything that lists paths (`Output::records` splits it), `--porcelain=v2`
//! for status, `--format` with explicit field separators for log-like output.
//! Human-facing output — `git status` without `--porcelain`, `git fetch`'s
//! progress lines — is never parsed on a path where a machine-readable form
//! exists. Standard error is prose for a person: it travels into the error
//! verbatim and is shown, never matched on.
//!
//! # Errors
//!
//! Failures are [`Error`] variants naming what the caller must handle:
//! [`Error::GitNotFound`], [`Error::GitTooOld`] and
//! [`Error::GitVersionUnreadable`] at startup, [`Error::GitNotStarted`] when
//! the process could not be launched, and [`Error::GitFailed`] when git ran
//! and exited non-zero — carrying the arguments, the exit status and git's own
//! stderr, so "git failed" is never the whole message. Because stderr is never
//! matched on, an outcome a caller must tell apart from "git failed" — a
//! credential prompt the user cancelled, say — has to come from a channel of
//! its own (the askpass channel in `cairn-askpass`, whose `Prompt::refuse`
//! is what the worker sees), never from reading git's prose.
//!
//! # The cache-invalidation contract
//!
//! D1 puts two implementations of git semantics in one process. After a `git`
//! subprocess writes, the gitoxide handle the worker holds may be looking at a
//! repository that no longer exists. So every operation declares what it
//! invalidated, as the [`Invalidated`] on its [`Performed`], and the worker
//! boundary honours the declaration — the engine states, the worker acts. What
//! each flag means, checked against gix 0.87.1 as linked:
//!
//! - **`refs`** — a ref moved, appeared or vanished. gix re-reads loose refs on
//!   every lookup and reloads `packed-refs` when its modification time changes
//!   (a rewrite inside the same timestamp tick as the previous read is the
//!   residual it cannot see, as for the index below), so a fresh lookup is
//!   fresh. What is NOT fresh is anything that resolved a ref earlier and kept
//!   the answer: an open [`crate::HistorySession`] and its
//!   [`crate::HistoryCursor`] hold the tips they started from. Honouring `refs`
//!   means dropping the open session and cursor and querying the history again
//!   from `HEAD`; a page from the old walk must not be appended to a view of
//!   the new refs.
//! - **`index`** — the staging area changed. gix shares one index snapshot
//!   between the [`crate::SharedRepository`] and every worker handle, and
//!   re-reads it when the file's modification time changes. Honouring `index`
//!   means re-running any query that read it; nothing in `cairn-git` caches an
//!   answer derived from it. A write inside the same timestamp tick as the
//!   previous read is the residual gix cannot see, so an operation that writes
//!   the index and then reads it back in one request must reopen a handle.
//! - **`objects`** — new objects arrived or packs were rewritten. gix's object
//!   store refreshes its view of the pack directory when a lookup misses after
//!   every known index is loaded (`RefreshMode::AfterAllIndicesLoaded`, the
//!   default Cairn keeps), so new objects are found on demand. A pack deleted
//!   by a repack stays readable through its existing mapping until that same
//!   lookup-miss refresh unloads it, so a stale pack is a wasted mapping, not
//!   a wrong answer. Honouring `objects` requires nothing of a handle today;
//!   the declaration exists so that an operation which writes objects says so,
//!   and so a future change to the refresh mode knows what it would break.
//! - **`working_tree`** — files on disk changed. Nothing in gix caches the
//!   working tree; honouring it means re-running a status or diff query.
//!
//! Where it is honoured: the repository worker in `cairn-app` (decision D3),
//! which owns the handle, the open session and the cursor, and is the only
//! place a `Performed` arrives. [`fetch`] is the first operation to reach it:
//! it declares `refs` and `objects`, and the worker answers by dropping its
//! open walk and querying the history again from `HEAD`, so the graph stops
//! showing the pre-fetch refs.

mod askpass;
mod binary;
mod cli;
mod environment;
mod fetch;
#[cfg(all(test, unix))]
mod stub_git;

use cairn_model::Confirmed;

pub use askpass::Askpass;
pub use binary::{GitBinary, GitVersion};
pub(crate) use cli::GitCommand;
pub use environment::GitEnvironment;
pub use fetch::{FetchCancel, FetchInProgress, fetch};

use crate::Repository;

/// What a mutation left stale in a gitoxide handle; see the module docs for what
/// honouring each flag means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Invalidated {
    pub refs: bool,
    pub index: bool,
    pub objects: bool,
    pub working_tree: bool,
}

impl Invalidated {
    /// A mutation that changed nothing a handle could be holding.
    pub const NOTHING: Self = Self {
        refs: false,
        index: false,
        objects: false,
        working_tree: false,
    };

    pub const fn refs() -> Self {
        Self {
            refs: true,
            ..Self::NOTHING
        }
    }

    pub const fn objects() -> Self {
        Self {
            objects: true,
            ..Self::NOTHING
        }
    }

    pub const fn index() -> Self {
        Self {
            index: true,
            ..Self::NOTHING
        }
    }

    pub const fn working_tree() -> Self {
        Self {
            working_tree: true,
            ..Self::NOTHING
        }
    }

    /// Both declarations at once; an operation names every flag it touches.
    pub const fn and(self, other: Self) -> Self {
        Self {
            refs: self.refs || other.refs,
            index: self.index || other.index,
            objects: self.objects || other.objects,
            working_tree: self.working_tree || other.working_tree,
        }
    }

    pub const fn anything(self) -> bool {
        self.refs || self.index || self.objects || self.working_tree
    }
}

/// A mutation that has been performed, for the operation log the UI shows and
/// for the worker that must honour what it invalidated.
///
/// The fields are private so that a record carrying an acknowledged prompt can
/// only be built from a [`Confirmed`] token: the log quotes what the user
/// agreed to, and no operation can write that quote by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performed {
    description: String,
    acknowledged: Option<String>,
    invalidated: Invalidated,
}

impl Performed {
    /// A mutation that needed no confirmation.
    pub(crate) fn new(description: impl Into<String>, invalidated: Invalidated) -> Self {
        Self {
            description: description.into(),
            acknowledged: None,
            invalidated,
        }
    }

    /// A destructive mutation: the prompt is taken from the token, never typed.
    pub(crate) fn destructive(
        description: impl Into<String>,
        confirmed: &Confirmed,
        invalidated: Invalidated,
    ) -> Self {
        Self {
            acknowledged: Some(confirmed.acknowledged().to_owned()),
            ..Self::new(description, invalidated)
        }
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// The prompt the user acknowledged, when the operation needed one.
    pub fn acknowledged(&self) -> Option<&str> {
        self.acknowledged.as_deref()
    }

    pub fn invalidated(&self) -> Invalidated {
        self.invalidated
    }
}

/// Placeholder proving the seal compiles end to end; replaced by the first real
/// destructive operation. It performs no I/O.
pub fn describe_destructive(repo: &Repository, confirmed: Confirmed) -> Performed {
    let _ = repo.inner();
    Performed::destructive(
        format!("no-op against {}", repo.git_dir().display()),
        &confirmed,
        Invalidated::NOTHING,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_destructive_operation_records_what_the_user_agreed_to() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let performed = describe_destructive(&repo, Confirmed::by_user("Discard 3 local commits?"));
        assert_eq!(performed.acknowledged(), Some("Discard 3 local commits?"));
        assert_eq!(
            performed.description(),
            format!("no-op against {}", repo.git_dir().display())
        );
        assert_eq!(performed.invalidated(), Invalidated::NOTHING);
        assert!(!performed.invalidated().anything());
    }

    #[test]
    fn an_unconfirmed_operation_carries_no_prompt() {
        let performed = Performed::new("fetched origin", Invalidated::refs());
        assert_eq!(performed.acknowledged(), None);
        assert_eq!(performed.description(), "fetched origin");
        assert!(performed.invalidated().refs);
    }

    /// Caught by: dropping any one flag from `anything`'s disjunction.
    #[test]
    fn every_single_flag_counts_as_something() {
        for single in [
            Invalidated::refs(),
            Invalidated::index(),
            Invalidated::objects(),
            Invalidated::working_tree(),
        ] {
            assert!(single.anything(), "{single:?}");
            assert_eq!(single.and(Invalidated::NOTHING), single);
        }
    }

    #[test]
    fn declarations_combine_without_losing_a_flag() {
        let fetch = Invalidated::refs().and(Invalidated::objects());
        assert_eq!(
            fetch,
            Invalidated {
                refs: true,
                objects: true,
                index: false,
                working_tree: false,
            }
        );
        assert!(fetch.anything());
        assert_eq!(fetch.and(Invalidated::NOTHING), fetch);
        let checkout = Invalidated::index().and(Invalidated::working_tree());
        assert!(checkout.index && checkout.working_tree && !checkout.refs && !checkout.objects);
    }
}
