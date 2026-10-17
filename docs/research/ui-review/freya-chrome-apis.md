# Freya chrome APIs for Fork-style window chrome (2026-10-16)

Research, read-only, for the UI-review packet: what the Freya that links offers for a toolbar, a
menu bar, context menus, tooltips, repository tabs, a Preferences window and denser sizing, and what
Cairn uses today. Every claim is cited to the vendored source.

## Where the source is

- The Freya that links is the git-pinned fork: `Cargo.toml:20` pins
  `freya = { git = "https://github.com/alexparlett/freya", rev = "caa46f8715744cb4fdb2d36304e2b1bbeb4acdb0" }`
  (and `freya-testing` at the same rev, `Cargo.toml:22`); `Cargo.lock` resolves every `freya-*`
  crate to that rev. Version `0.5.0-rc.4` (`crates/freya/Cargo.toml`).
- Source: `~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/`. Below, **`F/`** abbreviates
  `~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/crates/`. Two other checkouts of the same
  repository sit beside it (`a8d7a03`, `dd26d17`) and are not the one that links.
- winit is `0.30.13` (`F/../Cargo.toml:152`, Cargo.lock), read at
  `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.13/` (**`W/`** below).
- Features Cairn enables today: `freya` default (`winit`) in `cairn-app`, plus `engine` in
  `cairn-ui` (`crates/cairn-ui/Cargo.toml:16`). Not enabled: `titlebar`, `borderless`, `menu`,
  `icons`, `icons-lucide`, `tray`, `zoom-shortcuts` (feature list `F/freya/Cargo.toml`, "Features").

## 1. Menu bar

**Native menu bar: macOS only. NOT AVAILABLE on Linux.**

- `LaunchConfig::with_menu(menu: impl FnOnce() -> muda::Menu, menu_handler: impl FnMut(muda::MenuEvent, RendererContext))`,
  `F/freya-winit/src/config.rs:429`, behind the `menu` feature (`F/freya/Cargo.toml`:
  `menu = ["winit", "freya-winit/menu"]`; `F/freya-winit/Cargo.toml`: `menu = ["dep:muda"]`, muda
  `0.17`). The menu vocabulary is re-exported as `freya::menu` (`F/freya/src/lib.rs`, `pub mod menu`;
  `F/freya-winit/src/lib.rs:54-57`). Example: `F/../examples/feature_menu/src/main.rs` (`Submenu`,
  `MenuItem::with_id(.., "CmdOrCtrl+Q".parse().ok())`, `PredefinedMenuItem::separator()`).
- Attached only on macOS: `F/freya-winit/src/renderer.rs:441-446` builds the menu on `resumed` and
  calls `menu.init_for_nsapp()` under `#[cfg(target_os = "macos")]` alone; the doc says so outright:
  "on macOS it is then installed with `init_for_nsapp` (other platforms currently build it without
  attaching — attach support per platform can come later)" (`F/freya-winit/src/config.rs:416-419`).
  muda's Linux menubar attaches to a GTK window (`init_for_gtk_window`, per muda's own API; muda is
  not in Cairn's `~/.cargo/registry` because the feature is off, so this is not read from vendored
  source), and a winit window is not one. What it would take on Linux: a GTK host window and loop,
  which neither winit nor this fork has.
- Cost even on macOS: muda pulls `gtk` and `libxdo` on Linux at build time (`muda 0.17.2`'s
  dependency list in `F/../Cargo.lock`), new system-library build requirements, and a new
  dependency decision. Accelerators on a muda item are taken by the OS before the window sees the
  key (`F/freya-winit/src/config.rs:425-427`), so they would need routing to the same `Action`.
- Muda menu events reach `RendererContext`, not a component scope; Quit should use
  `NativeEventExt::request_close_window` so `with_on_close` keeps its veto
  (`F/freya-winit/src/config.rs:420-424`, `F/freya-winit/src/renderer.rs:199`).

**Drawn menu bar: no `MenuBar` component. NOT AVAILABLE as a component; buildable from parts.**

- There is no `MenuBar`/`Dropdown` type in `F/freya-components/src/lib.rs:3-54`. The parts exist:
  `Menu` (a floating panel, `F/freya-components/src/menu.rs:106`), `SubMenu` (`menu.rs:678`),
  `MenuButton` (`menu.rs:578`), `Attached` to hang a panel under a trigger
  (`F/freya-components/src/attached.rs:61`, `AttachedPosition::Bottom` `:18`, `AttachedAlign::Start`
  `:32`), and `Button::flat()` for the bar's captions (`F/freya-components/src/button.rs`).
