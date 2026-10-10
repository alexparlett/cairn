//! Create Branch's checkout (`docs/prd/staging-and-commit.md` R11.3): a branch created at a
//! commit and checked out, with the working tree's changes kept — "Don't change", `git checkout
//! -q -b <name> <id> --`, which git refuses, writing nothing, where a change or an untracked file
//! would be overwritten — or discarded — "Discard", Fork's forced checkout, `git checkout -q
//! --no-track -f -b <name> <id> --` (Fork's command, observed in its Activity Manager, with
//! Cairn's `-q` and `--`; the user's decision of 2026-10-10). Discard is destructive and sealed:
//! its [`Consequence::CheckoutDiscarding`] is fixed and generic — the branch, the commit and
//! `HEAD` — predicting nothing of what git deletes (the redesign's rule 7: git decides), and the
//! Create Branch dialog's press with Discard chosen is its confirmation. Its re-check is that
//! `HEAD`, the commit and the name are unchanged, and nothing else (R1.4). It is the one
//! operation that discards a staged change (R1.5's stated exception).
//!
//! What git does with it, on 2.56.0, 2.30.9 and 2.32.7 alike
//! (`docs/research/staging-and-commit/create-branch-discard-probe-2026-10-10.md`): staged and
//! unstaged changes and a staged new file are gone, an untracked file at a path the commit holds
//! is overwritten, other untracked files are kept; a conflicted path is discarded; a
//! submodule's change is left in place (its checkout, the edits inside it, and a staged change
//! of its commit listed again as unstaged), as Fork's command leaves it. During an operation —
//! a merge, a rebase, a cherry-pick, a revert, `git am` — git would abandon it without a word (a
//! merge's `MERGE_HEAD` and `MERGE_MSG` removed), so an operation in progress is refused before
//! git runs, when the consequence is read and again before the run.
//!
//! The name is `-b`'s value, which git reads as the name whatever it begins with; the commit is
//! its full id, and the `--` after it says it is no path. Checking out is this packet's only
//! through Create Branch; checking out a branch is branch-ops'.

use std::ffi::OsString;

use cairn_model::{AskpassToken, BranchName, Confirmed, Consequence, Oid};

use super::local_write::{locks_around, locks_now, run};
use super::{GitBinary, Invalidated, Performed};
use crate::object_id::{model_id, object_id};
use crate::{CancelSignal, CheckoutMoved, CheckoutRefusal, Error, Repository};

/// Creates the branch `name` at `commit` and checks it out, keeping the working tree's changes:
/// `git checkout -q -b <name> <id> --`, a write. git refuses — creating no branch — where a
/// change or an untracked file would be overwritten, and its words are the failure's
/// ([`Error::GitFailed`]). Invalidates the refs, the index and the working tree.
pub fn create_branch_and_checkout(
    git: &GitBinary,
    repo: &Repository,
    name: &str,
    commit: Oid,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let before = locks_now(repo);
    run(
        git,
        repo,
        token,
        &checkout_arguments(name, commit, false),
        None,
    )?;
    Ok(Performed::new(
        format!(
            "created and checked out branch {name} at {}",
            commit.short()
        ),
        checkout_invalidates(),
    )
    .with_locks(locks_around(repo, before)))
}

/// What creating `branch` at `at` and checking it out with Fork's forced checkout would be
/// confirmed as, read now: the branch, the commit and `HEAD`, nothing predicted of what git
/// deletes. Refused before any confirmation during a merge, a rebase, `git am`, a cherry-pick or
/// a revert ([`CheckoutRefusal::InProgress`]), which git would abandon; and when `at` cannot be
/// read. Reads no working tree and runs no `git`.
pub fn checkout_discarding_consequence(
    repo: &Repository,
    branch: &str,
    at: Oid,
) -> Result<Consequence, Error> {
    if let Some(operation) = repo.operation_in_progress() {
        return Err(Error::CheckoutRefused {
            why: CheckoutRefusal::InProgress(operation),
        });
    }
    let head = head_commit(repo)?;
    commit_is_there(repo, at)?;
    Ok(Consequence::CheckoutDiscarding {
        branch: branch.to_owned(),
        at,
        head,
    })
}

