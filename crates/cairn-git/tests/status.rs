//! The working tree's status (PRD R3; C4, and the measured half of C11). Every expectation
//! is read out of `git` at test time by a command that is not `git status` — comparing the
//! parsed answer with `git status --porcelain=v1` would only check that two of git's
//! printers agree:
//!
//! - staged changes: `git diff --cached --name-status`, under the rename detection the
//!   fixture's configuration gives `git status` (`--no-renames`, `-M` or `-C`, with
//!   `diff.renames` set off for the oracle alone: a `-C` given where `diff.renames=copies`
//!   already asks for copies is git's "find copies harder", which status never does);
//! - unstaged changes: `git diff --name-status`, the same way, for every path but an
//!   unmerged one or a submodule's (`git diff` reports a submodule otherwise than status
//!   does on 2.32.7 and 2.56.0, though not on 2.30.9: it leaves out one whose only change is
//!   untracked content);
//! - untracked files: `git ls-files --others --exclude-standard`, one per file, a nested
//!   repository as its directory;
//! - conflicts: `git ls-files -u`, each path's set of stages mapped to its kind
//!   (`ConflictKind::from_stages`);
//! - submodules: the submodule's own state — its `HEAD` against the gitlink the index
//!   records, its tracked changes (`git diff HEAD` inside it), its untracked files
//!   (`git ls-files --others` inside it) — masked by the `ignore` level the fixture set.
//!
//! Each read is also required to leave every index under the git directory byte-identical
//! (the superproject's and each submodule's), and no lock behind.
//!
//! Three sides, one configuration: the fixtures are built with the machine's configuration
//! shut out; Cairn's `git` runs with a home holding no configuration (its environment is
//! built as the application builds one, from a parent that answers `PATH` and that home);
//! and the oracles run with exactly those variables. Both of the last two read the system
//! file, which no fixture depends on: every key that decides an answer is set in the
//! fixture's own configuration.
//!
//! Run by `scripts/git-floor.sh` under git 2.30.9 and 2.32.7 as well as the host's git.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use cairn_git::ops::{Askpass, GitBinary, GitEnvironment, GitVersion};
use cairn_git::{CancelSignal, Error, Repository, SharedRepository};
use cairn_model::{
    ConflictKind, StagedChange, StatusEntry, UnreadableIndex, UnstagedChange, WorkingTreeStatus,
};

// ── the fixture ─────────────────────────────────────────────────────────────

/// A repository in a temporary directory, with a home of its own, removed when the test ends.
struct Repo {
    root: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "cairn-status-tests-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        for directory in ["home", "repo"] {
            std::fs::create_dir_all(root.join(directory)).unwrap_or_else(|e| panic!("{e}"));
        }
        let repo = Self { root };
        repo.git(&["init", "-q", "."]);
        // Every key that decides an answer here, pinned in the fixture's own configuration.
        repo.git(&["config", "diff.renames", "true"]);
        repo.git(&["config", "core.fileMode", "true"]);
        repo.git(&["config", "core.autocrlf", "false"]);
        repo
    }

    fn path(&self) -> PathBuf {
        self.root.join("repo")
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// `git` building the fixture in `dir`, with the machine's configuration shut out.
    fn build_in(&self, program: &Path, dir: &Path, args: &[&str]) {
        let output = Command::new(program)
            .current_dir(dir)
            .args(args)
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "A U Thor")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "C O Mitter")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .env("GIT_EDITOR", "true")
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
        assert!(
            output.status.success(),
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git(&self, args: &[&str]) {
        self.build_in(Path::new("git"), &self.path(), args);
    }

    /// `git` building the fixture, allowed to fail (a merge that conflicts).
    fn git_may_fail(&self, args: &[&str]) {
        let _ = Command::new("git")
            .current_dir(self.path())
            .args(args)
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "A U Thor")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "C O Mitter")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    }

    fn write(&self, path: impl AsRef<OsStr>, content: &str) {
        let path = self.path().join(Path::new(path.as_ref()));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
        }
        std::fs::write(&path, content).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }

    fn lines(&self, path: &str, word: &str, count: usize) {
        let body: String = (1..=count).map(|n| format!("{word} line {n}\n")).collect();
        self.write(path, &body);
    }

    fn append(&self, path: impl AsRef<OsStr>, content: &str) {
        use std::io::Write as _;
        let path = self.path().join(Path::new(path.as_ref()));
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(content.as_bytes()))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
    }

    fn remove(&self, path: &str) {
        std::fs::remove_file(self.path().join(path)).unwrap_or_else(|e| panic!("{e}"));
    }

    fn symlink(&self, path: &str, target: &str) {
        std::os::unix::fs::symlink(target, self.path().join(path))
            .unwrap_or_else(|e| panic!("{e}"));
    }

    fn chmod(&self, path: &str, mode: u32) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(
            self.path().join(path),
            std::fs::Permissions::from_mode(mode),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    }

    /// The `git` Cairn runs, as the application builds it: found on `PATH`, with this
    /// fixture's empty home for `HOME` and `XDG_CONFIG_HOME`.
    fn cairns_git(&self) -> GitBinary {
        let home = self.home().into_os_string();
        GitBinary::discover_with(GitEnvironment::new(
            |name| match name {
                "PATH" => std::env::var_os("PATH"),
                "HOME" | "XDG_CONFIG_HOME" => Some(home.clone()),
                _ => None,
            },
            &Askpass::new("/nonexistent/cairn-askpass", None),
        ))
        .unwrap_or_else(|e| panic!("{e}"))
    }

    /// git, the oracle, in `dir`: the variables Cairn's `git` has that decide an answer, and
    /// nothing else of this process's environment.
    fn oracle(&self, dir: &Path, args: &[&str]) -> Output {
        Command::new("git")
            .current_dir(dir)
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home())
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"))
    }

    fn ask(&self, dir: &Path, args: &[&str]) -> Vec<u8> {
        let output = self.oracle(dir, args);
        assert!(
            output.status.success(),
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn the_git_in_use() -> GitVersion {
    static VERSION: std::sync::OnceLock<GitVersion> = std::sync::OnceLock::new();
    *VERSION.get_or_init(|| {
        GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
            .unwrap_or_else(|e| panic!("{e}"))
            .version()
    })
}

fn since(minor: u32) -> GitVersion {
    GitVersion {
        major: 2,
        minor,
        patch: 0,
    }
}

// ── what is expected ────────────────────────────────────────────────────────

/// The rename detection the fixture's configuration gives `git status`: `status.renames`,
/// or `diff.renames` where it is unset.
#[derive(Debug, Clone, Copy)]
enum Renames {
    Off,
    On,
    Copies,
}

impl Renames {
    fn flag(self) -> &'static str {
        match self {
            Self::Off => "--no-renames",
            Self::On => "-M",
            Self::Copies => "-C",
        }
    }
}

/// What a submodule's `ignore` (or `diff.ignoreSubmodules`) has git not look at.
#[derive(Debug, Clone, Copy)]
enum Ignore {
    None,
    Untracked,
    Dirty,
    All,
}

struct Expect<'a> {
    renames: Renames,
    /// `false` under `status.showUntrackedFiles=no`.
    untracked: bool,
    /// Each initialised submodule, with the `ignore` level in force for it.
    submodules: &'a [(&'a str, Ignore)],
}

impl Default for Expect<'_> {
    fn default() -> Self {
        Self {
            renames: Renames::On,
            untracked: true,
            submodules: &[],
        }
    }
}

/// One fact, as bytes: a kind word, a code, the path, and a rename's source.
fn fact(kind: &str, code: &[u8], path: &[u8], from: Option<&[u8]>) -> Vec<u8> {
    let mut line = kind.as_bytes().to_vec();
    line.push(b' ');
    line.extend_from_slice(code);
    line.push(b' ');
    line.extend_from_slice(path);
    if let Some(from) = from {
        line.extend_from_slice(b" <- ");
        line.extend_from_slice(from);
    }
    line
}

