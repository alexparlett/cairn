//! git's date order, pinned against git itself. Every expectation is git's own output, run in
//! the fixture at test time by the git in use — at the host's git in the full suite and at
//! each floor in `scripts/gate.sh --step git-floor` — through one oracle:
//!
//! ```sh
//! { stashes whose base the refs reach; reflog ids } |
//!   git log --date-order --format=%H --branches --remotes --tags HEAD --stdin
//! ```
//!
//! with each such stash grafted onto its base (`git replace --graft <stash> <stash>^1`) in a
//! copy of the fixture, never in the repository Cairn reads, so the commit-graph there stays
//! one git would use. The walk is given the same tips in the same order: every local branch,
//! remote-tracking ref and tag in refname order, `HEAD`, the stashes in `stash@{n}` order,
//! then the reflog ids — `HEAD`'s log, then each local branch's in refname order, each oldest
//! entry first, old id then new.
//!
//! The fixtures: commit times tied across branches, merges, two stashes and a rebase of five
//! commits in one second; the ui-review recipe (`docs/research/ui-review/fixture-recipe.md`);
//! two stashes on two branches, and a stash whose base no ref reaches; clock skew, children
//! dated before their parents; and reflog entries for Show Lost Commits — each read with and
//! without a commit-graph, for the refs alone, with the stashes and with the reflogs. Then a
//! replace ref, a graft and a shallow boundary over a history with a commit-graph, each laid
//! out so that reading the graph's parents would change the order.

use std::collections::HashSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use cairn_git::{CancelSignal, DateOrderTips, Repository};
use cairn_model::Oid;

fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

/// A repository in a temporary directory, removed when the test ends.
struct Fixture {
    path: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "cairn-date-order-{}-{name}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path)
        .unwrap_or_else(|e| panic!("could not make {}: {e}", path.display()));
    path
}

/// A directory with nothing in it, for `HOME`: no configuration of the machine's user is read
/// through it, on any git.
fn empty_home() -> &'static Path {
    static HOME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    HOME.get_or_init(|| scratch("home"))
}

