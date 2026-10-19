//! The diff view's colours and typeface, as named tokens: a component reads these, never a
//! literal at the call site.
//!
//! **Where each colour comes from.** Fork Dark's, measured on Fork for Windows (which Mac
//! matches) — GitHub's diff colours, the vendor says — on Fork's diff ground, `#282828`, which
//! is Cairn's too (`style::tones::DIFF_BODY`). The bands, the changed word, the hunk header's
//! text and the header bar are read from `style::tones`, which holds Fork Dark's table; the
//! tokens Fork Dark's table does not name — the text, the line numbers, an added word, the
//! gutter's rule and the side-by-side filler — are Fork's measured values, written here.
//!
//! **Where colour carries the meaning.** There is no marker column, as Fork's default has
//! none: in unified the blank gutter of the side a line is not on still says what the tint
//! says; side by side a changed pair is told from context by its tint alone, as in Fork.

use freya::prelude::Color;

use crate::style::{metrics, tones};

/// The diff's ground: Fork Dark's diff body, under every row.
pub const GROUND: Color = tones::DIFF_BODY;

/// A removed line's tint: Fork Dark's `#633F3E`.
pub const REMOVED_TINT: Color = tones::REMOVED_LINE;
/// An added line's tint: Fork Dark's `#3A5C3F`.
pub const ADDED_TINT: Color = tones::ADDED_LINE;
/// The stronger tint of an intra-line range on a removed line: Fork Dark's `#9F4247`.
pub const REMOVED_EMPHASIS: Color = tones::CHANGED_WORD;
/// The stronger tint of an intra-line range on an added line: Fork's `#388442` taken 20 down
/// in each channel, since at Fork's own value the diff's text reads below WCAG AA's 4.5:1
/// (`the_text_reads_on_every_tint_and_an_emphasis_is_stronger_than_its_line`).
pub const ADDED_EMPHASIS: Color = rgb((0x38 - 20, 0x84 - 20, 0x42 - 20));
/// The diff's text: Fork's default token colour, `#DDDDDD`.
pub const DIFF_TEXT: Color = rgb((0xDD, 0xDD, 0xDD));
/// Line numbers and the notices beside the diff: Fork's `#A0A0A0`.
pub const DIFF_MUTED: Color = rgb((0xA0, 0xA0, 0xA0));
/// A hunk header's text, and git's end-of-file marker drawn in its place: Fork Dark's
/// `#959595`.
pub const HUNK_HEADER: Color = tones::HUNK_HEADER_TEXT;
/// The line between the line-number gutter and the text: Fork's `#4B4B4B`.
pub const GUTTER_SEPARATOR: Color = rgb((0x4B, 0x4B, 0x4B));
/// A side-by-side column's filler, where its side has no line beside the other's: Fork's
/// grey filler rows, `#424242` (Windows).
pub const FILLER: Color = rgb((0x42, 0x42, 0x42));
/// The header bar over the diff: Fork Dark's `#333333`, the detail pane's.
pub const HEADER_BAR: Color = tones::DETAIL;
/// The change last moved to with previous or next change, and an active toggle on the diff
/// bar: Fork outlines its active chunk in the system accent; Cairn's accent is Fork's blue
/// (`style::tones::ACCENT`), which the detail pane's strip marks its chosen tab with too.
pub const CURRENT_CHANGE: Color = tones::ACCENT;

/// The diff's typeface: IBM Plex Mono, embedded by the application with its licence.
/// Diff text, ids and paths are drawn in it.
pub const DIFF_FONT_FAMILY: &str = "IBM Plex Mono";
/// The diff's text size (`style::metrics::DIFF_TEXT`, where its measurement is told).
pub const DIFF_FONT_SIZE: f32 = metrics::DIFF_TEXT;
/// IBM Plex Mono's advance, as a fraction of its size: every glyph is 600 of 1000 units
/// wide (read from the font's `hmtx` table).
pub const MONO_ADVANCE_EM: f32 = 0.6;

const fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channels(colour: Color) -> [f64; 3] {
        [colour.r(), colour.g(), colour.b()].map(f64::from)
    }

    /// WCAG 2's relative luminance.
    fn luminance(colour: Color) -> f64 {
        let [r, g, b] = channels(colour).map(|c| {
            let c = c / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    fn contrast(one: Color, other: Color) -> f64 {
        let (a, b) = (luminance(one), luminance(other));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Every token is Fork's measured value on Fork's own ground, which Cairn's diff stands
    /// on. Caught by: a token typed in wrong, or a tint moved off Fork's.
    #[test]
    fn each_tint_is_forks_measured_value_on_forks_ground() {
        assert_eq!(GROUND, Color::from_rgb(0x28, 0x28, 0x28));
        assert_eq!(REMOVED_TINT, Color::from_rgb(0x63, 0x3F, 0x3E));
        assert_eq!(ADDED_TINT, Color::from_rgb(0x3A, 0x5C, 0x3F));
        assert_eq!(REMOVED_EMPHASIS, Color::from_rgb(0x9F, 0x42, 0x47));
        assert_eq!(ADDED_EMPHASIS, Color::from_rgb(36, 112, 46));
        assert_eq!(HUNK_HEADER, Color::from_rgb(0x95, 0x95, 0x95));
        assert_eq!(GUTTER_SEPARATOR, Color::from_rgb(0x4B, 0x4B, 0x4B));
        assert_eq!(FILLER, Color::from_rgb(0x42, 0x42, 0x42));
        assert_eq!(HEADER_BAR, Color::from_rgb(0x33, 0x33, 0x33));
    }

    /// The emphasis of an intra-line range stands further from the ground than its line's
    /// tint, and the text reads on every one of them at WCAG AA's 4.5:1. Caught by: an
    /// emphasis weaker than its line, or a tint dark or light enough to bury the text.
    #[test]
    fn the_text_reads_on_every_tint_and_an_emphasis_is_stronger_than_its_line() {
        for (tint, emphasis) in [
            (REMOVED_TINT, REMOVED_EMPHASIS),
            (ADDED_TINT, ADDED_EMPHASIS),
        ] {
            assert!(contrast(emphasis, GROUND) > contrast(tint, GROUND));
            assert_ne!(tint, GROUND);
            for background in [GROUND, tint, emphasis] {
                let ratio = contrast(DIFF_TEXT, background);
                assert!(ratio >= 4.5, "{ratio:.2}:1 on {:?}", channels(background));
            }
        }
        assert!(contrast(DIFF_MUTED, GROUND) >= 4.5);
        assert!(contrast(HUNK_HEADER, GROUND) >= 4.5);
    }

    /// The accent is the launch theme's, as its doc says: the change moved to, an active
    /// toggle and the chosen tab are lit in one colour. Caught by: an accent drifting from the
    /// theme's.
    #[test]
    fn the_current_change_is_the_themes_accent() {
        assert_eq!(CURRENT_CHANGE, crate::style::tones::sheet().text_highlight);
    }
}
