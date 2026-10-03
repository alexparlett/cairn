//! What a diff row draws of a line's bytes, and where its intra-line ranges fall in what it
//! draws.
//!
//! git prints a line's bytes as they are; a terminal turns them into columns. A row is a
//! Skia paragraph, which draws a tab and a control byte as a box (measured headlessly,
//! 2026-10-03), so the row does what the terminal does:
//!
//! - a tab moves to the next multiple of [`TAB_WIDTH`] columns, as `git diff` in a terminal
//!   and `git log`'s message lines (`message_lines`) show it, the columns before it counted
//!   as a terminal counts them (`crate::columns`: two for a wide character, none for a
//!   combining mark);
//! - a carriage return ending the line — a CRLF file's — is not drawn, as a terminal shows
//!   nothing for it;
//! - any other C0 control byte and DEL are drawn as their Unicode control pictures
//!   (`U+2400` ...), which IBM Plex Mono 2.5 carries, so they are seen rather than lost;
//! - bytes that are not UTF-8 are one `U+FFFD` per invalid sequence, as git's own lossy
//!   reading of a path is.
//!
//! The intra-line ranges the engine computes are byte ranges of the line; a paragraph's
//! highlight indexes UTF-16 code units of what it draws (Skia's `getRectsForRange`,
//! measured: a range splitting a surrogate pair draws nothing). [`shown_line`] maps one to
//! the other in the same pass that builds the text. A row's work is proportional to its own
//! line — never to the file, never to where the view is scrolled.
//!
//! **A line past the long-line limit is drawn cut** (R6.9). Only a diff loaded past R2.6's
//! ceilings holds one — the engine refuses any other with a line longer than
//! [`DiffLimits::MAX_LINE_BYTES`] — and drawn whole it would cost its whole length per row
//! built (a 64 MiB line is 64 MiB of text for one row) and a horizontal extent of hundreds of
//! millions of pixels. So a row draws at most `cairn_model::LINE_CUT_BYTES` of a line, ending
//! on a character (`cairn_model::drawn_bytes`), followed by [`LINE_CUT_MARKER`] in the muted
//! colour, which says that the rest is not drawn.

use cairn_model::{ByteRange, TAB_STOP, drawn_bytes};

use crate::columns::columns;

/// The column a tab moves to a multiple of: `git diff` in a terminal.
pub const TAB_WIDTH: usize = TAB_STOP;

/// Drawn after a line cut at [`LINE_CUT_BYTES`], in the muted colour (R6.9).
pub const LINE_CUT_MARKER: &str = " … line truncated";

/// A line as a row draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownLine {
    pub text: String,
    /// The intra-line ranges, as UTF-16 ranges of `text`; empty ones dropped.
    pub highlights: Vec<(usize, usize)>,
    /// Whether the line was longer than the long-line limit and `text` is its start only.
    pub cut: bool,
}

