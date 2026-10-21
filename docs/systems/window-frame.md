# The window's frame, its toolbar, its menus and About Cairn

How Cairn's window is framed today: Fork's caption row on Linux, the same menus as a row inside
the desktop's frame on macOS, the one menu component every menu is drawn by, the menu bar's
items and what each does, Fork's toolbar and the status box in it, the window's regions and its
minimum size, the desktop's title for the window, the zoom, and About Cairn. Intent:
`docs/design/ui.md`, "The chrome". Spec: `docs/prd/fork-alignment.md` R2, R3, R4, R9.1, R11.2.

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
decorations`). On Isle the same request is answered `configure(client_side)` and the window is
drawn as Cairn draws it. Seen on screen on each, the caption row the top of the window; no
window rule is needed. The desktop's title for the window reads `<repository> — Cairn` and its
app id `cairn`.

Driven by a virtual pointer on Isle (release build, the fixture): a press and drag on the
caption row's empty space moves the window by the drag; a double press there maximizes it,
the maximize button then drawing its restore glyph, and a second double press restores the box
it had; a secondary press opens Isle's window menu at the press (`show_window_menu`), which a
press elsewhere in the window dismisses; a drag on the right edge resizes the window, and Isle
stops that drag at the minimum width. The menus open by a press and switch as the pointer moves
along the bar. Hyprland's gestures were never driven by a pointer. What no headless test sees
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
highlighted, handing focus back to what held it as the menu opened; `menu::context_menu_over`
names that view itself, for a list that asked for focus in the same press that opens its menu.
Every context menu is built by them: a commit row's (`history::row_menu`), a commit's file's in
the Commit and Changes tabs (`detail::file_menu`), a ref's and the gear's in the sidebar, Local
Changes' (`local_changes::menu`), the commit box's Recent Commit Messages and the bar's View
Mode. Each row's items are Fork's that work, in Fork's groups and beside Fork's chords
(`docs/systems/sidebar.md`, `local-changes.md`, `history-graph.md`; pinned whole as data by
`every_context_menu_holds_its_rows_working_items`, `crates/cairn-ui/tests/context_menus.rs`).

**The copy chords.** Copy (Ctrl+C, ⌘C) and Copy Full (Ctrl+Shift+C, ⇧⌘C) are heard by a focused
list with a context menu alone (`Scope::List`): the history, the sidebar, the Changes tab's files
and Local Changes' lists. The list builds its menu for what it holds selected and chooses the
item showing the chord (`menu::chosen_by`), so the chord a menu shows and the key are one source;
an item that shows neither, or a list with nothing selected, copies nothing. No text field is in
that scope, so a field's Ctrl+C is the editor's own copy (`field_key`). What is copied is Fork's
(`docs/research/ui-review/fork-copy-and-dialogs-2026-10-20.md`): a commit's full id, or its seven-
character id, " - " and its subject (`commit_sha_text`, `commit_info_text`), each selected commit
one line, the upper row first; a ref's name past its namespace; a path as git writes it, or that
path joined to the repository's root (`crate::clipboard::full_path`), one per line in the lists'
order. The window writes the clipboard in one place, `crates/cairn-app/src/clipboard.rs`. Pinned by
`copy_and_copy_full_copy_what_the_focused_view_names` (the window), `the_copy_chord_copies_the_walked_refs_name`,
`a_files_menu_and_the_copy_chords_choose_its_copies` and the table's tests.

**A long menu** — more than `LONG_MENU_LINES` lines, items and rules — stands that many rows
tall at a fixed width (`metrics::MENU_LONG_WIDTH`) and draws its rows through a virtualizing view
of one row height, each group's rule along the top of its first row; ↑ and ↓ scroll the item
reached into view, and an open submenu or a greyed item's reason is drawn beside the view at its
row. Every menu's items are laid out once, as its panel is made, and read by index
(`a_long_menu_builds_one_viewport`). No menu today is that long.

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
the item alone on macOS); Show HEAD selects `HEAD`'s row and brings it into view as a press of
its ref does, paging the walk for it when it is not loaded (`history::head`,
`show_head_selects_heads_row`); Show Lost Commits reopens the history; Refresh asks the worker; Fetch fetches
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

## The toolbar

Under the caption row, Fork's toolbar, 41 px in the chrome's tone (`cairn_ui::Toolbar`,
`crates/cairn-ui/src/toolbar.rs`, its buttons `toolbar/button.rs`; built from the window's state
by `crate::toolbar::toolbar`). Fork's buttons in Fork's order and groups
(`fork-window-survey-2026-10-16.md`, sections 1 and 2), each a Lucide icon over its label in the
toolbar's tone, Fork's light box behind one under the pointer:

| Button | What it does |
| --- | --- |
| sidebar toggle (icon alone) | hides the sidebar, the main region taking the width, and shows it again at its width (`SidebarView::shown`) |
| Quick Launch | greyed: "Comes with Quick Launch." |
| Fetch | fetches the default remote, as the Repository menu's Fetch and its chord do (`shortcuts::act`), greyed with the same reason while it cannot (no repository, no remote, a fetch running); its tooltip "Fetch" over its chord, read from the accelerator table |
| Pull, Push | greyed: "Comes with pull and push."; the commits behind the upstream a badge on Pull, those ahead on Push, each absent at zero |
| Stash and its ⌄ | greyed: "Comes with stashing." |
| the status box | below |
| Branch and its ⌄ | Create Branch at `HEAD`, as New Branch… and its chord do; its ⌄ a menu of "New Branch…" alone, with its chord (New Worktree… comes with worktrees); both greyed while `HEAD` names no commit, with the menu bar's reason; no tooltip, as Fork's |
| Appearance and its ⌄ | greyed: "Comes with Preferences." |

Fork's Open in, Console and Work are not drawn: no packet plans them. A greyed button keeps its
place, draws in the disabled tone and reports nothing to a press or a key — none takes focus.
Every press is a `ToolbarPress` the window does (`crate::toolbar::pressed`). Pinned by
`the_toolbar_is_forks_with_the_unbuilt_buttons_greyed`,
`fetch_and_branch_are_greyed_with_their_reasons`, `ahead_and_behind_are_badges_on_push_and_pull`
(`crates/cairn-ui/tests/toolbar.rs`), `fetch_is_greyed_while_a_fetch_is_in_flight`,
`each_refresh_redraws_the_badges_on_push_and_pull` and
`the_sidebar_toggle_hides_and_shows_the_sidebar` (`crates/cairn-app/src/window.rs`).

The status box sits in the room between the groups with Branch beside it: Fork's 300 × 36 px box
in its tone, the repository's name bold over the branch (`docs/systems/history-graph.md`, "The
status box"), its list glyph at the left the one place that opens the activity popover — a press
on its centre opens nothing, as Fork's (`docs/systems/git-processes.md`, "The activity popover").
While an operation runs it shows it in the repository's place: a spinner and its name over git's
progress line up to its percentage ("Where an ending goes", there). It may shrink to 200 px as the
window narrows, but the window's minimum keeps it at 300.

## The regions and the minimum size

Under the toolbar stand the banners — the leftover lock's and a close waiting on a write — across
the whole window, above the sidebar and the main region, as Fork's notification banner does
(`the_banners_stand_under_the_toolbar`). The history and the detail pane share the main region's
height half each, growing together (`docs/systems/diff.md`, "Where it sits";
`the_detail_pane_grows_with_the_window`), and Hide Commit Details — the View item, Ctrl+Shift+D
on Linux, the strip's icon — leaves the pane its strip (`hiding_the_details_leaves_the_tab_strip`).
As the window narrows the history's subject column gives the room, the author, id and date
keeping their widths (`the_subject_column_shrinks_first`).

The window opens at 1024 × 768 and stops at **880 × 620** (`frame::OPENING_SIZE`,
`frame::MIN_SIZE`, `WindowConfig::with_min_size`, which reaches Wayland as
`xdg_toplevel.set_min_size`), at the zoom's own size. Hyprland holds the window at 880 × 620
when asked for less, and Isle stops an edge drag there; Isle's own `isle windows set-box`
ignores the minimum and configures a smaller box, which the window takes and clips. Measured in
the headless window and checked in the real one (release build, the fixture):

- the toolbar's groups take about 550 px at the body's size and the status box its 300, so the
  toolbar needs about 780 px; 880 leaves it whole with Fork's 300 px box;
- at 880 px, beside the sidebar's 300, the history keeps about 180 px for a row's chips and
  subject, cut with "…" where a row's chips take most of it, as Fork's subjects are cut;
- at 620 px the main region's half is 276 px, enough for the pane's strip and a one-line commit's
  Commit tab down to Expand All, which the pane's floor then keeps when a banner takes room;
- the diff bar's seven buttons stay inside the window.

Pinned by `the_window_keeps_every_region_whole_at_its_minimum_size` and
`expand_all_is_visible_at_the_minimum_size_and_at_1024_by_1200`. Fork clips below about 600 px
instead (`fork-window-survey-2026-10-16.md`, section 11); Cairn stops the window first. Zoomed in,
the window's own pixels hold less: a zoom above 1 at the minimum size can clip, which the window
does not prevent.

## The title, the zoom and About Cairn

The desktop's title for the window — its task bar and window switcher — is `<repository> —
Cairn` once a repository is open, "Cairn" before (`frame::title`, set by `frame::use_title`;
`the_window_title_names_the_repository`).

The zoom scales the whole window through Freya's custom scale factor
(`Platform::set_custom_scale_factor`), never the `zoom-shortcuts` feature, through the steps of
`frame::ZOOM_STEPS` — 0.5 to 2, the window's size at 1 — kept for the session
(`zoom_scales_the_window_in_steps`, `the_zoom_steps_through_its_table_and_stops_at_each_end`).

About Cairn (`cairn_ui::AboutDialog`) names Cairn, this build's version and licence and the `git`
found as the application started — the worker's first word on a stream, `Update::Started`,
"git <version>, at <path>" — or that none has been found; Close, focused as it opens, and
Escape close it. It is the last of the one modal state (`crate::modal`), so while it is open
the window's chords do nothing (`about_cairn_names_the_version_and_the_git`). Fork's About box
has only a ✕ and Escape leaves it open; Cairn's closes as every one of its dialogs does.
