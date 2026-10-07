//! The refresh, through the real boundary (refs-and-status R10, R11, criterion C10): one
//! refresh reads the refs, ahead/behind and status, each answered in its own lane; a refresh
//! asked again at once supersedes the one before it lane by lane, so only one answer of each
//! is drawn; a refresh cancels neither a page being walked nor a diff being read; and a slow
//! status, on the refresh thread, delays neither.
//!
//! Over a repository borrowing this checkout's objects, its refs written with `std::fs` (no
//! `Command` here — the guards scan this crate's tests), and a stub `git` in front of the
//! real one where a status must hang.

use std::path::Path;
use std::time::{Duration, Instant};

use cairn_git::{CancelSignal, HistoryRequest, Repository};
use cairn_model::{Oid, WorkingTreeStatus};

use super::discovery::Discovery;
use super::fetch_tests::{BorrowedRepository, Home, RuntimeDir, WAIT, collect_until, next_by};
use super::lifecycle_tests::StubGit;
use super::pool::{RepositoryHandle, Updates, open_with};
use super::request::{Comparison, Request, Update};
use super::startup::Startup;

/// A repository of this checkout's objects whose refs a test moves: `main` at the newest
/// commit of this checkout's `HEAD`, `topic` at the next, `other` beside `main`, `HEAD` on
/// `main`. Every commit named is one `main` reaches, so each is walked.
pub(crate) struct Refreshable {
    fixture: BorrowedRepository,
    /// The newest commits `HEAD` reaches in this checkout, newest first.
    pub(crate) commits: Vec<Oid>,
}

impl Refreshable {
    pub(crate) fn new(name: &str) -> Self {
        let commits = match Repository::discover(env!("CARGO_MANIFEST_DIR"))
            .and_then(|repo| repo.history(&HistoryRequest::from_head(4), &CancelSignal::new()))
        {
            Ok(page) => page.rows.ids().collect::<Vec<_>>(),
            Err(error) => panic!("walking this checkout: {error}"),
        };
        assert_eq!(
            commits.len(),
            4,
            "this checkout has fewer than four commits"
        );
        let fixture = BorrowedRepository::new(&format!("{name}-{}", std::process::id()));
        let refreshable = Self { fixture, commits };
        refreshable.point("main", refreshable.commits[0]);
        refreshable.point("topic", refreshable.commits[1]);
        refreshable.point("other", refreshable.commits[0]);
        refreshable
    }

    fn git_dir(&self) -> std::path::PathBuf {
        self.fixture.fixture.path.join(".git")
    }

    pub(crate) fn path(&self) -> &Path {
        &self.fixture.fixture.path
    }

    /// The boundary over this repository, with this process's `git`.
    pub(crate) fn open(&self) -> (RepositoryHandle, Updates) {
        match open_with(self.path(), Startup::of_this_process()) {
            Ok((handle, updates, _)) => (handle, updates),
            Err(error) => panic!("starting the worker: {error}"),
        }
    }

    /// Points the local branch `branch` at `commit`, making it if it is not there.
    pub(crate) fn point(&self, branch: &str, commit: Oid) {
        let file = self.git_dir().join("refs/heads").join(branch);
        std::fs::write(&file, format!("{commit}\n"))
            .unwrap_or_else(|error| panic!("writing {}: {error}", file.display()));
    }

    /// Puts `HEAD` on `branch`, as a checkout does: what moves no ref when `branch` is on
    /// the commit `HEAD` was.
    pub(crate) fn check_out(&self, branch: &str) {
        let head = self.git_dir().join("HEAD");
        std::fs::write(&head, format!("ref: refs/heads/{branch}\n"))
            .unwrap_or_else(|error| panic!("writing {}: {error}", head.display()));
    }

    /// Pushes `commit` onto the stash list, as `git stash` records it: `refs/stash` and its
    /// reflog. Any commit with a parent reads as a stash made on that parent.
    pub(crate) fn push_stash(&self, commit: Oid, message: &str) {
        let log = self.git_dir().join("logs/refs/stash");
        if let Some(parent) = log.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|error| panic!("making {}: {error}", parent.display()));
        }
        let old = std::fs::read_to_string(self.git_dir().join("refs/stash"))
            .map(|id| id.trim().to_owned())
            .unwrap_or_else(|_| "0".repeat(40));
        let mut lines = std::fs::read_to_string(&log).unwrap_or_default();
        lines.push_str(&format!(
            "{old} {commit} Cairn <cairn@example.com> 1700000000 +0000\t{message}\n"
        ));
        std::fs::write(&log, lines)
            .unwrap_or_else(|error| panic!("writing {}: {error}", log.display()));
        let stash = self.git_dir().join("refs/stash");
        std::fs::write(&stash, format!("{commit}\n"))
            .unwrap_or_else(|error| panic!("writing {}: {error}", stash.display()));
    }
}