/// `bytes` as a row draws them, with `ranges` (byte ranges of `bytes`) carried into the text.
/// A range boundary inside a character moves to that character's end. A line longer than
/// `cairn_model::LINE_CUT_BYTES` is drawn to its cut (`cairn_model::drawn_bytes`), and only the
/// ranges that start before the cut are read, so the work is bounded by the cut whatever the
/// line's length.
pub fn shown_line(bytes: &[u8], ranges: &[ByteRange]) -> ShownLine {
    let (bytes, cut) = drawn_bytes(bytes);
    let ranges = if cut {
        // In order, as the engine answers them: those that start before the cut.
        let kept = ranges
            .iter()
            .take_while(|range| (range.start as usize) < bytes.len())
            .count();
        ranges.get(..kept).unwrap_or_default()
    } else {
        ranges
    };
    let body = if cut {
        bytes
    } else {
        bytes.strip_suffix(b"\r").unwrap_or(bytes)
    };
    // Each range's two ends, in byte order, with where each lands in UTF-16 units.
    let mut ends: Vec<(usize, usize)> = ranges
        .iter()
        .enumerate()
        .flat_map(|(n, range)| {
            [
                (range.start as usize, 2 * n),
                (range.end as usize, 2 * n + 1),
            ]
        })
        .collect();
    ends.sort_unstable();
    let mut landed = vec![0usize; ends.len()];
    let mut next_end = 0usize;

    let mut text = String::with_capacity(body.len());
    let (mut units, mut column, mut at) = (0usize, 0usize, 0usize);
    let mut settle = |byte: usize, units: usize, next_end: &mut usize| {
        while let Some((end, slot)) = ends.get(*next_end) {
            if *end > byte {
                break;
            }
            if let Some(place) = landed.get_mut(*slot) {
                *place = units;
            }
            *next_end += 1;
        }
    };

    for chunk in body.utf8_chunks() {
        for (offset, character) in chunk.valid().char_indices() {
            settle(at + offset, units, &mut next_end);
            match character {
                '\t' => {
                    let spaces = TAB_WIDTH - column % TAB_WIDTH;
                    text.extend(std::iter::repeat_n(' ', spaces));
                    units += spaces;
                    column += spaces;
                }
                '\u{0}'..='\u{1f}' | '\u{7f}' => {
                    let picture = if character == '\u{7f}' {
                        '\u{2421}'
                    } else {
                        char::from_u32(0x2400 + u32::from(character)).unwrap_or('\u{fffd}')
                    };
                    text.push(picture);
                    units += picture.len_utf16();
                    column += columns(picture);
                }
                other => {
                    text.push(other);
                    units += other.len_utf16();
                    column += columns(other);
                }
            }
        }
        at += chunk.valid().len();
        if !chunk.invalid().is_empty() {
            settle(at, units, &mut next_end);
            text.push('\u{fffd}');
            units += 1;
            column += columns('\u{fffd}');
            at += chunk.invalid().len();
        }
    }
    settle(usize::MAX, units, &mut next_end);

    let highlights = (0..ranges.len())
        .filter_map(|n| {
            let start = *landed.get(2 * n)?;
            let end = *landed.get(2 * n + 1)?;
            (start < end).then_some((start, end))
        })
        .collect();
    ShownLine {
        text,
        highlights,
        cut,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: u32, end: u32) -> ByteRange {
        ByteRange::new(start, end)
    }

    #[test]
    fn plain_text_is_drawn_as_it_is_and_its_ranges_do_not_move() {
        let shown = shown_line(b"let x = 1;", &[range(4, 5), range(8, 9)]);
        assert_eq!(shown.text, "let x = 1;");
        assert_eq!(shown.highlights, [(4, 5), (8, 9)]);
    }

    /// Caught by: a tab drawn as one column (a box, in Skia), or tab stops counted from the
    /// start of the row's text rather than from the column reached.
    #[test]
    fn a_tab_moves_to_the_next_stop_and_the_ranges_after_it_follow() {
        let shown = shown_line(b"\tab\tc", &[range(1, 3), range(4, 5)]);
        assert_eq!(shown.text, "        ab      c");
        assert_eq!(shown.highlights, [(8, 10), (16, 17)]);
    }

    /// The ranges are UTF-16 units of the text, which is what Skia's paragraph indexes:
    /// `é` is two bytes and one unit, `😀` four bytes and two units. Caught by: passing
    /// byte offsets through, or counting characters.
    #[test]
    fn a_range_after_a_wide_character_lands_in_utf16_units() {
        let line = "é😀x".as_bytes();
        let shown = shown_line(line, &[range(6, 7), range(2, 6)]);
        assert_eq!(shown.text, "é😀x");
        assert_eq!(shown.highlights, [(3, 4), (1, 3)]);
    }

    /// A tab stop is counted in terminal columns (the user's decision, 2026-10-03): a wide
    /// character — an emoji, a CJK ideograph, a kana outside the old table — takes two, a
    /// combining mark none. Each line here is what `git diff` prints in a terminal. Caught
    /// by: counting every character as one column, or a hand-rolled table that misses one.
    #[test]
    fn a_tab_after_a_wide_or_combining_character_stops_where_a_terminal_stops() {
        for (line, shown) in [
            ("😀\tx", "😀      x"),
            ("🚀\tx", "🚀      x"),
            ("日本\tx", "日本    x"),
            ("\u{1b000}\tx", "\u{1b000}      x"),
            ("e\u{301}\tx", "e\u{301}       x"),
            ("\u{e01}\u{e48}\tx", "\u{e01}\u{e48}       x"),
        ] {
            assert_eq!(shown_line(line.as_bytes(), &[]).text, shown, "{line:?}");
        }
        // A control drawn as its picture takes the picture's one column.
        assert_eq!(shown_line(b"\x01\tx", &[]).text, "\u{2401}       x");
    }

    #[test]
    fn controls_are_pictures_a_crlf_ending_is_not_drawn_and_invalid_bytes_are_replaced() {
        let shown = shown_line(b"a\x01b\x7f\xffc\r", &[range(5, 6)]);
        assert_eq!(shown.text, "a\u{2401}b\u{2421}\u{fffd}c");
        assert_eq!(
            shown.highlights,
            [(5, 6)],
            "the range after an invalid byte"
        );
        assert_eq!(shown_line(b"a\rb", &[]).text, "a\u{240d}b");
    }

    /// A boundary inside a character moves to its end, and a range that becomes empty is
    /// dropped rather than drawn as a caret.
    #[test]
    fn a_boundary_inside_a_character_moves_to_its_end() {
        let line = "aéb".as_bytes();
        assert_eq!(shown_line(line, &[range(2, 4)]).highlights, [(2, 3)]);
        assert!(shown_line(line, &[range(2, 3)]).highlights.is_empty());
        assert!(shown_line(b"abc", &[range(2, 2)]).highlights.is_empty());
    }

    /// R6.9: a line longer than the long-line limit is drawn to the limit, never past it,
    /// with its ranges past the cut left out. Caught by: drawing the whole line (a
    /// multi-mebibyte row), or reading ranges past the cut.
    #[test]
    fn a_line_past_the_limit_is_drawn_to_the_limit() {
        let long = "a".repeat(3 * 1024 * 1024);
        let shown = shown_line(long.as_bytes(), &[range(10, 12), range(5_000, 5_002)]);
        assert!(shown.cut);
        assert_eq!(shown.text.len(), cairn_model::LINE_CUT_BYTES);
        assert_eq!(shown.highlights, [(10, 12)]);
        let at_limit = "a".repeat(cairn_model::LINE_CUT_BYTES);
        assert!(!shown_line(at_limit.as_bytes(), &[]).cut);
    }
}
