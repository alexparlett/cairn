//! C20, Show Lost Commits' half (`docs/prd/staging-and-commit.md` R11.1, R11.2): a walk from
//! every ref and from the old and the new id of every entry of `HEAD`'s and each local
//! branch's reflog draws every commit git's `rev-list --reflog --branches --remotes --tags
//! HEAD` lists, and draws as lost exactly those `git rev-list <every id the log files hold>
//! --not --branches --remotes --tags HEAD` lists — the ids read out of the files by this
//! test, and again by git itself (`--reflog`), on a fixture with an amend whose log the amend
//! created (the replaced commit only an entry's old id), an amend, a reset-away commit, a
//! commit with a subject past 4 KiB in the middle of the log, a deleted branch's tip, a
//! commit only a lost commit reaches, and a commit a tag reaches that is in a reflog too.
//! Every walk is paged to its end, held and cold, at several page sizes and windows.

mod fixtures;

use std::collections::BTreeSet;

use cairn_git::{CancelSignal, HistoryRequest, Repository};
use cairn_model::{History, RowContent, RowId};

use fixtures::Fixture;

fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

/// `git` in the fixture, dated `at`.
fn git_at(fixture: &Fixture, at: i64, args: &[&str]) -> String {
    fixtures::run(fixture.path(), args, Some(at))
}

struct Clock(i64);

impl Clock {
    fn tick(&mut self) -> i64 {
        self.0 += 60;
        self.0
    }
}

fn commit(fixture: &Fixture, clock: &mut Clock, text: &str, message: &str) -> String {
    ok(
        std::fs::write(fixture.path().join("f"), text),
        "writing a file",
    );
    let at = clock.tick();
    git_at(fixture, at, &["add", "--all"]);
    git_at(fixture, at, &["commit", "--quiet", "-m", message]);
    rev_parse(fixture, "HEAD")
}

fn rev_parse(fixture: &Fixture, name: &str) -> String {
    fixture.git(&["rev-parse", name]).trim().to_owned()
}

fn lines(text: &str) -> BTreeSet<String> {
    text.lines().map(str::to_owned).collect()
}

/// Every old and new id of every entry of `HEAD`'s log and each local branch's, read out of
/// the files as bytes — no line skipped for its length — the null id left out.
fn ids_in_the_log_files(fixture: &Fixture) -> BTreeSet<String> {
    let git_dir = fixture.path().join(".git");
    let mut files = vec![git_dir.join("logs").join("HEAD")];
    for branch in fixture
        .git(&["for-each-ref", "--format=%(refname)", "refs/heads"])
        .lines()
    {
        files.push(git_dir.join("logs").join(branch));
    }
    let mut ids = BTreeSet::new();
    for file in files {
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        for line in bytes.split(|byte| *byte == b'\n') {
            let line = String::from_utf8_lossy(line);
            for id in line.split(' ').take(2) {
                if id.len() == 40 && id.bytes().any(|byte| byte != b'0') {
                    ids.insert(id.to_owned());
                }
            }
        }
    }
    ids
}

/// C20's oracle: `git rev-list <ids> --not --branches --remotes --tags HEAD`.
fn lost_by_git(fixture: &Fixture, ids: &BTreeSet<String>) -> BTreeSet<String> {
    let mut args: Vec<&str> = vec!["rev-list"];
    args.extend(ids.iter().map(String::as_str));
    args.extend(["--not", "--branches", "--remotes", "--tags", "HEAD"]);
    lines(&fixture.git(&args))
}

/// Pages a held session to its end, `page` rows at a time, as the window holds them.
fn held(repo: &Repository, request: &HistoryRequest, page: usize) -> History {
    let mut session = ok(repo.history_session(request), "opening the session");
    let mut rows = History::new();
    loop {
        let next = ok(session.next_page(page, &CancelSignal::new()), "paging");
        ok(rows.append(next.rows), "holding a page");
        if next.cursor.is_none() {
            return rows;
        }
    }
}

/// Pages the cold route to its end, each page a fresh walk resumed from the last cursor.
fn cold(repo: &Repository, request: HistoryRequest, page: usize) -> History {
    let mut rows = History::new();
    let mut next = ok(repo.history(&request, &CancelSignal::new()), "a cold page");
    loop {
        let cursor = next.cursor.take();
        ok(rows.append(next.rows), "holding a page");
        let Some(cursor) = cursor else {
            return rows;
        };
        next = ok(
            repo.history(&HistoryRequest::resume(cursor, page), &CancelSignal::new()),
            "a cold page",
        );
    }
}

