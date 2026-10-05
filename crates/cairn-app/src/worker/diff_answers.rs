//! The commit and comparison answers the diff thread has already given (PRD R4.5), newest
//! last, each bounded, each with what its attributes were read from.
//!
//! Keyed by everything Cairn asks the answer with: a change set by its [`Comparison`] —
//! commit ids, so the trees cannot move under it — and a file's diff by its whole
//! [`FileQuery`]: the comparison, the [`cairn_model::ChangedFile`] (both paths, modes and
//! blob ids) and the options (context, whitespace, loading past the ceiling). What else an
//! answer was read from is the diff thread's to watch (`super::diff_freshness`): the
//! configuration and what every path reads let every answer go when they move, and each
//! answer carries the [`Dependence`] — the working tree's `.gitattributes` above its paths
//! — that a hit checks before it is used. Working-tree answers are never kept, and neither
//! is Expand All's.

use cairn_model::{ChangeSet, ChangeStatus, FileDiff, RepoPath};

use super::diff_freshness::Dependence;
use super::request::{Comparison, FileQuery, FileTarget};

#[derive(Debug, Default)]
pub(super) struct Answers {
    change_sets: Vec<(Comparison, ChangeSet, Dependence)>,
    file_diffs: Vec<(FileQuery, FileDiff, usize, Dependence)>,
}

/// How many change sets are kept, and how many of their files in all: a commit that
/// renames tens of thousands of files is kept alone, or not at all.
pub(super) const KEPT_CHANGE_SETS: usize = 16;
pub(super) const KEPT_CHANGED_FILES: usize = 100_000;
/// How many file diffs are kept, and roughly how many bytes in all ([`held_bytes`]); an
/// answer larger than the whole budget is not kept.
pub(super) const KEPT_FILE_DIFFS: usize = 64;
pub(super) const KEPT_DIFF_BYTES: usize = 32 * 1024 * 1024;

/// The paths whose attributes `git diff-tree` read to answer `changes`: with rename
/// detection on, every file a rename or copy search could pair — added, deleted, renamed
/// and copied, and modified ones too when copies are searched for — since an attribute
/// (`-diff`, `binary`) decides whether a file is scored as text; with it off, none, since
/// a listing reads no content.
pub(super) fn searched_paths(changes: &ChangeSet) -> impl Iterator<Item = &RepoPath> {
    let detection = changes.renames.enabled;
    let copies = changes.renames.copies;
    changes
        .files
        .iter()
        .filter(move |file| {
            detection
                && match file.status {
                    ChangeStatus::Added
                    | ChangeStatus::Deleted
                    | ChangeStatus::Renamed(_)
                    | ChangeStatus::Copied(_) => true,
                    ChangeStatus::Modified => copies,
                    ChangeStatus::TypeChanged => false,
                }
        })
        .flat_map(|file| [&file.old_path, &file.new_path])
}

impl Answers {
    /// `of`'s change set, if one is kept and `still` says what it read is as it was; one
    /// that is not is let go.
    pub(super) fn change_set(
        &mut self,
        of: Comparison,
        still: impl FnOnce(&Dependence) -> bool,
    ) -> Option<&ChangeSet> {
        let at = self
            .change_sets
            .iter()
            .position(|(kept, _, _)| *kept == of)?;
        if !self
            .change_sets
            .get(at)
            .is_some_and(|(_, _, read)| still(read))
        {
            self.change_sets.remove(at);
            return None;
        }
        self.change_sets.get(at).map(|(_, changes, _)| changes)
    }

    pub(super) fn keep_change_set(&mut self, of: Comparison, changes: ChangeSet, read: Dependence) {
        self.change_sets.retain(|(kept, _, _)| *kept != of);
        if changes.files.len() > KEPT_CHANGED_FILES {
            return;
        }
        self.change_sets.push((of, changes, read));
        while self.change_sets.len() > KEPT_CHANGE_SETS
            || self
                .change_sets
                .iter()
                .map(|(_, c, _)| c.files.len())
                .sum::<usize>()
                > KEPT_CHANGED_FILES
        {
            self.change_sets.remove(0);
        }
    }

