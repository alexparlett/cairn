//! A commit message as `git log` shows it (git's `medium` and `fuller` formats, what the
//! Commit tab mirrors): the lines git prints under the header, without the four spaces
//! it indents them by.
//!
//! git does not print a message as stored. Measured against git 2.56 (`pretty.c`'s
//! `pp_remainder` and `strbuf_add_tabexpand`):
//!
//! - every line loses its trailing whitespace — spaces, tabs and the `\r` of a CRLF line,
//!   but not a vertical tab, a form feed or a no-break space, which git does not count;
//! - blank lines before the first line with text are not printed, and neither are blank
//!   lines after the last (a line of whitespace alone is blank); blank lines between are;
//! - a tab becomes spaces to the next multiple of eight columns, counted from the start
//!   of the message's line, a wide character (CJK) counting two (`--expand-tabs`, on by
//!   default for these formats).

/// The columns git expands a tab to.
const TAB_WIDTH: usize = 8;

/// The lines git shows for `message`, in order; empty when it shows none.
pub fn shown_lines(message: &str) -> Vec<String> {
    let trimmed: Vec<&str> = message
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t', '\r']))
        .collect();
    let Some(first) = trimmed.iter().position(|line| !line.is_empty()) else {
        return Vec::new();
    };
    let last = trimmed
        .iter()
        .rposition(|line| !line.is_empty())
        .unwrap_or(first);
    trimmed
        .get(first..=last)
        .unwrap_or_default()
        .iter()
        .map(|line| expand_tabs(line))
        .collect()
}

/// `line` with each tab replaced by spaces to the next multiple of [`TAB_WIDTH`].
fn expand_tabs(line: &str) -> String {
    if !line.contains('\t') {
        return line.to_owned();
    }
    let mut out = String::with_capacity(line.len() + TAB_WIDTH);
    let mut column = 0usize;
    for c in line.chars() {
        if c == '\t' {
            let spaces = TAB_WIDTH - column % TAB_WIDTH;
            out.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            out.push(c);
            column += width(c);
        }
    }
    out
}

/// Columns `c` takes on a terminal, as git's `utf8_width` counts them: none for a combining
/// mark or a zero-width character, two for an East Asian wide or full-width one, one
/// otherwise. The main ranges of Unicode's East Asian Width table, not the whole of it: a
/// character outside them that git counts as two is a stated limit of the tab expansion.
fn width(c: char) -> usize {
    let point = u32::from(c);
    let zero = [
        0x0300..=0x036f,
        0x0483..=0x0489,
        0x0591..=0x05bd,
        0x1ab0..=0x1aff,
        0x1dc0..=0x1dff,
        0x200b..=0x200f,
        0x20d0..=0x20ff,
        0xfe00..=0xfe0f,
        0xfe20..=0xfe2f,
    ];
    let wide = [
        0x1100..=0x115f,
        0x2e80..=0x303e,
        0x3041..=0x33ff,
        0x3400..=0x4dbf,
        0x4e00..=0x9fff,
        0xa000..=0xa4cf,
        0xac00..=0xd7a3,
        0xf900..=0xfaff,
        0xfe30..=0xfe4f,
        0xff00..=0xff60,
        0xffe0..=0xffe6,
        0x1f300..=0x1f64f,
        0x1f900..=0x1f9ff,
        0x20000..=0x2fffd,
        0x30000..=0x3fffd,
    ];
    if zero.iter().any(|range| range.contains(&point)) {
        0
    } else if wide.iter().any(|range| range.contains(&point)) {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each case's right side is `git log --format=fuller` (git 2.56) on a commit with that
    /// message, its four-space indent removed. Caught by: popping only the last empty line,
    /// keeping a `\r`, leading blank lines or a whitespace-only blank line, trimming a
    /// vertical tab, or not expanding a tab (or expanding it from the indented column, or
    /// counting a wide character as one).
    #[test]
    fn a_message_reads_line_for_line_as_git_log_shows_it() {
        for (message, git) in [
            ("subj\r\n\r\nbody\r\n\n\n", &["subj", "", "body"][..]),
            (
                "\n\n  \nsubj2\n\nbody2\n\n  \n\t\n",
                &["subj2", "", "body2"],
            ),
            (
                "   lead\t subj  \n \t \n\tindented\tcode\n日本\tz\nab\tc\n\n\nlast\n",
                &[
                    "   lead  subj",
                    "",
                    "        indented        code",
                    "日本    z",
                    "ab      c",
                    "",
                    "",
                    "last",
                ],
            ),
            ("x", &["x"]),
            ("\n\n", &[]),
            (
                "Teach the engine\r\n\r\n\r\nSigned-off-by: A <a@x>  \r\n\r\n",
                &["Teach the engine", "", "", "Signed-off-by: A <a@x>"],
            ),
            (
                "subj\u{b}\n\u{c}\nmid\u{a0}\nend\n\u{b}\n",
                &["subj\u{b}", "\u{c}", "mid\u{a0}", "end", "\u{b}"],
            ),
        ] {
            assert_eq!(shown_lines(message), git, "{message:?}");
        }
    }
}
