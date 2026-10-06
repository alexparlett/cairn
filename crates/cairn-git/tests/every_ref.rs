//! C6: the history walked from every ref, labelled, with stash rows (PRD R4.1-R4.3). Every
//! expectation is read out of the `git` binary, run in the same fixture at test time —
//! `git rev-list --branches --remotes --tags HEAD`, `git log --decorate=full`, `git stash
//! list` — and every walk is paged to its end, so each comparison covers the whole history.

mod fixtures;

use std::collections::{BTreeSet, HashMap};

use cairn_git::{CancelSignal, HistoryOrder, HistoryRequest, Repository};
use cairn_model::{EdgeKind, History, HistoryRow, Lane, RefKind, RowContent, RowId, StashSummary};

use fixtures::Fixture;

fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

/// `git` in the fixture, dated `at` (author and committer alike).
fn git_at(fixture: &Fixture, at: i64, args: &[&str]) -> String {
    fixtures::run(fixture.path(), args, Some(at))
}

fn write(fixture: &Fixture, file: &str, text: &str) {
    ok(
        std::fs::write(fixture.path().join(file), text),
        "writing a file",
    );
}

/// A clock for a fixture: every call a minute later.
struct Clock(i64);

impl Clock {
    fn tick(&mut self) -> i64 {
        self.0 += 60;
        self.0
    }
}

fn commit(fixture: &Fixture, clock: &mut Clock, file: &str, text: &str, message: &str) {
    write(fixture, file, text);
    let at = clock.tick();
    git_at(fixture, at, &["add", "--all"]);
    git_at(fixture, at, &["commit", "--quiet", "-m", message]);
}

fn rev_parse(fixture: &Fixture, name: &str) -> String {
    fixture.git(&["rev-parse", name]).trim().to_owned()
}

fn snapshot_request(repo: &Repository, limit: usize) -> HistoryRequest {
    let read = ok(repo.refs(&CancelSignal::new()), "reading the refs");
    HistoryRequest::from_refs(&read.snapshot, limit)
}

