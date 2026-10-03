//! Repository worker threads.

mod askpass;
mod discovery;
mod epoch;
#[cfg(test)]
mod fetch_tests;
#[cfg(test)]
mod lifecycle_tests;
mod network_lane;
mod pool;
mod request;
mod startup;
mod wake;

pub use askpass::{PromptId, Reply};
pub use discovery::Discovery;
#[cfg(test)]
pub use pool::idle_handle;
pub use pool::{CLOSE_PATIENCE, Replier, RepositoryHandle, open};
pub use request::{Request, Update};
