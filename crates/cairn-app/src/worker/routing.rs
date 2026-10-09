//! The routing table: which thread serves each request (PRD R4.2, packet decision L8).
//!
//! | Lane or request | Thread |
//! | --- | --- |
//! | history (`OpenHistory`, `MoreHistory`, `FindRow`, `StopFinding`) | `cairn-repository`, which owns the live walk |
//! | changes (`Changes`) | `cairn-diff` |
//! | file diff (`FileDiff`, `Expand`) | `cairn-diff` |
//! | `ConfiguredContext` | `cairn-diff`, whose handle is opened again when the configuration moves, and which sends the context again each time |
//! | `ListRemotes`, `CommandLog`, `Close`, `Fetch` | `cairn-repository` (a fetch is forwarded on to the network lane) |
//! | `Retire` | `cairn-repository`, which frees what it is handed |
//! | file filter (`FilterFiles`) | `cairn-repository`, whose pages are bounded, so a keystroke never waits behind a diff |
//! | refs (`Refresh`'s first read) | `cairn-repository`, which reopens the walk from them (refs-and-status R11.2) |
//! | ahead/behind and status (`Refresh`'s other two) | `cairn-refresh`, so neither a page nor a diff queues behind a slow status or a long divergence; ahead/behind is forwarded there by the repository thread once the refs it counts are read |
//! | ref filter (`FilterRefs`) | `cairn-repository`, as the file filter |
//! | Local Changes' filter (`FilterLocalChanges`) | `cairn-repository`, as the file filter |
//! | `CancelFetch` | none: the fetch's control, from the caller's thread |
//! | `Write` | `cairn-local`, the local write lane, reached directly, so a write never waits behind a page or a find (staging-and-commit R4.1) |
//! | `DiscardConsequence` | `cairn-local`, in the lane's order, so what a discard would lose is counted after the writes asked before it |
//! | `CancelWrite` | none: the local lane's state, from the caller's thread |
//! | `RefreshStatus` | `cairn-refresh`, as a refresh's status |
//!
//! [`route`] is the table, applied to every request as it is submitted: it hands each one
//! to its thread as that thread's own job type, so a thread is never sent work it does not
//! serve. [`thread_of`] states the lanes' half of it as decided, and a test holds `route`
//! to it.
//!
//! The live walk borrows the repository thread's handle across turns and is not `Send`,
//! so the history lane stays there; diffs have a thread of their own, so a long page never
//! queues a diff behind it and a diff never queues a page. Routing happens as a request is
//! submitted, on the caller's thread, so neither thread forwards the other's work: a diff
//! asked while a page is walked reaches the diff thread at once.

#[cfg(test)]
use super::epoch::QueryLane;
use std::sync::Arc;

use cairn_model::{ChangeSet, Disclosure, LocalChanges, Oid, RefsSnapshot};

use super::epoch::Epoch;
use super::local_lane::{LocalWrite, OperationId};

use super::request::{Comparison, DiffQuery, FileQuery, Request, Retired};

/// A thread a repository's requests are served on.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Thread {
    /// `cairn-repository`: the history walk, the operations' front door, and the close.
    Repository,
    /// `cairn-diff`: the changes and file-diff lanes, commits and working tree alike.
    Diff,
    /// `cairn-refresh`: status and ahead/behind.
    Refresh,
    /// `cairn-local`: the local write lane.
    Local,
}

/// The thread each query lane is served on: the table above, for the lanes.
#[cfg(test)]
pub(super) const fn thread_of(lane: QueryLane) -> Thread {
    match lane {
        QueryLane::History
        | QueryLane::Walk
        | QueryLane::FileFilter
        | QueryLane::Refs
        | QueryLane::RefFilter
        | QueryLane::LocalChangesFilter => Thread::Repository,
        QueryLane::Changes | QueryLane::FileDiff => Thread::Diff,
        QueryLane::AheadBehind | QueryLane::Status => Thread::Refresh,
    }
}