    /// `asked`'s diff, if one is kept and `still` says what it read is as it was; one that
    /// is not is let go.
    pub(super) fn file_diff(
        &mut self,
        asked: &FileQuery,
        still: impl FnOnce(&Dependence) -> bool,
    ) -> Option<&FileDiff> {
        let at = self
            .file_diffs
            .iter()
            .position(|(kept, _, _, _)| kept == asked)?;
        if !self
            .file_diffs
            .get(at)
            .is_some_and(|(_, _, _, read)| still(read))
        {
            self.file_diffs.remove(at);
            return None;
        }
        self.file_diffs.get(at).map(|(_, diff, _, _)| diff)
    }

    pub(super) fn keep_file_diff(&mut self, asked: FileQuery, diff: FileDiff, read: Dependence) {
        if !matches!(asked.target, FileTarget::Committed { .. }) {
            return;
        }
        self.file_diffs.retain(|(kept, _, _, _)| *kept != asked);
        let bytes = held_bytes(&diff) + read.held_bytes();
        if bytes > KEPT_DIFF_BYTES {
            return;
        }
        self.file_diffs.push((asked, diff, bytes, read));
        while self.file_diffs.len() > KEPT_FILE_DIFFS
            || self.file_diffs.iter().map(|(_, _, b, _)| b).sum::<usize>() > KEPT_DIFF_BYTES
        {
            self.file_diffs.remove(0);
        }
    }

    #[cfg(test)]
    pub(super) fn counts(&self) -> (usize, usize) {
        (self.change_sets.len(), self.file_diffs.len())
    }
}

/// The longest function context git prints after a hunk header's `@@`: xdiff keeps 80
/// bytes of the line (`xemit.c`), and the overlay holds no more than git printed.
const FUNCTION_CONTEXT_BYTES: usize = 80;

/// What a file's diff holds: its lines' bytes and each line's own size, its changed ranges,
/// and the overlay's — the whitespace-ignoring ranges, each intra-line highlight with its
/// byte ranges, and the function context, counted at the most git prints per hunk, since
/// the overlay keeps its text private. Exact but for that last, an upper bound.
pub(super) fn held_bytes(diff: &FileDiff) -> usize {
    use std::mem::{size_of, size_of_val};
    let line = size_of::<cairn_model::DiffLine>();
    let lines_and_ranges = diff.text().map_or(0, |text| {
        text.old_lines()
            .iter()
            .chain(text.new_lines())
            .map(|l| l.bytes().len() + line)
            .sum::<usize>()
            + size_of_val(text.changes())
    });
    let overlay = match &diff.content {
        cairn_model::DiffContent::Text { overlay, .. } => {
            overlay.changes_ignoring_whitespace().map_or(0, size_of_val)
                + overlay
                    .highlights()
                    .iter()
                    .map(|highlight| {
                        size_of::<cairn_model::IntraLineHighlight>()
                            + (highlight.on_removed.len() + highlight.on_added.len())
                                * size_of::<cairn_model::ByteRange>()
                    })
                    .sum::<usize>()
                + overlay.function_context().len()
                    * (size_of::<(cairn_model::LineNumber, Vec<u8>)>() + FUNCTION_CONTEXT_BYTES)
        }
        cairn_model::DiffContent::Binary { .. }
        | cairn_model::DiffContent::TooLarge { .. }
        | cairn_model::DiffContent::LfsPointer { .. }
        | cairn_model::DiffContent::Submodule { .. }
        | cairn_model::DiffContent::ModeChangeOnly
        | cairn_model::DiffContent::Conflicted
        | cairn_model::DiffContent::Unsupported { .. } => 0,
    };
    lines_and_ranges + overlay + size_of::<FileDiff>()
}

#[cfg(test)]
pub(super) mod tests {
    use cairn_model::{
        ByteRange, ChangedFile, ChangedRange, Context, DiffContent, DiffLine, DisplayOverlay,
        IntraLineHighlight, LineNumber, LineSpan, Oid, RenameDetection, TextDiff,
    };