/// Every commit a history draws, and those it draws as lost. No wildcard arm.
fn drawn(rows: &History) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut all = BTreeSet::new();
    let mut lost = BTreeSet::new();
    for row in rows.rows() {
        match row.content() {
            RowContent::Commit(commit) => {
                all.insert(commit.id.to_string());
                if row.is_lost() {
                    lost.insert(commit.id.to_string());
                }
            }
            RowContent::Stash(stash) => {
                assert!(!row.is_lost(), "stash {} drawn as lost", stash.id);
            }
        }
    }
    (all, lost)
}

fn request(repo: &Repository, page: usize, window: usize) -> HistoryRequest {
    let read = ok(repo.refs(&CancelSignal::new()), "reading the refs");
    HistoryRequest::from_refs(&read.snapshot, page)
        .with_window(window)
        .with_lost_commits()
}

/// Every route, page size and window draws `all` and, as lost, `lost`.
fn assert_every_route_draws(fixture: &Fixture, all: &BTreeSet<String>, lost: &BTreeSet<String>) {
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    for window in [1, 3, 1024] {
        for page in [1, 2, 5, 64] {
            for (route, rows) in [
                ("held", held(&repo, &request(&repo, page, window), page)),
                ("cold", cold(&repo, request(&repo, page, window), page)),
            ] {
                let (drawn_all, drawn_lost) = drawn(&rows);
                assert_eq!(
                    &drawn_all, all,
                    "{route} walk, page {page}, window {window}: the commits drawn"
                );
                assert_eq!(
                    &drawn_lost, lost,
                    "{route} walk, page {page}, window {window}: the commits drawn as lost"
                );
            }
        }
    }
}

/// What the fixture holds, by name: each a commit a test can say is lost or not.
struct Lost {
    fixture: Fixture,
    replaced_in_own_log: String,
    amended: String,
    reset_away: String,
    long_subject: String,
    deleted_branch_tip: String,
    only_through_lost: String,
    tagged: String,
}

fn lost_commits_fixture() -> Lost {
    let fixture = fixtures::unborn();
    let mut clock = Clock(fixtures::EPOCH);
    commit(&fixture, &mut clock, "1\n", "c1");
    let replaced_in_own_log = commit(&fixture, &mut clock, "2\n", "c2");
    // An amend whose log it creates itself: no log before it, so the commit it replaces is
    // only the old id of the one entry it writes (phase 05's measurement).
    ok(
        std::fs::remove_dir_all(fixture.path().join(".git").join("logs")),
        "removing the logs",
    );
    git_at(
        &fixture,
        clock.tick(),
        &["commit", "--quiet", "--amend", "-m", "c2 amended"],
    );
    let amended = commit(&fixture, &mut clock, "3\n", "c3");
    git_at(
        &fixture,
        clock.tick(),
        &["commit", "--quiet", "--amend", "-m", "c3 amended"],
    );
    // A subject past gix's 4 KiB window, reset away, with entries written after it.
    let long_subject = commit(&fixture, &mut clock, "4\n", &"long ".repeat(1_000));
    let reset_away = commit(&fixture, &mut clock, "5\n", "reset away");
    git_at(
        &fixture,
        clock.tick(),
        &["reset", "--quiet", "--hard", "HEAD~2"],
    );
    // A branch's tip, the branch deleted: only `HEAD`'s log still names it.
    git_at(
        &fixture,
        clock.tick(),
        &["checkout", "--quiet", "-b", "topic"],
    );
    let deleted_branch_tip = commit(&fixture, &mut clock, "t\n", "on topic");
    git_at(&fixture, clock.tick(), &["checkout", "--quiet", "main"]);
    git_at(
        &fixture,
        clock.tick(),
        &["branch", "--quiet", "-D", "topic"],
    );
    // Two commits no log names but as one's parent: `HEAD` visits the second and leaves.
    let tree = rev_parse(&fixture, "HEAD^{tree}");
    let main = rev_parse(&fixture, "main");
    let at = clock.tick();
    let only_through_lost = git_at(
        &fixture,
        at,
        &[
            "commit-tree",
            &tree,
            "-p",
            &main,
            "-m",
            "under a lost commit",
        ],
    )
    .trim()
    .to_owned();
    let on_top = git_at(
        &fixture,
        clock.tick(),
        &[
            "commit-tree",
            &tree,
            "-p",
            &only_through_lost,
            "-m",
            "lost tip",
        ],
    )
    .trim()
    .to_owned();
    git_at(
        &fixture,
        clock.tick(),
        &["reset", "--quiet", "--hard", &on_top],
    );
    git_at(
        &fixture,
        clock.tick(),
        &["reset", "--quiet", "--hard", &main],
    );
    // A commit in the log that a tag reaches: never lost.
    let tagged = commit(&fixture, &mut clock, "6\n", "tagged");
    git_at(&fixture, clock.tick(), &["tag", "kept"]);
    git_at(
        &fixture,
        clock.tick(),
        &["reset", "--quiet", "--hard", "HEAD~1"],
    );
    commit(&fixture, &mut clock, "7\n", "main's tip");
    Lost {
        fixture,
        replaced_in_own_log,
        amended,
        reset_away,
        long_subject,
        deleted_branch_tip,
        only_through_lost,
        tagged,
    }
}

