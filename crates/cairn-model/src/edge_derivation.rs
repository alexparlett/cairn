//! The edges a row crosses, derived from the nearest lane snapshot above it and the lane
//! changes between.
//!
//! A row keeps only what happens to the lanes at it ([`LaneChange`]); every
//! [`LaneAssigner::snapshot_every`]th row also keeps the lines crossing into it
//! ([`LaneSnapshot`]). Advancing a snapshot through the rows below it reproduces, row by
//! row, every edge the assigner drew — in the order it drew them, so a row paints exactly
//! as it did when it carried them.
//!
//! [`LaneAssigner::snapshot_every`]: crate::LaneAssigner::snapshot_every

use crate::{EdgeSegment, GraphRow, Lane, LaneAssigner, LaneChange};

/// The lines crossing the top of one row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaneSnapshot {
    /// Bit `n` of word `n / 64`: lane `n` carries a line down into the row. No trailing
    /// zero word.
    open: Box<[u64]>,
    /// Out-of-order lines crossing into the row, in the order they are drawn.
    late: Box<[LateLine]>,
}

impl LaneSnapshot {
    pub(crate) fn highest_lane(&self) -> Option<Lane> {
        let open = self.open.iter().enumerate().rev().find_map(|(word, bits)| {
            (*bits != 0).then(|| word * 64 + 63 - bits.leading_zeros() as usize)
        });
        let late = self.late.iter().map(|line| line.lane.index()).max();
        open.max(late).map(Lane::new)
    }

    /// Heap bytes the snapshot holds, by capacity.
    pub fn heap_bytes(&self) -> usize {
        size_of_val::<[u64]>(&self.open) + size_of_val::<[LateLine]>(&self.late)
    }
}

/// An out-of-order line crossing into a row: down `lane`, ending at the commit `ends_in`
/// rows below (zero: this row's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LateLine {
    ends_in: u32,
    order: u32,
    lane: Lane,
}

impl LateLine {
    fn key(&self) -> (u32, u32) {
        (self.ends_in, self.order)
    }
}

/// One row as it is drawn: its lane and every line crossing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowEdges {
    pub lane: Lane,
    /// In the order the lane assigner drew them.
    pub edges: Vec<EdgeSegment>,
}

/// The edges crossing `rows[index]`, derived from the nearest row at or above it that
/// carries a snapshot. `None` when `index` is past the end, or when no row within
/// [`LaneAssigner::MAX_SNAPSHOT_EVERY`] above it carries one — never for rows a
/// [`LaneAssigner`] laid out and a reader kept from the first it was handed.
///
/// The work is one snapshot copied and advanced through at most the rows since it: bounded
/// by the snapshot interval, never by the history.
pub fn row_edges<R: AsRef<GraphRow>>(rows: &[R], index: usize) -> Option<RowEdges> {
    let row = rows.get(index)?.as_ref();
    let mut from = index;
    let snapshot = loop {
        if let Some(snapshot) = rows.get(from)?.as_ref().snapshot() {
            break snapshot;
        }
        if from == 0 || index - from + 1 >= LaneAssigner::MAX_SNAPSHOT_EVERY {
            return None;
        }
        from -= 1;
    };

    let mut state = LaneState::from_snapshot(snapshot);
    for above in rows.get(from..index)? {
        state.advance(above.as_ref());
    }
    Some(RowEdges {
        lane: row.lane,
        edges: state.edges(row),
    })
}

/// The lines crossing into the next row, as a walk over rows advances.
#[derive(Debug, Clone, Default)]
pub(crate) struct LaneState {
    open: Vec<u64>,
    /// Kept in drawing order: by the row each ends at, then by its rank among the lines
    /// ending there.
    late: Vec<LateLine>,
}

impl LaneState {
    fn from_snapshot(snapshot: &LaneSnapshot) -> Self {
        Self {
            open: snapshot.open.to_vec(),
            late: snapshot.late.to_vec(),
        }
    }

    pub(crate) fn snapshot(&self) -> LaneSnapshot {
        let used = self
            .open
            .iter()
            .rposition(|word| *word != 0)
            .map_or(0, |last| last + 1);
        LaneSnapshot {
            open: self.open.get(..used).unwrap_or_default().into(),
            late: self.late.as_slice().into(),
        }
    }

    fn open_lane(&mut self, lane: Lane) {
        let (word, bit) = (lane.index() / 64, lane.index() % 64);
        if self.open.len() <= word {
            self.open.resize(word + 1, 0);
        }
        if let Some(bits) = self.open.get_mut(word) {
            *bits |= 1 << bit;
        }
    }

    fn close_lane(&mut self, lane: Lane) {
        let (word, bit) = (lane.index() / 64, lane.index() % 64);
        if let Some(bits) = self.open.get_mut(word) {
            *bits &= !(1 << bit);
        }
    }