/// Which page of the history walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    /// A new walk from every ref, dropping any walk open; `walk` is the walk lane's number
    /// the open was given, which every page of the walk is answered under.
    Open { rows: usize, walk: Epoch },
    /// The next rows of the walk open, or a cold restart from the last good page.
    More { rows: usize },
    /// Pages of the walk open until one holds `target`'s row, or the walk ends.
    Find { target: Oid, rows: usize },
    /// Nothing: its number superseded the find in flight, which is all it is for.
    Stop,
}

/// What the repository thread is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RepositoryJob {
    History(Page),
    ListRemotes,
    Fetch {
        remote: String,
    },
    CommandLog,
    /// Which files of a change set a filter's text leaves.
    Filter {
        of: Comparison,
        files: Arc<ChangeSet>,
        text: String,
    },
    /// The refs read again, a refresh's first read; its epoch is the refs lane's, and
    /// `ahead_behind` the epoch the ahead/behind it forwards is answered under.
    Refs {
        ahead_behind: Epoch,
    },
    /// The sidebar's rows for a snapshot, a filter's text and what is open.
    FilterRefs {
        refs: Arc<RefsSnapshot>,
        text: String,
        disclosure: Arc<Disclosure>,
    },
    /// Which rows of Local Changes' two lists a filter's text leaves.
    FilterLocalChanges {
        changes: Arc<LocalChanges>,
        text: String,
    },
    /// Answers to free; the job does nothing else.
    Retire(Retired),
    Close,
}

/// Where a request goes. Not `Clone`, as a request is not.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Routed {
    Repository(RepositoryJob),
    Diff(DiffQuery),
    /// `diff.context`, read on the diff thread — the one whose handle follows the
    /// configuration — now and again whenever that handle is opened afresh. Not a query:
    /// numbered in no lane, superseding nothing.
    ConfiguredContext,
    /// Never queued: it reaches the fetch directly, ahead of any page.
    CancelFetch,
    /// A refresh: its refs to the repository thread ([`RepositoryJob::Refs`]), its status to
    /// the refresh thread — each numbered by `submit`, which alone holds the epochs.
    Refresh,
    /// A new walk, to the repository thread as [`Page::Open`] with the walk lane's number
    /// `submit` gave it.
    OpenHistory {
        rows: usize,
    },
    /// A local write, straight to the local lane.
    Write {
        id: OperationId,
        write: LocalWrite,
    },
    /// What a discard would lose, to the local lane behind the writes asked before it.
    DiscardConsequence {
        asked: OperationId,
        paths: Vec<cairn_model::RepoPath>,
    },
    /// Never queued: it reaches the local lane's state directly.
    CancelWrite(OperationId),
    /// A status alone, to the refresh thread under the status lane's number.
    RefreshStatus,
}

#[cfg(test)]
impl Routed {
    /// The thread this is served on; `None` for what no thread queues.
    pub(super) fn thread(&self) -> Option<Thread> {
        match self {
            Self::Repository(_) | Self::OpenHistory { .. } => Some(Thread::Repository),
            Self::Diff(_) | Self::ConfiguredContext => Some(Thread::Diff),
            Self::Write { .. } | Self::DiscardConsequence { .. } => Some(Thread::Local),
            Self::RefreshStatus => Some(Thread::Refresh),
            Self::CancelFetch | Self::CancelWrite(_) => None,
            // Its refs': see `lane_thread` for its ahead/behind. Its status, numbered in no
            // lane of the refresh's, goes to the refresh thread too.
            Self::Refresh => Some(Thread::Repository),
        }
    }

    /// The thread `lane`'s query of this is served on: for a refresh, each of the two reads it
    /// is numbered for (its refs and ahead/behind).
    pub(super) fn lane_thread(&self, lane: QueryLane) -> Option<Thread> {
        match (self, lane) {
            (Self::Refresh, QueryLane::AheadBehind) => Some(Thread::Refresh),
            _ => self.thread(),
        }
    }
}

