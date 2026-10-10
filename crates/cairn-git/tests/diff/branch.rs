//! staging-and-commit's `Create Branch Here…` against real `git` (R11.3, C20): the branch
//! `ops::create_branch` makes is the one `git branch -- <name> <commit>` makes — at the same
//! commit, logged with the same message — on a commit only a reflog still reaches, and a
//! name git refuses is refused with git's reason and nothing written; a name is taken or
//! refused as `git branch` and `git checkout -b` take it (C32); and Create Branch's Discard
//! runs Fork's forced checkout, leaving what Fork's command leaves, re-checking `HEAD`, the
//! commit and the name and nothing else, and refusing an operation in progress before git runs
//! (C34; the user's decision of 2026-10-10). Run on the host's git and, through
//! `scripts/git-floor.sh`, on 2.30.9 and 2.32.7.

use std::path::{Path, PathBuf};

use cairn_git::{CancelSignal, CheckoutMoved, CheckoutRefusal, Error, Repository, ops};
use cairn_model::{BranchName, Confirmed, Consequence, NameRefusal, Oid, OperationInProgress};

use super::repositories::Repo;
use super::{git, ok};

fn engine(repo: &Repo) -> Repository {
    ok(Repository::discover(repo.path()), "the fixture opens")
}

/// Every ref and what it names, as git lists them.
fn refs(repo: &Repo) -> String {
    repo.git(&["for-each-ref", "--format=%(refname) %(objectname)"])
}

/// A repository whose `HEAD` was amended: the replaced commit only its reflog reaches.
fn amended() -> (Repo, cairn_model::Oid) {
    let repo = Repo::new("create-branch");
    repo.write("f", b"1\n");
    repo.commit("first");
    repo.write("f", b"2\n");
    let replaced = repo.commit("second");
    repo.git(&["commit", "--quiet", "--amend", "-m", "second, amended"]);
    (repo, replaced)
}

/// Caught by: the branch made elsewhere than the commit given, a name read as an option or
/// a revision, or a branch made otherwise than `git branch` makes it (its reflog's message).
#[test]
fn create_branch_here_makes_the_branch_git_branch_makes() {
    let (repo, replaced) = amended();
    let performed = ok(
        ops::create_branch(git(), &engine(&repo), "recovered", replaced, None),
        "creating the branch",
    );
    assert!(performed.invalidated().refs);
    assert!(performed.acknowledged().is_none(), "not destructive");
    repo.git(&["branch", "--", "by-git", &replaced.to_string()]);
    assert_eq!(repo.rev("refs/heads/recovered"), replaced);
    assert_eq!(repo.rev("refs/heads/by-git"), replaced);
    let message = |branch: &str| {
        repo.git(&[
            "reflog",
            "show",
            "--format=%gs",
            &format!("refs/heads/{branch}"),
        ])
        .replace(branch, "<name>")
    };
    assert_eq!(
        message("recovered"),
        message("by-git"),
        "logged as git logs it"
    );
    assert_eq!(
        repo.git(&["rev-parse", "HEAD@{0}"]),
        repo.git(&["rev-parse", "HEAD"]),
        "HEAD is where it was"
    );
}

