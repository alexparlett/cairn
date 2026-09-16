//! Cairn's repository engine.
//!
//! Every read and every mutation of a repository happens here, behind functions
//! that speak [`cairn_model`] types. `gix` types never appear in a public
//! signature, which is what keeps the backend replaceable.
//!
//! Nothing here spawns threads or assumes an async runtime: callers decide
//! where the blocking work runs.

mod cancel;
mod error;
mod history;
pub mod ops;
mod repository;

pub use cancel::{Cancel, CancelSignal};
pub use error::Error;
pub use history::{HistoryCursor, HistoryOrder, HistoryPage, HistoryRequest, HistorySession};
pub use repository::{Repository, SharedRepository};
