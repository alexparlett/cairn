//! The query lanes, through the real boundary (PRD R4, criterion C8): a scroll does not
//! cancel a diff and a diff does not cancel a scroll; a changes query supersedes the
//! file-diff lane; a superseded diff's `git` is killed, not left to run out; an answer
//! naming another selection is never drawn (`session`'s
//! `an_answer_naming_another_selection_is_never_drawn`, through these helpers, since a
//! render-side test may name neither the toolkit here nor a wait there); and a fetch still
//! supersedes nothing.
//!
//! Real `git` over this checkout (read only) or over a fixture borrowing its objects, and a
//! stub `git` where a read must hang. No `Command` here — the guards scan this crate's
//! tests — so fixtures are built with `std::fs`.

use std::path::Path;
use std::time::{Duration, Instant};

use cairn_model::{ChangeSet, ChangeStatus, ChangedFile, DiffContent, FileDiff, Oid, RepoPath};

use super::diff_freshness::SETTLING;
use super::discovery::Discovery;
use super::fetch_tests::{BorrowedRepository, Home, RuntimeDir, WAIT, collect_until, next_by};
use super::lifecycle_tests::StubGit;
#[cfg(target_os = "linux")]
use super::lifecycle_tests::alive_in_group;
use super::pool::{RepositoryHandle, Updates, open_with};
use super::request::{
    Comparison, DiffOptions, DiffQuery, FileQuery, FileTarget, Request, Update, WorkingSide,
};
use super::startup::Startup;

/// The boundary over `repository`, with this process's `git`.
fn opened(repository: &Path) -> (RepositoryHandle, Updates) {
    match open_with(repository, Startup::of_this_process()) {
        Ok((handle, updates, _)) => (handle, updates),
        Err(error) => panic!("starting the worker: {error}"),
    }
}

/// The boundary over this checkout; for the window's tests too, which may not wait.
pub(crate) fn checkout() -> (RepositoryHandle, Updates) {
    opened(Path::new(env!("CARGO_MANIFEST_DIR")))
}

/// The first `count` commits of the history `HEAD` reaches, newest first.
pub(crate) fn commits(handle: &RepositoryHandle, updates: &mut Updates, count: usize) -> Vec<Oid> {
    handle.submit(Request::OpenHistory { rows: count });
    let seen = collect_until(updates, |u| matches!(u, Update::Rows { .. }));
    match seen.last() {
        Some(Update::Rows { rows, .. }) if rows.len() == count => {
            rows.iter().map(|row| row.graph.id).collect()
        }
        other => panic!("expected {count} rows, got {other:?}"),
    }
}

/// `of`'s change set, asked and awaited.
fn change_set(handle: &RepositoryHandle, updates: &mut Updates, of: Comparison) -> ChangeSet {
    handle.submit(Request::Changes { of });
    let seen = collect_until(updates, |u| matches!(u, Update::Changes { .. }));
    match seen.last() {
        Some(Update::Changes {
            of: answered,
            changes,
        }) if *answered == of => changes.clone(),
        other => panic!("expected {of:?}'s change set, got {other:?} after {seen:?}"),
    }
}

fn answers_changes(update: &Update, of: Comparison) -> bool {
    matches!(update, Update::Changes { of: answered, .. } if *answered == of)
}

/// The next update past the boundary — the epoch filter already behind it — for a test of
/// the window's side, which may not wait itself; a failure naming the wait if none comes.
pub(crate) fn next_update(updates: &mut Updates) -> Update {
    match next_by(updates, Instant::now() + WAIT, &[]) {
        Some(update) => update,
        None => panic!("the update stream ended"),
    }
}

/// A repository of this checkout's objects whose configuration a test may edit, and a
/// commit in it that modified a text file, with that file: for the window's tests of a
/// configuration edit, which may not wait themselves.
pub(crate) struct Configurable {
    fixture: BorrowedRepository,
    pub(crate) of: Comparison,
    pub(crate) file: ChangedFile,
}

impl Configurable {
    pub(crate) fn new(name: &str) -> Self {
        let (head, of, file, _) = two_rust_files();
        let fixture = BorrowedRepository::new(&format!("cairn-{name}-{}", std::process::id()));
        fixture.point_main_at(&head.to_string());
        settle();
        Self { fixture, of, file }
    }

    /// The boundary over the repository, with this process's `git`.
    pub(crate) fn open(&self) -> (RepositoryHandle, Updates) {
        opened(&self.fixture.fixture.path)
    }

    /// Appends `text` to `$GIT_DIR/config`.
    pub(crate) fn configure(&self, text: &str) {
        append(&self.fixture.fixture.path.join(".git").join("config"), text);
    }
}

/// The next answer naming `of`'s change set, as it leaves the boundary — the epoch filter
/// already behind it — for a test of the window's side, which may not wait itself.
pub(crate) fn changes_answer(updates: &mut Updates, of: Comparison) -> Update {
    let seen = collect_until(updates, |u| answers_changes(u, of));
    match seen.last() {
        Some(update) => update.clone(),
        None => unreachable!("collect_until returns what it stopped on"),
    }
}

