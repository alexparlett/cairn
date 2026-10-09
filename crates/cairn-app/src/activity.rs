//! The activity popover as the window keeps it (staging-and-commit R12): one entry per
//! operation of the session — every local write and every fetch — newest first, bounded, with
//! its name, when it started, how long it took, how it ended, the prompt it confirmed, each
//! `git` it ran with its stderr, the way back where there is one, and `Remove index.lock…`
//! where R12.4 offers it. Nothing of it outlives the window (R12.3, L14).
//!
//! Fed by `session::apply` from the updates the lanes send: a write's start, its output as it
//! streams (R10.4: a hook's lines while it runs), what it ran (`Update::OperationRan`, the
//! command log's records of its own `git`, scrubbed on the lane), and its ending; a fetch's the
//! same. Every text of git's kept here is scrubbed of a URL's userinfo (R12.2) — on the lane,
//! and again as it is kept, so no entry can draw a token whoever built the update. On the UI
//! thread: each update costs what it carries, and the popover's entries are built only while it
//! is open, from lines kept shared.

use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use cairn_model::{CommandExit, CommandRecord, Consequence, Oid};
use cairn_ui::{ActivityEntry, ActivityLine, ActivityPopover};
use freya::prelude::*;

use crate::confirming::Confirming;
use crate::local_writes::Asked;
use crate::shown_output::{ShownLines, shown_line, shown_lines};
use crate::window::View;
use crate::worker::{LocalWrite, Request};
use crate::worker::{OperationId, WriteEnding};

/// The most operations the popover keeps: the newest, the oldest let go of first.
pub const ACTIVITY_ENTRIES: usize = 200;
/// The most lines one operation keeps — its commands and what they said — the latest kept.
pub const ACTIVITY_LINES: usize = 10_000;
/// The most bytes of lines every operation together keeps: the oldest operations' lines are
/// let go of first, the entries themselves kept.
pub const ACTIVITY_BYTES: usize = 4 * 1024 * 1024;

/// Which operation an entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKey {
    Write(OperationId),
    /// The `n`th fetch of the session.
    Fetch(u64),
}

/// How an operation stands, in the popover's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Succeeded,
    Failed(String),
    /// Nothing was written: refused, stale, or never started.
    NotRun(String),
    /// Cancelled, or Cairn lost hold of its `git`: it may have done part of its work.
    MayHaveTakenEffect(String),
    /// A discard of files that did not take every file.
    PartlyDone(String),
    Cancelled,
}

impl Outcome {
    /// The status the list draws.
    pub fn status(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed(_) => "failed",
            Self::NotRun(_) => "not run",
            Self::MayHaveTakenEffect(_) => "may have taken effect",
            Self::PartlyDone(_) => "partly done",
            Self::Cancelled => "cancelled",
        }
    }

    fn message(&self) -> Option<String> {
        match self {
            Self::Failed(message)
            | Self::NotRun(message)
            | Self::MayHaveTakenEffect(message)
            | Self::PartlyDone(message) => Some(message.clone()),
            Self::Running | Self::Succeeded | Self::Cancelled => None,
        }
    }
}

/// One operation, as kept.
#[derive(Debug, Clone)]
pub struct Activity {
    pub key: ActivityKey,
    pub name: String,
    pub started: SystemTime,
    clock: Instant,
    pub took: Option<Duration>,
    pub outcome: Outcome,
    pub prompt: Option<String>,
    /// The commit an amend replaced: offered as the way back once the amend succeeded.
    replaces: Option<Oid>,
    pub cancellable: bool,
    /// `Remove index.lock…`'s offer, from what the operation ran.
    pub lock: Option<Rc<Consequence>>,
    /// Its `git` commands and what they said, once they are over.
    commands: Vec<ActivityLine>,
    /// What it wrote as it ran, while it runs.
    streamed: VecDeque<String>,
    shown: ShownLines,
    /// The lines drawn, built once each time they change.
    lines: Rc<Vec<ActivityLine>>,
}

