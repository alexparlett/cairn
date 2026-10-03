//! What a commit or a comparison changed, and how the rename search that paired its files
//! went — the answer to the changes query, as the window and the views receive it.

use crate::{ChangedFile, CommitDetails};

/// How rename and copy detection went, so a view can say when it was cut short (R2.2).
///
/// Detection is what the user's `diff.renames` asks for — off, renames, or renames and
/// copies — searched by git under `diff.renameLimit`, exactly as their own `git show` would.
/// Whether the limit cut the search short is decided by the engine from git's answer, never
/// from its stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenameDetection {
    /// False when `diff.renames` is off, and then every other field is empty.
    pub enabled: bool,
    /// Copies are detected only when `diff.renames` asks for them.
    pub copies: bool,
    /// The limit git applied: `diff.renameLimit`, or git's own default when it is not set.
    /// `None` when nothing limited the search.
    pub limit: Option<u32>,
    /// When the limit stopped git's exhaustive search: the limit that would have let it
    /// run, which is the number git's own warning asks the user to raise it to.
    pub needed_limit: Option<usize>,
}

impl RenameDetection {
    /// Whether `diff.renameLimit` stopped the search before it was exhaustive — the fact
    /// git prints as "exhaustive rename detection was skipped due to too many files". The
    /// answer then holds only the pairs git's cheap stages found, as git's own does.
    pub fn was_cut_short(self) -> bool {
        self.needed_limit.is_some()
    }
}

/// What a commit or a comparison changed (R2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    /// Sorted by path, a rename or a copy under its destination. The order is total, so
    /// two runs of the same query list the same files in the same places.
    pub files: Vec<ChangedFile>,
    /// Present when one commit was named, absent for a comparison of two (R2.1, R7.3).
    pub details: Option<CommitDetails>,
    pub renames: RenameDetection,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: deciding "cut short" from anything but the needed limit — the limit
    /// alone is set on every limited search, cut short or not.
    #[test]
    fn a_search_is_cut_short_exactly_when_it_names_the_limit_it_needed() {
        let limited = RenameDetection {
            enabled: true,
            copies: false,
            limit: Some(1000),
            needed_limit: None,
        };
        assert!(
            !limited.was_cut_short(),
            "a limit alone is not a cut: {limited:?}"
        );
        let cut = RenameDetection {
            needed_limit: Some(2774),
            ..limited
        };
        assert!(cut.was_cut_short(), "{cut:?}");
        assert!(!RenameDetection::default().was_cut_short());
    }
}
