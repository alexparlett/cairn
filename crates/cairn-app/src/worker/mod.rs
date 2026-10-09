//! Repository worker threads.

mod askpass;
mod diff_answers;
mod diff_freshness;
mod diff_lane;
#[cfg(test)]
mod diff_tests;
mod discovery;
mod epoch;
mod expand_all;
#[cfg(test)]
mod fetch_tests;
#[cfg(test)]
mod find_tests;
mod history_lane;
#[cfg(test)]
mod lifecycle_tests;
mod local_lane;
#[cfg(test)]
mod local_lane_tests;
mod network_lane;
mod pool;
mod refresh_lane;
#[cfg(test)]
mod refresh_tests;
mod request;
mod routing;
mod startup;
mod wake;
#[cfg(test)]
mod window_check_updates;
#[cfg(test)]
mod written_repository;

pub use askpass::{PromptId, Reply};
#[cfg(test)]
pub(crate) use diff_tests::{Configurable, changes_answer, checkout, commits, next_update};
pub use discovery::Discovery;
#[cfg(test)]
pub(crate) use expand_all::EXPAND_ALL_LINES;
#[cfg(test)]
pub use local_lane::Done;
pub use local_lane::{LocalWrite, OperationId, ReadAgain, UnstageTarget, WriteEnding};
#[cfg(test)]
pub use pool::Updates;
#[cfg(test)]
pub use pool::idle_handle;
pub use pool::{CLOSE_PATIENCE, Replier, RepositoryHandle, open};
#[cfg(test)]
pub(crate) use refresh_tests::Refreshable;
pub use request::{
    AllEnded, AllFrom, AllProgress, AmendRead, CommitReads, Comparison, DiffOptions, DiffQuery,
    ExpandQuery, ExpandedFile, FileQuery, FileTarget, OpenedFile, Refreshed, Request, Retired,
    TogetherEnded, TogetherFile, TogetherOutcome, TogetherQuery, Update, WorkingSide,
    expanded_diffs, together_diffs,
};
#[cfg(test)]
pub(crate) use window_check_updates::update_within;
