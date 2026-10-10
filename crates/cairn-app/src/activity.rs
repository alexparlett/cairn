//! The activity popover as the window keeps it (staging-and-commit R12): one entry per
//! operation of the session — every local write and every fetch — newest first, bounded, with
//! its name, when it started, how long it took, how it ended, the prompt it confirmed, each
//! `git` it ran with its stderr, the way back where there is one, and `Remove index.lock…`
//! where R12.4 offers it. Nothing of it outlives the window (R12.3, L14).
//!
//! Fed by `session::apply` from the updates the lanes send: a write's start, its output as it
//! streams (R10.4: a hook's lines while it runs), what it ran (`Update::OperationRan`, the
//! command log's records of its own `git`), and its ending; a fetch's the same. Every text of
//! git's kept here arrives as `cairn_model::ScrubbedLines`, scrubbed of a URL's userinfo by the
//! engine's runner as it split git's output (R12.2, R4.10), so no entry can draw a token; it is
//! only stripped of escape sequences here (`crate::shown_output`). Each line is
//! cut, as the diff view cuts one, at `cairn_model::LINE_CUT_BYTES` on a character with the diff
//! view's marker after it (`cairn_ui::cut_marker`), so no row holds or shapes a 256 KiB piece.
//!
//! On the UI thread, each update costs what it carries: an entry counts its bytes as lines go in
//! and out, the log keeps their total, and the bound lets go of the oldest entries' lines — and,
//! for the newest alone over the budget, its oldest lines — without walking any line; an
//! entry's drawn lines are built only when the open popover asks for the entry selected, and
//! kept until a line changes. What a running operation wrote is let go of once its commands'
//! records arrive, since those are drawn in its place.

use std::collections::VecDeque;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use cairn_model::{CommandExit, CommandRecord, Consequence, Oid, ScrubbedLines};
use cairn_ui::{ActivityEntry, ActivityLine, ActivityPopover, LockOffer};
use freya::prelude::*;

use crate::confirming::Confirming;
use crate::local_writes::Asked;
use crate::shown_output::{shown_lines, spoken_lines};
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
    /// The lock files found as the repository opened (the user's decision H, 2026-10-09).
    Opened,
}

/// What the entry for the lock files found as the repository opened is called (the user's
/// decision H, 2026-10-09).
pub const LOCKS_AT_OPEN_NAME: &str = "Lock files found as the repository opened";
/// Said beside `Remove index.lock…` while a `git` of Cairn's runs, which it waits on (the
/// user's decision H): the engine's own words for the refusal it would meet.
pub const GIT_RUNNING_NOTE: &str = "Cairn is running git in this repository";
/// Said in place of an operation's lines once the byte bound let them go (the user's decision
/// M, 2026-10-09).
pub const LINES_LET_GO: &str = "Its output was let go to make room for newer operations.";

/// How an operation stands, in the popover's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Succeeded,
    Failed(String),
    /// Nothing was written: refused, stale, or never started.
    NotRun(String),
    /// A discard of files that did not take every file.
    PartlyDone(String),
    Cancelled,
    /// Found as the repository opened: lock files a write left behind.
    Found,
}

