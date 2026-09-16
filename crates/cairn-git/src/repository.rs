use std::path::{Path, PathBuf};

use crate::Error;

/// An open repository that threads can share (D3). The expensive parts —
/// object database, mapped pack indices, parsed config — live here once, so N
/// workers cost one set of pack mmaps. `Send + Sync`, where [`Repository`] is
/// deliberately not `Sync`.
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
    /// Opens the repository containing `path`, walking upwards like `git`.
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

    /// One worker thread's handle. Call it once per worker and keep it for the
    /// thread's life: gitoxide rebuilds the object cache and the pack snapshot
    /// per handle, so a per-request call compiles, passes every test, and loses
    /// the point of the split.
    pub fn to_worker(&self) -> Repository {
        let mut inner = self.inner.to_thread_local();
        inner.object_cache_size_if_unset(Repository::OBJECT_CACHE_BYTES);
        Repository {
            inner,
            workdir: self.workdir.clone(),
        }
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    /// The working tree root, or `None` for a bare repository.
    pub fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }
}

/// One thread's handle on a repository. Not `Sync`: a thread that wants to read
/// takes its own from a [`SharedRepository`].
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
    /// Opens the repository containing `path` for this thread alone; it maps
    /// its own object database, so anything reading from more than one thread
    /// opens a [`SharedRepository`]. Installs the object cache a committer-date
    /// walk needs: over 50k commits, 178 ms without it and 116 ms with, and more
    /// than [`Self::OBJECT_CACHE_BYTES`] bought nothing
    /// (`docs/systems/history-graph.md`, "The worker boundary").
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, Error> {
        Ok(SharedRepository::discover(path)?.to_worker())
    }

    /// Measured: see [`Self::discover`].
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

    /// The object cache is a property of the handle, so `to_worker` is the only
    /// place that can install it. Deleting that line costs 53% on a commit-time
    /// walk and changes no observable answer.
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

    /// Reporter, not a test: walks one repository from 1, 2, 4 and 8 threads,
    /// each with its own handle. Asserts nothing about timing.
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

    /// Also the twin for gix's `parallel` feature staying on: it is what makes
    /// `ThreadSafeRepository` `Send + Sync`, so dropping the feature fails here
    /// rather than silently single-threading the object database.
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