/// C20: what Show Lost Commits draws is git's answer, read from the files and by git itself,
/// on every route. Caught by: new ids alone (the replaced commit of an amend that created
/// its log is missed), a log read through gix's newest-first reader (it stops at the long
/// subject, and the entries older than it are missed), a lost commit's parent left undrawn,
/// a tagged commit dimmed, or a reached commit drawn as lost.
#[test]
fn show_lost_commits_draws_what_git_reads_from_every_reflog_entry_and_dims_what_no_ref_reaches() {
    let lost = lost_commits_fixture();
    let fixture = &lost.fixture;
    let log = std::fs::read(fixture.path().join(".git/logs/HEAD")).unwrap_or_default();
    assert!(
        log.split(|byte| *byte == b'\n')
            .rev()
            .skip(2)
            .any(|line| line.len() > 4096),
        "the fixture's HEAD log holds a line past 4 KiB before its last entries"
    );

    let ids = ids_in_the_log_files(fixture);
    assert!(
        ids.contains(&lost.replaced_in_own_log),
        "the replaced commit is in the log, as an old id"
    );
    let expected_lost = lost_by_git(fixture, &ids);
    // The same, as git reads every reflog itself (only `HEAD` and the branches log here).
    let by_git_reflog = lines(&fixture.git(&[
        "rev-list",
        "--reflog",
        "--not",
        "--branches",
        "--remotes",
        "--tags",
        "HEAD",
    ]));
    assert_eq!(
        expected_lost, by_git_reflog,
        "git reads the files as this test does"
    );
    let expected_all = lines(&fixture.git(&[
        "rev-list",
        "--reflog",
        "--branches",
        "--remotes",
        "--tags",
        "HEAD",
    ]));

    for (what, commit, is_lost) in [
        (
            "the commit an amend replaced in its own log",
            &lost.replaced_in_own_log,
            true,
        ),
        ("an amended commit", &lost.amended, true),
        ("a reset-away commit", &lost.reset_away, true),
        (
            "a commit with a subject past 4 KiB",
            &lost.long_subject,
            true,
        ),
        ("a deleted branch's tip", &lost.deleted_branch_tip, true),
        (
            "a commit only a lost commit reaches",
            &lost.only_through_lost,
            true,
        ),
        ("a commit a tag reaches", &lost.tagged, false),
    ] {
        assert_eq!(expected_lost.contains(commit), is_lost, "git: {what}");
        assert!(expected_all.contains(commit), "git draws {what}");
    }

    assert_every_route_draws(fixture, &expected_all, &expected_lost);

    // Without the toggle, nothing is lost and nothing only a reflog reaches is drawn.
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    let read = ok(repo.refs(&CancelSignal::new()), "reading the refs");
    let (plain, plain_lost) = drawn(&held(
        &repo,
        &HistoryRequest::from_refs(&read.snapshot, 64),
        64,
    ));
    assert!(plain_lost.is_empty());
    assert_eq!(
        plain,
        lines(&fixture.git(&["rev-list", "--branches", "--remotes", "--tags", "HEAD"]))
    );
}

