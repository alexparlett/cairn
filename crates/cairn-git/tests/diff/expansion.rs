//! Expand All bounded (phase 08, R5.3, the obligation phase 02's QA left it): a page of a
//! change set's files read in order, each admitted or refused by the line budget BEFORE its
//! blobs are read, git asked about the page's paths alone, and each file's failure its own
//! outcome beside the others.

use cairn_git::{
    CancelSignal, ChangesRequest, ContentOptions, LineBudget, Offered, PAGE_FILES, PAGE_LINES,
    Page, SharedRepository,
};
use cairn_model::{ChangeSet, FileDiff};

use super::repositories::Repo;
use super::{every_file, ok};

/// `lines` numbered lines, the `edited`-th of them changed when it is `Some`.
fn numbered(lines: usize, edited: Option<usize>) -> Vec<u8> {
    let mut out = Vec::new();
    for line in 0..lines {
        let mark = if edited == Some(line) { " edited" } else { "" };
        out.extend_from_slice(format!("line {line}{mark}\n").as_bytes());
    }
    out
}

/// Five files of ten lines, each with one line edited in the last commit: each costs 21 —
/// ten lines a side and one for the file — and `f3.txt`'s new blob is removed from the
/// object database, so a page that reads it fails it, and a `git diff-tree -p` asked about
/// its path fails outright.
fn five_files() -> (Repo, String) {
    let repo = Repo::new("pages");
    for n in 0..5 {
        repo.write(&format!("f{n}.txt"), &numbered(10, None));
    }
    repo.commit("seed");
    for n in 0..5 {
        repo.write(&format!("f{n}.txt"), &numbered(10, Some(n)));
    }
    repo.commit("edit every file");
    let id = repo.git(&["rev-parse", "HEAD:f3.txt"]).trim().to_owned();
    let loose = repo
        .path()
        .join(".git/objects")
        .join(&id[..2])
        .join(&id[2..]);
    if let Err(error) = std::fs::remove_file(&loose) {
        panic!("removing {}: {error}", loose.display());
    }
    (repo, "f3.txt".to_owned())
}

struct Opened {
    shared: SharedRepository,
}

impl Opened {
    fn new(repo: &Repo) -> Self {
        Self {
            shared: ok(SharedRepository::discover(repo.path()), "the fixture opens"),
        }
    }

    /// Every `git` invocation that named `path` as an argument.
    fn runs_naming(&self, path: &str) -> usize {
        self.shared
            .command_log()
            .iter()
            .filter(|record| record.arguments.iter().any(|argument| argument == path))
            .count()
    }
}

fn changes(session: &mut cairn_git::DiffSession<'_>, request: &ChangesRequest) -> ChangeSet {
    ok(
        session.changes(super::git(), request, &CancelSignal::new()),
        "the changes query answers",
    )
}

fn page(
    session: &mut cairn_git::DiffSession<'_>,
    request: &ChangesRequest,
    (set, offered): (&ChangeSet, &[usize]),
    budget: Option<&mut LineBudget>,
) -> Page {
    ok(
        session.page(
            super::git(),
            request,
            Offered {
                changes: set,
                files: offered,
            },
            budget,
            &ContentOptions::default(),
            &CancelSignal::new(),
        ),
        "a page answers",
    )
}

fn read_paths(page: &Page, set: &ChangeSet) -> Vec<String> {
    page.files
        .iter()
        .map(|(index, _)| set.files[*index].new_path.to_string())
        .collect()
}