/// What the user's own `git branch -- <name> <commit>` says refusing `name`, run with this
/// process's environment — the locale and the configuration the engine's `git` inherits.
fn own_refusal(repo: &Repo, name: &str, commit: cairn_model::Oid) -> String {
    let output = ok(
        std::process::Command::new("git")
            .current_dir(repo.path())
            .args(["branch", "--", name, &commit.to_string()])
            .output(),
        "running git branch",
    );
    assert!(!output.status.success(), "git took {name:?}");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A name taken, one git's rules refuse, and one beginning with `-`: each refused by git,
/// with its reason, and no ref written or moved. Caught by: `-f` (a taken branch moved), a
/// name read as an option, or a failure without git's words.
#[test]
fn a_name_git_refuses_is_refused_with_its_reason_and_nothing_written() {
    let (repo, replaced) = amended();
    let before = refs(&repo);
    for name in ["main", "bad..name", "-f", ""] {
        match ops::create_branch(git(), &engine(&repo), name, replaced, None) {
            Err(Error::GitFailed { stderr, .. }) => {
                assert_eq!(
                    stderr.lines().next(),
                    own_refusal(&repo, name, replaced).lines().next(),
                    "{name:?}: git's reason"
                );
            }
            other => panic!("{name:?} was not refused by git: {other:?}"),
        }
        assert_eq!(refs(&repo), before, "{name:?} wrote a ref");
    }
}

/// Every file under `dir`, with its bytes, sorted: a before-and-after of what a read wrote.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap_or_default();
                found.push((path, bytes));
            }
        }
    }
    found.sort();
    found
}

/// What the user's own `git check-ref-format --branch <name>` says of `name`, run with this
/// process's environment: its first stderr line, `fatal: ` left off, or `None` where it takes
/// the name.
fn gits_own_refusal(repo: &Repo, name: &str) -> Option<String> {
    let output = ok(
        std::process::Command::new("git")
            .current_dir(repo.path())
            .args(["check-ref-format", "--branch", name])
            .output(),
        "running git check-ref-format",
    );
    if output.status.success() {
        return None;
    }
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let first = stderr.lines().next().unwrap_or_default();
    Some(first.strip_prefix("fatal: ").unwrap_or(first).to_owned())
}

/// The user's decision (2026-10-09): Create Branch refuses a name inline, before git runs, by
/// git's own rules — a name git takes and no branch has is free, one a branch has is taken,
/// and one git refuses is refused with git's own words — and the read writes nothing. Caught
/// by: a rule of git's left out or one of Cairn's own added, a taken name passed as free, or a
/// read that writes.
#[test]
fn a_branch_name_is_checked_by_gits_rules_and_the_check_writes_nothing() {
    let (repo, _) = amended();
    let engine = engine(&repo);
    let before = snapshot(&repo.path().join(".git"));
    for (name, expected) in [
        ("topic", BranchName::Free),
        ("feature/x", BranchName::Free),
        ("main", BranchName::Refused(NameRefusal::Taken)),
    ] {
        assert_eq!(
            ok(engine.branch_name(git(), name, &CancelSignal::new()), name),
            expected,
            "{name:?}"
        );
    }
    for name in [
        "bad..name",
        "-x",
        "ends.",
        "a b",
        "x.lock",
        "HEAD",
        "@",
        "",
        "/",
    ] {
        let answer = ok(engine.branch_name(git(), name, &CancelSignal::new()), name);
        match (answer, gits_own_refusal(&repo, name)) {
            (BranchName::Refused(NameRefusal::Invalid { reason }), Some(gits)) => {
                assert_eq!(reason, gits, "{name:?}")
            }
            (BranchName::Free | BranchName::Refused(NameRefusal::Taken), None) => {}
            (answer, gits) => panic!("{name:?}: Cairn said {answer:?}, git {gits:?}"),
        }
    }
    assert_eq!(
        snapshot(&repo.path().join(".git")),
        before,
        "the check wrote"
    );
}

/// A repository with `main` two commits long and `older` its first: the commit a branch is
/// created at, whose tree lacks `new.txt`.
fn two_commits() -> (Repo, Oid) {
    let repo = Repo::new("create-and-checkout");
    repo.write("f", b"1\n");
    repo.write("g", b"g1\n");
    let older = repo.commit("first");
    repo.write("f", b"2\n");
    repo.write("only-later.txt", b"later\n");
    repo.commit("second");
    (repo, older)
}

