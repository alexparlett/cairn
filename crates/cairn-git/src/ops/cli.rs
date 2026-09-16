//! Building and running one `git` invocation.
//!
//! The only place in Cairn a process is run. The [`Command`] itself comes from
//! [`GitEnvironment::command`], so it already carries the explicit environment
//! and nothing inherited; this module adds the arguments, the directory and the
//! standard streams, waits for the exit, and turns a failure into an [`Error`]
//! that carries git's own diagnostic.
//!
//! Standard input is always closed. Together with `GIT_TERMINAL_PROMPT=0` that
//! is what makes "git is waiting for something nobody will type" impossible.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use super::GitEnvironment;
use crate::{Error, Repository};

/// One invocation, built up and then run once with [`GitCommand::run`].
#[derive(Debug)]
pub struct GitCommand<'a> {
    program: &'a Path,
    environment: &'a GitEnvironment,
    arguments: Vec<OsString>,
    directory: Option<PathBuf>,
}

impl<'a> GitCommand<'a> {
    pub(crate) fn new(program: &'a Path, environment: &'a GitEnvironment) -> Self {
        Self {
            program,
            environment,
            arguments: Vec::new(),
            directory: None,
        }
    }

    pub fn arg(mut self, argument: impl AsRef<OsStr>) -> Self {
        self.arguments.push(argument.as_ref().to_owned());
        self
    }

    pub fn args(mut self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self {
        self.arguments
            .extend(arguments.into_iter().map(|a| a.as_ref().to_owned()));
        self
    }

    /// Runs inside `repo`: its working tree, or the git directory of a bare one.
    pub fn in_repository(mut self, repo: &Repository) -> Self {
        self.directory = Some(repo.workdir().unwrap_or(repo.git_dir()).to_owned());
        self
    }

    /// Runs to completion. A non-zero exit is [`Error::GitFailed`], with what git
    /// wrote to stderr; a process that never started is [`Error::GitNotStarted`].
    pub fn run(self) -> Result<Output, Error> {
        let mut command = self.environment.command(self.program);
        command
            .args(&self.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(directory) = &self.directory {
            command.current_dir(directory);
        }
        let output = command.output().map_err(|source| Error::GitNotStarted {
            program: self.program.to_owned(),
            source,
        })?;
        let stderr = String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_owned();
        if !output.status.success() {
            return Err(Error::GitFailed {
                arguments: describe(&self.arguments),
                status: output.status,
                stderr,
            });
        }
        Ok(Output {
            stdout: output.stdout,
            stderr,
        })
    }
}

/// The arguments as a user would have typed them, for an error message.
fn describe(arguments: &[OsString]) -> String {
    arguments
        .iter()
        .map(|argument| argument.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a successful invocation wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    stdout: Vec<u8>,
    stderr: String,
}

impl Output {
    /// Bytes, because paths are bytes: decode at the point that knows the format.
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    /// Lossily decoded; git's stderr is prose for a person, never parsed.
    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    pub fn stdout_text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    /// The records of `-z` output: git ends each with NUL, so the terminator is
    /// stripped rather than read as an empty record after the last one.
    pub fn records(&self) -> impl Iterator<Item = &[u8]> {
        let body = self.stdout.strip_suffix(b"\0").unwrap_or(&self.stdout);
        body.split(|byte| *byte == 0)
            .filter(move |_| !body.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(stdout: &[u8]) -> Output {
        Output {
            stdout: stdout.to_vec(),
            stderr: String::new(),
        }
    }

    #[test]
    fn nul_terminated_records_are_split_without_a_phantom_last_one() {
        let output = output(b"a\0b c\0");
        let records: Vec<&[u8]> = output.records().collect();
        assert_eq!(records, [b"a".as_slice(), b"b c".as_slice()]);
    }

    #[test]
    fn empty_output_has_no_records() {
        assert_eq!(output(b"").records().count(), 0);
        assert_eq!(output(b"\0").records().count(), 0);
    }

    #[test]
    fn a_record_without_the_final_terminator_still_counts() {
        let output = output(b"only");
        let records: Vec<&[u8]> = output.records().collect();
        assert_eq!(records, [b"only".as_slice()]);
    }

    #[test]
    fn arguments_are_described_as_typed() {
        let arguments = [OsString::from("fetch"), OsString::from("--prune")];
        assert_eq!(describe(&arguments), "fetch --prune");
    }
}
