//! The pieces both diff views build a row from: a line number, the separator after the
//! gutter, a line's text with its intra-line ranges and its cut marker, and a muted note.
//!
//! Unified (`unified_rows`) and side-by-side (`side_by_side_rows`) rows are these pieces in two
//! arrangements, so a line reads alike in both: the same typeface, colours, tab stops, control
//! pictures and cut (R6.9).

use cairn_model::{ByteRange, LineNumber};
use freya::prelude::*;

use crate::diff_line_text::{LINE_CUT_MARKER, shown_line};
use crate::diff_palette::{
    ADDED_EMPHASIS, ADDED_TINT, CURRENT_CHANGE, DIFF_FONT_FAMILY, DIFF_FONT_SIZE, DIFF_MUTED,
    DIFF_TEXT, GROUND, GUTTER_SEPARATOR, MONO_ADVANCE_EM, REMOVED_EMPHASIS, REMOVED_TINT,
};

/// One column of IBM Plex Mono at the diff's size.
pub(crate) const ADVANCE: f32 = DIFF_FONT_SIZE * MONO_ADVANCE_EM;
pub(crate) const NUMBER_PADDING: f32 = 6.0;
pub(crate) const SEPARATOR_WIDTH: f32 = 1.0;
pub(crate) const CURRENT_SEPARATOR_WIDTH: f32 = 3.0;
/// The `-`/`+` column.
pub(crate) const MARKER_WIDTH: f32 = 2.0 * ADVANCE + 6.0;
pub(crate) const TEXT_END_PADDING: f32 = 24.0;

/// What a line row draws: its marker, its tint, its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineKind {
    Context,
    Removed,
    Added,
}

impl LineKind {
    /// The marker column's character, the line's tint and its intra-line ranges' tint.
    pub(crate) fn dress(self) -> (&'static str, Color, Color) {
        match self {
            Self::Context => (" ", GROUND, GROUND),
            Self::Removed => ("-", REMOVED_TINT, REMOVED_EMPHASIS),
            Self::Added => ("+", ADDED_TINT, ADDED_EMPHASIS),
        }
    }
}

/// How wide a number column is for numbers of `digits` digits.
pub(crate) fn number_width(digits: usize) -> f32 {
    digits as f32 * ADVANCE + 2.0 * NUMBER_PADDING
}

/// One right-aligned line number on the diff's ground, or a blank of the same width.
pub(crate) fn number(at: Option<LineNumber>, width: f32) -> Label {
    label()
        .width(Size::px(width))
        .padding(Gaps::new(0., NUMBER_PADDING, 0., NUMBER_PADDING))
        .text_align(TextAlign::End)
        .max_lines(1)
        .font_family(DIFF_FONT_FAMILY)
        .font_size(DIFF_FONT_SIZE)
        .color(DIFF_MUTED)
        .text(
            at.map(|line| line.one_based().to_string())
                .unwrap_or_default(),
        )
}

/// The line between the gutter and the text: wider and in the accent on the change last
/// moved to.
pub(crate) fn separator(current: bool) -> Rect {
    rect()
        .width(Size::px(if current {
            CURRENT_SEPARATOR_WIDTH
        } else {
            SEPARATOR_WIDTH
        }))
        .height(Size::fill())
        .background(if current {
            CURRENT_CHANGE
        } else {
            GUTTER_SEPARATOR
        })
}

/// The `-`/`+` column.
pub(crate) fn marker(text: &'static str) -> Label {
    label()
        .width(Size::px(MARKER_WIDTH))
        .padding(Gaps::new(0., 0., 0., 4.))
        .max_lines(1)
        .font_family(DIFF_FONT_FAMILY)
        .font_size(DIFF_FONT_SIZE)
        .color(DIFF_TEXT)
        .text(text)
}

/// A row's words, in the diff's typeface, one line, never wrapped.
pub(crate) fn words(text: String, colour: Color) -> Paragraph {
    paragraph()
        .span(text)
        .font_family(DIFF_FONT_FAMILY)
        .font_size(DIFF_FONT_SIZE)
        .color(colour)
        .max_lines(1)
        .vertical_align(VerticalAlign::Center)
        .height(Size::fill())
}

/// A line's text as a row draws it — its intra-line `ranges` in `emphasis`, and, for a line
/// past the long-line limit, its start and the muted cut marker (R6.9). The work is the
/// drawn text's, never the whole line's.
pub(crate) fn line_text(bytes: &[u8], ranges: &[ByteRange], emphasis: Color) -> Paragraph {
    let line = shown_line(bytes, ranges);
    let drawn = words(line.text, DIFF_TEXT)
        .highlights(Some(line.highlights))
        .highlight_color(emphasis);
    if line.cut {
        drawn.span(Span::new(LINE_CUT_MARKER).color(DIFF_MUTED))
    } else {
        drawn
    }
}