/// `--name-status -z` records: a status, then one path, or two for a rename or a copy
/// (`R086 <from> <to>`). An unmerged path (`U`) is the conflict oracle's, and is left out.
fn name_status(kind: &str, bytes: &[u8], skip: &BTreeSet<Vec<u8>>) -> BTreeSet<Vec<u8>> {
    let mut records = bytes.split(|byte| *byte == 0).filter(|r| !r.is_empty());
    let mut facts = BTreeSet::new();
    while let Some(status) = records.next() {
        let letter = &status[..1];
        let first = records
            .next()
            .unwrap_or_else(|| panic!("no path after {status:?}"));
        if matches!(letter, b"R" | b"C") {
            let to = records.next().unwrap_or_else(|| panic!("no destination"));
            // `--name-status` prints the score as three digits; status as `%d`.
            let score: u32 = std::str::from_utf8(&status[1..])
                .unwrap_or_else(|e| panic!("{e}"))
                .parse()
                .unwrap_or_else(|e| panic!("{e}"));
            let code = format!("{}{score}", char::from(letter[0]));
            facts.insert(fact(kind, code.as_bytes(), to, Some(first)));
        } else if letter != b"U" && !skip.contains(first) {
            facts.insert(fact(kind, letter, first, None));
        }
    }
    facts
}

/// Every gitlink path the index holds.
fn gitlinks(repo: &Repo) -> BTreeSet<Vec<u8>> {
    let listed = repo.ask(&repo.path(), &["ls-files", "-s", "-z"]);
    listed
        .split(|byte| *byte == 0)
        .filter_map(|record| {
            let (meta, path) = record.split_at(record.iter().position(|b| *b == b'\t')?);
            meta.starts_with(b"160000").then(|| path[1..].to_vec())
        })
        .collect()
}

/// What the oracles say `git status` should list.
fn oracle_facts(repo: &Repo, expect: &Expect<'_>) -> BTreeSet<Vec<u8>> {
    let dir = repo.path();
    let rename = expect.renames.flag();

    // Conflicts: each unmerged path's stages, as `ls-files -u` lists them. An unmerged path
    // is this oracle's alone: `git diff` also prints it, as `U` and as a change.
    let mut stages: std::collections::BTreeMap<Vec<u8>, u8> = Default::default();
    for record in repo
        .ask(&dir, &["ls-files", "-u", "-z"])
        .split(|byte| *byte == 0)
        .filter(|r| !r.is_empty())
    {
        let tab = record
            .iter()
            .position(|b| *b == b'\t')
            .unwrap_or_else(|| panic!("{record:?}"));
        let stage = record[tab - 1] - b'0';
        *stages.entry(record[tab + 1..].to_vec()).or_default() |= 1 << (stage - 1);
    }
    let unmerged: BTreeSet<Vec<u8>> = stages.keys().cloned().collect();
    let mut facts = name_status(
        "staged",
        &repo.ask(
            &dir,
            &[
                "-c",
                "diff.renames=false",
                "diff",
                "--cached",
                "--name-status",
                "-z",
                rename,
            ],
        ),
        &unmerged,
    );
    let mut not_unstaged = gitlinks(repo);
    not_unstaged.extend(unmerged);
    facts.extend(name_status(
        "unstaged",
        &repo.ask(
            &dir,
            &[
                "-c",
                "diff.renames=false",
                "diff",
                "--name-status",
                "-z",
                rename,
            ],
        ),
        &not_unstaged,
    ));
    for (path, mask) in stages {
        let kind = ConflictKind::from_stages(mask).unwrap_or_else(|| panic!("mask {mask}"));
        facts.insert(fact("conflict", kind.code(), &path, None));
    }

    if expect.untracked {
        for path in repo
            .ask(&dir, &["ls-files", "--others", "--exclude-standard", "-z"])
            .split(|byte| *byte == 0)
            .filter(|r| !r.is_empty())
        {
            facts.insert(fact("untracked", b"?", path, None));
        }
    }

    for (path, ignore) in expect.submodules {
        let sub = dir.join(path);
        let recorded = repo.ask(&dir, &["rev-parse", &format!(":{path}")]);
        let checked_out = repo.ask(&sub, &["rev-parse", "HEAD"]);
        let mut new_commits = recorded != checked_out;
        let mut modified = !repo.ask(&sub, &["diff", "--name-only", "HEAD"]).is_empty();
        let mut untracked = !repo
            .ask(&sub, &["ls-files", "--others", "--exclude-standard"])
            .is_empty();
        match ignore {
            Ignore::None => {}
            Ignore::Untracked => untracked = false,
            Ignore::Dirty => (modified, untracked) = (false, false),
            Ignore::All => (new_commits, modified, untracked) = (false, false, false),
        }
        if !expect.untracked {
            untracked = false;
        }
        if new_commits || modified || untracked {
            let flags = [
                if new_commits { b'C' } else { b'.' },
                if modified { b'M' } else { b'.' },
                if untracked { b'U' } else { b'.' },
            ];
            facts.insert(fact("submodule", &flags, path.as_bytes(), None));
            facts.insert(fact("unstaged", b"M", path.as_bytes(), None));
        }
    }
    facts
}

/// The same facts, from Cairn's answer.
fn cairns_facts(entries: &[StatusEntry]) -> BTreeSet<Vec<u8>> {
    let mut facts = BTreeSet::new();
    for entry in entries {
        match entry {
            StatusEntry::Changed(changed) => {
                let path = changed.path.as_bytes();
                if let Some(staged) = &changed.staged {
                    let (code, from) = match staged {
                        StagedChange::Added => ("A".to_owned(), None),
                        StagedChange::Modified => ("M".to_owned(), None),
                        StagedChange::Deleted => ("D".to_owned(), None),
                        StagedChange::TypeChanged => ("T".to_owned(), None),
                        StagedChange::Renamed { from, similarity } => {
                            (format!("R{}", similarity.percent()), Some(from.as_bytes()))
                        }
                        StagedChange::Copied { from, similarity } => {
                            (format!("C{}", similarity.percent()), Some(from.as_bytes()))
                        }
                    };
                    facts.insert(fact("staged", code.as_bytes(), path, from));
                }
                if let Some(unstaged) = &changed.unstaged {
                    let (code, from) = match unstaged {
                        UnstagedChange::Modified => ("M".to_owned(), None),
                        UnstagedChange::Deleted => ("D".to_owned(), None),
                        UnstagedChange::TypeChanged => ("T".to_owned(), None),
                        UnstagedChange::IntentToAdd => ("A".to_owned(), None),
                        UnstagedChange::Renamed { from, similarity } => {
                            (format!("R{}", similarity.percent()), Some(from.as_bytes()))
                        }
                        UnstagedChange::Copied { from, similarity } => {
                            (format!("C{}", similarity.percent()), Some(from.as_bytes()))
                        }
                    };
                    facts.insert(fact("unstaged", code.as_bytes(), path, from));
                }
                if let Some(state) = changed.submodule {
                    let flags = [
                        if state.new_commits { b'C' } else { b'.' },
                        if state.modified_content { b'M' } else { b'.' },
                        if state.untracked_content { b'U' } else { b'.' },
                    ];
                    if flags != *b"..." {
                        facts.insert(fact("submodule", &flags, path, None));
                    }
                }
            }
            StatusEntry::Conflicted(conflict) => {
                facts.insert(fact(
                    "conflict",
                    conflict.kind.code(),
                    conflict.path.as_bytes(),
                    None,
                ));
                if let Some(state) = conflict.submodule {
                    let flags = [
                        if state.new_commits { b'C' } else { b'.' },
                        if state.modified_content { b'M' } else { b'.' },
                        if state.untracked_content { b'U' } else { b'.' },
                    ];
                    facts.insert(fact(
                        "conflicted submodule",
                        &flags,
                        conflict.path.as_bytes(),
                        None,
                    ));
                }
            }
            StatusEntry::Untracked(path) => {
                facts.insert(fact("untracked", b"?", path.as_bytes(), None));
            }
        }
    }
    facts
}

