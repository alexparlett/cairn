//! The history lane: its thread, `cairn-history`, with the walk held open across pages and a
//! find's pages; the refs it walks from, read on the repository thread; and when the window is
//! told how far an opening pass has read.

pub(super) mod lane;
#[cfg(test)]
mod opening;
pub(super) mod reading;
pub(super) mod refs;
#[cfg(test)]
pub(super) mod tests;
