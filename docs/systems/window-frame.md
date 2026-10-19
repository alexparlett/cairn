# The window's frame, its menus and About Cairn

How Cairn's window is framed today: Fork's caption row on Linux, the same menus as a row inside
the desktop's frame on macOS, the one menu component every menu is drawn by, the menu bar's
items and what each does, the desktop's title for the window, the zoom, and About Cairn. Intent:
`docs/design/ui.md`, "The chrome". Spec: `docs/prd/fork-alignment.md` R2, R9.1, R11.2.

## The frame

`cairn_ui::Frame` says who draws it: `Frame::Drawn` everywhere but macOS, `Frame::Native` there
(`Frame::current`). The window is built with the desktop's decorations off where Cairn draws
the frame (`WindowConfig::with_decorations`, `crates/cairn-app/src/main.rs`), and Freya's
`BorderlessPlugin` (the `borderless` feature, crate `freya-borderless-plugin`, pinned in
`deny.toml`) lays invisible bands along every edge and corner that ask the compositor for a
resize (`drag_resize_window`), hidden while the window is maximized. The window's Wayland app id is `cairn`, the
same name as its X11 class.

**The caption row** (`cairn_ui::CaptionRow`, `crates/cairn-ui/src/caption_row.rs`) is one row of
`metrics::CAPTION_ROW` (26 px) in the chrome's tone: the menu bar at its left, "Cairn"
(`CAPTION_TITLE`) centred on the window, and Freya's three `TitlebarButton`s (the `titlebar`
feature) at its right — minimize, maximize or restore as `Platform::is_maximized` says, and
close. Its empty space, and the title, which lies under the menus and the controls, behave as a
frame's title bar does on Fork for Windows
(`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`, question 5): a press asks the window
to follow the pointer, a double press (Freya's `EventsCombos`) to maximize or restore, a
secondary press for the compositor's window menu where it landed. A menu's name and a control
stop the press there, so neither drags the window. The row reaches no window itself: each
gesture is a `FrameRequest`, which the window does (`crate::frame::ask`: winit's `drag_window`,
`set_maximized`, `set_minimized`, `show_window_menu`; close through `request_close_window`, so
the close hook closes the repository first, `crate::closing`). Pinned by
`the_caption_row_draws_the_menus_the_title_and_the_controls` and
`a_drag_on_the_caption_row_moves_the_window` (`crates/cairn-ui/tests/caption_row.rs`).

With `Frame::Native` the row holds the menu bar alone — no title, no controls, no drag — and is
built from the same menus (the same test, its second half). No native menu bar is attached.

**Compositors.** With the desktop's decorations off, winit asks for the frame through the
standard protocols, before the window is first shown: on Wayland it binds
`zxdg_decoration_manager_v1` and sends `zxdg_toplevel_decoration_v1.set_mode(client_side)`
(winit's `WindowDecorations::RequestClient`, `platform_impl/linux/wayland/window/mod.rs`); on
X11 it sets `_MOTIF_WM_HINTS` with no decorations. On Hyprland 0.56 (the development host)
`WAYLAND_DEBUG=1` shows that request and the compositor's `configure(server_side)` in reply,
yet nothing of the compositor's own is drawn on the window: its decoration plugin, `hyprbars`,
draws no bar on a window whose most recent request was client-side, nor on an X11 window whose
Motif hints ask for none (it still lists its decoration as present, hidden, in `hyprctl
decorations`). Seen on screen on both, the caption row the top of the window; no window rule is
needed. The desktop's title for the window reads `<repository> — Cairn` and its class `cairn`
on both.

A press and drag, a double press, the window menu and the resize bands were not driven by a
pointer on either (no tool on the host moves the pointer into a Wayland window), nor were the
menus opened by the pointer or the pointer's move along the bar. What no headless test sees
either: `freya-testing` drops the requests the window makes of the platform, so what
`frame::ask` hands winit (`drag_window`, `set_maximized`, `set_minimized`, `show_window_menu`,
`request_close_window`), the scale factor the zoom sets, the title `frame::use_title` sets and
the decorations and resize bands `main.rs` asks for are pinned only up to the request — the
`FrameRequest`, the zoom's step, `frame::title` — and the rest was checked in the window as
said here, or not at all. What a compositor does with an ask is its own: a tiling compositor
may decline a move, a resize or a maximize.

**macOS** is not compiled on the development host (no Apple target is installed), so the
`Frame::Native` arm is checked only headlessly, with the frame forced, and the `cfg` paths and
⌘ chords are unverified on a Mac.

## The menu component

Every menu is drawn from data by one component (`cairn_ui::menu`, `crates/cairn-ui/src/menu.rs`
beside `menu/`): a menu is groups of `Item`s — a label, the `Action` whose chord it shows, the
reason it is greyed (a greyed item always has one), whether it is ticked, and what choosing it
does (`ItemKind::Choose` with a value, or `ItemKind::Submenu`). `menu::panel` draws it: a rule
between groups, each row `metrics::MENU_ITEM` high, its chord right-aligned in
`tones::MENU_CHORD`, read from the accelerator table (`accelerators::chord_name`) and never typed,
an action with no chord showing none; a greyed row in the disabled tone, its reason drawn under
it in the tooltip's look while the pointer is on it (Freya's own tooltip hides while a context
menu is open); and, while any item is ticked, a `metrics::MENU_TICK_GUTTER` (18 px) gutter before
every label with a ✓ in the ticked one's, Fork's View menu's look. One row is highlighted — by the
pointer or by the keys, one highlight for both — in Fork Dark's hover band with white text.

Its keys (`accelerators::menu_key`, resolved in the table, so the panel names no modifier): ↑ and
↓ walk the items that can be chosen, skipping greyed ones and wrapping; Return chooses; → opens
a submenu with its first item highlighted, or the next menu of the bar; ← closes a submenu, or
opens the previous menu; Escape closes. A submenu also opens when the pointer is on its row. The
panel takes focus as it opens and holds the keyboard while it is open: it stops every key but a
modifier key — which the window still hears, so its held keys stay right — so no chord of the
window or a list acts behind an open menu. A choice and Escape hand focus back to what held it
as the menu opened. Pinned by `a_menu_is_drawn_from_its_data`,
`the_menus_walk_by_key_and_hover` (`crates/cairn-ui/tests/menu.rs`) and
`a_menu_hears_its_keys_bare_and_leaves_the_modifiers_to_the_window` (the table).

`menu::context_menu` builds a context menu from groups for Freya's `ContextMenu`, its first item
highlighted. The three context menus are built by it: a commit row's "New Branch…"
(`history::list`), Local Changes' (`local_changes::menu`) and the commit box's Recent Commit
Messages (`commit_box`). A menu is not virtualized: none today holds more than ten items.

## The menu bar

`cairn_ui::menu::MenuBar` draws each menu's name and the open menu's panel under it, as Fork's
does (`fork-menu-toggles-2026-10-19.md`, question 3): a press on a name opens its menu with
nothing highlighted until ↓ is pressed, a press on the open menu's name closes it; while one is
open the pointer over another name opens that one, and → and ← move to the menu beside it,
wrapping; a press outside, Escape, a choice or the window losing focus closes it; a name under
the pointer, or whose menu is open, sits on Fork Dark's hover box. Pinned by
`the_menu_bar_opens_moves_and_closes_as_forks`.

Its menus are built from the window's state on each render (`crate::menu_bar::menus`), Fork's in
its order and groups (`fork-window-survey-2026-10-16.md`, the menu bar) with only what Cairn does:

| Menu | Items |
| --- | --- |
| File | Exit |
| View | Show Uncommitted Changes · Show All Commits — Hide Commit Details — Show HEAD — Show Lost Commits (Reflog) |
| Repository | Refresh — Fetch '<remote>' — New Branch… |
| Window | Zoom In · Zoom Out |
| Help | About Cairn |

Every item but Exit and About Cairn is an action of the accelerator table, chosen through
`shortcuts::act` exactly as its chord is, so a menu item and its key are one thing: Show
Uncommitted Changes and Show All Commits switch the main region; Hide Commit Details collapses
the detail pane to its tab strip and opens it again (Ctrl+Shift+D on Linux, Fork for Windows';
the item alone on macOS); Show HEAD selects `HEAD`'s row as a press of its ref does
(`history::head`); Show Lost Commits reopens the history; Refresh asks the worker; Fetch fetches
the default remote, as the toolbar's button does (`operations::fetch::start`); New Branch…
opens Create Branch at `HEAD`; Zoom In and Out step the zoom. Hide Commit Details and Show Lost
Commits are ticked while on, their labels unchanged
(`fork-menu-toggles-2026-10-19.md`, questions 1 and 2). An item that cannot act is greyed with
its reason: those a repository answers while none is open, Show HEAD and New Branch… until the
refs are read or while `HEAD` is unborn, Fetch with no remote or while one runs, a zoom at its
last step. Exit closes the window through the close hook. While a dialog or the activity
popover is up (the one modal state), a chosen action does nothing, as its chord does. Pinned by
`the_menu_bar_holds_only_what_works_in_forks_order`, `each_menu_item_does_its_action` and
`the_window_draws_the_menu_bar_and_its_choices_act` (`crates/cairn-app/src/menu_bar/tests.rs`).

The new actions' chords, Linux first (`accelerators::chords`): Show Uncommitted Changes Ctrl+1,
Show All Commits Ctrl+2, Show HEAD Ctrl+0, Fetch Ctrl+Shift+F, Zoom In Ctrl+= and Zoom Out Ctrl+-,
each digit and mark matched where it sits; ⌘ for Ctrl on macOS, not yet checked against Fork for
macOS. Show Lost Commits' Ctrl+Shift+. is heard wherever focus is.

## The title, the zoom and About Cairn

The desktop's title for the window — its task bar and window switcher — is `<repository> —
Cairn` once a repository is open, "Cairn" before (`frame::title`, set by `frame::use_title`;
`the_window_title_names_the_repository`).

The zoom scales the whole window through Freya's custom scale factor
(`Platform::set_custom_scale_factor`), never the `zoom-shortcuts` feature, through the steps of
`frame::ZOOM_STEPS` — 0.5 to 2, the window's size at 1 — kept for the session
(`zoom_scales_the_window_in_steps`, `the_zoom_steps_through_its_table_and_stops_at_each_end`).

About Cairn (`cairn_ui::AboutDialog`) names Cairn, this build's version and licence and the `git`
found as the application started — the worker's first word on a stream, `Update::GitFound`,
"git <version>, at <path>" — or that none has been found; Close, focused as it opens, and
Escape close it. It is the last of the one modal state (`crate::modal`), so while it is open
the window's chords do nothing (`about_cairn_names_the_version_and_the_git`). Fork's About box
has only a ✕ and Escape leaves it open; Cairn's closes as every one of its dialogs does.
