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

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "fetch and the version probe move onto this runner next, and reads/ gets its \
                  first caller with diff-engine"
    )
)]

use std::io::{self, Read as _};
use std::process::{ChildStderr, ChildStdout};
use std::sync::mpsc::SyncSender;

use super::group::Group;

/// What a reader hands the driving thread.
#[derive(Debug)]
pub(super) enum Event {
    /// Bytes from stdout, in order, as they arrived.
    Stdout(Vec<u8>),
    /// The lines of stderr one read completed, in order, each with its
    /// terminator stripped and lossily decoded, joined by `\n` (which no line
    /// contains) into one string. One event per read, not per line, so what git
    /// wrote before it exited is a few events however many lines it was, and the
    /// driver's last look at the queue takes it all; one string, not a string
    /// per line, so an event costs one allocation of at most a read's bytes and
    /// a pending line piece, however short its lines.
    Lines(String),
}

/// How many events may wait for the driving thread before a reader waits for
/// it: each is one allocation of at most a 64 KiB read — a stderr event also
/// a line piece of up to 256 KiB, and up to three bytes for each undecodable
/// one — so a few MiB of stdout queued at most, and some tens of MiB of
/// stderr in the worst case.
pub(super) const EVENTS_BOUND: usize = 64;

/// Bytes read from a pipe at a time: the most a Linux pipe holds by default.
const CHUNK: usize = 64 * 1024;

/// How much of git's stderr is retained for an error and the log: the last
/// 256 KiB. Lines before that are forwarded as progress and dropped.
pub(super) const TAIL_BYTES: usize = 256 * 1024;

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

/// Reads stderr to its end, sending each line on as it completes: a line ends
/// at `\n` or, as git's progress meters redraw themselves, at `\r`. A line
/// longer than [`TAIL_BYTES`] is sent in pieces of that size, so a stderr with
/// no terminator in it cannot grow without bound here either.
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
        let complete = lines.push(&chunk[..read]);
        if !complete.is_empty() && events.send(Event::Lines(complete.join("\n"))).is_err() {
            break 'reading;
        }
    }
    if let Some(line) = lines.rest() {
        let _ = events.send(Event::Lines(line));
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

/// Splits stderr into lines across reads.
#[derive(Debug, Default)]
struct Lines {
    pending: Vec<u8>,
}

impl Lines {
    /// Every line `bytes` completes, and any piece of an over-long one.
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut done = Vec::new();
        for &byte in bytes {
            if matches!(byte, b'\n' | b'\r') {
                done.push(String::from_utf8_lossy(&self.pending).into_owned());
                self.pending.clear();
            } else {
                self.pending.push(byte);
                if self.pending.len() >= TAIL_BYTES {
                    done.push(String::from_utf8_lossy(&self.pending).into_owned());
                    self.pending.clear();
                }
            }
        }
        done
    }

    /// The last line, when stderr ended without a terminator.
    fn rest(self) -> Option<String> {
        (!self.pending.is_empty()).then(|| String::from_utf8_lossy(&self.pending).into_owned())
    }
}

/// The retained end of stderr: at most [`TAIL_BYTES`], ending with the last
/// line, starting at a line's start where one falls inside the window. While
/// the process runs it holds at most twice that plus the line being added,
/// because it cuts in batches.
#[derive(Debug, Default)]
pub(super) struct Tail {
    text: String,
}

impl Tail {
    pub(super) fn push(&mut self, line: &str) {
        self.text.push_str(line);
        self.text.push('\n');
        // Cut in batches, so retaining is linear in what was said, not quadratic.
        if self.text.len() > 2 * TAIL_BYTES {
            self.cut();
        }
    }

    fn cut(&mut self) {
        let length = self.text.len();
        if length <= TAIL_BYTES {
            return;
        }
        let mut start = length - TAIL_BYTES;
        while !self.text.is_char_boundary(start) {
            start += 1;
        }
        if self.text.as_bytes().get(start.wrapping_sub(1)) != Some(&b'\n')
            && let Some(newline) = self.text[start..].find('\n')
            && start + newline + 1 < length
        {
            start += newline + 1;
        }
        self.text.drain(..start);
    }

    /// How many bytes it holds now, for a test of what it holds while running.
    #[cfg(test)]
    fn held(&self) -> usize {
        self.text.len()
    }

    /// The retained text, trailing whitespace trimmed.
    pub(super) fn into_text(mut self) -> String {
        self.cut();
        let kept = self.text.trim_end().len();
        self.text.truncate(kept);
        self.text
    }
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

    /// Caught by: splitting on one terminator only, or keeping the terminator.
    #[test]
    fn stderr_lines_end_at_either_terminator_across_reads() {
        let mut lines = Lines::default();
        let mut seen = lines.push(b"Receiving:  5");
        seen.extend(lines.push(b"0%\rReceiving: 100%\nFrom x"));
        assert_eq!(seen, ["Receiving:  50%", "Receiving: 100%"]);
        assert_eq!(lines.rest().as_deref(), Some("From x"));
    }

    /// Caught by: a line with no terminator growing without bound.
    #[test]
    fn a_line_without_a_terminator_is_sent_in_pieces_no_longer_than_the_tail() {
        let mut lines = Lines::default();
        let seen = lines.push(&vec![b'x'; TAIL_BYTES * 2 + 3]);
        assert_eq!(seen.len(), 2);
        assert!(seen.iter().all(|line| line.len() == TAIL_BYTES));
        assert_eq!(lines.rest().map(|rest| rest.len()), Some(3));
    }

    /// Caught by: retaining everything, or keeping the head instead of the end.
    #[test]
    fn the_tail_keeps_the_last_256_kib_from_a_line_start() {
        let mut tail = Tail::default();
        for n in 0..40_000 {
            tail.push(&format!("line {n:06} of a long stderr"));
        }
        let text = tail.into_text();
        assert!(text.len() <= TAIL_BYTES, "{}", text.len());
        assert!(text.len() > TAIL_BYTES - 64, "{}", text.len());
        assert!(text.ends_with("line 039999 of a long stderr"));
        assert!(text.starts_with("line "), "{:?}", &text[..20]);
    }

    /// R3.5, while the process runs: what is held never grows past twice the
    /// window and the line being added, however much stderr goes by. Caught by:
    /// cutting only at the end.
    #[test]
    fn the_tail_holds_a_bounded_amount_while_stderr_runs_on() {
        let mut tail = Tail::default();
        let line = "a line of a stderr that runs on and on, about sixty bytes long";
        for _ in 0..80_000 {
            tail.push(line);
            assert!(
                tail.held() <= 2 * 256 * 1024 + line.len() + 1,
                "{}",
                tail.held()
            );
        }
    }

    /// One line longer than the window keeps its end, never more than the window.
    #[test]
    fn a_single_line_longer_than_the_tail_keeps_its_end() {
        let mut tail = Tail::default();
        tail.push("first");
        tail.push(&format!("{}end", "é".repeat(TAIL_BYTES)));
        let text = tail.into_text();
        assert!(text.len() <= TAIL_BYTES);
        assert!(text.ends_with("éend"));
    }
}