- How a `Menu` opens: the caller mounts it (`.maybe_child(open.then(|| Menu::new()...))`, the doc
  example `menu.rs:59-79`) or hands it to `ContextMenu::open` (section 2). It draws on
  `Layer::Overlay` (`menu.rs:201`).
- Nesting: `SubMenu::new().label(..).child(..)` (`menu.rs:693-700`) opens its flyout on pointer
  enter and on press (`menu.rs:763-771`), positioned right of its row (`menu.rs:779`), with a stack
  of open menus (`close_menus_until`, `push_menu`, `menu.rs:830-838`). Menus of any depth.
- Closing: Escape pops the deepest open submenu first, then calls `on_escape` (falling back to
  `on_close`) and `prevent_default`s the key (`menu.rs:185-197`). An outside click closes it, armed
  on the first pointer-down after mount so the opening click does not count, and tested against the
  menu's own measured area (`menu.rs:175-224`). A `Menu` overflowing the window is shifted back in
  (`overflow_offset`, `menu.rs:813`).
- Shortcut text: **no slot**. A `MenuButton` takes any children (`ChildrenExt`, `menu.rs:600`), so a
  row is a horizontal `rect` of caption, flexible gap and the chord; `Menu::min_width` /
  `SubMenu::min_width` exist exactly so a trailing hint has a width to push against
  (`menu.rs:152-156`, `:709-717`). Cairn already renders a chord for a person from the table
  (`crates/cairn-ui/src/accelerators/chord_names.rs`, `accelerators::chord_name`).
- What a Fork-style bar needs beyond the parts: hover-to-switch between top-level menus while one is
  open, and Alt/F10 access, are application state Freya does not provide.

## 2. Context menus

**What Cairn uses today.** Freya's `ContextMenu` global, hosted once at the window root:
`crates/cairn-app/src/window.rs:270` (`.child(ContextMenuViewer::new())`). Four menus:

- History row: `New Branch…` alone, `ContextMenu::open_from_down(Menu::new().child(MenuButton..))`
  (`crates/cairn-ui/src/history/list.rs:439-449`).
- Local Changes rows: `menu(list, refusal, chosen)` in `crates/cairn-ui/src/local_changes/menu.rs:188-229`
  (Stage/Unstage, Discard Changes…, Stage All/Unstage All, Copy Path), disabled items via
  `MenuButton::new().enabled(false)` with the refusal's reason as a second disabled row
  (`menu.rs:211-222`); opened at `crates/cairn-ui/src/local_changes/list.rs:747`.
- The commit box's Recent Commit Messages, a dropdown built as a context menu at the pointer:
  `ContextMenu::open(menu)` (`crates/cairn-ui/src/commit_box.rs:528-546`).
- A window test opens one (`crates/cairn-app/src/window.rs:4612`).

Each item calls `ContextMenu::close()` itself before its action.

**What the fork offers** (`F/freya-components/src/context_menu.rs`):

- `ContextMenu::open(menu)` (`:66`), `open_from_down(menu)` for a pointer-down opener so its own
  release does not close it (`:71`, the `ClosePhase` dance `:13-22`, `:146-154`), `close()` (`:81`),
  `is_open()` (`:61`). `ContextMenuViewer` tracks the pointer globally and draws the menu at it on
  `Layer::Overlay` (`:142-174`); closes on Escape (`:170-172`), on an outside press once allowed
  (`:165-169`), and when the app loses focus (`:136-140`). Edge-of-window correction is
  `MenuContainer`'s self-measure (`menu.rs:307-341`).