    fn open_lanes(&self) -> impl Iterator<Item = Lane> + '_ {
        self.open.iter().enumerate().flat_map(|(word, &bits)| {
            let mut rest = bits;
            std::iter::from_fn(move || {
                if rest == 0 {
                    return None;
                }
                let bit = rest.trailing_zeros() as usize;
                rest &= rest - 1;
                Some(Lane::new(word * 64 + bit))
            })
        })
    }

    /// From the lines crossing into `row` to those crossing into the row below it.
    pub(crate) fn advance(&mut self, row: &GraphRow) {
        for change in row.changes() {
            if let LaneChange::Ends(lane) = *change {
                self.close_lane(lane);
            }
        }
        self.late.retain(|line| line.ends_in != 0);
        for change in row.changes() {
            match *change {
                LaneChange::Starts(lane) => self.open_lane(lane),
                LaneChange::StartsLate { lane, rows, order } => self.late.push(LateLine {
                    ends_in: rows,
                    order,
                    lane,
                }),
                LaneChange::Ends(_) => {}
            }
        }
        for line in &mut self.late {
            line.ends_in = line.ends_in.saturating_sub(1);
        }
        self.late.sort_unstable_by_key(LateLine::key);
    }

    /// Every edge crossing `row`, given the lines crossing into it, in the assigner's
    /// order: the lines passing by lane, the lines ending at the node by lane, the lines
    /// leaving it in parent order, then every out-of-order line by the row it ends at.
    pub(crate) fn edges(&self, row: &GraphRow) -> Vec<EdgeSegment> {
        let own = row.lane;
        let changes = row.changes();
        let ends = |lane: Lane| changes.contains(&LaneChange::Ends(lane));
        let mut edges = Vec::with_capacity(
            self.open
                .iter()
                .map(|bits| bits.count_ones() as usize)
                .sum::<usize>()
                + changes.len()
                + self.late.len(),
        );

        edges.extend(
            self.open_lanes()
                .filter(|&lane| !ends(lane))
                .map(EdgeSegment::passing),
        );
        for change in changes {
            if let LaneChange::Ends(lane) = *change {
                edges.push(EdgeSegment::into_commit(lane, own));
            }
        }
        for change in changes {
            if let LaneChange::Starts(lane) = *change {
                edges.push(EdgeSegment::out_of_commit(own, lane));
            }
        }

        let mut starting: Vec<LateLine> = changes
            .iter()
            .filter_map(|change| match *change {
                LaneChange::StartsLate { lane, rows, order } => Some(LateLine {
                    ends_in: rows,
                    order,
                    lane,
                }),
                LaneChange::Ends(_) | LaneChange::Starts(_) => None,
            })
            .collect();
        starting.sort_unstable_by_key(LateLine::key);
        let mut crossing = self.late.iter().peekable();
        let mut leaving = starting.iter().peekable();
        loop {
            let take_crossing = match (crossing.peek(), leaving.peek()) {
                (Some(through), Some(out)) => through.key() < out.key(),
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            };
            let edge = if take_crossing {
                crossing.next().map(|line| {
                    if line.ends_in == 0 {
                        EdgeSegment::into_commit(line.lane, own)
                    } else {
                        EdgeSegment::passing(line.lane)
                    }
                })
            } else {
                leaving
                    .next()
                    .map(|line| EdgeSegment::out_of_commit(own, line.lane))
            };
            if let Some(edge) = edge {
                edges.push(edge.marked_out_of_order());
            }
        }
        edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Oid;

    fn row(changes: Vec<LaneChange>) -> GraphRow {
        GraphRow::new(Oid::from_bytes(&[9; 20]).unwrap(), Lane::new(0), changes)
    }

    /// Caught by: a snapshot that keeps the words a closed lane left empty, or counts its
    /// lines by something other than what it holds.
    #[test]
    fn a_snapshot_holds_the_words_its_open_lanes_need_and_its_late_lines() {
        let mut state = LaneState::default();
        state.advance(&row(vec![
            LaneChange::Starts(Lane::new(3)),
            LaneChange::Starts(Lane::new(130)),
            LaneChange::StartsLate {
                lane: Lane::new(5),
                rows: 4,
                order: 0,
            },
        ]));
        let wide = state.snapshot();
        assert_eq!(
            wide.heap_bytes(),
            3 * size_of::<u64>() + size_of::<LateLine>()
        );
        assert_eq!(wide.highest_lane(), Some(Lane::new(130)));

        state.advance(&row(vec![LaneChange::Ends(Lane::new(130))]));
        let narrow = state.snapshot();
        assert_eq!(
            narrow.heap_bytes(),
            size_of::<u64>() + size_of::<LateLine>(),
            "the words lane 130 needed outlived it"
        );
        assert_eq!(narrow.highest_lane(), Some(Lane::new(5)));
    }
}
