use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cairn_model::CommandRecord;

use crate::Error;
use crate::ops::{GitBinary, GitVersion};
pub use crate::process::CLOSE_BOUND;
use crate::process::Processes;

/// The rule [`SharedRepository::discover`] applies: git 2.45's, the newest band of
/// `safe.bareRepository`'s rule, whose default is `all`.
const NEWEST_RULE: GitVersion = GitVersion {
    major: 2,
    minor: 45,
    patch: 0,
};

pub struct SharedRepository {
    inner: gix::ThreadSafeRepository,
    git_dir: PathBuf,
    workdir: Option<PathBuf>,
    /// The path it was opened from, which [`SharedRepository::reopen_for`] opens again.
    opened_from: PathBuf,
    /// Taken before anything was read, so a file changed while it was being opened has a
    /// time no earlier than this.
    opened_at: std::time::SystemTime,
    /// The `git` invocations running in it and the log of those that are
    /// over, shared with every worker handle made from it.
    processes: Arc<Processes>,
}

impl std::fmt::Debug for SharedRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedRepository")
            .field("git_dir", &self.git_dir)
            .field("workdir", &self.workdir)
            .finish()
    }
}

impl SharedRepository {
    /// Opens the repository containing `path`, walking upwards like `git`, as the newest
    /// `git` Cairn knows decides it in the environment Cairn was launched with: a bare
    /// repository found by searching is refused under `safe.bareRepository = explicit`
    /// ([`Error::BareRepositoryFoundBySearching`]). Where the `git` that will be asked is
    /// known, [`SharedRepository::discover_for`] decides it as that version does.
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::discover_as(path.as_ref(), NEWEST_RULE, &|name| std::env::var_os(name))
    }

    /// Opens the repository containing `path` as `git` would find it from there:
    /// walking upwards, and refusing a bare repository found by searching where that
    /// version of git, reading the configuration `environment` leads to, refuses it
    /// (`crate::bare_discovery`). `environment` answers what the launching environment
    /// holds for a name — what the user's own `git`, run from the same place, reads.
    /// Every `git` run in a fully trusted repository afterwards is given its git
    /// directory explicitly, so this is the one place git's own checks are made: the
    /// bare-repository one, and ownership — full trust only when the user owns every path
    /// git checks, or `safe.directory` names the repository (`crate::ownership`).
    pub fn discover_for(
        path: impl AsRef<Path>,
        git: &GitBinary,
        environment: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, Error> {
        Self::discover_as(path.as_ref(), git.version(), &environment)
    }

    fn discover_as(
        path: &Path,
        version: GitVersion,
        environment: &dyn Fn(&str) -> Option<OsString>,
    ) -> Result<Self, Error> {
        use gix::sec::trust::DefaultForLevel as _;

        let opened_at = std::time::SystemTime::now();
        // The path the check searched to, opened as it is: searching again could stop
        // somewhere else (`crate::bare_discovery::find`).
        let stop = crate::bare_discovery::find(path, version, environment)?;
        let not_a_repository = || Error::NotARepository {
            path: path.to_owned(),
        };
        // The trust git's own discovery reaches there: full only when the user owns every
        // path git checks — the `.git` file, the working tree's top and the git directory
        // (`crate::ownership`). It picks the options and is the git directory's trust, which
        // gix raises to full as it opens when `safe.directory` names the repository, as git
        // does; and only a fully trusted repository is named to the `git` Cairn runs, so
        // anything less is left to git's own check (`process/cli.rs`).
        let trust = crate::ownership::owners(&stop, &crate::ownership::is_owned).trust();
        let found = stop.into_path();
        let options = gix::open::Options::default_for_level(trust)
            .with(trust)
            .open_path_as_is(true);
        let inner =
            gix::ThreadSafeRepository::open_opts(found, options).map_err(
                |source| match source {
                    gix::open::Error::NotARepository { .. } => not_a_repository(),
                    other => Error::Open {
                        path: path.to_owned(),
                        source: Box::new(other),
                    },
                },
            )?;
        Ok(Self {
            git_dir: inner.git_dir().to_owned(),
            workdir: inner.work_dir().map(Path::to_owned),
            inner,
            opened_from: path.to_owned(),
            opened_at,
            processes: Arc::default(),
        })
    }

    /// A worker handle on this repository opened afresh — by the route
    /// [`SharedRepository::discover_for`] took, from the same path, as the `git` in use
    /// decides it — so that what gix read at open, the configuration above all, is read
    /// again. It shares this repository's registry and log, so closing the repository ends
    /// what runs in it and the log holds it. The git directory found must be this one's:
    /// anything else is [`Error::RepositoryReplaced`], and the old handle stays the one to
    /// use. It reads, so it is a worker's call; [`SharedRepository::opened_at`]'s time for
    /// it is the caller's to take before calling.
    pub fn reopen_for(
        &self,
        git: &GitBinary,
        environment: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Repository, Error> {
        let fresh = Self::discover_as(&self.opened_from, git.version(), &environment)?;
        if fresh.git_dir != self.git_dir {
            return Err(Error::RepositoryReplaced {
                path: self.opened_from.clone(),
                was: self.git_dir.clone(),
                now: fresh.git_dir,
            });
        }
        let mut inner = fresh.inner.to_thread_local();
        inner.object_cache_size_if_unset(Repository::OBJECT_CACHE_BYTES);
        Ok(Repository {
            inner,
            workdir: fresh.workdir,
            processes: Arc::clone(&self.processes),
        })
    }

    /// When opening began: no file this handle read can have changed before it unseen by a
    /// stamp taken after, unless its time is earlier than this.
    pub fn opened_at(&self) -> std::time::SystemTime {
        self.opened_at
    }

    /// Call once per worker thread and keep it: each call rebuilds the object cache and pack snapshot.
    pub fn to_worker(&self) -> Repository {
        let mut inner = self.inner.to_thread_local();
        inner.object_cache_size_if_unset(Repository::OBJECT_CACHE_BYTES);
        Repository {
            inner,
            workdir: self.workdir.clone(),
            processes: Arc::clone(&self.processes),
        }
    }

    /// Ends every `git` invocation running in this repository the way a
    /// cancel does — `SIGTERM` to each one's process group, `SIGKILL` after
    /// the grace — and waits up to `bound` for them all to be reaped; what
    /// closing the repository runs, with [`crate::CLOSE_BOUND`]. Returns how
    /// many were still running when it stopped waiting: zero unless one
    /// outlived the bound. Any invocation started in this repository
    /// afterwards is ended as soon as it starts, since the repository is
    /// closing.
    ///
    /// A write that outlasts the grace is `SIGKILL`ed, which can strand its
    /// lock files; its cancellation lists them to whoever drives it, but on a
    /// close nobody may be left to show them.
    ///
    /// It waits, so it is a worker's call, never the UI thread's.
    pub fn end_invocations(&self, bound: Duration) -> usize {
        self.processes.end_all(bound)
    }

    /// Every `git` invocation this repository has run that is over, oldest
    /// first, as far back as the log keeps: one record each, however it
    /// ended.
    pub fn command_log(&self) -> Vec<CommandRecord> {
        self.processes.log()
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// The working tree root, or `None` for a bare repository.
    pub fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }
}

pub struct Repository {
    inner: gix::Repository,
    workdir: Option<PathBuf>,
    processes: Arc<Processes>,
}

impl std::fmt::Debug for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repository")
            .field("git_dir", &self.inner.git_dir())
            .field("workdir", &self.workdir)
            .finish()
    }
}

