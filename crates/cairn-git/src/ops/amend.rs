//! What an amend will cost, computed from the repository as it is
//! (`docs/prd/staging-and-commit.md` R6.4, R10.6): the [`Consequence::Amend`] the commit box
//! builds its confirmation from, and the amend re-checks before it runs (R1.4).
//!
//! - **The commit** it replaces: `HEAD`'s id and its subject, as the history draws it.
//! - **Whether a remote already has it.** With an upstream — the current branch's, a
//!   remote-tracking ref that exists — whether that upstream reaches `HEAD`: its ahead
//!   count is zero (the walk `HEAD --not <upstream>` yields nothing), and the walk stops at
//!   the first commit it yields, since one is enough to say no. With none — a detached
//!   `HEAD`, a branch with no upstream, one whose upstream is gone or is a local branch —
//!   the same walk hidden by every remote-tracking ref, `HEAD --not --remotes`. Each walk is
//!   cancellable at every object it reads (`crate::history::walk::hiding`). It knows only
//!   what was last fetched, as `git branch -r --contains` does.
//! - **Whether git will write the reflog entry** that keeps the replaced commit findable
//!   ([`Reflog`]): git appends an entry for `HEAD`, and for the branch it names, when
//!   `core.logAllRefUpdates` is `true` or `always` — or, unset, unless the repository is bare
//!   (`is_bare_repository` in git, which a linked worktree of a bare repository is not) —
//!   or, whatever it is set to, when that ref's log file already exists, since git appends
//!   to a log it finds (`should_autocreate_reflog` and `log_ref_setup` in git's
//!   `refs/files-backend.c`). Only the files backend is opened (`crate::ref_storage`).
//!
//! Refused before any of it ([`Error::CommitRefused`]): an unborn branch, which has nothing
//! to amend, and any operation in progress — a merge among them, which git refuses to amend
//! in (R6.3, R6.9).

use cairn_model::{Consequence, HeadState, Oid, Publication, RefKind, RefName, Reflog, Upstream};
use gix::bstr::ByteSlice as _;

use crate::commit_encoding::CommitEncoding;
use crate::diff::git_config::{invalid, last_value, parse_bool};
use crate::history::walk;
use crate::object_id::object_id;
use crate::{Cancel, CommitRefusal, Error, Repository};

/// The `Consequence` of amending `HEAD` now (module docs). `cancel` stops the refs read and
/// the walk; a cancelled one is [`Error::RefsCancelled`] or [`Error::Cancelled`].
pub fn amend_consequence(repo: &Repository, cancel: &impl Cancel) -> Result<Consequence, Error> {
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
    let published = match upstream {
        Some((name, upstream)) => {
            if reaches(repo, &commit, [upstream], cancel)? {
                Publication::Upstream(name)
            } else {
                Publication::Unpublished
            }
        }
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
        subject: subject(repo, &commit)?,
        published,
        reflog: reflog(repo, branch.map(|branch| &branch.name))?,
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

/// `commit`'s subject as the history draws it: the summary of its message, in the characters
/// git shows.
fn subject(repo: &Repository, commit: &Oid) -> Result<String, Error> {
    let read = |source: Box<dyn std::error::Error + Send + Sync>| Error::ReadCommit {
        id: commit.to_string(),
        source,
    };
    let found = repo
        .inner()
        .find_commit(object_id(commit)?)
        .map_err(|source| read(Box::new(source)))?;
    let decoded = found.decode().map_err(|source| read(Box::new(source)))?;
    let encoding = CommitEncoding::of_commit(&found, &decoded);
    Ok(encoding.text(&decoded.message().summary()))
}

/// Whether git will log the amend in a reflog Show Lost Commits reads (module docs): `HEAD`'s
/// and, on a branch, the branch's.
fn reflog(repo: &Repository, branch: Option<&RefName>) -> Result<Reflog, Error> {
    let config = repo.inner().config_snapshot();
    let setting = last_value(config.plumbing(), "core", None, "logAllRefUpdates");
    let logs_by_default = match setting {
        None => repo.workdir().is_some(),
        Some(Some(value)) if value.eq_ignore_ascii_case(b"always") => true,
        Some(value) => parse_bool(value.as_ref().map(|value| value.as_bytes()))
            .ok_or_else(|| invalid("core.logAllRefUpdates", value))?,
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
