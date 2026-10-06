//! The refs snapshot, ahead/behind and the ref-storage refusal (PRD R1, R2; C1, C2, C3).
//! Every expectation is read out of the `git` binary, run in the same fixture at test
//! time: `git for-each-ref`, `git cat-file --batch-check`, `git symbolic-ref`,
//! `git rev-parse`, `git stash list` and `git rev-list --left-right --count`. None is typed
//! in from a run of Cairn.
//!
//! git, the oracle, runs with the machine's configuration shut out; Cairn's side opens in
//! this test process, which reads the developer's global configuration as Cairn does. So
//! every key the snapshot reads that a global file could set is set again in each
//! fixture's own configuration (`pin_configuration_read`), and both sides read the same
//! value.

mod fixtures;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use cairn_git::{Cancel, CancelSignal, Error, Repository, SharedRepository};
use cairn_model::{
    AheadBehind, HeadState, Oid, Ref, RefKind, RefName, RefTarget, RefsSnapshot, StashEntry,
    Upstream,
};

use fixtures::Fixture;

// ── git, the oracle ─────────────────────────────────────────────────────────

/// A directory nothing writes into, for `HOME` and `XDG_CONFIG_HOME`.
fn empty_home() -> &'static Path {
    static HOME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    HOME.get_or_init(|| {
        let path = std::env::temp_dir().join("cairn-refs-tests-empty-home");
        std::fs::create_dir_all(&path).unwrap_or_else(|e| panic!("{e}"));
        path
    })
}

/// `git` in `dir`, isolated from the machine's configuration and WITHOUT `GIT_NAMESPACE`,
/// which a test's own process may carry; `input` is its stdin. Never panics: the caller
/// reads the status.
fn oracle(dir: &Path, args: &[&str], input: Option<&[u8]>) -> Output {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args(args)
        .env_remove("GIT_NAMESPACE")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", empty_home())
        .env("XDG_CONFIG_HOME", empty_home())
        .env("GIT_AUTHOR_NAME", "A U Thor")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_NAME", "C O Mitter")
        .env("GIT_COMMITTER_EMAIL", "committer@example.com")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .unwrap_or_else(|e| panic!("could not run git {args:?}: {e}"));
    if let Some(input) = input {
        use std::io::Write as _;
        let mut stdin = child
            .stdin
            .take()
            .unwrap_or_else(|| panic!("nothing there"));
        let input = input.to_vec();
        std::thread::spawn(move || stdin.write_all(&input).unwrap_or_else(|e| panic!("{e}")));
    }
    child.wait_with_output().unwrap_or_else(|e| panic!("{e}"))
}

/// [`oracle`]'s stdout, failing the test when git failed.
fn ask(dir: &Path, args: &[&str]) -> String {
    let output = oracle(dir, args, None);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap_or_else(|e| panic!("{e}"))
}

fn oid(hex: &str) -> Oid {
    Oid::parse(hex.trim()).unwrap_or_else(|e| panic!("{hex:?} is not an id: {e:?}"))
}

/// What `name^{}` is — the object at the end of its tag chain — as `cat-file` says,
/// for every name at once: `(id, type)`, or `None` where git finds nothing.
fn peeled(dir: &Path, names: &[String]) -> Vec<Option<(Oid, String)>> {
    if names.is_empty() {
        return Vec::new();
    }
    let input: String = names.iter().map(|name| format!("{name}^{{}}\n")).collect();
    let output = oracle(
        dir,
        &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
        Some(input.as_bytes()),
    );
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap_or_else(|e| panic!("{e}"));
    let answers: Vec<_> = text
        .lines()
        .map(|line| {
            if line.ends_with(" missing") || line.ends_with(" ambiguous") {
                None
            } else {
                let (id, kind) = line
                    .split_once(' ')
                    .unwrap_or_else(|| panic!("nothing there"));
                Some((oid(id), kind.to_owned()))
            }
        })
        .collect();
    assert_eq!(answers.len(), names.len(), "{text}");
    answers
}

/// Every local branch, remote-tracking ref and tag as `git for-each-ref` lists them, in
/// its order, each with what it names, what that peels to and — for a local branch — its
/// `%(upstream)`, `[gone]` read from `%(upstream:track)`.
fn git_refs(dir: &Path) -> Vec<Ref> {
    let listed = ask(
        dir,
        &[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)%00%(objecttype)%00%(symref)%00%(upstream)%00%(upstream:track)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
    );
    let rows: Vec<Vec<String>> = listed
        .lines()
        .map(|line| line.split('\0').map(str::to_owned).collect())
        .collect();
    let tags: Vec<String> = rows
        .iter()
        .filter(|row| row[2] == "tag")
        .map(|row| row[0].clone())
        .collect();
    let mut tag_ends = tags
        .iter()
        .cloned()
        .zip(peeled(dir, &tags))
        .collect::<BTreeMap<_, _>>();
    let upstreams: Vec<String> = rows
        .iter()
        .filter(|row| !row[4].is_empty() && !row[5].contains("[gone]"))
        .map(|row| row[4].clone())
        .collect();
    let upstream_ends = upstreams
        .iter()
        .cloned()
        .zip(peeled(dir, &upstreams))
        .collect::<BTreeMap<_, _>>();
    let commit_of = |end: &Option<(Oid, String)>| match end {
        Some((id, kind)) if kind == "commit" => Some(*id),
        _ => None,
    };
    rows.iter()
        .map(|row| {
            let name = &row[0];
            let kind = if name.starts_with("refs/heads/") {
                RefKind::LocalBranch
            } else if name.starts_with("refs/remotes/") {
                RefKind::RemoteTracking
            } else {
                RefKind::Tag
            };
            let object = oid(&row[1]);
            let target = match row[2].as_str() {
                "commit" => RefTarget::Commit(object),
                "tag" => RefTarget::Tag {
                    object,
                    commit: commit_of(
                        &tag_ends
                            .remove(name)
                            .unwrap_or_else(|| panic!("nothing there")),
                    ),
                },
                "tree" | "blob" => RefTarget::Other(object),
                other => panic!("for-each-ref printed an object type {other:?}"),
            };
            let upstream = (!row[4].is_empty()).then(|| {
                let name = RefName::new(row[4].clone());
                if row[5].contains("[gone]") {
                    Upstream::Gone { name }
                } else {
                    Upstream::Exists {
                        commit: commit_of(&upstream_ends[&row[4]]),
                        name,
                    }
                }
            });
            Ref {
                name: RefName::new(name.clone()),
                kind,
                target,
                symbolic: (!row[3].is_empty()).then(|| RefName::new(row[3].clone())),
                upstream,
            }
        })
        .collect()
}

/// `HEAD` as `git symbolic-ref` and `git rev-parse` see it.
fn git_head(dir: &Path) -> HeadState {
    let symbolic = oracle(dir, &["symbolic-ref", "-q", "HEAD"], None);
    let commit = oracle(dir, &["rev-parse", "-q", "--verify", "HEAD"], None);
    let text = |output: &Output| String::from_utf8_lossy(&output.stdout).trim().to_owned();
    match (symbolic.status.success(), commit.status.success()) {
        (true, true) => HeadState::Branch(RefName::new(text(&symbolic))),
        (true, false) => HeadState::Unborn(RefName::new(text(&symbolic))),
        (false, true) => HeadState::Detached(oid(&text(&commit))),
        (false, false) => panic!("git can read no HEAD in {}", dir.display()),
    }
}

/// The stash list as `git stash list` prints it: the index git gives each entry (from
/// `stash@{n}`), reflog subject, commit and its first parent.
fn git_stashes(dir: &Path) -> Vec<StashEntry> {
    ask(dir, &["stash", "list", "--format=%gd%x00%H%x00%gs%x00%P"])
        .lines()
        .map(|line| {
            let fields: Vec<&str> = line.split('\0').collect();
            let index = fields[0]
                .strip_prefix("stash@{")
                .and_then(|rest| rest.strip_suffix('}'))
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("git named a stash {:?}", fields[0]));
            StashEntry {
                index,
                message: fields[2].to_owned(),
                commit: oid(fields[1]),
                base: oid(fields[3]
                    .split(' ')
                    .next()
                    .unwrap_or_else(|| panic!("nothing there"))),
            }
        })
        .collect()
}

