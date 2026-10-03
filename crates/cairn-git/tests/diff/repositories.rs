//! Repositories built by running real `git`, with real content in them.
//!
//! Only the diff tests declare this module, so everything here is theirs; the shared
//! `tests/fixtures` builds histories of empty commits, which decides nothing about a diff.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use cairn_model::Oid;

/// A repository in a temporary directory, removed when the test ends.
#[derive(Debug)]
pub struct Repo {
    path: PathBuf,
    borrowed: bool,
}

/// A directory with nothing in it, for `HOME` and `XDG_CONFIG_HOME`: no configuration
/// file of the machine's user can be read through it, on any git. One for every run, kept
/// rather than removed (a concurrent test binary may be using it), and checked empty.
pub fn empty_home() -> &'static Path {
    static HOME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    HOME.get_or_init(|| {
        let path = std::env::temp_dir().join("cairn-git-tests-empty-home");
        std::fs::create_dir_all(&path).unwrap_or_else(|e| panic!("making {}: {e}", path.display()));
        let mut entries =
            std::fs::read_dir(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        assert!(
            entries.next().is_none(),
            "{} is meant to be empty, and something wrote into it",
            path.display()
        );
        path
    })
}

/// Committer dates rise by a minute per commit, so `git log` orders them the way they were
/// made whatever the machine's clock says.
const EPOCH: i64 = 1_600_000_000;

impl Repo {
    pub fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("cairn-diff-{}-{name}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap_or_else(|e| panic!("making {}: {e}", path.display()));
        let repo = Self {
            path,
            borrowed: false,
        };
        repo.git(&["init", "--quiet", "--initial-branch=main", "."]);
        repo
    }

    /// A repository that already exists and must outlive this handle unharmed: nothing is
    /// created and nothing is removed when it is dropped.
    pub fn borrowed(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            borrowed: true,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Runs `git` in the repository and returns stdout. Panics on failure.
    pub fn git(&self, args: &[&str]) -> String {
        match self.try_git(args, &[], None) {
            Ok(out) => out,
            Err(message) => panic!("git {args:?}: {message}"),
        }
    }

    /// `Err` is git's own stderr, for the tests that expect a refusal.
    pub fn try_git(
        &self,
        args: &[&str],
        env: &[(&str, &str)],
        stdin: Option<&[u8]>,
    ) -> Result<String, String> {
        let (status, stdout, stderr) = self.run(args, env, stdin);
        if status.success() {
            Ok(stdout)
        } else {
            Err(format!("{status} {stderr}"))
        }
    }

    /// git's exit status, stdout and stderr whatever it did, for a test that reads git's
    /// own warnings as its oracle (in the C locale, so they are git's English).
    pub fn run(
        &self,
        args: &[&str],
        env: &[(&str, &str)],
        stdin: Option<&[u8]>,
    ) -> (std::process::ExitStatus, String, String) {
        let mut command = Command::new("git");
        command
            .current_dir(&self.path)
            .args(args)
            // Isolate from the machine's git config, e.g. a global `commit.gpgsign`. git
            // before 2.32 knows neither `GIT_CONFIG_*` file variable and reads
            // `~/.gitconfig` and the XDG file anyway, so the home it would look in is an
            // empty directory and the system file is switched off by the older spelling.
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", empty_home())
            .env("XDG_CONFIG_HOME", empty_home())
            .env("LC_ALL", "C")
            .env("GIT_AUTHOR_NAME", "A U Thor")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "C O Mitter")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command
            .spawn()
            .unwrap_or_else(|e| panic!("could not start git {args:?}: {e}"));
        if let Some(bytes) = stdin {
            use std::io::Write;
            let Some(mut pipe) = child.stdin.take() else {
                panic!("git {args:?} was given no standard input to write to");
            };
            pipe.write_all(bytes)
                .unwrap_or_else(|e| panic!("writing to git {args:?}: {e}"));
        }
        let output = child
            .wait_with_output()
            .unwrap_or_else(|e| panic!("waiting for git {args:?}: {e}"));
        (
            output.status,
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    pub fn write(&self, rela: &str, content: &[u8]) {
        let path = self.path.join(rela);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("making a directory: {e}"));
        }
        std::fs::write(&path, content).unwrap_or_else(|e| panic!("writing {rela}: {e}"));
    }

    pub fn remove(&self, rela: &str) {
        std::fs::remove_file(self.path.join(rela))
            .unwrap_or_else(|e| panic!("removing {rela}: {e}"));
    }

    pub fn symlink(&self, rela: &str, target: &str) {
        let path = self.path.join(rela);
        let _ = std::fs::remove_file(&path);
        std::os::unix::fs::symlink(target, &path).unwrap_or_else(|e| panic!("linking {rela}: {e}"));
    }

    pub fn chmod(&self, rela: &str, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.path.join(rela);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            .unwrap_or_else(|e| panic!("chmod {rela}: {e}"));
    }

