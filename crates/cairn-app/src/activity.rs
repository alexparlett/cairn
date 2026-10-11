//! Fork's Activity Manager as the window keeps it (staging-and-commit R12, C29): one entry per
//! operation of the session — every local write and every fetch — newest first, bounded, with
//! its one name, Fork's result line, when it started and how long it took, how it ended — one of
//! four words, R14.2 — its one sentence when it failed or was cancelled, the prompt it confirmed
//! when it ran, each `git` it ran with what that said, and an amend's way back. Nothing of it
//! outlives the window (R12.3, L14).
//!
//! Fed by `session::apply` from the updates the lanes send: a write's start, its output as it
//! is drained from the lane's tap (R10.4: a hook's lines while it runs), what it ran
//! (`Update::OperationRan`, the command log's records of its own `git`), and its ending; a
//! fetch's the same. Every text of git's kept here arrives as `cairn_model::ScrubbedLines`,
//! scrubbed of a URL's userinfo by the engine's runner as it split git's output (R12.2, R4.10),
//! so no entry can draw a token; it is only stripped of escape sequences here
//! (`crate::shown_output`). Each line is cut, as the diff view cuts one, at
//! `cairn_model::LINE_CUT_BYTES` on a character with the diff view's marker after it
//! (`cairn_ui::cut_marker`), so no row holds or shapes a 256 KiB piece.
//!
//! **One append-only list** (phase 20): an entry's lines are one bounded tail
//! (`crate::output_tail`) — what it wrote as it ran, then each `$ git …` it ran as its records
//! arrive, with each command's ending in one form; nothing already drawn is replaced. An
//! operation whose output streamed (a commit's hooks, a fetch's progress) keeps what streamed
//! as that output, and its records add their `$ git …` lines and endings beneath. An entry past
//! the tail's bound keeps its latest lines and says "Earlier output not kept." first; past
//! [`LOG_BYTES`] together, whole oldest entries go, never one still running and never the
//! newest.
//!
//! On the UI thread, each update costs what it carries, counted as lines go in and out; the
//! open popover reads a snapshot (`ActivityLog::shown`), the selected entry's lines shared by
//! a reference count. The popover is a child of the window ([`ActivityLayer`]) that alone reads
//! the log, and the window's root reads whether it is open ([`ActivityView::open`]), never the
//! log — so a commit's streamed line, which the commit box also keeps apart from the state the
//! root reads (`commit_box_state::StreamedOutput`), re-renders nothing while the popover is
//! closed, and only the popover while it is open. A fetch's progress does re-render the root,
//! by design: the status box in the title bar draws its latest line (`FetchStatus`).

use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use cairn_model::{CommandExit, CommandRecord, Oid, ScrubbedLines};
use cairn_ui::{ActivityEntry, ActivityLine, ActivityPopover, ActivityStatus};
use freya::prelude::*;

use crate::local_writes::Asked;
use crate::output_tail::{Measured, OUTPUT_BYTES, OutputTail};
use crate::shown_output::{shown_lines, spoken_lines};
use crate::window::View;
use crate::worker::Request;
use crate::worker::{OperationId, ResultLines, WriteEnding};

/// The most operations the popover keeps: the newest, the oldest let go of first.
pub const ACTIVITY_ENTRIES: usize = 200;
/// The most bytes of lines every operation together keeps — four of the one tail's bound: past
/// it, whole oldest operations go.
pub const LOG_BYTES: usize = 4 * OUTPUT_BYTES;

/// A cancelled write's one sentence: a commit or an amend git had not made (R4.7).
pub const WRITE_CANCELLED: &str = "Cancelled before git made it. Nothing was changed.";
/// A cancelled fetch's one sentence: what git had fetched before it was stopped stays, and the
/// refresh after it shows any ref it moved.
pub const FETCH_CANCELLED: &str = "Cancelled while git ran. Whatever it had already fetched stays.";

impl Measured for ActivityLine {
    fn lines(&self) -> usize {
        1
    }

    fn bytes(&self) -> usize {
        match self {
            ActivityLine::Ran(text) | ActivityLine::Output(text) => text.len(),
        }
    }
}

/// Which operation an entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKey {
    Write(OperationId),
    /// The `n`th fetch of the session.
    Fetch(u64),
}

/// How an operation stands, in one of the four words (rule 5, R14.2): running, succeeded,
/// failed, cancelled — a refusal and a partly done discard among the failures — with the one
/// sentence a failure or a cancel says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Succeeded,
    Failed(String),
    Cancelled(String),
}

impl Outcome {
    fn status(&self) -> ActivityStatus {
        match self {
            Self::Running => ActivityStatus::Running,
            Self::Succeeded => ActivityStatus::Succeeded,
            Self::Failed(_) => ActivityStatus::Failed,
            Self::Cancelled(_) => ActivityStatus::Cancelled,
        }
    }
}

/// One operation, as kept.
#[derive(Debug, Clone)]
pub struct Activity {
    pub key: ActivityKey,
    pub name: String,
    started: SystemTime,
    clock: Instant,
    took: Option<Duration>,
    outcome: Outcome,
    /// Fork's result line for each way it can end.
    results: ResultLines,
    /// The prompt its confirmation recorded: drawn once it ran (F3).
    prompt: Option<String>,
    /// It ran — git started, or it succeeded — so the prompt it confirmed is drawn.
    ran: bool,
    /// What Create Branch's Discard did, said once it succeeded in place of a prompt.
    happened: Option<String>,
    /// What an amend run at the press turned out to cost, said once beside its result (C35).
    reported: Option<String>,
    /// The commit an amend replaced: offered as the way back once the amend succeeded.
    replaced: Option<Oid>,
    pub cancellable: bool,
    /// Its output streamed as it ran: its records add their `$ git …` lines and endings alone.
    streamed: bool,
    /// What it wrote and ran, one append-only list in the one bounded tail.
    lines: OutputTail<ActivityLine>,
}