/// C8, the first half: a scroll asked while a changes query is in flight does not cancel
/// it — the change set still arrives, and so does the page. Mutation that reddens it:
/// numbering every lane on one counter (`QueryLane::index` answering 0 for every lane), so
/// the page supersedes the diff and its answer is dropped on arrival.
#[test]
fn a_scroll_does_not_cancel_a_diff() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let of = Comparison::Commit(ids[1]);

    handle.submit(Request::Changes { of });
    handle.submit(Request::OpenHistory { rows: 40 });

    let (mut pages, mut answered) = (0, false);
    let seen = collect_until(&mut updates, |u| {
        pages += usize::from(matches!(u, Update::Rows { .. }));
        answered |= answers_changes(u, of);
        answered && pages == 1
    });
    assert!(
        !seen
            .iter()
            .any(|u| matches!(u, Update::DiffFailed { .. } | Update::Failed { .. })),
        "{seen:?}"
    );
    drop(handle);
}

/// C8, the second half: a changes query and a file diff asked while a page is being
/// walked do not cancel it — the page arrives. Mutation that reddens it: one counter for
/// every lane, under which the diff supersedes the page and the walk is abandoned.
#[test]
fn a_diff_does_not_cancel_a_scroll() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let of = Comparison::Commit(ids[0]);
    let changes = change_set(&handle, &mut updates, of);
    let Some(file) = changes.files.first().cloned() else {
        panic!("{of:?} changed no file");
    };

    handle.submit(Request::OpenHistory { rows: 60 });
    handle.submit(Request::Changes {
        of: Comparison::Commit(ids[1]),
    });
    handle.submit(Request::FileDiff(FileQuery {
        target: FileTarget::Committed { of, file },
        options: DiffOptions::default(),
    }));
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    match seen.last() {
        Some(Update::Rows { rows, .. }) => assert!(!rows.is_empty(), "an empty page"),
        other => panic!("the page never arrived: {other:?}"),
    }
    drop(handle);
}

/// C8: a changes query supersedes the file diff in flight — its answer is never
/// delivered, whether it was cancelled before it started, killed while git read, or
/// finished and dropped on arrival — and nothing else. The file diff, served before the
/// changes query on the one diff thread, would arrive first if it survived. Mutation that
/// reddens it: `QueryLane::Changes.supersedes()` naming the changes lane alone.
#[test]
fn a_changes_query_supersedes_the_file_diff_in_flight() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let (first, second) = (Comparison::Commit(ids[0]), Comparison::Commit(ids[1]));
    let changes = change_set(&handle, &mut updates, first);
    let Some(file) = changes.files.first().cloned() else {
        panic!("{first:?} changed no file");
    };

    handle.submit(Request::FileDiff(FileQuery {
        target: FileTarget::Committed { of: first, file },
        options: DiffOptions::default(),
    }));
    handle.submit(Request::Changes { of: second });
    let seen = collect_until(&mut updates, |u| answers_changes(u, second));
    assert!(
        !seen
            .iter()
            .any(|u| matches!(u, Update::FileDiff { .. } | Update::DiffFailed { .. })),
        "the superseded file diff was delivered: {seen:?}"
    );
    drop(handle);
}

/// C8, the negative of the one above: a file diff does not supersede the changes query in
/// flight, nor one file diff the history — both answers arrive. Mutation that reddens it:
/// a file diff numbered in the changes lane too.
#[test]
fn a_file_diff_does_not_supersede_the_changes_query() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let first = Comparison::Commit(ids[0]);
    let changes = change_set(&handle, &mut updates, first);
    let Some(file) = changes.files.first().cloned() else {
        panic!("{first:?} changed no file");
    };
    let second = Comparison::Commit(ids[1]);
    let query = FileQuery {
        target: FileTarget::Committed { of: first, file },
        options: DiffOptions::default(),
    };

    handle.submit(Request::Changes { of: second });
    handle.submit(Request::FileDiff(query.clone()));
    let mut changes_seen = false;
    let mut file_seen = false;
    collect_until(&mut updates, |u| {
        changes_seen |= answers_changes(u, second);
        file_seen |=
            matches!(u, Update::FileDiff { query: answered, diff: Some(_) } if *answered == query);
        changes_seen && file_seen
    });
    drop(handle);
}

