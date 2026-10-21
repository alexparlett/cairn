//! The window's top row. On Linux, Fork's caption row: no frame from the desktop, and one row
//! of 26 px drawn in the chrome's tone holding the menus at its left, "Cairn" at its centre and
//! the window's three controls at its right; its empty space behaves as a frame's title bar — a
//! press and drag moves the window, a double press maximizes or restores it, a secondary press
//! opens the compositor's window menu (`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`
//! for Fork's). On macOS the desktop's own frame stays, and the row holds the same menus alone.
//!
//! The row asks; the window does. Each frame gesture is reported as a [`FrameRequest`], so the
//! row reaches no window itself and what it asks is what a test reads.

use std::rc::Rc;

use freya::prelude::*;

use crate::menu::{BarMenu, MenuBar};
use crate::style::{metrics, tones};

/// The title at the caption row's centre: the application's name alone, as Fork's reads
/// "Fork". The OS-level title names the repository.
pub const CAPTION_TITLE: &str = "Cairn";

/// Who draws the window's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Cairn: the caption row, with its title and controls (Linux).
    Drawn,
    /// The desktop: the menus alone, as a row inside its frame (macOS).
    Native,
}

impl Frame {
    /// This build's: drawn everywhere but macOS.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Native
        } else {
            Self::Drawn
        }
    }
}

/// What a gesture on the caption row asks of the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameRequest {
    /// A press on its empty space: the window follows the pointer until it is let go.
    Drag,
    /// A double press on its empty space, or the maximize or restore control.
    ToggleMaximize,
    Minimize,
    /// The close control: the window's close, as the desktop's would ask it.
    Close,
    /// A secondary press on its empty space, at that point in the window.
    WindowMenu {
        x: f64,
        y: f64,
    },
}

/// The window's top row over `menus`: a choice is reported to `on_choose`, a frame gesture to
/// `on_frame`.
pub struct CaptionRow<T> {
    menus: Rc<Vec<BarMenu<T>>>,
    frame: Frame,
    on_choose: EventHandler<T>,
    on_frame: EventHandler<FrameRequest>,
    key: DiffKey,
}

impl<T> CaptionRow<T> {
    pub fn new(menus: Vec<BarMenu<T>>) -> Self {
        Self {
            menus: Rc::new(menus),
            frame: Frame::current(),
            on_choose: EventHandler::new(|_: T| {}),
            on_frame: EventHandler::new(|_: FrameRequest| {}),
            key: DiffKey::None,
        }
    }

    /// Who draws the frame: this build's unless a test names the other.
    pub fn frame(mut self, frame: Frame) -> Self {
        self.frame = frame;
        self
    }

    pub fn on_choose(mut self, on_choose: impl Into<EventHandler<T>>) -> Self {
        self.on_choose = on_choose.into();
        self
    }

    pub fn on_frame(mut self, on_frame: impl Into<EventHandler<FrameRequest>>) -> Self {
        self.on_frame = on_frame.into();
        self
    }
}

// By what it draws: `EventHandler` never compares equal.
impl<T: PartialEq> PartialEq for CaptionRow<T> {
    fn eq(&self, other: &Self) -> bool {
        self.menus == other.menus && self.frame == other.frame && self.key == other.key
    }
}

impl<T> std::fmt::Debug for CaptionRow<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptionRow")
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}

impl<T> KeyExt for CaptionRow<T> {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl<T: Clone + PartialEq + 'static> Component for CaptionRow<T> {
    fn render(&self) -> impl IntoElement {
        let drawn: Element = self.drawn();
        drawn
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

impl<T: Clone + PartialEq + 'static> CaptionRow<T> {
    fn drawn(&self) -> Element {
        let menus = MenuBar::new((*self.menus).clone()).on_choose(self.on_choose.clone());
        let row = rect()
            .width(Size::fill())
            .height(Size::px(metrics::CAPTION_ROW))
            .background(tones::CHROME)
            .font_size(metrics::BODY);
        match self.frame {
            Frame::Native => row.child(menus).into(),
            Frame::Drawn => {
                let maximized = *Platform::get().is_maximized.read();
                let asking = self.on_frame.clone();
                let ask = move |request: FrameRequest| {
                    let asking = asking.clone();
                    move |_: Event<PressEventData>| asking.call(request)
                };
                let on_frame = self.on_frame.clone();
                row.on_pointer_down(move |e: Event<PointerEventData>| {
                    let at = e.global_location();
                    match e.button() {
                        Some(MouseButton::Right) => {
                            on_frame.call(FrameRequest::WindowMenu { x: at.x, y: at.y })
                        }
                        Some(MouseButton::Left) | None => match EventsCombos::pressed(at) {
                            PressEventType::Double => on_frame.call(FrameRequest::ToggleMaximize),
                            PressEventType::Single
                            | PressEventType::Triple
                            | PressEventType::Quadruple => on_frame.call(FrameRequest::Drag),
                        },
                        Some(_) => {}
                    }
                })
                // The title, centred on the window under the menus and the controls, so a press
                // on it is a press on the row's empty space.
                .child(
                    rect()
                        .position(Position::new_absolute().top(0.).left(0.))
                        .width(Size::fill())
                        .height(Size::fill())
                        .center()
                        .child(label().text(CAPTION_TITLE).max_lines(1).color(tones::TEXT)),
                )
                .child(
                    rect()
                        .horizontal()
                        .content(Content::Flex)
                        .width(Size::fill())
                        .height(Size::fill())
                        .child(menus)
                        .child(rect().width(Size::flex(1.)).height(Size::fill()))
                        .child(
                            TitlebarButton::new(TitlebarAction::Minimize)
                                .on_press(ask(FrameRequest::Minimize)),
                        )
                        .child(
                            TitlebarButton::new(if maximized {
                                TitlebarAction::Restore
                            } else {
                                TitlebarAction::Maximize
                            })
                            .on_press(ask(FrameRequest::ToggleMaximize)),
                        )
                        .child(
                            TitlebarButton::new(TitlebarAction::Close)
                                .on_press(ask(FrameRequest::Close)),
                        ),
                )
                .into()
            }
        }
    }
}
