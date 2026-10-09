//! git's output as the window draws it: its terminal control sequences stripped (R10.5) and
//! the userinfo of every URL in it removed (staging-and-commit R12.2, #46's display half) —
//! before any state that is drawn keeps it, so no view, tooltip or dialog can draw a token a
//! remote URL carried. Every text of git's the window keeps goes through here: a commit's
//! streamed output and the output the engine kept of a failure (`commit_box_state`), a failed
//! branch's output (`create_branch`) and the activity popover's (`activity`).

use cairn_model::Scrubber;

use crate::commit_box_state::strip_ansi;
use crate::worker::WriteEnding;

/// The smallest tail the engine cuts git's output to from the front — a commit's stdout,
/// 64 KiB (`cairn-git`'s `ops/commit.rs`) — so a text at least this long may begin part-way
/// through a line, and is read as if it did: its first line's leading run up to an `@` is
/// taken for the end of a userinfo. A shorter text was never cut.
pub const MAY_BE_CUT_BYTES: usize = 64 * 1024;

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
    let mut shown = ShownLines {
        scrubber: if text.len() >= MAY_BE_CUT_BYTES {
            Scrubber::after_cut()
        } else {
            Scrubber::new()
        },
    };
    text.lines().map(|line| shown.line(line)).collect()
}

/// `text` with the userinfo of every URL in it removed and nothing else changed — what the
/// workers do to every message and output of git's they hand the window, so even a message the
/// window draws as it is (a write's ending, a fetch's failure) carries no token.
pub fn scrubbed(text: &str) -> String {
    let mut scrubber = if text.len() >= MAY_BE_CUT_BYTES {
        Scrubber::after_cut()
    } else {
        Scrubber::new()
    };
    text.split('\n')
        .map(|line| scrubber.line(line))
        .collect::<Vec<_>>()
        .join("\n")
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
        let long = format!("er:SECRET@host/x\n{}", "y".repeat(MAY_BE_CUT_BYTES));
        assert!(shown_lines(&long)[0] == "host/x");
        let mut streamed = ShownLines::default();
        assert_eq!(streamed.line("to https://tok"), "to https://");
        assert_eq!(streamed.line("en@host/r"), "host/r");
    }
}