- Nesting: `SubMenu` inside the context `Menu`, as in section 1.
- Separators: **NOT AVAILABLE** as an item type (no separator in `menu.rs`). `Menu` takes any
  `Element` children (`menu.rs:116-120`), so a 1 px `rect` with a theme colour and vertical margin is
  a separator; it is not focusable or pressable. (muda's `PredefinedMenuItem::separator` is the
  native bar's only.)
- Disabled items: `MenuItem::enabled(false)` / `MenuButton::enabled(false)` — dimmed by
  `opacity(0.5)`, no hover, no press, not focusable (`menu.rs:443-448`, `:548-556`, `:632-637`).
- Selected item: `MenuButton::selected(bool)` (`menu.rs:627-630`).
- Keyboard navigation: partial. Items are accessibility nodes in a `MenuGroup` (`menu.rs:305`,
  `:536-539`); the root integration moves focus inside a group on bare ↑/↓ and between groups on
  Tab / Shift+Tab (`F/freya-components/src/integration.rs:12-32`), and a focused item fires its
  press on the OS activation key (`F/freya-core/src/elements/extensions.rs:340-368`). A focused item
  draws a 2 px focus border (`menu.rs:502-512`). **Not provided:** focusing the first item when a
  menu opens, → to open a submenu, ← to close one, type-ahead, mnemonics. Note the ↑/↓ handler is a
  global key handler at the root, so it runs beside Cairn's own window-level arrows
  (`crates/cairn-app/src/window.rs:197-204`) while a menu is open.
- Defaults to override for density: `MenuItem` padding `(8, 14)` and `min_width(105)`
  (`menu.rs:405`, `:540`, override per item with `MenuItem::padding`, `:451`; `MenuButton` has no
  padding setter, so a dense menu builds `MenuItem`s directly, as `SubMenu` and `MenuButton`
  themselves do); `menu_item` corner radius 10 and `menu_container` padding 4 / radius 8
  (`F/freya-components/src/theming/themes.rs:599-621`). A menu has no font size of its own: it
  inherits from where `ContextMenuViewer` is mounted (doc, `context_menu.rs:88-104`) — Cairn's root
  sets none, so menus draw at Freya's 16 px default (section 9).

## 3. Tooltips

**Available**: `Tooltip` + `TooltipContainer` (`F/freya-components/src/tooltip.rs`).

- `TooltipContainer::new(Tooltip::new_text("..")).child(trigger)` (`:262`, `:141`); `Tooltip::new()`
  takes arbitrary children (`:130`, `:123-127`).
- Hover delay: `TooltipContainer::delay(Duration)`, default **500 ms** (`:268`, `:278-283`), an
  `async_io::Timer` task started on pointer over and cancelled on out (`:314-323`). Fade and scale in
  over 150 ms (`:291-309`).
- Placement: `TooltipContainer::position(AttachedPosition)` — `Top`, `Bottom` (default), `Left`,
  `Right` (`:266`, `:273-276`); flips to the opposite side when the preferred one lacks room
  (`:343-371`); cross axis clamped by `Attached`. Long text wraps at 340 px (`:163`).
- Dismissed on pointer-down on the trigger (`:325-333`, `:386`) and hidden while a context menu is
  open (`:335`).
- Theme: `tooltip` — colours, `font_family` (empty inherits), `font_size` 14, `font_weight` 400
  (`tooltip.rs:37-49`, `themes.rs:380-390`); padding `(4, 10)` and radius 8 are hard-coded
  (`tooltip.rs:192`, `:200`).
- Cairn uses it today: `crates/cairn-ui/src/detail/changes_list.rs:498`,
  `crates/cairn-ui/src/history/commit_row.rs:305` (a chord as the tooltip),
  `crates/cairn-ui/src/diff/header.rs:182`, `:217`, `crates/cairn-ui/src/local_changes/list.rs:526`.

## 4. Icons

**SVG: available, through a component, not an element.** freya-core's elements are `rect`,
`label`, `paragraph`, `image` (`F/freya-core/src/elements/`); there is no `svg` element.

- `SvgViewer::new(source)` (`F/freya-components/src/svg_viewer.rs:127`, `:147`), source an
  `ImageSource` — `(id, &'static [u8])`, `(id, Bytes)`, `&'static [u8]`, a path
  (`F/freya-components/src/image_viewer.rs:95-125`). So bytes at build time: `include_bytes!` or an
  inline string literal, as `TitlebarButton` does with its four glyphs
  (`F/freya-components/src/titlebar.rs:93-113`).
- Rasterized with Skia's SVG DOM at the laid-out size (`svg_viewer.rs:62-88`), cached in the asset
  cache; `currentColor` set by `.color(..)` (or inherited text colour), plus `.fill`, `.stroke`,
  `.stroke_width` overrides (`svg_viewer.rs:185-206`); `.show_loader(false)` to avoid a spinner flash
  (`:172`); `.parallel(true)` to rasterize off-thread (`:178`).
- Icon set: `freya-icons` with `lucide` (`F/freya-icons/src/lib.rs:23-36`; one `fn name() -> Bytes`
  per SVG, generated by `F/freya-icons/build.rs` from `F/freya-icons/external/lucide/icons/`, 1,640
  files including `git-branch`, `git-commit-*`, `git-compare`, `git-fork`, `archive`, `download`,
  `folder-git`), enabled by freya's `icons-lucide` feature. Lucide is **ISC** licensed
  (`F/freya-icons/external/lucide/LICENSE`), not freya's MIT.
- Icon fonts: none shipped. `F/freya-icons/external/lucide/lucide-font` is in the submodule but not
  compiled into anything.
- Vector drawing: `canvas(RenderCallback::new(|ctx| ..))` (`F/freya-components/src/canvas.rs:227`,
  `:37`) gives a Skia canvas.

**How Cairn draws glyphs today**: as paths on a canvas, never a character or asset.
`crates/cairn-ui/src/history/ref_glyphs.rs` paints eleven ref glyphs with `PathBuilder`, circles
and strokes on a `canvas` (`:70-77`, `:80`; rationale `:10-16`: no font needed, shapes distinct,
keyed because a canvas repaints only on layout change). `crates/cairn-ui/src/diff/toggle_glyphs.rs`
builds the diff bar's glyphs from rectangles, borders and one rotation, and one Plex Mono character
`↕` (`:4-11`, `:54-60`). The graph is a canvas too (`crates/cairn-ui/src/history/graph_cell.rs:22`).
The commit box's Recent Messages control is the character `≡` (`crates/cairn-ui/src/commit_box.rs:528`).

**A toolbar of about twelve icons** — three routes:

1. Paint them on a canvas as `ref_glyphs.rs` does: no dependency, no asset, no licence; costs
   hand-drawn paths at Fork's ~20-24 px toolbar size, which is where Fork's icons are detailed.
2. Inline SVG literals rendered by `SvgViewer` (the `TitlebarButton` pattern): no new crate, no
   feature. If the SVG text is copied from a set (Lucide, ISC), that is third-party artwork in the
   source with a licence to carry, and **no guard sees it**: `the_embedded_fonts_are_the_roster_each_with_its_licence`
   reads only the files in `crates/cairn-app/assets/fonts/` (`crates/cairn-guards/tests/invariants.rs:9655-9680`,
   matcher `crates/cairn-guards/src/lib.rs:2819`). An icon asset directory would need its own roster
   and guard if the user wants the same rule for icons.
3. freya's `icons-lucide` feature: no new crate name in Cairn's manifests beyond a feature on
   `freya`, but it compiles in `freya-icons` (deps: `bytes`) and all 1,640 icons' bytes are
   `include_bytes!`'d functions (dead ones stripped by the linker); `cargo deny` would see
   `freya-icons`' MIT, not Lucide's ISC. A dependency decision for the user either way.

## 5. Window decorations

**Today Cairn uses the platform's decorations** and draws its own "title bar" row inside the
window below them: `WindowConfig::new(..).with_title("Cairn").with_on_close(..)`, no
`with_decorations` (`crates/cairn-app/src/main.rs:46-52`); the in-window row is
`crates/cairn-app/src/window.rs:538-575` ("Cairn" label, status box, Fetch).

**What the OS does with `decorations: true`** (Freya's default, `F/freya-winit/src/config.rs:148`,
passed to winit at `F/freya-winit/src/window.rs:209`):

- X11: the window manager decorates.
- Wayland: winit asks for server-side decorations; where the compositor answers client-side (GNOME's
  mutter has no xdg-decoration), winit draws its own frame with sctk-adwaita
  (`W/src/platform_impl/linux/wayland/window/state.rs:48`, `:278-290`, `:982-997`), on by default
  (`W/Cargo.toml:73-79`, `wayland-csd-adwaita`; `sctk-adwaita 0.10.1` is in Cairn's `Cargo.lock`).
  KDE and wlroots compositors decorate server-side.

**A custom title bar (Fork's menus-in-caption row): available.**

- `WindowConfig::with_decorations(false)` (`F/freya-winit/src/config.rs:183`).
- Drag region: `rect().window_drag()` — `WindowDragExt` (`F/freya-winit/src/extensions.rs:189-222`):
  a press calls winit's `Window::drag_window()` (`W/src/window.rs:1522`), a double press toggles
  maximized. A child that must not drag stops propagation of its pointer-down.
- Min/max/close: `TitlebarButton::new(TitlebarAction::{Minimize, Maximize, Restore, Close})`
  (`F/freya-components/src/titlebar.rs:38-82`), behind the `titlebar` feature
  (`F/freya-components/src/lib.rs:50-51`; adds no dependency, `F/freya-components/Cargo.toml`),
  themed `titlebar_button`: 46 px wide, full height, red close hover (`themes.rs:664-679`); it stops
  its pointer-down from starting a drag (`titlebar.rs:136-139`). The actions themselves are the
  caller's: `Platform::get().with_window(None, |w| w.set_minimized(true))`,
  `set_maximized`, `Platform::get().close_window(Platform::window_id())` — the worked example is
  `F/../examples/feature_titlebar.rs`. Maximized state: `Platform::is_maximized`
  (`F/freya-core/src/platform.rs:63`, refreshed on resize per `F/freya-winit/src/window.rs` comment
  at the `is_maximized` field).
- Resize edges: an undecorated window has none; `BorderlessPlugin` (feature `borderless`, crate
  `freya-borderless-plugin`, deps all freya workspace crates) overlays invisible resize bands on Linux
  and Windows (`F/freya-borderless-plugin/src/lib.rs:38-59`, `ResizeBands` `:165`) using winit's
  `drag_resize_window` (`W/src/window.rs:1537`). Docs: `F/freya/src/_docs/borderless.rs`.
- Window menu on right-click of a drawn caption: winit's `Window::show_window_menu`
  (`W/src/window.rs:1555`, implemented for Wayland at
  `W/src/platform_impl/linux/wayland/window/state.rs:934`), reachable through
  `Platform::with_window`.
- macOS: `with_traffic_light_inset(x, y)` moves the traffic lights for a full-size content view
  (`F/freya-winit/src/config.rs:239-255`, implementation `F/freya-winit/src/traffic_light.rs`);
  combine with winit's `WindowAttributesExtMacOS` via `with_window_attributes` (`config.rs:264`).
- Cost on Linux: dropping decorations loses the desktop's own title bar look (GNOME's Adwaita
  frame, KDE's Breeze), its button layout and side, and its theme; Cairn then owns drag, double
  click, resize bands and the window menu. Fork does this on Windows; Linux git clients that do it
  are divided.

## 6. Multiple windows

**Available**: a second OS window at runtime.

- `Platform::get().launch_window(WindowConfig::new(app)).await -> WindowId`
  (`WinitPlatformExt::launch_window`, `F/freya-winit/src/extensions.rs:63`, `:229-238`), plus
  `close_window(id)` (`:79`), `close_current_window()` (`:87`), `focus_window` (`:107`),
  `set_window_title` (`:148`), `with_window(id, |w| ..)` (`:168`). Also `LaunchConfig::with_window`
  repeatedly at startup (`F/freya-winit/src/config.rs:397`). Example passing a `State` between
  windows: `F/../examples/feature_multi_window.rs`.
- Each window is its own runner with its own `Platform`, theme, context menu host and key handling
  (`F/freya-winit/src/window.rs`, `AppWindow::new`): Preferences would call `use_init_theme` and mount
  a `ContextMenuViewer` of its own.
- Owner/child relationship: `set_window_parent(child, Some(parent))` keeps a utility window above its
  owner — **macOS only, a no-op on Linux** ("there is no Wayland equivalent",
  `F/freya-winit/src/parent_window.rs:15-28`, `F/freya-winit/src/extensions.rs:109-133`). No modal
  window: winit 0.30 has no cross-platform modality.
- Lifetime: the event loop exits only when no window is left (`F/freya-winit/src/renderer.rs:428`,
  `exit_on_close` default `true`, `config.rs:496`), so an open Preferences window would keep the
  process alive after the main window closes unless the main window's close path closes it
  (Cairn's close is `crates/cairn-app/src/closing.rs`).
- Choice for the packet: a second window is possible but, on Linux, unowned (it can fall behind the
  main window and has no parent link); an in-window modal is what Cairn's one modal state
  (`crates/cairn-app/src/modal.rs`) already decides for every dialog.

## 7. Tabs

**No `Tabs` component. NOT AVAILABLE as a tab strip; parts exist.**

- `FloatingTab` (`F/freya-components/src/floating_tab.rs:47`) is a single tab-shaped button with
  hover state and `on_press`; no selection model, no close button, no strip.
- Draggable tabs: `DockingArea` (`F/freya-components/src/docking.rs:318`, `new` `:352`) is a docking
  layout of `DockPanel`s (`:26`) whose tabs are dragged between and within panels, with caller
  renderers for the tab, the bar and the content — more than a repository strip needs.
  The generic parts beneath it are `DragZone` / `DropZone` / `use_drag`
  (`F/freya-components/src/drag_drop.rs:47`, `:221`, `:25`), which a reorderable strip can use
  directly.
- Cairn's own tabs today are compact `Button`s in a strip it draws itself
  (`crates/cairn-ui/src/detail/tabs.rs:1-3`, `:189-190`). Fork's repository strip (equal-width tabs,
  a dot, the name, a `+`; `docs/research/ui-review/fork-window-survey-2026-10-16.md`, section 1) would
  be drawn the same way.

## 8. Toolbar layout primitives and button sizing

- Icon + label button: `Button` takes any children (`F/freya-components/src/button.rs`, `elements`),
  so a vertical `rect` of icon over label is the child; `.flat()` for Fork's borderless toolbar look
  (`button.rs:269`).
- Dropdown / split button: **NOT AVAILABLE** as a component. Build as two adjacent buttons (action,
  then a chevron) with the chevron opening a `Menu` in an `Attached` below it (`attached.rs:61`), or
  `Select` for a pick-one list (`F/freya-components/src/select.rs:96`, `placement` `:152`).
- Segmented control: `SegmentedButton` + `ButtonSegment` (`F/freya-components/src/segmented_button.rs:282`,
  `:76`), selection the caller's (`:247-279` example); theme `button_segment` padding `(8, 16)`,
  `segmented_button` radius 99 (pill) (`themes.rs:622-645`).
- Separators: none; a 1 px `rect`.
- Badge (Fork's "1" on Push): none; an absolutely positioned `rect` over the button.
- `Chip` (`F/freya-components/src/chip.rs:68`) and `SideBarItem` (`F/freya-components/src/sidebar.rs:72`)
  exist but are not toolbar shapes.

**Button sizing knobs** (`F/freya-components/src/button.rs`):

- Layout theme `ButtonLayout { margin, corner_radius, width, height, padding }` (`:12-25`), three
  presets: `button_layout` padding `(6, 12, 6, 12)`, `compact_button_layout` `(3, 6, 3, 6)`,
  `expanded_button_layout` `(10, 16, 10, 16)`, all radius 6 (`themes.rs:170-199`). Chosen by
  `.compact()` / `.expanded()` (`button.rs:249`, `:254`), overridden per button by
  `.theme_layout(ButtonLayoutThemePartial)` (`:243`).
- Colour theme `ButtonColors` (`:27-45`), four variants `button`, `outline_button`, `filled_button`,
  `flat_button` (`:305-324`), overridden by `.theme_colors(..)` (`:238`).
- **No font size on a button**: the theme has none and `render` sets none (`button.rs:297-440`); a
  button's text inherits from its ancestors' text style.

**Where theme defaults live and how Cairn overrides them**:

- `Theme` is a palette plus a map of component themes by key, `Theme::set(key, value)` /
  `get` (`F/freya-components/src/theming/component_themes.rs:40-69`); the defaults are registered by
  `register_base_component_themes` (`F/freya-components/src/theming/themes.rs:159`), built into
  `dark_theme()` / `light_theme()` (`:700-721`), which take the OS accent colour as primary
  (`:712-728`).
- Cairn installs `dark_theme` unchanged (`crates/cairn-app/src/main.rs:61`, `use_init_theme`,
  `F/freya-components/src/theming/hooks.rs:15`), reads palette slots in components through
  `get_theme_or_default().read().colors()` (e.g. `crates/cairn-ui/src/check_box.rs:20`,
  `crates/cairn-ui/src/sidebar.rs:251`), and sizes each element locally: `.compact()` buttons
  throughout (e.g. `crates/cairn-ui/src/detail/tabs.rs:190`, `crates/cairn-app/src/window.rs:588`) and
  explicit `font_size(..)` per label (12 and 13 most often). It overrides **no** component theme
  today. A denser look in one place is a `Theme` built from `dark_theme()` with
  `theme.set("compact_button_layout", ..)`, `theme.set("menu_item", ..)`, `theme.set("tooltip", ..)`
  before `use_init_theme` returns it — one table, not per-call overrides.

## 9. Font sizes and DPI

- HiDPI: the effective scale factor is the OS's times a custom one,
  `window.scale_factor() * custom_scale_factor` (`F/freya-winit/src/window.rs:273-274`, `:488-490`);
  layout sizes are logical, scaled at render. The custom factor is `WindowConfig::with_custom_scale_factor`
  (`F/freya-winit/src/config.rs:214`), clamped 0.25-5.0 (`window.rs:140-145`), changed at runtime by
  `Platform::set_custom_scale_factor` (`F/freya-core/src/platform.rs:98`); the optional
  `zoom-shortcuts` feature binds Ctrl/Cmd +/−/0 to it (`F/freya-components/src/integration.rs:33-42`)
  — a literal-modifier binding outside Cairn's accelerator table, so it should stay off. `Platform`
  publishes `scale_factor` and `custom_scale_factor` (`platform.rs:69-71`). A font-size preference
  could be a custom scale factor (everything scales) or a theme value (text only).
- Default text: `FontSize(16)` (`F/freya-core/src/style/font_size.rs:10-13`); default families on
  Linux `Ubuntu`, `Adwaita Sans`, then `Noto Sans`, `Arial`; macOS `.AppleSystemUIFont`; Windows
  `Segoe UI` (`F/freya-core/src/style/default_fonts.rs:3-17`), the `LaunchConfig` fallback list
  (`config.rs:348`, `fallback_fonts: default_fonts()`).
- Cairn's UI font: no family is set anywhere for UI text — `LaunchConfig` adds only the diff's
  `IBM Plex Mono` (`crates/cairn-app/src/main.rs:40`, `:55`), the window root sets no font
  (`crates/cairn-app/src/window.rs:194-196`). So UI text is Freya's fallback list above, at 16 px
  wherever a component does not set a size (menus, notably).
- System UI font: **NOT AVAILABLE** as a query. Freya reads OS preferences through `mundy` with only
  `accent-color` enabled (`F/../Cargo.toml:96-100`); no font or text-scale preference is read.
  `LaunchConfig::with_default_font(name)` puts a family first (`config.rs:461`); naming the
  fontconfig alias `sans-serif` there would let fontconfig choose the desktop's sans, but whether
  Skia's font manager resolves the alias through `matchFamily` was not verified here. Reading GNOME's
  `font-name` / `text-scaling-factor` would be new code and probably a new dependency.

## 10. Freya components that already look like Fork chrome

| Fork chrome | Freya piece | Fit |
| --- | --- | --- |
| Caption buttons, drag region | `TitlebarButton`, `window_drag`, `BorderlessPlugin` (sections 5) | Direct; Windows-style 46 px buttons |
| Context menus with submenus | `ContextMenu`, `Menu`, `SubMenu`, `MenuButton` (section 2) | Direct; restyle for density; no separator, no shortcut slot |
| Tooltips | `TooltipContainer` (section 3) | Direct; 500 ms delay, flips |
| Dropdown under a toolbar button | `Attached` + `Menu`; `Select` | Parts |
| Segmented control (diff modes) | `SegmentedButton` | Direct; pill-shaped by default |
| Splitters | `ResizableContainer`, `ResizablePanel`, `ResizableHandle` (`F/freya-components/src/resizable_container.rs:461`, `:571`, `:797`) | Direct |
| Dialogs | `Popup`, `PopupTitle`, `PopupContent`, `PopupButtons` (`F/freya-components/src/popup.rs:134`, `:283`, `:306`, `:338`) | Direct; Cairn draws its own today |
| Tab strip | `FloatingTab`, `DockingArea`, `DragZone`/`DropZone` (section 7) | Parts |
| File tree (Fork's tree view) | `TreeItem`, `TreeConfig` (`F/freya-components/src/tree.rs:112`, `:59`) | Not virtualized by itself; Cairn's lists must stay on `VirtualScrollView` |
| Native menu bar | `LaunchConfig::with_menu` (muda) | macOS only |
| Toolbar icons | `SvgViewer`, Lucide via `icons-lucide`, `canvas` | Parts; dependency or drawing decision |