    /// Stages everything and commits, returning the new commit.
    pub fn commit(&self, message: &str) -> Oid {
        static CLOCK: AtomicUsize = AtomicUsize::new(0);
        let tick = CLOCK.fetch_add(1, Ordering::Relaxed) as i64;
        let stamp = format!("{} +0000", EPOCH + tick * 60);
        self.git(&["add", "--all", "."]);
        self.try_git(
            &["commit", "--quiet", "--allow-empty", "-m", message],
            &[("GIT_AUTHOR_DATE", &stamp), ("GIT_COMMITTER_DATE", &stamp)],
            None,
        )
        .unwrap_or_else(|e| panic!("committing {message:?}: {e}"));
        self.rev("HEAD")
    }

    pub fn rev(&self, spec: &str) -> Oid {
        let hex = self.git(&["rev-parse", spec]);
        Oid::parse(hex.trim()).unwrap_or_else(|e| panic!("{spec} is not an object id: {e}"))
    }

    pub fn config(&self, key: &str, value: &str) {
        self.git(&["config", key, value]);
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        if !self.borrowed {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// The edge cases C2 names, each in a commit of its own so a test can reach it by subject.
///
/// Commit order, oldest first: `seed`, `edits`, `endings`, `newlines`, `add and delete`,
/// `rename with edits`, `far apart`, `crlf edit`, `delete a line`, `mode and content`,
/// `mode change`, `type change`. The last two stay last: tests reach them as `HEAD~1` and
/// `HEAD`.
pub fn crafted() -> Repo {
    let repo = Repo::new("crafted");

    // A root commit, so the empty-tree comparison of L5 has a subject.
    repo.write(
        "plain.txt",
        b"alpha\nbravo\ncharlie\ndelta\necho\nfoxtrot\ngolf\nhotel\n",
    );
    repo.write("keep.txt", b"kept\n");
    repo.write("one-line.txt", b"only\n");
    repo.write("empty.txt", b"");
    let long: String = (1..=20).map(|n| format!("line {n}\n")).collect();
    repo.write("long.txt", long.as_bytes());
    repo.commit("seed");

    // Three changes, at lines 1, 7 and 8. Six unchanged lines between the first two is
    // exactly twice the context, so all three merge into ONE hunk at three lines of
    // context (`far apart` below is the commit with two).
    repo.write(
        "plain.txt",
        b"ALPHA\nbravo\ncharlie\ndelta\necho\nfoxtrot\nGOLF\nHOTEL\n",
    );
    repo.commit("edits");

    // A file that never ended in a newline, one that ends in CRLF, and a file whose last
    // line loses its newline in a later commit.
    repo.write("no-eol.txt", b"first\nsecond\nlast with no newline");
    repo.write("crlf.txt", b"one\r\ntwo\r\nthree\r\n");
    repo.write("loses-eol.txt", b"stays\ngoing\n");
    repo.commit("endings");

    // The newline on the last line moves in both directions at once.
    repo.write("no-eol.txt", b"first\nsecond\nlast with no newline\n");
    repo.write("loses-eol.txt", b"stays\ngoing");
    repo.commit("newlines");

    repo.write("added.txt", b"brand\nnew\n");
    repo.write("added-empty.txt", b"");
    repo.remove("keep.txt");
    repo.commit("add and delete");

    // A rename that keeps most of its content, so git pairs it by similarity.
    repo.write(
        "renamed.txt",
        b"ALPHA\nbravo\ncharlie\ndelta\necho\nfoxtrot\nGOLF\nindia\n",
    );
    repo.remove("plain.txt");
    repo.commit("rename with edits");

    // Changes at lines 2 and 18, with fifteen unchanged lines between them: more than
    // twice the context, so two hunks at three lines of context.
    let far: String = (1..=20)
        .map(|n| match n {
            2 => "LINE 2\n".to_owned(),
            18 => "LINE 18\n".to_owned(),
            n => format!("line {n}\n"),
        })
        .collect();
    repo.write("long.txt", far.as_bytes());
    repo.commit("far apart");

    // An edit inside a CRLF file, so its unchanged CRLF lines are the patch's context.
    repo.write("crlf.txt", b"one\r\nTWO\r\nthree\r\n");
    repo.commit("crlf edit");

    // A pure deletion inside a file: one middle line gone, nothing added, every line
    // around it distinct, so there is exactly one way to say it.
    let deleted: String = (1..=20)
        .filter(|n| *n != 10)
        .map(|n| match n {
            2 => "LINE 2\n".to_owned(),
            18 => "LINE 18\n".to_owned(),
            n => format!("line {n}\n"),
        })
        .collect();
    repo.write("long.txt", deleted.as_bytes());
    repo.commit("delete a line");

    // A mode change and an edit in one file: `old mode`/`new mode` beside a hunk.
    repo.write("added.txt", b"brand\nnew\nand more\n");
    repo.chmod("added.txt", 0o755);
    repo.commit("mode and content");

    repo.chmod("one-line.txt", 0o755);
    repo.commit("mode change");

    repo.remove("crlf.txt");
    repo.symlink("crlf.txt", "renamed.txt");
    repo.commit("type change");

    repo
}

/// Attributes and diff drivers: what must be treated as binary, and the two programs that
/// must never run.
pub fn attributes() -> Repo {
    let repo = Repo::new("attributes");
    let trap = repo.path().join("trap.sh");
    let sentinel = repo.path().join("trap-ran");
    std::fs::write(
        &trap,
        format!("#!/bin/sh\n: > '{}'\ncat \"$1\"\n", sentinel.display()),
    )
    .unwrap_or_else(|e| panic!("writing the trap: {e}"));
    repo.chmod("trap.sh", 0o755);

    repo.write(
        ".gitattributes",
        b"no-diff.txt -diff\nflagged.dat diff=flagged\ntrap.txt diff=trap\nmacro.dat binary\n",
    );
    repo.config("diff.flagged.binary", "true");
    // Both of the programs gix knows how to run. Neither may be started: `Mode::ToGit`
    // never applies a textconv, and the resource cache is built with the internal diff
    // forced on.
    repo.config("diff.trap.textconv", &format!("sh {}", trap.display()));
    repo.config("diff.trap.command", &format!("sh {}", trap.display()));

    repo.write("no-diff.txt", b"this is text\nthat git calls binary\n");
    repo.write("flagged.dat", b"also text\nunder a binary driver\n");
    repo.write("trap.txt", b"watched\nby a program\n");
    repo.write("nul.dat", b"before\x00after\n");
    repo.write("plain.txt", b"ordinary\n");
    // `binary` is git's built-in macro for `-diff -merge -text`.
    repo.write("macro.dat", b"text under\nthe binary macro\n");
    // git looks for a NUL in the first 8,000 bytes and no further: one at byte 7,999 is
    // the last it sees, one at byte 8,000 the first it does not.
    repo.write("nul-at-7999.dat", &nul_at(7999, ""));
    repo.write("nul-at-8000.txt", &nul_at(8000, ""));
    repo.commit("seed");

    repo.write(
        "no-diff.txt",
        b"this is text\nthat git calls binary, changed\n",
    );
    repo.write(
        "flagged.dat",
        b"also text\nunder a binary driver, changed\n",
    );
    repo.write("trap.txt", b"watched\nby a program, changed\n");
    repo.write("nul.dat", b"before\x00after, changed\n");
    repo.write("plain.txt", b"ordinary, changed\n");
    repo.write("macro.dat", b"text under\nthe binary macro, changed\n");
    repo.write("nul-at-7999.dat", &nul_at(7999, "changed\n"));
    repo.write("nul-at-8000.txt", &nul_at(8000, "changed\n"));
    repo.commit("edits");

    repo
}

/// Short lines of text with one NUL at byte `at`, then `tail` — every line well under the
/// line-length ceiling, so the only thing that can make the file binary is the NUL.
fn nul_at(at: usize, tail: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(at + 64);
    while bytes.len() < at {
        let line = b"forty bytes of ordinary text on a line\n";
        let room = at - bytes.len();
        bytes.extend_from_slice(&line[..line.len().min(room)]);
    }
    bytes.push(0);
    bytes.extend_from_slice(b" after the NUL\nmore text\n");
    bytes.extend_from_slice(tail.as_bytes());
    bytes
}

/// `core.bigFileThreshold` at 1 KiB: git calls a file past it binary without looking at
/// its bytes. One text file of a few KiB past it, and one small file under it.
pub fn big_file_threshold() -> Repo {
    let repo = Repo::new("big-file-threshold");
    repo.config("core.bigFileThreshold", "1k");
    let text = |tail: &str| -> Vec<u8> {
        let mut bytes: Vec<u8> = (0..64)
            .flat_map(|n| format!("line {n} of plain text past the threshold\n").into_bytes())
            .collect();
        bytes.extend_from_slice(tail.as_bytes());
        bytes
    };
    repo.write("past-the-threshold.txt", &text(""));
    repo.write("under-the-threshold.txt", b"small\n");
    repo.commit("seed");
    repo.write("past-the-threshold.txt", &text("changed\n"));
    repo.write("under-the-threshold.txt", b"small, changed\n");
    repo.commit("edits");
    repo
}

/// Whether the trap program ever ran. It writes this file first thing.
pub fn trap_ran(repo: &Repo) -> bool {
    repo.path().join("trap-ran").exists()
}

/// Runs the trap by hand, so the test that asserts it never ran is not asserting against a
/// script that could not run in the first place.
pub fn run_trap(repo: &Repo) {
    let status = Command::new("sh")
        .arg(repo.path().join("trap.sh"))
        .arg(repo.path().join("trap.txt"))
        .stdout(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("running the trap by hand: {e}"));
    assert!(status.success(), "the trap program does not run at all");
}

/// One file past each of R2.6's three ceilings, and one comfortably inside them.
pub fn oversized() -> Repo {
    let repo = Repo::new("oversized");
    repo.write("small.txt", b"inside every limit\n");
    repo.write("too-many-bytes.txt", &[b'a'; 64]);
    repo.write("too-many-lines.txt", b"one\n");
    repo.write("line-too-long.txt", b"short\n");
    repo.commit("seed");

    // 1 MiB + 1 byte, in lines short enough that only the byte ceiling fires.
    let mut bytes = Vec::with_capacity(1024 * 1024 + 64);
    while bytes.len() <= 1024 * 1024 {
        bytes.extend_from_slice(b"0123456789abcdef0123456789abcdef\n");
    }
    repo.write("too-many-bytes.txt", &bytes);

    // 50,001 lines, and 100 KB: over the line ceiling and well under the byte one.
    let many: Vec<u8> = std::iter::repeat_n(b"x\n".as_slice(), 50_001)
        .flatten()
        .copied()
        .collect();
    repo.write("too-many-lines.txt", &many);

    // One line of 2,049 bytes, and nothing else over any other ceiling.
    let mut long = vec![b'y'; 2049];
    long.push(b'\n');
    repo.write("line-too-long.txt", &long);

    repo.write("small.txt", b"inside every limit, edited\n");
    repo.commit("over the limits");
    repo
}

/// A repository whose over-limit blob is a loose object truncated after its header.
///
/// gix reads a loose object's header by inflating into a fixed buffer, so the SIZE is
/// still readable while the CONTENT is not. A query that answers "too large" from the
/// header therefore proves it never read the content, and a query that asks for the
/// content anyway fails — which is what makes the first assertion mean something.
pub fn truncated_object() -> (Repo, String) {
    let repo = Repo::new("truncated");
    repo.write("big.txt", b"seed\n");
    repo.commit("seed");

    let mut bytes = Vec::with_capacity(2 * 1024 * 1024 + 64);
    let mut line = 0u64;
    while bytes.len() <= 2 * 1024 * 1024 {
        bytes
            .extend_from_slice(format!("line {line} of a file past the byte ceiling\n").as_bytes());
        line += 1;
    }
    repo.write("big.txt", &bytes);
    repo.commit("a file past the byte ceiling");

    let id = repo.git(&["rev-parse", "HEAD:big.txt"]).trim().to_owned();
    let loose = repo
        .path()
        .join(".git/objects")
        .join(&id[..2])
        .join(&id[2..]);
    assert!(
        loose.is_file(),
        "{} is not a loose object; the fixture needs one to truncate",
        loose.display()
    );
    let whole = std::fs::read(&loose).unwrap_or_else(|e| panic!("reading the loose object: {e}"));
    assert!(
        whole.len() > 512,
        "the object compressed to {} bytes, which is too small to truncate",
        whole.len()
    );
    // git writes a loose object read-only.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&loose, std::fs::Permissions::from_mode(0o644))
        .unwrap_or_else(|e| panic!("making the loose object writable: {e}"));
    std::fs::write(&loose, &whole[..512]).unwrap_or_else(|e| panic!("truncating: {e}"));
    (repo, id)
}

/// Text similar enough for git to pair it by similarity but not identical, so the
/// exhaustive stage has something to find.
fn variation(seed: usize, tweak: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for line in 0..40 {
        out.extend_from_slice(format!("file {seed} line {line} of ordinary content\n").as_bytes());
    }
    out.extend_from_slice(tweak.as_bytes());
    out
}

/// Renames, copies and a rename limit, in one history. `config` is applied to the
/// repository before anything is read, which is how the queries under test see it.
///
/// Commits, oldest first: `seed` (eight files), `rename` (four of them moved, three of
/// those edited), `copy` (one file copied beside itself, unchanged, with one edit
/// elsewhere so the copy source is in the modified set).
pub fn rewrites(config: &[(&str, &str)]) -> Repo {
    let repo = Repo::new("rewrites");
    for (key, value) in config {
        repo.config(key, value);
    }
    for seed in 0..8 {
        repo.write(&format!("src/file{seed}.txt"), &variation(seed, ""));
    }
    repo.commit("seed");

    for seed in 0..4 {
        repo.remove(&format!("src/file{seed}.txt"));
        let tweak = if seed == 3 { "" } else { "one more line\n" };
        repo.write(&format!("moved/file{seed}.txt"), &variation(seed, tweak));
    }
    repo.commit("rename");

    // The copy's source has to be in the set of modified files, which is where git's `-C`
    // and gix's default `CopySource` both look: `file6` is edited and copied at once.
    repo.write("src/file6-copy.txt", &variation(6, ""));
    repo.write("src/file6.txt", &variation(6, "edited\n"));
    repo.commit("copy");
    repo
}

/// What `diff.renameLimit` decides, in two commits after a seed.
///
/// `exact and inexact`: three files moved unchanged — paired by git's exact stage, before
/// any limit — and two moved with an edit to a different name, which only the exhaustive
/// stage can pair: two sources against two destinations, so a limit of 2 lets the search
/// run and a limit of 1 cuts it short, and a git that still counted the exactly paired
/// sources would cut it short at 2 as well. `basename`: one file moved with an edit under
/// its own name, which git pairs by name ahead of the limit, and one moved to a new name.
pub fn limits(config: &[(&str, &str)]) -> Repo {
    let repo = Repo::new("limits");
    for (key, value) in config {
        repo.config(key, value);
    }
    for (path, seed) in [
        ("exact/e0.txt", 10),
        ("exact/e1.txt", 11),
        ("exact/e2.txt", 12),
        ("old/a.txt", 13),
        ("old/b.txt", 14),
        ("old/x.txt", 15),
        ("old/y.txt", 16),
    ] {
        repo.write(path, &variation(seed, ""));
    }
    repo.commit("seed");

    for n in 0..3 {
        repo.remove(&format!("exact/e{n}.txt"));
        repo.write(&format!("moved/f{n}.txt"), &variation(10 + n, ""));
    }
    repo.remove("old/a.txt");
    repo.remove("old/b.txt");
    repo.write("new/c.txt", &variation(13, "more\n"));
    repo.write("new/d.txt", &variation(14, "more\n"));
    repo.commit("exact and inexact");

    repo.remove("old/x.txt");
    repo.remove("old/y.txt");
    repo.write("new/x.txt", &variation(15, "more\n"));
    repo.write("new/z.txt", &variation(16, "more\n"));
    repo.commit("basename");
    repo
}

/// Two deletions and two additions with nothing in common, in the commit after a seed:
/// a search git runs pairs nothing, so the answer's unpaired counts are exactly what git
/// counted — two sources by two destinations, which a `diff.renameLimit` of 2 lets run
/// (four is not MORE than four) and a limit of 1 cuts short.
pub fn dissimilar(config: &[(&str, &str)]) -> Repo {
    let repo = Repo::new("dissimilar");
    for (key, value) in config {
        repo.config(key, value);
    }
    repo.write("gone/one.txt", &variation(30, ""));
    repo.write("gone/two.txt", &variation(31, ""));
    repo.commit("seed");
    repo.remove("gone/one.txt");
    repo.remove("gone/two.txt");
    let unlike = |word: &str| -> Vec<u8> {
        (0..40)
            .map(|n| format!("{word} {n} shares nothing with what was deleted\n"))
            .collect::<String>()
            .into_bytes()
    };
    repo.write("came/three.txt", &unlike("zebra"));
    repo.write("came/four.txt", &unlike("quokka"));
    repo.commit("unrelated");
    repo
}

/// A merge with a first parent worth diffing against, beside a root commit in the same
/// history.
pub fn merged() -> Repo {
    let repo = Repo::new("merged");
    repo.write("shared.txt", b"base\nlines\nhere\n");
    repo.commit("root");
    repo.git(&["branch", "side"]);

    repo.write("shared.txt", b"base\nlines\nhere\nfrom main\n");
    repo.write("main-only.txt", b"main\n");
    repo.commit("on main");

    repo.git(&["checkout", "--quiet", "side"]);
    repo.write("side-only.txt", b"side\n");
    repo.commit("on side");

    repo.git(&["checkout", "--quiet", "main"]);
    repo.try_git(
        &[
            "merge",
            "--quiet",
            "--no-ff",
            "--no-edit",
            "-m",
            "merge side",
            "side",
        ],
        &[
            ("GIT_AUTHOR_DATE", "1600009999 +0000"),
            ("GIT_COMMITTER_DATE", "1600009999 +0000"),
        ],
        None,
    )
    .unwrap_or_else(|e| panic!("merging: {e}"));
    repo
}

/// A commit whose author and committer differ in name, address and time zone, with a
/// message of several paragraphs — the fields R5.3 draws and `CommitSummary` does not
/// carry.
pub fn signed_by_two_people() -> Repo {
    let repo = Repo::new("signatures");
    repo.write("f.txt", b"one\n");
    repo.commit("root");
    repo.write("f.txt", b"two\n");
    repo.git(&["add", "--all", "."]);
    repo.try_git(
        &[
            "commit",
            "--quiet",
            "-m",
            "a subject line\n\nA body that runs\nover several lines.\n\nAnd a third paragraph.",
        ],
        &[
            ("GIT_AUTHOR_NAME", "Ada Lovelace"),
            ("GIT_AUTHOR_EMAIL", "ada@example.org"),
            ("GIT_AUTHOR_DATE", "1600000123 +0530"),
            ("GIT_COMMITTER_NAME", "Grace Hopper"),
            ("GIT_COMMITTER_EMAIL", "grace@example.net"),
            ("GIT_COMMITTER_DATE", "1600009876 -0800"),
        ],
        None,
    )
    .unwrap_or_else(|e| panic!("committing: {e}"));
    repo
}

/// Submodules — gitlinks, mode `160000` — beside an inexact rename, in two commits.
///
/// `.gitmodules` names `s` after its path and `t` as `named`; `u` is a gitlink it does not
/// name at all. The second commit moves `s` and `t` to another commit, adds `u`, and
/// renames `a.txt` to `b.txt` with an edit — one deletion against one addition, which a
/// `diff.renameLimit` of 1 lets git search only when the added gitlink is not counted.
/// The gitlinks point at this repository's own first commit and its parent-less twin, so
/// nothing is fetched and no submodule is checked out; they are staged by
/// `git update-index`, because `git add --all` would drop a gitlink whose directory is
/// missing. `config` is applied before anything is committed.
pub fn submodules(config: &[(&str, &str)]) -> Repo {
    let repo = Repo::new("submodules");
    for (key, value) in config {
        repo.config(key, value);
    }
    let commit = |message: &str, stamp: i64| {
        let stamp = format!("{stamp} +0000");
        repo.try_git(
            &["commit", "--quiet", "--allow-empty", "-m", message],
            &[("GIT_AUTHOR_DATE", &stamp), ("GIT_COMMITTER_DATE", &stamp)],
            None,
        )
        .unwrap_or_else(|e| panic!("committing {message:?}: {e}"));
    };
    let gitlink = |path: &str, id: &str| {
        repo.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{id},{path}"),
        ]);
    };
    commit("targets", EPOCH - 120);
    let first = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    commit("another target", EPOCH - 60);
    let second = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();

    repo.write("a.txt", &variation(20, ""));
    repo.write(
        ".gitmodules",
        b"[submodule \"s\"]\n\tpath = s\n\turl = ../s\n[submodule \"named\"]\n\tpath = t\n\turl = ../t\n",
    );
    repo.git(&["add", "a.txt", ".gitmodules"]);
    gitlink("s", &first);
    gitlink("t", &first);
    commit("seed", EPOCH);

    repo.git(&["mv", "a.txt", "b.txt"]);
    repo.write("b.txt", &variation(20, "one more line\n"));
    repo.git(&["add", "b.txt"]);
    gitlink("s", &second);
    gitlink("t", &second);
    gitlink("u", &second);
    commit("move them", EPOCH + 60);
    repo
}

/// A seeded generator, so a fixture built from it is the same on every machine and every
/// run (a 64-bit LCG, Knuth's MMIX constants).
pub struct Seeded(u64);

impl Seeded {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % bound.max(1)
    }

