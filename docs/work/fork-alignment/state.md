# State — fork-alignment

The cross-session cheat sheet. Every session updates it before ending.

**Status: in progress, 2026-10-20.** `history-topo-order` (roadmap 5c) merged as #142; the
integration branch `feature/fork-alignment` was cut from it. Phases 01, 02, 03, 04, 05 and 06 done (packet mode).

## Locked decisions (brainstorm.md has the evidence and the rejections)

- **S1-S5.** The packet's shape:
  - depth: layout, behaviour and density;
  - the shell now, actions later;
  - it runs next, before remote-sync;
  - the file tree, Quick Launch, Preferences and the repository manager are packets of their own;
  - topological order is packet 5c, run first.
- **L1. Caption row.** Fork's caption row on Linux: client-side decorations with menus, title and
  controls. macOS keeps its native frame, the menus a row inside the window.
- **L2. Unbuilt actions.**
  - The toolbar greys a planned action, its tooltip "Comes with <feature>.".
  - Menus list only what works.
  - What no packet plans is absent everywhere.
- **L3. Palette.**
  - Fork Dark's tones and its focused (#236BD2) and unfocused (#525252) selection.
  - The lanes keep Cairn's Okabe-Ito eight.
- **L4. Font and icons.**
  - No embedded UI font: Freya's fallback sans at Fork's sizes, 12 px body.
  - Lucide through `icons-lucide`, pinned in `deny.toml`.
- **L5. Local time.**
  - Through jiff, already in `Cargo.lock` by way of gix-date.
  - The zone is read on a worker and handed to `cairn-ui`, which never reads it itself.
- **L6. Three deviations reversed.**
  - Show Lost Commits moves to the View menu, with a strip while it is on.
  - Stage All moves to the bar over both lists; a visible Unstage All stays on the Staged heading.
  - A stash is drawn on its base's lane.
- **L7. No tree.** View Mode offers List alone.
- **L8. Not in this packet:** tabs, the manager, Preferences and Quick Launch.
- **L9. Ctrl+Shift+D** is Hide Commit Details on Linux. Discard keeps Backspace and Delete there.
- **L10. Dependencies.** New crates `freya-icons` (Lucide, ISC recorded in `deny.toml`) and
  `freya-borderless-plugin`, both from the pinned Freya source. No `muda`: macOS draws the menus
  as a row inside the window.

## Interfaces introduced so far

Phases record here what they add: module paths, public types, new `cairn-model` vocabulary,
accelerator actions, guard names, `deny.toml` pins.

**Phase 01:**
- `cairn_ui::style` (`crates/cairn-ui/src/style.rs` beside `style/`), public:
  - `style::metrics` — R1.2's table (`BODY`, `TOOLBAR_LABEL`, `CAPTION_ROW`, `TOOLBAR`,
    `HISTORY_ROW`, `SIDEBAR_ROW`, `LOCAL_CHANGES_ROW`, `MENU_ITEM`, `DIFF_ROW`,
    `SIDEBAR_WIDTH`, `SIDEBAR_INDENT`) plus `HEADING`, `DIFF_TEXT`, `DIFF_LINE_NUMBER`,
    `DETAIL_SUBJECT`, `CAPTION`, `TITLE`, `SWAP_GLYPH`, `ENTIRE_FILE_GLYPH`. Only the font sizes
    are read today; `HISTORY_ROW` (23), `LOCAL_CHANGES_ROW`, `MENU_ITEM`, `CAPTION_ROW`,
    `TOOLBAR`, `SIDEBAR_WIDTH` and `SIDEBAR_INDENT` (12; the sidebar still indents 14) wait for
    the phases that lay those regions out. `SIDEBAR_ROW_HEIGHT` and `DIFF_ROW_HEIGHT` now read
    their metric. The history's `ROW_HEIGHT` is still 26.
  - `style::tones` — R1.3's table as `pub const Color`s, extras sampled in FS2 §1
    (`PLACEHOLDER`, `SIDEBAR_CHOSEN`, `BANNER`, `FIELD`, `CHECK_BOX_BORDER`, `LINK`,
    `ADDED_BADGE`, `MODIFIED_BADGE`, `MENU_SHADOW`, `DISABLED_BORDER`), `Selected` and
    `selected(focused)`, `chip_fill(lane)`, `sheet()` and `theme()` (the launch theme, installed
    in `crates/cairn-app/src/main.rs`).
  - `style::icons` — `branch`, `cloud`, `archive`, `folder`, `gear`, `magnifier`, `sidebar`
    (Lucide), each `fn(size, colour) -> Element`; none drawn by a view yet.
