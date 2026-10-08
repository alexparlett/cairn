# Freya UI APIs for staging and commit

- **Date:** 2026-10-07
- **Commissioned by:** staging-and-commit planning
- **Freya:** the fork `github.com/alexparlett/freya`, rev
  `caa46f8715744cb4fdb2d36304e2b1bbeb4acdb0` (version `0.5.0-rc.4` plus fork commits,
  head "Let a titlebar button be themed, and fit a submenu to its own flyout"), pinned in
  the root `Cargo.toml` for both `freya` and `freya-testing`.
- **Checkout read:** `~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/` (written
  `$F` below). The registry copy was not used.
- **Features Cairn links:** `freya` default (`winit`) plus `engine` (in
  `crates/cairn-ui/Cargo.toml`). Nothing named here needs another feature, except where a
  section says so.

**Method.** Every claim comes from reading the checkout's source (components, `freya-edit`,
`freya-core` events and accessibility, `ragnarok` event dispatch, `freya-winit` input
mapping, `freya-testing`) and Cairn's own render crates (`crates/cairn-ui/src`,
`crates/cairn-app/src`, `crates/cairn-ui/tests`). Nothing was run. Anchors are paths and
symbols, not line numbers.

**Labels.** **VERIFIED-IN-SOURCE** (path) means the code was read and says what is claimed.
**(inferred)** means it was worked out from verified code but not seen running. **UNVERIFIED**
means it depends on something outside the checkout (the platform, winit's backends) or was
not read.

---

## Headlines

1. **There is no `TextArea`.** The editable surface is `Input` (`$F/crates/freya-components/src/input.rs`)
   with `.multiline(true)`, on `freya_edit`'s `use_editable`/`RopeEditor`. It covers what a
   commit box needs: cursor, selection by drag/double/triple press, IME preedit, undo/redo,
   copy/cut/paste, wrapping, a placeholder, setting the text from code and a caret binding.
   It has no character counter, no column guide, no spell check and no hard wrap.
2. **A focused `Input` blocks the window's global key listeners.** Its default key handler
   (`Input::key_down_default`) calls `prevent_default()` on every key except Enter, Escape,
   Shift and Tab, and in this toolkit that cancels the `GlobalKeyDown` that would follow. So
   while any text field has focus, `crates/cairn-app/src/window.rs`'s `on_global_key_down`
   never hears that key. Refresh (F5) and the tab chords are lost, and **`HeldKeys` never
   sees Control or Meta go down**, so a ⌘/Ctrl-click made while a field has focus does not
   extend the selection (inferred). This happens today with the filter fields.
3. **Two key behaviours the commit box must override.** (a) In a multiline `Input` with no
   `on_submit`, ⌘/Ctrl+Enter inserts a newline. With an `on_submit`, plain Enter submits.
   Neither matches Fork, where Enter is a newline and ⌘Enter commits. `on_pre_key_down`
   is the hook for this. (b) A primary+letter chord that is not one of the editor's own
   bindings types its letter (⌘R inserts "r").
4. **Pointer events carry no modifiers** (`MouseEventData`). Cairn's `HeldKeys` workaround
   is the only way to tell a modified click. Shift-click range selection needs a new row
   in the accelerator table.
5. **Freya has right-click menus, popups, checkboxes, tooltips, drag-and-drop and splitters,
   each with sharp edges:**
   - `ContextMenu` panics if no `ContextMenuViewer` is mounted.
   - `Popup` does not trap focus. The fork added `a11y_modal`, but `Popup` does not use it,
     and Cairn's credential dialog does not either.
   - `Button` takes focus on every click, even with `.focusable(false)`.
   - `DragZone` keeps its drag state per instance, so a row unmounted by virtualization in
     the middle of a drag leaves a stale global payload that the next mouse-up on a
     `DropZone` will drop.
   - Freya does no auto-scroll, neither for drag-and-drop nor for a drag selection.
6. **`freya-testing` can do almost everything the packet needs**, some of it through
   `send_event` rather than a helper: typing, keys with modifiers, Tab focus, clicks, drags,
   wheel, hover, right-click, key-up and IME. The clipboard in tests is the real system
   clipboard unless a test swaps in a fake.

---

## 1. Multi-line text editing (subject + description)

### What exists

| Piece | Where | Notes |
| --- | --- | --- |
| `Input` component | `$F/crates/freya-components/src/input.rs` | The only text field. Single-line by default. |
| `use_editable(content, config) -> UseEditable` | `$F/crates/freya-edit/src/use_editable.rs` | Low-level hook. `Input` is built on it. Docs: "not expected to be used by the common user". |
| `EditableConfig` | `$F/crates/freya-edit/src/config.rs` | `with_indentation`, `with_allow_tabs`, `with_allow_changes`, `with_allow_read_clipboard`, `with_allow_write_clipboard`, `with_select_all_on_double_click`, `with_select_all_on_init`. `Input` sets only the last three. |
| `RopeEditor` (ropey) + `EditorHistory` | `$F/crates/freya-edit/src/rope_editor.rs`, `editor_history.rs` | Undo/redo. `RopeEditor::set_edit_bindings` exists, but `Input` exposes no way to reach it. |
| `EditBindings` / `EditChord` / `EditAction` | `$F/crates/freya-edit/src/config.rs` | Defaults: primary+A/C/X/V/Z, primary+Shift+Z and primary+Y redo. "Primary" matches **Meta or Control on every platform**. |
| `CodeEditor` | `$F/crates/freya-code-editor/` (feature `code-editor`) | Brings in `tree-sitter`, which is a new dependency, and draws per line through a `VirtualScrollView`. Not a fit for a wrapping prose box. |

VERIFIED-IN-SOURCE for each row (paths as given).

### `Input` builder surface (VERIFIED-IN-SOURCE, `input.rs`)

`Input::new(value: impl Into<Writable<String>>)` takes the following, each returning `Self`:

- **Content and feedback:** `.placeholder(impl Into<Cow<'static, str>>)`,
  `.on_validate(EventHandler<InputValidator>)` (it can only reject an edit, and a rejection
  undoes it), `.on_submit(EventHandler<String>)`.
- **Behaviour:** `.mode(InputMode::{Shown, Hidden(char)})`, `.auto_focus(bool)`,
  `.select_all_on_init(bool)`, `.caret(impl Into<Writable<usize>>)` (a two-way cursor in
  UTF-16 code units), `.enabled(bool)`, `.a11y_id(AccessibilityId)`.
- **Size and layout:** `.width(Size)` (defaults to 150 px), `.height(Size)`,
  `.multiline(bool)`, `.min_height(f32)`, `.max_height(f32)`, `.text_align(..)`.
- **Look:** `.filled()`, `.flat()`, `.compact()`, `.expanded()`, `.theme_colors(..)`,
  `.theme_layout(..)`.
- **Extra content:** `.leading(Element)`, `.trailing(Element)`.
- **Key hook:** `.on_pre_key_down(Callback<Event<KeyboardEventData>, bool>)`, with the
  public fallback `Input::key_down_default(e) -> bool`.

No `on_change` exists: the bound `Writable<String>` is written on every accepted edit.

### Behaviour, feature by feature

- **Cursor and selection.** Arrows, Home/End, primary+Home/End, word jumps (Ctrl on
  Linux, Alt on macOS) and line jumps (⌘ on macOS) are all handled, Shift extends, and
  vertical motion follows wrapped visual lines. Double press selects a word, triple press
  a line, quadruple press everything (`TextEditor::press_selection`). Pointer drag-select
  works across lines and scrolls the box to follow the caret (`follow_cursor` in `Input`).
  VERIFIED-IN-SOURCE (`freya-edit/src/text_editor.rs` `CaretGranularity`, `process_key`;
  `freya-edit/src/event.rs`).
- **IME.** `on_ime_preedit` keeps the preedit text in the rope, underlined, outside the
  undo history. A commit arrives as a `KeyDown` of `Key::Character(text)`. VERIFIED-IN-SOURCE
  (`input.rs`, `freya-winit/src/renderer.rs` `WindowEvent::Ime`). Limitation: the IME
  candidate window is anchored to the whole focused node's visible area, not to the caret
  (`set_ime_cursor_area` in `freya-winit/src/window.rs` takes the node's `visible_area()`).
  VERIFIED-IN-SOURCE. How each platform's IME behaves is UNVERIFIED.
- **Undo/redo.** `EditorHistory::new(Duration::from_millis(10))` merges changes less than
  10 ms apart, which in practice means one undo step per keystroke. **Setting the value
  from outside clears the history and the selection:** a render where the bound value
  differs from the editor's `committed_text()` runs `editor.set(..)`,
  `editor_history_mut().clear()` and `clear_selection()`. VERIFIED-IN-SOURCE (`input.rs`).
  So loading the previous message on Amend cannot be undone, and turning Amend off cannot
  restore a draft through undo. Cairn has to keep the draft itself.
- **Clipboard.** Copy, cut and paste go through `freya_clipboard::Clipboard::{get,set}`,
  backed by arboard with `wayland-data-control`, and fall back to X11.
  VERIFIED-IN-SOURCE (`freya-clipboard/Cargo.toml`, `src/clipboard.rs`). Paste inserts
  the clipboard text **verbatim, newlines included, even into a single-line input**
  (the `EditAction::Paste` arm of `process_key`). A single-line field draws only line 1
  (`max_lines(1)`), but the bound value holds every line. VERIFIED-IN-SOURCE. The subject
  field has to clean pasted text itself.
- **Wrapping.** `.multiline(true)` makes the paragraph fill the box's width and wrap. The
  box grows with its text up to `max_height` and then scrolls vertically with a scrollbar.
  A stated `.height(Size::fill())` (or a pixel height) bounds the box, and the text scrolls
  inside it, which suits a box filling a resizable panel. The text is one paragraph, laid
  out again on every keystroke (`EditorLine::SingleParagraph`). That cost is fine at
  commit-message size (inferred). VERIFIED-IN-SOURCE (`input.rs`, the `resolved_height`
  match).
- **Enter.** In a single-line field, Enter calls `on_submit` if one is set and never
  inserts a newline. In a multiline field, `newline = Enter && (Shift held || no
  on_submit)`. With an `on_submit`, plain Enter **and ⌘/Ctrl+Enter** submit. Without one,
  **every** Enter, ⌘Enter included, inserts a newline, because the editor's Enter arm does
  not look at modifiers. VERIFIED-IN-SOURCE (`input.rs` `on_key_down`,
  `text_editor.rs` `process_key`). Freya's own test `input_multiline_test`
  (`$F/crates/freya-components/tests/input.rs`) pins Shift+Enter.
- **Escape** asks for unfocus (`request_unfocus`) and is **not** consumed, so a `Popup`
  or `Menu` global Escape handler also fires. VERIFIED-IN-SOURCE.
- **Tab** returns `false` from `key_down_default` (the editor skips it, nothing is
  prevented), so the integration's global handler moves focus (§2). Tabs cannot be typed
  (`allow_tabs` defaults to false and `Input` does not change it). VERIFIED-IN-SOURCE.
- **Primary+letter that is not an edit binding types the letter.** `process_key`'s
  `Key::Character` arm inserts whatever character arrives, whatever the modifiers, and
  `map_winit_key` passes winit's logical character through. So ⌘R on macOS inserts "r".
  VERIFIED-IN-SOURCE (`text_editor.rs`, `freya-winit/src/winit_mappings.rs`). The Linux
  case depends on what winit reports as the logical key for Ctrl+letter: UNVERIFIED.
- **Placeholder** is drawn only while the value is empty and no preedit is active. A
  single string, but it may contain `\n` (inferred). VERIFIED-IN-SOURCE.
- **Character counting / 72-column guide:** none. Cairn reads the bound value. Text style
  (font family, size) is inherited from ancestors, because the paragraph sets only colour
  and alignment (inferred from `input.rs`; Cairn sets `font_family` on labels the same way).
- **Setting the text from code (Amend).** Write the bound `State<String>`. The editor
  syncs on the next render, with the history and selection cleared as above. The cursor
  is not moved unless it would land past the end. `RopeEditor::set` compares a UTF-16
  cursor with a byte length (`text.len()`), a small unit mismatch with non-ASCII text.
  VERIFIED-IN-SOURCE (`rope_editor.rs` `fn set`). To put the cursor at the end, bind
  `.caret(..)` and write the length in UTF-16 units. `caret`'s own docs describe this use.
- **Programmatic focus.** Give the `Input` an id from `use_a11y()`, then call
  `id.request_focus()` (`AccessibilityIdExt`). VERIFIED-IN-SOURCE
  (`freya-core/src/accessibility/focus.rs`).

### How Cairn takes text today

- `crates/cairn-ui/src/credential_prompt.rs` (`CredentialPrompt`):
  `Popup::new().width(..).on_close_request(..)` containing `PopupTitle`, a `PopupContent`
  with labels and `Input::new(typed).width(Size::fill()).auto_focus(true).mode(..).on_submit(..)`,
  and `PopupButtons` holding `Button`s. The typed `String` is moved out with
  `std::mem::take` into `on_submit`. VERIFIED-IN-SOURCE.
- Filter fields: `Input::new(self.filter.clone()).placeholder(FILTER_PLACEHOLDER).compact().width(Size::fill())`
  in `crates/cairn-ui/src/local_changes.rs`, `changes_list.rs` and `sidebar.rs`. None of
  them sets `on_pre_key_down`. VERIFIED-IN-SOURCE.
- **Reuse potential:** the `Popup` + `Input` + `Button` arrangement carries straight over
  to a stash-message dialog. The commit box is new: two `Input`s, a single-line subject and
  a multiline description with `.height(Size::fill())`, with no `on_submit` on the
  description and an `on_pre_key_down` that claims the commit chord through the
  accelerator table.

Sketch in the builder API (shapes verified; Cairn's `Action::Commit` does not exist yet):

```rust
let mut description = use_state(String::new);
Input::new(description)
    .multiline(true)
    .height(Size::fill())
    .width(Size::fill())
    .placeholder("Description")
    .on_pre_key_down(Callback::new(move |e: Event<KeyboardEventData>| {
        if accelerators::resolve_key(&e, Scope::Window) == Some(Action::Commit) {
            e.stop_propagation();
            e.prevent_default();
            /* report the commit intent */
            return false; // the editor does not insert the newline
        }
        Input::key_down_default(e)
    }))
```

---

## 2. Keyboard focus and the accelerator table

### How a key is dispatched (VERIFIED-IN-SOURCE)

- A key event targets the **focused** node, as a potential event with no location
  (`ragnarok/src/measurement.rs` `measure_potential_events`), and bubbles to its ancestors
  until `stop_propagation()`.
- Every source event also emits its **global** variant (`GlobalKeyDown`/`GlobalKeyUp`) to
  every listener (`measure_source_global_events`). Global events fire after the
  non-global ones (`EventName::priority` in `freya-core/src/events/name.rs`). Several
  global listeners fire in document pre-order (`ragnarok/src/measurer.rs`), so an
  **ancestor's global listener runs before a descendant's**.
- `prevent_default()` on an event drops the events it can cancel (`get_cancellable_events`):
  `KeyDown` cancels `GlobalKeyDown`, and `KeyUp` cancels `GlobalKeyUp`
  (`ragnarok/src/executor.rs`, `freya-core/src/runner.rs` `handle_event` returns the
  default flag). `stop_propagation()` stops bubbling only. It does **not** stop the global
  event.
- The root `integration` component (`$F/crates/freya-components/src/integration.rs`)
  listens to `on_global_key_down`. Tab moves focus forward and Shift+Tab backward
  (`OutsideGroup`). ArrowUp/ArrowDown with no modifiers move focus inside an `a11y_member_of`
  group. `freya-testing` wraps the app in the same `integration` (`TestingRunner::new`).

### What a focused `Input` does to keys (VERIFIED-IN-SOURCE, `Input::key_down_default`)

| Key | Editor processes it | Bubbles | `GlobalKeyDown` fires |
| --- | --- | --- | --- |
| Enter (any modifiers) | single-line: submits. multiline: see §1 | yes | **yes** |
| Escape | unfocus | yes | **yes** |
| Shift | marks shift for drag-extend | yes | **yes** |
| Tab | no | yes | **yes** (the integration moves focus) |
| everything else (letters, Space, arrows, F5, Control, Meta, Alt, ⌘↑ …) | yes | **no** | **no** |

Consequences for Cairn, all (inferred) from the table and `crates/cairn-app/src/window.rs`:

- While a text field has focus, every `Scope::Window` chord except one completed by Enter
  is unheard. Refresh (F5 on Linux, ⌘R on macOS) and ⌘⌥1/⌘⌥2 do nothing, and on macOS
  ⌘R types "r".
- `HeldKeys` is fed only by the window's `on_global_key_down`/`on_global_key_up`. A
  Control or Meta pressed while a field has focus is never recorded as held. Its release
  is heard, because `Input`'s `on_key_up` only stops propagation. So a ⌘/Ctrl-click on a
  row made while a filter field (or a future commit box) has focus resolves as a plain
  press. The click only takes focus from the field on its mouse-up
  (`on_global_pointer_press`), after the modifier went unheard.
- ⌘Enter **is** heard globally, and by any ancestor's `on_key_down`, such as the Local
  Changes pane's `Scope::Detail` handler in `crates/cairn-app/src/local_changes_pane.rs`.
  But in a multiline field without `on_submit`, the field's own `KeyDown` has already
  inserted the newline, because non-global events fire first.
- The cure is local: give each Cairn `Input` an `on_pre_key_down` that lets through, without
  claiming, any key `accelerators::is_chord` recognises and any lone modifier key. Return
  `false` and call no `prevent_default`, so the global event survives. Everything else goes
  to `Input::key_down_default`. The handler names no modifier, because the table answers.
  A shared helper in `cairn-ui` would keep that rule in one place.

### Focus APIs (VERIFIED-IN-SOURCE, `freya-core/src/accessibility/`)

- `use_a11y() -> AccessibilityId` and `AccessibilityId::new_unique()`.
- `AccessibilityIdExt::{is_focused, request_focus, request_unfocus}`.
- `use_focus(id) -> Memo<Focus>`, where `Focus` is one of `Not`, `Pointer`, `Keyboard`.
- Element setters: `.a11y_id`, `.a11y_focusable(Focusable | bool)`, `.a11y_auto_focus(bool)`
  (applied when the node mounts), `.a11y_member_of(id)` (an arrow-key group), `.a11y_role`,
  `.a11y_alt`, `.a11y_builder(|node| ..)`, and the fork's `.a11y_modal(bool)` (§4).
- **Tab order is document (depth-first) order** of focusable nodes, wrapping at both ends
  (`AccessibilityTree::focus_node_with_strategy`). There is no tabindex.
- `request_focus` on a node that is **not** focusable still succeeds: the only checks are
  that the id is in the tree's map, which holds every node, and that it is inside the
  active modal. VERIFIED-IN-SOURCE (`accessibility/tree.rs`).
- Cairn's pattern today: each list (`history_list.rs`, `changes_list.rs`,
  `local_changes.rs`, `commit_tab.rs`, `diff_view.rs`) is one focusable `rect` with an
  `on_key_down` that returns early on `accelerators::is_chord(&e)` and otherwise handles
  ↑/↓ with `stop_propagation()`. Rows are not focusable and take focus for their list with
  `list_id.request_focus()` in `on_press`. VERIFIED-IN-SOURCE.

### The accelerator table (VERIFIED-IN-SOURCE, `crates/cairn-ui/src/accelerators.rs`)

- `Action` lists 11 actions today, `chord(action, Os) -> Option<Chord>` and
  `heard_in(action) -> Scope::{Window, Detail}`.
- A key press is matched with `resolve_key(&KeyboardEventData, Scope)` or `is_chord(&..)`.
- A pointer press is matched with `resolve_press_on(Os, held)` through
  `HeldKeys::{heard, press, press_on}`. `HeldKeys` answers an `Action` and never says
  which key is down.
- `Trigger::Press` exists for pointer chords. Only `ExtendSelection` (command + press)
  uses it, and Shift+command is deliberately another chord (the test
  `a_press_resolves_against_the_keys_the_window_heard`). A Shift-click range select or a
  Ctrl-click toggle in the file lists needs new rows here.
- Space and Enter on a focused list are free. A row's `on_press` fires for Space/Enter only
  when the row itself is the focused node or an ancestor of it
  (`KeyboardEventExt::is_press_event` in `focus.rs`), and Cairn's rows are neither.

---

## 3. Checkbox, toggle, buttons, dropdown and context menus, tooltips

All VERIFIED-IN-SOURCE under `$F/crates/freya-components/src/` unless marked.

- **`Checkbox`** (`checkbox.rs`): `Checkbox::new().selected(bool).size(f32).theme(..)`.
  It only draws: no press handler, no `on_change`. The documented pattern wraps it in
  `Tile::new().on_select(|_| ..).child(Checkbox::new().selected(..)).leading("label")`
  (`tile.rs`). The checkbox is focusable, and its `on_key_down` stops every key except the
  press keys, which bubble to `Tile`'s `on_press`. Its a11y role is `CheckBox`, but it sets
  **no toggled state** (no `accesskit::Toggled`), so a screen reader cannot tell checked
  from unchecked. `a11y_builder` on a wrapper can add one.
- **`Switch`** (`switch.rs`): `Switch::new().toggled(impl Into<Readable<bool>>).on_toggle(EventHandler<()>).enabled(..)`.
  It does set `Toggled`.
- **`Button`** (`button.rs`): `Button::new().on_press(..).on_secondary_down(..).on_pointer_down(..).enabled(bool).focusable(bool)`
  with `.filled()`, `.outline()`, `.flat()`, `.compact()`, `.expanded()` and themes. Its
  `on_all_press` handler calls **`a11y_id.request_focus()` on every press, whatever
  `.focusable(false)` says** (see §2). A floating Stage or Discard button would take focus
  from the file list or the message box. A plain `rect().on_press(..)` does not do this.
  A disabled button attaches no handlers and shows `CursorIcon::NotAllowed`. There is no
  auto-focus option, so a dialog's default button needs `a11y_auto_focus` on a wrapper.
- **Dropdown:**
  - `Select::new().selected_item(Element).children(MenuItem…)` with
    `.placement(SelectPlacement)`, `.open_up()`, `.open_down()` (`select.rs`).
  - `Menu::new().on_close(..).on_escape(..).min_width(..).child(MenuButton|MenuItem|SubMenu)`
    in an Overlay layer (`menu.rs`).
  - `MenuItem::new().on_press(..).selected(bool).enabled(bool)`. A disabled item is drawn
    at 50% opacity.
  - `Attached::new(inner).bottom()/…` anchors an overlay to an element, clamped to the
    window (`attached.rs`).
- **Menu keyboard and layout:** `MenuContainer` puts its items in an `a11y_member_of`
  group, so arrows move between them through the integration (§2), and it pulls itself back
  inside the window when it would spill past an edge (`overflow_offset`, with
  `EDGE_MARGIN`). There is no separator component and no column for showing an item's
  shortcut. Nothing focuses the menu when it opens.
- **Context menu** (`context_menu.rs`): mount `ContextMenuViewer::new()` once, then call
  `ContextMenu::open_from_down(Menu::new().child(..))` from an `on_secondary_down`. It opens
  at the last global pointer-move position. It closes on a press outside it, on Escape,
  and when the window loses focus. **`ContextMenu::get()` panics (`expect`) when no
  `ContextMenuViewer` is mounted.** Cairn mounts none today. Choosing an item does not
  close the menu by itself: the item's handler calls `ContextMenu::close()`, as
  `color_picker.rs` does. Nothing opens a context menu from the keyboard (Menu key,
  Shift+F10).
- **Right-click on a row:** `.on_secondary_down(..)` on any element fires on a Right
  `MouseDown` (`freya-core/src/elements/extensions.rs` `EventHandlersExt::on_secondary_down`).
  There is no secondary-press-on-release variant. `.on_all_press` fires on release for any
  button.
- **Tooltip** (`tooltip.rs`): `TooltipContainer::new(Tooltip::new_text(..)).position(AttachedPosition).delay(Duration)`
  (500 ms by default) `.child(..)`. It flips to the other side when the preferred side has
  no room, and it is dismissed on pointer-out and pointer-down. Cairn already uses it in
  `crates/cairn-ui/src/diff_header.rs` around a compact `Button`.

---

## 4. Modal dialogs and overlays

- **`Popup`** (`popup.rs`, VERIFIED-IN-SOURCE):
  - Builder: `Popup::new().on_close_request(EventHandler<()>).child(PopupTitle::new(..)).child(PopupContent::new().child(..)).child(PopupButtons::new().child(..))`.
  - Layer and placement: drawn in `Layer::Overlay` at a global position. A backdrop rect
    covers the window (`Size::window_percent(100.)`) and calls `on_close_request` when
    pressed. The content is centred.
  - Escape: an `on_global_key_down` on the dialog box calls `on_close_request` and then
    `prevent_default()` (`close_on_escape_key` is true, and no setter for it is visible).
  - Animation: it animates in, and animates out only when it stays mounted with no children.
  - It does **not** set `a11y_modal`.
- **Pointer blocking** (VERIFIED-IN-SOURCE, `ragnarok/src/measurement.rs`
  `measure_emmitable_events`): once a node with a non-transparent background is hit, only
  that node's ancestors receive bubbling events. The visible backdrop therefore stops
  clicks and wheel reaching the window behind it.
- **Focus containment:** the fork's `.a11y_modal(true)` (`extensions.rs`; logic in
  `AccessibilityTree::active_modal`/`inside_modal`; test `$F/crates/freya-core/tests/modal_focus.rs`)
  keeps Tab and explicit `request_focus` inside the subtree while it is mounted, and pulls
  focus into it. Neither `Popup` nor Cairn's `CredentialPrompt` uses it. With a dialog open
  today, Tab reaches controls behind the backdrop (inferred). Keys then go to whatever
  behind it has focus: Space on a file list, once it stages, would act behind a
  confirmation dialog.
- **Window chords stay live under a dialog** (inferred): the window's
  `on_global_key_down` in `crates/cairn-app/src/window.rs` is an ancestor of the dialog,
  so it runs **before** the dialog's global handler and cannot be pre-empted. F5 would
  refresh behind an open confirmation unless the window handler checks for an open dialog.
- **How Cairn draws its dialog:** `crates/cairn-app/src/window.rs` `dialog(..)` builds
  `CredentialPrompt` and the window adds it as its last child
  (`.maybe_child(prompt.map(|p| dialog(..)))`), so Popup's own overlay layer floats it.
  Submit and cancel both clear `view.prompt`. Tests: `crates/cairn-ui/tests/credential_prompt.rs`
  covers Enter, Escape, Cancel and a press on the backdrop.
- **What a destructive confirmation needs on top:**
  - a `rect().a11y_modal(true)` around the content;
  - `a11y_auto_focus` on the safe default button;
  - window chords made inert while the dialog is up;
  - the prompt text carried into `cairn_model::Confirmed`.

---

## 5. Pointer work over rows in a `VirtualScrollView`

All VERIFIED-IN-SOURCE unless marked.

- **The pointer events available** (`EventHandlersExt` in `freya-core/src/elements/extensions.rs`):
  - Mouse: `on_mouse_down`, `on_mouse_up`, `on_mouse_move`.
  - Pointer (mouse or touch): `on_pointer_down`, `on_pointer_press` (on release),
    `on_pointer_move`, `on_pointer_enter`, `on_pointer_leave`, `on_pointer_over`,
    `on_pointer_out`.
  - Global: `on_global_pointer_move`, `on_global_pointer_down`, `on_global_pointer_press`.
  - Capture: `on_capture_global_pointer_move`, `on_capture_global_pointer_press`. These
    run first and cancel the matching ordinary events.
  - Composite: `on_press` (left button, touch, or Space/Enter while focused),
    `on_all_press`, `on_secondary_down`, `on_focus_press`.
  - Other: `on_wheel`, `on_sized`, `on_visible`, `on_hidden`.
  - Element options: `.interactive(false)` stops a node receiving events, and
    `.cursor(CursorIcon)` sets the pointer shape.
- **Hover per row:** `on_pointer_enter`/`on_pointer_leave`, or `on_pointer_over`/
  `on_pointer_out`. `PointerEnter` is delivered once, to the deepest listener only
  (`is_emitted_once` → `is_exclusive_enter`), while `PointerOver` reaches every listening
  ancestor. So a row whose child also listens for enter should watch over/out, as `Button`
  does.
- **Keeping hover right in a virtual list.** Rows unmount as they scroll out, and an
  unmounted row never gets its leave event. Keep the hovered index (or hunk) in the
  list's state rather than in each row. After a **wheel** event, winit sends a synthetic
  mouse move on the next layout (`send_mouse_move_on_next_layout`,
  `freya-winit/src/window.rs`). That happens only after a wheel: after a data change, such
  as staging that moves rows, hover stays stale until the pointer moves.
- **Fork-style floating Stage/Discard buttons:** draw them in the hunk-header row when
  the hovered hunk is the row's, or as an overlay positioned inside the row (inferred).
  Avoid `Button` (§3, it takes focus).
- **Down, move and up across rows (drag selection):**
  - Pointer *move* events never carry the button (`button: None` in `CursorMoved`,
    `freya-winit/src/renderer.rs`). "Button held" has to be tracked from down to up.
  - The pattern that survives virtualization is the one `Input` uses: `on_pointer_down` on
    the list container (or a row) starts the drag; `on_global_pointer_move` and
    `on_global_pointer_press` **on the list container**, which never unmounts, track it and
    end it; the row index comes from geometry: `(global_y − viewport.min_y + scroll) /
    item_size`. Rows coming and going then do not matter (inferred).
  - On `CursorLeft`, winit keeps the last position while a button is held (renderer).
    Whether moves outside the window keep arriving during an implicit grab depends on the
    platform: UNVERIFIED.
- **Auto-scroll at the edge:** none built in. `ScrollController::scroll_to_y(i32)`,
  `scroll_to_offset(offset, size, Direction)` and `scroll_to_item(Area)` exist
  (`scrollviews/use_scroll_controller.rs`). A timer is needed to scroll while the pointer
  stands still past the edge. `spawn(future)` is available (`freya-core/src/lifecycle/task.rs`),
  but a timer future is not re-exported under Cairn's features:
  - Freya's own components use `async_io::Timer`, and Cairn depends on neither;
  - `freya-sdk`'s `use_timeout` (feature `sdk`) is a one-shot latch, not a ticker;
  - the other option is to scroll only on pointer moves, so a still pointer stops the
    scroll.

  Any of these is a dependency or feature decision. Separately, Cairn's UI-thread guard
  bans `sleep`.
- **Click with modifiers:** `MouseEventData { global_location, element_location, button }`
  has no modifiers, and `PointerEventData` is `Mouse | Touch` (`freya-core/src/events/data.rs`),
  which confirms the toolkit note in CLAUDE.md. Cairn resolves modified clicks through
  `HeldKeys` (§2), which has the focused-field hole described there.
- **Double-click:** `EventsCombos::pressed(location) -> PressEventType::{Single, Double, Triple, Quadruple}`
  (`freya-core/src/events_combos.rs`). Fixed thresholds: 500 ms and 5 px, not the
  desktop's setting. **One combo state for the whole app**, shared with every text editor's
  presses, which register through `EditableEvent::Down`. Call it from `on_pointer_down`,
  where its doc example does. `EventsCombos::moved` breaks a combo when the pointer drags
  away.
- **`VirtualScrollView`** (`scrollviews/virtual_scrollview.rs`):
  - Builder: `new_with_data_controlled(data, builder, controller)`, then `.length(usize)`,
    `.item_size(ItemSize)`, `.scroll_with_arrows(bool)`, `.drag_scrolling(bool)` (touch-style
    dragging of the content), `.show_scrollbar`, `.direction`.
  - `ItemSize::Dynamic(callback)` walks from index 0 on every frame (`visible_range`,
    `total_size`), so fixed-height rows are the cheap path.

---

## 6. Drag and drop between lists

VERIFIED-IN-SOURCE in `$F/crates/freya-components/src/drag_drop.rs`.

- **API:**
  - `DragZone::new(data: T).drag_element(Element).drag_threshold(f64)` (4 px by default)
    `.show_while_dragging(bool)` `.enabled(bool)` `.child(..)`.
  - `DropZone::new(EventHandler<T>).on_drag_over(EventHandler<bool>).child(..)`.
  - `use_drag::<T>() -> State<Option<T>>`, the global payload of type `T`.
  - Both zones accept layout setters.
- **Mechanics:**
  - A left `pointer_down` on the zone arms it; a global move past the threshold sets the
    global payload and draws the drag element in `Layer::Overlay` with
    `.interactive(false)`; a global pointer-press clears it.
  - `DropZone`'s `on_mouse_up` reads the payload and calls `on_drop`. Mouse-up is not
    global, so it fires before `DragZone`'s global clear.
  - The payload is any `Clone + PartialEq` type, so a `Vec` of chosen paths works.
- **Hazard under virtualization** (inferred): the drag phase lives in each `DragZone`'s
  own `use_state`, while the payload is global. If the dragged row unmounts in the middle
  of a drag (wheel scroll, a list redrawn by a status update), its global listeners go with
  it and the **payload is never cleared**. The next mouse-up over any `DropZone<T>` then
  drops a stale payload. A single `DragZone` wrapping the whole list, with the current
  selection as its data, avoids this.
- **Missing:**
  - auto-scroll while dragging;
  - Escape to cancel;
  - drags out to the operating system. Inbound file drops exist (`on_file_drop`,
    `on_global_file_hover`), which staging does not need;
  - drop-target hit-testing finer than "inside this zone".

---

## 7. Splitters and resizable panes

- **`ResizableContainer`** (`$F/crates/freya-components/src/resizable_container.rs`,
  VERIFIED-IN-SOURCE):
  - `ResizableContainer::new().direction(Direction).handle_size(f32).panel(ResizablePanel)`.
  - `.controller(Writable<ResizableContext>)` drives it programmatically: `reset`,
    `apply_resize`.
  - `ResizablePanel::new(PanelSize::px|percent).min_size(f32)` (in the panel's own units)
    `.max_size(..)` `.min_pixels(f32)` (the floor when the container shrinks)
    `.order(..)` `.on_resized(EventHandler<f32>)` (on a drag only)
    `.on_collapse(EventHandler<()>)` (an over-drag past `COLLAPSE_OVER_DRAG` = 24 px; the
    caller decides what collapsing means).
  - `ResizableContext::HANDLE_SIZE` is 4.0.
- **Cairn already uses it in four places:** the sidebar split and the list/detail split
  in `crates/cairn-app/src/window.rs` (`beside`, `split`), Unstaged over Staged in
  `crates/cairn-ui/src/local_changes.rs`, and list beside diff in
  `crates/cairn-app/src/local_changes_pane.rs`. Its pattern: read the stored size with
  `peek()` at layout, write it in `on_resized`, and turn pixels into a share with
  `share_of`. A commit box under the two lists is a third panel or a nested vertical
  container in the same pattern. Paired with a multiline `Input` at
  `.height(Size::fill())` (§1), the box fills the panel and scrolls inside it.

---

## 8. `freya-testing` headless runner

Each item below is VERIFIED-IN-SOURCE in `$F/crates/freya-testing/src/lib.rs` unless
marked.

- **Construction:** `launch_test(app)` gives a 500×500 runner.
  `TestingRunner::new(app, size, hook: FnOnce(&mut Runner) -> T, scale)` takes a hook,
  which tests use to `provide_root_context` (Cairn does this 25 times). The app is wrapped
  in `integration`, so Tab and arrow focus movement work in tests.
- **Typing:** `write_text(s)` sends **one** `KeyDown` of `Key::Character(s)`, so the whole
  string goes in as one insert, not as a sequence of keystrokes. `press_key(Key)` and
  `press_key_with_modifiers(Key, Modifiers)` send key-downs only. A key-up needs
  `send_event(PlatformEvent::Keyboard { name: KeyboardEventName::KeyUp, .. })`, as
  `crates/cairn-app/src/window.rs`'s tests do.
- **Focus:** click the field (`click_cursor`), or mount with `auto_focus` and call
  `sync_and_update()` a few times, as the credential tests do. Tab: `press_key(NamedKey::Tab)`.
- **Pointer:**
  - `click_cursor`, `press_cursor`, `release_cursor` and `move_cursor` all send the left
    button. A drag is press, then moves, then release; Cairn's splitter tests do this.
  - Hover: `move_cursor`.
  - Wheel: `scroll` and `scroll_lines`. Each sends a mouse move afterwards to refresh hover.
  - **Right-click has no helper.** Send
    `send_event(PlatformEvent::Mouse { name: MouseDown/MouseUp, button: Some(MouseButton::Right), .. })`,
    as `$F/crates/freya-components/tests/context_menu.rs` does.
  - **Double-click:** two `click_cursor`s inside 500 ms of wall-clock time
    (`EventsCombos` uses `Instant::now`), so the test is timing-dependent (inferred).
- **IME:** `send_event(PlatformEvent::ImePreedit { .. })`.
- **Time:** `poll(step, duration)` and `poll_n(step, n)` sleep for real time, which
  matters for animations and the 500 ms tooltip delay.
- **Finding things:** `find` and `find_many` with downcasts (`Label::try_downcast`,
  `Paragraph::try_downcast`; a paragraph's `cursor_index` shows the caret);
  `TestingNode::layout()`; `cursor_icon()`; `render_to_file` for an image.
- **Clipboard:** the runner installs the **real** arboard clipboard (`ClipboardContext::new()`).
  Headless, that is `NotAvailable`; on a developer's desktop a copy in a test overwrites
  their clipboard. The hook runs after the clipboard is installed, and a root context is
  replaced by `TypeId`, so a test can install a fake: implement
  `freya::clipboard::ClipboardProvider` and provide
  `State::create(Some(Box::new(fake) as Box<dyn ClipboardProvider>))`. VERIFIED-IN-SOURCE
  (`freya-core/src/runner.rs` `provide_root_context`). A test using this has not been
  written.
- **What Cairn's tests do today** (`crates/cairn-ui/tests/*.rs`, `crates/cairn-app/src/*tests*.rs`):
  - mostly `sync_and_update`, `find`/`find_many`, `scroll`, `click_cursor` and the
    `provide_root_context` hook;
  - `write_text` in the credential and filter tests;
  - `press_key_with_modifiers` with chords taken from the table (`Chord::key_press`,
    `Chord::press_hold`);
  - drags of the splitters (`local_changes.rs` `the_splitter_between_the_lists_drags`);
  - wheel bursts through `send_event` in `crates/cairn-app/src/window_check.rs`;
  - none right-clicks, drags rows, or uses the clipboard.

---

## Gaps: what Cairn would have to build itself

1. **A key policy for its text fields.** A shared `on_pre_key_down` that hands accelerator
   chords and lone modifier keys through to the window unclaimed (closing the `HeldKeys`
   and Window-chord hole in §2, which exists today with the filter fields), claims
   the commit chord without the newline, and stops primary+letter chords from typing
   their letter.
2. **The commit box's own logic:**
   - Sanitising the subject: a paste can put newlines into a single-line field.
   - Optionally splitting a pasted "subject\n\nbody".
   - A subject-length indicator or 72-column hint.
   - Keeping the draft across Amend on and off, since an external `set` clears undo.
   - Putting the cursor where it should go after loading a message, via `.caret(..)`.
3. **Multi-selection in the file lists:** Ctrl-toggle and Shift-range presses as new
   `Action`s with `Trigger::Press` in `accelerators.rs`; the selection state; keyboard
   extension with Shift+↑/↓.
4. **Drag selection across rows and over diff lines** (Fork's line-level staging),
   computed from list geometry so rows can unmount, plus **edge auto-scroll**, which needs
   a timer source (a dependency or feature decision) or a scroll-only-on-move compromise.
5. **Hover-driven floating Stage/Discard controls** over hunks: hover kept by index in the
   list's state; non-focus-stealing pressables instead of `Button`; a decision about stale
   hover after the data changes.
6. **Context menus:** mount one `ContextMenuViewer` at the window root (otherwise
   `ContextMenu::get` panics), close the menu from each item, build separators and a
   shortcut-hint column if wanted, and add keyboard invocation.
7. **Drag-and-drop between Unstaged and Staged,** if wanted: one `DragZone` per list
   rather than per row (the stale-payload hazard), carrying the selection; drop feedback;
   auto-scroll; Escape to cancel.
8. **A confirmation-dialog wrapper** for destructive operations, around `Popup`:
   `a11y_modal`, auto-focus on the safe button, window chords inert while it is open,
   honest prompt text carried into `Confirmed`. The stash dialog reuses it, with an
   `Input` and `Tile`+`Checkbox` rows for Keep index and Include untracked.
9. **Checkbox accessibility:** `Checkbox` reports no toggled state. Add it with
   `a11y_builder` or use `Switch`.
10. **Test support:** a fake clipboard provider; small helpers for right-click and key-up
    built on `send_event`; double-click tests kept inside the 500 ms window.
