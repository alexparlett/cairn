//! Every dialog's frame, Fork Dark's (`docs/research/ui-review/fork-copy-and-dialogs-2026-10-20.md`
//! §6, cp60-cp76; FS2 §1e): a square #2B2B2B ground in a 1 px #3BACED border over the toolkit's
//! shade, no title bar, the title white and bold at the heading's size and the body in Fork's
//! grey at the body's — never the toolkit's 18 px title or 15 px content.

use freya::prelude::*;

use crate::style::{metrics, tones};

/// The room inside the frame's border, round the title, the content and the buttons.
const FRAME_PADDING: f32 = 8.0;

/// A dialog `width` wide holding `body` in Fork's frame: Escape or a press outside asks
/// `on_close`. `body` is the dialog itself — its role, its modal hold, its title, content and
/// buttons — and is given the frame's ground, border and padding.
pub(crate) fn framed(width: f32, on_close: EventHandler<()>, body: Rect) -> Popup {
    Popup::new()
        .width(Size::px(width))
        .on_close_request(move |()| on_close.call(()))
        .child(
            body.width(Size::fill())
                .background(tones::DIALOG)
                .border(
                    Border::new()
                        .fill(tones::DIALOG_BORDER)
                        .width(1.)
                        .alignment(BorderAlignment::Inner),
                )
                .padding(FRAME_PADDING)
                .spacing(4.)
                .color(tones::DIALOG_TEXT)
                .font_size(metrics::BODY),
        )
}

/// A dialog's title: white, bold, at the heading's size, read as its title bar.
pub(crate) fn title(text: impl Into<String>) -> Element {
    rect()
        .width(Size::fill())
        .padding(Gaps::new(4., 8., 4., 8.))
        .child(
            label()
                .a11y_role(AccessibilityRole::TitleBar)
                .width(Size::fill())
                .text(text.into())
                .font_size(metrics::HEADING)
                .font_weight(FontWeight::BOLD)
                .color(tones::DIALOG_TITLE),
        )
        .into()
}

/// A dialog's content: the body's size, inset as the title is.
pub(crate) fn content() -> Rect {
    rect()
        .width(Size::fill())
        .padding(Gaps::new(4., 8., 4., 8.))
        .spacing(4.)
        .font_size(metrics::BODY)
}

/// A dialog's row of buttons, at the right, in the order the caller gives
/// (`button_order::ordered`).
pub(crate) fn buttons(buttons: Vec<Element>) -> Rect {
    rect()
        .width(Size::fill())
        .horizontal()
        .main_align(Alignment::End)
        .padding(Gaps::new(8., 8., 4., 8.))
        .spacing(6.)
        .children(buttons)
}