/// Routes `request`. Total, with no arm that defaults: a new request does not compile
/// until it is given a thread.
pub(super) fn route(request: Request) -> Routed {
    match request {
        Request::OpenHistory { rows } => Routed::OpenHistory { rows },
        Request::MoreHistory { rows } => {
            Routed::Repository(RepositoryJob::History(Page::More { rows }))
        }
        Request::FindRow { target, rows } => {
            Routed::Repository(RepositoryJob::History(Page::Find { target, rows }))
        }
        Request::StopFinding => Routed::Repository(RepositoryJob::History(Page::Stop)),
        Request::Changes { of } => Routed::Diff(DiffQuery::Changes(of)),
        Request::FileDiff(FileQuery { target, options }) => {
            Routed::Diff(DiffQuery::File(FileQuery { target, options }))
        }
        Request::Expand(asked) => Routed::Diff(DiffQuery::Expand(asked)),
        Request::FilterFiles { of, files, text } => {
            Routed::Repository(RepositoryJob::Filter { of, files, text })
        }
        Request::Refresh => Routed::Refresh,
        Request::FilterRefs {
            refs,
            text,
            disclosure,
        } => Routed::Repository(RepositoryJob::FilterRefs {
            refs,
            text,
            disclosure,
        }),
        Request::FilterLocalChanges { changes, text } => {
            Routed::Repository(RepositoryJob::FilterLocalChanges { changes, text })
        }
        Request::ListRemotes => Routed::Repository(RepositoryJob::ListRemotes),
        Request::ConfiguredContext => Routed::ConfiguredContext,
        Request::Fetch { remote } => Routed::Repository(RepositoryJob::Fetch { remote }),
        Request::CommandLog => Routed::Repository(RepositoryJob::CommandLog),
        Request::Retire(retired) => Routed::Repository(RepositoryJob::Retire(retired)),
        Request::Close => Routed::Repository(RepositoryJob::Close),
        Request::CancelFetch => Routed::CancelFetch,
        Request::Write { id, write } => Routed::Write { id, write },
        Request::DiscardConsequence { asked, paths } => Routed::DiscardConsequence { asked, paths },
        Request::CancelWrite { id } => Routed::CancelWrite(id),
        Request::RefreshStatus => Routed::RefreshStatus,
    }
}