impl Outcome {
    /// The status the list draws.
    pub fn status(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed(_) => "failed",
            Self::NotRun(_) => "not run",
            Self::PartlyDone(_) => "partly done",
            Self::Cancelled => "cancelled",
            Self::Found => "found",
        }
    }

    fn message(&self) -> Option<String> {
        match self {
            Self::Failed(message) | Self::NotRun(message) | Self::PartlyDone(message) => {
                Some(message.clone())
            }
            Self::Running | Self::Succeeded | Self::Cancelled | Self::Found => None,
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
    /// Its ending, or the repository's open, named the stale `index.lock`: `Remove index.lock…`
    /// is offered, its cost read when pressed (the user's decision H, 2026-10-09).
    lock_named: bool,
    /// Why the last press of `Remove index.lock…` could not go ahead.
    lock_note: Option<String>,
    /// The byte bound let go of its lines: it says so in their place (the user's decision M).
    let_go: bool,
    /// Its `git` commands and what they said, once they are over.
    commands: VecDeque<ActivityLine>,
    /// What it wrote as it ran, while it runs: let go of once `commands` arrive.
    streamed: VecDeque<String>,
    /// The bytes of `commands` and `streamed`, counted as lines go in and out.
    bytes: usize,
    /// The lines drawn, built when the open popover asks and kept until a line changes.
    drawn: std::cell::RefCell<Option<Rc<Vec<ActivityLine>>>>,
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
            lock_named: false,
            lock_note: None,
            let_go: false,
            commands: VecDeque::new(),
            streamed: VecDeque::new(),
            bytes: 0,
            drawn: std::cell::RefCell::new(None),
        }
    }

    /// The lines drawn: its commands once they are in, what it wrote while it runs. Built once
    /// each time a line changes, when the open popover asks.
    pub fn lines(&self) -> Rc<Vec<ActivityLine>> {
        let mut drawn = self.drawn.borrow_mut();
        if let Some(lines) = drawn.as_ref() {
            return Rc::clone(lines);
        }
        let lines: Vec<ActivityLine> = if self.let_go && self.commands.is_empty() {
            vec![ActivityLine::Output(LINES_LET_GO.to_owned())]
        } else if self.commands.is_empty() {
            self.streamed
                .iter()
                .cloned()
                .map(ActivityLine::Output)
                .collect()
        } else {
            self.commands.iter().cloned().collect()
        };
        let lines = Rc::new(lines);
        *drawn = Some(Rc::clone(&lines));
        lines
    }

    /// A line changed: what is drawn is built again when next asked.
    fn changed(&mut self) {
        *self.drawn.get_mut() = None;
    }

    /// Lets go of its oldest line — what it wrote while it runs, else its commands' — and
    /// answers the bytes freed, or `None` when it holds none.
    fn let_go_of_oldest(&mut self) -> Option<usize> {
        let freed = match self.streamed.pop_front() {
            Some(line) => line.len(),
            None => line_bytes(&self.commands.pop_front()?),
        };
        self.bytes -= freed;
        self.changed();
        Some(freed)
    }

    /// The byte bound lets go of every line, answering the bytes freed: it says so in their
    /// place.
    fn let_go_by_the_bound(&mut self) -> usize {
        let freed = self.let_go_of_lines();
        self.let_go |= freed > 0;
        freed
    }

    /// Lets go of every line, answering the bytes freed.
    fn let_go_of_lines(&mut self) -> usize {
        let freed = std::mem::take(&mut self.bytes);
        self.commands.clear();
        self.streamed.clear();
        self.changed();
        freed
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

/// `line` as an entry keeps it: at most `cairn_model::LINE_CUT_BYTES`, ending on a character,
/// with the diff view's marker saying how much more there was.
fn cut_line(line: String) -> String {
    let (drawn, cut) = cairn_model::drawn_bytes(line.as_bytes());
    if !cut {
        return line;
    }
    let kept = drawn.len();
    let not_drawn = line.len() - kept;
    let mut shown = line;
    shown.truncate(kept);
    shown.push_str(&cairn_ui::cut_marker(not_drawn));
    shown
}

/// The session's operations, oldest first, and the popover's own state.
#[derive(Debug, Default)]
pub struct ActivityLog {
    entries: VecDeque<Activity>,
    /// The status box's laid-out left edge and bottom, which the popover hangs from.
    anchor: Option<(f32, f32)>,
    /// `Remove index.lock…` pressed: its ask, and the entry it was pressed on.
    lock_asked: Option<(OperationId, ActivityKey)>,
    /// The removal asked from it, whose success takes the offer from every entry.
    removing: Option<OperationId>,
    /// What the removal would cost, read at the press and not yet put in front of the person.
    lock_ready: Option<Consequence>,
    /// The bytes every entry's lines hold together.
    bytes: usize,
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

    /// Where the status box was laid out: its left edge and bottom in the window.
    pub fn anchor(&self) -> Option<(f32, f32)> {
        self.anchor
    }

    /// Whether `anchor` is not where the status box was last laid out.
    pub fn anchor_moved(&self, anchor: (f32, f32)) -> bool {
        self.anchor != Some(anchor)
    }

    pub fn set_anchor(&mut self, anchor: (f32, f32)) {
        self.anchor = Some(anchor);
    }

    /// `Remove index.lock…` pressed on `key`'s entry: asked under `asked`.
    pub fn lock_asked(&mut self, asked: OperationId, key: ActivityKey) {
        self.lock_asked = Some((asked, key));
        if let Some(entry) = self.find(key) {
            entry.lock_note = None;
        }
    }

    /// What the lock's removal would cost, read at the press asked under `asked`: kept for the
    /// confirmation (`use_lock_confirmation`) — or, refused, why, said beside the button. An
    /// answer to an older press is let go of.
    pub fn lock_answered(&mut self, asked: OperationId, outcome: Result<Consequence, String>) {
        let key = match self.lock_asked {
            Some((pressed, key)) if pressed == asked => key,
            _ => return,
        };
        self.lock_asked = None;
        match outcome {
            Ok(consequence) => self.lock_ready = Some(consequence),
            Err(why) => {
                if let Some(entry) = self.find(key) {
                    entry.lock_note = Some(why);
                }
            }
        }
    }

    pub fn has_lock_ready(&self) -> bool {
        self.lock_ready.is_some()
    }

    pub fn take_lock_ready(&mut self) -> Option<Consequence> {
        self.lock_ready.take()
    }

    /// The removal of the stale lock was asked under `id`.
    pub fn removal_asked(&mut self, id: OperationId) {
        self.removing = Some(id);
    }

    /// The lock files found as the repository opened: an entry of their own, each named, and
    /// — when the repository's own `index.lock` is among them — its removal offered.
    pub fn locks_at_open(&mut self, locks: &[std::path::PathBuf], index_lock: bool) {
        let mut entry = Activity::new(
            ActivityKey::Opened,
            LOCKS_AT_OPEN_NAME.to_owned(),
            None,
            false,
        );
        entry.outcome = Outcome::Found;
        entry.lock_named = index_lock;
        let lines: VecDeque<ActivityLine> = locks
            .iter()
            .map(|lock| ActivityLine::Output(cut_line(lock.display().to_string())))
            .collect();
        entry.bytes = lines.iter().map(line_bytes).sum();
        entry.commands = lines;
        self.bytes += entry.bytes;
        self.push(entry);
        self.bound();
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
            if let Some(oldest) = self.entries.pop_front() {
                self.bytes -= oldest.bytes;
            }
        }
    }

    /// Keeps every entry's lines together under [`ACTIVITY_BYTES`], reading counts alone: the
    /// oldest entries' lines let go of first, and — when the newest alone is over — its oldest
    /// lines, so a running operation keeps its latest output.
    fn bound(&mut self) {
        let newest = self.entries.len().saturating_sub(1);
        for entry in self.entries.iter_mut().take(newest) {
            if self.bytes <= ACTIVITY_BYTES {
                return;
            }
            self.bytes -= entry.let_go_by_the_bound();
        }
        if let Some(entry) = self.entries.back_mut() {
            while self.bytes > ACTIVITY_BYTES {
                let Some(freed) = entry.let_go_of_oldest() else {
                    break;
                };
                self.bytes -= freed;
            }
        }
    }

    /// A local write has started.
    pub fn write_started(&mut self, asked: &Asked) {
        self.push(Activity::new(
            ActivityKey::Write(asked.id),
            asked.name.clone(),
            asked.replaces,
            asked.cancellable,
        ));
    }

    /// Lines a running operation wrote, kept while it runs (R10.4), the latest
    /// [`ACTIVITY_LINES`].
    pub fn output(&mut self, key: ActivityKey, lines: &ScrubbedLines) {
        let Some(entry) = self.find(key) else {
            return;
        };
        // Once its commands are in they are drawn, and a late line is not.
        if !entry.commands.is_empty() {
            return;
        }
        let before = entry.bytes;
        for line in spoken_lines(lines) {
            let line = cut_line(line);
            entry.bytes += line.len();
            entry.streamed.push_back(line);
        }
        while entry.streamed.len() > ACTIVITY_LINES {
            if let Some(dropped) = entry.streamed.pop_front() {
                entry.bytes -= dropped.len();
            }
        }
        entry.changed();
        let after = entry.bytes;
        self.bytes = self.bytes + after - before;
        self.bound();
    }

    /// What an operation ran, and `Remove index.lock…`'s offer.
    pub fn ran(&mut self, key: ActivityKey, commands: &[CommandRecord], lock_named: bool) {
        let Some(entry) = self.find(key) else {
            return;
        };
        let freed = entry.let_go_of_lines();
        let mut kept: VecDeque<ActivityLine> = commands.iter().flat_map(command_lines).collect();
        while kept.len() > ACTIVITY_LINES {
            kept.pop_front();
        }
        entry.bytes = kept.iter().map(line_bytes).sum();
        entry.commands = kept;
        entry.lock_named = lock_named;
        entry.changed();
        let added = entry.bytes;
        self.bytes = self.bytes + added - freed;
        self.bound();
    }

    /// A local write has ended, as `ending` says (already scrubbed by the window); `asked` is
    /// the write as the window asked it, if it saw it asked: its name, for one that never
    /// started (the user's decision N, 2026-10-09), and the prompt its confirmation recorded,
    /// quoted however it ended (R12.1) — the token is spent by then, and only an ending that ran
    /// carries the prompt back.
    pub fn write_ended(&mut self, id: OperationId, ending: &WriteEnding, asked: Option<&Asked>) {
        let key = ActivityKey::Write(id);
        if self.find(key).is_none() {
            // Never started: not run, as the repository was closing — named as it was asked.
            let name = asked.map_or_else(|| "A write".to_owned(), |asked| asked.name.clone());
            self.push(Activity::new(key, name, None, false));
        }
        // The stale lock removed: no entry offers its removal any more.
        if self.removing == Some(id) && matches!(ending, WriteEnding::Done(_)) {
            self.removing = None;
            for entry in &mut self.entries {
                entry.lock_named = false;
                entry.lock_note = None;
            }
        }
        let Some(entry) = self.find(key) else {
            return;
        };
        let (outcome, acknowledged) = match ending {
            WriteEnding::Done(done) => (Outcome::Succeeded, done.acknowledged.clone()),
            WriteEnding::Incomplete { done, message, .. } => (
                Outcome::PartlyDone(message.clone()),
                done.acknowledged.clone(),
            ),
            WriteEnding::Stale { message, .. }
            | WriteEnding::Refused { message }
            | WriteEnding::NotRun { message } => (Outcome::NotRun(message.clone()), None),
            WriteEnding::Failed { message, .. } => (Outcome::Failed(message.clone()), None),
            WriteEnding::Cancelled { .. } => (Outcome::Cancelled, None),
            // An amend at the press that must be confirmed: nothing ran, for the reason its
            // confirmation will say.
            WriteEnding::NeedsConfirming { consequence } => {
                (Outcome::NotRun(consequence.prompt()), None)
            }
        };
        entry.prompt = acknowledged.or_else(|| asked.and_then(|asked| asked.prompt.clone()));
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
    pub fn shown(&self, git_running: bool) -> (Rc<Vec<ActivityEntry>>, usize) {
        let selected = self
            .selected
            .and_then(|key| self.newest_first().position(|entry| entry.key == key))
            .unwrap_or(0);
        // The selected entry's lines alone: the rest are drawn by name and status.
        let none: Rc<Vec<ActivityLine>> = Rc::new(Vec::new());
        let entries: Vec<ActivityEntry> = self
            .newest_first()
            .enumerate()
            .map(|(place, activity)| {
                let lines = if place == selected {
                    activity.lines()
                } else {
                    Rc::clone(&none)
                };
                entry_of(activity, lines, git_running)
            })
            .collect();
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
    let mut lines = vec![ActivityLine::Ran(cut_line(format!(
        "$ git {}",
        record
            .arguments
            .iter()
            .map(|argument| shell_quoted(argument))
            .collect::<Vec<_>>()
            .join(" ")
    )))];
    lines.extend(
        shown_lines(&record.stderr)
            .into_iter()
            .map(|line| ActivityLine::Output(cut_line(line))),
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

/// `argument` as a POSIX shell needs it written to read it back whole (the user's decision L,
/// 2026-10-09), so a `$ git …` line pastes into a terminal: bare when every character is one
/// no shell treats specially, otherwise in single quotes, an embedded `'` written `'\''`; an
/// empty argument is `''`.
pub fn shell_quoted(argument: &str) -> String {
    let bare = !argument.is_empty()
        && argument.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '-' | '_' | '.' | '/' | ':' | ',' | '+' | '=' | '@' | '%')
        });
    if bare {
        argument.to_owned()
    } else {
        format!("'{}'", argument.replace('\'', "'\\''"))
    }
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

fn entry_of(activity: &Activity, lines: Rc<Vec<ActivityLine>>, git_running: bool) -> ActivityEntry {
    ActivityEntry {
        name: activity.name.clone(),
        status: activity.outcome.status().to_owned(),
        started: started_text(activity.started),
        took: activity.took.map(took_text),
        message: activity.outcome.message(),
        prompt: activity.prompt.clone(),
        replaced: activity.replaced(),
        // Offered while the lock is named, and while a git of Cairn's runs still drawn, saying
        // why it cannot be pressed (the user's decision H).
        lock: activity.lock_named.then(|| {
            if git_running {
                LockOffer::Blocked(GIT_RUNNING_NOTE.to_owned())
            } else {
                LockOffer::Ready {
                    note: activity.lock_note.clone(),
                }
            }
        }),
        cancellable: activity.cancellable,
        lines,
    }
}

/// What a confirmation of `Remove index.lock…` is titled (the user's decision of 2026-10-09, G).
pub const REMOVE_LOCK_TITLE: &str = "Remove stale lock";

/// The popover, while it is open (R12.1): drawn over the window, its operations from the log.
/// A cancel asks the write's or the fetch's cancel; the way back finds the replaced commit in
/// Show Lost Commits; `Remove index.lock…` opens the confirmation, whose token asks the local
/// lane for the removal.
pub fn popover(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Option<Element> {
    // A git of Cairn's the window knows runs: a write, or a fetch.
    let git_running = view.writes.read().is_running() || view.fetch.read().is_in_flight();
    let (entries, selected, anchor) = {
        let log = view.activity.read();
        if !log.is_open() {
            return None;
        }
        let (entries, selected) = log.shown(git_running);
        (entries, selected, log.anchor())
    };
    let mut activity = view.activity;
    let cancelling = submit.clone();
    let returning = submit.clone();
    let removing = submit;
    Some(
        ActivityPopover::new(entries, selected)
            .anchor(anchor)
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
                    Some(ActivityKey::Opened) | None => {}
                }
            })
            .on_replaced(move |commit: Oid| {
                let mut activity = view.activity;
                activity.write().close();
                show_replaced(commit, view, returning.as_deref());
            })
            // What the removal would cost is read now, at the press, on the local lane (the
            // user's decision H); its answer opens the confirmation (`lock_answered`).
            .on_remove_lock(move |place: usize| {
                let Some(submit) = removing.as_deref() else {
                    return;
                };
                let mut activity = view.activity;
                let key = activity.peek().key_at(place);
                if let Some(key) = key {
                    let asked = OperationId::next();
                    activity.write().lock_asked(asked, key);
                    submit(Request::LockConsequence { asked });
                }
            })
            .on_close(move |()| {
                let mut activity = view.activity;
                activity.write().close();
            })
            .into(),
    )
}

