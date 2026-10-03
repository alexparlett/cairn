//! What one file's diff turned out to be: text, or a state that stands in place of it.

use crate::{ChangedFile, DisplayOverlay, Oid, TextDiff};

/// The sizes past which a file is not diffed as text by default (R2.6, decision L10).
///
/// The line-length limit is Fork's, whose own is documented in characters; bytes is
/// Cairn's reading of it — identical for ASCII and stricter for anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffLimits {
    pub max_bytes: u64,
    pub max_lines: u32,
    pub max_line_bytes: u32,
    /// The ceiling on loading an over-limit file anyway. Past it, nothing is offered.
    pub load_anyway_bytes: u64,
}

impl DiffLimits {
    pub const MAX_BYTES: u64 = 1024 * 1024;
    pub const MAX_LINES: u32 = 50_000;
    pub const MAX_LINE_BYTES: u32 = 2_048;
    pub const LOAD_ANYWAY_BYTES: u64 = 64 * 1024 * 1024;
}

impl Default for DiffLimits {
    fn default() -> Self {
        Self {
            max_bytes: Self::MAX_BYTES,
            max_lines: Self::MAX_LINES,
            max_line_bytes: Self::MAX_LINE_BYTES,
            load_anyway_bytes: Self::LOAD_ANYWAY_BYTES,
        }
    }
}

/// Which limit a file crossed, and what it measured. The measurement is what a notice
/// quotes, so it is the reason the limit fired and not a recount of the whole file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeLimit {
    Bytes { limit: u64, measured: u64 },
    Lines { limit: u32, measured: u32 },
    LineLength { limit: u32, measured: u32 },
}

/// What stands where the rows would be (R1.2).
///
/// Every variant is a state a view has to draw something for (R6.8), so read one by
/// naming every variant: a wildcard arm compiles the day a ninth state lands and draws
/// nothing for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffContent {
    /// The file was diffed as text: the exact answer, and what only a view may see.
    Text {
        text: TextDiff,
        overlay: DisplayOverlay,
    },
    Binary {
        old_size: u64,
        new_size: u64,
    },
    TooLarge {
        crossed: SizeLimit,
        /// Whether asking for it anyway is offered — false past [`DiffLimits::load_anyway_bytes`].
        loadable: bool,
    },
    /// A Git LFS pointer file, which is text but stands for content that is not here.
    LfsPointer {
        old: Option<String>,
        new: Option<String>,
    },
    /// A submodule, which a tree records as the commit it points at: the commit on each
    /// side, `None` where the submodule did not exist. Named `*_target`, not `*_commit`:
    /// `new_commit` is a gitoxide write the mutation guard bans by name everywhere.
    Submodule {
        old_target: Option<Oid>,
        new_target: Option<Oid>,
        /// The new side is a working tree whose checkout of the submodule has changes of
        /// its own — what `git diff` prints as `-dirty` after the commit (R3.4). Always
        /// false where the new side is a commit or the index, which record no such thing.
        dirty: bool,
    },
    /// The content is the same on both sides; only the mode moved.
    ModeChangeOnly,
    Conflicted,
    Unsupported {
        reason: String,
    },
}

/// One file's diff: which path changed, and what its change turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub file: ChangedFile,
    pub content: DiffContent,
}

impl FileDiff {
    /// The exact answer, when there is one. This is what the patch emitter is handed.
    pub fn text(&self) -> Option<&TextDiff> {
        match &self.content {
            DiffContent::Text { text, .. } => Some(text),
            DiffContent::Binary { .. }
            | DiffContent::TooLarge { .. }
            | DiffContent::LfsPointer { .. }
            | DiffContent::Submodule { .. }
            | DiffContent::ModeChangeOnly
            | DiffContent::Conflicted
            | DiffContent::Unsupported { .. } => None,
        }
    }

