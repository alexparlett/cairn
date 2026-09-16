//! Startup discovery of `git`, against stubs on a `PATH` the test controls.
//!
//! Nothing here depends on the machine's git: each case builds its own `git`
//! script in a fresh directory and hands that directory to the backend as the
//! whole `PATH`, so "absent" is an empty directory and "too old" is a script.
//! The runner itself is `pub(crate)`, so what a found `git` is then handed is
//! tested inside the crate (`ops/cli.rs`), with a copy of this stub helper.
#![cfg(unix)]

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use cairn_git::Error;
use cairn_git::ops::{GitBinary, GitEnvironment, GitVersion};

/// A directory holding one stub `git`, removed when the test ends.
struct StubPath {
    directory: PathBuf,
}

impl StubPath {
    /// An empty directory: no `git` at all.
    fn empty() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("cairn-git-stub-{}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory)
            .unwrap_or_else(|e| panic!("could not make {}: {e}", directory.display()));
        Self { directory }
    }

    /// A `git` whose whole behaviour is `script`, run by `/bin/sh`.
    fn with_git(script: &str) -> Self {
        let stub = Self::empty();
        let git = stub.git_path();
        std::fs::write(&git, format!("#!/bin/sh\n{script}\n"))
            .unwrap_or_else(|e| panic!("could not write {}: {e}", git.display()));
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|e| panic!("could not chmod {}: {e}", git.display()));
        stub
    }

    /// A `git` that answers `--version` with `printed` and nothing else.
    fn printing_version(printed: &str) -> Self {
        Self::with_git(&format!("echo '{printed}'"))
    }

    fn git_path(&self) -> PathBuf {
        self.directory.join("git")
    }

    /// An environment whose `PATH` is this directory alone; `parent` supplies the rest.
    fn environment_with(&self, parent: impl Fn(&str) -> Option<OsString>) -> GitEnvironment {
        let directory = self.directory.clone();
        GitEnvironment::new(move |name| {
            if name == "PATH" {
                Some(directory.clone().into_os_string())
            } else {
                parent(name)
            }
        })
    }

    fn environment(&self) -> GitEnvironment {
        self.environment_with(|_| None)
    }

    fn discover(&self) -> Result<GitBinary, Error> {
        discover_retrying(self.environment())
    }
}

