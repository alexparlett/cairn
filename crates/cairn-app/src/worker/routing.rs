//! The routing table: which thread serves each request (PRD R4.2, packet decision L8).
//!
//! | Lane or request | Thread |
//! | --- | --- |
//! | history (`OpenHistory`, `MoreHistory`) | `cairn-repository`, which owns the live walk |
//! | changes (`Changes`) | `cairn-diff` |
//! | file diff (`FileDiff`, `ExpandAll`) | `cairn-diff` |
//! | `ListRemotes`, `CommandLog`, `Close`, `Fetch` | `cairn-repository` (a fetch is forwarded on to the network lane) |
//! | `CancelFetch` | none: the fetch's control, from the caller's thread |
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
use super::request::{DiffQuery, FileQuery, Request};

/// A thread a repository's requests are served on.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Thread {
    /// `cairn-repository`: the history walk, the operations' front door, and the close.
    Repository,
    /// `cairn-diff`: the changes and file-diff lanes, commits and working tree alike.
    Diff,
}

/// The thread each query lane is served on: the table above, for the lanes.
#[cfg(test)]
pub(super) const fn thread_of(lane: QueryLane) -> Thread {
    match lane {
        QueryLane::History => Thread::Repository,
        QueryLane::Changes | QueryLane::FileDiff => Thread::Diff,
    }
}

/// Which page of the history walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    /// A new walk from `HEAD`, dropping any walk open.
    Open { rows: usize },
    /// The next rows of the walk open, or a cold restart from the last good page.
    More { rows: usize },
}

/// What the repository thread is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RepositoryJob {
    History(Page),
    ListRemotes,
    Fetch { remote: String },
    CommandLog,
    Close,
}

/// Where a request goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Routed {
    Repository(RepositoryJob),
    Diff(DiffQuery),
    /// Never queued: it reaches the fetch directly, ahead of any page.
    CancelFetch,
}

#[cfg(test)]
impl Routed {
    /// The thread this is served on; `None` for what no thread queues.
    pub(super) fn thread(&self) -> Option<Thread> {
        match self {
            Self::Repository(_) => Some(Thread::Repository),
            Self::Diff(_) => Some(Thread::Diff),
            Self::CancelFetch => None,
        }
    }
}

/// Routes `request`. Total, with no arm that defaults: a new request does not compile
/// until it is given a thread.
pub(super) fn route(request: Request) -> Routed {
    match request {
        Request::OpenHistory { rows } => {
            Routed::Repository(RepositoryJob::History(Page::Open { rows }))
        }
        Request::MoreHistory { rows } => {
            Routed::Repository(RepositoryJob::History(Page::More { rows }))
        }
        Request::Changes { of } => Routed::Diff(DiffQuery::Changes(of)),
        Request::FileDiff(FileQuery { target, options }) => {
            Routed::Diff(DiffQuery::File(FileQuery { target, options }))
        }
        Request::ExpandAll { of, options } => Routed::Diff(DiffQuery::All { of, options }),
        Request::ListRemotes => Routed::Repository(RepositoryJob::ListRemotes),
        Request::Fetch { remote } => Routed::Repository(RepositoryJob::Fetch { remote }),
        Request::CommandLog => Routed::Repository(RepositoryJob::CommandLog),
        Request::Close => Routed::Repository(RepositoryJob::Close),
        Request::CancelFetch => Routed::CancelFetch,
    }
}

/// The request a routed job came from: the inverse of [`route`], for a test of the
/// window's side that reads back what it asked.
#[cfg(test)]
pub(super) fn unroute(routed: Routed) -> Request {
    match routed {
        Routed::Repository(RepositoryJob::History(Page::Open { rows })) => {
            Request::OpenHistory { rows }
        }
        Routed::Repository(RepositoryJob::History(Page::More { rows })) => {
            Request::MoreHistory { rows }
        }
        Routed::Repository(RepositoryJob::ListRemotes) => Request::ListRemotes,
        Routed::Repository(RepositoryJob::Fetch { remote }) => Request::Fetch { remote },
        Routed::Repository(RepositoryJob::CommandLog) => Request::CommandLog,
        Routed::Repository(RepositoryJob::Close) => Request::Close,
        Routed::Diff(DiffQuery::Changes(of)) => Request::Changes { of },
        Routed::Diff(DiffQuery::File(query)) => Request::FileDiff(query),
        Routed::Diff(DiffQuery::All { of, options }) => Request::ExpandAll { of, options },
        Routed::CancelFetch => Request::CancelFetch,
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
            Request::Changes { of: commit },
            Request::FileDiff(FileQuery {
                target: FileTarget::WorkingTree {
                    path: cairn_model::RepoPath::from("a"),
                    side: WorkingSide::Staged,
                },
                options: DiffOptions::default(),
            }),
            Request::ExpandAll {
                of: commit,
                options: DiffOptions::default(),
            },
            Request::ListRemotes,
            Request::Fetch {
                remote: "origin".to_owned(),
            },
            Request::CancelFetch,
            Request::CommandLog,
            Request::Close,
        ]
    }

    /// PRD R4.2: every query is served on the thread its lane is routed to, and the lanes
    /// are routed as decided — the history on the thread that owns the walk, the changes
    /// and file-diff lanes on the diff thread. Caught by: a diff routed to the repository
    /// thread (it queues behind a page), the table changed without the routing, or the
    /// routing without the table.
    #[test]
    fn every_query_is_served_on_the_thread_its_lane_is_routed_to() {
        assert_eq!(thread_of(QueryLane::History), Thread::Repository);
        assert_eq!(thread_of(QueryLane::Changes), Thread::Diff);
        assert_eq!(thread_of(QueryLane::FileDiff), Thread::Diff);
        for request in every_request() {
            let lane = request.lane();
            let routed = route(request.clone());
            match lane {
                Some(lane) => assert_eq!(
                    routed.thread(),
                    Some(thread_of(lane)),
                    "{request:?} is in the {lane:?} lane"
                ),
                // An operation: the repository thread's, or the fetch control's.
                None => assert_ne!(routed.thread(), Some(Thread::Diff), "{request:?}"),
            }
        }
    }

    /// Caught by: routing that loses or changes what was asked on the way.
    #[test]
    fn routing_keeps_every_request_whole() {
        for request in every_request() {
            assert_eq!(unroute(route(request.clone())), request);
        }
    }
}
