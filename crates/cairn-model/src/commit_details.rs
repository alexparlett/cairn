//! Everything the Commit tab shows about one commit (R1.8).
//!
//! Beside [`crate::CommitSummary`] rather than folded into it: the history list wants a
//! subject and one name, and paying for a committer, an offset and a whole message per row
//! of a ten-year monorepo would be paying for what nothing on that row draws.

use std::fmt;

use crate::Oid;

/// A moment as git records it: seconds since the epoch, and the offset the author's clock
/// was at. The offset is part of the record, not a rendering choice — two commits a second
/// apart in different places keep different offsets forever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    /// Seconds since the Unix epoch.
    pub seconds: i64,
    /// Seconds east of UTC. git writes it as `±HHMM`; see [`Timestamp::offset`].
    pub offset_seconds: i32,
}

impl Timestamp {
    pub fn new(seconds: i64, offset_seconds: i32) -> Self {
        Self {
            seconds,
            offset_seconds,
        }
    }

    /// The offset in git's own spelling, `+0530` or `-0800`. Always signed and always four
    /// digits, which is what `git log` shows and what a commit object carries.
    pub fn offset(self) -> String {
        let sign = if self.offset_seconds < 0 { '-' } else { '+' };
        let total = self.offset_seconds.unsigned_abs() / 60;
        let hours = total / 60;
        let minutes = total % 60;
        format!("{sign}{hours:02}{minutes:02}")
    }
}

/// Who did something, and when. git records an author and a committer separately, and a
/// rebase or a patch applied by hand makes them differ — which is the point of showing both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: String,
    pub email: String,
    pub time: Timestamp,
}

/// One commit, in the detail the Commit tab draws (R5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDetails {
    pub id: Oid,
    /// In git's order: the first parent is the one a commit's diff is taken against.
    pub parents: Vec<Oid>,
    pub author: Signature,
    pub committer: Signature,
    /// The whole message, not its subject.
    pub message: String,
}

impl CommitDetails {
    /// The first line of the message, which is what a title shows. git's `%s` folds a
    /// wrapped first line into one; this does not, because Cairn never re-wraps a message.
    pub fn subject(&self) -> &str {
        self.message
            .split('\n')
            .next()
            .unwrap_or_default()
            .trim_end_matches('\r')
    }

    /// The message past its subject and the blank line under it, or nothing when there is
    /// none. Leading blank lines are dropped; what follows is left exactly as written.
    pub fn body(&self) -> &str {
        let rest = match self.message.split_once('\n') {
            Some((_, rest)) => rest,
            None => return "",
        };
        rest.trim_start_matches(['\n', '\r'])
    }

    /// Whether this commit has more than one parent, which is what makes its diff a
    /// comparison against its first parent rather than against the only one it has.
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} <{}>", self.name, self.email)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details(message: &str, parents: Vec<Oid>) -> CommitDetails {
        let id = Oid::parse("1234567890abcdef1234567890abcdef12345678").expect("an id");
        let signature = |name: &str, offset| Signature {
            name: name.to_string(),
            email: format!("{name}@example.com"),
            time: Timestamp::new(1_700_000_000, offset),
        };
        CommitDetails {
            id,
            parents,
            author: signature("ada", 5 * 3600 + 30 * 60),
            committer: signature("grace", -8 * 3600),
            message: message.to_string(),
        }
    }

    /// The four digits git writes. Caught by: taking the offset for minutes, or losing the
    /// sign on a whole-hour offset west of UTC.
    #[test]
    fn an_offset_reads_the_way_git_writes_it() {
        for (seconds, expected) in [
            (0, "+0000"),
            (5 * 3600 + 30 * 60, "+0530"),
            (-8 * 3600, "-0800"),
            (-30 * 60, "-0030"),
            (14 * 3600, "+1400"),
            (45 * 60, "+0045"),
        ] {
            assert_eq!(
                Timestamp::new(0, seconds).offset(),
                expected,
                "{seconds} seconds east of UTC did not read as {expected}"
            );
        }
    }

    /// Caught by: reading the whole message where a title belongs, which puts a paragraph
    /// in a one-line header.
    #[test]
    fn a_subject_is_the_first_line_and_the_body_is_what_follows_the_blank_one() {
        let commit = details(
            "fix the thing\n\nBecause it was broken.\nTwice.\n",
            Vec::new(),
        );
        assert_eq!(commit.subject(), "fix the thing");
        assert_eq!(commit.body(), "Because it was broken.\nTwice.\n");
    }

    #[test]
    fn a_message_of_one_line_has_no_body() {
        assert_eq!(details("just this", Vec::new()).subject(), "just this");
        assert_eq!(details("just this", Vec::new()).body(), "");
        assert_eq!(details("just this\n", Vec::new()).subject(), "just this");
        assert_eq!(details("just this\n", Vec::new()).body(), "");
        assert_eq!(details("", Vec::new()).subject(), "");
        assert_eq!(details("", Vec::new()).body(), "");
    }

    /// A message written on Windows keeps its bytes; only the title drops the carriage
    /// return, because a gutter would draw it.
    #[test]
    fn a_carriage_return_does_not_reach_the_subject() {
        let commit = details("subject\r\n\r\nbody\r\n", Vec::new());
        assert_eq!(commit.subject(), "subject");
        assert_eq!(commit.body(), "body\r\n");
    }

    #[test]
    fn a_commit_with_two_parents_is_a_merge() {
        let one = Oid::parse("1111111111111111111111111111111111111111").expect("an id");
        let two = Oid::parse("2222222222222222222222222222222222222222").expect("an id");
        assert!(
            !details("x", Vec::new()).is_merge(),
            "a root commit read as a merge"
        );
        assert!(!details("x", vec![one]).is_merge());
        assert!(details("x", vec![one, two]).is_merge());
    }

    /// The author and the committer are two records, not one shown twice.
    #[test]
    fn the_author_and_the_committer_are_kept_apart() {
        let commit = details("x", Vec::new());
        assert_eq!(commit.author.to_string(), "ada <ada@example.com>");
        assert_eq!(commit.committer.to_string(), "grace <grace@example.com>");
        assert_eq!(commit.author.time.offset(), "+0530");
        assert_eq!(commit.committer.time.offset(), "-0800");
        assert_eq!(
            commit.author.time.seconds, commit.committer.time.seconds,
            "the two timestamps were not kept separately"
        );
    }
}