fn snapshot(dir: &Path) -> RefsSnapshot {
    Repository::discover(dir)
        .unwrap_or_else(|e| panic!("opening {}: {e}", dir.display()))
        .refs(&CancelSignal::new())
        .unwrap_or_else(|e| panic!("reading the refs: {e}"))
        .snapshot
}

/// The snapshot's refs, `HEAD` and stash list are git's. The unreadable count is the
/// caller's to check: git only warns.
fn assert_snapshot_is_gits(dir: &Path) -> RefsSnapshot {
    let cairn = snapshot(dir);
    let expected = git_refs(dir);
    assert_eq!(
        cairn.refs.len(),
        expected.len(),
        "listed {:#?}\ngit lists {:#?}",
        cairn
            .refs
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        expected.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
    );
    for (cairn, git) in cairn.refs.iter().zip(&expected) {
        assert_eq!(cairn, git, "{} differs from git's", git.name.as_str());
    }
    assert_eq!(cairn.head, git_head(dir), "HEAD differs from git's");
    assert_eq!(
        cairn.stashes,
        git_stashes(dir),
        "the stash list differs from git's"
    );
    cairn
}

// ── Fixtures ────────────────────────────────────────────────────────────────

fn commit(fixture: &Fixture, clock: &mut i64, message: &str) {
    *clock += 60;
    fixtures::run(
        fixture.path(),
        &["commit", "--quiet", "--allow-empty", "-m", message],
        Some(*clock),
    );
}

fn write(fixture: &Fixture, path: &str, content: &str) {
    let path = fixture.path().join(path);
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("nothing there")))
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(path, content).unwrap_or_else(|e| panic!("{e}"));
}

/// Sets, in the fixture's own configuration, every key the snapshot reads that a global
/// file could also set, so Cairn — which opens with the developer's global configuration,
/// a process's environment being its own — and git, isolated from it, read the same value.
/// Residual: a global `branch.<name>.*` or `remote.<name>.fetch` entry adds to the local
/// ones for the upstream read, and no fixture can pin that away.
fn pin_configuration_read(fixture: &Fixture) {
    fixture.git(&["config", "core.warnAmbiguousRefs", "true"]);
}

/// Every kind of ref C1 names: loose and packed refs, a loose ref over a packed one, a
/// packed annotated tag with its peeled line and a loose one without, lightweight,
/// annotated and chained tags (a packed chain and a loose one of three), tags on a tree and a blob, `origin/HEAD`, a symbolic local
/// branch, dangling symbolic refs one and two levels deep, an unreadable ref and invalid
/// names.
fn every_kind() -> Fixture {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    write(&fixture, "file.txt", "one\n");
    fixture.git(&["add", "file.txt"]);
    commit(&fixture, &mut clock, "first");
    commit(&fixture, &mut clock, "second");
    commit(&fixture, &mut clock, "third");
    fixture.git(&["branch", "feature"]);
    fixture.git(&["branch", "side", "HEAD~1"]);
    fixture.git(&["tag", "light", "HEAD~1"]);
    fixture.git(&["tag", "-a", "annotated", "-m", "annotated", "HEAD~2"]);
    fixture.git(&["tag", "-a", "inner", "-m", "inner", "HEAD"]);
    fixture.git(&["tag", "-a", "outer", "-m", "outer", "inner"]);
    fixture.git(&["tag", "-a", "outermost", "-m", "outermost", "outer"]);
    fixture.git(&["tag", "ontree", "HEAD^{tree}"]);
    fixture.git(&[
        "tag",
        "-a",
        "annotated-tree",
        "-m",
        "on a tree",
        "HEAD^{tree}",
    ]);
    fixture.git(&["tag", "onblob", "HEAD:file.txt"]);
    fixture.git(&["remote", "add", "origin", "https://example.invalid/r.git"]);
    fixture.git(&["update-ref", "refs/remotes/origin/main", "HEAD~1"]);
    fixture.git(&["update-ref", "refs/remotes/origin/feature", "HEAD"]);
    fixture.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);
    fixture.git(&["config", "branch.main.remote", "origin"]);
    fixture.git(&["config", "branch.main.merge", "refs/heads/main"]);
    fixture.git(&["config", "branch.side.remote", "origin"]);
    fixture.git(&["config", "branch.side.merge", "refs/heads/side"]);
    fixture.git(&["config", "branch.feature.remote", "."]);
    fixture.git(&["config", "branch.feature.merge", "refs/heads/main"]);
    pin_configuration_read(&fixture);
    fixture.git(&["pack-refs", "--all"]);
    // Loose after packing: a new branch, a loose ref over its packed self, and a loose
    // annotated tag (no peeled line to read).
    fixture.git(&["branch", "loose", "HEAD~2"]);
    fixture.git(&["update-ref", "refs/heads/side", "HEAD"]);
    fixture.git(&["tag", "-a", "loose-annotated", "-m", "loose", "HEAD~1"]);
    // A loose chain of three tags: no packed `^` line answers it, so it is peeled object
    // by object.
    fixture.git(&["tag", "-a", "loose-chain-1", "-m", "one", "HEAD~1"]);
    fixture.git(&["tag", "-a", "loose-chain-2", "-m", "two", "loose-chain-1"]);
    fixture.git(&["tag", "-a", "loose-chain-3", "-m", "three", "loose-chain-2"]);
    fixture.git(&["symbolic-ref", "refs/heads/alias", "refs/heads/main"]);
    fixture.git(&[
        "symbolic-ref",
        "refs/remotes/origin/dangling",
        "refs/remotes/origin/nothere",
    ]);
    fixture.git(&["symbolic-ref", "refs/heads/deep1", "refs/heads/deep2"]);
    fixture.git(&["symbolic-ref", "refs/heads/deep2", "refs/heads/nothere"]);
    let head = ask(fixture.path(), &["rev-parse", "HEAD"]);
    write(&fixture, ".git/refs/heads/bad..name", &head);
    write(&fixture, ".git/refs/heads/held.lock", &head);
    write(&fixture, ".git/refs/heads/broken", "this is not a ref\n");
    fixture
}

// ── C1: the snapshot is git's ───────────────────────────────────────────────

/// C1 over every kind of ref, with `HEAD` on a branch, then detached, then unborn. The
/// fixture is first checked to hold what it claims: a packed annotated tag with its `^`
/// line, a loose ref over its packed self, `origin/HEAD` symbolic. Caught by: peeling a
/// symbolic ref into its target's name (`origin/main` twice, `origin/HEAD` gone), listing
/// a dangling one, a tag chain followed one level or a tag's object lost to its commit, a
/// tag on a tree taken for a commit, the packed ref read over the loose one, an
/// unreadable ref failing the snapshot, or `HEAD` read as anything but what git says.
#[test]
fn the_snapshot_is_what_git_lists_whatever_the_kind_of_ref_and_head() {
    let fixture = every_kind();
    let dir = fixture.path();
    let packed = std::fs::read_to_string(dir.join(".git/packed-refs")).unwrap();
    assert!(
        packed.lines().any(|line| line.starts_with('^')),
        "the fixture has no packed ref with a peeled line:\n{packed}"
    );
    assert!(packed.contains("refs/heads/side") && dir.join(".git/refs/heads/side").is_file());
    let listed = ask(dir, &["for-each-ref", "--format=%(refname) %(symref)"]);
    assert!(
        listed.contains("refs/remotes/origin/HEAD refs/remotes/origin/main"),
        "{listed}"
    );

    let cairn = assert_snapshot_is_gits(dir);
    let origin_main = cairn
        .refs
        .iter()
        .filter(|r| r.name.as_str() == "refs/remotes/origin/main")
        .count();
    assert_eq!(origin_main, 1, "origin/main listed {origin_main} times");
    let origin_head = cairn
        .find(&RefName::new("refs/remotes/origin/HEAD"))
        .expect("origin/HEAD is not listed as itself");
    assert_eq!(
        origin_head.symbolic.as_ref().map(RefName::as_str),
        Some("refs/remotes/origin/main")
    );
    // The rows that decide the most, spelled out on top of the comparison with git.
    let tag = |name: &str| cairn.find(&RefName::new(name)).unwrap().target;
    let RefTarget::Tag { object, commit } = tag("refs/tags/outermost") else {
        panic!(
            "the chained tag is not a tag: {:?}",
            tag("refs/tags/outermost")
        );
    };
    assert_eq!(
        object,
        oid(&ask(dir, &["rev-parse", "refs/tags/outermost"]))
    );
    assert_eq!(
        commit,
        Some(oid(&ask(
            dir,
            &["rev-parse", "refs/tags/outermost^{commit}"]
        )))
    );
    let RefTarget::Tag { object, commit } = tag("refs/tags/loose-chain-3") else {
        panic!("the loose chain is not a tag");
    };
    assert_eq!(
        object,
        oid(&ask(dir, &["rev-parse", "refs/tags/loose-chain-3"]))
    );
    assert_eq!(
        commit,
        Some(oid(&ask(
            dir,
            &["rev-parse", "refs/tags/loose-chain-3^{commit}"]
        )))
    );
    assert!(
        !packed.contains("refs/tags/loose-chain-3"),
        "the loose chain was packed, so its peeled line answers it"
    );
    assert!(matches!(
        tag("refs/tags/annotated-tree"),
        RefTarget::Tag { commit: None, .. }
    ));
    assert!(matches!(tag("refs/tags/ontree"), RefTarget::Other(_)));
    assert!(matches!(tag("refs/tags/onblob"), RefTarget::Other(_)));
    assert_eq!(
        cairn.unreadable, 1,
        "the unreadable ref (and only it) is counted; dangling refs and invalid names are not"
    );

    fixture.git(&["checkout", "--quiet", "--detach", "HEAD~1"]);
    let detached = assert_snapshot_is_gits(dir);
    assert!(matches!(detached.head, HeadState::Detached(_)));

    fixture.git(&["checkout", "--quiet", "--orphan", "newroot"]);
    let unborn = assert_snapshot_is_gits(dir);
    assert_eq!(
        unborn.head,
        HeadState::Unborn(RefName::new("refs/heads/newroot"))
    );
}

