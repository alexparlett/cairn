//! The history lane on the repository thread: the walk held open across pages, and a
//! find's pages.

pub(super) mod lane;
#[cfg(test)]
pub(super) mod tests;
