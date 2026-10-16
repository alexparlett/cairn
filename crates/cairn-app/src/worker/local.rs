//! The local write lane, where every write to the index, the working tree and a local ref
//! runs.

pub(super) mod lane;
#[cfg(test)]
pub(super) mod tests;
