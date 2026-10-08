//! What staging, unstaging or discarding a selection of one file's diff emits (R2.2,
//! R2.3, L17).
//!
//! One rule writes every patch — [`crate::emit_patch`], forward — and what differs between
//! the three actions is only which diff it is given:
//!
//! - **stage**: the unstaged diff (index to working tree, or nothing to an untracked
//!   file), as it is, applied with `git apply --cached`;
//! - **unstage**: the staged diff (`HEAD` to index) inverted, applied with
//!   `git apply --cached`;
//! - **discard**: the unstaged diff inverted, applied with `git apply` to the working tree.
//!
//! So no patch is ever applied with `-R` (L17a). The inversion is [`TextDiff::inverted`],
//! [`Selection::inverted`] and [`ChangedFile::inverted`]; that what it emits is what git's
//! own mirrored patch applied in reverse gives is `cairn-git`'s C3, against real git.
//!
//! What is whole-file only gets no patch for part of it — the empty patch, which is also
//! what a selection of nothing makes (R2.3, L17c): a deletion on either side (a staged
//! deletion, or a file gone from the working tree), a type change, and every state that is
//! not text — binary, an LFS pointer, past the size limits, a submodule, a conflicted path,
//! a state Cairn cannot read. Each is staged, unstaged or discarded whole by its file verb,
//! never by a patch (and a submodule or a conflicted path is never discarded at all). Part
//! of a file ADDED is not whole-file only: an untracked or newly added file's lines are
//! staged as a `new file mode` patch of the lines selected, and discarded or unstaged as a
//! partial deletion, which the forward rule writes as a modification (L17b).
//!
//! Unstaging or discarding lines of a rename or a copy changes content only, at the path it
//! now has (R2.6, L17c): the patch names the new path on both sides and carries no `rename
//! from`, so the index keeps the rename and the lines go back. Undoing the rename itself is
//! `git reset -q -- <old> <new>`, a file verb.

use crate::{ChangeStatus, ChangedFile, DiffContent, FileDiff, Patch, Selection, TextDiff};

/// Which of the three things a selection is to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatchAction {
    /// Into the index, from the unstaged or untracked diff.
    Stage,
    /// Out of the index, from the staged diff.
    Unstage,
    /// Out of the working tree, from the unstaged or untracked diff. Destructive: the
    /// operation that applies it is sealed behind a confirmation (R1.5).
    Discard,
}

/// The patch for `action` over `diff` — the diff the selection was made on, as drawn: the
/// staged diff for [`PatchAction::Unstage`], the unstaged or untracked one otherwise — and
/// `selection`, its lines named on their own sides and the mode change apart.
///
/// Empty when there is nothing to do: nothing selected, or part of a change that is whole
/// only (module docs). Pure; costs what the file has lines, since an inversion copies them.
pub fn action_patch(action: PatchAction, diff: &FileDiff, selection: &Selection) -> Patch {
    let no_lines;
    let text = match &diff.content {
        DiffContent::Text { text, .. } => text,
        // The change is the mode alone, so the headers are all a patch can carry.
        DiffContent::ModeChangeOnly => {
            no_lines = TextDiff::new(Vec::new(), Vec::new(), Vec::new());
            &no_lines
        }
        DiffContent::Binary { .. }
        | DiffContent::TooLarge { .. }
        | DiffContent::LfsPointer { .. }
        | DiffContent::Submodule { .. }
        | DiffContent::Conflicted
        | DiffContent::Unsupported { .. } => return Patch::empty(),
    };
    let file = &diff.file;
    if whole_only(file) && !selection.holds_every_change(text) {
        return Patch::empty();
    }
    match action {
        PatchAction::Stage => crate::emit_patch(file, text, selection),
        PatchAction::Unstage | PatchAction::Discard => crate::emit_patch(
            &content_only(file).inverted(),
            &text.inverted(),
            &selection.inverted(),
        ),
    }
}

/// A change only its file verb takes, whole: a deletion, or a path that changed kind.
fn whole_only(file: &ChangedFile) -> bool {
    match file.status {
        ChangeStatus::Deleted | ChangeStatus::TypeChanged => true,
        ChangeStatus::Added
        | ChangeStatus::Modified
        | ChangeStatus::Renamed(_)
        | ChangeStatus::Copied(_) => false,
    }
}