    pub fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[self.below(from.len())]
    }
}

/// Lines from a small vocabulary of braces, blanks, repeated statements and declarations:
/// what makes a diff ambiguous — a change that could slide up or down over equal lines, a
/// block whose best placement the four algorithms and the indent heuristic disagree on.
const VOCABULARY: &[&str] = &[
    "{",
    "}",
    "",
    "",
    "    x = 1;",
    "    x = 1;",
    "    return x;",
    "    if (x) {",
    "    }",
    "        y();",
    "int f(void)",
    "int g(void)",
    "static int h;",
    "/* note */",
    "    z();",
];

fn random_lines(random: &mut Seeded, count: usize) -> Vec<String> {
    (0..count)
        .map(|_| random.pick(VOCABULARY).to_owned())
        .collect()
}

/// `lines` with a few random edits: blocks inserted, deleted and replaced.
fn edited(random: &mut Seeded, lines: &[String]) -> Vec<String> {
    let mut out = lines.to_vec();
    for _ in 0..1 + random.below(6) {
        let at = random.below(out.len() + 1);
        let block = {
            let count = 1 + random.below(4);
            random_lines(random, count)
        };
        match random.below(3) {
            0 => {
                for (offset, line) in block.into_iter().enumerate() {
                    out.insert((at + offset).min(out.len()), line);
                }
            }
            1 => {
                let end = (at + 1 + random.below(3)).min(out.len());
                if at < end {
                    out.drain(at..end);
                }
            }
            _ => {
                let end = (at + block.len()).min(out.len());
                if at < end {
                    out.splice(at..end, block);
                }
            }
        }
    }
    out
}

