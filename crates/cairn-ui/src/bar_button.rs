//! One button of a bar of glyphs, as Fork draws its bars' buttons — the diff's bar and Local
//! Changes' bar over its lists alike: its glyph in Fork's blue while its toggle is on and in the
//! bar's tone otherwise, a square behind it under the pointer, its name for assistive technology
//! and its tooltip. A disabled button is greyed and reports nothing.

use freya::prelude::*;

use crate::style::{focus, metrics, tones};

/// One glyph button of a bar.
#[derive(Clone)]
pub(crate) struct BarButton {
    /// Its glyph, drawn at the bar's icon size in its tone.
    pub glyph: fn(f32, Color) -> Element,
    /// Its name, which assistive technology reads: the tooltip's words without a chord.
    pub name: &'static str,
    /// Its tooltip, whole.
    pub tooltip: String,
    /// Whether it is a toggle, and whether that is on.
    pub toggle: Option<bool>,
    pub enabled: bool,
    pub on_press: EventHandler<()>,
}

// By what it draws: the glyph is the name's, and the handler the caller's for its life.
impl PartialEq for BarButton {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.tooltip == other.tooltip
            && self.toggle == other.toggle
            && self.enabled == other.enabled
    }
}

impl Component for BarButton {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let (name, toggle, enabled) = (self.name, self.toggle, self.enabled);
        let colour = if !enabled {
            tones::DISABLED_TEXT
        } else if toggle == Some(true) {
            tones::DIFF_BAR_ON
        } else {
            tones::DIFF_BAR_ICON
        };
        let on_press = self.on_press.clone();
        TooltipContainer::new(Tooltip::new_text(self.tooltip.clone())).child(
            rect()
                .a11y_id(a11y_id)
                // A Tab stop while it can be pressed, so a button with no chord is reached from
                // the keyboard; Space or Enter presses it.
                .a11y_focusable(enabled)
                .a11y_role(AccessibilityRole::Button)
                .a11y_builder(move |node| {
                    node.set_value(name);
                    if let Some(on) = toggle {
                        node.set_toggled(Toggled::from(on));
                    }
                    if !enabled {
                        node.set_disabled();
                    }
                })
                .width(Size::px(metrics::DIFF_BAR_BUTTON))
                .height(Size::px(metrics::DIFF_BAR_BUTTON))
                .center()
                .maybe(enabled && *hovered.read(), |el| {
                    el.background(tones::DIFF_BAR_HOVER)
                })
                .border(focus::ring_when(focus()))
                .on_pointer_enter(move |_| hovered.set(true))
                .on_pointer_leave(move |_| hovered.set_if_modified(false))
                .on_press(move |_| {
                    if enabled {
                        on_press.call(());
                    }
                })
                .child((self.glyph)(metrics::DIFF_BAR_ICON, colour)),
        )
    }
}
