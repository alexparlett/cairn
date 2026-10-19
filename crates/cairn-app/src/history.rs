//! The history as the window keeps it: its load state and status line, the row or pair
//! chosen and what they changed, a ref's row found by paging the walk, `HEAD`'s row shown, and
//! Show Lost Commits.

pub(crate) mod head;
pub(crate) mod lost_commits;
pub(crate) mod ref_find;
pub(crate) mod selection;
pub(crate) mod state;
pub(crate) mod status_text;
