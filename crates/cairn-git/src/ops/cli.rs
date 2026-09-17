//! Building and running one `git` invocation.
//!
//! The only place in Cairn a process is run. The [`Command`] itself comes from
//! [`GitEnvironment::command`], so it already carries the explicit environment
//! and nothing inherited; this module adds the arguments, the directory and the
//! standard streams, waits for the exit, and turns a failure into an [`Error`]
//! that carries git's own diagnostic.
//!
//! Standard input is always closed. Together with `GIT_TERMINAL_PROMPT=0` that
//! is what stops `git` itself from waiting for something nobody will type. It
//! does not reach `ssh`, which prompts on `/dev/tty` directly — a host-key
//! confirmation or a key passphrase from a Cairn launched in a terminal lands
//! on that terminal; closing that path is `SSH_ASKPASS_REQUIRE=force`, which
//! arrives with the askpass helper.
//!
//! Everything here is `pub(crate)`: the crate's public surface is named
//! operations, never a raw invocation, so a caller outside `ops` cannot run a
//! verb the confirmation seal does not know about.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use cairn_model::AskpassToken;

use super::GitEnvironment;
use crate::{Error, Repository};

/// One invocation, built up and then run once with [`GitCommand::run`].
#[derive(Debug)]
pub(crate) struct GitCommand<'a> {
    program: &'a Path,
    environment: &'a GitEnvironment,
    arguments: Vec<OsString>,
    directory: Option<PathBuf>,
    token: Option<AskpassToken>,
}

impl<'a> GitCommand<'a> {
    pub(crate) fn new(program: &'a Path, environment: &'a GitEnvironment) -> Self {
        Self {
            program,
            environment,
            arguments: Vec::new(),
            directory: None,
            token: None,
        }
    }

    pub(crate) fn arg(mut self, argument: impl AsRef<OsStr>) -> Self {
        self.arguments.push(argument.as_ref().to_owned());
        self
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the first operation to run a verb is fetch, in the next phase"
        )
    )]
    pub(crate) fn args(mut self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self {
        self.arguments
            .extend(arguments.into_iter().map(|a| a.as_ref().to_owned()));
        self
    }

    /// Runs inside `repo`: its working tree, or the git directory of a bare one.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the first operation to run a verb is fetch, in the next phase"
        )
    )]
    pub(crate) fn in_repository(mut self, repo: &Repository) -> Self {
        self.directory = Some(repo.workdir().unwrap_or(repo.git_dir()).to_owned());
        self
    }

    /// An invocation that may ask the user for a secret: `token` is what the
    /// askpass helper presents to the channel that issued it. Without one the
    /// helper is still what git runs, and it fails closed.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the first operation that can prompt is fetch, in the next phase"
        )
    )]
    pub(crate) fn authorized_by(mut self, token: &AskpassToken) -> Self {
        self.token = Some(token.clone());
        self
    }

    /// Runs to completion. A non-zero exit is [`Error::GitFailed`], with what git
    /// wrote to stderr; a process that never started is [`Error::GitNotStarted`].
    pub(crate) fn run(self) -> Result<Output, Error> {
        let mut command = self.environment.command(self.program, self.token.as_ref());
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
pub(crate) struct Output {
    stdout: Vec<u8>,
    stderr: String,
}

impl Output {
    /// Bytes, because paths are bytes: decode at the point that knows the format.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the first operation to run a verb is fetch, in the next phase"
        )
    )]
    pub(crate) fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    /// Lossily decoded; git's stderr is prose for a person, never parsed.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the first operation to run a verb is fetch, in the next phase"
        )
    )]
    pub(crate) fn stderr(&self) -> &str {
        &self.stderr
    }

    pub(crate) fn stdout_text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }

    /// The records of `-z` output: git ends each with NUL, so the terminator is
    /// stripped rather than read as an empty record after the last one.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the first operation to run a verb is fetch, in the next phase"
        )
    )]
    pub(crate) fn records(&self) -> impl Iterator<Item = &[u8]> {
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
        assert_eq!(output.stdout(), b"only");
    }

    #[test]
    fn arguments_are_described_as_typed() {
        let arguments = [OsString::from("fetch"), OsString::from("--prune")];
        assert_eq!(describe(&arguments), "fetch --prune");
    }
}

