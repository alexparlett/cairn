//! The glyphs on the diff bar's buttons, drawn as Fork draws its icons (Finding 10 of
//! `docs/research/diff-engine/fork-detail-and-diff-ui.md`): chevrons for previous and next
//! change, `⎵` for ignore whitespace, `−`, `+` and `↕` over lines for fewer lines, more
//! lines and the entire file, and a split rectangle for side-by-side.
//!
//! Each is built from plain shapes — rectangles, borders, one rotation — and never from an
//! icon font: IBM Plex Mono has no `⎵` and no chevron, and a font the system may lack would
//! draw a box. The one character drawn is `↕`, which Plex Mono carries (the entire file's and
//! the comparison's swap). Every glyph is drawn
//! in one colour, which is how the bar says a toggle is on (Fork's accent) and a button is
//! disabled. A glyph says nothing to assistive technology: the button carries its name
//! (`DiffHeader`), so its meaning never rests on the shape.

use freya::prelude::*;

use crate::diff_palette::DIFF_FONT_FAMILY;

/// The box every glyph is drawn in.
pub(crate) const GLYPH_WIDTH: f32 = 16.0;
pub(crate) const GLYPH_HEIGHT: f32 = 18.0;

/// A stroke's thickness.
const STROKE: f32 = 1.5;

/// Which glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Glyph {
    PreviousChange,
    NextChange,
    IgnoreWhitespace,
    FewerLines,
    MoreLines,
    EntireFile,
    SideBySide,
    /// A file in the Commit tab's list, closed: its diff opens under it when pressed (Fork's
    /// Windows disclosure, Finding 4).
    Closed,
    /// A file opened in place.
    Open,
    /// The comparison's base and tip swapped (Fork's swap-direction control, Finding 7).
    Swap,
    /// Fork's Stage All in Local Changes' Unstaged heading: a double chevron pointing down,
    /// into Staged (staging-and-commit R8.2).
    StageAll,
}

impl Glyph {
    /// The glyph in `colour`, centred in its box.
    pub(crate) fn draw(self, colour: Color) -> Rect {
        let shape: Element = match self {
            Self::PreviousChange => chevron(colour, Pointing::Up).into(),
            Self::NextChange => chevron(colour, Pointing::Down).into(),
            Self::Closed => chevron(colour, Pointing::Right).into(),
            Self::Swap => label()
                .text("↕")
                .font_family(DIFF_FONT_FAMILY)
                .font_size(14.)
                .line_height(1.)
                .color(colour)
                .into(),
            Self::Open => chevron(colour, Pointing::Down).into(),
            Self::StageAll => doubled(colour, Pointing::Down).into(),
            Self::IgnoreWhitespace => rect()
                .width(Size::px(12.))
                .height(Size::px(5.))
                .offset_y(3.)
                .border(stroke_border(colour, (0., STROKE, STROKE, STROKE)))
                .into(),
            Self::FewerLines => over_lines(bar(colour, 8., STROKE).into(), colour).into(),
            Self::MoreLines => over_lines(plus(colour).into(), colour).into(),
            Self::EntireFile => over_lines(
                label()
                    .text("↕")
                    .font_family(DIFF_FONT_FAMILY)
                    .font_size(11.)
                    .line_height(1.)
                    .color(colour)
                    .into(),
                colour,
            )
            .into(),
            Self::SideBySide => rect()
                .horizontal()
                .width(Size::px(14.))
                .height(Size::px(11.))
                .main_align(Alignment::Center)
                .border(stroke_border(colour, (STROKE, STROKE, STROKE, STROKE)))
                .child(bar(colour, STROKE, 11.))
                .into(),
        };
        rect()
            .width(Size::px(GLYPH_WIDTH))
            .height(Size::px(GLYPH_HEIGHT))
            .center()
            .child(shape)
    }
}

/// A border of `colour` with the given (top, right, bottom, left) widths, drawn inside.
fn stroke_border(colour: Color, (top, right, bottom, left): (f32, f32, f32, f32)) -> Border {
    Border::new()
        .fill(colour)
        .width(BorderWidth {
            top,
            right,
            bottom,
            left,
        })
        .alignment(BorderAlignment::Inner)
}

/// A filled bar.
fn bar(colour: Color, width: f32, height: f32) -> Rect {
    rect()
        .width(Size::px(width))
        .height(Size::px(height))
        .background(colour)
}

/// `+`: a stem above and below a bar.
fn plus(colour: Color) -> Rect {
    let stem = || bar(colour, STROKE, 3.25);
    rect()
        .cross_align(Alignment::Center)
        .child(stem())
        .child(bar(colour, 8., STROKE))
        .child(stem())
}

/// `sign` above two lines of text: Fork's fewer lines, more lines and entire file.
fn over_lines(sign: Element, colour: Color) -> Rect {
    rect()
        .cross_align(Alignment::Center)
        .spacing(2.)
        .child(sign)
        .child(bar(colour, 12., STROKE))
        .child(bar(colour, 12., STROKE))
}

/// Which way a chevron points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pointing {
    Up,
    Down,
    Right,
}

/// Two chevrons, one over the other.
fn doubled(colour: Color, pointing: Pointing) -> Rect {
    rect()
        .cross_align(Alignment::Center)
        .spacing(-2.)
        .child(chevron(colour, pointing))
        .child(chevron(colour, pointing))
}

/// A chevron: a square's two sides, turned an eighth of the way round.
fn chevron(colour: Color, pointing: Pointing) -> Rect {
    // (top, right, bottom, left), and the nudge that centres the point in the box.
    let (sides, (x, y)) = match pointing {
        Pointing::Up => ((STROKE, 0., 0., STROKE), (0., 2.)),
        Pointing::Down => ((0., STROKE, STROKE, 0.), (0., -2.)),
        Pointing::Right => ((STROKE, STROKE, 0., 0.), (-2., 0.)),
    };
    rect()
        .width(Size::px(7.))
        .height(Size::px(7.))
        .offset_x(x)
        .offset_y(y)
        .border(stroke_border(colour, sides))
        .rotation(45.)
}
