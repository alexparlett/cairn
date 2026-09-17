//! A reference applier for unified patches, written from the format.
//!
//! This exists to check the emitter, on both sides of the seam: `cairn-git`'s round-trip
//! tests compare what real `git apply` produces against what this produces, and a
//! disagreement means one of the two is wrong. So it is deliberately written from the
//! unified-diff format alone — it parses hunk headers and line markers and knows nothing
//! about [`crate::emit_patch`]'s internals. Sharing code between the two would make them
//! agree with each other whatever they both got wrong.
//!
//! It is strict on purpose: every line the patch claims is already there is checked against
//! the content, its newline included, and every hunk's counts are checked against the lines
//! it carries. An oracle that accepts anything decides nothing.
//!
//! It takes one file's patch, which is what [`crate::emit_patch`] produces. A hunk runs
//! until a line that is not one of its markers, so a second file's `--- a/…` line would be
//! read as a removal; the strictness that buys — a hunk carrying more lines than its header
//! promised is caught, not silently truncated — is worth more here than handling a patch
//! nothing in Cairn builds.

use std::fmt;

use crate::{DiffLine, split_lines};

/// What a patch got wrong, named by what the caller would have to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchApplyError {
    /// A line beginning `@@` that is not `@@ -a,b +c,d @@`.
    MalformedHunkHeader(String),
    /// A line inside a hunk that is not context, a removal, an addition or a marker.
    UnexpectedLine(String),
    /// A `\ No newline` marker with no line in front of it to belong to.
    MarkerWithoutALine,
    /// The content at `line` is not what the patch said was there, newline included.
    /// Counting from one.
    ContextMismatch { line: u32 },
    /// The hunk starts past the end of the content, or before the hunk before it.
    HunkOutOfOrder { line: u32 },
    /// The hunk's header promised one number of lines and carried another.
    WrongLineCount { expected: u32, found: u32 },
}

impl fmt::Display for PatchApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedHunkHeader(line) => write!(f, "malformed hunk header: {line}"),
            Self::UnexpectedLine(line) => write!(f, "unexpected line inside a hunk: {line}"),
            Self::MarkerWithoutALine => {
                f.write_str("a no-newline marker with no line in front of it")
            }
            Self::ContextMismatch { line } => {
                write!(
                    f,
                    "the content at line {line} is not what the patch expected"
                )
            }
            Self::HunkOutOfOrder { line } => {
                write!(f, "a hunk starting at line {line} is out of order")
            }
            Self::WrongLineCount { expected, found } => {
                write!(f, "a hunk promised {expected} lines and carried {found}")
            }
        }
    }
}

impl std::error::Error for PatchApplyError {}

/// Applies `patch` to `content`, giving what the patch says the result is.
pub fn apply_patch(content: &[u8], patch: &[u8]) -> Result<Vec<u8>, PatchApplyError> {
    Apply::new(content, patch, Direction::Forward).run()
}

/// Applies `patch` backwards, taking the result back to what it was made from — which is
/// what `git apply --reverse` does, and what R1.6's last sentence promises.
pub fn apply_patch_in_reverse(content: &[u8], patch: &[u8]) -> Result<Vec<u8>, PatchApplyError> {
    Apply::new(content, patch, Direction::Reverse).run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Forward,
    Reverse,
}

impl Direction {
    /// The marker naming a line the patch expects the content to already hold.
    fn consumed(self) -> u8 {
        match self {
            Self::Forward => b'-',
            Self::Reverse => b'+',
        }
    }

    /// The marker naming a line the patch puts there.
    fn produced(self) -> u8 {
        match self {
            Self::Forward => b'+',
            Self::Reverse => b'-',
        }
    }
}

/// One hunk header's four numbers, as the header spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Header {
    old_start: u32,
    old_count: u32,
    new_start: u32,
    new_count: u32,
}

impl Header {
    /// Where the hunk sits in the content it is applied to, counting from zero. An empty
    /// range names the line before it, so its start is already the insertion point.
    fn at(self, direction: Direction) -> Option<usize> {
        let (start, count) = match direction {
            Direction::Forward => (self.old_start, self.old_count),
            Direction::Reverse => (self.new_start, self.new_count),
        };
        if count == 0 {
            return Some(start as usize);
        }
        start.checked_sub(1).map(|index| index as usize)
    }

