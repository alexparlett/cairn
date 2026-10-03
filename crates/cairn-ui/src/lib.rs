//! Cairn's component library.

pub mod accelerators;
mod changes_list;
mod columns;
mod commit_row;
mod commit_tab;
mod credential_prompt;
mod date_text;
mod detail_tabs;
mod diff_header;
mod diff_line_text;
mod diff_notice;
pub mod diff_palette;
mod diff_row_parts;
mod diff_settings;
mod diff_view;
mod file_filter;
mod graph_cell;
pub mod graph_geometry;
mod history_list;
pub mod lane_palette;
mod message_lines;
mod side_by_side_rows;
mod toggle_glyphs;
mod unified_rows;

pub use changes_list::{
    ChangesList, ChangesSummary, FILTER_PLACEHOLDER, FILTERING, NO_FILE_MATCHES, SUMMARY_HEIGHT,
    filter_count, summary_parts,
};
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
pub use diff_line_text::{ShownLine, TAB_WIDTH, cut_marker, shown_line};
pub use diff_notice::{
    BINARY_FILE, CONFLICTED, COPIED_MODE_CHANGED, COPIED_WITHOUT_CHANGES, DiffNotice,
    DiffNoticeView, LFS_POINTER, LOAD_DIFF_CAPTION, MODE_CHANGED, NEW_SIDE, NO_CHANGES_SHOWN,
    NO_CONTENT_CHANGE, OLD_SIDE, ONLY_WHITESPACE_CHANGED, RENAMED_MODE_CHANGED,
    RENAMED_WITHOUT_CHANGES, SUBMODULE, TOO_LARGE_TO_DISPLAY, header_lines, size_text,
    subproject_line, too_large_reason,
};
pub use diff_settings::DiffSettings;
pub use diff_view::{
    ChangeCursor, DIFF_ROW_HEIGHT, DiffView, NO_NEWLINE_AT_END, content_width, step_change,
    text_width,
};
pub use file_filter::ShownFiles;
pub use graph_geometry::{LANE_WIDTH, MAX_DRAWN_LANES, ROW_HEIGHT, graph_width};
pub use history_list::{HistoryList, PREFETCH_ROWS, RowRender, reveal_row};