/// C1 over a braided history of merges, in a repository whose objects are named by SHA-1
/// and one named by SHA-256, each with an annotated tag packed and a lightweight one loose.
/// Caught by: an id of the other width refused or truncated.
#[test]
fn the_snapshot_is_gits_over_a_braided_history_in_either_hash() {
    for fixture in [fixtures::braided(4), fixtures::braided_in("sha256", 6)] {
        fixture.git(&["tag", "-a", "release", "-m", "release", "HEAD~1"]);
        fixture.git(&["pack-refs", "--all"]);
        fixture.git(&["tag", "light", "side"]);
        let cairn = assert_snapshot_is_gits(fixture.path());
        let main = cairn.find(&RefName::new("refs/heads/main")).unwrap();
        assert_eq!(
            main.commit_id().map(|id| id.to_string()),
            fixture.rev_list().first().cloned()
        );
    }
}

/// C1 from a linked worktree: its `HEAD` is its own, and the refs are the ones git lists
/// there. The worktree's private `refs/worktree/` is outside the three namespaces both
/// sides list, so its exclusion holds by construction and this decides nothing about it.
/// Caught by: the main worktree's `HEAD` read from the linked one.
#[test]
fn a_linked_worktree_lists_what_git_lists_there_with_its_own_head() {
    let fixture = every_kind();
    let worktree = fixture.path().join("linked");
    fixture.git(&[
        "worktree",
        "add",
        "--quiet",
        "-b",
        "in-worktree",
        worktree.to_str().unwrap(),
        "HEAD~1",
    ]);
    ask(&worktree, &["update-ref", "refs/worktree/private", "HEAD"]);
    let cairn = assert_snapshot_is_gits(&worktree);
    assert_eq!(
        cairn.head,
        HeadState::Branch(RefName::new("refs/heads/in-worktree"))
    );
    assert_eq!(
        snapshot(fixture.path()).head,
        HeadState::Branch(RefName::new("refs/heads/main"))
    );
}

/// C1's stash list: forty entries, one with a message over 4 KiB that is neither the
/// newest nor the oldest, and one made with `--include-untracked`. Caught by: gix's
/// newest-first reader, which stops at the long line and lists nothing past it, or an
/// index counted from the wrong end.
#[test]
fn the_stash_list_is_git_stash_lists_with_forty_entries_and_a_long_message() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    write(&fixture, "file.txt", "base\n");
    fixture.git(&["add", "file.txt"]);
    commit(&fixture, &mut clock, "base");
    let long = "x".repeat(5_000);
    for entry in 0..40 {
        write(&fixture, "file.txt", &format!("change {entry}\n"));
        if entry == 25 {
            write(&fixture, "untracked.txt", "new\n");
            fixture.git(&[
                "stash",
                "push",
                "--quiet",
                "--include-untracked",
                "-m",
                "with untracked",
            ]);
        } else if entry == 10 {
            fixture.git(&["stash", "push", "--quiet", "-m", &long]);
        } else {
            fixture.git(&["stash", "push", "--quiet", "-m", &format!("entry {entry}")]);
        }
    }
    let cairn = assert_snapshot_is_gits(fixture.path());
    assert_eq!(cairn.stashes.len(), 40);
    let long_one = cairn
        .stashes
        .iter()
        .find(|entry| entry.message.len() > 4096)
        .unwrap();
    assert!(
        long_one.index > 0 && long_one.index < 39,
        "the long message is at the end of the list, which proves nothing about reading past it"
    );
    assert_eq!(cairn.unreadable, 0);
}

/// C1's upstreams: every case measured, each against `%(upstream)` — the ones gix answers
/// differently (two `merge` values, a short `merge` with a named remote, two refspecs that
/// both map the merge, `remote = .`) and the ones it agrees on (gone, a local upstream
/// short or full, no remote, an unknown remote, a remote given as a URL, `remote` set
/// twice, a negative refspec, a remote with no fetch refspec, a short local name that
/// resolves to two refs). Caught by: any of parity rules 4 and 5 undone.
#[test]
fn each_upstream_is_what_git_resolves() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "first");
    let config = |key: &str, value: &str| {
        fixture.git(&["config", "--add", key, value]);
    };
    fixture.git(&["remote", "add", "origin", "https://example.invalid/o.git"]);
    fixture.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    fixture.git(&["update-ref", "refs/remotes/origin/other", "HEAD"]);
    config("remote.r1.url", "https://example.invalid/1.git");
    config(
        "remote.r1.fetch",
        "+refs/heads/main:refs/remotes/first/main",
    );
    config("remote.r1.fetch", "+refs/heads/*:refs/remotes/r1/*");
    config("remote.neg.url", "https://example.invalid/2.git");
    config("remote.neg.fetch", "+refs/heads/*:refs/remotes/neg/*");
    config("remote.neg.fetch", "^refs/heads/main");
    config("remote.nofetch.url", "https://example.invalid/3.git");
    pin_configuration_read(&fixture);
    fixture.git(&["tag", "amb"]);
    let cases: &[(&str, &[(&str, &str)])] = &[
        (
            "gone",
            &[("remote", "origin"), ("merge", "refs/heads/gone")],
        ),
        ("local", &[("remote", "."), ("merge", "refs/heads/main")]),
        ("local-short", &[("remote", "."), ("merge", "main")]),
        (
            "local-gone",
            &[("remote", "."), ("merge", "refs/heads/nothere")],
        ),
        ("local-ambiguous", &[("remote", "."), ("merge", "amb")]),
        (
            "two-merges",
            &[
                ("remote", "origin"),
                ("merge", "refs/heads/main"),
                ("merge", "refs/heads/other"),
            ],
        ),
        (
            "two-local-merges",
            &[
                ("remote", "."),
                ("merge", "refs/heads/main"),
                ("merge", "refs/heads/gone"),
            ],
        ),
        ("short-merge", &[("remote", "origin"), ("merge", "main")]),
        ("no-remote", &[("merge", "refs/heads/main")]),
        (
            "unknown-remote",
            &[("remote", "nosuch"), ("merge", "refs/heads/main")],
        ),
        (
            "remote-url",
            &[
                ("remote", "https://example.invalid/o.git"),
                ("merge", "refs/heads/main"),
            ],
        ),
        (
            "two-remotes",
            &[
                ("remote", "nosuch"),
                ("remote", "origin"),
                ("merge", "refs/heads/main"),
            ],
        ),
        (
            "two-fetch-specs",
            &[("remote", "r1"), ("merge", "refs/heads/main")],
        ),
        (
            "negated",
            &[("remote", "neg"), ("merge", "refs/heads/main")],
        ),
        (
            "no-fetch",
            &[("remote", "nofetch"), ("merge", "refs/heads/main")],
        ),
        ("no-merge", &[("remote", "origin")]),
    ];
    for (branch, settings) in cases {
        fixture.git(&["branch", branch]);
        for (key, value) in *settings {
            config(&format!("branch.{branch}.{key}"), value);
        }
    }
    fixture.git(&["branch", "amb"]);
    let cairn = assert_snapshot_is_gits(fixture.path());
    // The cases gix answers otherwise, spelled out against git on top of the comparison.
    let upstream = |branch: &str| {
        cairn
            .find(&RefName::new(format!("refs/heads/{branch}")))
            .unwrap()
            .upstream
            .as_ref()
            .map(|upstream| upstream.name().as_str().to_owned())
    };
    let gits = |branch: &str| {
        let printed = ask(
            fixture.path(),
            &[
                "for-each-ref",
                "--format=%(upstream)",
                &format!("refs/heads/{branch}"),
            ],
        );
        Some(printed.trim().to_owned()).filter(|name| !name.is_empty())
    };
    for branch in [
        "two-merges",
        "two-local-merges",
        "short-merge",
        "two-fetch-specs",
        "local",
        "local-ambiguous",
    ] {
        assert_eq!(upstream(branch), gits(branch), "{branch}");
    }
    assert_eq!(
        gits("short-merge"),
        None,
        "git changed how it maps a short merge"
    );
    assert_eq!(
        gits("two-fetch-specs").as_deref(),
        Some("refs/remotes/first/main")
    );
}

