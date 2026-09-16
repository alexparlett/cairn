//! Repository worker threads.
//!
//! Files here may block and name `cairn_git`; no other file in `cairn-app` may.
//! Twin: `the_ui_thread_never_waits_on_repository_work`. `submit`,
//! `Updates::next` and `Wake` are UI-thread-callable and exempt by location;
//! that they never block is a review obligation.

mod epoch;
mod pool;
mod request;
mod wake;

pub use pool::{RepositoryHandle, open};
pub use request::{Request, Update};
