//! The local write lane through the real boundary (staging-and-commit C10, C11, C12): writes
//! in order and each with its own ending, a status read across a write never drawn, a commit
//! that keeps refreshes back and a stage queued behind it, a cancel that reaches only the
//! commit it names, a close that waits on a commit and ends nothing, the lock files named as
//! a repository opens and by the write they fail, and a prompt a write's child raises answered
//! through the window.
//!
//! Commit is phase 05's, so a commit here is [`LocalWrite::HeldCommit`]: a long-running,
//! cancellable write the lane treats as a commit, a stub `git`'s `fetch` that is held, asks or
//! ends as each test says. Phase 05 runs the commit-dependent halves again against `git
//! commit` with a slow hook.
//!
//! No `Command` here — the guards scan this crate's tests — so fixtures are built with
//! `std::fs`, every write goes through the lane, and a stub `git` is a `/bin/sh` script.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cairn_git::ops::{Askpass, GitBinary};
use cairn_git::{CancelSignal, ContentOptions, Repository, WorkingTreeDiff};
use cairn_model::{
    Confirmed, FileDiff, RepoPath, Secret, Selection, StagedChange, StatusEntry, WorkingTreeStatus,
};

use super::Reply;
use super::discovery::Discovery;
use super::fetch_tests::{
    Home, RuntimeDir, UnbornRepository, WAIT, built_helper, collect_until, next_by, with_origin,
};
use super::lifecycle_tests::StubGit;
use super::local_lane::{LocalWrite, OperationId, ReadAgain, WriteEnding};
use super::pool::{Replier, RepositoryHandle, Updates, open};
use super::request::{Request, Update};
use super::startup::Startup;

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
            super::fetch_tests::opened_as(&mut updates);
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
    super::fetch_tests::opened_as(&mut updates);
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
    LocalWrite::HeldCommit {
        remote: "origin".to_owned(),
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

/// Every update that arrives within `quiet`.
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

/// C10 and the QA brief: five stages asked far faster than they run — each `git add` held a
/// third of a second — all run, one at a time, in the order asked, each with its own ending;
/// the stale one (a second stage of lines built from the same diff as the first, which the
/// first made stale) is dropped naming its path and the ones behind it still run; each says to
/// read status again, and status alone; and the status read after them lists what they did.
/// Caught by: writes refused or run beside each other for being second, an ending under the
/// wrong id, a stale patch that stalls the lane or is applied, or a stage that reads more than
/// status again.
#[test]
fn writes_asked_faster_than_they_run_run_in_order_each_with_its_own_ending() {
    let fixture = with_files("cairn-lane-in-order", &["a", "b", "c", "d"]);
    let stub = stub(&[("add", "  /bin/sleep 0.3\n  exec \"$REAL\" \"$@\"")]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));
    let (diff, selection) = untracked(&fixture.path, "b");

    let asked = Instant::now();
    let ids = [
        ask(&handle, stage(&["a"])),
        ask(
            &handle,
            LocalWrite::StageLines {
                diff: Box::new(diff.clone()),
                selection: selection.clone(),
            },
        ),
        ask(
            &handle,
            LocalWrite::StageLines {
                diff: Box::new(diff),
                selection,
            },
        ),
        ask(&handle, stage(&["c"])),
        ask(&handle, stage(&["d"])),
    ];
    assert!(
        asked.elapsed() < Duration::from_millis(100),
        "asking waited for the writes: {:?}",
        asked.elapsed()
    );
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
            (2, WriteEnding::Stale { path, message }) => {
                assert_eq!(path, "b");
                assert!(message.contains("nothing was written"), "{message}");
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
    // What the window asks once the write has ended.
    handle.submit(Request::RefreshStatus);
    let after = until_pids(&stub, "statuses", 2)[1];
    seen.extend(arriving_within(&mut updates, Duration::from_millis(500)));
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

/// C10 and the QA brief, against a stub's long-running write the lane treats as a commit
/// (phase 05 runs it again against `git commit`): while it runs no refresh of Cairn's own
/// starts — three asked of the handle start no status and answer nothing — and a stage asked
/// meanwhile waits for it; its ending says to read everything again, and once the window asks,
/// one refresh is answered, after the stage behind it. Caught by: a refresh read during a
/// commit, a stage run beside it, or a refresh kept back and lost.
#[test]
fn a_commit_keeps_refreshes_back_and_a_stage_asked_meanwhile_waits_for_it() {
    let fixture = with_origin(
        &format!("cairn-lane-quiet-commit-{}", std::process::id()),
        "/nonexistent/origin",
    );
    write_files(&fixture.path, &["a"]);
    let stub = stub(&[("fetch", &held("commits")), ("status", STATUS_COUNTED)]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));

    let commit = ask(&handle, commit());
    let pid = until_pids(&stub, "commits", 1)[0];
    let staged = ask(&handle, stage(&["a"]));
    handle.submit(Request::Refresh);
    handle.submit(Request::RefreshStatus);
    handle.submit(Request::Refresh);
    let meanwhile = arriving_within(&mut updates, Duration::from_millis(700));
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
        pids(&stub, "statuses"),
        [],
        "a status started while the commit ran"
    );

    release(&stub, pid);
    let seen = until_ended(&mut updates, commit);
    assert_eq!(ending_of(&seen, commit).1, ReadAgain::Everything);
    // What the window asks as the commit ends.
    handle.submit(Request::Refresh);
    let mut seen = until_ended(&mut updates, staged);
    assert_eq!(ending_of(&seen, staged).1, ReadAgain::Status);
    handle.submit(Request::RefreshStatus);
    seen.extend(collect_until(&mut updates, |update| {
        matches!(update, Update::Status { .. })
    }));
    seen.extend(arriving_within(&mut updates, Duration::from_millis(500)));
    let refs = seen
        .iter()
        .filter(|update| matches!(update, Update::Refs { .. }))
        .count();
    assert_eq!(
        refs, 1,
        "not the one refresh asked once the commit ended: {seen:?}"
    );
    // The refresh's own status may be drawn too, if it was read once the stage had ended;
    // never one read across it, and the last is the stage's.
    let drawn = statuses(&seen);
    let Some(last) = drawn.last() else {
        panic!("no status was drawn after the writes: {seen:?}");
    };
    assert_eq!(staged_and_untracked(last).0, ["a"]);
    drop(handle);
}

