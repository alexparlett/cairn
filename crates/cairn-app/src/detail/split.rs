//! How the history and the detail pane share the main region's height: as a proportion, half
//! each as the window opens, both growing with the window, as Fork's do
//! (`docs/research/ui-review/fork-window-survey-2-2026-10-16.md`, section 3); a drag of the
//! splitter moves the proportion, never a height.

/// The pane's share of the room the two panels split, and that room as last measured: a drag
/// reports one panel's height at a time — only the one whose size changed, when the other is
/// held at its floor — so each is read against the room rather than against the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneSplit {
    /// The pane's share of the room, between nought and one.
    pane: f32,
    /// The height the two panels split, their handle left out; nought until measured.
    room: f32,
}

impl Default for PaneSplit {
    fn default() -> Self {
        Self {
            pane: 0.5,
            room: 0.,
        }
    }
}

impl PaneSplit {
    /// The history's and the pane's flex weights, summing to a hundred.
    pub fn weights(self) -> (f32, f32) {
        let pane = 100. * self.pane;
        (100. - pane, pane)
    }

    /// The room as last measured.
    pub fn room(self) -> f32 {
        self.room
    }

    /// The room the two panels split was laid out `height` tall, their handle left out.
    pub fn measured(&mut self, height: f32) {
        self.room = height.max(0.);
    }

    /// The history was dragged to `height` pixels: the pane has what is left of the room.
    pub fn list_dragged(&mut self, height: f32) {
        if self.room > f32::EPSILON {
            self.pane = (1. - height / self.room).clamp(0., 1.);
        }
    }

    /// The pane was dragged to `height` pixels.
    pub fn pane_dragged(&mut self, height: f32) {
        if self.room > f32::EPSILON {
            self.pane = (height / self.room).clamp(0., 1.);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Half each until a drag; after one, the proportion the drag left, whichever panel reported
    /// it — the history alone, as when the pane sits on its floor, or the pane alone — read
    /// against the room measured, and nothing moved by a report before the room is known. Caught
    /// by: a fixed height kept, one panel's pixels weighed against the other's default, or the
    /// weights not summing to the whole.
    #[test]
    fn the_split_is_a_proportion_a_drag_moves() {
        let mut split = PaneSplit::default();
        assert_eq!(split.weights(), (50., 50.));
        split.list_dragged(300.);
        assert_eq!(
            split.weights(),
            (50., 50.),
            "a drag moved an unmeasured split"
        );
        split.measured(400.);
        split.list_dragged(300.);
        assert_eq!(split.weights(), (75., 25.));
        split.pane_dragged(200.);
        assert_eq!(split.weights(), (50., 50.));
        split.pane_dragged(1000.);
        assert_eq!(split.weights(), (0., 100.));
    }
}
