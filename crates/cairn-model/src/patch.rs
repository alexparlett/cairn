//! The unified patch a selection makes (R1.6).
//!
//! What goes in is a [`ChangedFile`], a [`TextDiff`] and a [`Selection`]; what comes out is
//! bytes `git apply --cached` accepts against the old version. There is no context
//! argument and no view: three lines of context, always, whatever is on screen. And the
//! input type carries no whitespace-ignoring ranges, so R1.7 holds by construction rather
//! than by care — the ranges a view may draw instead live in [`crate::DisplayOverlay`],
//! which this function never sees.
//!
//! The rules the format imposes, each of them read out of git's own `add-patch.c` and
//! `apply.c` and checked against `git diff` output:
//!
//! - An unselected removed line becomes context; an unselected added line is dropped.
//! - Counts are recounted from the lines actually emitted, and a hunk's new start is its
//!   old start shifted by the net lines of every hunk emitted before it.
//! - A hunk with nothing selected is left out entirely.
//! - `\ No newline at end of file` follows the line it belongs to, whatever marker that
//!   line ended up with.
//! - A deletion with some removals left out is a modification of the file, not a deletion.

use std::borrow::Cow;
use std::fmt;

use crate::{
    ChangeStatus, ChangedFile, Context, DiffLine, HunkHeader, Hunks, LineNumber, LineSpan, Oid,
    RepoPath, Selection, TextDiff,
};

/// The context a patch carries, whatever the view shows (R1.6).
pub const PATCH_CONTEXT: u32 = 3;

const NO_NEWLINE: &[u8] = b"\\ No newline at end of file\n";
const NULL_ID: &str = "0000000";

/// A unified patch, in bytes, ready for `git apply`.
#[derive(Clone, PartialEq, Eq)]
pub struct Patch(Vec<u8>);

impl Patch {
    /// The patch a selection that selects nothing makes.
    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// The patch as text. A patch is bytes because a path or a line may not be UTF-8.
    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }
}

impl fmt::Debug for Patch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Patch(\n{}\n)", self.text())
    }
}

/// Builds the patch for one file and one selection.
///
/// Returns an empty patch when the file has changes to select and none of them is
/// selected. A file whose change is not in its lines at all — a rename, a mode change, an
/// empty file added or deleted — still gets its headers, because that is the whole change.
pub fn emit_patch(file: &ChangedFile, text: &TextDiff, selection: &Selection) -> Patch {
    let (body, hunks_emitted) = emit_body(text, selection);
    let selectable = text
        .changes()
        .iter()
        .any(|change| !change.removed.is_empty() || !change.added.is_empty());
    if selectable && hunks_emitted == 0 {
        return Patch::empty();
    }

    let whole = selection.holds_every_change(text);
    let mut out = Vec::new();
    write_headers(&mut out, file, whole, hunks_emitted > 0);
    out.extend_from_slice(&body);
    Patch(out)
}

