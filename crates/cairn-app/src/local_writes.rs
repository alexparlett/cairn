//! The local writes as the window keeps them (staging-and-commit R4): each asked, queued
//! until it starts, running until it ends, and how the last one ended; the lock files last
//! listed; and whether the window is waiting on one to close (R4.9).
//!
//! On the UI thread, so nothing here waits: a write is asked by a submit and its news arrives
//! as updates (`session::apply`).

use std::collections::VecDeque;
use std::path::PathBuf;

use crate::worker::{LocalWrite, OperationId, Request, WriteEnding};

/// What a wait calls a write the window did not see asked.
const UNKNOWN_WRITE: &str = "another write to finish";

/// A write the window asked for: its id, and what it is called while it waits and runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub id: OperationId,
    pub what: String,
    /// What the activity popover calls it, in Fork's imperative form (`LocalWrite::name`).
    pub name: String,
    /// What a wait behind it waits for: "the commit to finish", "the branch to be created"
    /// (`LocalWrite::awaited`).
    pub awaited: &'static str,
    /// The commit it replaces — an amend's — which the activity popover points back to.
    pub replaces: Option<cairn_model::Oid>,
    /// Whether it can be cancelled while it runs: a commit or an amend (R4.3).
    pub cancellable: bool,
    /// The prompt a destructive write's confirmation recorded, copied as it was asked
    /// (`LocalWrite::prompt`): what the activity popover quotes however the write ends.
    pub prompt: Option<String>,
}

impl Asked {
    /// A write the window did not see asked.
    fn unknown(id: OperationId) -> Self {
        Self {
            id,
            what: "a write".to_owned(),
            name: "A write".to_owned(),
            awaited: UNKNOWN_WRITE,
            replaces: None,
            cancellable: false,
            prompt: None,
        }
    }
}

/// The local writes of this session, as the window draws them.
#[derive(Debug, Default)]
pub struct LocalWrites {
    /// Asked and not started, in the order asked: what the window draws as queued (R4.2).
    queued: VecDeque<Asked>,
    running: Option<Asked>,
    /// The latest write to end, and how.
    last: Option<(Asked, WriteEnding)>,
    /// The lock files last listed: as the repository opened, then around each write's ending
    /// (R3.8, R4.9) — a finding the window names, never acts on.
    locks: Vec<PathBuf>,
    /// The window asked to close while a write ran: it says which it is waiting for.
    closing: bool,
}

impl LocalWrites {
    /// `write` is asked for under `id`: queued until it starts.
    pub fn asked(&mut self, id: OperationId, write: &LocalWrite) {
        self.queued.push_back(Asked {
            id,
            what: write.what(),
            name: write.name(),
            awaited: write.awaited(),
            replaces: write.replaces(),
            cancellable: write.is_cancellable(),
            prompt: write.prompt(),
        });
    }

    /// `id` has started: every write asked before it has ended.
    pub fn started(&mut self, id: OperationId) {
        let at = self.queued.iter().position(|asked| asked.id == id);
        let asked = at
            .and_then(|at| self.queued.remove(at))
            .unwrap_or_else(|| Asked::unknown(id));
        self.running = Some(asked);
    }

    /// `id` has ended, as `ending` says; the lock files it names are the ones now listed.
    pub fn ended(&mut self, id: OperationId, ending: WriteEnding) {
        let asked = match self.running.take_if(|running| running.id == id) {
            Some(asked) => asked,
            // Never started: not run, as the repository was closing.
            None => {
                let at = self.queued.iter().position(|asked| asked.id == id);
                at.and_then(|at| self.queued.remove(at))
                    .unwrap_or_else(|| Asked::unknown(id))
            }
        };
        self.locks = ending.locks().to_vec();
        self.last = Some((asked, ending));
    }

    /// The lock files present as the repository opened.
    pub fn locks_at_open(&mut self, locks: Vec<PathBuf>) {
        self.locks = locks;
    }

    /// The window has asked to close.
    pub fn closing(&mut self) {
        self.closing = true;
    }

