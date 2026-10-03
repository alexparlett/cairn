//! Cairn's component library.

pub mod accelerators;
mod columns;
mod commit_row;
mod commit_tab;
mod credential_prompt;
mod date_text;
mod detail_tabs;
mod diff_header;
mod diff_line_text;
pub mod diff_palette;
mod diff_settings;
mod diff_view;
mod graph_cell;
pub mod graph_geometry;
mod history_list;
pub mod lane_palette;
mod message_lines;
mod toggle_glyphs;

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
pub use diff_header::{
    DIFF_HEADER_HEIGHT, DiffHeader, ENTIRE_FILE_LABEL, FEWER_LINES_LABEL, HIDDEN_CHANGES_NOTICE,
    HeaderAction, IGNORE_WHITESPACE_LABEL, MORE_LINES_LABEL, NEXT_CHANGE_LABEL,
    PREVIOUS_CHANGE_LABEL, SIDE_BY_SIDE_LABEL,
};
pub use diff_line_text::{ShownLine, TAB_WIDTH, shown_line};
pub use diff_settings::DiffSettings;
pub use diff_view::{
    ChangeCursor, DIFF_ROW_HEIGHT, NO_NEWLINE_AT_END, ShownDiff, UnifiedDiffView, step_change,
};
pub use graph_geometry::{LANE_WIDTH, MAX_DRAWN_LANES, ROW_HEIGHT, graph_width};
pub use history_list::{HistoryList, PREFETCH_ROWS, RowRender, reveal_row};
