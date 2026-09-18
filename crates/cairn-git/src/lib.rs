//! Cairn's repository engine.

mod cancel;
mod commit;
mod diff;
mod error;
mod history;
mod object_id;
pub mod ops;
mod refs;
mod remotes;
mod repository;

pub use cancel::{Cancel, CancelSignal};
pub use diff::{ChangeSet, ChangesRequest, ContentOptions, DiffSession, RenameDetection};
pub use error::{Error, RefusedWrite};
pub use history::{HistoryCursor, HistoryOrder, HistoryPage, HistoryRequest, HistorySession};
pub use repository::{Repository, SharedRepository};
