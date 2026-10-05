//! Grouping the exact changes into hunks at a context setting.

use std::fmt;
use std::ops::Range;

use crate::{ChangedRange, LineSpan, TextDiff};

/// How much unchanged text a hunk shows around its changes (R1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Lines(u32),
    EntireFile,
}

impl Context {
    /// What a diff view opens at, and the only context a patch is ever built with (R1.6).
    pub const DEFAULT_LINES: u32 = 3;

    /// Zero is raised to one: git apply wants at least one line of context, and R6.3 puts
    /// the same floor under the view's control.
    pub fn lines(count: u32) -> Self {
        Self::Lines(count.max(1))
    }

    pub fn line_count(self) -> Option<u32> {
        match self {
            Self::Lines(count) => Some(count.max(1)),
            Self::EntireFile => None,
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::lines(Self::DEFAULT_LINES)
    }
}

/// What a hunk header spells. The same type renders the header a view draws and the one a
/// patch carries, so the two cannot drift apart.
///
/// The counts are the emitted ones, which for a patch built from part of a selection are
/// not the diff's own: an unselected removed line that became context counts on both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HunkHeader {
    pub old: LineSpan,
    pub new: LineSpan,
}

impl HunkHeader {
    /// git's own rule, from `add-patch.c`: an empty range names the line before it, and a
    /// count of one is left out.
    fn write_side(f: &mut fmt::Formatter<'_>, marker: char, span: LineSpan) -> fmt::Result {
        let start = if span.is_empty() {
            span.start().index()
        } else {
            span.start().one_based()
        };
        write!(f, "{marker}{start}")?;
        if span.len() != 1 {
            write!(f, ",{}", span.len())?;
        }
        Ok(())
    }
}

impl fmt::Display for HunkHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("@@ ")?;
        Self::write_side(f, '-', self.old)?;
        f.write_str(" ")?;
        Self::write_side(f, '+', self.new)?;
        f.write_str(" @@")
    }
}

/// One hunk: the lines it covers on each side, and which of the diff's changes it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old: LineSpan,
    pub new: LineSpan,
    /// Indices into [`TextDiff::changes`], in order and never empty.
    pub changes: Range<u32>,
}

impl Hunk {
    pub fn header(&self) -> HunkHeader {
        HunkHeader {
            old: self.old,
            new: self.new,
        }
    }
}

/// The hunks of one diff at one context (R1.4).
///
/// Reached by index rather than as a slice: a hunk list is as long as a file has separate
/// changes, and handing it out whole invites a caller to walk it per frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunks {
    hunks: Vec<Hunk>,
    context: Context,
}

impl Hunks {
    /// The exact changes of `text` grouped at `context`, as `git diff -U<n>` groups them with
    /// no `diff.interHunkContext`.
    pub fn of(text: &TextDiff, context: Context) -> Self {
        Self::of_ranges(text, text.changes(), context, 0)
    }

    /// `changes` — the exact ranges of `text`, or the whitespace-ignoring ones a view draws
    /// instead — grouped as git groups them at `context` with `inter_hunk_context`
    /// (`diff.interHunkContext`, `--inter-hunk-context`): two changes share a hunk when no
    /// more than twice the context plus the inter-hunk context separates them. A hunk's
    /// [`Hunk::changes`] indexes `changes`. Nothing to group is no hunk, at any context,
    /// as git prints no hunk for a file with no change to show.
    pub fn of_ranges(
        text: &TextDiff,
        changes: &[ChangedRange],
        context: Context,
        inter_hunk_context: u32,
    ) -> Self {
        let old_len = u32::try_from(text.old_lines().len()).unwrap_or(u32::MAX);
        let new_len = u32::try_from(text.new_lines().len()).unwrap_or(u32::MAX);
        let hunks = match context {
            Context::EntireFile => entire_file(changes, old_len, new_len),
            Context::Lines(count) => grouped(
                changes,
                old_len,
                new_len,
                count
                    .max(1)
                    .saturating_mul(2)
                    .saturating_add(inter_hunk_context),
                count.max(1),
            ),
        };
        Self { hunks, context }
    }