/// The hunks, and how many of them survived the selection.
fn emit_body(text: &TextDiff, selection: &Selection) -> (Vec<u8>, usize) {
    let hunks = Hunks::of(text, Context::lines(PATCH_CONTEXT));
    let mut body = Vec::new();
    let mut emitted = 0usize;
    // How many lines every hunk emitted so far added to the file, net. The new side's
    // start is the old side's start shifted by it.
    let mut delta: i64 = 0;

    for index in 0..hunks.len() {
        let Some(hunk) = hunks.get(index) else {
            // Unreachable: `index` is below the length just read.
            break;
        };
        let mut draft = Draft::default();
        let mut old = hunk.old.start().index();
        let mut new = hunk.new.start().index();

        for change_index in hunk.changes.clone() {
            let Some(change) = text.changes().get(change_index as usize) else {
                // Unreachable for a hunk of this diff; a hunk from another one stops here.
                break;
            };
            let run = change
                .removed
                .start()
                .index()
                .saturating_sub(old)
                .min(change.added.start().index().saturating_sub(new));
            for step in 0..run {
                draft.context(text.old_line(LineNumber::from_index(old.saturating_add(step))));
            }
            for line in change.removed.numbers() {
                if selection.holds_removed(line) {
                    draft.removed(text.old_line(line));
                } else {
                    draft.context(text.old_line(line));
                }
            }
            for line in change.added.numbers() {
                if selection.holds_added(line) {
                    draft.added(text.new_line(line));
                }
            }
            old = change.removed.end().index();
            new = change.added.end().index();
        }

        let trailing = hunk
            .old
            .end()
            .index()
            .saturating_sub(old)
            .min(hunk.new.end().index().saturating_sub(new));
        for step in 0..trailing {
            draft.context(text.old_line(LineNumber::from_index(old.saturating_add(step))));
        }

        if !draft.selected {
            continue;
        }

        let new_start = i64::from(hunk.old.start().index())
            .saturating_add(delta)
            .clamp(0, i64::from(u32::MAX));
        let header = HunkHeader {
            old: LineSpan::new(hunk.old.start(), draft.old_count),
            new: LineSpan::at(new_start as u32, draft.new_count),
        };
        body.extend_from_slice(header.to_string().as_bytes());
        body.push(b'\n');
        body.extend_from_slice(&draft.body);
        delta = delta
            .saturating_add(i64::from(draft.new_count))
            .saturating_sub(i64::from(draft.old_count));
        emitted += 1;
    }

    (body, emitted)
}

/// One hunk under construction: its lines, the counts its header will spell, and whether
/// anything in it was actually selected.
#[derive(Debug, Default)]
struct Draft {
    body: Vec<u8>,
    old_count: u32,
    new_count: u32,
    selected: bool,
}

impl Draft {
    fn context(&mut self, line: Option<&DiffLine>) {
        if let Some(line) = line {
            Self::write(&mut self.body, b' ', line);
            self.old_count = self.old_count.saturating_add(1);
            self.new_count = self.new_count.saturating_add(1);
        }
    }

    fn removed(&mut self, line: Option<&DiffLine>) {
        if let Some(line) = line {
            Self::write(&mut self.body, b'-', line);
            self.old_count = self.old_count.saturating_add(1);
            self.selected = true;
        }
    }

    fn added(&mut self, line: Option<&DiffLine>) {
        if let Some(line) = line {
            Self::write(&mut self.body, b'+', line);
            self.new_count = self.new_count.saturating_add(1);
            self.selected = true;
        }
    }

    /// A patch line always ends in a newline; the marker line is what says the file's own
    /// line did not, and it travels with whichever marker the line ended up with.
    fn write(body: &mut Vec<u8>, marker: u8, line: &DiffLine) {
        body.push(marker);
        body.extend_from_slice(line.bytes());
        body.push(b'\n');
        if !line.ends_with_newline() {
            body.extend_from_slice(NO_NEWLINE);
        }
    }
}

fn write_headers(out: &mut Vec<u8>, file: &ChangedFile, whole: bool, has_hunks: bool) {
    out.extend_from_slice(b"diff --git a/");
    out.extend_from_slice(file.old_path.as_bytes());
    out.extend_from_slice(b" b/");
    out.extend_from_slice(file.new_path.as_bytes());
    out.push(b'\n');

    if file.mode_changed()
        && let (Some(old), Some(new)) = (file.old_mode, file.new_mode)
    {
        write_text(
            out,
            &format!("old mode {}\nnew mode {}\n", old.octal(), new.octal()),
        );
    }

    // A deletion only stays a deletion while every removed line is selected; otherwise
    // what is being staged is a smaller file, not the absence of one.
    let deleting = matches!(file.status, ChangeStatus::Deleted) && whole;
    if deleting && let Some(mode) = file.old_mode {
        write_text(out, &format!("deleted file mode {}\n", mode.octal()));
    } else if matches!(file.status, ChangeStatus::Added)
        && let Some(mode) = file.new_mode
    {
        write_text(out, &format!("new file mode {}\n", mode.octal()));
    }

    if let Some(similarity) = file.similarity() {
        write_text(
            out,
            &format!("similarity index {}%\n", similarity.percent()),
        );
        let verb = if file.is_copy() { "copy" } else { "rename" };
        write_path_line(out, &format!("{verb} from "), &file.old_path);
        write_path_line(out, &format!("{verb} to "), &file.new_path);
    }

    write_index_line(out, file, whole);

    if has_hunks {
        if matches!(file.status, ChangeStatus::Added) {
            write_text(out, "--- /dev/null\n");
        } else {
            write_path_line(out, "--- a/", &file.old_path);
        }
        if deleting {
            write_text(out, "+++ /dev/null\n");
        } else {
            write_path_line(out, "+++ b/", &file.new_path);
        }
    }
}