/// "Don't change" (the user's decision, 2026-10-09): the branch is made and checked out with
/// the working tree's changes carried over; where a change would be overwritten git refuses
/// with its words, and neither a branch nor a change is written. Caught by: `-f` on the kept
/// checkout (the change lost), or a branch left behind by a refused checkout.
#[test]
fn a_kept_checkout_carries_the_changes_or_is_refused_by_git_writing_nothing() {
    let (repo, older) = two_commits();
    repo.write("g", b"g edited\n");
    let performed = ok(
        ops::create_branch_and_checkout(git(), &engine(&repo), "kept", older, None),
        "the kept checkout",
    );
    assert!(performed.acknowledged().is_none(), "not destructive");
    assert_eq!(
        repo.git(&["symbolic-ref", "HEAD"]).trim(),
        "refs/heads/kept"
    );
    assert_eq!(repo.rev("HEAD"), older);
    assert_eq!(
        std::fs::read(repo.path().join("g")).unwrap_or_default(),
        b"g edited\n"
    );

    let (repo, older) = two_commits();
    repo.write("f", b"would be lost\n");
    let before = refs(&repo);
    match ops::create_branch_and_checkout(git(), &engine(&repo), "refused", older, None) {
        Err(Error::GitFailed { stderr, .. }) => assert!(stderr.contains('f'), "{stderr}"),
        other => panic!("git did not refuse: {other:?}"),
    }
    assert_eq!(refs(&repo), before, "a branch was written");
    assert_eq!(
        std::fs::read(repo.path().join("f")).unwrap_or_default(),
        b"would be lost\n"
    );
}

/// What `git status` lists, every untracked file named.
fn status(repo: &Repo) -> String {
    repo.git(&["status", "--porcelain=v1", "-uall"])
}

/// The consequence Discard is confirmed with, read now.
fn discarding(repo: &Repo, branch: &str, at: Oid) -> Consequence {
    ok(
        ops::checkout_discarding_consequence(&engine(repo), branch, at),
        "the consequence",
    )
}

/// A working tree Discard is pressed over: a staged edit to `f`, an unstaged edit to `g`, a
/// staged new file, `only-later.txt` deleted, an untracked file at a path `older` holds
/// (`in-the-way.txt`, which `main`'s tip does not), and one `older` holds nothing at.
fn dirty() -> (Repo, Oid) {
    let repo = Repo::new("discarding");
    repo.write("f", b"1\n");
    repo.write("g", b"g1\n");
    repo.write("in-the-way.txt", b"committed\n");
    let older = repo.commit("first");
    repo.git(&["rm", "-q", "in-the-way.txt"]);
    repo.write("f", b"2\n");
    repo.write("only-later.txt", b"later\n");
    repo.commit("second");
    repo.write("f", b"staged\n");
    repo.git(&["add", "f"]);
    repo.write("g", b"g one\ng two\n");
    repo.write("new.txt", b"brand new\n");
    repo.git(&["add", "new.txt"]);
    repo.remove("only-later.txt");
    repo.write("in-the-way.txt", b"my untracked work\n");
    repo.write("scratch.txt", b"keep me\n");
    (repo, older)
}

/// Every file under the working tree but `.git`, with its bytes, sorted.
fn tree(repo: &Repo) -> Vec<(PathBuf, Vec<u8>)> {
    snapshot(repo.path())
        .into_iter()
        .filter_map(|(path, bytes)| {
            let relative = path.strip_prefix(repo.path()).ok()?.to_owned();
            (!relative.starts_with(".git")).then_some((relative, bytes))
        })
        .collect()
}

