//! The commit box under Local Changes' diff (staging-and-commit R10, L9, L12), Fork's as
//! `docs/research/staging-and-commit/fork-staging-and-commit.md` section 5 records it: a
//! one-line subject with Fork's characters-left counter and the `≡` that opens Recent Commit
//! Messages, a description with a ruler at column 72, `Amend` at the left and the one button at
//! the right.
//!
//! **What it draws, and what it decides.** It draws what it is handed and reports what is
//! pressed; the window decides what the press does (`crates/cairn-app/src/commit_box_pane.rs`).
//! The subject and the description are the window's own text (`State<String>`), bound two ways
//! to the fields, so the draft is the window's for its life (R10.7) — Amend ticked and unticked,
//! a refresh, a failed hook — and a value the window writes (a recalled message, `HEAD`'s, a
//! merge's) is what the field shows; an edit the person makes in either field is reported
//! ([`CommitBox::on_edit`]), so the window knows the draft is theirs and fills it no more (C7).
//! Both fields take the one key policy, heard in the commit box's scope
//! (`FieldScope::CommitBox`): ⌘Return or Ctrl+Enter presses the button without a new line, the
//! window's chords still reach the window, and no list or staging chord fires (R7.3 as amended);
//! the subject offers a bare ↑ or ↓ to the window first, which recalls a recent message while
//! the subject is empty or holds the one last recalled (R10.2).
//!
//! **One button** (R10.6, C6, the redesign of 2026-10-10): `Commit N Files`, or Fork's "Amend
//! Last Commit" while Amend is ticked, and nothing under it; the commit chord presses it, in
//! every state. It builds no token: what an amend costs is read when it is pressed, in the job
//! that runs it, and one that must be confirmed opens the window's confirmation dialog (rule 2,
//! C1). While a commit is asked the button's place says what it is doing — "Waiting for the
//! commit to finish…", "Committing 0:03" with Cancel beside it, "Cancelling…" (R10.4, C5).
//!
//! **One line** (R10.8, C8, rule 6): at most one, over the fields — the operation in progress
//! and what committing does to it, or that `HEAD` is detached — wrapped, never cut
//! ([`operation_line`]).
//!
//! **Amend reports its state.** The toggle is a check box to assistive technology whose toggled
//! state is set ([`AMEND_CAPTION`]), which Freya's own `Checkbox` does not set
//! (`freya-ui-apis.md` §3).

use std::rc::Rc;
use std::time::{Duration, Instant};

use async_io::Timer;
use cairn_model::OperationInProgress;
use freya::prelude::*;

use crate::accelerators::{Action, FieldScope, RecallStep};
use crate::diff_palette::{DIFF_FONT_FAMILY, MONO_ADVANCE_EM};
use crate::text_field::{text_field_in, text_field_recalling};

/// The subject field's placeholder, as Fork's reads.
pub const SUBJECT_PLACEHOLDER: &str = "Commit subject";
/// The description field's placeholder.
pub const DESCRIPTION_PLACEHOLDER: &str = "Description";
/// The toggle's name, drawn beside it and read by assistive technology.
pub const AMEND_CAPTION: &str = "Amend";
/// The `≡` button's name, and the menu it opens.
pub const RECENT_MESSAGES_CAPTION: &str = "Recent Commit Messages";
/// What the running commit's cancel says.
pub const CANCEL_COMMIT_CAPTION: &str = "Cancel";
/// The button's caption while Amend is ticked, Fork's (R10.6, the user's answer of 2026-10-10,
/// 3): it names no commit; the dialog, where one is asked, names it once.
pub const AMEND_BUTTON_CAPTION: &str = "Amend Last Commit";
/// The confirmation dialog's title for an amend that must be confirmed (R10.6).
pub const AMEND_TITLE: &str = "Amend Commit";
/// What the button's place says from a cancel until git is reaped (R10.4, R4.7).
pub const CANCELLING: &str = "Cancelling…";
/// The line on a detached `HEAD` (R10.8, C8): the commit is recoverable, so it asks nothing.
pub const DETACHED_LINE: &str = "HEAD is detached: this commit will be on no branch.";
/// The Git Error dialog's skip (R10.5), Fork's words.
pub const SKIP_HOOKS_CAPTION: &str = "Skip pre-commit hooks and commit";

