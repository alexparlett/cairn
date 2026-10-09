//! The commit box under Local Changes' diff (staging-and-commit R10, L9, L12), Fork's as
//! `docs/research/staging-and-commit/fork-staging-and-commit.md` section 5 records it: a
//! one-line subject with Fork's characters-left counter and the `≡` that opens Recent Commit
//! Messages, a description with a ruler at column 72, `Amend` at the left and the commit
//! button at the right.
//!
//! **What it draws, and what it decides.** It draws what it is handed and reports what is
//! pressed; the window decides what the press does (`crates/cairn-app/src/commit_box.rs`). The
//! subject and the description are the window's own text (`State<String>`), bound two ways to
//! the fields, so the draft is the window's for its life (R10.7) — Amend ticked and unticked,
//! a refresh, a failed hook — and a value the window writes (a recalled message, `HEAD`'s,
//! `MERGE_MSG`) is what the field shows. Both fields take the one key policy, heard in the
//! commit box's scope (`FieldScope::CommitBox`): ⌘Return or Ctrl+Enter commits without a new
//! line, the window's chords still reach the window, and no list or staging chord fires
//! (R7.3 as amended); the subject offers a bare ↑ or ↓ to the window first, which recalls a
//! recent message while the subject is empty or holds the one last recalled (R10.2).
//!
//! **The button.** `Commit N Files` — plain `Commit` with none staged — or, amending,
//! `Amend <short id>` above the line its `Consequence` renders ("Replaces …", R10.6). This file
//! is the second confirmation surface (staging-and-commit R1.1, `CONFIRMATION_SURFACES`): an
//! amend no remote has is confirmed by that button — or by the commit chord from either field,
//! which does exactly what the button does — and [`AmendButton`], one component drawing the
//! button and the line it confirms, builds the token from the consequence it draws, once per
//! consequence. It refuses a consequence a remote has (`needs_force_push`): that amend's button
//! only reports the press, and the window asks the confirmation dialog first (L12). A hook that
//! fails an amend is skipped by [`AmendSkip`], which draws the line the person confirmed and
//! builds the skipped amend's token from it, in the Git Error dialog. While the consequence is
//! read the button says so and does nothing.
//!
//! **Amend reports its state.** The toggle is a check box to assistive technology whose toggled
//! state is set ([`AMEND_CAPTION`]), which Freya's own `Checkbox` does not set
//! (`freya-ui-apis.md` §3).

use std::rc::Rc;
use std::time::{Duration, Instant};

use async_io::Timer;
use cairn_model::{Confirmed, Consequence};
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
/// Said in place of the amend button while what it would replace is read.
pub const READING_AMEND: &str = "Reading what Amend would replace…";
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

/// What the box's button does.
#[derive(Debug, Clone)]
pub enum CommitButton {
    /// Commit what is staged: `files` of it.
    Commit { files: usize },
    /// Amend `HEAD`, confirmed by this button's own press: no remote has it (L12).
    AmendInPlace {
        serial: u64,
        consequence: Rc<Consequence>,
    },
    /// Amend `HEAD`, a remote has it: the press asks the confirmation dialog first (R10.6).
    AmendAsking {
        serial: u64,
        consequence: Rc<Consequence>,
    },
    /// What amending would replace is being read.
    ReadingAmend,
    /// What amending would replace could not be read: why.
    AmendUnreadable(String),
}

impl PartialEq for CommitButton {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Commit { files: one }, Self::Commit { files: two }) => one == two,
            (Self::AmendInPlace { serial: one, .. }, Self::AmendInPlace { serial: two, .. })
            | (Self::AmendAsking { serial: one, .. }, Self::AmendAsking { serial: two, .. }) => {
                one == two
            }
            (Self::ReadingAmend, Self::ReadingAmend) => true,
            (Self::AmendUnreadable(one), Self::AmendUnreadable(two)) => one == two,
            (
                Self::Commit { .. }
                | Self::AmendInPlace { .. }
                | Self::AmendAsking { .. }
                | Self::ReadingAmend
                | Self::AmendUnreadable(_),
                _,
            ) => false,
        }
    }
}