/// R1.8, C1: with `GIT_NAMESPACE` set in Cairn's own environment the snapshot is still
/// `git for-each-ref`'s without it. A process's environment is fixed for its threads, so
/// the half that opens the repository runs in a child of this test binary with the
/// variable set; gix, opened there as it opens by default, is checked to see the
/// namespace, so the variable is live. Caught by: the namespace gix reads at open left in
/// place, which lists `refs/heads/inside` and no `refs/heads/main`.
#[test]
fn git_namespace_in_cairns_environment_is_not_honoured() {
    const CHILD: &str = "CAIRN_TEST_NAMESPACE_CHILD";
    if let Some(dir) = std::env::var_os(CHILD) {
        let dir = PathBuf::from(dir);
        assert_eq!(std::env::var("GIT_NAMESPACE").as_deref(), Ok("ns"));
        // git's own `for-each-ref` does not read the variable; gix, opened as it opens
        // by default, does, and lists the namespace's refs under their own names.
        let gix_names: Vec<String> = gix::open(&dir)
            .unwrap()
            .references()
            .unwrap()
            .all()
            .unwrap()
            .map(|reference| reference.unwrap().name().as_bstr().to_string())
            .collect();
        assert!(
            gix_names.iter().any(|name| name == "refs/heads/inside"),
            "gix does not read GIT_NAMESPACE here, so this decides nothing: {gix_names:?}"
        );
        let cairn = assert_snapshot_is_gits(&dir);
        assert!(
            cairn
                .refs
                .iter()
                .any(|r| r.name.as_str() == "refs/heads/main")
        );
        return;
    }
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "first");
    fixture.git(&["update-ref", "refs/namespaces/ns/refs/heads/inside", "HEAD"]);
    fixture.git(&[
        "symbolic-ref",
        "refs/namespaces/ns/HEAD",
        "refs/namespaces/ns/refs/heads/inside",
    ]);
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "git_namespace_in_cairns_environment_is_not_honoured",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CHILD, fixture.path())
        .env("GIT_NAMESPACE", "ns")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the child failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("1 passed"),
        "the child ran no test:\n{stdout}"
    );
}

/// A ref naming an object that is not there is skipped and counted (a decision the user
/// made: git's own `for-each-ref` refuses to list anything at all, `fatal: missing
/// object`). The rest is what git lists by name. Caught by: the whole snapshot failing on
/// it, or the ref listed with a target nobody can read.
#[test]
fn a_ref_naming_a_missing_object_is_skipped_and_counted() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "first");
    fixture.git(&["branch", "fine"]);
    write(
        &fixture,
        ".git/refs/heads/missing",
        "1234567890123456789012345678901234567890\n",
    );
    let refused = oracle(fixture.path(), &["for-each-ref"], None);
    assert!(
        !refused.status.success()
            && String::from_utf8_lossy(&refused.stderr).contains("missing object"),
        "git now lists a ref whose object is missing; revisit this decision: {refused:?}"
    );
    let cairn = snapshot(fixture.path());
    let names: Vec<_> = cairn
        .refs
        .iter()
        .map(|r| r.name.as_str().to_owned())
        .collect();
    let by_name: Vec<_> = ask(fixture.path(), &["for-each-ref", "--format=%(refname)"])
        .lines()
        .filter(|name| *name != "refs/heads/missing")
        .map(str::to_owned)
        .collect();
    assert_eq!(names, by_name);
    assert_eq!(cairn.unreadable, 1);
}

/// `ref_tips`, what the network lane compares before and after a fetch, is the snapshot:
/// it sees a symbolic ref retargeted to a ref on the same commit, and an annotated tag
/// replaced by one on the same commit — each the ONLY change between its two reads, and
/// each invisible to a comparison of peeled ids. Caught by: tips peeled again.
#[test]
fn ref_tips_see_a_symbolic_ref_retargeted_and_a_tag_object_replaced() {
    let fixture = every_kind();
    // Both remote-tracking refs on one commit first, so the retarget moves no id.
    fixture.git(&[
        "update-ref",
        "refs/remotes/origin/feature",
        "refs/remotes/origin/main",
    ]);
    let repo = Repository::discover(fixture.path()).unwrap();
    let before = repo.ref_tips().unwrap();
    fixture.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/feature",
    ]);
    let retargeted = repo.ref_tips().unwrap();
    let peeled = |snapshot: &RefsSnapshot| {
        snapshot
            .refs
            .iter()
            .map(|r| (r.name.clone(), r.commit_id()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        peeled(&before),
        peeled(&retargeted),
        "the retarget moved an id"
    );
    assert_ne!(before, retargeted, "a retargeted symbolic ref was not seen");

    fixture.git(&[
        "tag",
        "-f",
        "-a",
        "annotated",
        "-m",
        "again",
        "annotated^{commit}",
    ]);
    let replaced = repo.ref_tips().unwrap();
    assert_eq!(
        peeled(&retargeted),
        peeled(&replaced),
        "the replacement moved a commit"
    );
    assert_ne!(
        retargeted, replaced,
        "a tag object replaced on the same commit was not seen"
    );
    assert_eq!(replaced, repo.ref_tips().unwrap());
}

/// A chain of `hops` symbolic refs, `refs/heads/{prefix}-1` → ... → `refs/heads/main`.
fn symbolic_chain(fixture: &Fixture, prefix: &str, hops: usize) {
    for hop in 1..=hops {
        let target = if hop == hops {
            "refs/heads/main".to_owned()
        } else {
            format!("refs/heads/{prefix}-{}", hop + 1)
        };
        fixture.git(&[
            "symbolic-ref",
            &format!("refs/heads/{prefix}-{hop}"),
            &target,
        ]);
    }
}

/// git resolves a ref by reading at most five refs, itself included, so a chain of four
/// symbolic hops is listed — naming the END of its chain, as `%(symref)` does — and one of
/// five is not; that one is counted as unreadable here (git hides it silently). Caught by:
/// the depth off by one either way, or `%(symref)` taken as the first hop.
#[test]
fn a_symbolic_chain_resolves_to_gits_depth_and_names_its_end() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "first");
    symbolic_chain(&fixture, "four", 4);
    symbolic_chain(&fixture, "five", 5);
    let listed = ask(fixture.path(), &["for-each-ref", "--format=%(refname)"]);
    assert!(listed.contains("refs/heads/four-1\n"), "{listed}");
    assert!(!listed.contains("refs/heads/five-1\n"), "{listed}");
    let cairn = assert_snapshot_is_gits(fixture.path());
    assert_eq!(
        cairn
            .find(&RefName::new("refs/heads/four-1"))
            .and_then(|r| r.symbolic.clone()),
        Some(RefName::new("refs/heads/main"))
    );
    assert_eq!(
        cairn.unreadable, 1,
        "the five-hop chain is skipped and counted"
    );
}

