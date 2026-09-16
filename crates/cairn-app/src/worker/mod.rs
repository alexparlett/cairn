//! Where repository work happens, and the only part of `cairn-app` that may
//! reach a repository at all.
//!
//! The rule this module exists to make true: **the UI thread never waits on
//! repository work.** It is enforced by a partition of FILES. Files under
//! `worker/` may name `cairn_git` and may block. Every other file in
//! `cairn-app` renders, names `freya`, and may name neither `cairn_git` nor any
//! waiting primitive. So a render path cannot reach a repository and cannot
//! wait on one. Twin: `the_ui_thread_never_waits_on_repository_work` in
//! `crates/cairn-guards/tests/invariants.rs`.
//!
//! Files, not threads — and the difference is the part to keep in mind while
//! editing here. Most of this module runs on a worker thread, but three things
//! do not: [`RepositoryHandle::submit`], [`Updates::next`] and the `Wake` it
//! parks on are called BY the UI thread, and being inside `worker/` exempts
//! them from the matcher. That they never block is a property of how they are
//! written, checked by review rather than by the guard, and it is stated as
//! such in `CLAUDE.md`. Anything added here that the UI thread will call
//! carries the same obligation.
//!
//! The seam across that partition is two values:
//!
//! - [`RepositoryHandle`], which components hold. Every method returns
//!   immediately, and it carries no receiving end of anything — there is
//!   nothing on it to wait on, which is the type-level half of the invariant.
//! - [`Updates`], which the application root owns and drives from one Freya
//!   task. Its `next` is a future: it yields to the event loop rather than
//!   holding it.
//!
//! ## Shaped for a second consumer
//!
//! This packet has one consumer — the paged history query — and an interface
//! shaped only around it would need widening for the next one. The second is
//! already specified: fetch (`docs/prd/credential-prompts.md` R4), which is
//! long-running, reports progress, and blocks mid-flight on a UI dialog for a
//! credential. Three properties here exist for it rather than for the graph:
//!
//! 1. A request is answered by a *stream* of [`Update`]s, not by one reply. A
//!    job may send as many as it likes before it finishes, so progress
//!    reporting is an added variant, not a changed shape.
//! 2. A worker runs ordinary blocking code. A job that must wait for a UI answer
//!    can make its own reply channel and block on it, because blocking a worker
//!    is exactly what workers are for. No pool surface is needed for that.
//! 3. Workers are pinned to a purpose, not fed from an anonymous queue. See
//!    [`pool::WORKERS_PER_REPOSITORY`]: a scroll's walk lives on one worker for
//!    its whole life, so fetch gets its own worker rather than a slot in a
//!    shared pool. That is the decision, not an omission.
//!
//! Nothing here builds fetch, and no part of it is stubbed. Those three are
//! properties of the design, checked against R4 so that adding fetch does not
//! mean touching every call site.

mod epoch;
mod pool;
mod request;
mod wake;

// Only what the view names. `Updates` and `OpenError` reach it through `open`'s
// signature and are never spelled out there — a binary has no external callers,
// so an unnameable type is not a gap.
//
// `RepositoryHandle` IS spelled out, because the view holds one across renders
// and hands it to the function that builds the list, which needs it in a
// signature. It is the safe half of the boundary to name: it has one method,
// that method returns immediately, and it carries no receiving end of anything,
// so there is nothing on it to wait on.
pub use pool::{RepositoryHandle, open};
pub use request::{Request, Update};
