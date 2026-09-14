//! Cairn's repository engine.
//!
//! Every read of a repository and every mutation of one happens here, behind
//! functions that speak [`cairn_model`] types. `gix` types never appear in a
//! public signature: the UI links this crate's vocabulary, not gitoxide's, so
//! the backend stays replaceable and the UI stays unable to reach a repository
//! by accident.
//!
//! Nothing in this crate spawns threads or assumes an async runtime. Callers
//! decide where the blocking work runs; `cairn-app` runs it off the UI thread.

mod error;
pub mod ops;
mod repository;

pub use error::Error;
pub use repository::Repository;
