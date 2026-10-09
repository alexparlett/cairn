//! A drag of paths from one of Local Changes' lists to the other (staging-and-commit R8.2):
//! Fork stages a file dragged from Unstaged into Staged and unstages one dragged back.
//!
//! **Tracked by the lists, never by a row.** A row is unmounted by its virtual list as it
//! scrolls out, or replaced when a status arrives, and its listeners go with it; so the press
//! that may become a drag is kept by the two lists together ([`ListDrag`], the state
//! `LocalChangesList` makes once), with the path it was pressed on — not the row, whose index
//! another status may give to another path. The lists' root hears every move and the release
//! through global listeners, so a drag survives its row unmounting. Each list is one drop zone:
//! the release over the other list is the drop, and a release anywhere else does nothing.
//!
//! **What ends a drag without a drop**: Escape; a press heard while a drag is on, which can
//! only come once the drag's own release was made where the window could not hear it (the
//! toolkit forgets the button as the pointer leaves the window); and the window losing focus.

use cairn_model::{ChangeList, RepoPath};
use freya::prelude::*;

/// How far the pointer moves, held, before a press on a row is a drag: Freya's own
/// `DragZone` threshold.
pub const DRAG_THRESHOLD: f64 = 4.0;

/// Where a press on a row stands.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum Drag {
    #[default]
    Idle,
    /// Pressed on `path` in `list` at `at`, not yet moved past [`DRAG_THRESHOLD`].
    Armed {
        list: ChangeList,
        path: RepoPath,
        at: CursorPoint,
    },
    /// Dragging from `from`, begun on `path`.
    Dragging { from: ChangeList, path: RepoPath },
}

impl Drag {
    /// The pointer moved to `to`: an armed press moved far enough is a drag.
    pub(crate) fn moved(&self, to: CursorPoint) -> Option<Drag> {
        match self {
            Self::Armed { list, path, at } if at.distance_to(to) > DRAG_THRESHOLD => {
                Some(Self::Dragging {
                    from: *list,
                    path: path.clone(),
                })
            }
            Self::Idle | Self::Armed { .. } | Self::Dragging { .. } => None,
        }
    }

    /// What a release over `list` drops, if it drops anything: a drag from the other list.
    pub(crate) fn dropped_on(&self, list: ChangeList) -> Option<(ChangeList, RepoPath)> {
        match self {
            Self::Dragging { from, path } if *from != list => Some((*from, path.clone())),
            Self::Idle | Self::Armed { .. } | Self::Dragging { .. } => None,
        }
    }

    /// Whether a drag is on, past its threshold.
    pub(crate) fn is_dragging(&self) -> bool {
        match self {
            Self::Dragging { .. } => true,
            Self::Idle | Self::Armed { .. } => false,
        }
    }

    /// Whether a drag from the other list is on, so `list` is where it would drop.
    pub(crate) fn targets(&self, list: ChangeList) -> bool {
        match self {
            Self::Dragging { from, .. } => *from != list,
            Self::Idle | Self::Armed { .. } => false,
        }
    }
}

/// The two lists' drag: one per `LocalChangesList`, shared by both lists. Copy, as Freya's
/// handles are.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ListDrag(State<Drag>);

impl ListDrag {
    /// Made once, by the component holding both lists; ended when the window loses focus.
    pub(crate) fn used() -> Self {
        let mut drag = use_state(Drag::default);
        use_side_effect(move || {
            if !*Platform::get().is_app_focused.read() && *drag.peek() != Drag::Idle {
                drag.set(Drag::Idle);
            }
        });
        Self(drag)
    }

    /// A primary press on `path` in `list`, at `at`: it may become a drag.
    pub(crate) fn arm(&mut self, list: ChangeList, path: RepoPath, at: CursorPoint) {
        self.0.set(Drag::Armed { list, path, at });
    }

    /// The drag as it stands, subscribing the caller to it.
    pub(crate) fn read(&self) -> Drag {
        self.0.read().clone()
    }

    /// The drag as it stands, without subscribing.
    pub(crate) fn peek(&self) -> Drag {
        self.0.peek().clone()
    }

    /// The lists' root, listening for the drag: every move, the release that ends it, a press
    /// while one is on (its release was never heard), and Escape.
    pub(crate) fn on(self, root: Rect) -> Rect {
        let mut drag = self.0;
        root.on_global_pointer_move(move |e: Event<PointerEventData>| {
            let moved = drag.peek().moved(e.global_location());
            if let Some(moved) = moved {
                drag.set(moved);
            }
        })
        // Global, so after the list's own release: a drop over a list is heard first.
        .on_global_pointer_press(move |_: Event<PointerEventData>| {
            if *drag.peek() != Drag::Idle {
                drag.set(Drag::Idle);
            }
        })
        // A row's own press, heard first, arms afresh; any other press while a drag is on means
        // its release was lost.
        .on_global_pointer_down(move |_: Event<PointerEventData>| {
            if drag.peek().is_dragging() {
                drag.set(Drag::Idle);
            }
        })
        .on_global_key_down(move |e: Event<KeyboardEventData>| {
            if e.key == Key::Named(NamedKey::Escape) && *drag.peek() != Drag::Idle {
                drag.set(Drag::Idle);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn armed(list: ChangeList) -> Drag {
        Drag::Armed {
            list,
            path: RepoPath::from("a.rs"),
            at: CursorPoint::new(10., 10.),
        }
    }

    /// A press is a drag only past the threshold, drops only on the other list, and nothing
    /// drops unless a drag is on. Caught by: a click read as a drag, a drop on the list it came
    /// from, or an armed press dropping.
    #[test]
    fn a_press_becomes_a_drag_past_the_threshold_and_drops_on_the_other_list_only() {
        let press = armed(ChangeList::Unstaged);
        assert_eq!(press.moved(CursorPoint::new(12., 12.)), None);
        assert_eq!(press.dropped_on(ChangeList::Staged), None);
        let Some(dragging) = press.moved(CursorPoint::new(10., 15.)) else {
            panic!("five pixels did not make a drag");
        };
        assert!(dragging.is_dragging());
        assert!(dragging.targets(ChangeList::Staged));
        assert!(!dragging.targets(ChangeList::Unstaged));
        assert_eq!(
            dragging.dropped_on(ChangeList::Staged),
            Some((ChangeList::Unstaged, RepoPath::from("a.rs")))
        );
        assert_eq!(dragging.dropped_on(ChangeList::Unstaged), None);
        assert_eq!(Drag::Idle.dropped_on(ChangeList::Staged), None);
        assert_eq!(dragging.moved(CursorPoint::new(200., 200.)), None);
    }
}
