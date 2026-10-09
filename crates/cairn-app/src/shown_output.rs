//! git's output as the window draws it: its terminal control sequences stripped (R10.5) and
//! the userinfo of every URL in it removed (staging-and-commit R12.2, #46's display half) —
//! before any state that is drawn keeps it, so no view, tooltip or dialog can draw a token a
//! remote URL carried. Every text of git's the window keeps goes through here: a commit's
//! streamed output and the output the engine kept of a failure (`commit_box_state`), a failed
//! branch's output (`create_branch`) and the activity popover's (`activity`).
//!
//! Where a text was cut is the engine's to say (phase 11's QA, TC5): a command log record's
//! `stderr_cut`, an `Error::GitFailed`'s `stderr_cut` offsets. The lanes scrub with it
//! ([`scrubbed_at`]), before anything leaves them; the window's own pass over what it keeps
//! reads every text from a line's start, and changes nothing the lane's left.

use cairn_model::Scrubber;

use crate::commit_box_state::strip_ansi;
use crate::worker::WriteEnding;

/// Lines arriving one at a time, shown in order, with a URL cut at a line's end carried over.
#[derive(Debug, Clone, Default)]
pub struct ShownLines {
    scrubber: Scrubber,
}

impl ShownLines {
    /// `line` as it is drawn.
    pub fn line(&mut self, line: &str) -> String {
        self.scrubber.line(&strip_ansi(line))
    }
}

/// A text kept whole — by the engine, or a record of the command log — as the lines drawn.
pub fn shown_lines(text: &str) -> Vec<String> {
    let mut shown = ShownLines::default();
    text.lines().map(|line| shown.line(line)).collect()
}

/// `text` with the userinfo of every URL in it removed and nothing else changed, read from a
/// line's start — what the workers do to every message of git's they hand the window, so even a
/// message the window draws as it is (a write's ending, a fetch's failure) carries no token.
pub fn scrubbed(text: &str) -> String {
    scrubbed_at(text, &[])
}

/// [`scrubbed`], for a text the engine says was cut: each part beginning at one of `cut`'s byte
/// offsets (each at a line's start in `text`, or `0`) is read as begun part-way through a line,
/// so a URL cut there loses its userinfo too; the parts are scrubbed apart.
pub fn scrubbed_at(text: &str, cut: &[usize]) -> String {
    let mut starts: Vec<usize> = cut
        .iter()
        .copied()
        .filter(|at| *at <= text.len() && text.is_char_boundary(*at))
        .collect();
    starts.push(0);
    starts.sort_unstable();
    starts.dedup();
    let mut shown = String::with_capacity(text.len());
    for (at, start) in starts.iter().enumerate() {
        let end = starts.get(at + 1).copied().unwrap_or(text.len());
        let mut scrubber = if cut.contains(start) {
            Scrubber::after_cut()
        } else {
            Scrubber::new()
        };
        let part = &text[*start..end];
        shown.push_str(
            &part
                .split('\n')
                .map(|line| scrubber.line(line))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    shown
}

/// A write's ending as the window keeps it: every message of it, git's words in them, scrubbed
/// (R12.2) — so the line under the lists and every dialog drawn from it carry no token, whoever
/// built the ending.
pub fn shown_ending(ending: WriteEnding) -> WriteEnding {
    match ending {
        WriteEnding::Done(done) => WriteEnding::Done(done),
        WriteEnding::Stale { path, message } => WriteEnding::Stale {
            path,
            message: scrubbed(&message),
        },
        WriteEnding::Refused { message } => WriteEnding::Refused {
            message: scrubbed(&message),
        },
        WriteEnding::Failed {
            message,
            locks,
            command,
            output,
        } => WriteEnding::Failed {
            message: scrubbed(&message),
            locks,
            command: command.as_deref().map(scrubbed),
            output: scrubbed(&output),
        },
        WriteEnding::MayHaveTakenEffect { message, locks } => WriteEnding::MayHaveTakenEffect {
            message: scrubbed(&message),
            locks,
        },
        WriteEnding::Incomplete {
            done,
            kept,
            message,
        } => WriteEnding::Incomplete {
            done,
            kept,
            message: scrubbed(&message),
        },
        WriteEnding::NotRun { message } => WriteEnding::NotRun {
            message: scrubbed(&message),
        },
    }
}

/// One line of git's, a command line among them, as it is drawn.
pub fn shown_line(line: &str) -> String {
    ShownLines::default().line(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R12.2: what is drawn has no userinfo and no escape sequences, a URL split over a line
    /// end included; a long text is read as possibly cut. Caught by: lines kept as git wrote
    /// them, or scrubbed one at a time with nothing carried.
    #[test]
    fn shown_lines_carry_no_userinfo_and_no_escapes() {
        let text = "\u{1b}[31mremote:\u{1b}[0m https://u:ghp_SECRET@host/r\nsee https://u:gh\np_SECRET@host/x";
        let shown = shown_lines(text);
        assert_eq!(shown, ["remote: https://host/r", "see https://", "host/x"]);
        assert!(shown.iter().all(|line| !line.contains("SECRET")));
        // Phase 11's QA (TC5): a cut is the engine's to say, never a length's — a long text
        // never said to be cut keeps its first line whole, and a short one said to be cut, at
        // its front or after another part, loses the userinfo its cut began inside.
        let long = format!("ada@example.com wrote this\n{}", "y".repeat(64 * 1024 + 1));
        assert!(shown_lines(&long)[0] == "ada@example.com wrote this");
        assert_eq!(scrubbed(&long), long);
        assert_eq!(scrubbed_at("er:SECRET@host/x", &[0]), "host/x");
        assert_eq!(
            scrubbed_at("out line\ner:SECRET@host/x", &[9]),
            "out line\nhost/x"
        );
        assert_eq!(
            scrubbed_at("a@b out\ner:SECRET@host/x", &[8]),
            "a@b out\nhost/x",
            "a part not cut kept"
        );
        let mut streamed = ShownLines::default();
        assert_eq!(streamed.line("to https://tok"), "to https://");
        assert_eq!(streamed.line("en@host/r"), "host/r");
    }
}
