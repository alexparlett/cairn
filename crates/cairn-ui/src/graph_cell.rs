//! The graph column of one row, painted.
//!
//! Everything about *where* a line goes is decided in
//! [`crate::graph_geometry`]; this file only turns those numbers into Skia
//! calls. Keeping the split means the arithmetic is unit-tested and the part
//! that needs a window is a straight transcription with no decisions in it.
//!
//! **Why one canvas per row and not a rect per segment.** A canvas is one
//! element whatever the row holds; a rect per segment is up to a dozen per row
//! at the p99 measured on a real repository, laid out by torin every frame, and
//! it cannot draw a curve at all. A bent line drawn as a right angle reads as a
//! different kind of connection.
//!
//! **What a caller must keep true.** A `Canvas`'s render callback is compared
//! equal to every other callback (`RenderCallback`'s `PartialEq` returns true
//! unconditionally, in the pinned Freya revision), so a canvas whose DRAWING
//! changed while its LAYOUT did not is not repainted. Two things keep that from
//! mattering here, and both are load-bearing:
//!
//! 1. Every row element is keyed by its `RowId`, so the row that arrives at a
//!    given place in the viewport during a scroll replaces the one that was
//!    there rather than inheriting its painting.
//! 2. What a row draws is a function of the row alone, and a row reaching a
//!    view is final: `cairn-git` hands out only rows its lane assigner has
//!    already evicted, so the edge repainting R1.2 permits happens entirely
//!    before delivery. Nothing here is a function of selection, focus or
//!    hover — those live on the surrounding `rect`, which does diff on style.

use cairn_model::GraphRow;
use freya::engine::prelude::{Paint, PaintStyle, PathBuilder, PathEffect, SkColor};
use freya::prelude::*;

use crate::graph_geometry::{
    self, NODE_RADIUS, Node, ROW_HEIGHT, RowGeometry, STROKE_WIDTH, Stroke,
};
use crate::lane_palette::lane_colour;

/// Painted length and gap of an out-of-order line (decision O4).
const DASH: [f32; 2] = [4.0, 3.0];

/// Stroke width of a merge commit's ring.
const RING_WIDTH: f32 = 2.0;

/// The graph column for one row, `lanes` columns wide.
pub(crate) fn graph_cell(row: &GraphRow, parents: usize, lanes: usize) -> Canvas {
    let geometry = graph_geometry::row_geometry(row, parents);

    canvas(RenderCallback::new(move |context| {
        paint_row(context.canvas, &geometry);
    }))
    .width(Size::px(graph_geometry::graph_width(lanes)))
    .height(Size::px(ROW_HEIGHT))
}

fn paint_row(canvas: &freya::engine::prelude::Canvas, geometry: &RowGeometry) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    for stroke in &geometry.strokes {
        paint_stroke(canvas, &mut paint, stroke);
    }

    paint.set_path_effect(None);
    paint.set_color(SkColor::from(lane_colour(geometry.node_lane)));
    match geometry.node {
        Node::Dot => {
            paint.set_style(PaintStyle::Fill);
            canvas.draw_circle(geometry.node_at, NODE_RADIUS, &paint);
        }
        Node::Ring => {
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(RING_WIDTH);
            canvas.draw_circle(geometry.node_at, NODE_RADIUS, &paint);
        }
    }
}

fn paint_stroke(canvas: &freya::engine::prelude::Canvas, paint: &mut Paint, stroke: &Stroke) {
    paint.set_style(PaintStyle::Stroke);
    paint.set_stroke_width(STROKE_WIDTH);
    paint.set_color(SkColor::from(lane_colour(stroke.colour_lane)));
    paint.set_path_effect(if stroke.dashed {
        PathEffect::dash(&DASH, 0.0)
    } else {
        None
    });

    if (stroke.from.0 - stroke.to.0).abs() < f32::EPSILON {
        canvas.draw_line(stroke.from, stroke.to, paint);
        return;
    }

    // A line that changes lane is drawn as a curve rather than a right angle:
    // the eye follows a curve across a crowded graph, and a right angle reads
    // as a different kind of connection rather than the same line moving over.
    let middle = (stroke.from.1 + stroke.to.1) / 2.0;
    let mut path = PathBuilder::new();
    path.move_to(stroke.from)
        .cubic_to((stroke.from.0, middle), (stroke.to.0, middle), stroke.to);
    canvas.draw_path(&path.detach(), paint);
}
