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
//!   sets `GIT_TERMINAL_PROMPT=0`. It is also the only place a
//!   `std::process::Command` is built, so no process exists without it.
//! - [`GitCommand`] adds the arguments and runs the process with standard
//!   input closed, then hands back an [`Output`] or an [`Error`].
//!
//! # Output policy
//!
//! Where git offers a machine-readable form, use it and nothing else: `-z` for
//! anything that lists paths ([`Output::records`] splits it), `--porcelain=v2`
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
//! stderr, so "git failed" is never the whole message.
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
//!   every lookup and reloads `packed-refs` when its modification time changes,
//!   so a fresh lookup is fresh. What is NOT fresh is anything that resolved a
//!   ref earlier and kept the answer: an open [`crate::HistorySession`] and its
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
//!   default Cairn keeps), so new objects are found on demand and a repacked
//!   store is reloaded when a pack goes missing. Honouring `objects` requires
//!   nothing of a handle today; the declaration exists so that an operation
//!   which writes objects says so, and so a future change to the refresh mode
//!   knows what it would break.
//! - **`working_tree`** — files on disk changed. Nothing in gix caches the
//!   working tree; honouring it means re-running a status or diff query.
//!
//! Where it is honoured: the repository worker in `cairn-app` (decision D3),
//! which owns the handle, the open session and the cursor, and is the only
//! place a `Performed` arrives. No operation reaches it yet — the first is
//! fetch, which invalidates `refs` and `objects`, and the graph must stop
//! showing the pre-fetch refs when it lands.

mod binary;
mod cli;
mod environment;

use cairn_model::Confirmed;

pub use binary::{GitBinary, GitVersion};
pub use cli::{GitCommand, Output};
pub use environment::GitEnvironment;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performed {
    pub description: String,
    /// The prompt the user acknowledged, when the operation needed one.
    pub acknowledged: Option<String>,
    pub invalidated: Invalidated,
}

impl Performed {
    pub(crate) fn destructive(
        description: impl Into<String>,
        confirmed: &Confirmed,
        invalidated: Invalidated,
    ) -> Self {
        Self {
            description: description.into(),
            acknowledged: Some(confirmed.acknowledged().to_owned()),
            invalidated,
        }
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
        assert_eq!(
            performed.acknowledged.as_deref(),
            Some("Discard 3 local commits?")
        );
        assert_eq!(performed.invalidated, Invalidated::NOTHING);
        assert!(!performed.invalidated.anything());
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