    fn consuming(self, direction: Direction) -> u32 {
        match direction {
            Direction::Forward => self.old_count,
            Direction::Reverse => self.new_count,
        }
    }

    fn producing(self, direction: Direction) -> u32 {
        match direction {
            Direction::Forward => self.new_count,
            Direction::Reverse => self.old_count,
        }
    }
}

/// Where a patch line landed, so a `\ No newline` marker can reach back to it.
#[derive(Debug, Clone, Copy)]
struct Placed {
    /// Index in the output, for a line the patch produced or carried through.
    produced: Option<usize>,
    /// Index in the content, for a line the patch expected to already be there.
    consumed: Option<usize>,
    /// Whether a marker has said this line has no newline.
    marked: bool,
}

struct Apply<'a> {
    source: Vec<DiffLine>,
    lines: Vec<&'a [u8]>,
    direction: Direction,
    out: Vec<DiffLine>,
    /// How far into `source` the apply has read.
    cursor: usize,
}

impl<'a> Apply<'a> {
    fn new(content: &[u8], patch: &'a [u8], direction: Direction) -> Apply<'a> {
        Apply {
            source: split_lines(content),
            lines: patch_lines(patch),
            direction,
            out: Vec::new(),
            cursor: 0,
        }
    }

    fn run(mut self) -> Result<Vec<u8>, PatchApplyError> {
        let mut at = 0usize;
        while at < self.lines.len() {
            let Some(line) = self.lines.get(at).copied() else {
                break;
            };
            at += 1;
            if !line.starts_with(b"@@") {
                // Everything outside a hunk is a header: it names the file, its modes and
                // its blob ids, none of which change a byte of the content.
                continue;
            }

            let header = parse_header(line)?;
            let start = header
                .at(self.direction)
                .ok_or_else(|| PatchApplyError::MalformedHunkHeader(text_of(line)))?;
            if start < self.cursor || start > self.source.len() {
                return Err(PatchApplyError::HunkOutOfOrder {
                    line: count_from_one(start),
                });
            }
            let carried = self
                .source
                .get(self.cursor..start)
                .unwrap_or_default()
                .to_vec();
            self.out.extend(carried);
            self.cursor = start;

            self.hunk(&mut at, header)?;
        }

        let tail = self.source.get(self.cursor..).unwrap_or_default().to_vec();
        self.out.extend(tail);
        Ok(joined(&self.out))
    }

    /// Reads one hunk's lines, checking each against the content and building the result.
    fn hunk(&mut self, at: &mut usize, header: Header) -> Result<(), PatchApplyError> {
        let wanted_consumed = header.consuming(self.direction);
        let wanted_produced = header.producing(self.direction);
        let mut consumed = 0u32;
        let mut produced = 0u32;
        let mut pending: Option<Placed> = None;

        while let Some(line) = self.lines.get(*at).copied() {
            if line.starts_with(b"@@") {
                break;
            }
            let marker = line.first().copied();
            let rest = line.get(1..).unwrap_or_default();

            if marker == Some(b'\\') {
                let Some(placed) = pending.as_mut() else {
                    return Err(PatchApplyError::MarkerWithoutALine);
                };
                placed.marked = true;
                let placed = *placed;
                self.unterminate(placed)?;
                *at += 1;
                continue;
            }

            // git's own `normalize_marker` reads a bare newline as a context line.
            let (marker, rest) = match marker {
                None => (b' ', &[][..]),
                Some(marker) => (marker, rest),
            };

            if marker == b' ' {
                self.settle(pending.take())?;
                let at_source = self.cursor;
                self.take(rest)?;
                self.out.push(DiffLine::terminated(rest));
                consumed += 1;
                produced += 1;
                pending = Some(Placed {
                    produced: Some(self.out.len().saturating_sub(1)),
                    consumed: Some(at_source),
                    marked: false,
                });
            } else if marker == self.direction.consumed() {
                self.settle(pending.take())?;
                let at_source = self.cursor;
                self.take(rest)?;
                consumed += 1;
                pending = Some(Placed {
                    produced: None,
                    consumed: Some(at_source),
                    marked: false,
                });
            } else if marker == self.direction.produced() {
                self.settle(pending.take())?;
                self.out.push(DiffLine::terminated(rest));
                produced += 1;
                pending = Some(Placed {
                    produced: Some(self.out.len().saturating_sub(1)),
                    consumed: None,
                    marked: false,
                });
            } else if consumed >= wanted_consumed && produced >= wanted_produced {
                // The hunk is complete; this line belongs to whatever comes next.
                break;
            } else {
                return Err(PatchApplyError::UnexpectedLine(text_of(line)));
            }
            *at += 1;
        }

        self.settle(pending.take())?;
        if consumed != wanted_consumed {
            return Err(PatchApplyError::WrongLineCount {
                expected: wanted_consumed,
                found: consumed,
            });
        }
        if produced != wanted_produced {
            return Err(PatchApplyError::WrongLineCount {
                expected: wanted_produced,
                found: produced,
            });
        }
        Ok(())
    }

    /// A line with no marker after it claimed the content's line ends in a newline. That
    /// has to be true, or a patch made for a file that ends cleanly would apply to one
    /// that does not.
    fn settle(&self, placed: Option<Placed>) -> Result<(), PatchApplyError> {
        let Some(placed) = placed else {
            return Ok(());
        };
        if placed.marked {
            return Ok(());
        }
        let Some(at) = placed.consumed else {
            return Ok(());
        };
        match self.source.get(at) {
            Some(line) if line.ends_with_newline() => Ok(()),
            _ => Err(PatchApplyError::ContextMismatch {
                line: count_from_one(at),
            }),
        }
    }

    /// A `\ No newline` marker: on the side the patch is applied to it has to match what
    /// is there; on the side it produces it is what gets written.
    fn unterminate(&mut self, placed: Placed) -> Result<(), PatchApplyError> {
        if let Some(at) = placed.consumed {
            match self.source.get(at) {
                Some(line) if !line.ends_with_newline() => {}
                _ => {
                    return Err(PatchApplyError::ContextMismatch {
                        line: count_from_one(at),
                    });
                }
            }
        }
        if let Some(at) = placed.produced {
            let line = self
                .out
                .get(at)
                .ok_or(PatchApplyError::MarkerWithoutALine)?
                .bytes()
                .to_vec();
            if let Some(slot) = self.out.get_mut(at) {
                *slot = DiffLine::unterminated(line);
            }
        }
        Ok(())
    }

    /// Checks that the content really holds the line the patch says it does, and steps past
    /// it. The newline is settled separately, once it is known whether a marker follows.
    fn take(&mut self, expected: &[u8]) -> Result<(), PatchApplyError> {
        let at = self.cursor;
        let line = self
            .source
            .get(at)
            .ok_or(PatchApplyError::ContextMismatch {
                line: count_from_one(at),
            })?;
        if line.bytes() != expected {
            return Err(PatchApplyError::ContextMismatch {
                line: count_from_one(at),
            });
        }
        self.cursor = at.saturating_add(1);
        Ok(())
    }
}

fn joined(lines: &[DiffLine]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in lines {
        bytes.extend_from_slice(line.bytes());
        if line.ends_with_newline() {
            bytes.push(b'\n');
        }
    }
    bytes
}

fn count_from_one(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1)
}

