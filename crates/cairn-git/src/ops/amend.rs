//! What an amend will cost, computed from the repository as it is
//! (`docs/prd/staging-and-commit.md` R6.4, R10.6): the [`Consequence::Amend`] an amend is
//! confirmed with when it must be, and re-checked against before a confirmed one runs (R1.4).
//! It is read when Amend is pressed, in the job that then runs the amend — never on a refresh —
//! and holds only what the amend's freshness rests on, no display field (`review-code-engine.md`
//! M2).
//!
//! - **The commit** it replaces: `HEAD`'s id.
//! - **Whether a remote already has it.** With an upstream — the current branch's, a
//!   remote-tracking ref that exists — whether that upstream reaches `HEAD`: its ahead
//!   count is zero (the walk `HEAD --not <upstream>` yields nothing), and the walk stops at
//!   the first commit it yields, since one is enough to say no. When it does not — or with
//!   none: a detached `HEAD`, a branch with no upstream, one whose upstream is gone or is a
//!   local branch — the same walk hidden by every remote-tracking ref, `HEAD --not
//!   --remotes`, so `Publication::Unpublished` means what it says: no remote-tracking ref
//!   reaches it. Each walk is cancellable at every object it reads
//!   (`crate::history::walk::hiding`) — or, where the commit-graph holds `HEAD` and every
//!   tip it is hidden by, answered from the graph alone, cut at `HEAD`'s generation as git's
//!   own check is (`crate::history::walk::reaches_through_graph`), cancellable at every
//!   commit. It knows only what was last fetched, as `git branch -r --contains` does.
//! - **Whether git will write the reflog entry** that keeps the replaced commit findable
//!   ([`Reflog`]): git appends an entry for `HEAD`, and for the branch it names, when
//!   `core.logAllRefUpdates` is `true` or `always` — or, unset, unless the repository is bare
//!   (`is_bare_repository` in git, which a linked worktree of a bare repository is not) —
//!   or, whatever it is set to, when that ref's log file already exists, since git appends
//!   to a log it finds (`should_autocreate_reflog` and `log_ref_setup` in git's
//!   `refs/files-backend.c`). The setting is git's answer, `git config` in query form
//!   (`crate::reads::log_all_ref_updates`, R6.11), so a linked worktree's `includeIf`, the
//!   system file and trust are read as the amend's own git reads them; gix's configuration is
//!   not asked. Only the files backend is opened (`crate::ref_storage`).
//!
//! An amend git logs and no remote has is recoverable — Show Lost Commits finds the replaced
//! commit in the reflog — so it runs at once, unconfirmed ([`super::amend_unconfirmed`], R1.5);
//! any other is confirmed first ([`Consequence::needs_confirming`]) and run by
//! [`super::amend`] with the token.
//!
//! Refused before any of it ([`Error::CommitRefused`]): an unborn branch, which has nothing
//! to amend, and any operation in progress — a merge among them, which git refuses to amend
//! in (R6.3, R6.9).

use cairn_model::{Consequence, HeadState, Oid, Publication, RefKind, RefName, Reflog, Upstream};
use gix::bstr::ByteSlice as _;

use super::GitBinary;
use crate::history::walk;
use crate::object_id::object_id;
use crate::reads::LogRefUpdates;
use crate::{Cancel, CommitRefusal, Error, Repository};

