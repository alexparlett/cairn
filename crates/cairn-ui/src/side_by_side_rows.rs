//! One row of the side-by-side view (PRD R6.1, R6.4): two columns of equal width inside the
//! one virtualising view, never two scroll views — the old side on the left and the new on
//! the right, each a line-number gutter, a separator, and a text area, with filler where its
//! side has no line beside the other's (Fork, Finding 11: equal panes, one gutter each, grey
//! filler rows; the hunk header repeated at the top of each). A changed line's tint spans its
//! whole column, number included, as Fork's side-by-side panes do (user-supplied Fork
//! capture, 2026-10-04) — unlike unified, whose gutter keeps the ground. No marker column
//! (the user's decision, 2026-10-04, as Fork's default): a changed pair is told from context
//! by its tint alone, as in Fork, a stated residual (`docs/systems/diff.md`).
//!
//! **Equal columns, both in view.** Each column is half the view, so both sides are on screen
//! together, as Fork's two panes are. A line wider than its column scrolls sideways: the
//! view's horizontal extent is the widest line's overflow past its column, and a sideways
//! scroll slides the text of both columns together while each gutter stays where it is —
//! every row is laid out at the scroll's offset, a spacer as wide as the scroll ahead of the
//! columns so they stay put while the virtualising view moves its content.
//!
//! **What each column draws is what `git diff` prints** (phase 07's parity rule, pinned by
//! `side_by_side_view` in `cairn-git`'s parity tests): a context line is git's printed line,
//! the new side's, in both columns; git's `\ No newline at end of file` stands in the column
//! of the side whose last line did not end.

use cairn_model::{ByteRange, LineNumber, ShownDiff, SideBySideRow};
use freya::prelude::*;

use crate::diff_palette::{DIFF_MUTED, FILLER, GUTTER_SEPARATOR};
use crate::diff_row_parts::{
    LineKind, SEPARATOR_WIDTH, TEXT_PADDING, line_text, number, separator, words,
};
use crate::diff_view::{NO_NEWLINE_AT_END, RowGeometry, header_words};

/// The rule between the two columns.
pub(crate) const MIDDLE_WIDTH: f32 = SEPARATOR_WIDTH;

/// Where a side-by-side row's parts go, for a view `view_width` wide scrolled sideways by
/// `scrolled_x` (zero or less).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Columns {
    /// Each column's width: half of what the rule leaves.
    pub(crate) column: f32,
    /// How far a column's text overflows it at most: the view's horizontal extent past its
    /// width.
    pub(crate) overflow: f32,
    /// How far the text is scrolled, between nothing and `overflow`.
    pub(crate) shift: f32,
}

impl Columns {
    pub(crate) fn of(view_width: f32, number_width: f32, text_width: f32, scrolled_x: f32) -> Self {
        let column = ((view_width - MIDDLE_WIDTH) / 2.0).max(0.0);
        let area = (column - number_width - SEPARATOR_WIDTH).max(0.0);
        let overflow = (text_width - area).max(0.0);
        Self {
            column,
            overflow,
            shift: (-scrolled_x).clamp(0.0, overflow),
        }
    }

    /// How wide every row is: the view, and the overflow to scroll through.
    pub(crate) fn row_width(self) -> f32 {
        2.0 * self.column + MIDDLE_WIDTH + self.overflow
    }
}

/// What one column of a row holds.
enum Cell<'a> {
    Line {
        at: LineNumber,
        kind: LineKind,
        bytes: &'a [u8],
        ranges: &'a [ByteRange],
    },
    /// Muted words where the text goes: a hunk header, git's end-of-file marker.
    Note(String),
    /// Nothing on this side beside the other side's line.
    Filler,
}

