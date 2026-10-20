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
/// The room at the toolbar's two ends.
pub const TOOLBAR_INSET: f32 = 6.0;
/// The corner radius of a toolbar button's hover box and of the status box, Fork's rounded
/// rectangles.
pub const CONTROL_RADIUS: f32 = 4.0;
/// The room above and below a toolbar button's icon and label, and between the two.
pub const TOOLBAR_BUTTON_EDGE: f32 = 2.0;
/// The room either side of a count badge's figures.
pub const TOOLBAR_BADGE_PADDING: f32 = 3.0;
/// How far the status box's list glyph stands in from the box's left and bottom edges.
pub const STATUS_BOX_GLYPH_INSET: f32 = 4.0;
/// The room either side of the status box's lines: the glyph's, so the lines stay centred.
pub const STATUS_BOX_TEXT_INSET: f32 = STATUS_BOX_GLYPH + 12.0;
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
/// The strip across the top of the history while Show Lost Commits is on: ~22 px in Fork
/// (`fork-menu-toggles-2026-10-19.md`, §2: y≈118-139).
pub const REFLOG_STRIP: f32 = 22.0;
/// The room at a strip's two ends.
pub const STRIP_INSET: f32 = 10.0;
/// The room between a strip's sentence and its link.
pub const STRIP_GAP: f32 = 6.0;
/// The detail pane's Hide commit details icon, at its tab strip's right.
pub const STRIP_ICON: f32 = 14.0;
/// The status box's list glyph, at its left.
pub const STATUS_BOX_GLYPH: f32 = 14.0;

/// A history row: Fork's row pitch.
pub const HISTORY_ROW: f32 = 23.0;
/// The distance between the centres of two neighbouring lanes of the history's graph: 12 px in
/// Fork (uv2, the graph of uv7 at 4×: lane centres at x 315.5 and 327.5).
pub const HISTORY_LANE: f32 = 12.0;
/// A commit's dot on its lane: ~5 px across in Fork (uv2: 4 px wide, 5 px tall).
pub const HISTORY_DOT: f32 = 5.0;
/// A merge's hollow circle, across its outer edge: ~13 px in Fork (uv2: x 310-321, y 196-208).
pub const HISTORY_RING: f32 = 13.0;
/// A line of the graph, and a merge's circle's stroke: 2 px in Fork.
pub const HISTORY_LINE: f32 = 2.0;
/// A ref chip on a history row: a rounded rectangle ~17 px tall in Fork.
pub const CHIP: f32 = 17.0;
/// An author's avatar before the name: a rounded square ~14 px across in Fork.
pub const AVATAR: f32 = 14.0;
/// An avatar's corner radius.
pub const AVATAR_RADIUS: f32 = 3.0;
/// An avatar's initials: Cairn's size, two letters fitting the square; Fork's survey does not
/// size them.
pub const AVATAR_TEXT: f32 = 8.0;
/// A sidebar row.
pub const SIDEBAR_ROW: f32 = 22.0;
/// A row of Local Changes' lists.
pub const LOCAL_CHANGES_ROW: f32 = 20.0;
/// The bar over Local Changes' two lists and the rule under it: Fork's 17 px and 1 px (uw6).
pub const LISTS_BAR: f32 = 18.0;
/// A heading of Local Changes' lists and the rule under it: Fork's 32 px and 1 px (uw6).
pub const LIST_HEADING: f32 = 33.0;
/// A heading's bordered Stage or Unstage button: Fork's 22 px tall (uw6).
pub const LIST_HEADING_BUTTON: f32 = 22.0;
/// The room the button gives either side of its caption: Fork's Stage is 54 px across (uw6).
pub const LIST_HEADING_BUTTON_PADDING: f32 = 12.0;
/// The commit box's margin round its field and its buttons: Fork's 15 px (uw6).
pub const COMMIT_BOX_INSET: f32 = 15.0;
/// The commit box's subject, one part of its field: Fork's 22 px (uw6).
pub const COMMIT_SUBJECT: f32 = 22.0;
/// The commit button's height: Fork's 22 px (uw6).
pub const COMMIT_BUTTON: f32 = 22.0;
/// The commit button's ⌄, a segment of its own: Fork's 16 px across (uw6).
pub const COMMIT_CHEVRON: f32 = 16.0;
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
/// A diff row: Cairn's measured choice, kept (`diff::view`); the Commit tab's one pitch too.
pub const DIFF_ROW: f32 = 17.0;
/// The detail pane's strip of tabs, its rules above and below included: Fork's 31 px strip
/// between two 1 px rules (FS2 §1b, uw5).
pub const DETAIL_STRIP: f32 = 33.0;
/// A tab of the detail pane's strip, its caption centred: Fork's ~96 px from the pane's left
/// edge, its underline as wide (uw5).
pub const DETAIL_TAB: f32 = 96.0;
/// The Changes tab's summary line and the rule under it: Fork's 23 px and 1 px (uw5).
pub const DETAIL_SUMMARY: f32 = 24.0;
/// The bar over a diff and over the Changes tab's files, and the rule under it: Fork's 17 px
/// and 1 px (uw5).
pub const DIFF_BAR: f32 = 18.0;
/// A glyph of the diff bar: Fork's are ~12 px across inside their box (uw5a), as Lucide's are
/// in a 14 px one.
pub const DIFF_BAR_ICON: f32 = 14.0;
/// A diff bar button's square, which its hover fills: Fork's 16 px (cm15).
pub const DIFF_BAR_BUTTON: f32 = 16.0;
/// The room between two of the bar's buttons: Fork's repeat every 20 px (cm22).
pub const DIFF_BAR_GAP: f32 = 4.0;
/// The Commit tab's author's avatar: 42 px (uv9, measured).
pub const DETAIL_AVATAR: f32 = 42.0;
/// Its initials: Fork's fill about half the square's width (cm7z).
pub const DETAIL_AVATAR_TEXT: f32 = 20.0;
/// The Changes tab's summary's avatar: 18 px (uw5, measured).
pub const SUMMARY_AVATAR: f32 = 18.0;
/// Its initials.
pub const SUMMARY_AVATAR_TEXT: f32 = 9.0;
/// How far the Commit tab's content stands in from the pane's sides: Fork's 20 px (uv9).
pub const COMMIT_TAB_INSET: f32 = 20.0;
/// The Commit tab's column of captions and the avatar, which its values stand right of: Fork's
/// 62 px (uv9: the captions' and the avatar's right edges at 82 px).
pub const COMMIT_TAB_CAPTIONS: f32 = 62.0;
/// The room between the Commit tab's captions and their values: Fork's 16 px (uv9).
pub const COMMIT_TAB_GAP: f32 = 16.0;