/// C8, a fast click through a file list: each file's diff supersedes the one before, and
/// only the last is answered. (Whether the ones before are killed rather than run out is
/// `a_superseded_diff_kills_its_git`; whether one that finished anyway is dropped on
/// arrival is `pool`'s `an_answer_superseded_in_its_lane_is_dropped_on_arrival`.)
#[test]
fn a_click_through_files_answers_the_last_file_only() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 8);
    // A commit that changed at least two files, so there is something to click through.
    let (of, changes) = ids
        .iter()
        .map(|id| {
            let of = Comparison::Commit(*id);
            (of, change_set(&handle, &mut updates, of))
        })
        .find(|(_, changes)| changes.files.len() >= 2)
        .unwrap_or_else(|| panic!("none of {ids:?} changed two files"));
    let queries: Vec<FileQuery> = changes
        .files
        .iter()
        .take(6)
        .map(|file| FileQuery {
            target: FileTarget::Committed {
                of,
                file: file.clone(),
            },
            options: DiffOptions::default(),
        })
        .collect();
    for query in &queries {
        handle.submit(Request::FileDiff(query.clone()));
    }
    let last = queries.last().cloned();
    let seen = collect_until(
        &mut updates,
        |u| matches!(u, Update::FileDiff { query, .. } if Some(query) == last.as_ref()),
    );
    let answered: Vec<_> = seen
        .iter()
        .filter_map(|u| match u {
            Update::FileDiff { query, .. } => Some(query),
            _ => None,
        })
        .collect();
    assert_eq!(answered, [&queries[queries.len() - 1]]);
    drop(handle);
}

/// A diff thread's `git` that leads its group, starts a grandchild holding its pipes,
/// appends its pid to `leaders` and waits on the grandchild for ten minutes: what only a
/// group kill ends. `/bin/sleep` by its path, since the stub's `PATH` is its own directory.
/// The grandchild is started before the pid is written, so a test that has read the pid can
/// rely on both being in the group.
const DIFF_TREE_HANGS: &str = "  /bin/sleep 600 &\n  echo $$ >> \"$DIR/leaders\"\n  wait";

