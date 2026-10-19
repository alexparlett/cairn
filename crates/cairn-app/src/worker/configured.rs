//! A worker thread's repository handle, opened again when the configuration moves.
//!
//! gix reads the configuration once, as a handle is opened, and every read it answers after
//! is under that reading — `core.useReplaceRefs` among it, which decides whether a walk follows
//! the replace refs, and the upstreams' `branch.<name>.*` keys. git reads its configuration
//! afresh as each command runs. So a thread whose reads must be the user's own `git`'s keeps
//! every configuration file stamped as its handle was opened ([`cairn_git::DiffInputs`]'s
//! configuration — the system, global and repository files and every include, existing or
//! not) and, before the work that reads it, opens the handle again when one moved, as the
//! diff thread does ([`super::diff::freshness`]). Checking is one `stat` per configuration
//! file.
//!
//! A stamp taken while its file could still change unseen — within [`SETTLING`] of an edit,
//! or for a file whose times are in the future — cannot say that nothing changed since, so
//! such a handle is opened again once [`SETTLING`] has passed since it was last opened, and no
//! sooner: a moved file is followed at once, an unsettled one at most once per [`SETTLING`].
//!
//! A handle that cannot be opened again — a configuration the user's own `git` would refuse
//! to read, a git directory replaced — leaves the old handle in use, its files stamped as
//! they are now, so the next edit tries again rather than every job.

use std::path::PathBuf;
use std::time::SystemTime;

use cairn_git::ops::GitBinary;
use cairn_git::{Repository, SharedRepository};

use super::diff::freshness::{SETTLING, Stamp, all_still, stamps};
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
    /// When the handle was last opened, or an opening tried.
    opened: SystemTime,
}

impl<'a> Configured<'a> {
    /// The thread's first handle, its configuration stamped against when `shared` was opened.
    pub(super) fn open(shared: &'a SharedRepository, reopening: &'a Reopening) -> Self {
        let handle = shared.to_worker();
        let files = handle.diff_inputs().configuration().to_vec();
        let opened = shared.opened_at();
        let stamps = stamps(&files, opened);
        Self {
            shared,
            reopening,
            handle,
            files,
            stamps,
            opened,
        }
    }

    pub(super) fn handle(&self) -> &Repository {
        &self.handle
    }

    /// Whether a configuration file moved since the handle was opened, or could have unseen
    /// and [`SETTLING`] has passed since it was.
    pub(super) fn moved(&self) -> bool {
        self.moved_at(SystemTime::now())
    }

    fn moved_at(&self, now: SystemTime) -> bool {
        let found = stamps(&self.files, now);
        if all_still(&self.stamps, &found) {
            return false;
        }
        let same_files = self.stamps.len() == found.len()
            && self
                .stamps
                .iter()
                .zip(&found)
                .all(|(then, now)| then.same_file(now));
        !same_files
            || now
                .duration_since(self.opened)
                .is_ok_and(|since| since >= SETTLING)
    }

    /// Opens the handle again, its configuration read afresh and stamped against when the
    /// opening began; where it cannot be, the old handle stays, its files stamped as they are.
    pub(super) fn reopen(&mut self) {
        let started = SystemTime::now();
        self.opened = started;
        let Reopening { git, startup } = self.reopening;
        match self.shared.reopen_for(git, |name| startup.parent(name)) {
            Ok(fresh) => {
                self.files = fresh.diff_inputs().configuration().to_vec();
                self.stamps = stamps(&self.files, started);
                self.handle = fresh;
            }
            Err(_) => self.stamps = stamps(&self.files, started),
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

    /// Past the settling time of every file the fixture wrote: stamps taken against it are
    /// trusted.
    fn settled_later() -> SystemTime {
        SystemTime::now() + SETTLING + std::time::Duration::from_secs(1)
    }

    /// A handle follows the configuration: settled and unchanged, it is kept; a key set after it
    /// was opened is seen, and read once the handle is opened again; an unsettled stamp opens it
    /// again only once the settling time has passed since it was opened; and a configuration that
    /// cannot be read keeps the old handle without asking again on every check. Caught by:
    /// `moved` comparing nothing (an edit unseen), opening on every check (a settled, unchanged
    /// handle reported moved), an unsettled stamp reopening on every check, or a failed opening
    /// tried again on every check — which spun the history thread.
    #[test]
    fn a_handle_follows_a_configuration_that_moved_and_keeps_one_that_did_not() {
        let fixture = crate::worker::network::tests::BorrowedRepository::new(&format!(
            "cairn-configured-{}",
            std::process::id()
        ));
        let shared = SharedRepository::discover(&fixture.fixture.path).unwrap();
        let reopening = Reopening::for_tests();
        let mut configured = Configured::open(&shared, &reopening);
        // As if opened once every file had settled.
        configured.stamps = stamps(&configured.files, settled_later());
        configured.opened = settled_later();
        assert!(
            !configured.moved_at(settled_later()),
            "a settled, unchanged configuration was reported moved"
        );

        let context = |repo: &Repository| repo.configured_context().ok();
        let before = context(configured.handle());
        let config = fixture.fixture.path.join(".git/config");
        let mut text = std::fs::read_to_string(&config).unwrap();
        text.push_str("[diff]\n\tcontext = 7\n");
        std::fs::write(&config, text).unwrap();
        assert!(
            configured.moved_at(settled_later()),
            "an edited configuration was not seen"
        );
        configured.reopen();
        let after = context(configured.handle());
        assert_ne!(
            after, before,
            "the handle still reads the old configuration"
        );
        assert_eq!(after, Some(cairn_model::Context::lines(7)));

        // Just reopened over a file edited a moment ago: its stamp is unsettled, which opens
        // the handle again once the settling time has passed, and not before.
        let just = configured.opened;
        assert!(
            !configured.moved_at(just),
            "an unsettled stamp reopened at once"
        );
        assert!(configured.moved_at(just + SETTLING));

        // A configuration git would refuse to read: the handle cannot be opened again, the old
        // one stays, and the files are stamped as they are, so nothing asks again until the
        // next edit.
        let mut text = std::fs::read_to_string(&config).unwrap();
        text.push_str("[unterminated\n");
        std::fs::write(&config, text).unwrap();
        assert!(configured.moved_at(settled_later()));
        let Reopening { git, startup } = &reopening;
        assert!(
            shared.reopen_for(git, |name| startup.parent(name)).is_err(),
            "the fixture's configuration was read after all"
        );
        configured.reopen();
        assert_eq!(
            context(configured.handle()),
            Some(cairn_model::Context::lines(7)),
            "the old handle did not stay"
        );
        assert!(
            !configured.moved_at(configured.opened),
            "a configuration that cannot be opened is tried again on every check"
        );
    }
}
