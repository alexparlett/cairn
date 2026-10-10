//! The three pipes of an invocation, each on a thread of its own.
//!
//! A child that fills one pipe while its parent reads another waits forever:
//! on Linux once 64 KiB is buffered, or two pages once the user's pipe limit is
//! reached, and on macOS after as little as 512 bytes. So nothing here reads one
//! pipe on the thread that reads or writes another — stdout, stderr and stdin
//! each have their own thread, and the thread driving the invocation only
//! receives what the readers send it. Correct at any pipe capacity, because
//! no thread ever waits on a pipe it is not the sole user of.
//!
//! What crosses to the driving thread is an [`Event`] on a bounded channel,
//! so a caller slower than git holds git back rather than buffering without
//! limit, and a reader whose receiver has gone stops reading, which closes its
//! end of the pipe: a process still writing there gets `EPIPE` instead of
//! filling a pipe nobody empties.

use std::io::{self, Read as _};
use std::process::{ChildStderr, ChildStdout};
use std::sync::mpsc::SyncSender;

use cairn_model::{ScrubbedLines, Scrubber};

use super::group::Group;

/// What a reader hands the driving thread.
#[derive(Debug)]
pub(super) enum Event {
    /// Bytes from stdout, in order, as they arrived.
    Stdout(Vec<u8>),
    /// The lines of stderr one read completed, in order, each split whole by [`Lines`] and
    /// scrubbed of a URL's userinfo as it was split (staging-and-commit R4.10). One event per
    /// read, not per line, so what git wrote before it exited is a few events however many
    /// lines it was, and the driver's last look at the queue takes it all; one text, not a
    /// string per line, so an event costs one allocation of at most a read's bytes and a
    /// pending line piece, however short its lines.
    Lines(ScrubbedLines),
}

/// How many events may wait for the driving thread before a reader waits for
/// it: each is one allocation of at most a 64 KiB read — a stderr event also
/// a line piece of up to [`PIECE_BYTES`], and up to three bytes for each
/// undecodable one — so a few MiB of stdout queued at most, and some tens of
/// MiB of stderr in the worst case.
pub(super) const EVENTS_BOUND: usize = 64;

/// Bytes read from a pipe at a time: the most a Linux pipe holds by default.
const CHUNK: usize = 64 * 1024;

/// How much of git's output is retained for an error and the log: the last
/// 256 KiB, as whole lines. Lines before that are forwarded as progress and
/// dropped.
pub(super) const TAIL_BYTES: usize = 256 * 1024;

/// The longest piece of a line [`Lines`] hands on: a line longer than this —
/// one with no terminator for 256 KiB — is handed on in pieces of at most this
/// size, each ending at a character's end, so a stream with no terminator in it
/// cannot grow without bound here. No longer than the tail, so the tail always
/// holds the newest piece whole.
pub(super) const PIECE_BYTES: usize = TAIL_BYTES;

/// Reads stdout to its end, sending each chunk on. Ends at the end of the pipe,
/// on a read error, or when the receiver has gone.
pub(super) fn read_stdout(mut pipe: ChildStdout, events: &SyncSender<Event>, group: &Group) {
    let mut chunk = vec![0u8; CHUNK];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                if events.send(Event::Stdout(chunk[..read].to_vec())).is_err() {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    group.pipe_closed();
}

/// Reads stderr to its end, sending on the lines each read completes, split
/// and scrubbed by [`Lines`].
pub(super) fn read_stderr(mut pipe: ChildStderr, events: &SyncSender<Event>, group: &Group) {
    let mut lines = Lines::default();
    let mut chunk = vec![0u8; CHUNK];
    'reading: loop {
        let read = match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let mut complete = ScrubbedLines::new();
        lines.push(&chunk[..read], &mut complete);
        if !complete.is_empty() && events.send(Event::Lines(complete)).is_err() {
            break 'reading;
        }
    }
    let mut last = ScrubbedLines::new();
    lines.finish(&mut last);
    if !last.is_empty() {
        let _ = events.send(Event::Lines(last));
    }
    group.pipe_closed();
}

