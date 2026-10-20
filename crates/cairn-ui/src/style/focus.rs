//! How a control shows that it holds keyboard focus — the one treatment every focusable control
//! applies: a 1 px ring in the focused field's blue, drawn while focus came by the keyboard; a
//! list shows focus by its selection's tone as well ([`super::tones::selected`]), as Fork's do,
//! so a list with nothing selected still shows it. A bordered button draws it inset, inside its own border, as
//! Fork's buttons draw their focus rectangle, so the default button's blue border and its focus
//! stay told apart (`fork-copy-and-dialogs-2026-10-20.md` §6).

use freya::prelude::*;

use super::tones;

/// The room between a bordered control's border and its inset ring.
pub const RING_INSET: f32 = 2.0;

/// Whether `focus` is shown: focus reached by the keyboard. A press focuses a control without a
/// ring, as Fork's do.
pub fn shown(focus: Focus) -> bool {
    focus == Focus::Keyboard
}

/// The ring: 1 px of the focused field's blue, inside the element it is drawn on.
pub fn ring() -> Border {
    Border::new()
        .fill(tones::ACCENT)
        .width(1.)
        .alignment(BorderAlignment::Inner)
}

/// The ring while `focus` is shown, else none: `.border(focus::ring_when(focus()))`.
pub fn ring_when(focus: Focus) -> Option<Border> {
    shown(focus).then(ring)
}

/// A control built by `build` from the accessibility id it is to take and the ring it shows
/// now — for a control drawn by a plain function, which has no hooks of its own to ask the
/// toolkit where focus is: a check box, a radio, a segment of a split button.
pub fn ringed(build: impl Fn(AccessibilityId, Option<Border>) -> Element + 'static) -> Element {
    Ringed {
        build: std::rc::Rc::new(build),
    }
    .into()
}

/// The component [`ringed`] draws.
struct Ringed {
    build: std::rc::Rc<dyn Fn(AccessibilityId, Option<Border>) -> Element>,
}

// A closure never compares equal: the control is built again whenever its parent is.
impl PartialEq for Ringed {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl Component for Ringed {
    fn render(&self) -> impl IntoElement {
        let id = use_a11y();
        let focus = use_focus(id);
        (self.build)(id, ring_when(focus()))
    }
}
