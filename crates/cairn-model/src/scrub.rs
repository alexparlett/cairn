//! Credentials in a URL removed from git's text before anything keeps or draws it
//! (staging-and-commit R12.2, R4.10; #46). git can quote a remote's URL in what it writes, and a
//! URL configured with userinfo — `https://user:token@host/repo.git` — carries a credential in it
//! that is not a [`crate::Secret`].
//!
//! **Scrubbed once, as whole lines.** The engine's runner splits git's output into whole lines
//! and hands each to a [`Scrubber`] as it is split, before anything is cut or kept; what it keeps
//! and hands on is a [`ScrubbedLine`] or [`ScrubbedLines`], which only a scrubber's output can
//! fill. So git's text crosses into the application only scrubbed, and no view scrubs again.
//!
//! The rule only removes, never classifies: in every `scheme://` URL, everything between the
//! `//` and the last `@` before the authority ends — at whitespace, `/`, `?`, `#`, `<`, `>` or
//! the end of the line, never at a quote, which a userinfo may hold — is removed, the `@` with it, so `https://user:token@host/x`
//! reads `https://host/x`. That covers http(s), `ssh://user:pass@host` and any other scheme
//! (`git+ssh://`, `ftp://`). An scp-like address (`git@host:path`) is no URL and carries no
//! password, so it is left as git wrote it.
//!
//! A line longer than the runner's piece limit is handed on in pieces, and a piece can end
//! inside a URL's authority, so a scrubber carries across the pieces of one line what the last
//! piece ended in ([`Scrubber::piece`]); a whole line ([`Scrubber::line`]) carries nothing to the
//! next. A piece ending in a scheme, or a scheme and part of its `://`, lets the next piece's
//! start complete the `://` and begin an authority. A piece ending inside an authority it had not
//! seen an `@` in has that unfinished authority removed where it ends (a host cut there is
//! removed too, since it cannot be told from a user name), and the next piece's leading run up to
//! its last `@` — before any character that ends an authority — is removed as the rest of it.

/// Scrubs git's lines, in order: each whole line alone, and the pieces of a line longer than one
/// piece with a URL cut at a piece's end carried over to the next.
#[derive(Debug, Clone, Default)]
pub struct Scrubber {
    carried: Carried,
}

/// What the line before ended in, that the next line's start may complete.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Carried {
    #[default]
    Nothing,
    /// A scheme and this much of its `://` — none of it, `:` or `:/`: a next line beginning with
    /// the rest of `://` begins an authority.
    Scheme { seen: usize },
    /// A URL's authority, with no `@` seen yet: the next piece's leading run up to an `@` is the
    /// rest of a userinfo.
    Authority,
}

const SEPARATOR: &str = "://";

impl Scrubber {
    /// A scrubber for text that starts at a line's start.
    pub fn new() -> Self {
        Self::default()
    }

    /// A whole line — or the last piece of one longer than a piece — with the userinfo of every
    /// URL in it removed. Nothing is carried to the next line: git writes a URL whole on one.
    pub fn line(&mut self, line: &str) -> ScrubbedLine {
        let shown = self.scrubbed(line);
        self.carried = Carried::Nothing;
        ScrubbedLine(shown)
    }

    /// A piece of a line longer than one piece, with the userinfo of every URL in it removed;
    /// a URL its end cuts is carried to the next piece, which [`Scrubber::piece`] or
    /// [`Scrubber::line`] reads next.
    pub fn piece(&mut self, piece: &str) -> ScrubbedLine {
        ScrubbedLine(self.scrubbed(piece))
    }

