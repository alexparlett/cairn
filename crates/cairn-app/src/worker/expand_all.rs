//! Expand All's line budget (PRD R5.3, packet question Q2).
//!
//! Expand All opens a change set's files in its order until this many lines are spent, then
//! leaves the rest collapsed and says how many. A file costs one, for itself, and every line of
//! both its versions it holds (`cairn_git::LineBudget`); whether a file is admitted is decided
//! before its blobs are read, so the most that is ever held is the budget and the one file
//! that crossed it, which R2.6's ceilings bound. Why this number, measured against the window
//! check: `docs/work/diff-engine/progress.md`, the phase 08 entry.

/// The lines Expand All reads before it stops.
pub const EXPAND_ALL_LINES: u64 = 50_000;
