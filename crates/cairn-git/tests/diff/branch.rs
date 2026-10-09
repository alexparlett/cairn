//! staging-and-commit's `Create Branch Here…` against real `git` (R11.3, C20): the branch
//! `ops::create_branch` makes is the one `git branch -- <name> <commit>` makes — at the same
//! commit, logged with the same message — on a commit only a reflog still reaches, and a
//! name git refuses is refused with git's reason and nothing written. Run on the host's git
//! and, through `scripts/git-floor.sh`, on 2.30.9 and 2.32.7.

use std::path::{Path, PathBuf};

use cairn_git::{CancelSignal, CheckoutRefusal, Error, Refusal, Repository, ops};
use cairn_model::{BranchName, ChangeLoss, ChangedKind, Confirmed, Consequence, Oid};

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
        ("main", BranchName::Taken),
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
            (BranchName::Refused { reason }, Some(gits)) => assert_eq!(reason, gits, "{name:?}"),
            (BranchName::Free | BranchName::Taken, None) => {}
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

/// "Discard" (the user's decision 3, 2026-10-09): its consequence names every staged and
/// unstaged change to a tracked file — each kind as git's status reads it, its lines as git's
/// numstat counts them — every untracked file the commit's tree overwrites, and how many other
/// untracked files stay; run, it creates and checks out the branch with exactly those losses,
/// and nothing else gone. Caught by: a staged-only change or a staged new file left
/// unnamed, an overwritten untracked file unsaid, a kept untracked file lost, or the lines
/// counted otherwise than git counts them.
#[test]
fn a_discarding_checkout_names_every_loss_and_then_loses_exactly_those() {
    let (repo, older) = two_commits();
    repo.write("f", b"staged\n");
    repo.git(&["add", "f"]);
    repo.write("g", b"g one\ng two\n");
    repo.write("new.txt", b"brand new\n");
    repo.git(&["add", "new.txt"]);
    std::fs::remove_file(repo.path().join("only-later.txt")).unwrap_or_default();
    // Untracked: one the older commit's tree has no file at, one it has.
    repo.write("scratch.txt", b"keep me\n");
    let consequence = ok(
        ops::checkout_discarding_consequence(
            git(),
            &engine(&repo),
            "rescue",
            older,
            &CancelSignal::new(),
        ),
        "the consequence",
    );
    let Consequence::CheckoutDiscarding {
        branch,
        at,
        changes,
        kept_untracked,
        ..
    } = &consequence
    else {
        panic!("{consequence:?}");
    };
    assert_eq!(
        (branch.as_str(), *at, *kept_untracked),
        ("rescue", older, 1)
    );
    let named: Vec<(String, Option<ChangedKind>)> = changes
        .iter()
        .map(|change| {
            let kind = match &change.loss {
                ChangeLoss::Changed { kind, .. } => Some(*kind),
                ChangeLoss::Overwritten { .. } => None,
            };
            (change.path.to_string(), kind)
        })
        .collect();
    assert_eq!(
        named,
        [
            ("f".to_owned(), Some(ChangedKind::Modified)),
            ("g".to_owned(), Some(ChangedKind::Modified)),
            ("new.txt".to_owned(), Some(ChangedKind::Added)),
            ("only-later.txt".to_owned(), Some(ChangedKind::Deleted)),
        ]
    );
    // Lines, as git's own numstat counts staged and unstaged changes together.
    let numstat = |args: &[&str]| -> u64 {
        repo.git(args)
            .lines()
            .filter_map(|line| {
                let mut fields = line.split('\t');
                Some(fields.next()?.parse::<u64>().ok()? + fields.next()?.parse::<u64>().ok()?)
            })
            .sum()
    };
    let gits_lines = numstat(&["diff", "--cached", "--numstat"]) + numstat(&["diff", "--numstat"]);
    let ours: u64 = changes
        .iter()
        .map(|change| match &change.loss {
            ChangeLoss::Changed { lines, .. } => lines.unwrap_or_default() as u64,
            ChangeLoss::Overwritten { .. } => 0,
        })
        .sum();
    assert_eq!(ours, gits_lines, "lines counted as git counts them");

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
        repo.git(&["status", "--porcelain=v1", "-uall"]),
        "?? scratch.txt\n",
        "every change gone, the kept untracked file kept"
    );
    assert_eq!(
        std::fs::read(repo.path().join("f")).unwrap_or_default(),
        b"1\n"
    );
}

