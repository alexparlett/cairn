//! Cairn's component library.
//!
//! Components here render [`cairn_model`] values and emit intent through
//! `EventHandler` props. They never open a repository, touch the filesystem, or
//! block, and the guard suite pins that. Wiring a component to the engine is
//! `cairn-app`'s job; `docs/systems/history-graph.md` describes the view.

mod commit_row;
mod date_text;
mod graph_cell;
pub mod graph_geometry;
mod history_list;
pub mod lane_palette;

pub use commit_row::{
    AUTHOR_WIDTH, COLUMN_GAP, CommitRow, DATE_WIDTH, HistoryHeader, ID_WIDTH, ROW_PADDING,
};
pub use graph_geometry::{LANE_WIDTH, MAX_DRAWN_LANES, ROW_HEIGHT, graph_width};
pub use history_list::{HistoryList, PREFETCH_ROWS, RowRender};
