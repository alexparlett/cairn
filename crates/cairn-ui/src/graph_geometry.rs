//! Row geometry, in row-local logical pixels with `y` growing downwards.

use cairn_model::{EdgeKind, EdgeSegment, GraphRow, Lane};

pub const ROW_HEIGHT: f32 = 26.0;

/// Distance between the centres of neighbouring lanes, in logical pixels.
pub const LANE_WIDTH: f32 = 14.0;

/// Radius of the dot or ring at a row's own commit.
pub const NODE_RADIUS: f32 = 4.0;

pub const STROKE_WIDTH: f32 = 1.8;

/// Lanes past this share the last column.
pub const MAX_DRAWN_LANES: usize = 24;

// A lane must be wider than the widest thing drawn in it.
const _: () = assert!(
    LANE_WIDTH >= 2.0 * NODE_RADIUS,
    "a lane is narrower than the node it holds: two lanes' nodes would overlap, \
     and a lane's identity is its column"
);
const _: () = assert!(
    LANE_WIDTH > STROKE_WIDTH,
    "a lane is narrower than the line that runs down it"
);

pub type Point = (f32, f32);

/// `colour_lane` is not always an endpoint's lane: a line keeps the colour of the track it runs in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub from: Point,
    pub to: Point,
    pub colour_lane: Lane,
    /// True for a line joining a commit to a parent drawn *above* it.
    pub dashed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    /// At most one parent: a filled dot.
    Dot,
    /// A merge.
    Ring,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowGeometry {
    pub strokes: Vec<Stroke>,
    pub node_at: Point,
    pub node_lane: Lane,
    pub node: Node,
}

/// Width of the graph column: at least one lane, at most [`MAX_DRAWN_LANES`].
pub fn graph_width(lanes: usize) -> f32 {
    drawn_lanes(lanes) as f32 * LANE_WIDTH
}

pub fn drawn_lanes(lanes: usize) -> usize {
    lanes.clamp(1, MAX_DRAWN_LANES)
}

/// Centre of `lane`'s column; lanes past [`MAX_DRAWN_LANES`] share the last one.
pub fn lane_x(lane: Lane) -> f32 {
    let index = lane.index().min(MAX_DRAWN_LANES - 1);
    (index as f32 + 0.5) * LANE_WIDTH
}

/// Where a node sits and where lines bend.
pub fn row_middle() -> f32 {
    ROW_HEIGHT / 2.0
}

/// `parents` decides the node shape only; the lines come from the row's segments.
pub fn row_geometry(row: &GraphRow, parents: usize) -> RowGeometry {
    let middle = row_middle();
    let strokes = row.edges.iter().map(|edge| stroke(edge, middle)).collect();

    RowGeometry {
        strokes,
        node_at: (lane_x(row.lane), middle),
        node_lane: row.lane,
        node: if parents > 1 { Node::Ring } else { Node::Dot },
    }
}