fn joined(lines: &[String]) -> Vec<u8> {
    let mut out = lines.join("\n").into_bytes();
    out.push(b'\n');
    out
}

/// Functions in the shape git's default function-name rule finds — a line that starts with
/// a letter — with indented bodies, so the text after a hunk's `@@` depends on where the
/// hunk starts, and so on the context.
fn functions(names: &[&str], body: usize) -> Vec<String> {
    let mut out = Vec::new();
    for name in names {
        out.push(format!("int {name}(void)"));
        out.push("{".to_owned());
        for n in 0..body {
            out.push(format!("    {name}_step({n});"));
        }
        out.push("}".to_owned());
        out.push(String::new());
    }
    out
}

/// The same for a driver whose `xfuncname` captures part of the line: `sub <name>`, of
/// which git shows the name alone.
fn subs(names: &[&str], body: usize) -> Vec<String> {
    let mut out = Vec::new();
    for name in names {
        out.push(format!("sub {name} {{"));
        for n in 0..body {
            out.push(format!("    my ${name}{n} = {n};"));
        }
        out.push("}".to_owned());
    }
    out
}

/// Content on which git's answers discriminate — between the four algorithms, with and
/// without the indent heuristic, between a diff driver's algorithm and `diff.algorithm` —
/// and whose hunks carry function context, from git's default rule and from a driver's
/// `xfuncname`. `config` is applied before anything is committed; `diff.renames=copies` and
/// the `drv` driver's `xfuncname` always are.
///
/// One commit after a seed. `random/` holds seeded files edited at random; `heuristic.py`
/// the indent heuristic's own example, a function inserted between two others; `code/`
/// functions edited at depths that put hunks inside and across them, a rename and a copy of
/// one of them with edits; `drv/` the same under the `drv` driver, and files added,
/// deleted, emptied and filled, under the driver and without it.
pub fn discriminating(config: &[(&str, &str)]) -> Repo {
    let repo = Repo::new("discriminating");
    repo.config("diff.renames", "copies");
    repo.config("diff.drv.xfuncname", "^sub ([a-z]+)");
    for (key, value) in config {
        repo.config(key, value);
    }
    repo.write(".gitattributes", b"drv/** diff=drv\n");

    let mut random = Seeded::new(20_261_003);
    let mut pairs: Vec<(String, Vec<u8>, Vec<u8>)> = Vec::new();
    for n in 0..48 {
        let old = {
            let count = 30 + random.below(90);
            random_lines(&mut random, count)
        };
        let new = edited(&mut random, &old);
        pairs.push((format!("random/r{n:02}.c"), joined(&old), joined(&new)));
    }
    for n in 0..16 {
        let old = {
            let count = 40 + random.below(60);
            random_lines(&mut random, count)
        };
        let new = edited(&mut random, &old);
        pairs.push((format!("drv/r{n:02}.pl"), joined(&old), joined(&new)));
    }
    // Long enough, and changed enough, that myers stops searching for the shortest script
    // (its cost limit) where minimal does not.
    for n in 0..2 {
        let old = random_lines(&mut random, 3000);
        let mut new = old.clone();
        for _ in 0..800 {
            let at = random.below(new.len());
            new[at] = random.pick(VOCABULARY).to_owned();
        }
        pairs.push((format!("random/large{n}.c"), joined(&old), joined(&new)));
    }
    pairs.push((
        "heuristic.py".to_owned(),
        b"def a():\n    one\n\ndef c():\n    three\n".to_vec(),
        b"def a():\n    one\n\ndef b():\n    two\n\ndef c():\n    three\n".to_vec(),
    ));

    let names = ["alpha", "bravo", "charlie", "delta", "echo"];
    let code = functions(&names, 9);
    let mut code_new = code.clone();
    for (at, line) in [
        (4usize, "    alpha_extra();"),
        (18, "    bravo_moved();"),
        (40, "    delta_new();"),
        (41, "    delta_more();"),
    ] {
        code_new.insert(at, line.to_owned());
    }
    code_new[27] = "    charlie_step(changed);".to_owned();
    pairs.push((
        "code/functions.c".to_owned(),
        joined(&code),
        joined(&code_new),
    ));

    let perl = subs(&names, 8);
    let mut perl_new = perl.clone();
    perl_new[3] = "    my $alpha2 = 'two';".to_owned();
    perl_new[15] = "    my $bravo4 = 'four';".to_owned();
    perl_new.insert(30, "    my $delta_extra = 0;".to_owned());
    pairs.push(("drv/subs.pl".to_owned(), joined(&perl), joined(&perl_new)));

    // The copy's source is edited in the same commit, which is where `-C` looks for one.
    let source = functions(&["source", "kept"], 12);
    let mut source_edited = source.clone();
    source_edited[5] = "    source_step(edited);".to_owned();
    let mut copy = source.clone();
    copy[20] = "    kept_step(in the copy);".to_owned();
    let renamed = functions(&["moving", "along"], 12);
    let mut renamed_edited = renamed.clone();
    renamed_edited[24] = "    along_step(renamed);".to_owned();

    for (path, old, _) in &pairs {
        repo.write(path, old);
    }
    repo.write("code/source.c", &joined(&source));
    repo.write("code/moving.c", &joined(&renamed));
    for path in ["drv/deleted.pl", "deleted.c", "drv/emptied.pl", "emptied.c"] {
        repo.write(path, &joined(&subs(&["gone"], 4)));
    }
    for path in ["drv/filled.pl", "filled.c"] {
        repo.write(path, b"");
    }
    repo.commit("seed");

    for (path, _, new) in &pairs {
        repo.write(path, new);
    }
    repo.write("code/source.c", &joined(&source_edited));
    repo.write("code/source-copy.c", &joined(&copy));
    repo.remove("code/moving.c");
    repo.write("code/moved.c", &joined(&renamed_edited));
    for path in ["drv/deleted.pl", "deleted.c"] {
        repo.remove(path);
    }
    for path in ["drv/emptied.pl", "emptied.c"] {
        repo.write(path, b"");
    }
    for path in ["drv/filled.pl", "filled.c", "drv/added.pl", "added.c"] {
        repo.write(path, &joined(&subs(&["fresh", "new"], 3)));
    }
    repo.commit("edits");
    repo
}

