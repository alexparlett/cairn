//! Credentials in a URL removed from text before it is drawn (staging-and-commit R12.2; #46,
//! for display). git can quote a remote's URL in what it writes to stderr, and a URL configured
//! with userinfo — `https://user:token@host/repo.git` — carries a credential in it that is not a
//! [`crate::Secret`]. Every stderr line a view draws goes through a [`Scrubber`] first.
//!
//! The rule only removes, never classifies: in every `scheme://` URL, everything between the
//! `//` and the last `@` before the authority ends — at whitespace, `/`, `?`, `#`, `<`, `>` or
//! the end of the line, never at a quote, which a userinfo may hold — is removed, the `@` with it, so `https://user:token@host/x`
//! reads `https://host/x`. That covers http(s), `ssh://user:pass@host` and any other scheme
//! (`git+ssh://`, `ftp://`). An scp-like address (`git@host:path`) is no URL and carries no
//! password, so it is left as git wrote it.
//!
//! A line can end inside a URL's authority — git's stderr reader sends a line longer than its
//! tail in pieces, and a retained tail can begin part-way through a line — so a scrubber
//! carries across lines what the last line ended in. A line ending in a scheme, or a scheme and
//! part of its `://`, lets the next line's start complete the `://` and begin an authority. A
//! line ending inside an authority it had not seen an `@` in has that unfinished authority
//! removed where it ends (a host cut there is removed too, since it cannot be told from a user
//! name), and the next line's leading run up to its last `@` — before any character that ends
//! an authority — is removed as the rest of it. A text known to start part-way through a line
//! ([`Scrubber::after_cut`]) is read as if the line before had ended in either.

/// Scrubs a sequence of lines, in order, carrying a URL cut at a line's end over to the next
/// line.
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
    /// A URL's authority, with no `@` seen yet: the next line's leading run up to an `@` is the
    /// rest of a userinfo.
    Authority,
    /// Text that may start anywhere in a line ([`Scrubber::after_cut`]): both of the above.
    Cut,
}

const SEPARATOR: &str = "://";

impl Scrubber {
    /// A scrubber for text that starts at a line's start.
    pub fn new() -> Self {
        Self::default()
    }

    /// A scrubber for text that may start part-way through a line — a tail cut from the
    /// front: its first line may begin inside a URL, anywhere from its `://` to its `@`.
    pub fn after_cut() -> Self {
        Self {
            carried: Carried::Cut,
        }
    }