/// C34, B1 and the user's decision of 2026-10-10: Discard runs Fork's command, and leaves
/// exactly what Fork's own command leaves — staged and unstaged changes and a staged new file
/// gone, an untracked file at a path the commit holds overwritten, an untracked file it holds
/// nothing at kept — on the new branch at the commit; the consequence it was confirmed with is
/// fixed, naming no file, and is the prompt recorded. The oracle is Fork's observed command,
/// `git checkout --no-track -b <name> <commit> --force`, run on an identical fixture. Caught by:
/// a kept checkout run, a prediction put back, `--no-track` dropped (an upstream set where
/// Fork's sets none), or anything lost or kept otherwise than Fork's command does.
#[test]
fn a_discarding_checkout_leaves_what_forks_command_leaves() {
    let (repo, older) = dirty();
    let consequence = discarding(&repo, "rescue", older);
    assert_eq!(
        consequence,
        Consequence::CheckoutDiscarding {
            branch: "rescue".to_owned(),
            at: older,
            head: Some(repo.rev("HEAD")),
        }
    );
    let performed = ok(
        ops::create_branch_discarding(
            git(),
            &engine(&repo),
            Confirmed::by_user(consequence.clone()),
            None,
        ),
        "the discarding checkout",
    );
    assert_eq!(
        performed.acknowledged(),
        Some(consequence.prompt().as_str())
    );
    assert_eq!(
        repo.git(&["symbolic-ref", "HEAD"]).trim(),
        "refs/heads/rescue"
    );
    assert_eq!(repo.rev("HEAD"), older);
    assert_eq!(
        status(&repo),
        "?? scratch.txt\n",
        "every change gone, scratch kept"
    );
    assert_eq!(
        std::fs::read(repo.path().join("in-the-way.txt")).unwrap_or_default(),
        b"committed\n",
        "the untracked file in the way overwritten"
    );

    let (forks, older) = dirty();
    forks.git(&[
        "checkout",
        "--no-track",
        "-b",
        "rescue",
        &older.to_string(),
        "--force",
    ]);
    assert_eq!(status(&repo), status(&forks));
    assert_eq!(
        tree(&repo),
        tree(&forks),
        "the working tree as Fork's leaves it"
    );
    assert_eq!(
        repo.try_git(&["config", "branch.rescue.merge"], &[], None)
            .ok(),
        forks
            .try_git(&["config", "branch.rescue.merge"], &[], None)
            .ok(),
        "the upstream as Fork's sets it"
    );
}

/// The user's decision of 2026-10-10 on phase 14's stopping rule: over a submodule's change,
/// Discard runs as Fork's does — git discards the rest and leaves the submodule's change in
/// place, its checkout on the moved commit and a staged change of its commit listed again as
/// unstaged — and over a conflicted path with no operation in progress git discards the
/// conflict. Neither is refused before git runs. Caught by: either refused, or git's handling
/// of them differing on this git from the probe's.
#[test]
fn a_submodules_change_survives_the_discard_and_a_conflict_is_discarded() {
    for staged in [false, true] {
        let repo = Repo::new("submodule-change");
        repo.write("a.txt", b"a\n");
        let inner = Repo::borrowed(&repo.path().join("sub"));
        ok(std::fs::create_dir_all(inner.path()), "making sub");
        inner.git(&["init", "--quiet", "--initial-branch=main", "."]);
        inner.write("s", b"1\n");
        inner.commit("s1");
        repo.git(&["add", "a.txt", "sub"]);
        let head = repo.commit("with a submodule");
        inner.write("s", b"2\n");
        let moved = inner.commit("s2");
        if staged {
            repo.git(&["add", "sub"]);
        }
        repo.write("a.txt", b"edited\n");
        let before = status(&repo);
        assert!(before.contains("sub\n"), "{before}");
        ok(
            ops::create_branch_discarding(
                git(),
                &engine(&repo),
                Confirmed::by_user(discarding(&repo, "away", head)),
                None,
            ),
            "the discarding checkout over a submodule",
        );
        assert_eq!(
            status(&repo),
            " M sub\n",
            "staged {staged}: the rest discarded, the submodule's change left unstaged"
        );
        assert_eq!(inner.rev("HEAD"), moved, "the submodule left on its commit");
    }

    let repo = Repo::new("stash-conflict");
    repo.write("a.txt", b"base\n");
    let base = repo.commit("base");
    repo.write("a.txt", b"stashed\n");
    repo.git(&["stash", "-q"]);
    repo.write("a.txt", b"conflicting\n");
    repo.commit("conflicting");
    assert!(repo.try_git(&["stash", "apply", "-q"], &[], None).is_err());
    assert_eq!(status(&repo), "UU a.txt\n");
    assert_eq!(engine(&repo).operation_in_progress(), None);
    ok(
        ops::create_branch_discarding(
            git(),
            &engine(&repo),
            Confirmed::by_user(discarding(&repo, "away", base)),
            None,
        ),
        "the discarding checkout over a conflict",
    );
    assert_eq!(status(&repo), "", "the conflict discarded");
    assert_eq!(repo.rev("HEAD"), base);
}