/// The sidebar's width as the window opens.
pub const SIDEBAR_WIDTH: f32 = 300.0;
/// How far each level of the sidebar indents what it holds.
pub const SIDEBAR_INDENT: f32 = 12.0;
/// An icon of the sidebar — a branch, a folder, a tag — ~14 px across in uw27.
pub const SIDEBAR_ICON: f32 = 14.0;
/// A section's or folder's chevron, and the slot it stands in before an icon: a leaf keeps the
/// slot empty, so its icon lines up with its folders' (uw27).
pub const SIDEBAR_CHEVRON: f32 = 12.0;
/// The room between a sidebar row's icon and its name, and before its counts.
pub const SIDEBAR_GAP: f32 = 6.0;
/// How far a sidebar row stands in from the sidebar's left edge, and its counts from the right.
pub const SIDEBAR_INSET: f32 = 8.0;
/// How far a selected sidebar row's band stands in from either edge: Fork's runs x 5-296 of a
/// 301 px sidebar (sb22z).
pub const SIDEBAR_BAND_INSET: f32 = 5.0;
/// A selected sidebar row's band's corners: ~3 px in Fork (sb22z).
pub const SIDEBAR_BAND_RADIUS: f32 = 3.0;
/// The warning a branch whose upstream is gone carries on its glyph's bottom right (sb14g).
pub const SIDEBAR_BADGE: f32 = 8.0;

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
/// The Commit tab's subject, over its body: larger than the body text so the subject reads
/// first, as Fork's ~14 px bold subject does (uv9), within the tab's one row height.
pub const DETAIL_SUBJECT: f32 = 14.0;
/// The Commit tab's captions (AUTHOR, PARENTS, …), the activity popover's times and a check
/// box's tick: Cairn's size, a step under the body; Fork's survey does not size them.
pub const CAPTION: f32 = 11.0;
