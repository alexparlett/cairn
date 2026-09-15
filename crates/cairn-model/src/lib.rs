//! The vocabulary that crosses Cairn's engine/UI boundary.
//!
//! Everything here is plain data — plus, in `lane_assignment`, the pure
//! algorithm that computes some of it. No `gix` types, no Freya types, no I/O,
//! no clock. Both sides of the seam name these types, which is what lets the UI
//! stay ignorant of how a repository is read and the engine stay ignorant of
//! how it is drawn.

mod confirm;
mod graph;
mod history;
mod lane_assignment;
mod oid;

pub use confirm::Confirmed;
pub use graph::{EdgeKind, EdgeSegment, GraphRow, Lane};
pub use history::HistoryRow;
pub use lane_assignment::LaneAssigner;
pub use oid::{Oid, OidParseError};

/// A commit as a list needs it: enough to draw a row, never the full object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub id: Oid,
    pub parents: Vec<Oid>,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    /// Seconds since the Unix epoch, as recorded in the commit.
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

    /// The part a human reads: `refs/heads/main` renders as `main`.
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