/// The user's decision of 2026-10-10: an operation in progress is refused before git runs,
/// since git's forced checkout would abandon it without a word — at the confirmation, and again
/// at the run when one began after it. Nothing is written and no `git checkout` runs: the merge
/// and its conflict stay. Caught by: the refusal dropped (the merge silently abandoned), or a
/// checkout started anyway.
#[test]
fn a_merge_in_progress_is_refused_with_git_not_run() {
    let repo = Repo::new("merging");
    repo.write("a.txt", b"base\n");
    let base = repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("a.txt", b"other\n");
    repo.commit("other");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("a.txt", b"main\n");
    repo.commit("main");
    // Confirmed before the merge began: the run refuses it all the same.
    let confirmed_before = discarding(&repo, "away", base);
    assert!(repo.try_git(&["merge", "-q", "other"], &[], None).is_err());
    assert_eq!(status(&repo), "UU a.txt\n");
    let engine = engine(&repo);
    let mark = engine.command_mark();
    assert!(matches!(
        ops::checkout_discarding_consequence(&engine, "away", base),
        Err(Error::CheckoutRefused {
            why: CheckoutRefusal::InProgress(OperationInProgress::Merge)
        })
    ));
    let refs_before = refs(&repo);
    assert!(matches!(
        ops::create_branch_discarding(git(), &engine, Confirmed::by_user(confirmed_before), None),
        Err(Error::CheckoutRefused {
            why: CheckoutRefusal::InProgress(OperationInProgress::Merge)
        })
    ));
    assert!(
        engine.commands_since(mark).iter().all(|record| !record
            .arguments
            .iter()
            .any(|argument| argument == "checkout")),
        "a git checkout ran"
    );
    assert_eq!(refs(&repo), refs_before, "a branch was written");
    assert!(
        repo.path().join(".git/MERGE_HEAD").exists(),
        "the merge abandoned"
    );
    assert_eq!(status(&repo), "UU a.txt\n");
}

