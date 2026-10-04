//! One row of the unified view (PRD R6.4, R6.5): an old and a new line-number gutter on the
//! plain ground, a thin separator, then — tinted for a changed line, from the separator to
//! the row's end — the line, a few pixels in. No marker column (the user's decision of
//! 2026-10-04, as Fork's default): a removed line leaves the new gutter blank and an added one
//! the old, so the gutter still says what the tint says without colour. A hunk header is
//! git's `@@ -a,b +c,d @@` and the function context git printed after it, muted, at the same
//! height, starting where a line's text starts, with no band and no button (Fork, Finding
//! 13); git's `\ No newline at end of file` is a muted row of its own. The gutter scrolls
//! sideways with the text.

use cairn_model::{ByteRange, LineNumber, ShownDiff, UnifiedRow};
use freya::prelude::*;

use crate::diff_palette::DIFF_MUTED;
use crate::diff_row_parts::{LineKind, TEXT_PADDING, line_text, number, separator, words};
use crate::diff_view::{NO_NEWLINE_AT_END, RowGeometry, header_words};

/// Row `index` of `shown`'s unified rows, `size` tall, keyed by `key`.
pub(crate) fn build(
    key: usize,
    index: usize,
    size: f32,
    shown: &ShownDiff,
    data: &RowGeometry,
) -> Element {
    let row = rect()
        .key(key)
        .horizontal()
        .content(Content::Flex)
        // As wide as the view, or as the widest line where that is wider: the tint reaches
        // the edge, and the extent is the answer's, whichever rows are built.
        .width(Size::fill())
        .min_width(Size::px(data.width))
        .height(Size::px(size));
    let Some((text, overlay)) = shown.text() else {
        return row.into();
    };
    let Some(drawn) = shown
        .layout()
        .and_then(|layout| layout.row(text, overlay, index))
    else {
        // The count and the answer can disagree for one frame; an empty row of the right
        // height stands in.
        return row.into();
    };
    let current = data
        .current
        .as_ref()
        .is_some_and(|rows| rows.contains(&index));
    let number_width = data.number_width;
    match drawn {
        UnifiedRow::Header(header) => {
            note_row(row, number_width, current, header_words(header, overlay))
        }
        UnifiedRow::Context { old, new, line } => line_row(
            row,
            number_width,
            current,
            (Some(old), Some(new)),
            LineKind::Context,
            line.bytes(),
            &[],
        ),
        UnifiedRow::Removed { old, line } => line_row(
            row,
            number_width,
            current,
            (Some(old), None),
            LineKind::Removed,
            line.bytes(),
            overlay.on_removed_line(old),
        ),
        UnifiedRow::Added { new, line } => line_row(
            row,
            number_width,
            current,
            (None, Some(new)),
            LineKind::Added,
            line.bytes(),
            overlay.on_added_line(new),
        ),
        UnifiedRow::NoNewlineAtEnd => {
            note_row(row, number_width, current, NO_NEWLINE_AT_END.to_owned())
        }
    }
}

/// The two numbers, right-aligned, on the diff's ground: a side the line is not on is blank.
fn gutter(numbers: (Option<LineNumber>, Option<LineNumber>), number_width: f32) -> Rect {
    rect()
        .horizontal()
        .height(Size::fill())
        .cross_align(Alignment::Center)
        .child(number(numbers.0, number_width))
        .child(number(numbers.1, number_width))
}

fn line_row(
    row: Rect,
    number_width: f32,
    current: bool,
    numbers: (Option<LineNumber>, Option<LineNumber>),
    kind: LineKind,
    bytes: &[u8],
    ranges: &[ByteRange],
) -> Element {
    let (tint, emphasis) = kind.dress();
    row.child(gutter(numbers, number_width))
        .child(separator(current))
        .child(
            rect()
                .horizontal()
                .width(Size::flex(1.))
                .height(Size::fill())
                .padding(Gaps::new(0., 0., 0., TEXT_PADDING))
                .cross_align(Alignment::Center)
                .background(tint)
                .child(line_text(bytes, ranges, emphasis)),
        )
        .into()
}

/// A hunk header, or git's end-of-file marker: muted text where the line's text goes, no
/// numbers, no tint, no band.
fn note_row(row: Rect, number_width: f32, current: bool, text: String) -> Element {
    row.child(gutter((None, None), number_width))
        .child(separator(current))
        .child(
            rect()
                .horizontal()
                .width(Size::flex(1.))
                .height(Size::fill())
                .padding(Gaps::new(0., 0., 0., TEXT_PADDING))
                .cross_align(Alignment::Center)
                .child(words(text, DIFF_MUTED)),
        )
        .into()
}
