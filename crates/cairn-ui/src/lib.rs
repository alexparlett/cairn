//! Cairn's component library.

pub mod accelerators;
mod check_box;
mod commit_box;
mod detail;
mod dialogs;
mod diff;
mod edge_scroll;
mod end_room;
mod history;
mod local_changes;
mod sidebar;
pub mod style;
mod text_field;
mod title_bar;

pub use commit_box::{
    AMEND_BUTTON_CAPTION, AMEND_CAPTION, AMEND_TITLE, Busy, CANCEL_COMMIT_CAPTION, CANCELLING,
    CommitBox, CommitButton, DESCRIPTION_PLACEHOLDER, DETACHED_LINE, RECENT_MESSAGES_CAPTION,
    RULER_COLUMN, SKIP_HOOKS_CAPTION, SUBJECT_HARD_LIMIT, SUBJECT_PLACEHOLDER, SUBJECT_SOFT_LIMIT,
    SubjectCount, UNBORN_LINE, commit_caption, elapsed_text, operation_line, subject_count,
};
pub use detail::changes_list::{
    BASE_CAPTION, ChangesList, ChangesSummary, ComparisonHeader, FILTER_PLACEHOLDER, FILTERING,
    NO_FILE_MATCHES, SUMMARY_HEIGHT, SWAP_LABEL, TIP_CAPTION, comparison_parts, filter_count,
    summary_parts,
};
pub use detail::commit_tab::{
    AUTHOR_CAPTION, COLLAPSE_ALL_CAPTION, COMMITTER_CAPTION, CommitTab, DETAIL_ROW_HEIGHT,
    EXPAND_ALL_CAPTION, ID_CAPTION, NO_FILES, PARENTS_CAPTION, READING_DIFF, REFS_CAPTION, Refs,
    budget_notice, cut_short_notice, file_text, status_letter,
};
pub use detail::expansion::{Expansion, Opened};
pub use detail::file_filter::ShownFiles;
pub use detail::tabs::{
    COLLAPSE_CAPTION, DETAIL_STRIP_HEIGHT, DetailTab, DetailTabs, EXPAND_CAPTION,
};
pub use dialogs::confirm::{
    CANCEL_CAPTION, COUNTING_TEXT, ConfirmDialog, DialogAnswer, NOTHING_CHANGED,
    SHOW_FILES_CAPTION, SHOWN_FILE_HEIGHT,
};
pub use dialogs::create_branch::{
    BRANCH_NAME_CAPTION, BRANCH_NAME_PLACEHOLDER, CANCEL_BRANCH_CAPTION, CHECK_OUT_AFTER_CREATE,
    COMES_WITH_STASHING, CREATE_AND_CHECKOUT_CAPTION, CREATE_BRANCH_AT, CREATE_BRANCH_SUBTITLE,
    CREATE_BRANCH_TITLE, CREATE_CAPTION, CreateBranchDialog, DISCARD_LOCAL_CAPTION,
    DONT_CHANGE_CAPTION, LOCAL_CHANGES_LABEL, LocalChoice, STASH_AND_REAPPLY_CAPTION,
    discard_refusal, name_refusal,
};
pub use dialogs::credential_prompt::CredentialPrompt;
pub use dialogs::git_error::{
    CLOSE_CAPTION, ERROR_DETAILS, GIT_ERROR_TEXT, GIT_ERROR_TITLE, GitErrorDialog,
    OUTPUT_ROW_HEIGHT, REMOVE_LOCK_CAPTION, couldnt_title,
};
pub use diff::header::{
    DIFF_HEADER_HEIGHT, DiffHeader, ENTIRE_FILE_LABEL, FEWER_LINES_LABEL, HIDDEN_CHANGES_NOTICE,
    HeaderAction, IGNORE_WHITESPACE_LABEL, MORE_LINES_LABEL, NEXT_CHANGE_LABEL,
    PREVIOUS_CHANGE_LABEL, SIDE_BY_SIDE_LABEL,
};
pub use diff::line_text::{ShownLine, TAB_WIDTH, cut_marker, shown_line};
pub use diff::notice::{
    BINARY_FILE, CONFLICTED, COPIED_MODE_CHANGED, COPIED_WITHOUT_CHANGES, DiffNotice,
    DiffNoticeView, LFS_POINTER, LOAD_DIFF_CAPTION, MODE_CHANGED, NEW_SIDE, NO_CHANGES_SHOWN,
    NO_CONTENT_CHANGE, NoticeRow, NoticeTone, OLD_SIDE, ONLY_WHITESPACE_CHANGED,
    RENAMED_MODE_CHANGED, RENAMED_WITHOUT_CHANGES, SUBMODULE, TOO_LARGE_TO_DISPLAY, header_lines,
    notice_rows, size_text, subproject_line, too_large_reason,
};
pub use diff::palette as diff_palette;
pub use diff::row_parts::{NUMBER_FONT_SIZE, NUMBER_PADDING, TEXT_PADDING, number_width};
pub use diff::settings::DiffSettings;
pub use diff::staging_gesture::{
    DISCARD_CHUNK_CAPTION, Gesture, GestureAct, GestureSide, GestureVerb, LineDrag, ModeRow,
    SELECTION_CLEARED, STAGE_CHUNK_CAPTION, UNSTAGE_CHUNK_CAPTION, chunk_caption, lines_caption,
    mode_caption,
};
pub use diff::view::{
    ChangeCursor, DIFF_ROW_HEIGHT, DiffView, NO_NEWLINE_AT_END, content_width, step_change,
    text_width,
};
pub use edge_scroll::{
    EDGE_BAND, EDGE_TICK, EdgeScroll, MOST_PER_TICK, edge_step, use_edge_scroll,
};
pub use end_room::{END_ROOM, SCROLLBAR_THICKNESS, with_end_room};
pub use history::commit_row::{
    AUTHOR_WIDTH, COLUMN_GAP, CommitRow, DATE_WIDTH, HistoryHeader, ID_WIDTH, ROW_FONT_SIZE,
    ROW_PADDING, SHOW_LOST_COMMITS_CAPTION, label_room,
};
pub use history::graph_geometry::{LANE_WIDTH, MAX_DRAWN_LANES, ROW_HEIGHT, graph_width};
pub use history::list::{HistoryList, NEW_BRANCH_CAPTION, PREFETCH_ROWS, RowRender, reveal_row};
pub use history::ref_chips::{
    CHIP_FONT_SIZE, CHIP_GAP, CHIP_HEIGHT, Chip, ChipKind, TAG_INDIGO, chip_element, chips_of_row,
    min_width, row_chips, tint,
};
pub use history::ref_glyphs::{GLYPH_SIZE, RefGlyph};
pub use history::{graph_geometry, lane_palette};
pub use local_changes::drag::DRAG_THRESHOLD;
pub use local_changes::list::{
    ChangeBadge, LIST_HEADER_HEIGHT, LISTS_SPLIT, ListIntent, LocalChangesList, NO_PATH_MATCHES,
    STAGED_CAPTION, UNSTAGED_CAPTION, change_badge, change_text, list_caption,
};
pub use local_changes::menu::{
    COPY_PATH_CAPTION, DISCARD_CAPTION, Kept, Left, MenuChoice, NoDiscard, STAGE_ALL_CAPTION,
    STAGE_CAPTION, UNSTAGE_ALL_CAPTION, UNSTAGE_CAPTION, kept, no_discard,
};
pub use local_changes::selection::{LocalSelection, nearest_remaining};
pub use sidebar::{
    ALL_COMMITS_CAPTION, BranchCounts, DETACHED_HEAD_CAPTION, DrawnRow, LOCAL_CHANGES_CAPTION,
    MainView, SIDEBAR_FILTER_PLACEHOLDER, SIDEBAR_INDENT, SIDEBAR_ROW_HEIGHT, Sidebar, SidebarRefs,
    SidebarTarget, drawn_row, local_changes_text, section_caption,
};
pub use text_field::{text_field, text_field_in, text_field_recalling};
pub use title_bar::activity_popover::{
    ACTIVITY_TITLE, ActivityEntry, ActivityLine, ActivityPopover, ActivityStatus,
    CANCEL_OPERATION_CAPTION, CANCELLED_GLYPH, CONFIRMED_CAPTION, EARLIER_NOT_KEPT,
    ENTRY_ROW_HEIGHT, FAILED_GLYPH, LINE_ROW_HEIGHT, NO_ACTIVITY, SHOW_IN_LOST_COMMITS_CAPTION,
};
pub use title_bar::lock_banner::{LOCK_BANNER_TEXT, LOCK_STALE_AFTER, LockBanner, lock_is_stale};
pub use title_bar::status_box::{
    NO_COMMITS_YET, PROGRESS_AFTER, Running, StatusBox, Tracking, UPSTREAM_GONE, counts_text,
    current_branch, head_text, name_text, progress_shown, running_text,
};
