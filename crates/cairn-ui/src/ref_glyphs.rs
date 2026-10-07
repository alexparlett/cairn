//! The glyphs a ref's chip, the title bar and the sidebar carry (refs-and-status R5.1, R5.4,
//! R5.5, R7.1, R8.2), drawn as Fork draws its label icons: a tag for a tag, a generic remote —
//! a cloud — for a remote-tracking ref (a forge's own icon is packet 6's), a box for a stash, a
//! check mark for the current branch, a branch for any other branch and the title bar's; and,
//! in the sidebar, a folder, a warning triangle for a branch whose upstream is gone (Fork's
//! icon for it), and the triangle that says whether a section or folder is open.
//!
//! Each is painted as a path on a canvas — strokes and fills, never a character — so no font
//! is needed and none can be missing. Each is drawn in one colour, the chip's text colour, so
//! what tells one kind of chip from another is the glyph's shape, never its colour (R5.5):
//! the shapes differ pixel for pixel (`every_glyph_paints_a_shape_no_other_glyph_paints`).
//!
//! A canvas is repainted only when its layout changes, never when what it would paint does
//! (`RenderCallback` always compares equal), so a glyph's element must be keyed by what it
//! draws: a chip is keyed by its kind and name (`ref_chips`).

use freya::engine::prelude::{Paint, PaintStyle, PathBuilder, SkColor};
use freya::prelude::*;

/// The square every glyph is painted in.
pub const GLYPH_SIZE: f32 = 10.0;

/// A stroke's thickness.
const STROKE: f32 = 1.3;

/// Which glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefGlyph {
    /// A remote-tracking ref, or a local branch drawn compact with its upstream.
    Remote,
    Tag,
    Stash,
    /// The branch `HEAD` is on.
    Current,
    /// The title bar's current branch, and a branch in the sidebar.
    Branch,
    /// A folder of refs in the sidebar.
    Folder,
    /// A branch whose upstream is configured but gone: a warning triangle.
    Gone,
    /// An open section or folder: a triangle pointing down.
    Opened,
    /// A closed section or folder: a triangle pointing right.
    Closed,
}

impl RefGlyph {
    pub const ALL: [Self; 9] = [
        Self::Remote,
        Self::Tag,
        Self::Stash,
        Self::Current,
        Self::Branch,
        Self::Folder,
        Self::Gone,
        Self::Opened,
        Self::Closed,
    ];

    /// The glyph in `colour`, on a canvas of its own size.
    pub fn draw(self, colour: Color) -> Canvas {
        canvas(RenderCallback::new(move |context| {
            paint(context.canvas, self, colour);
        }))
        .width(Size::px(GLYPH_SIZE))
        .height(Size::px(GLYPH_SIZE))
    }
}