/// An untracked file at a path the commit holds a file at is named overwritten, by its size,
/// and the checkout writes the commit's file over it.
#[test]
fn an_untracked_file_in_the_way_is_named_overwritten() {
    let repo = Repo::new("overwritten");
    repo.write("f", b"1\n");
    repo.write("in-the-way.txt", b"committed\n");
    let older = repo.commit("first");
    repo.git(&["rm", "-q", "in-the-way.txt"]);
    repo.write("f", b"2\n");
    repo.commit("second");
    repo.write("in-the-way.txt", b"untracked, 21 bytes\n");
    repo.write("f", b"edited\n");
    let consequence = ok(
        ops::checkout_discarding_consequence(
            git(),
            &engine(&repo),
            "b",
            older,
            &CancelSignal::new(),
        ),
        "the consequence",
    );
    assert!(
        consequence
            .prompt()
            .contains("1 untracked file overwritten (20 bytes)"),
        "{}",
        consequence.prompt()
    );
    ok(
        ops::create_branch_discarding(git(), &engine(&repo), Confirmed::by_user(consequence), None),
        "the checkout",
    );
    assert_eq!(
        std::fs::read(repo.path().join("in-the-way.txt")).unwrap_or_default(),
        b"committed\n"
    );
}

/// R1.4: a file edited, or another deleted, after the confirmation refuses the checkout, naming
/// the path, and writes nothing — no branch, no file. Caught by: a checkout that runs on what
/// it was told then, not what is there now.
#[test]
fn a_discarding_checkout_refuses_what_changed_since_its_confirmation() {
    for late in ["edited", "deleted"] {
        let (repo, older) = two_commits();
        repo.write("f", b"edited once\n");
        let consequence = ok(
            ops::checkout_discarding_consequence(
                git(),
                &engine(&repo),
                "late",
                older,
                &CancelSignal::new(),
            ),
            "the consequence",
        );
        let path = if late == "edited" {
            repo.write("f", b"edited twice\n");
            "f"
        } else {
            std::fs::remove_file(repo.path().join("only-later.txt")).unwrap_or_default();
            repo.git(&["rm", "-q", "--cached", "only-later.txt"]);
            "only-later.txt"
        };
        let before = refs(&repo);
        match ops::create_branch_discarding(
            git(),
            &engine(&repo),
            Confirmed::by_user(consequence),
            None,
        ) {
            Err(Error::ChangedSinceConfirmed { path: named }) => assert_eq!(named, path, "{late}"),
            other => panic!("{late}: {other:?}"),
        }
        assert_eq!(refs(&repo), before, "{late}: a branch was written");
    }
}

/// The discard is refused before any confirmation where it cannot count what it loses: during
/// a merge (git's `MERGE_HEAD`), for a conflicted path, for a submodule's change, and when
/// nothing is changed. Caught by: a forced checkout offered over an operation's state, a
/// submodule's inside, or nothing at all.
#[test]
fn a_discarding_checkout_is_refused_where_it_cannot_count_the_loss() {
    let consequence = |repo: &Repo, at: Oid| {
        ops::checkout_discarding_consequence(git(), &engine(repo), "x", at, &CancelSignal::new())
    };
    let (clean, older) = two_commits();
    assert!(matches!(
        consequence(&clean, older),
        Err(Error::CheckoutRefused {
            why: CheckoutRefusal::NothingToDiscard
        })
    ));

    let (merging, older) = two_commits();
    merging.write("f", b"dirty\n");
    let head = merging.rev("HEAD");
    ok(
        std::fs::write(merging.path().join(".git/MERGE_HEAD"), format!("{head}\n")),
        "writing MERGE_HEAD",
    );
    assert!(matches!(
        consequence(&merging, older),
        Err(Error::CheckoutRefused {
            why: CheckoutRefusal::InProgress(_)
        })
    ));

    let (submodule, older) = two_commits();
    let head = submodule.rev("HEAD").to_string();
    submodule.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{head},sub"),
    ]);
    assert!(matches!(
        consequence(&submodule, older),
        Err(Error::Refused {
            why: Refusal::Submodule,
            ..
        })
    ));
}