/// Paths `git diff` quotes or that a careless reader splits: a space, a double quote, a
/// tab, a newline, a backslash, a non-ASCII letter, glob characters, and a leading `-`, each
/// edited in the middle; and one renamed with an edit from one such path to another.
pub const UNUSUAL_PATHS: &[&str] = &[
    "a b.txt",
    "q\"t.txt",
    "tab\t.txt",
    "nl\n.txt",
    "back\\slash.txt",
    "\u{e9}.txt",
    "[ab].txt",
    "-lead.txt",
];

/// [`UNUSUAL_PATHS`] seeded and then each edited, beside `a.txt` and `b.txt`, which a
/// glob `[ab].txt` would match, unchanged; and `from \"x\".txt` renamed to `to\t\u{e9}.txt`.
pub fn unusual_paths() -> Repo {
    let repo = Repo::new("unusual-paths");
    let body = |path: &str| -> String {
        (0..12)
            .map(|line| format!("{path:?} line {line}\n"))
            .collect()
    };
    for path in UNUSUAL_PATHS
        .iter()
        .chain(&["a.txt", "b.txt", "from \"x\".txt"])
    {
        repo.write(path, body(path).as_bytes());
    }
    repo.commit("seed");
    for path in UNUSUAL_PATHS {
        repo.write(path, body(path).replace("line 6", "line six").as_bytes());
    }
    repo.remove("from \"x\".txt");
    repo.write(
        "to\t\u{e9}.txt",
        body("from \"x\".txt")
            .replace("line 6", "line six")
            .as_bytes(),
    );
    repo.commit("edit every unusual path");
    repo
}

