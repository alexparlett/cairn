//! The refresh thread: the working tree's status and each branch's ahead/behind.

pub(super) mod lane;
#[cfg(test)]
pub(super) mod tests;