/// A commit asked and not ended: what the box says while it waits or runs (R10.4).
#[derive(Debug, Clone, PartialEq)]
pub struct Busy {
    /// "Committing", "Amending", "Waiting for staging 2 files".
    pub what: String,
    /// When git started, for the elapsed time; `None` while it waits its turn.
    pub started: Option<Instant>,
    /// Whether Cancel is offered: the commit is running, never while queued (R4.3).
    pub cancellable: bool,
}

pub struct CommitBox {
    subject: State<String>,
    description: State<String>,
    amend: bool,
    amend_enabled: bool,
    button: CommitButton,
    ready: bool,
    busy: Option<Busy>,
    stopped: Option<String>,
    note: Option<(String, bool)>,
    recent: Rc<Vec<String>>,
    on_amend: EventHandler<bool>,
    on_commit: EventHandler<()>,
    on_confirmed: EventHandler<Confirmed>,
    on_cancel: EventHandler<()>,
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
            stopped: None,
            note: None,
            recent: Rc::default(),
            on_amend: EventHandler::new(|_| {}),
            on_commit: EventHandler::new(|()| {}),
            on_confirmed: EventHandler::new(|_| {}),
            on_cancel: EventHandler::new(|()| {}),
            on_recall: EventHandler::new(|_| {}),
            recall: Callback::new(|_| false),
            key: DiffKey::None,
        }
    }

    /// Whether Amend is ticked, and whether it can be (no `HEAD`, a merge: R10.1).
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

    /// A commit asked and not ended.
    pub fn busy(mut self, busy: Option<Busy>) -> Self {
        self.busy = busy;
        self
    }

    /// Why the box is disabled: the operation in progress, named (R10.8).
    pub fn stopped(mut self, why: Option<String>) -> Self {
        self.stopped = why;
        self
    }

    /// One line under the box, and whether it is a failure.
    pub fn note(mut self, note: Option<(String, bool)>) -> Self {
        self.note = note;
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

    /// The button was pressed — or the commit chord heard in a field — to commit, or to amend
    /// through the dialog. An amend confirmed in place is [`Self::on_confirmed`].
    pub fn on_commit(mut self, on_commit: impl Into<EventHandler<()>>) -> Self {
        self.on_commit = on_commit.into();
        self
    }

    /// The amend button confirmed in place: the token, built by the confirmation surface from
    /// the consequence it drew.
    pub fn on_confirmed(mut self, on_confirmed: impl Into<EventHandler<Confirmed>>) -> Self {
        self.on_confirmed = on_confirmed.into();
        self
    }

    /// Cancel was pressed: the running commit, never a queued one.
    pub fn on_cancel(mut self, on_cancel: impl Into<EventHandler<()>>) -> Self {
        self.on_cancel = on_cancel.into();
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
            && self.note == other.note
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
            .finish_non_exhaustive()
    }
}

impl KeyExt for CommitBox {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for CommitBox {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        let open = self.stopped.is_none();
        let subject = self.subject;
        let count = subject_count(&subject.read());
        // The consequence last confirmed in place, by its serial: one consequence, one token,
        // whether the button or the chord confirmed it.
        let confirmed = use_state(|| None::<u64>);

