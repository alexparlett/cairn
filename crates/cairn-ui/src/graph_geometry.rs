//! Where the lines of one row go, as pure arithmetic.
//!
//! Deliberately free of Freya and of Skia: given a [`GraphRow`] this produces
//! the segments to stroke in row-local coordinates, so the part of the graph
//! that can be *wrong* — a line that starts in the wrong lane, a node drawn
//! off its own track — is decidable by a unit test rather than by looking at a
//! screenshot. [`crate::graph_cell`] turns these into Skia calls and adds
//! nothing of its own.
//!
//! Row-local coordinates: `x` grows rightwards from the left edge of the graph
//! column, `y` downwards from the top edge of the row. A row is
//! [`ROW_HEIGHT`] tall and every row is the same height (locked decision L7),
//! which is what lets the list be virtualised without a measurement cache.

use cairn_model::{EdgeKind, EdgeSegment, GraphRow, Lane};

/// Height of every row in the history list, in logical pixels.
///
/// Uniform by L7. A variable-height row and a virtualised list together need a
/// measurement cache and produce scrollbar jitter; this packet does not spend
/// its budget there.
pub const ROW_HEIGHT: f32 = 26.0;

/// Horizontal distance between the centres of two neighbouring lanes.
pub const LANE_WIDTH: f32 = 14.0;

/// Radius of the dot (or ring) drawn where a row's own commit sits.
pub const NODE_RADIUS: f32 = 4.0;

/// Width of a connecting line.
pub const STROKE_WIDTH: f32 = 1.8;

/// How many lanes the graph column will grow to before it stops widening.
///
/// 24 lanes is 336 px, which is already more than the widest history measured
/// on a real repository in the order Cairn walks by default (8 lanes at the
/// maximum over every ref of the `freya` repository — see `progress.md`).
/// Beyond it a lane is drawn in the last column rather than pushing the subject
/// off the window; that is lossy, and it is the honest trade against a graph
/// column wider than the text it sits beside.
pub const MAX_DRAWN_LANES: usize = 24;

// The same property the lane-separation test decides, stated about the constants
// themselves and checked by the COMPILER rather than by a test: a lane has to be
// wider than the widest thing drawn in it, or two lanes' ink overlaps and lane
// identity stops surviving a monochrome screenshot. Retuning any of the three
// numbers above past that point does not compile.
const _: () = assert!(
    LANE_WIDTH >= 2.0 * NODE_RADIUS,
    "a lane is narrower than the node it holds: two lanes' nodes would overlap, \
     and a lane's identity is its column"
);
const _: () = assert!(
    LANE_WIDTH > STROKE_WIDTH,
    "a lane is narrower than the line that runs down it"
);

/// A point in row-local coordinates.
pub type Point = (f32, f32);

/// One line to stroke across a row.
///
/// `colour_lane` is the lane the line's *colour* comes from, which is not
/// always either endpoint's lane: a line that changes lane at this row keeps
/// the colour of the track it runs in above and below, so a single connection
/// is one colour down its whole length instead of changing hue wherever it
/// bends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub from: Point,
    pub to: Point,
    pub colour_lane: Lane,
    /// Drawn dashed. True exactly for a line joining a commit to a parent drawn
    /// *above* it — see `EdgeSegment::out_of_order` and decision O4.
    pub dashed: bool,
}

/// What a row's own commit is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    /// A commit with at most one parent: a filled dot.
    Dot,
    /// A merge: a ring. A SHAPE, not a colour, because the product rules say
    /// colour may never carry meaning alone.
    Ring,
}

/// Everything to draw for one row.
#[derive(Debug, Clone, PartialEq)]
pub struct RowGeometry {
    pub strokes: Vec<Stroke>,
    pub node_at: Point,
    pub node_lane: Lane,
    pub node: Node,
}

/// Width of the graph column holding `lanes` lanes.
///
/// At least one lane wide, so a linear history still has a column to draw its
/// single track in, and never wider than [`MAX_DRAWN_LANES`].
pub fn graph_width(lanes: usize) -> f32 {
    drawn_lanes(lanes) as f32 * LANE_WIDTH
}

/// How many lanes are actually given a column.
pub fn drawn_lanes(lanes: usize) -> usize {
    lanes.clamp(1, MAX_DRAWN_LANES)
}

