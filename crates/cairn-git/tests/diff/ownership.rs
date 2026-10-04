//! Ownership at open: a repository whose paths git checks are not all the user's is left
//! to git's own discovery, which refuses it as the user's `git` refuses it
//! (`crate::ownership` in `cairn-git`; its pure decision is unit-tested there).

use std::path::PathBuf;

use cairn_git::{CancelSignal, ChangesRequest, SharedRepository};

use super::repositories::Repo;
use super::{git, ok};

/// The privileged half, a stated review step rather than a gate step, because it needs a
/// second owner: as root, `cargo test -p cairn-git --test diff_engine -- --ignored
/// a_linked_worktree_whose_git_dir_is_someone_elses_is_refused_as_git_refuses_it`. A
/// linked worktree whose `.git` file and working tree are the user's but whose git
/// directory (`.git/worktrees/<name>`) is given to `nobody` (uid 65534) is refused by the
/// user's own `git status` there with "dubious ownership"; Cairn opens it with reduced
/// trust, so its read is left to git's discovery, and git refuses that read the same way.
/// Caught by: trust taken from the `.git` file alone, which names the repository to git
/// with `--git-dir` and skips git's check, so the read answers where git refuses. A
/// `safe.directory` in root's own global configuration would let both through; the test
/// says so rather than passing.
#[test]
#[ignore = "needs root, to give a directory to another user"]
fn a_linked_worktree_whose_git_dir_is_someone_elses_is_refused_as_git_refuses_it() {
    let main = Repo::new("ownership");
    main.write("a.txt", b"one\n");
    let head = main.commit("seed");
    let linked: PathBuf = main.path().with_extension("linked");
    let _ = std::fs::remove_dir_all(&linked);
    let _removed = Removed(linked.clone());
    main.git(&["worktree", "add", "--quiet", &linked.display().to_string()]);
    let name = linked
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let worktrees = main.path().join(".git/worktrees").join(name);
    assert!(worktrees.is_dir(), "{} is missing", worktrees.display());
    std::os::unix::fs::chown(&worktrees, Some(65534), Some(65534))
        .expect("giving a directory to nobody needs root: run this test as root");

    let (_, _, refusal) = Repo::borrowed(&linked).run(&["status"], &[], None);
    assert!(
        refusal.contains("dubious ownership"),
        "the user's own git does not refuse it, so this test decides nothing: {refusal}"
    );

    let shared = ok(SharedRepository::discover(&linked), "the worktree opens");
    let engine = shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    match session.changes(git(), &ChangesRequest::commit(head), &CancelSignal::new()) {
        Ok(set) => panic!("a read answered where git refuses the repository: {set:?}"),
        Err(error) => assert!(
            error.to_string().contains("dubious ownership"),
            "refused, but not by git's ownership check: {error}"
        ),
    }
}

/// Removes a directory however the test ends, as a fixture `Repo` removes its own.
struct Removed(PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
