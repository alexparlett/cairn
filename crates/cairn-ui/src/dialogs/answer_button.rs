//! A dialog's answer that can hold focus as the dialog opens: every dialog asking something
//! risky — a destructive confirmation, ssh's host-key question, a hook's skip — opens with
//! focus on its safe answer, whichever side of the row the platform puts it on, so a Return or
//! Space pressed by habit answers safely. Freya's `Popup` gives focus to nothing as it opens,
//! and a modal frame takes it only to hold it inside, so the dialog must say where it starts.
//! Each answer is the one push button (`crate::push_button`), the dialog's primary bordered as
//! Fork's default button.

use freya::prelude::*;

use crate::push_button::{ButtonSize, PushButton};

/// Which of a dialog's answers this is: its primary — the operation, bordered as Fork's default
/// button — or another (Cancel, Close, a second action).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    Primary,
    Other,
}

/// One answer of a dialog: focusable, pressed by the pointer, Return or Space while it has
/// focus, and — `focused_first` — holding focus as the dialog opens.
pub(crate) fn answer(
    serial: u64,
    id: AccessibilityId,
    caption: String,
    focused_first: bool,
    kind: Answer,
    pressed: impl Fn() + 'static,
) -> Element {
    PushButton::caption(caption, ButtonSize::Dialog)
        .default_answer(kind == Answer::Primary)
        .focus_id(id, focused_first)
        .serial(serial)
        .on_press(pressed)
        .into()
}

/// An answer drawn greyed and not yet pressable — the confirmation's button while the engine
/// counts what it would destroy, Create Branch's until it has a name: never focusable, never
/// pressed, read by assistive technology as disabled.
pub(crate) fn greyed_answer(caption: String) -> Element {
    PushButton::caption(caption, ButtonSize::Dialog)
        .enabled(false)
        .into()
}
