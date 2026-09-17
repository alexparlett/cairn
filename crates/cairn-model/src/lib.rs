//! Commit, ref, graph, lane and diff types shared by the engine and the UI.

mod askpass;
mod changed_file;
mod commit_details;
mod confirm;
mod diff_content;
mod diff_hunks;
mod diff_overlay;
mod diff_rows;
mod diff_text;
mod graph;
mod history;
mod lane_assignment;
mod line_selection;
mod oid;
mod patch;
mod patch_apply;
mod prompt;
mod remote;
mod repo_path;
mod secret;

pub use askpass::{AskpassToken, HELPER_PROGRAM, SOCKET_VARIABLE, TOKEN_VARIABLE};
pub use changed_file::{ChangeStatus, ChangedFile, FileMode, Similarity};
pub use commit_details::{CommitDetails, Signature, Timestamp};
pub use confirm::Confirmed;
pub use diff_content::{DiffContent, DiffLimits, FileDiff, SizeLimit};
pub use diff_hunks::{Context, Hunk, HunkHeader, Hunks};
pub use diff_overlay::{ByteRange, DisplayOverlay, IntraLineHighlight};
pub use diff_rows::{SideBySideRow, SideBySideRows, UnifiedRow, UnifiedRows};
pub use diff_text::{ChangedRange, DiffLine, LineNumber, LineSpan, TextDiff, split_lines};
pub use graph::{EdgeKind, EdgeSegment, GraphRow, Lane};
pub use history::{HistoryRow, RowContent, RowId};
pub use lane_assignment::LaneAssigner;
pub use line_selection::Selection;
pub use oid::{Oid, OidHex, OidParseError};
pub use patch::{PATCH_CONTEXT, Patch, emit_patch};
pub use patch_apply::{PatchApplyError, apply_patch, apply_patch_in_reverse};
pub use prompt::{PromptKind, prompt_subject};
pub use remote::RemoteSummary;
pub use repo_path::RepoPath;
pub use secret::Secret;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub id: Oid,
    pub parents: Vec<Oid>,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    /// Seconds since the Unix epoch.
    pub author_time: i64,
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
        for prefix in ["refs/heads/", "refs/remotes/", "refs/tags/"] {
            if let Some(rest) = self.0.strip_prefix(prefix) {
                return rest;
            }
        }
        &self.0
    }
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
