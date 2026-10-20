//! The strip across the top of the history while Show Lost Commits is on, as Fork for Windows
//! draws its reflog mode (`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`, §2): on
//! the banner's tone between two rules, "Reflog mode enabled" and an "Exit" link that turns
//! the mode off, as the View menu's item and its chord do.

use freya::prelude::*;

use crate::style::{metrics, tones};

/// What the strip says, Fork's words.
pub const REFLOG_STRIP_TEXT: &str = "Reflog mode enabled";
/// The link that turns the mode off.
pub const REFLOG_STRIP_EXIT: &str = "Exit";

/// The strip, reporting a press of its link through [`ReflogStrip::on_exit`].
#[derive(Clone)]
pub struct ReflogStrip {
    on_exit: EventHandler<()>,
    key: DiffKey,
}

impl ReflogStrip {
    pub fn new() -> Self {
        Self {
            on_exit: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    /// "Exit" pressed: what it turns off is the caller's.
    pub fn on_exit(mut self, on_exit: impl Into<EventHandler<()>>) -> Self {
        self.on_exit = on_exit.into();
        self
    }
}

impl Default for ReflogStrip {
    fn default() -> Self {
        Self::new()
    }
}

// Hand-written: an `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for ReflogStrip {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl std::fmt::Debug for ReflogStrip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReflogStrip").finish_non_exhaustive()
    }
}

impl KeyExt for ReflogStrip {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for ReflogStrip {
    fn render(&self) -> impl IntoElement {
        let on_exit = self.on_exit.clone();
        let rule = BorderWidth {
            top: 1.,
            right: 0.,
            bottom: 1.,
            left: 0.,
        };
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(metrics::REFLOG_STRIP))
            .cross_align(Alignment::center())
            .padding(Gaps::new(
                0.,
                metrics::STRIP_INSET,
                0.,
                metrics::STRIP_INSET,
            ))
            .spacing(metrics::STRIP_GAP)
            .background(tones::BANNER)
            .border(Border::new().fill(tones::RULE).width(rule))
            .child(
                label()
                    .text(REFLOG_STRIP_TEXT)
                    .max_lines(1)
                    .font_size(metrics::BODY)
                    .color(tones::TEXT),
            )
            .child(
                rect()
                    .a11y_role(AccessibilityRole::Link)
                    .on_press(move |_| on_exit.call(()))
                    .child(
                        label()
                            .text(REFLOG_STRIP_EXIT)
                            .max_lines(1)
                            .font_size(metrics::BODY)
                            .color(tones::LINK),
                    ),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