    /// What only a view may see, when the file was diffed as text.
    pub fn overlay(&self) -> Option<&DisplayOverlay> {
        match &self.content {
            DiffContent::Text { overlay, .. } => Some(overlay),
            DiffContent::Binary { .. }
            | DiffContent::TooLarge { .. }
            | DiffContent::LfsPointer { .. }
            | DiffContent::Submodule { .. }
            | DiffContent::ModeChangeOnly
            | DiffContent::Conflicted
            | DiffContent::Unsupported { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangeStatus, DiffLine, FileMode, RepoPath};

    fn file() -> ChangedFile {
        ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("f.txt"),
            new_path: RepoPath::from("f.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: None,
            new_id: None,
        }
    }

    /// The numbers are R2.6's, and an engine reading a different ceiling would answer
    /// "too large" for files Fork shows.
    #[test]
    fn the_default_limits_are_the_ones_the_packet_committed_to() {
        let limits = DiffLimits::default();
        assert_eq!(limits.max_bytes, 1024 * 1024);
        assert_eq!(limits.max_lines, 50_000);
        assert_eq!(limits.max_line_bytes, 2_048);
        assert_eq!(limits.load_anyway_bytes, 64 * 1024 * 1024);
    }

    /// A submodule whose checkout has changes of its own, at the commit the index
    /// records, is a change `git diff` shows (`-dirty`), so it is a different answer
    /// from the same submodule clean — never folded into "no change". Caught by: the
    /// flag dropped from equality, or from the variant.
    #[test]
    fn a_dirty_submodule_is_a_different_answer_from_a_clean_one() {
        let at = Oid::parse("07da224c7ec04501dfb451be161fa962effe1dc1").unwrap();
        let clean = DiffContent::Submodule {
            old_target: Some(at),
            new_target: Some(at),
            dirty: false,
        };
        let dirty = DiffContent::Submodule {
            old_target: Some(at),
            new_target: Some(at),
            dirty: true,
        };
        assert_ne!(clean, dirty);
        let DiffContent::Submodule { dirty: flagged, .. } = dirty else {
            panic!("not a submodule: {dirty:?}");
        };
        assert!(flagged);
    }

    /// Caught by: answering with a text diff for a state that has none, which would draw
    /// an empty file where a notice belongs.
    #[test]
    fn only_a_text_diff_answers_with_lines() {
        let states = [
            DiffContent::Binary {
                old_size: 1,
                new_size: 2,
            },
            DiffContent::TooLarge {
                crossed: SizeLimit::Bytes {
                    limit: DiffLimits::MAX_BYTES,
                    measured: DiffLimits::MAX_BYTES + 1,
                },
                loadable: true,
            },
            DiffContent::LfsPointer {
                old: None,
                new: Some("version https://git-lfs.github.com/spec/v1".into()),
            },
            DiffContent::Submodule {
                old_target: None,
                new_target: None,
                dirty: false,
            },
            DiffContent::ModeChangeOnly,
            DiffContent::Conflicted,
            DiffContent::Unsupported {
                reason: "an external diff driver".into(),
            },
        ];
        for content in states {
            let diff = FileDiff {
                file: file(),
                content: content.clone(),
            };
            assert!(
                diff.text().is_none(),
                "{content:?} answered with a text diff"
            );
            assert!(
                diff.overlay().is_none(),
                "{content:?} answered with a display overlay"
            );
        }

        let diff = FileDiff {
            file: file(),
            content: DiffContent::Text {
                text: TextDiff::new(
                    vec![DiffLine::terminated("a")],
                    vec![DiffLine::terminated("a")],
                    Vec::new(),
                ),
                overlay: DisplayOverlay::none(),
            },
        };
        assert!(diff.text().is_some(), "a text diff answered with nothing");
        assert!(diff.overlay().is_some());
    }

    /// A notice quotes both numbers; a limit that forgot what it measured says nothing
    /// useful.
    #[test]
    fn a_size_limit_carries_the_ceiling_and_the_measurement() {
        let crossed = SizeLimit::Lines {
            limit: DiffLimits::MAX_LINES,
            measured: 50_001,
        };
        match crossed {
            SizeLimit::Lines { limit, measured } => {
                assert_eq!(limit, 50_000);
                assert!(measured > limit, "the measurement did not cross the limit");
            }
            SizeLimit::Bytes { .. } | SizeLimit::LineLength { .. } => {
                panic!("the line limit read back as another kind")
            }
        }
    }
}