/// Fork's soft limit on a subject's length: the counter says how many characters are left
/// before it, and goes negative past it (R10.1).
pub const SUBJECT_SOFT_LIMIT: usize = 50;
/// Fork's hard limit: past it the counter turns red — where git tools truncate a subject, not
/// a limit on what is typed.
pub const SUBJECT_HARD_LIMIT: usize = 70;
/// The description's ruler: the column git's own guidance wraps a body at (R10.1).
pub const RULER_COLUMN: usize = 72;

/// The description's size: monospace, so the ruler stands at a column.
const DESCRIPTION_FONT_SIZE: f32 = 12.;
const DESCRIPTION_MIN_HEIGHT: f32 = 64.;
const DESCRIPTION_MAX_HEIGHT: f32 = 200.;
/// The field's inner margin: the toolkit's `input_layout` theme's, which the text starts after.
const FIELD_MARGIN: f32 = 8.;

/// The commit button's words with `files` staged: Fork's `Commit 35 Files`, `Commit 1 File`,
/// and plain `Commit` with none.
pub fn commit_caption(files: usize) -> String {
    match files {
        0 => "Commit".to_owned(),
        1 => "Commit 1 File".to_owned(),
        files => format!("Commit {files} Files"),
    }
}

/// The subject's counter (R10.1): characters left before [`SUBJECT_SOFT_LIMIT`], negative past
/// it, and whether it is past [`SUBJECT_HARD_LIMIT`], where it turns red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubjectCount {
    pub left: i64,
    pub past_hard_limit: bool,
}

/// The counter for `subject`, counting its characters as typed.
pub fn subject_count(subject: &str) -> SubjectCount {
    let typed = subject.chars().count();
    SubjectCount {
        left: i64::try_from(SUBJECT_SOFT_LIMIT).unwrap_or(i64::MAX)
            - i64::try_from(typed).unwrap_or(i64::MAX),
        past_hard_limit: typed > SUBJECT_HARD_LIMIT,
    }
}

/// The one line over the fields (R10.8, C3, C8, rule 4, rule 6): the operation in progress
/// and what committing does to it — Amend unavailable during every one, as git refuses it —
/// or, with none, that `HEAD` is detached; `None` otherwise. Short, so it is never cut.
pub fn operation_line(operation: Option<&OperationInProgress>, detached: bool) -> Option<String> {
    let refusing = |name: &str, command: &str| {
        format!("{name} is in progress. Continue or abort it with {command} --continue or --abort.")
    };
    let concluding = |doing: &str, commit: Option<&cairn_model::Oid>| match commit {
        Some(commit) => format!(
            "{doing} {}. Committing concludes it; Amend is unavailable.",
            commit.short().as_str()
        ),
        None => format!("{doing}. Committing concludes it; Amend is unavailable."),
    };
    match operation {
        Some(OperationInProgress::Merge) => {
            Some("Merging. Committing concludes the merge; Amend is unavailable.".to_owned())
        }
        Some(OperationInProgress::CherryPick { picked }) => {
            Some(concluding("Cherry-picking", picked.as_ref()))
        }
        Some(OperationInProgress::Revert { reverted }) => {
            Some(concluding("Reverting", reverted.as_ref()))
        }
        Some(
            operation @ (OperationInProgress::CherryPickSequence
            | OperationInProgress::RevertSequence
            | OperationInProgress::Rebase),
        ) => {
            let name = operation.name();
            let mut capitalised = name.chars();
            let name = capitalised
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + capitalised.as_str())
                .unwrap_or_default();
            Some(refusing(&name, operation.git_command()))
        }
        // "git am" is a command's name, written as git writes it.
        Some(operation @ OperationInProgress::ApplyingPatches) => {
            Some(refusing(operation.name(), operation.git_command()))
        }
        None => detached.then(|| DETACHED_LINE.to_owned()),
    }
}

/// What the box's one button does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitButton {
    /// Commit what is staged: `files` of it.
    Commit { files: usize },
    /// Amend `HEAD` with what is staged: Fork's "Amend Last Commit". What it costs is read when
    /// it is pressed (R6.4).
    Amend,
}

