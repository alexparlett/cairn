//! How many columns a character takes where `git diff` and `git log` are read: a terminal.
//! Both the diff's rows (`diff_line_text`) and a commit message's lines (`message_lines`)
//! expand a tab to the next multiple of eight columns counted this way, so the text after
//! it lines up as it does in the user's terminal (the user's decision, 2026-10-03).
//!
//! The width is Unicode's, as `unicode-width` reads it character by character — the rule
//! git's own `utf8_width` and a terminal's `wcwidth` follow: two for an East Asian wide or
//! full-width character and an emoji, none for a combining mark or a zero-width character,
//! one otherwise, an ambiguous character counting one as outside an East Asian locale. A
//! control character takes none, as git counts it; a caller that draws one as a picture
//! counts the picture.

use unicode_width::UnicodeWidthChar;

/// The terminal columns `c` takes.
pub(crate) fn columns(c: char) -> usize {
    c.width().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: every character counted as one, or controls counted as one.
    #[test]
    fn wide_characters_take_two_combining_marks_and_controls_none() {
        for (c, expected) in [
            ('a', 1),
            ('é', 1),
            ('日', 2),
            ('😀', 2),
            ('\u{1b000}', 2),
            ('\u{301}', 0),
            ('\u{e48}', 0),
            ('\u{200b}', 0),
            ('\u{1}', 0),
            ('\u{2401}', 1),
        ] {
            assert_eq!(columns(c), expected, "{c:?}");
        }
    }
}
