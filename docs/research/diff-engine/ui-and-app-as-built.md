# diff-engine research: the UI and application layers as built

Evidence for planning the `diff-engine` packet. As-built, read from the working
tree at `main` (`bf93a4e`) and from the vendored Freya checkout the workspace
pins. Facts, not design: the "Open questions for the planner" section at the end
is the only place anything is proposed. Anchors are file paths and exported
symbols; nothing here cites a line number.

Companion: `engine-and-worker-as-built.md` in this directory covers `cairn-git`
and `crates/cairn-app/src/worker/`; this file covers everything the UI thread
draws and the seam it draws it through.

---

## 1. `crates/cairn-ui/` — the component library

### 1.1 Crate shape

- Manifest `crates/cairn-ui/Cargo.toml`: dependencies are `cairn-model` and
  `freya` with the `engine` feature (Skia, for the painted graph column);
  dev-dependency `freya-testing`. Nothing else, and the dependency allowlist
  guard pins that (section 5).
- **There is no `crates/cairn-ui/CLAUDE.md`** and no `crates/cairn-app/CLAUDE.md`.
  The only local conventions are the root `CLAUDE.md`'s.
- `crates/cairn-ui/src/lib.rs` exports: `CommitRow`, `HistoryHeader`,
  `AUTHOR_WIDTH`, `COLUMN_GAP`, `DATE_WIDTH`, `ID_WIDTH`, `ROW_FONT_SIZE`,
  `ROW_PADDING` (from `commit_row`); `CredentialPrompt` (from
  `credential_prompt`); `LANE_WIDTH`, `MAX_DRAWN_LANES`, `ROW_HEIGHT`,
  `graph_width` plus the whole `graph_geometry` module as `pub mod`;
  `HistoryList`, `PREFETCH_ROWS`, `RowRender` (from `history_list`); and
  `pub mod lane_palette`. Private modules: `date_text`, `graph_cell`.
- Modules are named for behaviour (`commit_row`, `graph_cell`, `lane_palette`,
  `date_text`), per the root convention.

### 1.2 Public components, props and handlers

Every component follows the same Freya 0.5-rc builder shape: a `struct` with
private fields, `new(..)` plus chained setters, `impl KeyExt` (a `key: DiffKey`
field), and either `impl ComponentOwned` (`fn render(self)`) or `impl Component`
(`fn render(&self)`, needed when the render uses hooks such as `use_state`).
`PartialEq`/`Debug` are hand-written wherever an `EventHandler` or `Callback`
field exists, because those never compare equal.

| Component | File | Constructor / props | Handlers (`EventHandler<T>`) | Render trait |
| --- | --- | --- | --- | --- |
| `CommitRow` | `crates/cairn-ui/src/commit_row.rs` | `new(commit: CommitSummary, graph: GraphRow, lanes: usize)`, `.selected(bool)` | none — "No handler, so equal content compares equal and Freya skips re-rendering" | `ComponentOwned` |
| `HistoryHeader` | `crates/cairn-ui/src/commit_row.rs` | `new()` / `Default` | none | `ComponentOwned` |
| `HistoryList` | `crates/cairn-ui/src/history_list.rs` | `new(rows: State<Vec<HistoryRow>>, row: impl Fn(RowRender) -> Element)`, `.lanes(usize)`, `.selected(Option<RowId>)` | `.on_select(EventHandler<RowId>)`, `.on_reach_end(EventHandler<()>)` | `Component` (uses `use_a11y`, `use_focus`, `use_scroll_controller`, `use_state`) |
| `CredentialPrompt` | `crates/cairn-ui/src/credential_prompt.rs` | `new(remote: impl Into<String>, text: impl Into<String>)` | `.on_submit(EventHandler<String>)`, `.on_cancel(EventHandler<()>)` | `Component` (uses `use_state` for the typed buffer) |

There is **no toolbar component, no tab component, no dialog component other
than the credential prompt, and no detail pane** in `cairn-ui`. The title bar,
banner, notice and fetch button are private functions in `cairn-app`'s
`window.rs` (section 2).

`RowRender` (`crates/cairn-ui/src/history_list.rs`) is the value the per-row
builder receives: `{ row: HistoryRow, selected: bool, lanes: usize }`. The
list itself never matches `RowContent`; the builder the caller passes does, so
the exhaustiveness obligation lands in `cairn-app` (see section 2.5 and 5).

`CommitRow`'s layout: an outer horizontal `rect()` with `Content::Flex`,
`Size::fill()` width, `ROW_HEIGHT` height, `Gaps::new(0, ROW_PADDING, 0,
ROW_PADDING)`, `spacing(COLUMN_GAP)`; `.maybe(self.selected, |el|
el.background(highlight))` where `highlight` is the theme's
`surface_secondary`. Children: a flex-1 cell holding `graph_cell(..)` (a
`canvas`) and the subject `label()` with `max_lines(1)` and
`TextOverflow::Ellipsis`; then author (`AUTHOR_WIDTH` 150), short id
(`ID_WIDTH` 72, `commit.id.short()`), date (`DATE_WIDTH` 132,
`date_text::utc_minutes`). Column gap 10, row padding 10, `ROW_FONT_SIZE` 13.

`HistoryHeader` draws the four captions "Graph and subject", "Author",
"Commit", "Date (UTC)" at the same widths, with a 1px bottom border in the
theme's `border` colour and text in `text_placeholder`.

### 1.3 How `VirtualScrollView` is used

`HistoryList::render` (`crates/cairn-ui/src/history_list.rs`):

- Builds a private `ListData` (rows handle, lanes, selected, `length`, the row
  `Callback`, both handlers, the list's `AccessibilityId`, and a `cursor:
  State<usize>` hint) and calls
  `VirtualScrollView::new_with_data_controlled(data, build_row, controller)`
  with `.length(length)`, `.item_size(ROW_HEIGHT)` (a fixed 26px),
  `.scroll_with_arrows(false)` and `.expanded()`.
- `ListData` implements `PartialEq` by hand over everything but the handlers,
  because "Data captured inside the builder closure is invisible to
  `VirtualScrollView`'s diffing" — the comparable data must be passed as the
  `D` parameter, not captured.
- The one per-render read proportional to the history is
  `self.rows.read().len()`; the row vector is a `State` HANDLE the list
  subscribes to, never a copy.
- `build_row(item: VirtualItem, data: &ListData) -> Element` reads
  `rows.get(item.index)`; if length and vector disagree for a frame it draws an
  empty `rect()` of `item.size`. Otherwise it wraps the caller-built element in
  a `rect()` keyed by `RowId` (`.key(id)` — "a positional key lets a reused slot
  paint the previous row's graph"), with `.on_press` (request focus for the
  list, set the cursor hint, call `on_select`) and, for every row within
  `PREFETCH_ROWS` (24) of the end, `.on_visible(.. on_reach_end ..)`.
- The wrapper `rect()` carries `a11y_id`, `a11y_focusable(true)`,
  `a11y_auto_focus(true)`, `AccessibilityRole::List`, `.on_key_down(..)` and a
  1px `border_focus` border only while `use_focus(list_id)()` is
  `Focus::Keyboard`.
- The scroll controller is created with `use_scroll_controller(ScrollConfig::default)`
  and shared with the keyboard handler, which reveals a selection by offset:
  `controller.scroll_to_offset(next as f32 * ROW_HEIGHT, ROW_HEIGHT, Direction::Vertical)`.

### 1.4 Selection: representation and reporting

- Selection is **by identity**: `Option<RowId>` where `RowId` is
  `cairn_model::RowId` (`Commit(Oid)` today, plus a `#[cfg(test)]`
  `NotACommit`). `HistoryList::selected(Option<RowId>)` is an input prop; the
  list never owns the selection. `HistoryRow::id()` produces it.
