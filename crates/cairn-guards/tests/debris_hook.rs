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
        let status = Command::new("git")
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
    let child = Command::new("bash")
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
    scratch.write("src/lib.rs", DBG_LINE);
    assert!(blocks(&scratch.hook(), DBG_HIT));
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
