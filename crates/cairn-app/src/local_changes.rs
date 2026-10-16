//! Local Changes as the window keeps and draws it (`docs/systems/local-changes.md`): its
//! lists and selection, where the selection goes next, and what its actions do.

pub(crate) mod actions;
pub(crate) mod next_selection;
pub(crate) mod pane;
pub(crate) mod state;
#[cfg(test)]
pub(crate) mod tests;