/// Paints `glyph` in `colour` into the canvas's top-left `GLYPH_SIZE` square.
pub(crate) fn paint(canvas: &freya::engine::prelude::Canvas, glyph: RefGlyph, colour: Color) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(SkColor::from(colour));
    paint.set_stroke_width(STROKE);
    match glyph {
        RefGlyph::Tag => {
            // A label's outline, pointed at the left, with its hole.
            paint.set_style(PaintStyle::Stroke);
            let mut path = PathBuilder::new();
            path.move_to((0.8, 5.0))
                .line_to((4.0, 1.2))
                .line_to((9.2, 1.2))
                .line_to((9.2, 8.8))
                .line_to((4.0, 8.8))
                .close();
            canvas.draw_path(&path.detach(), &paint);
            paint.set_style(PaintStyle::Fill);
            canvas.draw_circle((4.6, 5.0), 1.1, &paint);
        }
        RefGlyph::Remote => {
            // A cloud: three lobes over a flat base, filled.
            paint.set_style(PaintStyle::Fill);
            canvas.draw_circle((3.0, 6.3), 2.4, &paint);
            canvas.draw_circle((5.6, 4.6), 3.0, &paint);
            canvas.draw_circle((7.8, 6.6), 2.0, &paint);
            let mut base = PathBuilder::new();
            base.move_to((3.0, 6.0))
                .line_to((7.8, 6.0))
                .line_to((7.8, 8.6))
                .line_to((3.0, 8.6))
                .close();
            canvas.draw_path(&base.detach(), &paint);
        }
        RefGlyph::Stash => {
            // A box: its lid, its body and the slot in its front, outlined.
            paint.set_style(PaintStyle::Stroke);
            let mut lid = PathBuilder::new();
            lid.move_to((0.8, 1.2))
                .line_to((9.2, 1.2))
                .line_to((9.2, 3.6))
                .line_to((0.8, 3.6))
                .close();
            canvas.draw_path(&lid.detach(), &paint);
            let mut body = PathBuilder::new();
            body.move_to((1.8, 3.6))
                .line_to((1.8, 9.0))
                .line_to((8.2, 9.0))
                .line_to((8.2, 3.6));
            canvas.draw_path(&body.detach(), &paint);
            canvas.draw_line((4.0, 5.6), (6.0, 5.6), &paint);
        }
        RefGlyph::Current => {
            // A check mark.
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(STROKE + 0.3);
            let mut path = PathBuilder::new();
            path.move_to((1.2, 5.4))
                .line_to((4.0, 8.2))
                .line_to((8.8, 1.8));
            canvas.draw_path(&path.detach(), &paint);
        }
        RefGlyph::Branch => {
            // Two commits on a line and a third branched off it.
            paint.set_style(PaintStyle::Stroke);
            canvas.draw_line((2.6, 2.4), (2.6, 7.6), &paint);
            let mut fork = PathBuilder::new();
            fork.move_to((7.4, 3.6))
                .cubic_to((7.4, 6.0), (2.6, 5.2), (2.6, 7.0));
            canvas.draw_path(&fork.detach(), &paint);
            paint.set_style(PaintStyle::Fill);
            canvas.draw_circle((2.6, 1.8), 1.4, &paint);
            canvas.draw_circle((2.6, 8.2), 1.4, &paint);
            canvas.draw_circle((7.4, 2.4), 1.4, &paint);
        }
        RefGlyph::Folder => {
            // A folder: its tab and its body, outlined.
            paint.set_style(PaintStyle::Stroke);
            let mut path = PathBuilder::new();
            path.move_to((0.8, 2.0))
                .line_to((4.0, 2.0))
                .line_to((5.2, 3.4))
                .line_to((9.2, 3.4))
                .line_to((9.2, 8.6))
                .line_to((0.8, 8.6))
                .close();
            canvas.draw_path(&path.detach(), &paint);
        }
        RefGlyph::Gone => {
            // A warning triangle with its mark.
            paint.set_style(PaintStyle::Stroke);
            let mut path = PathBuilder::new();
            path.move_to((5.0, 0.8))
                .line_to((9.4, 9.0))
                .line_to((0.6, 9.0))
                .close();
            canvas.draw_path(&path.detach(), &paint);
            canvas.draw_line((5.0, 3.8), (5.0, 6.0), &paint);
            paint.set_style(PaintStyle::Fill);
            canvas.draw_circle((5.0, 7.5), 0.8, &paint);
        }
        RefGlyph::Opened => {
            paint.set_style(PaintStyle::Fill);
            let mut path = PathBuilder::new();
            path.move_to((0.6, 2.2))
                .line_to((9.4, 2.2))
                .line_to((5.0, 8.6))
                .close();
            canvas.draw_path(&path.detach(), &paint);
        }
        RefGlyph::Closed => {
            paint.set_style(PaintStyle::Fill);
            let mut path = PathBuilder::new();
            path.move_to((2.2, 0.6))
                .line_to((8.6, 5.0))
                .line_to((2.2, 9.4))
                .close();
            canvas.draw_path(&path.detach(), &paint);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freya::engine::prelude::{ImageInfo, raster_n32_premul};

    /// Which pixels of the glyph's square `glyph` paints.
    fn mask(glyph: RefGlyph) -> Vec<bool> {
        let side = GLYPH_SIZE as i32;
        let Some(mut surface) = raster_n32_premul((side, side)) else {
            panic!("no raster surface");
        };
        paint(surface.canvas(), glyph, Color::WHITE);
        let info = ImageInfo::new_n32_premul((side, side), None);
        let stride = info.min_row_bytes();
        let mut pixels = vec![0u8; stride * side as usize];
        assert!(surface.read_pixels(&info, &mut pixels, stride, (0, 0)));
        (0..side as usize)
            .flat_map(|y| (0..side as usize).map(move |x| (x, y)))
            .map(|(x, y)| pixels.get(y * stride + x * 4 + 3).copied().unwrap_or(0) > 96)
            .collect()
    }

    /// R5.5: a kind of label is told by its glyph's shape, never its colour — every glyph is
    /// painted in the same colour, so each must cover pixels no other does. Caught by: two
    /// kinds sharing a glyph, or a glyph that paints nothing.
    #[test]
    fn every_glyph_paints_a_shape_no_other_glyph_paints() {
        let masks: Vec<(RefGlyph, Vec<bool>)> = RefGlyph::ALL
            .iter()
            .map(|glyph| (*glyph, mask(*glyph)))
            .collect();
        for (glyph, painted) in &masks {
            let covered = painted.iter().filter(|p| **p).count();
            assert!(covered >= 8, "{glyph:?} paints {covered} pixels");
        }
        for (i, (one, first)) in masks.iter().enumerate() {
            for (other, second) in masks.iter().skip(i + 1) {
                let differing = first.iter().zip(second).filter(|(a, b)| a != b).count();
                assert!(
                    differing >= 10,
                    "{one:?} and {other:?} differ in only {differing} pixels"
                );
            }
        }
    }
}