/// Two files renamed with an edit across the `drv` driver's boundary — `drv/in.pl` out to
/// `out/in.pl`, and `out/out.pl` in to `drv/out.pl` — each long and changed enough that
/// myers, which stops searching at its cost limit, and minimal give different answers; with
/// `diff.drv.algorithm = minimal` configured, and `config` after it.
pub fn renamed_across_a_driver(config: &[(&str, &str)]) -> Repo {
    let repo = Repo::new("renamed-across-a-driver");
    repo.config("diff.drv.algorithm", "minimal");
    for (key, value) in config {
        repo.config(key, value);
    }
    repo.write(".gitattributes", b"drv/** diff=drv\n");
    // Whether myers stops short of minimal is a property of the content, so each pair is
    // drawn until git itself shows the two algorithms disagree on it.
    let mut random = Seeded::new(20_261_004);
    let mut versions = Vec::new();
    while versions.len() < 2 {
        let old = random_lines(&mut random, 3000);
        let mut new = old.clone();
        for _ in 0..600 {
            let at = random.below(new.len());
            new[at] = random.pick(VOCABULARY).to_owned();
        }
        let (old, new) = (joined(&old), joined(&new));
        repo.write("probe/old", &old);
        repo.write("probe/new", &new);
        let under = |algorithm: &str| {
            repo.run(
                &[
                    "diff",
                    "--no-index",
                    &format!("--diff-algorithm={algorithm}"),
                    "probe/old",
                    "probe/new",
                ],
                &[],
                None,
            )
            .1
        };
        if under("myers") != under("minimal") {
            versions.push((old, new));
        }
    }
    repo.remove("probe/old");
    repo.remove("probe/new");
    repo.write("drv/in.pl", &versions[0].0);
    repo.write("out/out.pl", &versions[1].0);
    repo.commit("seed");
    repo.remove("drv/in.pl");
    repo.write("out/in.pl", &versions[0].1);
    repo.remove("out/out.pl");
    repo.write("drv/out.pl", &versions[1].1);
    repo.commit("rename across the driver");
    repo
}

