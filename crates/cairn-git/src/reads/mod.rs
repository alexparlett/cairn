//! Reads that `git` answers: one named function per read, and nothing else.
//!
//! Every repository read goes through gitoxide, in process, unless gix's
//! answer differs from git's (D1, `docs/design/engine.md`). Where showing what
//! git shows means asking git — the changes query, whose rename and copy
//! detection is where the two disagree — the read is a function here, built
//! with [`crate::ops::GitBinary`]'s read builder, and the runner is reached from
//! nowhere else but `ops/`. Seven functions today, five for `crate::diff`:
//! [`changes`], `git diff-tree --raw` for the changes query (`diff-engine`,
//! decision E); [`patches`], `git diff-tree -p` for the content query's
//! changed ranges and function context, which gix's line diff placed
//! differently from git's (the content-parity decision of 2026-10-03,
//! `docs/research/diff-engine/content-parity-spike.md`);
//! [`diff_attributes`], `git check-attr diff`, which says whether a path's
//! diff driver names an algorithm of its own; and [`working_tree_patch`], one
//! path's staged, unstaged or untracked diff — `git diff-index --cached`,
//! `git diff-files`, `git diff --no-index` — which reads the working tree
//! through git, so its side is git's form of the file (`diff-engine` phase 03),
//! with [`staged_pairing`] beside it, `git diff-index --cached --raw` over the
//! whole index, the rename or copy the user's `git diff --cached` pairs a path
//! into, since plumbing reads no `diff.renames` and a one-path pathspec pairs
//! nothing (`staging-and-commit` R2.6);
//! and one for fetch's refspec check, [`fetch_settings`], `git config` in query
//! form, what a fetch of a remote will read, because the check must decide on
//! exactly what the fetch's own git reads and gix's reading of a linked
//! worktree's `includeIf`, of the system file and of trust is not git's (the
//! user's decision of 2026-10-04); and one for the working tree's status,
//! [`status()`], `git status --porcelain=v2 -z`, because gix's status differs from
//! git's wherever status is hard — staged renames past its limit, conflicted paths,
//! sparse checkouts, a lying fsmonitor hook — and starts clean filters outside
//! `process/` (the refs-and-status packet's L1); and one for a stash's changes,
//! [`stash_changes`], `git stash show --raw`, because with `stash.showIncludeUntracked` set
//! git pairs a stash's untracked files with its tracked changes in one diff, which no
//! plumbing can ask without writing a tree (refs-and-status R6.2).
//!
//! # What a read may run
//!
//! **Query plumbing, or `git status`, and three named porcelain exceptions.**
//!
//! **The third, `git stash show --raw -z --no-abbrev --no-color --no-ext-diff
//! --no-textconv --no-relative --end-of-options <stash commit>`** (accepted by the user on
//! 2026-10-07), built only in [`stash_changes`], whose module docs carry its evidence: it
//! takes no lock, reads no index and runs no program in raw form, and
//! `a_stash_read_writes_nothing` holds the git directory byte-identical after it; git
//! reads `stash.showIncludeUntracked` itself, so git 2.30 and 2.31, which do not know it,
//! list what the user's own `git stash show` lists there.
//!
//! **The second, `git config --includes --null` with `--type=bool --get <key>`
//! or `--get-all <key>`** (accepted by the user on 2026-10-04): query form
//! only, never a setter, built only in [`fetch_settings`], whose module docs
//! carry its evidence — it takes no lock, reads no index and runs no program,
//! and `the_refspec_checks_reads_write_nothing` holds the git directory
//! byte-identical after it; the flags are all in git 2.18 and later, and git
//! 2.56 takes the form without a word.
//!
//! **The first, `git diff --no-index -- /dev/null <path>`, for an untracked
//! file, `<path>` work-tree-relative and given as `./-` when it is `-`**
//! (accepted by the user on 2026-10-03; [`working_tree::work_tree_relative`] refuses an empty or
//! absolute path, or one with a `.` or `..` component, before git runs). Why: an untracked file's git form — after the clean
//! filter driver and the line-ending conversion its attributes name — has no
//! plumbing that prints it. `diff-files` and `diff-index` list only what the
//! index holds, and putting the path in an index to ask them is a write; `git
//! diff --no-index` applies both conversions (a CRLF file under `text=auto`
//! reads LF, a filtered file reads in its filter's form), so reading the file's
//! own bytes would show what the user's git does not. Evidence that it writes
//! nothing: it reads no index, so it has none to refresh, and it starts no
//! fsmonitor daemon (reproduced with git 2.56.0 under `core.fsmonitor=true`); a
//! snapshot of every file under the git directory is byte-identical after it on
//! git 2.30.9, 2.32.7 and 2.56.0, with a clean filter, CRLF files and submodules
//! present; and
//! `a_working_tree_query_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor`,
//! `a_text_auto_file_with_crlf_endings_is_no_change_and_an_edit_is_one_line`,
//! `a_clean_filter_drivers_form_is_what_is_diffed_and_it_runs_under_git` and
//! `an_untracked_file_is_what_git_diff_no_index_shows` hold the git directory
//! byte-identical after each such read. What it runs is the clean filter and
//! nothing else (`--no-ext-diff`, `--no-textconv`; the first test above). Being
//! porcelain, it reads presentation settings plumbing never does — path prefixes
//! and quoting, hunk joining and order, a relative path — so each is set back to
//! git's default on that invocation (`working_tree::NO_INDEX_PRESENTATION`),
//! while the settings that decide git's form of the file stay the user's
//! (`an_untracked_answer_is_the_same_under_hostile_presentation_settings`). It
//! is the only porcelain `diff` a read runs, only in that mode, and only from
//! [`working_tree_patch`]. A read runs with
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
//! **The one program a read may run is the repository's `core.fsmonitor`**, in
//! either of its forms. In a repository with a working tree, `diff-tree` (raw
//! and patch alike), `check-attr`, `diff-files` and `diff-index --cached` read
//! the index, and as they do git consults the fsmonitor exactly as the user's
//! own `git diff` and `git status` do; nothing a read passes turns it off (only
//! `-c core.fsmonitor=false` would, which would make Cairn's read differ from
//! the user's). The user accepted both forms as the parity they are (the hook
//! on 2026-10-03, QA round 3; the daemon on 2026-10-04):
//!
//! - **A hook**, `core.fsmonitor` naming a program: git runs it as a child of
//!   the read's `git` (reproduced with git 2.30.9, 2.32.7, 2.40.0 and 2.56.0),
//!   and with `GIT_OPTIONAL_LOCKS=0` the read still writes nothing
//!   (`the_content_query_writes_nothing_and_runs_nothing` configures one and
//!   requires the git directory byte-identical).
//! - **git's builtin daemon**, `core.fsmonitor=true` (git 2.36 and later on
//!   macOS and Windows, 2.55 and later on Linux, where the build has it): the first read that consults it starts
//!   `git fsmonitor--daemon` if none is running, and the daemon creates its
//!   socket, `.git/fsmonitor--daemon.ipc`, and its cookie directory,
//!   `.git/fsmonitor--daemon/`, in the git directory (reproduced with git
//!   2.56.0 under `GIT_OPTIONAL_LOCKS=0`; `diff --no-index`, which reads no
//!   index, starts none). git starts it in a session of its own, so it is
//!   outside the process group a read is ended by, outside the repository's
//!   registry of running invocations, and outside
//!   `SharedRepository::end_invocations`: it outlives the read and the
//!   application, as it outlives the user's own `git status`, and it is not
//!   Cairn's to end. No object, ref, index or config is written under it; the
//!   daemon's own files are the one exception
//!   (`a_read_under_the_builtin_fsmonitor_writes_only_the_daemons_own_files`,
//!   skipped where the git or the platform has no builtin daemon).
//!
//! A planted repository naming a hook is the opening's to refuse, as git
//! refuses it (`crate::bare_discovery`), not the read's.
//!
//! **A read of the working tree also runs the path's clean filter driver** —
//! L6, D1 as amended (`docs/design/engine.md`, "Reads see git's form"): `git
//! diff-files` and `git diff --no-index` convert the file to git's form as the
//! user's `git diff` does, so git starts the driver the path's attributes name
//! and the configuration defines — a `filter.<driver>.clean` command, or the
//! long-running `filter.<driver>.process` that `git lfs install` configures,
//! started once per read and sent `command=clean` and nothing else
//! (`a_long_running_filter_process_is_sent_only_clean_and_its_form_is_diffed`)
//! — with the read's environment plus what git sets for a filter. And `diff-files`, asked about a submodule whose checkout
//! it must look into, runs `git status` inside it — again what `git diff`
//! does — which may run that repository's own fsmonitor and clean filters
//! ([`working_tree_patch`] says what each read runs). Nothing else a read starts
//! may run a program: no textconv, external diff, driver `command` or smudge
//! filter, and no clean filter on a read of trees
//! (`the_content_query_writes_nothing_and_runs_nothing`).
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
//! `--raw`, porcelain v2, and a patch's text — `diff-tree -p`'s headers and
//! marked lines, which are format, not prose), and classifies a failure by exit
//! status and the repository's state, never by matching stderr, which is prose
//! in the user's language. A patch's one line of text rather than format,
//! `\ No newline at end of file`, is read by its first byte alone, so it is safe
//! whatever its words: git prints it untranslated (`diff.c` at v2.30.0 and
//! v2.56.0), and the parser counts the lines each hunk header owes, so a line
//! starting with `\` is taken only right after a hunk's line, where nothing else
//! git prints can start with it — anywhere else it is refused
//! (`a_no_newline_marker_belongs_to_the_line_before_it`, in `patches.rs`).
//!
//! Who pins what: the read's environment is built in `process/` and spelled
//! out by its tests; that only this module and `ops/` name the runner is
//! `the_runner_is_named_only_by_ops_and_reads`; that a read cannot build a
//! write is the compiler's, because only `ops/` can construct the
//! `WriteAuthority` a write needs; and that the three porcelain verbs are built
//! once each, in their accepted forms, is
//! `the_porcelain_reads_are_the_three_named_queries` (matcher self-test
//! `the_porcelain_read_matcher_catches_the_shapes_it_claims`): the exact
//! literal `"diff"` appears in this module's production code only in
//! `working_tree.rs`, once, with `"--no-index"` the next literal on its line
//! and `"/dev/null"` in the file — the `diff` attribute's two lines in
//! `attributes.rs` excused by name — and the exact literal `"config"` only in
//! `fetch_settings.rs`, once, every option literal there a query option and
//! no `git config` setter literal anywhere here; and the exact literal
//! `"stash"` only in `stash_changes.rs`, once, `"show"` the literal after it,
//! and no `git stash` subcommand that writes (`push`, `pop`, `apply`, `drop`,
//! `store`, `clear`, `create`, `branch`, `save`, `export`, `import`) as a
//! literal anywhere here.
//! What it cannot see is a
//! review obligation (`destructive-ops-reviewer`, check 10): a verb or option
//! built at run time — by `format!`, `concat!` or from bytes — and whether
//! every other verb a read runs is query plumbing or `status`, since a token
//! scan cannot tell `diff-tree` from `update-index` by what it does.