impl Activity {
    fn new(key: ActivityKey, name: String, replaces: Option<Oid>, cancellable: bool) -> Self {
        Self {
            key,
            name,
            started: SystemTime::now(),
            clock: Instant::now(),
            took: None,
            outcome: Outcome::Running,
            prompt: None,
            replaces,
            cancellable,
            lock: None,
            commands: Vec::new(),
            streamed: VecDeque::new(),
            shown: ShownLines::default(),
            lines: Rc::new(Vec::new()),
        }
    }

    /// The bytes its lines hold.
    fn bytes(&self) -> usize {
        self.lines.iter().map(line_bytes).sum()
    }

    fn redraw(&mut self) {
        let mut lines: Vec<ActivityLine> = self.commands.clone();
        // While it runs, what it has written so far; once its commands are in, theirs.
        if self.commands.is_empty() {
            lines.extend(self.streamed.iter().cloned().map(ActivityLine::Output));
        }
        let over = lines.len().saturating_sub(ACTIVITY_LINES);
        lines.drain(..over);
        self.lines = Rc::new(lines);
    }

    fn end(&mut self, outcome: Outcome) {
        self.took = Some(self.clock.elapsed());
        self.outcome = outcome;
        self.cancellable = false;
    }

    /// The way back, once there is one: an amend that succeeded.
    pub fn replaced(&self) -> Option<Oid> {
        self.replaces
            .filter(|_| matches!(self.outcome, Outcome::Succeeded))
    }
}

fn line_bytes(line: &ActivityLine) -> usize {
    match line {
        ActivityLine::Ran(text) | ActivityLine::Output(text) => text.len(),
    }
}

/// The session's operations, oldest first, and the popover's own state.
#[derive(Debug, Default)]
pub struct ActivityLog {
    entries: VecDeque<Activity>,
    fetches: u64,
    /// The fetch running, if any.
    fetching: Option<u64>,
    open: bool,
    /// The entry drawn on the right; the newest when none was chosen or it was let go of.
    selected: Option<ActivityKey>,
}

impl ActivityLog {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.selected = None;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn choose(&mut self, key: ActivityKey) {
        self.selected = Some(key);
    }