/// A parent dated newer than its child, which a ref reaches only through that child, comes
/// off the walk first, through a lost commit: it is taken for lost, and once the child comes
/// off it is reached after all — so it is drawn as any other row however the pages and the
/// window fall, as git's own answer says. Caught by: a row carried as lost never corrected,
/// or a correction applied to the wrong row.
#[test]
fn a_parent_dated_after_its_reached_child_is_not_drawn_as_lost() {
    let fixture = fixtures::unborn();
    let mut clock = Clock(fixtures::EPOCH);
    commit(&fixture, &mut clock, "1\n", "root");
    let tree = rev_parse(&fixture, "HEAD^{tree}");
    let root = rev_parse(&fixture, "HEAD");
    let base = fixtures::EPOCH + 10_000;
    // `skewed`, dated newer than `child`, its child that `main` holds.
    let skewed = git_at(
        &fixture,
        base + 500,
        &["commit-tree", &tree, "-p", &root, "-m", "skewed"],
    )
    .trim()
    .to_owned();
    let child = git_at(
        &fixture,
        base + 100,
        &["commit-tree", &tree, "-p", &skewed, "-m", "child"],
    )
    .trim()
    .to_owned();
    // `lost`, newest of all, on `skewed`: visited by `HEAD`, then left.
    let lost = git_at(
        &fixture,
        base + 900,
        &["commit-tree", &tree, "-p", &skewed, "-m", "lost"],
    )
    .trim()
    .to_owned();
    git_at(&fixture, base + 901, &["reset", "--quiet", "--hard", &lost]);
    git_at(
        &fixture,
        base + 902,
        &["reset", "--quiet", "--hard", &child],
    );

    let ids = ids_in_the_log_files(&fixture);
    let expected_lost = lost_by_git(&fixture, &ids);
    assert_eq!(
        expected_lost,
        BTreeSet::from([lost.clone()]),
        "git's answer"
    );
    let expected_all = lines(&fixture.git(&[
        "rev-list",
        "--reflog",
        "--branches",
        "--remotes",
        "--tags",
        "HEAD",
    ]));
    assert!(expected_all.contains(&skewed));
    assert_every_route_draws(&fixture, &expected_all, &expected_lost);

    // The skew is real: the walk takes `skewed` before `child`.
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    let rows = held(&repo, &request(&repo, 64, 1024), 64);
    let at = |id: &str| {
        rows.rows()
            .position(|row| matches!(row.id(), RowId::Commit(commit) if commit.to_string() == id))
    };
    assert!(
        at(&skewed) < at(&child),
        "the fixture's parent comes off first"
    );
}

/// Every row's identity, lane and edges, as the list draws them.
fn laid_out(rows: &History) -> Vec<String> {
    rows.rows()
        .map(|row| format!("{:?} {:?} {:?}", row.id(), row.lane(), row.edges()))
        .collect()
}

/// A braided history every reflog entry of which a ref reaches: the toggle draws exactly
/// the rows, lanes and edges the walk without it draws, none lost, on both routes — so
/// the compact and slim rows' equivalences (refs-and-status C15, C16), pinned on the walk
/// without it, hold with it on. Caught by: a reflog tip reordering the walk, a row drawn
/// as lost that a ref reaches, or the toggle changing what a row keeps.
#[test]
fn where_every_reflog_entry_is_reached_the_toggle_draws_the_same_rows() {
    let fixture = fixtures::braided(14);
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    assert!(
        !ids_in_the_log_files(&fixture).is_empty(),
        "the fixture's commits are logged"
    );
    let read = ok(repo.refs(&CancelSignal::new()), "reading the refs");
    for page in [1, 7, 64] {
        let plain = HistoryRequest::from_refs(&read.snapshot, page).with_window(3);
        let with = plain.clone().with_lost_commits();
        let (on, off) = (held(&repo, &with, page), held(&repo, &plain, page));
        assert_eq!(laid_out(&on), laid_out(&off), "held, page {page}");
        assert_eq!(drawn(&on).1, BTreeSet::new(), "nothing is lost");
        let (cold_on, cold_off) = (cold(&repo, with, page), cold(&repo, plain, page));
        assert_eq!(laid_out(&cold_on), laid_out(&cold_off), "cold, page {page}");
        assert_eq!(
            laid_out(&cold_on),
            laid_out(&on),
            "cold and held, page {page}"
        );
    }
    let head: BTreeSet<String> = fixture.rev_list().into_iter().collect();
    assert!(head.is_subset(&drawn(&held(&repo, &request(&repo, 64, 1024), 64)).0));
}