/// `git` in `dir`, isolated from the machine's configuration, every commit dated `at`, fed
/// `input` on stdin. Panics on failure.
fn git_in(dir: &Path, at: i64, args: &[&str], input: &str) -> String {
    // `@` so that a small number of seconds is read as one, not as a date.
    let stamp = format!("@{at} +0000");
    let mut child = Command::new("git")
        .current_dir(dir)
        .args(["-c", "maintenance.auto=false", "-c", "gc.auto=0"])
        .args(["-c", "advice.graftFileDeprecated=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", empty_home())
        .env("XDG_CONFIG_HOME", empty_home())
        .env("GIT_AUTHOR_NAME", "Ada Lovelace")
        .env("GIT_AUTHOR_EMAIL", "ada@example.com")
        .env("GIT_COMMITTER_NAME", "Ada Lovelace")
        .env("GIT_COMMITTER_EMAIL", "ada@example.com")
        .env("GIT_AUTHOR_DATE", &stamp)
        .env("GIT_COMMITTER_DATE", &stamp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("could not run git {args:?}: {e}"));
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .unwrap_or_else(|e| panic!("could not feed git {args:?}: {e}"));
    }
    let output = child
        .wait_with_output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(
        output.status.success(),
        "git {args:?} failed in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Every fixture's commits share this second unless a step says otherwise.
const TIED: i64 = 1_760_612_400;

impl Fixture {
    fn new(name: &str) -> Self {
        let fixture = Self {
            path: scratch(name),
        };
        fixture.git(&["init", "-q", "--initial-branch=main", "."]);
        fixture
    }

    fn git(&self, args: &[&str]) -> String {
        git_in(&self.path, TIED, args, "")
    }

    fn git_at(&self, at: i64, args: &[&str]) -> String {
        git_in(&self.path, at, args, "")
    }

    fn rev(&self, name: &str) -> String {
        self.git(&["rev-parse", "--verify", "-q", name])
            .trim()
            .to_owned()
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.path.join(file);
        if let Some(parent) = path.parent() {
            ok(std::fs::create_dir_all(parent), "making a directory");
        }
        ok(std::fs::write(path, text), "writing a file");
    }

    fn append(&self, file: &str, text: &str) {
        let path = self.path.join(file);
        let mut existing = std::fs::read_to_string(&path).unwrap_or_default();
        existing.push_str(text);
        ok(std::fs::write(path, existing), "writing a file");
    }

    fn commit_all(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    /// `git commit-graph write --reachable`: what each git in use writes, v1 generations at
    /// the floor.
    fn write_commit_graph(&self) {
        self.git(&["commit-graph", "write", "--reachable"]);
    }

    /// A copy of the whole repository, for an oracle that writes refs.
    fn copy(&self, name: &str) -> Fixture {
        let copy = Fixture {
            path: scratch(name),
        };
        copy_dir(&self.path, &copy.path);
        copy
    }
}

fn copy_dir(from: &Path, to: &Path) {
    ok(std::fs::create_dir_all(to), "making a directory");
    for entry in ok(std::fs::read_dir(from), "listing a directory") {
        let entry = ok(entry, "listing a directory");
        let target = to.join(entry.file_name());
        if ok(entry.file_type(), "reading an entry").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            ok(std::fs::copy(entry.path(), &target), "copying a file");
        }
    }
}

/// The ui-review recipe, every commit in one second: two branches merged, a branch ahead, a
/// lightweight and an annotated tag, two refs pushed to a remote, and the recipe's stash.
fn recipe(name: &str) -> (Fixture, Fixture) {
    let remote = Fixture {
        path: scratch(&format!("{name}-remote")),
    };
    remote.git(&["init", "-q", "--bare", "--initial-branch=main", "."]);
    let t = Fixture::new(name);
    t.write("src/main.rs", "fn main() {\n    println!(\"hello\");\n}\n");
    t.write("README.md", "# Demo\n\nA demo repository.\n");
    t.commit_all("Initial commit");
    for i in 1..=6 {
        t.append("notes.txt", &format!("line {i}\n"));
        t.commit_all(&format!("Add note {i}"));
    }
    t.git(&["tag", "v0.1"]);
    t.git(&["checkout", "-qb", "feature/login"]);
    t.write("src/login.rs", "pub fn login() {}\n");
    t.commit_all("Add login module");
    t.append("src/login.rs", "// validate\n");
    t.commit_all("Validate login input");
    t.git(&["checkout", "-q", "main"]);
    t.append("README.md", "Fixed.\n");
    t.commit_all("Fix README typo");
    t.git(&[
        "merge",
        "-q",
        "--no-ff",
        "feature/login",
        "-m",
        "Merge branch 'feature/login'",
    ]);
    t.git(&["tag", "-a", "v0.2", "-m", "Release 0.2"]);
    t.git(&["checkout", "-qb", "bugfix/crash"]);
    t.append("src/main.rs", "// guard\n");
    t.commit_all("Fix crash on empty input");
    t.git(&["checkout", "-q", "main"]);
    t.git(&["remote", "add", "origin", &remote.path.to_string_lossy()]);
    t.git(&["push", "-qu", "origin", "main", "feature/login"]);
    t.append("notes.txt", "line 7\n");
    t.commit_all("Local only commit");
    t.append("src/main.rs", "// wip\n");
    t.git(&["stash", "push", "-qm", "WIP on parser"]);
    (t, remote)
}

/// The recipe, then a branch merged twice, a run of five commits rebased onto `main`, a
/// commit after it and a second stash — every commit in the same second.
fn tied() -> (Fixture, Fixture) {
    let (t, remote) = recipe("tied");
    t.git(&["checkout", "-qb", "topic"]);
    t.write("topic.txt", "t1\n");
    t.commit_all("Topic one");
    t.git(&["checkout", "-q", "main"]);
    t.append("notes.txt", "m1\n");
    t.commit_all("Main between merges");
    t.git(&["merge", "-q", "--no-ff", "topic", "-m", "Merge topic once"]);
    t.git(&["checkout", "-q", "topic"]);
    t.append("topic.txt", "t2\n");
    t.commit_all("Topic two");
    t.git(&["checkout", "-q", "main"]);
    t.git(&["merge", "-q", "--no-ff", "topic", "-m", "Merge topic twice"]);
    t.git(&["checkout", "-q", "-b", "rebased", "main~2"]);
    for i in 1..=5 {
        t.append("rebased.txt", &format!("r{i}\n"));
        t.commit_all(&format!("Rebased {i}"));
    }
    t.git(&["rebase", "-q", "main"]);
    t.git(&["checkout", "-q", "main"]);
    t.append("notes.txt", "line 8\n");
    t.commit_all("After the rebase");
    t.append("src/main.rs", "// wip\n");
    t.git(&["stash", "push", "-qm", "WIP on parser"]);
    (t, remote)
}

/// The recipe with a second stash on `bugfix/crash`, and a third made on a branch then
/// deleted, whose base no ref reaches.
fn two_stashes() -> (Fixture, Fixture) {
    let (t, remote) = recipe("two-stashes");
    t.git(&["checkout", "-q", "bugfix/crash"]);
    t.append("src/main.rs", "// wip2\n");
    t.git(&["stash", "push", "-qm", "WIP on crash"]);
    t.git(&["checkout", "-qb", "gone"]);
    t.append("gone.txt", "gone\n");
    t.commit_all("Only a stash is made on this");
    t.append("gone.txt", "wip\n");
    t.git(&["stash", "push", "-qm", "WIP on a branch deleted"]);
    t.git(&["checkout", "-q", "main"]);
    t.git(&["branch", "-qD", "gone"]);
    (t, remote)
}

/// Children dated before their parents: `S2-skewed` older than its parent and than `A`,
/// `D-skewed` older than its parent, a merge of the skewed side, and a late branch.
fn skew() -> Fixture {
    let t = Fixture::new("skew");
    let commit = |at: i64, name: &str| {
        t.write(name, name);
        t.git_at(at, &["add", "-A"]);
        t.git_at(at, &["commit", "-q", "-m", &format!("{name} (t={at})")]);
    };
    commit(1000, "A");
    commit(2000, "B");
    t.git(&["checkout", "-qb", "side"]);
    commit(2500, "S1");
    commit(500, "S2-skewed");
    commit(2600, "S3");
    t.git(&["checkout", "-q", "main"]);
    commit(3000, "C");
    commit(1500, "D-skewed");
    commit(3100, "E");
    t.git_at(
        4000,
        &[
            "merge",
            "-q",
            "--no-ff",
            "side",
            "-m",
            "M merge side (t=4000)",
        ],
    );
    t.git(&["checkout", "-qb", "late", "main~4"]);
    commit(5000, "L1");
    t.git(&["checkout", "-q", "main"]);
    t
}

/// Reflog entries naming commits no ref reaches, every commit in one second: a branch given
/// two commits then reset, another given one then reset, a detached commit, and an amend of
/// the root.
fn reflogs() -> Fixture {
    let t = Fixture::new("reflogs");
    t.write("f", "root");
    t.commit_all("root");
    for (branch, commits) in [("zzz", 2), ("aaa", 1)] {
        t.git(&["checkout", "-qb", branch]);
        for n in 0..commits {
            t.write("f", &format!("{branch} {n}"));
            t.commit_all(&format!("{branch} {n}"));
        }
        t.git(&["reset", "-q", "--hard", "main"]);
        t.git(&["checkout", "-q", "main"]);
    }
    t.git(&["checkout", "-q", "--detach"]);
    t.write("f", "detached");
    t.commit_all("detached");
    t.git(&["checkout", "-q", "main"]);
    t.write("f", "amended");
    t.git(&["add", "-A"]);
    t.git(&["commit", "-q", "--amend", "-m", "root, amended"]);
    t
}

/// What a walk starts from, as git is given it.
struct Tips {
    refs: Vec<String>,
    /// Each stash's commit and its base, `stash@{0}` first.
    stashes: Vec<(String, String)>,
    reflogs: Vec<String>,
}

/// Every local branch, remote-tracking ref and tag in refname order, each peeled to the
/// commit it names (a ref naming no commit left out), then `HEAD`'s commit.
fn ref_tips(fixture: &Fixture) -> Vec<String> {
    let listed = fixture.git(&[
        "for-each-ref",
        "--format=%(objectname) %(objecttype) %(*objectname) %(*objecttype)",
        "refs/heads/",
        "refs/remotes/",
        "refs/tags/",
    ]);
    let mut tips = Vec::new();
    for line in listed.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        match fields.as_slice() {
            [id, "commit", ..] => tips.push((*id).to_owned()),
            [_, "tag", peeled, "commit"] => tips.push((*peeled).to_owned()),
            _ => {}
        }
    }
    let head = fixture.rev("HEAD^{commit}");
    if !head.is_empty() {
        tips.push(head);
    }
    tips
}

/// `git stash list`'s commits, `stash@{0}` first, each with its base.
fn stash_tips(fixture: &Fixture) -> Vec<(String, String)> {
    fixture
        .git(&["stash", "list", "--format=%H %P"])
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(' ');
            Some((fields.next()?.to_owned(), fields.next()?.to_owned()))
        })
        .collect()
}