/// Every file named `index` or ending `.lock` under the git directory, with its bytes: the
/// superproject's index, each submodule's, and any lock.
fn indexes(git_dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![git_dir.to_owned()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
            let path = entry.path();
            let name = entry.file_name();
            if path.is_dir() && !path.is_symlink() {
                if name != "objects" {
                    pending.push(path);
                }
            } else if name == "index" || name.as_bytes().ends_with(b".lock") {
                let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{e}"));
                found.push((path, bytes));
            }
        }
    }
    found.sort();
    found
}

/// Cairn's status of `repo`, with every index under its git directory required
/// byte-identical afterwards and no lock left behind; and how many `git` reads it ran. The
/// superproject's index, and each of `submodules`' (`modules/<name>/index`), must be among
/// those compared, so a walk that found nothing cannot pass.
fn read_writing_nothing(repo: &Repo, submodules: &[&str]) -> (WorkingTreeStatus, usize) {
    let git_dir = repo.path().join(".git");
    let before = indexes(&git_dir);
    let required = std::iter::once(git_dir.join("index")).chain(
        submodules
            .iter()
            .map(|name| git_dir.join("modules").join(name).join("index")),
    );
    for index in required {
        assert!(
            before.iter().any(|(path, _)| *path == index),
            "{} is not among the indexes compared",
            index.display()
        );
    }
    assert!(
        before
            .iter()
            .all(|(path, _)| !path.as_os_str().as_bytes().ends_with(b".lock")),
        "a lock before the read: {before:?}"
    );
    let shared = SharedRepository::discover(repo.path()).unwrap_or_else(|e| panic!("{e}"));
    let answer = shared
        .to_worker()
        .status(&repo.cairns_git(), &CancelSignal::new())
        .unwrap_or_else(|e| panic!("{e}"));
    let after = indexes(&git_dir);
    assert!(
        before == after,
        "a status read wrote an index or left a lock: {:?} became {:?}",
        before
            .iter()
            .map(|(path, bytes)| (path, bytes.len()))
            .collect::<Vec<_>>(),
        after
            .iter()
            .map(|(path, bytes)| (path, bytes.len()))
            .collect::<Vec<_>>(),
    );
    (answer, shared.command_log().len())
}

/// Checks Cairn's status of `repo` against the oracles, after a read that wrote nothing.
fn assert_status_is_gits(repo: &Repo, expect: &Expect<'_>) -> Vec<StatusEntry> {
    let names: Vec<&str> = expect.submodules.iter().map(|(name, _)| *name).collect();
    let (answer, _) = read_writing_nothing(repo, &names);
    let WorkingTreeStatus::Listed(entries) = answer else {
        panic!("not a list: {answer:?}");
    };
    let actual = cairns_facts(&entries);
    let expected = oracle_facts(repo, expect);
    let shown = |set: &BTreeSet<Vec<u8>>| {
        set.iter()
            .map(|line| String::from_utf8_lossy(line).into_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        shown(&actual),
        shown(&expected),
        "git {} answered otherwise than its oracles",
        the_git_in_use()
    );
    entries
}

// ── C4 ──────────────────────────────────────────────────────────────────────

/// Every plain kind: an unstaged edit and deletion, a staged addition, edit and deletion,
/// and a file staged and then edited again.
#[test]
fn plain_staged_and_unstaged_changes_are_gits() {
    let repo = Repo::new("plain");
    for name in ["a", "b", "c", "d", "e", "f"] {
        repo.lines(&format!("{name}.txt"), name, 20);
    }
    repo.commit("base");
    repo.append("a.txt", "unstaged edit\n");
    repo.lines("new.txt", "new", 5);
    repo.git(&["add", "new.txt"]);
    repo.git(&["rm", "-q", "c.txt"]);
    repo.remove("d.txt");
    repo.append("e.txt", "staged\n");
    repo.git(&["add", "e.txt"]);
    repo.append("e.txt", "and unstaged\n");
    repo.append("f.txt", "staged only\n");
    repo.git(&["add", "f.txt"]);
    let entries = assert_status_is_gits(&repo, &Expect::default());
    assert_eq!(entries.len(), 6, "{entries:?}");
}

/// A clean tree lists nothing, in one read.
#[test]
fn a_clean_tree_lists_nothing() {
    let repo = Repo::new("clean");
    repo.lines("a.txt", "a", 3);
    repo.commit("base");
    let (answer, reads) = read_writing_nothing(&repo, &[]);
    assert_eq!(answer, WorkingTreeStatus::Listed(Vec::new()));
    assert_eq!(reads, 1);
}

/// The rename fixture: an exact staged rename, one with an edit, a source whose name has a
/// space, a copy of a file the change also modified and of one it did not.
fn renames_fixture(name: &str, config: &[(&str, &str)]) -> Repo {
    let repo = Repo::new(name);
    for (key, value) in config {
        repo.git(&["config", key, value]);
    }
    repo.lines("big.txt", "big", 40);
    repo.lines("other.txt", "other", 40);
    repo.lines("old name.txt", "spaced", 40);
    repo.lines("src.txt", "src", 40);
    repo.lines("quiet.txt", "quiet", 40);
    repo.commit("base");
    repo.git(&["mv", "big.txt", "moved.txt"]);
    repo.git(&["mv", "other.txt", "other2.txt"]);
    repo.append("other2.txt", "an edit\n");
    repo.git(&["mv", "old name.txt", "new name.txt"]);
    let source = std::fs::read_to_string(repo.path().join("src.txt")).unwrap_or_default();
    repo.write("copy.txt", &source);
    repo.append("src.txt", "changed\n");
    let quiet = std::fs::read_to_string(repo.path().join("quiet.txt")).unwrap_or_default();
    repo.write("quietcopy.txt", &quiet);
    repo.git(&["add", "-A"]);
    repo
}

/// Renames and copies are what the user's configuration makes `git status` report (R3.3),
/// under each `status.renames` value and, where it is unset, `diff.renames`. Nothing Cairn
/// passes overrides them. Caught by: `--no-renames`, `-M` or `--find-renames` added to the
/// read, or a source lost.
#[test]
fn renames_and_copies_follow_the_users_configuration() {
    type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], Renames);
    let cases: [Case<'_>; 6] = [
        ("unset", &[], Renames::On),
        ("off", &[("status.renames", "false")], Renames::Off),
        ("on", &[("status.renames", "true")], Renames::On),
        ("copies", &[("status.renames", "copies")], Renames::Copies),
        (
            "diff-copies",
            &[("diff.renames", "copies")],
            Renames::Copies,
        ),
        ("diff-off", &[("diff.renames", "false")], Renames::Off),
    ];
    for (name, config, renames) in cases {
        let repo = renames_fixture(name, config);
        let entries = assert_status_is_gits(
            &repo,
            &Expect {
                renames,
                ..Expect::default()
            },
        );
        let pairs = entries
            .iter()
            .filter(|entry| match entry {
                StatusEntry::Changed(changed) => matches!(
                    changed.staged,
                    Some(StagedChange::Renamed { .. } | StagedChange::Copied { .. })
                ),
                StatusEntry::Conflicted(_) | StatusEntry::Untracked(_) => false,
            })
            .count();
        // The fixture is decisive only if each setting changes what is paired.
        let expected = match renames {
            Renames::Off => 0,
            Renames::On => 3,
            // `-C` copies only from a file the change modified: `quietcopy.txt` is added.
            Renames::Copies => 4,
        };
        assert_eq!(pairs, expected, "{name}: {entries:?}");
    }
}

