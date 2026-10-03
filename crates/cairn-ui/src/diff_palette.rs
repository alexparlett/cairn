//! The diff view's colours and typeface (PRD R6.5, R6.6), as named tokens: a component
//! reads these, never a literal at the call site.
//!
//! **Where each colour comes from.** Fork's measured dark values
//! (`docs/research/diff-engine/fork-detail-and-diff-ui.md`, Finding 25, Windows 2023-2026,
//! which Mac 2025 matches) — GitHub's diff colours, the vendor says — drawn on Fork's ground,
//! `#282828`. Cairn's ground is darker: Freya's dark theme paints `rgb(20, 20, 20)`, which
//! is what the window and the diff view stand on. So each Fork colour is **retuned** to
//! Cairn's ground by keeping its offset from Fork's ground, channel by channel
//! ([`retuned`]): a removed line is as much redder than the ground here as it is in Fork,
//! and the text keeps Fork's own value, which is lighter against a darker ground. The
//! retuning is computed, not hand-picked, so the source value stays readable in the code.
//!
//! **Colour never carries the meaning alone** (L11): the marker column (`-`, `+`) and the
//! blank gutter of the side a line is not on say what the tint says.

use freya::prelude::Color;

/// Fork's dark diff ground, measured: `#282828`.
const FORK_GROUND: (u8, u8, u8) = (0x28, 0x28, 0x28);

/// Cairn's ground: Freya's dark theme background (`DARK_COLORS.background`), which the
/// window paints and the diff view paints again under its rows.
pub const GROUND: Color = rgb(GROUND_RGB);
const GROUND_RGB: (u8, u8, u8) = (20, 20, 20);

/// A removed line's tint: Fork's `#633F3E`, retuned.
pub const REMOVED_TINT: Color = retuned((0x63, 0x3F, 0x3E));
/// An added line's tint: Fork's `#3A5C3F`, retuned.
pub const ADDED_TINT: Color = retuned((0x3A, 0x5C, 0x3F));
/// The stronger tint of an intra-line range on a removed line: Fork's `#9F4247`, retuned.
pub const REMOVED_EMPHASIS: Color = retuned((0x9F, 0x42, 0x47));
/// The stronger tint of an intra-line range on an added line: Fork's `#388442`, retuned.
pub const ADDED_EMPHASIS: Color = retuned((0x38, 0x84, 0x42));
/// The diff's text: Fork's default token colour, `#DDDDDD`, kept — on a darker ground it
/// only gains contrast.
pub const DIFF_TEXT: Color = rgb((0xDD, 0xDD, 0xDD));
/// Line numbers, hunk headers and git's end-of-file marker: Fork's `#A0A0A0`, kept.
pub const DIFF_MUTED: Color = rgb((0xA0, 0xA0, 0xA0));
/// The line between the line-number gutter and the text: Fork's `#4B4B4B`, retuned.
pub const GUTTER_SEPARATOR: Color = retuned((0x4B, 0x4B, 0x4B));
/// The header bar over the diff: Fork's `#333333`, retuned.
pub const HEADER_BAR: Color = retuned((0x33, 0x33, 0x33));
/// The change last moved to with previous or next change: Fork outlines its active chunk
/// in the system accent (Finding 25, Mac, `#126CFB`); Cairn has no system accent to read, so
/// its accent is the dark theme's `text_highlight`, `rgb(96, 145, 224)` — Cairn's choice —
/// which the detail pane's strip already marks its tab with and the diff bar lights an
/// active toggle with. A copy, so a row reads no theme; pinned equal to the theme's.
pub const CURRENT_CHANGE: Color = rgb((96, 145, 224));

/// The diff's typeface: IBM Plex Mono, embedded by the application with its licence (L16).
/// Diff text, ids and paths are drawn in it.
pub const DIFF_FONT_FAMILY: &str = "IBM Plex Mono";
/// The diff's text size: Fork's, measured. Fork's Mac default diff font is Menlo at 11 pt
/// (Finding 17: `Menlo-Regular - 11.0` in two screenshots four years apart) with a 6.6 pt
/// advance (Finding 24); a point on macOS is a logical pixel, and IBM Plex Mono's 0.6 em
/// advance at 11 px is the same 6.6. Fork's Windows size is not established.
pub const DIFF_FONT_SIZE: f32 = 11.0;
/// IBM Plex Mono's advance, as a fraction of its size: every glyph is 600 of 1000 units
/// wide (read from the font's `hmtx` table).
pub const MONO_ADVANCE_EM: f32 = 0.6;

const fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::from_rgb(r, g, b)
}

/// `fork`, measured on Fork's ground, moved to Cairn's: the same offset from the ground in
/// each channel, clamped to the channel's range.
pub const fn retuned(fork: (u8, u8, u8)) -> Color {
    rgb((
        shift(fork.0, FORK_GROUND.0, GROUND_RGB.0),
        shift(fork.1, FORK_GROUND.1, GROUND_RGB.1),
        shift(fork.2, FORK_GROUND.2, GROUND_RGB.2),
    ))
}

const fn shift(value: u8, from: u8, to: u8) -> u8 {
    let moved = value as i16 - from as i16 + to as i16;
    if moved < 0 {
        0
    } else if moved > 255 {
        255
    } else {
        moved as u8
    }
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

    /// The retuning is the rule the module states, on the values Finding 25 measured.
    /// Caught by: a token typed in by hand that no longer follows from Fork's value.
    #[test]
    fn each_tint_is_forks_measured_value_moved_to_cairns_ground() {
        assert_eq!(REMOVED_TINT, Color::from_rgb(79, 43, 42));
        assert_eq!(ADDED_TINT, Color::from_rgb(38, 72, 43));
        assert_eq!(REMOVED_EMPHASIS, Color::from_rgb(139, 46, 51));
        assert_eq!(ADDED_EMPHASIS, Color::from_rgb(36, 112, 46));
        assert_eq!(GUTTER_SEPARATOR, Color::from_rgb(55, 55, 55));
        assert_eq!(
            retuned(FORK_GROUND),
            GROUND,
            "Fork's ground lands on Cairn's"
        );
        assert_eq!(
            shift(250, 40, 200),
            255,
            "a channel is clamped, not wrapped"
        );
        assert_eq!(shift(10, 40, 20), 0);
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
    }

    /// The accent is the dark theme's, as its doc says: the change moved to and an active
    /// toggle are lit in one colour. Caught by: a hand-typed accent drifting from the theme.
    #[test]
    fn the_current_change_is_the_themes_accent() {
        assert_eq!(CURRENT_CHANGE, freya::prelude::DARK_COLORS.text_highlight);
    }
}
