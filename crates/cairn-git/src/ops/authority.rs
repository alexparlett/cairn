//! The seal on a write.
//!
//! A `git` invocation is built as a read or a write (`crate::process`), and a
//! write can only be built from a [`WriteAuthority`]. The type has a private
//! field and one constructor, visible to `ops/` and its children alone, so the
//! compiler refuses a write anywhere else in the crate — in `reads/`, in
//! `history/`, in `process/` itself — and from outside the crate the type
//! cannot even be named (the `compile_fail` pins in the [`super`] module
//! docs). It is a token rather than a visibility on the builder because
//! `process/` cannot write `pub(in crate::ops)` on an item of its own: a
//! restricted visibility must name an ancestor of the item.
//!
//! The guard `the_runner_is_named_only_by_ops_and_reads` is the twin against
//! erosion: it refuses the constructor, a literal or an `impl` block for the
//! type anywhere outside `ops/`, and it pins the constructor's visibility and
//! the field's privacy here, so widening either fails the gate rather than
//! compiling quietly.

/// Permission to build a write invocation, which only `ops/` can construct.
///
/// Every operation takes a fresh one for each process it starts and hands it
/// to [`crate::ops::GitBinary`]'s write builder, which keeps it for the life of
/// the invocation. Not `Clone`, not `Copy` and not `Default`: there is one way
/// to get one, and it is here.
#[derive(Debug)]
pub(crate) struct WriteAuthority {
    _sealed: (),
}

impl WriteAuthority {
    /// For an operation in `ops/` about to start a write; nothing else can call it.
    pub(in crate::ops) fn new() -> Self {
        Self { _sealed: () }
    }
}

