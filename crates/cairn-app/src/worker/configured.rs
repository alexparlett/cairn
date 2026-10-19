//! A worker thread's repository handle, opened again when the configuration moves.
//!
//! gix reads the configuration once, as a handle is opened, and every read it answers after
//! is under that reading — `core.useReplaceRefs` among it, which decides whether a walk follows
//! the replace refs, and the upstreams' `branch.<name>.*` keys. git reads its configuration
//! afresh as each command runs. So a thread whose reads must be the user's own `git`'s keeps
//! every configuration file stamped as its handle was opened ([`cairn_git::DiffInputs`]'s
//! configuration — the system, global and repository files and every include, existing or
//! not) and, before the work that reads it, opens the handle again when one moved, as the
//! diff thread does ([`super::diff::freshness`]). A stamp taken while its file could still
//! change unseen matches nothing, so for a moment after an edit each check opens the handle
//! again. Checking is one `stat` per configuration file.
//!
//! A handle that cannot be opened again — a configuration the user's own `git` would refuse
//! to read, a git directory replaced — leaves the old handle in use, and the next check tries
//! again.

use std::path::PathBuf;
use std::time::SystemTime;

use cairn_git::ops::GitBinary;
use cairn_git::{Repository, SharedRepository};

use super::diff::freshness::{Stamp, all_still, stamps};
use super::startup::Startup;

/// What opening the repository again needs: the git in use, which decides how it is opened,
/// and how the launching environment is read.
#[derive(Clone)]
pub(super) struct Reopening {
    pub(super) git: GitBinary,
    pub(super) startup: Startup,
}

impl Reopening {
    /// The git on this machine's `PATH`, found as the application finds it, for a test.
    #[cfg(test)]
    pub(super) fn for_tests() -> Self {
        let startup = Startup::new(
            |name| match name {
                "PATH" => std::env::var_os("PATH"),
                _ => None,
            },
            PathBuf::from("/nonexistent/cairn-askpass"),
        );
        match GitBinary::discover_with(startup.probe_environment()) {
            Ok(git) => Self { git, startup },
            Err(error) => panic!("this machine's git was refused: {error}"),
        }
    }
}

/// A thread's handle and the configuration it was opened under.
pub(super) struct Configured<'a> {
    shared: &'a SharedRepository,
    reopening: &'a Reopening,
    handle: Repository,
    /// Every configuration file the handle read, existing or not.
    files: Vec<PathBuf>,
    /// Each of `files` as the handle's opening found it.
    stamps: Vec<Stamp>,
}

impl<'a> Configured<'a> {
    /// The thread's first handle, its configuration stamped against when `shared` was opened.
    pub(super) fn open(shared: &'a SharedRepository, reopening: &'a Reopening) -> Self {
        let handle = shared.to_worker();
        let files = handle.diff_inputs().configuration().to_vec();
        let stamps = stamps(&files, shared.opened_at());
        Self {
            shared,
            reopening,
            handle,
            files,
            stamps,
        }
    }

    pub(super) fn handle(&self) -> &Repository {
        &self.handle
    }

    /// Whether a configuration file moved since the handle was opened — or could have, unseen.
    pub(super) fn moved(&self) -> bool {
        !all_still(&self.stamps, &stamps(&self.files, SystemTime::now()))
    }

    /// Opens the handle again, its configuration read afresh and stamped against when the
    /// opening began; the old handle stays where it cannot be.
    pub(super) fn reopen(&mut self) {
        let started = SystemTime::now();
        let Reopening { git, startup } = self.reopening;
        if let Ok(fresh) = self.shared.reopen_for(git, |name| startup.parent(name)) {
            self.files = fresh.diff_inputs().configuration().to_vec();
            self.stamps = stamps(&self.files, started);
            self.handle = fresh;
        }
    }

    /// The handle, opened again first if the configuration moved.
    pub(super) fn follow(&mut self) -> &Repository {
        if self.moved() {
            self.reopen();
        }
        &self.handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A handle follows the configuration: a key set after it was opened is read once the
    /// handle follows, and a handle whose files are as they were — once settled — is kept.
    /// Caught by: a handle kept for the thread's life, which reads the configuration of its
    /// first open forever.
    #[test]
    fn a_handle_follows_a_configuration_that_moved() {
        let fixture = crate::worker::network::tests::BorrowedRepository::new(&format!(
            "cairn-configured-{}",
            std::process::id()
        ));
        let shared = SharedRepository::discover(&fixture.fixture.path).unwrap();
        let reopening = Reopening::for_tests();
        let mut configured = Configured::open(&shared, &reopening);
        let context = |repo: &Repository| repo.configured_context().ok();
        let before = context(configured.handle());
        let config = fixture.fixture.path.join(".git/config");
        let mut text = std::fs::read_to_string(&config).unwrap();
        text.push_str("[diff]\n\tcontext = 7\n");
        std::fs::write(&config, text).unwrap();
        assert!(configured.moved(), "an edited configuration was not seen");
        let after = context(configured.follow());
        assert_ne!(
            after, before,
            "the handle still reads the old configuration"
        );
        assert_eq!(after, Some(cairn_model::Context::lines(7)));
    }
}
