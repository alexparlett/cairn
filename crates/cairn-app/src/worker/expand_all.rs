//! Expand All's line budget (PRD R5.3, packet question Q2).
//!
//! Expand All opens a change set's files in its order until this many lines are spent, then
//! leaves the rest collapsed and says how many. A file costs one, for itself, and every line of
//! both its versions it holds (`cairn_git::LineBudget`); whether a file is admitted is decided
//! before its blobs are read, so the most that is ever held is the budget and the one file
//! that crossed it, which R2.6's ceilings bound.
//!
//! **Why fifty thousand** (measured in phase 08, `docs/work/diff-engine/progress.md`): it is
//! R2.6's own line ceiling for one file drawn without Load Diff, so Expand All never holds more
//! than about two of the largest files the diff draws unasked; on every subject of the bar it
//! is read to its end in at most 23 ms warm (S7, the edit-heavy commit), where 100,000 lines
//! took up to 86 ms (M1) and 200,000 up to 251 ms; and the window check found no frame near
//! 16.7 ms while it read and drew, on the heaviest subjects.

/// The lines Expand All reads before it stops: R2.6's line ceiling for one file.
pub const EXPAND_ALL_LINES: u64 = 50_000;

// At least a page of the engine's, so the first page Expand All reads is cut short by the page,
// never by the budget.
const _: () = assert!(EXPAND_ALL_LINES >= cairn_git::PAGE_LINES);

#[cfg(test)]
mod tests {
    use cairn_model::DiffLimits;

    use super::*;

    /// Q2: the budget is the number chosen and recorded, tied to the ceiling it was chosen
    /// beside — R2.6's line limit for one file, which the user's later change to either moves
    /// only together with this decision. Caught by: the constant edited without the record,
    /// or R2.6's limit moved under it.
    #[test]
    fn the_expand_all_budget_is_one_files_line_ceiling() {
        assert_eq!(EXPAND_ALL_LINES, 50_000);
        assert_eq!(EXPAND_ALL_LINES, u64::from(DiffLimits::MAX_LINES));
    }
}