    fn scrubbed(&mut self, line: &str) -> String {
        let mut shown = String::with_capacity(line.len());
        let mut rest = line;
        let carried = std::mem::take(&mut self.carried);
        // The rest of a `://` a cut separated from its scheme: what follows is an authority.
        let completes = |seen: usize| SEPARATOR[seen..].len();
        let separator_rest = match carried {
            Carried::Scheme { seen } if rest.starts_with(&SEPARATOR[seen..]) => {
                Some(completes(seen))
            }
            Carried::Nothing | Carried::Scheme { .. } | Carried::Authority => None,
        };
        if let Some(length) = separator_rest {
            shown.push_str(&rest[..length]);
            match self.authority(&rest[length..], &mut shown) {
                Some(after) => rest = after,
                None => return shown,
            }
        } else if carried == Carried::Authority {
            // The rest of a userinfo a cut separated from its URL: up to its last `@`.
            let run = authority_end(rest);
            if let Some(at) = rest[..run].rfind('@') {
                rest = &rest[at + 1..];
            }
        }
        while let Some(found) = rest.find(SEPARATOR) {
            let scheme_start = scheme_start(&rest[..found]);
            shown.push_str(&rest[..found + SEPARATOR.len()]);
            let after = &rest[found + SEPARATOR.len()..];
            if scheme_start == found {
                // `://` with no scheme before it: not a URL; keep going past it.
                rest = after;
                continue;
            }
            match self.authority(after, &mut shown) {
                Some(after) => rest = after,
                None => return shown,
            }
        }
        // A scheme, or a scheme and part of its `://`, at the line's end: the next line may
        // complete it.
        for seen in [2, 1, 0] {
            if let Some(before) = rest.strip_suffix(&SEPARATOR[..seen])
                && scheme_start(before) < before.len()
            {
                self.carried = Carried::Scheme { seen };
                break;
            }
        }
        shown.push_str(rest);
        shown
    }

    /// The authority at `text`'s start, its userinfo removed and its host copied to `shown`;
    /// what follows it, or `None` when the text ends inside it before any `@` — removed, and
    /// carried to the next piece.
    fn authority<'a>(&mut self, text: &'a str, shown: &mut String) -> Option<&'a str> {
        let end = authority_end(text);
        if end == text.len() && !text.contains('@') {
            self.carried = Carried::Authority;
            return None;
        }
        let rest = match text[..end].rfind('@') {
            Some(at) => &text[at + 1..],
            None => text,
        };
        // Copy the host on, so a `://` inside it is not read again as a scheme.
        let host = authority_end(rest);
        shown.push_str(&rest[..host]);
        Some(&rest[host..])
    }
}

/// One line of git's — or one piece of a line longer than the runner's piece limit — with the
/// userinfo of every URL in it removed. Only a [`Scrubber`] makes one.
///
/// ```compile_fail
/// // No literal: the field is private.
/// let line = cairn_model::ScrubbedLine(String::from("https://u:p@h"));
/// ```
///
/// ```compile_fail
/// // No conversion from text.
/// let line: cairn_model::ScrubbedLine = String::from("https://u:p@h").into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrubbedLine(String);

impl ScrubbedLine {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ScrubbedLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// git's text as whole lines, each scrubbed of a URL's userinfo as it was added — what the
/// engine hands the application of anything git wrote (R4.10): the lines one read of a pipe
/// completed, a failure's retained output, a command log record's. A line holds no `\n`.
/// Lines are added only as [`ScrubbedLine`]s, or scrubbed on the way in ([`Self::scrubbing`]),
/// so nothing in it was not scrubbed. Bounded by its holder through [`Self::keep_last`], which
/// lets go of whole lines, oldest first, and says so ([`Self::older_dropped`]) — never a cut
/// inside a line.
///
/// ```compile_fail
/// // No literal: the fields are private.
/// let lines = cairn_model::ScrubbedLines { text: String::new(), lines: 0, older_dropped: false };
/// ```
///
/// ```compile_fail
/// // A line is pushed only as a scrubber's output, never as text.
/// let mut lines = cairn_model::ScrubbedLines::default();
/// lines.push(String::from("https://u:p@h"));
/// ```
///
/// ```
/// // The passing scaffold the two refusals above each change one line of.
/// let mut lines = cairn_model::ScrubbedLines::default();
/// lines.push(cairn_model::Scrubber::new().line("https://u:p@h/x"));
/// assert_eq!(lines.text(), "https://h/x");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScrubbedLines {
    /// The lines, joined by `\n`.
    text: String,
    /// How many lines `text` holds: one empty line and none read alike.
    lines: usize,
    older_dropped: bool,
}