- `RowRender::focused` and `CommitRow::focused(bool)`: a history row's selection tone. The
  sidebar's list is focusable and takes focus on a row press (no keys yet: phase 05, C14).
- `diff_palette::HUNK_HEADER`; the diff palette no longer retunes (Cairn's diff ground is
  Fork's `#282828`), and its bands and hunk header are R1.3's — R7.5 is met already.
- Chips: tags and stashes toned by kind, branches and remotes bordered by lane and filled with
  `chip_fill` (R1.3's chip rows); the chip's height and shape stay for phase 04 (R5.5).
- The window root sets `font_size(metrics::BODY)` and `background(tones::HISTORY)`.
- Guards: `chrome_sizes_and_tones_come_from_the_style_module` (`STYLE_EXCEPTIONS`:
  `diff/palette.rs`, `history/lane_palette.rs`; tones only) and
  `every_icon_is_named_through_the_icon_module`, matchers `style_literals`,
  `names_an_icon_set`, rosters `COLOUR_CONSTRUCTORS`, `COLOUR_CONSTANTS`, `ICON_SET_IDENTS`.
- `deny.toml`: `[[bans.features]] crate = "freya"`, exact `["default", "engine", "icons",
  "icons-lucide", "winit"]` — phase 02 adds `titlebar` and `borderless` to it in the same change
  that enables them (exact refuses an allowed feature not enabled); Lucide's ISC licence noted
  under `[licenses]`. Lockfile: `freya-icons` alone.

**Phase 02:**
- `cairn_ui::menu` (`crates/cairn-ui/src/menu.rs` beside `menu/`): `Item<T>` (`label`, `chord:
  Option<Action>`, `reason: Option<String>` — greyed iff set — `ticked`, `kind: ItemKind<T>`:
  `Choose(T)` | `Submenu(Groups<T>)`), `Groups<T>`, `context_menu(groups, on_choose) -> Menu`
  (first item highlighted), `MenuBar<T>`/`BarMenu<T>` (Fork's bar: press opens with nothing
  highlighted, hover switches, ←/→ wrap, press on the open name closes). `menu/panel.rs` draws
  rows (`metrics::MENU_ITEM`), rules, chords from `chord_name`, a greyed item's reason under it
  while hovered (Freya's tooltip hides under a context menu), the ✓ gutter
  (`metrics::MENU_TICK_GUTTER`, 18 px) while any item is ticked; holds the keyboard while open.
  The three context menus are built by it, items unchanged (a greyed reason now under the item
  on hover rather than a second row). Not virtualized (C21 is 05/08's).
- `cairn_ui::CaptionRow<T>` (`caption_row.rs`), `Frame` (`Drawn`/`Native`, `Frame::current`),
  `FrameRequest` (`Drag`, `ToggleMaximize`, `Minimize`, `Close`, `WindowMenu { x, y }`),
  `CAPTION_TITLE`. `cairn_ui::AboutDialog`, `ABOUT_TITLE`, `NO_GIT_FOUND`, `version_line`,
  `licence_line` (`dialogs/about.rs`).
- Accelerators: new actions `ShowLocalChanges` (Ctrl+1), `ShowAllCommits` (Ctrl+2), `ShowHead`
  (Ctrl+0), `ToggleDetails` (no chord until phase 03), `Fetch` (Ctrl+Shift+F), `ZoomIn` (Ctrl+=),
  `ZoomOut` (Ctrl+-), ⌘ on macOS; all `Scope::Window`. `ShowLostCommits` moved to
  `Scope::Window`; `Scope::History` and `HistoryList::on_action` removed. `accelerators::MenuKey`
  and `menu_key` (an open menu's keys, resolved in the table).
- `metrics`: `MENU_MIN_WIDTH`, `MENU_INSET`, `MENU_TICK_GUTTER`, `MENU_CHORD_GAP`,
  `MENU_REASON_WIDTH`, `MENU_BAR_PADDING`; `TITLE` removed (the title row's "Cairn" is the
  caption row's now).
- `cairn-app`: `src/frame.rs` (`FrameView { zoom, about, git }` on `View::frame`, `Zoom`,
  `ZOOM_STEPS`, `ask(FrameRequest)`, `close()` through `request_close_window`, `title()`,
  `use_title()`, `about()`), `src/menu_bar.rs` (`BarChoice { Act(Action), Exit, About }`,
  `menus(view, open)`, `choose`), `src/history/head.rs` (`head_commit`, `show`),
  `operations::fetch::{start, refusal, default_remote, NO_REPOSITORY, NO_REMOTE, FETCH_RUNNING}`;
  `Modal::About` last in the one modal state; `Update::GitFound { described }` sent first on a
  stream once git is found; the window's app id `cairn`; `with_decorations(false)` off macOS and
  `BorderlessPlugin`.
- `deny.toml`: freya `exact` now `["borderless", "default", "engine", "icons", "icons-lucide",
  "titlebar", "winit"]`; lockfile gains `freya-borderless-plugin` alone. `titlebar` is enabled
  on `cairn-ui` (where `TitlebarButton` is named), `borderless` on `cairn-app`.
- `docs/systems/window-frame.md`; evidence `docs/research/ui-review/fork-menu-toggles-2026-10-19.md`.
- `menu::Panel::on_dismiss`: Escape closes the whole menu in the panel and goes no further
  (context menu and bar); `accelerators::MenuKey::Unclaimed` (a modifier key, AltGraph
  included, left to the window).

**Phase 03:**
- `title_bar` renamed `toolbar` (`crates/cairn-ui/src/toolbar.rs` beside `toolbar/`); no guard named
  the old path.
- `cairn_ui::Toolbar` (`toolbar.rs`; buttons in `toolbar/button.rs`): `new(StatusBox)`,
  `counts(Option<AheadBehind>)` (Push's and Pull's badges), `fetch_refusal`, `branch_refusal`,
  `on_press(ToolbarPress)`, `on_status_sized((f32, f32))`. `ToolbarPress { ToggleSidebar, Fetch,
  NewBranch, Activity }`. Constants: `SIDEBAR_TOGGLE_NAME`, `QUICK_LAUNCH_CAPTION`, `FETCH_CAPTION`,
  `PULL_CAPTION`, `PUSH_CAPTION`, `STASH_CAPTION`, `BRANCH_CAPTION`, `APPEARANCE_CAPTION`,
  `STASH_MENU_NAME`, `BRANCH_MENU_NAME`, `ACTIVITY_GLYPH_NAME`, `COMES_WITH_QUICK_LAUNCH`,
  `COMES_WITH_PULL_AND_PUSH`, `COMES_WITH_PREFERENCES` (stash's is `COMES_WITH_STASHING`).
- `StatusBox`: the lines alone (the toolbar draws the box and the glyph); `dirty`, `name_text`
  removed; `progress_text` (git's line to its percentage, else to its first comma, else whole —
  the user's option A); `running_text` is the name and " · N waiting" only. `counts_text` moved to
  `sidebar.rs` (still exported).
- `HIDE_DETAILS_TOOLTIP` replaces `COLLAPSE_CAPTION`/`EXPAND_CAPTION`: the strip's icon.
- Icons: `quick_launch`, `fetch`, `pull`, `push`, `new_branch`, `appearance`, `chevron_down`,
  `activity`, `bottom_pane`. Metrics: `TOOLBAR_ICON`, `TOOLBAR_INSET`, `CONTROL_RADIUS`,
  `TOOLBAR_BUTTON_EDGE`, `TOOLBAR_BUTTON_MIN_WIDTH`, `TOOLBAR_BUTTON_PADDING`, `TOOLBAR_GROUP_GAP`,
  `TOOLBAR_CHEVRON`, `TOOLBAR_BADGE`, `TOOLBAR_BADGE_TEXT`, `TOOLBAR_BADGE_PADDING`,
  `STATUS_BOX_WIDTH`, `STATUS_BOX_HEIGHT`, `STATUS_BOX_MIN_WIDTH`, `STATUS_BOX_GLYPH`,
  `STATUS_BOX_GLYPH_INSET`, `STATUS_BOX_TEXT_INSET`, `STRIP_ICON`. Tones: `STATUS_BOX`, `BADGE`,
  `BADGE_TEXT`.
- Accelerators: `ToggleDetails` is Ctrl+Shift+D on Linux, none on macOS; Linux `Discard` is
  Backspace and Delete; the pin's Shift+D example.
- `cairn-app`: `src/toolbar.rs` (`toolbar(view, submit)`, `pressed`, `running`);
  `src/detail/split.rs` (`PaneSplit`: the pane's share of the measured room) replacing
  `View::pane_height`/`PANE_HEIGHT` with `View::pane_split`; `SidebarView::shown`;
  `frame::OPENING_SIZE` (1024×768) and `frame::MIN_SIZE` (880×620) on the window config; the
  title row and its loaded count (`status_text::loaded_count`) gone.
- Docs: `docs/systems/window-frame.md` "The toolbar", "The regions and the minimum size";
  `history-graph.md` "The status box"; `ui.md` row for the status box's one cut.

**Phase 04:**
- `cairn-model`: `RowsPage::mark_beyond_head`, `RowsPage::head_reaches`, `HistoryRow::head_reaches`
  (a flag of the kept row, `BEYOND_HEAD`). `LaneAssigner::push_stash` puts a stash on the lowest
  lane awaiting its base, else the lowest free lane.
- `cairn-git`: `src/history/head_reach.rs` (`HeadReach`), marked in every date-order walk from
  refs (`Decoration::walked_from_refs`, `Decoration::head`); `Entry::Commit { beyond_head }`,
  `Entry::Stash { at, beyond_head }`.
- `cairn-ui`: `LocalZone` (`src/local_time.rs`: `read`, `unreadable`, `Default` = UTC unsaid,
  `fell_back_to_utc`, `offset_at`; compared by read serial); `history_date`, `long_date(time,
  zone)`, `time_of_day`, `utc_fallback_note` (`history::date_text`, now public);
  `avatar`, `avatar_colour`, `initials` (`src/avatar.rs`); `ReflogStrip`, `REFLOG_STRIP_TEXT`,
  `REFLOG_STRIP_EXIT` (`history/reflog_strip.rs`); `CommitRow::head_reaches`, `CommitRow::zone`,
  `HistoryList::zone`, `RowRender::{head_reaches, zone}`, `CommitTab::zone`,
  `ChangesSummary::zone`, `summary_parts(details, zone)`; `HistoryHeader` and
  `SHOW_LOST_COMMITS_CAPTION` removed; `ChipKind::is_tracked` (a tracked branch's upstream a
  cloud-only chip of its own); `graph_geometry::RING_RADIUS`; `ROW_HEIGHT` = `metrics::HISTORY_ROW`.
  Metrics: `HISTORY_LANE` 12, `HISTORY_DOT` 5, `HISTORY_RING` 13, `HISTORY_LINE` 2, `CHIP` 17,
  `AVATAR` 14, `AVATAR_RADIUS` 3, `AVATAR_TEXT` 8, `REFLOG_STRIP` 22, `STRIP_INSET`, `STRIP_GAP`.
  Tones: `REMOTE_GLYPH` #777676, `AVATAR_TEXT`, `AVATARS` (eight, the first Fork's lavender),
  `upstream_chip_border(lane)`. Column widths: author 140, id 64, date 116.
- `cairn-app`: `src/worker/zone.rs` (`read`, `TimeZone::try_system`); `Update::GitFound`
  renamed `Update::Started { git, zone: Option<TimeZone> }`; `View::zone`;
  `ActivityLog::shown_in(zone)`.
- Guard: `the_render_layer_is_handed_its_time_zone` (matcher `reads_the_system_zone`, rosters
  `SYSTEM_ZONE_IDENTS`, `SYSTEM_ZONE_PATHS`, `SYSTEM_ZONE_TYPES`, `ZONE_READER`), its CLAUDE.md
  invariant. `jiff` in the allowlist rows of cairn-ui and cairn-app; `deny.toml` exact pin
  (gix-date's default set). Lockfile: two dependency-edge lines, no new crate.
- `scripts/git-floor.sh`: `--test lost_commits` floor 6.

**Phase 05:**
- `cairn-model`: `Ref::commit_time` (a tag's commit's committer date; `None` otherwise);
  `Disclosure`'s default closes Tags (`SidebarSection::opens_closed`); tags laid out newest first
  by it, ties by name last first.
- `cairn-git`: the refs query reads each tag's commit date through `Grafted`
  (`Reading::commit_time`), polled per tag, counted in `RefsCost::objects_read`.
- `cairn-ui`: `Sidebar::name`, `on_gear(GearChoice)`, `on_ref_menu((SidebarRow, RefChoice))`;
  `SidebarGlyph`, `DrawnRow::glyph: Option<SidebarGlyph>`, `row_indent`, `ref_menu`, `stash_text`,
  `SidebarTarget::is`; `sidebar/keys.rs` (`Walk`, `SidebarStep`, `walked_to`,
  `chosen_when_walked`); constants `COPY_REPOSITORY_PATH_CAPTION`, `REPOSITORY_GEAR_NAME`,
  `COPY_BRANCH_NAME_CAPTION`, `COPY_TAG_NAME_CAPTION`, `NO_COMMIT_TO_BRANCH_FROM`;
  `counts_text` now `1↑ 2↓`; `SIDEBAR_INDENT` = `metrics::SIDEBAR_INDENT` (12). `RefGlyph` lost
  `Branch`, `LocalOnly`, `Folder`, `Opened`, `Closed`. Icons `chevron_right`, `tag`, `check`,
  `local_branch`, `warning`, `local_changes`, `all_commits`. Metrics `SIDEBAR_ICON`,
  `SIDEBAR_CHEVRON`, `SIDEBAR_GAP`, `SIDEBAR_INSET`, `SIDEBAR_BAND_INSET`, `SIDEBAR_BAND_RADIUS`,
  `SIDEBAR_BADGE`; tones `SIDEBAR_ICON`, `SIDEBAR_DIM_ICON`, `FILTER_FIELD`, `UPSTREAM_GONE`,
  `UNSEEN`. The menu panel: the rule a fill-minimum child of the panel; a chord's room held by an
  unseen copy, the chord placed against the row's right edge.
- `cairn-app`: `Update::Opened { name, path }`, `View::repository_path`, `sidebar/menu.rs`
  (`gear`, `chosen`, `ref_name_text`, `repository_path_text`); `SIDEBAR_WIDTH` = 300.
- Evidence: `docs/research/ui-review/fork-sidebar-2026-10-20.md`.

**Phase 06:**
- `cairn-ui`: `committer_differs(author, committer)` (name, address or instant to the second);
  `COMMIT_TAB_ROW_HEIGHT` = `metrics::DIFF_ROW` (17), the Commit tab's one pitch —
  `DETAIL_ROW_HEIGHT` (24) stays the Changes list's and Local Changes'; the Commit tab's people
  block (`Line::People`, `BlockRow`, the avatar as three overlapping bands) and `ExpandAll` (a Tab
  stop); `AvatarSize { Row, Summary, Detail }` and `avatar_of(name, size)` (`avatar(name)` is
  `Row`); `SUMMARY_ID_LENGTH` (9); `bar_tooltip(HeaderAction, Os)`, `HeaderAction::{ALL, action,
  label}`, the bar's `BarButton` (Tab stop, hover square, toggled state for assistive
  technology); labels `PREVIOUS_CHANGE_LABEL` "Go to previous change", `NEXT_CHANGE_LABEL` "Go to
  next change", `SIDE_BY_SIDE_LABEL` "Side by side diff"; `DIFF_HEADER_HEIGHT` =
  `metrics::DIFF_BAR`, `SUMMARY_HEIGHT` = `metrics::DETAIL_SUMMARY`, `DETAIL_STRIP_HEIGHT` =
  `metrics::DETAIL_STRIP`. `diff/toggle_glyphs.rs` removed; `DiffSettings::can_show_fewer_lines`
  and `can_show_more_lines` removed.
- Icons: `chevron_up`, `whitespace`, `fewer_lines`, `more_lines`, `entire_file` (the three
  mirrored), `side_by_side`, `file`, `swap`, `stage_all`, `unstage_all`. Metrics and tones: the
  progress entry's batch item 6.
- `cairn-app`: `shortcuts::of_header` delegates to `HeaderAction::action`.
- Evidence: `docs/research/ui-review/fork-committer-2026-10-20.md`.

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 | done (C1, C3, C4 met; C2 met but for `borderless`/`freya-borderless-plugin`, phase 02's) | full gate PASS | qa-checklist, responsiveness, test-coverage, gate-integrity; qa-confirm: 12 confirmed fixed, 3 deferred, 3 dismissed |
| 02 | done (C5, C6, C20's About half met headlessly; C2's borderless arm met; pointer gestures and macOS UNVERIFIED, batched for the user) | full gate PASS | qa-checklist ×2, responsiveness ×2, test-coverage ×2, gate-integrity ×2; qa-confirm: 15 of 26 confirmed and fixed or recorded, 11 dismissed |
| 03 | done (C7, C8, C9, C18's Ctrl+Shift+D half met; option A by the user; pointer gestures and macOS UNVERIFIED) | full gate PASS | qa-checklist, responsiveness, test-coverage, gate-integrity; qa-confirm: 10 of 14 confirmed and fixed (Q2 probed and fixed), 4 dismissed |
| 04 | done (C10, C11, C12, C13 met; pointer gestures and macOS UNVERIFIED) | full gate PASS | qa-checklist, responsiveness, test-coverage, gate-integrity; qa-confirm: 11 of 18 confirmed and fixed, G4 escalated (recorded, batched), 6 dismissed; fixes verified by a fresh qa-confirm |
| 05 | done (C14 met but its "Return opens Local Changes or All Commits" clause, built as Fork's and batched; C21's sidebar half met; real window NOT RUN — host compositor socket gone, headless release render instead; macOS UNVERIFIED) | full gate PASS | qa-checklist, responsiveness, test-coverage; qa-confirm: 16 raw (two pairs merged), 12 confirmed (9 fixed, Q1/Q2/Q7 batched), Q5 and R3 dismissed, T1's mutation half dismissed on re-run; fixes verified by a fresh qa-confirm |
| 06 | done (C15 met; Q1 Tab stops and the 17 px pitch batched; real window NOT RUN — host compositor socket gone, headless release render instead; macOS UNVERIFIED) | full gate PASS | qa-checklist, responsiveness, test-coverage; qa-confirm: 20 raw, 11 confirmed (4 merged) and fixed or recorded, Q1 escalated (decided, batched), 4 dismissed |
| 07 | not started | — | — |
| 08 | not started | — | — |
| 09 | not started | — | — |
