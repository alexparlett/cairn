//! How the history and the detail pane share the main region's height: as a proportion, half
//! each as the window opens, both growing with the window, as Fork's do
//! (`docs/research/ui-review/fork-window-survey-2-2026-10-16.md`, section 3); a drag of the
//! splitter moves the proportion, never a height.

/// Each side's share of the height, in whatever unit it was last measured in: equal until the
/// splitter is dragged, then the pixels each side was dragged to, which say the proportion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneSplit {
    list: f32,
    pane: f32,
}

impl Default for PaneSplit {
    fn default() -> Self {
        Self { list: 1., pane: 1. }
    }
}

impl PaneSplit {
    /// The history's and the pane's flex weights, summing to a hundred.
    pub fn weights(self) -> (f32, f32) {
        let total = self.list + self.pane;
        if total <= f32::EPSILON {
            return (50., 50.);
        }
        let pane = 100. * self.pane / total;
        (100. - pane, pane)
    }

    /// The history was dragged to `height` pixels.
    pub fn list_dragged(&mut self, height: f32) {
        self.list = height.max(0.);
    }

    /// The pane was dragged to `height` pixels.
    pub fn pane_dragged(&mut self, height: f32) {
        self.pane = height.max(0.);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Half each until a drag; after one, the proportion the drag left, whatever the window's
    /// height. Caught by: a fixed height kept, or the weights not summing to the whole.
    #[test]
    fn the_split_is_a_proportion_a_drag_moves() {
        let mut split = PaneSplit::default();
        assert_eq!(split.weights(), (50., 50.));
        split.list_dragged(300.);
        split.pane_dragged(100.);
        assert_eq!(split.weights(), (75., 25.));
        split.list_dragged(0.);
        split.pane_dragged(0.);
        assert_eq!(split.weights(), (50., 50.));
    }
}