impl Repository {
    /// Opens the repository containing `path` for this thread alone.
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(SharedRepository::discover(path)?.to_worker())
    }

    pub const OBJECT_CACHE_BYTES: usize = 4 * 1024 * 1024;

    pub fn git_dir(&self) -> &Path {
        self.inner.git_dir()
    }

    /// The working tree root, or `None` for a bare repository.
    pub fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }

    pub(crate) fn inner(&self) -> &gix::Repository {
        &self.inner
    }

    /// The registry and log every handle on this repository shares.
    pub(crate) fn processes(&self) -> &Arc<Processes> {
        &self.processes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_this_repository_from_a_nested_path() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        // Not `ends_with(".git")`: a linked worktree is `.git/worktrees/<n>`.
        assert!(
            repo.git_dir().join("HEAD").is_file(),
            "{} is not a git directory",
            repo.git_dir().display()
        );
        let workdir = repo.workdir().unwrap();
        assert!(workdir.join("crates/cairn-git/Cargo.toml").is_file());
    }

    #[test]
    fn reports_a_non_repository_as_such() {
        let err = Repository::discover("/").unwrap_err();
        assert!(matches!(err, Error::NotARepository { .. }), "got {err:?}");
    }

    #[test]
    fn a_shared_repository_reports_the_same_paths_as_a_worker_handle() {
        let shared = SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let worker = shared.to_worker();
        assert_eq!(shared.git_dir(), worker.git_dir());
        assert_eq!(shared.workdir(), worker.workdir());
        assert!(shared.git_dir().join("HEAD").is_file());
    }

    /// Caught by: removing the cache installation from `to_worker`, which changes no answer.
    #[test]
    fn every_worker_handle_carries_the_object_cache() {
        let shared = SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        for _ in 0..2 {
            let worker = shared.to_worker();
            assert!(
                worker.inner().objects.has_object_cache(),
                "a worker handle was built without the object cache O2 measured"
            );
        }
    }

    #[test]
    fn a_shared_repository_reports_a_non_repository_as_such() {
        let err = SharedRepository::discover("/").unwrap_err();
        assert!(matches!(err, Error::NotARepository { .. }), "got {err:?}");
    }

    /// Reporter. Env: `CAIRN_BENCH_REPO`, `CAIRN_BENCH_LIMIT`; run with `--release`.
    #[test]
    #[ignore = "needs a large repository named by CAIRN_BENCH_REPO"]
    fn measures_concurrent_walks_against_a_named_repository() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        use crate::{CancelSignal, HistoryRequest};

        let path = std::env::var("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO");
        let limit: usize = std::env::var("CAIRN_BENCH_LIMIT")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(50_000);
        let shared = Arc::new(SharedRepository::discover(&path).unwrap());
        let commit_graph = shared.git_dir().join("objects/info/commit-graph").exists()
            || shared.git_dir().join("objects/info/commit-graphs").exists();
        eprintln!(
            "repository {path}, {limit} commits per walk, commit-graph file present: \
             {commit_graph}"
        );

        for threads in [1usize, 2, 4, 8] {
            let started = Instant::now();
            let rows = Arc::new(AtomicUsize::new(0));
            let mut running = Vec::new();
            for _ in 0..threads {
                let shared = Arc::clone(&shared);
                let rows = Arc::clone(&rows);
                running.push(std::thread::spawn(move || {
                    // Once per worker, as the shipping worker does.
                    let repo = shared.to_worker();
                    let mut session = repo
                        .history_session(&HistoryRequest::from_head(limit))
                        .unwrap();
                    let began = Instant::now();
                    let mut walked = 0usize;
                    while walked < limit {
                        let page = session.next_page(1_000, &CancelSignal::new()).unwrap();
                        if page.rows.is_empty() {
                            break;
                        }
                        walked += page.rows.len();
                    }
                    rows.fetch_add(walked, Ordering::Relaxed);
                    began.elapsed()
                }));
            }
            let each: Vec<_> = running.into_iter().map(|t| t.join().unwrap()).collect();
            let wall = started.elapsed();
            let total = rows.load(Ordering::Relaxed);
            let slowest = each.iter().max().copied().unwrap_or_default();
            eprintln!(
                "  {threads} thread(s)\twall {wall:?}\tslowest walk {slowest:?}\t\
                 {total} rows\t{:.0} rows/s together",
                total as f64 / wall.as_secs_f64()
            );
        }
    }

    /// Also fails if gix's `parallel` feature is dropped, which removes `Send + Sync`.
    #[test]
    fn a_shared_repository_can_cross_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SharedRepository>();

        let shared = SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let expected = shared.git_dir().to_owned();
        let found = std::thread::spawn(move || shared.to_worker().git_dir().to_owned())
            .join()
            .unwrap();
        assert_eq!(found, expected);
    }
}
