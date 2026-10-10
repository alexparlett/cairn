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

use std::rc::Rc;

use cairn_model::{Confirmed, Consequence};
use freya::prelude::*;

use crate::accelerators::Os;
use crate::answer_button::{AnswerColours, answer};
use crate::button_order::ordered;

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
    /// One line under the prompt: what the selection held that the operation leaves as it is.
    left: Option<String>,
    on_confirm: EventHandler<Confirmed>,
    on_cancel: EventHandler<()>,
    platform: Os,
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
            left: None,
            on_confirm: EventHandler::new(|_| {}),
            on_cancel: EventHandler::new(|()| {}),
            platform: Os::current(),
            key: DiffKey::None,
        }
    }

    /// One line said under the prompt — what the selection held that the operation leaves as
    /// it is (the redesign's D1: "1 submodule and 1 conflicted file are left as they are.") —
    /// never part of the prompt the token records, which names only what is destroyed.
    pub fn left(mut self, left: Option<String>) -> Self {
        self.left = left;
        self
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

    /// Which platform's button order to draw in (the user's decision E): this build's own
    /// unless a test names another.
    pub fn platform(mut self, platform: Os) -> Self {
        self.platform = platform;
        self
    }
}

// By the serial alone: one confirmation is one serial, whose title, consequence and handlers
// never change, so a window's render compares two numbers rather than every file of a
// consequence; and a different confirmation is also a different key (`render_key`), so the
// toolkit mounts a new dialog rather than keeping this one's handlers under its words.
impl PartialEq for ConfirmDialog {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial && self.platform == other.platform && self.key == other.key
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
        let colours = AnswerColours::of_theme();

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
                    on_confirm.call(Confirmed::by_user((*consequence).clone()));
                }
            }
        };

        // In the platform's order (the user's decision E); focus starts on Cancel either way.
        let buttons = PopupButtons::new().children(ordered(
            self.platform,
            vec![answer(
                self.serial,
                confirm_id,
                action,
                false,
                &colours,
                confirm,
            )],
            answer(
                self.serial,
                cancel_id,
                CANCEL_CAPTION.to_owned(),
                true,
                &colours,
                {
                    let cancel = cancel.clone();
                    move || cancel()
                },
            ),
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
                            .child(label().text(prompt).width(Size::fill()).font_size(14.))
                            .maybe_child(
                                self.left.clone().map(|left| {
                                    label().text(left).width(Size::fill()).font_size(14.)
                                }),
                            ),
                    )
                    .child(buttons),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(DiffKey::U64(self.serial))
    }
}
