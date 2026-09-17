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
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

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

    pub(crate) fn args(mut self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self {
        self.arguments
            .extend(arguments.into_iter().map(|a| a.as_ref().to_owned()));
        self
    }

    /// Runs inside `repo`: its working tree, or the git directory of a bare one.
    pub(crate) fn in_repository(mut self, repo: &Repository) -> Self {
        self.directory = Some(repo.workdir().unwrap_or(repo.git_dir()).to_owned());
        self
    }

    /// An invocation that may ask the user for a secret: `token` is what the
    /// askpass helper presents to the channel that issued it. Without one the
    /// helper is still what git runs, and it fails closed.
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

    /// Starts the process and hands it back still running, for a verb whose
    /// stderr is progress a person watches (`fetch --progress`) and which may
    /// need killing before it is done. Standard output is discarded: nothing
    /// streamed this way has a machine-readable form on it, and a pipe nobody
    /// drains would stall the child once it filled.
    pub(crate) fn stream(self) -> Result<Running, Error> {
        let mut command = self.environment.command(self.program, self.token.as_ref());
        command
            .args(&self.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if let Some(directory) = &self.directory {
            command.current_dir(directory);
        }
        let not_started = |source| Error::GitNotStarted {
            program: self.program.to_owned(),
            source,
        };
        let mut child = command.spawn().map_err(not_started)?;
        let Some(stderr) = child.stderr.take() else {
            // Unreachable with `Stdio::piped()` above; reported rather than assumed.
            let _ = child.kill();
            let _ = child.wait();
            return Err(not_started(std::io::Error::other(
                "the child was started without a stderr pipe",
            )));
        };
        Ok(Running {
            process: Arc::new(Mutex::new(Process {
                child,
                terminated_at: None,
                killed: false,
            })),
            cancelled: Arc::new(AtomicBool::new(false)),
            stderr: Some(stderr),
            arguments: describe(&self.arguments),
        })
    }
}

/// A started process whose stderr is being streamed; see [`GitCommand::stream`].
/// Ends with [`Running::finish`], which always reaps the child, killed or not.
#[derive(Debug)]
pub(crate) struct Running {
    /// Shared with every [`ProcessKill`], which needs the handle to signal it.
    process: Arc<Mutex<Process>>,
    /// Set by a kill, so [`Running::finish`] reports a cancellation and not a failure.
    cancelled: Arc<AtomicBool>,
    stderr: Option<ChildStderr>,
    arguments: String,
}

/// The child and how far a cancel has got with it. One lock covers both, so
/// the two threads that act on a cancel — the one that asked, which sends
/// `SIGTERM`, and the one waiting in [`Running::finish`], which escalates —
/// never send the same signal twice or race the reap.
#[derive(Debug)]
struct Process {
    child: Child,
    /// When `SIGTERM` went out; `None` until a cancel reaches the process.
    terminated_at: Option<Instant>,
    /// `SIGKILL` has gone out: the grace period passed with git still running.
    killed: bool,
}

/// How long a cancelled git gets to act on `SIGTERM` before `SIGKILL`. git's
/// handler removes its temporary and lock files and exits at once, in
/// milliseconds; two seconds is that on a loaded machine with room to spare,
/// and short enough that "cancelled" still arrives while the user is looking.
pub(crate) const TERMINATION_GRACE: Duration = Duration::from_secs(2);

impl Process {
    /// `SIGTERM`, once: git removes the lock files it holds on the way out, which
    /// it cannot after `SIGKILL`. Never to a child that has exited: once it is
    /// reaped its pid is free for the system to hand to any other process, and
    /// `std`'s own `kill` refuses for that reason — `try_wait` under this lock
    /// is the same refusal, since nothing else can reap while the lock is held,
    /// and a child that has exited but not been reaped is a zombie the signal
    /// cannot hurt. A pid the signal cannot name — impossible on the platforms
    /// git runs on, handled rather than assumed — falls back to what `std` can
    /// send.
    fn terminate(&mut self) {
        if self.terminated_at.is_some() {
            return;
        }
        if !matches!(self.child.try_wait(), Ok(None)) {
            return;
        }
        self.terminated_at = Some(Instant::now());
        match i32::try_from(self.child.id()) {
            Ok(pid) => {
                let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
            }
            Err(_) => {
                let _ = self.child.kill();
                self.killed = true;
            }
        }
    }

    /// Whether the child has exited. While `cancelled`, also drives the cancel
    /// forward: sends `SIGTERM` if the killer missed the lock, and `SIGKILL`
    /// once [`TERMINATION_GRACE`] has passed with the process still running.
    fn poll(&mut self, cancelled: bool) -> std::io::Result<Option<ExitStatus>> {
        if let Some(status) = self.child.try_wait()? {
            return Ok(Some(status));
        }
        if cancelled {
            match self.terminated_at {
                None => self.terminate(),
                Some(at) if !self.killed && at.elapsed() >= TERMINATION_GRACE => {
                    let _ = self.child.kill();
                    self.killed = true;
                }
                Some(_) => {}
            }
        }
        Ok(None)
    }
}

/// Bytes read from the pipe at a time; git's progress lines are far shorter.
const STDERR_CHUNK: usize = 4096;

impl Running {
    /// A handle that kills the process from any thread; see [`ProcessKill::kill`].
    pub(crate) fn killer(&self) -> ProcessKill {
        ProcessKill {
            process: Arc::clone(&self.process),
            cancelled: Arc::clone(&self.cancelled),
        }
    }

    /// The child's process id, for a test that checks it was reaped.
    #[cfg(test)]
    pub(crate) fn id(&self) -> u32 {
        lock(&self.process).child.id()
    }

    /// Streams stderr to `progress`, one line as each completes — a line ends
    /// at `\n` or, as git's progress meters redraw themselves, at `\r` — and
    /// waits for the exit. A process killed through [`ProcessKill`] is
    /// [`Error::GitCancelled`]; any other non-zero exit is [`Error::GitFailed`]
    /// with everything stderr said.
    ///
    /// The pipe is read on a thread of its own, because git's children —
    /// `ssh`, `git-remote-https`, the askpass helper — inherit it and outlive a
    /// killed `git`. Waiting for the pipe to close would wait for them; after a
    /// kill this returns as soon as `git` itself is reaped, and the reader
    /// thread ends by itself when the last child lets the pipe go.
    ///
    /// This is also the thread that finishes a cancel: every poll while
    /// cancelled runs [`Process::poll`], so a `SIGTERM` the killer could not
    /// send (it only tries for the lock) goes out from here, and `SIGKILL`
    /// follows once [`TERMINATION_GRACE`] has passed — the wait after a cancel
    /// is bounded by that and the poll interval, whatever git does.
    pub(crate) fn finish(mut self, mut progress: impl FnMut(&str)) -> Result<Output, Error> {
        let (lines, read) = std::sync::mpsc::channel::<String>();
        let Some(stderr) = self.stderr.take() else {
            return self.reap(None, &mut progress, String::new());
        };
        // Detached on purpose: see above. `lines` closing is how it reports the pipe's end.
        std::thread::Builder::new()
            .name("cairn-git-stderr".to_owned())
            .spawn(move || read_lines(stderr, &lines))
            .map_err(|source| Error::GitNotStarted {
                program: PathBuf::from("git"),
                source,
            })?;

        let mut everything = String::new();
        let mut status = None;
        loop {
            match read.recv_timeout(EXIT_POLL) {
                Ok(line) => {
                    everything.push_str(&line);
                    everything.push('\n');
                    report(&line, &mut progress);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
            if self.cancelled.load(Ordering::Acquire) {
                // Cancelled: reap git as soon as it is gone and stop waiting on its
                // children; escalate to SIGKILL if it outstays the grace period.
                if let Ok(Some(exited)) = lock(&self.process).poll(true) {
                    status = Some(exited);
                    break;
                }
            }
        }
        self.reap(status, &mut progress, everything)
    }

    fn reap(
        &mut self,
        already: Option<ExitStatus>,
        progress: &mut impl FnMut(&str),
        everything: String,
    ) -> Result<Output, Error> {
        let _ = progress;
        let status = match already {
            Some(status) => status,
            // Polled rather than `wait()`ed, so the lock is never held across a block and
            // a kill from another thread can always take it.
            None => loop {
                let cancelled = self.cancelled.load(Ordering::Acquire);
                let exited =
                    lock(&self.process)
                        .poll(cancelled)
                        .map_err(|source| Error::GitNotStarted {
                            program: PathBuf::from("git"),
                            source,
                        })?;
                if let Some(status) = exited {
                    break status;
                }
                std::thread::sleep(EXIT_POLL);
            },
        };
        let stderr = everything.trim_end().to_owned();
        // A clean exit is a clean exit whatever the flag says: a kill that landed after
        // it changed nothing, and reporting "cancelled" would hide refs that moved.
        if status.success() {
            return Ok(Output {
                stdout: Vec::new(),
                stderr,
            });
        }
        if self.cancelled.load(Ordering::Acquire) {
            // What the cancel left on disk is the operation's to find: it knows the
            // repository, and this runner knows only a process.
            return Err(Error::GitCancelled {
                arguments: std::mem::take(&mut self.arguments),
                stranded_locks: Vec::new(),
            });
        }
        Err(Error::GitFailed {
            arguments: std::mem::take(&mut self.arguments),
            status,
            stderr,
        })
    }
}

/// How often a cancelled wait checks whether `git` has exited.
const EXIT_POLL: Duration = Duration::from_millis(20);

/// Reads `stderr` to its end, sending each completed line (ending at `\n` or
/// `\r`, terminator stripped, lossily decoded) down `lines`; a receiver that
/// has gone ends the read.
fn read_lines(mut stderr: ChildStderr, lines: &std::sync::mpsc::Sender<String>) {
    let mut pending = Vec::new();
    let mut chunk = [0u8; STDERR_CHUNK];
    loop {
        let read = match stderr.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(source) if source.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        pending.extend_from_slice(&chunk[..read]);
        while let Some(end) = pending
            .iter()
            .position(|byte| matches!(byte, b'\n' | b'\r'))
        {
            let line: Vec<u8> = pending.drain(..=end).collect();
            if lines
                .send(String::from_utf8_lossy(&line[..line.len() - 1]).into_owned())
                .is_err()
            {
                return;
            }
        }
    }
    if !pending.is_empty() {
        let _ = lines.send(String::from_utf8_lossy(&pending).into_owned());
    }
}

/// A finished line, handed on unless it was blank.
fn report(line: &str, progress: &mut impl FnMut(&str)) {
    let text = line.trim_end();
    if !text.is_empty() {
        progress(text);
    }
}

/// Ends the process a [`Running`] is waiting on, from another thread. The
/// process is still reaped by [`Running::finish`], which then reports
/// [`Error::GitCancelled`]; a kill after a clean exit changes nothing, and
/// the exit is reported as the success it was.
#[derive(Debug, Clone)]
pub(crate) struct ProcessKill {
    process: Arc<Mutex<Process>>,
    cancelled: Arc<AtomicBool>,
}

impl ProcessKill {
    /// `SIGTERM` now, `SIGKILL` after [`TERMINATION_GRACE`] if git is still
    /// running then. git removes the lock files it holds on `SIGTERM` — a
    /// `*.lock` beside a ref, `packed-refs.lock` — and cannot on `SIGKILL`,
    /// after which every later update of that ref fails with "another git
    /// process seems to be running" until the file is removed by hand; the
    /// escalation is for a git that does not go, and what it strands is the
    /// operation's to report (`stranded_locks`). A cancel while git waits on
    /// the network or a prompt — the common case — leaves at most a partial
    /// pack under `objects/pack/tmp_*`, which `gc` reaps.
    ///
    /// Never blocks: it tries for the lock and returns. The waiting thread
    /// does the rest ([`Process::poll`]), so this is safe to call from a
    /// thread that must not wait.
    pub(crate) fn kill(&self) {
        // Flagged first, so a `finish` that observes the exit sees why — and so a miss on
        // the lock below is not a lost cancel: the waiter sees the flag and terminates.
        self.cancelled.store(true, Ordering::Release);
        // `try_lock`: the waiter holds the lock only for a poll, which itself sends the
        // signal when it sees the flag, so a miss means the signal is going out anyway.
        if let Ok(mut process) = self.process.try_lock() {
            process.terminate();
        }
    }
}

fn lock(process: &Arc<Mutex<Process>>) -> std::sync::MutexGuard<'_, Process> {
    process.lock().unwrap_or_else(PoisonError::into_inner)
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
            reason = "fetch reads nothing from stdout; the first operation to will be status"
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
            reason = "a streamed invocation hands its stderr on line by line instead"
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
            reason = "fetch reads nothing from stdout; the first operation to will be status"
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
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use cairn_model::AskpassToken;

    use super::super::stub_git::{StubGit, discover_retrying};
    use super::{TERMINATION_GRACE, lock};
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

    /// Progress arrives one redraw at a time: git ends a meter's redraws with `\r`
    /// and its last one with `\n`, and both must reach the caller as lines.
    #[test]
    fn a_streamed_invocation_hands_stderr_on_a_redraw_at_a_time() {
        let stub = stub(
            "printf 'Receiving objects:  50%%\\rReceiving objects: 100%%, done.\\n' >&2; \
             printf 'From somewhere\\n * branch main -> FETCH_HEAD' >&2; \
             echo 'nothing to see' >&1",
        );
        let git = discover_retrying(stub.environment()).unwrap();
        let mut seen = Vec::new();
        let output = git
            .command()
            .arg("fetch")
            .stream()
            .unwrap()
            .finish(|line| seen.push(line.to_owned()))
            .unwrap();
        assert_eq!(
            seen,
            [
                "Receiving objects:  50%",
                "Receiving objects: 100%, done.",
                "From somewhere",
                " * branch main -> FETCH_HEAD",
            ]
        );
        assert!(output.stderr().contains("Receiving objects: 100%, done."));
        assert_eq!(
            output.stdout(),
            b"",
            "stdout is discarded on a streamed run"
        );
    }

    #[test]
    fn a_streamed_failure_carries_everything_stderr_said() {
        let stub =
            stub("echo 'fatal: could not read Username: terminal prompts disabled' >&2; exit 128");
        let git = discover_retrying(stub.environment()).unwrap();
        let mut seen = Vec::new();
        let error = git
            .command()
            .args(["fetch", "origin"])
            .stream()
            .unwrap()
            .finish(|line| seen.push(line.to_owned()))
            .unwrap_err();
        match error {
            crate::Error::GitFailed {
                arguments,
                status,
                stderr,
            } => {
                assert_eq!(arguments, "fetch origin");
                assert_eq!(status.code(), Some(128));
                assert!(stderr.contains("terminal prompts disabled"), "{stderr}");
            }
            other => panic!("expected the failure, got {other:?}"),
        }
        assert_eq!(seen.len(), 1);
    }

    /// A stub that hangs in a `sleep` child — or, where the system has no `sleep`, exits
    /// silently with a status no kill produces, before saying anything.
    const HANGING: &str = "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
                           echo 'hanging' >&2; sleep 30";

    /// PRD R4.3, the cancel half: a kill from another thread ends the wait promptly, the
    /// outcome says cancelled rather than failed, and the child is reaped, not orphaned.
    #[test]
    fn a_kill_from_another_thread_ends_a_hung_invocation_and_reaps_it() {
        // No `exec`: `sleep` is a child that inherits the pipe and outlives the killed shell,
        // the way `ssh` or the askpass helper outlives a killed git. The stub's PATH is its
        // own directory alone, so `sleep` is named through the system's — and checked for
        // BEFORE the stub says it is hanging: a stub dying of `sleep: not found` after that
        // line races the kill, and was killed first on most machines, so the test decided
        // nothing. Here such a stub exits without a word, and the kill never fires.
        let stub = stub(HANGING);
        let git = discover_retrying(stub.environment()).unwrap();
        let running = git.command().arg("fetch").stream().unwrap();
        let pid = running.id();
        let killer = running.killer();
        let mut seen = Vec::new();
        let (hung, hung_seen) = std::sync::mpsc::channel::<()>();
        let killing = std::thread::spawn(move || {
            // Kill once the child has said something, so it was really running; a stub that
            // exited before saying so ends this without a kill, and `finish` reports it.
            if hung_seen.recv().is_ok() {
                killer.kill();
            }
        });

        let started = std::time::Instant::now();
        let error = running
            .finish(|line| {
                seen.push(line.to_owned());
                if line == "hanging" {
                    let _ = hung.send(());
                }
            })
            .unwrap_err();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the kill did not end the wait: {:?}",
            started.elapsed()
        );
        assert!(
            matches!(
                &error,
                crate::Error::GitCancelled { arguments, stranded_locks }
                    if arguments == "fetch" && stranded_locks.is_empty()
            ),
            "a killed process reported {error:?}; the runner knows no repository to search"
        );
        assert_eq!(
            seen,
            ["hanging"],
            "the stub said more than it hung on: it was dying by itself, so the kill above \
             decided nothing"
        );
        killing.join().unwrap();
        assert!(
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "the killed git {pid} was not reaped (still in /proc, so a zombie or alive)"
        );
    }

    /// A kill after a clean exit changes nothing: the outcome is the success it was, so
    /// the refs it moved are not hidden behind "cancelled". The exit is observed (the
    /// process is a zombie, exited but not yet reaped) before the kill is sent.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_kill_after_a_clean_exit_reports_the_success() {
        let stub = stub("echo done >&2; exit 0");
        let git = discover_retrying(stub.environment()).unwrap();
        let running = git.command().arg("fetch").stream().unwrap();
        let pid = running.id();
        let killer = running.killer();
        let started = std::time::Instant::now();
        loop {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            if stat.contains(") Z ") {
                break;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(10),
                "the stub never exited"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        killer.kill();
        let output = running.finish(|_| {}).unwrap();
        assert_eq!(output.stderr(), "done");
    }

    /// The negative for the test above: a kill that ends a running process is a cancel.
    /// The same stub, killed only once it has said it is hanging, so a stub that died
    /// on its own is reported as the failure it was rather than mistaken for a kill.
    #[test]
    fn a_kill_that_ends_the_process_is_reported_as_cancelled() {
        let stub = stub(HANGING);
        let git = discover_retrying(stub.environment()).unwrap();
        let running = git.command().arg("fetch").stream().unwrap();
        let killer = running.killer();
        let outcome = running.finish(|line| {
            if line == "hanging" {
                killer.kill();
            }
        });
        assert!(
            matches!(outcome, Err(crate::Error::GitCancelled { .. })),
            "{outcome:?}"
        );
    }

    /// A stub that, on `SIGTERM`, says so, ends the child it was hanging in, and exits the
    /// way git does after removing its locks. `sleep` runs in the background and the shell
    /// `wait`s, because a shell waiting on a FOREGROUND child runs a trap only once that
    /// child ends (POSIX), which would make this stub look as if it ignored the signal;
    /// `wait` returns the moment a trapped signal arrives.
    const ENDING_ON_TERM: &str = "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
                                  sleep 30 & child=$!; \
                                  trap 'echo terminated >&2; kill $child; exit 143' TERM; \
                                  echo 'hanging' >&2; wait $child";

    /// A stub that ignores `SIGTERM` — `trap ''` — and hangs in one-second sleeps, so
    /// only `SIGKILL` ends it and nothing it spawned outlives that by more than a second.
    const IGNORING_TERM: &str = "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
                                 trap '' TERM; echo 'hanging' >&2; \
                                 while :; do sleep 1; done";

    /// Issue #19, the first half: a cancel is `SIGTERM` first, and a git that acts on it
    /// is not `SIGKILL`ed. Decisive because the stub reports the signal it got — a
    /// `SIGKILL` runs no trap, so "terminated" on stderr is `SIGTERM` and nothing else —
    /// and because it ends well inside the grace period, which only a process that
    /// exited on the first signal does (the ignoring stub below is what the grace period
    /// costs). A stub with no `sleep` exits 99 before saying anything, and the kill never
    /// fires.
    #[test]
    fn a_cancel_sends_sigterm_first_and_a_process_that_exits_on_it_is_not_killed() {
        let stub = stub(ENDING_ON_TERM);
        let git = discover_retrying(stub.environment()).unwrap();
        let running = git.command().arg("fetch").stream().unwrap();
        let pid = running.id();
        let killer = running.killer();
        let mut seen = Vec::new();
        let (hung, hung_seen) = std::sync::mpsc::channel::<Instant>();
        let killing = std::thread::spawn(move || hung_seen.recv().ok().inspect(|_| killer.kill()));
        let error = running
            .finish(|line| {
                seen.push(line.to_owned());
                if line == "hanging" {
                    let _ = hung.send(Instant::now());
                }
            })
            .unwrap_err();
        let Some(cancelled_at) = killing.join().unwrap() else {
            panic!("the stub never said it was hanging: {seen:?}");
        };
        let took = cancelled_at.elapsed();
        assert!(
            matches!(&error, crate::Error::GitCancelled { .. }),
            "a terminated process reported {error:?}"
        );
        assert_eq!(
            seen,
            ["hanging", "terminated"],
            "the stub did not report SIGTERM: it was ended some other way, or died by itself"
        );
        assert!(
            took < TERMINATION_GRACE,
            "a process that exited on SIGTERM was waited on for {took:?}, the grace period \
             or longer: the cancel did not end when the process did"
        );
        assert!(
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "the terminated git {pid} was not reaped"
        );
    }

    /// How long a test waits for a cancelled `finish` before calling it hung: the grace
    /// period, the poll, and margin for a loaded machine.
    const CANCEL_DEADLINE: Duration = Duration::from_secs(7);

    /// Runs `finish` on a thread of its own and cancels once the stub has said it is
    /// hanging (after `after`, so a cancel can be made to land at a chosen moment); hands
    /// back the outcome, what the stub said, and how long the cancel took to end the
    /// wait — or panics at [`CANCEL_DEADLINE`], so a runner that never ends a process
    /// fails here rather than hanging the suite for the stub's lifetime.
    fn cancelled_after_hanging(
        running: super::Running,
        after: Duration,
    ) -> (Result<super::Output, crate::Error>, Vec<String>, Duration) {
        let killer = running.killer();
        let (hung, hung_seen) = std::sync::mpsc::channel::<()>();
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut seen = Vec::new();
            let outcome = running.finish(|line| {
                seen.push(line.to_owned());
                if line == "hanging" {
                    let _ = hung.send(());
                }
            });
            let _ = done.send((outcome, seen));
        });
        hung_seen
            .recv_timeout(CANCEL_DEADLINE)
            .unwrap_or_else(|_| panic!("the stub never said it was hanging"));
        std::thread::sleep(after);
        let cancelled_at = Instant::now();
        killer.kill();
        let (outcome, seen) = finished.recv_timeout(CANCEL_DEADLINE).unwrap_or_else(|_| {
            panic!(
                "the cancel did not end the wait within {CANCEL_DEADLINE:?}: the runner never \
                 ended the process"
            )
        });
        (outcome, seen, cancelled_at.elapsed())
    }

    fn reaped(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    }

    /// Issue #19, the escalation: a git that ignores `SIGTERM` is `SIGKILL`ed once the
    /// grace period has passed, and the cancel is still a cancel. Decisive because the
    /// stub proves it ignored the first signal by outliving the grace period — a stub
    /// that died of `SIGTERM` would end in milliseconds (the test above) — and because
    /// `finish` runs under a deadline, so a runner that never escalated fails here rather
    /// than hanging for the stub's lifetime.
    #[test]
    fn a_process_that_ignores_sigterm_is_killed_once_the_grace_period_has_passed() {
        let stub = stub(IGNORING_TERM);
        let git = discover_retrying(stub.environment()).unwrap();
        let running = git.command().arg("fetch").stream().unwrap();
        let pid = running.id();
        let (outcome, seen, took) = cancelled_after_hanging(running, Duration::ZERO);
        assert!(
            matches!(&outcome, Err(crate::Error::GitCancelled { .. })),
            "a killed process reported {outcome:?}"
        );
        assert_eq!(seen, ["hanging"], "the stub was dying by itself");
        assert!(
            took >= TERMINATION_GRACE,
            "ended after {took:?}: the stub did not survive SIGTERM, so this decided nothing \
             about the escalation"
        );
        assert!(reaped(pid), "the killed git {pid} was not reaped");
    }

    /// A stub that ignores `SIGTERM` and closes its stderr the moment it has said it is
    /// hanging, so the runner's reader thread ends before the cancel arrives and the
    /// escalation must come from the reap loop, not the streaming one.
    const IGNORING_TERM_SILENTLY: &str = "PATH=/usr/bin:/bin; command -v sleep >/dev/null || \
                                          exit 99; trap '' TERM; echo 'hanging' >&2; \
                                          exec 2>&-; while :; do sleep 1; done";

    /// The escalation is the waiter's wherever it is waiting: a cancel that lands after
    /// git has closed its stderr — so `finish` is in its reap loop, not its streaming
    /// loop — is still `SIGTERM`, then `SIGKILL` after the grace period. The stub closes
    /// stderr right after speaking and the cancel is sent a little later, so it lands in
    /// the reap loop on any machine that is not pathologically slow; on one that is, the
    /// streaming loop handles it and the test decides nothing extra, never the wrong thing.
    #[test]
    fn a_cancel_that_lands_after_stderr_closed_is_still_escalated_to_sigkill() {
        let stub = stub(IGNORING_TERM_SILENTLY);
        let git = discover_retrying(stub.environment()).unwrap();
        let running = git.command().arg("fetch").stream().unwrap();
        let pid = running.id();
        let (outcome, seen, took) = cancelled_after_hanging(running, Duration::from_millis(300));
        assert!(
            matches!(&outcome, Err(crate::Error::GitCancelled { .. })),
            "{outcome:?}"
        );
        assert_eq!(seen, ["hanging"]);
        assert!(
            took >= TERMINATION_GRACE,
            "the stub did not survive SIGTERM: {took:?}"
        );
        assert!(reaped(pid), "the killed git {pid} was not reaped");
    }

    /// A cancel that arrives after the process has been reaped sends nothing: the pid is
    /// the system's to reuse by then, and `std`'s own `kill` refuses for that reason —
    /// the `nix` path must too. Observed through the runner's own state, since a signal
    /// to a recycled pid is not something a test can watch for from outside.
    #[test]
    fn a_cancel_after_the_reap_signals_nothing() {
        let stub = stub("echo done >&2; exit 0");
        let git = discover_retrying(stub.environment()).unwrap();
        let running = git.command().arg("fetch").stream().unwrap();
        let pid = running.id();
        let process = Arc::clone(&running.process);
        let killer = running.killer();
        running.finish(|_| {}).unwrap();
        assert!(reaped(pid));
        killer.kill();
        let process = lock(&process);
        assert_eq!(
            process.terminated_at, None,
            "a SIGTERM was sent to pid {pid} after it was reaped — the system may have given \
             that pid to another process by now"
        );
        assert!(!process.killed);
    }

    /// A kill that could not take the lock is not a lost cancel: the flag is set first,
    /// and the waiter, which sees it on its next poll, sends the signal itself. Pinned
    /// through the runner's own pieces since the race cannot be staged from outside: the
    /// lock is held across the kill, as the waiter's poll holds it.
    #[test]
    fn a_kill_that_misses_the_lock_is_finished_by_the_waiter() {
        let stub = stub(ENDING_ON_TERM);
        let git = discover_retrying(stub.environment()).unwrap();
        let running = git.command().arg("fetch").stream().unwrap();
        let killer = running.killer();
        let mut seen = Vec::new();
        let (hung, hung_seen) = std::sync::mpsc::channel::<()>();
        let held = Arc::clone(&running.process);
        let killing = std::thread::spawn(move || {
            if hung_seen.recv().is_err() {
                return false;
            }
            let guard = lock(&held);
            killer.kill();
            let sent_while_held = guard.terminated_at.is_some();
            drop(guard);
            !sent_while_held
        });
        let error = running
            .finish(|line| {
                seen.push(line.to_owned());
                if line == "hanging" {
                    let _ = hung.send(());
                }
            })
            .unwrap_err();
        assert!(
            killing.join().unwrap(),
            "the kill took the lock the test was holding, so the miss was not staged"
        );
        assert!(
            matches!(&error, crate::Error::GitCancelled { .. }),
            "{error:?}"
        );
        assert_eq!(
            seen,
            ["hanging", "terminated"],
            "the waiter did not send the SIGTERM the killer could not"
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
