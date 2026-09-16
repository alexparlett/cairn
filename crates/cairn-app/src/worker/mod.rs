//! Repository worker threads.
//!
//! Files here may name `cairn_git` and may block; no other file in `cairn-app`
//! may. Twin: `the_ui_thread_never_waits_on_repository_work`.
//!
//! [`RepositoryHandle::submit`], [`Updates::next`] and `Wake` run on the UI
//! thread and are exempt from that matcher by location. That they never block is
//! a review obligation.
//!
//! See `docs/systems/history-graph.md`, "The worker boundary".

mod epoch;
mod pool;
mod request;
mod wake;

pub use pool::{RepositoryHandle, open};
pub use request::{Request, Update};