    use super::*;
    use crate::worker::request::{DiffOptions, WorkingSide};

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    fn file(n: u8) -> ChangedFile {
        ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("a.txt"),
            new_path: RepoPath::from("a.txt"),
            old_mode: Some(cairn_model::FileMode::Regular),
            new_mode: Some(cairn_model::FileMode::Regular),
            old_id: Some(oid(n)),
            new_id: Some(oid(n.wrapping_add(1))),
        }
    }

    pub(in crate::worker) fn committed(n: u8, options: DiffOptions) -> FileQuery {
        FileQuery {
            target: FileTarget::Committed {
                of: Comparison::Commit(oid(n)),
                file: file(n),
            },
            options,
        }
    }

    pub(in crate::worker) fn text_diff(n: u8, bytes: usize) -> FileDiff {
        FileDiff {
            file: file(n),
            content: DiffContent::Text {
                text: TextDiff::new(
                    Vec::new(),
                    vec![DiffLine::terminated(vec![b'x'; bytes])],
                    vec![ChangedRange::new(LineSpan::at(0, 0), LineSpan::at(0, 1))],
                ),
                overlay: DisplayOverlay::default(),
            },
        }
    }

    fn changes(files: usize) -> ChangeSet {
        ChangeSet {
            files: (0..files).map(|_| file(1)).collect(),
            details: None,
            renames: RenameDetection::default(),
        }
    }

    fn always(_: &Dependence) -> bool {
        true
    }

    /// R4.5: a kept answer is found only under everything it was asked with. Caught by: a
    /// key that leaves out the context or the whitespace option (the view would draw an
    /// answer at another context, with function context git printed for other hunks), the
    /// file (another file's lines), or the comparison.
    #[test]
    fn a_kept_answer_is_found_only_under_everything_it_was_asked_with() {
        let mut answers = Answers::default();
        let asked = committed(1, DiffOptions::default());
        answers.keep_file_diff(asked.clone(), text_diff(1, 4), Dependence::default());
        assert_eq!(answers.file_diff(&asked, always), Some(&text_diff(1, 4)));

        let mut other_context = asked.clone();
        other_context.options.context = Context::lines(10);
        let mut whitespace = asked.clone();
        whitespace.options.ignore_whitespace = true;
        let mut anyway = asked.clone();
        anyway.options.load_anyway = true;
        let mut other_file = asked.clone();
        let mut other_old_side = asked.clone();
        let mut other_commit = asked.clone();
        if let FileTarget::Committed { file: f, .. } = &mut other_file.target {
            f.new_id = Some(oid(99));
        }
        // T9: the same new side against another old one is another diff.
        if let FileTarget::Committed { file: f, .. } = &mut other_old_side.target {
            f.old_id = Some(oid(98));
        }
        if let FileTarget::Committed { of, .. } = &mut other_commit.target {
            *of = Comparison::Between {
                old: oid(1),
                new: oid(2),
            };
        }
        for missed in [
            other_context,
            whitespace,
            anyway,
            other_file,
            other_old_side,
            other_commit,
        ] {
            assert_eq!(answers.file_diff(&missed, always), None, "{missed:?}");
        }
    }

    /// A kept answer whose attributes moved is let go on the hit that finds it so, and a
    /// change set's likewise. Caught by: answering a hit without asking whether what it
    /// read is as it was (an unstaged `-diff` would never reach a file already viewed).
    #[test]
    fn a_kept_answer_whose_attributes_moved_is_let_go() {
        let mut answers = Answers::default();
        let asked = committed(1, DiffOptions::default());
        answers.keep_file_diff(asked.clone(), text_diff(1, 4), Dependence::default());
        assert_eq!(answers.file_diff(&asked, |_| false), None);
        assert_eq!(answers.file_diff(&asked, always), None, "it was not let go");

        let of = Comparison::Commit(oid(1));
        answers.keep_change_set(of, changes(1), Dependence::default());
        assert!(answers.change_set(of, |_| false).is_none());
        assert!(
            answers.change_set(of, always).is_none(),
            "it was not let go"
        );
    }

    /// R4.5: working-tree answers are never kept. Caught by: keeping one, which would draw
    /// the file as it was when first asked.
    #[test]
    fn a_working_tree_answer_is_never_kept() {
        let mut answers = Answers::default();
        let asked = FileQuery {
            target: FileTarget::WorkingTree {
                path: RepoPath::from("a.txt"),
                side: WorkingSide::Unstaged,
            },
            options: DiffOptions::default(),
        };
        answers.keep_file_diff(asked.clone(), text_diff(1, 4), Dependence::default());
        assert_eq!(answers.file_diff(&asked, always), None);
        assert_eq!(answers.counts(), (0, 0));
    }

    /// Caught by: an unbounded cache, which holds every diff ever viewed for the life of
    /// the window; and (T9) a budget that refuses an answer exactly at it, or keeps one a
    /// byte past it.
    #[test]
    fn what_is_kept_is_bounded_by_count_and_by_size() {
        let mut answers = Answers::default();
        for n in 0..(KEPT_FILE_DIFFS as u8 + 8) {
            answers.keep_file_diff(
                committed(n, DiffOptions::default()),
                text_diff(n, 1),
                Dependence::default(),
            );
        }
        assert_eq!(answers.counts().1, KEPT_FILE_DIFFS);
        assert_eq!(
            answers.file_diff(&committed(0, DiffOptions::default()), always),
            None,
            "the oldest was not the one let go"
        );

        // Exactly at the budget is kept; a byte over is not.
        let overhead = held_bytes(&text_diff(1, 0));
        let mut answers = Answers::default();
        answers.keep_file_diff(
            committed(1, DiffOptions::default()),
            text_diff(1, KEPT_DIFF_BYTES - overhead),
            Dependence::default(),
        );
        assert_eq!(
            held_bytes(&text_diff(1, KEPT_DIFF_BYTES - overhead)),
            KEPT_DIFF_BYTES
        );
        assert_eq!(
            answers.counts().1,
            1,
            "an answer exactly at the budget was refused"
        );
        let mut answers = Answers::default();
        answers.keep_file_diff(
            committed(1, DiffOptions::default()),
            text_diff(1, KEPT_DIFF_BYTES - overhead + 1),
            Dependence::default(),
        );
        assert_eq!(answers.counts().1, 0, "an answer past the budget was kept");

        let mut answers = Answers::default();
        answers.keep_file_diff(
            committed(1, DiffOptions::default()),
            text_diff(1, KEPT_DIFF_BYTES / 2),
            Dependence::default(),
        );
        answers.keep_file_diff(
            committed(2, DiffOptions::default()),
            text_diff(2, KEPT_DIFF_BYTES / 2),
            Dependence::default(),
        );
        assert_eq!(answers.counts().1, 1, "the budget was not enforced");

        let mut answers = Answers::default();
        for n in 0..(KEPT_CHANGE_SETS as u8 + 2) {
            answers.keep_change_set(
                Comparison::Commit(oid(n)),
                changes(1),
                Dependence::default(),
            );
        }
        assert_eq!(answers.counts().0, KEPT_CHANGE_SETS);
        answers.keep_change_set(
            Comparison::Commit(oid(200)),
            changes(KEPT_CHANGED_FILES),
            Dependence::default(),
        );
        assert_eq!(answers.counts().0, 1, "the file budget was not enforced");
        answers.keep_change_set(
            Comparison::Commit(oid(201)),
            changes(KEPT_CHANGED_FILES + 1),
            Dependence::default(),
        );
        assert!(
            answers
                .change_set(Comparison::Commit(oid(201)), always)
                .is_none()
        );
    }

    /// R5: the budget counts what an answer holds besides its lines — its changed ranges
    /// and its overlay's. Caught by: counting the lines alone, under which a file of many
    /// small changes and long highlights is kept at a fraction of its size.
    #[test]
    fn the_budget_counts_ranges_and_the_overlay_too() {
        let plain = text_diff(1, 4);
        let ranges = 40usize;
        let lines: Vec<DiffLine> = (0..ranges)
            .map(|_| DiffLine::terminated(b"x".to_vec()))
            .collect();
        let changed: Vec<ChangedRange> = (0..ranges as u32)
            .map(|at| ChangedRange::new(LineSpan::at(at, 1), LineSpan::at(at, 1)))
            .collect();
        let highlights: Vec<IntraLineHighlight> = (0..ranges as u32)
            .map(|at| IntraLineHighlight {
                removed_line: LineNumber::from_index(at),
                added_line: LineNumber::from_index(at),
                on_removed: vec![ByteRange::new(0, 1)],
                on_added: vec![ByteRange::new(0, 1)],
            })
            .collect();
        let busy = FileDiff {
            file: file(1),
            content: DiffContent::Text {
                text: TextDiff::new(lines.clone(), lines, changed),
                overlay: DisplayOverlay::new(None, highlights),
            },
        };
        let lines_only = 2 * ranges * (1 + std::mem::size_of::<DiffLine>());
        let floor = lines_only
            + ranges * std::mem::size_of::<ChangedRange>()
            + ranges * std::mem::size_of::<IntraLineHighlight>();
        assert!(
            held_bytes(&busy) >= floor + std::mem::size_of::<FileDiff>(),
            "{} counts less than the ranges and highlights it holds ({floor})",
            held_bytes(&busy)
        );
        assert!(held_bytes(&plain) < held_bytes(&busy));
    }
}
