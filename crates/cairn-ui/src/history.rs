//! The history list (`docs/systems/history-graph.md`): its rows and heading, the graph
//! cell's geometry and lane colours, each row's ref chips and their glyphs, and the date
//! column's text.

pub(crate) mod commit_row;
pub(crate) mod date_text;
pub(crate) mod graph_cell;
pub mod graph_geometry;
pub mod lane_palette;
pub(crate) mod list;
pub(crate) mod ref_chips;
pub(crate) mod ref_glyphs;