impl ScrubbedLines {
    pub fn new() -> Self {
        Self::default()
    }

    /// `text`'s lines — split at `\n` — each scrubbed whole, for git's text read whole: a
    /// message the engine formats from what git said, or a test's.
    pub fn scrubbing(text: &str) -> Self {
        let mut lines = Self::new();
        let mut scrubber = Scrubber::new();
        for line in text.split('\n') {
            lines.push(scrubber.line(line));
        }
        lines
    }

    /// One line more, after the rest.
    pub fn push(&mut self, line: ScrubbedLine) {
        if self.lines > 0 {
            self.text.push('\n');
        }
        self.text.push_str(&line.0);
        self.lines += 1;
    }

    /// `other`'s lines after these.
    pub fn append(&mut self, other: &ScrubbedLines) {
        if other.lines == 0 {
            return;
        }
        if self.lines > 0 {
            self.text.push('\n');
        }
        self.text.push_str(&other.text);
        self.lines += other.lines;
    }

    /// The lines, oldest first.
    pub fn lines(&self) -> impl DoubleEndedIterator<Item = &str> {
        let text = if self.lines == 0 {
            None
        } else {
            Some(&self.text)
        };
        text.into_iter().flat_map(|text| text.split('\n'))
    }

    /// The lines a person reads: each with its trailing whitespace trimmed, the blank ones left
    /// out — what a progress line or a streamed output draws.
    pub fn spoken(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty())
    }

    /// The lines joined by `\n`.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.lines == 0
    }

    /// How many lines it holds — not its bytes, which [`Self::bytes`] counts.
    pub fn line_count(&self) -> usize {
        self.lines
    }

    /// The bytes of its text.
    pub fn bytes(&self) -> usize {
        self.text.len()
    }

    /// Older lines were let go of to keep it bounded: what it holds starts at a line's start,
    /// after lines that are not kept.
    pub fn older_dropped(&self) -> bool {
        self.older_dropped
    }

    /// Lets go of whole lines, oldest first, until its text is at most `bytes` long. A line is
    /// never cut: one longer than `bytes` is let go of whole, the newest too.
    pub fn keep_last(&mut self, bytes: usize) {
        let mut start = 0;
        let mut dropped = 0;
        while self.text.len() - start > bytes && dropped < self.lines {
            start = match self.text[start..].find('\n') {
                Some(end) => start + end + 1,
                None => self.text.len(),
            };
            dropped += 1;
        }
        if dropped > 0 {
            self.text.drain(..start);
            self.lines -= dropped;
            self.older_dropped = true;
        }
    }

    /// Lets go of whole lines, oldest first, until at most `count` are left.
    pub fn keep_last_lines(&mut self, count: usize) {
        if self.lines <= count {
            return;
        }
        let dropping = self.lines - count;
        let start = if count == 0 {
            self.text.len()
        } else {
            self.text
                .match_indices('\n')
                .nth(dropping - 1)
                .map_or(self.text.len(), |(at, _)| at + 1)
        };
        self.text.drain(..start);
        self.lines = count;
        self.older_dropped = true;
    }

    /// Lets go of trailing lines that are blank, and the trailing whitespace of the last line
    /// kept. Only removes, so what is left is as scrubbed as it was.
    pub fn trim_end(&mut self) {
        let kept = self.text.trim_end().len();
        self.text.truncate(kept);
        self.lines = if self.text.is_empty() {
            0
        } else {
            self.text.matches('\n').count() + 1
        };
    }
}