/// Creates the branch the confirmation names at the commit it names and checks it out with
/// `git checkout -q --no-track -f -b <name> <id> --`, discarding local changes and any untracked
/// files in the way as git does. Immediately before, it re-checks exactly what the confirmation
/// holds (R1.4) — `HEAD` is the commit it was, the commit can still be read, and the name is
/// still one git takes and no branch has — refusing with [`Error::CheckoutChangedSinceConfirmed`],
/// writing nothing, when any moved; nothing else is compared, so an edit made since is discarded
/// with the rest. An operation begun since is refused as at the confirmation, read last before
/// git runs. Every target is
/// read from the confirmation, never from a parameter beside it. Invalidates the refs, the index
/// and the working tree.
pub fn create_branch_discarding(
    git: &GitBinary,
    repo: &Repository,
    confirmed: Confirmed,
    token: Option<&AskpassToken>,
) -> Result<Performed, Error> {
    let (branch, at, head) = match confirmed.consequence() {
        Consequence::CheckoutDiscarding { branch, at, head } => (branch.clone(), *at, *head),
        Consequence::DiscardLines { .. }
        | Consequence::DiscardFiles { .. }
        | Consequence::Amend { .. }
        | Consequence::RemoveLock { .. } => {
            return Err(Error::CheckoutRefused {
                why: CheckoutRefusal::NotWhatWasConfirmed,
            });
        }
    };
    let moved = |what| Err(Error::CheckoutChangedSinceConfirmed { what });
    if head_commit(repo)? != head {
        return moved(CheckoutMoved::Head);
    }
    if commit_is_there(repo, at).is_err() {
        return moved(CheckoutMoved::Commit { at });
    }
    match repo.branch_name(git, &branch, &CancelSignal::new())? {
        BranchName::Free => {}
        BranchName::Refused(_) => return moved(CheckoutMoved::Name { name: branch }),
    }
    // Last before the run, after the name's re-check ran its `git` (phase 14's QA, DO#2): an
    // operation begun in a terminal since is refused, never abandoned by `-f` unseen. One begun
    // between this read and git's own start is not seen — the residual race
    // `docs/systems/staging.md` states.
    if let Some(operation) = repo.operation_in_progress() {
        return Err(Error::CheckoutRefused {
            why: CheckoutRefusal::InProgress(operation),
        });
    }
    let before = locks_now(repo);
    run(
        git,
        repo,
        token,
        &checkout_arguments(&branch, at, true),
        None,
    )?;
    Ok(Performed::destructive(
        format!(
            "created and checked out branch {branch} at {}, discarding the changes",
            at.short()
        ),
        confirmed,
        checkout_invalidates(),
    )
    .with_locks(locks_around(repo, before)))
}

/// `checkout -q -b <name> <id> --`, or, discarding, Fork's `checkout -q --no-track -f -b <name>
/// <id> --`.
fn checkout_arguments(name: &str, commit: Oid, discarding: bool) -> Vec<OsString> {
    let mut arguments = vec![OsString::from("checkout"), OsString::from("-q")];
    if discarding {
        arguments.extend([OsString::from("--no-track"), OsString::from("-f")]);
    }
    arguments.extend([
        OsString::from("-b"),
        OsString::from(name),
        OsString::from(commit.to_string()),
        OsString::from("--"),
    ]);
    arguments
}

fn checkout_invalidates() -> Invalidated {
    Invalidated::refs()
        .and(Invalidated::index())
        .and(Invalidated::working_tree())
}

/// `HEAD`'s commit; `None` on an unborn branch.
fn head_commit(repo: &Repository) -> Result<Option<Oid>, Error> {
    match repo.inner().head_id() {
        Ok(id) => Ok(Some(model_id(&id)?)),
        Err(_) => Ok(None),
    }
}

