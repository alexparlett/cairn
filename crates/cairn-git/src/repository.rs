use std::path::{Path, PathBuf};

use crate::Error;

/// An open repository that threads can share.
///
/// This is gitoxide's threading model made explicit (design decision D3). The
/// expensive parts of an open repository — the object database, the pack
/// indices it has mapped, the parsed configuration — live here once and are
/// shared; the per-thread parts (an object cache, an inflate buffer, the
/// snapshot of which packs are visible) come from [`Self::to_worker`], once per
/// worker thread. N workers therefore cost one set of pack mmaps, not N.
///
/// `SharedRepository` is `Send + Sync`; [`Repository`] is deliberately not
/// `Sync`, so the compiler stops a handle being used from two threads at once.
pub struct SharedRepository {
    inner: gix::ThreadSafeRepository,
    git_dir: PathBuf,
    workdir: Option<PathBuf>,
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
    /// Open the repository containing `path`, walking upwards like `git` does.
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let inner = gix::ThreadSafeRepository::discover(path).map_err(|source| match source {
            gix::discover::Error::Discover(_) => Error::NotARepository {
                path: path.to_owned(),
            },
            other => Error::Open {
                path: path.to_owned(),
                source: Box::new(other),
            },
        })?;
        Ok(Self {
            git_dir: inner.git_dir().to_owned(),
            workdir: inner.work_dir().map(Path::to_owned),
            inner,
        })
    }

    /// One worker thread's handle on this repository.
    ///
    /// **Call this once per worker, at the thread's start, and keep the result
    /// for the thread's whole life.** Calling it per request compiles, passes
    /// every test, and throws away the object cache and the pack snapshot that
    /// are the entire reason for the split: gitoxide builds both fresh for each
    /// handle (`gix::ThreadSafeRepository::to_thread_local` clones the store but
    /// makes a new `gix_odb` handle, and the object cache is a property of the
    /// handle, not of the store). The cache [`Repository::OBJECT_CACHE_BYTES`]
    /// describes is installed here for exactly that reason.
    pub fn to_worker(&self) -> Repository {
        let mut inner = self.inner.to_thread_local();
        inner.object_cache_size_if_unset(Repository::OBJECT_CACHE_BYTES);
        Repository {
            inner,
            workdir: self.workdir.clone(),
        }
    }

    /// The `.git` directory backing this repository.
    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// The working tree root, or `None` for a bare repository.
    pub fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }
}

/// One thread's handle on a repository.
///
/// Holds the gitoxide handle privately. Borrow it inside this crate with
/// [`Repository::inner`]; it is not part of the public surface.
///
/// Not `Sync`, and not to be shared: a thread that wants to read a repository
/// takes its own handle from a [`SharedRepository`].
pub struct Repository {
    inner: gix::Repository,
    workdir: Option<PathBuf>,
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
    /// Open the repository containing `path` for use on this thread alone.
    ///
    /// The single-threaded route, kept for tests and for callers that will
    /// never hand the repository to a worker. Anything that reads from more
    /// than one thread opens a [`SharedRepository`] instead, because this
    /// constructor maps its own object database.
    ///
    /// Opening installs a small object cache. Walking by committer date looks
    /// each commit up twice without one: measured over 50k commits of a
    /// repository with no commit-graph file, the walk alone took 178 ms with no
    /// cache and 116 ms with one, and a cache larger than
    /// [`Self::OBJECT_CACHE_BYTES`] bought nothing further
    /// (`docs/work/history-graph/progress.md`, open question O2).
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(SharedRepository::discover(path)?.to_worker())
    }

    /// How much memory one open repository spends on caching decoded objects.
    /// Measured, not guessed: see [`Self::discover`].
    pub const OBJECT_CACHE_BYTES: usize = 4 * 1024 * 1024;

    /// The `.git` directory backing this repository.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_this_repository_from_a_nested_path() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        // Not `ends_with(".git")`: a linked worktree — which is how this
        // repository asks parallel work to be checked out — is backed by
        // `.git/worktrees/<name>`, so the thing to assert is that the path is
        // a git directory, not what it happens to be called.
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

    #[test]
    fn a_shared_repository_reports_a_non_repository_as_such() {
        let err = SharedRepository::discover("/").unwrap_err();
        assert!(matches!(err, Error::NotARepository { .. }), "got {err:?}");
    }

    /// **Open question O3: how many workers should one repository have?**
    ///
    /// Walks the same repository from 1, 2, 4 and 8 threads, each taking its own
    /// handle from one [`SharedRepository`], and reports what each walk cost and
    /// what the group achieved together. Ignored by default: it needs a
    /// repository worth measuring, and a timing assertion in the gate would be
    /// flaky within a week.
    ///
    /// `CAIRN_BENCH_REPO=<path> CAIRN_BENCH_LIMIT=<n> cargo test -p cairn-git
    /// --release --lib -- --ignored --nocapture measures_concurrent_walks`
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

    /// The whole worker-pool design rests on this: a `SharedRepository` crosses
    /// to a worker thread, a `Repository` never does. gitoxide only makes
    /// `ThreadSafeRepository` `Send + Sync` when its `parallel` feature is on,
    /// so this is also the twin for that feature staying enabled — dropping it
    /// makes this a compile error rather than a silent single-threading of the
    /// object database.
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
