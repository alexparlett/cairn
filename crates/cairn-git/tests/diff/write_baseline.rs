//! git's own time for the writes staging-and-commit's C21 bounds (R13.2): stage a hunk,
//! unstage it, discard it and commit — each the `git` Cairn runs for it — and one status
//! read, as Cairn reads it, so C21's margin is written against git before any Cairn number
//! exists. Numbers go into `docs/research/staging-and-commit/measured-baseline.md`.
//!
//! Not an assertion, for the reason C14's reporter is not: a timing check in CI is flaky and
//! bound to a machine. It WRITES — every verb it times changes the repository — so it runs
//! only on a scratch clone named by `CAIRN_BENCH_SCRATCH_CLONE`: a plain, non-shared clone of
//! rust-lang/rust checked out at `c999cef531e`, on tmpfs, never the bench repository itself
//! (refused when it is the one `CAIRN_BENCH_REPO` names, when it borrows objects through
//! `objects/info/alternates`, when `HEAD` is not that commit, and when its working tree is
//! not clean). Everything it changes it puts back between runs and at the end.
//!
//! ```text
//! git clone --no-hardlinks --no-checkout ~/Development/bench/rust /tmp/<scratch>/rust
//! git -C /tmp/<scratch>/rust checkout --detach c999cef531e
//! CAIRN_BENCH_SCRATCH_CLONE=/tmp/<scratch>/rust \
//!   cargo test -p cairn-git --release --test diff_engine -- --ignored --nocapture write_baseline
//! ```

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// R13.1's subject: rust-lang/rust at this commit.
const SUBJECT: &str = "c999cef531ea9059e189e82fe0e82c5daf249bc9";

/// The file a hunk is staged, unstaged and discarded in, and where a line goes into it.
const FILE: &str = "library/core/src/option.rs";
const LINE: usize = 1_500;

const RUNS: usize = 7;

struct Clone {
    path: PathBuf,
    home: PathBuf,
}

impl Clone {
    fn git(&self, args: &[&str], stdin: Option<&[u8]>, read: bool) -> (Duration, Vec<u8>) {
        let mut command = Command::new("git");
        command
            .current_dir(&self.path)
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &self.home)
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "A U Thor")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "C O Mitter")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if read {
            command.env("GIT_OPTIONAL_LOCKS", "0");
        }
        let started = Instant::now();
        let mut child = command
            .spawn()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
        if let Some(bytes) = stdin {
            let mut pipe = child
                .stdin
                .take()
                .unwrap_or_else(|| panic!("git {args:?} has no stdin"));
            pipe.write_all(bytes)
                .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
        }
        let output = child
            .wait_with_output()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
        let taken = started.elapsed();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        (taken, output.stdout)
    }

    fn file(&self) -> PathBuf {
        self.path.join(FILE)
    }

    fn index(&self) -> PathBuf {
        self.path.join(".git/index")
    }
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
}

fn median(mut taken: Vec<Duration>) -> Duration {
    taken.sort_unstable();
    taken[taken.len() / 2]
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Times `run` once to warm, then [`RUNS`] times, with `reset` between runs untimed; prints
/// and returns the median.
fn time(name: &str, mut reset: impl FnMut(), mut run: impl FnMut() -> Duration) -> Duration {
    reset();
    run();
    let mut taken = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        reset();
        taken.push(run());
    }
    let best = taken.iter().min().copied().unwrap_or_default();
    let worst = taken.iter().max().copied().unwrap_or_default();
    let middle = median(taken);
    eprintln!(
        "  {name}\n    median {:.2} ms\tmin {:.2}\tmax {:.2}",
        ms(middle),
        ms(best),
        ms(worst)
    );
    middle
}

