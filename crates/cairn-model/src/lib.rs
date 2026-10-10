//! Commit, ref, graph, lane, diff and status types shared by the engine and the UI, and
//! the record of a `git` invocation the engine's command log keeps.

mod action_patch;
mod askpass;
mod branch_name;
mod c_quote;
mod change_set;
mod changed_file;
mod chunked_store;
mod command_log;
mod commit_details;
mod confirm;
mod consequence;
mod diff_content;
mod diff_function_context;
mod diff_hunks;
mod diff_overlay;
mod diff_rows;
mod diff_shown;
mod diff_text;
mod edge_derivation;
mod graph;
mod history;
mod lane_assignment;
mod line_selection;
mod local_changes;
mod oid;
mod operation_in_progress;
mod patch;
mod patch_apply;
mod prompt;
mod refs;
mod remote;
mod repo_path;
mod row_labels;
mod row_selection;
mod rows_page;
mod scrub;
mod secret;
mod sidebar_rows;
mod status;
mod text_filter;

pub use action_patch::{PatchAction, action_patch};
pub use askpass::{AskpassToken, HELPER_PROGRAM, SOCKET_VARIABLE, TOKEN_VARIABLE};
pub use branch_name::BranchName;
pub use change_set::{ChangeSet, RenameDetection};
pub use changed_file::{ChangeStatus, ChangedFile, FileMode, Similarity};
pub use command_log::{CommandExit, CommandRecord};
pub use commit_details::{CommitDetails, Signature, Timestamp};
pub use confirm::Confirmed;
pub use consequence::{
    ChangeLoss, ChangedKind, Consequence, DiscardedFile, FileLoss, LostChange, Publication, Reflog,
    RemovedKind,
};
pub use diff_content::{DiffContent, DiffLimits, FileDiff, SizeLimit};
pub use diff_function_context::FunctionContext;
pub use diff_hunks::{Context, Hunk, HunkHeader, Hunks};
pub use diff_overlay::{ByteRange, DisplayOverlay, IntraLineHighlight};
pub use diff_rows::{
    ChangeStops, DrawnRanges, SideBySideLayout, SideBySideRow, SideBySideRows, UnifiedLayout,
    UnifiedRow, UnifiedRows,
};
pub use diff_shown::{LINE_CUT_BYTES, ShownDiff, TAB_STOP, drawn_bytes, widest_drawn_columns};
pub use diff_text::{ChangedRange, DiffLine, LineNumber, LineSpan, TextDiff, split_lines};
pub use edge_derivation::{LaidOutRow, LaidOutRows, LaneSnapshot, RowEdges, row_edges};
pub use graph::{EdgeKind, EdgeSegment, GraphRow, Lane, LaneChange};
pub use history::{History, HistoryFull, HistoryRow, RetainedBytes, RowContent, RowId};
pub use lane_assignment::LaneAssigner;
pub use line_selection::Selection;
pub use local_changes::{
    ChangeKind, ChangeList, LocalChange, LocalChanges, MatchedRows, PathState, StagedAgainst,
    path_order,
};
pub use oid::{Oid, OidHex, OidParseError};
pub use operation_in_progress::OperationInProgress;
pub use patch::{PATCH_CONTEXT, Patch, emit_patch};
pub use patch_apply::{PatchApplyError, apply_patch, apply_patch_in_reverse};
pub use prompt::{PromptKind, prompt_subject};
pub use refs::{
    AheadBehind, HeadState, Ref, RefKind, RefTarget, RefsMatched, RefsSnapshot, StashEntry,
    Upstream,
};
pub use remote::RemoteSummary;
pub use repo_path::RepoPath;
pub use row_labels::{Label, RowLabels};
pub use row_selection::SideColumn;
pub use rows_page::{PagedCommit, PagedStash, RowsPage};
pub use scrub::{ScrubbedLine, ScrubbedLines, Scrubber, strip_ansi};
pub use secret::Secret;
pub use sidebar_rows::{
    Disclosure, SidebarRow, SidebarSection, folder_name, folder_path, natural_order,
};
pub use status::{
    ChangedEntry, ConflictKind, ConflictedEntry, StagedChange, StatusEntry, SubmoduleState,
    UnreadableIndex, UnstagedChange, WorkingTreeStatus,
};

/// A commit as the history list draws it, read out of a [`History`]: no parents beyond
/// their count and no author address, which the list never draws — the details query
/// carries both (`CommitDetails`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub id: Oid,
    /// How many parents the commit names, saturating at `u16::MAX`: more than one draws
    /// its node as a merge's ring.
    pub parent_count: usize,
    pub summary: String,
    pub author_name: String,
    /// Seconds since the Unix epoch.
    pub author_time: i64,
}

/// A stash as the history list draws its row (PRD R4.2, R5.4), read out of a [`History`]:
/// the stash commit `stash@{index}` names, the commit it was made on — its first parent, and
/// the one line its row draws — and the stash list's message, its row's subject. Its index
/// and untracked commits are not rows, and are the details query's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashSummary {
    /// The stash commit itself.
    pub id: Oid,
    /// `0` is the newest: `stash@{0}`.
    pub index: usize,
    /// The commit the stash was made on.
    pub base: Oid,
    /// The stash list's message, `On main: wip` or `WIP on main: 1234567 subject`.
    pub message: String,
    pub author_name: String,
    /// Seconds since the Unix epoch.
    pub author_time: i64,
}

impl StashSummary {
    /// The stash commit as a commit's row describes one: the stash's message as its
    /// subject, and one parent — the one its row draws a line to — so it draws no merge's
    /// ring.
    pub fn as_commit(&self) -> CommitSummary {
        CommitSummary {
            id: self.id,
            parent_count: 1,
            summary: self.message.clone(),
            author_name: self.author_name.clone(),
            author_time: self.author_time,
        }
    }
}

/// A fully-qualified reference name, e.g. `refs/heads/main`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RefName(String);

impl RefName {
    pub fn new(full_name: impl Into<String>) -> Self {
        Self(full_name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `refs/heads/main` becomes `main`; an unknown namespace is left whole.
    pub fn shorthand(&self) -> &str {
        short_ref_name(&self.0)
    }
}

/// `full` past the namespace a branch, a remote-tracking ref or a tag lives in; any other
/// name whole.
fn short_ref_name(full: &str) -> &str {
    for prefix in ["refs/heads/", "refs/remotes/", "refs/tags/"] {
        if let Some(rest) = full.strip_prefix(prefix) {
            return rest;
        }
    }
    full
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorthand_strips_known_prefixes() {
        assert_eq!(RefName::new("refs/heads/main").shorthand(), "main");
        assert_eq!(
            RefName::new("refs/remotes/origin/main").shorthand(),
            "origin/main"
        );
        assert_eq!(RefName::new("refs/tags/v1.0").shorthand(), "v1.0");
    }

    #[test]
    fn shorthand_leaves_unknown_namespaces_intact() {
        assert_eq!(RefName::new("refs/stash").shorthand(), "refs/stash");
        assert_eq!(RefName::new("HEAD").shorthand(), "HEAD");
    }
}
