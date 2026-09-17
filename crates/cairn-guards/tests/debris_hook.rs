//! The Stop hook's debris scan, run against scratch repositories.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use cairn_guards::repo_root;

fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    result.unwrap_or_else(|error| panic!("{what}: {error}"))
}

// Split so this file's own lines are not debris to the hook it tests.
const DBG_LINE: &str = concat!("pub fn f(x: u8) -> u8 { db", "g!(x) }\n");
const DBG_HIT: &str = concat!(
    "src/lib.rs:pub fn f(x: u8) -> u8 { db",
    "g!(x) } [dbg! left in]"
);
const REPORTER: &str = concat!(
    "#[test]\n#[ign",
    "ore = \"needs a large repository\"]\nfn measures() {\n    eprint",
    "ln!(\"took {}\", 1);\n}\n"
);
const BARE_IGNORE: &str = concat!("#[test]\n#[ign", "ore]\nfn skipped() {}\n");

/// Git environment a caller can leak in: a pre-push hook in a linked worktree exports `GIT_DIR`.
const INHERITED_GIT_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CEILING_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_PREFIX",
];

/// `command` with every inherited repository-selecting variable removed.
fn isolated(command: &mut Command) -> &mut Command {
    for var in INHERITED_GIT_VARS {
        command.env_remove(var);
    }
    command
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    /// A repository with one clean commit on `main`, checked out on `feature`.
    fn on_a_branch() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "cairn-debris-hook-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        ok(
            std::fs::create_dir_all(&path),
            "making the scratch directory",
        );
        let scratch = Self { path };
        scratch.git(&["init", "--quiet", "--initial-branch=main", "."]);
        scratch.write("src/lib.rs", "pub fn clean() {}\n");
        scratch.commit("clean");
        scratch.git(&["checkout", "--quiet", "-b", "feature"]);
        scratch
    }

    fn git(&self, args: &[&str]) {
        let status = isolated(&mut Command::new("git"))
            .current_dir(&self.path)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "A U Thor")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "C O Mitter")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .stdout(Stdio::null())
            .status();
        let status = ok(status, "running git");
        assert!(status.success(), "git {args:?} failed");
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.path.join(file);
        if let Some(parent) = path.parent() {
            ok(std::fs::create_dir_all(parent), "making a source directory");
        }
        ok(std::fs::write(&path, text), "writing a source file");
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "-m", message]);
    }

    /// The hook's stdout: empty when it lets the turn stop.
    fn hook(&self) -> String {
        run_hook(&self.path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run_hook(project: &Path) -> String {
    let child = isolated(&mut Command::new("bash"))
        .arg(repo_root().join(".claude/hooks/qa-stop.sh"))
        .env("CLAUDE_PROJECT_DIR", project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();
    let mut child = ok(child, "running the hook");
    {
        use std::io::Write;
        let Some(mut stdin) = child.stdin.take() else {
            panic!("the hook's stdin was not piped");
        };
        ok(stdin.write_all(b"{}"), "writing the hook's input");
    }
    let output = ok(child.wait_with_output(), "waiting for the hook");
    assert!(output.status.success(), "the hook itself failed");
    ok(
        String::from_utf8(output.stdout),
        "reading the hook's output",
    )
}

fn blocks(output: &str, because: &str) -> bool {
    output.contains("\"decision\":\"block\"") && output.contains(because)
}

#[test]
fn a_clean_branch_is_let_through() {
    let scratch = Scratch::on_a_branch();
    scratch.write("src/lib.rs", "pub fn clean() {}\npub fn also_clean() {}\n");
    scratch.commit("more");
    assert_eq!(scratch.hook(), "", "a clean branch was blocked");
}

#[test]
fn uncommitted_debris_is_caught() {
    let scratch = Scratch::on_a_branch();
    // A commit on the branch, so the branch scan also sees the uncommitted line.
    scratch.write("src/other.rs", "pub fn clean() {}\n");
    scratch.commit("clean on the branch");
    scratch.write("src/lib.rs", DBG_LINE);
    let output = scratch.hook();
    assert!(blocks(&output, DBG_HIT));
    assert_eq!(
        output.matches(DBG_HIT).count(),
        1,
        "a line both scans see was reported twice: {output:?}"
    );
}

/// Caught by: scanning only the diff against `HEAD`.
#[test]
fn debris_committed_on_the_branch_is_still_caught() {
    let scratch = Scratch::on_a_branch();
    scratch.write("src/lib.rs", DBG_LINE);
    scratch.commit("debris");
    let output = scratch.hook();
    assert!(
        blocks(&output, DBG_HIT),
        "a committed dbg! was let through: {output:?}"
    );

    scratch.write("src/lib.rs", "pub fn f(x: u8) -> u8 { x }\n");
    assert_eq!(
        scratch.hook(),
        "",
        "debris already removed from the working tree still blocked"
    );
}

#[test]
fn debris_already_on_main_is_not_the_branchs_to_fix() {
    let scratch = Scratch::on_a_branch();
    scratch.git(&["checkout", "--quiet", "main"]);
    scratch.write("src/lib.rs", DBG_LINE);
    scratch.commit("debris on main");
    scratch.git(&["checkout", "--quiet", "feature"]);
    scratch.git(&["merge", "--quiet", "--ff-only", "main"]);
    scratch.write("src/other.rs", "pub fn clean() {}\n");
    scratch.commit("clean on the branch");
    assert_eq!(scratch.hook(), "");
}

/// A measurement reporter prints and is ignored with a reason, on purpose, once committed.
#[test]
fn a_committed_reporter_passes_but_a_new_one_is_questioned() {
    let scratch = Scratch::on_a_branch();
    scratch.write("src/report.rs", REPORTER);
    let uncommitted = scratch.hook();
    assert!(
        blocks(&uncommitted, "left in; delete it"),
        "{uncommitted:?}"
    );
    assert!(
        blocks(&uncommitted, "ignored test left in"),
        "{uncommitted:?}"
    );

    scratch.commit("reporter");
    assert_eq!(
        scratch.hook(),
        "",
        "a committed reporter blocked every turn"
    );

    scratch.write("src/skip.rs", BARE_IGNORE);
    scratch.commit("bare ignore");
    assert!(
        blocks(&scratch.hook(), "ignored test left in"),
        "a committed ignore attribute with no reason was let through"
    );
}

/// Caught by: a scratch command obeying an inherited `GIT_DIR` and writing to the real repository.
#[test]
fn scratch_commands_ignore_an_inherited_repository() {
    let scratch = Scratch::on_a_branch();
    let decoy = Scratch::on_a_branch();
    let mut command = Command::new("git");
    command
        .env("GIT_DIR", decoy.path.join(".git"))
        .env("GIT_WORK_TREE", &decoy.path);
    let output = isolated(&mut command)
        .current_dir(&scratch.path)
        .args(["rev-parse", "--absolute-git-dir"])
        .output();
    let output = ok(output, "running git");
    let git_dir = ok(String::from_utf8(output.stdout), "reading git's output");
    assert_eq!(
        Path::new(git_dir.trim()),
        ok(
            scratch.path.join(".git").canonicalize(),
            "resolving the scratch git dir"
        ),
        "an inherited GIT_DIR chose the repository"
    );
}

/// Caught by: an unquoted `*.toml` pathspec globbing to the root manifests only.
#[test]
fn a_nested_manifest_is_scanned_beside_a_root_one() {
    let scratch = Scratch::on_a_branch();
    scratch.write("Cargo.toml", "[workspace]\n");
    scratch.commit("root manifest");
    let sealed = "[dependencies]\ngix = \"1\"\n";
    scratch.write("crates/cairn-ui/Cargo.toml", sealed);
    let uncommitted = scratch.hook();
    assert!(
        blocks(&uncommitted, "cairn-ui is sealed from the git engine"),
        "an uncommitted nested manifest was not scanned: {uncommitted:?}"
    );
    scratch.commit("sealed dependency");
    let committed = scratch.hook();
    assert!(
        blocks(&committed, "cairn-ui is sealed from the git engine"),
        "a committed nested manifest was not scanned: {committed:?}"
    );
}

/// Caught by: a token missing from the grep prefilter silently switching a rule off.
#[test]
fn every_rule_fires_through_the_prefilter_committed_or_not() {
    let debris: &[(&str, &str, &str)] = &[
        (
            "src/conflict.rs",
            concat!("<<<<", "<<< ours\n"),
            "merge conflict marker",
        ),
        (
            "src/theirs.rs",
            concat!(">>>>", ">>> theirs\n"),
            "merge conflict marker",
        ),
        (
            "src/todo.rs",
            concat!("fn a() { to", "do!() }\n"),
            "todo! left in",
        ),
        (
            "src/unfinished.rs",
            concat!("fn b() { unimpl", "emented!() }\n"),
            "unimplemented! left in",
        ),
        (
            "src/dead.rs",
            concat!("#[all", "ow(dead_code)]\nfn c() {}\n"),
            "blanket allow(dead_code/unused)",
        ),
        (
            "crates/cairn-ui/src/reach.rs",
            "use gix::Repository;\n",
            "cairn-ui is sealed from the git engine",
        ),
        (
            "crates/cairn-model/src/draw.rs",
            "use freya::prelude::*;\n",
            "cairn-model is plain data",
        ),
        (
            "crates/cairn-git/src/draw.rs",
            "use cairn_ui::CommitRow;\n",
            "cairn-git is sealed from the UI toolkit",
        ),
        (
            "crates/cairn-app/src/reach.rs",
            "use cairn_git::Repository;\n",
            "only crates/cairn-app/src/worker may reach the git engine",
        ),
        (
            "crates/cairn-askpass/src/leak.rs",
            "use tracing::info;\n",
            "cairn-askpass holds a plaintext secret",
        ),
        (
            "crates/cairn-askpass/src/reach.rs",
            "let repo = gix::open(p)?;\n",
            "cairn-askpass holds a plaintext secret",
        ),
        (
            "crates/cairn-app/src/worker/draw.rs",
            "use freya::prelude::*;\n",
            "the worker module runs off the UI thread",
        ),
    ];

    let scratch = Scratch::on_a_branch();
    for (file, text, _) in debris {
        scratch.write(file, text);
    }
    for state in ["uncommitted", "committed"] {
        if state == "committed" {
            scratch.commit("debris");
        }
        let output = scratch.hook();
        for (file, _, label) in debris {
            assert!(
                output.contains(&format!("{file}:")) && blocks(&output, label),
                "the {state} {label:?} rule did not fire on {file}: {output:?}"
            );
        }
    }
}

/// Caught by: taking the fork point from a stale local `main` instead of a newer `origin/main`.
#[test]
fn a_newer_origin_main_is_where_the_branch_is_measured_from() {
    let scratch = Scratch::on_a_branch();
    scratch.git(&["checkout", "--quiet", "main"]);
    scratch.write("src/lib.rs", DBG_LINE);
    scratch.commit("debris upstream");
    scratch.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    scratch.git(&["checkout", "--quiet", "feature"]);
    scratch.git(&["branch", "--force", "main", "main~1"]);
    scratch.git(&["merge", "--quiet", "--ff-only", "origin/main"]);
    scratch.write("src/other.rs", "pub fn clean() {}\n");
    scratch.commit("clean on the branch");
    assert_eq!(
        scratch.hook(),
        "",
        "debris already in origin/main was charged to the branch"
    );
}
