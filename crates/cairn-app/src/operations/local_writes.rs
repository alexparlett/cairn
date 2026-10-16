//! The local writes as the window keeps them (staging-and-commit R4): each asked, queued
//! until it starts, running until it ends, and how the last one ended; and whether the window is
//! waiting on one to close (R4.9).
//!
//! On the UI thread, so nothing here waits: a write is asked by a submit and its news arrives
//! as updates (`session::apply`).

use std::collections::VecDeque;
use std::time::Instant;

use crate::worker::{LocalWrite, OperationId, Request, ResultLines};

/// A write the window asked for: its id and its one name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub id: OperationId,
    /// Its one name (`LocalWrite::name`, R14.2, rule 5), in Fork's imperative form: the status
    /// box, a wait ("Waiting for Commit 1 file…"), the closing line, the "Couldn't <name>" dialog and
    /// the activity popover all say this.
    pub name: String,
    /// Whether it can be cancelled while it runs: a commit or an amend (R4.3).
    pub cancellable: bool,
    /// The prompt a destructive write's confirmation recorded, copied as it was asked
    /// (`LocalWrite::prompt`): what the activity popover quotes once the write ran.
    pub prompt: Option<String>,
    /// What Create Branch's Discard did once it succeeded, said in its entry in place of a
    /// prompt (`LocalWrite::happened`).
    pub happened: Option<String>,
    /// Fork's result line under its row for each way it can end (`LocalWrite::result_lines`).
    pub results: ResultLines,
}

impl Asked {
    /// A write the window did not see asked.
    fn unknown(id: OperationId) -> Self {
        Self {
            id,
            name: "A write".to_owned(),
            cancellable: false,
            prompt: None,
            happened: None,
            results: ResultLines::default(),
        }
    }
}

/// The local writes of this session, as the window draws them.
#[derive(Debug, Default)]
pub struct LocalWrites {
    /// Asked and not started, in the order asked: what the status box counts as waiting (R4.2,
    /// R14.1).
    queued: VecDeque<Asked>,
    /// The write running, and when the window heard it start: the status box shows it once it
    /// has run 250 ms (F1).
    running: Option<(Asked, Instant)>,
    /// The window asked to close while a write ran: it says which it is waiting for.
    closing: bool,
}

impl LocalWrites {
    /// `write` is asked for under `id`: queued until it starts.
    pub fn asked(&mut self, id: OperationId, write: &LocalWrite) {
        self.queued.push_back(Asked {
            id,
            name: write.name(),
            cancellable: write.is_cancellable(),
            prompt: write.prompt(),
            happened: write.happened(),
            results: write.result_lines(),
        });
    }

    /// `id` has started: every write asked before it has ended.
    pub fn started(&mut self, id: OperationId) {
        let at = self.queued.iter().position(|asked| asked.id == id);
        let asked = at
            .and_then(|at| self.queued.remove(at))
            .unwrap_or_else(|| Asked::unknown(id));
        self.running = Some((asked, Instant::now()));
    }

    /// `id` has ended — run, or never started as the repository closed: it leaves the queue or
    /// stops running. Its ending's home is the window's (`crate::operations::ending_dialog`), not this.
    pub fn ended(&mut self, id: OperationId) {
        if self
            .running
            .take_if(|(running, _)| running.id == id)
            .is_none()
        {
            self.queued.retain(|asked| asked.id != id);
        }
    }

    /// The window has asked to close.
    pub fn closing(&mut self) {
        self.closing = true;
    }

    /// The write running, if any: the commit box draws its commit running, and cancels only it
    /// (R4.3, R10.4).
    pub fn running(&self) -> Option<&Asked> {
        self.running.as_ref().map(|(asked, _)| asked)
    }

    /// When the window heard the write running start.
    pub fn running_since(&self) -> Option<Instant> {
        self.running.as_ref().map(|(_, since)| *since)
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// Whether a write runs or waits to: the leftover lock's banner waits on both (F6).
    pub fn is_busy(&self) -> bool {
        self.running.is_some() || !self.queued.is_empty()
    }

    /// The writes asked and not yet started, in the order they will run.
    pub fn queued(&self) -> impl Iterator<Item = &Asked> {
        self.queued.iter()
    }

    /// The write the window is waiting on to close, if it asked to and one runs.
    pub fn closing_on(&self) -> Option<&Asked> {
        self.running().filter(|_| self.closing)
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

    /// R4.2: writes asked are kept queued in the order asked until each starts, one runs at
    /// a time, each by its one name, and an ending takes it off. Caught by: a write that leaves
    /// the queue out of order, stays queued once running, or a name other than the write's.
    #[test]
    fn writes_are_queued_in_order_until_each_starts() {
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
        assert!(writes.is_busy(), "writes waiting are busy");

        writes.started(ids[0]);
        assert_eq!(
            writes.running().map(|asked| asked.name.as_str()),
            Some("Stage 1 File")
        );
        assert!(writes.running_since().is_some());
        assert_eq!(
            writes.queued().map(|asked| asked.id).collect::<Vec<_>>(),
            ids[1..],
            "a running write is still drawn queued"
        );
        writes.ended(ids[0]);
        assert!(!writes.is_running());

        // A write ended before it started — not run, the repository closing — leaves the queue.
        writes.ended(ids[2]);
        assert_eq!(
            writes.queued().map(|asked| asked.id).collect::<Vec<_>>(),
            [ids[1]]
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
            writes.closing_on().map(|asked| asked.name.as_str()),
            Some("Stage 1 File")
        );
        writes.ended(id);
        assert_eq!(
            writes.closing_on(),
            None,
            "waiting on a write that has ended"
        );
    }
}
