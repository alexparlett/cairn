//! The diff thread: its lane, the answers it keeps and whether they
//! are still fresh, and Expand All's line budget.

pub(super) mod answers;
pub(super) mod expand_all;
pub(super) mod freshness;
pub(super) mod lane;
#[cfg(test)]
pub(super) mod tests;
