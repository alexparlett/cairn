//! Fork's sizes, in logical pixels, measured on Fork for Windows 2.23.2 at 1024×768
//! (`docs/research/ui-review/fork-window-survey-2026-10-16.md`), grouped by surface; and the
//! sizes of the surfaces Fork's survey does not size, each with where it comes from.

/// Body text: every row, label, menu, button and tooltip — Segoe UI at ~12 px in Fork.
pub const BODY: f32 = 12.0;
/// The toolbar's labels, under their icons: ~11 px in Fork.
pub const TOOLBAR_LABEL: f32 = 11.0;
/// A heading: a dialog's title (Create Branch's, ~14 px bold) and a list's heading
/// (Unstaged and Staged, bold ~14 px), as Fork draws them.
pub const HEADING: f32 = 14.0;

/// The caption row: the menus, the title and the window's controls.
pub const CAPTION_ROW: f32 = 26.0;
/// The toolbar under it.
pub const TOOLBAR: f32 = 41.0;

/// A history row: Fork's row pitch.
pub const HISTORY_ROW: f32 = 23.0;
/// A sidebar row.
pub const SIDEBAR_ROW: f32 = 22.0;
/// A row of Local Changes' lists.
pub const LOCAL_CHANGES_ROW: f32 = 20.0;
/// A menu item: the pitch between items of a context menu and a menu bar's menu.
pub const MENU_ITEM: f32 = 23.0;
/// A diff row: Cairn's measured choice, kept (`diff::view`).
pub const DIFF_ROW: f32 = 17.0;

/// The sidebar's width as the window opens.
pub const SIDEBAR_WIDTH: f32 = 300.0;
/// How far each level of the sidebar indents what it holds.
pub const SIDEBAR_INDENT: f32 = 12.0;

/// The diff's text size: Fork's, measured. Fork's Mac default diff font is Menlo at 11 pt
/// (`Menlo-Regular - 11.0` in two screenshots four years apart) with a 6.6 pt advance; a
/// point on macOS is a logical pixel, and IBM Plex Mono's 0.6 em advance at 11 px is the same
/// 6.6. Fork's Windows size is not established.
pub const DIFF_TEXT: f32 = 11.0;
/// The diff's line numbers: Fork's numbers are smaller than its text. Fork's Mac numbers are
/// 6.5 pt tall, and Plex Mono's figures are 0.698 em tall, so 6.5 ÷ 0.698 ≈ 9.3 px; a
/// Windows capture of Fork agrees — its digits advance 0.84 of its code's advance, and
/// 0.84 × 11 px ≈ 9.3 px. This toolkit build takes a font size in whole pixels (`FontSize`
/// holds an `i32`), so 9.
pub const DIFF_LINE_NUMBER: f32 = 9.0;
/// The Commit tab's subject, over its body: Cairn's size, larger than the body text so the
/// subject reads first; Fork's survey does not size it.
pub const DETAIL_SUBJECT: f32 = 15.0;
/// The Commit tab's captions (AUTHOR, PARENTS, …), the activity popover's times and a check
/// box's tick: Cairn's size, a step under the body; Fork's survey does not size them.
pub const CAPTION: f32 = 11.0;
/// The product's name in the title row: Cairn's size.
pub const TITLE: f32 = 16.0;

/// The diff bar's swap glyph, `↕` in IBM Plex Mono: Cairn's size, filling the bar's glyph box.
pub const SWAP_GLYPH: f32 = 14.0;
/// The `↕` drawn over lines in the diff bar's entire-file glyph: Cairn's size, small enough to
/// sit between the lines.
pub const ENTIRE_FILE_GLYPH: f32 = 11.0;
