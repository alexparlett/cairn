//! Repositories built by running real `git`.
//!
//! A fake object database proves nothing about gitoxide, so every fixture is
//! made by the binary with its configuration isolated, and every expectation is
//! read back out of `git`. `unwrap` is unavailable here — `clippy.toml`'s
//! carve-out reaches `#[cfg(test)]` only, and an integration test crate is not
//! that.

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

    /// Runs `git` in the fixture and returns stdout. Any failure panics: a
    /// half-built fixture makes every assertion after it meaningless.
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

fn run(dir: &Path, args: &[&str], at: Option<i64>) -> String {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args(args)
        // The machine's git config must not reach the fixture: a global
        // `commit.gpgsign` would make these pass or fail per laptop.
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

/// Fixed, so a test that prints a date prints the same one tomorrow.
const EPOCH: i64 = 1_500_000_000;

/// A repository whose committer dates rise along every parent link, so
/// commit-time and `git`'s reverse-chronological order are one sequence and a
/// divergence is gitoxide's; the skew case is [`skewed`]. A trunk and a side
/// branch merging back, repeated: `steps` counts ordinary commits and the merges
/// are extra, so read every expectation back from `git`.
pub fn braided(steps: usize) -> Fixture {
    let path = fresh_directory("braided");
    let fixture = Fixture { path };
    fixture.git(&["init", "--quiet", "--initial-branch=main", "."]);

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

/// Merges `side` into the trunk; a no-op when there is nothing to merge, which
/// `git` reports as success without a commit.
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

/// A repository whose newest-first walk hands a parent over before its child.
///
/// A *fork*, not a chain: a chain is emitted in order however the dates are
/// stamped. `stale` is stamped older than `shared`, so the walk reaches `shared`
/// through the trunk and emits it first — what `git` produces whenever somebody
/// rebases or has a clock a few minutes out
/// (`docs/research/history-graph/gix-revwalk-ordering.md`, finding 2). Stamps
/// are seconds past [`EPOCH`]:
///
/// ```text
///   merge  9500   parents: recent, stale
///   recent 9000   parent: shared        (trunk)
///   shared 8000   parent: base
///   stale  2000   parent: shared        (side branch)
///   base   1000
/// ```
///
/// so commit time orders them `merge, recent, shared, stale, base`, and
/// `shared` sits two rows above the child it belongs to.
pub fn skewed() -> Fixture {
    let path = fresh_directory("skewed");
    let fixture = Fixture { path };
    fixture.git(&["init", "--quiet", "--initial-branch=main", "."]);
    commit_stamped(&fixture, "base", EPOCH + 1000);
    commit_stamped(&fixture, "shared", EPOCH + 8000);
    commit_stamped(&fixture, "recent", EPOCH + 9000);
    fixture.git(&["checkout", "--quiet", "-b", "side", "HEAD~1"]);
    commit_stamped(&fixture, "stale", EPOCH + 2000);
    fixture.git(&["checkout", "--quiet", "main"]);
    run(
        fixture.path(),
        &[
            "merge",
            "--quiet",
            "--no-ff",
            "--no-edit",
            "-m",
            "merge",
            "side",
        ],
        Some(EPOCH + 9500),
    );
    fixture
}

fn commit_stamped(fixture: &Fixture, message: &str, seconds: i64) {
    run(
        fixture.path(),
        &["commit", "--quiet", "--allow-empty", "-m", message],
        Some(seconds),
    );
}

/// Writes a commit-graph file, so a walk reads parent ids and commit times
/// without touching the object database.
pub fn write_commit_graph(fixture: &Fixture) {
    fixture.git(&["commit-graph", "write", "--reachable"]);
    // Local config: the machine may have turned commit-graph use off globally.
    fixture.git(&["config", "core.commitGraph", "true"]);
    assert!(
        fixture
            .path()
            .join(".git/objects/info/commit-graph")
            .is_file(),
        "git did not write a commit-graph file"
    );
}

/// Deletes the loose object backing each of `ids`: walking past them still works
/// from the commit-graph, but reading one fails. A counter proves one call site;
/// a missing object proves every call site.
///
/// Never pass a starting point: gitoxide reads the tips out of the object
/// database to seed the walk's queue (`gix-traverse`'s `add_to_queue`).
pub fn delete_objects(fixture: &Fixture, ids: &[String]) {
    for id in ids {
        let (dir, file) = id.split_at(2);
        let path = fixture.path().join(".git/objects").join(dir).join(file);
        std::fs::remove_file(&path)
            .unwrap_or_else(|e| panic!("could not delete {}: {e}", path.display()));
    }
}

/// A repository with `HEAD` on a branch that has no commits.
pub fn unborn() -> Fixture {
    let path = fresh_directory("unborn");
    let fixture = Fixture { path };
    fixture.git(&["init", "--quiet", "--initial-branch=main", "."]);
    fixture
}
