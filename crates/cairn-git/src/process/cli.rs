//! Building and running one `git` invocation.
//!
//! The only place in Cairn a process is run. The [`Command`] itself comes from
//! [`GitEnvironment::command`], so it already carries the explicit environment
//! and nothing inherited; this module adds the arguments, the directory and the
//! standard streams, waits for the exit, and turns a failure into an [`Error`]
//! that carries git's own diagnostic.
//!
//! An invocation is a [`GitCommand`] of one of two kinds, fixed when it is
//! built and never changed after: a [`Read`], which carries no askpass token
//! because it has no field to hold one, or a [`Write`], which holds the
//! [`WriteAuthority`] it was built from and, when the operation may prompt,
//! its token. The kind is what chooses the environment's profile, so a caller
//! cannot pick a write's environment for a read or the reverse.
//!
//! One way to run one: [`GitCommand::start`], the runner (`runner.rs`). The
//! process leads a new process group, every pipe it uses has a thread of its
//! own, stdout is handed over as it arrives, stderr kept as a bounded tail, and
//! the invocation ends by a cancel signal, a kill handle or a drop. The version
//! probe and fetch run on it like everything else.
//!
//! Standard input is closed unless the caller gives bytes for it
//! ([`GitCommand::input`]), which are written and then closed. Together with
//! `GIT_TERMINAL_PROMPT=0` that is what stops `git` itself from waiting for
//! something nobody will type. It does not reach `ssh`, which prompts on
//! `/dev/tty` directly — a host-key confirmation or a key passphrase from a
//! Cairn launched in a terminal lands on that terminal; closing that path is
//! `SSH_ASKPASS_REQUIRE=force`, which arrives with the askpass helper.
//!
//! Everything here is `pub(crate)`, and [`GitCommand::new`] is visible to
//! `process/` alone: the crate's public surface is named operations, never a
//! raw invocation, and inside the crate an invocation is reached only through
//! [`super::GitBinary`]'s two builders, which the guard suite lets `ops/` and
//! `reads/` name and nothing else.
//!
//! [`Command`]: std::process::Command

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use cairn_model::AskpassToken;

use super::GitEnvironment;
use super::environment::Profile;
use super::group::{Spawner, os_thread};
use super::registry::{Processes, Registration};
use super::runner::{Invocation, watch};
use crate::ops::WriteAuthority;
use crate::ops::stranded_locks::stranded_locks;
use crate::{Error, Repository};

/// One invocation of the kind `K` ([`Read`] or [`Write`]), built up and then
/// started once with [`GitCommand::start`].
#[derive(Debug)]
pub(crate) struct GitCommand<'a, K> {
    program: &'a Path,
    environment: &'a GitEnvironment,
    kind: K,
    /// The repository's location as global options ahead of the verb —
    /// `--git-dir` and, when it has one, `--work-tree` — set with the
    /// directory ([`GitCommand::in_repository`]); empty for an invocation in
    /// no repository, or one git is left to find by discovery.
    location: Vec<OsString>,
    arguments: Vec<OsString>,
    directory: Option<PathBuf>,
    /// Where a write's lock files live, for its outcome; set with the directory.
    dirs: Option<GitDirs>,
    /// What [`GitCommand::start`] writes to stdin before closing it; `None`
    /// leaves stdin closed from the start.
    input: Option<Vec<u8>>,
    /// The repository's registry and log, which the invocation is booked in
    /// from its spawn to its end; set with the directory (`registry.rs`).
    processes: Option<Arc<Processes>>,
}

/// A repository's git directory and the common directory it shares (the same
/// path unless it is a linked worktree): the two places its lock files live.
#[derive(Debug, Clone)]
pub(crate) struct GitDirs {
    git_dir: PathBuf,
    common_dir: PathBuf,
}

impl GitDirs {
    /// Every `*.lock` under the two directories now.
    fn locks(&self) -> Vec<PathBuf> {
        stranded_locks(&self.git_dir, &self.common_dir)
    }
}

