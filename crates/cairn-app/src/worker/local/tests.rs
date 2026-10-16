//! The local write lane through the real boundary (staging-and-commit C10, C11, C12): writes
//! in order and each with its own ending, a status read across a write never drawn, a commit
//! that keeps refreshes back and a stage queued behind it, a cancel that reaches only the
//! commit it names, a close that waits on a commit and ends nothing, the lock files named as
//! a repository opens and by the write they fail, and a prompt a write's child raises answered
//! through the window, titled by its own operation.
//!
//! Every commit here is a real `git commit`, held in its `pre-commit` hook for as long as a
//! test says ([`held_hook`]) — the slow hook the commit-dependent halves of C10-C12 name.
//! Nothing waits a fixed time to see that nothing happened: a test waits on the answer to a
//! later request instead ([`after_the_repository_thread`]), or on what a held process wrote.
//!
//! No `Command` here — the guards scan this crate's tests — so fixtures are built with
//! `std::fs`, every write goes through the lane, and a stub `git` is a `/bin/sh` script.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cairn_git::ops::{Askpass, GitBinary};
use cairn_git::{CancelSignal, ContentOptions, Repository, WorkingTreeDiff};
use cairn_model::{
    ChangeList, CommandExit, Confirmed, FileDiff, RepoPath, Secret, Selection, StagedChange,
    StatusEntry, WorkingTreeStatus,
};

use crate::worker::Reply;
use crate::worker::discovery::Discovery;
use crate::worker::lifecycle_tests::StubGit;
use crate::worker::local::lane::{
    LOCK_RETRIES, LOCK_RETRY_AFTER, LocalWrite, OperationId, ReadAgain, WriteEnding,
};
use crate::worker::network::tests::{
    Home, RuntimeDir, UnbornRepository, WAIT, built_helper, collect_until, next_by, with_origin,
};
use crate::worker::pool::{Replier, RepositoryHandle, Updates, open};
use crate::worker::request::{CheckoutRefused, RanBy, Request, Update};
use crate::worker::startup::Startup;

/// The real `git` on this process's `PATH`, which a stub hands every other verb to.
fn real_git() -> PathBuf {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("no git on this process's PATH"))
}

/// A stub `git` in front of the real one: each `(verb, body)` does what its body says, `$DIR`
/// naming the stub's directory and `$REAL` the real git; every other invocation is the real
/// git's. The verb is the first argument past the repository's location and a local write's
/// `--literal-pathspecs`.
fn stub(verbs: &[(&str, &str)]) -> StubGit {
    let real = real_git();
    let arms: String = verbs
        .iter()
        .map(|(verb, body)| format!("{verb})\n{body}\n  ;;\n"))
        .collect();
    StubGit::scripted(|directory| {
        format!(
            "#!/bin/sh\nDIR='{}'\nREAL='{}'\nVERB=\n\
             for argument in \"$@\"; do case \"$argument\" in \
             --git-dir=*|--work-tree=*|--literal-pathspecs) ;; \
             *) VERB=\"$argument\"; break ;; esac; done\n\
             case \"$VERB\" in\n{arms}*)\n  exec \"$REAL\" \"$@\"\n  ;;\nesac\n",
            directory.display(),
            real.display()
        )
    })
}

/// A held write: its pid on a line of `$DIR/<name>`, then waiting until `$DIR/release` or
/// `$DIR/release.<pid>` exists — bounded, so a failing test leaves nothing running long — and
/// its finishing written to `$DIR/finished`. `/bin/sleep` by its path: the stub's `PATH` is its
/// own directory.
fn held(name: &str) -> String {
    format!(
        "  echo $$ >> \"$DIR/{name}\"\n  n=0\n  \
         while [ ! -e \"$DIR/release\" ] && [ ! -e \"$DIR/release.$$\" ] && [ $n -lt 1200 ]; \
         do /bin/sleep 0.05; n=$((n+1)); done\n  echo $$ >> \"$DIR/finished\"\n  exit 0"
    )
}

/// A `pre-commit` hook in `repository` held as [`held`] holds, its pid on a line of
/// `<directory>/commits`: the slow hook a real `git commit` runs while a test does what it
/// says. `directory` is a stub's, whose release and finish files it shares.
fn held_hook(repository: &Path, directory: &Path) {
    hook(
        repository,
        "pre-commit",
        &format!("DIR='{}'\n{}\n", directory.display(), held("commits")),
    );
}

/// An executable hook named `name` in `repository`, `body` after its `#!/bin/sh`.
fn hook(repository: &Path, name: &str, body: &str) {
    let hooks = repository.join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap_or_else(|error| panic!("{error}"));
    let path = hooks.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap_or_else(|error| panic!("{error}"));
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Names who commits in `repository`'s own configuration, as a user's does.
fn identify(repository: &Path) {
    let config = repository.join(".git/config");
    let mut text = std::fs::read_to_string(&config).unwrap_or_else(|error| panic!("{error}"));
    text.push_str("[user]\n\tname = Commit Ter\n\temail = committer@example.com\n");
    std::fs::write(&config, text).unwrap_or_else(|error| panic!("{error}"));
}

/// The process group `pid` is in, read from `/proc`: a hook's is its `git commit`'s.
#[cfg(target_os = "linux")]
fn group_of(pid: i32) -> i32 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .unwrap_or_else(|error| panic!("/proc/{pid}/stat: {error}"));
    let Some((_, after)) = stat.rsplit_once(')') else {
        panic!("{stat}");
    };
    after
        .split_whitespace()
        .nth(2)
        .and_then(|group| group.parse().ok())
        .unwrap_or_else(|| panic!("{stat}"))
}

/// A status that reads the repository as it is when it begins, then is held as [`held`] is,
/// then answers what it read: a status that began before whatever the test does meanwhile.
fn status_read_then_held() -> String {
    "  \"$REAL\" \"$@\" > \"$DIR/read.$$\"\n  echo $$ >> \"$DIR/statuses\"\n  n=0\n  \
     while [ ! -e \"$DIR/release\" ] && [ ! -e \"$DIR/release.$$\" ] && [ $n -lt 1200 ]; \
     do /bin/sleep 0.05; n=$((n+1)); done\n  /bin/cat \"$DIR/read.$$\""
        .to_owned()
}

/// A status the real git answers, its pid on a line of `$DIR/statuses` first.
const STATUS_COUNTED: &str = "  echo $$ >> \"$DIR/statuses\"\n  exec \"$REAL\" \"$@\"";