/// Writes `input` to stdin and closes it. git closing its end early is its
/// own choice and not an error; any other failure ends the process before the
/// pipe closes, because git would take what it had as the whole input.
pub(super) fn feed(mut pipe: impl io::Write, input: &[u8], group: &Group) {
    match pipe.write_all(input) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {}
        Err(error) => group.input_failed(error),
    }
    drop(pipe);
}

/// git's output split into lines across reads — the one line type, for stdout and stderr
/// alike (staging-and-commit R4.10, C31). A line ends at `\n` or at `\r`, on both streams: git's
/// progress meters redraw themselves with `\r`, and a hook writing one to stdout redraws the
/// same way, so each redraw is a line of its own whichever pipe carried it. A line is decoded
/// whole, so a character is never split across two reads; one longer than [`PIECE_BYTES`] is
/// handed on in pieces, each cut where a character ends, never inside one. Each line, and each
/// piece, is scrubbed of a URL's userinfo as it is split (R12.2), before anything keeps or cuts
/// it: the scrubber carries across the pieces of one line, and nothing across a line's end.
#[derive(Debug, Default)]
pub(super) struct Lines {
    pending: Vec<u8>,
    scrubber: Scrubber,
}

impl Lines {
    /// Every line `bytes` completes, and any piece of an over-long one, added to `out`.
    pub(super) fn push(&mut self, bytes: &[u8], out: &mut ScrubbedLines) {
        for &byte in bytes {
            if matches!(byte, b'\n' | b'\r') {
                out.push(self.scrubber.line(&String::from_utf8_lossy(&self.pending)));
                self.pending.clear();
            } else {
                self.pending.push(byte);
                if self.pending.len() >= PIECE_BYTES {
                    let whole = piece_end(&self.pending);
                    out.push(
                        self.scrubber
                            .piece(&String::from_utf8_lossy(&self.pending[..whole])),
                    );
                    self.pending.drain(..whole);
                }
            }
        }
    }

    /// The last line, when the stream ended without a terminator, added to `out`.
    pub(super) fn finish(mut self, out: &mut ScrubbedLines) {
        if !self.pending.is_empty() {
            out.push(self.scrubber.line(&String::from_utf8_lossy(&self.pending)));
        }
    }
}

/// Where a piece of `bytes` ends: at a character's end ([`whole_characters`]), and before a
/// terminal escape sequence the piece would cut open, so the scrubber, which strips escapes
/// before it reads a URL, sees each sequence whole — an escape split by a cut would otherwise
/// leave its tail in the next piece, between a scheme and its `://`. A sequence begun at the
/// piece's very start is cut where it is, since the piece must move on.
fn piece_end(bytes: &[u8]) -> usize {
    let whole = whole_characters(bytes);
    match bytes[..whole].iter().rposition(|byte| *byte == 0x1b) {
        Some(at) if at > 0 && !escape_complete(&bytes[at + 1..whole]) => at,
        Some(_) | None => whole,
    }
}

/// Whether the escape sequence after an `ESC` is whole in `after`: a CSI (`[`) ended by a byte
/// in `@`..=`~`, an OSC (`]`) by BEL or `ESC \`, any other by a byte past its intermediates
/// (` `..=`/`) — as `cairn_model::strip_ansi` reads them.
fn escape_complete(after: &[u8]) -> bool {
    match after.split_first() {
        None => false,
        Some((b'[', rest)) => rest.iter().any(|byte| (0x40..=0x7e).contains(byte)),
        Some((b']', rest)) => {
            rest.contains(&0x07) || rest.windows(2).any(|pair| pair == [0x1b, b'\\'])
        }
        Some((first, rest)) if (0x20..=0x2f).contains(first) => {
            rest.iter().any(|byte| !(0x20..=0x2f).contains(byte))
        }
        Some(_) => true,
    }
}

/// How many of `bytes` end at a character's end: all of them, unless they end inside a UTF-8
/// sequence, whose started bytes are left out to go with the next piece. Bytes that are no
/// UTF-8 at all are counted, to be read lossily as they always were.
fn whole_characters(bytes: &[u8]) -> usize {
    let length = bytes.len();
    for back in 1..=length.min(3) {
        let byte = bytes[length - back];
        if (byte & 0xC0) == 0x80 {
            continue;
        }
        // A lead byte: how long its sequence is, and whether all of it is here.
        let needs = match byte {
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => 1,
        };
        return if back < needs { length - back } else { length };
    }
    length
}

