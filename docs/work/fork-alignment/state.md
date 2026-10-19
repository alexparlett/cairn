# State — fork-alignment

The cross-session cheat sheet. Every session updates it before ending.

**Status: in progress, 2026-10-19.** `history-topo-order` (roadmap 5c) merged as #142; the
integration branch `feature/fork-alignment` was cut from it. Phases 01 and 02 done (packet mode).

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

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 | done (C1, C3, C4 met; C2 met but for `borderless`/`freya-borderless-plugin`, phase 02's) | full gate PASS | qa-checklist, responsiveness, test-coverage, gate-integrity; qa-confirm: 12 confirmed fixed, 3 deferred, 3 dismissed |
| 02 | done (C5, C6, C20's About half met headlessly; C2's borderless arm met; pointer gestures and macOS UNVERIFIED, batched for the user) | full gate PASS | qa-checklist ×2, responsiveness ×2, test-coverage ×2, gate-integrity ×2; qa-confirm: 15 of 26 confirmed and fixed or recorded, 11 dismissed |
| 03 | not started | — | — |
| 04 | not started | — | — |
| 05 | not started | — | — |
| 06 | not started | — | — |
| 07 | not started | — | — |
| 08 | not started | — | — |
| 09 | not started | — | — |