/// An invocation that reads: optional locks off, and no askpass token.
#[derive(Debug)]
pub(crate) struct Read;

/// An invocation that may write, built from the [`WriteAuthority`] only `ops/`
/// can construct and holding it for its life.
#[derive(Debug)]
pub(crate) struct Write {
    _authority: WriteAuthority,
    token: Option<AskpassToken>,
}

impl Write {
    pub(super) fn new(authority: WriteAuthority) -> Self {
        Self {
            _authority: authority,
            token: None,
        }
    }
}

/// What an invocation's kind decides: what it adds to the base environment,
/// and what its cancellation and failure report.
pub(crate) trait Kind {
    fn profile(&self) -> Profile<'_>;

    /// The error for a cancelled invocation, built after the reap.
    fn cancelled(&self, arguments: String, dirs: Option<&GitDirs>) -> Error;

    /// The lock files a failed invocation reports as present.
    fn present_locks(&self, dirs: Option<&GitDirs>) -> Vec<PathBuf>;
}

/// A read writes nothing, so it leaves nothing behind and fails on no lock of
/// its own: its cancellation is that and nothing else (R5.2).
impl Kind for Read {
    fn profile(&self) -> Profile<'_> {
        Profile::Read
    }

    fn cancelled(&self, arguments: String, _: Option<&GitDirs>) -> Error {
        Error::GitReadCancelled { arguments }
    }

    fn present_locks(&self, _: Option<&GitDirs>) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// A write's cancellation lists the lock files present once it is reaped —
/// what a `SIGKILL` stranded, or a lock another git holds — and its failure
/// names those present, which is what a write fails on (R5.2, R5.3). Listed,
/// never removed.
impl Kind for Write {
    fn profile(&self) -> Profile<'_> {
        Profile::Write {
            token: self.token.as_ref(),
        }
    }

    fn cancelled(&self, arguments: String, dirs: Option<&GitDirs>) -> Error {
        Error::GitCancelled {
            arguments,
            stranded_locks: dirs.map(GitDirs::locks).unwrap_or_default(),
        }
    }

    fn present_locks(&self, dirs: Option<&GitDirs>) -> Vec<PathBuf> {
        dirs.map(GitDirs::locks).unwrap_or_default()
    }
}