/// The retained end of git's output: at most [`TAIL_BYTES`] of whole lines, the oldest let go
/// of first, already scrubbed. While the process runs it holds at most twice that plus the
/// lines being added, because it lets go in batches.
#[derive(Debug, Default)]
pub(super) struct Tail {
    lines: ScrubbedLines,
}

impl Tail {
    pub(super) fn push(&mut self, lines: &ScrubbedLines) {
        self.lines.append(lines);
        // Let go in batches, so retaining is linear in what was said, not quadratic.
        if self.lines.bytes() > 2 * TAIL_BYTES {
            self.lines.keep_last(TAIL_BYTES);
        }
    }

    /// How many bytes it holds now, for a test of what it holds while running.
    #[cfg(test)]
    fn held(&self) -> usize {
        self.lines.bytes()
    }

    /// The retained lines, at most [`TAIL_BYTES`], trailing whitespace trimmed, saying whether
    /// older lines were let go of.
    pub(super) fn into_lines(mut self) -> ScrubbedLines {
        self.lines.keep_last(TAIL_BYTES);
        self.lines.trim_end();
        self.lines
    }
}

/// The lines a person reads of `lines` — each with its trailing whitespace trimmed, the blank
/// ones left out — owned, for a test of what a progress callback was handed.
#[cfg(test)]
pub(crate) fn spoken(lines: &ScrubbedLines) -> Vec<String> {
    lines.spoken().map(str::to_owned).collect()
}

/// Splits stdout into NUL-terminated records across chunks, handing each to
/// the caller as soon as it is whole.
#[derive(Debug, Default)]
pub(super) struct Records {
    pending: Vec<u8>,
}

impl Records {
    pub(super) fn push(&mut self, mut chunk: &[u8], record: &mut impl FnMut(&[u8])) {
        while let Some(end) = chunk.iter().position(|byte| *byte == 0) {
            if self.pending.is_empty() {
                record(&chunk[..end]);
            } else {
                self.pending.extend_from_slice(&chunk[..end]);
                record(&self.pending);
                self.pending.clear();
            }
            chunk = &chunk[end + 1..];
        }
        self.pending.extend_from_slice(chunk);
    }