- It is reported through `on_select: EventHandler<RowId>` on a click (in
  `build_row`) and on every key the list owns (in `HistoryList::keyboard`).
  Keys: `ArrowDown`, `ArrowUp`, `PageDown`, `PageUp` (`PAGE_JUMP` 10), `Home`,
  `End`, decided by the pure private `moved_to(key, current, last)`; anything
  else returns `None` and is left unhandled (no `stop_propagation`).
- The index behind an id is found by the private `index_of(rows, id, hint)`:
  cursor hint first, then `rows.iter().position(..)` — the history-sized fallback
  scan the root `CLAUDE.md` names as a stated review obligation.
- `CommitRow` receives `.selected(bool)` and draws only a background change.

### 1.5 Theme, palette, typography

- **Theme**: every colour is read at render time from Freya's theme via
  `get_theme_or_default().read().colors().<field>`. Fields used today:
  `text_primary`, `text_secondary`, `text_placeholder`, `surface_secondary`
  (selected row), `surface_tertiary` (banner), `border`, `border_focus`,
  `error`. The application installs `dark_theme` in `main.rs`
  (`use_init_theme(dark_theme)`). `cairn-ui` defines **no palette constants of
  its own** other than the lane palette.
- **Lane colours**: `crates/cairn-ui/src/lane_palette.rs`, `LANE_COLOURS:
  &[(u8, u8, u8)]` — eight entries: the seven non-black Okabe–Ito colours
  (sky blue, orange, bluish green, reddish purple, yellow, blue, vermillion)
  plus neutral grey `(190, 190, 190)`. `pub fn lane_colour(Lane) -> Color`
  cycles by `lane.index() % len`; `pub fn palette_len()`. Pinned by
  `the_palette_is_the_okabe_ito_set_less_black_plus_a_neutral`. This differs
  from the mockup's six-step `--l0..--l5` set (section 3.4); the code is the
  as-built authority and the design doc says "intent, not as-built".
- **Typography**: **no font is embedded and no font family is named anywhere
  in `cairn-ui` or `cairn-app`** (grep for `Plex`, `font_family`, `with_font`,
  `include_bytes` over `crates/` finds nothing). Text renders in Freya's default
  family stack (section 4.4). `ROW_FONT_SIZE` is 13; the title is 16, the
  path/count 13, notices 14/12, dialog text 14/12. The design doc's IBM Plex
  Sans/Mono decision is unimplemented.
- **Geometry constants** (`crates/cairn-ui/src/graph_geometry.rs`):
  `ROW_HEIGHT` 26, `LANE_WIDTH` 14, `NODE_RADIUS` 4, `STROKE_WIDTH` 1.8,
  `MAX_DRAWN_LANES` 24, with `const _` assertions that a lane is wider than its
  ink. `graph_cell.rs` paints via `canvas(RenderCallback::new(..))` using
  `freya::engine::prelude::{Paint, PathBuilder, PathEffect, SkColor}` — the
  only Skia use in the crate.

### 1.6 Accelerators and keyboard (decision D5)

