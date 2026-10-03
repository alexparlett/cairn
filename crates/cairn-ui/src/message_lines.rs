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
//!   of the message's line in terminal columns (`crate::columns`): a wide character — CJK,
//!   an emoji — counting two and a combining mark none (`--expand-tabs`, on by default for
//!   these formats).

use crate::columns::columns;

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
            column += columns(c);
        }
    }
    out
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
            // Tab stops after characters a terminal draws two columns wide and none wide,
            // where the old hand-rolled table counted one: a rocket, a kana from the
            // supplement, a Thai tone mark.
            (
                "🚀\tz\n\u{1b000}\tz\n\u{e01}\u{e48}\tz\n",
                &["🚀      z", "\u{1b000}      z", "\u{e01}\u{e48}       z"],
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