/// A rename or a copy as a change of content at the path it now has; anything else as it
/// is. Undoing lines inside a rename moves lines, never the path.
fn content_only(file: &ChangedFile) -> ChangedFile {
    match file.status {
        ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => ChangedFile {
            status: ChangeStatus::Modified,
            old_path: file.new_path.clone(),
            ..file.clone()
        },
        ChangeStatus::Added
        | ChangeStatus::Deleted
        | ChangeStatus::Modified
        | ChangeStatus::TypeChanged => file.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChangedRange, DisplayOverlay, FileMode, LineNumber, LineSpan, Oid, RepoPath, Similarity,
        apply_patch, split_lines,
    };

    fn text(old: &[u8], new: &[u8], changes: Vec<((u32, u32), (u32, u32))>) -> TextDiff {
        TextDiff::new(
            split_lines(old),
            split_lines(new),
            changes
                .into_iter()
                .map(|(removed, added)| {
                    ChangedRange::new(
                        LineSpan::at(removed.0, removed.1),
                        LineSpan::at(added.0, added.1),
                    )
                })
                .collect(),
        )
    }

    fn diff(status: ChangeStatus, text: TextDiff) -> FileDiff {
        let (old_mode, new_mode) = match status {
            ChangeStatus::Added => (None, Some(FileMode::Regular)),
            ChangeStatus::Deleted => (Some(FileMode::Regular), None),
            ChangeStatus::TypeChanged => (Some(FileMode::Regular), Some(FileMode::Symlink)),
            ChangeStatus::Modified | ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => {
                (Some(FileMode::Regular), Some(FileMode::Regular))
            }
        };
        FileDiff {
            file: ChangedFile {
                status,
                old_path: RepoPath::from("f"),
                new_path: RepoPath::from("f"),
                old_mode,
                new_mode,
                old_id: None,
                new_id: None,
            },
            content: DiffContent::Text {
                text,
                overlay: DisplayOverlay::default(),
            },
        }
    }

    fn content(diff: &FileDiff) -> &TextDiff {
        match &diff.content {
            DiffContent::Text { text, .. } => text,
            other => panic!("not text: {other:?}"),
        }
    }

    /// Stage is the forward rule over the diff as drawn, nothing more.
    #[test]
    fn staging_is_the_forward_patch_of_the_diff_as_drawn() {
        let drawn = diff(
            ChangeStatus::Modified,
            text(
                b"a\nb\nc\n",
                b"a\nB\nc\nd\n",
                vec![((1, 1), (1, 1)), ((3, 0), (3, 1))],
            ),
        );
        let mut selection = Selection::empty();
        selection.select_added(LineNumber::from_index(3));
        assert_eq!(
            action_patch(PatchAction::Stage, &drawn, &selection),
            crate::emit_patch(&drawn.file, content(&drawn), &selection)
        );
    }

    /// Unstage and discard undo exactly what was selected, applied to the diff's NEW side,
    /// and leave what stays after what comes back — the order git's mirrored patch leaves.
    /// Caught by: a patch made of the original diff rather than its inversion (it does not
    /// apply to the new side), or the replacement inverted as one change (`B b`, not `b B`).
    #[test]
    fn unstaging_and_discarding_undo_the_selection_on_the_new_side() {
        let drawn = diff(
            ChangeStatus::Modified,
            text(b"a\nb\nc\n", b"a\nB\nc\n", vec![((1, 1), (1, 1))]),
        );
        for action in [PatchAction::Unstage, PatchAction::Discard] {
            // The removal of `b` undone alone: `b` comes back, `B` stays after it.
            let mut removal = Selection::empty();
            removal.select_removed(LineNumber::from_index(1));
            let patch = action_patch(action, &drawn, &removal);
            assert_eq!(
                apply_patch(&content(&drawn).new_content(), patch.as_bytes()),
                Ok(b"a\nb\nB\nc\n".to_vec()),
                "{action:?}\n{patch:?}"
            );

            // The addition of `B` undone alone: `B` goes, and `b` stays gone.
            let mut addition = Selection::empty();
            addition.select_added(LineNumber::from_index(1));
            let patch = action_patch(action, &drawn, &addition);
            assert_eq!(
                apply_patch(&content(&drawn).new_content(), patch.as_bytes()),
                Ok(b"a\nc\n".to_vec()),
                "{action:?}\n{patch:?}"
            );

            // Both: the old side again.
            let every = Selection::with_every_change(content(&drawn));
            let patch = action_patch(action, &drawn, &every);
            assert_eq!(
                apply_patch(&content(&drawn).new_content(), patch.as_bytes()),
                Ok(content(&drawn).old_content()),
                "{action:?}\n{patch:?}"
            );
        }
    }

    /// R2.3: part of a deletion or of a type change makes no patch, on either side, by any
    /// action; the whole of one does. Caught by: a partial deletion staged as a smaller file
    /// (`git add -p`'s rule is the deletion whole), or the check made on the inverted file,
    /// which would refuse part of an untracked file's discard instead.
    #[test]
    fn part_of_a_deletion_or_a_type_change_makes_no_patch() {
        let deleted = diff(
            ChangeStatus::Deleted,
            text(b"one\ntwo\n", b"", vec![((0, 2), (0, 0))]),
        );
        let typed = diff(
            ChangeStatus::TypeChanged,
            text(b"one\ntwo\n", b"target", vec![((0, 2), (0, 1))]),
        );
        for drawn in [&deleted, &typed] {
            let mut part = Selection::empty();
            part.select_removed(LineNumber::from_index(0));
            for action in [
                PatchAction::Stage,
                PatchAction::Unstage,
                PatchAction::Discard,
            ] {
                assert!(
                    action_patch(action, drawn, &part).is_empty(),
                    "{action:?} emitted part of a {:?}",
                    drawn.file.status
                );
                assert!(
                    !action_patch(action, drawn, &Selection::with_every_change(content(drawn)))
                        .is_empty(),
                    "{action:?} emitted nothing for the whole of a {:?}",
                    drawn.file.status
                );
            }
        }
    }

    /// R2.3, L17b: part of an added file is staged as a new file of the lines selected, and
    /// discarded (or unstaged) as a partial deletion written as a modification — the file
    /// stays. Caught by: the whole-file rule applied to the inverted file, which is a
    /// deletion.
    #[test]
    fn part_of_an_added_file_is_staged_new_and_discarded_as_a_modification() {
        let added = diff(
            ChangeStatus::Added,
            text(b"", b"one\ntwo\nthree\n", vec![((0, 0), (0, 3))]),
        );
        let mut part = Selection::empty();
        part.select_added(LineNumber::from_index(1));

        let staged = action_patch(PatchAction::Stage, &added, &part);
        assert!(
            staged.text().contains("new file mode 100644\n"),
            "{staged:?}"
        );
        assert_eq!(apply_patch(b"", staged.as_bytes()), Ok(b"two\n".to_vec()));

        for action in [PatchAction::Discard, PatchAction::Unstage] {
            let patch = action_patch(action, &added, &part);
            assert!(
                !patch.text().contains("deleted file mode"),
                "{action:?} {patch:?}"
            );
            assert!(!patch.text().contains("/dev/null"), "{action:?} {patch:?}");
            assert_eq!(
                apply_patch(b"one\ntwo\nthree\n", patch.as_bytes()),
                Ok(b"one\nthree\n".to_vec()),
                "{action:?}"
            );
        }
        // The whole of it undone is the file deleted.
        let whole = action_patch(
            PatchAction::Discard,
            &added,
            &Selection::with_every_change(content(&added)),
        );
        assert!(
            whole.text().contains("deleted file mode 100644\n"),
            "{whole:?}"
        );
    }

    /// R2.6, L17c: lines of a staged rename or copy are unstaged as content at the path the
    /// file has now. Caught by: the inverted rename emitted as a rename back, which would
    /// move the file when only lines were asked for. (The restored line is written ahead of
    /// the one it replaces: an inverted replacement is its insertion, then its removal —
    /// `TextDiff::inverted` — which `git apply` applies the same.)
    #[test]
    fn unstaging_lines_of_a_rename_or_a_copy_changes_content_at_its_new_path() {
        for status in [
            ChangeStatus::Renamed(Similarity::from_percent(80)),
            ChangeStatus::Copied(Similarity::from_percent(80)),
        ] {
            let mut drawn = diff(
                status,
                text(b"a\nb\nc\n", b"a\nB\nc\n", vec![((1, 1), (1, 1))]),
            );
            drawn.file.old_path = RepoPath::from("old.txt");
            drawn.file.new_path = RepoPath::from("new.txt");
            drawn.file.old_id = Oid::parse("587be6b4c3f93f93c489c0111bba5596147a26cb").ok();
            drawn.file.new_id = Oid::parse("b77b4eb4c3f93f93c489c0111bba5596147a26cb").ok();
            let patch = action_patch(
                PatchAction::Unstage,
                &drawn,
                &Selection::with_every_change(content(&drawn)),
            );
            assert_eq!(
                patch.text(),
                "diff --git a/new.txt b/new.txt\n\
                 index b77b4eb..587be6b 100644\n\
                 --- a/new.txt\n\
                 +++ b/new.txt\n\
                 @@ -1,3 +1,3 @@\n\
                 \x20a\n\
                 +b\n\
                 -B\n\
                 \x20c\n",
                "{status:?}"
            );
        }
    }

    /// R2.4 through an action: the mode alone undoes the mode alone, and the lines leave it.
    #[test]
    fn the_mode_change_is_unstaged_apart_from_the_lines() {
        let mut drawn = diff(
            ChangeStatus::Modified,
            text(b"a\n", b"A\n", vec![((0, 1), (0, 1))]),
        );
        drawn.file.new_mode = Some(FileMode::Executable);
        let mut mode = Selection::empty();
        mode.select_mode();
        assert_eq!(
            action_patch(PatchAction::Unstage, &drawn, &mode).text(),
            "diff --git a/f b/f\nold mode 100755\nnew mode 100644\n"
        );
        let lines = action_patch(
            PatchAction::Unstage,
            &drawn,
            &Selection::with_every_change(content(&drawn)),
        );
        assert!(!lines.text().contains("mode"), "{lines:?}");

        let only_mode = FileDiff {
            file: drawn.file.clone(),
            content: DiffContent::ModeChangeOnly,
        };
        assert_eq!(
            action_patch(PatchAction::Stage, &only_mode, &mode).text(),
            "diff --git a/f b/f\nold mode 100644\nnew mode 100755\n"
        );
        assert!(action_patch(PatchAction::Stage, &only_mode, &Selection::empty()).is_empty());
    }

    /// Every state that is not text is whole-file only, by every action.
    #[test]
    fn a_state_that_is_not_text_makes_no_patch() {
        let file = diff(
            ChangeStatus::Modified,
            TextDiff::new(Vec::new(), Vec::new(), Vec::new()),
        )
        .file;
        let mut everything = Selection::empty();
        everything.select_mode();
        everything.select_added(LineNumber::from_index(0));
        for content in [
            DiffContent::Binary {
                old_size: 1,
                new_size: 2,
            },
            DiffContent::TooLarge {
                crossed: crate::SizeLimit::Lines {
                    limit: 1,
                    measured: 2,
                },
                loadable: true,
            },
            DiffContent::LfsPointer {
                old: None,
                new: None,
            },
            DiffContent::Submodule {
                old_target: None,
                new_target: None,
                dirty: true,
            },
            DiffContent::Conflicted,
            DiffContent::Unsupported {
                reason: String::new(),
            },
        ] {
            let state = FileDiff {
                file: file.clone(),
                content,
            };
            for action in [
                PatchAction::Stage,
                PatchAction::Unstage,
                PatchAction::Discard,
            ] {
                assert!(
                    action_patch(action, &state, &everything).is_empty(),
                    "{action:?} emitted a patch for {:?}",
                    state.content
                );
            }
        }
    }
}