- **There is no accelerator table in code.** `docs/design/cairn.md` D5 and
  `docs/design/feature-inventory.md` ("Accelerator table — One
  logical-action-to-chord map (D5). Not per-component literals.") state the
  rule; nothing implements it and no guard checks it. The only keyboard code is
  `HistoryList::keyboard`/`moved_to`, which matches unmodified
  `Key::Named(NamedKey::..)` values and never reads `modifiers`.
- No component names `Ctrl`, `Modifiers`, or a chord. Freya's
  `KeyboardEventData` (`crates/freya-core/src/events/data.rs` in the checkout)
  carries `key: keyboard_types::Key`, `code: keyboard_types::Code`,
  `modifiers: keyboard_types::Modifiers`, so a table has what it needs to read.

### 1.7 The headless tests (`crates/cairn-ui/tests/`)

Three files, all on `freya_testing::TestingRunner`:

- **The runner pattern** (`crates/cairn-ui/tests/history_list.rs`,
  `commit_row.rs`, `credential_prompt.rs`): `TestingRunner::new(app_fn,
  (WIDTH, HEIGHT).into(), |runner| runner.provide_root_context(|| Fixture {..}),
  1.)` returns `(runner, T)` where `T` is what the context closure returns; the
  app reads its fixture with `use_consume::<Fixture>()`. Then
  `test.sync_and_update()` (a few times where a popup animates in or a focus
  request needs a frame). Fixtures hold `State::create(..)` handles so a test
  can mutate the rows (`held.write().extend(..)`) and re-sync.
- **Finding elements**: `test.find(|node, element| Label::try_downcast(element)
  .filter(..).map(|_| node.layout().area ..))` and `test.find_many(..)`;
  `Rect::try_downcast` for backgrounds/borders/roles; `Paragraph::try_downcast`
  for an `Input`'s spans (`p.spans`, `p.cursor_index`); `node.is_visible()`.
- **Driving**: `test.click_cursor((x, y))`, `test.press_key(Key::Named(..))`,
  `test.write_text(..)`, `test.scroll((x, y), (dx, dy))` with a negative `dy`
  scrolling down, `test.run_in(|| ..)` to run code (e.g. set
  `Platform::get().navigation_mode`) inside the runtime.
- **`only_a_viewport_of_rows_is_built_however_long_the_history`** (in
  `history_list.rs`): for lengths 1,000 and 100,000, counts the labels whose
  text contains `"commit "`; asserts the count is within `[viewport_rows,
  viewport_rows + 2]` where `viewport_rows = (HEIGHT / ROW_HEIGHT).ceil()` at
  the top, scrolls to row `length - 300` by `test.scroll((100,100), (0,
  -(deep * ROW_HEIGHT)))`, asserts the same bound and that the deep row is
  built and visible, and finally that all four counts are equal.
- Other pinned behaviours: click selects and reports `RowId`; keys move the
  selection in the stated order; selection follows its row when rows arrive
  below and above; End/Home/PageDown reveal an off-screen selection; a click
  gives the list the keyboard (and a click elsewhere takes it away);
  `on_reach_end` fires once when the last screen comes into view and not on
  re-render; the list is outlined only under keyboard focus. `commit_row.rs`
  pins column order/widths, heading alignment, graph width growing with
  lanes, the selected background differing, and that the date and short id fit
  their columns at the row font size (a measured width — "needs a system
  font"). `credential_prompt.rs` pins naming the remote and URL, masking,
  Enter/button submit, host-key confirmation without a text field, cancel by
  button/Escape/press-outside.

---

## 2. `crates/cairn-app/src/` outside `worker/`

Files: `main.rs`, `window.rs`, `session.rs`, `history_state.rs`,
`fetch_state.rs`, `status_text.rs`, `repository_path.rs`. All render-path files
under the UI-thread guard (section 5).

### 2.1 `main.rs` — wiring

`fn app()`:

1. `use_init_theme(dark_theme)`.
2. Creates the six `State`s and bundles them into `window::View { rows,
   progress, selected, fetch, prompt, remotes }`. The comment "this scope must
   not read it, only `progress`" records that `app()` must not subscribe to the
   row vector.
3. `use_hook` resolves the repository path (`repository_path::chosen(args,
   working_directory())`).
4. `use_hook` calls `worker::open(&path)` → `(RepositoryHandle, Updates,
   Replier)`; submits `Request::ListRemotes` then `Request::OpenHistory { rows:
   PAGE_ROWS }` (`PAGE_ROWS` = 64); spawns a Freya task (`spawn(async ..)`)
   that loops `while let Some(update) = updates.next().await` and calls
   `session::apply(update, view, &session::Worker { submit, refuse })`. When the
   stream ends it writes `progress.stream_ended(..)`. The `Replier` is held
   `Weak` inside the task so it cannot outlive the window.
5. Builds `submit: Option<Rc<dyn Fn(Request)>>` via
   `RepositoryHandle::into_submitter()` and passes `window::window(&opened, view,
   submit, reply)`.

### 2.2 `window.rs` — the view state and the layout

`pub struct View` (`Clone, Copy`): `rows: State<Vec<HistoryRow>>`, `progress:
State<Progress>`, `selected: State<Option<RowId>>`, `fetch:
State<FetchStatus>`, `prompt: State<Option<PromptView>>`, `remotes:
State<Vec<RemoteSummary>>`. "Handles, not values: the window subscribes to what
it reads."

`pub fn window(opened: &str, view: View, submit: Option<Rc<dyn Fn(Request)>>,
answer: Option<Replier>) -> Element` composes, top to bottom, inside one
`rect().expanded().theme_background()`:

1. `title_bar(..)` — a horizontal flex `rect()` with "Cairn", the repository
   path (flex 1, ellipsised), the loaded count (`status_text::loaded_count`),
   and `fetch_button(..)` (a `Button::new().compact()` reading "Fetch <remote>"
   or "Cancel", or nothing while cancelling / with no remote). **This is the
   only toolbar.** There is no sidebar, no repository tabs.
2. Optionally a `banner(line, failed)` for `status_text::fetch_line(&fetch)`.
3. `HistoryHeader::new()`.
4. Either `notice(message, opened)` (loading / empty / failed-before-rows, via
   `status_text::placeholder`) or `history(view.rows, lanes, view.selected,
   view.progress, submit)`.
5. Optionally a failure banner under the rows.
6. `.maybe_child(prompt.map(|p| dialog(p, &fetch, view.prompt, answer)))` — the
   credential dialog, appended last so it overlays.

**There is no detail pane, no placeholder for one, and no split.** The history
list takes the whole remaining height through `.expanded()`.

### 2.3 Selection in the app

`history(..)` builds `HistoryList::new(rows, |render: RowRender| match
render.row.content { RowContent::Commit(commit) =>
CommitRow::new(commit, render.row.graph, render.lanes).selected(render.selected).into() })`
— the comment says "No wildcard arm: a new row kind must fail to compile
here." Then `.lanes(lanes).selected(*selected.read()).on_select(move |id:
RowId| selected.set(Some(id)))`.

So **there is a selected commit**: `View::selected: State<Option<RowId>>`,
written only by `on_select`, read only to pass back into the list. Nothing
else reacts to it. `RowId::Commit(Oid)` gives the `Oid` a diff request would
name.

`on_reach_end` peeks `progress.wants_more()` (peek, not read — "reading here
subscribes the window to the progress it writes, and loops"), submits
`Request::MoreHistory { rows: PAGE_ROWS }` and calls `progress.write().asked()`.

### 2.4 How worker updates become state (`session.rs`)

`pub fn apply(update: Update, view: View, worker: &Worker<'_>)` on the UI
thread, matching every `Update` variant by name:

- `Rows { rows, complete }` → `history_state::widest_lane(&page)`, extend
  `view.rows`, `progress.received(widest, complete, loaded)`.
- `Failed { message }` / `WorkerLost { message }` → `progress.failed(message)`.
- `Remotes { remotes }` → `remotes.set(..)`.
- `FetchStarted` / `FetchProgress` → `fetch.write().started(..)` /
  `.progressed(line)`.
- `FetchFinished` / `FetchCancelled` / `FetchFailed` → `withdraw(&mut prompt,
  worker)` (takes down the dialog and refuses its prompt), sets the terminal
  `FetchStatus`, and `reload_if(refreshed, ..)` which clears the rows, resets
  `Progress::opening()` and resubmits `OpenHistory`. **Note: the selection is
  not cleared on reload**; it survives as a `RowId` that may or may not be in
  the new rows.
- `Prompt { id, text }` → shown as `PromptView { id, text }` only if
  `fetch.read().is_in_flight()`, else refused.

`session::Worker<'a> { submit: &'a dyn Fn(Request), refuse: &'a dyn
Fn(PromptId) }` — "two plain callbacks, never a struct holding the answering
end".

### 2.5 View-state types

- `history_state::Progress` (private fields; `opening()`, `status()`,
  `lanes()`, `loaded()`, `has_rows()`, `complete()`, `wants_more()`,
  `asked()`, `received(widest_lane, complete, loaded)`, `failed(msg)`,
  `stream_ended(msg)`) and `history_state::Status { Loading, Empty, Ready,
  Failed(String) }`. `lanes` only grows. `pub fn widest_lane(&[HistoryRow])`.
- `fetch_state::FetchStatus { Idle, Starting{remote}, Running{remote,
  line: Option<String>}, Cancelling{remote}, Finished{remote},
  Cancelled{remote, stranded_locks: Vec<PathBuf>}, Failed{remote, message} }`
  with `is_in_flight`, `can_be_cancelled`, `remote_in_flight`, `starting`,
  `started`, `cancelling`, `progressed`. `fetch_state::PromptView { id:
  PromptId, text: String }`.
- `status_text`: `fetch_line(&FetchStatus) -> Option<String>`,
  `placeholder(&Status, has_rows) -> Option<String>`, `loaded_count(&Progress)
  -> String`; `why_it_failed` picks the first `fatal:`/`error:` line.

### 2.6 Dialogs today

- **Credential dialog**: `window::dialog(prompt, &fetch, view.prompt, answer)`
  builds `CredentialPrompt::new(remote, prompt.text)` where `remote` is
  `fetch.remote_in_flight().unwrap_or("git")`; `on_submit` wraps the moved
  `String` in `Secret::from_string`, calls `answer(Reply::Provide { prompt: id,
  secret })` and `showing.set(None)`; `on_cancel` calls `answer(Reply::Refuse {
  prompt: id })` and clears. The component itself is a Freya `Popup` with
  `PopupTitle`, `PopupContent`, `PopupButtons`, `Button`, `Input` (with
  `InputMode::new_password()` for secrets), `.on_close_request(cancel)` for
  Escape/press-outside.
- **There is no `Confirmed` dialog.** `cairn_model::Confirmed::by_user(prompt)`
  has no caller in the UI or app; no destructive op exists yet. The pattern the
  credential dialog sets — a `Popup` drawn last from an `Option<..>` in `View`,
  answering through a callback and clearing its own state — is the only
  precedent.

### 2.7 Where a "selected commit → request diff → render diff" flow plugs in

Every hop has one obvious home today:

1. **Selection** already lands in `View::selected: State<Option<RowId>>` via
   `HistoryList::on_select` in `window::history`. A diff request needs the
   `Oid` inside `RowId::Commit(oid)`; a working-tree row would be a second
   `RowId`/`RowContent` variant (which every match in `window.rs` and
   `session.rs` must then name — the exhaustiveness guard in section 5).
2. **Request**: `worker::Request` (`crates/cairn-app/src/worker/request.rs`)
   is where a `CommitDiff { .. }` / `WorkingTreeDiff` / `Compare { .. }`
   variant goes; `Request::is_query()` decides whether it is epoch-numbered
   (superseded by the next query — today only `OpenHistory`/`MoreHistory`) or
   an operation. **A diff query numbered like a history page would be
   superseded by the next scroll page and vice versa**; the epoch scheme is one
   counter (`worker::epoch::Epochs`), not per-kind. `RepositoryHandle::submit`
   is what the UI calls, through the `Rc<dyn Fn(Request)>` `window()` receives.
   Where the UI would submit: an effect or handler reacting to `selected`
   changing — today nothing reacts to it; `window()` is a plain function, not a
   component, so a `use_effect`/`use_memo` on `selected` would need a
   component boundary (see open questions).
3. **Serving**: `worker::pool::serve` dispatches every `Request` on the
   `cairn-repository` thread that owns the `HistorySession`; a new variant is a
   new arm there (the match is total). It has `repo` (the thread-local
   gitoxide handle) and `epochs.watch(epoch)` for cancellation.
4. **Update**: `worker::Update` gains a `Diff { .. }` variant; `Updates::next`
   drops epoch-numbered updates that are no longer current, so a superseded
   diff is discarded automatically.
5. **State**: `session::apply` gains an arm writing a new `State<..>` on
   `View` (a `diff` slot, like `prompt`), which `window()` reads and hands to
   a new `cairn-ui` component.
6. **Layout**: `window()` currently stacks `HistoryHeader` + list in one
   column; a detail pane must be inserted here (below, per the mockup) and the
   list's `.expanded()` height shared — `ResizableContainer` exists (section
   4.6).
7. **Render**: a new component in `cairn-ui` taking a `cairn-model` diff type,
   virtualized (the guard requires `VirtualScrollView` for anything
   history-sized and forbids naming `ScrollView` on render paths, section 5).

Tests for each hop already have templates: `window.rs`'s `launch_with(rows,
progress, fetch, prompt)` captures `Submitted = Rc<RefCell<Vec<Request>>>` and
`Answered`, and `session.rs`'s `applying(&test, view, &asked, update)` runs
`apply` inside `test.run_in`.

---

## 3. `docs/design/ui.md` and the mockup

### 3.1 `docs/design/ui.md` in full — what it commits to

Header: "Intent, not as-built … Nothing here exists in code." Modelled on Fork.

Layout model (kept from Fork), verbatim diagram:

```
┌──────────────────────────────────────────────────────────────┐
│ toolbar: Fetch · Pull · Push · Stash │ repo · branch │ actions │
├───────────┬──────────────────────────────────────────────────┤
│ sidebar   │ main: history graph  OR  local changes           │
│ refs      ├──────────────────────────────────────────────────┤
│           │ detail: commit / changes / file tree  OR  diff   │
└───────────┴──────────────────────────────────────────────────┘
```

Kept items relevant to this packet: "The graph is the main view … Columns:
lanes, subject with ref badges, author, short id, date"; "Staging is one
screen: unstaged and staged trees, the diff of the selected file, and the commit
box … Hunk actions live on the hunk header, in the diff"; "A detail pane with
Commit / Changes / File Tree tabs below the graph."

Palette and type (verbatim): "Dark-first, because the application launches
with Freya's `dark_theme`. Cool graphite neutrals with a slight blue bias … One
accent — a desaturated steel blue — for HEAD, selection and the primary action,
chosen because red and green are already spoken for by diffs and must stay
semantic. Lane colours are a six-step set … Type is IBM Plex — Sans for the
interface, Mono for ids, paths and diffs … `tabular-nums` wherever digits
align. Light theme is not designed yet … a diff palette that works on a dark
ground does not survive inversion. Open item."

Screens: 1 History (with the detail pane), 2 Local changes ("Unified diff with
hunk-header actions and a line-selection gutter, so hunk *and* line staging are
visible as one mechanism"), 3 Discard, 4 Conflict, 5 Worktrees.

Open list (verbatim, the diff-relevant ones): "Whether the detail pane sits
below the graph (Fork) or to its right. Below wins on a laptop; right wins on
a wide monitor. Probably a user preference, which means a layout the
components must not assume." and "Side-by-side diff, which Fork offers. The
`diff-engine` packet's O1."

The changes table's one diff-adjacent row: the destructive dialog "says what is
lost, how much, and whether it is recoverable — in that order — and its text IS
the `Confirmed` prompt."

### 3.2 Mockup: the detail pane (History screen)

`docs/design/mockups/cairn-ui.html`, `.detail`:

```css
.detail { border-top: 1px solid var(--line); background: var(--surface); height: 250px; display: grid; grid-template-rows: 30px 1fr; }
.dtabs { display: flex; gap: 2px; padding: 0 12px; border-bottom: 1px solid var(--line); align-items: flex-end; }
.dtabs span { padding: 6px 12px; color: var(--muted); font-size: 12px; }
.dtabs span.on { color: var(--fg); box-shadow: inset 0 -2px 0 var(--accent); }
.commit { display: grid; grid-template-columns: 1fr 1fr; gap: 6px 24px; padding: 14px 16px; font-size: 12.5px; }
.commit .kv { display: grid; grid-template-columns: 78px 1fr; gap: 4px 10px; }
.commit .kv dt { color: var(--muted); font-size: 11px; text-transform: uppercase; letter-spacing: .05em; }
.commit .kv dd { margin: 0; font-family: var(--mono); font-size: 12px; }
.commit .msg { grid-column: 1 / -1; border-top: 1px solid var(--line); padding-top: 10px; }
.commit .msg b { display: block; font-weight: 500; margin-bottom: 4px; }
.commit .msg p { margin: 0; color: var(--muted); max-width: 70ch; }
.files { grid-column: 1 / -1; display: flex; gap: 14px; font-family: var(--mono); font-size: 11.5px; color: var(--muted); }
.files i { font-style: normal; color: var(--add); }
```

```html
<div class="detail">
  <div class="dtabs"><span class="on">Commit</span><span>Changes <small style="color:var(--faint)">8</small></span><span>File Tree</span></div>
  <div class="commit">
    <dl class="kv"><dt>Author</dt><dd>Alex Parlett · Sep 14 19:41</dd><dt>Commit</dt><dd>7f9c648</dd><dt>Parents</dt><dd>b8f5152</dd></dl>
    <dl class="kv"><dt>Committer</dt><dd>Alex Parlett · Sep 14 19:41</dd><dt>Refs</dt><dd>main, origin/main</dd></dl>
    <div class="msg"><b>docs(design): stamp the out-of-scope list as reviewed against real usage</b><p>Walked the remaining entries …</p></div>
    <div class="files">docs/design/feature-inventory.md <i>+8</i> docs/work/daily-loop/progress.md <i>+14</i></div>
  </div>
