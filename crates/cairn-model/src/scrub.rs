//! Credentials in a URL removed from text before it is drawn (staging-and-commit R12.2; #46,
//! for display). git can quote a remote's URL in what it writes to stderr, and a URL configured
//! with userinfo — `https://user:token@host/repo.git` — carries a credential in it that is not a
//! [`crate::Secret`]. Every stderr line a view draws goes through a [`Scrubber`] first.
//!
//! The rule only removes, never classifies: in every `scheme://` URL, everything between the
//! `//` and the last `@` before the authority ends — at whitespace, `/`, `?`, `#`, a quote, `<`,
//! `>` or the end of the line — is removed, the `@` with it, so `https://user:token@host/x`
//! reads `https://host/x`. That covers http(s), `ssh://user:pass@host` and any other scheme
//! (`git+ssh://`, `ftp://`). An scp-like address (`git@host:path`) is no URL and carries no
//! password, so it is left as git wrote it.
//!
//! A line can end inside a URL's authority — git's stderr reader sends a line longer than its
//! tail in pieces, and a retained tail can begin part-way through a line — so a scrubber keeps
//! one bit across lines: whether the last line ended in an authority it had not seen the end
//! of. That unfinished authority is removed where it ends (a host cut there is removed too,
//! since it cannot be told from a user name), and the next line's leading run up to its first
//! `@` — before any character that ends an authority — is removed as the rest of it. A text
//! known to start part-way through a line ([`Scrubber::after_cut`]) is read as if the line
//! before it had ended in an authority.

/// Scrubs a sequence of lines, in order, carrying an authority cut at a line's end over to the
/// next line.
#[derive(Debug, Clone, Default)]
pub struct Scrubber {
    /// The line before ended inside a URL's authority, with no `@` seen yet.
    in_authority: bool,
}

impl Scrubber {
    /// A scrubber for text that starts at a line's start.
    pub fn new() -> Self {
        Self::default()
    }

    /// A scrubber for text that may start part-way through a line — a tail cut from the
    /// front: its first line's leading run up to an `@` is taken for the end of a userinfo.
    pub fn after_cut() -> Self {
        Self { in_authority: true }
    }

    /// `line` with the userinfo of every URL in it removed.
    pub fn line(&mut self, line: &str) -> String {
        let mut shown = String::with_capacity(line.len());
        let mut rest = line;
        if self.in_authority {
            self.in_authority = false;
            let run = authority_end(rest);
            if let Some(at) = rest[..run].rfind('@') {
                rest = &rest[at + 1..];
            }
        }
        while let Some(found) = rest.find("://") {
            let scheme_start = scheme_start(&rest[..found]);
            if scheme_start == found {
                // `://` with no scheme before it: not a URL; keep going past it.
                shown.push_str(&rest[..found + 3]);
                rest = &rest[found + 3..];
                continue;
            }
            shown.push_str(&rest[..found + 3]);
            let authority = &rest[found + 3..];
            let end = authority_end(authority);
            if end == authority.len() && !authority[..end].contains('@') {
                // The line ends inside the authority: it is removed, and the next line's run up
                // to an `@` is read as its rest.
                self.in_authority = true;
                rest = "";
                break;
            }
            match authority[..end].rfind('@') {
                Some(at) => rest = &authority[at + 1..],
                None => rest = authority,
            }
            // Copy the host on, so a `://` inside it is not read again as a scheme.
            let host = authority_end(rest);
            shown.push_str(&rest[..host]);
            rest = &rest[host..];
        }
        shown.push_str(rest);
        shown
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
/// holds — whitespace, `/`, `?`, `#`, a quote or an angle bracket — or the end.
fn authority_end(text: &str) -> usize {
    text.find(|c: char| {
        c.is_whitespace() || matches!(c, '/' | '?' | '#' | '\'' | '"' | '`' | '<' | '>')
    })
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