/// Centre of `lane`'s column, in row-local x.
///
/// Lanes past [`MAX_DRAWN_LANES`] share the last column: they are drawn on top
/// of each other rather than off the edge of the window.
pub fn lane_x(lane: Lane) -> f32 {
    let index = lane.index().min(MAX_DRAWN_LANES - 1);
    (index as f32 + 0.5) * LANE_WIDTH
}

/// Vertical middle of a row, where a node sits and where lines bend.
pub fn row_middle() -> f32 {
    ROW_HEIGHT / 2.0
}

/// Lay out one row's lines and node.
///
/// `parents` is how many parents the row's commit has, which decides the node
/// shape and nothing else — the lines themselves come from the row's own
/// segments, so a renderer never has to know what a commit is.
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

/// One segment, clipped to one row.
///
/// The three kinds are the whole geometric vocabulary (`EdgeKind`): a line
/// crosses the row, ends at its commit, or leaves it. The colour lane is the
/// end that is *not* the node, because that is the track the line belongs to
/// above and below this row; for a passing line both ends are that track.
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

    /// The property the product rules actually ask for: lane identity must
    /// survive a monochrome screenshot. It survives because a lane's POSITION
    /// is its identity — so the separation between columns has to be measured
    /// against the INK that sits in them, not against the spacing that produced
    /// it. Comparing the gap to a fraction of `LANE_WIDTH` would be true for
    /// every positive `LANE_WIDTH`, including one narrow enough for every dot
    /// to swallow its neighbours.
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

    /// The cap is lossy, and says so out loud rather than drawing off the edge
    /// of the window.
    #[test]
    fn lanes_past_the_cap_share_the_last_column() {
        let last = lane_x(Lane::new(MAX_DRAWN_LANES - 1));
        assert_eq!(lane_x(Lane::new(MAX_DRAWN_LANES)), last);
        assert_eq!(lane_x(Lane::new(10_000)), last);
        assert_eq!(graph_width(10_000), MAX_DRAWN_LANES as f32 * LANE_WIDTH);
    }

    /// A column exists even for a history with one lane: a linear history still
    /// draws a track.
    #[test]
    fn the_graph_column_is_never_zero_wide() {
        assert_eq!(graph_width(0), LANE_WIDTH);
        assert_eq!(graph_width(1), LANE_WIDTH);
        assert_eq!(graph_width(3), 3.0 * LANE_WIDTH);
    }

    /// A passing line runs the full height of the row in one column, so a track
    /// crossing many rows is continuous from the top of the list to the bottom.
    #[test]
    fn a_passing_line_spans_the_whole_row_in_its_own_lane() {
        let geometry = row_geometry(&row(0, vec![EdgeSegment::passing(Lane::new(2))]), 1);
        let stroke = geometry.strokes[0];
        assert_eq!(stroke.from, (lane_x(Lane::new(2)), 0.0));
        assert_eq!(stroke.to, (lane_x(Lane::new(2)), ROW_HEIGHT));
        assert_eq!(stroke.colour_lane, Lane::new(2));
        assert!(!stroke.dashed);
    }

    /// The two halves of a connection meet at the node, so a line arriving from
    /// above and one leaving below join without a gap.
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

    /// A bending line keeps the colour of the track it runs in, not of the
    /// node it touches: one connection is one colour down its whole length.
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

    /// O4: a line joining a commit to a parent drawn ABOVE it is dashed, and
    /// dashing is the only thing that changes. Its geometry stays identical to
    /// any other line, because the connection is not a different KIND of
    /// connection — only its ancestry runs the other way.
    ///
    /// Every segment KIND, not just the one that leaves the commit. An
    /// out-of-order connection spans the rows between parent and child, and the
    /// assigner marks all three pieces of it (`LaneAssigner::connect_upward`);
    /// a rule that dashed only the first would draw one dashed row and then a
    /// solid line, which reads as two different connections.
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

    /// Merges are told apart by SHAPE. A monochrome screenshot still says which
    /// commits are merges, which is what the product rule asks.
    #[test]
    fn a_merge_is_a_different_shape_not_a_different_colour() {
        assert_eq!(row_geometry(&row(0, Vec::new()), 0).node, Node::Dot);
        assert_eq!(row_geometry(&row(0, Vec::new()), 1).node, Node::Dot);
        assert_eq!(row_geometry(&row(0, Vec::new()), 2).node, Node::Ring);
        assert_eq!(row_geometry(&row(0, Vec::new()), 7).node, Node::Ring);
    }

    /// A row draws every segment it carries and invents none: the model decides
    /// what is on the row, the view only places it.
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