mod attributes;
mod changes;
mod fetch_settings;
mod hash_object;
mod hooks_path;
mod patches;
mod stash_changes;
mod status;
mod working_tree;

pub(crate) use attributes::{DiffAttribute, diff_attributes};
pub(crate) use changes::{Detection, Submodules, changes};
pub(crate) use fetch_settings::{FetchSettings, fetch_settings};
pub(crate) use hash_object::hash_object;
pub(crate) use hooks_path::hooks_path;
#[cfg(test)]
pub(crate) use patches::parse as parse_patches;
pub(crate) use patches::{Algorithm, FilePatch, PatchQuery, PatchText, Reading, Scope, patches};
pub(crate) use stash_changes::stash_changes;
pub(crate) use status::status;
pub(crate) use working_tree::{
    Paired, Side, WorkingTreeAnswer, WorkingTreeQuery, staged_pairing, staged_since,
    work_tree_relative, working_tree_patch,
};

/// The read as the diff thread will run it: built here from a `GitBinary` copy
/// that thread holds, run on that thread, stopped by an epoch, answering `-z`
/// records. Proved before `diff-engine` built [`changes`] as a sketch of it,
/// and kept against the real function.
#[cfg(test)]
mod diff_engine_path_forward {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use cairn_model::{ChangeStatus, Oid};