/// Forty staged renames, each with an edit: git pairs every one (gix's unsquared limit
/// pairs none), and so does the read.
#[test]
fn forty_inexact_renames_are_all_paired() {
    let repo = Repo::new("many-renames");
    for n in 1..=40 {
        repo.lines(&format!("f{n}.txt"), &format!("file{n}"), 30);
    }
    repo.commit("base");
    for n in 1..=40 {
        repo.git(&["mv", &format!("f{n}.txt"), &format!("g{n}.txt")]);
        repo.append(format!("g{n}.txt"), &format!("edit {n}\n"));
    }
    repo.git(&["add", "-A"]);
    let entries = assert_status_is_gits(&repo, &Expect::default());
    assert_eq!(entries.len(), 40, "{entries:?}");
}

/// Intent-to-add: an entry with content, an empty one, and a tracked file moved to an
/// intent-to-add entry, which git pairs as an unstaged rename — and, under copies, an
/// intent-to-add copy of a modified file. Caught by: `.A` read as a staged addition, or the
/// unstaged pair dropped or put on the staged side.
#[test]
fn intent_to_add_entries_and_their_pairs_are_gits() {
    for (name, config, renames) in [
        ("ita", &[][..], Renames::On),
        (
            "ita-copies",
            &[("status.renames", "copies")][..],
            Renames::Copies,
        ),
        ("ita-off", &[("status.renames", "false")][..], Renames::Off),
    ] {
        let repo = Repo::new(name);
        for (key, value) in config {
            repo.git(&["config", key, value]);
        }
        repo.lines("x.txt", "x", 40);
        repo.lines("a.txt", "a", 40);
        repo.commit("base");
        repo.write("new.txt", "new\n");
        repo.git(&["add", "-N", "new.txt"]);
        repo.write("empty.txt", "");
        repo.git(&["add", "-N", "empty.txt"]);
        std::fs::rename(repo.path().join("x.txt"), repo.path().join("y.txt"))
            .unwrap_or_else(|e| panic!("{e}"));
        repo.git(&["add", "-N", "y.txt"]);
        let a = std::fs::read_to_string(repo.path().join("a.txt")).unwrap_or_default();
        repo.write("acopy.txt", &a);
        repo.git(&["add", "-N", "acopy.txt"]);
        repo.append("a.txt", "more\n");
        let entries = assert_status_is_gits(
            &repo,
            &Expect {
                renames,
                ..Expect::default()
            },
        );
        let unstaged_pairs = entries
            .iter()
            .filter(|entry| match entry {
                StatusEntry::Changed(changed) => matches!(
                    changed.unstaged,
                    Some(UnstagedChange::Renamed { .. } | UnstagedChange::Copied { .. })
                ),
                StatusEntry::Conflicted(_) | StatusEntry::Untracked(_) => false,
            })
            .count();
        let expected = match renames {
            Renames::Off => 0,
            Renames::On => 1,
            Renames::Copies => 2,
        };
        assert_eq!(unstaged_pairs, expected, "{name}: {entries:?}");
    }
}

/// Type changes, staged and not (a file become a symlink), and mode changes, staged and not
/// (the executable bit, with `core.fileMode` true).
#[test]
fn type_and_mode_changes_are_gits() {
    let repo = Repo::new("types");
    for name in ["l.txt", "s.txt", "x.sh", "y.sh", "target"] {
        repo.write(name, &format!("{name}\n"));
    }
    repo.commit("base");
    repo.remove("l.txt");
    repo.symlink("l.txt", "target");
    repo.remove("s.txt");
    repo.symlink("s.txt", "target");
    repo.git(&["add", "s.txt"]);
    repo.chmod("x.sh", 0o755);
    repo.chmod("y.sh", 0o755);
    repo.git(&["add", "y.sh"]);
    let entries = assert_status_is_gits(&repo, &Expect::default());
    assert_eq!(entries.len(), 4, "{entries:?}");
}

/// All seven unmerged states from one real merge, each its own kind — and a conflicted path
/// whose "ours" stage is missing is not also listed as untracked (gix's divergence).
#[test]
fn every_conflict_kind_is_gits_own() {
    let repo = Repo::new("conflicts");
    for name in ["uu", "ud", "du", "rd", "dr"] {
        repo.lines(&format!("{name}.txt"), name, 20);
    }
    repo.lines("rr.txt", "rr", 30);
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "theirs"]);
    repo.append("uu.txt", "theirs\n");
    repo.lines("aa.txt", "aa-theirs", 20);
    repo.git(&["rm", "-q", "ud.txt"]);
    repo.append("du.txt", "theirs\n");
    repo.git(&["rm", "-q", "rd.txt"]);
    repo.git(&["mv", "dr.txt", "dr-theirs.txt"]);
    repo.git(&["mv", "rr.txt", "rr-theirs.txt"]);
    repo.commit("theirs");
    repo.git(&["checkout", "-q", "-"]);
    repo.append("uu.txt", "ours\n");
    repo.lines("aa.txt", "aa-ours", 20);
    repo.append("ud.txt", "ours\n");
    repo.git(&["rm", "-q", "du.txt"]);
    repo.git(&["mv", "rd.txt", "rd-ours.txt"]);
    repo.git(&["rm", "-q", "dr.txt"]);
    repo.git(&["mv", "rr.txt", "rr-ours.txt"]);
    repo.commit("ours");
    repo.git_may_fail(&["merge", "-q", "theirs"]);
    let entries = assert_status_is_gits(&repo, &Expect::default());
    let kinds: BTreeSet<&[u8; 2]> = entries
        .iter()
        .filter_map(|entry| match entry {
            StatusEntry::Conflicted(conflict) => Some(conflict.kind.code()),
            StatusEntry::Changed(_) | StatusEntry::Untracked(_) => None,
        })
        .collect();
    assert_eq!(
        kinds.len(),
        7,
        "the merge did not produce all seven unmerged states: {entries:?}"
    );
}

/// Untracked files one per file — in nested untracked directories, beside tracked files, a
/// symlink — a nested repository as its directory, and nothing ignored, under each value of
/// `status.showUntrackedFiles` that lists them; none under `no`, in one read.
#[test]
fn untracked_files_are_listed_per_file_unless_the_user_said_none() {
    for (name, setting) in [
        ("unset", None),
        ("normal", Some("normal")),
        ("all", Some("all")),
        ("no", Some("no")),
    ] {
        let repo = Repo::new(&format!("untracked-{name}"));
        if let Some(value) = setting {
            repo.git(&["config", "status.showUntrackedFiles", value]);
        }
        repo.write("tracked/t.txt", "t\n");
        repo.write(".gitignore", "*.log\nbuild/\n");
        repo.commit("base");
        for path in [
            "u1/a.txt",
            "u2/sub/deep/b.txt",
            "u2/c.txt",
            "onlyignored/x.log",
            "mix/a.txt",
            "mix/b.log",
            "tracked/new.txt",
            "tracked/newdir/f.txt",
            "build/out.o",
            "excluded.txt",
        ] {
            repo.write(path, "x\n");
        }
        std::fs::create_dir_all(repo.path().join("empty/nested")).unwrap_or_else(|e| panic!("{e}"));
        repo.write(".git/info/exclude", "excluded.txt\n");
        repo.symlink("link", "tracked/t.txt");
        repo.git(&["init", "-q", "nested"]);
        repo.write("nested/n.txt", "n\n");
        let listed = setting != Some("no");
        let (_, reads) = read_writing_nothing(&repo, &[]);
        assert_eq!(
            reads,
            if listed { 2 } else { 1 },
            "{name}: one read where git listed none, a second where it collapsed a directory"
        );
        let entries = assert_status_is_gits(
            &repo,
            &Expect {
                untracked: listed,
                ..Expect::default()
            },
        );
        let untracked = entries
            .iter()
            .filter(|entry| matches!(entry, StatusEntry::Untracked(_)))
            .count();
        assert_eq!(untracked, if listed { 8 } else { 0 }, "{name}: {entries:?}");
    }
}