/// Against a stub `git` that reports what it was given. Inside the crate because the
/// runner is `pub(crate)`: nothing outside `ops` may run a raw invocation.
#[cfg(all(test, unix))]
mod stub_tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;

    use cairn_model::AskpassToken;

    use super::super::stub_git::{StubGit, discover_retrying};
    use crate::Repository;

    /// What `/bin/sh` itself adds to a child's environment; not ours and not git's.
    const SHELL_OWN: &[&str] = &["PWD", "OLDPWD", "SHLVL", "_"];

    /// Answers `--version`, then runs `rest` for anything else.
    fn stub(rest: &str) -> StubGit {
        StubGit::with_git(&format!(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n{rest}"
        ))
    }

    /// PRD B1 end to end: what the child actually sees is the built environment and
    /// nothing from this process. The test process is full of `CARGO_*` variables,
    /// which is what makes a leak visible.
    #[test]
    fn the_child_sees_the_built_environment_and_nothing_inherited() {
        let stub = stub("exec /usr/bin/env");
        let environment = stub.environment_with(|name| match name {
            "HOME" => Some(OsString::from("/nonexistent/home-from-cairn")),
            _ => None,
        });
        let expected: BTreeMap<String, String> = environment
            .variables()
            .map(|(name, value)| (name.to_owned(), value.to_string_lossy().into_owned()))
            .collect();
        let git = discover_retrying(environment).unwrap();
        let output = git.command().arg("print-environment").run().unwrap();
        let seen: BTreeMap<String, String> = output
            .stdout_text()
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();

        for (name, value) in &expected {
            assert_eq!(
                seen.get(name),
                Some(value),
                "{name} did not reach git as built"
            );
        }
        let leaked: Vec<&String> = seen
            .keys()
            .filter(|name| !expected.contains_key(*name) && !SHELL_OWN.contains(&name.as_str()))
            .collect();
        assert!(leaked.is_empty(), "inherited by git: {leaked:?}");
        assert!(
            !seen.keys().any(|name| name.starts_with("CARGO_")),
            "cargo's variables reached git: {:?}",
            seen.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            seen.get("GIT_TERMINAL_PROMPT").map(String::as_str),
            Some("0")
        );
        assert_eq!(
            seen.get("HOME").map(String::as_str),
            Some("/nonexistent/home-from-cairn"),
            "HOME must be the value the builder chose, not this process's"
        );
        assert_eq!(
            seen.get("GIT_ASKPASS").map(String::as_str),
            Some(StubGit::HELPER),
            "git must be pointed at Cairn's askpass helper"
        );
        assert_eq!(
            seen.get("SSH_ASKPASS").map(String::as_str),
            Some(StubGit::HELPER)
        );
        assert_eq!(
            seen.get("SSH_ASKPASS_REQUIRE").map(String::as_str),
            Some("force")
        );
        assert!(
            !seen.contains_key("CAIRN_ASKPASS_TOKEN"),
            "an invocation nobody authorised carried a token"
        );
    }

    /// The token reaches the child only on an invocation that was given one.
    #[test]
    fn an_authorised_invocation_carries_its_token_and_only_that_one() {
        let stub = stub("exec /usr/bin/env");
        let git = discover_retrying(stub.environment()).unwrap();
        let token = AskpassToken::new(format!("token-{}", std::process::id()));
        let output = git
            .command()
            .arg("print-environment")
            .authorized_by(&token)
            .run()
            .unwrap();
        let text = output.stdout_text();
        let seen: BTreeMap<&str, &str> = text.lines().filter_map(|l| l.split_once('=')).collect();
        assert_eq!(seen.get("CAIRN_ASKPASS_TOKEN"), Some(&token.as_str()));
        assert_eq!(seen.get("GIT_ASKPASS"), Some(&StubGit::HELPER));
    }

    #[test]
    fn arguments_arrive_in_order_and_nul_records_split() {
        let stub = stub("printf '%s\\0%s\\0' \"$1\" \"$2\"");
        let git = discover_retrying(stub.environment()).unwrap();
        let output = git
            .command()
            .args(["rev-parse", "--show-toplevel"])
            .run()
            .unwrap();
        let records: Vec<&[u8]> = output.records().collect();
        assert_eq!(
            records,
            [b"rev-parse".as_slice(), b"--show-toplevel".as_slice()]
        );
        assert_eq!(output.stderr(), "");
    }

    /// `pwd` is a shell builtin, so the stub needs nothing on its PATH.
    #[test]
    fn a_command_runs_in_the_repository_it_is_asked_to() {
        let stub = stub("pwd");
        let git = discover_retrying(stub.environment()).unwrap();
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let expected = std::fs::canonicalize(repo.workdir().unwrap()).unwrap();

        let output = git.command().in_repository(&repo).run().unwrap();
        let ran_in = std::fs::canonicalize(output.stdout_text().trim()).unwrap();
        assert_eq!(ran_in, expected);

        // Without `in_repository`, the process runs wherever this one does.
        let output = git.command().run().unwrap();
        let ran_in = std::fs::canonicalize(output.stdout_text().trim()).unwrap();
        assert_eq!(
            ran_in,
            std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap()
        );
    }

    /// Caught by: `Stdio::null()` becoming `inherit()`, which is how a git waiting on a
    /// pipe nobody writes to would come back. Linux only: it reads `/proc`.
    #[cfg(target_os = "linux")]
    #[test]
    fn standard_input_is_closed_not_inherited() {
        let stub = stub("PATH=/usr/bin:/bin readlink /proc/$$/fd/0");
        let git = discover_retrying(stub.environment()).unwrap();
        let output = git.command().run().unwrap();
        assert_eq!(output.stdout_text().trim(), "/dev/null");
    }
}