impl<'a, K: Kind> GitCommand<'a, K> {
    /// Visible to `process/` alone: everything else starts from
    /// [`super::GitBinary::read_invocation`] or
    /// [`super::GitBinary::write_invocation`].
    pub(super) fn new(program: &'a Path, environment: &'a GitEnvironment, kind: K) -> Self {
        Self {
            program,
            environment,
            kind,
            location: Vec::new(),
            arguments: Vec::new(),
            directory: None,
            dirs: None,
            input: None,
            processes: None,
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

    /// Runs inside `repo`: its working tree, or the git directory of a bare one,
    /// with the repository named to git rather than left to its discovery
    /// ([`repository_location`]). The invocation is booked in `repo`'s registry
    /// from its spawn to its end, and written to its command log once, however
    /// it ends.
    pub(crate) fn in_repository(mut self, repo: &Repository) -> Self {
        let inner = repo.inner();
        let absolute = |path: &Path| inner.current_dir().join(path);
        self.location = repository_location(
            inner.git_dir_trust() == gix::sec::Trust::Full,
            &absolute(repo.git_dir()),
            repo.workdir().map(absolute).as_deref(),
        );
        self.directory = Some(repo.workdir().unwrap_or(repo.git_dir()).to_owned());
        self.dirs = Some(GitDirs {
            git_dir: repo.git_dir().to_owned(),
            common_dir: repo.inner().common_dir().to_owned(),
        });
        self.processes = Some(Arc::clone(repo.processes()));
        self
    }

    /// Bytes [`GitCommand::start`] writes to the process's stdin, on a thread
    /// of their own, and then closes it: git reads a patch or a list to its
    /// end before it acts, so a stdin left open would hold it forever.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the first verb fed on stdin arrives with staging; tests drive it today"
        )
    )]
    pub(crate) fn input(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.input = Some(bytes.into());
        self
    }

    /// Starts the process as the leader of a new process group, with a thread
    /// on each pipe it uses, and hands it back running (`runner.rs`). A process
    /// that never started is [`Error::GitNotStarted`]; one whose threads could
    /// not start is ended and reaped, then [`Error::GitUnwatched`].
    pub(crate) fn start(self) -> Result<Invocation<K>, Error> {
        self.start_with(&os_thread)
    }

    /// Starts and collects to the end, for a test whose stub or real `git`
    /// writes little: never cancelled, and a ceiling far above what any test
    /// prints.
    #[cfg(test)]
    pub(crate) fn collected(self) -> Result<Output, Error> {
        self.start()?
            .collect(&crate::CancelSignal::new(), 16 * 1024 * 1024, |_| {})
    }

    /// [`GitCommand::start`] with no thread able to start, for a test outside
    /// `process/` of what a runner-ended invocation reports.
    #[cfg(test)]
    pub(crate) fn start_without_threads(self) -> Result<Invocation<K>, Error> {
        self.start_with(&|name, _| Err(std::io::Error::other(format!("no thread for {name}"))))
    }

    /// [`GitCommand::start`] with the thread starter given, so a test can make
    /// one fail.
    pub(super) fn start_with(self, spawner: &Spawner) -> Result<Invocation<K>, Error> {
        // The repository's location goes ahead of the verb on the command line
        // and nowhere else: a log is the repository's own, and an error is
        // about the verb, so both record the verb and its arguments.
        let mut command = self.environment.command(self.program, self.kind.profile());
        command
            .args(&self.location)
            .args(&self.arguments)
            .stdin(if self.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Its own group, so ending it reaches everything it started (R3.1).
            .process_group(0);
        if let Some(directory) = &self.directory {
            command.current_dir(directory);
        }
        let registration = self.processes.as_ref().map(|processes| {
            Registration::new(
                processes,
                self.arguments
                    .iter()
                    .map(|argument| argument.to_string_lossy().into_owned())
                    .collect(),
                self.directory.clone(),
            )
        });
        let child = match command.spawn() {
            Ok(child) => child,
            Err(source) => {
                if let Some(registration) = registration {
                    registration.not_started();
                }
                return Err(Error::GitNotStarted {
                    program: self.program.to_owned(),
                    source,
                });
            }
        };
        watch(
            child,
            self.input,
            self.kind,
            describe(&self.arguments),
            self.dirs,
            registration,
            spawner,
        )
    }
}

impl GitCommand<'_, Write> {
    /// A write that may ask the user for a secret: `token` is what the
    /// askpass helper presents to the channel that issued it. Without one the
    /// helper is still what git runs, and it fails closed. There is no such
    /// method on a read.
    pub(crate) fn authorized_by(mut self, token: &AskpassToken) -> Self {
        self.kind.token = Some(token.clone());
        self
    }
}

