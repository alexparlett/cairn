//! The worker's half of the process manager, through the real boundary with
//! a stub `git` on a `PATH` the test controls: `git` found once per
//! application (G16), a second fetch refused with a reason (G15), the command
//! log answered as values (G17's worker half), and closing a repository
//! ending and reaping every `git` in it, its grandchildren included, with
//! nothing left behind (G14).
//!
//! No `Command` here — the guards scan this crate's tests — so the stub is a
//! `/bin/sh` script written with `std::fs`, and what is left running is read
//! from `/proc`.

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cairn_git::CLOSE_BOUND;
use cairn_git::ops::GitBinary;
use cairn_model::CommandExit;

use super::discovery::Discovery;
use super::fetch_tests::{
    Home, RuntimeDir, UnbornRepository, WAIT, built_helper, collect_until, next_by, with_origin,
};
use super::pool::{RepositoryHandle, Updates, open};
use super::request::{Request, Update};
use super::startup::Startup;

/// The arguments fetch gives git, after the program; pinned in `cairn-git`.
const FETCH: [&str; 5] = [
    "fetch",
    "--progress",
    "--no-prune-tags",
    "--end-of-options",
    "origin",
];

/// A directory holding one stub `git`, removed when the test ends. `--version`
/// is answered, and counted in `probes`; `fetch` does what the test says,
/// with `$DIR` naming this directory. An invocation in a repository names it
/// ahead of the verb (`--git-dir=`, `--work-tree=`), which the stub skips.
pub(super) struct StubGit {
    pub(super) directory: PathBuf,
}

impl StubGit {
    fn new(fetch: &str) -> Self {
        Self::reporting("2.45.0", fetch)
    }

    /// A stub whose `--version` reports `version`.
    fn reporting(version: &str, fetch: &str) -> Self {
        Self::answering(version, "fetch", fetch)
    }