        // The commit chord heard in either field: exactly what the button does.
        let chord = {
            let on_commit = self.on_commit.clone();
            let on_confirmed = self.on_confirmed.clone();
            let ready = self.ready && open;
            let in_place = match &self.button {
                CommitButton::AmendInPlace {
                    serial,
                    consequence,
                } => Some((*serial, consequence.clone())),
                CommitButton::Commit { .. }
                | CommitButton::AmendAsking { .. }
                | CommitButton::ReadingAmend
                | CommitButton::AmendUnreadable(_) => None,
            };
            EventHandler::new(move |action: Action| {
                if action != Action::Commit || !ready {
                    return;
                }
                match &in_place {
                    Some((serial, consequence)) => {
                        confirm_in_place(*serial, consequence, confirmed, &on_confirmed);
                    }
                    None => on_commit.call(()),
                }
            })
        };
        let subject_field = text_field_recalling(
            self.subject,
            FieldScope::CommitBox,
            chord.clone(),
            self.recall.clone(),
        )
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
        let busy = self
            .busy
            .clone()
            .map(|busy| busy_line(busy, self.on_cancel.clone()));
        let ready = self.ready && open;
        let actions = rect()
            .width(Size::fill())
            .horizontal()
            .content(Content::Flex)
            .cross_align(Alignment::Start)
            .spacing(8.)
            .child(toggle)
            .maybe_child(busy);
        // The amend confirmed in place: its button and the line it confirms, one component,
        // taking the room left of the toggle.
        let actions = match &self.button {
            CommitButton::AmendInPlace {
                serial,
                consequence,
            } => actions.child(
                AmendButton::new(*serial, consequence.clone(), confirmed)
                    .ready(ready)
                    .on_confirmed(self.on_confirmed.clone()),
            ),
            CommitButton::Commit { .. }
            | CommitButton::AmendAsking { .. }
            | CommitButton::ReadingAmend
            | CommitButton::AmendUnreadable(_) => actions
                .child(rect().width(Size::flex(1.)))
                .child(commit_button(&self.button, ready, self.on_commit.clone())),
        };
        // An amend the dialog confirms: the line it replaces, said under the box; the dialog
        // draws it again with the force push and builds the token.
        let replaces = match &self.button {
            CommitButton::AmendAsking { consequence, .. } => consequence.replaces(),
            CommitButton::AmendInPlace { .. }
            | CommitButton::Commit { .. }
            | CommitButton::ReadingAmend
            | CommitButton::AmendUnreadable(_) => None,
        };