/// Row `index` of `shown`'s side-by-side rows, `size` tall, keyed by `key`.
pub(crate) fn build(
    key: usize,
    index: usize,
    size: f32,
    shown: &ShownDiff,
    data: &RowGeometry,
) -> Element {
    let (scrolled_x, _): (i32, i32) = data.scroll.into();
    let columns = Columns::of(
        data.view_width,
        data.number_width,
        data.text_width,
        scrolled_x as f32,
    );
    let row = rect()
        .key(key)
        .horizontal()
        .width(Size::px(columns.row_width()))
        .height(Size::px(size));
    let Some((text, overlay)) = shown.text() else {
        return row.into();
    };
    let Some(drawn) = shown
        .side_by_side_layout()
        .and_then(|layout| layout.row(text, overlay, index))
    else {
        // The count and the answer can disagree for one frame.
        return row.into();
    };
    let current = data
        .current
        .as_ref()
        .is_some_and(|rows| rows.contains(&index));
    let note = |present: bool| {
        if present {
            Cell::Note(NO_NEWLINE_AT_END.to_owned())
        } else {
            Cell::Filler
        }
    };
    let (left, right) = match drawn {
        SideBySideRow::Header(header) => {
            let words = header_words(header, overlay);
            (Cell::Note(words.clone()), Cell::Note(words))
        }
        SideBySideRow::Context { old, new, line } => (
            Cell::Line {
                at: old,
                kind: LineKind::Context,
                bytes: line.bytes(),
                ranges: &[],
            },
            Cell::Line {
                at: new,
                kind: LineKind::Context,
                bytes: line.bytes(),
                ranges: &[],
            },
        ),
        SideBySideRow::Replaced {
            old,
            old_line,
            new,
            new_line,
        } => (
            Cell::Line {
                at: old,
                kind: LineKind::Removed,
                bytes: old_line.bytes(),
                ranges: overlay.on_removed_line(old),
            },
            Cell::Line {
                at: new,
                kind: LineKind::Added,
                bytes: new_line.bytes(),
                ranges: overlay.on_added_line(new),
            },
        ),
        SideBySideRow::Removed { old, line } => (
            Cell::Line {
                at: old,
                kind: LineKind::Removed,
                bytes: line.bytes(),
                ranges: overlay.on_removed_line(old),
            },
            Cell::Filler,
        ),
        SideBySideRow::Added { new, line } => (
            Cell::Filler,
            Cell::Line {
                at: new,
                kind: LineKind::Added,
                bytes: line.bytes(),
                ranges: overlay.on_added_line(new),
            },
        ),
        SideBySideRow::NoNewlineAtEnd { old, new } => (note(old), note(new)),
    };
    row.child(rect().width(Size::px(columns.shift)))
        .child(column(left, columns, data, current))
        .child(
            rect()
                .width(Size::px(MIDDLE_WIDTH))
                .height(Size::fill())
                .background(GUTTER_SEPARATOR),
        )
        .child(column(right, columns, data, current))
        .into()
}

/// One column: its number, the separator, and its text area clipped to the column, the text
/// slid by the sideways scroll; a changed line's tint behind all three.
fn column(cell: Cell<'_>, columns: Columns, data: &RowGeometry, current: bool) -> Rect {
    let (at, tint, content): (Option<LineNumber>, Option<Color>, Option<Rect>) = match cell {
        Cell::Line {
            at,
            kind,
            bytes,
            ranges,
        } => {
            let (tint, emphasis) = kind.dress();
            (
                Some(at),
                Some(tint),
                Some(
                    text_strip(data.text_width, columns.shift)
                        .child(line_text(bytes, ranges, emphasis)),
                ),
            )
        }
        Cell::Note(text) => (
            None,
            None,
            Some(text_strip(data.text_width, columns.shift).child(words(text, DIFF_MUTED))),
        ),
        Cell::Filler => (None, None, None),
    };
    let filler = content.is_none();
    let area = rect()
        .horizontal()
        .width(Size::flex(1.))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .maybe(filler, |el| el.background(FILLER))
        .maybe_child(content);
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::px(columns.column))
        .height(Size::fill())
        .cross_align(Alignment::Center)
        .maybe(tint.is_some(), |el| el.background(tint.unwrap_or(FILLER)))
        .child(number(at, data.number_width))
        .child(separator(current))
        .child(area)
}

/// A column's text, as wide as the widest line so it never wraps — the few pixels after the
/// separator included — slid left by the scroll.
fn text_strip(text_width: f32, shift: f32) -> Rect {
    rect()
        .horizontal()
        .width(Size::px(text_width))
        .height(Size::fill())
        .padding(Gaps::new(0., 0., 0., TEXT_PADDING))
        .cross_align(Alignment::Center)
        .offset_x(-shift)
}
