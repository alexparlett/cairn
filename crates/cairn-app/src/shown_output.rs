//! git's output as the window draws it: its terminal control sequences stripped (R10.5), the
//! one thing the window does to it. A URL's userinfo is already gone (staging-and-commit R4.10,
//! R12.2): git's text reaches the window only as `cairn_model::ScrubbedLines`, which the engine's
//! runner fills as it splits git's output into whole lines, before anything keeps or cuts it — a
//! commit's streamed output, a fetch's progress, a failure's kept output and the command log's
//! records alike — and an error's text as `cairn_git::Error::shown` renders it in the worker. So
//! no view, tooltip or dialog can draw a token a remote URL carried, and nothing here scrubs.

use cairn_model::ScrubbedLines;

/// Every line of `lines`, as it is drawn: a text the engine kept whole — a failure's output, a
/// command log record's stderr — blank lines among it.
pub fn shown_lines(lines: &ScrubbedLines) -> Vec<String> {
    lines.lines().map(strip_ansi).collect()
}

/// The lines of `lines` a person reads, as they are drawn: a read of streamed output, its blank
/// lines left out.
pub fn spoken_lines(lines: &ScrubbedLines) -> impl Iterator<Item = String> + '_ {
    lines.spoken().map(strip_ansi)
}

/// `line` without its terminal control sequences (R10.5; Fork Tracker #1218 prints them raw):
/// every CSI (`ESC [ … final`), OSC (`ESC ] … BEL` or `ESC \`) and two-byte escape, and every
/// other control character but a tab — a carriage return a progress meter writes among them.
pub fn strip_ansi(line: &str) -> String {
    let mut shown = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                // CSI: parameters and intermediates, ended by a byte in `@`..=`~`.
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                // OSC: ended by BEL or ST (`ESC \`).
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Any other escape: intermediates (` `..=`/`) and one final character, such as
                // a character set chosen (`ESC ( B`).
                Some(' '..='/') => {
                    for c in chars.by_ref() {
                        if !(' '..='/').contains(&c) {
                            break;
                        }
                    }
                }
                Some(_) | None => {}
            },
            '\t' => shown.push(c),
            c if c.is_control() => {}
            c => shown.push(c),
        }
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R10.5: a hook's coloured output reads as its text — colours, cursor moves and window
    /// titles gone, a carriage return gone, a tab kept. Caught by: an escape's tail left in
    /// (`[31m`), or an OSC swallowing the text after it.
    #[test]
    fn ansi_sequences_and_control_characters_are_stripped() {
        assert_eq!(
            strip_ansi("\u{1b}[1;31merror:\u{1b}[0m lint failed"),
            "error: lint failed"
        );
        assert_eq!(
            strip_ansi("\u{1b}]0;title\u{7}after\u{1b}]8;;url\u{1b}\\link"),
            "afterlink"
        );
        assert_eq!(strip_ansi("50%\r100%\tdone\u{1b}(B"), "50%100%\tdone");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    /// What is drawn of git's lines: as the engine scrubbed them, escapes stripped, every line
    /// of a kept text and only the spoken ones of a stream. Caught by: escapes left in, or a
    /// stream's blank lines drawn.
    #[test]
    fn git_lines_are_drawn_scrubbed_and_stripped() {
        let lines = ScrubbedLines::scrubbing(
            "\u{1b}[31mremote:\u{1b}[0m https://u:ghp_SECRET@host/r\n\nsee https://h/x  ",
        );
        assert_eq!(
            shown_lines(&lines),
            ["remote: https://host/r", "", "see https://h/x  "]
        );
        assert_eq!(
            spoken_lines(&lines).collect::<Vec<_>>(),
            ["remote: https://host/r", "see https://h/x"]
        );
    }
}