/// R5.3 and phase 02 QA's R2: Expand All opens files in the change set's order until its
/// line budget is spent, and decides each file before reading it — the file after the one
/// that spent the budget is never read, and git is never asked about it. Each file here costs
/// 21; a budget of 50 is not spent after two (42) and is after three (63), so three are read
/// and the fourth, whose blob is gone, is not touched. Taken up again with the budget spent,
/// nothing more is read. Caught by: the budget checked after a file is read (the fourth
/// read, and failed), a file charged nothing (every file read), a page asking git about the
/// whole comparison (the missing blob named on its command line), or the count of files
/// taken off by one.
#[test]
fn expand_all_stops_at_its_budget_before_reading_the_next_file() {
    let (repo, missing) = five_files();
    let opened = Opened::new(&repo);
    let engine = opened.shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let request = ChangesRequest::commit(repo.rev("HEAD"));
    let set = changes(&mut session, &request);
    let offered: Vec<usize> = (0..set.files.len()).collect();
    assert_eq!(set.files.len(), 5);

    let mut budget = LineBudget::new(50);
    let read = page(&mut session, &request, (&set, &offered), Some(&mut budget));
    assert_eq!(read_paths(&read, &set), ["f0.txt", "f1.txt", "f2.txt"]);
    assert_eq!(
        read.taken, 3,
        "the page did not stop where the budget was spent"
    );
    assert_eq!(budget.spent(), 63, "each file costs its lines and one");
    assert!(budget.is_spent());
    for (index, outcome) in &read.files {
        let diff = match outcome {
            Ok(diff) => diff,
            Err(error) => panic!("{}: {error}", set.files[*index].new_path),
        };
        let alone = ok(
            session.file_diff(
                super::git(),
                &request,
                &set.files[*index],
                &ContentOptions::default(),
                &CancelSignal::new(),
            ),
            "the file alone",
        );
        assert_eq!(*diff, alone, "a page's answer is the file's answer alone");
    }
    assert_eq!(
        opened.runs_naming(&missing),
        0,
        "git was asked about a file the budget refused"
    );

    let again = page(
        &mut session,
        &request,
        (&set, &offered[3..]),
        Some(&mut budget),
    );
    assert!(again.files.is_empty() && again.taken == 0, "{again:?}");

    // Resumed where a superseded Expand All stopped: two files in, 42 spent.
    let mut resumed = LineBudget::resumed(50, 42);
    let read = page(
        &mut session,
        &request,
        (&set, &offered[2..]),
        Some(&mut resumed),
    );
    assert_eq!(read_paths(&read, &set), ["f2.txt"]);
    assert_eq!(resumed.spent(), 63);
}

/// Phase 04 QA's R1: a file that cannot be read is that file's outcome, shown on that file,
/// and every other file of the page is read and answered as it is alone. `f3.txt`'s blob is
/// gone; asked with no budget, the page answers the other four. Caught by: one failure
/// failing the page (the old all-or-nothing Expand All), or a failure dropped (a file that
/// silently vanishes from the expansion).
#[test]
fn a_failing_file_is_that_files_outcome_and_the_rest_are_read() {
    let (repo, missing) = five_files();
    let opened = Opened::new(&repo);
    let engine = opened.shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let request = ChangesRequest::commit(repo.rev("HEAD"));
    let set = changes(&mut session, &request);
    let offered: Vec<usize> = (0..set.files.len()).collect();

    let read = page(&mut session, &request, (&set, &offered), None);
    assert_eq!(read.taken, 5);
    assert_eq!(
        read_paths(&read, &set),
        ["f0.txt", "f1.txt", "f2.txt", "f3.txt", "f4.txt"],
        "every file offered has an outcome, in order"
    );
    for (index, outcome) in &read.files {
        let path = set.files[*index].new_path.to_string();
        match (path == missing, outcome) {
            (true, Err(_)) | (false, Ok(FileDiff { .. })) => {}
            (true, Ok(diff)) => panic!("a file with no blob was answered: {diff:?}"),
            (false, Err(error)) => panic!("{path} failed beside the broken file: {error}"),
        }
    }
}