/// The colour lane is the end that is not the node.
fn stroke(edge: &EdgeSegment, middle: f32) -> Stroke {
    let top = |lane: Lane| (lane_x(lane), 0.0);
    let bottom = |lane: Lane| (lane_x(lane), ROW_HEIGHT);
    let node = |lane: Lane| (lane_x(lane), middle);

    let (from, to, colour_lane) = match edge.kind {
        EdgeKind::Passing => (top(edge.from), bottom(edge.to), edge.from),
        EdgeKind::IntoCommit => (top(edge.from), node(edge.to), edge.from),
        EdgeKind::OutOfCommit => (node(edge.from), bottom(edge.to), edge.to),
    };

    Stroke {
        from,
        to,
        colour_lane,
        dashed: edge.out_of_order,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::Oid;

    fn oid() -> Oid {
        match Oid::parse("0123456789abcdef0123456789abcdef01234567") {
            Ok(id) => id,
            Err(_) => unreachable!("a fixed valid hex id"),
        }
    }

    fn row(lane: usize, edges: Vec<EdgeSegment>) -> GraphRow {
        GraphRow {
            id: oid(),
            lane: Lane::new(lane),
            edges,
        }
    }

    /// Measured against the ink, not the spacing, so `LANE_WIDTH` cannot decide its own test.
    #[test]
    fn every_lane_below_the_cap_has_a_column_wider_than_what_is_drawn_in_it() {
        let ink = (2.0 * NODE_RADIUS).max(STROKE_WIDTH);
        let mut seen: Vec<f32> = Vec::new();
        for index in 0..MAX_DRAWN_LANES {
            let x = lane_x(Lane::new(index));
            assert!(
                seen.iter().all(|other| (other - x).abs() >= ink),
                "lane {index} at x={x} is closer than {ink} px — the width of what \
                 is drawn in a lane — to another lane: {seen:?}"
            );
            seen.push(x);
        }
        assert_eq!(seen.len(), MAX_DRAWN_LANES);
    }

    #[test]
    fn lanes_past_the_cap_share_the_last_column() {
        let last = lane_x(Lane::new(MAX_DRAWN_LANES - 1));
        assert_eq!(lane_x(Lane::new(MAX_DRAWN_LANES)), last);
        assert_eq!(lane_x(Lane::new(10_000)), last);
        assert_eq!(graph_width(10_000), MAX_DRAWN_LANES as f32 * LANE_WIDTH);
    }

    #[test]
    fn the_graph_column_is_never_zero_wide() {
        assert_eq!(graph_width(0), LANE_WIDTH);
        assert_eq!(graph_width(1), LANE_WIDTH);
        assert_eq!(graph_width(3), 3.0 * LANE_WIDTH);
    }

    #[test]
    fn a_passing_line_spans_the_whole_row_in_its_own_lane() {
        let geometry = row_geometry(&row(0, vec![EdgeSegment::passing(Lane::new(2))]), 1);
        let stroke = geometry.strokes[0];
        assert_eq!(stroke.from, (lane_x(Lane::new(2)), 0.0));
        assert_eq!(stroke.to, (lane_x(Lane::new(2)), ROW_HEIGHT));
        assert_eq!(stroke.colour_lane, Lane::new(2));
        assert!(!stroke.dashed);
    }

    #[test]
    fn lines_touching_a_commit_meet_at_its_node() {
        let geometry = row_geometry(
            &row(
                1,
                vec![
                    EdgeSegment::into_commit(Lane::new(0), Lane::new(1)),
                    EdgeSegment::out_of_commit(Lane::new(1), Lane::new(3)),
                ],
            ),
            2,
        );

        let into = geometry.strokes[0];
        let out = geometry.strokes[1];
        assert_eq!(
            into.to, geometry.node_at,
            "the incoming line missed the node"
        );
        assert_eq!(
            out.from, geometry.node_at,
            "the outgoing line missed the node"
        );
        assert_eq!(into.from, (lane_x(Lane::new(0)), 0.0));
        assert_eq!(out.to, (lane_x(Lane::new(3)), ROW_HEIGHT));
    }

    #[test]
    fn a_bending_line_takes_its_colour_from_the_track_not_the_node() {
        let geometry = row_geometry(
            &row(
                1,
                vec![
                    EdgeSegment::into_commit(Lane::new(4), Lane::new(1)),
                    EdgeSegment::out_of_commit(Lane::new(1), Lane::new(5)),
                ],
            ),
            1,
        );
        assert_eq!(geometry.strokes[0].colour_lane, Lane::new(4));
        assert_eq!(geometry.strokes[1].colour_lane, Lane::new(5));
    }

    /// Caught by: dashing only the segment leaving the commit.
    #[test]
    fn an_out_of_order_line_is_dashed_along_its_whole_length() {
        let pieces = [
            EdgeSegment::out_of_commit(Lane::new(0), Lane::new(2)),
            EdgeSegment::passing(Lane::new(2)),
            EdgeSegment::into_commit(Lane::new(2), Lane::new(1)),
        ];

        for plain in pieces {
            let marked = plain.marked_out_of_order();
            let ordinary = row_geometry(&row(0, vec![plain]), 1).strokes[0];
            let late = row_geometry(&row(0, vec![marked]), 1).strokes[0];

            assert!(
                !ordinary.dashed,
                "{:?} was dashed without being marked",
                plain.kind
            );
            assert!(
                late.dashed,
                "an out-of-order {:?} segment was drawn as an ordinary one",
                plain.kind
            );
            assert_eq!(late.from, ordinary.from);
            assert_eq!(late.to, ordinary.to);
            assert_eq!(late.colour_lane, ordinary.colour_lane);
        }
    }

    #[test]
    fn a_merge_is_a_different_shape_not_a_different_colour() {
        assert_eq!(row_geometry(&row(0, Vec::new()), 0).node, Node::Dot);
        assert_eq!(row_geometry(&row(0, Vec::new()), 1).node, Node::Dot);
        assert_eq!(row_geometry(&row(0, Vec::new()), 2).node, Node::Ring);
        assert_eq!(row_geometry(&row(0, Vec::new()), 7).node, Node::Ring);
    }

    #[test]
    fn a_row_draws_exactly_the_segments_it_carries() {
        let edges = vec![
            EdgeSegment::passing(Lane::new(0)),
            EdgeSegment::passing(Lane::new(1)),
            EdgeSegment::into_commit(Lane::new(2), Lane::new(2)),
        ];
        let geometry = row_geometry(&row(2, edges.clone()), 1);
        assert_eq!(geometry.strokes.len(), edges.len());
        assert_eq!(geometry.node_lane, Lane::new(2));
    }
}