/// `HEAD` on a branch that is itself a symbolic ref: git's `symbolic-ref HEAD` names the
/// end of the chain, and the branch is unborn when that end does not exist. Caught by:
/// gix's `head()`, which follows one level and calls such a `HEAD` a branch.
#[test]
fn head_through_a_symbolic_branch_is_what_git_says() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "first");
    fixture.git(&["symbolic-ref", "refs/heads/via", "refs/heads/main"]);
    fixture.git(&["symbolic-ref", "HEAD", "refs/heads/via"]);
    let through = assert_snapshot_is_gits(fixture.path());
    assert_eq!(
        through.head,
        HeadState::Branch(RefName::new("refs/heads/main"))
    );
    fixture.git(&["symbolic-ref", "refs/heads/dangling", "refs/heads/nothere"]);
    fixture.git(&["symbolic-ref", "HEAD", "refs/heads/dangling"]);
    let dangling = assert_snapshot_is_gits(fixture.path());
    assert_eq!(
        dangling.head,
        HeadState::Unborn(RefName::new("refs/heads/nothere"))
    );
}

/// A refs query stops at whichever poll is cancelled — listing the refs, resolving an
/// upstream, reading the stash reflog or its entries — and polls nothing after it. Over a
/// fixture with every kind of poll: for each poll in turn, cancelling there is
/// `Error::RefsCancelled` after exactly that many polls. Caught by: any poll point
/// removed, or a loop that goes on after its cancel.
#[test]
fn a_refs_query_stops_at_whichever_poll_is_cancelled() {
    let fixture = every_kind();
    for made in 0..3 {
        write(&fixture, "file.txt", &format!("change {made}\n"));
        fixture.git(&["stash", "push", "--quiet", "-m", &format!("stash {made}")]);
    }
    let repo = Repository::discover(fixture.path()).unwrap();
    let all = CancelAfter {
        after: usize::MAX,
        polls: Cell::new(0),
    };
    let read = repo.refs(&all).unwrap();
    let total = all.polls.get();
    let branches = read.snapshot.of_kind(RefKind::LocalBranch).count();
    // One poll per ref the store yields — listed, hidden as dangling (every_kind has three:
    // origin/dangling, deep1, deep2) or skipped as unreadable — one per local branch's
    // upstream, one per stash reflog line and one per stash entry. Exact, so a poll point
    // removed fails here even though every cancel below still lands.
    let dangling = 3;
    assert_eq!(
        total,
        read.snapshot.refs.len()
            + dangling
            + read.snapshot.unreadable
            + branches
            + 2 * read.snapshot.stashes.len(),
        "{total} polls"
    );
    for after in 0..total {
        let cancel = CancelAfter {
            after,
            polls: Cell::new(0),
        };
        assert!(
            matches!(repo.refs(&cancel), Err(Error::RefsCancelled)),
            "cancelled at poll {}, the query answered",
            after + 1
        );
        assert_eq!(
            cancel.polls.get(),
            after + 1,
            "it polled on after poll {}",
            after + 1
        );
    }
}

/// What a refs query reports it paid, on a fixture small enough to count by hand:
/// `HEAD` (one read, one hop to `main`); five refs listed, `origin/HEAD` one more for its
/// hop; the stash ref; an object looked up for each of the five, the annotated tag read
/// and peeled to its commit (two more), and the stash's commit; one reflog line. Caught
/// by: a cost that counts something else, or nothing.
#[test]
fn a_refs_query_reports_the_cost_it_paid() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    write(&fixture, "file.txt", "one\n");
    fixture.git(&["add", "file.txt"]);
    commit(&fixture, &mut clock, "first");
    fixture.git(&["tag", "light"]);
    fixture.git(&["tag", "-a", "ann", "-m", "ann"]);
    fixture.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    fixture.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);
    write(&fixture, "file.txt", "two\n");
    fixture.git(&["stash", "push", "--quiet"]);
    let read = Repository::discover(fixture.path())
        .unwrap()
        .refs(&CancelSignal::new())
        .unwrap();
    assert_eq!(read.snapshot.refs.len(), 5);
    assert_eq!(read.cost.refs_read, 2 + 5 + 1 + 1, "{:?}", read.cost);
    assert_eq!(read.cost.objects_read, 5 + 2 + 1, "{:?}", read.cost);
    assert_eq!(read.cost.reflog_lines_read, 1, "{:?}", read.cost);
}

/// The stash list kept as git numbers it when the reflog is damaged: a line that does not
/// parse is skipped and takes no number (the entries older than it move up), and an entry
/// whose commit is missing is skipped but keeps its number; both are counted. Caught by: a
/// damaged line numbered, a missing commit's number reused, or either failing the list.
#[test]
fn a_damaged_stash_reflog_is_numbered_as_git_numbers_it() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    write(&fixture, "file.txt", "base\n");
    fixture.git(&["add", "file.txt"]);
    commit(&fixture, &mut clock, "base");
    for made in 0..5 {
        write(&fixture, "file.txt", &format!("change {made}\n"));
        fixture.git(&["stash", "push", "--quiet", "-m", &format!("stash {made}")]);
    }
    let log = fixture.path().join(".git/logs/refs/stash");
    let lines: Vec<String> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut damaged = lines.clone();
    // Oldest first: the second line becomes garbage, the fourth names a missing commit.
    damaged[1] = "this is not a reflog line".to_owned();
    let mut fields: Vec<&str> = lines[3].splitn(3, ' ').collect();
    fields[1] = "1234567890123456789012345678901234567890";
    damaged[3] = fields.join(" ");
    std::fs::write(&log, damaged.join("\n") + "\n").unwrap();
    let git_numbers: Vec<usize> = git_stashes(fixture.path())
        .iter()
        .map(|e| e.index)
        .collect();
    assert_eq!(
        git_numbers,
        [0, 2, 3],
        "git numbers the damaged list otherwise now"
    );
    let cairn = assert_snapshot_is_gits(fixture.path());
    assert_eq!(cairn.unreadable, 2);
}

// ── C3: ahead and behind ────────────────────────────────────────────────────

/// What `git rev-list --left-right --count <branch>...<upstream>` prints.
fn git_ahead_behind(dir: &Path, branch: &str, upstream: &str) -> AheadBehind {
    let counted = ask(
        dir,
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("{branch}...{upstream}"),
        ],
    );
    let (ahead, behind) = counted
        .trim()
        .split_once('\t')
        .unwrap_or_else(|| panic!("nothing there"));
    AheadBehind {
        ahead: ahead.parse().unwrap_or_else(|e| panic!("{e}")),
        behind: behind.parse().unwrap_or_else(|e| panic!("{e}")),
    }
}