/// Whitespace-only edits beside real ones, seeded: indentation changed, trailing blanks
/// added, a tab for spaces, a blank line's spaces, a line split by a space — and files with
/// both, and one with only a real edit.
pub fn whitespace() -> Repo {
    let repo = Repo::new("whitespace-parity");
    let mut random = Seeded::new(7);
    let mut pairs: Vec<(String, Vec<u8>, Vec<u8>)> = Vec::new();
    for n in 0..32 {
        let old = {
            let count = 20 + random.below(50);
            random_lines(&mut random, count)
        };
        let mut new = old.clone();
        for _ in 0..1 + random.below(5) {
            let at = random.below(new.len());
            new[at] = match random.below(5) {
                0 => format!("  {}", new[at]),
                1 => format!("{} \t", new[at]),
                2 => new[at].replace("    ", "\t"),
                3 => new[at].replace(' ', ""),
                _ => new[at].replace("x = 1", "x  =  1"),
            };
        }
        // Half of the files also carry a real edit.
        let new = if n % 2 == 0 {
            edited(&mut random, &new)
        } else {
            new
        };
        pairs.push((format!("w{n:02}.c"), joined(&old), joined(&new)));
    }
    pairs.push((
        "only-whitespace.c".to_owned(),
        joined(&functions(&["alpha"], 4)),
        joined(&functions(&["alpha"], 4))
            .iter()
            .flat_map(|byte| {
                if *byte == b'\n' {
                    vec![b' ', b'\n']
                } else {
                    vec![*byte]
                }
            })
            .collect(),
    ));
    let real = functions(&["alpha", "bravo"], 6);
    let mut real_new = real.clone();
    real_new[10] = "    bravo_step(different);".to_owned();
    pairs.push(("only-real.c".to_owned(), joined(&real), joined(&real_new)));
    for (path, old, _) in &pairs {
        repo.write(path, old);
    }
    repo.commit("seed");
    for (path, _, new) in &pairs {
        repo.write(path, new);
    }
    repo.commit("respace");
    repo
}

impl Repo {
    /// A clone of the repository at `source` sharing its objects (`git clone --shared`),
    /// so a test can set configuration on it without touching the source; removed when
    /// the test ends.
    pub fn shared_clone_of(source: &Path, name: &str) -> Self {
        let repo = Self::new(name);
        std::fs::remove_dir_all(repo.path().join(".git"))
            .unwrap_or_else(|e| panic!("emptying the clone's directory: {e}"));
        repo.git(&[
            "clone",
            "--quiet",
            "--shared",
            "--no-checkout",
            &source.to_string_lossy(),
            ".",
        ]);
        repo
    }
}
