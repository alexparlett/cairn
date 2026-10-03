//! Cairn's component library.

pub mod accelerators;
mod commit_row;
mod commit_tab;
mod credential_prompt;
mod date_text;
mod detail_tabs;
mod graph_cell;
pub mod graph_geometry;
mod history_list;
pub mod lane_palette;
mod message_lines;

pub use commit_row::{
    AUTHOR_WIDTH, COLUMN_GAP, CommitRow, DATE_WIDTH, HistoryHeader, ID_WIDTH, ROW_FONT_SIZE,
    ROW_PADDING,
};
pub use commit_tab::{
    AUTHOR_CAPTION, COMMITTER_CAPTION, CommitTab, DETAIL_ROW_HEIGHT, ID_CAPTION, NO_FILES,
    PARENTS_CAPTION, cut_short_notice, file_text, status_letter,
};
pub use credential_prompt::CredentialPrompt;
pub use detail_tabs::{
    COLLAPSE_CAPTION, DETAIL_STRIP_HEIGHT, DetailTab, DetailTabs, EXPAND_CAPTION,
};
pub use graph_geometry::{LANE_WIDTH, MAX_DRAWN_LANES, ROW_HEIGHT, graph_width};
pub use history_list::{HistoryList, PREFETCH_ROWS, RowRender, reveal_row};