/// Phase 10's decision A: with a commit-graph holding some of the tips — the reachable ones
/// and some reflog-only ones, not all — Show Lost Commits draws the same rows, lanes, edges and
/// lost marks in the same order as from the objects alone, on both routes: a date taken from
/// the graph is the commit's own, as git's walk takes it. Caught by: a graph date read wrong (a
/// tip ordered elsewhere), or a reflog tip the graph does not hold left undated or dropped.
#[test]
fn a_commit_graph_dates_the_tips_as_the_objects_do() {
    let lost = lost_commits_fixture();
    let fixture = &lost.fixture;
    let read = |fixture: &Fixture| {
        let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
        let mut drawn = Vec::new();
        for page in [1, 3, 64] {
            let request = request(&repo, page, 3);
            drawn.push(laid_out_lost(&held(&repo, &request, page)));
            drawn.push(laid_out_lost(&cold(&repo, request, page)));
        }
        drawn
    };
    let without = read(fixture);
    // The reachable commits, and two of the reflog's lost ones: not every tip is in the graph.
    let some_lost = format!("{}\n{}\n", lost.amended, lost.reset_away);
    fixture.git(&["commit-graph", "write", "--reachable"]);
    let graph = fixture.path().join(".git/objects/info/commit-graph");
    assert!(graph.exists(), "git wrote no commit-graph");
    let mut writer = std::process::Command::new("git")
        .args(["commit-graph", "write", "--stdin-commits", "--append"])
        .current_dir(fixture.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write as _;
    writer
        .stdin
        .take()
        .unwrap()
        .write_all(some_lost.as_bytes())
        .unwrap();
    assert!(writer.wait().unwrap().success());
    assert_eq!(read(fixture), without, "the graph changed what is drawn");
}

/// [`laid_out`], with each row's lost mark.
fn laid_out_lost(rows: &History) -> Vec<String> {
    rows.rows()
        .map(|row| {
            format!(
                "{:?} {:?} {:?} lost={}",
                row.id(),
                row.lane(),
                row.edges(),
                row.is_lost()
            )
        })
        .collect()
}

// --- The reporter: Show Lost Commits' first page (phase 10's stopping rule, C21) ---

/// The first page of 64 rows from every ref — the snapshot already read, each run a fresh
/// open — with Show Lost Commits off and on, on the repository `CAIRN_BENCH_REPO` names (read
/// only: a plain tmpfs clone of the bench, never the bench itself), warm, median of seven
/// after a warm-up. Prints how many rows each page drew as lost and how many commits it pulled
/// off the walk. Run with `cargo test --release -p cairn-git --test lost_commits -- --ignored
/// --nocapture`.
#[test]
#[ignore = "needs a repository named by CAIRN_BENCH_REPO; run with --release"]
fn measures_the_first_page_with_show_lost_commits() {
    use std::time::{Duration, Instant};

    const PAGE: usize = 64;
    const RUNS: usize = 7;
    let path = std::env::var("CAIRN_BENCH_REPO").unwrap_or_else(|_| panic!("set CAIRN_BENCH_REPO"));
    for lost in [false, true] {
        let mut samples: Vec<Duration> = Vec::with_capacity(RUNS);
        let mut shape = (0, 0, 0);
        for run in 0..=RUNS {
            let repo = ok(Repository::discover(&path), "opening the repository");
            let snapshot = ok(repo.refs(&CancelSignal::new()), "reading the refs").snapshot;
            let started = Instant::now();
            let request = HistoryRequest::from_refs(&snapshot, PAGE);
            let request = if lost {
                request.with_lost_commits()
            } else {
                request
            };
            let mut session = ok(repo.history_session(&request), "opening the session");
            let page = ok(
                session.next_page(PAGE, &CancelSignal::new()),
                "the first page",
            );
            let elapsed = started.elapsed();
            let dimmed = (0..page.rows.len())
                .filter(|&at| page.rows.is_lost(at))
                .count();
            shape = (page.rows.len(), dimmed, session.commits_walked());
            if run > 0 {
                samples.push(elapsed);
            }
        }
        samples.sort_unstable();
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        let (rows, dimmed, pulled) = shape;
        eprintln!(
            "FIRST PAGE lost={lost} median={:.2} ms [{:.2}-{:.2}] rows={rows} lost_rows={dimmed} \
             commits_pulled={pulled}",
            ms(samples[RUNS / 2]),
            ms(samples[0]),
            ms(samples[RUNS - 1]),
        );
    }

    // The whole walk, once each way, paged as a scroll pages it: what tracking reachability
    // costs past the first page, while a reflog tip deep in the history keeps it on.
    for lost in [false, true] {
        let repo = ok(Repository::discover(&path), "opening the repository");
        let snapshot = ok(repo.refs(&CancelSignal::new()), "reading the refs").snapshot;
        let started = Instant::now();
        let request = HistoryRequest::from_refs(&snapshot, PAGE);
        let request = if lost {
            request.with_lost_commits()
        } else {
            request
        };
        let mut session = ok(repo.history_session(&request), "opening the session");
        let mut rows = 0;
        loop {
            let page = ok(session.next_page(1_000, &CancelSignal::new()), "a page");
            rows += page.rows.len();
            if page.cursor.is_none() {
                break;
            }
        }
        eprintln!(
            "WHOLE WALK lost={lost} {:.0} ms rows={rows}",
            started.elapsed().as_secs_f64() * 1e3
        );
    }
}
