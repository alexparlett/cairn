//! Cairn's component library.
//!
//! Components here render [`cairn_model`] values and emit intent through
//! `EventHandler` props. They never open a repository, touch the filesystem, or
//! block: the crate does not depend on `cairn-git` and the guard suite pins
//! that. Wiring a component to the engine is `cairn-app`'s job.
//!
//! The history view is split so that the part which can be *wrong* is testable
//! without a window: [`graph_geometry`] is pure arithmetic with unit tests,
//! [`lane_palette`] is a table, `graph_cell` is a transcription of the first
//! two into Skia calls, and [`HistoryList`] owns virtualisation, selection and
//! the keyboard.

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