/// What one refresh's three answers were, read until all three have arrived.
#[derive(Debug, Default)]
struct Answered {
    refs: Vec<bool>,
    ahead_behind: usize,
    status: usize,
    /// Every other update seen meanwhile.
    others: Vec<Update>,
}

impl Answered {
    fn take(&mut self, update: Update) {
        match update {
            Update::Refs { reopen, .. } => self.refs.push(reopen),
            Update::AheadBehind { .. } => self.ahead_behind += 1,
            Update::Status { .. } => self.status += 1,
            other => self.others.push(other),
        }
    }

    fn all_three(&self) -> bool {
        !self.refs.is_empty() && self.ahead_behind > 0 && self.status > 0
    }
}

/// Reads until a refresh's three answers have all arrived.
fn one_refresh(updates: &mut Updates) -> Answered {
    let deadline = Instant::now() + WAIT;
    let mut answered = Answered::default();
    while !answered.all_three() {
        match next_by(updates, deadline, &answered.others) {
            Some(update) => answered.take(update),
            None => panic!("the stream ended mid-refresh: {answered:?}"),
        }
    }
    answered
}

/// Whatever arrives within `quiet`, which is expected to be nothing more of a refresh.
fn arriving_within(updates: &mut Updates, quiet: Duration) -> Answered {
    let mut answered = Answered::default();
    let deadline = Instant::now() + quiet;
    loop {
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        let mut next = std::pin::pin!(updates.next());
        match next.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(Some(update)) => answered.take(update),
            std::task::Poll::Ready(None) => return answered,
            std::task::Poll::Pending if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            std::task::Poll::Pending => return answered,
        }
    }
}

/// R10.1, R11.1: a refresh reads the refs, ahead/behind and the working tree's status, each
/// answered once; the first, with no walk to compare with, says to open the history, and once
/// the history is open from those refs, a refresh that finds nothing changed says to leave it.
/// Caught by: a refresh that reads one of the three not at all (no answer arrives), a status
/// or count not answered through the boundary, or a reopen decided against nothing.
#[test]
fn a_refresh_answers_the_refs_ahead_behind_and_status_each_once() {
    let fixture = Refreshable::new("cairn-refresh-answers");
    let (handle, mut updates) = fixture.open();
    handle.submit(Request::Refresh);
    let first = one_refresh(&mut updates);
    assert_eq!(first.refs, [true], "{first:?}");
    assert_eq!((first.ahead_behind, first.status), (1, 1), "{first:?}");

    handle.submit(Request::OpenHistory { rows: 8 });
    let seen = collect_until(&mut updates, |update| matches!(update, Update::Rows { .. }));
    assert!(
        !seen
            .iter()
            .any(|update| matches!(update, Update::Refs { .. })),
        "the open read its own refs though a refresh had read them: {seen:?}"
    );

    handle.submit(Request::Refresh);
    let again = one_refresh(&mut updates);
    assert_eq!(again.refs, [false], "nothing changed, yet {again:?}");
    drop(handle);
}

/// The QA brief: focus flapping — two refreshes asked at once, as Alt+Tab twice quickly asks
/// them — draws one answer of each kind, the second refresh's; the first's is superseded in
/// each of its lanes, cancelled where it was still running and dropped where it had finished,
/// before the window reads it. Deterministic: the second is numbered before any answer can be
/// read, and an answer is drawn only while its number is current. Caught by: a refresh
/// numbered in fewer than its three lanes (two answers of the lane left out arrive), or a
/// refresh that supersedes nothing.
#[test]
fn a_refresh_asked_twice_at_once_draws_one_answer_of_each() {
    let fixture = Refreshable::new("cairn-refresh-flapping");
    let (handle, mut updates) = fixture.open();
    handle.submit(Request::Refresh);
    handle.submit(Request::Refresh);
    let answered = one_refresh(&mut updates);
    let later = arriving_within(&mut updates, Duration::from_millis(1_500));
    assert_eq!(answered.refs.len(), 1, "{answered:?}");
    assert_eq!(
        (later.refs.len(), later.ahead_behind, later.status),
        (0, 0, 0),
        "a superseded refresh was drawn: {later:?}"
    );
    // What the first had finished comes back only to be freed.
    assert!(
        later
            .others
            .iter()
            .chain(&answered.others)
            .all(|update| matches!(update, Update::Superseded(_))),
        "{later:?}"
    );
    drop(handle);
}