/// The answer to a press of `Remove index.lock…`, put in front of the person (the user's
/// decision H, 2026-10-09): the confirmation opens over what the engine read at the press, its
/// token asking the local lane for the removal. Run by the window, which holds the submitter
/// the token's ask needs; a refusal was said beside the button as it arrived.
pub fn use_lock_confirmation(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    use_side_effect(move || {
        if !view.activity.read().has_lock_ready() {
            return;
        }
        let mut activity = view.activity;
        let Some(consequence) = activity.write().take_lock_ready() else {
            return;
        };
        let Some(submit) = submit.clone() else {
            return;
        };
        let mut confirming = view.confirming;
        confirming.set(Some(Confirming::new(
            REMOVE_LOCK_TITLE,
            consequence,
            move |token| {
                let mut writes = view.writes;
                let id = crate::local_writes::ask(
                    &mut writes.write(),
                    &*submit,
                    LocalWrite::RemoveLock(token),
                );
                let mut activity = view.activity;
                activity.write().removal_asked(id);
            },
        )));
    });
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
            name: crate::status_text::capitalised(what),
            awaited: "the commit to finish",
            replaces,
            cancellable: true,
            prompt: None,
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
            stderr: ScrubbedLines::scrubbing(stderr),
        }
    }

    /// A read's lines, as the engine would hand them on.
    fn said<S: AsRef<str>>(lines: &[S]) -> ScrubbedLines {
        let mut said = ScrubbedLines::new();
        for line in lines {
            said.append(&ScrubbedLines::scrubbing(line.as_ref()));
        }
        said
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
        log.write_ended(stage, &done(None), None);
        let replaced = Oid::from_bytes(&[0xab; 20]).unwrap();
        log.write_started(&asked(amend, "amend", Some(replaced)));
        log.output(
            ActivityKey::Write(amend),
            &said(&["hook: \u{1b}[31mok\u{1b}[0m"]),
        );
        let (entries, selected) = log.shown(false);
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
            false,
        );
        log.write_ended(amend, &done(Some("Amend abababa? It replaces …")), None);
        let (entries, _) = log.shown(false);
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
                output: ScrubbedLines::new(),
            },
            None,
        );
        let (entries, _) = log.shown(false);
        assert_eq!(entries[0].status, "failed");
        assert_eq!(entries[0].message.as_deref(), Some("git commit failed"));
        assert_eq!(entries[0].replaced, None);
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
            log.write_ended(id, &done(None), None);
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
        log.output(ActivityKey::Write(id), &said(&lines));
        let (entries, _) = log.shown(false);
        assert_eq!(entries[0].lines.len(), ACTIVITY_LINES);
        assert_eq!(
            entries[0].lines.last(),
            Some(&ActivityLine::Output(format!(
                "line {}",
                ACTIVITY_LINES + 6
            )))
        );
    }

    /// The bytes every entry keeps — its commands and what it wrote — read from the lines.
    fn held(log: &ActivityLog) -> usize {
        log.newest_first()
            .map(|entry| {
                entry.commands.iter().map(line_bytes).sum::<usize>()
                    + entry.streamed.iter().map(String::len).sum::<usize>()
            })
            .sum()
    }

    /// Phase 11's QA (app item 1): what is held stays under [`ACTIVITY_BYTES`] across many
    /// operations — their streamed output let go of once their commands arrive, never held
    /// uncounted beside them — and the count the log keeps is what its entries hold. Caught by:
    /// `streamed` kept after `ran`, or a count that drifts from the lines.
    #[test]
    fn what_is_held_stays_under_the_bound_across_many_operations() {
        let mut log = ActivityLog::default();
        let wide: Vec<String> = (0..1_000)
            .map(|n| format!("{n:05}{}", "w".repeat(1_500)))
            .collect();
        for n in 0..40 {
            let id = OperationId::for_tests(n);
            log.write_started(&asked(id, "commit", None));
            log.output(ActivityKey::Write(id), &said(&wide));
            log.ran(
                ActivityKey::Write(id),
                &[record(
                    &["commit"],
                    &wide[..200].join("\n"),
                    CommandExit::Code(1),
                )],
                false,
            );
            log.write_ended(id, &done(None), None);
            assert!(
                log.newest_first().all(|entry| entry.streamed.is_empty()),
                "what a finished operation wrote was kept beside its commands"
            );
        }
        assert!(log.bytes <= ACTIVITY_BYTES, "{} bytes held", log.bytes);
        assert_eq!(
            log.bytes,
            held(&log),
            "the count drifted from the lines held"
        );
        assert_eq!(
            log.newest_first().map(|entry| entry.bytes).sum::<usize>(),
            log.bytes
        );
    }

    /// Phase 11's QA (app item 1): a running operation alone over the budget keeps its latest
    /// lines, its oldest let go of — never its whole output. Caught by: a bound that clears the
    /// newest entry as it clears the old ones.
    #[test]
    fn a_running_operation_over_the_budget_keeps_its_latest_lines() {
        let mut log = ActivityLog::default();
        let id = OperationId::for_tests(1);
        log.write_started(&asked(id, "commit", None));
        for n in 0..4_000 {
            let line = format!("{n:05}{}", "x".repeat(1_500));
            log.output(ActivityKey::Write(id), &said(&[&line]));
        }
        assert!(log.bytes <= ACTIVITY_BYTES, "{}", log.bytes);
        let (entries, _) = log.shown(false);
        let lines = &entries[0].lines;
        assert!(lines.len() > 1_000, "{} lines kept", lines.len());
        assert!(
            matches!(lines.last(), Some(ActivityLine::Output(last)) if last.starts_with("03999")),
            "the latest line was let go of"
        );
    }

    /// Phase 11's QA (app item 1): with the popover closed, thirty thousand one-line outputs
    /// build no drawn lines — each costs the line it carries — and the open popover builds the
    /// selected entry's alone, once, until a line changes. Caught by: lines rebuilt per update.
    #[test]
    fn closed_the_popover_builds_no_lines() {
        let mut log = ActivityLog::default();
        let (older, newer) = (OperationId::for_tests(1), OperationId::for_tests(2));
        log.write_started(&asked(older, "commit", None));
        log.write_started(&asked(newer, "commit", None));
        for n in 0..30_000 {
            log.output(ActivityKey::Write(newer), &said(&[format!("line {n}")]));
        }
        assert!(
            log.newest_first()
                .all(|entry| entry.drawn.borrow().is_none())
        );
        let (entries, _) = log.shown(false);
        assert_eq!(entries[0].lines.len(), ACTIVITY_LINES);
        assert!(
            entries[1].lines.is_empty(),
            "an entry not selected built its lines"
        );
        let built = log
            .newest_first()
            .next()
            .and_then(|entry| entry.drawn.borrow().clone());
        let (again, _) = log.shown(false);
        assert!(
            built.is_some_and(|built| Rc::ptr_eq(&built, &again[0].lines)),
            "the lines were built again with nothing changed"
        );
    }

    /// Phase 11's QA (app item 3): a line past the long-line limit is kept cut on a character,
    /// with the diff view's marker saying how much more there was. Caught by: a 256 KiB piece
    /// kept, or cut inside a character.
    #[test]
    fn a_long_line_is_kept_cut_with_the_diff_views_marker() {
        let mut log = ActivityLog::default();
        let id = OperationId::for_tests(1);
        log.write_started(&asked(id, "commit", None));
        let long = format!("x{}", "é".repeat(128 * 1024));
        log.output(ActivityKey::Write(id), &said(&[&long]));
        let (entries, _) = log.shown(false);
        let Some(ActivityLine::Output(kept)) = entries[0].lines.first() else {
            panic!("{:?}", entries[0].lines);
        };
        let (drawn, _) = cairn_model::drawn_bytes(long.as_bytes());
        let marker = cairn_ui::cut_marker(long.len() - drawn.len());
        assert!(kept.ends_with(&marker), "{}", &kept[kept.len() - 40..]);
        assert_eq!(kept.len(), drawn.len() + marker.len());
        assert!(kept.len() <= cairn_model::LINE_CUT_BYTES + marker.len());
        assert_eq!(cut_line("short".to_owned()), "short");
    }

    /// R12.4's offer is kept with what the operation ran — pressable while no `git` of Cairn's
    /// runs and drawn saying why while one does (the user's decision H) — and a command that did
    /// not exit cleanly says how it ended.
    #[test]
    fn the_lock_offer_and_an_unclean_exit_are_kept() {
        let mut log = ActivityLog::default();
        let id = OperationId::for_tests(1);
        log.write_started(&asked(id, "staging 1 file", None));
        log.ran(
            ActivityKey::Write(id),
            &[record(
                &["add"],
                "fatal: Unable to create index.lock",
                CommandExit::Code(128),
            )],
            true,
        );
        let (entries, _) = log.shown(false);
        assert_eq!(entries[0].lock, Some(LockOffer::Ready { note: None }));
        let (entries, _) = log.shown(true);
        assert_eq!(
            entries[0].lock,
            Some(LockOffer::Blocked(GIT_RUNNING_NOTE.to_owned()))
        );
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

    fn stale_lock() -> Consequence {
        Consequence::RemoveLock {
            path: PathBuf::from("/r/.git/index.lock"),
            modified: SystemTime::UNIX_EPOCH,
            read_at: SystemTime::UNIX_EPOCH,
            bytes: 0,
            device: 1,
            inode: 2,
        }
    }

    /// The user's decision H: the removal's consequence is read at the press, and only the
    /// answer to the latest press is kept — for the confirmation, or, refused, said beside the
    /// button until the next press; a removal that succeeded takes the offer from every entry,
    /// and one that failed leaves it. Caught by: an older press's answer opening a confirmation,
    /// a refusal that says nothing, or an offer left on a lock that is gone.
    #[test]
    fn the_lock_offer_answers_its_latest_press_and_goes_once_the_lock_is_removed() {
        let mut log = ActivityLog::default();
        log.locks_at_open(&[PathBuf::from("/r/.git/index.lock")], true);
        let id = OperationId::for_tests(1);
        log.write_started(&asked(id, "staging 1 file", None));
        log.ran(ActivityKey::Write(id), &[], true);
        let (older, newer) = (OperationId::for_tests(10), OperationId::for_tests(11));
        log.lock_asked(older, ActivityKey::Write(id));
        log.lock_asked(newer, ActivityKey::Write(id));
        log.lock_answered(older, Ok(stale_lock()));
        assert!(!log.has_lock_ready(), "an older press's answer was kept");
        log.lock_answered(newer, Err("index.lock is no longer there".to_owned()));
        assert!(!log.has_lock_ready());
        let (entries, _) = log.shown(false);
        assert_eq!(
            entries[0].lock,
            Some(LockOffer::Ready {
                note: Some("index.lock is no longer there".to_owned())
            })
        );
        let again = OperationId::for_tests(12);
        log.lock_asked(again, ActivityKey::Write(id));
        let (entries, _) = log.shown(false);
        assert_eq!(entries[0].lock, Some(LockOffer::Ready { note: None }));
        log.lock_answered(again, Ok(stale_lock()));
        assert_eq!(log.take_lock_ready(), Some(stale_lock()));
        assert!(!log.has_lock_ready());

        // The removal fails: offered still. It succeeds: offered nowhere.
        let removal = OperationId::for_tests(20);
        log.removal_asked(removal);
        log.write_started(&asked(removal, "removing the stale lock", None));
        log.write_ended(
            removal,
            &WriteEnding::Refused {
                message: "a git of Cairn's is running".to_owned(),
            },
            None,
        );
        assert_eq!(
            log.shown(false)
                .0
                .iter()
                .filter(|e| e.lock.is_some())
                .count(),
            2
        );
        let removal = OperationId::for_tests(21);
        log.removal_asked(removal);
        log.write_started(&asked(removal, "removing the stale lock", None));
        log.write_ended(removal, &done(None), None);
        assert!(log.shown(false).0.iter().all(|entry| entry.lock.is_none()));
    }

    /// The user's decision H: the locks found as the repository opened are an entry of their
    /// own, found rather than run, each lock a line, the offer only where the index's is among
    /// them.
    #[test]
    fn the_locks_found_at_open_are_an_entry_of_their_own() {
        let mut log = ActivityLog::default();
        log.locks_at_open(&[PathBuf::from("/r/.git/HEAD.lock")], false);
        let (entries, _) = log.shown(false);
        assert_eq!(entries[0].name, LOCKS_AT_OPEN_NAME);
        assert_eq!(entries[0].status, "found");
        assert_eq!(entries[0].lock, None);
        assert_eq!(
            *entries[0].lines,
            [ActivityLine::Output("/r/.git/HEAD.lock".to_owned())]
        );
        log.locks_at_open(&[PathBuf::from("/r/.git/index.lock")], true);
        assert_eq!(
            log.shown(false).0[0].lock,
            Some(LockOffer::Ready { note: None })
        );
    }

    /// The user's decision L: each argument of a `$ git …` line is written as a POSIX shell
    /// reads it back whole — bare when nothing in it is special, single-quoted otherwise, an
    /// embedded quote `'\''`, an empty argument `''`, non-ASCII quoted. Checked against `sh`
    /// itself: each quoted argument, read back by the shell, is the argument. Caught by: a line
    /// that splits an argument with a space, or drops an empty one, when pasted.
    #[test]
    fn each_argument_is_quoted_as_a_shell_reads_it_back() {
        let cases = [
            ("add", "add"),
            ("--literal-pathspecs", "--literal-pathspecs"),
            ("src/main.rs", "src/main.rs"),
            ("refs/heads/a@b:c,d+e=f%g", "refs/heads/a@b:c,d+e=f%g"),
            ("my file.rs", "'my file.rs'"),
            ("it's", "'it'\\''s'"),
            ("", "''"),
            ("naïve.rs", "'naïve.rs'"),
            ("$HOME", "'$HOME'"),
            ("a\"b", "'a\"b'"),
            ("*.rs", "'*.rs'"),
            ("-", "-"),
        ];
        for (argument, quoted) in cases {
            assert_eq!(shell_quoted(argument), quoted, "{argument:?}");
        }
        // The shell reads each back whole: `printf '%s\0'` of the quoted words.
        let line = cases
            .iter()
            .map(|(argument, _)| shell_quoted(argument))
            .collect::<Vec<_>>()
            .join(" ");
        let read = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf '%s\\0' {line}"))
            .output()
            .expect("sh runs");
        let words: Vec<&str> = std::str::from_utf8(&read.stdout)
            .expect("utf-8")
            .split_terminator('\0')
            .collect();
        let arguments: Vec<&str> = cases.iter().map(|(argument, _)| *argument).collect();
        assert_eq!(words, arguments);
        // And the line drawn is built of them.
        let lines = command_lines(&record(
            &["commit", "-m", "it's done"],
            "",
            CommandExit::Code(0),
        ));
        assert_eq!(
            lines[0],
            ActivityLine::Ran("$ git commit -m 'it'\\''s done'".to_owned())
        );
    }

    /// The user's decision M: an operation whose lines the byte bound let go of says so in their
    /// place, and one that never had lines says nothing. Caught by: an empty pane for an
    /// operation that ran, after newer output pushed its lines out.
    #[test]
    fn an_operation_whose_lines_were_let_go_says_so() {
        let mut log = ActivityLog::default();
        let quiet = OperationId::for_tests(1);
        log.write_started(&asked(quiet, "commit", None));
        log.write_ended(quiet, &done(None), None);
        let old = OperationId::for_tests(2);
        log.write_started(&asked(old, "commit", None));
        log.ran(
            ActivityKey::Write(old),
            &[record(
                &["commit"],
                "a hook said this",
                CommandExit::Code(1),
            )],
            false,
        );
        log.write_ended(old, &done(None), None);
        let new = OperationId::for_tests(3);
        log.write_started(&asked(new, "commit", None));
        let wide: Vec<String> = (0..4_000)
            .map(|n| format!("{n:05}{}", "x".repeat(1_500)))
            .collect();
        log.output(ActivityKey::Write(new), &said(&wide));
        let lines: Vec<_> = log
            .newest_first()
            .map(|entry| (entry.key, entry.lines()))
            .collect();
        assert_eq!(
            *lines[1].1,
            [ActivityLine::Output(LINES_LET_GO.to_owned())],
            "{:?}",
            lines[1].0
        );
        assert!(
            lines[2].1.is_empty(),
            "an operation with no lines said they went"
        );
    }
}