/// A commit or an amend asked and not ended: what stands in the button's place (R10.4, C5).
#[derive(Debug, Clone, PartialEq)]
pub enum Busy {
    /// Queued behind another write: "Waiting for the commit to finish…", the one form every
    /// queued write uses.
    Waiting(String),
    /// Running: "Committing" or "Amending", the time since git started beside it once it has,
    /// and Cancel.
    Running {
        verb: &'static str,
        started: Option<Instant>,
    },
    /// Cancelled, until git is reaped: [`CANCELLING`].
    Cancelling,
}

pub struct CommitBox {
    subject: State<String>,
    description: State<String>,
    amend: bool,
    amend_enabled: bool,
    button: CommitButton,
    ready: bool,
    busy: Option<Busy>,
    stopped: bool,
    line: Option<String>,
    recent: Rc<Vec<String>>,
    on_amend: EventHandler<bool>,
    on_commit: EventHandler<()>,
    on_cancel: EventHandler<()>,
    on_edit: EventHandler<()>,
    on_recall: EventHandler<usize>,
    recall: Callback<RecallStep, bool>,
    key: DiffKey,
}

impl CommitBox {
    /// The box over the window's draft, `subject` and `description`.
    pub fn new(subject: State<String>, description: State<String>) -> Self {
        Self {
            subject,
            description,
            amend: false,
            amend_enabled: true,
            button: CommitButton::Commit { files: 0 },
            ready: false,
            busy: None,
            stopped: false,
            line: None,
            recent: Rc::default(),
            on_amend: EventHandler::new(|_| {}),
            on_commit: EventHandler::new(|()| {}),
            on_cancel: EventHandler::new(|()| {}),
            on_edit: EventHandler::new(|()| {}),
            on_recall: EventHandler::new(|_| {}),
            recall: Callback::new(|_| false),
            key: DiffKey::None,
        }
    }

    /// Whether Amend is ticked, and whether it can be (no `HEAD`, an operation in progress:
    /// R10.1, the reason on the box's line).
    pub fn amend(mut self, ticked: bool, enabled: bool) -> Self {
        self.amend = ticked;
        self.amend_enabled = enabled;
        self
    }

    /// What the button does, and whether a press does it now.
    pub fn button(mut self, button: CommitButton, ready: bool) -> Self {
        self.button = button;
        self.ready = ready;
        self
    }

    /// A commit asked and not ended, drawn in the button's place.
    pub fn busy(mut self, busy: Option<Busy>) -> Self {
        self.busy = busy;
        self
    }

    /// The box is disabled: an operation in progress refuses a commit (R10.8), which the line
    /// names.
    pub fn stopped(mut self, stopped: bool) -> Self {
        self.stopped = stopped;
        self
    }

    /// The one line over the fields ([`operation_line`]), wrapped.
    pub fn line(mut self, line: Option<String>) -> Self {
        self.line = line;
        self
    }

    /// Recent Commit Messages, newest first, each whole.
    pub fn recent(mut self, recent: Rc<Vec<String>>) -> Self {
        self.recent = recent;
        self
    }

    /// Amend was ticked (`true`) or unticked.
    pub fn on_amend(mut self, on_amend: impl Into<EventHandler<bool>>) -> Self {
        self.on_amend = on_amend.into();
        self
    }

    /// The button was pressed — or the commit chord heard in a field: commit, or amend.
    pub fn on_commit(mut self, on_commit: impl Into<EventHandler<()>>) -> Self {
        self.on_commit = on_commit.into();
        self
    }

    /// Cancel was pressed: the running commit, never a queued one.
    pub fn on_cancel(mut self, on_cancel: impl Into<EventHandler<()>>) -> Self {
        self.on_cancel = on_cancel.into();
        self
    }

    /// The person changed the text of either field — typed, deleted, pasted, cleared: the draft
    /// is theirs (C7). A value the window writes is no edit.
    pub fn on_edit(mut self, on_edit: impl Into<EventHandler<()>>) -> Self {
        self.on_edit = on_edit.into();
        self
    }

    /// A recent message was chosen from the menu, by its place in [`Self::recent`].
    pub fn on_recall(mut self, on_recall: impl Into<EventHandler<usize>>) -> Self {
        self.on_recall = on_recall.into();
        self
    }

    /// A bare ↑ or ↓ in the subject, offered first: `true` when it took the step.
    pub fn recall(mut self, recall: Callback<RecallStep, bool>) -> Self {
        self.recall = recall;
        self
    }
}

