//! Cairn's repository engine.

mod bare_discovery;
mod cancel;
mod commit;
mod commit_encoding;
mod diff;
mod error;
mod history;
mod object_id;
pub mod ops;
mod ownership;
mod process;
mod reads;
mod refs;
mod remotes;
mod repository;
mod shallow;

pub use cancel::{Cancel, CancelSignal};
pub use diff::{
    ChangesRequest, ContentOptions, DiffInputs, DiffSession, LineBudget, Offered, PAGE_FILES,
    PAGE_LINES, Page, StagedInputs, WorkingTreeDiff,
};
pub use error::{Error, RefusedWrite};
pub use history::{HistoryCursor, HistoryOrder, HistoryPage, HistoryRequest, HistorySession};
pub use repository::{CLOSE_BOUND, Repository, SharedRepository};