/// C3: a branch ahead, behind, diverged (with a merge on each side), equal, with a local
/// upstream, and with a gone upstream, which has no counts. Caught by: either walk not
/// hiding the other side, the two counts swapped, or a gone upstream counted.
#[test]
fn ahead_and_behind_are_what_rev_list_counts() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "root");
    commit(&fixture, &mut clock, "base");
    fixture.git(&["remote", "add", "origin", "https://example.invalid/o.git"]);
    let track = |branch: &str, remote: &str, merge: &str| {
        fixture.git(&["config", &format!("branch.{branch}.remote"), remote]);
        fixture.git(&["config", &format!("branch.{branch}.merge"), merge]);
    };
    let on = |branch: &str, commits: usize, clock: &mut i64| {
        fixture.git(&["checkout", "--quiet", branch]);
        for made in 0..commits {
            commit(&fixture, clock, &format!("{branch} {made}"));
        }
    };
    for name in ["ahead", "behind", "diverged", "equal", "local", "gone"] {
        fixture.git(&["branch", name, "main"]);
        fixture.git(&["update-ref", &format!("refs/remotes/origin/{name}"), "main"]);
        track(name, "origin", &format!("refs/heads/{name}"));
    }
    fixture.git(&["update-ref", "-d", "refs/remotes/origin/gone"]);
    track("local", ".", "refs/heads/main");
    on("ahead", 2, &mut clock);
    // Behind: the upstream moves on without the branch.
    on("main", 3, &mut clock);
    fixture.git(&["update-ref", "refs/remotes/origin/behind", "main"]);
    // The diverged branch's upstream ends in a merge of its own.
    fixture.git(&["checkout", "--quiet", "-b", "upstream-topic", "main~1"]);
    commit(&fixture, &mut clock, "upstream topic");
    fixture.git(&["checkout", "--quiet", "-b", "upstream-side", "main"]);
    clock += 60;
    fixtures::run(
        fixture.path(),
        &["merge", "--quiet", "--no-ff", "--no-edit", "upstream-topic"],
        Some(clock),
    );
    fixture.git(&[
        "update-ref",
        "refs/remotes/origin/diverged",
        "upstream-side",
    ]);
    on("diverged", 2, &mut clock);
    fixture.git(&["checkout", "--quiet", "-b", "topic", "diverged~1"]);
    commit(&fixture, &mut clock, "topic");
    fixture.git(&["checkout", "--quiet", "diverged"]);
    clock += 60;
    fixtures::run(
        fixture.path(),
        &["merge", "--quiet", "--no-ff", "--no-edit", "topic"],
        Some(clock),
    );
    on("local", 1, &mut clock);
    fixture.git(&["checkout", "--quiet", "main"]);

    let repo = Repository::discover(fixture.path()).unwrap();
    let snapshot = repo.refs(&CancelSignal::new()).unwrap().snapshot;
    let read = repo.ahead_behind(&snapshot, &CancelSignal::new()).unwrap();
    let counted: BTreeMap<_, _> = read
        .counts
        .iter()
        .map(|(name, counts)| (name.as_str().to_owned(), *counts))
        .collect();
    let mut expected = BTreeMap::new();
    for (branch, upstream) in [
        ("ahead", "origin/ahead"),
        ("behind", "origin/behind"),
        ("diverged", "origin/diverged"),
        ("equal", "origin/equal"),
        ("local", "main"),
    ] {
        expected.insert(
            format!("refs/heads/{branch}"),
            git_ahead_behind(fixture.path(), branch, upstream),
        );
    }
    assert_eq!(counted, expected);
    // Each shape is what it claims, so the comparison above covers it.
    let shape = |branch: &str| expected[&format!("refs/heads/{branch}")];
    assert!(shape("ahead").ahead > 0 && shape("ahead").behind == 0);
    assert!(shape("behind").ahead == 0 && shape("behind").behind > 0);
    assert!(shape("diverged").ahead > 0 && shape("diverged").behind > 0);
    assert_eq!(shape("equal"), AheadBehind::default());
    assert!(shape("local").ahead > 0 && shape("local").behind > 0);
    assert!(
        !counted.contains_key("refs/heads/gone") && !counted.contains_key("refs/heads/topic"),
        "a branch with a gone upstream, or none, was counted"
    );
    // Every commit counted was read at least once, by the walk that counted it.
    let counted_commits: usize = expected.values().map(|c| c.ahead + c.behind).sum();
    assert!(
        read.commits_read >= counted_commits,
        "{} commits read for {counted_commits} counted",
        read.commits_read
    );
}

/// Cancels on its `after`-th poll and counts every poll.
struct CancelAfter {
    after: usize,
    polls: Cell<usize>,
}

impl Cancel for CancelAfter {
    fn is_cancelled(&self) -> bool {
        self.polls.set(self.polls.get() + 1);
        self.polls.get() > self.after
    }
}

/// `count` empty commits on top of `from`, on `branch`, through one `git fast-import`,
/// committer dates rising from `first_date`.
fn long_branch(fixture: &Fixture, branch: &str, from: &str, count: usize, first_date: i64) {
    let mut stream = String::new();
    for made in 1..=count {
        stream.push_str(&format!(
            "commit refs/heads/{branch}\nmark :{made}\ncommitter C <c@example.com> {} +0000\ndata 2\nc\n",
            first_date + made as i64
        ));
        if made == 1 {
            stream.push_str(&format!("from {from}\n"));
        }
        stream.push('\n');
    }
    let imported = oracle(
        fixture.path(),
        &["fast-import", "--quiet"],
        Some(stream.as_bytes()),
    );
    assert!(imported.status.success(), "{imported:?}");
}

/// C3: a cancelled ahead/behind stops its walk rather than finishing it and dropping the
/// answer — DURING the frontier gix paints before a hiding walk's first commit, the one
/// long call in it. The counted side is one commit past the base; its upstream, the hidden
/// side, is two thousand past it, so counting the one commit means painting the two
/// thousand. Cancelled on the twentieth poll, the query has read about twenty objects when
/// it stops; with the read poll removed (`history::walk`'s `Polled`) the paint reads all
/// two thousand first. Also: a query cancelled between two counted branches answers the
/// first (`branches: 1`), and one cancelled before it starts answers none. Caught by: a
/// walk that polls nothing, or polls only between the commits it yields.
#[test]
fn a_cancelled_ahead_behind_stops_its_walk() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "root");
    let base = ask(fixture.path(), &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    long_branch(&fixture, "long", &base, 2000, fixtures::EPOCH + 120);
    fixture.git(&["branch", "short", &base]);
    fixture.git(&["checkout", "--quiet", "short"]);
    commit(&fixture, &mut clock, "short");
    fixture.git(&["checkout", "--quiet", "main"]);
    fixture.git(&["config", "branch.short.remote", "."]);
    fixture.git(&["config", "branch.short.merge", "refs/heads/long"]);
    // A branch sorting first whose upstream is itself: counted without a walk.
    fixture.git(&["branch", "aaa", &base]);
    fixture.git(&["config", "branch.aaa.remote", "."]);
    fixture.git(&["config", "branch.aaa.merge", "refs/heads/aaa"]);
    let git_says = git_ahead_behind(fixture.path(), "short", "long");
    assert_eq!(
        git_says,
        AheadBehind {
            ahead: 1,
            behind: 2000
        }
    );

    let repo = Repository::discover(fixture.path()).unwrap();
    let snapshot = repo.refs(&CancelSignal::new()).unwrap().snapshot;
    let uncancelled = CancelAfter {
        after: usize::MAX,
        polls: Cell::new(0),
    };
    let counted = repo.ahead_behind(&snapshot, &uncancelled).unwrap();
    assert_eq!(
        counted.counts,
        vec![
            (RefName::new("refs/heads/aaa"), AheadBehind::default()),
            (RefName::new("refs/heads/short"), git_says),
        ]
    );
    assert!(
        counted.commits_read > 2000,
        "the count read only {} commits, so the paint this test cancels is not long",
        counted.commits_read
    );

    // Polls 1 and 2 are the two branches' own; the third is the first read of the paint.
    let cancel = CancelAfter {
        after: 20,
        polls: Cell::new(0),
    };
    match repo.ahead_behind(&snapshot, &cancel) {
        Err(Error::AheadBehindCancelled {
            branches: 1,
            commits_read,
        }) => assert!(
            commits_read <= 25,
            "cancelled at the twentieth poll, the paint went on to read {commits_read} commits"
        ),
        other => panic!("not cancelled within the paint: {other:?}"),
    }
    // The 21st poll cancels; the one after it is the walk asking whether the read it saw
    // fail was refused for the cancel. Nothing polls after that.
    assert!(
        cancel.polls.get() <= 22,
        "it polled {} times",
        cancel.polls.get()
    );

    let between = CancelAfter {
        after: 1,
        polls: Cell::new(0),
    };
    assert!(matches!(
        repo.ahead_behind(&snapshot, &between),
        Err(Error::AheadBehindCancelled {
            branches: 1,
            commits_read: 0
        })
    ));

    let cancelled = CancelSignal::new();
    cancelled.cancel();
    assert!(matches!(
        repo.ahead_behind(&snapshot, &cancelled),
        Err(Error::AheadBehindCancelled {
            branches: 0,
            commits_read: 0
        })
    ));
}