/// C10, R4.3 and the QA brief: a cancel names its write and reaches only that one — a cancel
/// for a commit still queued does nothing, the running commit's ends it as one that may have
/// taken effect, and that same cancel sent again once the next commit runs leaves the next
/// one to finish. Caught by: a cancel that reaches whatever runs (#47's shape), or one kept
/// for a write it was not for.
#[test]
fn a_cancel_names_its_commit_and_never_reaches_the_one_queued_behind_it() {
    let fixture = with_origin(
        &format!("cairn-lane-cancel-by-id-{}", std::process::id()),
        "/nonexistent/origin",
    );
    let stub = stub(&[("fetch", &held("commits"))]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));

    let first = ask(&handle, commit());
    let second = ask(&handle, commit());
    until_pids(&stub, "commits", 1);
    handle.submit(Request::CancelWrite { id: second });
    handle.submit(Request::CancelWrite { id: first });
    let seen = until_ended(&mut updates, first);
    match ending_of(&seen, first).0 {
        WriteEnding::MayHaveTakenEffect { message, .. } => {
            assert!(message.contains("cancelled"), "{message}");
        }
        other => panic!("the cancelled commit ended {other:?}"),
    }
    let next = until_pids(&stub, "commits", 2)[1];
    handle.submit(Request::CancelWrite { id: first });
    let after = arriving_within(&mut updates, Duration::from_millis(500));
    assert!(
        !lane_news(&after).contains(&Lane::Ended(second)),
        "a late cancel of the first commit ended the second: {after:?}"
    );
    release(&stub, next);
    let seen = until_ended(&mut updates, second);
    assert!(
        matches!(ending_of(&seen, second).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    drop(handle);
}

/// R4.9 beside a fetch: a close asked while a fetch and a commit both run ends the fetch at
/// once, as it ends every read and network operation, and waits on the commit alone — the
/// fetch's ending arrives while the commit is still held. Caught by: a close that ends the
/// fetch only once the local lane has been waited for, which leaves a fetch reaching the
/// network for as long as a commit's hooks run.
#[test]
fn a_close_ends_a_fetch_at_once_while_it_waits_on_a_commit() {
    let fixture = with_origin(
        &format!("cairn-lane-close-fetch-{}", std::process::id()),
        "/nonexistent/origin",
    );
    let config = fixture.path.join(".git/config");
    let mut text = std::fs::read_to_string(&config).unwrap_or_else(|error| panic!("{error}"));
    text.push_str(
        "[remote \"upstream\"]\n\turl = /nonexistent/upstream\n\t\
         fetch = +refs/heads/*:refs/remotes/upstream/*\n",
    );
    std::fs::write(&config, text).unwrap_or_else(|error| panic!("{error}"));
    // The network lane's fetch is of `origin`, the commit's of `upstream`: each held, each
    // writing its pid where the test reads it.
    let body = format!(
        "  case \"$*\" in\n  *upstream*)\n{}\n  ;;\n  *)\n{}\n  ;;\n  esac",
        held("commits"),
        held("fetches")
    );
    let stub = stub(&[("fetch", &body)]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    until_pids(&stub, "fetches", 1);
    let commit = ask(
        &handle,
        LocalWrite::HeldCommit {
            remote: "upstream".to_owned(),
        },
    );
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

/// C11 and R4.9, against a stub's long-running write the lane treats as a commit: a close
/// asked while it runs waits for it — the stream stays open and the commit's process alive —
/// and ends nothing; once it finishes, its ending arrives, a write queued behind it is not
/// run, and the stream ends. Caught by: a close that ends the commit's `git` (the registry's
/// close-everything), or one that runs the queued write.
#[test]
fn a_close_during_a_commit_waits_for_it_and_ends_nothing() {
    let fixture = with_origin(
        &format!("cairn-lane-close-waits-{}", std::process::id()),
        "/nonexistent/origin",
    );
    write_files(&fixture.path, &["a"]);
    let stub = stub(&[("fetch", &held("commits"))]);
    let _released = ReleaseAll(&stub);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = boundary(&fixture.path, &stub, (&home, &runtime));

    let commit = ask(&handle, commit());
    let pid = until_pids(&stub, "commits", 1)[0];
    let queued = ask(&handle, stage(&["a"]));
    handle.submit(Request::Close);
    // Past CLOSE_BOUND, when a close that ends what runs has given up waiting on it.
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
        !super::lifecycle_tests::alive_in_group(pid).is_empty(),
        "the commit's git is gone"
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
    assert_eq!(pids(&stub, "finished"), [pid], "the commit did not finish");
}

/// C11 and R4.9: a lock file left in the git directory — by a write a close gave up on, say
/// — is named as the repository opens, and by the write that fails on it. Caught by: an open
/// that lists nothing, or a failed write that drops the locks git failed on.
#[test]
fn a_lock_left_behind_is_named_as_the_repository_opens_and_by_the_write_it_fails() {
    let fixture = with_files("cairn-lane-lock-left", &["a"]);
    let lock = fixture.path.join(".git/index.lock");
    std::fs::write(&lock, "").unwrap_or_else(|error| panic!("{error}"));
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _reply) = real_open(&fixture.path, (&home, &runtime));
    match next_by(&mut updates, Instant::now() + WAIT, &[]) {
        Some(Update::Opened { locks, .. }) => assert_eq!(
            locks
                .iter()
                .map(|path| path.ends_with(".git/index.lock"))
                .collect::<Vec<_>>(),
            [true]
        ),
        other => panic!("{other:?}"),
    }
    let id = ask(&handle, stage(&["a"]));
    let seen = until_ended(&mut updates, id);
    match ending_of(&seen, id).0 {
        WriteEnding::Failed { locks, .. } => assert!(
            locks.iter().any(|path| path.ends_with(".git/index.lock")),
            "{locks:?}"
        ),
        other => panic!("a stage over index.lock ended {other:?}"),
    }
    drop(handle);
}

/// Answers the next prompt among the updates with `secret`, returning what came before it.
fn answer_next_prompt(updates: &mut Updates, reply: &Replier, secret: &str) -> Vec<Update> {
    let seen = collect_until(updates, |update| matches!(update, Update::Prompt { .. }));
    let Some(Update::Prompt { id, text }) = seen.last() else {
        unreachable!("collected until a prompt");
    };
    assert!(text.contains("passphrase"), "{text}");
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
    let seen = answer_next_prompt(&mut updates, &reply, "correct horse");
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

/// C12's commit half, against a stub's long-running write the lane treats as a commit (phase 05
/// signs a real `git commit` with an SSH key): what it runs asks for a passphrase through the
/// helper, the window is asked, and the answer reaches it.
#[test]
fn a_prompt_a_commit_raises_is_shown_and_answered() {
    let fixture = with_origin(
        &format!("cairn-lane-commit-asks-{}", std::process::id()),
        "/nonexistent/origin",
    );
    let stub = stub(&[("fetch", &format!("{}  exit 0", asks("$DIR")))]);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, reply) = boundary(&fixture.path, &stub, (&home, &runtime));

    let id = ask(&handle, commit());
    answer_next_prompt(&mut updates, &reply, "battery staple");
    let seen = until_ended(&mut updates, id);
    assert!(
        matches!(ending_of(&seen, id).0, WriteEnding::Done(_)),
        "{seen:?}"
    );
    assert_eq!(
        std::fs::read_to_string(stub.directory.join("heard")).unwrap_or_default(),
        "battery staple"
    );
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
    let lines = cairn_git::ops::discard_lines_consequence(&git, &repo, &diff, selection)
        .unwrap_or_else(|error| panic!("{error}"));
    let files = cairn_git::ops::discard_files_consequence(&git, &repo, &[RepoPath::from("b")])
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
