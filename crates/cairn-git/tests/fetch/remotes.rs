//! Remotes the fetch tests reach, each demanding a credential of its own
//! kind, and the askpass channel that answers for them. Only the fetch test
//! (`fetch/main.rs`) declares this module.

pub mod askpass;
pub mod http;
pub mod ssh;