/// The pids the stub's `diff-tree` has written, waiting until there are `count`.
fn leaders(stub: &StubGit, count: usize) -> Vec<i32> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let written: Vec<i32> = std::fs::read_to_string(stub.directory.join("leaders"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect();
        if written.len() >= count {
            return written;
        }
        assert!(
            Instant::now() < deadline,
            "the stub's diff-tree ran {} times, not {count}",
            written.len()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// C8 and the design input on fast clicks: a superseded diff's `git` is KILLED — its whole
/// process group, the grandchild holding its pipes included — and the command log records
/// it as cancelled; it is not left to run out with its answer discarded. Mutation that
/// reddens it: handing the engine a cancel that never fires (`CancelSignal::new()` in
/// place of the epoch's `Superseded`), which leaves the first `diff-tree` running for ten
/// minutes.
#[test]
fn a_superseded_diff_kills_its_git() {
    let ids = {
        let (handle, mut updates) = checkout();
        commits(&handle, &mut updates, 2)
    };
    let stub = StubGit::answering("2.45.0", "diff-tree", DIFF_TREE_HANGS);
    let fixture = BorrowedRepository::new(&format!("cairn-diff-killed-{}", std::process::id()));
    fixture.point_main_at(&ids[0].to_string());
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates) = match super::pool::open(
        &fixture.fixture.path,
        &Discovery::new(stub.startup(Some((&home, &runtime)))),
    ) {
        Ok((handle, updates, _)) => (handle, updates),
        Err(error) => panic!("starting the worker: {error}"),
    };

    handle.submit(Request::Changes {
        of: Comparison::Commit(ids[0]),
    });
    let first = leaders(&stub, 1)[0];
    #[cfg(target_os = "linux")]
    assert!(
        alive_in_group(first).len() >= 2,
        "the case needs the first diff-tree and its grandchild running in one group"
    );
    // The next commit selected, as a click on the list does.
    let superseded = Instant::now();
    handle.submit(Request::Changes {
        of: Comparison::Commit(ids[1]),
    });
    leaders(&stub, 2);

    #[cfg(target_os = "linux")]
    {
        let deadline = superseded + Duration::from_secs(10);
        while !alive_in_group(first).is_empty() {
            assert!(
                Instant::now() < deadline,
                "ten seconds after it was superseded, the first diff-tree's group was still \
                 running: {:?}",
                alive_in_group(first)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // The log holds each invocation once it is over; the first is, as a cancel.
    let deadline = Instant::now() + WAIT;
    let record = loop {
        handle.submit(Request::CommandLog);
        let seen = collect_until(&mut updates, |u| matches!(u, Update::CommandLog { .. }));
        let found = seen.iter().find_map(|u| match u {
            Update::CommandLog { records } => records
                .iter()
                .find(|record| {
                    record.arguments.iter().any(|a| a == "diff-tree")
                        && record
                            .arguments
                            .iter()
                            .any(|a| a.contains(&ids[0].to_string()))
                })
                .cloned(),
            _ => None,
        });
        if let Some(record) = found {
            break record;
        }
        assert!(
            Instant::now() < deadline,
            "the first diff-tree never reached the log"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        record.cancelled,
        "the superseded diff-tree was not ended as a cancel: {record:?}"
    );

    // Neither query answers: one superseded, the other closed with the repository.
    handle.submit(Request::Close);
    let mut seen = Vec::new();
    while let Some(update) = next_by(&mut updates, Instant::now() + WAIT, &seen) {
        seen.push(update);
    }
    assert!(
        !seen
            .iter()
            .any(|u| matches!(u, Update::Changes { .. } | Update::DiffFailed { .. })),
        "a superseded or closed diff answered: {seen:?}"
    );
    drop(handle);
}

/// C8: a fetch still supersedes nothing — a page and a changes query asked before it are
/// both answered. A fixture borrowing this checkout's objects, so the fetch (of a remote
/// that does not exist, which fails at once and writes nothing) runs in a repository of
/// the test's own. Mutation that reddens it: an operation numbered in a lane, such as
/// `Request::Fetch` in the changes lane.
#[test]
fn a_fetch_still_supersedes_nothing() {
    let ids = {
        let (handle, mut updates) = checkout();
        commits(&handle, &mut updates, 1)
    };
    let fixture = BorrowedRepository::new(&format!("cairn-diff-fetch-{}", std::process::id()));
    fixture.point_main_at(&ids[0].to_string());
    let (handle, mut updates) = opened(&fixture.fixture.path);
    let of = Comparison::Commit(ids[0]);

    handle.submit(Request::OpenHistory { rows: 1 });
    handle.submit(Request::Changes { of });
    handle.submit(Request::Fetch {
        remote: "/nonexistent/cairn-remote.git".to_owned(),
    });
    let (mut rows, mut changes, mut fetched) = (false, false, false);
    collect_until(&mut updates, |u| {
        rows |= matches!(u, Update::Rows { .. });
        changes |= answers_changes(u, of);
        fetched |= matches!(
            u,
            Update::FetchFailed { .. }
                | Update::FetchFinished { .. }
                | Update::FetchCancelled { .. }
        );
        rows && changes && fetched
    });
    drop(handle);
}

/// Every `diff-tree` the repository has run and finished, oldest first, by its arguments.
fn diff_trees(handle: &RepositoryHandle, updates: &mut Updates) -> Vec<Vec<String>> {
    handle.submit(Request::CommandLog);
    let seen = collect_until(updates, |u| matches!(u, Update::CommandLog { .. }));
    match seen.last() {
        Some(Update::CommandLog { records }) => records
            .iter()
            .filter(|record| record.arguments.iter().any(|a| a == "diff-tree"))
            .map(|record| record.arguments.clone())
            .collect(),
        other => panic!("expected the command log, got {other:?}"),
    }
}

/// How many patch reads (`diff-tree -p`, a file's lines) and listings (`diff-tree --raw`
/// alone, a change set) have run.
fn reads(handle: &RepositoryHandle, updates: &mut Updates) -> (usize, usize) {
    let runs = diff_trees(handle, updates);
    let patches = runs.iter().filter(|a| a.iter().any(|a| a == "-p")).count();
    (patches, runs.len() - patches)
}

/// The `--diff-algorithm` the newest patch read was given.
fn last_algorithm(handle: &RepositoryHandle, updates: &mut Updates) -> String {
    let runs = diff_trees(handle, updates);
    runs.iter()
        .rev()
        .find(|a| a.iter().any(|a| a == "-p"))
        .and_then(|a| a.iter().find_map(|a| a.strip_prefix("--diff-algorithm=")))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("no patch read named an algorithm: {runs:?}"))
}

/// `query`'s answer, asked and awaited; a failure is a panic naming it.
fn file_answer(
    handle: &RepositoryHandle,
    updates: &mut Updates,
    query: &FileQuery,
) -> Option<FileDiff> {
    handle.submit(Request::FileDiff(query.clone()));
    let seen = collect_until(updates, |u| match u {
        Update::FileDiff {
            query: answered, ..
        } => answered == query,
        Update::DiffFailed { .. } => true,
        _ => false,
    });
    match seen.last() {
        Some(Update::FileDiff { diff, .. }) => diff.clone().map(|shown| shown.into_diff()),
        other => panic!("expected {query:?}'s diff, got {other:?}"),
    }
}

fn committed(of: Comparison, file: &ChangedFile) -> FileQuery {
    FileQuery {
        target: FileTarget::Committed {
            of,
            file: file.clone(),
        },
        options: DiffOptions::default(),
    }
}

fn is_text(diff: &Option<FileDiff>) -> bool {
    matches!(
        diff,
        Some(FileDiff {
            content: DiffContent::Text { .. },
            ..
        })
    )
}

fn is_binary(diff: &Option<FileDiff>) -> bool {
    matches!(
        diff,
        Some(FileDiff {
            content: DiffContent::Binary { .. },
            ..
        })
    )
}

/// A commit of this checkout's history that modified at least two Rust files git has to
/// compare line by line, and two of them: what an attribute edit can turn binary.
fn two_rust_files() -> (Oid, Comparison, ChangedFile, ChangedFile) {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 40);
    for id in ids {
        let of = Comparison::Commit(id);
        let changes = change_set(&handle, &mut updates, of);
        let modified: Vec<&ChangedFile> = changes
            .files
            .iter()
            .filter(|f| f.status == ChangeStatus::Modified && f.new_path.display().ends_with(".rs"))
            .collect();
        if let [first, second, ..] = modified[..] {
            let (first, second) = (first.clone(), second.clone());
            let mut text = |file: &ChangedFile| {
                is_text(&file_answer(&handle, &mut updates, &committed(of, file)))
            };
            if text(&first) && text(&second) {
                return (id, of, first, second);
            }
        }
    }
    panic!("none of the last forty commits modified two Rust files");
}

/// The engine's answer for `query` in `repository`, asked on a handle and session of its
/// own: nothing kept. Its parity with `git show` is pinned in `cairn-git`
/// (`binary_detection_reads_the_attributes_where_git_reads_them`).
fn uncached(repository: &Path, query: &FileQuery) -> FileDiff {
    let FileTarget::Committed { of, file } = &query.target else {
        panic!("{query:?} is not a commit's");
    };
    let git = match cairn_git::ops::GitBinary::discover(&cairn_git::ops::Askpass::new(
        "/nonexistent/cairn-askpass",
        None,
    )) {
        Ok(git) => git,
        Err(error) => panic!("finding git: {error}"),
    };
    let request = match of {
        Comparison::Commit(id) => cairn_git::ChangesRequest::commit(*id),
        Comparison::Between { old, new } => cairn_git::ChangesRequest::between(*old, *new),
    };
    let engine = match cairn_git::Repository::discover(repository) {
        Ok(engine) => engine,
        Err(error) => panic!("opening {}: {error}", repository.display()),
    };
    match engine.file_diff(
        &git,
        &request,
        file,
        &cairn_git::ContentOptions::default(),
        &cairn_git::CancelSignal::new(),
    ) {
        Ok(diff) => diff,
        Err(error) => panic!("the engine's own answer: {error}"),
    }
}

/// Past the settling time, so every file a fixture just wrote can be trusted by its stamp
/// and what is read now is kept.
fn settle() {
    std::thread::sleep(SETTLING + Duration::from_millis(300));
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        panic!("making {}: {error}", parent.display());
    }
    if let Err(error) = std::fs::write(path, content) {
        panic!("writing {}: {error}", path.display());
    }
}

fn append(path: &Path, content: &str) {
    let mut held = std::fs::read_to_string(path).unwrap_or_default();
    held.push_str(content);
    write(path, &held);
}

/// P2 and R4.5 as amended: an unstaged attribute edit reaches an answer already KEPT and
/// one never asked — the working tree's `.gitattributes`, then `info/attributes` — each
/// binary as the engine answers on a handle of its own, which is `git show`'s answer.
/// The second identical ask, before either edit, runs no `git`: the answer was kept.
/// Caught by: a hit that does not check the directories its answer read (the kept answer
/// stays text), a renewal that keeps the session (its stack holds the top of the tree's
/// attributes), or the global tier left out (`info/attributes` never seen).
#[test]
fn an_unstaged_attribute_edit_reaches_a_kept_answer_and_a_new_one() {
    let (head, of, first, second) = two_rust_files();
    let fixture = BorrowedRepository::new(&format!("cairn-diff-attributes-{}", std::process::id()));
    fixture.point_main_at(&head.to_string());
    let root = fixture.fixture.path.clone();
    settle();
    let (handle, mut updates) = opened(&root);
    let (kept, other) = (committed(of, &first), committed(of, &second));

    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    let before = reads(&handle, &mut updates);
    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    assert_eq!(
        reads(&handle, &mut updates),
        before,
        "the second ask was not answered from what was kept"
    );

    write(&root.join(".gitattributes"), "*.rs -diff\n");
    let flipped = file_answer(&handle, &mut updates, &kept);
    assert!(
        is_binary(&flipped),
        "the kept answer did not see -diff: {flipped:?}"
    );
    assert_eq!(flipped, Some(uncached(&root, &kept)));
    let new = file_answer(&handle, &mut updates, &other);
    assert!(is_binary(&new), "a new answer did not see -diff: {new:?}");
    assert_eq!(new, Some(uncached(&root, &other)));

    if let Err(error) = std::fs::remove_file(root.join(".gitattributes")) {
        panic!("removing .gitattributes: {error}");
    }
    settle();
    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    let before = reads(&handle, &mut updates);
    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    assert_eq!(
        reads(&handle, &mut updates),
        before,
        "the answer was not kept again"
    );
    write(&root.join(".git/info/attributes"), "*.rs -diff\n");
    let flipped = file_answer(&handle, &mut updates, &kept);
    assert!(
        is_binary(&flipped),
        "the kept answer did not see info/attributes: {flipped:?}"
    );
    assert_eq!(flipped, Some(uncached(&root, &kept)));
    drop(handle);
}

/// P1 and R4.5 as amended: a configuration edit reaches the next answer, kept or not —
/// an include target created where none was, an edit to `$GIT_DIR/config` itself, and
/// `diff.renames` reaching the next change set. Caught by: answering on the handle opened
/// first (whose configuration gix read once), or stamping only the files gix loaded (the
/// include target did not exist).
#[test]
fn a_configuration_edit_reaches_the_next_answer() {
    let (head, of, first, _) = two_rust_files();
    let fixture = BorrowedRepository::new(&format!("cairn-diff-config-{}", std::process::id()));
    fixture.point_main_at(&head.to_string());
    let dot = fixture.fixture.path.join(".git");
    append(&dot.join("config"), "[include]\n\tpath = later.config\n");
    settle();
    let (handle, mut updates) = opened(&fixture.fixture.path);
    let kept = committed(of, &first);

    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    let before = last_algorithm(&handle, &mut updates);
    let (patches, _) = reads(&handle, &mut updates);
    file_answer(&handle, &mut updates, &kept);
    assert_eq!(
        reads(&handle, &mut updates).0,
        patches,
        "the answer was not kept"
    );
    let mut others = ["histogram", "patience", "minimal", "myers"]
        .into_iter()
        .filter(|algorithm| *algorithm != before);
    let (included, local) = match (others.next(), others.next()) {
        (Some(included), Some(local)) => (included, local),
        _ => unreachable!("three algorithms are not the one in use"),
    };

    write(
        &dot.join("later.config"),
        &format!("[diff]\n\talgorithm = {included}\n"),
    );
    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    assert_eq!(
        last_algorithm(&handle, &mut updates),
        included,
        "a created include target was not read"
    );

    append(
        &dot.join("config"),
        &format!("[diff]\n\talgorithm = {local}\n"),
    );
    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    assert_eq!(
        last_algorithm(&handle, &mut updates),
        local,
        "a config edit was not read"
    );

    let renamed = change_set(&handle, &mut updates, of).renames.enabled;
    append(
        &dot.join("config"),
        &format!("[diff]\n\trenames = {}\n", !renamed),
    );
    assert_eq!(
        change_set(&handle, &mut updates, of).renames.enabled,
        !renamed,
        "diff.renames did not reach the next change set"
    );
    drop(handle);
}

/// R4.5 as amended: a stat-only refresh of the index — a new file holding the same
/// entries, as `git update-index --refresh` writes it — keeps what is kept, and asking
/// again runs no `git`. Caught by: keying on the index file's stamp or its checksum,
/// which every refresh moves.
#[test]
fn a_stat_only_index_refresh_keeps_what_is_kept() {
    let (head, of, first, _) = two_rust_files();
    let fixture = BorrowedRepository::new(&format!("cairn-diff-refresh-{}", std::process::id()));
    fixture.point_main_at(&head.to_string());
    let dot = fixture.fixture.path.join(".git");
    let checkout = match cairn_git::SharedRepository::discover(env!("CARGO_MANIFEST_DIR")) {
        Ok(shared) => shared.git_dir().join("index"),
        Err(error) => panic!("opening the checkout: {error}"),
    };
    let index = match std::fs::read(&checkout) {
        Ok(index) => index,
        Err(error) => panic!("reading {}: {error}", checkout.display()),
    };
    if let Err(error) = std::fs::write(dot.join("index"), &index) {
        panic!("writing the index: {error}");
    }
    settle();
    let (handle, mut updates) = opened(&fixture.fixture.path);
    let kept = committed(of, &first);

    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    let before = reads(&handle, &mut updates);
    if let Err(error) = std::fs::write(dot.join("index.lock"), &index)
        .and_then(|()| std::fs::rename(dot.join("index.lock"), dot.join("index")))
    {
        panic!("refreshing the index: {error}");
    }
    assert!(is_text(&file_answer(&handle, &mut updates, &kept)));
    assert_eq!(
        reads(&handle, &mut updates),
        before,
        "a stat-only refresh let the kept answer go"
    );
    drop(handle);
}

/// T7: two identical asks run `git` once each — the change set's listing and the file's
/// patch read — the second answered from what was kept. Over this checkout, whose files
/// are long settled. Caught by: either early return taken out, which reads again.
#[test]
fn an_identical_ask_is_answered_from_what_is_kept() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 40);
    let start = reads(&handle, &mut updates);
    let (of, file) = ids
        .iter()
        .find_map(|id| {
            let of = Comparison::Commit(*id);
            change_set(&handle, &mut updates, of)
                .files
                .into_iter()
                .find(|f| f.status == ChangeStatus::Modified)
                .map(|file| (of, file))
        })
        .unwrap_or_else(|| panic!("none of {ids:?} modified a file"));
    let listed = reads(&handle, &mut updates).1 - start.1;
    change_set(&handle, &mut updates, of);
    assert_eq!(
        reads(&handle, &mut updates).1 - start.1,
        listed,
        "the second change set was listed again"
    );

    let query = committed(of, &file);
    file_answer(&handle, &mut updates, &query);
    let (patches, _) = reads(&handle, &mut updates);
    assert!(
        patches > start.0,
        "the file asked no patch read, so the test decides nothing"
    );
    file_answer(&handle, &mut updates, &query);
    assert_eq!(
        reads(&handle, &mut updates).0,
        patches,
        "the second file diff read again"
    );
    drop(handle);
}

/// T2: a read git fails reaches the window as `DiffFailed`, naming the query. A stub `git`
/// whose `diff-tree` exits 128. Caught by: a failure arm that sends nothing (`Err(_) =>
/// {}`), which leaves the pane waiting for good.
#[test]
fn a_failed_read_is_sent_as_a_failure_naming_its_query() {
    let ids = {
        let (handle, mut updates) = checkout();
        commits(&handle, &mut updates, 1)
    };
    let stub = StubGit::answering(
        "2.45.0",
        "diff-tree",
        "  echo 'fatal: bad object' >&2\n  exit 128",
    );
    let fixture = BorrowedRepository::new(&format!("cairn-diff-failed-{}", std::process::id()));
    fixture.point_main_at(&ids[0].to_string());
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates) = match super::pool::open(
        &fixture.fixture.path,
        &Discovery::new(stub.startup(Some((&home, &runtime)))),
    ) {
        Ok((handle, updates, _)) => (handle, updates),
        Err(error) => panic!("starting the worker: {error}"),
    };
    let of = Comparison::Commit(ids[0]);
    handle.submit(Request::Changes { of });
    let seen = collect_until(&mut updates, |u| {
        matches!(u, Update::DiffFailed { .. } | Update::Changes { .. })
    });
    match seen.last() {
        Some(Update::DiffFailed { query, message }) => {
            assert_eq!(*query, DiffQuery::Changes(of));
            assert!(
                message.contains("128") || message.contains("bad object"),
                "{message}"
            );
        }
        other => panic!("expected the failure, got {other:?}"),
    }
    drop(handle);
}

