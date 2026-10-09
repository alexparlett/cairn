//! The command log: what Cairn ran in a repository, one record per
//! invocation, newest last, bounded by count and by bytes.
//!
//! In memory and nowhere else, and kept without a logging crate: it is state
//! the application owns, for a view that shows the user what Cairn ran on
//! their behalf, not a diagnostic stream. A record is a
//! [`cairn_model::CommandRecord`], which has no field for the environment, so
//! the askpass token an invocation carried cannot reach it.

use std::collections::VecDeque;
use std::thread::ThreadId;

use cairn_model::CommandRecord;

/// How many records the log keeps: the last thousand.
///
/// A record is a few hundred bytes when git said little, which is nearly
/// always, so the count is what bounds the log in the common case. A thousand
/// is a long session's worth: a fetch is one, and the reads `diff-engine` adds
/// are one per selection the user makes, so the user scrolling back through
/// the log finds the afternoon's work, not the last minute's.
pub(crate) const LOG_ENTRIES: usize = 1000;

/// How many bytes of arguments, directories and stderr the log keeps: 4 MiB.
///
/// What the count cannot bound is git saying a lot: a failing hook's output,
/// a fetch's progress, each retained up to the runner's 256 KiB tail, so a
/// thousand of those would be 256 MiB. 4 MiB keeps sixteen such tails whole,
/// and the log drops its oldest records first to stay under it. It is also
/// above what one record can hold on the platforms git runs on — at most
/// `ARG_MAX` of arguments (2 MiB on Linux's default stack, 1 MiB on macOS),
/// a path and a 256 KiB tail — so a record is trimmed to fit only past those
/// defaults ([`CommandLog::push`]).
pub(crate) const LOG_BYTES: usize = 4 * 1024 * 1024;

/// Where an invocation was asked from: the thread that built it, and its place in the order
/// invocations were built in, in its repository. Kept beside a record, never in it, so the
/// record stays exactly what R8.1 lists; it is what lets an operation name the `git` it ran
/// itself among what other threads ran meanwhile (staging-and-commit R12.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Origin {
    pub(crate) thread: ThreadId,
    pub(crate) order: u64,
}

/// The records, oldest first, each with its origin, and the bytes they hold.
#[derive(Debug, Default)]
pub(crate) struct CommandLog {
    records: VecDeque<(Origin, CommandRecord)>,
    bytes: usize,
}

impl CommandLog {
    /// Appends `record`, then drops the oldest records until both bounds hold
    /// again. A record that alone holds more than [`LOG_BYTES`] is trimmed to
    /// fit before it is kept: its stderr keeps its end, then its arguments
    /// keep their start, with one last argument saying how many were not
    /// kept — never cut silently.
    pub(crate) fn push(&mut self, origin: Origin, mut record: CommandRecord) {
        fit(&mut record, LOG_BYTES);
        self.bytes += record.held_bytes();
        self.records.push_back((origin, record));
        while self.records.len() > LOG_ENTRIES || self.bytes > LOG_BYTES {
            let Some((_, oldest)) = self.records.pop_front() else {
                break;
            };
            self.bytes -= oldest.held_bytes();
        }
    }

    /// Every record kept, oldest first.
    pub(crate) fn records(&self) -> Vec<CommandRecord> {
        self.records
            .iter()
            .map(|(_, record)| record.clone())
            .collect()
    }

    /// The records kept of invocations `thread` built at or after `order`, oldest first by
    /// when they were over.
    pub(crate) fn built_on_since(&self, thread: ThreadId, order: u64) -> Vec<CommandRecord> {
        self.records
            .iter()
            .filter(|(origin, _)| origin.thread == thread && origin.order >= order)
            .map(|(_, record)| record.clone())
            .collect()
    }
}

