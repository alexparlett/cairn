//! Cairn's repository engine.

mod ahead_behind;
mod bare_discovery;
mod branch_names;
mod cancel;
mod commit;
mod commit_encoding;
mod commit_hooks;
mod diff;
mod error;
mod history;
mod object_id;
mod operation_in_progress;
pub mod ops;
mod ownership;
mod process;
mod reads;
mod recent_messages;
mod ref_storage;
mod refs;
mod remotes;
mod repository;
mod shallow;
mod status;

pub use ahead_behind::AheadBehindRead;
pub use cancel::{Cancel, CancelSignal};
pub use diff::{
    ChangesRequest, ContentOptions, DiffInputs, DiffSession, LineBudget, Offered, PAGE_FILES,
    PAGE_LINES, Page, StagedInputs, WorkingTreeDiff,
};
pub use error::{CheckoutRefusal, CommitRefusal, Error, Refusal, RefusedWrite};
pub use history::{HistoryCursor, HistoryOrder, HistoryPage, HistoryRequest, HistorySession};
pub use recent_messages::RECENT_MESSAGES;
pub use refs::{RefsCost, RefsRead};
pub use repository::{CLOSE_BOUND, Repository, SharedRepository};