/// Names `-z` keeps whole: spaces, a leading space, a newline, a tab, a backslash, a
/// quote, a leading dash, Unicode and bytes that are not UTF-8 — tracked and edited,
/// renamed from a name with a space, and untracked.
#[test]
fn awkward_names_arrive_whole() {
    let repo = Repo::new("names");
    let names: Vec<OsString> = [
        &b"with space.txt"[..],
        b" leading space.txt",
        b"new\nline.txt",
        b"tab\tname.txt",
        b"back\\slash.txt",
        b"quo\"te.txt",
        b"-dash.txt",
        "\u{fc}n\u{ef}c\u{f6}d\u{e9}.txt".as_bytes(),
        b"bad\xffname.txt",
    ]
    .iter()
    .map(|bytes| OsStr::from_bytes(bytes).to_owned())
    .collect();
    for name in &names {
        repo.write(name, "content\n");
    }
    repo.lines("old name.txt", "renamed", 30);
    repo.commit("base");
    for name in &names {
        repo.append(name, "edited\n");
    }
    repo.git(&["mv", "old name.txt", "new\nname.txt"]);
    repo.write(OsStr::from_bytes(b"untracked\nbad\xfe name"), "u\n");
    repo.write("dir with space/ leading/x", "u\n");
    let entries = assert_status_is_gits(&repo, &Expect::default());
    assert_eq!(entries.len(), names.len() + 3, "{entries:?}");
}

/// An unborn branch: files staged, one edited after, one untracked.
#[test]
fn an_unborn_branch_is_gits() {
    let repo = Repo::new("unborn");
    repo.write("a.txt", "a\n");
    repo.write("d/b.txt", "b\n");
    repo.git(&["add", "a.txt", "d"]);
    repo.write("u.txt", "u\n");
    repo.append("a.txt", "edited\n");
    let entries = assert_status_is_gits(&repo, &Expect::default());
    assert_eq!(entries.len(), 3, "{entries:?}");
}

/// A submodule fixture: one origin, and a submodule of it per state — clean, modified,
/// moved to a new commit, with untracked content, with modified and untracked content under
/// `ignore = dirty` and `ignore = untracked`, moved under `ignore = all`, and one
/// deinitialised.
fn submodules_fixture(name: &str) -> Repo {
    let repo = Repo::new(name);
    let origin = repo.root.join("origin");
    std::fs::create_dir_all(&origin).unwrap_or_else(|e| panic!("{e}"));
    repo.build_in(Path::new("git"), &origin, &["init", "-q", "."]);
    std::fs::write(origin.join("s.txt"), "s\n").unwrap_or_else(|e| panic!("{e}"));
    repo.build_in(Path::new("git"), &origin, &["add", "s.txt"]);
    repo.build_in(Path::new("git"), &origin, &["commit", "-q", "-m", "s"]);
    repo.write("top.txt", "top\n");
    repo.commit("base");
    let origin = origin.display().to_string();
    for sub in [
        "sub_clean",
        "sub_mod",
        "sub_newc",
        "sub_untr",
        "sub_ign_dirty",
        "sub_ign_untracked",
        "sub_ign_all",
        "sub_uninit",
    ] {
        repo.git(&[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            &origin,
            sub,
        ]);
    }
    for (sub, value) in [
        ("sub_ign_dirty", "dirty"),
        ("sub_ign_untracked", "untracked"),
        ("sub_ign_all", "all"),
    ] {
        repo.git(&[
            "config",
            "-f",
            ".gitmodules",
            &format!("submodule.{sub}.ignore"),
            value,
        ]);
    }
    repo.commit("submodules");
    repo.git(&["submodule", "deinit", "-q", "-f", "sub_uninit"]);
    let in_sub =
        |sub: &str, args: &[&str]| repo.build_in(Path::new("git"), &repo.path().join(sub), args);
    repo.append("sub_mod/s.txt", "m\n");
    repo.write("sub_newc/n.txt", "n\n");
    in_sub("sub_newc", &["add", "n.txt"]);
    in_sub("sub_newc", &["commit", "-q", "-m", "n"]);
    repo.write("sub_untr/u.txt", "u\n");
    repo.append("sub_ign_dirty/s.txt", "m\n");
    repo.write("sub_ign_dirty/u.txt", "u\n");
    repo.write("sub_ign_untracked/u.txt", "u\n");
    repo.append("sub_ign_untracked/s.txt", "m\n");
    repo.write("sub_ign_all/n.txt", "n\n");
    in_sub("sub_ign_all", &["add", "n.txt"]);
    in_sub("sub_ign_all", &["commit", "-q", "-m", "n"]);
    repo
}

/// Submodules as each `submodule.<name>.ignore` makes `git status` report them, and as
/// `diff.ignoreSubmodules` does for those with no `ignore` of their own; and none reporting
/// untracked content under `status.showUntrackedFiles=no`, which git passes down. Every
/// state of R3.2 reaches the model: new commits (`SC..`), modified content (`S.M.`) and
/// untracked content (`S..U`). Caught by: `--ignore-submodules` added to the read, or a
/// flag read from the wrong letter.
#[test]
fn submodules_are_reported_as_their_ignore_settings_say() {
    let own = [
        ("sub_clean", Ignore::None),
        ("sub_ign_dirty", Ignore::Dirty),
        ("sub_ign_untracked", Ignore::Untracked),
        ("sub_ign_all", Ignore::All),
    ];
    let repo = submodules_fixture("submodules");
    let entries = assert_status_is_gits(
        &repo,
        &Expect {
            submodules: &[
                own[0],
                own[1],
                own[2],
                own[3],
                ("sub_mod", Ignore::None),
                ("sub_newc", Ignore::None),
                ("sub_untr", Ignore::None),
            ],
            ..Expect::default()
        },
    );
    let states: BTreeSet<(bool, bool, bool)> = entries
        .iter()
        .filter_map(|entry| match entry {
            StatusEntry::Changed(changed) => changed
                .submodule
                .map(|s| (s.new_commits, s.modified_content, s.untracked_content)),
            StatusEntry::Conflicted(_) | StatusEntry::Untracked(_) => None,
        })
        .collect();
    for needed in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        assert!(
            states.contains(&needed),
            "{needed:?} never reached the model: {entries:?}"
        );
    }

    let repo = submodules_fixture("submodules-diff-ignore");
    repo.git(&["config", "diff.ignoreSubmodules", "dirty"]);
    assert_status_is_gits(
        &repo,
        &Expect {
            submodules: &[
                own[0],
                own[1],
                own[2],
                own[3],
                ("sub_mod", Ignore::Dirty),
                ("sub_newc", Ignore::Dirty),
                ("sub_untr", Ignore::Dirty),
            ],
            ..Expect::default()
        },
    );

    let repo = submodules_fixture("submodules-no-untracked");
    repo.git(&["config", "status.showUntrackedFiles", "no"]);
    assert_status_is_gits(
        &repo,
        &Expect {
            untracked: false,
            submodules: &[
                own[0],
                own[1],
                own[2],
                own[3],
                ("sub_mod", Ignore::None),
                ("sub_newc", Ignore::None),
                ("sub_untr", Ignore::None),
            ],
            ..Expect::default()
        },
    );
}

// ── R3.7 ────────────────────────────────────────────────────────────────────