    pub fn len(&self) -> usize {
        self.hunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Hunk> {
        self.hunks.get(index)
    }

    pub fn context(&self) -> Context {
        self.context
    }
}

/// One hunk holding the whole file, which is what `entire file` shows — git's answer at a
/// context as long as the file. A file with no change has no hunk at all, as git prints
/// none for it however much context it is asked for.
fn entire_file(changes: &[ChangedRange], old_len: u32, new_len: u32) -> Vec<Hunk> {
    if changes.is_empty() {
        return Vec::new();
    }
    let changes = u32::try_from(changes.len()).unwrap_or(u32::MAX);
    vec![Hunk {
        old: LineSpan::at(0, old_len),
        new: LineSpan::at(0, new_len),
        changes: 0..changes,
    }]
}

/// git's rule (`xdl_get_hunk`): two changes share a hunk when the unchanged lines between
/// them, counted on the old side, are no more than `max_gap` — twice the context plus the
/// inter-hunk context — which with no inter-hunk context is exactly when their context runs
/// would touch.
fn grouped(
    changes: &[ChangedRange],
    old_len: u32,
    new_len: u32,
    max_gap: u32,
    context: u32,
) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut first = 0usize;
    while first < changes.len() {
        let mut last = first;
        while let (Some(current), Some(next)) = (changes.get(last), changes.get(last + 1)) {
            let gap = next
                .removed
                .start()
                .index()
                .saturating_sub(current.removed.end().index());
            if gap > max_gap {
                break;
            }
            last += 1;
        }

        let (Some(opening), Some(closing)) = (changes.get(first), changes.get(last)) else {
            // Unreachable: `first` indexes `changes` and `last` never passes its end.
            break;
        };
        let lead = context
            .min(opening.removed.start().index())
            .min(opening.added.start().index());
        let trail = context
            .min(old_len.saturating_sub(closing.removed.end().index()))
            .min(new_len.saturating_sub(closing.added.end().index()));
        let old_start = opening.removed.start().index().saturating_sub(lead);
        let new_start = opening.added.start().index().saturating_sub(lead);
        let old_end = closing.removed.end().index().saturating_add(trail);
        let new_end = closing.added.end().index().saturating_add(trail);

        hunks.push(Hunk {
            old: LineSpan::at(old_start, old_end.saturating_sub(old_start)),
            new: LineSpan::at(new_start, new_end.saturating_sub(new_start)),
            changes: u32::try_from(first).unwrap_or(u32::MAX)
                ..u32::try_from(last + 1).unwrap_or(u32::MAX),
        });
        first = last + 1;
    }
    hunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangedRange, DiffLine, split_lines};

    fn lines(count: u32) -> Vec<DiffLine> {
        (0..count)
            .map(|n| DiffLine::terminated(format!("l{n}")))
            .collect()
    }

    fn text(old: u32, new: u32, changes: Vec<ChangedRange>) -> TextDiff {
        TextDiff::new(lines(old), lines(new), changes)
    }

    fn change(removed: (u32, u32), added: (u32, u32)) -> ChangedRange {
        ChangedRange::new(
            LineSpan::at(removed.0, removed.1),
            LineSpan::at(added.0, added.1),
        )
    }

    /// The four headers real git writes for the awkward ranges, verified against
    /// `git diff` output. Caught by: counting an empty range from one, which applies a
    /// patch a line off.
    #[test]
    fn a_header_spells_what_git_spells() {
        let cases = [
            (LineSpan::at(0, 0), LineSpan::at(0, 2), "@@ -0,0 +1,2 @@"),
            (LineSpan::at(0, 2), LineSpan::at(0, 0), "@@ -1,2 +0,0 @@"),
            (LineSpan::at(0, 1), LineSpan::at(0, 1), "@@ -1 +1 @@"),
            (LineSpan::at(0, 0), LineSpan::at(0, 1), "@@ -0,0 +1 @@"),
            (LineSpan::at(0, 3), LineSpan::at(0, 4), "@@ -1,3 +1,4 @@"),
            (LineSpan::at(4, 4), LineSpan::at(4, 4), "@@ -5,4 +5,4 @@"),
            (LineSpan::at(5, 0), LineSpan::at(5, 2), "@@ -5,0 +6,2 @@"),
        ];
        for (old, new, expected) in cases {
            assert_eq!(
                HunkHeader { old, new }.to_string(),
                expected,
                "the header for {old:?} against {new:?} was not git's"
            );
        }
    }

    /// Zero context would make a patch git apply refuses, and R6.3 puts the same floor
    /// under the view.
    #[test]
    fn a_context_of_zero_is_raised_to_one() {
        assert_eq!(Context::lines(0), Context::Lines(1));
        assert_eq!(Context::lines(0).line_count(), Some(1));
        assert_eq!(Context::Lines(0).line_count(), Some(1));
        assert_eq!(Context::EntireFile.line_count(), None);
        assert_eq!(Context::default(), Context::Lines(3));

        let text = text(8, 8, vec![change((4, 1), (4, 1))]);
        let hunks = Hunks::of(&text, Context::Lines(0));
        assert_eq!(hunks.len(), 1);
        let hunk = hunks.get(0).expect("one hunk");
        assert_eq!(
            hunk.old,
            LineSpan::at(3, 3),
            "zero context was taken at face value"
        );
    }

    #[test]
    fn a_diff_with_no_changes_has_no_hunks() {
        let text = text(4, 4, Vec::new());
        assert!(Hunks::of(&text, Context::lines(3)).is_empty());
        assert!(Hunks::of(&text, Context::lines(3)).get(0).is_none());
    }

    /// Caught by: taking the context from the start of the file rather than from the
    /// change, which underflows at line 1.
    #[test]
    fn a_hunk_at_the_first_line_takes_the_context_that_exists() {
        let text = text(8, 8, vec![change((0, 1), (0, 1))]);
        let hunks = Hunks::of(&text, Context::lines(3));
        let hunk = hunks.get(0).expect("one hunk");
        assert_eq!(
            hunk.old,
            LineSpan::at(0, 4),
            "the leading context ran off the top"
        );
        assert_eq!(hunk.new, LineSpan::at(0, 4));
        assert_eq!(hunk.header().to_string(), "@@ -1,4 +1,4 @@");
    }

    /// Caught by: running the trailing context past the end of the file.
    #[test]
    fn a_hunk_at_the_last_line_stops_at_the_end_of_the_file() {
        let text = text(8, 8, vec![change((7, 1), (7, 1))]);
        let hunks = Hunks::of(&text, Context::lines(3));
        let hunk = hunks.get(0).expect("one hunk");
        assert_eq!(
            hunk.old,
            LineSpan::at(4, 4),
            "the trailing context ran off the end"
        );
        assert_eq!(hunk.new, LineSpan::at(4, 4));
    }

    /// The context on both sides has to be the same run of unchanged lines, or the
    /// header's two counts describe different amounts of text.
    #[test]
    fn the_context_matches_on_both_sides_when_the_sides_are_different_lengths() {
        // Eight old lines, ten new: line 4 became three lines.
        let text = text(8, 10, vec![change((4, 1), (4, 3))]);
        let hunks = Hunks::of(&text, Context::lines(3));
        let hunk = hunks.get(0).expect("one hunk");
        assert_eq!(hunk.old, LineSpan::at(1, 7));
        assert_eq!(hunk.new, LineSpan::at(1, 9));
        assert_eq!(
            hunk.old.len() - 1,
            hunk.new.len() - 3,
            "the two sides took different amounts of context"
        );
    }

    /// git's merging rule, at the boundary: four unchanged lines between two changes
    /// merge at a context of three and separate at a context of one.
    #[test]
    fn adjacent_changes_merge_at_one_context_and_separate_at_another() {
        let text = text(16, 16, vec![change((2, 1), (2, 1)), change((7, 1), (7, 1))]);

        let merged = Hunks::of(&text, Context::lines(3));
        assert_eq!(
            merged.len(),
            1,
            "a gap of four did not merge at a context of three"
        );
        let hunk = merged.get(0).expect("one hunk");
        assert_eq!(hunk.changes, 0..2);
        assert_eq!(hunk.old, LineSpan::at(0, 11));

        let separate = Hunks::of(&text, Context::lines(1));
        assert_eq!(
            separate.len(),
            2,
            "a gap of four did not separate at a context of one"
        );
        assert_eq!(separate.get(0).expect("first").changes, 0..1);
        assert_eq!(separate.get(1).expect("second").changes, 1..2);
    }

    /// Exactly twice the context is the last gap that merges; one more separates.
    #[test]
    fn the_merging_rule_is_twice_the_context_inclusive() {
        let touching = text(32, 32, vec![change((2, 1), (2, 1)), change((9, 1), (9, 1))]);
        assert_eq!(
            Hunks::of(&touching, Context::lines(3)).len(),
            1,
            "a gap of exactly twice the context separated"
        );

        let apart = text(
            32,
            32,
            vec![change((2, 1), (2, 1)), change((10, 1), (10, 1))],
        );
        assert_eq!(
            Hunks::of(&apart, Context::lines(3)).len(),
            2,
            "a gap of one past twice the context merged"
        );
    }

    #[test]
    fn the_entire_file_is_one_hunk_holding_every_change() {
        let text = text(
            20,
            22,
            vec![change((2, 1), (2, 1)), change((15, 0), (15, 2))],
        );
        let hunks = Hunks::of(&text, Context::EntireFile);
        assert_eq!(hunks.len(), 1);
        let hunk = hunks.get(0).expect("one hunk");
        assert_eq!(hunk.old, LineSpan::at(0, 20));
        assert_eq!(hunk.new, LineSpan::at(0, 22));
        assert_eq!(hunk.changes, 0..2);
        assert_eq!(hunks.context(), Context::EntireFile);
    }

    /// Caught by: showing a hunk header over a file that has no lines at all.
    #[test]
    fn a_file_with_nothing_on_either_side_has_no_hunk_even_entire() {
        let text = TextDiff::new(Vec::new(), Vec::new(), Vec::new());
        assert!(Hunks::of(&text, Context::EntireFile).is_empty());

        let added = TextDiff::new(
            Vec::new(),
            split_lines(b"x\n"),
            vec![change((0, 0), (0, 1))],
        );
        assert_eq!(
            Hunks::of(&added, Context::EntireFile).len(),
            1,
            "a file that gained its first line showed no hunk"
        );
    }

    /// Git parity (phase 06): `git diff -U<n>` prints no hunk for a file with no change to
    /// show, however large `n` is — a rename that kept its content, or a file whose every
    /// change ignoring whitespace hides — so the entire file of one is no hunk either. This
    /// replaced phase 01's rule that drew such a file whole. Caught by: an entire-file hunk
    /// built over no change.
    #[test]
    fn the_entire_file_of_an_unchanged_file_is_no_hunk_as_git_prints_none() {
        let text = text(5, 5, Vec::new());
        assert!(Hunks::of(&text, Context::EntireFile).is_empty());
        assert!(Hunks::of_ranges(&text, &[], Context::EntireFile, 4).is_empty());
    }

    /// git's `diff.interHunkContext`: changes further apart than twice the context still
    /// share a hunk while the gap is within twice the context plus it, and only then.
    /// Measured against git 2.56: `git -c diff.interHunkContext=2 diff -U1` over a gap of
    /// four merges, over a gap of five separates. Caught by: ignoring the inter-hunk
    /// context, or adding it once per side.
    #[test]
    fn the_inter_hunk_context_widens_the_gap_that_merges() {
        let four = text(32, 32, vec![change((2, 1), (2, 1)), change((7, 1), (7, 1))]);
        assert_eq!(
            Hunks::of_ranges(&four, four.changes(), Context::lines(1), 0).len(),
            2
        );
        let merged = Hunks::of_ranges(&four, four.changes(), Context::lines(1), 2);
        assert_eq!(
            merged.len(),
            1,
            "a gap of four separated at -U1 with two more"
        );
        assert_eq!(merged.get(0).expect("one").old, LineSpan::at(1, 8));

        let five = text(32, 32, vec![change((2, 1), (2, 1)), change((8, 1), (8, 1))]);
        assert_eq!(
            Hunks::of_ranges(&five, five.changes(), Context::lines(1), 2).len(),
            2,
            "a gap of five merged at -U1 with two more"
        );
    }

    /// The ranges grouped are the ones given, and a hunk indexes them: a view drawing the
    /// whitespace-ignoring ranges groups those, not the exact ones. Caught by: grouping
    /// `text.changes()` whatever was passed.
    #[test]
    fn the_ranges_given_are_the_ones_grouped() {
        let text = text(
            32,
            32,
            vec![change((2, 1), (2, 1)), change((20, 1), (20, 1))],
        );
        let shown = [change((20, 1), (20, 1))];
        let hunks = Hunks::of_ranges(&text, &shown, Context::lines(3), 0);
        assert_eq!(hunks.len(), 1);
        let hunk = hunks.get(0).expect("one hunk");
        assert_eq!(hunk.changes, 0..1, "the hunk indexed another set of ranges");
        assert_eq!(hunk.old, LineSpan::at(17, 7));
    }
}
