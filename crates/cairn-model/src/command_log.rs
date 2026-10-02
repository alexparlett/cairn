//! What Cairn ran on the user's behalf: one record per `git` invocation, for
//! the command log the engine keeps per repository and a view will one day
//! show, the way Fork's does.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// One `git` invocation that is over, however it ended. Plain data, and only
/// what the invocation was given and what it said: there is no field for the
/// environment it ran with — which is where an askpass token travels — and
/// none that could hold a [`crate::Secret`]. Its arguments are what Cairn
/// passed: remote names, refs, paths and options, never a credential and never
/// a URL Cairn read from configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRecord {
    /// The arguments after the program, in order, each lossily decoded.
    pub arguments: Vec<String>,
    /// Where it ran: a repository's working tree, or a bare one's git
    /// directory. `None` for an invocation run in no repository.
    pub directory: Option<PathBuf>,
    /// When it was started, by the wall clock.
    pub started: SystemTime,
    /// From the start until it was over: reaped, or abandoned, or refused at
    /// the start.
    pub duration: Duration,
    /// How it ended.
    pub exit: CommandExit,
    /// It was asked to end — cancelled, superseded or dropped — and the end
    /// was that, not a clean exit that beat the request.
    pub cancelled: bool,
    /// The end of what it wrote to stderr: the tail the runner retains, at
    /// most 256 KiB, trailing whitespace trimmed. Empty when it never started.
    pub stderr: String,
}

/// How a recorded invocation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandExit {
    /// It exited, with this status code.
    Code(i32),
    /// A signal ended it, by number: one Cairn sent on a cancel, or another.
    Signal(i32),
    /// It never started: the program could not be run.
    NotStarted,
    /// What became of it is not known: waiting on it failed, or it was
    /// abandoned before it could be reaped.
    Unknown,
}

impl CommandRecord {
    /// The bytes the record holds beyond its own size — its arguments, its
    /// directory and its stderr — which is what a log bounded by bytes counts.
    pub fn held_bytes(&self) -> usize {
        self.arguments.iter().map(String::len).sum::<usize>()
            + self
                .directory
                .as_ref()
                .map_or(0, |directory| directory.as_os_str().len())
            + self.stderr.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> CommandRecord {
        CommandRecord {
            arguments: vec!["fetch".to_owned(), "origin".to_owned()],
            directory: Some(PathBuf::from("/work/repo")),
            started: SystemTime::UNIX_EPOCH,
            duration: Duration::from_millis(1500),
            exit: CommandExit::Code(0),
            cancelled: false,
            stderr: "From x".to_owned(),
        }
    }

    /// Caught by: leaving a part of the record out of the count, which would let
    /// a log bounded by bytes hold more than its bound.
    #[test]
    fn the_held_bytes_are_the_arguments_the_directory_and_the_stderr() {
        let record = record();
        assert_eq!(record.held_bytes(), "fetch".len() + "origin".len() + 10 + 6);
        let nowhere = CommandRecord {
            directory: None,
            stderr: String::new(),
            ..record
        };
        assert_eq!(nowhere.held_bytes(), 11);
    }

    /// Plain data: a record compares by every field and survives a clone, so a
    /// snapshot of the log handed to a view is the log as it was.
    #[test]
    fn a_record_is_plain_data() {
        let record = record();
        assert_eq!(record.clone(), record);
        let cancelled = CommandRecord {
            cancelled: true,
            exit: CommandExit::Signal(15),
            ..record.clone()
        };
        assert_ne!(cancelled, record);
        assert_ne!(CommandExit::NotStarted, CommandExit::Unknown);
    }
}