/// A page is bounded whatever the budget: at most `PAGE_FILES` files, and it ends at the file
/// whose lines cross `PAGE_LINES`, so a comparison of any size is read a page at a time and
/// the next page is offered from where the last stopped. Caught by: a page that reads every
/// file offered (the memory is the comparison's again), or a `taken` that does not say
/// where the next page starts (a file read twice, or skipped).
#[test]
fn a_page_ends_at_its_file_count_or_its_line_count() {
    let repo = Repo::new("page-caps");
    let files = PAGE_FILES + 7;
    for n in 0..files {
        repo.write(&format!("many/{n:04}.txt"), b"one\n");
    }
    let long = usize::try_from(PAGE_LINES).unwrap_or(usize::MAX) / 2 + 10;
    repo.write("long/a.txt", &numbered(long, None));
    repo.write("long/b.txt", &numbered(long, None));
    repo.commit("seed");
    for n in 0..files {
        repo.write(&format!("many/{n:04}.txt"), b"two\n");
    }
    repo.write("long/a.txt", &numbered(long, Some(3)));
    repo.write("long/b.txt", &numbered(long, Some(4)));
    repo.commit("edit");

    let opened = Opened::new(&repo);
    let engine = opened.shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let request = ChangesRequest::commit(repo.rev("HEAD"));
    let set = changes(&mut session, &request);
    let many: Vec<usize> = (0..set.files.len())
        .filter(|index| set.files[*index].new_path.to_string().starts_with("many/"))
        .collect();
    assert_eq!(many.len(), files);

    let first = page(&mut session, &request, (&set, &many), None);
    assert_eq!(first.taken, PAGE_FILES);
    assert_eq!(first.files.len(), PAGE_FILES);
    let second = page(&mut session, &request, (&set, &many[first.taken..]), None);
    assert_eq!(second.taken, 7);
    let mut paths = read_paths(&first, &set);
    paths.extend(read_paths(&second, &set));
    let expected: Vec<String> = many
        .iter()
        .map(|index| set.files[*index].new_path.to_string())
        .collect();
    assert_eq!(paths, expected, "the pages read every file once, in order");

    let long_files: Vec<usize> = (0..set.files.len())
        .filter(|index| set.files[*index].new_path.to_string().starts_with("long/"))
        .collect();
    let crossing = page(&mut session, &request, (&set, &long_files), None);
    assert_eq!(
        read_paths(&crossing, &set),
        ["long/a.txt"],
        "the page did not end at the file whose lines crossed its limit"
    );
    assert_eq!(crossing.taken, 1);

    // The whole comparison, page after page, is every file's answer alone.
    let all = ok(
        every_file(
            &mut session,
            &request,
            &set,
            &ContentOptions::default(),
            &CancelSignal::new(),
        ),
        "every page answers",
    );
    assert_eq!(all.len(), set.files.len());
}

/// Phase 08 QA's E4: a file git itself fails on — here its diff driver's `xfuncname` is a
/// regular expression git refuses, which git finds only as it diffs that path and dies on
/// (`fatal: Invalid regexp to look for hunk header`), ending the whole run — fails alone: the
/// page's one `diff-tree -p` over the three paths fails, each file is then asked on its own,
/// and the other two are answered as they are alone. Caught by: a failed grouped run returned
/// as the page's failure (every file lost to one), or the files beside it never asked again.
#[test]
fn a_file_git_fails_on_in_a_grouped_run_fails_alone() {
    let repo = Repo::new("grouped-failure");
    repo.write(".gitattributes", b"f1.txt diff=broken\n");
    for n in 0..3 {
        repo.write(&format!("f{n}.txt"), &numbered(10, None));
    }
    repo.commit("seed");
    for n in 0..3 {
        repo.write(&format!("f{n}.txt"), &numbered(10, Some(n)));
    }
    repo.commit("edit every file");
    repo.config("diff.broken.xfuncname", "[");

    let opened = Opened::new(&repo);
    let engine = opened.shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let request = ChangesRequest::commit(repo.rev("HEAD"));
    let set = changes(&mut session, &request);
    assert_eq!(set.files.len(), 3);
    let offered: Vec<usize> = (0..set.files.len()).collect();

    let read = page(&mut session, &request, (&set, &offered), None);
    assert_eq!(read_paths(&read, &set), ["f0.txt", "f1.txt", "f2.txt"]);
    for (index, outcome) in &read.files {
        let file = &set.files[*index];
        let alone = session.file_diff(
            super::git(),
            &request,
            file,
            &ContentOptions::default(),
            &CancelSignal::new(),
        );
        match (file.new_path.to_string().as_str(), outcome, alone) {
            ("f1.txt", Err(_), Err(_)) => {}
            ("f1.txt", Ok(diff), _) => panic!("git refuses f1.txt, yet it was answered: {diff:?}"),
            (path, Ok(diff), Ok(alone)) => assert_eq!(*diff, alone, "{path}"),
            (path, Err(error), _) => panic!("{path} failed beside the file git refuses: {error}"),
            (path, Ok(_), Err(error)) => panic!("{path} alone: {error}"),
        }
    }
    // The run over the page named every path, and failed; each was then asked alone.
    assert!(
        opened.runs_naming("f0.txt") >= 2 && opened.runs_naming("f2.txt") >= 2,
        "the files beside the refused one were not asked in a grouped run and then alone"
    );
}

