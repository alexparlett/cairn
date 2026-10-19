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
/// A toolbar button's icon, over its label: ~20 px in Fork.
pub const TOOLBAR_ICON: f32 = 20.0;
/// The least width of a toolbar button: Fork's narrowest, ~45 px.
pub const TOOLBAR_BUTTON_MIN_WIDTH: f32 = 44.0;
/// The room either side of a toolbar button's icon and label.
pub const TOOLBAR_BUTTON_PADDING: f32 = 6.0;
/// The gap between the toolbar's groups (Fork's sidebar and Quick Launch, the remote's three,
/// Stash, the status box, Branch, Appearance).
pub const TOOLBAR_GROUP_GAP: f32 = 14.0;
/// The ⌄ beside Stash, Branch and Appearance.
pub const TOOLBAR_CHEVRON: f32 = 12.0;
/// A count badge on Push's or Pull's icon: its diameter, a grey circle at the icon's top right.
pub const TOOLBAR_BADGE: f32 = 14.0;
/// A count badge's figures.
pub const TOOLBAR_BADGE_TEXT: f32 = 9.0;
/// The status box: ~300 × 36 px at 1024 px wide in Fork, centred in the toolbar.
pub const STATUS_BOX_WIDTH: f32 = 300.0;
/// The status box's height.
pub const STATUS_BOX_HEIGHT: f32 = 36.0;
/// The narrowest the status box shrinks to as the window narrows, before anything else gives.
pub const STATUS_BOX_MIN_WIDTH: f32 = 200.0;
/// The status box's list glyph, at its left.
pub const STATUS_BOX_GLYPH: f32 = 14.0;

/// A history row: Fork's row pitch.
pub const HISTORY_ROW: f32 = 23.0;
/// A sidebar row.
pub const SIDEBAR_ROW: f32 = 22.0;
/// A row of Local Changes' lists.
pub const LOCAL_CHANGES_ROW: f32 = 20.0;
/// A menu item: the pitch between items of a context menu and a menu bar's menu.
pub const MENU_ITEM: f32 = 23.0;
/// A menu's narrowest width: Cairn's, so a menu of short items is not a sliver; Fork's own are
/// as wide as their longest item.
pub const MENU_MIN_WIDTH: f32 = 180.0;
/// How far a menu item's label stands in from the band's left, and its chord from the right.
pub const MENU_INSET: f32 = 24.0;
/// The gutter a ticked item's ✓ sits in, opened before every label of a menu that holds one:
/// Fork's labels move 18 px right while one is ticked
/// (`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`).
pub const MENU_TICK_GUTTER: f32 = 18.0;
/// The least room between a menu item's label and its chord.
pub const MENU_CHORD_GAP: f32 = 40.0;
/// The width a greyed menu item's reason wraps at, drawn under it while the pointer is on it.
pub const MENU_REASON_WIDTH: f32 = 300.0;
/// The room either side of a menu bar's name.
pub const MENU_BAR_PADDING: f32 = 8.0;
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

/// The diff bar's swap glyph, `↕` in IBM Plex Mono: Cairn's size, filling the bar's glyph box.
pub const SWAP_GLYPH: f32 = 14.0;
/// The `↕` drawn over lines in the diff bar's entire-file glyph: Cairn's size, small enough to
/// sit between the lines.
pub const ENTIRE_FILE_GLYPH: f32 = 11.0;