/// The QA brief: a refresh while a page is loading cancels neither the page nor a diff
/// being read — each answer arrives, whole, beside the refresh's. Deterministic: were a
/// refresh numbered in the history or the changes lane, the page's or the diff's number
/// would no longer be current when it arrived, and it would never be drawn. Caught by: a
/// refresh that supersedes the history lane or a diff lane.
#[test]
fn a_refresh_cancels_neither_a_page_being_walked_nor_a_diff_being_read() {
    let fixture = Refreshable::new("cairn-refresh-beside");
    let (handle, mut updates) = fixture.open();
    handle.submit(Request::Refresh);
    one_refresh(&mut updates);
    handle.submit(Request::OpenHistory { rows: 2 });
    collect_until(&mut updates, |update| matches!(update, Update::Rows { .. }));

    let of = Comparison::Commit(fixture.commits[0]);
    handle.submit(Request::MoreHistory { rows: 500 });
    handle.submit(Request::Changes { of });
    handle.submit(Request::Refresh);
    let deadline = Instant::now() + WAIT;
    let mut seen = Vec::new();
    let (mut page, mut changes, mut refs) = (None, false, false);
    while page.is_none() || !changes || !refs {
        let Some(update) = next_by(&mut updates, deadline, &seen) else {
            panic!("the stream ended: {seen:?}");
        };
        match &update {
            Update::Rows { rows, complete } => page = Some((rows.len(), *complete)),
            Update::Changes { of: answered, .. } => changes = *answered == of,
            Update::Refs { reopen, .. } => {
                assert!(!reopen, "nothing moved, yet the refresh said to reopen");
                refs = true;
            }
            _ => {}
        }
        seen.push(update);
    }
    let Some((rows, complete)) = page else {
        unreachable!("the loop waits for the page");
    };
    assert!(
        rows == 500 || complete,
        "the page was cut short: {rows} rows, complete {complete}"
    );
    drop(handle);
}

/// A `status` that hangs until `$DIR/release` exists, saying `$DIR/started` as it begins;
/// bounded, so a failing test leaves nothing running long.
/// The stub's `PATH` is its own directory, so only the shell's builtins and programs named
/// by their path run.
const STATUS_HANGS: &str = "  : > \"$DIR/started\"\n  n=0\n  \
     while [ ! -e \"$DIR/release\" ] && [ $n -lt 600 ]; do /bin/sleep 0.05; n=$((n+1)); done\n  \
     exit 0";

/// R11.2 and the QA brief: a slow status — 736 ms on a stat-dirty rust-lang/rust — runs on
/// the refresh thread, so neither a page nor a diff asked while it runs waits for it: both
/// answers arrive while the status is still running, and the status after. Deterministic:
/// the status cannot finish before the test releases it. Caught by: status served on the
/// repository thread (the refs and the page queue behind it) or the diff thread (the diff
/// does).
#[test]
fn a_slow_status_delays_neither_a_page_nor_a_diff() {
    let fixture = Refreshable::new("cairn-refresh-slow-status");
    let stub = StubGit::wrapping("status", STATUS_HANGS);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates) = match super::pool::open(
        fixture.path(),
        &Discovery::new(stub.startup(Some((&home, &runtime)))),
    ) {
        Ok((handle, updates, _)) => (handle, updates),
        Err(error) => panic!("starting the worker: {error}"),
    };
    let release = stub.directory.join("release");
    // Released however the test ends, so the stub's loop ends with it.
    struct Release<'a>(&'a Path);
    impl Drop for Release<'_> {
        fn drop(&mut self) {
            let _ = std::fs::write(self.0, "");
        }
    }
    let _released = Release(&release);

    handle.submit(Request::Refresh);
    let started = stub.directory.join("started");
    let deadline = Instant::now() + WAIT;
    while !started.exists() {
        assert!(Instant::now() < deadline, "the status never started");
        std::thread::sleep(Duration::from_millis(5));
    }

    let of = Comparison::Commit(fixture.commits[0]);
    handle.submit(Request::Changes { of });
    let mut seen = Vec::new();
    let (mut opened, mut rows, mut changes) = (false, false, false);
    let deadline = Instant::now() + WAIT;
    while !(rows && changes) {
        let Some(update) = next_by(&mut updates, deadline, &seen) else {
            panic!("the stream ended: {seen:?}");
        };
        match &update {
            Update::Refs { reopen: true, .. } if !opened => {
                handle.submit(Request::OpenHistory { rows: 8 });
                opened = true;
            }
            Update::Rows { rows: page, .. } => rows = !page.is_empty(),
            Update::Changes { of: answered, .. } => changes = *answered == of,
            Update::Status { .. } => panic!("the status finished before it was released"),
            _ => {}
        }
        seen.push(update);
    }
    assert!(!release.exists(), "released early");

    std::fs::write(&release, "")
        .unwrap_or_else(|error| panic!("writing {}: {error}", release.display()));
    let seen = collect_until(&mut updates, |update| {
        matches!(update, Update::Status { .. } | Update::RefreshFailed { .. })
    });
    match seen.last() {
        Some(Update::Status { status }) => {
            assert_eq!(*status, WorkingTreeStatus::Listed(Vec::new()));
        }
        other => panic!("expected the released status, got {other:?}"),
    }
    drop(handle);
}
