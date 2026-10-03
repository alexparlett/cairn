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
pub(crate) use diff_tests::{changes_answer, checkout, commits};
pub use discovery::Discovery;
#[cfg(test)]
pub use pool::idle_handle;
pub use pool::{CLOSE_PATIENCE, Replier, RepositoryHandle, open};
pub use request::{Comparison, DiffOptions, DiffQuery, FileQuery, Request, Update};
#[cfg(test)]
pub use request::{FileTarget, WorkingSide};
