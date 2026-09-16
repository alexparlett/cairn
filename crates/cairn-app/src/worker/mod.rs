//! Repository worker threads.

mod epoch;
mod pool;
mod request;
mod wake;

pub use pool::{RepositoryHandle, open};
pub use request::{Request, Update};
