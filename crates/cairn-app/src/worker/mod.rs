//! Where repository work happens, and the only part of `cairn-app` that may
//! reach a repository at all.
//!
//! The rule this module exists to make true: **the UI thread never waits on
//! repository work.** It is enforced by a partition rather than by care. Files
//! under `worker/` name `cairn_git` and may block — they run on worker threads.
//! Every other file in `cairn-app` renders, names `freya`, and may name neither
//! `cairn_git` nor any waiting primitive. The two sets are disjoint, so a render
//! path cannot reach a repository and cannot wait on one. Twin:
//! `the_ui_thread_never_waits_on_repository_work` in
//! `crates/cairn-guards/tests/invariants.rs`.
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

// Only what the view names. `RepositoryHandle`, `Updates` and `OpenError`
// reach it through `open`'s signature and are never spelled out there — a
// binary has no external callers, so an unnameable type is not a gap.
pub use pool::open;
pub use request::{Request, Update};