/// Pages a held session to its end, `page` rows at a time, as the window holds them.
fn page_all(repo: &Repository, request: &HistoryRequest, page: usize) -> History {
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
fn cold_all(repo: &Repository, request: HistoryRequest, page: usize) -> History {
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

/// Every row as a line of text: its identity, what it draws and its labels. No wildcard.
fn described(rows: &History) -> Vec<String> {
    rows.rows()
        .map(|row| {
            let labels: Vec<String> = row
                .labels()
                .iter()
                .map(|label| format!("{}:{:?}:{}", label.name, label.kind, label.current))
                .collect();
            let head = row.labels().is_head();
            match row.content() {
                RowContent::Commit(commit) => format!(
                    "commit {} {:?} head={head} {labels:?} parents={}",
                    commit.id, commit.summary, commit.parent_count
                ),
                RowContent::Stash(stash) => format!(
                    "stash {} @{} on {} {:?} head={head} {labels:?}",
                    stash.id, stash.index, stash.base, stash.message
                ),
            }
        })
        .collect()
}

fn stash_of(row: HistoryRow<'_>) -> Option<StashSummary> {
    match row.content() {
        RowContent::Commit(_) => None,
        RowContent::Stash(stash) => Some(stash),
    }
}

/// The rows' commits and stashes by hex id, each with its row index.
fn index(rows: &History) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut commits = HashMap::new();
    let mut stashes = HashMap::new();
    for row in rows.rows() {
        let (into, id) = match row.id() {
            RowId::Commit(id) => (&mut commits, id),
            RowId::Stash(id) => (&mut stashes, id),
        };
        assert!(
            into.insert(id.to_string(), row.index()).is_none(),
            "{:?} has two rows",
            row.id()
        );
    }
    (commits, stashes)
}

/// git's labels for every commit the walk reaches: `%D` under `--decorate=full`, split
/// into the names it lists and whether `HEAD` is there, `refs/stash` aside, with the branch
/// `HEAD ->` names.
#[derive(Debug, Default, PartialEq, Eq)]
struct GitLabels {
    head: bool,
    current: Option<String>,
    names: BTreeSet<String>,
}

fn git_labels(fixture: &Fixture) -> HashMap<String, GitLabels> {
    let log = fixture.git(&[
        "log",
        "--decorate=full",
        "--format=%H%x1f%D",
        "--branches",
        "--remotes",
        "--tags",
        "HEAD",
    ]);
    let mut labels = HashMap::new();
    for line in log.lines() {
        let Some((id, decoration)) = line.split_once('\u{1f}') else {
            continue;
        };
        let mut read = GitLabels::default();
        for item in decoration.split(", ").filter(|item| !item.is_empty()) {
            if let Some(branch) = item.strip_prefix("HEAD -> ") {
                read.head = true;
                read.current = Some(branch.to_owned());
                read.names.insert(branch.to_owned());
            } else if item == "HEAD" {
                read.head = true;
            } else if item == "refs/stash" {
                // `refs/stash` labels no row (C6).
            } else {
                read.names
                    .insert(item.trim_start_matches("tag: ").to_owned());
            }
        }
        labels.insert(id.to_owned(), read);
    }
    labels
}

fn kind_of(name: &str) -> RefKind {
    if name.starts_with("refs/heads/") {
        RefKind::LocalBranch
    } else if name.starts_with("refs/remotes/") {
        RefKind::RemoteTracking
    } else {
        assert!(name.starts_with("refs/tags/"), "git labelled with {name}");
        RefKind::Tag
    }
}

/// One entry of `git stash list`.
#[derive(Debug)]
struct GitStash {
    index: usize,
    id: String,
    parents: Vec<String>,
    message: String,
    author: String,
    author_time: i64,
    committed: i64,
}

fn git_stashes(fixture: &Fixture) -> Vec<GitStash> {
    let list = fixture.git(&[
        "stash",
        "list",
        "--format=%gd%x1f%H%x1f%P%x1f%gs%x1f%an%x1f%at%x1f%ct",
    ]);
    list.lines()
        .map(|line| {
            let fields: Vec<&str> = line.split('\u{1f}').collect();
            assert_eq!(fields.len(), 7, "git stash list printed {line:?}");
            let index = fields[0]
                .trim_start_matches("stash@{")
                .trim_end_matches('}')
                .parse()
                .unwrap_or_else(|e| panic!("{line:?}: {e}"));
            GitStash {
                index,
                id: fields[1].to_owned(),
                parents: fields[2].split_whitespace().map(str::to_owned).collect(),
                message: fields[3].to_owned(),
                author: fields[4].to_owned(),
                author_time: ok(fields[5].parse(), "an author date"),
                committed: ok(fields[6].parse(), "a committer date"),
            }
        })
        .collect()
}

fn rev_list(fixture: &Fixture) -> BTreeSet<String> {
    fixture
        .git(&["rev-list", "--branches", "--remotes", "--tags", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Every commit git's walk reaches, with its committer date.
fn committed(fixture: &Fixture) -> HashMap<String, i64> {
    fixture
        .git(&[
            "log",
            "--format=%H %ct",
            "--branches",
            "--remotes",
            "--tags",
            "HEAD",
        ])
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(id, time)| (id.to_owned(), ok(time.parse(), "a committer date")))
        .collect()
}

/// The fixture C6 names, and then some: on `main` a commit `HEAD` reaches; `side`, which
/// `HEAD` cannot reach; a remote-tracking ref and a tag each reaching a commit nothing else
/// does; a lightweight, an annotated and a nested tag; a tag on a tree, lightweight and
/// annotated; a stash whose branch was deleted; two stashes on `main`'s tip, one made with
/// `--include-untracked`; a stash on `side`'s tip dated older than it; and a branch at the
/// untracked stash's index commit.
struct Everything {
    fixture: Fixture,
    /// The deleted branch's commits and its stash.
    gone: Vec<String>,
    gone_stash: String,
    /// `main`'s tip and the two stashes on it, the untracked one's index and untracked
    /// commits, and the plain one's index commit.
    main_tip: String,
    untracked_stash: String,
    plain_stash: String,
    untracked_index: String,
    untracked_files: String,
    plain_index: String,
    /// `side`'s tip and the stash on it dated older than it.
    side_tip: String,
    skewed_stash: String,
}

fn everything() -> Everything {
    let fixture = fixtures::unborn();
    let mut clock = Clock(fixtures::EPOCH);
    commit(&fixture, &mut clock, "f", "1\n", "c1");
    git_at(&fixture, clock.tick(), &["tag", "-a", "v1", "-m", "v1"]);
    git_at(
        &fixture,
        clock.tick(),
        &["tag", "-a", "nested", "-m", "nested", "v1"],
    );
    commit(&fixture, &mut clock, "f", "2\n", "c2");
    fixture.git(&["branch", "side"]);
    fixture.git(&["branch", "gone"]);
    commit(&fixture, &mut clock, "f", "3\n", "c3");
    let main_tip = rev_parse(&fixture, "main");

    fixture.git(&["checkout", "--quiet", "side"]);
    commit(&fixture, &mut clock, "g", "s1\n", "s1");
    fixture.git(&["tag", "light"]);
    commit(&fixture, &mut clock, "g", "s2\n", "s2");
    let side_tip = rev_parse(&fixture, "side");

    // A stash whose branch is then deleted: its base and the branch's other commit are
    // reached by nothing.
    fixture.git(&["checkout", "--quiet", "gone"]);
    commit(&fixture, &mut clock, "f", "g1\n", "g1");
    commit(&fixture, &mut clock, "f", "g2\n", "g2");
    let gone = vec![rev_parse(&fixture, "gone~1"), rev_parse(&fixture, "gone")];
    write(&fixture, "f", "dirty on gone\n");
    git_at(
        &fixture,
        clock.tick(),
        &["stash", "push", "--quiet", "-m", "on gone"],
    );
    let gone_stash = rev_parse(&fixture, "stash@{0}");
    fixture.git(&["checkout", "--quiet", "main"]);
    fixture.git(&["branch", "--quiet", "-D", "gone"]);

    // A remote-tracking ref and a tag, each the only ref reaching its commit.
    fixture.git(&["checkout", "--quiet", "-b", "temp"]);
    commit(&fixture, &mut clock, "h", "r1\n", "r1");
    fixture.git(&["update-ref", "refs/remotes/origin/feature", "HEAD"]);
    commit(&fixture, &mut clock, "h", "t1\n", "t1");
    git_at(
        &fixture,
        clock.tick(),
        &["tag", "-a", "only-tag", "-m", "only"],
    );
    fixture.git(&["checkout", "--quiet", "main"]);
    fixture.git(&["branch", "--quiet", "-D", "temp"]);
    fixture.git(&["update-ref", "refs/remotes/origin/main", "main"]);
    fixture.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);
    fixture.git(&["tag", "treetag", "main^{tree}"]);
    git_at(
        &fixture,
        clock.tick(),
        &["tag", "-a", "annot-tree", "-m", "a tree", "main^{tree}"],
    );

    // Two stashes on main's tip, the newest rows of all.
    write(&fixture, "f", "wip one\n");
    git_at(
        &fixture,
        clock.tick(),
        &["stash", "push", "--quiet", "-m", "plain"],
    );
    let plain_stash = rev_parse(&fixture, "stash@{0}");
    let plain_index = rev_parse(&fixture, "stash@{0}^2");
    write(&fixture, "f", "wip two\n");
    write(&fixture, "new.txt", "untracked\n");
    git_at(
        &fixture,
        clock.tick(),
        &[
            "stash",
            "push",
            "--quiet",
            "--include-untracked",
            "-m",
            "with untracked",
        ],
    );
    let untracked_stash = rev_parse(&fixture, "stash@{0}");
    let untracked_index = rev_parse(&fixture, "stash@{0}^2");
    let untracked_files = rev_parse(&fixture, "stash@{0}^3");
    // A branch reaching the untracked stash's index commit: a commit's row, once.
    fixture.git(&["branch", "idx", &untracked_index]);

    // A stash on side's tip, its dates older than that tip's.
    fixture.git(&["checkout", "--quiet", "side"]);
    write(&fixture, "g", "wip on side\n");
    let skewed_at = fixtures::EPOCH + 30;
    git_at(
        &fixture,
        skewed_at,
        &["stash", "push", "--quiet", "-m", "skewed"],
    );
    let skewed_stash = rev_parse(&fixture, "stash@{0}");
    fixture.git(&["checkout", "--quiet", "main"]);

    Everything {
        fixture,
        gone,
        gone_stash,
        main_tip,
        untracked_stash,
        plain_stash,
        untracked_index,
        untracked_files,
        plain_index,
        side_tip,
        skewed_stash,
    }
}

/// C6's first half: the commits walked from every ref, to the end, are git's
/// `rev-list --branches --remotes --tags HEAD` — the branch `HEAD` cannot reach, the
/// remote-tracking ref's and the tag's own commits among them, the tag on a tree and the
/// stashes adding nothing — each once; the deleted branch's stash has no row and pulls none
/// of its commits in; a stash's index and untracked commits are no rows, unless a ref
/// reaches one, which is then a commit's row, once. Caught by: `refs/stash` or a stash's
/// base seeding the walk, a stash laid out with all its parents, a tag on a tree failing
/// the walk, or a seed kind left out.
#[test]
fn the_commits_walked_from_every_ref_are_git_rev_lists() {
    let it = everything();
    let repo = ok(
        Repository::discover(it.fixture.path()),
        "opening the fixture",
    );
    let rows = page_all(&repo, &snapshot_request(&repo, 3), 3);
    let (commits, stashes) = index(&rows);

    let walked: BTreeSet<String> = commits.keys().cloned().collect();
    let expected = rev_list(&it.fixture);
    assert_eq!(walked, expected, "the walk is not git's");
    assert!(expected.len() >= 8, "the fixture decides too little");

    for gone in it.gone.iter().chain([&it.gone_stash]) {
        assert!(
            !commits.contains_key(gone) && !stashes.contains_key(gone),
            "{gone}, reached only by a deleted branch's stash, is drawn"
        );
    }
    assert!(
        commits.contains_key(&it.untracked_index),
        "the index commit a branch reaches is not drawn"
    );
    for internal in [&it.untracked_files, &it.plain_index] {
        assert!(
            !commits.contains_key(internal),
            "the stash's internal commit {internal} is a row"
        );
    }
    let mut drawn_stashes: Vec<&String> = stashes.keys().collect();
    drawn_stashes.sort();
    let mut expected_stashes = vec![&it.plain_stash, &it.untracked_stash, &it.skewed_stash];
    expected_stashes.sort();
    assert_eq!(drawn_stashes, expected_stashes);
}

/// C6's labels: every commit's row carries exactly the refs `git log --decorate=full`
/// names for it, `refs/stash` aside — `HEAD`, the branch it is on as current, the
/// remote-tracking refs (`origin/HEAD` among them), lightweight, annotated and nested tags
/// — each with its kind; a stash's row carries none. Caught by: a symbolic ref or a nested
/// tag dropped, a tag's kind lost, `HEAD` marked on another row, or labels read off by a
/// row.
#[test]
fn every_rows_labels_are_what_git_log_decorates_it_with() {
    let it = everything();
    let repo = ok(
        Repository::discover(it.fixture.path()),
        "opening the fixture",
    );
    let rows = page_all(&repo, &snapshot_request(&repo, 2), 2);
    let expected = git_labels(&it.fixture);

    let mut kinds = BTreeSet::new();
    let mut heads = 0;
    for row in rows.rows() {
        let labels = row.labels();
        match row.id() {
            RowId::Stash(id) => {
                assert!(
                    labels.is_empty() && !labels.is_head(),
                    "stash {id}'s row carries {labels:?}"
                );
                continue;
            }
            RowId::Commit(id) => {
                let want = expected
                    .get(&id.to_string())
                    .unwrap_or_else(|| panic!("git did not walk {id}"));
                let names: BTreeSet<String> =
                    labels.iter().map(|label| label.name.to_owned()).collect();
                assert_eq!(names, want.names, "the labels of {id}");
                assert_eq!(labels.is_head(), want.head, "whether {id} is HEAD's");
                let current: Vec<String> = labels
                    .iter()
                    .filter(|label| label.current)
                    .map(|label| label.name.to_owned())
                    .collect();
                assert_eq!(current, want.current.iter().cloned().collect::<Vec<_>>());
                for label in labels.iter() {
                    assert_eq!(label.kind, kind_of(label.name), "{}'s kind", label.name);
                    kinds.insert(format!("{:?}", label.kind));
                }
                heads += usize::from(labels.is_head());
            }
        }
    }
    assert_eq!(heads, 1, "HEAD labels one row");
    assert_eq!(kinds.len(), 3, "the fixture labels every kind: {kinds:?}");
    let main = rows
        .rows()
        .find(|row| row.id() == RowId::Commit(ok(cairn_model::Oid::parse(&it.main_tip), "an id")))
        .unwrap_or_else(|| panic!("main's tip has no row"));
    assert!(
        main.labels()
            .iter()
            .any(|label| label.name == "refs/remotes/origin/HEAD"),
        "the symbolic remote-tracking ref labels nothing"
    );
}

/// C6's stash rows: each stash whose base is walked is one row — its index, base, message,
/// author and date git's — with one line, leaving its node down its own lane and ending at
/// its base; two stashes on one commit take a lane each; a stash dated older than its base
/// is directly above it; the others sit at their dates. Caught by: a stash's row laid out
/// as a commit's (lines joined, or to every parent), placed after its base, or placed by
/// its base alone and never by its date.
#[test]
fn each_stash_on_a_walked_commit_is_one_row_with_one_line_to_it() {
    let it = everything();
    let repo = ok(
        Repository::discover(it.fixture.path()),
        "opening the fixture",
    );
    let rows = page_all(&repo, &snapshot_request(&repo, 64), 64);
    let (commits, stashes) = index(&rows);
    let dates = committed(&it.fixture);

    for listed in git_stashes(&it.fixture) {
        let Some(&at) = stashes.get(&listed.id) else {
            assert_eq!(
                listed.id, it.gone_stash,
                "stash@{{{}}} has no row",
                listed.index
            );
            continue;
        };
        let row = rows.row(at).unwrap_or_else(|| panic!("no row {at}"));
        let stash = stash_of(row).unwrap_or_else(|| panic!("row {at} is not a stash's"));
        let base = &listed.parents[0];
        assert_eq!(stash.index, listed.index);
        assert_eq!(stash.base.to_string(), *base);
        assert_eq!(stash.message, listed.message);
        assert_eq!(stash.author_name, listed.author);
        assert_eq!(stash.author_time, listed.author_time);

        // One line: out of its node, down its own lane, into its base's node.
        let lane = row.lane();
        let drawn = row
            .edges()
            .unwrap_or_else(|| panic!("row {at} drew nothing"));
        let own: Vec<_> = drawn
            .edges
            .iter()
            .filter(|edge| edge.kind != EdgeKind::Passing)
            .collect();
        assert_eq!(own.len(), 1, "stash@{{{}}} draws {own:?}", listed.index);
        assert_eq!(
            (own[0].kind, own[0].from, own[0].to),
            (EdgeKind::OutOfCommit, lane, lane)
        );
        let base_at = commits[base];
        assert!(base_at > at, "stash@{{{}}} is below its base", listed.index);
        for between in at + 1..base_at {
            let passing = rows
                .row(between)
                .and_then(|row| row.edges())
                .is_some_and(|drawn| {
                    drawn
                        .edges
                        .iter()
                        .any(|edge| edge.kind == EdgeKind::Passing && edge.from == lane)
                });
            assert!(
                passing,
                "stash@{{{}}}'s line breaks at row {between}",
                listed.index
            );
        }
        let base_row = rows.row(base_at).and_then(|row| row.edges());
        let base_lane = rows.row(base_at).map(|row| row.lane());
        assert!(
            base_row.is_some_and(|drawn| drawn.edges.iter().any(|edge| {
                edge.kind == EdgeKind::IntoCommit && edge.from == lane && Some(edge.to) == base_lane
            })),
            "stash@{{{}}}'s line does not end at its base",
            listed.index
        );

        if listed.id == it.skewed_stash {
            assert!(listed.committed < dates[base], "the skew decides nothing");
            assert_eq!(
                at + 1,
                base_at,
                "the skewed stash is not directly above its base"
            );
        } else {
            // At its date: every commit above it newer, every commit below it older.
            for row in rows.rows() {
                let RowId::Commit(id) = row.id() else {
                    continue;
                };
                let date = dates[&id.to_string()];
                if row.index() < at {
                    assert!(date >= listed.committed, "{id} is newer yet below");
                } else {
                    assert!(date <= listed.committed, "{id} is older yet above");
                }
            }
        }
    }

    // The two stashes on main's tip: a lane each, both ending there.
    let (plain, untracked) = (stashes[&it.plain_stash], stashes[&it.untracked_stash]);
    let lane = |at: usize| rows.row(at).map(|row| row.lane());
    assert_ne!(lane(plain), lane(untracked), "two stashes share a lane");
    assert!(commits.contains_key(&it.side_tip));
}

/// The QA brief's paging: a walk paged one, two or five rows at a time — held, or cold
/// from each cursor — draws every row, stash rows among them, exactly as one page of the
/// whole does, so a stash older than the first page's last commit arrives with the page
/// that reaches its date. Caught by: stashes merged into the first page alone, or a cursor
/// that forgets which were placed.
#[test]
fn a_walk_paged_any_way_draws_the_rows_one_page_does() {
    let it = everything();
    let repo = ok(
        Repository::discover(it.fixture.path()),
        "opening the fixture",
    );
    let whole = page_all(&repo, &snapshot_request(&repo, 1_000), 1_000);
    let expected = described(&whole);
    let lanes = |rows: &History| -> Vec<Lane> { rows.rows().map(|row| row.lane()).collect() };
    let first_stash = whole
        .rows()
        .position(|row| matches!(row.id(), RowId::Stash(_)))
        .unwrap_or(0);
    let last_stash = whole
        .rows()
        .filter(|row| matches!(row.id(), RowId::Stash(_)))
        .map(|row| row.index())
        .max()
        .unwrap_or(0);
    // Past two pages of two: the skewed stash, dated older than every commit.
    assert!(
        last_stash >= 4 && last_stash > first_stash,
        "no stash row falls past the first pages"
    );
    for page in [1, 2, 5] {
        let held = page_all(&repo, &snapshot_request(&repo, page), page);
        assert_eq!(described(&held), expected, "held pages of {page}");
        assert_eq!(lanes(&held), lanes(&whole), "held pages of {page}");
        let cold = cold_all(&repo, snapshot_request(&repo, page), page);
        assert_eq!(described(&cold), expected, "cold pages of {page}");
    }
}

/// The QA brief's deep base: a stash newer than everything, made on a commit far below.
/// Looking ahead far enough, its row is at its date; looking ahead less than the distance,
/// directly above its base — never after it, and never missing. Caught by: a stash whose
/// base was not found within the look-ahead dropped, or drawn below its base.
#[test]
fn a_stash_on_a_deep_base_is_drawn_at_its_date_or_directly_above_its_base() {
    let fixture = fixtures::unborn();
    let mut clock = Clock(fixtures::EPOCH);
    commit(&fixture, &mut clock, "f", "base\n", "base");
    let base = rev_parse(&fixture, "HEAD");
    fixture.git(&["branch", "old"]);
    for n in 0..12 {
        commit(
            &fixture,
            &mut clock,
            "f",
            &format!("{n}\n"),
            &format!("after {n}"),
        );
    }
    fixture.git(&["checkout", "--quiet", "old"]);
    write(&fixture, "f", "wip\n");
    git_at(
        &fixture,
        clock.tick(),
        &["stash", "push", "--quiet", "-m", "deep"],
    );
    fixture.git(&["checkout", "--quiet", "main"]);
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");

    let place = |lookahead: usize| -> (usize, usize) {
        let request = snapshot_request(&repo, 4).with_stash_lookahead(lookahead);
        let rows = page_all(&repo, &request, 4);
        let (commits, stashes) = index(&rows);
        assert_eq!(stashes.len(), 1, "the deep stash has no row");
        assert_eq!(rows.len(), 14);
        (stashes.values().copied().sum(), commits[&base])
    };
    assert_eq!(place(64), (0, 13), "looking far enough, it is at its date");
    assert_eq!(
        place(3),
        (12, 13),
        "looking less far, it is directly above its base"
    );
}

/// A detached `HEAD` at a commit no ref reaches seeds the walk and labels its row `HEAD`,
/// with no branch current. Caught by: a detached `HEAD` not seeded.
#[test]
fn a_detached_head_no_ref_reaches_is_walked_and_labelled() {
    let fixture = fixtures::unborn();
    let mut clock = Clock(fixtures::EPOCH);
    commit(&fixture, &mut clock, "f", "1\n", "one");
    fixture.git(&["checkout", "--quiet", "--detach"]);
    commit(&fixture, &mut clock, "f", "2\n", "detached");
    let head = rev_parse(&fixture, "HEAD");
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    let rows = page_all(&repo, &snapshot_request(&repo, 1), 1);
    let (commits, _) = index(&rows);
    assert_eq!(
        commits.keys().cloned().collect::<BTreeSet<_>>(),
        rev_list(&fixture)
    );
    let row = rows
        .row(commits[&head])
        .unwrap_or_else(|| panic!("no HEAD row"));
    assert!(row.labels().is_head());
    assert!(row.labels().is_empty(), "{:?}", row.labels());
    assert_eq!(git_labels(&fixture)[&head].names, BTreeSet::new());
}

/// A stash commit a branch reaches is a commit's row, labelled by the branch, and no
/// stash's; its index commit is a commit's row too, as git walks it. Caught by: the
/// stash's row drawn as well, the commit then having two rows.
#[test]
fn a_stash_commit_a_branch_reaches_is_a_commits_row_and_no_stashs() {
    let fixture = fixtures::unborn();
    let mut clock = Clock(fixtures::EPOCH);
    commit(&fixture, &mut clock, "f", "1\n", "one");
    write(&fixture, "f", "wip\n");
    git_at(
        &fixture,
        clock.tick(),
        &["stash", "push", "--quiet", "-m", "kept"],
    );
    let stash = rev_parse(&fixture, "stash@{0}");
    fixture.git(&["branch", "keep", &stash]);
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    let rows = page_all(&repo, &snapshot_request(&repo, 2), 2);
    let (commits, stashes) = index(&rows);
    assert!(
        stashes.is_empty(),
        "the reached stash also drew a stash's row"
    );
    assert_eq!(
        commits.keys().cloned().collect::<BTreeSet<_>>(),
        rev_list(&fixture)
    );
    assert!(commits.contains_key(&stash));
}

/// A braided history of merges, two branches and no stash: from every ref, git's commits
/// — `HEAD`'s and `side`'s — each once, in the order git's own date walk gives them.
/// Caught by: a ref's commit walked twice, or `side` left unseeded.
#[test]
fn a_braided_history_from_every_ref_is_git_rev_lists() {
    let fixture = fixtures::braided(14);
    // One commit on `side` that `HEAD` does not reach, newer than everything.
    fixture.git(&["checkout", "--quiet", "side"]);
    git_at(
        &fixture,
        fixtures::EPOCH + 100_000,
        &["commit", "--quiet", "--allow-empty", "-m", "side only"],
    );
    fixture.git(&["checkout", "--quiet", "main"]);
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    let rows = page_all(&repo, &snapshot_request(&repo, 3), 3);
    let walked: Vec<String> = rows
        .rows()
        .map(|row| match row.id() {
            RowId::Commit(id) => id.to_string(),
            RowId::Stash(id) => panic!("a fixture with no stash drew stash {id}"),
        })
        .collect();
    let expected: Vec<String> = fixture
        .git(&[
            "rev-list",
            "--date-order",
            "--branches",
            "--remotes",
            "--tags",
            "HEAD",
        ])
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(walked, expected);
    assert_eq!(
        fixture.rev_list().len() + 1,
        expected.len(),
        "side reaches nothing past HEAD, so its seed decides nothing"
    );
}

/// In graph order, which carries no dates, a stash's row is directly above its base.
#[test]
fn in_graph_order_a_stash_is_directly_above_its_base() {
    let it = everything();
    let repo = ok(
        Repository::discover(it.fixture.path()),
        "opening the fixture",
    );
    let request = snapshot_request(&repo, 4).with_order(HistoryOrder::GraphOrder);
    let rows = page_all(&repo, &request, 4);
    let (commits, stashes) = index(&rows);
    assert_eq!(
        commits.keys().cloned().collect::<BTreeSet<_>>(),
        rev_list(&it.fixture)
    );
    assert_eq!(stashes.len(), 3);
    for (id, &at) in &stashes {
        let stash = rows.row(at).and_then(stash_of);
        let base = stash
            .map(|stash| stash.base.to_string())
            .unwrap_or_default();
        let base_at = commits[&base];
        assert!(base_at > at, "stash {id} below its base");
        // Directly above: only other stashes on the same base between.
        for between in at + 1..base_at {
            let other = rows.row(between).and_then(stash_of);
            assert!(
                other.is_some_and(|other| other.base.to_string() == base),
                "row {between} separates stash {id} from its base"
            );
        }
    }
}

// --- The reporter: C11's first page from every ref ---

/// C11's first page: on the repository `CAIRN_BENCH_REPO` names (read only), the first page
/// of 64 rows from `HEAD` and from every ref — the snapshot already read, and with its read
/// — warm, median of seven after a warm-up, each run a fresh open of the repository. It
/// prints how many commits each first page pulled off the walk, look-ahead for a stash's
/// base included, and how many stashes it drew. Run with
/// `cargo test --release -p cairn-git --test every_ref -- --ignored --nocapture`.
#[test]
#[ignore = "needs a repository named by CAIRN_BENCH_REPO; run with --release"]
fn measures_the_first_page_from_every_ref() {
    use std::time::{Duration, Instant};

    const PAGE: usize = 64;
    const RUNS: usize = 7;
    let path = std::env::var("CAIRN_BENCH_REPO").unwrap_or_else(|_| panic!("set CAIRN_BENCH_REPO"));

    let median = |mut samples: Vec<Duration>| -> (f64, f64, f64) {
        samples.sort_unstable();
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        let (first, last) = (samples[0], samples[samples.len() - 1]);
        (ms(samples[samples.len() / 2]), ms(first), ms(last))
    };
    for seed in ["head", "refs", "refs+snapshot"] {
        let mut samples = Vec::with_capacity(RUNS);
        let mut shape = (0, 0, 0, 0);
        for run in 0..=RUNS {
            let repo = ok(Repository::discover(&path), "opening the repository");
            let snapshot = (seed == "refs")
                .then(|| ok(repo.refs(&CancelSignal::new()), "reading the refs").snapshot);
            let started = Instant::now();
            let request = match (seed, &snapshot) {
                ("head", _) => HistoryRequest::from_head(PAGE),
                (_, Some(snapshot)) => HistoryRequest::from_refs(snapshot, PAGE),
                (_, None) => snapshot_request(&repo, PAGE),
            };
            let mut session = ok(repo.history_session(&request), "opening the session");
            let page = ok(
                session.next_page(PAGE, &CancelSignal::new()),
                "the first page",
            );
            let elapsed = started.elapsed();
            let mut held = History::new();
            let rows = page.rows.len();
            ok(held.append(page.rows), "holding the page");
            let stashes = held
                .rows()
                .filter(|row| matches!(row.id(), RowId::Stash(_)))
                .count();
            shape = (rows, page.walked, session.commits_walked(), stashes);
            if run > 0 {
                samples.push(elapsed);
            }
        }
        let (median, low, high) = median(samples);
        let (rows, laid_out, pulled, stashes) = shape;
        eprintln!(
            "FIRST PAGE seed={seed} median={median:.2} ms [{low:.2}-{high:.2}] rows={rows} \
             laid_out={laid_out} commits_pulled={pulled} stash_rows={stashes}"
        );
    }
}