    use super::{Algorithm, Detection, PatchQuery, Scope, Submodules, changes, patches};
    use crate::ops::{Askpass, GitBinary, GitEnvironment};
    use crate::{Cancel, Error, SharedRepository};

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

    fn commit_id(fixture: &Fixture, spec: &str) -> Oid {
        Oid::parse(&fixture.git(&["rev-parse", spec], None)).unwrap_or_else(|e| panic!("{e}"))
    }

    /// A rename with an edit, between two commits: the answer `gix` and `git`
    /// can differ on, and the reason a read here runs `git` at all.
    #[test]
    fn a_read_built_here_and_run_on_a_threads_own_copy_answers_git_s_pairs() {
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
        let (old, new) = (commit_id(&fixture, "HEAD~1"), commit_id(&fixture, "HEAD"));

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
            let detection = Detection::Renames { limit: 1000 };
            changes(
                &thread_git,
                &repo,
                &old,
                &new,
                detection,
                Submodules::AsListed,
                &query,
            )
            .map(|files| (files, shared.command_log()))
        })
        .join()
        .unwrap_or_else(|_| panic!("the diff thread panicked"))
        .unwrap_or_else(|error| panic!("{error}"));

        let (files, log) = answered;
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(
            matches!(files[0].status, ChangeStatus::Renamed(_)),
            "not a rename: {files:?}"
        );
        assert_eq!(
            files[0].old_path.as_bytes(),
            b"old name",
            "a space in a path survives -z"
        );
        assert_eq!(
            files[0].new_path.as_bytes(),
            b"new\tname",
            "a tab in a path survives -z"
        );
        assert_eq!(log.len(), 1, "the read is booked once: {log:?}");
        assert_eq!(
            log[0].arguments.first().map(String::as_str),
            Some("diff-tree")
        );
        assert!(!log[0].cancelled);
    }

    /// Writes `count` files named `prefix-N` whose lines are all different, so
    /// every pair of a deletion and an addition is compared in full.
    fn write_files(fixture: &Fixture, prefix: &str, salt: usize, count: usize) {
        for n in 0..count {
            let body: String = (0..40)
                .map(|line| format!("line {} of {prefix}\n", n * salt + line))
                .collect();
            std::fs::write(fixture.repo.join(format!("{prefix}-{n:05}")), body)
                .unwrap_or_else(|e| panic!("{e}"));
        }
    }

    /// The same read superseded while `git` is still searching: an exhaustive
    /// rename search over thousands of deletions against thousands of additions,
    /// which takes `git` seconds. The epoch moves, `git` is stopped rather than
    /// waited for — the command log says it was ended, which a cancel that lost
    /// the race to `git`'s own exit never says — the caller hears a cancelled
    /// query, and nothing is left running.
    #[test]
    fn a_changes_query_superseded_by_a_newer_epoch_stops_git_and_reports_it() {
        const FILES: usize = 4_000;
        let fixture = Fixture::new("superseded");
        write_files(&fixture, "old", 7_919, FILES);
        fixture.git(&["add", "."], None);
        fixture.git(&["commit", "-q", "-m", "one"], None);
        fixture.git(&["rm", "-q", "-r", "."], None);
        write_files(&fixture, "new", 104_729, FILES);
        fixture.git(&["add", "."], None);
        fixture.git(&["commit", "-q", "-m", "two"], None);
        let (old, new) = (commit_id(&fixture, "HEAD~1"), commit_id(&fixture, "HEAD"));

        let shared = SharedRepository::discover(&fixture.repo).unwrap_or_else(|e| panic!("{e}"));
        let git = fixture.binary();
        let epochs = Arc::new(AtomicU64::new(0));
        let query = Epoch {
            current: Arc::clone(&epochs),
            started_under: 0,
        };
        let repo = shared.to_worker();
        // A newer query supersedes this one once `git` is well into its search.
        let superseding = {
            let epochs = Arc::clone(&epochs);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                epochs.fetch_add(1, Ordering::Release);
            })
        };
        let started = Instant::now();
        // No limit: the search is exhaustive, so `git` has seconds of work.
        let outcome = changes(
            &git,
            &repo,
            &old,
            &new,
            Detection::Renames { limit: 0 },
            Submodules::AsListed,
            &query,
        );
        let elapsed = started.elapsed();
        superseding
            .join()
            .unwrap_or_else(|_| panic!("the superseding thread panicked"));

        assert!(
            matches!(outcome, Err(Error::ChangesCancelled { .. })),
            "expected a cancelled query, got {outcome:?}"
        );
        assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
        let log = shared.command_log();
        assert_eq!(log.len(), 1, "{log:?}");
        assert!(log[0].cancelled, "git finished before the cancel: {log:?}");
        assert_eq!(
            shared.end_invocations(Duration::from_secs(1)),
            0,
            "left running"
        );
    }

    /// A query superseded before it starts never starts `git` at all.
    #[test]
    fn a_changes_query_superseded_before_it_starts_runs_nothing() {
        let fixture = Fixture::new("before");
        std::fs::write(fixture.repo.join("a"), "a\n").unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&["add", "."], None);
        fixture.git(&["commit", "-q", "-m", "one"], None);
        let head = commit_id(&fixture, "HEAD");

        let shared = SharedRepository::discover(&fixture.repo).unwrap_or_else(|e| panic!("{e}"));
        let epochs = Arc::new(AtomicU64::new(1));
        let query = Epoch {
            current: epochs,
            started_under: 0,
        };
        let outcome = changes(
            &fixture.binary(),
            &shared.to_worker(),
            &head,
            &head,
            Detection::Off,
            Submodules::AsListed,
            &query,
        );
        assert!(
            matches!(outcome, Err(Error::ChangesCancelled { changed: 0 })),
            "{outcome:?}"
        );
        assert!(shared.command_log().is_empty(), "a process was started");
    }

    /// Two versions of a 60,000-line file of random small numbers, which `minimal` takes
    /// seconds to diff: a content read that is still running when its epoch moves.
    fn slow_to_diff(fixture: &Fixture) -> (Oid, Oid) {
        let numbers = |seed: u64| -> String {
            let mut state = seed;
            (0..60_000)
                .map(|_| {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    format!("{}\n", (state >> 33) % 50)
                })
                .collect()
        };
        std::fs::write(fixture.repo.join("numbers"), numbers(1)).unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&["add", "."], None);
        fixture.git(&["commit", "-q", "-m", "one"], None);
        std::fs::write(fixture.repo.join("numbers"), numbers(2)).unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&["commit", "-q", "-a", "-m", "two"], None);
        (commit_id(fixture, "HEAD~1"), commit_id(fixture, "HEAD"))
    }

    /// The content read superseded while `git` is still diffing: `git` is stopped rather
    /// than waited for — the command log says it was ended — the caller hears a cancelled
    /// content query, and nothing is left running (R2.9, as the changes query is).
    #[test]
    fn a_content_read_superseded_by_a_newer_epoch_stops_git_and_reports_it() {
        let fixture = Fixture::new("content-superseded");
        let (old, new) = slow_to_diff(&fixture);
        let shared = SharedRepository::discover(&fixture.repo).unwrap_or_else(|e| panic!("{e}"));
        let repo = shared.to_worker();
        let git = fixture.binary();
        let unbounded = Epoch {
            current: Arc::new(AtomicU64::new(0)),
            started_under: 0,
        };
        let files = changes(
            &git,
            &repo,
            &old,
            &new,
            Detection::Off,
            Submodules::AsListed,
            &unbounded,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let epochs = Arc::new(AtomicU64::new(0));
        let query = Epoch {
            current: Arc::clone(&epochs),
            started_under: 0,
        };
        let superseding = {
            let epochs = Arc::clone(&epochs);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                epochs.fetch_add(1, Ordering::Release);
            })
        };
        let started = Instant::now();
        let outcome = patches(
            &git,
            &repo,
            &PatchQuery {
                old: &old,
                new: &new,
                context: 3,
                algorithm: Some(Algorithm::Minimal),
                ignore_whitespace: false,
                scope: Scope::File(&files[0]),
            },
            &query,
        );
        let elapsed = started.elapsed();
        superseding
            .join()
            .unwrap_or_else(|_| panic!("the superseding thread panicked"));
        assert!(
            matches!(outcome, Err(Error::ContentCancelled)),
            "expected a cancelled content query, got {outcome:?}"
        );
        assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
        let log = shared.command_log();
        let last = log.last().unwrap_or_else(|| panic!("nothing was logged"));
        assert_eq!(
            last.arguments.iter().filter(|a| *a == "diff-tree").count(),
            1
        );
        assert!(last.cancelled, "git finished before the cancel: {log:?}");
        assert_eq!(
            shared.end_invocations(Duration::from_secs(1)),
            0,
            "left running"
        );
    }

    /// A content read superseded before it starts never starts `git`.
    #[test]
    fn a_content_read_superseded_before_it_starts_runs_nothing() {
        let fixture = Fixture::new("content-before");
        std::fs::write(fixture.repo.join("a"), "a\n").unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&["add", "."], None);
        fixture.git(&["commit", "-q", "-m", "one"], None);
        std::fs::write(fixture.repo.join("a"), "b\n").unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&["commit", "-q", "-a", "-m", "two"], None);
        let (old, new) = (commit_id(&fixture, "HEAD~1"), commit_id(&fixture, "HEAD"));
        let shared = SharedRepository::discover(&fixture.repo).unwrap_or_else(|e| panic!("{e}"));
        let file = cairn_model::ChangedFile {
            status: ChangeStatus::Modified,
            old_path: cairn_model::RepoPath::new("a"),
            new_path: cairn_model::RepoPath::new("a"),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        };
        let query = Epoch {
            current: Arc::new(AtomicU64::new(1)),
            started_under: 0,
        };
        let outcome = patches(
            &fixture.binary(),
            &shared.to_worker(),
            &PatchQuery {
                old: &old,
                new: &new,
                context: 3,
                algorithm: None,
                ignore_whitespace: false,
                scope: Scope::File(&file),
            },
            &query,
        );
        assert!(
            matches!(outcome, Err(Error::ContentCancelled)),
            "{outcome:?}"
        );
        assert!(shared.command_log().is_empty(), "a process was started");
    }
}
