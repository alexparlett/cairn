//! Reads that `git` answers: one named function per read, and nothing else.
//!
//! Every repository read goes through gitoxide, in process, unless gix's
//! answer differs from git's (D1, `docs/design/engine.md`). Where showing what
//! git shows means asking git — the changes query, whose rename and copy
//! detection is where the two disagree — the read is a function here, built
//! with [`crate::ops::GitBinary`]'s read builder, and the runner is reached from
//! nowhere else but `ops/`. Empty until `diff-engine` adds its first function;
//! the module exists so that the guard suite can name it as one of the runner's
//! two callers.
//!
//! # What a read may run
//!
//! **Query plumbing, or `git status`, and nothing else.** A read runs with
//! `GIT_OPTIONAL_LOCKS=0`, so that looking at a repository never refreshes its
//! index behind the user's back or holds `index.lock` while their own
//! `git commit` needs it. But only `status` honours that variable: porcelain
//! `git diff` against the working tree refreshes the index whenever
//! `diff.autoRefreshIndex` finds stat-only changes, and `git describe --dirty`
//! refreshes it too, each taking `index.lock` to write it whatever the
//! variable says. The diff plumbing — `diff-tree`, `diff-index`,
//! `diff-files` — never writes the index (evidence:
//! `docs/research/process-manager/platform-and-git-behaviour.md`, C3). So a
//! read that wants a diff runs `diff-tree` or `diff-index`, never `diff`; and
//! a refresh, if one is ever wanted, is a write, built in `ops/`.
//!
//! Plumbing is not write-free for every flag. With `diff.<driver>.cachetextconv`
//! set, `--textconv` writes a ref (`refs/notes/textconv/<driver>`) and objects
//! whatever `GIT_OPTIONAL_LOCKS` says, and `--ext-diff` runs a program the
//! user configured. So a read never passes `--textconv` or `--ext-diff`, and
//! the raw and patch forms without them write nothing (reproduced with
//! git 2.56: `diff-tree --raw` and `diff-tree -p` leave the refs alone,
//! `diff-tree --textconv -p` and the porcelain `log -p` create the notes ref). Any other
//! query plumbing a read adds (`ls-files`, `rev-parse`, `cat-file`, ...) brings
//! its own evidence that it writes nothing, because C3 does not cover it.
//!
//! Plumbing is not the same as a query: `update-ref`, `update-index` (its
//! `--refresh` included), `read-tree`, `write-tree`, `hash-object -w` and
//! `commit-tree` are plumbing writers, and each is a write, built in `ops/`.
//!
//! **A read never lazily fetches — on git 2.44 or later.** In a partial
//! clone, asking for an object only the promisor remote holds fetches it,
//! writing a pack and reaching the network. A read runs with
//! `GIT_NO_LAZY_FETCH=1`, so git answers that the object is missing instead
//! (decided by the user on 2026-10-02, keeping the 2.30 floor). Git older than
//! 2.44 ignores the variable: there, a read in a partial clone may still
//! lazy-fetch. Carrying no askpass token, it fails closed only where the
//! promisor needs a prompt; one a configured credential helper or the ssh
//! agent answers fetches. That is a constraint each read here designs around — a read that
//! may touch an object a partial clone lacks must treat both answers, the
//! object missing and the fetch that failed, as what they are — not one the
//! environment removes. Pinned against real git by
//! `a_read_in_a_partial_clone_does_not_fetch_a_missing_object`.
//!
//! A read can carry no askpass token — its invocation has nowhere to hold one —
//! so a read that reached a credential prompt fails closed rather than asking
//! the user. It parses only output `git` does not translate (`-z` records,
//! `--raw`, porcelain v2), and classifies a failure by exit status and the
//! repository's state, never by matching stderr, which is prose in the user's
//! language.
//!
//! Who pins what: the read's environment is built in `process/` and spelled
//! out by its tests; that only this module and `ops/` name the runner is
//! `the_runner_is_named_only_by_ops_and_reads`; that a read cannot build a
//! write is the compiler's, because only `ops/` can construct the
//! `WriteAuthority` a write needs. That each function here runs query
//! plumbing or `status` is a review obligation: a token scan cannot tell `diff-tree` from
//! `diff` in an argument list built at run time.