    /// A stub whose `--version` reports `version` and whose `verb` does what
    /// `body` says; every other verb fails.
    pub(super) fn answering(version: &str, verb: &str, body: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "cairn-app-stub-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&directory);
        if let Err(error) = std::fs::create_dir_all(&directory) {
            panic!("could not make {}: {error}", directory.display());
        }
        let git = directory.join("git");
        let script = format!(
            "#!/bin/sh\nDIR='{}'\n\
             while case \"$1\" in --git-dir=*|--work-tree=*) true ;; *) false ;; esac; do shift; done\n\
             case \"$1\" in\n--version)\n  echo probed >> \"$DIR/probes\"\n  \
             echo 'git version {version}'\n  ;;\n{verb})\n{body}\n  ;;\n*)\n  exit 1\n  ;;\nesac\n",
            directory.display()
        );
        if let Err(error) = std::fs::write(&git, script) {
            panic!("could not write {}: {error}", git.display());
        }
        if let Err(error) = std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)) {
            panic!("could not chmod {}: {error}", git.display());
        }
        let stub = Self { directory };
        stub.until_runnable();
        stub
    }

    /// Runs the stub until it runs. A fork in a parallel test can hold this
    /// thread's write descriptor to the script for the microseconds until that
    /// child execs, and executing it then fails "text file busy"; once one
    /// exec has succeeded nothing can hold it again. The probes this makes are
    /// counted from, never into, what a test measures.
    fn until_runnable(&self) {
        const ETXTBSY: i32 = 26;
        for _ in 0..500 {
            match GitBinary::discover_with(self.startup(None).probe_environment()) {
                // It ran: found, or refused for the version it reported.
                Ok(_) | Err(cairn_git::Error::GitTooOld { .. }) => return,
                Err(cairn_git::Error::GitNotStarted { source, .. })
                    if source.raw_os_error() == Some(ETXTBSY) =>
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("the stub git does not run: {error}"),
            }
        }
        panic!("the stub git stayed text-file-busy");
    }

    /// A launch whose `PATH` is this directory, with a `HOME` and, when given,
    /// a runtime directory for the askpass channel.
    pub(super) fn startup(&self, around: Option<(&Home, &RuntimeDir)>) -> Startup {
        let path = self.directory.clone().into_os_string();
        let home = around.map(|(home, _)| home.path.clone().into_os_string());
        let runtime = around.map(|(_, runtime)| runtime.path.clone().into_os_string());
        Startup::new(
            move |name| -> Option<OsString> {
                match name {
                    "PATH" => Some(path.clone()),
                    "HOME" => home.clone(),
                    "XDG_RUNTIME_DIR" => runtime.clone(),
                    _ => None,
                }
            },
            built_helper(),
        )
    }

    /// How many times `git --version` has run.
    fn probes(&self) -> usize {
        std::fs::read_to_string(self.directory.join("probes"))
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }

    /// A pid the fetch wrote to `name`, waiting for it to be written.
    fn pid(&self, name: &str) -> i32 {
        let file = self.directory.join(name);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(pid) = std::fs::read_to_string(&file)
                .ok()
                .and_then(|text| text.trim().parse().ok())
            {
                return pid;
            }
            assert!(
                Instant::now() < deadline,
                "the stub never wrote {}",
                file.display()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for StubGit {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// A fetch that leads its group, starts a grandchild holding stdout and
/// stderr, and waits on it for ten minutes: what only a group kill ends.
/// `/bin/sleep` by its path, since the stub's `PATH` is its own directory.
const HANGS_WITH_A_GRANDCHILD: &str = "  echo $$ > \"$DIR/leader\"\n  /bin/sleep 600 &\n  \
                                       echo $! > \"$DIR/grandchild\"\n  wait";

/// The processes in group `group` that are still alive — not yet exited, or
/// exited and waiting to be reaped is not "alive" — read from `/proc`.
#[cfg(target_os = "linux")]
pub(super) fn alive_in_group(group: i32) -> Vec<i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        panic!("/proc is not readable");
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
        .filter(|pid| {
            let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                return false;
            };
            // After the command's closing parenthesis: state, parent, group.
            let Some((_, after)) = stat.rsplit_once(')') else {
                return false;
            };
            let fields: Vec<&str> = after.split_whitespace().collect();
            fields.get(2).and_then(|g| g.parse::<i32>().ok()) == Some(group)
                && fields.first() != Some(&"Z")
        })
        .collect()
}

/// The whole boundary over `repository`, `git` coming from `discovery`.
fn opened(repository: &Path, discovery: &Discovery) -> (RepositoryHandle, Updates) {
    match open(repository, discovery) {
        Ok((handle, updates, _reply)) => (handle, updates),
        Err(error) => panic!("starting the worker: {error}"),
    }
}

/// Reads updates until the stream ends, returning them and how long the end
/// took; a stream still open `within` after the call is a failure, then and
/// there, rather than a wait on the boundary's whole patience.
fn until_the_stream_ends(updates: &mut Updates, within: Duration) -> (Vec<Update>, Duration) {
    let started = Instant::now();
    let mut seen = Vec::new();
    while let Some(update) = next_by(updates, started + within, &seen) {
        seen.push(update);
    }
    (seen, started.elapsed())
}

/// The next update, within the boundary's patience.
fn next(updates: &mut Updates) -> Option<Update> {
    next_by(updates, Instant::now() + WAIT, &[])
}

/// PRD G16: `git` is found once per application, not once per repository
/// opened. Three repositories opened, each through to its first page, on the
/// one discovery started as the application starts; then, as the negative,
/// two opened with a discovery each, which must probe twice — so the count is
/// one that moves. Caught by: discovery moving back into `open`, or a
/// `Discovery` that does not keep its answer.
#[test]
fn git_is_found_once_per_application_not_once_per_repository() {
    let stub = StubGit::new("  exit 0");
    let fixture = UnbornRepository::new(&format!("cairn-discovered-once-{}", std::process::id()));
    let before = stub.probes();

    // Started, as `main` starts it, and raced by the first open.
    let discovery = Discovery::start_with(stub.startup(None));
    for _ in 0..3 {
        let (handle, mut updates) = opened(&fixture.path, &discovery);
        handle.submit(Request::OpenHistory { rows: 1 });
        match next(&mut updates) {
            Some(Update::Rows { .. }) => {}
            other => panic!("the repository was not served: {other:?}"),
        }
        handle.submit(Request::Close);
        until_the_stream_ends(&mut updates, Duration::from_secs(10));
    }
    assert_eq!(
        stub.probes() - before,
        1,
        "git was probed more than once for one application"
    );

    let before = stub.probes();
    for _ in 0..2 {
        let (handle, mut updates) = opened(&fixture.path, &Discovery::new(stub.startup(None)));
        handle.submit(Request::Close);
        until_the_stream_ends(&mut updates, Duration::from_secs(10));
    }
    assert_eq!(
        stub.probes() - before,
        2,
        "the negative: a discovery per open must probe per open, or the count above proves \
         nothing"
    );
}

/// PRD G16's other half: a `git` refused at discovery is refused once, and
/// every repository that asks is told so in the same words, naming the
/// version found and the version Cairn needs. Caught by: a discovery that
/// keeps only a found `git` and probes again after a refusal (the count
/// moves), or a refusal reworded per repository.
#[test]
fn a_git_refused_at_discovery_is_refused_to_every_repository_that_asks() {
    let stub = StubGit::reporting("2.20.0", "  exit 0");
    let fixture = UnbornRepository::new(&format!("cairn-refused-git-{}", std::process::id()));
    let before = stub.probes();
    let discovery = Discovery::new(stub.startup(None));
    let mut said = Vec::new();
    for _ in 0..2 {
        let (_handle, mut updates) = opened(&fixture.path, &discovery);
        match next(&mut updates) {
            Some(Update::Failed { message }) => said.push(message),
            other => panic!("expected the refusal, got {other:?}"),
        }
        assert!(next(&mut updates).is_none(), "the repository was served");
    }
    assert!(
        said[0].contains("2.30.0") && said[0].contains("2.20.0"),
        "the refusal does not name both versions: {}",
        said[0]
    );
    assert_eq!(
        said[0], said[1],
        "the refusal was said differently the second time"
    );
    assert_eq!(
        stub.probes() - before,
        1,
        "a refused git was probed again for the second repository"
    );
}

/// PRD G15, the worker's half: a second fetch while one runs is refused with a
/// reason naming the fetch in flight — never dropped in silence — and the fetch
/// in flight goes on. Caught by: `perform` returning without a word when `arm`
/// refuses, or forwarding the second fetch anyway.
#[test]
fn a_second_fetch_while_one_runs_is_refused_with_a_reason() {
    let stub = StubGit::new(HANGS_WITH_A_GRANDCHILD);
    let fixture = with_origin(
        &format!("cairn-refused-fetch-{}", std::process::id()),
        "/nonexistent/remote.git",
    );
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let discovery = Discovery::new(stub.startup(Some((&home, &runtime))));
    let (handle, mut updates) = opened(&fixture.path, &discovery);

    let origin = || Request::Fetch {
        remote: "origin".to_owned(),
    };
    handle.submit(origin());
    collect_until(&mut updates, |u| matches!(u, Update::FetchStarted { .. }));
    handle.submit(origin());
    let seen = collect_until(&mut updates, |u| {
        matches!(
            u,
            Update::FetchRefused { .. }
                | Update::FetchStarted { .. }
                | Update::FetchFinished { .. }
                | Update::FetchFailed { .. }
                | Update::FetchCancelled { .. }
        )
    });
    assert_eq!(
        seen.last(),
        Some(&Update::FetchRefused {
            remote: "origin".to_owned(),
            reason: "a fetch of origin is already running".to_owned(),
        }),
        "the second fetch was not refused with a reason: {seen:?}"
    );

    // The first is still the one in flight: cancelling it ends it.
    handle.submit(Request::CancelFetch);
    let seen = collect_until(&mut updates, |u| {
        matches!(
            u,
            Update::FetchCancelled { .. }
                | Update::FetchFinished { .. }
                | Update::FetchFailed { .. }
        )
    });
    assert!(
        matches!(seen.last(), Some(Update::FetchCancelled { .. })),
        "{seen:?}"
    );
    handle.submit(Request::Close);
    until_the_stream_ends(&mut updates, CLOSE_BOUND);
}

/// PRD G17, the worker's half: the command log is answered through the
/// boundary as `cairn-model` values, and a fetch's entry is in it once, with
/// what it was given and how it ended. The probe ran in no repository, so it
/// is not. Caught by: the request going unanswered, or answered from anywhere
/// but the repository's log.
#[test]
fn the_command_log_is_answered_through_the_worker_with_the_fetch_in_it() {
    let stub = StubGit::new("  echo 'Receiving objects: 100%' >&2\n  exit 0");
    let fixture = with_origin(
        &format!("cairn-logged-fetch-{}", std::process::id()),
        "/nonexistent/remote.git",
    );
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let discovery = Discovery::new(stub.startup(Some((&home, &runtime))));
    let (handle, mut updates) = opened(&fixture.path, &discovery);

    handle.submit(Request::CommandLog);
    let seen = collect_until(&mut updates, |u| matches!(u, Update::CommandLog { .. }));
    assert_eq!(
        seen.last(),
        Some(&Update::CommandLog {
            records: Vec::new()
        }),
        "a repository that has run nothing has something in its log"
    );

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    let seen = collect_until(&mut updates, |u| {
        matches!(
            u,
            Update::FetchFinished { .. }
                | Update::FetchFailed { .. }
                | Update::FetchCancelled { .. }
        )
    });
    assert!(
        matches!(seen.last(), Some(Update::FetchFinished { .. })),
        "{seen:?}"
    );

    handle.submit(Request::CommandLog);
    let seen = collect_until(&mut updates, |u| matches!(u, Update::CommandLog { .. }));
    let Some(Update::CommandLog { records }) = seen.last() else {
        panic!("no log: {seen:?}");
    };
    assert_eq!(records.len(), 1, "{records:?}");
    let record = &records[0];
    assert_eq!(record.arguments, FETCH);
    assert_eq!(
        record
            .directory
            .as_deref()
            .map(std::fs::canonicalize)
            .and_then(Result::ok),
        std::fs::canonicalize(&fixture.path).ok()
    );
    assert_eq!(record.exit, CommandExit::Code(0));
    assert!(!record.cancelled);
    assert_eq!(record.stderr, "Receiving objects: 100%");

    handle.submit(Request::Close);
    until_the_stream_ends(&mut updates, CLOSE_BOUND);
}

/// PRD G14: closing a repository with a fetch in flight ends and reaps its
/// whole group — the leader and a grandchild holding its pipes — within
/// `CLOSE_BOUND`, and the update stream ends with nothing left running and the
/// askpass socket gone. Caught by: a close that does not end the registry's
/// invocations (the grandchild keeps the pipes, the lane never finishes, and
/// the stream outlives the bound), or one that ends only the leader.
#[test]
fn closing_a_repository_ends_and_reaps_every_git_in_it_within_the_bound() {
    let stub = StubGit::new(HANGS_WITH_A_GRANDCHILD);
    let fixture = with_origin(
        &format!("cairn-closed-fetch-{}", std::process::id()),
        "/nonexistent/remote.git",
    );
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let discovery = Discovery::new(stub.startup(Some((&home, &runtime))));
    let (handle, mut updates) = opened(&fixture.path, &discovery);

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    collect_until(&mut updates, |u| matches!(u, Update::FetchStarted { .. }));
    let (leader, grandchild) = (stub.pid("leader"), stub.pid("grandchild"));
    let made: Vec<_> = std::fs::read_dir(&runtime.path)
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    assert_eq!(
        made.len(),
        1,
        "the case needs the askpass channel's directory there while the fetch runs"
    );
    #[cfg(target_os = "linux")]
    {
        let alive = alive_in_group(leader);
        assert!(
            alive.contains(&leader) && alive.contains(&grandchild),
            "the case needs the fetch and its grandchild running in one group: {alive:?}"
        );
    }

    handle.submit(Request::Close);
    let (seen, took) = until_the_stream_ends(&mut updates, CLOSE_BOUND);
    assert!(
        seen.iter()
            .any(|u| matches!(u, Update::FetchCancelled { .. })),
        "the fetch was not ended as a cancel: {seen:?}"
    );
    assert!(took < CLOSE_BOUND, "the close took {took:?}");
    #[cfg(target_os = "linux")]
    {
        assert_eq!(
            alive_in_group(leader),
            Vec::<i32>::new(),
            "the fetch's group outlived the close"
        );
        assert!(
            !Path::new(&format!("/proc/{leader}")).exists(),
            "the leader was not reaped"
        );
    }
    let left: Vec<_> = std::fs::read_dir(&runtime.path)
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    assert!(
        left.is_empty(),
        "the askpass socket outlived the close: {left:?}"
    );
    drop(handle);
    // Read only by the Linux checks above.
    let _ = (leader, grandchild);
}

/// PRD R6.3 and the QA brief: a fetch forwarded to the network lane and
/// closed at once — mid-spawn or just running when the close lands, whichever
/// this run catches — is still ended as a cancel, never missed for not yet
/// being in the registry when the close looked. The command log is asked for
/// in between because the repository thread answers in order: once it has
/// answered, the fetch has been forwarded, so the close cannot drop it before
/// it reaches the lane. Caught by: a close that ends only what is registered
/// when it starts, or none at all — the stub then runs for ten minutes and the
/// stream outlives the bound.
#[test]
fn a_fetch_closed_as_it_starts_is_still_ended() {
    let stub = StubGit::new(HANGS_WITH_A_GRANDCHILD);
    let fixture = with_origin(
        &format!("cairn-closed-at-start-{}", std::process::id()),
        "/nonexistent/remote.git",
    );
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let discovery = Discovery::new(stub.startup(Some((&home, &runtime))));
    let (handle, mut updates) = opened(&fixture.path, &discovery);

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    handle.submit(Request::CommandLog);
    let before = collect_until(&mut updates, |u| matches!(u, Update::CommandLog { .. }));
    handle.submit(Request::Close);
    let (seen, took) = until_the_stream_ends(&mut updates, CLOSE_BOUND);
    assert!(took < CLOSE_BOUND, "the close took {took:?}: {seen:?}");
    let ended = before
        .iter()
        .chain(&seen)
        .filter(|u| {
            matches!(
                u,
                Update::FetchCancelled { .. }
                    | Update::FetchFinished { .. }
                    | Update::FetchFailed { .. }
            )
        })
        .collect::<Vec<_>>();
    assert!(
        matches!(ended.as_slice(), [Update::FetchCancelled { .. }]),
        "the fetch forwarded before the close did not end, once, as a cancel: {before:?} {seen:?}"
    );
    // The stub may not have got as far as writing its pid; if it did, its group is gone.
    #[cfg(target_os = "linux")]
    if let Some(leader) = std::fs::read_to_string(stub.directory.join("leader"))
        .ok()
        .and_then(|text| text.trim().parse::<i32>().ok())
    {
        assert_eq!(
            alive_in_group(leader),
            Vec::<i32>::new(),
            "a fetch that started as the repository closed outlived it: {seen:?}"
        );
    }
    drop(handle);
}
