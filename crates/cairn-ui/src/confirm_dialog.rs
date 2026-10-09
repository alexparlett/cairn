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

use cairn_model::{Confirmed, Consequence};
use freya::prelude::*;

/// Width of the dialog: room for a prompt naming a long path.
const DIALOG_WIDTH: f32 = 520.0;

/// The safe answer, which holds focus as the dialog opens.
pub const CANCEL_CAPTION: &str = "Cancel";

pub struct ConfirmDialog {
    title: String,
    consequence: Consequence,
    on_confirm: EventHandler<Confirmed>,
    on_cancel: EventHandler<()>,
    key: DiffKey,
}

impl ConfirmDialog {
    /// A dialog titled `title` (Fork's "Discard changes") asking whether to accept
    /// `consequence`.
    pub fn new(title: impl Into<String>, consequence: Consequence) -> Self {
        Self {
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

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for ConfirmDialog {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title && self.consequence == other.consequence && self.key == other.key
    }
}

impl std::fmt::Debug for ConfirmDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmDialog")
            .field("title", &self.title)
            .field("consequence", &self.consequence)
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
                    on_confirm.call(Confirmed::by_user(consequence.clone()));
                }
            }
        };

        let buttons = PopupButtons::new()
            .child(choice(
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
                confirm_id,
                self.consequence.action(),
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
                        PopupContent::new().child(
                            label()
                                .text(self.consequence.prompt())
                                .width(Size::fill())
                                .font_size(14.),
                        ),
                    )
                    .child(buttons),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// One of the dialog's two answers: focusable, pressed by the pointer, Return or Space while
/// it has focus. Not Freya's `Button`, whose focus cannot be given to it as the dialog opens.
fn choice(
    id: AccessibilityId,
    caption: String,
    focused_first: bool,
    colours: &Colours,
    pressed: impl Fn() + 'static,
) -> Element {
    ChoiceButton {
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
    id: AccessibilityId,
    caption: String,
    focused_first: bool,
    colours: Colours,
    pressed: EventHandler<()>,
}

impl PartialEq for ChoiceButton {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
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