/// Whether `at` is a commit the repository can read.
fn commit_is_there(repo: &Repository, at: Oid) -> Result<(), Error> {
    repo.inner()
        .find_commit(object_id(&at)?)
        .map(|_| ())
        .map_err(|source| Error::ReadCommit {
            id: at.to_string(),
            source: Box::new(source),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::recording_stub::RecordingStub;

    /// R11.3's argv, kept and discarding: `checkout -q -b`, Fork's `--no-track -f` only when
    /// discarding (C34), the name as `-b`'s value, the commit by its full id, `--` after it; a
    /// write's environment. Caught by: `-f` on the kept checkout, `--no-track` dropped (Fork's
    /// argv, held literal here: from a full id git sets no upstream either way, so no effect
    /// test can see it), a name or a commit read as a path, a short id.
    #[test]
    fn the_checkouts_run_as_r11_names_them() {
        let id = Oid::parse("07da224c7ec04501dfb451be161fa962effe1dc1").unwrap();
        let strings = |arguments: Vec<OsString>| -> Vec<String> {
            arguments
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            strings(checkout_arguments("topic", id, false)),
            [
                "checkout",
                "-q",
                "-b",
                "topic",
                id.to_string().as_str(),
                "--"
            ]
        );
        assert_eq!(
            strings(checkout_arguments("-x", id, true)),
            [
                "checkout",
                "-q",
                "--no-track",
                "-f",
                "-b",
                "-x",
                id.to_string().as_str(),
                "--"
            ]
        );
        let stub = RecordingStub::new();
        let (git, repo) = (stub.git_binary(), stub.repository());
        let head = model_id(&repo.inner().head_id().unwrap()).unwrap();
        // The stub's working tree has a staged and an unstaged edit on file.txt: git refuses
        // nothing here, since the checkout is of `HEAD` itself.
        create_branch_and_checkout(&git, &repo, "kept", head, None).unwrap();
        let recorded = stub.recorded();
        let checkout = recorded
            .iter()
            .find(|record| {
                record
                    .arguments_after_location()
                    .contains(&"checkout".to_owned())
            })
            .unwrap();
        assert_eq!(
            checkout.arguments_after_location(),
            [
                "--literal-pathspecs",
                "checkout",
                "-q",
                "-b",
                "kept",
                head.to_string().as_str(),
                "--"
            ]
        );
        checkout.assert_a_write();

        // Discard, as the stub git records it (C34): Fork's command with Cairn's `-q` and `--`.
        let consequence = checkout_discarding_consequence(&repo, "discarded", head).unwrap();
        create_branch_discarding(&git, &repo, Confirmed::by_user(consequence), None).unwrap();
        let recorded = stub.recorded();
        let discarding: Vec<_> = recorded
            .iter()
            .filter(|record| {
                record
                    .arguments_after_location()
                    .contains(&"discarded".to_owned())
                    && record
                        .arguments_after_location()
                        .contains(&"checkout".to_owned())
            })
            .collect();
        assert_eq!(discarding.len(), 1, "one forced checkout");
        let forced = discarding.first().unwrap();
        assert_eq!(
            forced.arguments_after_location(),
            [
                "--literal-pathspecs",
                "checkout",
                "-q",
                "--no-track",
                "-f",
                "-b",
                "discarded",
                head.to_string().as_str(),
                "--"
            ]
        );
        forced.assert_a_write();
    }

    /// Phase 14's QA (DO#2): the in-progress check runs after the name's re-check, whose `git
    /// check-ref-format` is a process an operation could begin during — so a merge begun while
    /// the re-check ran is refused, the forced checkout never run and the merge left. The
    /// `git` here begins one (writes `MERGE_HEAD`) when asked `check-ref-format`, then runs the
    /// real git. Caught by: the in-progress check made before the name's re-check.
    #[test]
    fn an_operation_begun_during_the_recheck_is_refused_before_git_runs() {
        let fixture = RecordingStub::new();
        let repo = fixture.repository();
        let head = model_id(&repo.inner().head_id().unwrap()).unwrap();
        let program = GitBinary::discover(&super::super::Askpass::new(
            crate::process::stub_git::StubGit::HELPER,
            None,
        ))
        .unwrap()
        .path()
        .to_owned();
        let stub = crate::process::stub_git::StubGit::with_git(&format!(
            "dir=\n\
             for a in \"$@\"; do case \"$a\" in --git-dir=*) dir=\"${{a#--git-dir=}}\";; esac; done\n\
             case \" $* \" in *\" check-ref-format \"*) printf '%s\\n' {head} > \"$dir/MERGE_HEAD\";; esac\n\
             exec '{git}' \"$@\"",
            git = program.display(),
        ));
        let git = crate::process::stub_git::discover_retrying(stub.environment()).unwrap();
        let consequence = checkout_discarding_consequence(&repo, "late", head).unwrap();
        assert!(!repo.git_dir().join("MERGE_HEAD").exists());
        let refused = create_branch_discarding(&git, &repo, Confirmed::by_user(consequence), None);
        assert!(
            matches!(
                refused,
                Err(Error::CheckoutRefused {
                    why: CheckoutRefusal::InProgress(cairn_model::OperationInProgress::Merge)
                })
            ),
            "{refused:?}"
        );
        assert!(
            repo.git_dir().join("MERGE_HEAD").exists(),
            "the merge abandoned"
        );
        assert!(
            repo.inner()
                .try_find_reference("refs/heads/late")
                .unwrap()
                .is_none(),
            "a branch was written"
        );
    }
}
