//! A stub `git` on a `PATH` the test controls, for the runner's own tests and
//! for the operations in `ops/` that run through it.
//!
//! `crates/cairn-git/tests/git_binary.rs` carries the same helper for the
//! discovery tests, which need only the public API; this copy exists because
//! the runner is `pub(crate)` and an integration test cannot reach it.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
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
        std::fs::write(&git, format!("#!/bin/sh\n{script}\n"))
            .unwrap_or_else(|e| panic!("could not write {}: {e}", git.display()));
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|e| panic!("could not chmod {}: {e}", git.display()));
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
