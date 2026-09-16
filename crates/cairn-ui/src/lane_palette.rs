//! Which colour a lane's lines are drawn in.
//!
//! Colour is an aid, never the information: a lane's identity is its column
//! (`crate::graph_geometry`). The eight hues are the Okabe–Ito set less its
//! black, plus a neutral grey, so they stay distinguishable under protanopia,
//! deuteranopia and tritanopia.

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

pub fn lane_colour(lane: Lane) -> Color {
    // The modulus is the slice's own length, so the `None` arm is unreachable.
    let index = lane.index() % LANE_COLOURS.len();
    match LANE_COLOURS.get(index) {
        Some(&rgb) => Color::from(rgb),
        None => Color::from((190, 190, 190)),
    }
}

/// How many hues before the palette repeats.
pub fn palette_len() -> usize {
    LANE_COLOURS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one thing colour is asked to do here.
    #[test]
    fn neighbouring_lanes_differ_in_colour() {
        for index in 0..palette_len() * 3 {
            let here = lane_colour(Lane::new(index));
            let next = lane_colour(Lane::new(index + 1));
            assert_ne!(here, next, "lanes {index} and {} share a colour", index + 1);
        }
    }

    /// A history wider than the palette still draws every lane.
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

    /// Caught by: a duplicate entry, halving the lanes colour can tell apart.
    #[test]
    fn the_palette_holds_no_duplicates() {
        for (index, colour) in LANE_COLOURS.iter().enumerate() {
            assert!(
                !LANE_COLOURS[..index].contains(colour),
                "{colour:?} appears twice in the lane palette"
            );
        }
    }

    /// Caught by: eight distinct, cycling, non-adjacent blues — they pass every
    /// other test here while undoing the reason for the table. Nothing else
    /// decides the module doc's colour-vision claim.
    #[test]
    fn the_palette_is_the_okabe_ito_set_less_black_plus_a_neutral() {
        // Okabe & Ito, "Color Universal Design" (2008): the seven non-black
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
