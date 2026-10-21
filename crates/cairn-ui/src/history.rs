//! The history list (`docs/systems/history-graph.md`): its rows, the graph cell's geometry
//! and lane colours, each row's ref chips and their glyphs, the dates' text, and the strip over
//! it while Show Lost Commits is on.

pub(crate) mod commit_row;
pub mod date_text;
pub(crate) mod graph_cell;
pub mod graph_geometry;
pub mod lane_palette;
pub(crate) mod list;
pub(crate) mod ref_chips;
pub(crate) mod ref_glyphs;
pub(crate) mod reflog_strip;
pub(crate) mod row_menu;