impl Activity {
    fn new(key: ActivityKey, name: String, cancellable: bool) -> Self {
        Self {
            key,
            name,
            started: SystemTime::now(),
            clock: Instant::now(),
            took: None,
            outcome: Outcome::Running,
            results: ResultLines::default(),
            prompt: None,
            ran: false,
            happened: None,
            reported: None,
            replaced: None,
            cancellable,
            streamed: false,
            lines: OutputTail::default(),
        }
    }

    fn of_write(asked: &Asked) -> Self {
        Self {
            results: asked.results.clone(),
            prompt: asked.prompt.clone(),
            happened: asked.happened.clone(),
            ..Self::new(
                ActivityKey::Write(asked.id),
                asked.name.clone(),
                asked.cancellable,
            )
        }
    }

    fn end(&mut self, outcome: Outcome) {
        self.took = Some(self.clock.elapsed());
        self.outcome = outcome;
        self.cancellable = false;
    }

    /// Fork's result line, for how it stands now.
    fn result(&self) -> Option<String> {
        match self.outcome {
            Outcome::Running => None,
            Outcome::Succeeded => self.results.succeeded.clone(),
            Outcome::Failed(_) => self.results.failed.clone(),
            Outcome::Cancelled(_) => self.results.cancelled.clone(),
        }
    }

    /// Its one sentence: a failure's or a cancel's, or what a success did beyond its name.
    fn sentence(&self) -> Option<String> {
        match &self.outcome {
            Outcome::Running => None,
            Outcome::Failed(said) | Outcome::Cancelled(said) => Some(said.clone()),
            Outcome::Succeeded => self.reported.clone().or_else(|| self.happened.clone()),
        }
    }

    /// The way back, once there is one: an amend that succeeded.
    fn way_back(&self) -> Option<Oid> {
        match self.outcome {
            Outcome::Succeeded => self.replaced,
            Outcome::Running | Outcome::Failed(_) | Outcome::Cancelled(_) => None,
        }
    }

    /// Lines after the rest, answering how many bytes it holds more — or fewer, past its bound.
    fn append(&mut self, lines: impl IntoIterator<Item = ActivityLine>, cut: bool) -> isize {
        let before = self.lines.bytes();
        self.lines.extend(lines);
        if cut {
            self.lines.mark_cut();
        }
        self.lines.bytes() as isize - before as isize
    }

