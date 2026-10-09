//! The confirmation a destructive operation waits on (staging-and-commit R7.4, L8).
//!
//! It draws a [`Consequence`] the engine computed — its prompt and its button's label, both
//! rendered from it by its own `prompt` and `action`, so no count or path is typed apart
//! from the value it describes — and, when its button is pressed, builds the
//! [`Confirmed`] token from that same value: this file is a confirmation surface, the one
//! place in the window a token is made (`CONFIRMATION_SURFACES`, root `CLAUDE.md`).
//!
//! It is modal to the keyboard and to assistive technology: focus is held inside it
//! (`a11y_modal`) and starts on **Cancel**, a deliberate deviation from Fork, which styles
//! Discard as the default (L8, Fork Tracker #1080) — so a Return pressed by habit cancels.
//! Escape cancels and so does a press outside it, as `Popup` does. The window's chords are
//! made inert while it is open by the window itself, whose key listener runs before any
//! dialog's (`shortcuts::act`). One acknowledgement builds one token: once answered, the
//! dialog ignores every later press until it is closed.
//!
//! [`ConfirmButton`] is the same confirming button drawn in place rather than in a dialog: the
//! commit box's amend button (staging-and-commit R10.6, L12), which confirms an amend no remote
//! has by its own press, above the line its `Consequence` renders. It is here, beside the
//! dialog, so the one file that builds a token stays the one row of `CONFIRMATION_SURFACES` —
//! a view that draws it hands the token on and never builds one.

use std::rc::Rc;

use cairn_model::{Confirmed, Consequence};
use freya::prelude::*;

/// Width of the dialog: room for a prompt naming a long path.
const DIALOG_WIDTH: f32 = 520.0;

/// The safe answer, which holds focus as the dialog opens.
pub const CANCEL_CAPTION: &str = "Cancel";

pub struct ConfirmDialog {
    /// Which confirmation this is: the dialog's identity, and its key, so a different
    /// confirmation is a new dialog — its answers, its focus on Cancel and its one token
    /// afresh — however the window got from one to the next.
    serial: u64,
    title: String,
    consequence: Rc<Consequence>,
    on_confirm: EventHandler<Confirmed>,
    on_cancel: EventHandler<()>,
    key: DiffKey,
}