/// The path `diff-engine` takes, proved against what shipped: a read built here
/// in `reads/` from a `GitBinary` copy the diff thread holds, run on that
/// thread, stopped by an epoch, answering `-z` records. `changes` is the shape
/// of that packet's first function (`git diff-tree -r -M -z --raw`); it is
/// declared inside this test module because nothing in the product calls a read
/// yet.
#[cfg(test)]
mod diff_engine_path_forward {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use crate::ops::{Askpass, GitBinary, GitEnvironment};
    use crate::{Cancel, Error, Repository, SharedRepository};

    /// What the worker's epoch is: a counter the UI advances, and a query that
    /// carries the value it started under and is superseded once they differ.
    struct Epoch {
        current: Arc<AtomicU64>,
        started_under: u64,
    }

    impl Cancel for Epoch {
        fn is_cancelled(&self) -> bool {
            self.current.load(Ordering::Acquire) != self.started_under
        }
    }

    /// The read: one named function, plumbing only, `-z` records handed over as
    /// they arrive. A fresh `GitBinary` copy and a thread-local `Repository`
    /// are all it needs.
    fn changes(
        git: &GitBinary,
        repo: &Repository,
        from: &str,
        to: &str,
        cancel: &impl Cancel,
        mut record: impl FnMut(&[u8]),
    ) -> Result<(), Error> {
        git.read_invocation()
            .in_repository(repo)
            .args([
                "diff-tree",
                "-r",
                "-M",
                "-z",
                "--raw",
                "--end-of-options",
                from,
                to,
            ])
            .start()?
            .records(cancel, &mut record, |_| {})
            .map(|_| ())
    }

    fn git_in(
        program: &Path,
        directory: &Path,
        home: &Path,
        args: &[&str],
        input: Option<&str>,
    ) -> String {
        use std::io::Write;
        let mut child = std::process::Command::new(program)
            .arg("-C")
            .arg(directory)
            .args(args)
            .env("HOME", home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "A U Thor")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "A U Thor")
            .env("GIT_COMMITTER_EMAIL", "author@example.com")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        let mut stdin = child.stdin.take().unwrap_or_else(|| panic!("piped stdin"));
        let bytes = input.unwrap_or_default().to_owned();
        let feeder = std::thread::spawn(move || {
            let _ = stdin.write_all(bytes.as_bytes());
        });
        let output = child
            .wait_with_output()
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        let _ = feeder.join();
        assert!(output.status.success(), "git {args:?} failed");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    struct Fixture {
        program: PathBuf,
        root: PathBuf,
        repo: PathBuf,
        home: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "cairn-reads-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&root);
            let (repo, home) = (root.join("repo"), root.join("home"));
            std::fs::create_dir_all(&home).unwrap_or_else(|error| panic!("{error}"));
            std::fs::create_dir_all(&repo).unwrap_or_else(|error| panic!("{error}"));
            // The `git` Cairn itself would find, so the fixture is built by the
            // binary the read under test runs.
            let program = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
                .unwrap_or_else(|error| panic!("{error}"))
                .path()
                .to_owned();
            git_in(&program, &repo, &home, &["init", "-q", "."], None);
            Self {
                program,
                root,
                repo,
                home,
            }
        }

        fn git(&self, args: &[&str], input: Option<&str>) -> String {
            git_in(&self.program, &self.repo, &self.home, args, input)
        }