// By what it draws: its handlers are the window's for the life of the view.
impl PartialEq for CommitBox {
    fn eq(&self, other: &Self) -> bool {
        self.subject == other.subject
            && self.description == other.description
            && self.amend == other.amend
            && self.amend_enabled == other.amend_enabled
            && self.button == other.button
            && self.ready == other.ready
            && self.busy == other.busy
            && self.stopped == other.stopped
            && self.line == other.line
            && Rc::ptr_eq(&self.recent, &other.recent)
            && self.key == other.key
    }
}

impl std::fmt::Debug for CommitBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommitBox")
            .field("amend", &self.amend)
            .field("button", &self.button)
            .field("ready", &self.ready)
            .field("busy", &self.busy)
            .field("stopped", &self.stopped)
            .field("line", &self.line)
            .finish_non_exhaustive()
    }
}

impl KeyExt for CommitBox {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// Reports an edit of `field` — a key that changed its text — to `on_edit`.
fn edits(field: State<String>, on_edit: EventHandler<()>) -> impl Fn(InputValidator) + 'static {
    move |validator: InputValidator| {
        if *validator.text() != *field.peek() {
            on_edit.call(());
        }
    }
}

impl Component for CommitBox {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        let open = !self.stopped;
        let subject = self.subject;
        let count = subject_count(&subject.read());
        let ready = self.ready && open && self.busy.is_none();

        // The commit chord heard in either field: exactly what the button does.
        let chord = {
            let on_commit = self.on_commit.clone();
            EventHandler::new(move |action: Action| {
                if action == Action::Commit && ready {
                    on_commit.call(());
                }
            })
        };
        let subject_field = text_field_recalling(
            self.subject,
            FieldScope::CommitBox,
            chord.clone(),
            self.recall.clone(),
        )
        .on_validate(edits(self.subject, self.on_edit.clone()))
        .placeholder(SUBJECT_PLACEHOLDER)
        .width(Size::fill())
        .enabled(open);
        let counter = label()
            .text(count.left.to_string())
            .font_size(12.)
            .color(if count.past_hard_limit {
                colours.error
            } else {
                colours.text_secondary
            })
            .a11y_alt(format!("{} characters left", count.left));
        let recent = recent_button(self.recent.clone(), open, self.on_recall.clone());
        let subject_row = rect()
            .width(Size::fill())
            .horizontal()
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(rect().width(Size::flex(1.)).child(subject_field))
            .child(counter)
            .child(recent);

        // The description's height as laid out: the ruler stands its whole height.
        let mut described = use_state(|| 0f32);
        let description = rect()
            .width(Size::fill())
            .on_sized(move |e: Event<SizedEventData>| described.set_if_modified(e.area.height()))
            .font_family(DIFF_FONT_FAMILY)
            .font_size(DESCRIPTION_FONT_SIZE)
            .child(
                text_field_in(self.description, FieldScope::CommitBox, chord)
                    .on_validate(edits(self.description, self.on_edit.clone()))
                    .multiline(true)
                    .placeholder(DESCRIPTION_PLACEHOLDER)
                    .width(Size::fill())
                    .min_height(DESCRIPTION_MIN_HEIGHT)
                    .max_height(DESCRIPTION_MAX_HEIGHT)
                    .enabled(open),
            )
            .child(
                rect()
                    .interactive(Interactive::No)
                    .position(Position::new_absolute().top(0.).left(ruler_x()))
                    .layer(Layer::Relative(1))
                    .width(Size::px(1.))
                    .height(Size::px(*described.read()))
                    .background(colours.border),
            );

        let toggle = amend_toggle(
            self.amend,
            self.amend_enabled && open && self.busy.is_none(),
            self.on_amend.clone(),
        );
        // The button, or in its place what the commit asked is doing.
        let place = match self.busy.clone() {
            Some(busy) => busy_place(busy, self.on_cancel.clone()),
            None => {
                let caption = match &self.button {
                    CommitButton::Commit { files } => commit_caption(*files),
                    CommitButton::Amend => AMEND_BUTTON_CAPTION.to_owned(),
                };
                let on_commit = self.on_commit.clone();
                control(caption.clone(), caption, ready, move || on_commit.call(()))
            }
        };
        let actions = rect()
            .width(Size::fill())
            .horizontal()
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(toggle)
            .child(rect().width(Size::flex(1.)))
            .child(place);

