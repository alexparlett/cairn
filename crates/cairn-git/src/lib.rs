//! Cairn's repository engine: repository handles, history queries, and `ops`.
//!
//! `gix` types never appear in a public signature. Nothing here spawns threads
//! or assumes an async runtime; callers decide where the blocking work runs.

mod cancel;
mod error;
mod history;
pub mod ops;
mod repository;

pub use cancel::{Cancel, CancelSignal};
pub use error::Error;
pub use history::{HistoryCursor, HistoryOrder, HistoryPage, HistoryRequest, HistorySession};
pub use repository::{Repository, SharedRepository};
