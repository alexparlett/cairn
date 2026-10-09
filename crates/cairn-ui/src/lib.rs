//! Cairn's component library.

pub mod accelerators;
mod changes_list;
mod check_box;
mod columns;
mod commit_box;
mod commit_row;
mod commit_tab;
mod confirm_dialog;
mod create_branch_dialog;
pub use create_branch_dialog::{
    BRANCH_NAME_CAPTION, BRANCH_NAME_PLACEHOLDER, CANCEL_BRANCH_CAPTION, CHECK_OUT_AFTER_CREATE,
    CREATE_AND_CHECKOUT_CAPTION, CREATE_BRANCH_AT, CREATE_BRANCH_TITLE, CREATE_CAPTION,
    CreateBranchDialog, DISCARD_LOCAL_CAPTION, DONT_CHANGE_CAPTION, LOCAL_CHANGES_LABEL,
    LocalChoice,
};
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
mod edge_scroll;
pub use edge_scroll::{
    EDGE_BAND, EDGE_TICK, EdgeScroll, MOST_PER_TICK, edge_step, use_edge_scroll,
};
mod end_room;
pub use end_room::{END_ROOM, SCROLLBAR_THICKNESS, with_end_room};
mod expansion;
mod file_filter;
mod git_error_dialog;
pub use git_error_dialog::{
    CLOSE_CAPTION, ERROR_DETAILS, GIT_ERROR_TEXT, GIT_ERROR_TITLE, GitErrorDialog,
    OUTPUT_ROW_HEIGHT,
};
mod graph_cell;
pub mod graph_geometry;
mod history_list;
pub mod lane_palette;
mod list_selection;
pub use list_selection::{ListSelection, nearest_remaining};
mod local_changes;
mod local_changes_drag;
pub use local_changes_drag::DRAG_THRESHOLD;
mod local_changes_menu;
pub use local_changes_menu::{
    COPY_PATH_CAPTION, DISCARD_CAPTION, MenuChoice, NoDiscard, STAGE_ALL_CAPTION, STAGE_CAPTION,
    UNSTAGE_ALL_CAPTION, UNSTAGE_CAPTION, no_discard,
};
mod message_lines;
mod ref_chips;
mod ref_glyphs;
mod side_by_side_rows;
mod sidebar;
mod stacked_diff;
mod staging_gesture;
mod status_box;
mod text_field;
mod toggle_glyphs;
mod unified_rows;

pub use changes_list::{
    BASE_CAPTION, ChangesList, ChangesSummary, ComparisonHeader, FILTER_PLACEHOLDER, FILTERING,
    NO_FILE_MATCHES, SUMMARY_HEIGHT, SWAP_LABEL, TIP_CAPTION, comparison_parts, filter_count,
    summary_parts,
};
pub use commit_box::{
    AMEND_CAPTION, AmendButton, AmendSkip, Busy, CANCEL_COMMIT_CAPTION, CommitBox, CommitButton,
    DESCRIPTION_PLACEHOLDER, READING_AMEND, RECENT_MESSAGES_CAPTION, RULER_COLUMN,
    SKIP_HOOKS_CAPTION, SUBJECT_HARD_LIMIT, SUBJECT_PLACEHOLDER, SUBJECT_SOFT_LIMIT, SubjectCount,
    commit_caption, subject_count,
};
pub use commit_row::{
    AUTHOR_WIDTH, COLUMN_GAP, CommitRow, DATE_WIDTH, HistoryHeader, ID_WIDTH, ROW_FONT_SIZE,
    ROW_PADDING, SHOW_LOST_COMMITS_CAPTION, label_room,
};
pub use commit_tab::{
    AUTHOR_CAPTION, COLLAPSE_ALL_CAPTION, COMMITTER_CAPTION, CommitTab, DETAIL_ROW_HEIGHT,
    EXPAND_ALL_CAPTION, ID_CAPTION, NO_FILES, PARENTS_CAPTION, READING_DIFF, REFS_CAPTION, Refs,
    budget_notice, cut_short_notice, file_text, status_letter,
};
pub use confirm_dialog::{CANCEL_CAPTION, ConfirmDialog};
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
    NO_CONTENT_CHANGE, NoticeRow, NoticeTone, OLD_SIDE, ONLY_WHITESPACE_CHANGED,
    RENAMED_MODE_CHANGED, RENAMED_WITHOUT_CHANGES, SUBMODULE, TOO_LARGE_TO_DISPLAY, header_lines,
    notice_rows, size_text, subproject_line, too_large_reason,
};
pub use diff_row_parts::{NUMBER_FONT_SIZE, NUMBER_PADDING, TEXT_PADDING, number_width};
pub use diff_settings::DiffSettings;
pub use diff_view::{
    ChangeCursor, DIFF_ROW_HEIGHT, DiffView, NO_NEWLINE_AT_END, content_width, step_change,
    text_width,
};
pub use expansion::{Expansion, Opened};
pub use file_filter::ShownFiles;
pub use graph_geometry::{LANE_WIDTH, MAX_DRAWN_LANES, ROW_HEIGHT, graph_width};
pub use history_list::{HistoryList, NEW_BRANCH_CAPTION, PREFETCH_ROWS, RowRender, reveal_row};
pub use local_changes::{
    ChangeBadge, LIST_HEADER_HEIGHT, LISTS_SPLIT, ListIntent, LocalChangesList, NO_PATH_MATCHES,
    STAGED_CAPTION, UNSTAGED_CAPTION, change_badge, change_text, list_caption,
};
pub use ref_chips::{
    CHIP_FONT_SIZE, CHIP_GAP, CHIP_HEIGHT, Chip, ChipKind, TAG_INDIGO, chip_element, chips_of_row,
    min_width, row_chips, tint,
};
pub use ref_glyphs::{GLYPH_SIZE, RefGlyph};
pub use sidebar::{
    ALL_COMMITS_CAPTION, BranchCounts, DETACHED_HEAD_CAPTION, DrawnRow, LOCAL_CHANGES_CAPTION,
    MainView, SIDEBAR_FILTER_PLACEHOLDER, SIDEBAR_INDENT, SIDEBAR_ROW_HEIGHT, Sidebar, SidebarRefs,
    SidebarTarget, drawn_row, local_changes_text, section_caption,
};
pub use stacked_diff::{READING_FILE_DIFF, StackedDiff};
pub use staging_gesture::{
    DISCARD_CHUNK_CAPTION, Gesture, GestureAct, GestureSide, GestureVerb, LineDrag, ModeRow,
    STAGE_CHUNK_CAPTION, UNSTAGE_CHUNK_CAPTION, chunk_caption, lines_caption, mode_caption,
};
pub use status_box::{
    NO_COMMITS_YET, StatusBox, Tracking, UPSTREAM_GONE, counts_text, current_branch, head_text,
    name_text,
};
pub use text_field::{text_field, text_field_in, text_field_recalling};