/// T8: Expand All through the boundary answers every file of the change set, in its order,
/// complete. Caught by: a lane that drops the batch, or answers another comparison's.
#[test]
fn expand_all_answers_every_file_through_the_boundary() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 8);
    let (of, changes) = ids
        .iter()
        .map(|id| {
            let of = Comparison::Commit(*id);
            (of, change_set(&handle, &mut updates, of))
        })
        .find(|(_, changes)| changes.files.len() >= 2)
        .unwrap_or_else(|| panic!("none of {ids:?} changed two files"));
    let options = DiffOptions::default();
    handle.submit(Request::ExpandAll { of, options });
    let seen = collect_until(&mut updates, |u| {
        matches!(u, Update::FileDiffs { .. } | Update::DiffFailed { .. })
    });
    match seen.last() {
        Some(Update::FileDiffs {
            of: answered,
            options: at,
            diffs,
            complete,
        }) => {
            assert_eq!((*answered, *at, *complete), (of, options, true));
            let files: Vec<&ChangedFile> = diffs.iter().map(|d| &d.file).collect();
            assert_eq!(files, changes.files.iter().collect::<Vec<_>>());
        }
        other => panic!("expected Expand All's files, got {other:?}"),
    }
    drop(handle);
}

/// T8: a working-tree file diff through the boundary answers the working tree as it is:
/// an untracked file's lines, and nothing for its unstaged side, which git does not list.
/// Caught by: a working-tree query routed to the commit path, or answered for another
/// side.
#[test]
fn a_working_tree_diff_answers_through_the_boundary() {
    let ids = {
        let (handle, mut updates) = checkout();
        commits(&handle, &mut updates, 1)
    };
    let fixture = BorrowedRepository::new(&format!("cairn-diff-worktree-{}", std::process::id()));
    fixture.point_main_at(&ids[0].to_string());
    write(&fixture.fixture.path.join("new.txt"), "hello\n");
    let (handle, mut updates) = opened(&fixture.fixture.path);
    let side = |side| FileQuery {
        target: FileTarget::WorkingTree {
            path: RepoPath::from("new.txt"),
            side,
        },
        options: DiffOptions::default(),
    };
    let untracked = file_answer(&handle, &mut updates, &side(WorkingSide::Untracked));
    match untracked.as_ref().and_then(FileDiff::text) {
        Some(text) => assert_eq!(text.new_content(), b"hello\n"),
        None => panic!("the untracked file was not answered as text: {untracked:?}"),
    }
    assert_eq!(
        file_answer(&handle, &mut updates, &side(WorkingSide::Unstaged)),
        None
    );
    drop(handle);
}