/// C3 where committer dates run against the graph: the base is dated after every commit
/// on both sides, and one side's commits are dated in decreasing order, so an order by
/// date meets the merge base first. Counts equal `git rev-list --left-right --count`.
/// Caught by: a count that trusts dates to find where the two sides meet.
#[test]
fn ahead_and_behind_are_what_rev_list_counts_when_dates_run_backwards() {
    let fixture = fixtures::unborn();
    commit(&fixture, &mut (fixtures::EPOCH + 100_000), "root");
    commit(
        &fixture,
        &mut (fixtures::EPOCH + 900_000),
        "base, dated after everything above it",
    );
    let base = ask(fixture.path(), &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    fixture.git(&["branch", "forward", &base]);
    fixture.git(&["checkout", "--quiet", "forward"]);
    let mut rising = fixtures::EPOCH + 200_000;
    for made in 0..5 {
        commit(&fixture, &mut rising, &format!("forward {made}"));
    }
    fixture.git(&["checkout", "--quiet", "-b", "backward", &base]);
    for made in 0..7 {
        let mut falling = fixtures::EPOCH + 600_000 - 10_000 * made;
        commit(&fixture, &mut falling, &format!("backward {made}"));
    }
    // A merge on the backward side, of a commit dated before the root.
    fixture.git(&["checkout", "--quiet", "-b", "ancient", &base]);
    commit(&fixture, &mut (fixtures::EPOCH + 50), "ancient");
    fixture.git(&["checkout", "--quiet", "backward"]);
    fixtures::run(
        fixture.path(),
        &["merge", "--quiet", "--no-ff", "--no-edit", "ancient"],
        Some(fixtures::EPOCH + 300_000),
    );
    fixture.git(&["checkout", "--quiet", "main"]);
    for (branch, upstream) in [("forward", "backward"), ("backward", "forward")] {
        fixture.git(&["config", &format!("branch.{branch}.remote"), "."]);
        fixture.git(&[
            "config",
            &format!("branch.{branch}.merge"),
            &format!("refs/heads/{upstream}"),
        ]);
    }
    let repo = Repository::discover(fixture.path()).unwrap();
    let snapshot = repo.refs(&CancelSignal::new()).unwrap().snapshot;
    let counted = repo.ahead_behind(&snapshot, &CancelSignal::new()).unwrap();
    assert_eq!(
        counted.counts,
        vec![
            (
                RefName::new("refs/heads/backward"),
                git_ahead_behind(fixture.path(), "backward", "forward")
            ),
            (
                RefName::new("refs/heads/forward"),
                git_ahead_behind(fixture.path(), "forward", "backward")
            ),
        ]
    );
}

/// C3 in a shallow clone, whose boundary commits git reads as having no parents: counts
/// equal `git rev-list --left-right --count` there, for a divergence that reaches past the
/// boundary. Caught by: the walk following a parent the clone cut off.
#[test]
fn ahead_and_behind_are_what_rev_list_counts_in_a_shallow_clone() {
    let source = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    for made in 0..8 {
        commit(&source, &mut clock, &format!("main {made}"));
    }
    source.git(&["checkout", "--quiet", "-b", "side", "HEAD~5"]);
    for made in 0..3 {
        commit(&source, &mut clock, &format!("side {made}"));
    }
    source.git(&["checkout", "--quiet", "main"]);
    let clone = source.path().with_extension("shallow");
    let _ = std::fs::remove_dir_all(&clone);
    let url = format!("file://{}", source.path().display());
    let cloned = oracle(
        source.path(),
        &[
            "clone",
            "--quiet",
            "--depth=3",
            "--no-single-branch",
            &url,
            clone.to_str().unwrap_or_else(|| panic!("not UTF-8")),
        ],
        None,
    );
    assert!(cloned.status.success(), "{cloned:?}");
    assert!(
        clone.join(".git/shallow").is_file(),
        "the clone is not shallow"
    );
    ask(&clone, &["branch", "--track", "local-side", "origin/side"]);
    ask(
        &clone,
        &["config", "branch.local-side.merge", "refs/heads/main"],
    );
    let expected = git_ahead_behind(&clone, "local-side", "origin/main");
    let full = git_ahead_behind(source.path(), "side", "main");
    let repo = Repository::discover(&clone).unwrap();
    let snapshot = repo.refs(&CancelSignal::new()).unwrap().snapshot;
    let counted = repo.ahead_behind(&snapshot, &CancelSignal::new()).unwrap();
    let _ = std::fs::remove_dir_all(&clone);
    assert_ne!(
        expected, full,
        "the clone counts what the full repository counts, so its boundary decides nothing"
    );
    assert!(
        counted
            .counts
            .contains(&(RefName::new("refs/heads/local-side"), expected)),
        "{:?}, git says {expected:?}",
        counted.counts
    );
}

// ── C2: ref storage at open ─────────────────────────────────────────────────

/// Whether the git on `PATH` can make a reftable repository, asked with the same
/// `git init` command `scripts/gate.sh`'s `require_reftable_where_possible` runs (here in
/// this file's isolated configuration): `None` when it can.
fn no_reftable_here(at: &Path) -> Option<String> {
    let made = oracle(
        at.parent()
            .unwrap_or_else(|| panic!("{} has no parent", at.display())),
        &[
            "init",
            "--quiet",
            "--ref-format=reftable",
            at.to_str()
                .unwrap_or_else(|| panic!("{} is not UTF-8", at.display())),
        ],
        None,
    );
    (!made.status.success()).then(|| String::from_utf8_lossy(&made.stderr).trim().to_owned())
}

/// C2: a reftable repository, and a linked worktree of one, is refused at open with its
/// reason — Cairn's, not gix's failure on the first ref it reads — and a files repository
/// opens. Skipped where the
/// host's git cannot make a reftable repository (before 2.45), and required wherever it
/// can: `CAIRN_REQUIRE_REFTABLE`, which `scripts/gate.sh`'s `test-full` sets where its
/// probe makes one (`the_reftable_refusal_is_required_wherever_it_can_run`).
#[test]
fn a_reftable_repository_is_refused_at_open_and_a_files_one_opens() {
    let files = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&files, &mut clock, "first");
    SharedRepository::discover(files.path()).unwrap();

    let reftable = files.path().with_extension("reftable");
    let _ = std::fs::remove_dir_all(&reftable);
    if let Some(reason) = no_reftable_here(&reftable) {
        assert!(
            std::env::var_os("CAIRN_REQUIRE_REFTABLE").is_none(),
            "CAIRN_REQUIRE_REFTABLE is set, and the git here cannot make a reftable \
             repository: {reason}"
        );
        eprintln!(
            "SKIPPED a_reftable_repository_is_refused_at_open_and_a_files_one_opens: {reason}"
        );
        return;
    }
    fixtures::run(
        &reftable,
        &["commit", "--quiet", "--allow-empty", "-m", "first"],
        Some(clock),
    );
    assert!(
        reftable.join(".git/reftable").is_dir(),
        "git made no reftable store"
    );
    // git reads it, and from a linked worktree of it, whose own git directory holds no
    // configuration: the setting is read from the common directory.
    ask(&reftable, &["rev-parse", "HEAD"]);
    let linked = reftable.with_extension("linked");
    let _ = std::fs::remove_dir_all(&linked);
    ask(
        &reftable,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "linked",
            linked
                .to_str()
                .unwrap_or_else(|| panic!("{} is not UTF-8", linked.display())),
        ],
    );
    ask(&linked, &["rev-parse", "HEAD"]);
    let refused_linked = SharedRepository::discover(&linked);
    let refused = SharedRepository::discover(&reftable);
    let _ = std::fs::remove_dir_all(&reftable);
    let _ = std::fs::remove_dir_all(&linked);
    assert!(
        matches!(&refused_linked, Err(Error::RefStorageUnsupported { storage, .. }) if storage == "reftable"),
        "a linked worktree of a reftable repository was not refused: {refused_linked:?}"
    );
    match refused {
        Err(error @ Error::RefStorageUnsupported { .. }) => {
            let Error::RefStorageUnsupported { storage, .. } = &error else {
                unreachable!()
            };
            assert_eq!(storage, "reftable");
            assert!(error.to_string().contains("reftable"), "{error}");
        }
        other => panic!("a reftable repository was not refused for its refs: {other:?}"),
    }
}

