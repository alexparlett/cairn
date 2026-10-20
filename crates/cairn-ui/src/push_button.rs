//! The one push button every dialog, banner and notice draws: Fork Dark's — a #373737 fill in a
//! 1 px border — the default answer of a dialog bordered in Fork's blue rather than filled, as
//! Fork's default button is (`docs/research/ui-review/fork-copy-and-dialogs-2026-10-20.md` §6,
//! cp60-cp76), a greyed one in the disabled tones, and keyboard focus shown by the style's one
//! ring, drawn inset inside the border as Fork draws its focus rectangle (`style::focus`).
//!
//! Not Freya's `Button`: focus cannot be given to one as a dialog opens, and its focus ring is
//! its own, two pixels wide.

use freya::prelude::*;

use crate::style::{focus, icons, metrics, tones};

/// How much room a button gives its caption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonSize {
    /// A dialog's answer.
    Dialog,
    /// A button inside a line of text: Load Diff, Remove index.lock…, a popover's Cancel.
    Compact,
}

/// What a button shows: its caption, or a glyph read by assistive technology as `name`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ButtonFace {
    Caption(String),
    Glyph { glyph: ButtonGlyph, name: String },
}

/// The glyphs a push button can show in place of a caption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonGlyph {
    /// A comparison's swap of its two commits.
    Swap,
}

impl ButtonGlyph {
    fn draw(self, colour: Color) -> Element {
        match self {
            ButtonGlyph::Swap => icons::swap(metrics::DIFF_BAR_ICON, colour),
        }
    }
}

/// A push button: pressed by the pointer, or by Return or Space while it has focus.
#[derive(Clone)]
pub(crate) struct PushButton {
    face: ButtonFace,
    size: ButtonSize,
    /// Bordered in Fork's blue: the answer Return gives in a dialog.
    default: bool,
    enabled: bool,
    /// The accessibility id the caller owns, so it can hold focus as its dialog opens.
    id: Option<AccessibilityId>,
    focused_first: bool,
    /// Whose handler `on_press` is — a dialog's serial — so a button of another dialog never
    /// compares equal and keeps no stale handler.
    serial: u64,
    on_press: EventHandler<()>,
    key: DiffKey,
}

impl PushButton {
    pub(crate) fn new(face: ButtonFace, size: ButtonSize) -> Self {
        Self {
            face,
            size,
            default: false,
            enabled: true,
            id: None,
            focused_first: false,
            serial: 0,
            on_press: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    /// A button reading `caption`.
    pub(crate) fn caption(caption: impl Into<String>, size: ButtonSize) -> Self {
        Self::new(ButtonFace::Caption(caption.into()), size)
    }

    /// Bordered as the dialog's default answer.
    pub(crate) fn default_answer(mut self, default: bool) -> Self {
        self.default = default;
        self
    }

    /// Greyed, pressing nothing and never focused, while `enabled` is false.
    pub(crate) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// The caller's accessibility id, holding focus as the button mounts when `first`.
    pub(crate) fn focus_id(mut self, id: AccessibilityId, first: bool) -> Self {
        self.id = Some(id);
        self.focused_first = first;
        self
    }

    pub(crate) fn serial(mut self, serial: u64) -> Self {
        self.serial = serial;
        self
    }

    pub(crate) fn on_press(mut self, on_press: impl Fn() + 'static) -> Self {
        self.on_press = EventHandler::new(move |()| on_press());
        self
    }
}

impl PartialEq for PushButton {
    fn eq(&self, other: &Self) -> bool {
        self.face == other.face
            && self.size == other.size
            && self.default == other.default
            && self.enabled == other.enabled
            && self.id == other.id
            && self.focused_first == other.focused_first
            && self.serial == other.serial
            && self.key == other.key
    }
}

impl KeyExt for PushButton {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for PushButton {
    fn render(&self) -> impl IntoElement {
        let own = use_a11y();
        let id = self.id.unwrap_or(own);
        let focus = use_focus(id);
        let (fill, border, text) = match (self.enabled, self.default) {
            (false, _) => (
                tones::DISABLED_FILL,
                tones::DISABLED_BORDER,
                tones::DISABLED_TEXT,
            ),
            (true, true) => (tones::BUTTON, tones::ACCENT, tones::TEXT),
            (true, false) => (tones::BUTTON, tones::BUTTON_BORDER, tones::TEXT),
        };
        let (vertical, horizontal) = match self.size {
            ButtonSize::Dialog => (
                metrics::DIALOG_BUTTON_PADDING_Y,
                metrics::DIALOG_BUTTON_PADDING_X,
            ),
            ButtonSize::Compact => (
                metrics::COMPACT_BUTTON_PADDING_Y,
                metrics::COMPACT_BUTTON_PADDING_X,
            ),
        };
        let (face, name): (Element, String) = match &self.face {
            ButtonFace::Caption(caption) => (
                label()
                    .text(caption.clone())
                    .max_lines(1)
                    .font_size(metrics::BODY)
                    .color(text)
                    .into(),
                caption.clone(),
            ),
            ButtonFace::Glyph { glyph, name } => (glyph.draw(text), name.clone()),
        };
        let enabled = self.enabled;
        let glyph_face = matches!(self.face, ButtonFace::Glyph { .. });
        let pressed = self.on_press.clone();
        rect()
            .a11y_id(id)
            .a11y_focusable(enabled)
            .a11y_auto_focus(enabled && self.focused_first)
            .a11y_role(AccessibilityRole::Button)
            .a11y_alt(name.clone())
            .a11y_builder(move |node| {
                // A glyph's name is its value too, as the bar's glyph buttons give theirs.
                if glyph_face {
                    node.set_value(name);
                }
                if !enabled {
                    node.set_disabled();
                }
            })
            .padding(focus::RING_INSET)
            .corner_radius(metrics::CONTROL_RADIUS)
            .background(fill)
            .border(
                Border::new()
                    .fill(border)
                    .width(1.)
                    .alignment(BorderAlignment::Inner),
            )
            .maybe(enabled, |el| {
                el.cursor(CursorIcon::Pointer)
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        pressed.call(());
                    })
            })
            .child(
                rect()
                    .padding(Gaps::new(
                        vertical - focus::RING_INSET,
                        horizontal - focus::RING_INSET,
                        vertical - focus::RING_INSET,
                        horizontal - focus::RING_INSET,
                    ))
                    .corner_radius(metrics::CONTROL_RADIUS / 2.)
                    .border(focus::ring_when(focus()))
                    .child(face),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
