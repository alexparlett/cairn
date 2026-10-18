//! git's date order over a fixture, the order the history from a refs snapshot is drawn in:
//!
//! ```sh
//! git log --date-order --format=%H --branches --remotes --tags HEAD <stashes> <reflog ids>
//! ```
//!
//! each stash whose base the refs reach, and whose commit they do not, given after `HEAD` in
//! `stash@{n}` order with its base as its one parent — grafted, from a graft file outside the
//! repository (`GIT_GRAFT_FILE`), so the fixture itself is never written and its commit-graph
//! stays one git would read — then each reflog id, as Show Lost Commits gives them. A stash
//! whose commit a ref reaches is that commit, with the parents it was made with, in git's own
//! log, so it is given no graft. Shared by the test binaries that compare a history with git;
//! it needs the binary's `fixtures` module beside it.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// git's order for the history of the repository at `dir`: its refs, then `stashes` (each a
/// commit and its base, `stash@{0}` first) where the refs reach the base and not the commit,
/// then `reflogs`.
pub fn git_date_order(dir: &Path, stashes: &[(String, String)], reflogs: &[String]) -> Vec<String> {
    let reached: HashSet<String> = crate::fixtures::run(
        dir,
        &["rev-list", "--branches", "--remotes", "--tags", "HEAD"],
        None,
    )
    .lines()
    .map(str::to_owned)
    .collect();
    let grafted: Vec<&(String, String)> = stashes
        .iter()
        .filter(|(stash, base)| reached.contains(base) && !reached.contains(stash))
        .collect();
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let grafts = std::env::temp_dir().join(format!(
        "cairn-date-order-grafts-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let lines: String = grafted
        .iter()
        .map(|(stash, base)| format!("{stash} {base}\n"))
        .collect();
    std::fs::write(&grafts, lines)
        .unwrap_or_else(|e| panic!("could not write {}: {e}", grafts.display()));
    let mut args = vec![
        "-c",
        "advice.graftFileDeprecated=false",
        "log",
        "--date-order",
        "--format=%H",
        "--branches",
        "--remotes",
        "--tags",
        "HEAD",
    ];
    args.extend(grafted.iter().map(|(stash, _)| stash.as_str()));
    args.extend(reflogs.iter().map(String::as_str));
    let output = Command::new("git")
        .current_dir(dir)
        .args(&args)
        .env("GIT_GRAFT_FILE", &grafts)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", crate::fixtures::empty_home())
        .env("XDG_CONFIG_HOME", crate::fixtures::empty_home())
        .output()
        .unwrap_or_else(|e| panic!("could not run git {args:?}: {e}"));
    let _ = std::fs::remove_file(&grafts);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `git stash list`'s commits, `stash@{0}` first, each with its base.
pub fn stash_tips(dir: &Path) -> Vec<(String, String)> {
    crate::fixtures::run(dir, &["stash", "list", "--format=%H %P"], None)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(' ');
            Some((fields.next()?.to_owned(), fields.next()?.to_owned()))
        })
        .collect()
}
