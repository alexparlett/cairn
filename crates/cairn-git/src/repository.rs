use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use cairn_model::CommandRecord;

use crate::Error;
use crate::ops::{GitBinary, GitVersion};
use crate::ownership::{Asked, Identity};
pub use crate::process::CLOSE_BOUND;
use crate::process::Processes;

/// The rule [`SharedRepository::discover`] applies: git 2.45's, the newest band of
/// `safe.bareRepository`'s rule, whose default is `all`.
const NEWEST_RULE: GitVersion = GitVersion {
    major: 2,
    minor: 45,
    patch: 0,
};

/// gix's `gitoxide.objects.allocLimitIfReducedTrust` at zero, which turns off the
/// allocation limit gix gives a repository it trusts less than fully: git reads an object
/// of any size in a repository it opens.
const NO_REDUCED_TRUST_ALLOCATION_LIMIT: &str = "gitoxide.objects.allocLimitIfReducedTrust=0";

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
    /// ([`Error::BareRepositoryFoundBySearching`]), and one git refuses for dubious
    /// ownership is refused ([`Error::DubiousOwnership`]) — with no `git` known, a
    /// `safe.directory` entry spelled `%(prefix)/` with a relative rest names nothing. Where
    /// the `git` that will be asked is known, [`SharedRepository::discover_for`] decides it
    /// as that version does.
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::discover_as(
            path.as_ref(),
            Asked {
                version: NEWEST_RULE,
                executable: None,
            },
            &|name| std::env::var_os(name),
            &Identity::of_this_process(),
        )
    }

    /// Opens the repository containing `path` as `git` would find it from there:
    /// walking upwards, and refusing it where that version of git, reading the
    /// configuration `environment` leads to, refuses it — a bare repository found by
    /// searching under `safe.bareRepository` (`crate::bare_discovery`), and a repository
    /// whose paths are not all the user's and that `safe.directory` does not name
    /// ([`Error::DubiousOwnership`], `crate::ownership`). `environment` answers what the
    /// launching environment holds for a name — what the user's own `git`, run from the
    /// same place, reads. Every `git` run in the repository afterwards is given its git
    /// directory explicitly, which skips both checks, so this is the one place they are
    /// made.
    pub fn discover_for(
        path: impl AsRef<Path>,
        git: &GitBinary,
        environment: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, Error> {
        Self::discover_as(
            path.as_ref(),
            Asked {
                version: git.version(),
                executable: Some(git.path()),
            },
            &environment,
            &Identity::of_this_process(),
        )
    }

    /// The one route every open takes, with who the user is given: the process's own
    /// identity in shipping code, another's in a test.
    fn discover_as(
        path: &Path,
        asked: Asked<'_>,
        environment: &dyn Fn(&str) -> Option<OsString>,
        identity: &Identity<'_>,
    ) -> Result<Self, Error> {
        use gix::sec::trust::DefaultForLevel as _;

        let opened_at = std::time::SystemTime::now();
        // The path the check searched to, opened as it is: searching again could stop
        // somewhere else (`crate::bare_discovery::find`).
        let stop = crate::bare_discovery::find(path, asked.version, environment)?;
        let not_a_repository = || Error::NotARepository {
            path: path.to_owned(),
        };
        let start = std::fs::canonicalize(path).map_err(|_| not_a_repository())?;
        // git's ownership check, made as the git in use makes it, refusing what it refuses
        // (`crate::ownership`); what passes is opened with full trust, as git opens it.
        let judged = crate::ownership::decide(&stop, &start, asked, environment, identity)?;
        let trust = gix::sec::Trust::Full;
        // `with(Full)` sets the git directory's trust, but gix still checks the WORKING
        // TREE's owner again as it opens, by its own rule (the directory `core.worktree`
        // names, `safe.directory` from the system and global files only, compared as
        // written), and where that refuses it lowers the repository to reduced trust; no
        // option skips that check (gix 0.87.1, `src/open/repository.rs`,
        // `open_from_paths`). Git's check has already passed, so what reduced trust would
        // change is undone: the repository's configuration was loaded at full trust and
        // stays so, the `git` Cairn runs is named the repository whatever the trust
        // (`process/cli.rs`), and the one other effect — a 16 MiB ceiling on any object
        // gix reads, which git does not have — is switched off here. That holds for THIS
        // open only: a second open of the repository left to gix's own rule loads the
        // repository's configuration at reduced trust where its git directory is another
        // user's, and gix's lookups then filter it out, so no other open is left to that
        // rule: fetch's refspec check, which once opened the repository a second time to
        // read its remote, now asks git (`reads::fetch_settings`).
        let options = gix::open::Options::default_for_level(trust)
            .with(trust)
            .config_overrides([NO_REDUCED_TRUST_ALLOCATION_LIMIT])
            .open_path_as_is(true);
        let inner =
            gix::ThreadSafeRepository::open_opts(stop.into_path(), options).map_err(|source| {
                match source {
                    gix::open::Error::NotARepository { .. } => not_a_repository(),
                    other => Error::Open {
                        path: path.to_owned(),
                        source: Box::new(other),
                    },
                }
            })?;
        // The git directory opened must be the one judged: a `.git` file is read once for
        // the check and again by gix, and one rewritten between the two reads names a
        // repository nobody checked.
        let opened = inner.git_dir().to_owned();
        let same = |a: &Path, b: &Path| matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(a), Ok(b)) if a == b);
        if !same(&judged, &opened) {
            return Err(Error::RepositoryReplaced {
                path: path.to_owned(),
                was: judged,
                now: opened,
            });
        }
        Ok(Self {
            git_dir: opened,
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
        let fresh = Self::discover_as(
            &self.opened_from,
            Asked {
                version: git.version(),
                executable: Some(git.path()),
            },
            &environment,
            &Identity::of_this_process(),
        )?;
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

    /// The launch environment of these tests: no system file, no global one.
    fn isolated(name: &str) -> Option<OsString> {
        match name {
            "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
            "GIT_CONFIG_GLOBAL" => Some(OsString::from("/dev/null")),
            _ => None,
        }
    }

    fn lstat_owner(path: &Path) -> Option<u32> {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::symlink_metadata(path).ok().map(|meta| meta.uid())
    }

    fn newest() -> Asked<'static> {
        Asked {
            version: NEWEST_RULE,
            executable: None,
        }
    }

    /// The ownership check is wired into every open, and decides it: this checkout opened
    /// by a user whose effective uid is not its owner's is refused with git's refusal,
    /// naming its top — unless `safe.directory`, here on the command line, names it, or the
    /// git asked is one from before git checked ownership — and opens for its owner.
    /// Caught by: the open given full trust whatever the check says (the gap the old
    /// reduced-trust open left: shown, not refused), the identity ignored for the
    /// process's own, or every path taken as owned.
    #[test]
    fn a_repository_someone_else_owns_is_refused_at_open_unless_safe_directory_names_it() {
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR"));
        let top = std::fs::canonicalize(
            SharedRepository::discover(checkout)
                .unwrap()
                .workdir()
                .unwrap(),
        )
        .unwrap();
        let owner = lstat_owner(&top).unwrap();
        let stranger = Identity {
            euid: Some(owner.wrapping_add(1)),
            owner_of: &lstat_owner,
        };
        match SharedRepository::discover_as(checkout, newest(), &isolated, &stranger) {
            Err(Error::DubiousOwnership { path }) => assert_eq!(path, top),
            other => panic!("opened, or refused otherwise, for a stranger: {other:?}"),
        }
        let named = |name: &str| match name {
            "GIT_CONFIG_PARAMETERS" => Some(OsString::from("'safe.directory'='*'")),
            other => isolated(other),
        };
        assert!(SharedRepository::discover_as(checkout, newest(), &named, &stranger).is_ok());
        let unchecked = Asked {
            version: GitVersion {
                major: 2,
                minor: 30,
                patch: 2,
            },
            executable: None,
        };
        assert!(SharedRepository::discover_as(checkout, unchecked, &isolated, &stranger).is_ok());
        let owner_identity = Identity {
            euid: Some(owner),
            owner_of: &lstat_owner,
        };
        assert!(
            SharedRepository::discover_as(checkout, newest(), &isolated, &owner_identity).is_ok()
        );
    }

    /// The git directory opened is the one judged: a `.git` file rewritten after the check
    /// read it and before gix reads it again names a repository nobody checked, and is
    /// refused as [`Error::RepositoryReplaced`]; left alone, it opens. The rewrite is made
    /// by the identity's owner lookup as it is asked about the git directory, the last
    /// path the check reads. Caught by: the check's git directory not compared with the
    /// one opened.
    #[test]
    fn a_gitfile_rewritten_between_the_check_and_the_open_is_refused() {
        let scratch =
            std::env::temp_dir().join(format!("cairn-judged-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_dir_all(&scratch);
        let git_dir = |name: &str| {
            let path = scratch.join(name);
            std::fs::create_dir_all(path.join("objects")).unwrap();
            std::fs::create_dir_all(path.join("refs/heads")).unwrap();
            std::fs::write(path.join("HEAD"), b"ref: refs/heads/main\n").unwrap();
            std::fs::write(
                path.join("config"),
                b"[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
            )
            .unwrap();
            std::fs::canonicalize(path).unwrap()
        };
        let checked = git_dir("checked.git");
        let other = git_dir("other.git");
        let work = scratch.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let gitfile = work.join(".git");
        let point = |at: &Path| {
            std::fs::write(&gitfile, format!("gitdir: {}\n", at.display())).unwrap();
        };
        let euid = lstat_owner(&work);

        point(&checked);
        let steady = Identity {
            euid,
            owner_of: &lstat_owner,
        };
        let opened = SharedRepository::discover_as(&work, newest(), &isolated, &steady).unwrap();
        assert_eq!(std::fs::canonicalize(opened.git_dir()).unwrap(), checked);

        let rewrite = |path: &Path| {
            if path == checked {
                point(&other);
            }
            lstat_owner(path)
        };
        let racing = Identity {
            euid,
            owner_of: &rewrite,
        };
        match SharedRepository::discover_as(&work, newest(), &isolated, &racing) {
            Err(Error::RepositoryReplaced { was, now, .. }) => {
                assert_eq!(was, checked);
                assert_eq!(std::fs::canonicalize(now).unwrap(), other);
            }
            other => panic!("the rewritten .git file was not refused: {other:?}"),
        }
        std::fs::remove_dir_all(&scratch).unwrap();
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