/// C34, R1.4 as the user decided it (2026-10-10): between the press and the run, `HEAD` moved,
/// the commit gone or the name taken each refuses, writing nothing — and nothing else is
/// compared, so a file edited after the press is discarded with the rest. Caught by: a checkout
/// run on what was confirmed then, a re-check left out, or the old prediction's comparison
/// (any edit refusing) kept.
#[test]
fn the_recheck_refuses_head_the_commit_or_the_name_moved_and_nothing_else() {
    // HEAD moved.
    let (repo, older) = dirty();
    let confirmed = discarding(&repo, "late", older);
    repo.git(&["commit", "-q", "-m", "moved"]);
    let before = refs(&repo);
    match ops::create_branch_discarding(git(), &engine(&repo), Confirmed::by_user(confirmed), None)
    {
        Err(Error::CheckoutChangedSinceConfirmed {
            what: CheckoutMoved::Head,
        }) => {}
        other => panic!("HEAD moved: {other:?}"),
    }
    assert_eq!(refs(&repo), before, "HEAD moved: a branch was written");

    // The commit gone: a commit only a reflog reached, its reflog expired and pruned.
    let (repo, lost) = amended();
    repo.write("f", b"dirty\n");
    let confirmed = discarding(&repo, "late", lost);
    repo.git(&[
        "reflog",
        "expire",
        "--expire=now",
        "--expire-unreachable=now",
        "--all",
    ]);
    repo.git(&["gc", "-q", "--prune=now"]);
    let before = refs(&repo);
    match ops::create_branch_discarding(git(), &engine(&repo), Confirmed::by_user(confirmed), None)
    {
        Err(Error::CheckoutChangedSinceConfirmed {
            what: CheckoutMoved::Commit { at },
        }) => assert_eq!(at, lost),
        other => panic!("the commit gone: {other:?}"),
    }
    assert_eq!(refs(&repo), before, "the commit gone: a branch was written");

    // The name taken.
    let (repo, older) = dirty();
    let confirmed = discarding(&repo, "late", older);
    repo.git(&["branch", "late"]);
    let before = (refs(&repo), status(&repo));
    match ops::create_branch_discarding(git(), &engine(&repo), Confirmed::by_user(confirmed), None)
    {
        Err(Error::CheckoutChangedSinceConfirmed {
            what: CheckoutMoved::Name { name },
        }) => assert_eq!(name, "late"),
        other => panic!("the name taken: {other:?}"),
    }
    assert_eq!(
        (refs(&repo), status(&repo)),
        before,
        "the name taken: something was written"
    );

    // Nothing else: an edit after the press is discarded with the rest.
    let (repo, older) = dirty();
    let confirmed = discarding(&repo, "late", older);
    repo.write("g", b"edited after the press\n");
    repo.write("another.txt", b"untracked after the press\n");
    ok(
        ops::create_branch_discarding(git(), &engine(&repo), Confirmed::by_user(confirmed), None),
        "an edit after the press",
    );
    assert_eq!(
        status(&repo),
        "?? another.txt\n?? scratch.txt\n",
        "every change gone, the untracked files kept"
    );
}

/// C32 and the review's M3, dismissed against real git: a name is taken or refused as `git
/// branch -- <name>` and `git checkout -b <name>` take it. Both verbs resolve `@{-N}` to the
/// previous branch's name, as `git check-ref-format --branch` does — so it is the oracle they
/// agree with — while `git check-ref-format refs/heads/<name>`, the review's proposal, takes
/// `-x` and `HEAD`, which both verbs refuse. Cairn refuses a name holding `@{` before git is
/// asked (decision F), and every name it answers free both verbs take, every one it refuses
/// both refuse. Caught by: the oracle swapped for `refs/heads/<name>`, a taken name or a folder
/// clash passed as free, or a name the verbs take refused.
#[test]
fn a_name_is_taken_or_refused_as_gits_verbs_take_it() {
    let (repo, _) = amended();
    repo.git(&["branch", "prev"]);
    repo.git(&["checkout", "-q", "prev"]);
    repo.git(&["checkout", "-q", "main"]);
    repo.git(&["branch", "taken"]);
    repo.git(&["branch", "folder/inner"]);
    repo.git(&["branch", "leaf"]);
    let head = repo.rev("HEAD").to_string();
    let engine = engine(&repo);
    // Why `--branch` and not `refs/heads/<name>`: the verbs resolve `@{-1}` — `prev` here, the
    // branch checked out before `main` — as `--branch` does, and refuse `-x` and `HEAD` (below),
    // which `refs/heads/<name>` takes.
    let refused = repo
        .try_git(&["branch", "--", "@{-1}", &head], &[], None)
        .err()
        .unwrap_or_default();
    assert!(refused.contains("'prev' already exists"), "{refused}");
    assert_eq!(
        repo.git(&["check-ref-format", "--branch", "@{-1}"]).trim(),
        "prev"
    );
    let made = || repo.git(&["for-each-ref", "--format=%(refname)", "refs/heads/"]);
    let before = made();
    // Back on `main` only after a checkout that moved `HEAD`, so `@{-N}` still names `prev` and
    // `main` while the names holding it are asked, which come first.
    let undo = || {
        if repo.git(&["symbolic-ref", "HEAD"]).trim() != "refs/heads/main" {
            repo.git(&["checkout", "-q", "main"]);
        }
        for created in made().lines() {
            if !before.lines().any(|kept| kept == created) {
                repo.git(&["update-ref", "-d", created]);
            }
        }
    };
    // Whether each verb takes `name`, every branch it made removed again.
    let verbs_take = |name: &str| -> (bool, bool) {
        let branch = repo
            .try_git(&["branch", "--", name, &head], &[], None)
            .is_ok();
        undo();
        let checkout = repo
            .try_git(&["checkout", "-q", "-b", name, &head, "--"], &[], None)
            .is_ok();
        undo();
        (branch, checkout)
    };
    for name in [
        "@{-1}",
        "@{-2}",
        "x@{y",
        "a..b",
        "-x",
        "HEAD",
        "taken",
        "folder",
        "leaf/child",
        "topic",
        "@",
    ] {
        let answer = ok(engine.branch_name(git(), name, &CancelSignal::new()), name);
        let (branch, checkout) = verbs_take(name);
        assert_eq!(branch, checkout, "{name:?}: the verbs disagree");
        assert_eq!(
            answer == BranchName::Free,
            branch,
            "{name:?}: Cairn said {answer:?}"
        );
    }
    for name in ["-x", "HEAD"] {
        assert!(
            repo.try_git(
                &["check-ref-format", &format!("refs/heads/{name}")],
                &[],
                None
            )
            .is_ok(),
            "{name}: refs/heads/ refused it, so it is no longer the wrong oracle"
        );
        assert!(
            repo.try_git(&["check-ref-format", "--branch", name], &[], None)
                .is_err()
        );
    }
}