/// A repository's format, set by hand on a files repository, against git reading the same
/// fixture: `extensions.refStorage = reftable` refused (whatever the refs really are, so
/// the refusal is the configuration's); `files` in a format-version-1 repository opened,
/// as git opens it; any `refStorage` in a format-version-0 repository — `files` or
/// `reftable` — refused for its version, as git refuses it ("v1-only extension"); and a
/// repository with no `config` file at all opened, as git and gix open it. A git before
/// 2.45 knows no `refStorage` and refuses it as an unknown extension, so git's halves are
/// asked only of a git that can make a reftable repository: elsewhere they are skipped,
/// saying so, and required wherever that git can (`CAIRN_REQUIRE_REFTABLE`). Caught by:
/// the setting ignored, a `files` repository refused, the version not read or read after
/// the storage, or a missing file refused.
#[test]
fn the_ref_storage_setting_is_read_as_git_reads_it() {
    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "first");
    let probe = fixture.path().with_extension("probe");
    let reftable_reason = no_reftable_here(&probe);
    let _ = std::fs::remove_dir_all(&probe);
    if let Some(reason) = &reftable_reason {
        assert!(
            std::env::var_os("CAIRN_REQUIRE_REFTABLE").is_none(),
            "CAIRN_REQUIRE_REFTABLE is set, and the git here cannot make a reftable \
             repository: {reason}"
        );
        eprintln!("SKIPPED the_ref_storage_setting_is_read_as_git_reads_it (git's half): {reason}");
    }
    let git_knows_ref_storage = reftable_reason.is_none();
    // Written by hand: a git that refuses the result will not set a key in it.
    let set = |version: &str, storage: &str| {
        std::fs::write(
            fixture.path().join(".git/config"),
            format!(
                "[core]\n\trepositoryformatversion = {version}\n\tbare = false\n\
                 [extensions]\n\trefStorage = {storage}\n"
            ),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    };
    let git_opens = || oracle(fixture.path(), &["rev-parse", "HEAD"], None);

    set("1", "files");
    if git_knows_ref_storage {
        assert!(
            git_opens().status.success(),
            "git refuses refStorage = files"
        );
    }
    SharedRepository::discover(fixture.path()).unwrap();

    set("1", "reftable");
    match SharedRepository::discover(fixture.path()) {
        Err(Error::RefStorageUnsupported { storage, .. }) => assert_eq!(storage, "reftable"),
        other => panic!("refStorage = reftable was not refused: {other:?}"),
    }

    for storage in ["files", "reftable"] {
        set("0", storage);
        if git_knows_ref_storage {
            let refused = git_opens();
            assert!(
                !refused.status.success()
                    && String::from_utf8_lossy(&refused.stderr).contains("v1-only extension"),
                "git now opens a version-0 repository with refStorage = {storage}: {refused:?}"
            );
        }
        match SharedRepository::discover(fixture.path()) {
            Err(error @ Error::RefStorageNeedsFormatVersion1 { .. }) => {
                assert!(
                    error.to_string().contains("repositoryFormatVersion is 0"),
                    "{error}"
                );
            }
            other => panic!(
                "a version-0 repository with refStorage = {storage} was not refused for its \
                 version: {other:?}"
            ),
        }
    }

    std::fs::remove_file(fixture.path().join(".git/config")).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        git_opens().status.success(),
        "git refuses a repository with no config"
    );
    SharedRepository::discover(fixture.path())
        .unwrap_or_else(|e| panic!("a repository with no config file was refused: {e}"));
}

// ── C11: what it costs ──────────────────────────────────────────────────────

/// Reporter for C11's refs numbers: the refs snapshot alone and with every ahead/behind,
/// warm, median of seven, on the repository `CAIRN_BENCH_REPO` names (read only) and on a
/// generated fixture of 10,000 refs (6,000 packed, a quarter of the tags annotated, 500
/// loose). Run with `--release --ignored --nocapture`.
#[test]
#[ignore = "needs a large repository named by CAIRN_BENCH_REPO; run with --release"]
fn measures_the_refs_snapshot_and_ahead_behind() {
    use std::time::{Duration, Instant};

    fn median(mut runs: Vec<Duration>) -> Duration {
        runs.sort();
        runs[runs.len() / 2]
    }
    fn report(label: &str, dir: &Path) {
        let repo = Repository::discover(dir).unwrap();
        let read = repo.refs(&CancelSignal::new()).unwrap();
        let mut refs_only = Vec::new();
        let mut with_counts = Vec::new();
        for _ in 0..7 {
            let started = Instant::now();
            std::hint::black_box(repo.refs(&CancelSignal::new()).unwrap());
            refs_only.push(started.elapsed());
            let started = Instant::now();
            let read = repo.refs(&CancelSignal::new()).unwrap();
            let counts = repo
                .ahead_behind(&read.snapshot, &CancelSignal::new())
                .unwrap();
            with_counts.push(started.elapsed());
            std::hint::black_box(counts);
        }
        let counts = repo
            .ahead_behind(&read.snapshot, &CancelSignal::new())
            .unwrap();
        eprintln!(
            "{label}: {} refs, {} stashes, {} unreadable, {} counted branches; \
             refs median {:?}; refs + ahead/behind median {:?}; cost {:?}; \
             ahead/behind read {} commits",
            read.snapshot.refs.len(),
            read.snapshot.stashes.len(),
            read.snapshot.unreadable,
            counts.counts.len(),
            median(refs_only),
            median(with_counts),
            read.cost,
            counts.commits_read,
        );
    }

    let bench = PathBuf::from(std::env::var("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO"));
    report(&bench.display().to_string(), &bench);
    // A divergence the bench's own branch does not have: a branch at HEAD whose upstream is
    // `HEAD~n`, built as plain data over the real repository (git only reads here).
    let repo = Repository::discover(&bench).unwrap();
    for n in [100, 1_000, 10_000] {
        let tip = oid(&ask(&bench, &["rev-parse", "HEAD"]));
        let base = oid(&ask(&bench, &["rev-parse", &format!("HEAD~{n}")]));
        let diverged = RefsSnapshot {
            refs: vec![Ref {
                name: RefName::new("refs/heads/measured"),
                kind: RefKind::LocalBranch,
                target: RefTarget::Commit(tip),
                symbolic: None,
                upstream: Some(Upstream::Exists {
                    name: RefName::new("refs/remotes/origin/measured"),
                    commit: Some(base),
                }),
            }],
            head: HeadState::Detached(tip),
            stashes: Vec::new(),
            unreadable: 0,
        };
        let mut runs = Vec::new();
        let mut answer = None;
        for _ in 0..7 {
            let started = Instant::now();
            answer = Some(repo.ahead_behind(&diverged, &CancelSignal::new()).unwrap());
            runs.push(started.elapsed());
        }
        let answer = answer.unwrap();
        eprintln!(
            "  ahead/behind of HEAD against HEAD~{n}: {:?}, median {:?}, {} commits read; \
             git rev-list --left-right --count says {}",
            answer.counts[0].1,
            median(runs),
            answer.commits_read,
            ask(
                &bench,
                &[
                    "rev-list",
                    "--left-right",
                    "--count",
                    &format!("HEAD...HEAD~{n}")
                ]
            )
            .trim()
        );
    }

    let fixture = fixtures::unborn();
    let mut clock = fixtures::EPOCH;
    commit(&fixture, &mut clock, "first");
    let head = ask(fixture.path(), &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let mut stream = String::new();
    for n in 0..3_000 {
        stream.push_str(&format!("reset refs/heads/branch-{n:05}\nfrom {head}\n\n"));
        stream.push_str(&format!(
            "reset refs/remotes/origin/branch-{n:05}\nfrom {head}\n\n"
        ));
    }
    for n in 0..4_000 {
        if n % 4 == 0 {
            stream.push_str(&format!(
                "tag tag-{n:05}\nfrom {head}\ntagger T <t@example.com> 1500000000 +0000\ndata 1\nt\n"
            ));
        } else {
            stream.push_str(&format!("reset refs/tags/tag-{n:05}\nfrom {head}\n\n"));
        }
    }
    let imported = oracle(
        fixture.path(),
        &["fast-import", "--quiet"],
        Some(stream.as_bytes()),
    );
    assert!(imported.status.success(), "{imported:?}");
    // Ten branches tracking their remote-tracking twins, so ahead/behind has work to do.
    fixture.git(&["remote", "add", "origin", "https://example.invalid/o.git"]);
    for n in 0..10 {
        fixture.git(&["config", &format!("branch.branch-{n:05}.remote"), "origin"]);
        fixture.git(&[
            "config",
            &format!("branch.branch-{n:05}.merge"),
            &format!("refs/heads/branch-{n:05}"),
        ]);
    }
    fixture.git(&["pack-refs", "--all"]);
    for n in 0..500 {
        write(
            &fixture,
            &format!(".git/refs/heads/branch-{n:05}"),
            &format!("{head}\n"),
        );
    }
    let total = ask(fixture.path(), &["for-each-ref", "--format=x"])
        .lines()
        .count();
    report(&format!("generated fixture ({total} refs)"), fixture.path());
}