/// Phase 08 QA's E6: a page asked once its query is already superseded reads nothing — no
/// file charged to its budget, no `git` started — and answers `ContentCancelled`, never a
/// page or a file's failure. Caught by: the cancel checked only after a file is read (the
/// first file charged), or only while git runs (a `diff-tree` started).
#[test]
fn a_page_asked_after_its_cancel_reads_nothing() {
    let (repo, _) = five_files();
    let opened = Opened::new(&repo);
    let engine = opened.shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let request = ChangesRequest::commit(repo.rev("HEAD"));
    let set = changes(&mut session, &request);
    let offered: Vec<usize> = (0..set.files.len()).collect();
    let runs_before = opened.shared.command_log().len();

    let cancel = CancelSignal::new();
    cancel.cancel();
    let mut budget = LineBudget::new(1_000);
    let answer = session.page(
        super::git(),
        &request,
        Offered {
            changes: &set,
            files: &offered,
        },
        Some(&mut budget),
        &ContentOptions::default(),
        &cancel,
    );
    assert!(
        matches!(answer, Err(cairn_git::Error::ContentCancelled)),
        "{answer:?}"
    );
    assert_eq!(budget.spent(), 0, "a file was read after the cancel");
    assert_eq!(
        opened.shared.command_log().len(),
        runs_before,
        "git was started after the cancel"
    );
}

/// Phase 08 QA's E1, the boundaries exactly: a budget is spent when its lines REACH its limit,
/// not past it — resumed at the limit it is spent, one short it is not — and a page ends at the
/// file whose cost brings the page's lines to exactly `PAGE_LINES`, as a budget ends at the
/// file that brings it to exactly its limit. Two files here cost exactly half of `PAGE_LINES`
/// each (5,000 lines, then 4,999, and one for the file). Caught by: either comparison made
/// strict (`>`), which reads the third file onto a full page, or past a spent budget.
#[test]
fn a_page_and_a_budget_end_exactly_at_their_limits() {
    let limit = 20_000u64;
    assert!(LineBudget::resumed(limit, limit).is_spent());
    assert!(!LineBudget::resumed(limit, limit - 1).is_spent());
    assert!(LineBudget::new(0).is_spent());

    let quarter = usize::try_from(PAGE_LINES / 4).unwrap_or(usize::MAX);
    let repo = Repo::new("exact-limits");
    repo.write("exact/a.txt", &numbered(quarter, None));
    repo.write("exact/b.txt", &numbered(quarter, None));
    repo.write("exact/c.txt", &numbered(3, None));
    repo.commit("seed");
    // One line deleted from the end: 5,000 lines old, 4,999 new, one for the file.
    repo.write("exact/a.txt", &numbered(quarter - 1, None));
    repo.write("exact/b.txt", &numbered(quarter - 1, None));
    repo.write("exact/c.txt", &numbered(3, Some(1)));
    repo.commit("edit");

    let opened = Opened::new(&repo);
    let engine = opened.shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let request = ChangesRequest::commit(repo.rev("HEAD"));
    let set = changes(&mut session, &request);
    let offered: Vec<usize> = (0..set.files.len()).collect();
    assert_eq!(set.files.len(), 3);

    let full = page(&mut session, &request, (&set, &offered), None);
    assert_eq!(
        read_paths(&full, &set),
        ["exact/a.txt", "exact/b.txt"],
        "the page did not end at exactly PAGE_LINES"
    );
    assert_eq!(full.taken, 2);

    let mut budget = LineBudget::new(PAGE_LINES / 2);
    let spent = page(&mut session, &request, (&set, &offered), Some(&mut budget));
    assert_eq!(
        budget.spent(),
        PAGE_LINES / 2,
        "a file costs its lines and one"
    );
    assert_eq!(
        read_paths(&spent, &set),
        ["exact/a.txt"],
        "the budget did not end at exactly its limit"
    );
}

/// Phase 08 QA's E2: a page's two bounds are the numbers measured in phase 08 (`progress.md`,
/// C14). Changing either is a measured C14 decision — re-run the engine's sweep and the
/// window check, and record them — never an edit on its own.
#[test]
fn a_pages_bounds_are_the_measured_ones() {
    assert_eq!(PAGE_FILES, 256);
    assert_eq!(PAGE_LINES, 20_000);
}