    /// As the popover draws it: its lines only when it is the one selected.
    fn entry(&self, with_lines: bool, none: &Arc<VecDeque<ActivityLine>>) -> ActivityEntry {
        ActivityEntry {
            name: self.name.clone(),
            result: self.result(),
            status: self.outcome.status(),
            started: started_text(self.started),
            took: self.took.map(took_text),
            sentence: self.sentence(),
            prompt: self.prompt.clone().filter(|_| self.ran),
            replaced: self.way_back(),
            cancellable: self.cancellable,
            lines: if with_lines {
                self.lines.snapshot()
            } else {
                Arc::clone(none)
            },
            earlier_not_kept: with_lines && self.lines.cut(),
        }
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

/// The window's handles on the popover: the session's operations, and whether it is open —
/// two states, so the window's root, which reads only the second, is not drawn again for each
/// line the first takes in.
#[derive(Clone, Copy, PartialEq)]
pub struct ActivityView {
    pub log: State<ActivityLog>,
    pub open: State<bool>,
}

impl ActivityView {
    /// Each handle made by its hook, in the window's root.
    pub fn used() -> Self {
        Self {
            log: use_state(ActivityLog::default),
            open: use_state(|| false),
        }
    }

    /// Each handle made outside a component, for a test's view.
    #[cfg(test)]
    pub fn created() -> Self {
        Self {
            log: State::create(ActivityLog::default()),
            open: State::create(false),
        }
    }

    /// Opened from the status box: the newest entry selected.
    pub fn open(self) {
        let (mut log, mut open) = (self.log, self.open);
        log.write().opened();
        open.set(true);
    }

    pub fn close(self) {
        let mut open = self.open;
        open.set(false);
    }

    /// Whether it is open, read without subscribing: what a key handler asks.
    pub fn is_open(self) -> bool {
        *self.open.peek()
    }
}

/// The session's operations, oldest first, and the popover's own state.
#[derive(Debug, Default)]
pub struct ActivityLog {
    entries: VecDeque<Activity>,
    /// The status box's laid-out left edge and bottom, which the popover hangs from.
    anchor: Option<(f32, f32)>,
    /// The bytes every entry's lines hold together.
    bytes: usize,
    fetches: u64,
    /// The fetch running, if any.
    fetching: Option<u64>,
    /// The entry drawn on the right; the newest when none was chosen or it was let go of.
    selected: Option<ActivityKey>,
}

impl ActivityLog {
    /// The popover opened: the newest entry is the one drawn.
    fn opened(&mut self) {
        self.selected = None;
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

    /// Lets go of the entry at `at` and the bytes its lines hold.
    fn remove(&mut self, at: usize) {
        if let Some(entry) = self.entries.remove(at) {
            self.bytes -= entry.lines.bytes();
            if self.selected == Some(entry.key) {
                self.selected = None;
            }
        }
    }

    /// Lets go of `key`'s entry.
    fn forget(&mut self, key: ActivityKey) {
        if let Some(at) = self.entries.iter().position(|entry| entry.key == key) {
            self.remove(at);
        }
    }

    fn push(&mut self, entry: Activity) {
        self.entries.push_back(entry);
        self.bound(|log| log.entries.len() > ACTIVITY_ENTRIES);
    }

    /// Lines added to — or let go of by — `key`'s entry: the log's count kept, then its bound.
    fn grew(&mut self, by: isize) {
        self.bytes = self.bytes.saturating_add_signed(by);
        self.bound(|log| log.bytes > LOG_BYTES);
    }

    /// Lets go of whole oldest entries while `over` holds: never one still running — the
    /// status box and its updates read it — and never the newest.
    fn bound(&mut self, over: impl Fn(&Self) -> bool) {
        let mut at = 0;
        while over(self) && at + 1 < self.entries.len() {
            if matches!(self.entries[at].outcome, Outcome::Running) {
                at += 1;
            } else {
                self.remove(at);
            }
        }
    }

    /// A local write has started.
    pub fn write_started(&mut self, asked: &Asked) {
        self.push(Activity::of_write(asked));
    }

    /// What a running operation wrote, drained from its lane's tap (R10.4): its spoken lines
    /// after the rest, "Earlier output not kept." first once older ones went — the lane's or
    /// the entry's own.
    pub fn output(&mut self, key: ActivityKey, drained: &OutputTail<ScrubbedLines>) {
        let Some(entry) = self.find(key) else {
            return;
        };
        entry.streamed = true;
        let lines = drained
            .iter()
            .flat_map(spoken_lines)
            .map(|line| ActivityLine::Output(cut_line(line)));
        let by = entry.append(lines, drained.cut());
        self.grew(by);
    }

    /// What an operation ran, after what it wrote: each `$ git …` line and how that ended — with
    /// what it wrote to stderr, unless its output already streamed.
    pub fn ran(&mut self, key: ActivityKey, commands: &[CommandRecord]) {
        let Some(entry) = self.find(key) else {
            return;
        };
        let streamed = entry.streamed;
        let lines: Vec<ActivityLine> = commands
            .iter()
            .flat_map(|record| command_lines(record, !streamed))
            .collect();
        let by = entry.append(lines, false);
        self.grew(by);
    }

    /// A local write has ended, as `ending` says (already scrubbed by the engine); `asked` is
    /// the write as the window asked it, if it saw it asked: its name, for one that never
    /// started (the user's decision N, 2026-10-09).
    pub fn write_ended(&mut self, id: OperationId, ending: &WriteEnding, asked: Option<&Asked>) {
        let key = ActivityKey::Write(id);
        // An amend at the press that must be confirmed ran nothing: no entry; the amend
        // confirmed is an operation of its own.
        if matches!(ending, WriteEnding::NeedsConfirming { .. }) {
            self.forget(key);
            return;
        }
        if self.find(key).is_none() {
            // Never started: not run, as the repository was closing — named as it was asked.
            let entry = match asked {
                Some(asked) => Activity::of_write(asked),
                None => Activity::new(key, "A write".to_owned(), false),
            };
            self.push(entry);
        }
        let Some(entry) = self.find(key) else {
            return;
        };
        let (outcome, ran) = match ending {
            // An amend's way back is the commit its run answered it replaced (R12.1), never one
            // read from the request; and what it turned out to cost, read after git was
            // reaped, is said once, where it would now have needed confirming (C35).
            WriteEnding::Done(done) => {
                entry.replaced = done.replaced;
                entry.reported = done
                    .cost_after
                    .as_deref()
                    .and_then(|cost| cost.amend_reported());
                (Outcome::Succeeded, true)
            }
            WriteEnding::Incomplete { said, .. } => (Outcome::Failed(said.clone()), true),
            WriteEnding::Stale { said, .. }
            | WriteEnding::Refused { said }
            | WriteEnding::NotRun { said } => (Outcome::Failed(said.clone()), false),
            // git ran and failed — or something beneath git did, before it ran.
            WriteEnding::Failed(failure) => (
                Outcome::Failed(failure.said.clone()),
                failure.command.is_some(),
            ),
            // A cancel heard before git ran ran nothing, and quotes no prompt (F3).
            WriteEnding::Cancelled { ran, .. } => {
                (Outcome::Cancelled(WRITE_CANCELLED.to_owned()), *ran)
            }
            WriteEnding::NeedsConfirming { .. } => return,
        };
        entry.ran = ran;
        entry.end(outcome);
    }

    /// A fetch of `remote` has started.
    pub fn fetch_started(&mut self, remote: &str) {
        self.fetches += 1;
        self.fetching = Some(self.fetches);
        self.push(Activity::new(
            ActivityKey::Fetch(self.fetches),
            format!("Fetch {remote}"),
            true,
        ));
    }

    /// The fetch running, if any — what a fetch's lines and its ending are of.
    pub fn fetch_key(&self) -> Option<ActivityKey> {
        self.fetching.map(ActivityKey::Fetch)
    }

    /// The fetch running, for the status box (R14.1, F9): its number, its one name and when it
    /// started.
    pub fn fetch_running(&self) -> Option<(u64, String, Instant)> {
        let n = self.fetching?;
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.key == ActivityKey::Fetch(n))
            .map(|entry| (n, entry.name.clone(), entry.clock))
    }

    /// The fetch of `remote` has ended — one that never started (refused before git ran) is
    /// an entry of its own.
    pub fn fetch_ended(&mut self, remote: &str, outcome: Outcome) {
        let key = match self.fetching.take() {
            Some(n) => ActivityKey::Fetch(n),
            None => {
                self.fetches += 1;
                let key = ActivityKey::Fetch(self.fetches);
                self.push(Activity::new(key, format!("Fetch {remote}"), false));
                key
            }
        };
        if let Some(entry) = self.find(key) {
            entry.end(outcome);
        }
    }

    /// What the window's popover draws: every entry, newest first, and the place of the one
    /// selected — the newest when none is — whose lines alone are handed over, as a snapshot.
    pub fn shown(&self) -> (Rc<Vec<ActivityEntry>>, usize) {
        let selected = self
            .selected
            .and_then(|key| self.newest_first().position(|entry| entry.key == key))
            .unwrap_or(0);
        let none = Arc::new(VecDeque::new());
        let entries: Vec<ActivityEntry> = self
            .newest_first()
            .enumerate()
            .map(|(place, activity)| activity.entry(place == selected, &none))
            .collect();
        (Rc::new(entries), selected)
    }

    /// The key of the entry at `place`, newest first.
    pub fn key_at(&self, place: usize) -> Option<ActivityKey> {
        self.newest_first().nth(place).map(|entry| entry.key)
    }
}

/// A record of the command log as the popover draws it: `$ git <arguments>`, then — with
/// `output` — what it wrote to stderr, then how it ended, in one form, when that was not a
/// clean exit: `exit code N`, `killed by signal N` or `cancelled`.
fn command_lines(record: &CommandRecord, output: bool) -> Vec<ActivityLine> {
    let mut lines = vec![ActivityLine::Ran(cut_line(format!(
        "$ git {}",
        record
            .arguments
            .iter()
            .map(|argument| shell_quoted(argument))
            .collect::<Vec<_>>()
            .join(" ")
    )))];
    if output {
        lines.extend(
            shown_lines(&record.stderr)
                .into_iter()
                .map(|line| ActivityLine::Output(cut_line(line))),
        );
    }
    lines.extend(ending_line(record).map(ActivityLine::Output));
    lines
}

/// How a command ended, in one form, when that was not a clean exit; nothing where how it
/// ended is not git's to say (never started, or not known).
fn ending_line(record: &CommandRecord) -> Option<String> {
    if record.cancelled {
        return Some("cancelled".to_owned());
    }
    match record.exit {
        CommandExit::Code(0) | CommandExit::NotStarted | CommandExit::Unknown => None,
        CommandExit::Code(code) => Some(format!("exit code {code}")),
        CommandExit::Signal(signal) => Some(format!("killed by signal {signal}")),
    }
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
/// dates are drawn (F7).
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

/// How long an operation took, as the popover says it (F8).
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

/// The popover, while it is open (R12.1): a child of the window, the one reader of the log, so
/// only it is drawn again as an operation's lines arrive. A cancel asks the write's or the
/// fetch's cancel; the way back shows the replaced commit in Show Lost Commits.
pub struct ActivityLayer {
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
}

impl ActivityLayer {
    pub fn new(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Self {
        Self { view, submit }
    }
}

// By the handles it reads: the submitter is the same repository's however often it is built.
impl PartialEq for ActivityLayer {
    fn eq(&self, other: &Self) -> bool {
        self.view.activity == other.view.activity
            && self.view.fetch == other.view.fetch
            && self.submit.is_some() == other.submit.is_some()
    }
}

impl Component for ActivityLayer {
    fn render(&self) -> impl IntoElement {
        let view = self.view;
        let (entries, selected, anchor) = {
            let log = view.activity.log.read();
            let (entries, selected) = log.shown();
            (entries, selected, log.anchor())
        };
        let mut log = view.activity.log;
        let cancelling = self.submit.clone();
        let returning = self.submit.clone();
        ActivityPopover::new(entries, selected)
            .anchor(anchor)
            .on_select(move |place: usize| {
                let key = log.peek().key_at(place);
                if let Some(key) = key {
                    log.write().choose(key);
                }
            })
            .on_cancel(move |place: usize| {
                let key = view.activity.log.peek().key_at(place);
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
                view.activity.close();
                show_in_lost_commits(commit, view, returning.as_deref());
            })
            .on_close(move |()| view.activity.close())
    }
}

/// The way back from an amend (R12.1, F4): Show Lost Commits turned on, if it is not, and the
/// replaced commit found in the history and selected, as a ref's press finds its row.
fn show_in_lost_commits(commit: Oid, view: View, submit: Option<&dyn Fn(Request)>) {
    crate::ref_find::find_commit(commit, commit.short().as_str().to_owned(), view, submit);
    // After the find: the reopen asks it again of the walk that draws lost commits.
    if !*view.show_lost.peek() {
        crate::lost_commits::toggle(view, submit);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use cairn_model::{Consequence, Publication, RefName, Reflog};

    use super::*;
    use crate::worker::Done;

    fn asked(id: OperationId, name: &str) -> Asked {
        Asked {
            id,
            name: name.to_owned(),
            cancellable: true,
            prompt: None,
            happened: None,
            results: ResultLines::default(),
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

    /// One drain of a lane's tap holding `lines`, as the window takes it.
    fn drained<S: AsRef<str>>(lines: &[S]) -> OutputTail<ScrubbedLines> {
        let mut tail = OutputTail::default();
        for line in lines {
            tail.push(ScrubbedLines::scrubbing(line.as_ref()));
        }
        tail
    }

    fn done() -> Done {
        Done {
            description: "done".to_owned(),
            acknowledged: None,
            locks_before: Vec::new(),
            locks_after: Vec::new(),
            replaced: None,
            cost_after: None,
        }
    }

    fn failed(said: &str, command: Option<&str>) -> WriteEnding {
        WriteEnding::Failed(crate::worker::Failure {
            said: said.to_owned(),
            command: command.map(str::to_owned),
            output: ScrubbedLines::new(),
            locks: Vec::new(),
            index_lock: false,
        })
    }

    fn texts(lines: &VecDeque<ActivityLine>) -> Vec<&str> {
        lines
            .iter()
            .map(|line| match line {
                ActivityLine::Ran(text) | ActivityLine::Output(text) => text.as_str(),
            })
            .collect()
    }

    /// C29: an entry per operation, newest first — its name and Fork's result line, the four
    /// words, its one sentence on a failure; what it wrote as it ran, then its `$ git …` lines
    /// appended beneath without replacing a line, git's stderr not drawn twice for an operation
    /// whose output streamed; and an amend's way back once it succeeded. Caught by: entries
    /// oldest first, the streamed lines swapped for the records, a result line for an ending it
    /// was not given, or the way back offered for an amend that failed.
    #[test]
    fn each_operation_is_an_entry_with_its_result_its_git_and_its_way_back() {
        let mut log = ActivityLog::default();
        let (stage, amend) = (OperationId::for_tests(1), OperationId::for_tests(2));
        log.write_started(&Asked {
            results: ResultLines {
                succeeded: Some("staged".to_owned()),
                failed: Some("stage failed".to_owned()),
                cancelled: None,
            },
            ..asked(stage, "Stage 1 file")
        });
        log.ran(
            ActivityKey::Write(stage),
            &[record(
                &["add", "a.rs"],
                "warning: LF",
                CommandExit::Code(0),
            )],
        );
        log.write_ended(stage, &WriteEnding::Done(done()), None);
        log.write_started(&asked(amend, "Amend"));
        log.output(
            ActivityKey::Write(amend),
            &drained(&["hook: \u{1b}[31mok\u{1b}[0m"]),
        );
        let (entries, selected) = log.shown();
        assert_eq!(selected, 0);
        assert_eq!(entries[0].name, "Amend");
        assert_eq!(entries[0].status, ActivityStatus::Running);
        assert_eq!(entries[0].result, None, "a result line while it runs");
        assert!(entries[0].cancellable);
        let streamed = Arc::clone(&entries[0].lines);
        assert_eq!(texts(&streamed), ["hook: ok"]);
        log.ran(
            ActivityKey::Write(amend),
            &[record(
                &["commit", "-q", "--amend", "-F", "-"],
                "hook: ok",
                CommandExit::Code(0),
            )],
        );
        let replaced = Oid::from_bytes(&[0xab; 20]).unwrap();
        log.write_ended(
            amend,
            &WriteEnding::Done(Done {
                replaced: Some(replaced),
                ..done()
            }),
            None,
        );
        let (entries, _) = log.shown();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["Amend", "Stage 1 file"]
        );
        let amended = &entries[0];
        assert_eq!(amended.status, ActivityStatus::Succeeded);
        assert!(!amended.cancellable);
        assert!(amended.took.is_some());
        assert_eq!(amended.replaced, Some(replaced));
        assert_eq!(
            amended.sentence, None,
            "a recoverable amend reported something"
        );
        assert_eq!(
            texts(&amended.lines),
            ["hook: ok", "$ git commit -q --amend -F -"],
            "the streamed lines were replaced, or git's stderr drawn twice"
        );
        assert_eq!(
            texts(&streamed),
            ["hook: ok"],
            "a snapshot changed under its reader"
        );
        assert_eq!(entries[1].result.as_deref(), Some("staged"));
        assert_eq!(entries[1].replaced, None);
        log.choose(ActivityKey::Write(stage));
        let (entries, selected) = log.shown();
        assert_eq!(selected, 1);
        assert!(
            entries[0].lines.is_empty(),
            "an entry not selected handed its lines"
        );
        assert_eq!(texts(&entries[1].lines), ["$ git add a.rs", "warning: LF"]);

        // A stage that failed: Fork's "stage failed", ⚠'s status, its one sentence.
        let refused = OperationId::for_tests(3);
        log.write_started(&Asked {
            results: ResultLines {
                succeeded: Some("staged".to_owned()),
                failed: Some("stage failed".to_owned()),
                cancelled: None,
            },
            ..asked(refused, "Stage 1 file")
        });
        log.write_ended(refused, &failed("git refused it.", Some("git add")), None);
        // An amend that failed points nowhere: nothing was replaced.
        let broken = OperationId::for_tests(4);
        log.write_started(&asked(broken, "Amend"));
        log.write_ended(
            broken,
            &failed("git commit failed.", Some("git commit")),
            None,
        );
        let (entries, _) = log.shown();
        assert_eq!(entries[0].status, ActivityStatus::Failed);
        assert_eq!(entries[0].sentence.as_deref(), Some("git commit failed."));
        assert_eq!(entries[0].replaced, None);
        assert_eq!(entries[1].result.as_deref(), Some("stage failed"));
        assert_eq!(entries[1].sentence.as_deref(), Some("git refused it."));
    }

    /// F3 and the user's decision of 2026-10-10 on phase 14's item 11: the prompt an operation
    /// confirmed is drawn whole only once it ran — git ran it, failed, was cancelled while it
    /// ran, partly did it, or succeeded — never for one refused, stale, not run or cancelled
    /// before git ran (phase 20's QA items 7 and 8); and Create Branch's Discard quotes none,
    /// saying once it succeeded what it did instead. Caught by: "Confirmed:" over an operation
    /// that never happened, a partly done discard's prompt hidden, or the forced checkout's
    /// question quoted.
    #[test]
    fn the_prompt_is_drawn_only_for_an_operation_that_ran() {
        let prompt = "Discard all changes in a.rs? You can't undo this.";
        let endings = [
            (WriteEnding::Done(done()), true),
            (failed("git failed.", Some("git restore")), true),
            (
                WriteEnding::Cancelled {
                    locks: Vec::new(),
                    ran: true,
                },
                true,
            ),
            (
                WriteEnding::Cancelled {
                    locks: Vec::new(),
                    ran: false,
                },
                false,
            ),
            (
                WriteEnding::Incomplete {
                    done: done(),
                    kept: vec!["b.rs".to_owned()],
                    said: "b.rs was kept as it was.".to_owned(),
                    failure: None,
                },
                true,
            ),
            (failed("a.rs could not be read.", None), false),
            (
                WriteEnding::Refused {
                    said: "Nothing is selected.".to_owned(),
                },
                false,
            ),
            (
                WriteEnding::Stale {
                    path: "a.rs".to_owned(),
                    said: "a.rs changed since you confirmed.".to_owned(),
                },
                false,
            ),
            (
                WriteEnding::NotRun {
                    said: "The repository was closing.".to_owned(),
                },
                false,
            ),
        ];
        for (n, (ending, ran)) in endings.iter().enumerate() {
            let mut log = ActivityLog::default();
            let id = OperationId::for_tests(n as u64);
            log.write_started(&Asked {
                prompt: Some(prompt.to_owned()),
                ..asked(id, "Discard 1 file")
            });
            log.write_ended(id, ending, None);
            let (entries, _) = log.shown();
            assert_eq!(
                entries[0].prompt.as_deref(),
                ran.then_some(prompt),
                "{ending:?}"
            );
        }
        let mut log = ActivityLog::default();
        let id = OperationId::for_tests(9);
        let happened = "Discarded local changes and any untracked files in the way, then \
                        checked out 'topic' at abababa.";
        log.write_started(&Asked {
            happened: Some(happened.to_owned()),
            results: ResultLines {
                succeeded: Some("Switched to branch 'topic'".to_owned()),
                ..ResultLines::default()
            },
            ..asked(id, "Create branch 'topic'")
        });
        log.write_ended(id, &WriteEnding::Done(done()), None);
        let (entries, _) = log.shown();
        assert_eq!(entries[0].prompt, None);
        assert_eq!(entries[0].sentence.as_deref(), Some(happened));
        assert_eq!(
            entries[0].result.as_deref(),
            Some("Switched to branch 'topic'")
        );
    }

    /// C35: an amend run at the press whose cost, read once git was reaped, says it would now
    /// have needed confirming reports so in its entry, once, beside its result — and one still
    /// recoverable, or whose read failed, reports nothing. Caught by: the engine's answer never
    /// drawn, or drawn for a recoverable amend.
    #[test]
    fn an_amend_pushed_while_it_ran_says_so_in_its_entry() {
        let pushed = Consequence::Amend {
            commit: Oid::from_bytes(&[0x3f; 20]).unwrap(),
            published: Publication::Upstream(RefName::new("refs/remotes/origin/main")),
            reflog: Reflog::Written,
        };
        let recoverable = Consequence::Amend {
            commit: Oid::from_bytes(&[0x3f; 20]).unwrap(),
            published: Publication::Unpublished,
            reflog: Reflog::Written,
        };
        for (n, (after, said)) in [
            (
                Some(pushed),
                Some(
                    "Amended 3f3f3f3, which was pushed to origin/main while the amend ran. \
                     Sharing the amended commit needs a force push.",
                ),
            ),
            (Some(recoverable), None),
            (None, None),
        ]
        .into_iter()
        .enumerate()
        {
            let mut log = ActivityLog::default();
            let id = OperationId::for_tests(n as u64);
            log.write_started(&asked(id, "Amend"));
            log.write_ended(
                id,
                &WriteEnding::Done(Done {
                    replaced: Some(Oid::from_bytes(&[0x3f; 20]).unwrap()),
                    cost_after: after.map(Box::new),
                    ..done()
                }),
                None,
            );
            let (entries, _) = log.shown();
            assert_eq!(entries[0].sentence.as_deref(), said);
            assert_eq!(entries[0].status, ActivityStatus::Succeeded);
            assert_eq!(
                entries[0].prompt, None,
                "an amend at the press confirmed nothing"
            );
        }
    }

    /// C29's bounds: at most [`ACTIVITY_ENTRIES`] operations; one operation's lines the one
    /// tail's latest, "Earlier output not kept." heading them — whether the lane or the entry let
    /// go; and past [`LOG_BYTES`] together, whole oldest operations go, never one still running
    /// and never the newest. Caught by: an unbounded log, lines of an old entry trimmed instead
    /// of the entry going, the newest cut while it fits, or a running fetch evicted.
    #[test]
    fn whole_oldest_entries_go_at_the_bound_and_an_entry_alone_is_cut() {
        let mut log = ActivityLog::default();
        for n in 0..ACTIVITY_ENTRIES as u64 + 5 {
            let id = OperationId::for_tests(n);
            log.write_started(&asked(id, &format!("Write {n}")));
            log.write_ended(id, &WriteEnding::Done(done()), None);
        }
        assert_eq!(log.len(), ACTIVITY_ENTRIES);
        assert_eq!(
            log.newest_first().last().map(|entry| entry.name.as_str()),
            Some("Write 5")
        );

        // One operation alone past the tail's bound keeps its latest lines, said cut.
        let mut log = ActivityLog::default();
        let id = OperationId::for_tests(1);
        log.write_started(&asked(id, "Commit"));
        let lines: Vec<String> = (0..crate::output_tail::OUTPUT_LINES + 7)
            .map(|n| format!("line {n}"))
            .collect();
        log.output(ActivityKey::Write(id), &drained(&[lines.join("\n")]));
        let (entries, _) = log.shown();
        assert!(entries[0].earlier_not_kept);
        assert_eq!(entries[0].lines.len(), crate::output_tail::OUTPUT_LINES);
        assert_eq!(
            entries[0].lines.back(),
            Some(&ActivityLine::Output(format!(
                "line {}",
                crate::output_tail::OUTPUT_LINES + 6
            )))
        );
        // The lane's cut is said too.
        let lane_cut = OperationId::for_tests(2);
        log.write_started(&asked(lane_cut, "Commit"));
        let mut cut = drained(&["the latest"]);
        cut.mark_cut();
        log.output(ActivityKey::Write(lane_cut), &cut);
        assert!(log.shown().0[0].earlier_not_kept);

        // Past the log's bound, whole oldest entries go — the running fetch and the newest kept.
        let mut log = ActivityLog::default();
        log.fetch_started("origin");
        let wide: Vec<String> = (0..400)
            .map(|n| format!("{n:05}{}", "w".repeat(2_000)))
            .collect();
        for n in 0..12 {
            let id = OperationId::for_tests(n);
            log.write_started(&asked(id, &format!("Commit {n}")));
            log.output(ActivityKey::Write(id), &drained(&wide));
            log.write_ended(id, &WriteEnding::Done(done()), None);
            assert!(log.bytes <= LOG_BYTES, "{} bytes held", log.bytes);
        }
        let names: Vec<&str> = log
            .newest_first()
            .map(|entry| entry.name.as_str())
            .collect();
        // Exactly as many whole commits as fit under the bound, the newest of them, then the
        // running fetch (phase 20's QA item 5).
        let each: usize = wide.iter().map(String::len).sum();
        let fit = LOG_BYTES / each;
        assert!(
            (1..12).contains(&fit),
            "{fit} entries fit: the test's sizes are wrong"
        );
        let expected: Vec<String> = (12 - fit..12)
            .rev()
            .map(|n| format!("Commit {n}"))
            .chain(std::iter::once("Fetch origin".to_owned()))
            .collect();
        assert_eq!(
            names, expected,
            "the entries kept are not the newest that fit"
        );
        assert!(
            log.newest_first()
                .filter(|entry| entry.key != ActivityKey::Fetch(1))
                .all(|entry| !entry.lines.cut()),
            "an entry's lines were trimmed rather than whole entries let go"
        );
        assert_eq!(
            log.bytes,
            log.newest_first()
                .map(|entry| entry.lines.bytes())
                .sum::<usize>(),
            "the count drifted from the lines held"
        );
    }

    /// The open popover reads a snapshot: with nothing changed, it is handed the same lines;
    /// a line taken in hands the next a new one, and an entry not selected hands none. Caught
    /// by: lines copied for every read, or a snapshot changed in place under the popover.
    #[test]
    fn the_popover_reads_a_shared_snapshot() {
        let mut log = ActivityLog::default();
        let id = OperationId::for_tests(1);
        log.write_started(&asked(id, "Commit"));
        log.output(ActivityKey::Write(id), &drained(&["one"]));
        let (first, _) = log.shown();
        let (again, _) = log.shown();
        assert!(Arc::ptr_eq(&first[0].lines, &again[0].lines));
        log.output(ActivityKey::Write(id), &drained(&["two"]));
        let (after, _) = log.shown();
        assert!(!Arc::ptr_eq(&first[0].lines, &after[0].lines));
        assert_eq!(texts(&first[0].lines), ["one"]);
        assert_eq!(texts(&after[0].lines), ["one", "two"]);
    }

    /// Phase 11's QA (app item 3): a line past the long-line limit is kept cut on a character,
    /// with the diff view's marker saying how much more there was. Caught by: a 256 KiB piece
    /// kept, or cut inside a character.
    #[test]
    fn a_long_line_is_kept_cut_with_the_diff_views_marker() {
        let mut log = ActivityLog::default();
        let id = OperationId::for_tests(1);
        log.write_started(&asked(id, "Commit"));
        let long = format!("x{}", "é".repeat(128 * 1024));
        log.output(ActivityKey::Write(id), &drained(&[&long]));
        let (entries, _) = log.shown();
        let Some(ActivityLine::Output(kept)) = entries[0].lines.front() else {
            panic!("{:?}", entries[0].lines);
        };
        let (drawn, _) = cairn_model::drawn_bytes(long.as_bytes());
        let marker = cairn_ui::cut_marker(long.len() - drawn.len());
        assert!(kept.ends_with(&marker), "{}", &kept[kept.len() - 40..]);
        assert_eq!(kept.len(), drawn.len() + marker.len());
        assert!(kept.len() <= cairn_model::LINE_CUT_BYTES + marker.len());
        assert_eq!(cut_line("short".to_owned()), "short");
    }

    /// C29: each command ends in one form — `exit code N`, `killed by signal N`, `cancelled` —
    /// a clean exit, one never started and one whose end is not known saying nothing; and the
    /// times read as the popover draws them. Caught by: "(not started)", "(how it ended is not
    /// known)", "exit status", or two forms for one end.
    #[test]
    fn each_command_ends_in_one_form() {
        let ended = |exit, cancelled| {
            ending_line(&CommandRecord {
                cancelled,
                ..record(&["add"], "", exit)
            })
        };
        assert_eq!(ended(CommandExit::Code(0), false), None);
        assert_eq!(
            ended(CommandExit::Code(128), false).as_deref(),
            Some("exit code 128")
        );
        assert_eq!(
            ended(CommandExit::Signal(9), false).as_deref(),
            Some("killed by signal 9")
        );
        assert_eq!(
            ended(CommandExit::Signal(15), true).as_deref(),
            Some("cancelled")
        );
        assert_eq!(
            ended(CommandExit::Code(0), true).as_deref(),
            Some("cancelled")
        );
        assert_eq!(ended(CommandExit::NotStarted, false), None);
        assert_eq!(ended(CommandExit::Unknown, false), None);
        let lines = command_lines(
            &record(
                &["add"],
                "fatal: Unable to create index.lock",
                CommandExit::Code(128),
            ),
            true,
        );
        assert_eq!(
            lines,
            [
                ActivityLine::Ran("$ git add".to_owned()),
                ActivityLine::Output("fatal: Unable to create index.lock".to_owned()),
                ActivityLine::Output("exit code 128".to_owned()),
            ]
        );
        assert_eq!(took_text(Duration::from_millis(12)), "12 ms");
        assert_eq!(took_text(Duration::from_millis(1_500)), "1.5 s");
        assert_eq!(took_text(Duration::from_secs(125)), "2 min 5 s");
        assert_eq!(
            started_text(SystemTime::UNIX_EPOCH + Duration::from_secs(3_723)),
            "01:02:03 UTC"
        );
    }

    /// Rule 5 (R14.2): every ending is one of four words — a refusal, a stale patch, a write
    /// never run and a partly done discard all failed, with their sentence; a cancel is
    /// cancelled with its own — and an amend asking to be confirmed, which ran nothing, leaves
    /// no entry. Caught by: another word drawn, a cancel with nothing said, or a pending amend
    /// drawn as an operation.
    #[test]
    fn every_ending_is_one_of_four_words() {
        let mut log = ActivityLog::default();
        let refused = OperationId::for_tests(1);
        log.write_started(&asked(refused, "Stage 1 file"));
        log.write_ended(
            refused,
            &WriteEnding::Refused {
                said: "Nothing is selected.".to_owned(),
            },
            None,
        );
        let (entries, _) = log.shown();
        assert_eq!(entries[0].status, ActivityStatus::Failed);
        assert_eq!(entries[0].sentence.as_deref(), Some("Nothing is selected."));
        let pending = OperationId::for_tests(2);
        log.write_started(&asked(pending, "Amend"));
        log.write_ended(
            pending,
            &WriteEnding::NeedsConfirming {
                consequence: Consequence::RemoveLock {
                    path: PathBuf::from("/r/.git/index.lock"),
                    modified: SystemTime::UNIX_EPOCH,
                    read_at: SystemTime::UNIX_EPOCH,
                    bytes: 0,
                    device: 0,
                    inode: 0,
                },
            },
            None,
        );
        assert_eq!(
            log.len(),
            1,
            "an amend asking to be confirmed left an entry"
        );
        let cancelled = OperationId::for_tests(3);
        log.write_started(&asked(cancelled, "Commit"));
        log.write_ended(
            cancelled,
            &WriteEnding::Cancelled {
                locks: Vec::new(),
                ran: true,
            },
            None,
        );
        log.fetch_started("origin");
        log.fetch_ended("origin", Outcome::Cancelled(FETCH_CANCELLED.to_owned()));
        let (entries, _) = log.shown();
        assert_eq!(entries[0].status, ActivityStatus::Cancelled);
        assert_eq!(entries[0].sentence.as_deref(), Some(FETCH_CANCELLED));
        assert_eq!(entries[1].status, ActivityStatus::Cancelled);
        assert_eq!(entries[1].sentence.as_deref(), Some(WRITE_CANCELLED));
        for entry in entries.iter() {
            assert!(
                ["running", "succeeded", "failed", "cancelled"].contains(&entry.status.word()),
                "{}",
                entry.status.word()
            );
        }
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
        let lines = command_lines(
            &record(&["commit", "-m", "it's done"], "", CommandExit::Code(0)),
            true,
        );
        assert_eq!(
            lines[0],
            ActivityLine::Ran("$ git commit -m 'it'\\''s done'".to_owned())
        );
    }
}
