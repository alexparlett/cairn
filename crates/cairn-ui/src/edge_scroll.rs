//! Edge auto-scroll for a drag over a virtualized list (staging-and-commit R7.6, L5).
//!
//! Freya scrolls nothing while a drag stands at a list's edge. A drag over Local Changes'
//! lists or a diff's lines must reach rows the viewport does not show, so the list scrolls
//! itself while the pointer is near or past its top or bottom edge — and keeps scrolling while
//! the pointer stands still there, which pointer moves alone cannot do. A timer paces it:
//! `async_io::Timer`, a future on the toolkit's own executor, so the UI thread never sleeps
//! (the user's decision, L5).
//!
//! The drag is tracked by the LIST, never by a row: a row is unmounted by the virtual list as
//! it scrolls out, and its listeners with it, so the list's container — which stays mounted —
//! hears every move and the release through global pointer listeners ([`EdgeScroll::on`]),
//! and the view that starts a drag says so ([`EdgeScroll::begin`]).

use std::time::Duration;

use async_io::Timer;
use freya::prelude::*;

/// How far inside an edge a drag starts the list scrolling, in pixels.
pub const EDGE_BAND: f32 = 24.0;

/// How often a drag held at an edge scrolls the list once more.
pub const EDGE_TICK: Duration = Duration::from_millis(16);

/// The most one tick scrolls, reached a band's depth past the edge: deeper is no faster.
pub const MOST_PER_TICK: f32 = 32.0;

/// How far one tick scrolls for a pointer at `y` over a list whose viewport spans `top` to
/// `bottom`: negative toward the list's start, positive toward its end, zero away from both
/// edges and for a viewport with no height. It grows with how near the edge — then how far past it — the pointer is, up to
/// [`MOST_PER_TICK`].
pub fn edge_step(y: f32, top: f32, bottom: f32) -> f32 {
    // A viewport not yet laid out (`(0, 0)` before the list's first size), collapsed or
    // inverted has no edges: nothing scrolls.
    if bottom <= top {
        return 0.0;
    }
    // A viewport too short for two bands scrolls toward whichever edge is nearer.
    let band = EDGE_BAND.min((bottom - top) / 2.0).max(0.0);
    let pace = |depth: f32| ((depth + 1.0) * MOST_PER_TICK / (2.0 * EDGE_BAND + 1.0)).max(1.0);
    if y < top + band {
        -pace(top + band - y).min(MOST_PER_TICK)
    } else if y >= bottom - band {
        pace(y - (bottom - band)).min(MOST_PER_TICK)
    } else {
        0.0
    }
}

/// One list's edge auto-scroll: which drag is on, where the pointer last was, and the ticking
/// task while one runs. Copy, as Freya's handles are.
#[derive(Clone, Copy, PartialEq)]
pub struct EdgeScroll {
    controller: ScrollController,
    /// Whether a drag the list tracks is on.
    dragging: State<bool>,
    /// The pointer's last height, in window coordinates.
    pointer: State<f32>,
    /// The list's viewport as last laid out: its top and bottom, in window coordinates.
    viewport: State<(f32, f32)>,
    /// The ticking task, while one runs.
    ticking: State<Option<TaskHandle>>,
}

impl std::fmt::Debug for EdgeScroll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgeScroll")
            .field("dragging", &*self.dragging.peek())
            .finish_non_exhaustive()
    }
}

/// The edge auto-scroll of the list `controller` scrolls.
pub fn use_edge_scroll(controller: ScrollController) -> EdgeScroll {
    let dragging = use_state(|| false);
    let pointer = use_state(|| 0.0_f32);
    let viewport = use_state(|| (0.0_f32, 0.0_f32));
    let ticking = use_state(|| None::<TaskHandle>);
    EdgeScroll {
        controller,
        dragging,
        pointer,
        viewport,
        ticking,
    }
}

impl EdgeScroll {
    /// A drag the list tracks began: from now until the pointer is released, a pointer at an
    /// edge scrolls the list.
    pub fn begin(&mut self) {
        self.dragging.set(true);
    }

    /// The drag ended: nothing scrolls any more.
    pub fn end(&mut self) {
        self.dragging.set(false);
        if let Some(task) = self.ticking.write().take() {
            task.cancel();
        }
    }

    /// Whether a drag the list tracks is on.
    pub fn is_dragging(&self) -> bool {
        *self.dragging.peek()
    }

    /// The list's container, listening for the drag: its viewport as laid out, every move of
    /// the pointer while a drag is on, and the release that ends it — all on the container,
    /// which stays mounted however the rows under the pointer come and go.
    pub fn on(self, list: Rect) -> Rect {
        let mut edge = self;
        let mut viewport = self.viewport;
        list.on_sized(move |e: Event<SizedEventData>| {
            viewport.set_if_modified((e.area.min_y(), e.area.max_y()));
        })
        .on_global_pointer_move(move |e: Event<PointerEventData>| {
            if edge.is_dragging() {
                edge.moved(e.global_location().y as f32);
            }
        })
        .on_global_pointer_press(move |_: Event<PointerEventData>| {
            if edge.is_dragging() {
                edge.end();
            }
        })
    }

    /// The pointer moved to `y` during a drag: at an edge, the list starts scrolling, and
    /// keeps on, one step each [`EDGE_TICK`], until the pointer leaves the edge or the drag
    /// ends.
    fn moved(&mut self, y: f32) {
        self.pointer.set(y);
        let (top, bottom) = *self.viewport.peek();
        if edge_step(y, top, bottom) == 0.0 || self.ticking.peek().is_some() {
            return;
        }
        let edge = *self;
        let task = spawn(async move {
            let mut edge = edge;
            loop {
                Timer::after(EDGE_TICK).await;
                if !edge.is_dragging() || !edge.step() {
                    break;
                }
            }
            edge.ticking.set(None);
        });
        self.ticking.set(Some(task));
    }

    /// One tick: scrolls the list toward the edge the pointer is at, and says whether it is
    /// still at one with room to scroll.
    fn step(&mut self) -> bool {
        let (top, bottom) = *self.viewport.peek();
        let step = edge_step(*self.pointer.peek(), top, bottom);
        if step == 0.0 {
            return false;
        }
        let (_, at) = <(i32, i32)>::from(self.controller);
        // The position is negative-going: the content moves up as the list scrolls down.
        if step < 0.0 {
            if at >= 0 {
                return false;
            }
            self.controller.scroll_to_y((at - step as i32).min(0));
        } else {
            if self.controller.is_at_end(Direction::Vertical) {
                return false;
            }
            self.controller.scroll_to_y(at - step as i32);
        }
        true
    }
}
