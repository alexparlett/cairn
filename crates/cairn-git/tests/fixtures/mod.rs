//! Repositories built by running real `git`. Shared by every integration test
//! binary, so it holds only what each of them uses; a builder one test file
//! alone needs lives in that file, since an unused item here is a warning in
//! every other binary.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A repository in a temporary directory, removed when the test ends.
#[derive(Debug)]
pub struct Fixture {
    path: PathBuf,
}

impl Fixture {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Runs `git` in the fixture and returns stdout. Panics on failure.
    pub fn git(&self, args: &[&str]) -> String {
        run(&self.path, args, None)
    }

    /// Every commit reachable from `HEAD`, newest first, as `git` orders them.
    pub fn rev_list(&self) -> Vec<String> {
        self.git(&["rev-list", "HEAD"])
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Runs `git` in `dir`, isolated from the machine's configuration, dated `at`.
pub fn run(dir: &Path, args: &[&str], at: Option<i64>) -> String {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args(args)
        // Isolate from the machine's git config, e.g. a global `commit.gpgsign`.
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "A U Thor")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_NAME", "C O Mitter")
        .env("GIT_COMMITTER_EMAIL", "committer@example.com");
    if let Some(seconds) = at {
        let stamp = format!("{seconds} +0000");
        command
            .env("GIT_AUTHOR_DATE", &stamp)
            .env("GIT_COMMITTER_DATE", &stamp);
    }
    let output = command
        .output()
        .unwrap_or_else(|e| panic!("could not run git {args:?}: {e}"));
    if !output.status.success() {
        panic!(
            "git {args:?} failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn fresh_directory(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "cairn-history-{}-{name}-{unique}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path)
        .unwrap_or_else(|e| panic!("could not make {}: {e}", path.display()));
    path
}

pub const EPOCH: i64 = 1_500_000_000;

/// Committer dates rise along every parent link. `steps` counts ordinary commits;
/// the merges are extra, so read expectations back from `git`.
pub fn braided(steps: usize) -> Fixture {
    braided_in("sha1", steps)
}

/// [`braided`], in a repository whose objects are named by `object_format` (`sha1` or `sha256`).
pub fn braided_in(object_format: &str, steps: usize) -> Fixture {
    let path = fresh_directory(&format!("braided-{object_format}"));
    let fixture = Fixture { path };
    fixture.git(&[
        "init",
        "--quiet",
        "--initial-branch=main",
        &format!("--object-format={object_format}"),
        ".",
    ]);

    let mut clock = EPOCH;
    commit_at(&fixture, &mut clock, "root");
    fixture.git(&["branch", "side"]);

    for made in 1..steps {
        let branch = if made % 3 == 0 { "side" } else { "main" };
        fixture.git(&["checkout", "--quiet", branch]);
        commit_at(&fixture, &mut clock, &format!("{branch} {made}"));
        if made % 9 == 0 {
            merge_side(&fixture, &mut clock);
        }
    }
    merge_side(&fixture, &mut clock);
    fixture
}

fn commit_at(fixture: &Fixture, clock: &mut i64, message: &str) {
    *clock += 60;
    run(
        fixture.path(),
        &["commit", "--quiet", "--allow-empty", "-m", message],
        Some(*clock),
    );
}

/// A no-op when there is nothing to merge.
fn merge_side(fixture: &Fixture, clock: &mut i64) {
    fixture.git(&["checkout", "--quiet", "main"]);
    *clock += 60;
    run(
        fixture.path(),
        &[
            "merge",
            "--quiet",
            "--no-ff",
            "--no-edit",
            "-m",
            "merge side",
            "side",
        ],
        Some(*clock),
    );
}

pub fn unborn() -> Fixture {
    let path = fresh_directory("unborn");
    let fixture = Fixture { path };
    fixture.git(&["init", "--quiet", "--initial-branch=main", "."]);
    fixture
}
