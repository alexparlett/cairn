//! Which colour a lane's lines are drawn in.
//!
//! Colour here is an aid, never the information. The product rule for this
//! packet is that lane identity must survive a monochrome screenshot and a
//! colour-vision deficiency, and it does for a reason that has nothing to do
//! with this file: a lane's identity is its COLUMN, and every lane has a column
//! of its own (`crate::graph_geometry`). Colour only makes two lines easier to
//! follow past each other when they are on screen together in colour.
//!
//! The palette is the Okabe–Ito qualitative set — minus its black, which is
//! invisible on a dark ground, plus a neutral grey to bring it back to eight.
//! Okabe–Ito was chosen for exactly this job: its members stay distinguishable
//! under protanopia, deuteranopia and tritanopia. It is short on purpose —
//! cycling eight hues past each other reads better than twenty nearly-identical
//! ones, and the column, not the hue, is what says which lane a line is in.

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

    /// The colour-vision claim in the module doc is a claim about THESE values,
    /// and nothing else in the file decides it: eight distinct, cycling,
    /// non-adjacent blues would pass every other test here while quietly
    /// undoing the only reason this particular table was chosen. So the table
    /// is pinned against Okabe–Ito's published RGB values, and changing it is a
    /// decision someone has to make rather than a retune that slips through.
    #[test]
    fn the_palette_is_the_okabe_ito_set_less_black_plus_a_neutral() {
        // Okabe & Ito, "Color Universal Design" (2008), the seven non-black
        // members, in any order.
        let okabe_ito = [
            (230, 159, 0),   // orange
            (86, 180, 233),  // sky blue
            (0, 158, 115),   // bluish green
            (240, 228, 66),  // yellow
            (0, 114, 178),   // blue
            (213, 94, 0),    // vermillion
            (204, 121, 167), // reddish purple
        ];
        for member in okabe_ito {
            assert!(
                LANE_COLOURS.contains(&member),
                "{member:?} is an Okabe–Ito colour the lane palette dropped"
            );
        }
        assert_eq!(
            LANE_COLOURS.len(),
            okabe_ito.len() + 1,
            "the palette is the Okabe–Ito set plus exactly one neutral"
        );
        assert!(
            LANE_COLOURS
                .iter()
                .any(|&(r, g, b)| r == g && g == b && r > 0),
            "the eighth entry is meant to be a neutral grey"
        );
    }
}