/// The pids `$DIR/<name>` lists, in order.
fn pids(stub: &StubGit, name: &str) -> Vec<i32> {
    std::fs::read_to_string(stub.directory.join(name))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

/// Waits, within [`WAIT`], until `$DIR/<name>` lists `count` pids.
fn until_pids(stub: &StubGit, name: &str, count: usize) -> Vec<i32> {
    let deadline = Instant::now() + WAIT;
    loop {
        let listed = pids(stub, name);
        if listed.len() >= count {
            return listed;
        }
        assert!(
            Instant::now() < deadline,
            "{} of {count} in {name}",
            listed.len()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn release(stub: &StubGit, pid: i32) {
    let file = stub.directory.join(format!("release.{pid}"));
    std::fs::write(&file, "").unwrap_or_else(|error| panic!("writing {}: {error}", file.display()));
}

/// Releases everything the stub holds when dropped, however the test ends.
struct ReleaseAll<'a>(&'a StubGit);

impl Drop for ReleaseAll<'_> {
    fn drop(&mut self) {
        let _ = std::fs::write(self.0.directory.join("release"), "");
    }
}

/// The boundary over `repository` with `stub` as its `git`, a `HOME` and a runtime directory
/// for the askpass channel.
fn boundary(
    repository: &Path,
    stub: &StubGit,
    around: (&Home, &RuntimeDir),
) -> (RepositoryHandle, Updates, Replier) {
    match open(repository, &Discovery::new(stub.startup(Some(around)))) {
        Ok((handle, mut updates, reply)) => {
            crate::worker::network::tests::opened_as(&mut updates);
            (handle, updates, reply)
        }
        Err(error) => panic!("starting the worker: {error}"),
    }
}

/// The boundary over `repository` with this process's own `git`, a `HOME` and a runtime
/// directory for the askpass channel, past its `Update::Opened`.
fn real_boundary(
    repository: &Path,
    around: (&Home, &RuntimeDir),
) -> (RepositoryHandle, Updates, Replier) {
    let (handle, mut updates, reply) = real_open(repository, around);
    crate::worker::network::tests::opened_as(&mut updates);
    (handle, updates, reply)
}

/// [`real_boundary`], its first update still to read.
fn real_open(
    repository: &Path,
    (home, runtime): (&Home, &RuntimeDir),
) -> (RepositoryHandle, Updates, Replier) {
    let home = home.path.clone();
    let runtime = runtime.path.clone();
    let startup = Startup::new(
        move |name| match name {
            "PATH" => std::env::var_os("PATH"),
            "HOME" => Some(home.clone().into_os_string()),
            "XDG_RUNTIME_DIR" => Some(runtime.clone().into_os_string()),
            _ => None,
        },
        built_helper(),
    );
    match open(repository, &Discovery::new(startup)) {
        Ok(opened) => opened,
        Err(error) => panic!("starting the worker: {error}"),
    }
}

/// An unborn repository whose `path` git has staged as `text` — through the local lane, as a
/// stage asked of the window is — for the window's tests through a real refresh, which may not
/// wait themselves (C26's window test; merge bar, phase 21, item 12). Its working tree is the
/// test's to write, so the path's unstaged diff is whatever the test makes it.
pub(crate) struct StagedFile {
    fixture: UnbornRepository,
    home: Home,
    runtime: RuntimeDir,
}

impl StagedFile {
    pub(crate) fn new(name: &str, path: &str, text: &str) -> Self {
        let staged = Self {
            fixture: UnbornRepository::new(&format!("{name}-{}", std::process::id())),
            home: Home::new(),
            runtime: RuntimeDir::new(),
        };
        staged.write(path, text);
        let (handle, mut updates, _reply) =
            real_boundary(&staged.fixture.path, (&staged.home, &staged.runtime));
        let id = ask(&handle, stage(&[path]));
        let seen = until_ended(&mut updates, id);
        assert!(
            matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
            "staging {path}: {seen:?}"
        );
        drop(handle);
        staged
    }

    /// The boundary over the repository, its first update — what it opened — still to read.
    pub(crate) fn open(&self) -> (RepositoryHandle, Updates) {
        let (handle, updates, _reply) = real_open(&self.fixture.path, (&self.home, &self.runtime));
        (handle, updates)
    }

    /// Writes `text` to `path` in the working tree.
    pub(crate) fn write(&self, path: &str, text: &str) {
        let at = self.fixture.path.join(path);
        std::fs::write(&at, text)
            .unwrap_or_else(|error| panic!("writing {}: {error}", at.display()));
    }
}

/// An unborn repository named `name` whose working tree holds each of `files`, untracked.
fn with_files(name: &str, files: &[&str]) -> UnbornRepository {
    let fixture = UnbornRepository::new(&format!("{name}-{}", std::process::id()));
    write_files(&fixture.path, files);
    fixture
}

fn write_files(root: &Path, files: &[&str]) {
    for file in files {
        let path = root.join(file);
        std::fs::write(&path, format!("{file}\nline two\n"))
            .unwrap_or_else(|error| panic!("writing {}: {error}", path.display()));
    }
}

/// `path`'s untracked diff as the engine answers it, with every line of it selected.
fn untracked(repository: &Path, path: &str) -> (FileDiff, Selection) {
    let git = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
        .unwrap_or_else(|error| panic!("finding git: {error}"));
    let repo = Repository::discover(repository).unwrap_or_else(|error| panic!("{error}"));
    let diff = repo
        .working_tree_diff(
            &git,
            &RepoPath::from(path),
            WorkingTreeDiff::Untracked,
            &ContentOptions::default(),
            &CancelSignal::new(),
        )
        .unwrap_or_else(|error| panic!("diffing {path}: {error}"))
        .unwrap_or_else(|| panic!("{path} has no untracked diff"));
    let selection =
        Selection::with_every_change(diff.text().unwrap_or_else(|| panic!("{path} drew no text")));
    (diff, selection)
}

fn stage(paths: &[&str]) -> LocalWrite {
    LocalWrite::StageFiles {
        paths: paths.iter().map(|path| RepoPath::from(*path)).collect(),
    }
}

fn commit() -> LocalWrite {
    LocalWrite::Commit {
        message: "the subject\n\nthe body\n".to_owned(),
        skip_hooks: false,
        files: 1,
    }
}

/// Asks for `write` under a fresh id, as the window does.
fn ask(handle: &RepositoryHandle, write: LocalWrite) -> OperationId {
    let id = OperationId::next();
    handle.submit(Request::Write { id, write });
    id
}

/// What a lane's news says, in the order it came: each start and each ending.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Lane {
    Started(OperationId),
    Ended(OperationId),
}

fn lane_news(seen: &[Update]) -> Vec<Lane> {
    seen.iter()
        .filter_map(|update| match update {
            Update::WriteStarted { id } => Some(Lane::Started(*id)),
            Update::WriteEnded { id, .. } => Some(Lane::Ended(*id)),
            _ => None,
        })
        .collect()
}

/// The ending of `id` among `seen`, and what it said to read again.
fn ending_of(seen: &[Update], id: OperationId) -> (WriteEnding, ReadAgain) {
    seen.iter()
        .find_map(|update| match update {
            Update::WriteEnded {
                id: ended,
                ending,
                read_again,
                ..
            } if *ended == id => Some((ending.clone(), *read_again)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{id:?} has not ended: {seen:?}"))
}

/// Reads updates until `id` has ended.
fn until_ended(updates: &mut Updates, id: OperationId) -> Vec<Update> {
    collect_until(
        updates,
        |update| matches!(update, Update::WriteEnded { id: ended, .. } if *ended == id),
    )
}

/// Every update sent before the repository thread answered a request asked now: the command
/// log, which that thread answers in turn, so whatever it — or any thread, earlier — sent
/// before is ahead of it on the one stream. What a test waits on instead of a fixed time.
fn after_the_repository_thread(handle: &RepositoryHandle, updates: &mut Updates) -> Vec<Update> {
    handle.submit(Request::CommandLog);
    let mut seen = collect_until(updates, |update| {
        matches!(update, Update::CommandLog { .. })
    });
    seen.pop();
    seen
}

/// Every update already on the stream, waiting for none: what was sent before a process the
/// test watched wrote its mark.
fn already_sent(updates: &mut Updates) -> Vec<Update> {
    let mut seen = Vec::new();
    while let Some(next) = next_by_or_quiet(updates, Duration::ZERO) {
        match next {
            Some(update) => seen.push(update),
            None => panic!("the stream ended: {seen:?}"),
        }
    }
    seen
}

/// Every update that arrives within `quiet`: only where the wait itself is what is tested —
/// a close's patience run out.
fn arriving_within(updates: &mut Updates, quiet: Duration) -> Vec<Update> {
    let deadline = Instant::now() + quiet;
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return seen;
        }
        // `next_by` fails at its deadline; this waits only as long as is left.
        match next_by_or_quiet(updates, left) {
            Some(Some(update)) => seen.push(update),
            Some(None) => panic!("the stream ended: {seen:?}"),
            None => return seen,
        }
    }
}

/// The next update within `wait`: `Some(None)` once the stream has ended, `None` if nothing
/// came.
fn next_by_or_quiet(updates: &mut Updates, wait: Duration) -> Option<Option<Update>> {
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut next = std::pin::pin!(updates.next());
    let deadline = Instant::now() + wait;
    loop {
        if let Poll::Ready(next) = next.as_mut().poll(&mut cx) {
            return Some(next);
        }
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        std::thread::park_timeout(deadline - now);
    }
}

/// The statuses drawn among `seen`.
fn statuses(seen: &[Update]) -> Vec<WorkingTreeStatus> {
    seen.iter()
        .filter_map(|update| match update {
            Update::Status { changes } => Some(changes.status().clone()),
            _ => None,
        })
        .collect()
}

/// The paths a status lists as staged additions, and as untracked.
fn staged_and_untracked(status: &WorkingTreeStatus) -> (Vec<String>, Vec<String>) {
    let WorkingTreeStatus::Listed(entries) = status else {
        panic!("status could not be read: {status:?}");
    };
    let mut staged = Vec::new();
    let mut untracked = Vec::new();
    for entry in entries {
        match entry {
            StatusEntry::Changed(changed) if changed.staged == Some(StagedChange::Added) => {
                staged.push(changed.path.to_string());
            }
            StatusEntry::Untracked(path) => untracked.push(path.to_string()),
            StatusEntry::Changed(_) | StatusEntry::Conflicted(_) => {}
        }
    }
    staged.sort();
    untracked.sort();
    (staged, untracked)
}

/// C10 and the QA brief: five stages asked while the first is still held in its `git add` —
/// every one asked before it has finished — all run, one at a time, in the order asked, each
/// with its own ending;
/// the stale one (a second stage of lines built from the same diff as the first, which the
/// first made stale) is dropped naming its path and the ones behind it still run; each says to
/// read status again, and status alone; and the status read after them lists what they did.
/// Caught by: asking that waits for a write, writes refused or run beside each other for being
/// second, an ending under the wrong id, a stale patch that stalls the lane or is applied, or a
/// stage that reads more than status again.
#[test]
fn writes_asked_faster_than_they_run_run_in_order_each_with_its_own_ending() {
    let fixture = with_files("cairn-lane-in-order", &["a", "b", "c", "d"]);
    let body = format!(
        "{}\n",
        held("adds").replace("  exit 0", "  exec \"$REAL\" \"$@\"")
    );
    let stub = stub(&[("add", &body)]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    let (diff, selection) = untracked(&fixture.path, "b");

    let first = ask(&handle, stage(&["a"]));
    until_pids(&stub, "adds", 1);
    let ids = [
        first,
        ask(
            &handle,
            LocalWrite::StageLines {
                diff: Arc::new(diff.clone()),
                selection: selection.clone(),
            },
        ),
        ask(
            &handle,
            LocalWrite::StageLines {
                diff: Arc::new(diff),
                selection,
            },
        ),
        ask(&handle, stage(&["c"])),
        ask(&handle, stage(&["d"])),
    ];
    assert_eq!(
        (pids(&stub, "adds").len(), pids(&stub, "finished")),
        (1, Vec::new()),
        "asking waited for the write running"
    );
    let _ = std::fs::write(stub.directory.join("release"), "");
    let seen = until_ended(&mut updates, ids[4]);
    let expected: Vec<Lane> = ids
        .iter()
        .flat_map(|id| [Lane::Started(*id), Lane::Ended(*id)])
        .collect();
    assert_eq!(lane_news(&seen), expected, "not one at a time, in order");
    for (at, id) in ids.iter().enumerate() {
        let (ending, read_again) = ending_of(&seen, *id);
        assert_eq!(
            read_again,
            ReadAgain::Status,
            "write {at} reads more than status"
        );
        match (at, &ending) {
            (2, WriteEnding::Stale { path, said }) => {
                assert_eq!(path, "b");
                assert_eq!(said, "b changed after its diff was read.");
            }
            (2, other) => panic!("the stale stage was not dropped: {other:?}"),
            (_, WriteEnding::Done(done)) => assert!(done.acknowledged.is_none()),
            (_, other) => panic!("write {at} did not run: {other:?}"),
        }
    }

    handle.submit(Request::RefreshStatus);
    let seen = collect_until(&mut updates, |update| {
        matches!(update, Update::Status { .. })
    });
    assert!(
        !seen
            .iter()
            .any(|update| matches!(update, Update::Refs { .. } | Update::AheadBehind { .. })),
        "a status refresh read the refs: {seen:?}"
    );
    let drawn = statuses(&seen);
    assert_eq!(
        staged_and_untracked(&drawn[0]),
        (
            vec![
                "a".to_owned(),
                "b".to_owned(),
                "c".to_owned(),
                "d".to_owned()
            ],
            Vec::new()
        )
    );
    drop(handle);
}

/// C10 and the QA brief: a status that begins before a write and ends after it is never
/// drawn — the status reads the repository, is held while a stage runs and ends, and answers
/// what it read once released — and the status read after the write is the one drawn, listing
/// what it did. Deterministic: each status is released by the test. Caught by: a status sent
/// whatever writes ran while it was read (the pre-write lists are drawn after the write).
#[test]
fn a_status_begun_before_a_write_ended_is_never_drawn() {
    let fixture = with_files("cairn-lane-stale-status", &["a"]);
    let stub = stub(&[("status", &status_read_then_held())]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));

    handle.submit(Request::RefreshStatus);
    let before = until_pids(&stub, "statuses", 1)[0];
    let id = ask(&handle, stage(&["a"]));
    let mut seen = until_ended(&mut updates, id);
    assert!(matches!(ending_of(&seen, id).0, WriteEnding::Done(_)));

    release(&stub, before);
    // What the window asks once the write has ended. The refresh thread starts it only once it
    // has done with the status before it, so by the time it has begun, that status's answer
    // was sent or dropped: whatever was sent is already on the stream.
    handle.submit(Request::RefreshStatus);
    let after = until_pids(&stub, "statuses", 2)[1];
    seen.extend(already_sent(&mut updates));
    assert_eq!(
        statuses(&seen),
        [],
        "the status read before the write was drawn after it"
    );
    release(&stub, after);
    seen.extend(collect_until(&mut updates, |update| {
        matches!(update, Update::Status { .. })
    }));
    let drawn = statuses(&seen);
    assert_eq!(drawn.len(), 1, "{seen:?}");
    assert_eq!(
        staged_and_untracked(&drawn[0]),
        (vec!["a".to_owned()], Vec::new()),
        "the status drawn is not the one read after the write"
    );
    drop(handle);
}

/// A repository named `name` with `files` untracked, an `origin` remote that is never
/// reached, and an identity: ready for a commit once something is staged.
fn to_commit(name: &str, files: &[&str]) -> UnbornRepository {
    let fixture = with_origin(
        &format!("{name}-{}", std::process::id()),
        "/nonexistent/origin",
    );
    identify(&fixture.path);
    write_files(&fixture.path, files);
    fixture
}

/// Stages `paths` through the lane and waits for it, as the window would before a commit.
fn staged(handle: &RepositoryHandle, updates: &mut Updates, paths: &[&str]) {
    let id = ask(handle, stage(paths));
    let seen = until_ended(updates, id);
    assert!(
        matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
}

/// C10 and the QA brief against a real `git commit` held in its `pre-commit` hook: while it
/// runs no refresh of Cairn's own starts — three asked of the handle start no status and
/// answer nothing, by the time the repository thread has answered a request asked after them
/// — and a stage asked meanwhile waits for it; its ending says to read everything again, and
/// once the window asks, one refresh is answered, after the stage behind it. Caught by: a
/// refresh read during a commit, a stage run beside it, or a commit's ending that reads less
/// than everything.
#[test]
fn a_commit_keeps_refreshes_back_and_a_stage_asked_meanwhile_waits_for_it() {
    let fixture = to_commit("cairn-lane-quiet-commit", &["a", "b"]);
    let stub = stub(&[("status", STATUS_COUNTED)]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    held_hook(&fixture.path, &stub.directory);
    let statuses_before = pids(&stub, "statuses").len();

    let commit = ask(&handle, commit());
    let pid = until_pids(&stub, "commits", 1)[0];
    let staged_b = ask(&handle, stage(&["b"]));
    handle.submit(Request::Refresh);
    handle.submit(Request::RefreshStatus);
    handle.submit(Request::Refresh);
    let meanwhile = after_the_repository_thread(&handle, &mut updates);
    assert_eq!(
        lane_news(&meanwhile),
        [Lane::Started(commit)],
        "the stage did not wait for the commit"
    );
    assert!(
        !meanwhile.iter().any(|update| matches!(
            update,
            Update::Refs { .. } | Update::Status { .. } | Update::AheadBehind { .. }
        )),
        "a refresh was answered while the commit ran: {meanwhile:?}"
    );
    assert_eq!(
        pids(&stub, "statuses").len(),
        statuses_before,
        "a status started while the commit ran"
    );

    release(&stub, pid);
    let seen = until_ended(&mut updates, commit);
    let (ending, read_again) = ending_of(&seen, commit);
    assert!(matches!(ending, WriteEnding::Done(_)), "{ending:?}");
    assert_eq!(read_again, ReadAgain::Everything);
    // What the window asks as the commit ends.
    handle.submit(Request::Refresh);
    let mut seen = until_ended(&mut updates, staged_b);
    assert_eq!(ending_of(&seen, staged_b).1, ReadAgain::Status);
    handle.submit(Request::RefreshStatus);
    seen.extend(collect_until(&mut updates, |update| {
        matches!(update, Update::Status { .. })
    }));
    seen.extend(after_the_repository_thread(&handle, &mut updates));
    let refs = seen
        .iter()
        .filter(|update| matches!(update, Update::Refs { .. }))
        .count();
    assert_eq!(
        refs, 1,
        "not the one refresh asked once the commit ended: {seen:?}"
    );
    // The refresh's own status may be drawn too, if it was read once the stage had ended;
    // never one read across it, and the last is the stage's: `a` committed, `b` staged.
    let drawn = statuses(&seen);
    let Some(last) = drawn.last() else {
        panic!("no status was drawn after the writes: {seen:?}");
    };
    assert_eq!(
        staged_and_untracked(last),
        (vec!["b".to_owned()], Vec::new())
    );
    drop(handle);
}

/// R4.6 on the threads' side, against a real commit: a refresh asked BEFORE the commit, whose
/// reads are still queued on the refresh thread — its status held, its ahead/behind waiting
/// behind it — when the commit starts, draws nothing while the commit runs: the status,
/// released, was read across a write and is dropped, and the ahead/behind is kept back as it
/// is taken up. The commit is held until the status's process has ended and the repository
/// thread has answered a later request, so the refresh thread takes the ahead/behind up while
/// it runs. Once the commit has ended and the window asks, one refresh is answered. Caught by:
/// the refresh thread counting ahead/behind while a commit runs (only the handle's gate kept
/// back).
#[test]
fn a_refresh_asked_before_a_commit_draws_nothing_while_it_runs() {
    let fixture = to_commit("cairn-lane-refresh-before-commit", &["a"]);
    let stub = stub(&[("status", &held("statuses"))]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    // Staged with the status held: released here, so nothing of it is left to answer.
    let id = ask(&handle, stage(&["a"]));
    until_ended(&mut updates, id);
    held_hook(&fixture.path, &stub.directory);

    handle.submit(Request::Refresh);
    let status = until_pids(&stub, "statuses", 1)[0];
    // A second refresh while the first's status is held: its refs are read on the
    // repository thread, which forwards its ahead/behind to the refresh thread, where it
    // waits behind the held status — and supersedes the first's, which may have been counted
    // before the status began.
    collect_until(&mut updates, |update| matches!(update, Update::Refs { .. }));
    handle.submit(Request::Refresh);
    collect_until(&mut updates, |update| matches!(update, Update::Refs { .. }));
    let commit = ask(&handle, commit());
    let pid = until_pids(&stub, "commits", 1)[0];
    release(&stub, status);
    let deadline = Instant::now() + WAIT;
    while !pids(&stub, "finished").contains(&status) {
        assert!(Instant::now() < deadline, "the status never ended");
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut seen = after_the_repository_thread(&handle, &mut updates);
    release(&stub, pid);
    seen.extend(until_ended(&mut updates, commit));
    let during: Vec<&Update> = seen
        .iter()
        .skip_while(|update| !matches!(update, Update::WriteStarted { id } if *id == commit))
        .take_while(|update| !matches!(update, Update::WriteEnded { id, .. } if *id == commit))
        .collect();
    assert!(!during.is_empty(), "the commit never started: {seen:?}");
    assert!(
        !during.iter().any(|update| matches!(
            update,
            Update::Refs { .. } | Update::Status { .. } | Update::AheadBehind { .. }
        )),
        "a refresh asked before the commit drew while it ran: {during:?}"
    );
    assert_eq!(
        pids(&stub, "statuses"),
        [status],
        "a status started while the commit ran"
    );
    assert_eq!(ending_of(&seen, commit).1, ReadAgain::Everything);

    // What the window asks as the commit ends: answered once, its status released.
    handle.submit(Request::Refresh);
    let follow_up = until_pids(&stub, "statuses", 2)[1];
    release(&stub, follow_up);
    let mut after = collect_until(&mut updates, |update| {
        matches!(update, Update::Status { .. })
    });
    while !after
        .iter()
        .any(|update| matches!(update, Update::AheadBehind { .. }))
    {
        after.extend(collect_until(&mut updates, |update| {
            matches!(update, Update::AheadBehind { .. })
        }));
    }
    after.extend(after_the_repository_thread(&handle, &mut updates));
    let count = |kind: fn(&Update) -> bool| after.iter().filter(|update| kind(update)).count();
    assert_eq!(
        (
            count(|update| matches!(update, Update::Refs { .. })),
            count(|update| matches!(update, Update::Status { .. })),
        ),
        (1, 1),
        "not one read of each after the commit: {after:?}"
    );
    drop(handle);
}

/// C10, R4.3, R4.7 and the QA brief, against real commits held in their hooks: a cancel names
/// its write and reaches only that one — a cancel for a commit still queued does nothing, the
/// running commit's ends it as cancelled (its hook killed, nothing committed, `HEAD` read after
/// the reap), and that same cancel sent again once the next commit runs leaves the next one
/// to finish and commit what is staged. Caught by: a cancel that reaches whatever runs (#47's
/// shape), or one kept for a write it was not for.
#[test]
fn a_cancel_names_its_commit_and_never_reaches_the_one_queued_behind_it() {
    let fixture = to_commit("cairn-lane-cancel-by-id", &["a"]);
    let stub = stub(&[]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    held_hook(&fixture.path, &stub.directory);

    let first = ask(&handle, commit());
    let second = ask(&handle, commit());
    until_pids(&stub, "commits", 1);
    handle.submit(Request::CancelWrite { id: second });
    handle.submit(Request::CancelWrite { id: first });
    let seen = until_ended(&mut updates, first);
    match ending_of(&seen, first) {
        (WriteEnding::Cancelled { .. }, ReadAgain::Everything) => {}
        other => panic!("the cancelled commit ended {other:?}"),
    }
    let next = until_pids(&stub, "commits", 2)[1];
    // Reaches the lane's state as it is submitted; a cancel that ended the second commit would
    // make its ending a cancel's rather than a commit's.
    handle.submit(Request::CancelWrite { id: first });
    release(&stub, next);
    let seen = until_ended(&mut updates, second);
    assert!(
        matches!(ending_of(&seen, second).0, WriteEnding::Done(_)),
        "a late cancel of the first commit reached the second: {seen:?}"
    );
    assert!(
        fixture.path.join(".git/refs/heads/main").is_file(),
        "the second commit committed nothing"
    );
    drop(handle);
}

/// R4.9 beside a fetch: a close asked while a fetch and a real commit both run ends the fetch
/// at once, as it ends every read and network operation, and waits on the commit alone — the
/// fetch's ending arrives while the commit is still held in its hook. Caught by: a close that
/// ends the fetch only once the local lane has been waited for, which leaves a fetch reaching
/// the network for as long as a commit's hooks run.
#[test]
fn a_close_ends_a_fetch_at_once_while_it_waits_on_a_commit() {
    let fixture = to_commit("cairn-lane-close-fetch", &["a"]);
    let stub = stub(&[("fetch", &held("fetches"))]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    held_hook(&fixture.path, &stub.directory);

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    until_pids(&stub, "fetches", 1);
    let commit = ask(&handle, commit());
    let pid = until_pids(&stub, "commits", 1)[0];
    handle.submit(Request::Close);
    let seen = collect_until(&mut updates, |update| {
        matches!(
            update,
            Update::FetchCancelled { .. }
                | Update::FetchFailed { .. }
                | Update::FetchFinished { .. }
        )
    });
    assert!(
        !lane_news(&seen).contains(&Lane::Ended(commit)),
        "the commit ended before the fetch: {seen:?}"
    );
    assert!(
        matches!(seen.last(), Some(Update::FetchCancelled { .. })),
        "the close did not end the fetch: {seen:?}"
    );
    release(&stub, pid);
    let mut seen = seen;
    while let Some(update) = next_by(&mut updates, Instant::now() + WAIT, &seen) {
        seen.push(update);
    }
    assert!(
        matches!(ending_of(&seen, commit).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
}

/// C11 and R4.9, against a real commit held in its hook: a close asked while it runs waits for
/// it — the stream stays open and the commit's `git` and its hook alive, past `CLOSE_BOUND`,
/// when a close that ended what runs would have given up on it — and ends nothing; once it
/// finishes, its ending arrives, a write queued behind it is not run, and the stream ends.
/// Caught by: a close that ends the commit's `git` (the registry's close-everything), or one
/// that runs the queued write.
#[test]
fn a_close_during_a_commit_waits_for_it_and_ends_nothing() {
    let fixture = to_commit("cairn-lane-close-waits", &["a", "b"]);
    let stub = stub(&[]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    held_hook(&fixture.path, &stub.directory);

    let commit = ask(&handle, commit());
    let pid = until_pids(&stub, "commits", 1)[0];
    #[cfg(target_os = "linux")]
    let group = group_of(pid);
    let queued = ask(&handle, stage(&["b"]));
    handle.submit(Request::Close);
    // Past CLOSE_BOUND, when a close that ends what runs has given up waiting on it: the one
    // wait here that is a length of time, since the patience running out is what is tested.
    let waited = arriving_within(
        &mut updates,
        cairn_git::CLOSE_BOUND + Duration::from_secs(1),
    );
    assert!(
        !lane_news(&waited).contains(&Lane::Ended(commit)),
        "the close ended the commit: {waited:?}"
    );
    #[cfg(target_os = "linux")]
    assert!(
        crate::worker::lifecycle_tests::alive_in_group(group).len() >= 2,
        "the commit's git or its hook is gone"
    );
    release(&stub, pid);
    let mut seen = waited;
    while let Some(update) = next_by(&mut updates, Instant::now() + WAIT, &seen) {
        seen.push(update);
    }
    assert!(
        matches!(ending_of(&seen, commit).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    assert!(
        matches!(ending_of(&seen, queued).0, WriteEnding::NotRun { .. }),
        "{seen:?}"
    );
    assert!(
        !lane_news(&seen).contains(&Lane::Started(queued)),
        "a write queued behind the close was run"
    );
    assert_eq!(
        pids(&stub, "finished"),
        [pid],
        "the commit's hook did not finish"
    );
}

/// R6.1 and R10.5 through the lane (phase 05's QA item 8): a commit whose `pre-commit` hook
/// fails ends `Failed` with the hook's words and commits nothing; asked again with
/// `skip_hooks`, it commits past the hook; and the next commit, hooks on, runs the hook again
/// and fails on it, leaving the branch where the skip put it. Caught by: the skip dropped, or
/// passed when not asked for, between the window's write and the engine's `Hooks`, or kept
/// for the commits after the one it was asked for.
#[test]
fn a_failing_hook_fails_a_commit_the_skip_commits_past_it_and_the_next_runs_it_again() {
    let fixture = to_commit("cairn-lane-hook-skip", &["a"]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    hook(
        &fixture.path,
        "pre-commit",
        "echo 'lint failed' >&2\nexit 3\n",
    );
    let head = fixture.path.join(".git/refs/heads/main");

    let refused = ask(&handle, commit());
    let seen = until_ended(&mut updates, refused);
    match ending_of(&seen, refused) {
        (
            WriteEnding::Failed(crate::worker::Failure {
                said: message,
                command,
                output,
                ..
            }),
            ReadAgain::Everything,
        ) => {
            assert_eq!(message, crate::worker::GIT_FAILED);
            // The Git Error dialog's command and git's own words (R10.5).
            assert_eq!(command.as_deref(), Some("git commit -q -F -"));
            assert!(output.contains("lint failed"), "{output}");
        }
        other => panic!("a failing hook's commit ended {other:?}"),
    }
    assert!(!head.exists(), "the commit was made past a failing hook");

    let skipped = ask(
        &handle,
        LocalWrite::Commit {
            message: "past the hook\n".to_owned(),
            skip_hooks: true,
            files: 1,
        },
    );
    let seen = until_ended(&mut updates, skipped);
    assert!(
        matches!(ending_of(&seen, skipped).0, WriteEnding::Done(_)),
        "the skip did not commit: {seen:?}"
    );
    assert!(head.is_file(), "the skip committed nothing");
    let skipped_to = std::fs::read_to_string(&head).unwrap_or_else(|error| panic!("{error}"));

    write_files(&fixture.path, &["b"]);
    staged(&handle, &mut updates, &["b"]);
    let again = ask(&handle, commit());
    let seen = until_ended(&mut updates, again);
    match ending_of(&seen, again).0 {
        WriteEnding::Failed(crate::worker::Failure { output, .. }) => {
            assert!(output.contains("lint failed"), "{output}");
        }
        other => panic!("the commit after the skip ended {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(&head).unwrap_or_else(|error| panic!("{error}")),
        skipped_to,
        "the commit after the skip was made past the failing hook"
    );
    drop(handle);
}

/// C11, C30 and R4.9, R12.4, F5: a lock left in the git directory — by a write a close gave up
/// on, say — is found by the stat every refresh makes, the open's first; a write git refuses for
/// it is run again about a second later, twice, before it ends failed with the lock's removal
/// offered; what that would cost is read at the press, and confirmed, the lane removes exactly
/// that file; the next stat finds it gone, and the write then runs. Caught by: an open that does
/// not stat, a write reported at its first refusal, a retry past two, or the removal of another
/// file.
#[test]
fn a_lock_left_behind_is_stated_retried_on_and_removed_through_its_confirmation() {
    let fixture = with_files("cairn-lane-lock-left", &["a"]);
    let lock = fixture.path.join(".git/index.lock");
    std::fs::write(&lock, "").unwrap_or_else(|error| panic!("{error}"));
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    handle.submit(Request::RefreshStatus);
    let seen = collect_until(&mut updates, |update| {
        matches!(update, Update::IndexLock { .. })
    });
    assert!(
        matches!(seen.last(), Some(Update::IndexLock { changed: Some(_) })),
        "{seen:?}"
    );
    let started = Instant::now();
    let id = ask(&handle, stage(&["a"]));
    let seen = until_ended(&mut updates, id);
    assert!(
        started.elapsed() >= LOCK_RETRY_AFTER * 2,
        "reported before it was tried again: {:?}",
        started.elapsed()
    );
    match ending_of(&seen, id).0 {
        WriteEnding::Failed(failure) => {
            assert!(failure.index_lock, "{failure:?}");
            assert!(
                failure
                    .locks
                    .iter()
                    .any(|path| path.ends_with(".git/index.lock")),
                "{failure:?}"
            );
        }
        other => panic!("a stage over index.lock ended {other:?}"),
    }
    // R12.1: what the write ran — its own `git add`, once and then twice again, and nothing the
    // refresh ran meanwhile.
    let commands = seen
        .iter()
        .find_map(|update| match update {
            Update::OperationRan {
                by: RanBy::Write(ran),
                commands,
            } if *ran == id => Some(commands.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no record: {seen:?}"));
    assert_eq!(commands.len(), 1 + LOCK_RETRIES, "{commands:?}");
    assert!(commands.iter().all(|command| {
        command.arguments.iter().any(|argument| argument == "add")
            && command.stderr.contains("index.lock")
    }));
    let asked = OperationId::next();
    handle.submit(Request::LockConsequence { asked });
    let answered = collect_until(&mut updates, |update| {
        matches!(update, Update::LockConsequence { .. })
    });
    let Some(Update::LockConsequence {
        asked: answered_for,
        outcome: Ok(offered),
    }) = answered.last().cloned()
    else {
        panic!("the press read no consequence: {answered:?}");
    };
    assert_eq!(answered_for, asked);
    let removed = ask(&handle, LocalWrite::RemoveLock(Confirmed::by_user(offered)));
    let seen = until_ended(&mut updates, removed);
    assert!(
        matches!(ending_of(&seen, removed).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    assert!(!lock.exists(), "the lock is still there");
    handle.submit(Request::RefreshStatus);
    let refreshed = collect_until(&mut updates, |update| {
        matches!(update, Update::IndexLock { .. })
    });
    assert!(
        matches!(refreshed.last(), Some(Update::IndexLock { changed: None })),
        "{refreshed:?}"
    );
    let read = collect_until(&mut updates, |update| {
        matches!(update, Update::Status { .. })
    });
    // C30, never a retry (merge bar, phase 21, item 13): the removal is followed by a refresh,
    // never by the refused write run again — no write starts but the removal itself, and the
    // stage the lock refused has not happened: `a` is still listed unstaged, never staged.
    let started: Vec<OperationId> = seen
        .iter()
        .chain(&refreshed)
        .chain(&read)
        .filter_map(|update| match update {
            Update::WriteStarted { id } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(
        started,
        vec![removed],
        "a write ran again after the removal"
    );
    let Some(Update::Status { changes }) = read.last() else {
        panic!("no status after the removal: {read:?}");
    };
    let a = RepoPath::from("a");
    assert!(
        changes.row_of(ChangeList::Unstaged, &a).is_some()
            && changes.row_of(ChangeList::Staged, &a).is_none(),
        "the refused stage ran after the removal: {:?}",
        changes.status()
    );
    // And the stage now runs.
    let id = ask(&handle, stage(&["a"]));
    let seen = until_ended(&mut updates, id);
    assert!(
        matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    drop(handle);
}

/// The stopping rule's negative arms (phase 19's QA items 1 and 2): a write git refused for the
/// lock is run again only where the same lock stood from before the attempt until after it.
/// (a) A lock made during the attempt — none before — and (b) a lock swapped during it for
/// another file of the same time but another size each end the write failed after ONE `git`,
/// since another git may have written the index between. Caught by: the condition dropped, or a
/// lock told the same by its time alone.
#[test]
fn a_lock_made_or_swapped_during_the_attempt_is_never_run_through_again() {
    for swapped in [false, true] {
        let fixture = with_files(&format!("cairn-lane-lock-swapped-{swapped}"), &["a"]);
        let lock = fixture.path.join(".git/index.lock");
        if swapped {
            std::fs::write(&lock, "").unwrap_or_else(|error| panic!("{error}"));
        }
        let body = format!(
            "  L='{lock}'\n  echo $$ >> \"$DIR/adds\"\n  \
             if [ -e \"$L\" ]; then /usr/bin/touch -r \"$L\" \"$DIR/stamp\"; /bin/rm -f \"$L\"; \
             printf 'another' > \"$L\"; /usr/bin/touch -r \"$DIR/stamp\" \"$L\"; \
             else : > \"$L\"; fi\n  \
             echo \"fatal: Unable to create '$L': File exists.\" >&2\n  exit 128",
            lock = lock.display()
        );
        let stub = stub(&[("add", &body)]);
        let (home, runtime) = (Home::new(), RuntimeDir::new());
        let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
        let before = std::fs::metadata(&lock).ok().map(|m| m.modified().ok());
        let id = ask(&handle, stage(&["a"]));
        let seen = until_ended(&mut updates, id);
        assert!(
            matches!(ending_of(&seen, id).0, WriteEnding::Failed(_)),
            "swapped={swapped}: {seen:?}"
        );
        if swapped {
            // The swap took: another file of another size, at the old lock's time.
            let now = std::fs::metadata(&lock).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(now.len(), "another".len() as u64);
            assert_eq!(Some(now.modified().ok()), before, "the swap kept no time");
        }
        let adds = std::fs::read_to_string(stub.directory.join("adds"))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            adds.lines().count(),
            1,
            "swapped={swapped}: run again over a lock that did not stand throughout"
        );
        drop(handle);
    }
}

/// F5 and the phase's stopping rule: a write git refused for a lock another program held, and let
/// go of within the second, runs again and writes once — its first attempt wrote nothing, since
/// the lock stood throughout it. Caught by: a write reported failed though the lock went, or
/// one run past its success.
#[test]
fn a_write_refused_for_a_lock_let_go_of_runs_again_and_writes_once() {
    let fixture = with_files("cairn-lane-lock-retried", &["a"]);
    let lock = fixture.path.join(".git/index.lock");
    std::fs::write(&lock, "").unwrap_or_else(|error| panic!("{error}"));
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    let releasing = lock.clone();
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(LOCK_RETRY_AFTER / 2);
        std::fs::remove_file(&releasing).unwrap_or_else(|error| panic!("{error}"));
    });
    let id = ask(&handle, stage(&["a"]));
    let seen = until_ended(&mut updates, id);
    releaser
        .join()
        .unwrap_or_else(|_| panic!("the releaser panicked"));
    assert!(
        matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    let commands = seen
        .iter()
        .find_map(|update| match update {
            Update::OperationRan {
                by: RanBy::Write(ran),
                commands,
            } if *ran == id => Some(commands.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no record: {seen:?}"));
    let exits: Vec<_> = commands.iter().map(|command| command.exit).collect();
    assert_eq!(
        exits,
        [CommandExit::Code(128), CommandExit::Code(0)],
        "{commands:?}"
    );
    drop(handle);
}

/// Answers the next prompt among the updates with `secret`, returning what came before it,
/// the prompt last. It must be titled `asking`.
fn answer_next_prompt(
    updates: &mut Updates,
    reply: &Replier,
    secret: &str,
    asking: &str,
) -> Vec<Update> {
    let seen = collect_until(updates, |update| matches!(update, Update::Prompt { .. }));
    let Some(Update::Prompt {
        id,
        text,
        asking: titled,
    }) = seen.last()
    else {
        unreachable!("collected until a prompt");
    };
    assert!(text.contains("passphrase"), "{text}");
    assert_eq!(
        titled.as_deref(),
        Some(asking),
        "the prompt is not titled by its own operation"
    );
    reply(Reply::Provide {
        prompt: *id,
        secret: Secret::from_string(secret.to_owned()),
    });
    seen
}

/// A hook or a program that asks through the askpass helper, as ssh asks for a signing key's
/// passphrase, writing what it was told to `<directory>/heard` (expanded by the shell, so
/// `$DIR` names a stub's directory).
fn asks(directory: &str) -> String {
    format!(
        "answer=$(\"$SSH_ASKPASS\" \"Enter passphrase for key '/home/u/.ssh/id_ed25519': \")\n\
         printf '%s' \"$answer\" > \"{directory}/heard\"\n"
    )
}

/// C12 and R5.1: a local write whose child asks for a secret — a real `git add` whose
/// `post-index-change` hook asks through the helper, as a signing program does — raises the
/// prompt in the window while it runs, and the answer reaches the child. Caught by: a local
/// write run without an askpass token (the helper refuses it), or a prompt that never leaves
/// the worker.
#[test]
fn a_prompt_a_stages_hook_raises_is_shown_and_answered() {
    let fixture = with_files("cairn-lane-hook-asks", &["a"]);
    let heard = fixture.path.join("heard");
    let hook = fixture.path.join(".git/hooks/post-index-change");
    std::fs::create_dir_all(fixture.path.join(".git/hooks")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        &hook,
        format!("#!/bin/sh\n{}", asks(&fixture.path.display().to_string())),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("{error}"));
    }
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, reply) = real_boundary(&fixture.path, (&home, &runtime));

    let id = ask(&handle, stage(&["a"]));
    let seen = answer_next_prompt(&mut updates, &reply, "correct horse", "Stage 1 File");
    assert!(
        lane_news(&seen).contains(&Lane::Started(id)),
        "the prompt came before the write started: {seen:?}"
    );
    let seen = until_ended(&mut updates, id);
    assert!(
        matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&heard).unwrap_or_default(),
        "correct horse",
        "the hook was not told the answer"
    );
    drop(handle);
}

/// A stand-in `ssh-keygen` for `gpg.ssh.program`: for `-Y sign`, it asks for the key's
/// passphrase through `SSH_ASKPASS` as ssh-keygen does with an encrypted key and no agent,
/// writes what it heard to `<directory>/heard`, and writes a signature beside the buffer git
/// gave it — the last argument — as `<buffer>.sig`; any other use prints a fingerprint.
fn signing_program(directory: &Path) -> PathBuf {
    let program = directory.join("ssh-keygen");
    let script = format!(
        "#!/bin/sh\n\
         for last in \"$@\"; do :; done\n\
         case \"$*\" in\n\
         *-Y\\ sign*)\n\
           {asks}\
           printf -- '-----BEGIN SSH SIGNATURE-----\\nsigned after asking\\n-----END SSH SIGNATURE-----\\n' \
             > \"$last.sig\"\n\
           ;;\n\
         *)\n\
           echo '256 SHA256:stand-in key (ED25519)'\n\
           ;;\n\
         esac\n",
        asks = asks(&directory.display().to_string())
    );
    std::fs::write(&program, script).unwrap_or_else(|error| panic!("{error}"));
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("{error}"));
    program
}

/// C12's commit half and phase 04's QA item 13, against a real signed `git commit` (git
/// 2.34's `gpg.format=ssh`): the signing program it runs asks for the key's passphrase
/// through the helper while a fetch is also in flight, the prompt reaches the window titled by
/// the commit — not the fetch — and the answer reaches the program, whose signature is the
/// commit's. Caught by: a commit run without an askpass token (the helper refuses it), a
/// prompt titled by whatever else is in flight, or an answer that never arrives.
#[test]
fn a_signed_commits_prompt_is_titled_by_the_commit_while_a_fetch_runs() {
    let fixture = to_commit("cairn-lane-commit-asks", &["a"]);
    let stub = stub(&[("fetch", &held("fetches"))]);
    let _released = ReleaseAll(&stub);
    let program = signing_program(&stub.directory);
    let key = stub.directory.join("id_ed25519");
    std::fs::write(&key, "a stand-in key\n").unwrap_or_else(|error| panic!("{error}"));
    let config = fixture.path.join(".git/config");
    let mut text = std::fs::read_to_string(&config).unwrap_or_else(|error| panic!("{error}"));
    text.push_str(&format!(
        "[commit]\n\tgpgSign = true\n[gpg]\n\tformat = ssh\n[gpg \"ssh\"]\n\tprogram = {}\n\
         [user]\n\tsigningKey = {}\n",
        program.display(),
        key.display()
    ));
    std::fs::write(&config, text).unwrap_or_else(|error| panic!("{error}"));
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    until_pids(&stub, "fetches", 1);
    let id = ask(&handle, commit());
    answer_next_prompt(&mut updates, &reply, "battery staple", "Commit 1 file");
    let seen = until_ended(&mut updates, id);
    assert!(
        matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    assert_eq!(
        std::fs::read_to_string(stub.directory.join("heard")).unwrap_or_default(),
        "battery staple"
    );
    // The commit object carries the signature the program wrote once it was answered.
    let head = std::fs::read_to_string(fixture.path.join(".git/refs/heads/main"))
        .unwrap_or_else(|error| panic!("{error}"));
    let repo = Repository::discover(&fixture.path).unwrap_or_else(|error| panic!("{error}"));
    let details = repo
        .commit_details(&cairn_model::Oid::parse(head.trim()).unwrap_or_else(|e| panic!("{e}")))
        .unwrap_or_else(|error| panic!("{error}"));
    // git writes the commit only once the program has signed it, so a commit made is signed.
    assert_eq!(details.message, "the subject\n\nthe body\n");
    drop(handle);
}

/// R6.2, R1.6 and phase 04's QA item 7, through the lane: an amend held in its hook keeps a
/// refresh back like a commit, runs under the confirmation of the `Consequence` the engine
/// computed, and ends quoting the prompt it confirmed and reading everything again. Caught
/// by: an amend run as a plain commit, its prompt lost, or a refresh read while it ran.
#[test]
fn an_amend_through_the_lane_quotes_its_prompt_and_keeps_refreshes_back() {
    let fixture = to_commit("cairn-lane-amend", &["a", "b"]);
    let stub = stub(&[]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    let first = ask(&handle, commit());
    until_ended(&mut updates, first);
    staged(&handle, &mut updates, &["b"]);
    held_hook(&fixture.path, &stub.directory);

    let repo = Repository::discover(&fixture.path).unwrap_or_else(|error| panic!("{error}"));
    let consequence =
        cairn_git::ops::amend_consequence(&reading_git(), &repo, &CancelSignal::new())
            .unwrap_or_else(|error| panic!("{error}"));
    let prompt = consequence.prompt();
    let amend = ask(
        &handle,
        LocalWrite::Amend {
            confirmed: Confirmed::by_user(consequence),
            message: "amended".to_owned(),
            skip_hooks: false,
        },
    );
    let pid = until_pids(&stub, "commits", 1)[0];
    handle.submit(Request::Refresh);
    let meanwhile = after_the_repository_thread(&handle, &mut updates);
    assert!(
        !meanwhile
            .iter()
            .any(|update| matches!(update, Update::Refs { .. })),
        "a refresh was read while the amend ran: {meanwhile:?}"
    );
    release(&stub, pid);
    let seen = until_ended(&mut updates, amend);
    match ending_of(&seen, amend) {
        (WriteEnding::Done(done), ReadAgain::Everything) => {
            assert_eq!(done.acknowledged.as_deref(), Some(prompt.as_str()));
        }
        other => panic!("{other:?}"),
    }
    let head = std::fs::read_to_string(fixture.path.join(".git/refs/heads/main"))
        .unwrap_or_else(|error| panic!("{error}"));
    let details = repo
        .commit_details(&cairn_model::Oid::parse(head.trim()).unwrap_or_else(|e| panic!("{e}")))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(details.message, "amended\n");
    assert!(details.parents.is_empty(), "the amend made a second commit");
    drop(handle);
}

/// The `git` this process finds, for a read a test makes beside the lane.
fn reading_git() -> GitBinary {
    GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
        .unwrap_or_else(|error| panic!("{error}"))
}

/// C14's worker half and the QA brief (R6.4, rules 2 and 3 of the redesign): Amend pressed is one
/// job on the lane that reads what the amend costs and, in the same job, runs it — git logs it
/// and no remote has `HEAD`: amended, with no token and no prompt — or ends without running git,
/// answering the `Consequence` for the dialog — a remote-tracking ref holds `HEAD`: nothing
/// amended — and the dialog's token then runs it. Caught by: a recoverable amend sent to the
/// dialog, a published one run unconfirmed, or the answer carrying another `HEAD`.
#[test]
fn amend_pressed_runs_a_recoverable_amend_and_answers_any_other_for_the_dialog() {
    let fixture = to_commit("cairn-lane-amend-press", &["a", "b", "c"]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    let first = ask(&handle, commit());
    until_ended(&mut updates, first);
    let committed = head_commit(&fixture.path);
    staged(&handle, &mut updates, &["b"]);
    let press = |message: &str| LocalWrite::AmendAtPress {
        message: message.to_owned(),
        skip_hooks: false,
    };

    let recoverable = ask(&handle, press("amended at once"));
    let seen = until_ended(&mut updates, recoverable);
    match ending_of(&seen, recoverable) {
        (WriteEnding::Done(done), ReadAgain::Everything) => {
            assert_eq!(done.acknowledged, None, "a prompt recorded with none shown");
            // Phase 18, step 6: the way back is the commit the run answered it replaced; and
            // C35, read again once git was reaped, it still asks nothing.
            assert_eq!(done.replaced, Some(committed));
            assert!(
                done.cost_after
                    .as_ref()
                    .is_some_and(|after| !after.needs_confirming()),
                "{:?}",
                done.cost_after
            );
        }
        other => panic!("the recoverable amend ended {other:?}"),
    }
    let amended = head_commit(&fixture.path);
    assert_ne!(amended, committed);

    // A remote-tracking ref at `HEAD`: a remote has it.
    std::fs::create_dir_all(fixture.path.join(".git/refs/remotes/origin"))
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        fixture.path.join(".git/refs/remotes/origin/main"),
        format!("{amended}\n"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    staged(&handle, &mut updates, &["c"]);
    let published = ask(&handle, press("amended after asking"));
    let seen = until_ended(&mut updates, published);
    let consequence = match ending_of(&seen, published) {
        (WriteEnding::NeedsConfirming { consequence }, _) => consequence,
        other => panic!("the published amend ended {other:?}"),
    };
    assert_eq!(consequence.amended(), Some(amended));
    assert!(consequence.needs_confirming());
    assert_eq!(
        head_commit(&fixture.path),
        amended,
        "git ran before the dialog"
    );

    let prompt = consequence.prompt();
    let confirmed = ask(
        &handle,
        LocalWrite::Amend {
            confirmed: Confirmed::by_user(consequence),
            message: "amended after asking".to_owned(),
            skip_hooks: false,
        },
    );
    let seen = until_ended(&mut updates, confirmed);
    match ending_of(&seen, confirmed) {
        (WriteEnding::Done(done), _) => {
            assert_eq!(done.acknowledged, Some(prompt));
            assert_eq!(done.replaced, Some(amended));
            assert_eq!(
                done.cost_after, None,
                "a confirmed amend reads nothing after"
            );
        }
        other => panic!("the confirmed amend ended {other:?}"),
    }
    assert_ne!(head_commit(&fixture.path), amended);
    drop(handle);
}

/// R1.1's option (a), the user's answer of 2026-10-10, through the lane: a confirmed amend a
/// `pre-commit` hook fails, `HEAD` unmoved, ends git's failure and carries its token back unspent
/// on the ending, for the window's Git Error state; the skip run with that token amends, and its
/// ending carries none. Caught by: the token dropped by the lane (the skip asking again), or
/// handed back on an ending that spent it.
#[test]
fn a_confirmed_amend_a_hook_fails_carries_its_token_back_for_the_skip() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = to_commit("cairn-lane-amend-unspent", &["a", "b"]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    let first = ask(&handle, commit());
    until_ended(&mut updates, first);
    let committed = head_commit(&fixture.path);
    std::fs::create_dir_all(fixture.path.join(".git/refs/remotes/origin"))
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        fixture.path.join(".git/refs/remotes/origin/main"),
        format!("{committed}\n"),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    staged(&handle, &mut updates, &["b"]);
    let pressed = ask(
        &handle,
        LocalWrite::AmendAtPress {
            message: "amended".to_owned(),
            skip_hooks: false,
        },
    );
    let seen = until_ended(&mut updates, pressed);
    let consequence = match ending_of(&seen, pressed) {
        (WriteEnding::NeedsConfirming { consequence }, _) => consequence,
        other => panic!("the published amend ended {other:?}"),
    };
    let hook = fixture.path.join(".git/hooks/pre-commit");
    std::fs::create_dir_all(fixture.path.join(".git/hooks")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&hook, "#!/bin/sh\necho refused >&2\nexit 1\n")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|e| panic!("{e}"));
    let amend = |confirmed, skip_hooks| LocalWrite::Amend {
        confirmed,
        message: "amended".to_owned(),
        skip_hooks,
    };
    let failed = ask(&handle, amend(Confirmed::by_user(consequence), false));
    let seen = until_ended(&mut updates, failed);
    let mut unspent = seen
        .into_iter()
        .find_map(|update| match update {
            Update::WriteEnded {
                id,
                ending,
                unspent,
                ..
            } if id == failed => {
                assert!(
                    matches!(
                        ending,
                        WriteEnding::Failed(crate::worker::Failure {
                            command: Some(_),
                            ..
                        })
                    ),
                    "{ending:?}"
                );
                Some(unspent)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{failed:?} did not end"));
    let token = unspent
        .take()
        .unwrap_or_else(|| panic!("the token was not handed back"));
    assert_eq!(head_commit(&fixture.path), committed);
    let skipped = ask(&handle, amend(token, true));
    let seen = until_ended(&mut updates, skipped);
    let mut spent = seen
        .into_iter()
        .find_map(|update| match update {
            Update::WriteEnded {
                id,
                ending,
                unspent,
                ..
            } if id == skipped => {
                assert!(matches!(ending, WriteEnding::Done(_)), "{ending:?}");
                Some(unspent)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{skipped:?} did not end"));
    assert!(spent.take().is_none(), "a spent token came back");
    assert_ne!(head_commit(&fixture.path), committed);
    drop(handle);
}

/// R1, R4.1: each destructive write runs through the lane with the confirmation the user gave
/// it — a discard of lines, then a discard of files — and its ending quotes the prompt they
/// accepted. Caught by: a lane that drops a confirmation or does not spend it.
#[test]
fn confirmed_discards_run_through_the_lane_and_quote_their_prompts() {
    let fixture = with_files("cairn-lane-discard", &["a", "b"]);
    let git = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
        .unwrap_or_else(|error| panic!("finding git: {error}"));
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    let staged = ask(&handle, stage(&["a"]));
    until_ended(&mut updates, staged);
    let edited = fixture.path.join("a");
    std::fs::write(&edited, "a\nline two\nline three\n").unwrap_or_else(|error| panic!("{error}"));

    let repo = Repository::discover(&fixture.path).unwrap_or_else(|error| panic!("{error}"));
    let diff = repo
        .working_tree_diff(
            &git,
            &RepoPath::from("a"),
            WorkingTreeDiff::Unstaged,
            &ContentOptions::default(),
            &CancelSignal::new(),
        )
        .unwrap_or_else(|error| panic!("{error}"))
        .unwrap_or_else(|| panic!("a has no unstaged diff"));
    let selection = Selection::with_every_change(diff.text().unwrap_or_else(|| panic!("no text")));
    let lines = cairn_git::ops::discard_lines_consequence(&git, &repo, &diff, selection, false)
        .unwrap_or_else(|error| panic!("{error}"));
    let files = cairn_git::ops::discard_files_consequence(
        &git,
        &repo,
        &[RepoPath::from("b")],
        &cairn_git::CancelSignal::new(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let prompts = [lines.prompt(), files.prompt()];

    let ids = [
        ask(&handle, LocalWrite::DiscardLines(Confirmed::by_user(lines))),
        ask(&handle, LocalWrite::DiscardFiles(Confirmed::by_user(files))),
    ];
    let seen = until_ended(&mut updates, ids[1]);
    for (id, prompt) in ids.into_iter().zip(prompts) {
        match ending_of(&seen, id) {
            (WriteEnding::Done(done), ReadAgain::Status) => {
                assert_eq!(done.acknowledged.as_deref(), Some(prompt.as_str()));
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        std::fs::read_to_string(&edited).unwrap_or_default(),
        "a\nline two\n",
        "the lines were not discarded"
    );
    assert!(!fixture.path.join("b").exists(), "the file was not deleted");
    drop(handle);
}

/// What the lane answered for the discard asked under `asked`.
fn consequence_of(seen: &[Update], asked: OperationId) -> Result<cairn_model::Consequence, String> {
    seen.iter()
        .find_map(|update| match update {
            Update::DiscardConsequence { asked: id, outcome } if *id == asked => {
                Some(outcome.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no consequence for {asked:?}: {seen:?}"))
}

/// R8.4 and phase 04's carry: what a discard would lose is counted on the local lane in the
/// order asked — after a stage asked before it, held in its `git add` meanwhile — so it counts
/// what that stage left: the path staged whole has no unstaged change, and the count refuses —
/// the refusal the dialog, opened counting at the press, then shows as "Couldn't Discard
/// Changes". Caught by: the consequence computed on another thread, or ahead
/// of the write asked before it (it would count the file as untracked and offer to delete it).
#[test]
fn a_discards_consequence_is_counted_after_the_writes_asked_before_it() {
    let fixture = with_files("cairn-lane-consequence-order", &["a", "b"]);
    let body = format!(
        "{}\n",
        held("adds").replace("  exit 0", "  exec \"$REAL\" \"$@\"")
    );
    let stub = stub(&[("add", &body)]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    let staging = ask(&handle, stage(&["a"]));
    let pid = until_pids(&stub, "adds", 1)[0];
    let asked = OperationId::next();
    handle.submit(Request::DiscardConsequence {
        asked,
        paths: vec![RepoPath::from("a"), RepoPath::from("b")],
    });
    let meanwhile = after_the_repository_thread(&handle, &mut updates);
    assert!(
        !meanwhile
            .iter()
            .any(|update| matches!(update, Update::DiscardConsequence { .. })),
        "the consequence was counted while the stage ahead of it ran: {meanwhile:?}"
    );
    release(&stub, pid);
    let seen = collect_until(&mut updates, |update| {
        matches!(update, Update::DiscardConsequence { .. })
    });
    assert!(
        seen.iter()
            .any(|update| matches!(update, Update::WriteEnded { id, .. } if *id == staging)),
        "the consequence came before the stage ended: {seen:?}"
    );
    match consequence_of(&seen, asked) {
        Err(why) => assert!(why.contains('a'), "{why}"),
        Ok(consequence) => panic!("a staged path was offered for discard: {consequence:?}"),
    }
    drop(handle);
}

/// The QA brief, R8.4, R1.2: a selection mixing a modified file and an untracked one — what the
/// window asks — is counted by the lane, and the discard confirmed from exactly that count
/// restores the one and deletes the other, its ending quoting the prompt the dialog drew.
/// Caught by: a count that is not what the discard then does, or a discard that takes a file
/// the prompt did not name.
#[test]
fn the_dialogs_count_is_what_the_discard_then_does_to_a_mixed_selection() {
    let fixture = to_commit("cairn-lane-mixed-discard", &["kept", "modified"]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    staged(&handle, &mut updates, &["kept", "modified"]);
    let committing = ask(&handle, commit());
    let seen = until_ended(&mut updates, committing);
    assert!(
        matches!(ending_of(&seen, committing).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    std::fs::write(
        fixture.path.join("modified"),
        "modified\nedited\nline three\n",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(fixture.path.join("new.txt"), "twelve bytes")
        .unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(fixture.path.join("beside.txt"), "not selected\n")
        .unwrap_or_else(|error| panic!("{error}"));

    let asked = OperationId::next();
    handle.submit(Request::DiscardConsequence {
        asked,
        paths: vec![RepoPath::from("modified"), RepoPath::from("new.txt")],
    });
    let seen = collect_until(&mut updates, |update| {
        matches!(update, Update::DiscardConsequence { .. })
    });
    let consequence = consequence_of(&seen, asked).unwrap_or_else(|why| panic!("{why}"));
    let prompt = consequence.prompt();
    assert_eq!(
        prompt,
        "Discard all changes in 2 files? 1 untracked file will be deleted. You can't undo this."
    );
    assert_eq!(consequence.action(), "Discard Changes in 2 Files");
    let discarding = ask(
        &handle,
        LocalWrite::DiscardFiles(Confirmed::by_user(consequence)),
    );
    let seen = until_ended(&mut updates, discarding);
    match ending_of(&seen, discarding) {
        (WriteEnding::Done(done), ReadAgain::Status) => {
            assert_eq!(done.acknowledged.as_deref(), Some(prompt.as_str()));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(fixture.path.join("modified")).unwrap_or_default(),
        "modified\nline two\n"
    );
    assert!(
        !fixture.path.join("new.txt").exists(),
        "the untracked file is still there"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.path.join("beside.txt")).unwrap_or_default(),
        "not selected\n",
        "a file the prompt did not name was taken"
    );
    assert!(fixture.path.join("kept").exists());
    drop(handle);
}

/// Phase 07's QA item 1: a count of what a discard would lose holds the local lane while its
/// `git diff-files` reads run, so it is numbered in a lane of its own — a newer ask ends it,
/// its held read's process ended with it, and answers nothing, the newer one answered; a
/// `StopCounting` ends one with nothing after it; and the writes behind each still run. Caught
/// by: a count that runs to its end whatever was asked after it (it held the lane and the close
/// for as long as the selection was wide).
#[test]
fn a_newer_ask_or_a_stop_ends_a_discards_count_and_its_read() {
    let fixture = to_commit("cairn-lane-count-cancelled", &["kept", "modified"]);
    let body = format!(
        "{}\n",
        held("diffs").replace("  exit 0", "  exec \"$REAL\" \"$@\"")
    );
    // Every read here past the commit is a working-tree diff, the only verb led by `-c`.
    let stub = stub(&[("-c", &body)]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    staged(&handle, &mut updates, &["kept", "modified"]);
    let committing = ask(&handle, commit());
    until_ended(&mut updates, committing);
    std::fs::write(fixture.path.join("modified"), "modified\nedited\n")
        .unwrap_or_else(|error| panic!("{error}"));

    let count = |handle: &RepositoryHandle| {
        let asked = OperationId::next();
        handle.submit(Request::DiscardConsequence {
            asked,
            paths: vec![RepoPath::from("modified")],
        });
        asked
    };
    let first = count(&handle);
    let held_first = until_pids(&stub, "diffs", 1)[0];
    let second = count(&handle);
    // The second is counted once the first has stopped: release only the second's read.
    let held_second = until_pids(&stub, "diffs", 2)[1];
    assert_ne!(held_first, held_second);
    release(&stub, held_second);
    let seen = collect_until(&mut updates, |update| {
        matches!(update, Update::DiscardConsequence { .. })
    });
    assert!(
        matches!(
            seen.last(),
            Some(Update::DiscardConsequence { asked, outcome: Ok(_) }) if *asked == second
        ),
        "the newer count was not the one answered: {seen:?}"
    );
    // The superseded count's held read was ended, never released.
    let deadline = Instant::now() + WAIT;
    while std::path::Path::new(&format!("/proc/{held_first}")).exists() {
        assert!(
            Instant::now() < deadline,
            "the superseded count's read was left running"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!pids(&stub, "finished").contains(&held_first));

    // A stop ends the count in flight, which answers nothing; a write asked after it runs.
    let third = count(&handle);
    until_pids(&stub, "diffs", 3);
    handle.submit(Request::StopCounting);
    let staging = ask(&handle, stage(&["modified"]));
    let _ = std::fs::write(stub.directory.join("release"), "");
    let seen = until_ended(&mut updates, staging);
    assert!(
        !seen.iter().any(|update| matches!(
            update,
            Update::DiscardConsequence { asked, .. } if *asked == first || *asked == third
        )),
        "a count ended before its end was answered: {seen:?}"
    );
    drop(handle);
}

/// A status read through the lane, as the window keeps it.
fn status_now(handle: &RepositoryHandle, updates: &mut Updates) -> Arc<cairn_model::LocalChanges> {
    handle.submit(Request::RefreshStatus);
    let seen = collect_until(updates, |update| matches!(update, Update::Status { .. }));
    match seen.into_iter().last() {
        Some(Update::Status { changes }) => changes,
        other => panic!("{other:?}"),
    }
}

/// Staging-and-commit R6.7, R6.3 and R10 through the real boundary: the commit box's reads
/// answer no operation, no message prepared, and the branch's messages newest first, the last
/// commit's among them; and amend's read answers `HEAD`'s message and amend's lists over the
/// status asked with: the file `HEAD` added and the one staged since in Staged, the first
/// commit's file in neither — and, C14 and rule 3, reads nothing of what the amend costs: no
/// `git config` read of `core.logAllRefUpdates` runs for it. Caught by: a read run on the UI
/// thread's terms (an answer missing), lists against `HEAD`, or the cost read on a status.
#[test]
fn the_commit_boxs_reads_and_an_amends_read_come_through_the_lane() {
    let fixture = to_commit("cairn-lane-box-reads", &["a", "b", "c"]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    for (path, message) in [("a", "first\n"), ("b", "second\n\nwith a body\n")] {
        staged(&handle, &mut updates, &[path]);
        let id = ask(
            &handle,
            LocalWrite::Commit {
                message: message.to_owned(),
                skip_hooks: false,
                files: 1,
            },
        );
        let seen = until_ended(&mut updates, id);
        assert!(
            matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
            "{seen:?}"
        );
    }
    handle.submit(Request::CommitReads);
    let seen = collect_until(&mut updates, |u| matches!(u, Update::CommitReads(_)));
    let Some(Update::CommitReads(reads)) = seen.into_iter().last() else {
        panic!("no reads");
    };
    assert_eq!(reads.operation, None);
    assert_eq!(reads.message, Ok(None));
    assert_eq!(
        reads.recent.as_deref(),
        Ok(["second\n\nwith a body\n".to_owned(), "first\n".to_owned()].as_slice())
    );

    staged(&handle, &mut updates, &["c"]);
    let status = status_now(&handle, &mut updates);
    let logged = |updates: &mut Updates| {
        handle.submit(Request::CommandLog);
        let seen = collect_until(updates, |u| matches!(u, Update::CommandLog { .. }));
        match seen.into_iter().last() {
            Some(Update::CommandLog { records }) => records,
            other => panic!("no command log: {other:?}"),
        }
    };
    let before = logged(&mut updates).len();
    handle.submit(Request::Amending { status });
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Amending { .. }));
    let Some(Update::Amending { read, .. }) = seen.into_iter().last() else {
        panic!("no amend read");
    };
    let ran: Vec<Vec<String>> = logged(&mut updates)
        .into_iter()
        .skip(before)
        .map(|record| record.arguments)
        .collect();
    assert!(!ran.is_empty(), "amend's staged list ran no git");
    assert!(
        ran.iter()
            .all(|arguments| !arguments.iter().any(|argument| argument == "config")),
        "amend's read asked what the amend costs: {ran:?}"
    );
    assert_eq!(read.message.as_deref(), Ok("second\n\nwith a body\n"));
    let lists = read.lists.unwrap_or_else(|error| panic!("{error}"));
    let staged: Vec<String> = (0..lists.len(cairn_model::ChangeList::Staged))
        .filter_map(|row| lists.get(cairn_model::ChangeList::Staged, row))
        .map(|change| change.path.to_string())
        .collect();
    assert_eq!(staged, ["b", "c"]);
    assert!(matches!(
        lists.staged_against(),
        cairn_model::StagedAgainst::HeadParent(Some(_))
    ));
    drop(handle);
}

/// Phase 08's QA item 7, end to end: a chunk drawn at a context of ten lines — two edits
/// thirteen lines apart, which three lines of context keep apart — staged through the lane is
/// exactly that chunk, as `git diff --cached` reads it back: both edits staged, nothing else.
/// Caught by: a patch emitted at another context than the one drawn, which would stage one
/// edit or misplace the other.
#[test]
fn a_chunk_drawn_at_context_ten_stages_exactly_as_drawn() {
    use crate::worker::request::{DiffOptions, FileQuery, FileTarget, WorkingSide};
    let fixture = to_commit("cairn-lane-context-ten", &[]);
    let lines: Vec<String> = (0..60).map(|n| format!("line {n}\n")).collect();
    std::fs::write(fixture.path.join("f"), lines.concat()).unwrap_or_else(|e| panic!("{e}"));
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    staged(&handle, &mut updates, &["f"]);
    let id = ask(&handle, commit());
    let seen = until_ended(&mut updates, id);
    assert!(
        matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    let mut edited = lines.clone();
    edited[5] = "EDIT five\n".to_owned();
    edited[18] = "EDIT eighteen\n".to_owned();
    edited[50] = "EDIT fifty\n".to_owned();
    std::fs::write(fixture.path.join("f"), edited.concat()).unwrap_or_else(|e| panic!("{e}"));

    let query = FileQuery {
        target: FileTarget::WorkingTree {
            path: RepoPath::from("f"),
            side: WorkingSide::Unstaged,
        },
        options: DiffOptions {
            context: cairn_model::Context::lines(10),
            ..DiffOptions::default()
        },
    };
    handle.submit(Request::FileDiff(query));
    let seen = collect_until(&mut updates, |u| matches!(u, Update::FileDiff { .. }));
    let Some(Update::FileDiff {
        diff: Some(shown), ..
    }) = seen.into_iter().last()
    else {
        panic!("no diff of f");
    };
    let layout = shown.layout().unwrap_or_else(|| panic!("f drew no rows"));
    let (text, _) = shown.text().unwrap_or_else(|| panic!("f is not text"));
    // At ten lines of context the first two edits are one chunk, the third its own.
    assert_eq!(layout.hunks().len(), 2, "{:?}", layout.hunks());
    let selection = layout
        .hunk_selection(text, 0)
        .unwrap_or_else(|| panic!("the first chunk selects nothing"));
    let id = ask(
        &handle,
        LocalWrite::StageLines {
            diff: shown.shared_diff(),
            selection,
        },
    );
    let seen = until_ended(&mut updates, id);
    assert!(
        matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
        "{seen:?}"
    );

    let git = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
        .unwrap_or_else(|error| panic!("finding git: {error}"));
    let repo = Repository::discover(&fixture.path).unwrap_or_else(|error| panic!("{error}"));
    let cached = repo
        .working_tree_diff(
            &git,
            &RepoPath::from("f"),
            WorkingTreeDiff::Staged,
            &ContentOptions::default(),
            &CancelSignal::new(),
        )
        .unwrap_or_else(|error| panic!("{error}"))
        .unwrap_or_else(|| panic!("nothing was staged"));
    let index: Vec<u8> = cached
        .text()
        .map(|text| text.new_content())
        .unwrap_or_else(|| panic!("the staged diff is not text"));
    let mut expected = lines.clone();
    expected[5] = edited[5].clone();
    expected[18] = edited[18].clone();
    assert_eq!(
        String::from_utf8_lossy(&index),
        expected.concat(),
        "the index is not the file with exactly the chunk drawn staged"
    );
    drop(handle);
}

/// `HEAD`'s commit in `repository`, as the engine reads its refs.
fn head_commit(repository: &Path) -> cairn_model::Oid {
    let repo = Repository::discover(repository).unwrap_or_else(|error| panic!("{error}"));
    let read = repo
        .refs(&CancelSignal::new())
        .unwrap_or_else(|error| panic!("reading the refs: {error}"));
    match &read.snapshot.head {
        cairn_model::HeadState::Branch(name) => read
            .snapshot
            .find(name)
            .and_then(cairn_model::Ref::commit_id)
            .unwrap_or_else(|| panic!("{name:?} names no commit")),
        other => panic!("HEAD is {other:?}"),
    }
}

/// staging-and-commit R11.3 through the lane: `Create Branch Here…` runs as a local write,
/// in order, its ending saying to read everything again — the refs moved, and the history
/// with them — and a name git refuses ends failed with git's command and its reason, nothing
/// written. Caught by: a branch write that reads only the status after it (the history keeps
/// drawing the commit as lost), or a refusal that loses git's words.
#[test]
fn a_branch_made_through_the_lane_reads_everything_again_and_a_refusal_says_gits_reason() {
    let fixture = to_commit("cairn-lane-branch", &["a"]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    let committed = ask(&handle, commit());
    let seen = until_ended(&mut updates, committed);
    assert!(
        matches!(ending_of(&seen, committed).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    let at = head_commit(&fixture.path);

    let made = ask(
        &handle,
        LocalWrite::CreateBranch {
            name: "recovered".to_owned(),
            at,
        },
    );
    let seen = until_ended(&mut updates, made);
    match ending_of(&seen, made) {
        (WriteEnding::Done(done), ReadAgain::Everything) => {
            assert!(done.description.contains("recovered"), "{done:?}");
        }
        other => panic!("the branch write ended {other:?}"),
    }
    let repo = Repository::discover(&fixture.path).unwrap_or_else(|error| panic!("{error}"));
    let read = repo
        .refs(&CancelSignal::new())
        .unwrap_or_else(|error| panic!("reading the refs: {error}"));
    let recovered = read
        .snapshot
        .find(&cairn_model::RefName::new("refs/heads/recovered"))
        .and_then(cairn_model::Ref::commit_id);
    assert_eq!(recovered, Some(at), "the branch is on the commit asked");

    let taken = ask(
        &handle,
        LocalWrite::CreateBranch {
            name: "recovered".to_owned(),
            at,
        },
    );
    let seen = until_ended(&mut updates, taken);
    match ending_of(&seen, taken) {
        (
            WriteEnding::Failed(crate::worker::Failure {
                command, output, ..
            }),
            ReadAgain::Everything,
        ) => {
            assert_eq!(
                command.as_deref(),
                Some(format!("git --literal-pathspecs branch -- recovered {at}").as_str())
            );
            assert!(output.contains("recovered"), "git's reason: {output:?}");
        }
        other => panic!("a taken name ended {other:?}"),
    }
}

/// Create Branch's reads through the lane (staging-and-commit R11.3): a name is answered for
/// the text asked — taken, free, or refused in git's words — and what Discard would be
/// confirmed as is answered under the id asked with the engine's `Consequence`, or, during a
/// merge, the operation it would abandon. Caught by: an answer under another name or id, a
/// query routed where no `git` runs it, or the refusal lost in words.
#[test]
fn create_branchs_reads_are_answered_on_the_lane() {
    let fixture = to_commit("cairn-lane-branch-reads", &["a"]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    let committed = ask(&handle, commit());
    until_ended(&mut updates, committed);
    let at = head_commit(&fixture.path);
    for (name, expected) in [
        (
            "main",
            Some(cairn_model::BranchName::Refused(
                cairn_model::NameRefusal::Taken,
            )),
        ),
        ("topic", Some(cairn_model::BranchName::Free)),
        ("bad..name", None),
    ] {
        handle.submit(Request::CheckBranchName {
            name: name.to_owned(),
        });
        let seen = collect_until(&mut updates, |update| {
            matches!(update, Update::BranchName { .. })
        });
        match seen.last() {
            Some(Update::BranchName {
                name: answered,
                outcome: Ok(answer),
            }) => {
                assert_eq!(answered, name);
                match expected {
                    Some(expected) => assert_eq!(answer, &expected, "{name}"),
                    None => assert!(
                        matches!(
                            answer,
                            cairn_model::BranchName::Refused(
                                cairn_model::NameRefusal::Invalid { .. }
                            )
                        ),
                        "{name}: {answer:?}"
                    ),
                }
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    std::fs::write(fixture.path.join("a"), "edited\n").unwrap_or_else(|e| panic!("{e}"));
    let asked = OperationId::next();
    handle.submit(Request::CheckoutConsequence {
        asked,
        name: "rescue".to_owned(),
        at,
    });
    let seen = collect_until(&mut updates, |update| {
        matches!(update, Update::CheckoutConsequence { .. })
    });
    match seen.last() {
        Some(Update::CheckoutConsequence {
            asked: answered,
            outcome: Ok(consequence),
        }) => {
            assert_eq!(*answered, asked);
            assert!(consequence.prompt().contains("rescue"), "{consequence:?}");
        }
        other => panic!("{other:?}"),
    }
    // A merge in progress, as git records it: Discard would abandon it, so it is refused.
    std::fs::write(fixture.path.join(".git/MERGE_HEAD"), format!("{at}\n"))
        .unwrap_or_else(|e| panic!("{e}"));
    let asked = OperationId::next();
    handle.submit(Request::CheckoutConsequence {
        asked,
        name: "rescue".to_owned(),
        at,
    });
    let seen = collect_until(&mut updates, |update| {
        matches!(update, Update::CheckoutConsequence { .. })
    });
    match seen.last() {
        Some(Update::CheckoutConsequence {
            asked: answered,
            outcome: Err(CheckoutRefused::InProgress(cairn_model::OperationInProgress::Merge)),
        }) => assert_eq!(*answered, asked),
        other => panic!("{other:?}"),
    }
}

/// Phase 10's QA, item 13: a name's check superseded before its turn on the lane runs no `git`
/// — only the newest of two asked behind a held write starts `check-ref-format`, and only its
/// answer arrives. Caught by: a check that polls its cancel only once `git` is running.
#[test]
fn a_name_check_superseded_before_its_turn_runs_no_git() {
    let fixture = with_files("cairn-lane-name-checks", &["a"]);
    let body = format!(
        "{}\n",
        held("adds").replace("  exit 0", "  exec \"$REAL\" \"$@\"")
    );
    let checks = "  echo $$ >> \"$DIR/checks\"\n  exec \"$REAL\" \"$@\"".to_owned();
    let stub = stub(&[("add", &body), ("check-ref-format", &checks)]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    let staged_a = ask(&handle, stage(&["a"]));
    until_pids(&stub, "adds", 1);
    for name in ["first", "second"] {
        handle.submit(Request::CheckBranchName {
            name: name.to_owned(),
        });
    }
    let _ = std::fs::write(stub.directory.join("release"), "");
    let mut seen = until_ended(&mut updates, staged_a);
    seen.extend(collect_until(&mut updates, |update| {
        matches!(update, Update::BranchName { .. })
    }));
    let answered: Vec<&str> = seen
        .iter()
        .filter_map(|update| match update {
            Update::BranchName { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(answered, ["second"]);
    let after = after_the_repository_thread(&handle, &mut updates);
    assert!(
        !after
            .iter()
            .any(|update| matches!(update, Update::BranchName { .. })),
        "{after:?}"
    );
    assert_eq!(pids(&stub, "checks").len(), 1, "a superseded check ran git");
}

/// Phase 20's QA item 2: each write's output waits in a tap of its own. Two writes whose hooks
/// print run back to back while the window has not applied the first one's wake; the window
/// then drains each wake in turn, giving what it drains to the write the wake names — and the
/// first write's lines hold none of the second's. Caught by: one tap for the lane, whose pending
/// wake swallows the next write's lines under the first write's id.
#[test]
fn a_writes_output_never_lands_under_the_write_before_it() {
    let fixture = to_commit("cairn-lane-tap-per-write", &["a"]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_boundary(&fixture.path, (&home, &runtime));
    staged(&handle, &mut updates, &["a"]);
    hook(
        &fixture.path,
        "pre-commit",
        "n=$(cat .git/hook-runs 2>/dev/null || echo 0)\nn=$((n + 1))\necho $n > .git/hook-runs\n\
         echo \"hook run $n\" >&2\n",
    );
    let first = ask(&handle, commit());
    let second = ask(
        &handle,
        LocalWrite::AmendAtPress {
            message: "amended\n".to_owned(),
            skip_hooks: false,
        },
    );
    // Neither wake applied until both writes have ended.
    let seen = until_ended(&mut updates, second);
    for id in [first, second] {
        assert!(
            matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
            "{seen:?}"
        );
    }
    let mut drained: Vec<(OperationId, Vec<String>)> = Vec::new();
    for update in &seen {
        if let Update::WriteOutput { id, tap } = update {
            let lines = tap
                .drain()
                .iter()
                .flat_map(|read| read.lines().map(str::to_owned).collect::<Vec<_>>())
                .collect();
            drained.push((*id, lines));
        }
    }
    let of = |write: OperationId| -> Vec<String> {
        drained
            .iter()
            .filter(|(id, _)| *id == write)
            .flat_map(|(_, lines)| lines.clone())
            .collect()
    };
    assert_eq!(of(first), ["hook run 1"], "{drained:?}");
    assert_eq!(of(second), ["hook run 2"], "{drained:?}");
    drop(handle);
}
