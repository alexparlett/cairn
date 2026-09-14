//! Cairn's component library.
//!
//! Components here render [`cairn_model`] values and emit intent through
//! `EventHandler` props. They never open a repository, touch the filesystem, or
//! block: the crate does not depend on `cairn-git` and the guard suite pins
//! that. Wiring a component to the engine is `cairn-app`'s job.

mod commit_row;

pub use commit_row::CommitRow;
