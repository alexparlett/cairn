//! A stub `git` on a `PATH` the test controls, for the runner's own tests and
//! for the operations in `ops/` that run through it.
//!
//! `crates/cairn-git/tests/git_binary.rs` carries the same helper for the
//! discovery tests, which need only the public API; this copy exists because
//! the runner is `pub(crate)` and an integration test cannot reach it.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{Askpass, GitBinary, GitEnvironment};
use crate::Error;

/// A directory holding one stub `git`, removed when the test ends.
pub(crate) struct StubGit {
    directory: PathBuf,
}

impl StubGit {
    /// Where the environment points git for a secret; nothing here runs it.
    pub(crate) const HELPER: &str = "/nonexistent/cairn-askpass";

    /// A `git` whose whole behaviour is `script`, run by `/bin/sh`.
    pub(crate) fn with_git(script: &str) -> Self {
        Self::with_git_from(|_| script.to_owned())
    }

    /// A `git` whose script is written knowing the directory it lives in, for
    /// a stub that leaves a file beside itself.
    pub(crate) fn with_git_from(script: impl FnOnce(&Path) -> String) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "cairn-git-runner-stub-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory)
            .unwrap_or_else(|e| panic!("could not make {}: {e}", directory.display()));
        let git = directory.join("git");
        let script = script(&directory);
        write_executable(&git, &format!("#!/bin/sh\n{script}\n"), 0o755);
        Self { directory }
    }

    /// An environment whose `PATH` is this directory alone; `parent` supplies the rest.
    pub(crate) fn environment_with(
        &self,
        parent: impl Fn(&str) -> Option<OsString>,
    ) -> GitEnvironment {
        let directory = self.directory.clone();
        GitEnvironment::new(
            move |name| {
                if name == "PATH" {
                    Some(directory.clone().into_os_string())
                } else {
                    parent(name)
                }
            },
            &Askpass::new(Self::HELPER, None),
        )
    }

    pub(crate) fn environment(&self) -> GitEnvironment {
        self.environment_with(|_| None)
    }

    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }
}

pub(crate) use script_writer::write_executable;

/// What `/bin/sh` itself adds to a child's environment; not ours and not git's.
const SHELL_OWN: &[&str] = &["PWD", "OLDPWD", "SHLVL", "_"];

/// The environment a stub that ran `/usr/bin/env` reported, as a map; the
/// variables `/bin/sh` adds for itself are dropped, being neither Cairn's nor
/// git's, and nothing else is.
pub(crate) fn printed_environment(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(name, _)| !SHELL_OWN.contains(name))
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

impl Drop for StubGit {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// [`GitBinary::discover_with`], retried while the stub is "text file busy".
///
/// Tests run in parallel, and a fork in another thread inherits this thread's
/// write descriptor to a stub it is still creating for the microseconds until
/// that child execs; executing the stub in that window fails with ETXTBSY. A
/// property of the test harness, not of the backend, so it is absorbed here.
pub(crate) fn discover_retrying(environment: GitEnvironment) -> Result<GitBinary, Error> {
    const ETXTBSY: i32 = 26;
    let mut attempts = 0;
    loop {
        match GitBinary::discover_with(environment.clone()) {
            Err(Error::GitNotStarted { source, .. })
                if source.raw_os_error() == Some(ETXTBSY) && attempts < 50 =>
            {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            other => return other,
        }
    }
}

/// The script writer, in a test module of its own: this file is test-only (its declaration is
/// `#[cfg(all(test, unix))]`), but the process guards read a file's inline test modules as test
/// code, and only those, so the one child it starts stays visibly outside the runner.
#[cfg(test)]
mod script_writer {
    use std::path::Path;

    /// Writes `contents` to `path` with `mode` — a stub `git`, a hook — through a child `sh`, so this
    /// multi-threaded test process never holds a writable descriptor to a file a test then has git
    /// execute: one held while a parallel test forks would make that exec fail "Text file busy" (os
    /// error 26). The same writer as `crates/cairn-git/tests/support/write_executable.rs`, for the
    /// crate's unit tests.
    pub(crate) fn write_executable(path: &Path, contents: &str, mode: u32) {
        use std::io::Write as _;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("making {}: {e}", parent.display()));
        }
        let mut writer = std::process::Command::new("/bin/sh")
            .args(["-c", "cat > \"$0\" && chmod \"$1\" \"$0\""])
            .arg(path)
            .arg(format!("{mode:o}"))
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("starting the writer of {}: {e}", path.display()));
        {
            let Some(mut stdin) = writer.stdin.take() else {
                panic!("the writer of {} has no stdin", path.display());
            };
            stdin
                .write_all(contents.as_bytes())
                .unwrap_or_else(|e| panic!("handing {} to its writer: {e}", path.display()));
        }
        let status = writer
            .wait()
            .unwrap_or_else(|e| panic!("waiting on the writer of {}: {e}", path.display()));
        assert!(
            status.success(),
            "writing {} failed: {status}",
            path.display()
        );
    }
}
