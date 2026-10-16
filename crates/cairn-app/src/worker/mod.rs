//! Repository worker threads.

mod askpass;
mod diff;
mod discovery;
mod ending_words;
mod epoch;
mod history;
#[cfg(test)]
mod lifecycle_tests;
mod local;
mod network;
mod output_tap;
mod pool;
mod refresh;
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
pub(crate) use diff::expand_all::EXPAND_ALL_LINES;
#[cfg(test)]
pub(crate) use diff::tests::{Configurable, changes_answer, checkout, commits, next_update};
pub use discovery::Discovery;
#[cfg(test)]
pub(crate) use ending_words::{GIT_FAILED, checkout_refused_in, commit_refused_in};
#[cfg(test)]
pub use local::lane::Done;
pub use local::lane::{
    Failure, LocalWrite, OperationId, ReadAgain, ResultLines, Unspent, UnstageTarget, WriteEnding,
};
#[cfg(test)]
pub(crate) use local::tests::StagedFile;
pub use output_tap::OutputTap;
#[cfg(test)]
pub use pool::Updates;
#[cfg(test)]
pub use pool::idle_handle;
pub use pool::{CLOSE_PATIENCE, Replier, RepositoryHandle, open};
#[cfg(test)]
pub(crate) use refresh::tests::Refreshable;
pub use request::{
    AllEnded, AllFrom, AllProgress, AmendRead, CheckoutRefused, CommitReads, Comparison,
    DiffOptions, DiffQuery, ExpandQuery, ExpandedFile, FileQuery, FileTarget, OpenedFile, RanBy,
    Refreshed, Request, Retired, Update, WorkingSide, expanded_diffs,
};
#[cfg(test)]
pub(crate) use window_check_updates::update_within;
