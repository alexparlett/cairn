//! The graph column of one row, painted.
//!
//! [`crate::graph_geometry`] decides where a line goes; this file only turns
//! those numbers into Skia calls. One canvas per row, not a rect per segment: a
//! rect cannot draw a curve.
//!
//! A `Canvas`'s render callback compares equal to every other callback
//! (`RenderCallback`'s `PartialEq` is unconditionally true in the pinned Freya
//! revision), so a canvas whose drawing changed while its layout did not is not
//! repainted. What a caller must keep true so that cannot matter: every row
//! element is keyed by its `RowId`, and what a row draws is a function of the
//! row alone — never of selection, focus or hover, which live on the
//! surrounding `rect`.

use cairn_model::GraphRow;
use freya::engine::prelude::{Paint, PaintStyle, PathBuilder, PathEffect, SkColor};
use freya::prelude::*;

use crate::graph_geometry::{
    self, NODE_RADIUS, Node, ROW_HEIGHT, RowGeometry, STROKE_WIDTH, Stroke,
};
use crate::lane_palette::lane_colour;

/// Painted length and gap of an out-of-order line (O4).
const DASH: [f32; 2] = [4.0, 3.0];

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

    // A curve, not a right angle: a right angle reads as a different kind of
    // connection.
    let middle = (stroke.from.1 + stroke.to.1) / 2.0;
    let mut path = PathBuilder::new();
    path.move_to(stroke.from)
        .cubic_to((stroke.from.0, middle), (stroke.to.0, middle), stroke.to);
    canvas.draw_path(&path.detach(), paint);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{EdgeSegment, Lane, Oid};
    use freya::engine::prelude::{ImageInfo, raster_n32_premul};

    fn oid() -> Oid {
        match Oid::from_bytes(&[7u8; 20]) {
            Ok(id) => id,
            Err(_) => unreachable!("20 bytes is a SHA-1"),
        }
    }

    /// Paints one row onto an offscreen Skia surface and reports, per pixel of
    /// `column`, whether anything was drawn. Two decisions no arithmetic test
    /// can reach: whether a dashed stroke is dashed, whether a merge is a ring.
    fn painted_column(row: &GraphRow, parents: usize, column: f32) -> Vec<bool> {
        let width = graph_geometry::graph_width(4).ceil() as i32;
        let height = ROW_HEIGHT.ceil() as i32;
        let Some(mut surface) = raster_n32_premul((width, height)) else {
            panic!("no raster surface");
        };
        let geometry = graph_geometry::row_geometry(row, parents);
        paint_row(surface.canvas(), &geometry);

        let info = ImageInfo::new_n32_premul((width, height), None);
        let stride = info.min_row_bytes();
        let mut pixels = vec![0u8; stride * height as usize];
        assert!(
            surface.read_pixels(&info, &mut pixels, stride, (0, 0)),
            "could not read the painted pixels back"
        );

        let x = column.round() as usize;
        (0..height as usize)
            .map(|y| {
                // Anti-aliasing puts partial coverage either side of a line, so
                // "painted" is any non-zero alpha rather than a full one.
                let alpha = pixels.get(y * stride + x * 4 + 3).copied().unwrap_or(0);
                alpha > 0
            })
            .collect()
    }

    fn row(lane: usize, edges: Vec<EdgeSegment>) -> GraphRow {
        GraphRow {
            id: oid(),
            lane: Lane::new(lane),
            edges,
        }
    }

    /// Caught by: deleting the dash branch in `paint_stroke`, which makes the
    /// two identical and which no geometry test can see (O4).
    #[test]
    fn an_out_of_order_line_is_actually_drawn_with_gaps() {
        let lane = Lane::new(1);
        let column = graph_geometry::lane_x(lane);

        let solid = painted_column(&row(0, vec![EdgeSegment::passing(lane)]), 1, column);
        let dashed = painted_column(
            &row(0, vec![EdgeSegment::passing(lane).marked_out_of_order()]),
            1,
            column,
        );

        assert!(
            solid.iter().all(|painted| *painted),
            "a solid line left gaps in its own column: {solid:?}"
        );
        assert!(
            dashed.iter().any(|painted| !*painted),
            "a dashed line was painted solid: the dash is what marks a line \
             running backwards, and it is the only thing that does"
        );
        assert!(
            dashed.iter().any(|painted| *painted),
            "a dashed line was not painted at all"
        );
    }

    /// Caught by: painting the ring filled, leaving colour as the only thing
    /// telling a merge from an ordinary commit (R4.2).
    #[test]
    fn a_merge_is_drawn_hollow_and_an_ordinary_commit_solid() {
        let node = row(0, Vec::new());
        let column = graph_geometry::lane_x(Lane::new(0));
        let middle = graph_geometry::row_middle().round() as usize;

        let dot = painted_column(&node, 1, column);
        let ring = painted_column(&node, 2, column);

        assert_eq!(
            dot.get(middle),
            Some(&true),
            "an ordinary commit's node was not painted at its own centre"
        );
        assert_eq!(
            ring.get(middle),
            Some(&false),
            "a merge was painted filled: the only thing telling it from an \
             ordinary commit would then be colour"
        );
        // A ring is still a node: it is painted somewhere in its column.
        assert!(
            ring.iter().any(|painted| *painted),
            "a merge was not painted at all"
        );
    }
}