        /// The discovery the application does once, on the machine's `git`, with
        /// a home that holds no configuration.
        fn binary(&self) -> GitBinary {
            let home = self.home.clone().into_os_string();
            let environment = GitEnvironment::new(
                |name| match name {
                    "PATH" => std::env::var_os("PATH"),
                    "HOME" => Some(home.clone()),
                    _ => None,
                },
                &Askpass::new("/nonexistent/cairn-askpass", None),
            );
            GitBinary::discover_with(environment).unwrap_or_else(|error| panic!("{error}"))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// A rename with an edit, between two commits: the answer `gix` and `git`
    /// can differ on, and the reason a read here runs `git` at all.
    #[test]
    fn a_read_built_here_and_run_on_a_threads_own_copy_answers_z_records() {
        let fixture = Fixture::new("rename");
        let body: String = (0..40).map(|n| format!("line {n}\n")).collect();
        std::fs::write(fixture.repo.join("old name"), &body).unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&["add", "."], None);
        fixture.git(&["commit", "-q", "-m", "one"], None);
        fixture.git(&["mv", "old name", "new\tname"], None);
        std::fs::write(
            fixture.repo.join("new\tname"),
            body.replace("line 7\n", "line 7!\n"),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&["add", "."], None);
        fixture.git(&["commit", "-q", "-m", "two"], None);

        let shared = SharedRepository::discover(&fixture.repo).unwrap_or_else(|e| panic!("{e}"));
        let git = fixture.binary();
        let epochs = Arc::new(AtomicU64::new(0));
        let query = Epoch {
            current: Arc::clone(&epochs),
            started_under: 0,
        };
        // The diff thread: its own `GitBinary` copy, its own thread-local handle.
        let thread_git = git.clone();
        let answered = std::thread::spawn(move || {
            let repo = shared.to_worker();
            let mut records = Vec::new();
            changes(&thread_git, &repo, "HEAD~1", "HEAD", &query, |record| {
                records.push(String::from_utf8_lossy(record).into_owned());
            })
            .map(|()| (records, shared.command_log()))
        })
        .join()
        .unwrap_or_else(|_| panic!("the diff thread panicked"))
        .unwrap_or_else(|error| panic!("{error}"));

        let (records, log) = answered;
        assert_eq!(records.len(), 3, "{records:?}");
        assert!(
            records[0].starts_with(':') && records[0].contains(" R"),
            "not a rename: {records:?}"
        );
        assert_eq!(records[1], "old name", "a space in a path survives -z");
        assert_eq!(records[2], "new\tname", "a tab in a path survives -z");
        assert_eq!(log.len(), 1, "the read is booked once: {log:?}");
        assert_eq!(
            log[0].arguments.first().map(String::as_str),
            Some("diff-tree")
        );
        assert!(!log[0].cancelled);
    }

    /// The same read superseded: the epoch moves while a slow consumer holds
    /// the answer back, `git` is stopped rather than waited for, the caller
    /// hears a cancelled read, and nothing is left running.
    #[test]
    fn a_read_superseded_by_a_newer_epoch_stops_git_and_reports_a_cancelled_read() {
        const ENTRIES: usize = 60_000;
        let fixture = Fixture::new("superseded");
        let blob = |content: &str| fixture.git(&["hash-object", "-w", "--stdin"], Some(content));
        let (before, after) = (blob("before\n"), blob("after\n"));
        let tree = |blob: &str| {
            let listing: String = (0..ENTRIES)
                .map(|n| format!("100644 blob {blob}\tf-{n:06}\n"))
                .collect();
            fixture.git(&["mktree"], Some(&listing))
        };
        let (from, to) = (tree(&before), tree(&after));

        let shared = SharedRepository::discover(&fixture.repo).unwrap_or_else(|e| panic!("{e}"));
        let git = fixture.binary();
        let epochs = Arc::new(AtomicU64::new(0));
        let query = Epoch {
            current: Arc::clone(&epochs),
            started_under: 0,
        };
        let repo = shared.to_worker();
        let mut seen = 0_usize;
        let started = Instant::now();
        let outcome = changes(&git, &repo, &from, &to, &query, |_| {
            seen += 1;
            if seen == 1 {
                // A newer query supersedes this one while the consumer is slow:
                // `git` is blocked writing into a pipe nobody is emptying.
                epochs.fetch_add(1, Ordering::Release);
                std::thread::sleep(Duration::from_millis(300));
            }
        });
        let elapsed = started.elapsed();

        assert!(
            matches!(outcome, Err(Error::GitReadCancelled { .. })),
            "expected a cancelled read, got {outcome:?}"
        );
        assert!(seen < ENTRIES, "the whole answer was read ({seen} records)");
        assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
        let log = shared.command_log();
        assert_eq!(log.len(), 1, "{log:?}");
        assert!(log[0].cancelled, "{log:?}");
        assert_eq!(
            shared.end_invocations(Duration::from_secs(1)),
            0,
            "left running"
        );
    }
}
