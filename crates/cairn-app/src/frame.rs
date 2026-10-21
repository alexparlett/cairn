//! The window's frame as the window keeps it: what the caption row asks of the window done to
//! it, the title the desktop shows for it, its zoom, and About Cairn with the `git` it names.

use cairn_ui::{AboutDialog, FrameRequest};
use freya::prelude::*;
use freya::winit::dpi::LogicalPosition;

use crate::window::View;

/// The window's size as it opens: the 1024 × 768 Fork's layout was measured at.
pub const OPENING_SIZE: (f64, f64) = (1024., 768.);

/// The least the window shrinks to, at the zoom's own size: the narrowest and shortest at which
/// every region keeps its content whole — the toolbar's buttons, the status box at its narrowest,
/// the history's columns with room for a subject, the detail pane's bar, and the Commit tab down
/// to Expand All under the pane's even share (`docs/systems/window-frame.md`, "The regions and
/// the minimum size").
/// Fork clips below about 600 px instead; Cairn does not.
pub const MIN_SIZE: (f64, f64) = (880., 620.);

/// The steps the zoom moves through, the window's own size at `1.0`: Cairn's, a tenth at a time
/// near the window's size and a quarter further out, as far as half and twice.
pub const ZOOM_STEPS: [f64; 9] = [0.5, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

/// The zoom step the window is drawn at, kept for the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zoom(usize);

impl Default for Zoom {
    /// The window's own size.
    fn default() -> Self {
        Self(ZOOM_STEPS.iter().position(|step| *step == 1.0).unwrap_or(0))
    }
}

impl Zoom {
    /// The scale this step draws the window at.
    pub fn factor(self) -> f64 {
        ZOOM_STEPS.get(self.0).copied().unwrap_or(1.0)
    }

    /// The step larger or smaller than this one; `None` at the end of the steps.
    pub fn stepped(self, larger: bool) -> Option<Self> {
        if larger {
            (self.0 + 1 < ZOOM_STEPS.len()).then_some(Self(self.0 + 1))
        } else {
            self.0.checked_sub(1).map(Self)
        }
    }
}

/// The frame's state handles, each a session's: the zoom, whether About Cairn is open, and the
/// `git` found as the application started, as the worker described it.
#[derive(Clone, Copy, PartialEq)]
pub struct FrameView {
    pub zoom: State<Zoom>,
    pub about: State<bool>,
    pub git: State<Option<String>>,
}

impl FrameView {
    /// Each handle made by its hook, in the window's root.
    pub fn used() -> Self {
        Self {
            zoom: use_state(Zoom::default),
            about: use_state(|| false),
            git: use_state(|| None),
        }
    }

    /// Each handle made outside a component, for a test's view.
    #[cfg(test)]
    pub fn created() -> Self {
        Self {
            zoom: State::create(Zoom::default()),
            about: State::create(false),
            git: State::create(None),
        }
    }
}

/// Why the zoom cannot move `larger` (in) or not from where it is, if it cannot.
pub fn zoom_refusal(view: View, larger: bool) -> Option<String> {
    let zoom = *view.frame.zoom.read();
    zoom.stepped(larger).is_none().then(|| {
        if larger {
            "The window is as large as the zoom goes.".to_owned()
        } else {
            "The window is as small as the zoom goes.".to_owned()
        }
    })
}

/// Zooms the whole window a step in or out, through the toolkit's own scale factor; nothing at
/// the end of the steps.
pub fn zoom(view: View, larger: bool) {
    let mut zoom = view.frame.zoom;
    let Some(next) = zoom.peek().stepped(larger) else {
        return;
    };
    zoom.set(next);
    Platform::get().set_custom_scale_factor(next.factor());
}

/// The title the desktop shows for the window — in its task bar and window switcher — the
/// repository's name before Cairn's, as `<repository> — Cairn`; Cairn's alone until one is open.
pub fn title(repository: Option<&str>) -> String {
    match repository {
        Some(name) => format!("{name} — {}", cairn_ui::CAPTION_TITLE),
        None => cairn_ui::CAPTION_TITLE.to_owned(),
    }
}

/// Keeps the desktop's title for the window naming the repository open.
pub fn use_title(view: View) {
    use_side_effect(move || {
        let named = title(view.repository.read().as_deref());
        Platform::get().set_window_title(None, named);
    });
}

/// Does to the window what its caption row asked.
pub fn ask(request: FrameRequest) {
    let platform = Platform::get();
    match request {
        FrameRequest::Drag => platform.with_window(None, |window| {
            // Refused where the pointer is no longer down; nothing to say then.
            let _ = window.drag_window();
        }),
        FrameRequest::ToggleMaximize => platform.with_window(None, |window| {
            window.set_maximized(!window.is_maximized());
        }),
        FrameRequest::Minimize => platform.with_window(None, |window| window.set_minimized(true)),
        FrameRequest::Close => close(),
        FrameRequest::WindowMenu { x, y } => platform.with_window(None, move |window| {
            window.show_window_menu(LogicalPosition::new(x, y));
        }),
    }
}

/// Closes the window as the desktop's close would: through the close hook, which closes the
/// repository first (`crate::closing`).
pub fn close() {
    drop(Platform::get().post_callback(|window, renderer| {
        renderer.request_close_window(Some(window));
    }));
}

/// Opens About Cairn.
pub fn open_about(view: View) {
    let mut about = view.frame.about;
    about.set(true);
}

/// About Cairn while it is open: this build's version and licence, and the `git` found.
pub fn about(view: View) -> Option<Element> {
    if !*view.frame.about.read() {
        return None;
    }
    let mut about = view.frame.about;
    Some(
        AboutDialog::new(
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_LICENSE"),
            view.frame.git.read().clone(),
        )
        .on_close(move |()| about.set(false))
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desktop's title names the repository before Cairn, and Cairn alone with none open.
    /// Caught by: the repository left out of the title, or the caption row's "Cairn" alone
    /// shown to the window switcher.
    #[test]
    fn the_window_title_names_the_repository() {
        assert_eq!(title(Some("engine")), "engine — Cairn");
        assert_eq!(title(None), "Cairn");
    }

    /// The zoom starts at the window's own size and steps through the table both ways, stopping
    /// at each end. Caught by: a first step that is not the window's size, a step skipped, or a
    /// zoom past either end.
    #[test]
    fn the_zoom_steps_through_its_table_and_stops_at_each_end() {
        let start = Zoom::default();
        assert_eq!(start.factor(), 1.0);
        let mut at = start;
        let mut larger = Vec::new();
        while let Some(next) = at.stepped(true) {
            larger.push(next.factor());
            at = next;
        }
        assert_eq!(larger, [1.1, 1.25, 1.5, 1.75, 2.0]);
        let mut smaller = Vec::new();
        let mut at = start;
        while let Some(next) = at.stepped(false) {
            smaller.push(next.factor());
            at = next;
        }
        assert_eq!(smaller, [0.9, 0.75, 0.5]);
    }
}