/// The old and the new id of every entry of `HEAD`'s log, then of each local branch's in
/// refname order, each oldest first, the null id left out.
fn reflog_tips(fixture: &Fixture) -> Vec<String> {
    let logs = fixture.path.join(".git").join("logs");
    let mut files = vec![logs.join("HEAD")];
    let mut branches: Vec<String> = fixture
        .git(&["for-each-ref", "--format=%(refname)", "refs/heads/"])
        .lines()
        .map(str::to_owned)
        .collect();
    branches.sort();
    files.extend(branches.iter().map(|branch| logs.join(branch)));
    let mut ids = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in text.lines() {
            for id in line.split(' ').take(2) {
                if id.bytes().any(|byte| byte != b'0') {
                    ids.push(id.to_owned());
                }
            }
        }
    }
    ids
}

/// Which tips a reading is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reading {
    Refs,
    Stashes,
    LostCommits,
}

fn tips_for(fixture: &Fixture, reading: Reading) -> Tips {
    Tips {
        refs: ref_tips(fixture),
        stashes: match reading {
            Reading::Refs => Vec::new(),
            Reading::Stashes | Reading::LostCommits => stash_tips(fixture),
        },
        reflogs: match reading {
            Reading::Refs | Reading::Stashes => Vec::new(),
            Reading::LostCommits => reflog_tips(fixture),
        },
    }
}