impl std::fmt::Display for ScrubbedLines {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// Read as its text, the lines joined by `\n`: what it holds is scrubbed, so reading it is free.
impl std::ops::Deref for ScrubbedLines {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text
    }
}

/// Compared with text by its text alone.
impl PartialEq<str> for ScrubbedLines {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for ScrubbedLines {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<String> for ScrubbedLines {
    fn eq(&self, other: &String) -> bool {
        &self.text == other
    }
}

/// Where an authority beginning at `text`'s start ends: at the first character no authority
/// holds — whitespace, `/`, `?`, `#` or an angle bracket — or the end. A quote does not end it:
/// an apostrophe is legal in a userinfo (git splits a URL's credentials at `/`, `?` and `#`
/// alone), so a quote git puts around a URL is read into the host and copied on unchanged.
fn authority_end(text: &str) -> usize {
    text.find(|c: char| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '<' | '>'))
        .unwrap_or(text.len())
}

/// Where the scheme ending at `before`'s end starts: the run of letters, digits, `+`, `-` and
/// `.` before `://` whose first character is a letter (RFC 3986). `before.len()` when there is
/// none.
fn scheme_start(before: &str) -> usize {
    let run = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        .last()
        .map_or(before.len(), |(at, _)| at);
    // The scheme starts at its first letter: a leading digit or symbol is not part of it.
    before[run..]
        .char_indices()
        .find(|(_, c)| c.is_ascii_alphabetic())
        .map_or(before.len(), |(at, _)| run + at)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scrubbed(line: &str) -> String {
        Scrubber::new().line(line).into_string()
    }

    /// R12.2's first case: an http(s) URL's user and password are removed, and nothing else
    /// of the line. Caught by: the token drawn, or the host or path lost with it.
    #[test]
    fn an_https_urls_userinfo_is_removed_and_the_rest_kept() {
        assert_eq!(
            scrubbed("fatal: unable to access 'https://user:ghp_TOKEN@github.com/o/r.git/': 403"),
            "fatal: unable to access 'https://github.com/o/r.git/': 403"
        );
        assert_eq!(
            scrubbed("remote: see http://token@example.com:8080/x?y#z for details"),
            "remote: see http://example.com:8080/x?y#z for details"
        );
        assert_eq!(
            scrubbed("From https://github.com/o/r"),
            "From https://github.com/o/r",
            "a URL with no userinfo is left as it is"
        );
        // A password holding an unencoded `@`: everything to the last one in the authority.
        assert_eq!(scrubbed("https://user:p@ss@host/x"), "https://host/x");
        // Two URLs on one line, the first without userinfo.
        assert_eq!(
            scrubbed("To https://host/a and https://u:secret@host/b"),
            "To https://host/a and https://host/b"
        );
    }

    /// R12.2's ssh-style case: an `ssh://` URL's userinfo is removed as an http one's is, and
    /// an scp-like address, which is no URL and holds no password, is left. Caught by: a rule
    /// keyed on `http` alone.
    #[test]
    fn an_ssh_urls_userinfo_is_removed_and_an_scp_address_left() {
        assert_eq!(
            scrubbed("fatal: Could not read from ssh://git:hunter2@host:2222/repo.git"),
            "fatal: Could not read from ssh://host:2222/repo.git"
        );
        assert_eq!(
            scrubbed("pushing to git+ssh://deploy:k3y@example.org/srv/r"),
            "pushing to git+ssh://example.org/srv/r"
        );
        assert_eq!(
            scrubbed("Cloning into git@github.com:o/r.git"),
            "Cloning into git@github.com:o/r.git"
        );
    }

    /// R12.2's split case: a URL whose authority a piece's end cuts — a line longer than the
    /// runner's piece limit — loses the userinfo on both sides of the cut, and a whole line
    /// carries nothing to the next. Caught by: pieces scrubbed one at a time with nothing
    /// carried, or a carry left over past a line's end.
    #[test]
    fn a_url_split_across_pieces_loses_its_userinfo_on_both_sides() {
        let mut scrubber = Scrubber::new();
        assert_eq!(
            scrubber.piece("error: https://user:ghp_TO").as_str(),
            "error: https://"
        );
        assert_eq!(
            scrubber.line("KEN@github.com/o/r failed").as_str(),
            "github.com/o/r failed"
        );
        assert_eq!(
            scrubber.line("next https://plain/x").as_str(),
            "next https://plain/x"
        );
        // Cut before the `@` was reached, with no `@` in the next piece: the host is kept there.
        let mut scrubber = Scrubber::new();
        assert_eq!(scrubber.piece("see https://exam").as_str(), "see https://");
        assert_eq!(scrubber.line("ple.com/x").as_str(), "ple.com/x");
        // A whole line ending inside an authority carries nothing: the next line is its own.
        let mut scrubber = Scrubber::new();
        assert_eq!(scrubber.line("see https://exam").as_str(), "see https://");
        assert_eq!(
            scrubber.line("ada:x@example.com wrote").as_str(),
            "ada:x@example.com wrote"
        );
    }

    /// Phase 11's QA (TC3), for pieces: a line cut into two pieces at any byte — inside the
    /// scheme, at or inside its `://`, in the userinfo, the host or after it — loses no
    /// userinfo on either side of the cut. Caught by: a carry that waits for a whole `://`
    /// before it starts.
    #[test]
    fn a_url_cut_at_any_byte_loses_its_userinfo() {
        let line = "error: 'https://user:tok@host/r' failed";
        for cut in 1..line.len() {
            let mut scrubber = Scrubber::new();
            let shown = format!(
                "{}{}",
                scrubber.piece(&line[..cut]),
                scrubber.line(&line[cut..])
            );
            assert!(
                !shown.contains("tok") && !shown.contains("user"),
                "cut at {cut}: {shown:?}"
            );
            assert!(shown.contains("host/r' failed"), "cut at {cut}: {shown:?}");
        }
        let mut scrubber = Scrubber::new();
        assert_eq!(scrubber.piece("see https:").as_str(), "see https:");
        assert_eq!(scrubber.line("//u:p@h/x").as_str(), "//h/x");
        let mut scrubber = Scrubber::new();
        assert_eq!(scrubber.piece("it failed").as_str(), "it failed");
        assert_eq!(
            scrubber.line("// a comment, ada@example.com").as_str(),
            "// a comment, ada@example.com",
            "a piece after a word is no authority unless it starts with `://`'s rest"
        );
    }

    /// Phase 11's QA (TC4): an apostrophe is legal in a userinfo, so it never ends one; a quote
    /// git puts around a URL is kept. Caught by: an authority ended at a quote, the token after
    /// it drawn.
    #[test]
    fn an_apostrophe_in_a_userinfo_does_not_end_it() {
        assert_eq!(
            scrubbed("'https://user:to'ken@host/r/'"),
            "'https://host/r/'"
        );
        assert_eq!(scrubbed("'https://u:p@host'"), "'https://host'");
        assert_eq!(scrubbed("\"https://u:p\"q@host\" x"), "\"https://host\" x");
        assert_eq!(
            scrubbed("'https://host/r' denied"),
            "'https://host/r' denied"
        );
    }

    /// A text read whole is scrubbed line by line, each line its own.
    #[test]
    fn a_text_is_scrubbed_line_by_line() {
        let lines = ScrubbedLines::scrubbing("a https://u:p@h/x\nb https://u:p@h/y\n\nc");
        assert_eq!(lines.text(), "a https://h/x\nb https://h/y\n\nc");
        assert_eq!(lines.line_count(), 4);
        assert_eq!(
            lines.lines().collect::<Vec<_>>(),
            ["a https://h/x", "b https://h/y", "", "c"]
        );
        assert!(!lines.older_dropped());
        assert_eq!(lines.to_string(), lines.text());
    }

    /// `://` with no scheme before it, and a scheme starting with a digit, are not read as a
    /// URL's start.
    #[test]
    fn only_a_scheme_starts_a_url() {
        assert_eq!(scrubbed("odd ://user@host"), "odd ://user@host");
        assert_eq!(scrubbed("9http://u:p@h/x"), "9http://h/x");
        assert_eq!(scrubbed("ünicode https://ü:p@h/x"), "ünicode https://h/x");
    }

    /// R4.10's tail: bounded by letting go of whole lines, oldest first, and saying so with a
    /// plain fact — never a cut inside a line, never the newest line let go of. Caught by: a
    /// byte cut, the head kept instead of the end, or a dropped line not said.
    #[test]
    fn keeping_the_last_lets_go_of_whole_oldest_lines_and_says_so() {
        let mut lines = ScrubbedLines::new();
        let mut scrubber = Scrubber::new();
        for n in 0..10 {
            lines.push(scrubber.line(&format!("line {n}")));
        }
        lines.keep_last(20);
        assert_eq!(lines.text(), "line 7\nline 8\nline 9");
        assert_eq!(lines.line_count(), 3);
        assert!(lines.older_dropped());
        let mut short = ScrubbedLines::scrubbing("all of it");
        short.keep_last(1024);
        assert!(!short.older_dropped());
        let mut long = ScrubbedLines::scrubbing(&format!("first\n{}", "é".repeat(100)));
        long.keep_last(200);
        assert_eq!(long.text(), "é".repeat(100), "the newest line kept whole");
        assert!(long.older_dropped());
        long.keep_last(10);
        assert!(
            long.is_empty(),
            "a line longer than the bound let go of whole, never cut"
        );
        assert!(long.older_dropped());
        assert_eq!(long.lines().count(), 0);
        let mut empty = ScrubbedLines::new();
        empty.keep_last(0);
        assert!(empty.is_empty() && !empty.older_dropped());
        assert_eq!(empty.lines().count(), 0);
    }

    /// Bounded by count as by bytes, and what a person reads of it leaves blank lines out.
    #[test]
    fn keeping_the_last_lines_and_reading_what_is_spoken() {
        let mut lines = ScrubbedLines::scrubbing("a\n\nb  \nc\nd");
        assert_eq!(lines.spoken().collect::<Vec<_>>(), ["a", "b", "c", "d"]);
        lines.keep_last_lines(2);
        assert_eq!((lines.text(), lines.line_count()), ("c\nd", 2));
        assert!(lines.older_dropped());
        let mut kept = ScrubbedLines::scrubbing("a\nb");
        kept.keep_last_lines(2);
        assert!(!kept.older_dropped());
        kept.keep_last_lines(0);
        assert!(kept.is_empty() && kept.older_dropped());
    }

    /// Trailing blank lines and whitespace go; one empty line and none are told apart until
    /// then. Caught by: a count left stale by the trim.
    #[test]
    fn trimming_the_end_lets_go_of_blank_lines() {
        let mut lines = ScrubbedLines::scrubbing("fatal: no  \n\n  ");
        assert_eq!(lines.line_count(), 3);
        lines.trim_end();
        assert_eq!((lines.text(), lines.line_count()), ("fatal: no", 1));
        let mut blank = ScrubbedLines::scrubbing("");
        assert_eq!(blank.line_count(), 1);
        blank.trim_end();
        assert!(blank.is_empty());
        let mut joined = ScrubbedLines::scrubbing("a");
        joined.append(&ScrubbedLines::scrubbing("b\nc"));
        joined.append(&ScrubbedLines::new());
        assert_eq!((joined.text(), joined.line_count()), ("a\nb\nc", 3));
    }
}
