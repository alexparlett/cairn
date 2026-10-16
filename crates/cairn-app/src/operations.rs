//! An operation from its ask to its ending, as the window keeps it: the local writes, a
//! fetch and its credential prompt, a confirmation, Create Branch, where an ending goes,
//! the leftover lock, the activity popover, and git's output as the window holds and
//! draws it.

pub(crate) mod activity;
pub(crate) mod confirming;
pub(crate) mod create_branch;
pub(crate) mod ending_dialog;
pub(crate) mod fetch;
pub(crate) mod leftover_lock;
pub(crate) mod local_writes;
pub(crate) mod output_tail;
pub(crate) mod shown_output;
#[cfg(test)]
mod tests;