</div>
```

So the Commit tab shows: two key/value columns (Author · time, Commit short id,
Parents / Committer · time, Refs), the full message (subject bold, body muted
at 70ch), and a one-line file list with per-file `+N` in the add colour. The
"Changes" tab carries a count badge (`8`). **The Changes and File Tree tab
bodies are never drawn in any screen** — only their captions. The pane is a
fixed 250px below the list (`.main { grid-template-rows: 1fr auto }`), the tab
strip 30px. The Worktrees screen repeats the pane with a "Refs" row listing
`feature/history-graph, origin/feature/history-graph`.

### 3.3 Mockup: the diff (Local changes screen)

```css
.diffpane { display: grid; grid-template-rows: 32px 1fr auto; min-height: 0; }
.diffhd { display: flex; align-items: center; gap: 12px; padding: 0 14px; border-bottom: 1px solid var(--line); font-family: var(--mono); font-size: 12px; color: var(--muted); }
.diffhd .path { color: var(--fg); }
.diffhd .opts { margin-left: auto; display: flex; gap: 6px; font-family: var(--sans); }
.diff { overflow: hidden; font-family: var(--mono); font-size: 12px; line-height: 20px; }
.hunk { display: flex; align-items: center; gap: 10px; background: var(--raised); color: var(--muted); padding: 0 12px 0 0; height: 24px; border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); }
.hunk .hh { padding-left: 12px; flex: 1; }
.ln { display: grid; grid-template-columns: 28px 40px 40px 1fr; }
.ln > span { padding: 0 8px; white-space: pre; }
.ln .n { color: var(--faint); text-align: right; }
.ln .cb { display: grid; place-items: center; padding: 0; }
.ln .cb i { width: 12px; height: 12px; border: 1px solid var(--line-strong); border-radius: 2px; }
.ln.a { background: var(--add-bg); } .ln.a .c::before { content: "+ "; color: var(--add); }
.ln.d { background: var(--del-bg); } .ln.d .c::before { content: "- "; color: var(--del); }
.ln.x .c::before { content: "  "; }
.ln.pick { background: rgba(111,163,214,.22); }
.ln.pick .cb i { background: var(--accent); border-color: var(--accent); }
.ln.pick.a .c::before { color: var(--fg); }
```

```html
<div class="diffhd"><span class="path">crates/cairn-ui/src/commit_row.rs</span><span>+9 −3</span><span class="opts"><button class="btn sm q">Unified</button><button class="btn sm q">Side by side</button><button class="btn sm q">Ignore whitespace</button></span></div>
<div class="diff">
  <div class="hunk"><span class="hh">@@ -28,9 +28,12 @@ impl ComponentOwned for CommitRow</span><button class="btn sm">Stage hunk</button><button class="btn sm q">Discard hunk…</button></div>
  <div class="ln x"><span class="cb"><i></i></span><span class="n">28</span><span class="n">28</span><span class="c">    fn render(self) -&gt; impl IntoElement {</span></div>
  <div class="ln d"><span class="cb"><i></i></span><span class="n">30</span><span class="n"></span><span class="c">        rect()</span></div>
  <div class="ln a pick"><span class="cb"><i></i></span><span class="n"></span><span class="n">30</span><span class="c">        let lane = self.row.lane;</span></div>
  …
</div>
```

What that fixes: a **32px header** with the path (fg, mono), the `+N −M`
totals, and three option buttons — Unified, Side by side, Ignore whitespace —
right-aligned in the sans face; a **24px hunk header** row on the `--raised`
surface with the `@@` line (including the function context) and the hunk
actions on the right; **20px line rows** in mono 12px with four columns:
28px checkbox gutter, 40px old number, 40px new number, then content; the
`+ `/`- `/two-space marker is prepended to the content column in the
add/remove colour; added rows get `--add-bg`, removed `--del-bg`, context
none; a picked (line-selected) row gets `rgba(111,163,214,.22)` and a filled
accent checkbox. The mockup annotates: "Line selection is a gutter of
checkboxes, so staging a hunk and staging four lines are visibly one
mechanism. Both emit a patch for `git apply --cached` — which is why the diff
model is patch-capable from its first commit (daily-loop L2)."

**There is no intra-line (word) highlight in the mockup** — no CSS class, no
`<mark>`, no second-level add/remove tone. **No side-by-side rendering is
drawn**; only the button. **No context-lines control** is drawn; only Ignore
whitespace. The Discard screen shows a `diffpane` header reading "2 files
selected" over an empty diff — a multi-select file state.

### 3.4 Mockup palette (the application's own world, "fixed on purpose")

```css
--bg: #1B1D22; --surface: #23262D; --raised: #2C3038; --line: #353A44; --line-strong: #454B57;
--fg: #E6E8EC; --muted: #8B919C; --faint: #5E6470;
--accent: #6FA3D6; --accent-fg: #0F1620; --accent-dim: rgba(111,163,214,.16);
--add: #4FAE6E; --add-bg: rgba(79,174,110,.13); --del: #E06C6C; --del-bg: rgba(224,108,108,.13);
--danger: #E05A5A; --warn: #D9A441;
--l0: #6FA3D6; --l1: #D9A441; --l2: #B57BD6; --l3: #4FAE6E; --l4: #E08A5A; --l5: #62C2C2;
--sans: "IBM Plex Sans", "Segoe UI", system-ui, sans-serif;
--mono: "IBM Plex Mono", ui-monospace, "Cascadia Mono", Menlo, monospace;
```

Diff-relevant values: add foreground `#4FAE6E` on `rgba(79,174,110,.13)`;
delete foreground `#E06C6C` on `rgba(224,108,108,.13)`; line-number gutter
text `--faint #5E6470`; hunk header on `--raised #2C3038` in `--muted
#8B919C`; borders `--line #353A44`; selection `--accent-dim`; picked line
`rgba(111,163,214,.22)`. The app frame uses `font-variant-numeric:
tabular-nums` and 13px base; the mockup's list rows are 28px against the
code's `ROW_HEIGHT` 26; the mockup's status letters use `--warn` for M,
`--add` for A, `--del` for D, `--danger` for C.

None of these hex values exist in code; the code draws from Freya's
`DARK_COLORS` (section 4.5), whose greys are neutral (`background`
`rgb(20,20,20)`, `surface_secondary` `rgb(45,45,45)`) rather than the
blue-biased graphite above, and whose `success`/`error` are
`rgb(129,199,132)`/`rgb(229,115,115)`.

---

## 4. Freya 0.5-rc, verified against the vendored checkout

Pinned rev (root `Cargo.toml` `[workspace.dependencies]`): `freya` and
`freya-testing` from `https://github.com/alexparlett/freya` at
`caa46f8715744cb4fdb2d36304e2b1bbeb4acdb0`; `Cargo.lock` resolves it as
`freya 0.5.0-rc.4`. Checkout:
`~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/` (a git dependency,
so under `git/checkouts`, not `registry/src`). Crates below are cited relative
to that root. Features enabled: `default = ["winit"]` plus `engine` from
`cairn-ui`; nothing else (`code-editor`, `markdown`, `clipboard` extras are
NOT on — see each item).

### 4.1 `VirtualScrollView` — variable row heights: YES, with a cost

`crates/freya-components/src/scrollviews/virtual_scrollview.rs`:

- `pub enum ItemSize { Fixed(f32), Dynamic(Callback<usize, f32>) }`;
  `.item_size(impl Into<ItemSize>)` accepts an `f32` or any
  `Fn(usize) -> f32 + 'static`.
- `ItemSize::visible_range` for `Dynamic` **walks from index 0 accumulating
  sizes every render** until it passes the scroll offset, then continues to
  the viewport bottom — O(scroll position) per frame, not O(viewport).
  `ItemSize::total_size` for `Dynamic` **extrapolates** the content length
  from the average of items measured down to the viewport bottom ("keeping the
  scrollbar stable as it scrolls"), so the scrollbar is an estimate.
- Builder signature: `Fn(VirtualItem, &D) -> Element` where `pub struct
  VirtualItem { pub index: usize, pub size: f32 }`; constructors `new`,
  `new_controlled`, `new_with_data`, `new_with_data_controlled`; setters
  `length`, `item_size`, `direction(Direction)`, `show_scrollbar`,
  `scroll_with_arrows`, `wheel_axis_lock`, `invert_scroll_wheel`,
  `drag_scrolling`, `scrollbar_theme`, `scroll_controller`, `min_/max_width`,
  `min_/max_height`.
- Rows are built for `render_range` only: `render_range.map(|i|
  (self.builder)(VirtualItem { index: i, size }, &data)).collect()`.

### 4.2 Horizontal scroll — YES, on the cross axis and as a direction

- Same file: for `Direction::Vertical`, `inner_width` is
  `size.read().inner_sizes.width` (the measured width of the built children)
  and the content `rect()` gets `.offset_x(corrected_scrolled_x)`; a horizontal
  scrollbar is shown when `inner_width > viewport width`. So a vertical
  `VirtualScrollView` whose rows are wider than the viewport scrolls
  horizontally too, and the wheel handler splits x/y with an optional
  `wheel_axis_lock`. The cross-axis extent is whatever the built rows measure —
  the widest **visible** row, not the widest row in the file (OPEN: a
  horizontal position past a short row's width is clamped by
  `get_corrected_scroll_position`).
- `direction(Direction::Horizontal)` makes the item axis horizontal instead.
- `ScrollView` (`scrollviews/scrollview.rs`): "Scrollable area with
  bidirectional support and scrollbars", `.direction(..)`, `.spacing(..)`,
  `.contain_wheel()`, `.latch_wheel()`, `.wheel_axis_lock(..)`, `.on_sized(..)`,
  `max_width/height`. **Naming it on any render path fails the guard unless
  the file is on `UNBOUNDED_VIEW_EXCEPTIONS`, which is empty** (section 5).

### 4.3 Text with per-span styling — spans YES, per-span background NO, ranged highlight YES (one colour)

`crates/freya-core/src/elements/paragraph.rs` and
`crates/freya-core/src/data.rs`:

- `paragraph()` takes `.span(impl Into<Span<'static>>)` and
  `.spans_iter(..)`. `pub struct Span<'a> { pub text_style_data:
  TextStyleData, pub text: Cow<'a, str> }`, built by `Span::new(text)` or from
  `&'static str`/`String`, styled through `TextStyleExt`.
- `TextStyleData` fields: `color`, `font_size`, `font_families`, `text_align`,
  `text_height`, `text_overflow`, `text_shadows`, `text_decoration`,
  `text_decoration_style`, `text_decoration_color`, `font_slant`,
  `font_weight`, `font_width`, `letter_spacing`. **No background field** —
  a span cannot carry its own background colour.
- The paragraph does have `.highlights(Vec<(usize, usize)>)` and
  `.highlight_color(Color)`: for each `(from, to)` it calls Skia's
  `get_rects_for_range(from..to, RectHeightStyle::Tight,
  RectWidthStyle::Tight)` and fills each rect with `highlight_color` before
  the text is painted (the "Draw highlights" block in `ParagraphElement`'s
  render). One colour per paragraph, any number of ranges. That is a viable
  intra-line highlight for a diff line: one paragraph per line, its word
  ranges as `highlights`, and one colour per line kind. OPEN: the index unit
  of `get_rects_for_range` (Skia paragraph indices are UTF-16 code units; the
  doc comment says "character ranges") — must be verified against a non-ASCII
  line before relying on it.
- `.cursor_mode(CursorMode::Expanded)` makes highlights use the paragraph's
  inner area (full line height) rather than glyph-tight — the code editor
  uses it.
- `label()` implements `TextStyleExt` too, so `.font_family("..")` works on a
  plain label, and `rect().font_size(..)` cascades (the code editor sets
  `font_size` on the row rect).
- `SelectableText` (`crates/freya-components/src/selectable_text.rs`):
  `new()`, `.span(..)`, `.child(..)`, `.max_lines`, `.line_height` — a
  paragraph with mouse selection. Freya's clipboard crate (`freya-clipboard`,
  `arboard`) is an unconditional dependency of `freya`, so copy is reachable;
  OPEN whether `SelectableText` copies across multiple rows.

### 4.4 Monospace font loading — YES, by embedding bytes

`crates/freya-winit/src/config.rs`: `LaunchConfig::with_font(font_name, bytes:
impl Into<Bytes>)` embeds a font (registered through Skia's
`TypefaceFontProvider` in `crates/freya-winit/src/lib.rs`);
`with_default_font(name)` puts a family first in the fallback list;
`with_fallback_font(name)` appends. Default stack
(`crates/freya-core/src/style/default_fonts.rs`): on Linux `Ubuntu`, `Adwaita
Sans`, then `Noto Sans`, `Arial`. **No monospace family is in the default
stack**; a diff must name one (`.font_family("IBM Plex Mono")` after
embedding, or a system family). Tests: `TestingRunner::set_fonts(HashMap<&str,
&[u8]>)` and `set_default_fonts(&[..])` embed fonts headlessly — the current
tests use none, which is why `commit_row.rs` notes a measured width "needs a
system font". Embedding IBM Plex is a dependency-shaped decision: the bytes
would be `include_bytes!` in `cairn-app` (only it builds `LaunchConfig`), and
the licence (OFL) would need recording.

### 4.5 Theme

`crates/freya-components/src/theming/component_themes.rs` `pub struct
ColorsSheet` fields: `primary`, `secondary`, `tertiary`; `success`, `warning`,
`error`, `info`; `background`, `surface_primary`, `surface_secondary`,
`surface_tertiary`, `surface_inverse{,_secondary,_tertiary}`; `border`,
`border_focus`, `border_disabled`; `text_primary`, `text_secondary`,
`text_placeholder`, `text_inverse`, `text_highlight`; `focus`, `active`,
`disabled`; `overlay`, `shadow`. **There is no add/remove/diff colour in the
sheet.** `DARK_COLORS` (`theming/themes.rs`): `primary rgb(103,80,164)`
(overridden by the OS accent colour when available — `dark_theme()` reads
`Platform::accent_color`), `success rgb(129,199,132)`, `error rgb(229,115,115)`,
`background rgb(20,20,20)`, `surface_primary rgb(60,60,60)`,
`surface_secondary rgb(45,45,45)`, `surface_tertiary rgb(25,25,25)`, `border
rgb(60,60,60)`, `border_focus rgb(110,110,110)`, `text_primary
rgb(250,250,250)`, `text_secondary rgb(210,210,210)`, `text_placeholder
rgb(150,150,150)`, `text_highlight rgb(96,145,224)`.

### 4.6 Tabs and resizable panels

- **No `Tabs` component.** Closest: `SegmentedButton` +
  `ButtonSegment` (`crates/freya-components/src/segmented_button.rs`) —
  `ButtonSegment::new().key(i).selected(bool).enabled(bool).on_press(..)
  .child(..)`, multi-select by the caller's own state; `FloatingTab`
  (`floating_tab.rs`) — a single themed tab with `selected_background`
  /`selected_color` and `on_press`, no strip. `Sidebar`/`SidebarItem`,
  `Tile`, `Accordion`, `Table`/`TableRow`/`TableCell` also exist.
- **`ResizableContainer`** (`crates/freya-components/src/resizable_container.rs`):
  `ResizableContainer::new().direction(Direction).handle_size(f32)
  .panel(ResizablePanel).panels_iter(..).controller(Writable<ResizableContext>)`;
  `ResizablePanel::new(PanelSize).initial_size(..).min_size(f32).max_size(f32)
  .min_pixels(f32).order(usize).on_resized(EventHandler<f32>)` with
  `ChildrenExt` (`.child(..)`); `PanelSize::px(f32)` / `PanelSize::percent(f32)`.
  A real split with a draggable handle; the mockup's fixed 250px pane is
  `PanelSize::px(250.)`.
- **`Popup`** (`popup.rs`): `Popup::new().on_close_request(..)` +
  `PopupTitle::new(text)`, `PopupContent::new()`, `PopupButtons::new()` — what
  the credential dialog uses and what a `Confirmed` dialog would use.

### 4.7 The code editor crate (a reference implementation of "lines of spans")

`crates/freya-code-editor/` (feature `code-editor`, **not enabled** in this
workspace): `CodeEditor` renders through `VirtualScrollView::new(..)` one
`EditorLineUI` per line; each line is a horizontal `rect()` of fixed
`line_height` with an optional gutter `label()` and a `paragraph()` given
`.spans_iter(..)` (one `Span` per syntax token, coloured), `.highlights(..)`,
`.highlight_color(..)`, `.cursor_mode(CursorMode::Expanded)`,
`.font_family(..)`, `.max_lines(1)`, `.width(Size::px(longest_width))` and
`.min_width(Size::fill())` — the `longest_width` is how it gives every row the
same cross-axis width so horizontal scroll is uniform. That is the shape a diff
line renderer would take without the crate (it pulls `ropey`, `tree-sitter`
and `freya-edit`).

### 4.8 Summary table

| Capability | Verdict | Source |
| --- | --- | --- |
| Variable row heights in `VirtualScrollView` | Yes (`ItemSize::Dynamic`), O(scroll offset) per frame, estimated total | `freya-components/src/scrollviews/virtual_scrollview.rs` |
| Horizontal scroll | Yes: cross-axis in a vertical VSV (`inner_sizes.width`), or `Direction::Horizontal` | same |
| Per-span colour/font/weight | Yes (`Span` + `TextStyleExt`) | `freya-core/src/elements/paragraph.rs`, `data.rs` |
| Per-span background | No field | `freya-core/src/data.rs` `TextStyleData` |
| Ranged background highlight | Yes, one colour per paragraph (`highlights` + `highlight_color`) | `paragraph.rs` render, `get_rects_for_range` |
| Monospace font | Embed via `LaunchConfig::with_font`; none in default stack | `freya-winit/src/config.rs`, `freya-core/src/style/default_fonts.rs` |
| Tabs | No component; `SegmentedButton`/`FloatingTab` are the parts | `freya-components/src/segmented_button.rs`, `floating_tab.rs` |
| Resizable split | Yes, `ResizableContainer`/`ResizablePanel` | `freya-components/src/resizable_container.rs` |
| `ScrollView` | Exists; guard forbids naming it on render paths | `scrollviews/scrollview.rs`; section 5 |
| Text selection/copy | `SelectableText`; clipboard linked | `selectable_text.rs`, `freya/Cargo.toml` |
| Keyboard modifiers for an accelerator table | `KeyboardEventData.modifiers: keyboard_types::Modifiers` | `freya-core/src/events/data.rs` |

---

## 5. Guard tests that bear on UI and app files

All in `crates/cairn-guards/tests/invariants.rs`, matchers in
`crates/cairn-guards/src/lib.rs`. Rosters quoted exactly.

- **`layer_dependencies_are_allowlisted`** — `DEPENDENCY_ALLOWLIST`:
  `cairn-ui` → `["cairn-model", "freya"]`; `cairn-app` → `["cairn-askpass",
  "cairn-git", "cairn-model", "cairn-ui", "freya"]`; `cairn-model` →
  `["zeroize"]`. `TEST_ONLY_ALLOWLIST`: `freya-testing` for `cairn-ui` and
  `cairn-app`. Every dependency table and rename counts. **A diff library
  (`similar`, `imara-diff`, …) in `cairn-model` or `cairn-git` needs a new row
  and is a user decision; `cairn-ui` can add nothing.** Enabling a Freya
  feature (`code-editor`) adds no crate row but is still a dependency change
  for `deny.toml`.
- **`layers_never_name_the_crates_they_are_sealed_from`** — `FORBIDDEN_IDENTS`:
  `crates/cairn-ui` may not name `gix` or `cairn_git` anywhere (src and tests);
  `crates/cairn-model` may not name `gix`, `freya`, `cairn_git`, `cairn_ui`.
  Aliased imports and qualified paths match. The debris hook
  (`.claude/hooks/qa-stop.sh`) echoes the same idents per turn.
- **`every_view_of_a_row_names_every_kind_of_row`** — over every crate but
  `cairn-model` and `cairn-guards` (`ROW_CONTENT_EXEMPT`), any file naming
  `RowContent` may not read it through `_ =>`, a catch-all binding, `Some(_)`
  beside `Some(RowContent::..)`, `if let`/`while let`/let-chain/`let .. else`,
  `matches!`, a glob import, a `use` of its variants or a rename. **A
  working-tree row (`RowContent::WorkingTree`) breaks
  `crates/cairn-app/src/window.rs`'s match at compile time — intended — and
  every new reader must be an exhaustive `match`.** Note `RowId` is NOT
  guarded, only `RowContent`; `HistoryRow::id()` is in `cairn-model`.
- **`the_ui_thread_never_waits_on_repository_work`** — `RENDER_SOURCE_DIRS`
  = `crates/cairn-ui/src`, `crates/cairn-app/src`; `WORKER_DIR` =
  `crates/cairn-app/src/worker`. Outside `worker/`, no file may contain any
  `WAITING_IDENTS` (`Barrier`, `Condvar`, `JoinHandle`, `Mutex`, `Receiver`,
  `RwLock`, `bounded`, `channel`, `sync_channel`, `unbounded`, `block_on`,
  `blocking_lock`, `blocking_recv`, `blocking_send`, `park`, `park_timeout`,
  `recv_deadline`, `recv_timeout`, `scope`, `select`, `sleep`, `wait_timeout`,
  `wait_while`, `spin_loop`, `try_iter`, `try_lock`, `try_recv`, `yield_now`)
  as a whole identifier in code (strings blanked), nor the nullary calls
  `join()`, `lock()`, `recv()`, `wait()`; and no `cairn-app` render file may
  name `cairn_git`, `gix` or `cairn_askpass`. Inside `worker/`, no file may
  name `freya`, `dioxus` or `cairn_ui`. **Practical traps for a diff view: a
  local or function literally named `select`, `scope` or `sleep` trips it
  (`on_select` is fine — `_` is an identifier byte); a `Mutex`/`RwLock` in a
  component cache trips it; the `cairn-model` diff type must not be built by
  a `cairn-app` render file calling the engine.**
- **`a_history_sized_list_renders_through_a_virtualizing_view`** — over the
  same render dirs: no render file may name `ScrollView` (`UNBOUNDED_VIEW`,
  word-boundary match so `VirtualScrollView` does not count; strings blanked)
  unless listed in `UNBOUNDED_VIEW_EXCEPTIONS: &[(&str, &str)] = &[]`
  (file path, reason); and at least one production render file must name both
  `VirtualScrollView` and `HistoryRow`. **A diff pane that wants a plain
  `ScrollView` (say, for a bounded commit-message box) needs a roster row,
  which excuses every `ScrollView` in that file forever — the packet's file
  split should keep such a file small.** The positive arm is satisfied by
  `history_list.rs` and untouched by a second virtualized list.
- **`only_the_ops_module_mutates_a_repository`** and
  **`every_git_invocation_disables_the_terminal_prompt`** — scan
  `PRODUCT_SOURCE_DIRS` including `cairn-ui/src` and `cairn-app/src` for
  mutation spellings and `Command`; a UI crate never touches either, but a
  "copy patch to clipboard" or "open in editor" feature that spawns a process
  from a render file would trip the second.
- **`no_credential_value_is_logged_printed_serialised_or_stored`** — every
  crate; only matters to the diff packet if a type holds a `Secret` (it will
  not).
- **`destructive_operations_are_sealed_behind_the_confirmation_token`** —
  `cairn-model` and `cairn-git` only; no UI bearing until a destructive verb
  exists.
- **Workspace lints** (root `Cargo.toml`): `unsafe_code = "forbid"`; clippy
  denies `unwrap`, `expect`, `todo!`, `unimplemented!`, `dbg!` outside tests.
  Every `match` in a component must be total and every `Option` handled.
- **`ci_runs_every_merge_bar_gate_step`** — a new gate step (e.g. a diff
  benchmark) must appear in both `scripts/gate.sh` and `.github/workflows/ci.yml`.

Residual review obligations named by the root `CLAUDE.md` that a diff view
inherits: `responsiveness-reviewer` decides whether an iteration is over a
history-sized thing, whether a `VirtualScrollView` is handed a truncated
length, and whether per-frame work grows with scroll depth while the built-row
count stays flat — exactly the `ItemSize::Dynamic` cost in 4.1.

---

## Open questions for the planner

1. **Which epoch does a diff query ride?** `Request::is_query()` puts every
   query on ONE counter, so a `CommitDiff` request would be superseded by the
   next `MoreHistory` page (and would supersede it). Either the diff is an
   "operation" (never superseded, so a fast arrow-key sweep queues every diff)
   or the epoch scheme grows a second lane. `Updates::next` filters by the
   same single counter.
2. **Where does "selection changed → submit request" live?** `window()` is a
   plain function with no hook scope; the selection is written in
   `HistoryList::on_select`'s closure. Submitting on select from that closure
   is the smallest change; a `use_effect` on `View::selected` needs a
   component. Also: the selection is not cleared on `reload_if`, so a diff for
   a commit no longer in the rows can be showing after a fetch.
3. **Fixed vs dynamic row heights.** A unified diff with one line per row is
   `ItemSize::Fixed`; hunk headers at 24px vs lines at 20px (mockup) or
   wrapped long lines need `Dynamic`, which walks from row 0 every frame and
   estimates the scrollbar. Flattening to one row kind, or padding hunk
   headers to the line height, keeps the guard's residual obligation trivial.
4. **Intra-line highlight mechanism.** `paragraph().highlights(..)` with one
   `highlight_color` per line is the only background-range primitive; a span
   cannot carry a background. Verify the index unit against non-ASCII text
   before committing to it. Alternative: paint the highlight rects on a
   `canvas` behind a label, as `graph_cell` does for the graph.
5. **Horizontal scroll extent.** The cross-axis width of a vertical
   `VirtualScrollView` is the widest BUILT row; the code editor pins every row
   to `longest_width`. A diff model would need the longest line's measured
   width (font-dependent) or the pane must accept the clamp.
6. **Monospace font and IBM Plex.** Nothing is embedded; the default stack has
   no mono face. Embedding Plex Mono (OFL) via `LaunchConfig::with_font` in
   `cairn-app` and `TestingRunner::set_fonts` in tests is a dependency-shaped
   decision to surface to the user; naming a system family is the fallback.
7. **Diff colours have no home.** Freya's `ColorsSheet` has no add/remove
   fields and the mockup's hex palette exists nowhere in code. Options: a
   `cairn-ui` `diff_palette` module (like `lane_palette`), or a theme
   extension. The design doc flags light-theme inversion as an open item.
8. **Detail pane placement and split.** The design doc leaves below-vs-right
   open ("a layout the components must not assume"); `ResizableContainer`
   supports either with `Direction`. `window()`'s `.expanded()` list must
   yield height.
9. **Tabs.** No Freya tab strip; the Commit / Changes / File Tree strip is a
   `SegmentedButton` or a hand-rolled row of `FloatingTab`s, and the Changes
   and File Tree bodies were never mocked up.
10. **`ScrollView` roster.** Any bounded scrollable (commit message body, file
    list) either virtualizes or claims the first `UNBOUNDED_VIEW_EXCEPTIONS`
    row — a file-scoped, permanent excuse. Decide the file split with that in
    mind.
11. **Accelerator table (D5) does not exist.** The diff view is the second
    keyboard consumer; if it adds chords (next/previous hunk, toggle
    whitespace) the table should be born with it, and its guard twin with the
    table.
12. **Row kinds.** A working-tree row as a second `RowContent`/`RowId`
    variant compiles-breaks `window.rs` (good) but `GraphRow` still keys by
    `Oid` (`docs/systems/history-graph.md`, known limits) — the lane assigner's
    output is not total over non-commit rows.
13. **Test strategy for the diff view.** The headless pattern counts built
    labels and reads `Paragraph` spans/`Rect` backgrounds; a diff test can pin
    "one viewport of lines built for a 100k-line diff" the same way, and read
    the word ranges back: `Paragraph::try_downcast(element)` returns a
    `ParagraphElement` whose `spans`, `highlights: Vec<(usize, usize)>`,
    `cursor_style_data` (holding `highlight_color`) and `text_style_data` are
    all `pub` fields (verified in `freya-core/src/elements/paragraph.rs`), so
    a test can assert which ranges of which line are highlighted without
    reading pixels.
