//! The sidebar as the window keeps and draws it; what a press on a ref does is
//! `history::ref_find`'s, since it selects and pages the history.

pub(crate) mod menu;
pub(crate) mod pane;
pub(crate) mod state;
#[cfg(test)]
mod tests;
