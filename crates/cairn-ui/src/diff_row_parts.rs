//! The pieces both diff views build a row from: a line number, the separator after the
//! gutter, a line's text with its intra-line ranges and its cut marker, and a muted note.
//!
//! Unified (`unified_rows`) and side-by-side (`side_by_side_rows`) rows are these pieces in two
//! arrangements, so a line reads alike in both: the same typeface, colours, tab stops, control
//! pictures and cut (R6.9).
//!
//! **No marker column** (the user's decision, 2026-10-04, reversing 2026-10-03's): Fork draws
//! none by default, only colours, and neither does Cairn. **The gutter is Fork's**, sized from
//! its measurements and the user-supplied Fork captures of 2026-10-04: the numbers in a
//! smaller size than the text ([`NUMBER_FONT_SIZE`]), each column as wide as the file's widest
//! number and a few pixels either side ([`number_width`]), a clear gap before the separator
//! ([`NUMBER_PADDING`]) and a small one after it before the text ([`TEXT_PADDING`]). The gap
//! is made by a box around the number, not by the number's own padding: this toolkit build
//! lays a label's paragraph out over the label's whole width, padding included, so a
//! right-aligned number padded on its own label touches the separator anyway.

use cairn_model::{ByteRange, LineNumber};
use freya::prelude::*;

use crate::diff_line_text::{cut_marker, shown_line};
use crate::diff_palette::{
    ADDED_EMPHASIS, ADDED_TINT, CURRENT_CHANGE, DIFF_FONT_FAMILY, DIFF_FONT_SIZE, DIFF_MUTED,
    DIFF_TEXT, GROUND, GUTTER_SEPARATOR, MONO_ADVANCE_EM, REMOVED_EMPHASIS, REMOVED_TINT,
};

/// One column of IBM Plex Mono at the diff's size.
pub(crate) const ADVANCE: f32 = DIFF_FONT_SIZE * MONO_ADVANCE_EM;
/// The line numbers' size: Fork's numbers are smaller than its text. Fork's Mac numbers are
/// 6.5 pt tall (Finding 24), and Plex Mono's figures are 0.698 em tall, so 6.5 ÷ 0.698 ≈
/// 9.3 px; the user-supplied Windows capture agrees — its digits advance 0.84 of its code's
/// advance, and 0.84 × 11 px ≈ 9.3 px. This toolkit build takes a font size in whole pixels
/// (`FontSize` holds an `i32`), so 9.
pub const NUMBER_FONT_SIZE: f32 = 9.0;
/// One digit of a line number: Plex Mono's advance at [`NUMBER_FONT_SIZE`].
pub(crate) const NUMBER_ADVANCE: f32 = NUMBER_FONT_SIZE * MONO_ADVANCE_EM;
/// Before a number's digits: Fork's columns sit about a digit apart, all of it on the
/// digits' right but this sliver.
pub(crate) const NUMBER_LEAD: f32 = 2.0;
/// After a number's digits, before the next column or the separator: Fork's gap there,
/// about 6 px in the user-supplied captures.
pub const NUMBER_PADDING: f32 = 6.0;
/// After the separator, before a line's text: a few pixels, as in Fork (the user's
/// correction of 2026-10-04: the wider space in the capture was the code's own indentation).
pub const TEXT_PADDING: f32 = 4.0;
pub(crate) const SEPARATOR_WIDTH: f32 = 1.0;
pub(crate) const CURRENT_SEPARATOR_WIDTH: f32 = 3.0;
pub(crate) const TEXT_END_PADDING: f32 = 24.0;

/// What a line row draws: its tint, its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineKind {
    Context,
    Removed,
    Added,
}

impl LineKind {
    /// The line's tint and its intra-line ranges' tint.
    pub(crate) fn dress(self) -> (Color, Color) {
        match self {
            Self::Context => (GROUND, GROUND),
            Self::Removed => (REMOVED_TINT, REMOVED_EMPHASIS),
            Self::Added => (ADDED_TINT, ADDED_EMPHASIS),
        }
    }
}

/// How wide a number column is for numbers of `digits` digits: the digits at
/// [`NUMBER_FONT_SIZE`], the lead before them and the gap after.
pub fn number_width(digits: usize) -> f32 {
    digits as f32 * NUMBER_ADVANCE + NUMBER_LEAD + NUMBER_PADDING
}

/// One right-aligned line number, or a blank of the same width: a box `width` wide whose
/// padding holds the gap, the number filling what is left.
pub(crate) fn number(at: Option<LineNumber>, width: f32) -> Rect {
    rect()
        .width(Size::px(width))
        .padding(Gaps::new(0., NUMBER_PADDING, 0., NUMBER_LEAD))
        .child(
            label()
                .width(Size::fill())
                .text_align(TextAlign::End)
                .max_lines(1)
                .font_family(DIFF_FONT_FAMILY)
                .font_size(NUMBER_FONT_SIZE)
                .color(DIFF_MUTED)
                .text(
                    at.map(|line| line.one_based().to_string())
                        .unwrap_or_default(),
                ),
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
    if line.cut > 0 {
        drawn.span(Span::new(cut_marker(line.cut)).color(DIFF_MUTED))
    } else {
        drawn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gutter's columns for one, two and four digits, spelled out: Plex Mono's 0.6 em
    /// at 9 px is 5.4 px a digit, and 8 px of gaps — so two columns of four digits come to
    /// 59.2 px, Fork's ≈ 60 in the user-supplied capture. Caught by: the numbers measured
    /// at the text's size (6.6 px a digit), or the gaps changed without the docs' table.
    #[test]
    fn a_number_column_is_its_digits_at_the_numbers_size_and_two_gaps() {
        for (digits, width) in [(1, 13.4), (2, 18.8), (4, 29.6)] {
            assert!(
                (number_width(digits) - width).abs() < 0.001,
                "{digits} digits: {} px, not {width}",
                number_width(digits)
            );
        }
    }
}
