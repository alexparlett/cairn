//! staging-and-commit's `Create Branch Here…` against real `git` (R11.3, C20): the branch
//! `ops::create_branch` makes is the one `git branch -- <name> <commit>` makes — at the same
//! commit, logged with the same message — on a commit only a reflog still reaches, and a
//! name git refuses is refused with git's reason and nothing written. Run on the host's git
//! and, through `scripts/git-floor.sh`, on 2.30.9 and 2.32.7.

use cairn_git::{Error, Repository, ops};

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