/// The first `git` on `PATH` that writes a sparse index (2.32 or later): under
/// `scripts/git-floor.sh` the floor's own git comes first and cannot, so the fixture is
/// built by the machine's git further along, and read by the floor's.
fn a_git_that_writes_a_sparse_index() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let program = dir.join("git");
        let Ok(output) = Command::new(&program).arg("--version").output() else {
            continue;
        };
        let version = GitVersion::parse(&String::from_utf8_lossy(&output.stdout));
        if version.is_some_and(|version| version >= since(32)) {
            return program;
        }
    }
    panic!("no git on PATH writes a sparse index (2.32 or later), so R3.7 cannot be checked")
}

/// A sparse index on a git that cannot read one (before 2.32) answers R3.7's state, never an
/// empty or partial list, and writes nothing; on a git that can, the list is git's.
/// `sparse-checkout init --cone --sparse-index` is the one spelling that writes a sparse
/// index from 2.32 on (`a_sparse_index_is_unsupported_and_says_so`).
#[test]
fn a_sparse_index_git_cannot_read_is_said_so_and_one_it_can_is_listed() {
    let repo = Repo::new("sparse");
    for dir in ["a", "b", "c"] {
        repo.lines(&format!("{dir}/x.txt"), dir, 20);
    }
    repo.write("top.txt", "top\n");
    repo.commit("base");
    let builder = a_git_that_writes_a_sparse_index();
    repo.build_in(
        &builder,
        &repo.path(),
        &["sparse-checkout", "init", "--cone", "--sparse-index"],
    );
    repo.build_in(&builder, &repo.path(), &["sparse-checkout", "set", "a"]);
    repo.build_in(
        &builder,
        &repo.path(),
        &["config", "advice.sparseIndexExpanded", "false"],
    );
    let index = std::fs::read(repo.path().join(".git/index")).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        index.windows(4).any(|window| window == b"sdir"),
        "the index is not a sparse index"
    );
    repo.append("a/x.txt", "edited\n");
    repo.append("top.txt", "edited\n");
    repo.write("b/new.txt", "new\n");

    if the_git_in_use() < since(32) {
        let (answer, _) = read_writing_nothing(&repo, &[]);
        assert_eq!(
            answer,
            WorkingTreeStatus::IndexUnreadable(UnreadableIndex::Sparse),
            "git {}",
            the_git_in_use()
        );
    } else {
        let entries = assert_status_is_gits(&repo, &Expect::default());
        assert!(!entries.is_empty());
    }
}

/// A read that fails for any other reason — here an index whose signature is not `DIRC`,
/// which no git reads (git does not verify an index's trailing checksum on a read, so an
/// index cut short may be read as garbage instead) — is git's failure, never R3.7's state
/// and never an empty list.
#[test]
fn a_failed_read_that_is_not_a_sparse_index_is_an_error() {
    let repo = Repo::new("corrupt");
    repo.lines("a.txt", "a", 5);
    repo.commit("base");
    let handle = Repository::discover(repo.path()).unwrap_or_else(|e| panic!("{e}"));
    let index = repo.path().join(".git/index");
    let mut bytes = std::fs::read(&index).unwrap_or_else(|e| panic!("{e}"));
    bytes[..4].copy_from_slice(b"XXXX");
    std::fs::write(&index, &bytes).unwrap_or_else(|e| panic!("{e}"));
    let outcome = handle.status(&repo.cairns_git(), &CancelSignal::new());
    assert!(
        matches!(outcome, Err(Error::GitFailed { .. })),
        "{outcome:?}"
    );
}

/// A bare repository has no working tree: it says so, and runs nothing.
#[test]
fn a_bare_repository_has_no_working_tree_and_runs_nothing() {
    let repo = Repo::new("bare");
    let bare = repo.root.join("bare.git");
    repo.build_in(
        Path::new("git"),
        &repo.root,
        &["init", "-q", "--bare", "bare.git"],
    );
    let shared = cairn_git::SharedRepository::discover(&bare).unwrap_or_else(|e| panic!("{e}"));
    let answer = shared
        .to_worker()
        .status(&repo.cairns_git(), &CancelSignal::new())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(answer, WorkingTreeStatus::NoWorkingTree);
    assert!(shared.command_log().is_empty(), "a process was started");
}

// ── what a status read never does ───────────────────────────────────────────

/// The packs under a git directory's object store, by name: a lazy fetch adds one.
fn packs(git_dir: &Path) -> Vec<OsString> {
    let mut names: Vec<OsString> = std::fs::read_dir(git_dir.join("objects/pack"))
        .unwrap_or_else(|e| panic!("{}: {e}", git_dir.display()))
        .map(|entry| entry.unwrap_or_else(|e| panic!("{e}")).file_name())
        .collect();
    names.sort();
    names
}

/// In a blob-less partial clone whose sparse checkout left a file's blob with the promisor
/// alone, a staged inexact rename of that file makes git compare the missing blob. A read
/// never fetches it (`crate::reads`, `GIT_NO_LAZY_FETCH=1`, git 2.44 and later): git fails,
/// so the whole status read fails — `Error::GitFailed`, nothing listed — and no pack is
/// written, where the user's own `git status` would fetch the blob and answer. A git older
/// than 2.44 ignores the variable: there the read fetches, writing a pack, and answers the
/// rename — the floor's residual, pinned so that it is seen. Caught by: a read that may
/// lazy-fetch on a git that honours the variable (the read's environment lost), or an
/// answer read from a failed status.
#[test]
fn in_a_partial_clone_a_status_read_fails_rather_than_fetching() {
    let repo = Repo::new("partial-source");
    repo.git(&["config", "uploadpack.allowFilter", "true"]);
    repo.write("in/i.txt", "i\n");
    let big: String = (1..=200).map(|n| format!("{n}\n")).collect();
    repo.write("out/big", &big);
    repo.commit("base");
    repo.git(&["branch", "-M", "main"]);
    let url = format!("file://{}", repo.path().display());
    repo.build_in(
        Path::new("git"),
        &repo.root,
        &[
            "clone",
            "-q",
            "--filter=blob:none",
            "--no-checkout",
            &url,
            "clone",
        ],
    );
    let clone = repo.root.join("clone");
    let git = |args: &[&str]| repo.build_in(Path::new("git"), &clone, args);
    git(&["sparse-checkout", "init", "--cone"]);
    git(&["sparse-checkout", "set", "in"]);
    git(&["checkout", "-q", "main"]);
    let missing = |clone: &Path| {
        let listed = repo.ask(clone, &["rev-list", "--objects", "--missing=print", "HEAD"]);
        listed
            .split(|byte| *byte == b'\n')
            .any(|line| line.starts_with(b"?"))
    };
    assert!(
        missing(&clone),
        "the clone holds every blob, so nothing is lazy here"
    );
    // A staged rename of the excluded file, with an edit: git compares its blob.
    git(&["update-index", "--force-remove", "out/big"]);
    let renamed: String = (1..=199).map(|n| format!("{n}\n")).collect();
    std::fs::write(clone.join("in/big2"), renamed).unwrap_or_else(|e| panic!("{e}"));
    git(&["add", "in/big2"]);
    assert!(missing(&clone), "building the fixture fetched the blob");
    let before = packs(&clone.join(".git"));

    let shared = SharedRepository::discover(&clone).unwrap_or_else(|e| panic!("{e}"));
    let outcome = shared
        .to_worker()
        .status(&repo.cairns_git(), &CancelSignal::new());
    if the_git_in_use() >= since(44) {
        assert!(
            matches!(outcome, Err(Error::GitFailed { .. })),
            "git {}: a rename over a blob the clone lacks did not fail: {outcome:?}",
            the_git_in_use()
        );
        assert_eq!(packs(&clone.join(".git")), before, "the read fetched");
        assert!(missing(&clone), "the read fetched the blob");
    } else {
        // The floor's residual: git ignores GIT_NO_LAZY_FETCH, fetches, and answers.
        let Ok(WorkingTreeStatus::Listed(entries)) = outcome else {
            panic!("git {}: {outcome:?}", the_git_in_use());
        };
        assert!(
            entries.iter().any(|entry| matches!(
                entry,
                StatusEntry::Changed(changed)
                    if matches!(changed.staged, Some(StagedChange::Renamed { .. }))
            )),
            "{entries:?}"
        );
        assert_ne!(
            packs(&clone.join(".git")),
            before,
            "git {} did not fetch, so the residual stated is wrong",
            the_git_in_use()
        );
    }
}

