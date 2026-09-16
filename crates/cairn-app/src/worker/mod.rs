//! Where repository work happens, and the only part of `cairn-app` that may
//! reach a repository at all.
//!
//! **The UI thread never waits on repository work**, enforced by a partition of
//! FILES: files under `worker/` may name `cairn_git` and may block, and every
//! other file in `cairn-app` may name neither `cairn_git` nor a waiting
//! primitive. Twin: `the_ui_thread_never_waits_on_repository_work` in
//! `crates/cairn-guards/tests/invariants.rs`.
//!
//! Files, not threads — which matters while editing here. [`RepositoryHandle::submit`],
//! [`Updates::next`] and the `Wake` it parks on are called BY the UI thread, and
//! being inside `worker/` exempts them from the matcher. That they never block is
//! a review obligation, not a checked one; anything added here that the UI thread
//! will call carries the same obligation.
//!
//! The seam is two values: [`RepositoryHandle`], which components hold and whose
//! methods return immediately, and [`Updates`], driven from exactly one Freya
//! task. The shape is deliberately wider than this packet needs — see
//! `docs/systems/history-graph.md`, "The worker boundary".

mod epoch;
mod pool;
mod request;
mod wake;

// Only what the view names. `Updates` and `OpenError` reach it through `open`'s
// signature without ever being spelled out; `RepositoryHandle` is named because
// the view holds one across renders and passes it on.
pub use pool::{RepositoryHandle, open};
pub use request::{Request, Update};
