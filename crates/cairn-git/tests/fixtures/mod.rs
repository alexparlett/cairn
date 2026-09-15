//! Repositories built by running real `git`.
//!
//! A fake object database proves nothing about gitoxide: the whole question
//! these tests answer is whether the engine reads what `git` wrote, in the
//! order `git` reports it. So every fixture here is made by the binary, with
//! its own configuration isolated from the machine's, and every expectation is
//! read back out of `git` rather than written down by hand.
//!
//! `unwrap` is unavailable here — `clippy.toml`'s carve-out only reaches
//! `#[cfg(test)]` code, and an integration test crate is not that — so failure
//! paths name what went wrong instead.

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

    /// Run `git` inside the fixture and return its stdout. Any failure is the
    /// test's failure: a fixture that half-built would make every assertion
    /// after it meaningless.
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
        // The machine's own git configuration must not reach the fixture: a
        // global `commit.gpgsign` or a template directory would make these
        // tests pass or fail depending on whose laptop they run on.
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

/// The clock every fixture commits against. Fixed, so a test that prints a
/// date prints the same one tomorrow.
const EPOCH: i64 = 1_500_000_000;

/// A repository whose committer dates rise strictly along every parent link.
///
/// That is what lets these tests compare an ordering against `git`'s at all:
/// with monotone dates, pure commit-time order and `git`'s reverse-chronological
/// order are the same sequence, so a divergence is gitoxide's, not a
/// disagreement about what "newest first" means. Deliberately not the skew
/// case — that one is the assigner's, and it is pinned in `cairn-model`.
///
/// The shape: a trunk, a side branch that lives across several trunk commits,
/// and merges back into the trunk, repeated. `steps` is how many ordinary
/// commits to make; the merges are extra, so the history is a little longer
/// than `steps` and every expectation is read back from `git` rather than
/// counted from here.
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

/// Merge `side` into the trunk. A no-op when there is nothing to merge, which
/// `git` reports as success without making a commit.
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

/// A repository a newest-first walk hands a parent over before its own child.
///
/// A single chain can never do that — the walk only ever holds one commit at a
/// time, so it emits the chain in order however the dates are stamped. It takes
/// a *fork*: `shared` is the parent of both branches, and because `stale` on the
/// side branch is stamped older than `shared`, the walk reaches `shared` through
/// the trunk and emits it before it ever gets to `stale`. Real `git` produces
/// exactly this every time somebody rebases, imports a history, or has a clock a
/// few minutes out (`docs/research/history-graph/gix-revwalk-ordering.md`,
/// finding 2).
///
/// Shape and stamps, seconds past [`EPOCH`]:
///
/// ```text
///   merge  9500   parents: recent, stale
///   recent 9000   parent: shared        (trunk)
///   shared 8000   parent: base
///   stale  2000   parent: shared        (side branch)
///   base   1000
/// ```
///
/// so commit time orders them `merge, recent, shared, stale, base` — and
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

/// Write a commit-graph file, so a walk can read parent ids and commit times
/// without touching the object database at all.
pub fn write_commit_graph(fixture: &Fixture) {
    fixture.git(&["commit-graph", "write", "--reachable"]);
    // Local config, because the engine reads the repository's own: the machine
    // running this may have turned commit-graph use off globally.
    fixture.git(&["config", "core.commitGraph", "true"]);
    assert!(
        fixture
            .path()
            .join(".git/objects/info/commit-graph")
            .is_file(),
        "git did not write a commit-graph file"
    );
}

/// Delete the loose object backing each of `ids`, so that walking past them
/// still works (from the commit-graph) but reading one fails.
///
/// Never pass a starting point: gitoxide reads the tips themselves out of the
/// object database to seed the walk's queue (`gix-traverse`'s `add_to_queue`),
/// and only the commits it reaches from there come from the commit-graph.
///
/// This is how "the replayed prefix is walked, never decoded" becomes something
/// a test can see. A counter the query increments beside its own `object()`
/// call proves only that *that* call site behaves; an object that is not there
/// any more proves it about every call site at once.
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