/// Every file under a git directory with its bytes and its mtime, `objects` included.
fn snapshot(git_dir: &Path) -> Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> {
    let mut found = Vec::new();
    let mut pending = vec![git_dir.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.unwrap_or_else(|e| panic!("{e}")).path();
            let meta = std::fs::symlink_metadata(&path).unwrap_or_else(|e| panic!("{e}"));
            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() {
                let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{e}"));
                let modified = meta.modified().unwrap_or_else(|e| panic!("{e}"));
                found.push((path, bytes, modified));
            }
        }
    }
    found.sort();
    found
}

/// A script beside the repository (outside its working tree, so never listed), executable.
fn script(repo: &Repo, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = repo.root.join(name);
    std::fs::write(&path, body).unwrap_or_else(|e| panic!("{e}"));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|e| panic!("{e}"));
    path
}

/// R3.8 against what a status read must not write or run. The index carries an untracked
/// cache, an fsmonitor token and a split index's link, all written by a locked `git status`
/// before the read, and the tree has changed since, so a locked status would rewrite each;
/// a caching textconv, an external diff, a driver's `command`, a smudge filter and a
/// `post-index-change` hook are configured to leave a mark. The read leaves the git
/// directory byte-identical, every file's mtime unchanged but the shared index's (git
/// freshens the one it uses, the residual R3.8 states), and runs none of those: only the
/// `core.fsmonitor` hook, which answers a token, and the clean filter of the stat-dirty file
/// it rehashes, which leave theirs. Only `git status` ran. Caught by: status built so that it
/// writes (the untracked cache, the token, the refreshed stat), or a flag that runs a
/// driver.
#[test]
fn a_status_read_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor() {
    let repo = Repo::new("traps");
    let mark = |name: &str| repo.root.join(format!("{name}-ran"));
    let trap = script(
        &repo,
        "trap.sh",
        &format!("#!/bin/sh\n: > '{}'\ncat \"$1\"\n", mark("trap").display()),
    );
    let smudge = script(
        &repo,
        "smudge.sh",
        &format!("#!/bin/sh\n: > '{}'\ncat\n", mark("smudge").display()),
    );
    let clean = script(
        &repo,
        "clean.sh",
        &format!("#!/bin/sh\n: > '{}'\ncat\n", mark("clean").display()),
    );
    // Protocol 2: a token, a NUL, then the paths changed since — `/`, everything.
    let monitor = script(
        &repo,
        "monitor.sh",
        &format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nprintf 'cairn-token\\0/\\0'\n",
            mark("fsmonitor").display()
        ),
    );
    let hook_mark = mark("hook");
    repo.write("a.txt", "one\ntwo\n");
    repo.write("b.txt", "one\ntwo\n");
    repo.write("d/c.txt", "three\n");
    repo.commit("three files");
    for (key, value) in [
        ("filter.mark.clean", clean.display().to_string()),
        ("filter.mark.smudge", smudge.display().to_string()),
        ("diff.trap.textconv", trap.display().to_string()),
        ("diff.trap.cachetextconv", "true".to_owned()),
        ("diff.trap.command", trap.display().to_string()),
        ("diff.external", trap.display().to_string()),
        ("core.fsmonitor", monitor.display().to_string()),
        ("core.fsmonitorHookVersion", "2".to_owned()),
        ("core.untrackedCache", "true".to_owned()),
        ("core.splitIndex", "true".to_owned()),
    ] {
        repo.git(&["config", key, &value]);
    }
    let hooks = repo.path().join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        hooks.join("post-index-change"),
        format!("#!/bin/sh\n: > '{}'\n", hook_mark.display()),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    repo.chmod(".git/hooks/post-index-change", 0o755);
    repo.write(".gitattributes", "*.txt diff=trap filter=mark\n");
    repo.git(&["add", ".gitattributes"]);
    // A locked status, as the user's own, writes the untracked cache and the token; the
    // split comes after it, since git 2.30.9 and 2.32.7 merge a split index whenever a
    // locked status under `core.fsmonitor` rewrites it (2.56.0 keeps it; reproduced).
    repo.git(&["status", "--porcelain"]);
    repo.git(&["update-index", "--split-index", "--untracked-cache"]);
    // Since then: a file touched with its content the same (rehashed through the clean
    // filter), an edit, and an untracked directory.
    std::fs::File::open(repo.path().join("a.txt"))
        .and_then(|file| {
            file.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(60))
        })
        .unwrap_or_else(|e| panic!("{e}"));
    repo.write("b.txt", "one\ntwo, edited\n");
    repo.write("new/u.txt", "untracked\n");
    for name in ["trap", "smudge", "clean", "fsmonitor", "hook"] {
        let _ = std::fs::remove_file(mark(name));
    }

    let git_dir = repo.path().join(".git");
    let index = std::fs::read(git_dir.join("index")).unwrap_or_else(|e| panic!("{e}"));
    for extension in [&b"UNTR"[..], b"FSMN", b"link"] {
        assert!(
            index.windows(4).any(|window| window == extension),
            "the index carries no {:?} extension, so the read decides nothing about it",
            String::from_utf8_lossy(extension)
        );
    }
    let before = snapshot(&git_dir);
    let shared = SharedRepository::discover(repo.path()).unwrap_or_else(|e| panic!("{e}"));
    let answer = shared
        .to_worker()
        .status(&repo.cairns_git(), &CancelSignal::new())
        .unwrap_or_else(|e| panic!("{e}"));
    let after = snapshot(&git_dir);
    assert!(matches!(answer, WorkingTreeStatus::Listed(_)), "{answer:?}");

    let paths = |taken: &[(PathBuf, Vec<u8>, std::time::SystemTime)]| {
        taken
            .iter()
            .map(|(path, _, _)| path.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        paths(&after),
        paths(&before),
        "a file under .git appeared or went"
    );
    for ((path, bytes, modified), (_, bytes_after, modified_after)) in before.iter().zip(&after) {
        assert!(bytes == bytes_after, "{} was rewritten", path.display());
        let shared_index = path
            .file_name()
            .is_some_and(|name| name.as_bytes().starts_with(b"sharedindex."));
        assert!(
            shared_index || modified == modified_after,
            "{} was touched",
            path.display()
        );
    }
    assert!(
        !git_dir.join("refs/notes").exists(),
        "a textconv cache was written"
    );
    assert!(
        !mark("trap").exists(),
        "a textconv, external diff or driver command ran"
    );
    assert!(!mark("smudge").exists(), "a smudge filter ran");
    assert!(!hook_mark.exists(), "a hook ran");
    assert!(
        mark("clean").exists(),
        "the clean filter never ran on the touched file"
    );
    assert!(
        mark("fsmonitor").exists(),
        "the repository's core.fsmonitor did not run"
    );
    for record in shared.command_log() {
        assert!(
            record.arguments.iter().any(|argument| argument == "status"),
            "a read ran {:?}",
            record.arguments
        );
    }

    // Decisive only because each could run, and git with locks allowed would have written.
    for (program, name) in [(&trap, "trap"), (&smudge, "smudge")] {
        let ran = Command::new(program)
            .arg(repo.path().join("a.txt"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status();
        assert!(ran.is_ok_and(|status| status.success()) && mark(name).exists());
    }
    repo.git(&["status", "--porcelain"]);
    assert!(
        std::fs::read(git_dir.join("index")).unwrap_or_else(|e| panic!("{e}")) != index,
        "a locked git status left the index alone too"
    );
    assert!(
        hook_mark.exists(),
        "the post-index-change hook cannot run at all"
    );
}

// ── C11 ─────────────────────────────────────────────────────────────────────

/// C11's status numbers: release build, warm, median of seven, through
/// `Repository::status` with the `git` the application would find. On the repository
/// `CAIRN_BENCH_REPO` names, clean and read only (its index and lock files are checked
/// unchanged); then on a scratch clone of it (`git clone --local --no-hardlinks` into the
/// temporary directory, never inside the bench repository): 1,000 tracked files modified and
/// 10,000 untracked files in 1,000 tracked directories (one read); the same with the
/// untracked files in 1,000 new directories (git collapses them, so a second read with
/// `--untracked-files=all`, new data the spike did not measure); and every tracked file's
/// stat changed, content the same (recorded, not barred), with a read superseded 100 ms in
/// to show the cancel ends it. Before each settled scenario the clone sleeps 1.5 s and a
/// locked `git status` refreshes its index, so no entry is racily clean, as the spike did.
/// Run with `cargo test --release -p cairn-git --test status -- --ignored --nocapture`.
#[test]
#[ignore = "needs a large repository named by CAIRN_BENCH_REPO; run with --release"]
fn measures_the_status_read() {
    use std::time::{Duration, Instant};

    let bench = PathBuf::from(
        std::env::var_os("CAIRN_BENCH_REPO").unwrap_or_else(|| panic!("set CAIRN_BENCH_REPO")),
    );
    let git = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
        .unwrap_or_else(|e| panic!("{e}"));
    let plain = |dir: &Path, args: &[&str]| {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
        assert!(
            output.status.success() || args.first() == Some(&"update-index"),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let measure = |label: &str, dir: &Path| {
        let shared = SharedRepository::discover(dir).unwrap_or_else(|e| panic!("{e}"));
        let repo = shared.to_worker();
        let first = repo
            .status(&git, &CancelSignal::new())
            .unwrap_or_else(|e| panic!("{e}"));
        let reads = shared.command_log().len();
        let mut runs = Vec::new();
        for _ in 0..7 {
            let started = Instant::now();
            let answer = repo
                .status(&git, &CancelSignal::new())
                .unwrap_or_else(|e| panic!("{e}"));
            runs.push(started.elapsed());
            assert_eq!(answer, first, "{label}: the answer moved between runs");
        }
        runs.sort();
        let entries = match &first {
            WorkingTreeStatus::Listed(entries) => entries.len(),
            other => panic!("{label}: {other:?}"),
        };
        eprintln!(
            "{label}: {entries} entries, {reads} git read(s) per status; median {:?} [{:?} - {:?}]",
            runs[3], runs[0], runs[6]
        );
    };

    // The bench itself, read only.
    let before = indexes(&bench.join(".git"));
    measure("bench, clean", &bench);
    assert!(
        indexes(&bench.join(".git")) == before,
        "the bench repository's index or locks changed"
    );

    // A scratch clone.
    let scratch = std::env::temp_dir().join(format!("cairn-status-c11-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let clone = scratch.join("rust");
    std::fs::create_dir_all(&scratch).unwrap_or_else(|e| panic!("{e}"));
    plain(
        &scratch,
        &[
            "clone",
            "--quiet",
            "--local",
            "--no-hardlinks",
            &bench.display().to_string(),
            "rust",
        ],
    );
    let settle = || {
        std::thread::sleep(Duration::from_millis(1_500));
        plain(&clone, &["update-index", "-q", "--refresh"]);
        plain(&clone, &["status", "--porcelain"]);
    };
    let tracked: Vec<PathBuf> = plain(&clone, &["ls-files", "-s", "-z"])
        .split(|byte| *byte == 0)
        .filter(|record| record.starts_with(b"100644"))
        .filter_map(|record| {
            let tab = record.iter().position(|byte| *byte == b'\t')?;
            Some(PathBuf::from(OsStr::from_bytes(&record[tab + 1..])))
        })
        .collect();
    settle();
    measure("clone, clean", &clone);

    // 1,000 modified, 10,000 untracked in 1,000 tracked directories.
    let step = tracked.len() / 1_000;
    let modified: Vec<&PathBuf> = tracked.iter().step_by(step).take(1_000).collect();
    for path in &modified {
        use std::io::Write as _;
        std::fs::OpenOptions::new()
            .append(true)
            .open(clone.join(path))
            .and_then(|mut file| file.write_all(b"\ncairn status measurement\n"))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    let directories: BTreeSet<PathBuf> = tracked
        .iter()
        .filter_map(|path| path.parent().map(Path::to_owned))
        .filter(|dir| !dir.as_os_str().is_empty())
        .collect();
    let chosen: Vec<&PathBuf> = directories
        .iter()
        .step_by((directories.len() / 1_000).max(1))
        .take(1_000)
        .collect();
    for dir in &chosen {
        for n in 0..10 {
            std::fs::write(
                clone.join(dir).join(format!("cairn-untracked-{n}.txt")),
                "u\n",
            )
            .unwrap_or_else(|e| panic!("{e}"));
        }
    }
    settle();
    measure(
        "clone, 1,000 modified + 10,000 untracked in tracked directories",
        &clone,
    );

    // The same untracked files moved into 1,000 new directories.
    for (index, dir) in chosen.iter().enumerate() {
        let fresh = clone.join(format!("cairn-new-{index:04}"));
        std::fs::create_dir_all(&fresh).unwrap_or_else(|e| panic!("{e}"));
        for n in 0..10 {
            let name = format!("cairn-untracked-{n}.txt");
            std::fs::rename(clone.join(dir).join(&name), fresh.join(&name))
                .unwrap_or_else(|e| panic!("{e}"));
        }
    }
    settle();
    measure(
        "clone, 1,000 modified + 10,000 untracked in new directories",
        &clone,
    );

    // Every tracked file's stat changed, its content the same; nothing settles it.
    plain(&clone, &["checkout", "--quiet", "--", "."]);
    for (index, _) in chosen.iter().enumerate() {
        let _ = std::fs::remove_dir_all(clone.join(format!("cairn-new-{index:04}")));
    }
    settle();
    let touched = std::time::SystemTime::now();
    for path in &tracked {
        std::fs::File::open(clone.join(path))
            .and_then(|file| file.set_modified(touched))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    measure("clone, every tracked file's stat changed", &clone);

    // Superseded 100 ms into that slow read.
    let shared = SharedRepository::discover(&clone).unwrap_or_else(|e| panic!("{e}"));
    let repo = shared.to_worker();
    let cancel = CancelSignal::new();
    let canceller = {
        let cancel = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel.cancel();
        })
    };
    let started = Instant::now();
    let outcome = repo.status(&git, &cancel);
    let elapsed = started.elapsed();
    canceller
        .join()
        .unwrap_or_else(|_| panic!("the cancelling thread panicked"));
    assert!(
        matches!(outcome, Err(Error::StatusCancelled)),
        "{outcome:?}"
    );
    assert_eq!(
        shared.end_invocations(Duration::from_secs(1)),
        0,
        "left running"
    );
    let log = shared.command_log();
    eprintln!(
        "clone, every stat changed, superseded at 100 ms: answered cancelled after {elapsed:?}; \
         the log says cancelled: {}",
        log.last().is_some_and(|record| record.cancelled)
    );

    let _ = std::fs::remove_dir_all(&scratch);
}