/// The scratch clone the variable names, after every refusal the module docs list.
fn scratch_clone() -> Option<Clone> {
    let path = std::env::var_os("CAIRN_BENCH_SCRATCH_CLONE")?;
    let path = std::fs::canonicalize(PathBuf::from(path))
        .unwrap_or_else(|e| panic!("CAIRN_BENCH_SCRATCH_CLONE: {e}"));
    if let Some(bench) = std::env::var_os("CAIRN_BENCH_REPO") {
        let bench = std::fs::canonicalize(PathBuf::from(bench)).ok();
        assert_ne!(
            bench.as_deref(),
            Some(path.as_path()),
            "CAIRN_BENCH_SCRATCH_CLONE is the bench repository, which is never written"
        );
    }
    assert!(
        !path.join(".git/objects/info/alternates").exists(),
        "{} borrows objects through alternates: a shared clone, which is never made of the \
         bench",
        path.display()
    );
    let home = std::env::temp_dir().join(format!("cairn-write-baseline-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap_or_else(|e| panic!("{e}"));
    let clone = Clone { path, home };
    let (_, head) = clone.git(&["rev-parse", "HEAD"], None, true);
    assert_eq!(
        String::from_utf8_lossy(&head).trim(),
        SUBJECT,
        "the scratch clone is not at R13.1's commit"
    );
    let (_, status) = clone.git(&["status", "--porcelain=v2", "-z"], None, true);
    assert!(
        status.is_empty(),
        "the scratch clone's working tree is not clean"
    );
    Some(clone)
}

/// R13.2: git's own time for each verb C21 bounds, and one status read.
#[test]
#[ignore = "a reporter: needs CAIRN_BENCH_SCRATCH_CLONE, a writable clone of the bench"]
fn write_baseline() {
    let Some(clone) = scratch_clone() else {
        eprintln!("SKIPPED write_baseline: CAIRN_BENCH_SCRATCH_CLONE is not set");
        return;
    };
    let (_, version) = clone.git(&["--version"], None, true);
    eprintln!(
        "{} on {} at {SUBJECT}, {RUNS} runs after one to warm, median:",
        String::from_utf8_lossy(&version).trim(),
        clone.path.display()
    );

    let original = read(&clone.file());
    let clean_index = read(&clone.index());
    let mut edited = Vec::with_capacity(original.len() + 64);
    for (n, line) in original.split_inclusive(|byte| *byte == b'\n').enumerate() {
        if n == LINE {
            edited.extend_from_slice(b"// a line Cairn's baseline stages, unstages and discards\n");
        }
        edited.extend_from_slice(line);
    }
    write(&clone.file(), &edited);
    // The hunk as Cairn would stage it: git's own patch of the one change, which is what the
    // model emits for a whole hunk; and its inversion, which `diff -R` prints and Cairn's
    // emitter writes without `-R`.
    let (_, forward) = clone.git(&["diff", "--no-color", "--", FILE], None, true);
    let (_, inverted) = clone.git(&["diff", "--no-color", "-R", "--", FILE], None, true);
    assert!(!forward.is_empty() && !inverted.is_empty());

    let status = time(
        "status read (git status --porcelain=v2 -z, GIT_OPTIONAL_LOCKS=0), one edited file",
        || {},
        || clone.git(&["status", "--porcelain=v2", "-z"], None, true).0,
    );

    let stage = time(
        "stage a hunk (git apply --cached --whitespace=nowarn -)",
        || write(&clone.index(), &clean_index),
        || {
            clone
                .git(
                    &[
                        "--literal-pathspecs",
                        "apply",
                        "--cached",
                        "--whitespace=nowarn",
                        "-",
                    ],
                    Some(&forward),
                    false,
                )
                .0
        },
    );
    let staged_index = read(&clone.index());

    let unstage = time(
        "unstage a hunk (git apply --cached --whitespace=nowarn -, the inverted patch)",
        || write(&clone.index(), &staged_index),
        || {
            clone
                .git(
                    &[
                        "--literal-pathspecs",
                        "apply",
                        "--cached",
                        "--whitespace=nowarn",
                        "-",
                    ],
                    Some(&inverted),
                    false,
                )
                .0
        },
    );

    write(&clone.index(), &clean_index);
    let discard = time(
        "discard a hunk (git apply --whitespace=nowarn -, the inverted patch, on the working tree)",
        || write(&clone.file(), &edited),
        || {
            clone
                .git(
                    &["--literal-pathspecs", "apply", "--whitespace=nowarn", "-"],
                    Some(&inverted),
                    false,
                )
                .0
        },
    );

    write(&clone.file(), &edited);
    write(&clone.index(), &staged_index);
    let committed = std::cell::Cell::new(false);
    let commit = time(
        "commit the staged hunk (git commit -F -, no hooks)",
        || {
            if committed.get() {
                clone.git(&["reset", "--soft", "-q", "HEAD^"], None, false);
            }
        },
        || {
            committed.set(true);
            clone
                .git(
                    &["commit", "-q", "-F", "-"],
                    Some(b"A commit Cairn's baseline makes\n"),
                    false,
                )
                .0
        },
    );

    // Put the clone back as it was found.
    clone.git(&["reset", "--soft", "-q", "HEAD^"], None, false);
    write(&clone.file(), &original);
    write(&clone.index(), &clean_index);
    let (_, head) = clone.git(&["rev-parse", "HEAD"], None, true);
    assert_eq!(String::from_utf8_lossy(&head).trim(), SUBJECT);
    let _ = std::fs::remove_dir_all(&clone.home);

    eprintln!("git's own time plus one status read, the sum C21's margin is added to:");
    for (name, verb) in [
        ("stage", stage),
        ("unstage", unstage),
        ("discard", discard),
        ("commit", commit),
    ] {
        eprintln!(
            "  {name}: {:.2} + {:.2} = {:.2} ms",
            ms(verb),
            ms(status),
            ms(verb + status)
        );
    }
}

/// What Cairn's own verbs cost on the same clone and hunk (staging-and-commit phase 11, C21's
/// engine half and #87): each through `cairn_git::ops` as the local lane calls it — the stale
/// check, the lock listings before and after, git — and the lock listing alone, a walk of
/// `refs/`, which every local write makes twice. The repository is opened before each run,
/// untimed, as the lane holds its handle. C21's bar is the window's (press to the refreshed
/// lists drawn), measured by `window_check`; this says where the engine's share goes.
#[test]
#[ignore = "a reporter: needs CAIRN_BENCH_SCRATCH_CLONE, a writable clone of the bench"]
fn cairn_write_costs() {
    use cairn_git::{CancelSignal, ContentOptions, Repository, SharedRepository, WorkingTreeDiff};
    use cairn_model::{Confirmed, RepoPath, Selection};

    let Some(clone) = scratch_clone() else {
        eprintln!("SKIPPED cairn_write_costs: CAIRN_BENCH_SCRATCH_CLONE is not set");
        return;
    };
    let git = super::git();
    let open = || -> Repository {
        SharedRepository::discover(&clone.path)
            .unwrap_or_else(|e| panic!("{e}"))
            .to_worker()
    };
    let original = read(&clone.file());
    let clean_index = read(&clone.index());
    let mut edited = Vec::with_capacity(original.len() + 64);
    for (n, line) in original.split_inclusive(|byte| *byte == b'\n').enumerate() {
        if n == LINE {
            edited.extend_from_slice(b"// a line Cairn's baseline stages, unstages and discards\n");
        }
        edited.extend_from_slice(line);
    }
    write(&clone.file(), &edited);
    let path = RepoPath::new(FILE);
    let diff_of = |which| {
        open()
            .working_tree_diff(
                git,
                &path,
                which,
                &ContentOptions::default(),
                &CancelSignal::new(),
            )
            .unwrap_or_else(|e| panic!("{e}"))
            .unwrap_or_else(|| panic!("no diff"))
    };
    let every = |diff: &cairn_model::FileDiff| {
        let mut selection = Selection::empty();
        if let Some(text) = diff.text() {
            for change in text.changes() {
                selection.select_change(change);
            }
        }
        selection
    };
    eprintln!(
        "Cairn's verbs on {}, {RUNS} runs after one to warm, median:",
        clone.path.display()
    );

    let shared = SharedRepository::discover(&clone.path).unwrap_or_else(|e| panic!("{e}"));
    let locks = time(
        "the lock listing (SharedRepository::lock_files: the git directory, refs/ walked whole)",
        || {},
        || {
            let started = Instant::now();
            let listed = shared.lock_files(&CancelSignal::new());
            assert_eq!(listed.map(|locks| locks.len()), Some(0));
            started.elapsed()
        },
    );

    let unstaged = diff_of(WorkingTreeDiff::Unstaged);
    let selection = every(&unstaged);
    let repo = std::cell::RefCell::new(open());
    let stage = time(
        "ops::stage_lines (stale check, locks before and after, git apply --cached)",
        || {
            write(&clone.index(), &clean_index);
            *repo.borrow_mut() = open();
        },
        || {
            let started = Instant::now();
            cairn_git::ops::stage_lines(git, &repo.borrow(), &unstaged, &selection, None)
                .unwrap_or_else(|e| panic!("{e}"));
            started.elapsed()
        },
    );
    let staged_index = read(&clone.index());

    let staged = diff_of(WorkingTreeDiff::Staged);
    let staged_selection = every(&staged);
    let unstage = time(
        "ops::unstage_lines",
        || {
            write(&clone.index(), &staged_index);
            *repo.borrow_mut() = open();
        },
        || {
            let started = Instant::now();
            cairn_git::ops::unstage_lines(git, &repo.borrow(), &staged, &staged_selection, None)
                .unwrap_or_else(|e| panic!("{e}"));
            started.elapsed()
        },
    );

    write(&clone.index(), &clean_index);
    let consequence = time(
        "ops::discard_lines_consequence (asked before the dialog, not in C21's press)",
        || *repo.borrow_mut() = open(),
        || {
            let started = Instant::now();
            cairn_git::ops::discard_lines_consequence(
                git,
                &repo.borrow(),
                &unstaged,
                selection.clone(),
            )
            .unwrap_or_else(|e| panic!("{e}"));
            started.elapsed()
        },
    );
    let confirmed = std::cell::RefCell::new(None);
    let discard = time(
        "ops::discard_lines (re-check, locks before and after, git apply)",
        || {
            write(&clone.file(), &edited);
            *repo.borrow_mut() = open();
            *confirmed.borrow_mut() = Some(Confirmed::by_user(
                cairn_git::ops::discard_lines_consequence(
                    git,
                    &repo.borrow(),
                    &unstaged,
                    selection.clone(),
                )
                .unwrap_or_else(|e| panic!("{e}")),
            ));
        },
        || {
            let token = confirmed
                .borrow_mut()
                .take()
                .unwrap_or_else(|| panic!("no confirmation"));
            let started = Instant::now();
            cairn_git::ops::discard_lines(git, &repo.borrow(), token, None)
                .unwrap_or_else(|e| panic!("{e}"));
            started.elapsed()
        },
    );

    write(&clone.file(), &edited);
    write(&clone.index(), &staged_index);
    clone.git(&["config", "user.name", "C O Mitter"], None, false);
    clone.git(
        &["config", "user.email", "committer@example.com"],
        None,
        false,
    );
    let committed = std::cell::Cell::new(false);
    let commit = time(
        "ops::commit (checks, locks before and after, git commit -q -F -, no hooks)",
        || {
            if committed.get() {
                clone.git(&["reset", "--soft", "-q", "HEAD^"], None, false);
            }
            *repo.borrow_mut() = open();
        },
        || {
            committed.set(true);
            let cancel = CancelSignal::new();
            let (mut running, mut output) = (|_| {}, |_: &[&str]| {});
            let started = Instant::now();
            cairn_git::ops::commit(
                git,
                &repo.borrow(),
                "A commit Cairn's reporter makes",
                cairn_git::ops::Hooks::Run,
                None,
                cairn_git::ops::CommitWatch {
                    cancel: &cancel,
                    running: &mut running,
                    output: &mut output,
                },
            )
            .unwrap_or_else(|e| panic!("{e}"));
            started.elapsed()
        },
    );

    clone.git(&["reset", "--soft", "-q", "HEAD^"], None, false);
    clone.git(&["config", "--unset", "user.name"], None, false);
    clone.git(&["config", "--unset", "user.email"], None, false);
    write(&clone.file(), &original);
    write(&clone.index(), &clean_index);
    let (_, head) = clone.git(&["rev-parse", "HEAD"], None, true);
    assert_eq!(String::from_utf8_lossy(&head).trim(), SUBJECT);
    let _ = std::fs::remove_dir_all(&clone.home);
    eprintln!(
        "two lock listings a write: {:.2} ms of stage {:.2}, unstage {:.2}, discard {:.2} \
         (its consequence {:.2} before the dialog), commit {:.2}",
        ms(locks * 2),
        ms(stage),
        ms(unstage),
        ms(discard),
        ms(consequence),
        ms(commit)
    );
}