/// Phase 10's QA, item 15: a name a branch's directory holds (`foo` beside `foo/bar`), or one
/// under a branch's name (`baz/qux` beside `baz`), is refused before git runs, the
/// branch each clashes with named, as git's own refusal names it — git cannot lock such a ref —
/// and a name beside them is free.
/// Caught by: only `refs/heads/<name>` looked up.
#[test]
fn a_name_clashing_with_a_branchs_directory_is_refused_before_git_runs() {
    let (repo, _) = amended();
    repo.git(&["branch", "foo/bar"]);
    repo.git(&["branch", "baz"]);
    let engine = engine(&repo);
    let check = |name: &str| ok(engine.branch_name(git(), name, &CancelSignal::new()), name);
    for (name, clash) in [
        (
            "foo",
            NameRefusal::HoldsABranch {
                branch: "refs/heads/foo/bar".to_owned(),
            },
        ),
        (
            "baz/qux",
            NameRefusal::InsideABranch {
                branch: "refs/heads/baz".to_owned(),
            },
        ),
        (
            "baz/qux/deeper",
            NameRefusal::InsideABranch {
                branch: "refs/heads/baz".to_owned(),
            },
        ),
    ] {
        assert_eq!(check(name), BranchName::Refused(clash), "{name}");
        assert!(
            repo.try_git(&["branch", "--", name, "HEAD"], &[], None)
                .is_err(),
            "git took {name}"
        );
    }
    for name in ["fo", "foobar", "baz2", "foo-bar/x"] {
        assert_eq!(check(name), BranchName::Free, "{name}");
    }
}

/// The user's decision F (2026-10-09): a name holding `@{` is refused before git is asked —
/// git would read `@{-1}` as another branch's name. Caught by: the name handed to git, or a
/// name with `@` alone or `{` alone refused.
#[test]
fn a_name_holding_at_brace_is_refused_in_cairns_words() {
    let (repo, _) = amended();
    let engine = engine(&repo);
    for name in ["topic@{1}", "@{", "a@{b"] {
        assert_eq!(
            ok(engine.branch_name(git(), name, &CancelSignal::new()), name),
            BranchName::Refused(NameRefusal::AtBrace),
            "{name}"
        );
    }
    for name in ["user@host", "a{b}"] {
        assert_eq!(
            ok(engine.branch_name(git(), name, &CancelSignal::new()), name),
            BranchName::Free,
            "{name}"
        );
    }
}