impl ConfirmDialog {
    /// A dialog titled `title` (Fork's "Discard changes") asking whether to accept
    /// `consequence`; `serial` names this confirmation, and must differ for every other.
    pub fn new(serial: u64, title: impl Into<String>, consequence: Rc<Consequence>) -> Self {
        Self {
            serial,
            title: title.into(),
            consequence,
            on_confirm: EventHandler::new(|_| {}),
            on_cancel: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    /// The button was pressed: the token, built from the consequence drawn, once.
    pub fn on_confirm(mut self, on_confirm: impl Into<EventHandler<Confirmed>>) -> Self {
        self.on_confirm = on_confirm.into();
        self
    }

    /// The user declined: Cancel, Escape, or a press outside the dialog.
    pub fn on_cancel(mut self, on_cancel: impl Into<EventHandler<()>>) -> Self {
        self.on_cancel = on_cancel.into();
        self
    }
}

// By the serial alone: one confirmation is one serial, whose title, consequence and handlers
// never change, so a window's render compares two numbers rather than every file of a
// consequence; and a different confirmation is also a different key (`render_key`), so the
// toolkit mounts a new dialog rather than keeping this one's handlers under its words.
impl PartialEq for ConfirmDialog {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial && self.key == other.key
    }
}

impl std::fmt::Debug for ConfirmDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmDialog")
            .field("serial", &self.serial)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

impl KeyExt for ConfirmDialog {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for ConfirmDialog {
    fn render(&self) -> impl IntoElement {
        let cancel_id = use_a11y();
        let confirm_id = use_a11y();
        // Rendered once, as the dialog mounts: a confirmation's words never change, and a
        // consequence of many files is not rendered again on every frame.
        let (prompt, action) = use_hook(|| (self.consequence.prompt(), self.consequence.action()));
        // Set by the first answer; every later press is ignored, so one acknowledgement is one
        // token however fast the button is pressed again.
        let answered = use_state(|| false);
        let theme = get_theme_or_default();
        let colours = {
            let theme = theme.read();
            let sheet = theme.colors();
            Colours {
                border: sheet.border,
                focus_ring: sheet.border_focus,
                background: sheet.surface_primary,
            }
        };

        let cancel = {
            let on_cancel = self.on_cancel.clone();
            move || {
                let mut answered = answered;
                if !*answered.peek() {
                    answered.set(true);
                    on_cancel.call(());
                }
            }
        };
        let confirm = {
            let on_confirm = self.on_confirm.clone();
            let consequence = self.consequence.clone();
            move || {
                let mut answered = answered;
                if !*answered.peek() {
                    answered.set(true);
                    on_confirm.call(token(&consequence));
                }
            }
        };

        let buttons = PopupButtons::new()
            .child(choice(
                self.serial,
                cancel_id,
                CANCEL_CAPTION.to_owned(),
                true,
                &colours,
                {
                    let cancel = cancel.clone();
                    move || cancel()
                },
            ))
            .child(choice(
                self.serial,
                confirm_id,
                action,
                false,
                &colours,
                confirm,
            ));

        Popup::new()
            .width(Size::px(DIALOG_WIDTH))
            .on_close_request(move |()| cancel())
            .child(
                rect()
                    .width(Size::fill())
                    .a11y_modal(true)
                    .a11y_role(AccessibilityRole::AlertDialog)
                    .child(PopupTitle::new(self.title.clone()))
                    .child(
                        PopupContent::new()
                            .child(label().text(prompt).width(Size::fill()).font_size(14.)),
                    )
                    .child(buttons),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(DiffKey::U64(self.serial))
    }
}

/// The one place a token is made: from the consequence the surface drew, once per press it
/// accepts.
fn token(consequence: &Consequence) -> Confirmed {
    Confirmed::by_user(consequence.clone())
}

/// A confirming button drawn in place (module docs): labelled by its consequence's `action`
/// ("Amend 1a2b3c4"), and building the token from that consequence when pressed — once per
/// consequence, whatever presses follow, since `serial` names one consequence and a button of
/// another serial is another button. Disabled, it builds nothing.
pub struct ConfirmButton {
    serial: u64,
    consequence: Rc<Consequence>,
    enabled: bool,
    on_confirm: EventHandler<Confirmed>,
    key: DiffKey,
}

impl ConfirmButton {
    /// A button confirming `consequence`; `serial` names this consequence, and must differ
    /// for every other.
    pub fn new(serial: u64, consequence: Rc<Consequence>) -> Self {
        Self {
            serial,
            consequence,
            enabled: true,
            on_confirm: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The button was pressed: the token, built from the consequence drawn, once.
    pub fn on_confirm(mut self, on_confirm: impl Into<EventHandler<Confirmed>>) -> Self {
        self.on_confirm = on_confirm.into();
        self
    }
}

// By the serial and whether it is enabled: one serial is one consequence and its handler.
impl PartialEq for ConfirmButton {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial && self.enabled == other.enabled && self.key == other.key
    }
}

impl std::fmt::Debug for ConfirmButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmButton")
            .field("serial", &self.serial)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

impl KeyExt for ConfirmButton {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for ConfirmButton {
    fn render(&self) -> impl IntoElement {
        // Rendered once, as the button mounts: one serial's words never change.
        let action = use_hook(|| self.consequence.action());
        let answered = use_state(|| false);
        let colours = get_theme_or_default().read().colors().clone();
        let enabled = self.enabled && !*answered.read();
        let on_confirm = self.on_confirm.clone();
        let consequence = self.consequence.clone();
        let button = rect()
            .a11y_role(AccessibilityRole::Button)
            .a11y_alt(action.clone())
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
            .child(label().text(action).font_size(13.).color(if enabled {
                colours.text_primary
            } else {
                colours.text_secondary
            }));
        if !enabled {
            return button;
        }
        button
            .cursor(CursorIcon::Pointer)
            .on_press(move |e: Event<PressEventData>| {
                e.stop_propagation();
                let mut answered = answered;
                if !*answered.peek() {
                    answered.set(true);
                    on_confirm.call(token(&consequence));
                }
            })
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(DiffKey::U64(self.serial))
    }
}

/// One of the dialog's two answers: focusable, pressed by the pointer, Return or Space while
/// it has focus. Not Freya's `Button`, whose focus cannot be given to it as the dialog opens.
fn choice(
    serial: u64,
    id: AccessibilityId,
    caption: String,
    focused_first: bool,
    colours: &Colours,
    pressed: impl Fn() + 'static,
) -> Element {
    ChoiceButton {
        serial,
        id,
        caption,
        focused_first,
        colours: *colours,
        pressed: EventHandler::new(move |()| pressed()),
    }
    .into()
}

/// The theme's colours an answer is drawn in.
#[derive(Clone, Copy, PartialEq)]
struct Colours {
    border: Color,
    focus_ring: Color,
    background: Color,
}

/// A button whose accessibility id the dialog owns, so it can hold focus as it opens.
struct ChoiceButton {
    /// The dialog's confirmation: its handler is that confirmation's, so a button of another
    /// one never compares equal and keeps no stale handler.
    serial: u64,
    id: AccessibilityId,
    caption: String,
    focused_first: bool,
    colours: Colours,
    pressed: EventHandler<()>,
}

impl PartialEq for ChoiceButton {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
            && self.id == other.id
            && self.caption == other.caption
            && self.focused_first == other.focused_first
            && self.colours == other.colours
    }
}

impl Component for ChoiceButton {
    fn render(&self) -> impl IntoElement {
        let focus = use_focus(self.id);
        let ring = if focus() == Focus::Not {
            Border::new().fill(self.colours.border).width(1.)
        } else {
            Border::new().fill(self.colours.focus_ring).width(2.)
        };
        let pressed = self.pressed.clone();
        rect()
            .a11y_id(self.id)
            .a11y_focusable(true)
            .a11y_auto_focus(self.focused_first)
            .a11y_role(AccessibilityRole::Button)
            .a11y_alt(self.caption.clone())
            .padding(Gaps::new(6., 14., 6., 14.))
            .corner_radius(6.)
            .border(ring.alignment(BorderAlignment::Inner))
            .background(self.colours.background)
            .on_press(move |e: Event<PressEventData>| {
                e.stop_propagation();
                pressed.call(());
            })
            .child(label().text(self.caption.clone()).font_size(14.))
    }
}