/// Splits the patch into lines, dropping the newline that ends each one. A patch always
/// terminates its lines; a marker line is what says the file's own line did not.
fn patch_lines(patch: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut rest = patch;
    while let Some(at) = rest.iter().position(|byte| *byte == b'\n') {
        lines.push(rest.get(..at).unwrap_or_default());
        rest = rest.get(at + 1..).unwrap_or_default();
    }
    if !rest.is_empty() {
        lines.push(rest);
    }
    lines
}

fn text_of(line: &[u8]) -> String {
    String::from_utf8_lossy(line).into_owned()
}

fn parse_header(line: &[u8]) -> Result<Header, PatchApplyError> {
    let malformed = || PatchApplyError::MalformedHunkHeader(text_of(line));
    let mut at = 0usize;
    expect(line, &mut at, b"@@ -").ok_or_else(malformed)?;
    let old_start = number(line, &mut at).ok_or_else(malformed)?;
    let old_count = match expect(line, &mut at, b",") {
        Some(()) => number(line, &mut at).ok_or_else(malformed)?,
        None => 1,
    };
    expect(line, &mut at, b" +").ok_or_else(malformed)?;
    let new_start = number(line, &mut at).ok_or_else(malformed)?;
    let new_count = match expect(line, &mut at, b",") {
        Some(()) => number(line, &mut at).ok_or_else(malformed)?,
        None => 1,
    };
    expect(line, &mut at, b" @@").ok_or_else(malformed)?;
    Ok(Header {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

fn expect(line: &[u8], at: &mut usize, want: &[u8]) -> Option<()> {
    let end = at.checked_add(want.len())?;
    if line.get(*at..end)? == want {
        *at = end;
        Some(())
    } else {
        None
    }
}

fn number(line: &[u8], at: &mut usize) -> Option<u32> {
    let start = *at;
    let mut value: u32 = 0;
    while let Some(&digit) = line.get(*at).filter(|byte| byte.is_ascii_digit()) {
        value = value
            .checked_mul(10)?
            .checked_add(u32::from(digit.saturating_sub(b'0')))?;
        *at = at.checked_add(1)?;
    }
    if *at == start { None } else { Some(value) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EDIT: &[u8] = b"diff --git a/f.txt b/f.txt\n\
         --- a/f.txt\n\
         +++ b/f.txt\n\
         @@ -1,3 +1,3 @@\n\
         \x20a\n\
         -b\n\
         +B\n\
         \x20c\n";

    #[test]
    fn a_patch_applied_to_what_it_was_made_from_gives_what_it_says() {
        assert_eq!(apply_patch(b"a\nb\nc\n", EDIT), Ok(b"a\nB\nc\n".to_vec()));
    }

    /// The property R1.6 promises: the same patch backwards takes the result back.
    #[test]
    fn the_same_patch_backwards_undoes_it() {
        assert_eq!(
            apply_patch_in_reverse(b"a\nB\nc\n", EDIT),
            Ok(b"a\nb\nc\n".to_vec())
        );
    }

    /// An oracle that accepts anything decides nothing: the content a context line claims
    /// has to actually be there.
    #[test]
    fn content_that_is_not_what_the_patch_expected_is_refused() {
        assert_eq!(
            apply_patch(b"a\nX\nc\n", EDIT),
            Err(PatchApplyError::ContextMismatch { line: 2 })
        );
        assert_eq!(
            apply_patch(b"z\nb\nc\n", EDIT),
            Err(PatchApplyError::ContextMismatch { line: 1 })
        );
    }

    /// Caught by: taking the header's counts on trust, which is how a recount bug survives.
    #[test]
    fn a_header_that_does_not_match_its_lines_is_refused() {
        let lying_old = b"@@ -1,4 +1,3 @@\n a\n-b\n+B\n c\n";
        assert_eq!(
            apply_patch(b"a\nb\nc\n", lying_old),
            Err(PatchApplyError::WrongLineCount {
                expected: 4,
                found: 3
            })
        );

        let lying_new = b"@@ -1,3 +1,4 @@\n a\n-b\n+B\n c\n";
        assert_eq!(
            apply_patch(b"a\nb\nc\n", lying_new),
            Err(PatchApplyError::WrongLineCount {
                expected: 4,
                found: 3
            })
        );
    }

    #[test]
    fn a_hunk_that_reaches_backwards_or_past_the_end_is_refused() {
        let past = b"@@ -9,1 +9,1 @@\n-x\n+y\n";
        assert!(matches!(
            apply_patch(b"a\n", past),
            Err(PatchApplyError::HunkOutOfOrder { .. })
        ));

        let backwards = b"@@ -3 +3 @@\n-c\n+C\n@@ -1 +1 @@\n-a\n+A\n";
        assert!(matches!(
            apply_patch(b"a\nb\nc\n", backwards),
            Err(PatchApplyError::HunkOutOfOrder { .. })
        ));
    }

    #[test]
    fn a_malformed_header_is_refused_rather_than_guessed_at() {
        for header in [
            &b"@@ -1,3 1,3 @@\n a\n"[..],
            b"@@ +1,3 -1,3 @@\n a\n",
            b"@@ -x,3 +1,3 @@\n a\n",
            b"@@ -1,3 +1,3\n a\n",
        ] {
            assert!(
                matches!(
                    apply_patch(b"a\nb\nc\n", header),
                    Err(PatchApplyError::MalformedHunkHeader(_))
                ),
                "a header that says nothing definite was accepted"
            );
        }
    }

    /// The marker has to agree with the content it sits on, or a patch made for a file that
    /// ends cleanly would apply to one that does not.
    #[test]
    fn a_no_newline_marker_is_checked_against_the_content() {
        let patch = b"@@ -1 +1 @@\n-one\n\\ No newline at end of file\n+one\n";
        assert_eq!(apply_patch(b"one", patch), Ok(b"one\n".to_vec()));
        assert!(
            matches!(
                apply_patch(b"one\n", patch),
                Err(PatchApplyError::ContextMismatch { .. })
            ),
            "a marker was accepted over a line that does end in a newline"
        );
    }

    /// The other direction, which a lenient applier misses: no marker means the content's
    /// line must end in a newline.
    #[test]
    fn a_missing_marker_is_refused_over_a_line_that_never_ended() {
        let patch = b"@@ -1 +1 @@\n-one\n+two\n";
        assert!(
            matches!(
                apply_patch(b"one", patch),
                Err(PatchApplyError::ContextMismatch { .. })
            ),
            "a patch claiming a newline was applied to a file that has none"
        );
        assert_eq!(apply_patch(b"one\n", patch), Ok(b"two\n".to_vec()));
    }

    #[test]
    fn a_line_the_patch_leaves_without_a_newline_comes_out_without_one() {
        let patch = b"@@ -1 +1 @@\n-one\n+two\n\\ No newline at end of file\n";
        assert_eq!(apply_patch(b"one\n", patch), Ok(b"two".to_vec()));
        assert_eq!(
            apply_patch_in_reverse(b"two", patch),
            Ok(b"one\n".to_vec()),
            "the marker did not travel with its line when the patch was reversed"
        );
    }

    /// A marker on a context line says both sides lost their newline.
    #[test]
    fn a_marker_on_a_context_line_carries_through_to_the_result() {
        let patch = b"@@ -1,2 +1,2 @@\n-a\n+A\n b\n\\ No newline at end of file\n";
        assert_eq!(apply_patch(b"a\nb", patch), Ok(b"A\nb".to_vec()));
    }

    #[test]
    fn a_marker_with_nothing_in_front_of_it_is_refused() {
        let patch = b"@@ -1 +1 @@\n\\ No newline at end of file\n-a\n+b\n";
        assert_eq!(
            apply_patch(b"a\n", patch),
            Err(PatchApplyError::MarkerWithoutALine)
        );
    }

    #[test]
    fn a_new_file_is_a_hunk_starting_at_nothing() {
        let patch = b"diff --git a/n b/n\nnew file mode 100644\n--- /dev/null\n+++ b/n\n@@ -0,0 +1,2 @@\n+x\n+y\n";
        assert_eq!(apply_patch(b"", patch), Ok(b"x\ny\n".to_vec()));
        assert_eq!(
            apply_patch_in_reverse(b"x\ny\n", patch),
            Ok(Vec::new()),
            "reversing a new file did not empty it"
        );
    }

    #[test]
    fn a_deleted_file_takes_every_line() {
        let patch = b"@@ -1,2 +0,0 @@\n-x\n-y\n";
        assert_eq!(apply_patch(b"x\ny\n", patch), Ok(Vec::new()));
    }

    #[test]
    fn two_hunks_apply_in_order_and_leave_what_is_between_them_alone() {
        let patch = b"@@ -1 +1 @@\n-a\n+A\n@@ -5 +5 @@\n-e\n+E\n";
        assert_eq!(
            apply_patch(b"a\nb\nc\nd\ne\n", patch),
            Ok(b"A\nb\nc\nd\nE\n".to_vec())
        );
    }

    #[test]
    fn a_line_inside_a_hunk_that_is_none_of_the_markers_is_refused() {
        let patch = b"@@ -1 +1 @@\n?a\n";
        assert!(matches!(
            apply_patch(b"a\n", patch),
            Err(PatchApplyError::UnexpectedLine(_))
        ));
    }

    /// A patch that changes nothing leaves the content exactly as it was, byte for byte,
    /// including a last line that never ended.
    #[test]
    fn a_patch_with_no_hunks_leaves_the_content_alone() {
        let headers = b"diff --git a/r b/r2\nsimilarity index 100%\nrename from r\nrename to r2\n";
        assert_eq!(apply_patch(b"x\ny", headers), Ok(b"x\ny".to_vec()));
    }

    /// git's `normalize_marker`: an empty line in a patch is a context line holding nothing.
    #[test]
    fn an_empty_patch_line_is_a_context_line() {
        let patch = b"@@ -1,2 +1,2 @@\n\n-b\n+B\n";
        assert_eq!(apply_patch(b"\nb\n", patch), Ok(b"\nB\n".to_vec()));
    }
}
