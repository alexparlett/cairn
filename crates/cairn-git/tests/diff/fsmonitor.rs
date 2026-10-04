//! `core.fsmonitor=true`: git's own fsmonitor daemon, which a read starts as the user's own
//! `git diff` does (the user's decision of 2026-10-04, accepted as parity).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cairn_git::{CancelSignal, ChangesRequest, ContentOptions, SharedRepository, WorkingTreeDiff};
use cairn_model::RepoPath;

use super::repositories::Repo;
use super::{ok, since};

/// The names git gives the daemon's files at the top of the git directory: its socket,
/// `fsmonitor--daemon.ipc`, and its cookie directory, `fsmonitor--daemon/`.
const DAEMON_FILES: &str = "fsmonitor--daemon";

/// What is at each path under `dir`: a regular file's bytes, or a marker for anything else
/// (the daemon's socket is not a file `read` can open).
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(next) = pending.pop() {
        for entry in ok(std::fs::read_dir(&next), "a directory reads") {
            let entry = ok(entry, "an entry");
            let path = entry.path();
            let kind = ok(entry.file_type(), "an entry's type");
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                found.insert(path.clone(), Some(ok(std::fs::read(&path), "a file reads")));
            } else {
                found.insert(path, None);
            }
        }
    }
    found
}

/// Stops the daemon a read started, however the test ends: it runs in a session of its
/// own and would outlive the test, the fixture's directory and the test binary.
struct StopsTheDaemon<'a>(&'a Repo);

impl Drop for StopsTheDaemon<'_> {
    fn drop(&mut self) {
        let _ = self.0.run(&["fsmonitor--daemon", "stop"], &[], None);
    }
}

/// Why this git or this machine cannot run the builtin daemon, if it cannot: a git before
/// 2.36, one built without it, or a directory it refuses to watch (asked in a throwaway
/// repository beside the fixture, so the fixture's daemon is the reads' alone).
fn no_daemon_here() -> Option<String> {
    if super::git().version() < since(36) {
        return Some(format!(
            "git {} has no builtin fsmonitor daemon on this platform",
            super::git().version()
        ));
    }
    let probe = Repo::new("fsmonitor-probe");
    if !probe
        .git(&["version", "--build-options"])
        .contains("feature: fsmonitor--daemon")
    {
        return Some("this git was built without fsmonitor--daemon".to_owned());
    }
    let _stop = StopsTheDaemon(&probe);
    match probe.try_git(&["fsmonitor--daemon", "start"], &[], None) {
        Ok(_) => None,
        Err(refusal) => Some(format!(
            "the daemon will not watch {}: {refusal}",
            probe.path().display()
        )),
    }
}

/// Under `core.fsmonitor=true` every read that looks at the index — the changes query
/// (`diff-tree --raw`), a file's content (`diff-tree -p` and `check-attr`), a staged and an
/// unstaged path (`diff-index --cached`, `diff-files`) — starts git's own fsmonitor daemon,
/// exactly as the user's `git diff` and `git status` do, and the daemon creates its socket
/// and cookie directory in the git directory. Nothing else there changes: no object, ref,
/// index or config is written. And the daemon is still running once Cairn's handle on the
/// repository is gone, because it is git's, in a session of its own, outside every process
/// Cairn starts and ends. Caught by: a read that writes the index or an object under the
/// daemon, and the daemon no longer started (the claim the docs make would then be stale).
#[test]
fn a_read_under_the_builtin_fsmonitor_writes_only_the_daemons_own_files() {
    if let Some(reason) = no_daemon_here() {
        eprintln!(
            "SKIPPED a_read_under_the_builtin_fsmonitor_writes_only_the_daemons_own_files: \
             {reason}"
        );
        return;
    }
    let repo = Repo::new("fsmonitor-daemon");
    let _stop = StopsTheDaemon(&repo);
    repo.write(".gitattributes", b"*.txt diff=named\n");
    repo.write("unstaged.txt", b"one\ntwo\nthree\n");
    repo.write("staged.txt", b"one\ntwo\nthree\n");
    repo.commit("seed");
    repo.write("unstaged.txt", b"one\nTWO\nthree\n");
    repo.write("staged.txt", b"one\nTWO\nthree\n");
    let head = repo.commit("edits");
    repo.write("staged.txt", b"one\nTWO\nTHREE\n");
    repo.git(&["add", "staged.txt"]);
    repo.write("unstaged.txt", b"ONE\nTWO\nthree\n");
    repo.write("untracked.txt", b"new\n");
    // A driver naming its own algorithm, so the content query runs `check-attr` too.
    repo.config("diff.named.algorithm", "patience");
    repo.config("core.fsmonitor", "true");
    let git_dir = repo.path().join(".git");
    let before = snapshot(&git_dir);

    let shared = ok(SharedRepository::discover(repo.path()), "the fixture opens");
    let engine = shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let request = ChangesRequest::commit(head);
    let set = ok(
        session.changes(super::git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    assert_eq!(set.files.len(), 2, "{set:?}");
    for file in &set.files {
        let _ = ok(
            session.file_diff(
                super::git(),
                &request,
                file,
                &ContentOptions::default(),
                &CancelSignal::new(),
            ),
            "a file diff",
        );
    }
    drop(session);
    for (path, which) in [
        ("staged.txt", WorkingTreeDiff::Staged),
        ("unstaged.txt", WorkingTreeDiff::Unstaged),
        ("untracked.txt", WorkingTreeDiff::Untracked),
    ] {
        let answer = ok(
            engine.working_tree_diff(
                super::git(),
                &RepoPath::new(path),
                which,
                &ContentOptions::default(),
                &CancelSignal::new(),
            ),
            &format!("the {which:?} diff of {path}"),
        );
        assert!(answer.is_some(), "no {which:?} diff of {path}");
    }
    drop(engine);
    drop(shared);

    assert!(
        git_dir.join("fsmonitor--daemon.ipc").exists() && git_dir.join(DAEMON_FILES).is_dir(),
        "no read started git's fsmonitor daemon, which the user's own git diff starts"
    );
    assert!(
        repo.try_git(&["fsmonitor--daemon", "status"], &[], None)
            .is_ok(),
        "the daemon ended with Cairn's handle; it is git's, and outlives the read"
    );
    let daemons = |path: &PathBuf| {
        path.strip_prefix(&git_dir)
            .ok()
            .and_then(|inside| inside.components().next())
            .is_some_and(|top| top.as_os_str().to_string_lossy().starts_with(DAEMON_FILES))
    };
    let after = snapshot(&git_dir);
    let changed: Vec<&PathBuf> = before
        .keys()
        .chain(after.keys())
        .filter(|path| !daemons(path))
        .filter(|path| before.get(*path) != after.get(*path))
        .collect();
    assert!(
        changed.is_empty(),
        "a read changed the git directory beyond the daemon's own files: {changed:?}"
    );
    for kept in ["index", "config", "HEAD"] {
        assert!(
            before.contains_key(&git_dir.join(kept)),
            "the snapshot did not see {kept}, so its comparison decides nothing"
        );
    }
}