/// The request a routed job came from: the inverse of [`route`], for a test of the
/// window's side that reads back what it asked.
#[cfg(test)]
pub(super) fn unroute(routed: Routed) -> Request {
    match routed {
        Routed::Repository(RepositoryJob::History(Page::Open { rows, .. }))
        | Routed::OpenHistory { rows } => Request::OpenHistory { rows },
        Routed::Repository(RepositoryJob::History(Page::More { rows })) => {
            Request::MoreHistory { rows }
        }
        Routed::Repository(RepositoryJob::History(Page::Find { target, rows })) => {
            Request::FindRow { target, rows }
        }
        Routed::Repository(RepositoryJob::History(Page::Stop)) => Request::StopFinding,
        Routed::Repository(RepositoryJob::ListRemotes) => Request::ListRemotes,
        Routed::Repository(RepositoryJob::Fetch { remote }) => Request::Fetch { remote },
        Routed::Repository(RepositoryJob::CommandLog) => Request::CommandLog,
        Routed::Repository(RepositoryJob::Filter { of, files, text }) => {
            Request::FilterFiles { of, files, text }
        }
        Routed::Repository(RepositoryJob::Retire(retired)) => Request::Retire(retired),
        Routed::Repository(RepositoryJob::Refs { .. }) | Routed::Refresh => Request::Refresh,
        Routed::Repository(RepositoryJob::FilterRefs {
            refs,
            text,
            disclosure,
        }) => Request::FilterRefs {
            refs,
            text,
            disclosure,
        },
        Routed::Repository(RepositoryJob::FilterLocalChanges { changes, text }) => {
            Request::FilterLocalChanges { changes, text }
        }
        Routed::Repository(RepositoryJob::Close) => Request::Close,
        Routed::Diff(DiffQuery::Changes(of)) => Request::Changes { of },
        Routed::Diff(DiffQuery::File(query)) => Request::FileDiff(query),
        Routed::Diff(DiffQuery::Expand(asked)) => Request::Expand(asked),
        Routed::ConfiguredContext => Request::ConfiguredContext,
        Routed::CancelFetch => Request::CancelFetch,
        Routed::Write { id, write } => Request::Write { id, write },
        Routed::DiscardConsequence { asked, paths } => Request::DiscardConsequence { asked, paths },
        Routed::CancelWrite(id) => Request::CancelWrite { id },
        Routed::RefreshStatus => Request::RefreshStatus,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::request::{Comparison, DiffOptions, FileTarget, WorkingSide};

    fn every_request() -> Vec<Request> {
        let commit = Comparison::Commit(cairn_model::Oid::from_bytes(&[7; 20]).unwrap());
        vec![
            Request::OpenHistory { rows: 3 },
            Request::MoreHistory { rows: 3 },
            Request::FindRow {
                target: cairn_model::Oid::from_bytes(&[8; 20]).unwrap(),
                rows: 3,
            },
            Request::StopFinding,
            Request::Changes { of: commit },
            Request::FileDiff(FileQuery {
                target: FileTarget::WorkingTree {
                    path: cairn_model::RepoPath::from("a"),
                    side: WorkingSide::Staged,
                },
                options: DiffOptions::default(),
            }),
            Request::Expand(crate::worker::request::ExpandQuery {
                of: commit,
                changes: Arc::new(cairn_model::ChangeSet {
                    files: Vec::new(),
                    details: None,
                    renames: cairn_model::RenameDetection::default(),
                }),
                options: DiffOptions::default(),
                files: vec![crate::worker::request::OpenedFile {
                    index: 0,
                    load_anyway: true,
                }],
                all: Some(crate::worker::request::AllFrom { next: 3, spent: 9 }),
                kept_open: vec![0],
            }),
            Request::FilterFiles {
                of: commit,
                files: Arc::new(cairn_model::ChangeSet {
                    files: Vec::new(),
                    details: None,
                    renames: cairn_model::RenameDetection::default(),
                }),
                text: "lib".to_owned(),
            },
            Request::FilterLocalChanges {
                changes: Arc::new(cairn_model::LocalChanges::new(
                    cairn_model::WorkingTreeStatus::Listed(vec![
                        cairn_model::StatusEntry::Untracked(cairn_model::RepoPath::from("a")),
                    ]),
                )),
                text: "a".to_owned(),
            },
            Request::Refresh,
            Request::FilterRefs {
                refs: Arc::new(cairn_model::RefsSnapshot {
                    refs: Vec::new(),
                    head: cairn_model::HeadState::Unborn(cairn_model::RefName::new(
                        "refs/heads/main",
                    )),
                    stashes: Vec::new(),
                    unreadable: 0,
                }),
                text: "main".to_owned(),
                disclosure: Arc::new(cairn_model::Disclosure::default()),
            },
            Request::ListRemotes,
            Request::ConfiguredContext,
            Request::Fetch {
                remote: "origin".to_owned(),
            },
            Request::CancelFetch,
            Request::CommandLog,
            Request::Write {
                id: OperationId::for_tests(1),
                write: crate::worker::LocalWrite::StageFiles {
                    paths: vec![cairn_model::RepoPath::from("a")],
                },
            },
            Request::DiscardConsequence {
                asked: OperationId::for_tests(3),
                paths: vec![cairn_model::RepoPath::from("a")],
            },
            Request::CancelWrite {
                id: OperationId::for_tests(1),
            },
            Request::RefreshStatus,
            Request::Retire(
                Retired::of(
                    Some(Arc::new(cairn_model::ChangeSet {
                        files: Vec::new(),
                        details: None,
                        renames: cairn_model::RenameDetection::default(),
                    })),
                    Vec::new(),
                )
                .unwrap_or_else(|| unreachable!("a change set is something to retire")),
            ),
            Request::Close,
        ]
    }

    /// PRD R4.2 and refs-and-status R11.2: every query is served on the thread its lane is
    /// routed to, and the lanes are routed as decided — the history and the refs on the
    /// thread that owns the walk, the changes and file-diff lanes on the diff thread, status
    /// and ahead/behind on the refresh thread. Caught by: a diff routed to the repository
    /// thread (it queues behind a page), a status routed to the repository or diff thread (a
    /// page or a diff queues behind it), the table changed without the routing, or the
    /// routing without the table.
    #[test]
    fn every_query_is_served_on_the_thread_its_lane_is_routed_to() {
        assert_eq!(thread_of(QueryLane::History), Thread::Repository);
        assert_eq!(thread_of(QueryLane::Walk), Thread::Repository);
        assert_eq!(thread_of(QueryLane::Changes), Thread::Diff);
        assert_eq!(thread_of(QueryLane::FileDiff), Thread::Diff);
        assert_eq!(thread_of(QueryLane::FileFilter), Thread::Repository);
        assert_eq!(thread_of(QueryLane::Refs), Thread::Repository);
        assert_eq!(thread_of(QueryLane::AheadBehind), Thread::Refresh);
        assert_eq!(thread_of(QueryLane::Status), Thread::Refresh);
        assert_eq!(thread_of(QueryLane::RefFilter), Thread::Repository);
        assert_eq!(thread_of(QueryLane::LocalChangesFilter), Thread::Repository);
        // Staging-and-commit R4.1: a write goes straight to the local lane, never through the
        // repository thread, where it would wait behind a page or a find; a status asked alone
        // goes where a refresh's does; a write's cancel queues nowhere.
        let write = Request::Write {
            id: OperationId::for_tests(2),
            write: crate::worker::LocalWrite::StageFiles { paths: Vec::new() },
        };
        assert_eq!(route(write).thread(), Some(Thread::Local));
        // What a discard would lose is counted on the local lane, after the writes ahead of it.
        let consequence = Request::DiscardConsequence {
            asked: OperationId::for_tests(2),
            paths: Vec::new(),
        };
        assert_eq!(route(consequence).thread(), Some(Thread::Local));
        assert_eq!(
            route(Request::RefreshStatus).thread(),
            Some(Thread::Refresh)
        );
        assert_eq!(
            route(Request::CancelWrite {
                id: OperationId::for_tests(2)
            })
            .thread(),
            None
        );
        for (request, asked) in every_request().into_iter().zip(every_request()) {
            let lanes = request.lanes();
            let routed = route(asked);
            for lane in lanes {
                assert_eq!(
                    routed.lane_thread(*lane),
                    Some(thread_of(*lane)),
                    "{request:?} is in the {lane:?} lane"
                );
            }
            match lanes.first() {
                Some(_) => {}
                // The configured context: the diff thread's, since its handle is the one the
                // configuration's freshness opens again (phase 06 QA, T7).
                None if request == Request::ConfiguredContext => {
                    assert_eq!(routed.thread(), Some(Thread::Diff));
                }
                // Any other operation: the repository thread's, or the fetch control's.
                None => assert_ne!(routed.thread(), Some(Thread::Diff), "{request:?}"),
            }
        }
    }

    /// Caught by: routing that loses or changes what was asked on the way.
    #[test]
    fn routing_keeps_every_request_whole() {
        for (request, asked) in every_request().into_iter().zip(every_request()) {
            assert_eq!(unroute(route(asked)), request);
        }
    }
}