/// [`GitBinary::discover_with`], retried while the stub is "text file busy".
///
/// Tests run in parallel, and a fork in another thread inherits this thread's
/// write descriptor to a stub it is still creating for the microseconds until
/// that child execs; executing the stub in that window fails with ETXTBSY. It
/// is a property of the test harness, not of the backend, so it is absorbed here.
fn discover_retrying(environment: GitEnvironment) -> Result<GitBinary, Error> {
    let mut attempts = 0;
    loop {
        match GitBinary::discover_with(environment.clone()) {
            Err(Error::GitNotStarted { source, .. })
                if source.raw_os_error() == Some(libc_etxtbsy()) && attempts < 50 =>
            {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            other => return other,
        }
    }
}

/// `ETXTBSY` on Linux and macOS alike.
const fn libc_etxtbsy() -> i32 {
    26
}

impl Drop for StubPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn failure(result: Result<GitBinary, Error>, case: &str) -> Error {
    match result {
        Ok(found) => panic!("{case}: accepted {found:?}"),
        Err(error) => error,
    }
}

/// PRD B2: a missing git is refused, and the message names the required version.
#[test]
fn a_missing_git_is_refused_naming_the_required_version() {
    let stub = StubPath::empty();
    let error = failure(stub.discover(), "no git on PATH");
    match &error {
        Error::GitNotFound { searched, required } => {
            assert_eq!(searched, std::slice::from_ref(&stub.directory));
            assert_eq!(*required, GitVersion::MINIMUM);
        }
        other => panic!("expected GitNotFound, got {other:?}"),
    }
    let message = error.to_string();
    assert!(message.contains("2.30.0"), "{message}");
    assert!(
        message.contains(&stub.directory.display().to_string()),
        "the message does not say where it looked: {message}"
    );
}

#[test]
fn an_unset_path_is_reported_rather_than_searched_nowhere_silently() {
    let error = failure(
        discover_retrying(GitEnvironment::new(|_| None)),
        "PATH unset",
    );
    assert!(
        matches!(&error, Error::GitNotFound { searched, .. } if searched.is_empty()),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("PATH is unset"), "{message}");
    assert!(message.contains("2.30.0"), "{message}");
}

/// PRD B2: one minor version under the floor is refused, and the message names both versions.
#[test]
fn a_git_older_than_the_floor_is_refused_naming_both_versions() {
    let stub = StubPath::printing_version("git version 2.29.2");
    let error = failure(stub.discover(), "git 2.29.2");
    match &error {
        Error::GitTooOld {
            path,
            found,
            required,
        } => {
            assert_eq!(path, &stub.git_path());
            assert_eq!(found.to_string(), "2.29.2");
            assert_eq!(*required, GitVersion::MINIMUM);
        }
        other => panic!("expected GitTooOld, got {other:?}"),
    }
    let message = error.to_string();
    assert!(
        message.contains("2.29.2") && message.contains("2.30.0"),
        "{message}"
    );
}

/// The case people skip: a `git` that runs but prints nothing parseable must not panic.
#[test]
fn an_unreadable_version_is_refused_quoting_what_git_printed() {
    let stub = StubPath::printing_version("banana");
    let error = failure(stub.discover(), "unparseable --version");
    match &error {
        Error::GitVersionUnreadable {
            path,
            output,
            required,
        } => {
            assert_eq!(path, &stub.git_path());
            assert_eq!(output, "banana");
            assert_eq!(*required, GitVersion::MINIMUM);
        }
        other => panic!("expected GitVersionUnreadable, got {other:?}"),
    }
    let message = error.to_string();
    assert!(
        message.contains("banana") && message.contains("2.30.0"),
        "{message}"
    );
}

#[test]
fn a_silent_git_is_refused_the_same_way() {
    let stub = StubPath::with_git("exit 0");
    assert!(
        matches!(
            stub.discover(),
            Err(Error::GitVersionUnreadable { output, .. }) if output.is_empty()
        ),
        "an empty --version must be unreadable, not a panic"
    );
}

/// A git that fails its own `--version` surfaces its stderr, not a bare code.
#[test]
fn stderr_reaches_the_error_when_git_fails() {
    let stub = StubPath::with_git("echo 'libgit.so: cannot open shared object' >&2; exit 127");
    let error = failure(stub.discover(), "failing --version");
    match &error {
        Error::GitFailed {
            arguments,
            status,
            stderr,
        } => {
            assert_eq!(arguments, "--version");
            assert_eq!(status.code(), Some(127));
            assert_eq!(stderr, "libgit.so: cannot open shared object");
        }
        other => panic!("expected GitFailed, got {other:?}"),
    }
    let message = error.to_string();
    assert!(message.contains("cannot open shared object"), "{message}");
    assert!(message.contains("--version"), "{message}");
}

#[test]
fn a_git_that_cannot_be_executed_is_not_found_rather_than_run() {
    let stub = StubPath::printing_version("git version 2.55.0");
    std::fs::set_permissions(stub.git_path(), std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        matches!(stub.discover(), Err(Error::GitNotFound { .. })),
        "a non-executable file named git is not a git"
    );
}

#[test]
fn the_floor_itself_is_accepted_and_reported() {
    let stub = StubPath::printing_version("git version 2.30.0");
    let git = stub.discover().unwrap();
    assert_eq!(git.version(), GitVersion::MINIMUM);
    assert_eq!(git.path(), stub.git_path());
    assert_eq!(git.environment(), &stub.environment());
}

#[test]
fn a_newer_git_is_accepted() {
    let stub = StubPath::printing_version("git version 2.55.0");
    let git = stub.discover().unwrap();
    assert_eq!(git.version().to_string(), "2.55.0");
}

/// The first executable `git` on PATH wins, as a shell would choose it.
#[test]
fn the_first_directory_on_path_wins() {
    let first = StubPath::printing_version("git version 2.31.0");
    let second = StubPath::printing_version("git version 2.55.0");
    let joined = std::env::join_paths([&first.directory, &second.directory]).unwrap();
    let environment = GitEnvironment::new(|name| (name == "PATH").then(|| joined.clone()));
    let git = discover_retrying(environment).unwrap();
    assert_eq!(git.path(), first.git_path());
    assert_eq!(git.version().to_string(), "2.31.0");
}

/// The machine's own git, through the same path the application takes at startup.
#[test]
fn the_installed_git_is_discovered_from_the_process_environment() {
    let git = match GitBinary::discover() {
        Ok(git) => git,
        Err(error) => panic!("this machine's git was refused: {error}"),
    };
    assert!(git.version() >= GitVersion::MINIMUM);
    assert!(git.path().is_file());
}

/// A stub whose interpreter does not exist fails at `exec`, before git could run.
#[test]
fn a_git_that_cannot_be_started_names_the_program_and_the_cause() {
    let stub = StubPath::empty();
    let git = stub.git_path();
    std::fs::write(&git, "#!/nonexistent/interpreter\n").unwrap();
    std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = failure(stub.discover(), "unstartable git");
    match &error {
        Error::GitNotStarted { program, source } => {
            assert_eq!(program, &git);
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
        }
        other => panic!("expected GitNotStarted, got {other:?}"),
    }
    let message = error.to_string();
    assert!(
        message.contains(&git.display().to_string()) && message.contains("could not start"),
        "{message}"
    );
}