    /// The write running, if any: the commit box draws its commit running, and cancels only it
    /// (R4.3, R10.4).
    pub fn running(&self) -> Option<&Asked> {
        self.running.as_ref()
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// The writes asked and not yet started, in the order they will run.
    pub fn queued(&self) -> impl Iterator<Item = &Asked> {
        self.queued.iter()
    }

    /// The latest write to end, and how.
    pub fn last(&self) -> Option<&(Asked, WriteEnding)> {
        self.last.as_ref()
    }

    pub fn locks(&self) -> &[PathBuf] {
        &self.locks
    }

    /// The write the window is waiting on to close, if it asked to and one runs.
    pub fn closing_on(&self) -> Option<&Asked> {
        self.running.as_ref().filter(|_| self.closing)
    }
}

/// Asks for `write`: a fresh id, the write kept as queued, and the request submitted — one
/// atomic increment and one send, so nothing here waits.
pub fn ask(writes: &mut LocalWrites, submit: &dyn Fn(Request), write: LocalWrite) -> OperationId {
    let id = OperationId::next();
    writes.asked(id, &write);
    submit(Request::Write { id, write });
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::RepoPath;
    use std::cell::RefCell;

    fn stage(path: &str) -> LocalWrite {
        LocalWrite::StageFiles {
            paths: vec![RepoPath::from(path)],
        }
    }

    /// R4.2: writes asked are drawn queued in the order asked until each starts, one runs at
    /// a time, and the last ending is kept with the locks it names. Caught by: a write that
    /// leaves the queue out of order, stays queued once running, or an ending that loses its
    /// locks.
    #[test]
    fn writes_are_queued_in_order_until_each_starts_and_end_with_their_locks() {
        let asked = RefCell::new(Vec::new());
        let submit = |request: Request| asked.borrow_mut().push(request);
        let mut writes = LocalWrites::default();
        let ids: Vec<OperationId> = ["a", "b", "c"]
            .into_iter()
            .map(|path| ask(&mut writes, &submit, stage(path)))
            .collect();
        assert_eq!(asked.borrow().len(), 3, "each write is one request");
        assert!(matches!(
            asked.borrow().first(),
            Some(Request::Write { id, .. }) if *id == ids[0]
        ));
        assert_eq!(
            writes.queued().map(|asked| asked.id).collect::<Vec<_>>(),
            ids,
            "not queued in the order asked"
        );
        assert!(!writes.is_running());

        writes.started(ids[0]);
        assert_eq!(
            writes.running().map(|asked| asked.what.as_str()),
            Some("staging 1 file")
        );
        assert_eq!(
            writes.queued().map(|asked| asked.id).collect::<Vec<_>>(),
            ids[1..],
            "a running write is still drawn queued"
        );
        let lock = PathBuf::from("/r/.git/index.lock");
        writes.ended(
            ids[0],
            WriteEnding::Failed {
                message: "index.lock exists".to_owned(),
                locks: vec![lock.clone()],
                command: None,
                output: String::new(),
            },
        );
        assert!(!writes.is_running());
        assert_eq!(writes.locks(), [lock]);
        assert_eq!(writes.last().map(|(asked, _)| asked.id), Some(ids[0]));

        // A write ended before it started — not run, the repository closing — leaves the queue.
        writes.ended(
            ids[2],
            WriteEnding::NotRun {
                message: "closing".to_owned(),
            },
        );
        assert_eq!(
            writes.queued().map(|asked| asked.id).collect::<Vec<_>>(),
            [ids[1]]
        );
        assert_eq!(
            writes.locks(),
            [] as [PathBuf; 0],
            "the locks were not listed again"
        );
    }

    /// R4.9: the window says which write it waits on only once it has asked to close, and
    /// only while one runs.
    #[test]
    fn a_close_names_the_write_it_waits_on() {
        let mut writes = LocalWrites::default();
        let id = OperationId::for_tests(7);
        writes.asked(id, &stage("a"));
        writes.started(id);
        assert_eq!(
            writes.closing_on(),
            None,
            "waiting on a close nobody asked for"
        );
        writes.closing();
        assert_eq!(
            writes.closing_on().map(|asked| asked.what.as_str()),
            Some("staging 1 file")
        );
        writes.ended(
            id,
            WriteEnding::Refused {
                message: "x".to_owned(),
            },
        );
        assert_eq!(
            writes.closing_on(),
            None,
            "waiting on a write that has ended"
        );
    }
}
