//! Repository worker threads.

mod askpass;
mod diff_answers;
mod diff_freshness;
mod diff_lane;
#[cfg(test)]
mod diff_tests;
mod discovery;
mod epoch;
#[cfg(test)]
mod fetch_tests;
#[cfg(test)]
mod lifecycle_tests;
mod network_lane;
mod pool;
mod request;
mod routing;
mod startup;
mod wake;

pub use askpass::{PromptId, Reply};
#[cfg(test)]
pub(crate) use diff_tests::{Configurable, changes_answer, checkout, commits, next_update};
pub use discovery::Discovery;
#[cfg(test)]
pub use pool::idle_handle;
pub use pool::{CLOSE_PATIENCE, Replier, RepositoryHandle, open};
#[cfg(test)]
pub use request::WorkingSide;
pub use request::{
    Comparison, DiffOptions, DiffQuery, FileQuery, FileTarget, Request, Retired, Update,
};
