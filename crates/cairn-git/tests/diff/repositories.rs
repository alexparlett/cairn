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
        let mut command = Command::new("git");
        command
            .current_dir(&self.path)
            .args(args)
            // Isolate from the machine's git config, e.g. a global `commit.gpgsign`.
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
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
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(format!("{} {stderr}", output.status))
        }
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
/// `rename with edits`, `mode change`, `type change`, `copy`.
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
    repo.commit("seed");

    // Two changes far enough apart to be two hunks at three lines of context, and two
    // close enough to merge into one.
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
        b"no-diff.txt -diff\nflagged.dat diff=flagged\ntrap.txt diff=trap\n",
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
