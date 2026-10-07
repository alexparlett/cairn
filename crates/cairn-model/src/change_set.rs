//! What a commit or a comparison changed, and how the rename search that paired its files
//! went — the answer to the changes query, as the window and the views receive it.

use crate::text_filter::{BETWEEN_CHECKS, Folded};
use crate::{ChangeStatus, ChangedFile, CommitDetails};

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

impl ChangeSet {
    /// The indices of the files whose path holds `text`, in order, or `None` when
    /// `keep_going` says to stop — asked before the first file and every few thousand after.
    /// An empty `text` matches every file.
    ///
    /// The Changes tab's filter (R5.4), as Cairn reads Fork's: the text anywhere in a file's
    /// path — a file name, an extension or a piece of a path, no wildcards (the vendor,
    /// TrackerWin #152, 22 Jun 2019; Tracker #1482, 7 Oct 2021) — and for a rename or a copy
    /// in either of its paths, ignoring case as Unicode reads it (`str::to_lowercase`, so `É`
    /// finds `é`) — the user's decision of 2026-10-03, since whether Fork ignores case is not
    /// established; a path that is not UTF-8 is read as git's lossy reading of it is. A pass
    /// over every path, so the application runs it on a worker.
    ///
    /// A known limit, accepted (the user's decision, 2026-10-03): `str::to_lowercase` is a
    /// mapping, not a case fold, so a character whose lowercase is longer than itself folds
    /// apart from what it is typed as — `İ` (U+0130) lowercases to `i̇`, an `i` and a combining
    /// dot, so "istanbul" does not find `İstanbul.txt` (`the_dotted_capital_i_is_a_known_limit`).
    pub fn files_matching(
        &self,
        text: &str,
        mut keep_going: impl FnMut() -> bool,
    ) -> Option<Vec<u32>> {
        let wanted = Folded::of(text);
        let mut matched = Vec::new();
        for (index, file) in self.files.iter().enumerate() {
            if index % BETWEEN_CHECKS == 0 && !keep_going() {
                return None;
            }
            let paired = match file.status {
                ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => true,
                ChangeStatus::Added
                | ChangeStatus::Deleted
                | ChangeStatus::Modified
                | ChangeStatus::TypeChanged => false,
            };
            if wanted.found_in(file.new_path.as_bytes())
                || (paired && wanted.found_in(file.old_path.as_bytes()))
            {
                matched.push(u32::try_from(index).unwrap_or(u32::MAX));
            }
        }
        Some(matched)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RepoPath, Similarity};

    fn file(path: &str) -> ChangedFile {
        ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from(path),
            new_path: RepoPath::from(path),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        }
    }

    fn set(files: Vec<ChangedFile>) -> ChangeSet {
        ChangeSet {
            files,
            details: None,
            renames: RenameDetection::default(),
        }
    }

    /// Fork's filter: a file name, an extension or a piece of a path, no wildcards, case
    /// ignored; a rename by either name; an empty filter keeps everything. Caught by: matching
    /// the name alone (a directory is a path expression), a case-sensitive match, or a rename
    /// found only by its new name.
    #[test]
    fn a_filter_keeps_the_files_whose_path_holds_its_text() {
        let mut renamed = file("src/new_name.rs");
        renamed.old_path = RepoPath::from("src/old_name.rs");
        renamed.status = ChangeStatus::Renamed(Similarity::from_percent(90));
        let changes = set(vec![
            file("src/lib.rs"),
            file("docs/Guide.md"),
            file("src/ui/view.tsx"),
            renamed,
        ]);
        let keep = || true;
        assert_eq!(changes.files_matching("", keep), Some(vec![0, 1, 2, 3]));
        assert_eq!(changes.files_matching(".rs", keep), Some(vec![0, 3]));
        assert_eq!(changes.files_matching("guide", keep), Some(vec![1]));
        assert_eq!(changes.files_matching("SRC/UI", keep), Some(vec![2]));
        assert_eq!(changes.files_matching("old_name", keep), Some(vec![3]));
        assert_eq!(changes.files_matching("*.rs", keep), Some(vec![]));
    }

    /// The user's decision (2026-10-03): case is ignored as Unicode reads it, not ASCII
    /// alone — `É` finds `é`, `ÉCOLE` finds `école`, `Σ` finds `σ` — and a path that is not
    /// UTF-8 is still searched. Caught by: an ASCII-only fold, which leaves `É` and `é` apart.
    #[test]
    fn case_is_ignored_as_unicode_reads_it() {
        let changes = set(vec![
            file("docs/école/Résumé.md"),
            file("src/ΣIGMA.rs"),
            file("plain/file.txt"),
            ChangedFile {
                new_path: RepoPath::new(b"bytes/\xffCaf\xc3\xa9.bin".to_vec()),
                ..file("unused")
            },
        ]);
        let keep = || true;
        assert_eq!(changes.files_matching("É", keep), Some(vec![0, 3]));
        assert_eq!(changes.files_matching("ÉCOLE/RÉSUMÉ", keep), Some(vec![0]));
        assert_eq!(changes.files_matching("σigma", keep), Some(vec![1]));
        assert_eq!(changes.files_matching("CAFÉ", keep), Some(vec![3]));
        assert_eq!(changes.files_matching("FILE.TXT", keep), Some(vec![2]));
    }

    /// A copy is found by its source's path as a rename is, though its destination holds
    /// none of the text. Caught by: a copy matched by its new path alone.
    #[test]
    fn a_copy_is_found_by_the_path_it_was_copied_from() {
        let mut copied = file("lib/vendored.rs");
        copied.old_path = RepoPath::from("src/original.rs");
        copied.status = ChangeStatus::Copied(Similarity::from_percent(100));
        let changes = set(vec![file("src/lib.rs"), copied]);
        let keep = || true;
        assert_eq!(changes.files_matching("original", keep), Some(vec![1]));
        assert_eq!(changes.files_matching("vendored", keep), Some(vec![1]));
        assert_eq!(changes.files_matching("src/", keep), Some(vec![0, 1]));
    }

    /// The accepted limit of folding case with `str::to_lowercase` (the user's decision,
    /// 2026-10-03), pinned so a change to the fold is seen: `İ` lowercases to `i` and a
    /// combining dot, so "istanbul" does not find `İstanbul.txt`, though typing the path's
    /// own capital does.
    #[test]
    fn the_dotted_capital_i_is_a_known_limit() {
        let changes = set(vec![file("docs/İstanbul.txt")]);
        let keep = || true;
        assert_eq!(changes.files_matching("istanbul", keep), Some(vec![]));
        assert_eq!(changes.files_matching("İstanbul", keep), Some(vec![0]));
    }

    /// A newer filter stops the one running: it is asked between files, at the start and
    /// every few thousand files after. Caught by: a match that runs to the end regardless.
    #[test]
    fn a_superseded_filter_stops_where_it_is() {
        let changes = set((0..10_000).map(|n| file(&format!("f{n}"))).collect());
        let mut asked = 0;
        assert_eq!(
            changes.files_matching("f", || {
                asked += 1;
                asked < 2
            }),
            None
        );
        assert_eq!(asked, 2);
    }

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
