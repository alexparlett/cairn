//! git's output as the window draws it (R10.5, staging-and-commit R4.10, R12.2): git's text
//! reaches the window only as `cairn_model::ScrubbedLines`, which the engine's runner fills as it
//! splits git's output into whole lines — each stripped of terminal control sequences
//! (`cairn_model::strip_ansi`) and then scrubbed of a URL's userinfo, before anything keeps or
//! cuts it: a commit's streamed output, a fetch's progress, a failure's kept output and the
//! command log's records alike — and an error's text as `cairn_git::Error::shown` renders it in
//! the worker. So no view, tooltip or dialog can draw a token a remote URL carried, and nothing
//! here scrubs. The window strips escapes again as it draws, which changes nothing the engine
//! already stripped and keeps a line drawn plain whoever built it.

use cairn_model::{ScrubbedLines, strip_ansi};

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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The phase 12 QA's first finding, drawn: an escape between a scheme and its `://` hid the
    /// URL from a scrubber that read the line before stripping it, and the strip here then drew
    /// the credential whole. Caught by: the engine scrubbing what git wrote before stripping it.
    #[test]
    fn an_escape_inside_a_url_draws_no_token() {
        for line in ["https\u{7}://u:TOKEN@h/x", "https:\u{1b}[0m//u:TOKEN@h/x"] {
            let lines = ScrubbedLines::scrubbing(line);
            for drawn in shown_lines(&lines).into_iter().chain(spoken_lines(&lines)) {
                assert!(!drawn.contains("TOKEN"), "{line:?} drawn as {drawn:?}");
                assert_eq!(drawn, "https://h/x");
            }
        }
    }
}