        rect()
            .width(Size::fill())
            .padding(Gaps::new(8., 8., 8., 8.))
            .spacing(6.)
            .background(colours.surface_secondary)
            .border(Border::new().fill(colours.border).width(BorderWidth {
                top: 1.,
                ..BorderWidth::default()
            }))
            .maybe_child(self.line.clone().map(|text| {
                label()
                    .text(text)
                    .width(Size::fill())
                    .font_size(12.)
                    .color(colours.text_secondary)
            }))
            .child(subject_row)
            .child(description)
            .child(actions)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// Where the ruler stands: after the field's margin, [`RULER_COLUMN`] monospace advances in.
fn ruler_x() -> f32 {
    FIELD_MARGIN + RULER_COLUMN as f32 * MONO_ADVANCE_EM * DESCRIPTION_FONT_SIZE
}

/// A plain button: named for assistive technology, pressed by the pointer or, focused, by
/// Return or Space; disabled, it does nothing and is not focusable.
fn control(caption: String, name: String, enabled: bool, pressed: impl Fn() + 'static) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    let control = rect()
        .a11y_role(AccessibilityRole::Button)
        .a11y_alt(name)
        .a11y_focusable(enabled)
        .padding(Gaps::new(5., 14., 5., 14.))
        .corner_radius(6.)
        .border(
            Border::new()
                .fill(if enabled {
                    colours.border
                } else {
                    colours.disabled
                })
                .width(1.)
                .alignment(BorderAlignment::Inner),
        )
        .background(colours.surface_primary)
        .child(label().text(caption).font_size(13.).color(if enabled {
            colours.text_primary
        } else {
            colours.text_secondary
        }));
    if enabled {
        control
            .cursor(CursorIcon::Pointer)
            .on_press(move |e: Event<PressEventData>| {
                e.stop_propagation();
                pressed();
            })
            .into()
    } else {
        control.into()
    }
}

/// The Amend toggle: a check box to assistive technology, its toggled state set, pressed by
/// the pointer or, focused, by Space or Return.
fn amend_toggle(ticked: bool, enabled: bool, on_amend: EventHandler<bool>) -> Element {
    crate::check_box::check_box(AMEND_CAPTION, ticked, enabled, on_amend)
}

/// The `≡` that opens Recent Commit Messages: each message's subject, newest first.
fn recent_button(recent: Rc<Vec<String>>, open: bool, on_recall: EventHandler<usize>) -> Element {
    let enabled = open && !recent.is_empty();
    control(
        "≡".to_owned(),
        RECENT_MESSAGES_CAPTION.to_owned(),
        enabled,
        move || {
            let mut menu = Menu::new();
            for (at, message) in recent.iter().enumerate() {
                let on_recall = on_recall.clone();
                let subject = message.lines().next().unwrap_or_default().to_owned();
                menu = menu.child(
                    MenuButton::new()
                        .on_press(move |_: Event<PressEventData>| {
                            ContextMenu::close();
                            on_recall.call(at);
                        })
                        .child(subject),
                );
            }
            ContextMenu::open(menu);
        },
    )
}

/// What stands in the button's place while a commit is asked (R10.4, C5): "Waiting for the
/// commit to finish…", "Committing 0:03" and its Cancel, or "Cancelling…".
fn busy_place(busy: Busy, on_cancel: EventHandler<()>) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    let said = |text: String| -> Element {
        label()
            .text(text)
            .font_size(12.)
            .color(colours.text_secondary)
            .into()
    };
    match busy {
        Busy::Waiting(text) => said(text),
        Busy::Cancelling => said(CANCELLING.to_owned()),
        Busy::Running { verb, started } => rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(match started {
                Some(started) => Elapsed { verb, started }.into_element(),
                None => said(verb.to_owned()),
            })
            .child(control(
                CANCEL_COMMIT_CAPTION.to_owned(),
                CANCEL_COMMIT_CAPTION.to_owned(),
                true,
                move || on_cancel.call(()),
            ))
            .into(),
    }
}

/// "Committing 0:03": what runs and for how long, `m:ss`, drawn again each second while it is
/// shown.
#[derive(PartialEq)]
struct Elapsed {
    verb: &'static str,
    started: Instant,
}