/// git's order for `tips`: each stash whose base the refs reach grafted onto it in a copy,
/// then `git log --date-order` with the stashes and the reflog ids on stdin.
fn oracle(fixture: &Fixture, tips: &Tips) -> Vec<String> {
    let reached: HashSet<String> = fixture
        .git(&["rev-list", "--branches", "--remotes", "--tags", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    let stashes: Vec<&String> = tips
        .stashes
        .iter()
        .filter(|(_, base)| reached.contains(base))
        .map(|(stash, _)| stash)
        .collect();
    let copy = (!stashes.is_empty()).then(|| fixture.copy("oracle"));
    let at = copy.as_ref().unwrap_or(fixture);
    for (stash, base) in &tips.stashes {
        if stashes.contains(&stash) {
            at.git(&["replace", "--graft", stash, base]);
        }
    }
    let mut input = String::new();
    for id in stashes.iter().copied().chain(&tips.reflogs) {
        input.push_str(id);
        input.push('\n');
    }
    git_in(
        &at.path,
        TIED,
        &[
            "log",
            "--date-order",
            "--format=%H",
            "--branches",
            "--remotes",
            "--tags",
            "HEAD",
            "--stdin",
        ],
        &input,
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// The walk's order for `tips`, and how many commits it read from the commit-graph.
fn walked(fixture: &Fixture, tips: &Tips) -> (Vec<String>, usize) {
    let oid = |hex: &String| -> Oid {
        Oid::parse(hex).unwrap_or_else(|e| panic!("{hex} is no id: {e:?}"))
    };
    let request = DateOrderTips::new(tips.refs.iter().map(oid))
        .with_stashes(
            tips.stashes
                .iter()
                .map(|(stash, base)| (oid(stash), oid(base))),
        )
        .with_reflog_tips(tips.reflogs.iter().map(oid));
    let repo = Repository::discover(&fixture.path)
        .unwrap_or_else(|e| panic!("opening {}: {e}", fixture.path.display()));
    let mut walk = repo
        .date_order(&request, &CancelSignal::new(), &mut |_| {})
        .unwrap_or_else(|e| panic!("opening the walk: {e}"));
    let mut ids = Vec::new();
    while let Some(commit) = walk
        .next(&CancelSignal::new())
        .unwrap_or_else(|e| panic!("walking: {e}"))
    {
        ids.push(commit.id.to_string());
    }
    (ids, walk.read_from_graph())
}

/// Every mismatch between the walk and the oracle over `fixture`, for each reading, each
/// named; with `graph`, also each reading where the walk read nothing from the commit-graph
/// though git would use it.
fn mismatches(name: &str, fixture: &Fixture, graph: bool, readings: &[Reading]) -> Vec<String> {
    let mut found = Vec::new();
    for &reading in readings {
        let tips = tips_for(fixture, reading);
        let expected = oracle(fixture, &tips);
        let (order, from_graph) = walked(fixture, &tips);
        let label = format!("{name}, {reading:?}, commit-graph {graph}");
        if expected.len() < 2 {
            found.push(format!("{label}: the oracle listed {expected:?}"));
        }
        if order != expected {
            found.push(format!(
                "{label}:\n  git:   {}\n  cairn: {}",
                short(&expected),
                short(&order)
            ));
        }
        if graph && from_graph == 0 {
            found.push(format!("{label}: the commit-graph was not read"));
        }
        if !graph && from_graph != 0 {
            found.push(format!("{label}: {from_graph} commits read from no graph"));
        }
    }
    found
}

fn short(ids: &[String]) -> String {
    ids.iter()
        .map(|id| id.get(..7).unwrap_or(id))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A fixture built, with the remote it pushed to where it has one.
type Build = fn() -> (Fixture, Option<Fixture>);

fn with_remote((fixture, remote): (Fixture, Fixture)) -> (Fixture, Option<Fixture>) {
    (fixture, Some(remote))
}

/// The walk's rows equal git's `--date-order` on every fixture, with and without a
/// commit-graph, for the refs, with the stashes seeded as tips, and with Show Lost Commits'
/// reflog ids. Caught by: ties broken otherwise than in insertion order, a merge's parents
/// queued in another order, tips not stably sorted, a stash merged by date rather than
/// walked as a tip or seeded though no ref reaches its base, or a reflog tip given before a
/// stash.
#[test]
fn the_walk_is_in_git_date_order() {
    let all = [Reading::Refs, Reading::Stashes, Reading::LostCommits];
    let mut found = Vec::new();
    for graph in [false, true] {
        let build: [(&str, Build); 5] = [
            ("tied", || with_remote(tied())),
            ("recipe", || with_remote(recipe("recipe"))),
            ("two stashes", || with_remote(two_stashes())),
            ("skew", || (skew(), None)),
            ("reflogs", || (reflogs(), None)),
        ];
        for (name, make) in build {
            // The remote outlives the reading: the fixture's remote-tracking refs name it.
            let (fixture, _remote) = make();
            if graph {
                fixture.write_commit_graph();
            }
            found.extend(mismatches(name, &fixture, graph, &all));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// Show Lost Commits changes the tied fixture's order — the five commits the rebase replaced
/// join it — and the stashes change it too, so the readings each decide something.
#[test]
fn each_reading_of_the_tied_fixture_is_a_different_order() {
    let (fixture, _remote) = tied();
    let orders: Vec<Vec<String>> = [Reading::Refs, Reading::Stashes, Reading::LostCommits]
        .into_iter()
        .map(|reading| oracle(&fixture, &tips_for(&fixture, reading)))
        .collect();
    assert_ne!(orders[0], orders[1]);
    assert_ne!(orders[1], orders[2]);
    assert_eq!(
        orders[2].len(),
        orders[1].len() + 5,
        "the rebase replaced five commits"
    );
}

/// Sixty commits on `main`, a minute apart, and stashes git's walk meets at different
/// depths: one dated just after an old base, one dated after everything on an old base,
/// one dated before its own base by clock skew, one dated the second of a commit the walk
/// makes ready only after it has begun, one on a branch since deleted, and one older than
/// everything on a commit no ref reaches.
fn old_stashes() -> Fixture {
    let t = Fixture::new("old-stashes");
    let at = |n: i64| 10_000 + n * 60;
    for n in 0..60 {
        t.write("f", &format!("{n}"));
        t.git_at(at(n), &["add", "-A"]);
        t.git_at(at(n), &["commit", "-q", "-m", &format!("c{n}")]);
    }
    let stash_on = |base: &str, when: i64, message: &str| {
        t.git(&["checkout", "-q", base]);
        t.write("f", message);
        t.git_at(when, &["stash", "push", "-qm", message]);
    };
    stash_on("main~50", at(10) + 30, "just after its old base");
    stash_on("main~40", at(60) + 1000, "after everything, on an old base");
    stash_on("main~30", at(30) - 5000, "before its own base");
    stash_on("main~45", at(40), "dated as a commit made ready after it");
    t.git(&["checkout", "-qb", "gone", "main~45"]);
    t.write("g", "gone");
    t.git_at(at(15) + 10, &["add", "-A"]);
    t.git_at(
        at(15) + 10,
        &["commit", "-q", "-m", "on a branch since deleted"],
    );
    t.write("g", "wip");
    t.git_at(
        at(15) + 20,
        &["stash", "push", "-qm", "on a deleted branch"],
    );
    t.git(&["checkout", "-qb", "lost", "main~55"]);
    t.write("g", "lost");
    t.git_at(100, &["add", "-A"]);
    t.git_at(100, &["commit", "-q", "-m", "older than everything"]);
    t.write("g", "lost wip");
    t.git_at(
        150,
        &["stash", "push", "-qm", "older than everything, unreached"],
    );
    t.git(&["checkout", "-q", "main"]);
    t.git(&["branch", "-qD", "gone", "lost"]);
    t
}

/// With a commit-graph, a stash is decided no sooner than the walk needs it: opening from
/// `main` reads a commit or two whatever the stashes' bases, and each stash still lands where
/// git puts it — one popped first though its base is deep, one met at its date, one popped
/// before the commit it ties with from the place it took as a tip, one decided as its base
/// is counted down, two whose bases no ref reaches dropped, one of them only once the walk
/// has run out. Without a graph the opening pass reads everything, stashes or not.
/// Caught by: the open walking down to the stashes' bases, a stash put at the place it is
/// decided rather than the place it took, or a base emitted before a stash on it.
#[test]
fn a_stash_is_decided_no_sooner_than_the_walk_needs_it() {
    let fixture = old_stashes();
    let mut found = mismatches("old stashes", &fixture, false, &[Reading::Stashes]);
    fixture.write_commit_graph();
    found.extend(mismatches(
        "old stashes",
        &fixture,
        true,
        &[Reading::Refs, Reading::Stashes, Reading::LostCommits],
    ));
    assert!(found.is_empty(), "{}", found.join("\n"));

    let tips = tips_for(&fixture, Reading::Stashes);
    assert_eq!(tips.stashes.len(), 6, "{:?}", tips.stashes);
    let oid = |hex: &String| Oid::parse(hex).unwrap_or_else(|e| panic!("{hex}: {e:?}"));
    let request = DateOrderTips::new(tips.refs.iter().map(oid)).with_stashes(
        tips.stashes
            .iter()
            .map(|(stash, base)| (oid(stash), oid(base))),
    );
    let repo = ok(Repository::discover(&fixture.path), "opening the fixture");
    let mut walk = ok(
        repo.date_order(&request, &CancelSignal::new(), &mut |_| {}),
        "opening the walk",
    );
    assert!(
        walk.walked() <= 2,
        "the open walked {} commits for stashes it did not need yet",
        walk.walked()
    );
    let mut stashes = Vec::new();
    while let Some(commit) = ok(walk.next(&CancelSignal::new()), "walking") {
        if let Some(index) = commit.stash {
            stashes.push(index);
        }
    }
    assert_eq!(
        stashes.len(),
        4,
        "four stashes' bases are reached: {stashes:?}"
    );
}

/// A history whose commit-graph names the parents its commits were made with — `A`, `B`, `C`,
/// `D` on `main` and `S1` on `side` from `A`, `A` dated after the rest — then one of three
/// substitutions git makes and the graph does not know.
fn substituted(name: &str) -> (Fixture, [String; 5]) {
    let t = Fixture::new(name);
    let commit = |at: i64, message: &str| -> String {
        t.write("f", message);
        t.git_at(at, &["add", "-A"]);
        t.git_at(at, &["commit", "-q", "-m", message]);
        t.rev("HEAD")
    };
    let a = commit(10_000, "A");
    let b = commit(2000, "B");
    let c = commit(3000, "C");
    let d = commit(4000, "D");
    t.git(&["checkout", "-qb", "side", &a]);
    let s1 = commit(5000, "S1");
    t.git(&["checkout", "-q", "main"]);
    t.write_commit_graph();
    (t, [a, b, c, d, s1])
}

/// The walk reads the commit-graph only where git does: not under a replace ref, a graft or a
/// shallow boundary, each of which gives a commit parents the graph does not hold — and each
/// fixture laid out so that the graph's parents give another order. Where git does read it,
/// so does the walk. Caught by: `commit_graph_if_enabled` taken alone, as gix decides, which
/// reads the original parents and walks to the graph's generations.
#[test]
fn the_commit_graph_is_used_only_where_git_uses_it() {
    let refs = [Reading::Refs];
    let mut found = Vec::new();

    let (plain, _) = substituted("plain");
    found.extend(mismatches("no substitution", &plain, true, &refs));

    // `C`'s parent replaced by `S1`: `B` is no longer reached, and `S1` waits for `C`.
    let (replaced, [_, _, c, _, s1]) = substituted("replace");
    replaced.git(&["replace", "--graft", &c, &s1]);
    found.extend(mismatches("replace ref", &replaced, false, &refs));

    // The same through the graft file.
    let (grafted, [_, _, c, _, s1]) = substituted("graft");
    grafted.write(".git/info/grafts", &format!("{c} {s1}\n"));
    found.extend(mismatches("graft", &grafted, false, &refs));

    // `B` a shallow boundary: `A`, dated after the rest, waits only for `S1`.
    let (shallow, [_, b, ..]) = substituted("shallow");
    shallow.write(".git/shallow", &format!("{b}\n"));
    found.extend(mismatches("shallow boundary", &shallow, false, &refs));

    // A real shallow clone of `main`, two commits deep, beside the source's commit-graph.
    let (source, _) = substituted("shallow-source");
    let clone = Fixture {
        path: scratch("shallow-clone"),
    };
    git_in(
        &clone.path,
        TIED,
        &[
            "clone",
            "-q",
            "--depth=2",
            "--no-single-branch",
            &format!("file://{}", source.path.display()),
            ".",
        ],
        "",
    );
    let info = clone.path.join(".git/objects/info");
    ok(std::fs::create_dir_all(&info), "making a directory");
    ok(
        std::fs::copy(
            source.path.join(".git/objects/info/commit-graph"),
            info.join("commit-graph"),
        ),
        "copying the commit-graph",
    );
    found.extend(mismatches("shallow clone", &clone, false, &refs));

    // Each substitution changes the order git gives, so reading the graph's parents could not
    // have passed by chance.
    let plain_order = oracle(&plain, &tips_for(&plain, Reading::Refs));
    for substituted in [&replaced, &grafted, &shallow] {
        let order = oracle(substituted, &tips_for(substituted, Reading::Refs));
        assert_ne!(order, plain_order, "a substitution git ignored");
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}