/// Trims `record` until it holds at most `limit` bytes; see [`CommandLog::push`].
fn fit(record: &mut CommandRecord, limit: usize) {
    let over = record.held_bytes().saturating_sub(limit);
    if over == 0 {
        return;
    }
    let mut cut = over.min(record.stderr.len());
    while !record.stderr.is_char_boundary(cut) {
        cut += 1;
    }
    record.stderr.drain(..cut);
    // Cut from the front: said, so a view scrubs its first line as begun part-way.
    record.stderr_cut |= cut > 0;
    if record.held_bytes() <= limit {
        return;
    }
    // The arguments alone are over: keep a prefix, and say what went.
    let directory = record
        .directory
        .as_ref()
        .map_or(0, |directory| directory.as_os_str().len());
    let total = record.arguments.len();
    let mut room = limit.saturating_sub(directory + record.stderr.len() + 64);
    let mut kept = 0;
    for argument in &record.arguments {
        if argument.len() > room {
            break;
        }
        room -= argument.len();
        kept += 1;
    }
    record.arguments.truncate(kept);
    record
        .arguments
        .push(format!("[{} more arguments not kept]", total - kept));
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use cairn_model::CommandExit;

    use super::*;

    fn record(n: usize, stderr: String) -> CommandRecord {
        CommandRecord {
            arguments: vec!["rev-parse".to_owned(), format!("n{n}")],
            directory: Some(PathBuf::from("/work/repo")),
            started: SystemTime::UNIX_EPOCH,
            duration: Duration::ZERO,
            exit: CommandExit::Code(0),
            cancelled: false,
            stderr,
            stderr_cut: false,
        }
    }

    fn origin(n: usize) -> Origin {
        Origin {
            thread: std::thread::current().id(),
            order: n as u64,
        }
    }

    fn held(log: &CommandLog) -> usize {
        log.records().iter().map(CommandRecord::held_bytes).sum()
    }

    /// R8.2, the count: past `LOG_ENTRIES` the oldest go first, and what is kept
    /// is the newest, in order. Caught by: no bound, dropping the newest, or
    /// an off-by-one at the bound.
    #[test]
    fn the_log_keeps_the_newest_log_entries_records() {
        let mut log = CommandLog::default();
        for n in 0..LOG_ENTRIES + 7 {
            log.push(origin(n), record(n, String::new()));
        }
        let records = log.records();
        assert_eq!(records.len(), LOG_ENTRIES);
        assert_eq!(records[0].arguments[1], "n7");
        assert_eq!(
            records[LOG_ENTRIES - 1].arguments[1],
            format!("n{}", LOG_ENTRIES + 6)
        );
        assert_eq!(log.bytes, held(&log), "the running count drifted");
    }

    /// R8.2, the bytes: records whose stderr is a full 256 KiB tail push the
    /// oldest out once their sum would pass `LOG_BYTES`, long before the count
    /// does. Caught by: bounding by count alone, or a running total that
    /// forgets what a dropped record held.
    #[test]
    fn the_log_holds_no_more_than_log_bytes() {
        let tail = "x".repeat(256 * 1024);
        let mut log = CommandLog::default();
        for n in 0..40 {
            log.push(origin(n), record(n, tail.clone()));
            assert!(log.bytes <= LOG_BYTES, "{} bytes held", log.bytes);
        }
        let records = log.records();
        assert!(
            records.len() < 40 && records.len() >= 15,
            "{}",
            records.len()
        );
        assert_eq!(records.last().unwrap().arguments[1], "n39");
        assert_eq!(log.bytes, held(&log), "the running count drifted");
        assert!(held(&log) <= LOG_BYTES);
    }

    /// A record that alone is over the byte bound is kept, trimmed to fit, and
    /// says so: its stderr keeps its end; past that, its arguments keep their
    /// start and a last argument counts what went. Caught by: keeping it whole
    /// (the bound breaks), dropping it (an invocation goes unrecorded), or
    /// cutting the arguments without a word.
    #[test]
    fn a_record_larger_than_the_bound_is_trimmed_to_fit_and_says_so() {
        let mut log = CommandLog::default();
        // An odd overage, so the first cut lands inside an `é` and must move
        // to the next character boundary rather than split one.
        let mut loud = record(0, format!("{}the end.", "é".repeat(LOG_BYTES)));
        loud.arguments = vec!["fetch".to_owned()];
        assert_eq!(
            (loud.held_bytes() - LOG_BYTES) % 2,
            1,
            "the overage is even, so the cut lands on a boundary and this decides nothing"
        );
        log.push(origin(0), loud);
        let kept = &log.records()[0];
        assert!(kept.held_bytes() <= LOG_BYTES);
        assert!(kept.stderr.ends_with("the end."));
        assert!(
            kept.stderr_cut,
            "a stderr cut to fit was not said to be cut"
        );
        assert_eq!(kept.arguments, ["fetch"]);

        let mut long = record(1, "said".to_owned());
        long.arguments = (0..LOG_BYTES / 1000 + 10)
            .map(|n| format!("{n:01000}"))
            .collect();
        log.push(origin(1), long);
        let records = log.records();
        let kept = records.last().unwrap();
        assert!(kept.held_bytes() <= LOG_BYTES, "{}", kept.held_bytes());
        assert!(
            kept.arguments
                .last()
                .unwrap()
                .ends_with("more arguments not kept]"),
            "{:?}",
            kept.arguments.last()
        );
        assert_eq!(kept.arguments[0], format!("{:01000}", 0));
        assert!(log.bytes <= LOG_BYTES);
        assert_eq!(log.bytes, held(&log));
    }
}