/// Writes against a stub `git` that prints what it was given, and against real
/// `git`. Here rather than in `process/` because a write needs the authority,
/// and nothing outside `ops/` can construct one — tests included.
#[cfg(all(test, unix))]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant, SystemTime};

    use cairn_model::AskpassToken;

    use super::WriteAuthority;
    use crate::ops::{Askpass, GitBinary, GitEnvironment};
    use crate::process::stub_git::{StubGit, discover_retrying, printed_environment};
    use crate::{CancelSignal, Error, Repository};

    /// Answers `--version`, then prints its environment for anything else.
    fn printing_stub() -> StubGit {
        StubGit::with_git(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
             exec /usr/bin/env",
        )
    }

    /// The write environment of a stub on `path` with no inherited variable but
    /// `PATH`, spelled out variable by variable, plus `extra`.
    fn write_environment(path: &str, extra: &[(&str, &str)]) -> BTreeMap<String, String> {
        [
            ("GIT_ASKPASS", StubGit::HELPER),
            ("GIT_EDITOR", "false"),
            ("GIT_SEQUENCE_EDITOR", "false"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("PATH", path),
            ("SSH_ASKPASS", StubGit::HELPER),
            ("SSH_ASKPASS_REQUIRE", "force"),
        ]
        .iter()
        .chain(extra)
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
    }

    fn path_of(environment: &GitEnvironment) -> String {
        environment
            .get("PATH")
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| panic!("the stub environment has no PATH"))
    }

    /// PRD G3 end to end, for a write: the base with the editor pinned, no
    /// `GIT_OPTIONAL_LOCKS`, no token when none was given — exactly that, and
    /// nothing from this process.
    #[test]
    fn a_write_sees_exactly_the_write_environment_and_nothing_inherited() {
        let stub = printing_stub();
        let environment = stub.environment();
        let path = path_of(&environment);
        let git = discover_retrying(environment).unwrap();
        let output = git
            .write_invocation(WriteAuthority::new())
            .arg("print-environment")
            .run()
            .unwrap();
        assert_eq!(
            printed_environment(&output.stdout_text()),
            write_environment(&path, &[])
        );
    }

    /// The token reaches the child only on a write that was given one, and it
    /// is the only thing the authorisation adds.
    #[test]
    fn an_authorised_write_carries_its_token_and_only_that_one() {
        let stub = printing_stub();
        let environment = stub.environment();
        let path = path_of(&environment);
        let git = discover_retrying(environment).unwrap();
        let token = AskpassToken::new(format!("token-{}", std::process::id()));
        let output = git
            .write_invocation(WriteAuthority::new())
            .arg("print-environment")
            .authorized_by(&token)
            .run()
            .unwrap();
        assert_eq!(
            printed_environment(&output.stdout_text()),
            write_environment(&path, &[("CAIRN_ASKPASS_TOKEN", token.as_str())])
        );
    }

    /// A directory holding a home with no configuration in it and a repository
    /// made by real `git`, through Cairn's own write invocations; removed when
    /// the test ends.
    struct Scratch {
        root: PathBuf,
        git: GitBinary,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "cairn-ops-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("home")).unwrap_or_else(|error| panic!("{error}"));
            let home = root.join("home").into_os_string();
            // The machine's `git` on this process's PATH, with a HOME that holds no
            // configuration, so a user's `commit.gpgsign` or hooks cannot decide
            // anything here.
            let environment = GitEnvironment::new(
                |name| match name {
                    "PATH" => std::env::var_os("PATH"),
                    "HOME" => Some(home.clone()),
                    _ => None,
                },
                &Askpass::new("/nonexistent/cairn-askpass", None),
            );
            let git =
                GitBinary::discover_with(environment).unwrap_or_else(|error| panic!("{error}"));
            let scratch = Self { root, git };
            scratch.write(&["init", "-q", "repo"], &scratch.root);
            for (key, value) in [
                ("user.name", "A U Thor"),
                ("user.email", "author@example.com"),
            ] {
                scratch.write(&["config", key, value], &scratch.repo());
            }
            scratch
        }

        fn repo(&self) -> PathBuf {
            self.root.join("repo")
        }

        /// Runs a write `git` in `directory` (through `-C`, so no repository need
        /// exist to run it in), panicking on failure.
        fn write(&self, args: &[&str], directory: &Path) {
            self.git
                .write_invocation(WriteAuthority::new())
                .arg("-C")
                .arg(directory)
                .args(args)
                .run()
                .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        }

        fn head(&self) -> String {
            self.git
                .read_invocation()
                .arg("-C")
                .arg(self.repo())
                .args(["rev-parse", "HEAD"])
                .run()
                .unwrap_or_else(|error| panic!("git rev-parse HEAD: {error}"))
                .stdout_text()
                .trim()
                .to_owned()
        }

        /// A script that records it ran, then outstays any test's patience: the
        /// editor a user configured, which Cairn must never wait on.
        fn hanging_editor(&self) -> (String, PathBuf) {
            let marker = self.root.join("editor-ran");
            let script = self.root.join("editor.sh");
            std::fs::write(&script, format!("touch '{}'\nsleep 5\n", marker.display()))
                .unwrap_or_else(|error| panic!("{error}"));
            // Run by `sh` rather than executed, so a parallel test's fork holding this
            // file's write descriptor cannot make it "text file busy".
            (format!("sh '{}'", script.display()), marker)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// How long a verb that would open an editor may take to fail: far less than
    /// the hanging editor's sleep, and far more than `git` needs on a loaded
    /// machine.
    const PROMPTLY: Duration = Duration::from_secs(4);

    /// PRD G5: `git commit` with no message fails promptly instead of waiting on
    /// an editor — the user's configured `core.editor` is never run, because
    /// `GIT_EDITOR=false` outranks it — and nothing is committed. Decisive
    /// because the configured editor records that it ran and then hangs past
    /// the deadline, so an environment without the pin fails both assertions.
    #[test]
    fn a_commit_without_a_message_fails_promptly_instead_of_opening_an_editor() {
        let scratch = Scratch::new("commit-editor");
        let repo = scratch.repo();
        std::fs::write(repo.join("tracked"), "one\n").unwrap();
        scratch.write(&["add", "tracked"], &repo);
        scratch.write(&["commit", "-q", "-m", "initial"], &repo);
        let before = scratch.head();
        let (editor, marker) = scratch.hanging_editor();
        scratch.write(&["config", "core.editor", &editor], &repo);
        std::fs::write(repo.join("tracked"), "two\n").unwrap();
        scratch.write(&["add", "tracked"], &repo);

        let handle = Repository::discover(&repo).unwrap();
        let started = Instant::now();
        let outcome = scratch
            .git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .arg("commit")
            .run();
        let took = started.elapsed();

        assert!(
            matches!(&outcome, Err(Error::GitFailed { arguments, .. }) if arguments == "commit"),
            "a commit with no message and no editor must fail: {outcome:?}"
        );
        assert!(took < PROMPTLY, "the commit waited {took:?} for an editor");
        assert!(
            !marker.exists(),
            "the user's core.editor ran: GIT_EDITOR did not outrank it"
        );
        assert_eq!(scratch.head(), before, "something was committed");
    }

    /// The same for the rebase todo list, whose editor is `GIT_SEQUENCE_EDITOR`,
    /// then `sequence.editor`, and only then `GIT_EDITOR` — so a user's
    /// `sequence.editor` would beat the commit editor's pin, and this is what
    /// the second pin is for.
    #[test]
    fn an_interactive_rebase_fails_promptly_instead_of_opening_the_sequence_editor() {
        let scratch = Scratch::new("sequence-editor");
        let repo = scratch.repo();
        for content in ["one\n", "two\n"] {
            std::fs::write(repo.join("tracked"), content).unwrap();
            scratch.write(&["add", "tracked"], &repo);
            scratch.write(&["commit", "-q", "-m", content.trim()], &repo);
        }
        let before = scratch.head();
        let (editor, marker) = scratch.hanging_editor();
        scratch.write(&["config", "sequence.editor", &editor], &repo);

        let handle = Repository::discover(&repo).unwrap();
        let started = Instant::now();
        let outcome = scratch
            .git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .args(["rebase", "-i", "HEAD~1"])
            .run();
        let took = started.elapsed();

        assert!(
            matches!(&outcome, Err(Error::GitFailed { .. })),
            "an interactive rebase with no sequence editor must fail: {outcome:?}"
        );
        assert!(took < PROMPTLY, "the rebase waited {took:?} for an editor");
        assert!(
            !marker.exists(),
            "the user's sequence.editor ran: GIT_SEQUENCE_EDITOR did not outrank it"
        );
        assert_eq!(scratch.head(), before, "the rebase moved HEAD");
    }

    /// PRD G4: a read of `git status` on a dirty repository whose index is stale
    /// leaves the index byte for byte as it was, because a read runs with
    /// `GIT_OPTIONAL_LOCKS=0`. Decisive because the same `status` as a write —
    /// the one thing that differs being the read profile — DOES rewrite the
    /// index of the same repository: without that, an index git had no reason
    /// to refresh would pass this whatever the environment said.
    #[test]
    fn a_status_read_leaves_a_stale_index_byte_identical() {
        let scratch = Scratch::new("status-read");
        let repo = scratch.repo();
        std::fs::write(repo.join("tracked"), "one\n").unwrap();
        std::fs::write(repo.join("edited"), "a\n").unwrap();
        scratch.write(&["add", "tracked", "edited"], &repo);
        scratch.write(&["commit", "-q", "-m", "initial"], &repo);
        // Stale: `tracked`'s content is what the index records but its stat
        // information is not, so `status` re-hashes it, finds it clean, and would
        // write the refreshed stat back. Dirty: an edit and an untracked file.
        std::fs::File::options()
            .write(true)
            .open(repo.join("tracked"))
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
        std::fs::write(repo.join("edited"), "b\n").unwrap();
        std::fs::write(repo.join("untracked"), "c\n").unwrap();
        let index = repo.join(".git").join("index");
        let before = std::fs::read(&index).unwrap();

        let handle = Repository::discover(&repo).unwrap();
        let read = scratch
            .git
            .read_invocation()
            .in_repository(&handle)
            .args(["status", "--porcelain=v2", "-z"])
            .run()
            .unwrap();
        let records: Vec<&[u8]> = read.records().collect();
        assert!(
            records.iter().any(|record| record.ends_with(b" edited"))
                && records.contains(&b"? untracked".as_slice()),
            "status did not see the dirty working tree: {records:?}"
        );
        assert!(
            std::fs::read(&index).unwrap() == before,
            "a status read rewrote the index"
        );
        assert!(
            !repo.join(".git").join("index.lock").exists(),
            "a status read left index.lock behind"
        );

        scratch
            .git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .args(["status", "--porcelain=v2", "-z"])
            .run()
            .unwrap_or_else(|error| panic!("the same status as a write: {error}"));
        assert!(
            std::fs::read(&index).unwrap() != before,
            "the same status as a write left the index alone too, so the fixture was not \
             stale and the read above decided nothing"
        );
    }

    /// The runner's criteria that are about `git` itself, with real `git`: they
    /// need a write, and only `ops/` can build one.
    ///
    /// G6, real git: a tree of 100,000 entries made by `mktree` fed on stdin, then
    /// read back by `ls-tree -z` — over 5 MiB of records — arrives whole and in
    /// order. Caught by: a record lost or split at a chunk boundary, or stdin
    /// never closed (`mktree` waits for the end of its input forever).
    #[test]
    fn a_real_read_of_over_five_mib_of_records_arrives_whole_and_in_order() {
        const ENTRIES: usize = 100_000;
        const EMPTY_BLOB: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
        let scratch = Scratch::new("big-read");
        let handle = Repository::discover(scratch.repo()).unwrap();
        let listing: String = (0..ENTRIES)
            .map(|n| format!("100644 blob {EMPTY_BLOB}\tf-{n:06}\n"))
            .collect();
        let tree = scratch
            .git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .args(["mktree", "--missing"])
            .input(listing)
            .start()
            .unwrap()
            .collect(&CancelSignal::new(), 1024, |_| {})
            .unwrap()
            .stdout_text()
            .trim()
            .to_owned();
        let mut records = Vec::with_capacity(ENTRIES);
        scratch
            .git
            .read_invocation()
            .in_repository(&handle)
            .args(["ls-tree", "-z", &tree])
            .start()
            .unwrap()
            .records(
                &CancelSignal::new(),
                |record| records.push(String::from_utf8_lossy(record).into_owned()),
                |_| {},
            )
            .unwrap();
        let bytes: usize = records.iter().map(|record| record.len() + 1).sum();
        assert!(bytes >= 5 * 1024 * 1024, "only {bytes} bytes");
        assert_eq!(records.len(), ENTRIES);
        for (n, record) in records.iter().enumerate() {
            assert_eq!(record, &format!("100644 blob {EMPTY_BLOB}\tf-{n:06}"));
        }
    }

    /// A repository with a commit and a tracked file changed since, and a
    /// `pre-commit` hook that says it is running and then sleeps: a `commit -a`
    /// holds `index.lock` while the hook runs.
    fn committing_into_a_sleeping_hook(name: &str) -> (Scratch, PathBuf) {
        let scratch = Scratch::new(name);
        let repo = scratch.repo();
        std::fs::write(repo.join("tracked"), "one\n").unwrap_or_else(|error| panic!("{error}"));
        scratch.write(&["add", "tracked"], &repo);
        scratch.write(&["commit", "-q", "-m", "initial"], &repo);
        std::fs::write(repo.join("tracked"), "two\n").unwrap_or_else(|error| panic!("{error}"));
        let marker = scratch.root.join("hook-running");
        let hook = repo.join(".git/hooks/pre-commit");
        std::fs::create_dir_all(
            hook.parent()
                .unwrap_or_else(|| panic!("a hook path has a parent")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        std::fs::write(
            &hook,
            format!("#!/bin/sh\ntouch '{}'\nexec sleep 30\n", marker.display()),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap_or_else(|error| panic!("{error}"));
        (scratch, marker)
    }

    /// Runs `commit -a` in `scratch` and cancels it through its kill handle once
    /// the hook is running, after checking `index.lock` is held then — so the
    /// test decides something. Retried when the hook could not be executed
    /// because a parallel test's fork still held the file just written ("Text
    /// file busy"), a property of the harness rather than of the runner.
    fn cancel_inside_the_hook(scratch: &Scratch, marker: &Path) -> (Result<(), Error>, Duration) {
        let handle = Repository::discover(scratch.repo()).unwrap_or_else(|error| panic!("{error}"));
        let index_lock = scratch.repo().join(".git/index.lock");
        for _ in 0..5 {
            let _ = std::fs::remove_file(marker);
            let invocation = scratch
                .git
                .write_invocation(WriteAuthority::new())
                .in_repository(&handle)
                .args(["commit", "-a", "-m", "second"])
                .start()
                .unwrap_or_else(|error| panic!("{error}"));
            let killer = invocation.kill_handle();
            let done = Arc::new(AtomicBool::new(false));
            let finished = Arc::clone(&done);
            let watched = marker.to_owned();
            let lock = index_lock.clone();
            let watching = std::thread::spawn(move || {
                let started = Instant::now();
                while !finished.load(Ordering::Acquire) && started.elapsed() < PROMPTLY * 2 {
                    if watched.exists() {
                        let held = lock.exists();
                        let at = Instant::now();
                        killer.kill();
                        return Some((held, at));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                None
            });
            let outcome = invocation.finish(&CancelSignal::new(), |_| {}, |_| {});
            done.store(true, Ordering::Release);
            let killed = watching
                .join()
                .unwrap_or_else(|_| panic!("the watching thread panicked"));
            match (&outcome, killed) {
                (Err(Error::GitFailed { stderr, .. }), None)
                    if stderr.contains("Text file busy") =>
                {
                    continue;
                }
                (_, Some((held, at))) => {
                    assert!(
                        held,
                        "index.lock was not held while the hook ran: decided nothing"
                    );
                    return (outcome.map(drop), at.elapsed());
                }
                (_, None) => panic!("the hook never ran: {outcome:?}"),
            }
        }
        panic!("the hook stayed 'text file busy' through every retry");
    }

    /// G9, real git: a `commit` cancelled while its `pre-commit` hook sleeps
    /// leaves no `index.lock` — `SIGTERM` to the group, which git cleans up on and
    /// which ends the hook too — commits nothing, and reports a cancelled write
    /// with nothing stranded. Caught by: `SIGKILL` first (the lock stays), or a
    /// signal to git alone (the hook's `sleep` holds the pipes for 30 s).
    #[test]
    fn a_commit_cancelled_inside_a_sleeping_hook_leaves_no_index_lock() {
        let (scratch, marker) = committing_into_a_sleeping_hook("cancelled-commit");
        let before = scratch.head();
        let (outcome, took) = cancel_inside_the_hook(&scratch, &marker);
        assert!(
            matches!(
                &outcome,
                Err(Error::GitCancelled { arguments, stranded_locks })
                    if arguments == "commit -a -m second" && stranded_locks.is_empty()
            ),
            "{outcome:?}"
        );
        assert!(
            took < TERMINATION_GRACE_FOR_TESTS,
            "the cancel took {took:?}: not SIGTERM"
        );
        assert!(
            !scratch.repo().join(".git/index.lock").exists(),
            "index.lock was stranded"
        );
        assert_eq!(scratch.head(), before, "the cancelled commit committed");
    }

    /// Below the runner's two seconds: a cancel that took the grace was `SIGKILL`.
    const TERMINATION_GRACE_FOR_TESTS: Duration = Duration::from_millis(1500);

    /// G11, a cancelled write lists the lock files present AFTER the reap: a
    /// stale lock left by an earlier crash is reported, and the `index.lock` git
    /// held until the signal is not, because git removed it on its way out.
    /// Caught by: searching before the reap (the index lock is listed too), or
    /// not searching.
    #[test]
    fn a_cancelled_write_lists_the_locks_present_after_the_reap() {
        let (scratch, marker) = committing_into_a_sleeping_hook("cancelled-locks");
        let stale = scratch.repo().join(".git/refs/heads/stale.lock");
        std::fs::write(&stale, "").unwrap();
        let (outcome, _) = cancel_inside_the_hook(&scratch, &marker);
        match outcome {
            Err(Error::GitCancelled { stranded_locks, .. }) => {
                let stale = std::fs::canonicalize(&stale).unwrap();
                let found: Vec<PathBuf> = stranded_locks
                    .iter()
                    .map(|path| std::fs::canonicalize(path).unwrap())
                    .collect();
                assert_eq!(found, [stale]);
            }
            other => panic!("expected a cancelled write, got {other:?}"),
        }
    }

    /// G11, the ordering pinned: a write whose process takes a moment to remove its
    /// lock on `SIGTERM` — as git does, but slowly enough to see — has its locks
    /// listed only after it is reaped, so the lock it removed on its way out is
    /// not reported and the stale one is. A stub `git`, run as a write in a real
    /// repository, makes `index.lock` and removes it 300 ms into its `SIGTERM`
    /// trap. Caught by: listing the locks when the cancel is asked for, or at any
    /// point before the reap.
    #[test]
    fn a_cancelled_write_lists_its_locks_only_once_it_is_reaped() {
        let scratch = Scratch::new("locks-after-reap");
        let repo = scratch.repo();
        let stale = repo.join(".git/refs/heads/stale.lock");
        std::fs::write(&stale, "").unwrap();
        let stub = StubGit::with_git(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
             PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
             : > .git/index.lock; \
             trap 'sleep 0.3; rm -f .git/index.lock; exit 143' TERM; \
             sleep 30 & echo hanging >&2; wait",
        );
        let git = discover_retrying(stub.environment()).unwrap();
        let handle = Repository::discover(&repo).unwrap();
        let invocation = git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .arg("commit")
            .start()
            .unwrap();
        let killer = invocation.kill_handle();
        let outcome = invocation.finish(
            &CancelSignal::new(),
            |_| {},
            |line| {
                if line == "hanging" {
                    killer.kill();
                }
            },
        );
        match outcome {
            Err(Error::GitCancelled { stranded_locks, .. }) => {
                let found: Vec<PathBuf> = stranded_locks
                    .iter()
                    .map(|path| std::fs::canonicalize(path).unwrap())
                    .collect();
                assert_eq!(
                    found,
                    [std::fs::canonicalize(&stale).unwrap()],
                    "the locks were listed before the process that held index.lock was gone"
                );
            }
            other => panic!("expected a cancelled write, got {other:?}"),
        }
        assert!(!repo.join(".git/index.lock").exists());
    }

    /// R5.2 for the writes the runner ends itself: a crossed ceiling and a pipe
    /// thread that could not start each list the lock files present after the
    /// reap, as a cancel does — here a stale one planted beforehand. Caught by:
    /// either error built without the write's lock search.
    #[test]
    fn a_write_the_runner_ends_lists_the_locks_present() {
        let scratch = Scratch::new("runner-ended-write");
        let repo = scratch.repo();
        let stale = repo.join(".git/refs/heads/stale.lock");
        std::fs::write(&stale, "").unwrap();
        let stale = std::fs::canonicalize(&stale).unwrap();
        let stub = StubGit::with_git(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
             PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
             sleep 30 & head -c 4096 /dev/zero; wait",
        );
        let git = discover_retrying(stub.environment()).unwrap();
        let handle = Repository::discover(&repo).unwrap();
        let listed = |locks: &[PathBuf]| -> Vec<PathBuf> {
            locks
                .iter()
                .map(|path| std::fs::canonicalize(path).unwrap())
                .collect()
        };

        let over = git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .arg("commit")
            .start()
            .unwrap()
            .collect(&CancelSignal::new(), 1024, |_| {});
        match over {
            Err(Error::GitOutputTooLarge { stranded_locks, .. }) => {
                assert_eq!(listed(&stranded_locks), std::slice::from_ref(&stale));
            }
            other => panic!("expected the ceiling error, got {other:?}"),
        }

        let unwatched = git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .arg("commit")
            .start_without_threads();
        match unwatched {
            Err(Error::GitUnwatched { stranded_locks, .. }) => {
                assert_eq!(listed(&stranded_locks), [stale]);
            }
            other => panic!("expected the thread failure, got {other:?}"),
        }
    }

    /// G11, a failed write names the `index.lock` it failed on — another git, or
    /// a stale lock — and nothing removes it. Caught by: a write failure that
    /// reports no locks.
    #[test]
    fn a_failed_write_names_a_present_index_lock() {
        let scratch = Scratch::new("failed-write");
        let repo = scratch.repo();
        std::fs::write(repo.join("tracked"), "one\n").unwrap();
        let index_lock = repo.join(".git/index.lock");
        std::fs::write(&index_lock, "").unwrap();
        let handle = Repository::discover(&repo).unwrap();
        let outcome = scratch
            .git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .args(["add", "tracked"])
            .start()
            .unwrap()
            .finish(&CancelSignal::new(), |_| {}, |_| {});
        match outcome {
            Err(Error::GitFailed {
                arguments,
                status,
                stderr,
                present_locks,
            }) => {
                assert_eq!(arguments, "add tracked");
                assert_eq!(status.code(), Some(128));
                assert!(stderr.contains("index.lock"), "{stderr}");
                let found: Vec<PathBuf> = present_locks
                    .iter()
                    .map(|path| std::fs::canonicalize(path).unwrap())
                    .collect();
                assert_eq!(found, [std::fs::canonicalize(&index_lock).unwrap()]);
            }
            other => panic!("expected the failure, got {other:?}"),
        }
        assert!(
            index_lock.exists(),
            "the lock was removed; nothing may remove a lock"
        );
    }

    /// G11, a read with real git: its failure carries the arguments, the status and
    /// git's stderr, and no locks; cancelled, it reports a read's cancellation and
    /// nothing else. The long read is a shell alias, so it runs as long as the test
    /// needs. Caught by: a read cancelled as a write (lock files searched), or a
    /// superseded read that runs to its end.
    #[test]
    fn a_real_read_fails_with_its_diagnostic_and_cancels_as_a_read() {
        let scratch = Scratch::new("read-outcomes");
        let handle = Repository::discover(scratch.repo()).unwrap();
        let failed = scratch
            .git
            .read_invocation()
            .in_repository(&handle)
            .args([
                "rev-parse",
                "--verify",
                "--end-of-options",
                "refs/heads/absent",
            ])
            .start()
            .unwrap()
            .collect(&CancelSignal::new(), 1024, |_| {});
        match failed {
            Err(Error::GitFailed {
                arguments,
                status,
                stderr,
                present_locks,
            }) => {
                assert_eq!(
                    arguments,
                    "rev-parse --verify --end-of-options refs/heads/absent"
                );
                assert_eq!(status.code(), Some(128));
                assert!(stderr.contains("fatal"), "{stderr}");
                assert!(
                    present_locks.is_empty(),
                    "a read named locks: {present_locks:?}"
                );
            }
            other => panic!("expected the failure, got {other:?}"),
        }

        let superseded = CancelSignal::new();
        let invocation = scratch
            .git
            .read_invocation()
            .in_repository(&handle)
            .args(["-c", "alias.linger=!sleep 30", "linger"])
            .start()
            .unwrap();
        superseded.cancel();
        let started = Instant::now();
        let outcome = invocation.finish(&superseded, |_| {}, |_| {});
        assert!(
            matches!(&outcome, Err(Error::GitReadCancelled { arguments }) if arguments == "-c alias.linger=!sleep 30 linger"),
            "{outcome:?}"
        );
        assert!(
            started.elapsed() < TERMINATION_GRACE_FOR_TESTS,
            "{:?}",
            started.elapsed()
        );
    }
}
