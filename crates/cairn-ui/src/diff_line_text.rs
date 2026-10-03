//! What a diff row draws of a line's bytes, and where its intra-line ranges fall in what it
//! draws.
//!
//! git prints a line's bytes as they are; a terminal turns them into columns. A row is a
//! Skia paragraph, which draws a tab and a control byte as a box (measured headlessly,
//! 2026-10-03), so the row does what the terminal does:
//!
//! - a tab moves to the next multiple of [`TAB_WIDTH`] columns, as `git diff` in a terminal
//!   and `git log`'s message lines (`message_lines`) show it;
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

use cairn_model::{ByteRange, DiffLine};

/// The column a tab moves to a multiple of.
pub const TAB_WIDTH: usize = 8;

/// A line as a row draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownLine {
    pub text: String,
    /// The intra-line ranges, as UTF-16 ranges of `text`; empty ones dropped.
    pub highlights: Vec<(usize, usize)>,
}

/// `bytes` as a row draws them, with `ranges` (byte ranges of `bytes`) carried into the text.
/// A range boundary inside a character moves to that character's end.
pub fn shown_line(bytes: &[u8], ranges: &[ByteRange]) -> ShownLine {
    let body = bytes.strip_suffix(b"\r").unwrap_or(bytes);
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
                    column += 1;
                }
                other => {
                    text.push(other);
                    units += other.len_utf16();
                    column += 1;
                }
            }
        }
        at += chunk.valid().len();
        if !chunk.invalid().is_empty() {
            settle(at, units, &mut next_end);
            text.push('\u{fffd}');
            units += 1;
            column += 1;
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
    ShownLine { text, highlights }
}

/// At least as many columns as the widest of `lines` draws: a byte draws at most one
/// column and a tab at most [`TAB_WIDTH`]. One pass over the bytes, done once per answer
/// so the view's width does not change as it scrolls.
pub fn widest_columns<'a>(lines: impl Iterator<Item = &'a DiffLine>) -> usize {
    lines
        .map(|line| {
            let bytes = line.bytes();
            let tabs = bytes.iter().filter(|byte| **byte == b'\t').count();
            bytes.len() + tabs * (TAB_WIDTH - 1)
        })
        .max()
        .unwrap_or(0)
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

    #[test]
    fn the_widest_line_is_bounded_from_above_by_its_bytes_and_tabs() {
        let lines = [
            DiffLine::terminated("short"),
            DiffLine::terminated("\t\tx"),
            DiffLine::unterminated("é"),
        ];
        assert_eq!(widest_columns(lines.iter()), 17);
        for line in &lines {
            assert!(
                shown_line(line.bytes(), &[]).text.chars().count() <= widest_columns(lines.iter())
            );
        }
        assert_eq!(widest_columns(std::iter::empty()), 0);
    }
}
