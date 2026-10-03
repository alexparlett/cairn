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