/// git writes this line when the blob changed, and leaves it out when only the mode or the
/// path did. A patch built from part of a selection leaves it out too: the content it
/// makes is neither the old blob nor the new one, and a wrong id here would be a lie a
/// three-way apply could act on.
fn write_index_line(out: &mut Vec<u8>, file: &ChangedFile, whole: bool) {
    if !whole {
        return;
    }
    let old = abbreviate(file.old_id.as_ref());
    let new = abbreviate(file.new_id.as_ref());
    if old == new {
        return;
    }
    let mode = match (file.old_mode, file.new_mode) {
        (Some(old_mode), Some(new_mode)) if old_mode == new_mode => Some(new_mode),
        _ => None,
    };
    match mode {
        Some(mode) => write_text(out, &format!("index {old}..{new} {}\n", mode.octal())),
        None => write_text(out, &format!("index {old}..{new}\n")),
    }
}

fn abbreviate(id: Option<&Oid>) -> String {
    id.map_or_else(|| NULL_ID.to_string(), |id| id.short().as_str().to_string())
}

fn write_text(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
}

/// A path goes out as the bytes git stores, so that a patch names the file git names.
fn write_path_line(out: &mut Vec<u8>, prefix: &str, path: &RepoPath) {
    write_text(out, prefix);
    out.extend_from_slice(path.as_bytes());
    out.push(b'\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangedRange, FileMode, split_lines};

    fn modified(old: &[u8], new: &[u8], changes: Vec<ChangedRange>) -> (ChangedFile, TextDiff) {
        (
            ChangedFile {
                status: ChangeStatus::Modified,
                old_path: RepoPath::from("f.txt"),
                new_path: RepoPath::from("f.txt"),
                old_mode: Some(FileMode::Regular),
                new_mode: Some(FileMode::Regular),
                old_id: None,
                new_id: None,
            },
            TextDiff::new(split_lines(old), split_lines(new), changes),
        )
    }

    fn change(removed: (u32, u32), added: (u32, u32)) -> ChangedRange {
        ChangedRange::new(
            LineSpan::at(removed.0, removed.1),
            LineSpan::at(added.0, added.1),
        )
    }

    fn patch_text(file: &ChangedFile, text: &TextDiff, selection: &Selection) -> String {
        emit_patch(file, text, selection).text().into_owned()
    }

    /// The shape `git diff` itself writes for a one-line edit in the middle of a file,
    /// down to the omitted `,1` and the three lines of context.
    #[test]
    fn a_whole_selection_writes_what_git_writes() {
        let (file, text) = modified(
            b"a\nb\nc\nd\ne\nf\ng\nh\n",
            b"a\nb\nc\nD\ne\nf\ng\nh\n",
            vec![change((3, 1), (3, 1))],
        );
        let selection = Selection::with_every_change(&text);
        assert_eq!(
            patch_text(&file, &text, &selection),
            "diff --git a/f.txt b/f.txt\n\
             --- a/f.txt\n\
             +++ b/f.txt\n\
             @@ -1,7 +1,7 @@\n\
             \x20a\n\x20b\n\x20c\n\
             -d\n\
             +D\n\
             \x20e\n\x20f\n\x20g\n"
        );
    }

    /// The context is three whatever the view is set to, which is the point of the emitter
    /// taking no context at all.
    #[test]
    fn a_patch_carries_three_lines_of_context_and_takes_no_say_in_it() {
        let (file, text) = modified(
            b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\n",
            b"a\nb\nc\nd\ne\nF\ng\nh\ni\nj\nk\n",
            vec![change((5, 1), (5, 1))],
        );
        let patch = patch_text(&file, &text, &Selection::with_every_change(&text));
        let context = patch.lines().filter(|line| line.starts_with(' ')).count();
        assert_eq!(
            context, 6,
            "the patch did not carry three lines either side"
        );
        assert!(patch.contains("@@ -3,7 +3,7 @@"));
    }

    /// R1.6's line rules: a removal left out becomes context and an addition left out goes
    /// away, and the counts follow the lines that were emitted.
    #[test]
    fn an_unselected_removal_becomes_context_and_an_unselected_addition_is_dropped() {
        let (file, text) = modified(
            b"a\nb\nc\nd\n",
            b"a\nB\nC\nd\n",
            vec![change((1, 2), (1, 2))],
        );
        let mut selection = Selection::with_every_change(&text);
        selection.unselect_removed(LineNumber::from_index(2));
        selection.unselect_added(LineNumber::from_index(2));

        assert_eq!(
            patch_text(&file, &text, &selection),
            "diff --git a/f.txt b/f.txt\n\
             --- a/f.txt\n\
             +++ b/f.txt\n\
             @@ -1,4 +1,4 @@\n\
             \x20a\n\
             -b\n\
             \x20c\n\
             +B\n\
             \x20d\n"
        );
    }

    /// The header arithmetic that a golden string would never catch: leaving a whole hunk
    /// out moves every later hunk's new side, and only its new side.
    #[test]
    fn a_hunk_with_nothing_selected_is_left_out_and_the_next_hunks_new_side_shifts() {
        let old = b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\n";
        let new = b"a\nb\nc\nX\nY\nd\ne\nf\ng\nh\ni\nj\nk\nl\nM\nn\no\np\n";
        let (file, text) = modified(
            old,
            new,
            vec![change((3, 0), (3, 2)), change((12, 1), (14, 1))],
        );

        let mut only_second = Selection::empty();
        only_second.select_removed(LineNumber::from_index(12));
        only_second.select_added(LineNumber::from_index(14));
        let patch = patch_text(&file, &text, &only_second);
        assert!(
            !patch.contains("+X"),
            "a hunk with nothing selected was still written"
        );
        assert!(
            patch.contains("@@ -10,7 +10,7 @@"),
            "the second hunk's new side did not stay where the skipped hunk left it: {patch}"
        );

        let whole = Selection::with_every_change(&text);
        let both = patch_text(&file, &text, &whole);
        assert!(
            both.contains("@@ -10,7 +12,7 @@"),
            "with the first hunk kept, the second hunk's new side did not shift by two: {both}"
        );
    }

    /// Both awkward newline cases, in the shape `git diff` writes them.
    #[test]
    fn a_line_that_never_ended_carries_its_marker_on_whichever_side_it_is_on() {
        let (file, text) = modified(b"one", b"one\n", vec![change((0, 1), (0, 1))]);
        assert_eq!(
            patch_text(&file, &text, &Selection::with_every_change(&text)),
            "diff --git a/f.txt b/f.txt\n\
             --- a/f.txt\n\
             +++ b/f.txt\n\
             @@ -1 +1 @@\n\
             -one\n\
             \\ No newline at end of file\n\
             +one\n"
        );

        let (file, text) = modified(b"one\n", b"two", vec![change((0, 1), (0, 1))]);
        assert_eq!(
            patch_text(&file, &text, &Selection::with_every_change(&text)),
            "diff --git a/f.txt b/f.txt\n\
             --- a/f.txt\n\
             +++ b/f.txt\n\
             @@ -1 +1 @@\n\
             -one\n\
             +two\n\
             \\ No newline at end of file\n"
        );
    }

    /// R1.6 again: the marker stays with its line when that line becomes context.
    #[test]
    fn the_marker_follows_a_removal_that_was_turned_into_context() {
        let (file, text) = modified(b"a\nb", b"a\nB\n", vec![change((1, 1), (1, 1))]);
        let mut selection = Selection::with_every_change(&text);
        selection.unselect_removed(LineNumber::from_index(1));
        let patch = patch_text(&file, &text, &selection);
        assert!(
            patch.contains(" b\n\\ No newline at end of file\n"),
            "the marker did not follow its line into the context: {patch}"
        );
    }

    /// A partial deletion is a modification: the headers must not say the file went.
    #[test]
    fn a_deletion_with_a_removal_left_out_is_a_modification() {
        let file = ChangedFile {
            status: ChangeStatus::Deleted,
            old_path: RepoPath::from("d.txt"),
            new_path: RepoPath::from("d.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: None,
            old_id: None,
            new_id: None,
        };
        let text = TextDiff::new(
            split_lines(b"l1\nl2\nl3\n"),
            Vec::new(),
            vec![change((0, 3), (0, 0))],
        );

        let whole = patch_text(&file, &text, &Selection::with_every_change(&text));
        assert!(whole.contains("deleted file mode 100644\n"), "{whole}");
        assert!(whole.contains("+++ /dev/null\n"), "{whole}");

        let mut partial = Selection::with_every_change(&text);
        partial.unselect_removed(LineNumber::from_index(1));
        let patch = patch_text(&file, &text, &partial);
        assert!(
            !patch.contains("deleted file mode"),
            "a partial deletion still claimed to delete the file: {patch}"
        );
        assert!(
            patch.contains("+++ b/d.txt\n"),
            "a partial deletion sent the new side to /dev/null: {patch}"
        );
        assert!(patch.contains("@@ -1,3 +1 @@"), "{patch}");
    }

    /// An addition stays an addition however little of it is selected: the file did not
    /// exist before, whatever is put in it now.
    #[test]
    fn an_addition_with_a_line_left_out_is_still_a_new_file() {
        let file = ChangedFile {
            status: ChangeStatus::Added,
            old_path: RepoPath::from("n.txt"),
            new_path: RepoPath::from("n.txt"),
            old_mode: None,
            new_mode: Some(FileMode::Executable),
            old_id: None,
            new_id: None,
        };
        let text = TextDiff::new(
            Vec::new(),
            split_lines(b"alpha\nbeta\n"),
            vec![change((0, 0), (0, 2))],
        );
        let mut selection = Selection::with_every_change(&text);
        selection.unselect_added(LineNumber::from_index(0));
        let patch = patch_text(&file, &text, &selection);
        assert!(patch.contains("new file mode 100755\n"), "{patch}");
        assert!(patch.contains("--- /dev/null\n"), "{patch}");
        assert!(patch.contains("@@ -0,0 +1 @@\n+beta\n"), "{patch}");
        assert!(!patch.contains("+alpha"), "{patch}");
    }

    #[test]
    fn a_selection_of_nothing_makes_no_patch_at_all() {
        let (file, text) = modified(b"a\nb\n", b"a\nB\n", vec![change((1, 1), (1, 1))]);
        let patch = emit_patch(&file, &text, &Selection::empty());
        assert!(patch.is_empty(), "{patch:?}");
        assert_eq!(patch.as_bytes(), b"");
    }

    /// A change that is not in the lines still has to be staged, so it still gets headers.
    #[test]
    fn a_rename_and_a_mode_change_are_headers_with_no_hunks() {
        let renamed = ChangedFile {
            status: ChangeStatus::Renamed(crate::Similarity::from_percent(100)),
            old_path: RepoPath::from("r.txt"),
            new_path: RepoPath::from("r2.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: None,
            new_id: None,
        };
        let unchanged = TextDiff::new(split_lines(b"x\n"), split_lines(b"x\n"), Vec::new());
        assert_eq!(
            patch_text(&renamed, &unchanged, &Selection::empty()),
            "diff --git a/r.txt b/r2.txt\n\
             similarity index 100%\n\
             rename from r.txt\n\
             rename to r2.txt\n"
        );

        let moded = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("k.txt"),
            new_path: RepoPath::from("k.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Executable),
            old_id: None,
            new_id: None,
        };
        assert_eq!(
            patch_text(&moded, &unchanged, &Selection::empty()),
            "diff --git a/k.txt b/k.txt\n\
             old mode 100644\n\
             new mode 100755\n"
        );
    }

    /// git writes the `index` line when the blob moved and leaves it out when it did not,
    /// and the mode goes on it only when the file has the same mode either side.
    #[test]
    fn the_index_line_follows_gits_own_rule() {
        let old_id = Oid::parse("587be6b4c3f93f93c489c0111bba5596147a26cb").expect("an id");
        let new_id = Oid::parse("b77b4eb4c3f93f93c489c0111bba5596147a26cb").expect("an id");

        let mut file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("k.txt"),
            new_path: RepoPath::from("k.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: Some(old_id),
            new_id: Some(new_id),
        };
        let text = TextDiff::new(
            split_lines(b"x\n"),
            split_lines(b"x\ny\n"),
            vec![change((1, 0), (1, 1))],
        );
        let whole = Selection::with_every_change(&text);
        assert!(
            patch_text(&file, &text, &whole).contains("index 587be6b..b77b4eb 100644\n"),
            "the mode was not carried on the index line of an unchanged mode"
        );

        file.new_mode = Some(FileMode::Executable);
        let patch = patch_text(&file, &text, &whole);
        assert!(
            patch.contains("index 587be6b..b77b4eb\n"),
            "a changed mode was written twice: {patch}"
        );

        file.new_mode = Some(FileMode::Regular);
        file.new_id = Some(old_id);
        assert!(
            !patch_text(&file, &text, &whole).contains("index "),
            "an index line was written for a blob that did not move"
        );
    }

    /// A partial selection makes content that is neither blob, so no id is claimed for it.
    #[test]
    fn a_partial_selection_claims_no_blob_id() {
        let old_id = Oid::parse("587be6b4c3f93f93c489c0111bba5596147a26cb").expect("an id");
        let new_id = Oid::parse("b77b4eb4c3f93f93c489c0111bba5596147a26cb").expect("an id");
        let file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("k.txt"),
            new_path: RepoPath::from("k.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: Some(old_id),
            new_id: Some(new_id),
        };
        let text = TextDiff::new(
            split_lines(b"x\n"),
            split_lines(b"x\ny\nz\n"),
            vec![change((1, 0), (1, 2))],
        );
        let mut partial = Selection::empty();
        partial.select_added(LineNumber::from_index(1));
        assert!(
            !patch_text(&file, &text, &partial).contains("index "),
            "a patch of part of a change claimed the whole change's blob id"
        );
    }

    /// A path git cannot spell as text still has to name the same file.
    #[test]
    fn a_path_goes_out_as_the_bytes_git_stores() {
        let file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::new(vec![b'd', 0xff, b'/', b'f']),
            new_path: RepoPath::new(vec![b'd', 0xff, b'/', b'f']),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: None,
            new_id: None,
        };
        let text = TextDiff::new(
            split_lines(b"a\n"),
            split_lines(b"b\n"),
            vec![change((0, 1), (0, 1))],
        );
        let patch = emit_patch(&file, &text, &Selection::with_every_change(&text));
        let header = b"diff --git a/d\xff/f b/d\xff/f\n";
        assert!(
            patch.as_bytes().starts_with(header),
            "the path was rewritten on its way into the patch"
        );
    }
}