impl Component for Elapsed {
    fn render(&self) -> impl IntoElement {
        let mut tick = use_state(|| 0u64);
        // A task of this component's scope: it ends with the line, as the commit does.
        use_hook(move || {
            spawn(async move {
                loop {
                    Timer::after(Duration::from_secs(1)).await;
                    *tick.write() += 1;
                }
            })
        });
        let _ = tick.read();
        let colours = get_theme_or_default().read().colors().clone();
        label()
            .text(elapsed_text(self.verb, self.started.elapsed()))
            .font_size(12.)
            .color(colours.text_secondary)
    }
}

/// "Committing 0:03".
pub fn elapsed_text(verb: &str, elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    format!("{verb} {}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R10.1: Fork's counter is the characters left before 50, negative past it, red only past
    /// 70; characters are counted, not bytes. Caught by: bytes counted (a subject of accented
    /// letters running out early), or red from 51.
    #[test]
    fn the_counter_counts_characters_left_before_fifty_and_reddens_past_seventy() {
        assert_eq!(
            subject_count(""),
            SubjectCount {
                left: 50,
                past_hard_limit: false
            }
        );
        assert_eq!(subject_count(&"é".repeat(50)).left, 0);
        let over = subject_count(&"x".repeat(70));
        assert_eq!(over.left, -20);
        assert!(!over.past_hard_limit, "red at the hard limit itself");
        let past = subject_count(&"x".repeat(88));
        assert_eq!(past.left, -38);
        assert!(past.past_hard_limit);
    }

    /// R10.1: the button names how many files a commit takes, as Fork's does.
    #[test]
    fn the_button_names_the_files_staged() {
        assert_eq!(commit_caption(0), "Commit");
        assert_eq!(commit_caption(1), "Commit 1 File");
        assert_eq!(commit_caption(35), "Commit 35 Files");
    }

    /// R10.8, C3, C8 and rule 6: one line at most — the operation's, naming what committing does
    /// to it or git's command to continue or abort it, a picked commit by its short id — or, with
    /// none, a detached `HEAD`'s; none otherwise. Caught by: two lines, a sentence for one
    /// operation drawn for another, or `git am` capitalised.
    #[test]
    fn one_line_names_the_operation_or_a_detached_head() {
        let id = cairn_model::Oid::from_bytes(&[0xab; 20]).ok();
        let line = |operation: Option<OperationInProgress>, detached| {
            operation_line(operation.as_ref(), detached)
        };
        assert_eq!(
            line(Some(OperationInProgress::Merge), true).as_deref(),
            Some("Merging. Committing concludes the merge; Amend is unavailable.")
        );
        assert_eq!(
            line(Some(OperationInProgress::CherryPick { picked: id }), false).as_deref(),
            Some("Cherry-picking abababa. Committing concludes it; Amend is unavailable.")
        );
        assert_eq!(
            line(Some(OperationInProgress::Revert { reverted: id }), false).as_deref(),
            Some("Reverting abababa. Committing concludes it; Amend is unavailable.")
        );
        assert_eq!(
            line(Some(OperationInProgress::Rebase), true).as_deref(),
            Some(
                "A rebase is in progress. Continue or abort it with git rebase --continue or --abort."
            )
        );
        assert_eq!(
            line(Some(OperationInProgress::ApplyingPatches), false).as_deref(),
            Some("git am is in progress. Continue or abort it with git am --continue or --abort.")
        );
        assert_eq!(
            line(Some(OperationInProgress::RevertSequence), false).as_deref(),
            Some(
                "A revert is in progress. Continue or abort it with git revert --continue or --abort."
            )
        );
        assert_eq!(line(None, true).as_deref(), Some(DETACHED_LINE));
        assert_eq!(line(None, false), None);
    }

    /// R10.4: "Committing 0:03", minutes and two-digit seconds.
    #[test]
    fn the_elapsed_time_reads_as_the_button_says_it() {
        assert_eq!(
            elapsed_text("Committing", Duration::from_secs(3)),
            "Committing 0:03"
        );
        assert_eq!(
            elapsed_text("Amending", Duration::from_secs(75)),
            "Amending 1:15"
        );
    }

    /// The ruler stands at column 72 of the description's monospace text.
    #[test]
    fn the_ruler_stands_at_column_seventy_two() {
        let advance = MONO_ADVANCE_EM * DESCRIPTION_FONT_SIZE;
        assert!((ruler_x() - FIELD_MARGIN - 72. * advance).abs() < f32::EPSILON);
    }
}