    /// The last record, when the output did not end with a NUL.
    pub(super) fn finish(self, record: &mut impl FnMut(&[u8])) {
        if !self.pending.is_empty() {
            record(&self.pending);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(chunks: &[&[u8]]) -> Vec<Vec<u8>> {
        let mut seen = Vec::new();
        let mut splitter = Records::default();
        for chunk in chunks {
            splitter.push(chunk, &mut |record| seen.push(record.to_vec()));
        }
        splitter.finish(&mut |record| seen.push(record.to_vec()));
        seen
    }

    /// What `reads` split into, read by read, and the last line on finish.
    fn split(reads: &[&[u8]]) -> Vec<String> {
        let mut lines = Lines::default();
        let mut out = ScrubbedLines::new();
        for read in reads {
            lines.push(read, &mut out);
        }
        lines.finish(&mut out);
        out.lines().map(str::to_owned).collect()
    }

    /// Caught by: a record split across two reads delivered as two, or a
    /// phantom empty record after the final NUL.
    #[test]
    fn a_record_split_across_chunks_arrives_whole_and_once() {
        assert_eq!(
            records(&[b"ab", b"c\0d", b"e\0", b"\0f"]),
            [b"abc".to_vec(), b"de".to_vec(), b"".to_vec(), b"f".to_vec()]
        );
        assert_eq!(records(&[b"a\0"]), [b"a".to_vec()]);
        assert!(records(&[b""]).is_empty());
    }

    /// The one line definition, pinned (R4.10): a line ends at `\n` or `\r`, on stdout and
    /// stderr alike, across reads, the terminator not kept. Caught by: splitting on one
    /// terminator only, or keeping the terminator.
    #[test]
    fn lines_end_at_either_terminator_across_reads() {
        assert_eq!(
            split(&[b"Receiving:  5", b"0%\rReceiving: 100%\nFrom x"]),
            ["Receiving:  50%", "Receiving: 100%", "From x"]
        );
        assert_eq!(split(&[b"a\r\nb\n"]), ["a", "", "b"]);
        assert!(split(&[b""]).is_empty());
    }

    /// Caught by: a line with no terminator growing without bound.
    #[test]
    fn a_line_without_a_terminator_is_sent_in_pieces_no_longer_than_the_limit() {
        let seen = split(&[&vec![b'x'; PIECE_BYTES * 2 + 3]]);
        assert_eq!(seen.len(), 3);
        assert!(seen[..2].iter().all(|line| line.len() == PIECE_BYTES));
        assert_eq!(seen[2].len(), 3);
    }

    /// C31, the live bug (R4.10): a line longer than the piece limit, with a multi-byte
    /// character straddling the limit — every offset of a two-, three- and four-byte one —
    /// arrives with the character whole, never as U+FFFD; so does one split across two reads.
    /// Caught by: a piece cut at the byte count and decoded lossily, or a read decoded alone.
    #[test]
    fn a_character_straddling_the_piece_limit_arrives_whole() {
        for character in ["é", "€", "😀"] {
            for before in 1..character.len() {
                let mut long = vec![b'x'; PIECE_BYTES - before];
                long.extend_from_slice(character.as_bytes());
                long.extend_from_slice(b"rest\n");
                for reads in [vec![&long[..]], long.chunks(4096).collect()] {
                    let seen = split(&reads).concat();
                    assert!(
                        !seen.contains('\u{FFFD}'),
                        "{character} cut {before} bytes in read as U+FFFD"
                    );
                    assert!(seen.ends_with(&format!("{character}rest")), "{before}");
                    assert_eq!(seen.len(), long.len() - 1);
                }
            }
            // A character split across two reads, far from the limit.
            let bytes = format!("a{character}b\n");
            let (first, second) = bytes.as_bytes().split_at(2);
            assert_eq!(split(&[first, second]), [format!("a{character}b")]);
        }
        assert_eq!(whole_characters("a€".as_bytes()), 4);
        assert_eq!(whole_characters(&"a€".as_bytes()[..3]), 1);
        assert_eq!(
            whole_characters(&[b'a', 0xFF]),
            2,
            "not UTF-8: counted as it is"
        );
    }

    /// C31: a URL's userinfo is removed as the line is split — across two reads, across the
    /// piece limit at every byte of the URL, and across a `\r` redraw — so nothing kept or
    /// handed on ever holds it. Caught by: a scrub after the cut, or none.
    #[test]
    fn a_urls_userinfo_never_survives_the_split() {
        assert_eq!(
            split(&[b"fatal: https://user:ghp_TO", b"KEN@host/r denied\n"]),
            ["fatal: https://host/r denied"]
        );
        assert_eq!(
            split(&[b"50%\rfrom https://u:SECRET@host/x\r100%\n"]),
            ["50%", "from https://host/x", "100%"]
        );
        let url = "https://user:SECRET@host/r";
        for at in 0..url.len() {
            let mut long = vec![b'x'; PIECE_BYTES - at];
            long.extend_from_slice(url.as_bytes());
            long.push(b'\n');
            let seen = split(&[&long]).concat();
            assert!(
                !seen.contains("SECRET") && !seen.contains("user:"),
                "cut {at} bytes into the URL"
            );
            assert!(seen.ends_with("/r"), "cut {at}: the path lost");
        }
        // A password holding an `@`, cut at every byte by the piece limit (the phase 12 QA).
        let url = "https://u:SEC@RET@host/y";
        for at in 0..url.len() {
            let mut long = vec![b'x'; PIECE_BYTES - at];
            long.extend_from_slice(url.as_bytes());
            long.push(b'\n');
            let seen = split(&[&long]).concat();
            assert!(
                !seen.contains("SEC") && !seen.contains("RET"),
                "cut {at} bytes into the URL: {}",
                &seen[seen.len().saturating_sub(40)..]
            );
            assert!(seen.ends_with("/y"), "cut {at}");
        }
    }

    /// The phase 12 QA: an escape sequence between a scheme and its `://`, straddling the piece
    /// limit at every byte, neither hides the URL nor reaches the next piece. Caught by: a piece
    /// cut inside an escape, its tail left between the scheme and the `://`.
    #[test]
    fn an_escape_straddling_the_piece_limit_hides_no_url() {
        let url = "https:\x1b[0m//u:SECRET@host/r";
        for at in 0..url.len() {
            let mut long = vec![b'x'; PIECE_BYTES - at];
            long.extend_from_slice(url.as_bytes());
            long.push(b'\n');
            let seen = split(&[&long]).concat();
            assert!(
                !seen.contains("SECRET") && !seen.contains("u:"),
                "cut {at} bytes into the URL: {}",
                &seen[seen.len().saturating_sub(40)..]
            );
            assert!(!seen.contains('\x1b') && seen.ends_with("/r"), "cut {at}");
        }
        assert_eq!(piece_end(b"ab\x1b[3"), 2, "a CSI cut open");
        assert_eq!(piece_end(b"ab\x1b[3m"), 6, "a CSI whole");
        assert_eq!(piece_end(b"ab\x1b]0;t"), 2, "an OSC cut open");
        assert_eq!(piece_end(b"ab\x1b]0;t\x07"), 8, "an OSC whole");
        assert_eq!(
            piece_end(b"\x1b[3"),
            3,
            "a sequence at the start is cut where it is"
        );
    }

    /// Caught by: retaining everything, or keeping the head instead of the end.
    #[test]
    fn the_tail_keeps_the_last_256_kib_of_whole_lines() {
        let mut tail = Tail::default();
        for n in 0..40_000 {
            tail.push(&ScrubbedLines::scrubbing(&format!(
                "line {n:06} of a long stderr"
            )));
        }
        let kept = tail.into_lines();
        let text = kept.text();
        assert!(text.len() <= TAIL_BYTES, "{}", text.len());
        assert!(text.len() > TAIL_BYTES - 64, "{}", text.len());
        assert!(text.ends_with("line 039999 of a long stderr"));
        assert!(text.starts_with("line "), "{:?}", &text[..20]);
        assert!(kept.older_dropped(), "older lines let go of, and said");
        let mut short = Tail::default();
        short.push(&ScrubbedLines::scrubbing("all of it\n"));
        let short = short.into_lines();
        assert_eq!((short.text(), short.older_dropped()), ("all of it", false));
    }

    /// R3.5, while the process runs: what is held never grows past twice the
    /// window and the line being added, however much stderr goes by. Caught by:
    /// letting go only at the end.
    #[test]
    fn the_tail_holds_a_bounded_amount_while_stderr_runs_on() {
        let mut tail = Tail::default();
        let line = ScrubbedLines::scrubbing(
            "a line of a stderr that runs on and on, about sixty bytes long",
        );
        for _ in 0..80_000 {
            tail.push(&line);
            assert!(
                tail.held() <= 2 * 256 * 1024 + line.bytes() + 1,
                "{}",
                tail.held()
            );
        }
    }

    /// A line as long as the window, then another: the newest kept whole, the older let go of
    /// whole — the tail never starts part-way through a line.
    #[test]
    fn a_line_as_long_as_the_tail_is_let_go_of_whole() {
        let mut tail = Tail::default();
        let mut pieces = Lines::default();
        let mut long = ScrubbedLines::new();
        pieces.push(&vec![b'x'; PIECE_BYTES * 2], &mut long);
        tail.push(&ScrubbedLines::scrubbing("first"));
        tail.push(&long);
        tail.push(&ScrubbedLines::scrubbing("the end"));
        let kept = tail.into_lines();
        assert_eq!(kept.text(), "the end");
        assert!(kept.older_dropped());
        let mut only = Tail::default();
        only.push(&ScrubbedLines::scrubbing(&"é".repeat(PIECE_BYTES / 2)));
        let only = only.into_lines();
        assert_eq!(
            only.bytes(),
            PIECE_BYTES,
            "a piece as long as the tail kept whole"
        );
        assert!(!only.older_dropped());
    }
}
