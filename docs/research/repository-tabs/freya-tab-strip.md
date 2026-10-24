# Freya at caa46f8 for a tab strip: components, state, tasks, windows, pickers and file drops

Research, read-only, 2026-10-24. Each claim is marked **VERIFIED** (read in the vendored source,
or measured with `cargo tree` against Cairn's own `Cargo.lock`) or **UNVERIFIED** (inferred, or
from outside the vendored source). Facts that could not be settled are listed under **OPEN**, not
guessed. This document adds to `docs/research/ui-review/freya-chrome-apis.md` (2026-10-16), whose
sections 6 (multiple windows) and 7 (tabs) it re-checks and takes further.

## Where the source is

- Cairn pins `freya = { git = "https://github.com/alexparlett/freya", rev = "caa46f8715744cb4fdb2d36304e2b1bbeb4acdb0" }`
  (`Cargo.toml:20`, with `freya-testing` at the same rev on `:22`). Version
  `0.5.0-rc.4` (`crates/freya/Cargo.toml:3`). `cargo fetch` in a worktree of `feature/fork-alignment` (b539d9d) succeeded.
  **VERIFIED.**
- Source: `~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/`, `git log -1` =
  `caa46f8715744cb4fdb2d36304e2b1bbeb4acdb0`. Below, **`F/`** means
  `~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/crates/`, and **`W/`** means winit 0.30.13
  at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.13/`. **VERIFIED.**
- Everything below is the builder API (`rect().child(..)`, `impl Component for T { fn render(&self) -> impl IntoElement }`).
  No `rsx!` appears anywhere in what was read. **VERIFIED.**

## 1. A tabs component

**No `Tabs`, `TabBar` or `Tab` component. A repository tab strip has to be built from `rect`s
and `label`s, or from the parts below.** **VERIFIED**: the module list
`F/freya-components/src/lib.rs` has no such module. A grep for `struct Tab`, `struct Tabs` and
`TabBar` over `F/` and `F/../examples/` finds only `docking.rs` (`TabBarContext`, a context type)
and two examples.

What the fork does have:

- **`FloatingTab`** (`F/freya-components/src/floating_tab.rs`). One tab-shaped button. It takes any
  children (`ChildrenExt`) and an `on_press` (`:122`). It keeps its own hover state with
  `use_state(TabStatus::default)` and pointer enter/leave. It reads "am I the active tab?" from an
  ancestor `Activable` (`use_is_active()`, `:133`). Its theme has a separate `selected_background`
  and `selected_color` (`:25`) so the active tab and the hovered tab look different. Its a11y role
  is `AccessibilityRole::Tab` (`:167`). **VERIFIED.** It has no close button, no selection model,
  no strip, no overflow handling and no drag. **VERIFIED** (none appears in the file).
  ```rust
  let (background, color) = match (is_active, hovering) {
      (true, _) => (selected_background, selected_color),
      (false, true) => (hover_background, color),
      (false, false) => (background, color),
  };
  ```
- **`Activable`** (`F/freya-components/src/activable.rs`). Wraps one child and provides an
  `ActivableContext(Readable<bool>)`, which `FloatingTab` and `SideBarItem` read
  (`F/freya-components/src/activable_context.rs`). **VERIFIED.**
- **`DockingArea<M: DockingModel>`** (`F/freya-components/src/docking.rs:318`, `new` `:352`). An IDE
  docking layout:
  - The model is the caller's `trait DockingModel` (`:227`): `root()`, `on_drop(value, DropTarget)`
    and `set_active(panel, tab)`.
  - It renders a tree of `DockNode::Split` / `DockNode::Panel` nodes through `ResizableContainer`.
  - Four renderer callbacks are the caller's: content, tab, drag ghost and tab bar.
  - Each tab is a `DragZone` inside a `DropZone`, keyed `.key(tab_id)` (`:527`).
  - A panel draws **only its active tab's content**: `renderers.content.call(ContentContext { tab_id: active_tab_id, .. })` (`:554`).
  - `DockPanel::insert_tab` reorders within a panel (`:47`). `DropTarget::Split` splits a panel.

  **VERIFIED.** This is more than a repository strip needs: splits, edge drop zones, an overlay.
  Its tab bar is also whatever the caller's `render_tab_bar` lays out, so overflow is still the
  caller's problem. **VERIFIED.**
- **`DragZone<T>` / `DropZone<T>` / `use_drag::<T>()`** (`F/freya-components/src/drag_drop.rs`). The
  generic drag that `DockingArea` is built on, and usable directly to reorder tabs:
  - `use_drag` (`:25`) is a `State<Option<T>>` provided at the root scope, one per payload type.
  - `DragZone::new(data)` (`:47`) starts a drag on a **left** pointer-down (`:153`) once the pointer
    has moved past `drag_threshold` (4 px by default). Its `drag_element` follows the cursor on
    `Layer::Overlay`.
  - `DropZone::new(on_drop)` (`:221`) receives the payload on `on_mouse_up` (`:292`).
    `on_drag_over(bool)` (`:263`) says when a drag enters or leaves it, which is what a reorder
    needs to draw an insertion marker.

  **VERIFIED.**

**Closable tabs:** there is no built-in support. A close glyph inside the tab's `rect` is the way:
give it its own `on_press`, and stop the press from reaching the tab (see section 7). **VERIFIED**
that nothing provides it. Cairn already draws glyphs as canvas shapes
(`crates/cairn-ui/src/history/ref_glyphs.rs`).

**Drag reorder:** no ready-made strip. Build it from `DragZone`/`DropZone` as `DockingArea` does, or
from Cairn's own drag pattern (`crates/cairn-ui/src/local_changes/drag.rs`, which uses global
pointer move, press and down). **VERIFIED** (parts exist; no strip uses them).

**Overflow:**
- There is no tab-overflow menu or scroll. **VERIFIED.**
- `OverflowedContent` (`F/freya-components/src/overflowed_content.rs`) is a marquee animation for
  text that does not fit. It does not handle a strip's overflow. **VERIFIED.**
- A horizontally scrolling strip would normally be a `ScrollView`. Cairn's guard
  `a_history_sized_list_renders_through_a_virtualizing_view` forbids naming `ScrollView` in any
  render file (its exceptions roster is empty), so that is a user decision. `VirtualScrollView`
  does take `.direction(Direction)` (`F/freya-components/src/scrollviews/virtual_scrollview.rs:381`).
  **VERIFIED.**
- Fork-like options that need no scroll: equal-width tabs that shrink, plus a "more" menu built from
  `Menu`/`Attached`. **UNVERIFIED** as a design. It is an inference from the parts.

Cairn's own precedent is the detail pane's strip, a keyed `rect` per tab with an accent bottom
border and `on_press` (`crates/cairn-ui/src/detail/tabs.rs`, `.key(tab.caption())`
at `:142`). **VERIFIED.**

## 2. Component-local state when a subtree is swapped, keys, and hidden tabs

### How state is held

- `use_state(init)` is `use_hook(|| State::create(init()))` (`F/freya-core/src/lifecycle/state.rs:703`).
  `State<T>` is a `Copy` handle into a `generational_box` `UnsyncStorage` owned by the **scope** it
  was created in (`create_in_scope`, `:566-579`). **VERIFIED.**
- Hooks are positional: `use_hook` indexes `scopes_storage.values` by call order
  (`F/freya-core/src/lifecycle/base.rs:75-110`). The hooks error text forbids calling them
  conditionally or in loops (`base.rs:9-40`), and it panics on a mismatch. So a component cannot
  call `use_state` once per tab for a variable number of tabs. **VERIFIED.**
- `State::create_in_scope(value, scope_id)` is not a hook. It can create state at runtime in another
  scope, such as the root, as `use_drag` does with `ScopeId::ROOT`. It lives until that scope's
  storage drops. **VERIFIED.** No public way to free a single `State` early was found in `state.rs`
  (a grep for `drop` finds only `take()` on `State<Option<T>>`, `:664`). So per-tab states made in
  the root this way would accumulate for the session unless their contents are `take()`n.
  **VERIFIED** that no free is visible in the code; that `generational_box` has no reachable
  per-box drop through Freya is part of the same reading.
- `State::create_global` leaks its owner on purpose (`state.rs:607-618`). **VERIFIED.**
- `use_drop(f)` runs `f` when the component drops (`base.rs:126`). **VERIFIED.**

### What a key is

- `DiffKey` is `enum { Root, U64(u64), DefaultU64(u64), None }` (`F/freya-core/src/diff_key.rs`).
  **VERIFIED.**
- `.key(value)` comes from `KeyExt`, implemented by `rect()`, `label()` and every component that
  stores a `key: DiffKey` (for example `FloatingTab`, `DragZone`, `Activable`). It hashes
  `TypeId::of::<Self>()` together with the value into `DiffKey::U64`
  (`F/freya-core/src/elements/extensions.rs:126-142`). So a key is scoped to the element or
  component type, and the same value on two types never collides. **VERIFIED.**
  ```rust
  fn key(mut self, key: impl Hash) -> Self where Self: 'static {
      let mut hasher = FxHasher::default();
      std::any::TypeId::of::<Self>().hash(&mut hasher);
      key.hash(&mut hasher);
      *self.write_key() = DiffKey::U64(hasher.finish());
      self
  }
  ```
- With no key, a component's key is its `TypeId` hash, `DiffKey::DefaultU64`
  (`F/freya-core/src/element.rs:380-393`). A component's `render_key()` returns
  `self.key.clone().or(self.default_key())`, the pattern every built-in uses. **VERIFIED.**

### Reconciliation (`F/freya-core/src/path_element.rs`, `diff_inner`)

- Children pair **positionally while keys match** (`:215`). After the first mismatch, the remaining
  old children go into a map from key to a queue of indices, and each new child takes the first old
  child with an equal key (`:234`). A matched child is diffed; a child that moved records a move. An
  unmatched new child is **added**; an unmatched old child is **removed**. **VERIFIED.**
- So keyed siblings that are reordered keep their scopes and state, which is what a tab reorder
  needs. **VERIFIED.**
- A component at the same path whose key changes is removed and added
  (`if k1 != k2 { diff.added.push; diff.removed.push }`). In the runner, a scope whose key or type
  changed has its storage `reset()`: its hooks are cleared and its state is gone
  (`F/freya-core/src/runner.rs:827-842`). **VERIFIED.** So swapping keyed child A for keyed child B
  drops A's state, and B starts fresh. Switching back to A builds A anew.
- When a scope is removed, the runner removes the scope, then **drops every task whose `scope_id` is
  that scope**, then drops its storage (`runner.rs:1267-1296`, the `retain` at `:1287`).
  **VERIFIED.**
- Debug builds panic on a duplicate sibling key, ignoring `None` and `DefaultU64` keys
  (`path_element.rs:194-212`). **VERIFIED.**

### Keeping N tab subtrees alive but hidden

- **Not mounted** (only the active tab is a child): cheapest per frame. But each tab's component
  state, its tasks and its `use_scroll_controller` positions are dropped on every switch. They
  survive only if they are held outside the tab's scope: in the window's root (as Cairn's `View`
  does today, every `State` made in `app`'s scope, `crates/cairn-app/src/main.rs`)
  or in a model object. **VERIFIED** (from the reconciliation above).
- **Mounted but hidden:** Freya has no public `display: none`.
  - The layout node's `hidden` flag that the renderer skips (`F/freya-core/src/render_pipeline.rs:64`)
    is set only by an element's custom post-measure. The only one found is `paragraph`'s, for its
    inline children (`F/torin/src/measure.rs:109-145`, `F/freya-core/src/elements/paragraph.rs:418-437`).
    **VERIFIED.**
  - So "hidden" means a zero-size, `overflow(Overflow::Clip)` parent, or one that is off-screen.
    The renderer then skips each node whose visible area misses its clip
    (`render_pipeline.rs:75-88`). But it still **visits every node on every frame**
    (`// TODO: Use incremental rendering`, `:58`). **VERIFIED.**
  - Layout still runs for hidden nodes. **UNVERIFIED** how much it costs: torin is incremental for
    dirty nodes, but a zero-size parent's fixed-size descendants are still measured.
  - `interactive(false)` stops pointer hit-testing (`F/freya-core/src/events/measurer.rs:109-113`),
    but **global** events reach every listener wherever it is: "a global equivalent event is emitted"
    to `get_listeners_of(global)` (`F/ragnarok/src/measurement.rs:13-40`). **VERIFIED.**
  - Consequence for Cairn: a hidden, mounted tab's `on_global_key_down` / `on_global_key_up`
    (the window root's at `crates/cairn-app/src/window.rs:228`, `:235`),
    `on_global_pointer_*` (`edge_scroll.rs`, `local_changes/drag.rs`, `diff/staging_gesture.rs`)
    and `Activity` key handler would all still fire. Each tab's handlers would have to check
    whether their tab is active. **VERIFIED** for the dispatch; the consequence is inferred.
  - A hidden tab whose `State`s are written (for example by its own update stream) re-renders its
    scopes, diffs and lays out while hidden. **VERIFIED** for the mechanism (a write marks the
    subscribed scope dirty, `State::notify` in `state.rs`). The cost is unmeasured.
- **Cheap middle ground:** keep each tab's state in a per-tab model value that the window holds
  (plain data, not hooks), and mount only the active tab's view over it. Scroll offsets and similar
  view-only state would have to be part of that model rather than `use_scroll_controller` hooks in
  the subtree. **UNVERIFIED** as a recommendation; it is inferred from the mechanics above.
- `Lazy` (`F/freya-components/src/lazy.rs`) renders its children only once its wrapper becomes
  visible. It is not a keep-alive. **VERIFIED.**
- No `KeepAlive` or offscreen component exists. A grep for `keep_alive|KeepAlive|Offscreen` in
  freya-core and freya-components found none. **VERIFIED.**

## 3. Async tasks and streams feeding state

- `spawn(future)` attaches a task to the **current component scope**, and "Freya cancels the task
  when that component unmounts" (`F/freya-core/src/lifecycle/task.rs:44-64`). That is enforced by
  the runner's `retain` on scope removal (`runner.rs:1287`). **VERIFIED.**
- `spawn_forever(future)` attaches it to `ScopeId::ROOT` (`task.rs:40`). `spawn_in_scope(future, scope)`
  takes any scope (`:67`). **VERIFIED.**
- `TaskHandle::cancel()` cancels early. `TaskHandle::owned()` gives an `OwnedTaskHandle` that cancels
  when its last clone drops (`task.rs:113`, `:152`). **VERIFIED.**
- Tasks are stored in a map keyed by `TaskId` and polled one at a time when woken
  (`runner.rs:656-695`), so **any number run concurrently** on the UI executor. **VERIFIED.**
- The waker is `TaskWaker` holding a `futures_channel::mpsc::UnboundedSender<Message>`
  (`task.rs:212-223`), which is `Send`. A worker thread can wake it. freya-winit wakes its event loop
  through `FuturesWaker(EventLoopProxy)` (`F/freya-winit/src/lib.rs:122-132`) and polls the runner on
  `NativeWindowEventAction::PollRunner` (`F/freya-winit/src/renderer.rs:682`). **VERIFIED.**
- A task updates state by writing a `State` it captured. That marks the subscribed scopes dirty.
  **VERIFIED.**
- `use_future` / `FutureTask` (`F/freya-core/src/lifecycle/future_task.rs:146`) is a hook-held task
  with a loading state, `cancel` and `start`. **VERIFIED.**
- **How Cairn does it today:** in `app`'s `use_hook`, `worker::open` returns `(handle, updates, reply)`.
  Then `spawn(async move { while let Some(update) = updates.next().await { session::apply(update, view, ..) } })`
  (`crates/cairn-app/src/main.rs`). It is spawned in the root app scope, so it
  lives as long as the window. `Updates::next` is an `async fn` over a `Receiver` and a `Wake`
  (`crates/cairn-app/src/worker/pool.rs:1020-1062`). **VERIFIED.**
- **For N tabs:** each repository's stream can be its own task.
  - Spawned where the tab's state lives: in the root via `spawn_forever` or `spawn_in_scope(.., ROOT)`
    if state is held per tab in the window, cancelled with its `TaskHandle` when the tab closes.
  - Or with `spawn` inside a keyed per-tab component, which then dies with that component's mount.
    So a tab that is not mounted cannot own its own stream.

  **VERIFIED** for the mechanics. That `Drop for Updates` stops the walk (`pool.rs:1057-1061`), and so
  dropping a cancelled task's future stops that repository's walk, is **VERIFIED** from Cairn's
  source. What a close then requires (`Closing`, the reaps) is Cairn's own design. **OPEN.**

## 4. Multiple OS windows (a rejected alternative)

- **Supported.** `Platform::get().launch_window(WindowConfig::new(app)).await -> WindowId`
  (`F/freya-winit/src/extensions.rs:63`, implementation `:229-238`, through a oneshot to the event
  loop). Also `close_window(id)` (`:79`), `focus_window`, `set_window_title` and `with_window`, and
  `LaunchConfig::with_window` repeated at startup (`F/freya-winit/src/config.rs:397`). Examples:
  `F/../examples/feature_multi_window.rs` and its `_i18n`, `_queries`, `_radio` and `_router`
  variants. **VERIFIED.**
- `set_window_parent` (`extensions.rs:133`) is macOS-only. On Linux it is a no-op
  (`F/freya-winit/src/parent_window.rs`). **VERIFIED** (re-checked from the earlier research,
  section 6).
- The event loop exits when no window is left and `exit_on_close` is true (the default)
  (`renderer.rs:414-429`, `config.rs:351`, `:496`). **VERIFIED.**
- Each window has its own runner, scopes and contexts. Sharing between windows goes through
  `State::create_global` (leaked) or values passed in. **VERIFIED.**

## 5. A native file or folder picker

- **Freya provides no picker component or API.** No `FileDialog` or picker appears in
  `F/freya-components/src`. **VERIFIED.**
- **But `rfd` 0.17.2 is already in Cairn's dependency graph.** `freya-winit` depends on it on every
  non-Android target, for its crash dialog (`F/freya-winit/Cargo.toml`, the
  `[target.'cfg(not(target_os = "android"))'.dependencies] rfd`; used at `F/freya-winit/src/lib.rs:75`,
  `rfd::MessageDialog`). `cargo tree -i rfd` in a worktree of `feature/fork-alignment` (b539d9d) gives
  `rfd v0.17.2 └── freya-winit`. **VERIFIED.**

### Option A: `rfd` as a direct dependency of `cairn-app`

- **Cost:** measured on a copy of the repository
  (a scratch copy outside the repository)
  with `rfd = "0.17.2"` added. The `Cargo.lock` diff is **one line**: `"rfd"` in `cairn-app`'s
  dependency list. **Zero new packages.** **VERIFIED.**
- It is still a user decision:
  - It needs a `layer_dependencies_are_allowlisted` row.
  - `rfd`'s features would need pinning in `deny.toml`, because Freya enables its defaults,
    `xdg-portal` and `wayland`.
- Linux backend at those defaults:
  - It talks D-Bus to `org.freedesktop.portal.FileChooser` through **`libdbus-1.so.3`, `dlopen`ed at
    run time** (`rfd-0.17.2/src/backend/xdg_desktop_portal/portal/ffi.rs:199-227`).
  - Failing that, it falls back to running **`zenity`** with `std::process::Command`
    (`rfd-0.17.2/src/backend/linux/zenity.rs:1`, `:40-41`; the `warn!("Using zenity fallback")` in
    `xdg_desktop_portal.rs:188`).
  - `build.rs` refuses to build with both or neither of `gtk3` and `xdg-portal`.

  **VERIFIED.**
- `pick_folder` exists in both the sync `FileDialog` and the async `AsyncFileDialog`
  (`rfd-0.17.2/src/file_dialog.rs:132`, `:281`). `set_parent` takes a raw window handle (`:96`,
  `:241`) so the portal can parent the dialog. **VERIFIED.**
- Threading:
  - The sync call blocks: it runs `pollster::block_on` in the portal backend. It would have to run
    inside `crates/cairn-app/src/worker/`.
  - The async call spawns a `std::thread` itself and awaits a runtime-agnostic oneshot
    (`xdg_desktop_portal.rs:18-32`, `:232-234`; `rfd-0.17.2/src/oneshot.rs`). It could be awaited in
    a Freya `spawn` on the render side without naming a waiting primitive.

  **VERIFIED.**
- Two review points, for whether the async call fits `the_ui_thread_never_waits_on_repository_work`:
  - Its parent-window lookup, `to_window_identifier`, runs `block_on`, but inside `pick_folder`,
    which the async call runs on the thread it spawned (`:166`, `:233`), not on the caller's.
    **VERIFIED.**
  - The zenity fallback starts a process outside Cairn's `process/` module. The process twins scan
    Cairn's source only, so they would not see it. That is a review point, not a guard failure.
    **VERIFIED** for the scan scope stated in `CLAUDE.md`.

### Option B: `ashpd` (XDG portal wrapper over `zbus`)

- Latest is 0.13.13 (`cargo info ashpd`). Its default feature is `tokio`; it also offers `async-io`,
  and per-portal features (`file_chooser`, `raw_handle`, `wayland`, …) (`ashpd-0.13.13/Cargo.toml`
  `[features]`). **VERIFIED.**
- **Cost**, measured on the same repository copy with
  `ashpd = { version = "0.13", default-features = false, features = ["async-io", "file_chooser", "raw_handle", "wayland"] }`:
  **one new package (`ashpd`)**, plus two features turned on in crates already present (the
  `Cargo.lock` diff also adds `futures-io` and `memchr` to an existing package's dependency list).
  `zbus` 5.19, `zvariant`, `serde_repr`, `enumflags2` and the wayland crates are already in Cairn's
  tree through `accesskit_unix` and `mundy` (`cargo tree -i zbus`). **VERIFIED.**
- With its default `tokio` feature it would pull in tokio. **VERIFIED**: the first probe, with
  defaults, resolved `tokio v1.54.1`.
- It is pure Rust over the session bus with no libdbus and no zenity fallback. **UNVERIFIED** in
  full: only its manifest and dependency list were read, not its source.
- It covers Linux portals only. macOS would still need another route (rfd's AppKit backend).
  **UNVERIFIED** (from what the crate is for, not read).

### Option C: no picker

Cairn takes the repository on its command line today (`crates/cairn-app/src/repository_path.rs`).
A typed-path field, or drag and drop where it works (section 6), needs no dependency. **VERIFIED**
that the module exists.

## 6. A folder dropped from the file manager onto the window

- **Freya exposes file drops.** On any element there are:
  - `on_file_drop` (`EventName::FileDrop`). It bubbles: it is not global, moved, enter or capture
    (`F/freya-core/src/events/name.rs` `does_bubble`, `is_global`).
  - `on_global_file_hover` and `on_global_file_hover_cancelled`, both global.

  They are declared at `F/freya-core/src/elements/extensions.rs:284-293`. The payload is
  `FileEventData { cursor: CursorPoint, file_paths: Vec<PathBuf> }`
  (`F/freya-core/src/events/data.rs:332-335`). **VERIFIED.**
- winit's `DroppedFile(PathBuf)` is buffered and delivered as **one `FileDrop` on the next
  `CursorMoved`**, hit-tested at that cursor position (`F/freya-winit/src/renderer.rs:1303-1309`,
  `:1353-1355`). `HoveredFile` and `HoveredFileCancelled` go straight through as the global events
  (`:1356-1369`). **VERIFIED.** So the drop lands where the pointer is at its next move, not
  necessarily at the release point. On X11 that is normally the same place. **UNVERIFIED.**
- **Platform support in winit 0.30.13:**
  - **X11**: `DroppedFile` is emitted from XDND's `text/uri-list`, `file://` URIs parsed into paths
    (`W/src/platform_impl/linux/x11/event_processor.rs:541-550`, `W/src/platform_impl/linux/x11/dnd.rs:146-163`).
    **VERIFIED.**
  - macOS and Windows: implemented (`W/src/platform_impl/macos/window_delegate.rs:384-429`,
    `W/src/platform_impl/windows/drop_handler.rs`). **VERIFIED.**
  - **Wayland: not implemented.** Nothing under `W/src/platform_impl/linux/wayland/` emits
    `DroppedFile` or `HoveredFile`, and nothing there names a data device. **VERIFIED** by absence
    (grep).
  - So **on a native Wayland session a dropped folder does nothing**. Under XWayland it would arrive
    through the X11 path. **UNVERIFIED**: depends on the backend winit picks.
- Freya builds both the x11 and wayland backends on Linux (`F/freya-winit/Cargo.toml`, `"x11"`,
  `"wayland"` features on freya-engine). **VERIFIED.** No code forcing a backend was found in
  `F/freya-winit/src`. **VERIFIED** (a grep for `with_x11`, `with_wayland` and
  `EventLoopBuilderExt*` found none).
- A dropped directory arrives as its path, like a file's. **VERIFIED** on X11 (the URI is parsed to a
  path with no file-type check, `dnd.rs:146-163`).

## 7. Middle-click, pointer buttons and hover, for a tab's close button

- `MouseButton { Left, Right, Middle, Back, Forward, Other(u16) }` (`F/freya-core/src/events/data.rs:24-31`),
  mapped one to one from winit (`F/freya-winit/src/winit_mappings.rs:15-24`). **VERIFIED.**
- `MouseDown` and `MouseUp` are sent for **every** button, carrying
  `button: Some(map_winit_mouse_button(button))` (`F/freya-winit/src/renderer.rs:1193-1200`).
  **VERIFIED.**
- Read the button from `PointerEventData::button()` (`data.rs:303-308`; `None` for touch) or from
  `MouseEventData.button`. **VERIFIED.**
- Which handler fires for which button:

  | Handler | Fires for | Source |
  | --- | --- | --- |
  | `on_press` | left mouse, touch, or the activation key on a focused element; filters out the middle button | `extensions.rs:347-369`, filter at `:353` |
  | `on_all_press` | any button | `:396` |
  | `on_secondary_down` | right mouse-down | `:374` |

  A middle-click close is `on_all_press` (or `on_pointer_press`) matching
  `PressEventData::Mouse(m) if m.button == Some(MouseButton::Middle)`, or `on_pointer_down` matching
  `e.button() == Some(MouseButton::Middle)`. **VERIFIED.**
  ```rust
  rect().on_all_press(move |e: Event<PressEventData>| {
      if let PressEventData::Mouse(m) = e.data() { if m.button == Some(MouseButton::Middle) { /* close */ } }
  })
  ```
  That `Event::data()` is the accessor name is **UNVERIFIED**. Cairn's code uses `e.button()` on
  `Event<PointerEventData>` (`crates/cairn-ui/src/history/list.rs:460`) and `e.button` on mouse data
  (`crates/cairn-ui/src/local_changes/list.rs:600`); `drag_drop.rs:153` uses `e.data().button()`.
- Cairn already tells buttons apart this way, Right and Left only (`crates/cairn-ui/src/caption_row.rs:150-153`,
  `local_changes/list.rs:813`). No `Middle` appears in Cairn's source today. **VERIFIED.**
- Modifiers: Cairn's `CLAUDE.md` records that this toolkit build carries no modifiers on a pointer
  event, so a modified press resolves through `HeldKeys` and the accelerator table. A tab gesture
  with a modifier must go the same way. **VERIFIED** as Cairn's stated contract; not re-read in Freya.
- **Hover:** `on_pointer_enter` / `on_pointer_leave` plus a `use_state(bool)` is the pattern both
  Freya (`FloatingTab`) and Cairn (`crates/cairn-ui/src/bar_button.rs:37`, `:72-73`,
  `set_if_modified(false)`) use. `PointerEnter` and `PointerLeave` are exclusive enter/leave, and
  `PointerOver` / `PointerOut` include children (`F/freya-core/src/events/name.rs` `is_exclusive_enter`,
  `is_enter`; doc comments at `extensions.rs:270-279`). For a close glyph shown only while its tab is
  hovered, use the tab `rect`'s enter/leave state, plus the glyph's own enter/leave for its hover
  fill. **VERIFIED** for the events; the layout is a suggestion.
- **Keeping the close press from selecting the tab:** `on_press` bubbles (`does_bubble`, name.rs).
  The glyph's handler calls `e.stop_propagation()`, as `DropZone`'s `on_mouse_up` does
  (`drag_drop.rs`) and `TitlebarButton` does for its pointer-down
  (`F/freya-components/src/titlebar.rs:136-139`, per the earlier research). **VERIFIED** that
  `stop_propagation` exists and is used that way; that it stops the parent's `on_press` in this case
  is **UNVERIFIED** (not exercised).
- If the tab is a `DragZone`, a left press on the close glyph also arms that `DragZone`'s pointer-down
  (`drag_drop.rs:152-159`) unless the glyph stops the pointer-down too. A drag then starts only past
  4 px of movement. **VERIFIED** for the code; the interaction is not exercised.

## OPEN

- How much layout and per-frame render-walk cost N mounted, hidden repository views would add (each
  a virtualized history, sidebar and diff) was not measured. Only the mechanisms are verified.
- Whether, after a `DroppedFile`, the next `CursorMoved` reliably arrives at the drop point on X11,
  and whether a release with no further motion leaves the drop undelivered until the pointer moves.
  The code buffers until the next `CursorMoved`; the runtime behaviour was not observed.
- Which winit backend Cairn ends up on under GNOME, KDE and wlroots sessions (native Wayland, where
  drops do not work, or XWayland), and whether `WINIT_UNIX_BACKEND` or `WAYLAND_DISPLAY` decide it at
  this winit version. Not read.
- `ashpd`'s run-time behaviour (whether its `async-io` path spawns its own executor thread, how it
  behaves when no portal is running) was not read in source.
- Whether rfd's portal dialog is modal to Cairn's window on Wayland: it needs xdg-foreign through the
  `wayland` feature's `WindowIdentifier`, in `rfd-0.17.2/src/backend/xdg_desktop_portal/window_identifier`.
  Not read past the call site.
- Whether a `deny.toml` `[[bans.features]]` pin on `rfd` is satisfiable while `freya-winit` enables its
  defaults. Not run.
- Whether `e.stop_propagation()` on a child's `on_press` stops a parent `rect`'s `on_press`. Not
  exercised in a headless test.
