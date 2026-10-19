//! A check box with its caption beside it: the Amend toggle, Show Lost Commits in the
//! history's heading strip and Create Branch's "Check out after create". A check box to
//! assistive technology, its toggled state set — which Freya's own `Checkbox` does not set —
//! pressed by the pointer or, focused, by Space or Return.

use freya::prelude::*;

use crate::style::{metrics, tones};

/// `caption`'s check box, ticked or not; pressed while `enabled`, it reports the state it is
/// turned to through `on_toggle`.
pub fn check_box(
    caption: impl Into<String>,
    ticked: bool,
    enabled: bool,
    on_toggle: EventHandler<bool>,
) -> Element {
    let caption = caption.into();
    let mark = rect()
        .width(Size::px(14.))
        .height(Size::px(14.))
        .corner_radius(3.)
        .main_align(Alignment::Center)
        .cross_align(Alignment::Center)
        .border(
            Border::new()
                .fill(if enabled {
                    tones::CHECK_BOX_BORDER
                } else {
                    tones::DISABLED_TEXT
                })
                .width(1.)
                .alignment(BorderAlignment::Inner),
        )
        .background(if ticked {
            tones::SELECTION_FOCUSED
        } else {
            tones::FIELD
        })
        .maybe_child(ticked.then(|| {
            label()
                .text("✓")
                .font_size(metrics::CAPTION)
                .color(tones::SELECTED_TEXT)
        }));
    let toggle = rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .a11y_role(AccessibilityRole::CheckBox)
        .a11y_alt(caption.clone())
        .a11y_focusable(enabled)
        .a11y_builder(move |node| node.set_toggled(Toggled::from(ticked)))
        .child(mark)
        .child(
            label()
                .text(caption)
                .font_size(metrics::BODY)
                .max_lines(1)
                .color(if enabled {
                    tones::TEXT
                } else {
                    tones::DISABLED_TEXT
                }),
        );
    if enabled {
        toggle
            .cursor(CursorIcon::Pointer)
            .on_press(move |e: Event<PressEventData>| {
                e.stop_propagation();
                on_toggle.call(!ticked);
            })
            .into()
    } else {
        toggle.into()
    }
}