        rect()
            .width(Size::fill())
            .padding(Gaps::new(8., 8., 8., 8.))
            .spacing(6.)
            .background(colours.surface_secondary)
            .border(Border::new().fill(colours.border).width(BorderWidth {
                top: 1.,
                ..BorderWidth::default()
            }))
            .maybe_child(
                self.stopped
                    .clone()
                    .map(|why| line(why, colours.text_secondary)),
            )
            .child(subject_row)
            .child(description)
            .child(actions)
            .maybe_child(replaces.map(|text| line(text, colours.text_secondary)))
            .maybe_child(self.note.clone().map(|(text, failed)| {
                line(
                    text,
                    if failed {
                        colours.error
                    } else {
                        colours.text_secondary
                    },
                )
            }))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// Where the ruler stands: after the field's margin, [`RULER_COLUMN`] monospace advances in.
fn ruler_x() -> f32 {
    FIELD_MARGIN + RULER_COLUMN as f32 * MONO_ADVANCE_EM * DESCRIPTION_FONT_SIZE
}

/// One line of text under or over the box.
fn line(text: String, colour: Color) -> Element {
    label()
        .text(text)
        .width(Size::fill())
        .max_lines(3)
        .text_overflow(TextOverflow::Ellipsis)
        .font_size(12.)
        .color(colour)
        .into()
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

/// The commit button but an amend confirmed in place: commit, amend through the dialog, or what
/// stands in its place while an amend cannot be pressed.
fn commit_button(button: &CommitButton, ready: bool, on_commit: EventHandler<()>) -> Element {
    match button {
        CommitButton::Commit { files } => {
            let caption = commit_caption(*files);
            control(caption.clone(), caption, ready, move || on_commit.call(()))
        }
        // Drawn by `AmendButton`; never reached.
        CommitButton::AmendInPlace { .. } => rect().into(),
        CommitButton::AmendAsking {
            serial: _,
            consequence,
        } => {
            let caption = format!("{}…", consequence.action());
            control(caption.clone(), caption, ready, move || on_commit.call(()))
        }
        CommitButton::ReadingAmend => control(
            AMEND_CAPTION.to_owned(),
            READING_AMEND.to_owned(),
            false,
            || {},
        ),
        CommitButton::AmendUnreadable(why) => {
            control(AMEND_CAPTION.to_owned(), why.clone(), false, || {})
        }
    }
}

/// The one place this surface makes a token: from the consequence it drew.
fn token(consequence: &Consequence) -> Confirmed {
    Confirmed::by_user(consequence.clone())
}

/// Confirms `consequence` in place — the amend button pressed, or the commit chord heard — once
/// per consequence: `serial` names it, and `confirmed` keeps the serial last confirmed. A
/// consequence a remote has is refused: the dialog confirms it (L12). `true` when a token was
/// built and handed on.
fn confirm_in_place(
    serial: u64,
    consequence: &Consequence,
    mut confirmed: State<Option<u64>>,
    on_confirmed: &EventHandler<Confirmed>,
) -> bool {
    if consequence.needs_force_push() || *confirmed.peek() == Some(serial) {
        return false;
    }
    confirmed.set(Some(serial));
    on_confirmed.call(token(consequence));
    true
}

/// The amend button confirmed in place (R10.6): `Amend <short id>` above the line its
/// `Consequence` renders — "Replaces <short id> '<subject>'.", then whether the old commit can
/// be found again — and, pressed, the token built from that same consequence, once. It refuses
/// a consequence a remote has, drawing it disabled and building nothing.
pub struct AmendButton {
    serial: u64,
    consequence: Rc<Consequence>,
    confirmed: State<Option<u64>>,
    ready: bool,
    on_confirmed: EventHandler<Confirmed>,
    key: DiffKey,
}

impl AmendButton {
    /// The button for `consequence`, numbered `serial` — another consequence is another serial
    /// — `confirmed` keeping the serial last confirmed, shared with the commit chord.
    pub fn new(serial: u64, consequence: Rc<Consequence>, confirmed: State<Option<u64>>) -> Self {
        Self {
            serial,
            consequence,
            confirmed,
            ready: true,
            on_confirmed: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// Whether a press confirms now.
    pub fn ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }

    /// The token, built from the consequence drawn.
    pub fn on_confirmed(mut self, on_confirmed: impl Into<EventHandler<Confirmed>>) -> Self {
        self.on_confirmed = on_confirmed.into();
        self
    }
}

impl PartialEq for AmendButton {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
            && self.ready == other.ready
            && self.confirmed == other.confirmed
            && self.key == other.key
    }
}

impl std::fmt::Debug for AmendButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmendButton")
            .field("serial", &self.serial)
            .field("ready", &self.ready)
            .finish_non_exhaustive()
    }
}

impl KeyExt for AmendButton {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for AmendButton {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        // Rendered once, as the button mounts: one serial's words never change.
        let (action, line) = use_hook(|| {
            (
                self.consequence.action(),
                self.consequence.replaces().unwrap_or_default(),
            )
        });
        let enabled = self.ready
            && !self.consequence.needs_force_push()
            && *self.confirmed.read() != Some(self.serial);
        let (serial, consequence, confirmed, on_confirmed) = (
            self.serial,
            self.consequence.clone(),
            self.confirmed,
            self.on_confirmed.clone(),
        );
        rect()
            .width(Size::flex(1.))
            .cross_align(Alignment::End)
            .spacing(4.)
            .child(control(action.clone(), action, enabled, move || {
                confirm_in_place(serial, &consequence, confirmed, &on_confirmed);
            }))
            .child(
                label()
                    .text(line)
                    .width(Size::fill())
                    .text_align(TextAlign::End)
                    .max_lines(3)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(12.)
                    .color(colours.text_secondary),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(DiffKey::U64(self.serial))
    }
}

/// The skip of a hook that failed an amend (R10.5, the user's decision of 2026-10-09), drawn in
/// the Git Error dialog: the prompt the person confirmed the amend with — the line, and a
/// remote's force push before it where there was one — and `Skip pre-commit hooks and commit`,
/// whose press builds the skipped amend's token from that same consequence, once. The engine
/// re-checks it before git runs, so an amend whose `HEAD` moved or was pushed since is refused.
pub struct AmendSkip {
    serial: u64,
    consequence: Rc<Consequence>,
    on_confirmed: EventHandler<Confirmed>,
    key: DiffKey,
}

impl AmendSkip {
    /// The skip of the failure numbered `serial`, re-confirming `consequence`.
    pub fn new(serial: u64, consequence: Rc<Consequence>) -> Self {
        Self {
            serial,
            consequence,
            on_confirmed: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// The token, built from the consequence drawn.
    pub fn on_confirmed(mut self, on_confirmed: impl Into<EventHandler<Confirmed>>) -> Self {
        self.on_confirmed = on_confirmed.into();
        self
    }
}

impl PartialEq for AmendSkip {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial && self.key == other.key
    }
}

impl std::fmt::Debug for AmendSkip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmendSkip")
            .field("serial", &self.serial)
            .finish_non_exhaustive()
    }
}

impl KeyExt for AmendSkip {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for AmendSkip {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        let prompt = use_hook(|| self.consequence.prompt());
        let answered = use_state(|| false);
        let (consequence, on_confirmed) = (self.consequence.clone(), self.on_confirmed.clone());
        rect()
            .width(Size::fill())
            .spacing(6.)
            .child(
                label()
                    .text(prompt)
                    .width(Size::fill())
                    .font_size(12.)
                    .color(colours.text_secondary),
            )
            .child(control(
                SKIP_HOOKS_CAPTION.to_owned(),
                SKIP_HOOKS_CAPTION.to_owned(),
                !*answered.read(),
                move || {
                    let mut answered = answered;
                    if !*answered.peek() {
                        answered.set(true);
                        on_confirmed.call(token(&consequence));
                    }
                },
            ))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(DiffKey::U64(self.serial))
    }
}

/// The Amend toggle: a check box to assistive technology, its toggled state set, pressed by
/// the pointer or, focused, by Space or Return.
fn amend_toggle(ticked: bool, enabled: bool, on_amend: EventHandler<bool>) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    let mark = rect()
        .width(Size::px(14.))
        .height(Size::px(14.))
        .corner_radius(3.)
        .main_align(Alignment::Center)
        .cross_align(Alignment::Center)
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
        .background(if ticked {
            colours.primary
        } else {
            colours.surface_primary
        })
        .maybe_child(ticked.then(|| label().text("✓").font_size(11.).color(colours.text_inverse)));
    let toggle = rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .a11y_role(AccessibilityRole::CheckBox)
        .a11y_alt(AMEND_CAPTION)
        .a11y_focusable(enabled)
        .a11y_builder(move |node| node.set_toggled(Toggled::from(ticked)))
        .child(mark)
        .child(
            label()
                .text(AMEND_CAPTION)
                .font_size(13.)
                .color(if enabled {
                    colours.text_primary
                } else {
                    colours.text_secondary
                }),
        );
    if enabled {
        toggle
            .cursor(CursorIcon::Pointer)
            .on_press(move |e: Event<PressEventData>| {
                e.stop_propagation();
                on_amend.call(!ticked);
            })
            .into()
    } else {
        toggle.into()
    }
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

/// What a commit asked is doing — waiting its turn, or running for so long — and, while it runs,
/// its Cancel.
fn busy_line(busy: Busy, on_cancel: EventHandler<()>) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(
            label()
                .text(format!("{}…", busy.what))
                .font_size(12.)
                .color(colours.text_secondary),
        )
        .maybe_child(
            busy.started
                .map(|started| Elapsed { started }.into_element()),
        )
        .maybe_child(busy.cancellable.then(|| {
            control(
                CANCEL_COMMIT_CAPTION.to_owned(),
                CANCEL_COMMIT_CAPTION.to_owned(),
                true,
                move || on_cancel.call(()),
            )
        }))
        .into()
}

/// How long a commit has run, `m:ss`, drawn again each second while it is shown.
#[derive(PartialEq)]
struct Elapsed {
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
        let seconds = self.started.elapsed().as_secs();
        let colours = get_theme_or_default().read().colors().clone();
        label()
            .text(format!("{}:{:02}", seconds / 60, seconds % 60))
            .font_size(12.)
            .color(colours.text_secondary)
    }
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

    /// The ruler stands at column 72 of the description's monospace text.
    #[test]
    fn the_ruler_stands_at_column_seventy_two() {
        let advance = MONO_ADVANCE_EM * DESCRIPTION_FONT_SIZE;
        assert!((ruler_x() - FIELD_MARGIN - 72. * advance).abs() < f32::EPSILON);
    }
}