/// The `Consequence` of amending `HEAD` now (module docs). `cancel` stops the refs read, the
/// walk and the configuration read; a cancelled one is [`Error::RefsCancelled`],
/// [`Error::Cancelled`] or [`Error::GitReadCancelled`].
pub fn amend_consequence(
    git: &GitBinary,
    repo: &Repository,
    cancel: &impl Cancel,
) -> Result<Consequence, Error> {
    if let Some(operation) = repo.operation_in_progress()
        && operation.refuses_amend()
    {
        return Err(Error::CommitRefused {
            why: CommitRefusal::InProgress(operation),
        });
    }
    let snapshot = repo.refs(cancel)?.snapshot;
    let (commit, branch) = match &snapshot.head {
        HeadState::Unborn(_) => {
            return Err(Error::CommitRefused {
                why: CommitRefusal::NothingToAmend,
            });
        }
        HeadState::Detached(id) => (*id, None),
        HeadState::Branch(name) => match snapshot.find(name).and_then(|found| found.commit_id()) {
            Some(id) => (id, snapshot.find(name)),
            None => {
                return Err(Error::CommitRefused {
                    why: CommitRefusal::NothingToAmend,
                });
            }
        },
    };
    let upstream = branch.and_then(|branch| match &branch.upstream {
        Some(Upstream::Exists {
            name,
            commit: Some(upstream),
        }) if name.as_str().starts_with("refs/remotes/") => Some((name.clone(), *upstream)),
        Some(Upstream::Exists { .. } | Upstream::Gone { .. }) | None => None,
    });
    let held_by_upstream = match &upstream {
        Some((name, upstream)) => {
            reaches(repo, &commit, [*upstream], cancel)?.then(|| name.clone())
        }
        None => None,
    };
    let published = match held_by_upstream {
        Some(name) => Publication::Upstream(name),
        None => {
            let remotes: Vec<Oid> = snapshot
                .of_kind(RefKind::RemoteTracking)
                .filter_map(|remote| remote.commit_id())
                .collect();
            if reaches(repo, &commit, remotes, cancel)? {
                Publication::SomeRemote
            } else {
                Publication::Unpublished
            }
        }
    };
    Ok(Consequence::Amend {
        commit,
        published,
        reflog: reflog(git, repo, branch.map(|branch| &branch.name), cancel)?,
    })
}

/// Whether any of `hidden` reaches `commit`: whether the walk `commit --not <hidden>` yields
/// nothing. Stops at the first commit it yields.
fn reaches(
    repo: &Repository,
    commit: &Oid,
    hidden: impl IntoIterator<Item = Oid>,
    cancel: &impl Cancel,
) -> Result<bool, Error> {
    let hidden = hidden
        .into_iter()
        .map(|id| object_id(&id))
        .collect::<Result<Vec<_>, _>>()?;
    if hidden.is_empty() {
        return Ok(false);
    }
    let tip = object_id(commit)?;
    if hidden.contains(&tip) {
        return Ok(true);
    }
    let reads = std::cell::Cell::new(0);
    // With a commit-graph holding them all, the graph answers alone, cut at `HEAD`'s
    // generation as git cuts it; otherwise the object walk does.
    if let Some(answer) = walk::reaches_through_graph(repo.inner(), tip, &hidden, cancel, &reads) {
        return answer;
    }
    let mut walk = walk::hiding(repo.inner(), tip, hidden, cancel, &reads)?;
    match walk.next() {
        None => Ok(true),
        Some(Ok(_)) => Ok(false),
        Some(Err(_)) if cancel.is_cancelled() => Err(Error::Cancelled {
            walked: reads.get(),
        }),
        Some(Err(source)) => Err(Error::Walk {
            source: Box::new(source),
        }),
    }
}

/// Whether git will log the amend in a reflog Show Lost Commits reads (module docs): `HEAD`'s
/// and, on a branch, the branch's.
fn reflog(
    git: &GitBinary,
    repo: &Repository,
    branch: Option<&RefName>,
    cancel: &impl Cancel,
) -> Result<Reflog, Error> {
    let logs_by_default = match crate::reads::log_all_ref_updates(git, repo, cancel)? {
        None => repo.workdir().is_some(),
        Some(LogRefUpdates::Normal | LogRefUpdates::Always) => true,
        Some(LogRefUpdates::None) => false,
    };
    let head_log = repo.git_dir().join("logs").join("HEAD");
    let branch_log = branch.map(|branch| {
        repo.inner()
            .common_dir()
            .join("logs")
            .join(gix::path::from_bstr(branch.as_str().as_bytes().as_bstr()))
    });
    let written = logs_by_default
        || head_log.is_file()
        || branch_log.as_deref().is_some_and(std::path::Path::is_file);
    Ok(if written {
        Reflog::Written
    } else {
        Reflog::NotWritten
    })
}