    /// `line` with the userinfo of every URL in it removed.
    pub fn line(&mut self, line: &str) -> String {
        let mut shown = String::with_capacity(line.len());
        let mut rest = line;
        let carried = std::mem::take(&mut self.carried);
        // The rest of a `://` a cut separated from its scheme: what follows is an authority.
        let completes = |seen: usize| SEPARATOR[seen..].len();
        let separator_rest = match carried {
            Carried::Scheme { seen } if rest.starts_with(&SEPARATOR[seen..]) => {
                Some(completes(seen))
            }
            Carried::Cut => (0..SEPARATOR.len())
                .find(|seen| rest.starts_with(&SEPARATOR[*seen..]))
                .map(completes),
            Carried::Nothing | Carried::Scheme { .. } | Carried::Authority => None,
        };
        if let Some(length) = separator_rest {
            shown.push_str(&rest[..length]);
            match self.authority(&rest[length..], &mut shown) {
                Some(after) => rest = after,
                None => return shown,
            }
        } else if matches!(carried, Carried::Authority | Carried::Cut) {
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
    /// what follows it, or `None` when the line ends inside it before any `@` — removed, and
    /// carried to the next line.
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

/// `text`'s lines, each scrubbed, joined by `\n` — for a text read whole from its first line.
pub fn scrub_userinfo(text: &str) -> String {
    let mut scrubber = Scrubber::new();
    text.split('\n')
        .map(|line| scrubber.line(line))
        .collect::<Vec<_>>()
        .join("\n")
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
        Scrubber::new().line(line)
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

    /// R12.2's split case: a URL whose authority a line's end cuts — a line sent in pieces —
    /// loses the userinfo on both sides of the cut. Caught by: lines scrubbed one at a time
    /// with nothing carried across.
    #[test]
    fn a_url_split_across_a_line_loses_its_userinfo_on_both_sides() {
        let mut scrubber = Scrubber::new();
        assert_eq!(
            scrubber.line("error: https://user:ghp_TO"),
            "error: https://"
        );
        assert_eq!(
            scrubber.line("KEN@github.com/o/r failed"),
            "github.com/o/r failed"
        );
        assert_eq!(
            scrubber.line("next https://plain/x"),
            "next https://plain/x"
        );
        // Cut before the `@` was reached, with no `@` on the next line: the host is kept there.
        let mut scrubber = Scrubber::new();
        assert_eq!(scrubber.line("see https://exam"), "see https://");
        assert_eq!(scrubber.line("ple.com/x"), "ple.com/x");
        // A tail cut from the front: its first line may begin inside a userinfo.
        let mut cut = Scrubber::after_cut();
        assert_eq!(
            cut.line("er:secret@host/r.git' denied"),
            "host/r.git' denied"
        );
        assert_eq!(
            cut.line("user:secret@x"),
            "user:secret@x",
            "only the first line"
        );
        let mut cut = Scrubber::after_cut();
        assert_eq!(cut.line("remote: done"), "remote: done");
    }

    /// Phase 11's QA (TC3): a line cut at any byte — inside the scheme, at or inside its `://`,
    /// in the userinfo, the host or after it — loses no userinfo on either side of the cut, and
    /// a text that starts at any byte of the line does not either. Caught by: a carry that waits
    /// for a whole `://` before it starts.
    #[test]
    fn a_url_cut_at_any_byte_loses_its_userinfo() {
        let line = "error: 'https://user:tok@host/r' failed";
        for cut in 1..line.len() {
            let mut scrubber = Scrubber::new();
            let shown = format!(
                "{}{}",
                scrubber.line(&line[..cut]),
                scrubber.line(&line[cut..])
            );
            assert!(
                !shown.contains("tok") && !shown.contains("user"),
                "cut at {cut}: {shown:?}"
            );
            assert!(shown.contains("host/r' failed"), "cut at {cut}: {shown:?}");
            let from = Scrubber::after_cut().line(&line[cut..]);
            assert!(
                !from.contains("tok") && !from.contains("user"),
                "from {cut}: {from:?}"
            );
        }
        let mut scrubber = Scrubber::new();
        assert_eq!(scrubber.line("see https:"), "see https:");
        assert_eq!(scrubber.line("//u:p@h/x"), "//h/x");
        assert_eq!(
            Scrubber::after_cut().line("://u:p@h/x"),
            "://h/x",
            "a text that starts at its `://`"
        );
        assert_eq!(
            Scrubber::after_cut().line("/home/ada@x is mine"),
            "/home/ada@x is mine"
        );
        let mut scrubber = Scrubber::new();
        assert_eq!(scrubber.line("it failed"), "it failed");
        assert_eq!(
            scrubber.line("// a comment, ada@example.com"),
            "// a comment, ada@example.com",
            "a line after a word is no authority unless it starts with `://`'s rest"
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

    /// The text form reads each line in order with one scrubber.
    #[test]
    fn a_text_is_scrubbed_line_by_line_with_the_cut_carried() {
        assert_eq!(
            scrub_userinfo("a https://u:p@h/x\nb https://u:\np@h/y\nc"),
            "a https://h/x\nb https://\nh/y\nc"
        );
    }

    /// `://` with no scheme before it, and a scheme starting with a digit, are not read as a
    /// URL's start.
    #[test]
    fn only_a_scheme_starts_a_url() {
        assert_eq!(scrubbed("odd ://user@host"), "odd ://user@host");
        assert_eq!(scrubbed("9http://u:p@h/x"), "9http://h/x");
        assert_eq!(scrubbed("ünicode https://ü:p@h/x"), "ünicode https://h/x");
    }
}
