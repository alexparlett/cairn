//! A dialog's answer that can hold focus as the dialog opens (staging-and-commit R7.4; the
//! user's decision E, 2026-10-09): every dialog asking something risky — a destructive
//! confirmation, ssh's host-key question, a hook's skip — opens with focus on its safe answer,
//! whichever side of the row the platform puts it on, so a Return or Space pressed by habit
//! answers safely. Freya's `Popup` gives focus to nothing as it opens, and a modal frame takes
//! it only to hold it inside, so the dialog must say where it starts.

use freya::prelude::*;

/// One answer of a dialog: focusable, pressed by the pointer, Return or Space while it has
/// focus, and — `focused_first` — holding focus as the dialog opens. Not Freya's `Button`, whose
/// focus cannot be given to it as the dialog opens.
pub(crate) fn answer(
    serial: u64,
    id: AccessibilityId,
    caption: String,
    focused_first: bool,
    colours: &AnswerColours,
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
pub(crate) struct AnswerColours {
    border: Color,
    focus_ring: Color,
    background: Color,
}

impl AnswerColours {
    /// The current theme's.
    pub(crate) fn of_theme() -> Self {
        let theme = get_theme_or_default();
        let theme = theme.read();
        let sheet = theme.colors();
        Self {
            border: sheet.border,
            focus_ring: sheet.border_focus,
            background: sheet.surface_primary,
        }
    }
}

/// A button whose accessibility id the dialog owns, so it can hold focus as it opens.
struct ChoiceButton {
    /// The dialog's identity: its handler is that dialog's, so a button of another one never
    /// compares equal and keeps no stale handler.
    serial: u64,
    id: AccessibilityId,
    caption: String,
    focused_first: bool,
    colours: AnswerColours,
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