/// The global options that name a repository to git: `--git-dir`, and
/// `--work-tree` when it has a working tree — given only when `trusted`.
///
/// Without them git finds the repository by discovery from the directory it
/// runs in, and discovery can find a different one: a working tree whose git
/// directory lives elsewhere (`core.worktree`), sitting inside another
/// repository's working tree, is discovered as that enclosing repository; and
/// under `safe.bareRepository=explicit` git refuses to discover a bare
/// repository at all. Naming it is what makes the repository git reads the
/// one Cairn opened.
///
/// But a git directory named explicitly is one git does not check the
/// ownership of — `safe.directory` guards discovery only (reproduced with
/// git 2.56: `GIT_TEST_ASSUME_DIFFERENT_OWNER=1 git log` refuses with
/// "dubious ownership", and the same command given `--git-dir` answers). So
/// the options are given only for a repository gix opened with full trust,
/// which is gix's reading of git's own rule — the git directory and the
/// working tree owned by the user, or listed under `safe.directory` in the
/// configuration git protects. A repository opened with reduced trust is
/// left to git's discovery, as before, so git's own check decides it and a
/// repository git would refuse to read is refused.
fn repository_location(trusted: bool, git_dir: &Path, workdir: Option<&Path>) -> Vec<OsString> {
    if !trusted {
        return Vec::new();
    }
    let option = |name: &str, path: &Path| {
        let mut option = OsString::from(name);
        option.push(path);
        option
    };
    let mut location = vec![option("--git-dir=", git_dir)];
    if let Some(workdir) = workdir {
        location.push(option("--work-tree=", workdir));
    }
    location
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
    pub(super) fn new(stdout: Vec<u8>, stderr: String) -> Self {
        Self { stdout, stderr }
    }

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
            reason = "fetch hands its stderr on line by line, and nothing yet reads the tail \
                      a success retained"
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
/// runner is `pub(crate)`: nothing outside `ops` may run a raw invocation. How an
/// invocation is cancelled and ended is `runner.rs`'s to pin; these are what the
/// builder gives the process — its environment, its arguments, its directory, its
/// stdin — and what the runner hands back of what it wrote.
#[cfg(all(test, unix))]
mod stub_tests {
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::path::Path;

    use super::super::stub_git::{StubGit, discover_retrying, printed_environment};
    use crate::{CancelSignal, Repository};

    /// Enough for anything these stubs print.
    const CEILING: usize = 64 * 1024;

    /// Answers `--version`, then runs `rest` for anything else.
    fn stub(rest: &str) -> StubGit {
        StubGit::with_git(&format!(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n{rest}"
        ))
    }

    /// PRD B1 and G3 end to end, for a read: what the child actually sees is
    /// the base, the editor pinned, optional locks and lazy fetching off, no
    /// token — spelled out variable by variable — and nothing from this
    /// process. The test process is full of `CARGO_*` variables, which is what
    /// makes a leak visible.
    #[test]
    fn a_read_sees_exactly_the_read_environment_and_nothing_inherited() {
        let stub = stub("exec /usr/bin/env");
        let environment = stub.environment_with(|name| match name {
            "HOME" => Some(OsString::from("/nonexistent/home-from-cairn")),
            _ => None,
        });
        let path = environment
            .get("PATH")
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap();
        let git = discover_retrying(environment).unwrap();
        let output = git
            .read_invocation()
            .arg("print-environment")
            .start()
            .unwrap()
            .collect(&CancelSignal::new(), CEILING, |_| {})
            .unwrap();
        let seen = printed_environment(&output.stdout_text());
        let expected: BTreeMap<String, String> = [
            ("GIT_ASKPASS", StubGit::HELPER),
            ("GIT_EDITOR", "false"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_SEQUENCE_EDITOR", "false"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("HOME", "/nonexistent/home-from-cairn"),
            ("PATH", path.as_str()),
            ("SSH_ASKPASS", StubGit::HELPER),
            ("SSH_ASKPASS_REQUIRE", "force"),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
        assert_eq!(
            seen, expected,
            "a read must see exactly the read environment: no token, nothing inherited"
        );
    }

    /// The version probe is a read like any other, and runs with that
    /// environment: the stub writes what it was given to a file beside itself
    /// before answering, since the probe reads nothing but the version line.
    #[test]
    fn the_version_probe_runs_with_the_read_environment() {
        let stub = StubGit::with_git_from(|directory| {
            format!(
                "if [ \"$1\" = --version ]; then /usr/bin/env > '{}'; \
                 echo 'git version 2.30.0'; exit 0; fi; exit 1",
                directory.join("probe-environment").display()
            )
        });
        let dump = stub.directory().join("probe-environment");
        let environment = stub.environment();
        let path = environment
            .get("PATH")
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap();
        discover_retrying(environment).unwrap();
        let seen = printed_environment(&std::fs::read_to_string(&dump).unwrap());
        let expected: BTreeMap<String, String> = [
            ("GIT_ASKPASS", StubGit::HELPER),
            ("GIT_EDITOR", "false"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_SEQUENCE_EDITOR", "false"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("PATH", path.as_str()),
            ("SSH_ASKPASS", StubGit::HELPER),
            ("SSH_ASKPASS_REQUIRE", "force"),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
        assert_eq!(seen, expected);
    }

    /// The probe reads the version from stdout and nothing else: a stub that
    /// prints a version git never would is refused as unreadable, with what it
    /// printed, and one that fails is the failure it was. Caught by: the probe
    /// reading stderr, or treating a failed `--version` as a version.
    #[test]
    fn the_version_probe_reports_an_unreadable_answer_and_a_failure_as_such() {
        let unreadable = StubGit::with_git("echo 'not a version'; echo 'git version 2.30.0' >&2");
        match discover_retrying(unreadable.environment()) {
            Err(crate::Error::GitVersionUnreadable { output, .. }) => {
                assert_eq!(output, "not a version");
            }
            other => panic!("expected the unreadable version, got {other:?}"),
        }
        let failing = StubGit::with_git("echo 'git version 2.30.0'; exit 3");
        match discover_retrying(failing.environment()) {
            Err(crate::Error::GitFailed {
                arguments, status, ..
            }) => {
                assert_eq!(arguments, "--version");
                assert_eq!(status.code(), Some(3));
            }
            other => panic!("expected the failure, got {other:?}"),
        }
    }

    #[test]
    fn arguments_arrive_in_order_and_nul_records_split() {
        let stub = stub("printf '%s\\0%s\\0' \"$1\" \"$2\"");
        let git = discover_retrying(stub.environment()).unwrap();
        let output = git
            .read_invocation()
            .args(["rev-parse", "--show-toplevel"])
            .start()
            .unwrap()
            .collect(&CancelSignal::new(), CEILING, |_| {})
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
        let ran = |command: super::GitCommand<'_, super::Read>| {
            let output = command
                .start()
                .unwrap()
                .collect(&CancelSignal::new(), CEILING, |_| {})
                .unwrap();
            std::fs::canonicalize(output.stdout_text().trim()).unwrap()
        };

        assert_eq!(ran(git.read_invocation().in_repository(&repo)), expected);
        // Without `in_repository`, the process runs wherever this one does.
        assert_eq!(
            ran(git.read_invocation()),
            std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap()
        );
    }

    /// Every invocation in a repository names it to git ahead of the verb — its git
    /// directory and its working tree, absolute — and the repository's own command
    /// log records the verb and its arguments, as it always has. Caught by: a
    /// repository left to git's discovery, which finds an enclosing repository or
    /// refuses a bare one under `safe.bareRepository=explicit` (both against real
    /// git in `tests/diff/changes.rs` and `tests/fetch.rs`), or the location put
    /// after the verb, where git reads it as the verb's own option.
    #[test]
    fn a_trusted_repository_is_named_to_git_ahead_of_the_verb() {
        let stub = stub("printf '%s\\0' \"$@\"");
        let git = discover_retrying(stub.environment()).unwrap();
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        assert_eq!(
            repo.inner().git_dir_trust(),
            gix::sec::Trust::Full,
            "this checkout is the user's own, so it is trusted"
        );
        let git_dir = std::fs::canonicalize(repo.git_dir()).unwrap();
        let workdir = std::fs::canonicalize(repo.workdir().unwrap()).unwrap();
        let output = git
            .read_invocation()
            .in_repository(&repo)
            .args(["diff-tree", "--raw"])
            .start()
            .unwrap()
            .collect(&CancelSignal::new(), CEILING, |_| {})
            .unwrap();
        let records: Vec<String> = output
            .records()
            .map(|record| String::from_utf8_lossy(record).into_owned())
            .collect();
        assert_eq!(records.len(), 4, "{records:?}");
        let named = |option: &str, record: &str| {
            let path = record
                .strip_prefix(option)
                .unwrap_or_else(|| panic!("{record:?} is not {option}"));
            assert!(Path::new(path).is_absolute(), "{record:?} is relative");
            std::fs::canonicalize(path).unwrap()
        };
        assert_eq!(named("--git-dir=", &records[0]), git_dir);
        assert_eq!(named("--work-tree=", &records[1]), workdir);
        assert_eq!(records[2..], ["diff-tree", "--raw"]);

        let log = repo.processes().log();
        assert_eq!(log.len(), 1, "{log:?}");
        assert_eq!(log[0].arguments, ["diff-tree", "--raw"]);
    }

    /// The other half of the rule, which no fixture can reach without a second user:
    /// a repository gix trusts less than fully is left to git's discovery, so git's
    /// own ownership check (`safe.directory`) still decides it. Caught by: naming
    /// every repository whatever its trust, which bypasses that check.
    #[test]
    fn a_repository_trusted_less_than_fully_is_left_to_gits_discovery() {
        let git_dir = Path::new("/somewhere/repo/.git");
        let workdir = Path::new("/somewhere/repo");
        assert!(super::repository_location(false, git_dir, Some(workdir)).is_empty());
        assert_eq!(
            super::repository_location(true, git_dir, Some(workdir)),
            [
                "--git-dir=/somewhere/repo/.git",
                "--work-tree=/somewhere/repo"
            ]
        );
        assert_eq!(
            super::repository_location(true, git_dir, None),
            ["--git-dir=/somewhere/repo/.git"],
            "a bare repository has no working tree to name"
        );
    }

    /// Progress arrives one redraw at a time: git ends a meter's redraws with `\r`
    /// and its last one with `\n`, and both must reach the caller as lines. What
    /// git wrote to stdout goes to the stdout callback, never to progress.
    #[test]
    fn a_finished_invocation_hands_stderr_on_a_redraw_at_a_time() {
        let stub = stub(
            "printf 'Receiving objects:  50%%\\rReceiving objects: 100%%, done.\\n' >&2; \
             printf 'From somewhere\\n * branch main -> FETCH_HEAD' >&2; \
             echo 'nothing to see' >&1",
        );
        let git = discover_retrying(stub.environment()).unwrap();
        let mut seen = Vec::new();
        let mut stdout = Vec::new();
        let output = git
            .read_invocation()
            .arg("fetch")
            .start()
            .unwrap()
            .finish(
                &CancelSignal::new(),
                |chunk| stdout.extend_from_slice(chunk),
                |line| seen.push(line.to_owned()),
            )
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
        assert_eq!(stdout, b"nothing to see\n");
        assert_eq!(
            output.stdout(),
            b"",
            "finish hands stdout to its callback, never into the output"
        );
    }

    #[test]
    fn a_finished_failure_carries_everything_stderr_said() {
        let stub =
            stub("echo 'fatal: could not read Username: terminal prompts disabled' >&2; exit 128");
        let git = discover_retrying(stub.environment()).unwrap();
        let mut seen = Vec::new();
        let error = git
            .read_invocation()
            .args(["fetch", "origin"])
            .start()
            .unwrap()
            .finish(
                &CancelSignal::new(),
                |_| {},
                |line| seen.push(line.to_owned()),
            )
            .unwrap_err();
        match error {
            crate::Error::GitFailed {
                arguments,
                status,
                stderr,
                ..
            } => {
                assert_eq!(arguments, "fetch origin");
                assert_eq!(status.code(), Some(128));
                assert!(stderr.contains("terminal prompts disabled"), "{stderr}");
            }
            other => panic!("expected the failure, got {other:?}"),
        }
        assert_eq!(seen.len(), 1);
    }

    /// Caught by: `Stdio::null()` becoming `inherit()`, which is how a git waiting on a
    /// pipe nobody writes to would come back. Linux only: it reads `/proc`.
    #[cfg(target_os = "linux")]
    #[test]
    fn standard_input_is_closed_not_inherited() {
        let stub = stub("PATH=/usr/bin:/bin readlink /proc/$$/fd/0");
        let git = discover_retrying(stub.environment()).unwrap();
        let output = git
            .read_invocation()
            .start()
            .unwrap()
            .collect(&CancelSignal::new(), CEILING, |_| {})
            .unwrap();
        assert_eq!(output.stdout_text().trim(), "/dev/null");
    }
}
