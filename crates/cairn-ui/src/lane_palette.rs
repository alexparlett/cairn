//! Which colour a lane's lines are drawn in.
//!
//! Colour here is an aid, never the information. The product rule for this
//! packet is that lane identity must survive a monochrome screenshot and a
//! colour-vision deficiency, and it does for a reason that has nothing to do
//! with this file: a lane's identity is its COLUMN, and every lane has a column
//! of its own (`crate::graph_geometry`). Colour only makes two lines easier to
//! follow past each other when they are on screen together in colour.
//!
//! The palette is the Okabe–Ito qualitative set, which was chosen for exactly
//! this: its members stay distinguishable under protanopia, deuteranopia and
//! tritanopia. It is short on purpose — cycling eight hues past each other
//! reads better than twenty nearly-identical ones, and the column, not the hue,
//! is what says which lane a line is in.

use cairn_model::Lane;
use freya::prelude::Color;

/// The hues lanes cycle through, in order.
const LANE_COLOURS: &[(u8, u8, u8)] = &[
    (86, 180, 233),  // sky blue
    (230, 159, 0),   // orange
    (0, 158, 115),   // bluish green
    (204, 121, 167), // reddish purple
    (240, 228, 66),  // yellow
    (0, 114, 178),   // blue
    (213, 94, 0),    // vermillion
    (190, 190, 190), // neutral grey
];

/// The colour lines in `lane` are drawn in.
pub fn lane_colour(lane: Lane) -> Color {
    // Indexing is total: the modulus is the slice's own length, and the slice
    // is a non-empty constant, so there is no absent case to handle and no
    // panicking access to explain away.
    let index = lane.index() % LANE_COLOURS.len();
    match LANE_COLOURS.get(index) {
        Some(&rgb) => Color::from(rgb),
        None => Color::from((190, 190, 190)),
    }
}

/// How many hues there are before the palette repeats.
pub fn palette_len() -> usize {
    LANE_COLOURS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Neighbouring lanes never share a hue, which is the only thing colour is
    /// asked to do here: tell two lines apart where they run side by side.
    #[test]
    fn neighbouring_lanes_differ_in_colour() {
        for index in 0..palette_len() * 3 {
            let here = lane_colour(Lane::new(index));
            let next = lane_colour(Lane::new(index + 1));
            assert_ne!(here, next, "lanes {index} and {} share a colour", index + 1);
        }
    }

    /// The palette cycles rather than running out, so a history wider than the
    /// palette still draws every lane.
    #[test]
    fn the_palette_cycles_instead_of_running_out() {
        assert_eq!(
            lane_colour(Lane::new(0)),
            lane_colour(Lane::new(palette_len()))
        );
        assert_eq!(
            lane_colour(Lane::new(3)),
            lane_colour(Lane::new(palette_len() * 7 + 3))
        );
    }

    /// Every entry is distinct: a duplicate in the table would silently halve
    /// the number of lanes colour can tell apart.
    #[test]
    fn the_palette_holds_no_duplicates() {
        for (index, colour) in LANE_COLOURS.iter().enumerate() {
            assert!(
                !LANE_COLOURS[..index].contains(colour),
                "{colour:?} appears twice in the lane palette"
            );
        }
    }
}
