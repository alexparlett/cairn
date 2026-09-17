//! What a git that was ended before it could clean up leaves behind: the
//! `*.lock` files it takes while writing.
//!
//! git writes a ref, `packed-refs`, `HEAD`, the index or its config by
//! creating `<file>.lock`, filling it and renaming it into place, and removes
//! the lock on any exit it gets to handle — a `SIGTERM` included. A `SIGKILL`,
//! a crash or a lost power supply leave the lock where it was, and from then
//! on every operation that needs that file fails with "another git process
//! seems to be running in this repository" until somebody removes it. The
//! diagnosis is one a user should not need a shell for, so a cancelled
//! operation reports what it found ([`crate::Error::GitCancelled`]'s
//! `stranded_locks`), and this module is the finding. A finding, not a
//! verdict: a listing cannot tell a lock the cancel stranded from one a git
//! in a terminal holds this instant, so what is reported is what is there,
//! and the words around it say when acting is safe. Read-only: it lists, and
//! removing a lock another process may still hold is a decision for a
//! confirmed operation that does not exist yet.
//!
//! Where git puts them: beside the file, so the git directory itself
//! (`HEAD.lock`, `index.lock`, `FETCH_HEAD.lock`, `config.lock`), the common
//! directory when the repository is a linked worktree (`packed-refs.lock`
//! and the shared refs), everything under `refs/`, which is the one tree
//! walked in full — a repository has as many entries there as it has refs —
//! and the three places under `objects/` where git locks a file it rewrites
//! whole: `objects/pack/multi-pack-index.lock`, and the commit-graph chain
//! and its parts under `objects/info/` and `objects/info/commit-graphs/`,
//! which a fetch with `fetch.writeCommitGraph` takes and a later one fails on.
//! Not the pack files themselves: an interrupted pack write leaves `tmp_*`
//! files, which `git gc` reaps and no later operation trips over.

use std::path::{Path, PathBuf};

/// Every `*.lock` a cancel could have stranded, sorted, each once: the search
/// over the two directories a repository's locks live in — its own git
/// directory and, for a linked worktree, the common one it shares (the same
/// directory otherwise; `fetch` passes what gix reports for both). Never
/// fails: a directory that cannot be read contributes nothing, since this
/// runs after a cancel to add to a report, not to decide one.
pub(crate) fn stranded_locks(git_dir: &Path, common_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for directory in directories(git_dir, common_dir) {
        top_level(directory, &mut found);
        walk(&directory.join("refs"), &mut found);
        for objects in ["objects/pack", "objects/info", "objects/info/commit-graphs"] {
            top_level(&directory.join(objects), &mut found);
        }
    }
    found.sort();
    found.dedup();
    found
}

/// The distinct directories to search; one when the repository is not a
/// linked worktree.
fn directories<'a>(git_dir: &'a Path, common_dir: &'a Path) -> Vec<&'a Path> {
    if git_dir == common_dir {
        vec![git_dir]
    } else {
        vec![git_dir, common_dir]
    }
}

/// The lock files directly in `directory`, not below it.
fn top_level(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_lock(&path) && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            found.push(path);
        }
    }
}

/// Every lock file under `directory`, at any depth. Iterative, since a
/// `refs/` tree is as deep as the longest ref name has slashes.
fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
    let mut pending = vec![directory.to_owned()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(path),
                Ok(kind) if kind.is_file() && is_lock(&path) => found.push(path),
                _ => {}
            }
        }
    }
}

fn is_lock(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "lock")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A directory laid out like a git directory, removed when the test ends.
    struct Layout {
        root: PathBuf,
    }

    impl Layout {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "cairn-stranded-locks-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn file(&self, relative: &str) -> PathBuf {
            let path = self.root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"").unwrap();
            path
        }

        fn dir(&self, relative: &str) -> PathBuf {
            let path = self.root.join(relative);
            std::fs::create_dir_all(&path).unwrap();
            path
        }
    }

    impl Drop for Layout {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// Caught by: skipping the top level, skipping a nested ref directory, or
    /// reporting a file that merely contains "lock".
    #[test]
    fn every_lock_file_git_could_strand_is_found_and_nothing_else() {
        let layout = Layout::new("plain");
        let git_dir = layout.dir(".git");
        let head = layout.file(".git/HEAD.lock");
        let packed = layout.file(".git/packed-refs.lock");
        let index = layout.file(".git/index.lock");
        let main = layout.file(".git/refs/remotes/origin/main.lock");
        let deep = layout.file(".git/refs/heads/feature/one/two/three.lock");
        let midx = layout.file(".git/objects/pack/multi-pack-index.lock");
        let chain = layout.file(".git/objects/info/commit-graphs/commit-graph-chain.lock");
        let graph = layout.file(".git/objects/info/commit-graph.lock");
        // Not locks: the files git keeps, a pack in progress, and look-alikes.
        layout.file(".git/HEAD");
        layout.file(".git/packed-refs");
        layout.file(".git/refs/remotes/origin/main");
        layout.file(".git/refs/heads/lock");
        layout.file(".git/refs/heads/lockfile");
        layout.file(".git/objects/pack/tmp_pack_abc");
        layout.file(".git/objects/pack/pack-abc.idx");
        layout.file(".git/objects/ab/cdef.lock");
        layout.file(".git/logs/refs/heads/main.lock");
        layout.dir(".git/refs/heads/dir.lock");

        let mut expected = vec![head, packed, index, main, deep, midx, chain, graph];
        expected.sort();
        assert_eq!(stranded_locks(&git_dir, &git_dir), expected);
    }

    /// A linked worktree keeps `HEAD` and the index of its own and shares the
    /// refs: both directories are searched, and the shared one once.
    #[test]
    fn a_linked_worktree_is_searched_in_both_of_its_directories() {
        let layout = Layout::new("worktree");
        let common = layout.dir(".git");
        let git_dir = layout.dir(".git/worktrees/feature");
        let own_head = layout.file(".git/worktrees/feature/HEAD.lock");
        let own_ref = layout.file(".git/worktrees/feature/refs/bisect/bad.lock");
        let shared = layout.file(".git/refs/remotes/origin/main.lock");
        let packed = layout.file(".git/packed-refs.lock");

        let mut expected = vec![own_head, own_ref, shared.clone(), packed.clone()];
        expected.sort();
        assert_eq!(stranded_locks(&git_dir, &common), expected);
        let mut shared_only = vec![shared, packed];
        shared_only.sort();
        assert_eq!(
            stranded_locks(&common, &common),
            shared_only,
            "the main worktree does not report a linked worktree's private locks"
        );
    }

    #[test]
    fn a_clean_repository_reports_nothing_and_a_missing_directory_is_not_an_error() {
        let layout = Layout::new("clean");
        let git_dir = layout.dir(".git");
        layout.file(".git/HEAD");
        layout.file(".git/refs/heads/main");
        assert_eq!(stranded_locks(&git_dir, &git_dir), Vec::<PathBuf>::new());
        let gone = layout.root.join("nowhere");
        assert_eq!(stranded_locks(&gone, &gone), Vec::<PathBuf>::new());
    }
}