/// R2: a change set handed back to the worker is freed there without an answer, and the
/// repository thread goes on serving: the next request asked after it is answered first.
/// Caught by: a retirement answered with an update, or one that ends the thread.
#[test]
fn a_retired_change_set_is_freed_on_the_worker_without_an_answer() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let of = Comparison::Commit(ids[1]);
    let kept = change_set(&handle, &mut updates, of);
    let retired =
        super::request::Retired::of(Some(std::sync::Arc::new(kept)), Vec::new(), Vec::new())
            .unwrap_or_else(|| unreachable!("a change set is something to retire"));

    handle.submit(Request::Retire(retired));
    handle.submit(Request::ListRemotes);
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Remotes { .. }));
    assert_eq!(
        seen.len(),
        1,
        "the retirement was answered, or the thread stopped serving: {seen:?}"
    );
}

/// Phase 06: the context the views open at is the user's `diff.context`, read on the
/// repository thread and answered as a value; a value git refuses answers nothing, and the
/// thread goes on serving. Caught by: a fixed three, or an answer for a refused value.
#[test]
fn the_configured_context_is_answered_through_the_boundary() {
    let fixture = BorrowedRepository::new(&format!("cairn-diff-context-{}", std::process::id()));
    let config = fixture.fixture.path.join(".git").join("config");
    append(&config, "[diff]\n\tcontext = 5\n");
    let (handle, mut updates) = opened(&fixture.fixture.path);
    handle.submit(Request::ConfiguredContext);
    let seen = collect_until(&mut updates, |u| {
        matches!(u, Update::ConfiguredContext { .. })
    });
    assert_eq!(
        seen.last(),
        Some(&Update::ConfiguredContext {
            context: cairn_model::Context::Lines(5)
        })
    );
    drop(handle);

    append(&config, "[diff]\n\tcontext = abc\n");
    let (handle, mut updates) = opened(&fixture.fixture.path);
    handle.submit(Request::ConfiguredContext);
    handle.submit(Request::ListRemotes);
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Remotes { .. }));
    assert!(
        !seen
            .iter()
            .any(|u| matches!(u, Update::ConfiguredContext { .. })),
        "a refused diff.context was answered: {seen:?}"
    );
    drop(handle);
}