    /// Every entry, newest first.
    pub fn newest_first(&self) -> impl Iterator<Item = &Activity> {
        self.entries.iter().rev()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn find(&mut self, key: ActivityKey) -> Option<&mut Activity> {
        self.entries.iter_mut().rev().find(|entry| entry.key == key)
    }

    fn push(&mut self, entry: Activity) {
        self.entries.push_back(entry);
        while self.entries.len() > ACTIVITY_ENTRIES {
            self.entries.pop_front();
        }
    }

    /// Keeps every entry's lines together under [`ACTIVITY_BYTES`]: the oldest entries' lines
    /// let go of first.
    fn bound(&mut self) {
        let mut total: usize = self.entries.iter().map(Activity::bytes).sum();
        for entry in &mut self.entries {
            if total <= ACTIVITY_BYTES {
                break;
            }
            total -= entry.bytes();
            entry.commands.clear();
            entry.streamed.clear();
            entry.lines = Rc::new(Vec::new());
        }
    }

    /// A local write has started.
    pub fn write_started(&mut self, asked: &Asked) {
        self.push(Activity::new(
            ActivityKey::Write(asked.id),
            crate::status_text::capitalised(&asked.what),
            asked.replaces,
            asked.cancellable,
        ));
    }

    /// Lines a running operation wrote, kept while it runs (R10.4), the latest
    /// [`ACTIVITY_LINES`].
    pub fn output(&mut self, key: ActivityKey, lines: &[String]) {
        let Some(entry) = self.find(key) else {
            return;
        };
        for line in lines {
            let line = entry.shown.line(line);
            entry.streamed.push_back(line);
        }
        while entry.streamed.len() > ACTIVITY_LINES {
            entry.streamed.pop_front();
        }
        entry.redraw();
        self.bound();
    }

    /// What an operation ran, and `Remove index.lock…`'s offer.
    pub fn ran(&mut self, key: ActivityKey, commands: &[CommandRecord], lock: Option<Consequence>) {
        let Some(entry) = self.find(key) else {
            return;
        };
        entry.commands = commands.iter().flat_map(command_lines).collect();
        entry.lock = lock.map(Rc::new);
        entry.redraw();
        self.bound();
    }

    /// A local write has ended, as `ending` says (already scrubbed by the window).
    pub fn write_ended(&mut self, id: OperationId, ending: &WriteEnding) {
        let key = ActivityKey::Write(id);
        if self.find(key).is_none() {
            // Never started: not run, as the repository was closing.
            self.push(Activity::new(key, "A write".to_owned(), None, false));
        }
        let Some(entry) = self.find(key) else {
            return;
        };
        let (outcome, prompt) = match ending {
            WriteEnding::Done(done) => (Outcome::Succeeded, done.acknowledged.clone()),
            WriteEnding::Incomplete { done, message, .. } => (
                Outcome::PartlyDone(message.clone()),
                done.acknowledged.clone(),
            ),
            WriteEnding::Stale { message, .. }
            | WriteEnding::Refused { message }
            | WriteEnding::NotRun { message } => (Outcome::NotRun(message.clone()), None),
            WriteEnding::Failed { message, .. } => (Outcome::Failed(message.clone()), None),
            WriteEnding::MayHaveTakenEffect { message, .. } => {
                (Outcome::MayHaveTakenEffect(message.clone()), None)
            }
        };
        entry.prompt = prompt;
        entry.end(outcome);
    }

    /// A fetch of `remote` has started.
    pub fn fetch_started(&mut self, remote: &str) {
        self.fetches += 1;
        self.fetching = Some(self.fetches);
        self.push(Activity::new(
            ActivityKey::Fetch(self.fetches),
            format!("Fetch {remote}"),
            None,
            true,
        ));
    }

    /// The fetch running, if any — what a fetch's lines and its ending are of.
    pub fn fetch_key(&self) -> Option<ActivityKey> {
        self.fetching.map(ActivityKey::Fetch)
    }

    /// The fetch of `remote` has ended — one that never started (refused before git ran) is
    /// an entry of its own.
    pub fn fetch_ended(&mut self, remote: &str, outcome: Outcome) {
        let key = match self.fetching.take() {
            Some(n) => ActivityKey::Fetch(n),
            None => {
                self.fetches += 1;
                let key = ActivityKey::Fetch(self.fetches);
                self.push(Activity::new(key, format!("Fetch {remote}"), None, false));
                key
            }
        };
        if let Some(entry) = self.find(key) {
            entry.end(outcome);
        }
    }

    /// What the window's popover draws: every entry, newest first, and the place of the one
    /// selected — the newest when none is.
    pub fn shown(&self) -> (Rc<Vec<ActivityEntry>>, usize) {
        let entries: Vec<ActivityEntry> = self.newest_first().map(entry_of).collect();
        let selected = self
            .selected
            .and_then(|key| self.newest_first().position(|entry| entry.key == key))
            .unwrap_or(0);
        (Rc::new(entries), selected)
    }

    /// The key of the entry at `place`, newest first.
    pub fn key_at(&self, place: usize) -> Option<ActivityKey> {
        self.newest_first().nth(place).map(|entry| entry.key)
    }
}

/// A record of the command log as the popover draws it: `$ git <arguments>`, then what it
/// wrote to stderr, and how it ended when that was not a clean exit.
fn command_lines(record: &CommandRecord) -> Vec<ActivityLine> {
    let mut lines = vec![ActivityLine::Ran(shown_line(&format!(
        "$ git {}",
        record.arguments.join(" ")
    )))];
    lines.extend(
        shown_lines(&record.stderr)
            .into_iter()
            .map(ActivityLine::Output),
    );
    let ended = match record.exit {
        CommandExit::Code(0) if !record.cancelled => None,
        CommandExit::Code(code) => Some(format!("exit status {code}")),
        CommandExit::Signal(signal) => Some(format!("ended by signal {signal}")),
        CommandExit::NotStarted => Some("not started".to_owned()),
        CommandExit::Unknown => Some("how it ended is not known".to_owned()),
    };
    let ended = match (ended, record.cancelled) {
        (Some(ended), true) => Some(format!("{ended}, cancelled")),
        (None, true) => Some("cancelled".to_owned()),
        (ended, false) => ended,
    };
    lines.extend(ended.map(|ended| ActivityLine::Output(format!("({ended})"))));
    lines
}

/// When an operation started, as the popover says it: the time of day, UTC, as the history's
/// dates are drawn.
fn started_text(started: SystemTime) -> String {
    let seconds = started
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let of_day = seconds % 86_400;
    format!(
        "{:02}:{:02}:{:02} UTC",
        of_day / 3600,
        (of_day % 3600) / 60,
        of_day % 60
    )
}

/// How long an operation took, as the popover says it.
pub fn took_text(took: Duration) -> String {
    let millis = took.as_millis();
    if millis < 1_000 {
        format!("{millis} ms")
    } else if millis < 60_000 {
        format!("{:.1} s", took.as_secs_f64())
    } else {
        let seconds = took.as_secs();
        format!("{} min {} s", seconds / 60, seconds % 60)
    }
}

fn entry_of(activity: &Activity) -> ActivityEntry {
    ActivityEntry {
        name: activity.name.clone(),
        status: activity.outcome.status().to_owned(),
        started: started_text(activity.started),
        took: activity.took.map(took_text),
        message: activity.outcome.message(),
        prompt: activity.prompt.clone(),
        replaced: activity.replaced(),
        lock: activity.lock.clone(),
        cancellable: activity.cancellable,
        lines: activity.lines.clone(),
    }
}

/// What a confirmation of `Remove index.lock…` is titled.
pub const REMOVE_LOCK_TITLE: &str = "Remove index.lock";

/// The popover, while it is open (R12.1): drawn over the window, its operations from the log.
/// A cancel asks the write's or the fetch's cancel; the way back finds the replaced commit in
/// Show Lost Commits; `Remove index.lock…` opens the confirmation, whose token asks the local
/// lane for the removal.
pub fn popover(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Option<Element> {
    let (entries, selected) = {
        let log = view.activity.read();
        if !log.is_open() {
            return None;
        }
        log.shown()
    };
    let mut activity = view.activity;
    let cancelling = submit.clone();
    let returning = submit.clone();
    let removing = submit;
    Some(
        ActivityPopover::new(entries, selected)
            .on_select(move |place: usize| {
                let key = activity.peek().key_at(place);
                if let Some(key) = key {
                    activity.write().choose(key);
                }
            })
            .on_cancel(move |place: usize| {
                let key = view.activity.peek().key_at(place);
                let Some(submit) = cancelling.as_deref() else {
                    return;
                };
                match key {
                    Some(ActivityKey::Write(id)) => submit(Request::CancelWrite { id }),
                    Some(ActivityKey::Fetch(_)) => {
                        let mut fetch = view.fetch;
                        fetch.write().cancelling();
                        submit(Request::CancelFetch);
                    }
                    None => {}
                }
            })
            .on_replaced(move |commit: Oid| {
                let mut activity = view.activity;
                activity.write().close();
                show_replaced(commit, view, returning.as_deref());
            })
            .on_remove_lock(move |consequence: Rc<Consequence>| {
                let Some(submit) = removing.clone() else {
                    return;
                };
                let mut confirming = view.confirming;
                confirming.set(Some(Confirming::new(
                    REMOVE_LOCK_TITLE,
                    (*consequence).clone(),
                    move |token| {
                        let mut writes = view.writes;
                        crate::local_writes::ask(
                            &mut writes.write(),
                            &*submit,
                            LocalWrite::RemoveLock(token),
                        );
                    },
                )));
            })
            .on_close(move |()| {
                let mut activity = view.activity;
                activity.write().close();
            })
            .into(),
    )
}

/// The way back from an amend (R12.1): Show Lost Commits turned on, if it is not, and the
/// replaced commit found in the history and selected, as a ref's press finds its row.
fn show_replaced(commit: Oid, view: View, submit: Option<&dyn Fn(Request)>) {
    crate::ref_find::find_commit(commit, commit.short().as_str().to_owned(), view, submit);
    // After the find: the reopen asks it again of the walk that draws lost commits.
    if !*view.show_lost.peek() {
        crate::lost_commits::toggle(view, submit);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::worker::Done;

    fn asked(id: OperationId, what: &str, replaces: Option<Oid>) -> Asked {
        Asked {
            id,
            what: what.to_owned(),
            noun: "the commit",
            replaces,
            cancellable: true,
        }
    }

    fn record(arguments: &[&str], stderr: &str, exit: CommandExit) -> CommandRecord {
        CommandRecord {
            arguments: arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
            directory: None,
            started: SystemTime::UNIX_EPOCH,
            duration: Duration::ZERO,
            exit,
            cancelled: false,
            stderr: stderr.to_owned(),
            stderr_cut: false,
        }
    }

    fn done(acknowledged: Option<&str>) -> WriteEnding {
        WriteEnding::Done(Done {
            description: "amended".to_owned(),
            acknowledged: acknowledged.map(str::to_owned),
            locks_before: Vec::new(),
            locks_after: Vec::new(),
        })
    }

    /// R12.1: an entry per operation, newest first, with its name, status, the prompt it
    /// confirmed, each `git` it ran with its stderr, and the way back for an amend that
    /// succeeded; a hook's lines while it runs, then the commands once they are in. Caught by:
    /// entries oldest first, the prompt lost, the way back offered for an amend that failed.
    #[test]
    fn each_operation_is_an_entry_with_its_git_its_prompt_and_its_way_back() {
        let mut log = ActivityLog::default();
        let (stage, amend) = (OperationId::for_tests(1), OperationId::for_tests(2));
        log.write_started(&asked(stage, "staging 1 file", None));
        log.write_ended(stage, &done(None));
        let replaced = Oid::from_bytes(&[0xab; 20]).unwrap();
        log.write_started(&asked(amend, "amend", Some(replaced)));
        log.output(
            ActivityKey::Write(amend),
            &["hook: \u{1b}[31mok\u{1b}[0m".to_owned()],
        );
        let (entries, selected) = log.shown();
        assert_eq!(selected, 0);
        assert_eq!(entries[0].name, "Amend");
        assert_eq!(entries[0].status, "running");
        assert!(entries[0].cancellable);
        assert_eq!(
            *entries[0].lines,
            [ActivityLine::Output("hook: ok".to_owned())]
        );
        log.ran(
            ActivityKey::Write(amend),
            &[record(
                &["commit", "-q", "--amend", "-F", "-"],
                "hook: ok",
                CommandExit::Code(0),
            )],
            None,
        );
        log.write_ended(amend, &done(Some("Amend abababa? It replaces …")));
        let (entries, _) = log.shown();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["Amend", "Staging 1 file"]
        );
        let amended = &entries[0];
        assert_eq!(amended.status, "succeeded");
        assert!(!amended.cancellable);
        assert!(amended.took.is_some());
        assert_eq!(
            amended.prompt.as_deref(),
            Some("Amend abababa? It replaces …")
        );
        assert_eq!(amended.replaced, Some(replaced));
        assert_eq!(
            *amended.lines,
            [
                ActivityLine::Ran("$ git commit -q --amend -F -".to_owned()),
                ActivityLine::Output("hook: ok".to_owned()),
            ]
        );
        assert_eq!(entries[1].replaced, None);

        // An amend that failed points nowhere: nothing was replaced.
        let failed = OperationId::for_tests(3);
        log.write_started(&asked(failed, "amend", Some(replaced)));
        log.write_ended(
            failed,
            &WriteEnding::Failed {
                message: "git commit failed".to_owned(),
                locks: Vec::new(),
                command: None,
                output: String::new(),
            },
        );
        let (entries, _) = log.shown();
        assert_eq!(entries[0].status, "failed");
        assert_eq!(entries[0].message.as_deref(), Some("git commit failed"));
        assert_eq!(entries[0].replaced, None);
    }

    /// R12.2 in the popover: a command's arguments, its stderr and a streamed line carry no
    /// userinfo, a URL split over two reads included. Caught by: any of them kept as git wrote
    /// it.
    #[test]
    fn no_line_of_an_entry_carries_a_token() {
        let mut log = ActivityLog::default();
        log.fetch_started("origin");
        let key = log.fetch_key().unwrap();
        log.output(key, &["fatal: https://u:SECRET@h/r".to_owned()]);
        log.output(key, &["see https://u:SEC".to_owned(), "RET@h/x".to_owned()]);
        let (entries, _) = log.shown();
        assert!(
            entries[0]
                .lines
                .iter()
                .all(|line| !format!("{line:?}").contains("SEC")),
            "{:?}",
            entries[0].lines
        );
        log.ran(
            key,
            &[record(
                &["fetch", "https://u:SECRET@h/r"],
                "remote: https://u:SECRET@h/r denied",
                CommandExit::Code(128),
            )],
            None,
        );
        log.fetch_ended("origin", Outcome::Failed("boom".to_owned()));
        let (entries, _) = log.shown();
        assert_eq!(
            *entries[0].lines,
            [
                ActivityLine::Ran("$ git fetch https://h/r".to_owned()),
                ActivityLine::Output("remote: https://h/r denied".to_owned()),
                ActivityLine::Output("(exit status 128)".to_owned()),
            ]
        );
    }

    /// R12.1's bounds: at most [`ACTIVITY_ENTRIES`] operations, the oldest let go of; at most
    /// [`ACTIVITY_LINES`] lines one, the latest kept; at most [`ACTIVITY_BYTES`] of lines
    /// together, the oldest operations' lines let go of first. Caught by: an unbounded log.
    #[test]
    fn the_log_is_bounded_by_entries_lines_and_bytes() {
        let mut log = ActivityLog::default();
        for n in 0..ACTIVITY_ENTRIES as u64 + 5 {
            let id = OperationId::for_tests(n);
            log.write_started(&asked(id, &format!("write {n}"), None));
            log.write_ended(id, &done(None));
        }
        assert_eq!(log.len(), ACTIVITY_ENTRIES);
        assert_eq!(
            log.newest_first().last().map(|entry| entry.name.as_str()),
            Some("Write 5")
        );
        let id = OperationId::for_tests(10_000);
        log.write_started(&asked(id, "commit", None));
        let lines: Vec<String> = (0..ACTIVITY_LINES + 7)
            .map(|n| format!("line {n}"))
            .collect();
        log.output(ActivityKey::Write(id), &lines);
        let (entries, _) = log.shown();
        assert_eq!(entries[0].lines.len(), ACTIVITY_LINES);
        assert_eq!(
            entries[0].lines.last(),
            Some(&ActivityLine::Output(format!(
                "line {}",
                ACTIVITY_LINES + 6
            )))
        );
        let wide = "x".repeat(300 * 1024);
        for n in 0..20 {
            let id = OperationId::for_tests(20_000 + n);
            log.write_started(&asked(id, "commit", None));
            log.output(ActivityKey::Write(id), std::slice::from_ref(&wide));
        }
        let held: usize = log.newest_first().map(Activity::bytes).sum();
        assert!(held <= ACTIVITY_BYTES, "{held} bytes held");
        let (entries, _) = log.shown();
        assert_eq!(
            entries[0].lines.len(),
            1,
            "the newest operation's lines were let go of"
        );
    }

    /// R12.4's offer is kept with what the operation ran, and a command that did not exit
    /// cleanly says how it ended.
    #[test]
    fn the_lock_offer_and_an_unclean_exit_are_kept() {
        let mut log = ActivityLog::default();
        let id = OperationId::for_tests(1);
        log.write_started(&asked(id, "staging 1 file", None));
        let lock = Consequence::RemoveLock {
            path: PathBuf::from("/r/.git/index.lock"),
            modified: SystemTime::UNIX_EPOCH,
            read_at: SystemTime::UNIX_EPOCH,
            bytes: 0,
            device: 1,
            inode: 2,
        };
        log.ran(
            ActivityKey::Write(id),
            &[record(
                &["add"],
                "fatal: Unable to create index.lock",
                CommandExit::Code(128),
            )],
            Some(lock),
        );
        let (entries, _) = log.shown();
        assert!(entries[0].lock.is_some());
        assert_eq!(
            entries[0].lines.last(),
            Some(&ActivityLine::Output("(exit status 128)".to_owned()))
        );
        assert_eq!(took_text(Duration::from_millis(12)), "12 ms");
        assert_eq!(took_text(Duration::from_millis(1_500)), "1.5 s");
        assert_eq!(took_text(Duration::from_secs(125)), "2 min 5 s");
        assert_eq!(
            started_text(SystemTime::UNIX_EPOCH + Duration::from_secs(3_723)),
            "01:02:03 UTC"
        );
    }
}