/// Phase 07, R5.4: the Changes tab's filter is answered on a worker, through the real
/// boundary, with the indices of the files whose path holds the text — in order, a rename by
/// either name — and a filter superseded by the next keystroke is not answered, while a
/// filter supersedes no diff. Here a change set of 55,184 files (the largest subject's
/// count) is filtered twice in a row; only the second is answered, and with every match.
/// Caught by: the filter run on the caller's thread, a superseded text answered (the list
/// flickers through every keystroke), or the filter lane superseding a diff.
#[test]
fn a_filter_is_answered_on_a_worker_and_a_newer_one_supersedes_it() {
    use std::sync::Arc;

    let (handle, mut updates) = checkout();
    let of = Comparison::Commit(commits(&handle, &mut updates, 1)[0]);
    let files: Vec<ChangedFile> = (0..55_184)
        .map(|n| {
            let path = format!("dir{}/file-{n:05}.rs", n % 13);
            ChangedFile {
                status: ChangeStatus::Modified,
                old_path: RepoPath::from(path.as_str()),
                new_path: RepoPath::from(path.as_str()),
                old_mode: None,
                new_mode: None,
                old_id: None,
                new_id: None,
            }
        })
        .collect();
    let set = Arc::new(ChangeSet {
        files,
        details: None,
        renames: cairn_model::RenameDetection::default(),
    });
    let expected: Vec<u32> = (0..55_184u32).filter(|n| n % 13 == 7).collect();

    let changes = handle.submit(Request::Changes { of });
    handle.submit(Request::FilterFiles {
        of,
        files: Arc::clone(&set),
        text: "DIR".to_owned(),
    });
    handle.submit(Request::FilterFiles {
        of,
        files: Arc::clone(&set),
        text: "dir7/".to_owned(),
    });
    // Until both the filter and the changes query asked before it have answered.
    let (mut filtered, mut changed) = (false, false);
    let seen = collect_until(&mut updates, |u| {
        filtered |= matches!(u, Update::FilteredFiles { text, .. } if text == "dir7/");
        changed |= answers_changes(u, of);
        filtered && changed
    });
    assert!(
        !seen
            .iter()
            .any(|u| matches!(u, Update::FilteredFiles { text, .. } if text == "DIR")),
        "a superseded filter was answered: {:?}",
        seen.len()
    );
    let answer = seen.iter().find_map(|u| match u {
        Update::FilteredFiles {
            of: answered,
            files,
            ..
        } => Some((*answered, files)),
        _ => None,
    });
    assert_eq!(answer, Some((of, &expected)));
    // The changes query asked before the filters was answered: no lane but its own
    // supersedes it.
    assert!(changes.is_some());
}
