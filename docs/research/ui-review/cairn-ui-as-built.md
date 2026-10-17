# Cairn's UI as built

An evidence record of the window Cairn draws today, read from the code, for the ui-review packet,
which will redesign Cairn's UX to align more closely with Fork. It describes; it recommends nothing.
Every claim is anchored to a `path:line` at `feature/plan-ui-review` (`main` at `967bb72`). The code
was read, not run, and no Fork observation is recorded here. What Fork itself does belongs in a
first-hand observation (`observe-fork`), compared over the fixture in `fixture-recipe.md` beside
this file.

**Conventions.** Section by section, paths are written relative to the repository root, or under
the abbreviations each section declares: `UI`/`ui/` = `crates/cairn-ui/src`, `APP`/`app/` =
`crates/cairn-app/src`, `MODEL` = `crates/cairn-model/src`. The window calls
`use_init_theme(dark_theme)` (`crates/cairn-app/src/main.rs:61`), so every "theme" colour
resolves to Freya's `DARK_COLORS` in the linked fork, rev `caa46f8`
(`~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/crates/freya-components/src/theming/themes.rs:116`).
Test code is left out, except where a test pins a surface. Strings appear verbatim in quotes, with
`{placeholders}` or `<placeholders>` where the code formats them.

## The biggest structural facts

- **One window, one row of chrome.** The window has the OS's decorations, the fixed title "Cairn"
  (`main.rs:50`) and Freya's default size. It has no menu bar, no toolbar, no repository tabs and
  no repository picker. The one row at the top reads: the word "Cairn", the status box (repository
  name with `*` when dirty, the branch, `n↓m↑`, and running operations), "N commits…", and one
  "Fetch <first remote>" button (`crates/cairn-app/src/window.rs:538-602`).
- **Layout.** The sidebar sits beside the main region behind a splitter (240 px, minimum 140). The
  main region is one of two things, never both:
  - All Commits: the history list over the detail pane, with the pane 260 px tall, collapsing to a
    30 px tab strip.
  - Local Changes: the lists beside the diff, with the commit box under the diff.
- **Dialogs.** A single modal state draws at most one dialog or the activity popover
  (`crates/cairn-app/src/modal.rs`).
- **Context menus.** There are only three: a commit row ("New Branch…" alone), a Local Changes row
  (Stage or Unstage, "Discard Changes…", Stage All or Unstage All, "Copy Path"), and the commit
  box's Recent Commit Messages list. The sidebar, the chips, the diff and the file lists have none.
- **Theme and type.**
  - The theme is Freya's stock dark theme, with no light theme and no central chrome palette.
    Cairn's own colours are the diff palette (`crates/cairn-ui/src/diff/palette.rs`), the lane
    palette (`history/lane_palette.rs`) and the tag indigo.
  - IBM Plex Mono Regular is the only embedded face. All other text is the system font.
  - Sizes and paddings are mostly inline literals, spread per file.
- **Keyboard.** 20 `Action`s in `crates/cairn-ui/src/accelerators.rs`. Fetch, Pull, Push, Stash,
  Find, Preferences, quit, and switching between Local Changes and All Commits have none.

## Contents

1. Overview: the built UI in one paragraph
2. 01: Window shell, title bar, toolbar, menu bar, window-level states
3. 02: Sidebar and history list
4. 03: Detail pane and diff view
5. 04: Local Changes and the commit box
6. 05: Dialogs, context menus, activity popover
7. 06: Keyboard, palette and typography
8. Fork surfaces Cairn does not have at all
9. Where the code and `docs/design/ui.md` disagree
10. The structural cost of changing layout

## Overview: the built UI in one paragraph

Freya's stock `dark_theme` (`crates/cairn-app/src/main.rs:61`),
one OS-decorated window titled "Cairn" (`main.rs:50`, `WindowConfig` default `decorations: true`).
Inside it one row, the "title bar": the word "Cairn" (`crates/cairn-app/src/window.rs:553`), the
status box (repo name + `*`, branch glyph + branch, `n↓m↑`, running operations; clipped,
`window.rs:555-557`), a loaded-commit count "N commits…" (`window.rs:560-565`,
`history/status_text.rs:17-27`) and one `Fetch <first remote>` button (`window.rs:576-602`). Under
it up to two banners (leftover lock, closing; `window.rs:236-237`). Then sidebar | main region
behind a splitter (`window.rs:300-321`): the main region is the history list + detail pane
(Commit, Changes tabs) or Local Changes (`window.rs:240-243`). Modal layer: credential prompt,
confirmation, Git Error / "Couldn't …", Create Branch, Activity popover (`window.rs:246-266`).
Context menus exist in exactly three places: a commit row ("New Branch…" alone,
`crates/cairn-ui/src/history/list.rs:414-447`), a Local Changes row (`local_changes/menu.rs:215-219`)
and the commit box's Recent Commit Messages (`commit_box.rs:545`).



## 01 — Window shell, title bar, toolbar, menu bar, window-level states

Read-only audit of the built UI at `main` (967bb72). Paths are relative to `crates/`.
`window.rs` production code is lines 1–657; `#[cfg(test)] mod tests` starts at
`cairn-app/src/window.rs:658` (the other ~4,100 lines are tests).

### (a) Window shell

#### Launch and native window

| Item | Value | Anchor |
| --- | --- | --- |
| Native window title | `"Cairn"`, a `&'static str`, never changes (the OS title never names the repository or branch) | `cairn-app/src/main.rs:50` |
| Window size | **not set** — Freya's default `(700.0, 500.0)` | Freya `freya-winit/src/config.rs:145` (rev caa46f8) |
| Min / max size | **not set** — Freya default `None` / `None` | `config.rs:146-147` |
| Decorations | Freya default `true` (native title bar from the OS/WM) | `config.rs:148` |
| Resizable | Freya default `true` | `config.rs:152` |
| Close hook | `with_on_close(... closing.requested())` | `cairn-app/src/main.rs:52` |
| Embedded font | `DIFF_FONT_FAMILY` ← `assets/fonts/IBMPlexMono-Regular.ttf` | `cairn-app/src/main.rs:40,55` |
| Theme | `use_init_theme(dark_theme)` — dark always; no light theme, no following the system's scheme | `cairn-app/src/main.rs:61` |
| History page size | `PAGE_ROWS = 64` | `cairn-app/src/main.rs:35` |
| Repository path | first CLI argument (empty ignored), else `current_dir()`, else `"."`; later arguments ignored | `cairn-app/src/repository_path.rs:7-21` |

Freya's dark theme (`freya-components/src/theming/themes.rs:116-157`) supplies every colour the
shell uses: `background` rgb(20,20,20); `surface_primary` (60,60,60); `surface_secondary`
(45,45,45); `surface_tertiary` (25,25,25); `border` (60,60,60); `text_primary` (250,250,250);
`text_secondary` (210,210,210); `text_placeholder` (150,150,150); `warning` (255,213,79);
`error` (229,115,115); `primary` (103,80,164), replaced by the platform accent colour when one
is reported (`themes.rs:712-725`).

At open (`main.rs:140-207`): `worker::open` on the chosen path; on success it submits
`Request::ListRemotes`, `Request::ConfiguredContext`, `Request::Refresh` (the refresh's refs
then ask for the history), and spawns the update loop feeding `session::apply`. Remotes are
listed **only at open** (`main.rs:159`; no other call site) — a remote added later is not
offered until restart. When the update stream ends: the window closes if a close was asked
(`main.rs:189-192`), otherwise `progress.stream_ended("the repository worker has stopped")`
(`main.rs:195-197`). If the worker thread itself cannot start, `progress.failed(error)`
(`main.rs:202-204`) and the window runs with `submit = None` (no Fetch, no actions).

#### Layout tree (`window::window`, `cairn-app/src/window.rs:123-272`)

```
rect().expanded().theme_background()            window.rs:194-196   on_global_key_down/up (199-209)
├─ title_bar(...)                                window.rs:210-233 → 538-569
│   ├─ label "Cairn"                              553
│   ├─ rect flex(1) clip  ← pressable status box  555-558 (box: 213-227)
│   ├─ label "<n> commits[…]"                     560-566
│   └─ Fetch button (optional)                    567 → 576-602
├─ leftover-lock banner (optional)               236  (operations/leftover_lock.rs:142-155)
├─ closing banner (optional)                     237  (banner(): 623-638)
├─ beside(sidebar, main)  ResizableContainer H    238-245 → 300-319
│   ├─ panel px(sidebar width), min 140           SidebarPane
│   └─ panel 100%, min_pixels 240                  main region:
│        MainView::AllCommits → split(list, DetailPane)   241 → 323-360
│        MainView::LocalChanges → LocalChangesPane         242
├─ the one modal on top (optional)               252-268 (crate::modal::drawn)
└─ ContextMenuViewer                              270
```

The commit list (`list`, `window.rs:173-192`): `HistoryHeader` (column headings "Graph and
subject", "Author", "Commit", "Date (UTC)" and the "Show Lost Commits" check box —
`cairn-ui/src/history/commit_row.rs:199,294-316`), then either a centred `notice` (placeholder)
or the virtualized `HistoryList` (`window.rs:393-471`), then — only when rows exist and the
status is `Failed` — `list_line(message)` under the rows.

`split` (`window.rs:323-360`): vertical `ResizableContainer`, list panel 100% with
`min_pixels(LIST_MIN_HEIGHT)`, pane panel `px(pane_height)` with `min_size(PANE_MIN_HEIGHT)`,
`on_resized` stores the height, `on_collapse` sets `pane_collapsed`. Collapsed: a flex column,
list `flex(1)` over a rect of `DETAIL_STRIP_HEIGHT` holding the pane (its tab strip only).

Main-region switch: `view.sidebar.main: State<MainView>` (enum
`cairn-ui/src/sidebar.rs:44-48`, default `AllCommits`). Set by the sidebar's two entries
(`cairn-ui/src/sidebar.rs:292-301`, wired `cairn-app/src/sidebar/pane.rs:83`) and forced to
`AllCommits` by any ref/stash press or row find (`cairn-app/src/history/ref_find.rs:120,146`).
The two views are exclusive: Local Changes replaces the list **and** the detail pane.

#### Layout constants (shell)

| Name | Value | Anchor |
| --- | --- | --- |
| `PANE_HEIGHT` (detail pane until dragged) | 260.0 | `cairn-app/src/window.rs:33` |
| `PANE_MIN_HEIGHT` (drag past → collapse) | 90.0 | `window.rs:35` |
| `LIST_MIN_HEIGHT` | 80.0 | `window.rs:37` |
| `MAIN_MIN_WIDTH` | 240.0 | `window.rs:39` |
| `SIDEBAR_WIDTH` (initial) | 240.0 | `cairn-app/src/sidebar/state.rs:24` |
| `SIDEBAR_MIN_WIDTH` | 140.0 | `sidebar/state.rs:26` |
| `DETAIL_STRIP_HEIGHT` (collapsed pane) | 30.0 | `cairn-ui/src/detail/tabs.rs:8` |
| `ROW_HEIGHT` (history row, header, list_line min) | 26.0 | `cairn-ui/src/history/graph_geometry.rs:5` |
| Changes tab `LIST_WIDTH` (percent) | 35.0 | `cairn-app/src/detail/changes_tab.rs:42` |
| Splitter handle | Freya `HANDLE_SIZE = 4.0`; idle `surface_secondary`, hover/drag `surface_primary`, radius 6 | `freya-components/src/resizable_container.rs:143`; `themes.rs:481-486` |
| Splitter widths/heights are **peeked**, not read, so a drag doesn't redraw the window | — | `window.rs:302-304,342-344` |

#### Interactions (shell)

| Interaction | What | Anchor |
| --- | --- | --- |
| Any key (global) | held-key tracking; `resolve_key(Scope::Window)` → `shortcuts::act` | `window.rs:199-209` |
| Window loses focus | held keys cleared | `window.rs:147-152` |
| Window gains focus | `Request::Refresh` | `cairn-app/src/refresh/focus.rs:16-26` |
| Drag sidebar splitter | width stored for the session | `window.rs:310` |
| Drag pane splitter | height stored; drag past 90 collapses | `window.rs:355-356` |
| Scroll the history | supersedes a sidebar find | `window.rs:159-171` |
| Reach list end | `Request::MoreHistory { rows: 64 }` (not while a find pages) | `window.rs:455-469` |
| Window-scope chords | Refresh: Linux `F5`, macOS `⌘R`; New Branch: `Ctrl/⌘+Shift+B`; Commit tab `Ctrl/⌘+Alt+1`; Changes tab `Ctrl/⌘+Alt+2` | `cairn-ui/src/accelerators.rs:293-302,348,371-390`; `cairn-app/src/shortcuts.rs:84-92` |
| All keys inert | while any modal (prompt, confirmation, Git Error, Create Branch, popover) is up | `cairn-app/src/shortcuts.rs:30-36`; `modal.rs` |
| Close (window X) | first: `Request::Close`, keep open, closing line shown if a write runs; again after `CLOSE_PATIENCE` (5 s, `worker/pool.rs:69`) closes anyway; no repository / worker gone: closes at once | `cairn-app/src/closing.rs:63-82` |

#### Modal stacking (`cairn-app/src/modal.rs:16-70`)

One drawn at a time, priority: credential `Prompt` → `Confirmation` → `CommitError` →
`Ending` (oldest first) → `CreateBranch` → `Popover`. Lower ones are kept, not drawn.

Credential prompt (window side, `window.rs:363-391`): titled by `asking`, falling back to
`"git"`; title format `"{} is asking for a credential"`
(`cairn-ui/src/dialogs/credential_prompt.rs:199-202`). (Dialog body strings belong to the
dialogs section.)

### (b) Title bar

Assembled in `cairn-app/src/window.rs:538-569`; the status box component is
`cairn-ui/src/title_bar/status_box.rs`. It is a drawn row inside the client area, **under** the
native OS title bar (not a custom/CSD title bar).

#### Title bar row

| Element | Detail | Anchor |
| --- | --- | --- |
| Row | horizontal, `Content::Flex`, fill width, centre cross-aligned, `spacing(10.)`, `padding(8, 12, 8, 12)`; no background of its own (window background) and no bottom border | `window.rs:546-552` |
| App name | label `"Cairn"`, `theme_color()`, `font_size(16.)` | `window.rs:553` |
| Status box slot | `rect().width(flex(1)).overflow(Clip)`; the wrapping rect is pressable (`on_press` → `view.activity.open()`) and reports its `(min_x, max_y)` as the popover's anchor via `on_sized` | `window.rs:213-227,555-558` |
| Loaded count | label of `status_text::loaded_count`: `"{loaded} {noun}…"` while more is coming, `"{loaded} {noun}"` otherwise, noun `"commit"`/`"commits"`; `max_lines(1)`, `font_size(13.)`, colour `text_placeholder` | `window.rs:560-566`; `cairn-app/src/history/status_text.rs:16-26` |
| Fetch button | see below | `window.rs:576-602` |

#### Status box (`StatusBox`, `cairn-ui/src/title_bar/status_box.rs:153-221`)

Horizontal, centre-aligned, `spacing(8.)`. Children in order:

1. Repository name — `name_text`: `"{name}*"` when dirty, else `"{name}"`
   (`status_box.rs:96-102`); bold, `FONT_SIZE` 13, `text_primary` (`:187-194`). Name is the
   working tree's (or git dir's) folder name (`cairn-app/src/worker/pool.rs:251-257`), absent
   until `Update::Opened`. Dirty = last status `Listed` with ≥1 entry
   (`window.rs:491-496`).
2. Branch glyph — `RefGlyph::Branch.draw(text_secondary)`, only for a branch or unborn
   `HEAD` (`status_box.rs:159-162,195`); glyph `GLYPH_SIZE = 10.0`, stroke 1.3
   (`cairn-ui/src/history/ref_glyphs.rs:23,26`).
3. HEAD text — `head_text` (`status_box.rs:79-85`), 13, `text_primary`:
   - branch: its shorthand, e.g. `main`
   - detached: `"HEAD detached at {short id}"`
   - unborn: `"{name} ({NO_COMMITS_YET})"` with `NO_COMMITS_YET = "no commits yet"`
     (`:25`) — e.g. `trunk (no commits yet)`
4. Tracking (`:163-182`):
   - `Counts` → `counts_text`: `"{behind}↓"` then `"{ahead}↑"`, no space, zero omitted, empty
     when level (`:89-93`), e.g. `18↓1↑`; drawn in IBM Plex Mono (`DIFF_FONT_FAMILY`), 13,
     `text_secondary`; nothing drawn when level.
   - `Gone` → `UPSTREAM_GONE = "upstream gone"` (`:27`), 13, colour `warning`.
   - `Untold` (no upstream, detached/unborn, counts not read) → nothing.
   Decided in `window.rs:477-501`.
5. Running lines — one `RunningLine` per operation (`:204-215`, `:225-268`): nothing until
   `PROGRESS_AFTER` has elapsed, then `CircularLoader` `size(14.)` + label, `spacing(6.)`, 13,
   `text_secondary`. Text = `running_text` (`:49-59`): `"{name}"`, + `" · {n} waiting"` when
   writes queue, + two spaces and git's latest progress line for a fetch. Examples in source:
   `"Commit 1 file · 1 waiting"`, `"Fetch 'origin'  Receiving objects: 40%"`. The fetch's name
   is `"Fetch '{remote}'"` (`cairn-app/src/operations/activity.rs:164`). Built by
   `window::running` (`window.rs:506-536`).

| Constant | Value | Anchor |
| --- | --- | --- |
| `FONT_SIZE` | 13.0 | `status_box.rs:29` |
| `PROGRESS_AFTER` | 250 ms (`async_io::Timer`, a task of the line's scope) | `status_box.rs:31,240-249` |

Interaction: press anywhere on the status box opens the activity popover (hangs from its
bottom-left); Escape alone closes it (`cairn-app/src/shortcuts.rs:30-33`). There is no hover
state, tooltip or affordance saying the box is pressable.

#### Fetch button (`window.rs:571-602`)

- Freya `Button::new().compact()` — compact layout padding (3, 6, 3, 6), radius 6
  (`freya-components/src/theming/themes.rs:181-187`).
- Caption `format!("Fetch {remote}")` — e.g. `Fetch origin` (no quotes, unlike the activity
  name `Fetch 'origin'`). No tooltip, no icon.
- Remote = first of `repo.remotes()`, which sorts git's default fetch remote first
  (`cairn-git/src/remotes.rs:17-20`). No remote, or no repository → no button.
- `enabled(!in_flight)`: greyed while Starting/Running/Cancelling
  (`cairn-app/src/operations/fetch.rs:40-45`); the press sets `Starting` before submitting.
- No way to choose another remote, fetch all, or cancel here — cancel is in the activity popover.

#### Lock banner (`cairn-ui/src/title_bar/lock_banner.rs`)

Shown under the title bar when `<gitdir>/index.lock` exists **and** no write runs/queues
**and** no fetch is in flight (`cairn-app/src/operations/leftover_lock.rs:142-155`) **and** it
is ≥ `LOCK_STALE_AFTER` old (decided in the component, waiting on a timer).

| Item | Value | Anchor |
| --- | --- | --- |
| Text `LOCK_BANNER_TEXT` | "index.lock is in this repository, so git can't change the index. If no other git program is running, it was left behind." | `lock_banner.rs:16-17` |
| `LOCK_STALE_AFTER` | 10 s | `lock_banner.rs:19` |
| Button `REMOVE_LOCK_CAPTION` | "Remove index.lock…" (compact Button) | `cairn-ui/src/dialogs/git_error.rs:38`; `lock_banner.rs:106-111` |
| Layout | horizontal flex, fill, `spacing(12.)`, `padding(6, 12, 6, 12)`, background `surface_tertiary`; label 13, `text_primary`, `flex(1)`, wraps | `lock_banner.rs:90-105` |
| Confirmation title `REMOVE_LOCK_TITLE` | "Remove Stale Lock" | `cairn-app/src/operations/leftover_lock.rs:27` |
| Refusal name `REMOVE_LOCK_NAME` | "Remove index.lock" → dialog "Couldn't Remove index.lock" | `leftover_lock.rs:29,92-106` |

#### Closing banner (`window.rs:623-638`, text from `operations/local_writes.rs:131-138`)

`"Closing after {name} finishes. Close again to quit now."` — e.g. "Closing after Commit 1 file
finishes. Close again to quit now." Fill width, `padding(6, 12, 6, 12)`, background
`surface_tertiary`, label 13 `text_secondary`, wraps. Only while closing and a write runs.

### (c) Toolbar

**There is no toolbar.** The only button at the top of the window is `Fetch <remote>`
(`window.rs:567`), plus `Remove index.lock…` on the lock banner when it shows. A search of
`cairn-ui/src` and `cairn-app/src` production code finds no Pull, Push, Stash, Branch, Merge,
Open-in-terminal, Open-repository or Settings button anywhere at the window's top. (New Branch
exists only as a history row's context-menu item and the `Ctrl/⌘+Shift+B` chord; "Stash and
reapply" exists only inside Create Branch, greyed —
`cairn-ui/src/dialogs/create_branch.rs:54`.)

### (d) Menu bar

**None, native or drawn.** Freya's `menu` feature (muda; `freya/Cargo.toml:103`,
`freya-winit/src/config.rs:298-306`) is not enabled — `cairn-app` takes `freya` with default
features (`default = ["winit"]`), `cairn-ui` adds only `engine`. `main.rs` builds no menu;
`window.rs`'s only "menu" is the `ContextMenuViewer` host (`window.rs:270`) for right-click
menus. No repository picker, no recent repositories, no About/Preferences/Quit items; quitting
is the WM's close button only.

### (e) Window-level empty / loading / error states

#### History placeholder (`cairn-app/src/history/status_text.rs:6-13`)

Drawn by `notice(message, opened)` in place of the list (`window.rs:183-186,604-620`): centred
column, `spacing(6.)`; message label 14 `theme_color`; under it the opened path as typed, 12,
`text_placeholder`, one line, ellipsised. The header row (column headings, Show Lost Commits)
stays above it; title bar and sidebar stay drawn.

| Status | Text | Notes |
| --- | --- | --- |
| `Loading` | "Reading history…" | also while a page with 0 rows and more to come arrives (`history/state.rs:90-95`) |
| `Empty` | "No commits yet." | 0 rows and complete |
| `Failed`, no rows | the failure's own sentence | engine/worker text verbatim |
| `Failed`, rows loaded | list stays; `list_line(message)` under it: min height 26, `padding(0, 12, 0, 12)`, 13, colour `error` (`window.rs:641-656`) | |

Title-bar count during these: `"0 commits…"` while loading; `"{n} commits"` once failed
(a failure stops the ellipsis, `status_text.rs:19`).

Failure sentences that can reach the placeholder (verbatim templates):

| Source | Text | Anchor |
| --- | --- | --- |
| stream ended, no cause named | "the repository worker has stopped" | `cairn-app/src/main.rs:197` |
| worker died | "the repository worker stopped unexpectedly; reopen the repository to carry on" (there is no way to reopen in-app) | `cairn-app/src/worker/pool.rs:985-986` |
| worker thread not started | "could not start a repository worker for {path}: {source}" | `worker/pool.rs:213-217` |
| history full | "the history is too large to hold" | `cairn-model/src/history.rs:57-59` |
| not a repository | "no git repository at {path}" | `cairn-git/src/error.rs:10` |
| open failed | "failed to open the repository at {path}: {source}" | `error.rs:28` |
| replaced | "{path} now names the repository at {now}, not the one at {was} the application checked and opened" | `error.rs:18-21` |
| bare by search | "{path} is a bare repository found by searching, which git refuses because safe.bareRepository is explicit; it was not opened" | `error.rs:40-43` |
| dubious ownership | "detected dubious ownership in repository at {path}: it is not all yours, and safe.directory does not name it, so git refuses it; it was not opened (git config --global --add safe.directory {path} adds an exception)" | `error.rs:53-57` |
| uid unknown | "could not tell which user Cairn is running as, so whether git would open the repository at {path} — which it does only for its owner, or where safe.directory names it — cannot be decided; it was not opened (git config --global --add safe.directory {path} names it)" | `error.rs:66-71` |
| reftable | "the repository at {path} keeps its refs in {storage} (extensions.refStorage), which Cairn does not read; it was not opened" | `error.rs:80-83` |
| format version | "the repository at {path} sets extensions.refStorage = {storage} but its core.repositoryFormatVersion is 0, which git refuses (a version-1 extension in a version-0 repository); it was not opened" | `error.rs:91-95` |
| protected config | "failed to read the system or global git configuration: {source}" | `error.rs:100` |
| git missing | "git was not found on PATH (searched {dirs}); Cairn needs git {required} or newer" | `error.rs:188-191` |
| git too old | "git {found} at {path} is too old; Cairn needs git {required} or newer" | `error.rs:198` |
| version unreadable | "could not read the version of git at {path}: \`git --version\` printed {output:?}; Cairn needs git {required} or newer" | `error.rs:206-209` |
| git not started | "could not start {program}: {source}" | `error.rs:217` |
| refs refresh failed before any refs | the refresh's message (`Update::RefreshFailed` with no refs → `progress.failed`) | `cairn-app/src/session.rs:311-317` |

Engine messages are lower-case, technical and long; they are shown as-is, centred, at 14 px.
The failed-open window still draws the title bar ("Cairn", no name, `0 commits`), the sidebar
and the History header, with no Fetch (no `submit`).

#### Sidebar find lines (`cairn-app/src/history/ref_find.rs:49-62`)

Drawn by the sidebar (its section owns placement): "Finding {name}…", "{name} is not in the
graph", "{name} names no commit, so it is not in the graph"; a stash's name `"stash@{n}"`
(`:111`), a detached HEAD's `"HEAD"` (`cairn-ui/src/sidebar.rs:39`). `FIND_PAGE_ROWS = 512`
(`ref_find.rs:31`).

#### Fetch endings (`cairn-app/src/session.rs:131-174,327`)

- Success: nothing said; a refresh follows.
- Cancel: nothing drawn; the activity entry says `FETCH_CANCELLED` = "Cancelled while git ran.
  Whatever it had already fetched stays." (`cairn-app/src/operations/activity.rs:59`).
- Failure: an ending dialog via `of_failure("Fetch '{remote}'", ...)`.
- Refusal (`Update::FetchRefused`): **never drawn** — the button is greyed instead
  (`session.rs:325-327`; `worker/request.rs:610-612`).
- A prompt arriving with no fetch or write in flight is refused, not shown (`session.rs:331-336`).

#### Ending dialogs (`cairn-app/src/operations/ending_dialog.rs`)

| Ending | Home | Strings |
| --- | --- | --- |
| success, cancel, amend needing confirmation | nothing | — |
| git failure (has a command) | Git Error dialog | title `GIT_ERROR_TITLE` "Git Error"; sentence = failure's `said`; details = command + output; `Remove index.lock…` when git failed on the lock (`:80-92`) |
| failure beneath git | "Couldn't {name}" | `couldnt_title` → `format!("Couldn't {name}")` (`cairn-ui/src/dialogs/git_error.rs:41-43`) |
| refused / stale / not run | "Couldn't {name}", sentence `"{said} Nothing was changed."` (`NOTHING_CHANGED`, `cairn-ui/src/dialogs/confirm.rs:54`) | `:106-109,117-119` |
| partly done discard | "Couldn't {name}" + sentence, Show files list (`shown_path`) | `:121-144` |
| write name unknown | name `"A write"` | `session.rs:224-226`; `local_writes.rs:37` |

Other Git Error constants: `GIT_ERROR_TEXT` "An unexpected error occurred while performing the
git request.", `ERROR_DETAILS` "Error Details:", `CLOSE_CAPTION` "Close"
(`git_error.rs:33-35`); `DIALOG_WIDTH` 680, `OUTPUT_ROW_HEIGHT` 16, `OUTPUT_HEIGHT` 240,
`FILES_SHOWN` 10 (`git_error.rs:45-51`). Queue: oldest first, one at a time; Close pops it
(`ending_dialog.rs:43-77,150-174`). The commit box keeps its own Git Error (`Modal::CommitError`).

#### Tests pinning these surfaces

- Window states: `loading_and_an_empty_repository_draw_different_windows`,
  `every_view_state_puts_its_own_sentence_in_the_window`,
  `a_failure_with_nothing_loaded_is_said_once`,
  `a_placeholder_replaces_the_list_and_rows_replace_the_placeholder`,
  `a_failed_page_is_asked_for_again_on_the_next_approach_to_the_end` (`cairn-app/src/window.rs` tests).
- Title bar: `the_title_bar_names_the_repository_the_branch_and_how_far_it_is_from_its_upstream`
  (`window.rs:2842`; asserts "engine*", "main", "18↓1↑", "upstream gone", "HEAD detached at …",
  "trunk (no commits yet)"); `fetch_is_greyed_while_a_fetch_is_in_flight`;
  `the_status_box_is_handed_what_runs_by_its_one_name`;
  `the_window_names_the_write_it_waits_on_and_its_prompt`; unit tests
  `head_is_named_as_git_names_it`, `counts_are_behind_then_ahead_and_a_zero_is_left_out`,
  `a_dirty_repository_is_starred` (`status_box.rs:287-316`);
  `the_status_box_shows_an_operation_only_once_it_has_run_250_ms` (`cairn-ui/tests/status_box.rs:19`).
- Lock: `a_lock_is_stale_from_ten_seconds_old` (`lock_banner.rs:128`);
  `the_leftover_lock_banner_shows_exactly_while_it_should`,
  `a_write_refused_for_the_lock_offers_its_removal_through_the_confirmation`
  (`cairn-app/src/operations/tests.rs:352,244`).
- Popover opening: `the_popover_hangs_under_the_status_box`,
  `while_the_popover_is_open_the_keys_do_nothing_but_close_it` (`operations/tests.rs:724,756`).
- Shell: `the_splitter_drags_and_the_pane_keeps_its_height`,
  `gaining_focus_asks_for_a_refresh_and_nothing_else`, `the_refresh_chord_asks_for_a_refresh`,
  `the_keys_held_are_let_go_of_when_the_window_loses_focus`,
  `a_context_menu_opens_from_inside_the_window` (`window.rs` tests); closing tests in
  `cairn-app/src/closing.rs:110-205`; `history/status_text.rs:28-110` and
  `history/state.rs:138-364` unit tests.

### Structural notes

- **Assembly:** `cairn-app/src/main.rs` launches (title, font, theme, worker, update loop) and
  calls `window::window` (`main.rs:214`); `cairn-app/src/window.rs:123-272` assembles the whole
  tree, and also *builds* the title bar row, the Fetch button, the notice, both banner styles
  and the list line inline (`window.rs:538-656`) rather than in `cairn-ui` — only the status
  box, lock banner, header, list, sidebar, pane and dialogs are `cairn-ui` components.
  `cairn-ui/src/title_bar.rs` is just a module index (`:4-6`); there is no `TitleBar` component.
- **Constants are per file, no shared metrics/theme module.** Shell dimensions are `const`s at
  the top of `window.rs:32-39` and `sidebar/state.rs:24-26`; the title bar's own paddings,
  spacing and font sizes (8/12 padding, 10 spacing, 16 and 13 px) are inline literals in
  `window.rs:551-564`; banner paddings (6/12) are literals repeated in `window.rs:628` and
  `lock_banner.rs:96`. Font size 13 is redeclared as a local `FONT_SIZE` in
  `status_box.rs:29` and `cairn-ui/src/sidebar.rs:40`, and as `ROW_FONT_SIZE` in
  `history/commit_row.rs:20`. ~33 files across both crates declare their own `f32` layout
  constants. Colours come from Freya's dark theme tokens throughout; the only literal colours
  are in `cairn-ui/src/diff/palette.rs` and one in `history/ref_chips.rs`.
- Window title, size and min size are absent from config, so the window opens at Freya's
  700×500 with no minimum; the inner `min_pixels`/`min_size` of the splitters are the only floors.

## 02 — Sidebar and history list (as built)

Read-only audit of the code at `feature/plan-ui-review` (HEAD `967bb72`). Freya is the fork at rev
`caa46f8`; the app sets `use_init_theme(dark_theme)` (`crates/cairn-app/src/main.rs:61`), so every
theme token below resolves to Freya's `DARK_COLORS`
(`~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/crates/freya-components/src/theming/themes.rs:115`).

Theme tokens used on these surfaces (dark theme values):

| Token | RGB |
| --- | --- |
| `surface_secondary` (selection / chosen background) | (45, 45, 45) |
| `surface_primary` | (60, 60, 60) |
| `text_primary` | (250, 250, 250) |
| `text_secondary` | (210, 210, 210) |
| `text_placeholder` (column headings, lost-commit dimming, title-bar count) | (150, 150, 150) |
| `border` (header underline, check box border) | (60, 60, 60) |
| `border_focus` (list keyboard-focus outline) | (110, 110, 110) |
| `warning` (gone-upstream triangle) | (255, 213, 79) |
| `error` (history failure line) | (229, 115, 115) |
| `primary` (ticked check box fill) | (103, 80, 164) |
| `background` | (20, 20, 20) |

---

### (a) Sidebar

#### Files

- `crates/cairn-ui/src/sidebar.rs` — the component (`Sidebar`), `drawn_row`, `build_row`.
- `crates/cairn-app/src/sidebar/pane.rs` — `SidebarPane`, wiring (filter → worker, notice, counts).
- `crates/cairn-app/src/sidebar/state.rs` — `SidebarState`, width constants, disclosure, reveal.
- `crates/cairn-model/src/sidebar_rows.rs` — `SidebarRow`, `Disclosure`, `RefsSnapshot::sidebar_rows`, `natural_order`.
- `crates/cairn-app/src/history/ref_find.rs` — what a press does (select / find).
- `crates/cairn-ui/src/history/ref_glyphs.rs` — every glyph (shared with chips and title bar).
- `crates/cairn-app/src/window.rs:300` `beside()` — the sidebar splitter.

#### Layout, top to bottom (`sidebar.rs:289-321`)

Outer `rect().expanded()`, top padding 6 (`Gaps::new(6., 0., 0., 0.)`, `sidebar.rs:291`). Children:

1. **"Local Changes"** entry (`sidebar.rs:292`), then
2. **"All Commits"** entry (`sidebar.rs:297`) — both from the `entry` closure (`sidebar.rs:252-269`): full width,
   height `SIDEBAR_ROW_HEIGHT` 22, text vertically centred, padding left/right 8, label 13 px
   (`FONT_SIZE`) `text_primary`, one line. The one matching the main view gets background
   `surface_secondary`. **No glyph/icon** on either entry. Press sets `MainView`.
3. **Filter box** (`sidebar.rs:302-309`): wrapper padding 6 all round; `text_field(..)`
   `.placeholder("Filter")` `.compact()` full width. (Freya `Input` compact layout variant, via
   `cairn_ui::text_field`.)
4. **Notice line** (`sidebar.rs:310-320`), only when there is one: padding (top 0, right 8, bottom 4,
   left 8), label 12 px `text_secondary`, wraps (no `max_lines`).
5. **Ref list** (`sidebar.rs:270-288`): `VirtualScrollView` over `SidebarRow`s, `item_size` 22, expanded.
   Before the first rows arrive: an empty expanded rect (no "loading" text).

There is no section header strip, no toolbar, no repository picker, no search icon in the sidebar.

#### Strings (verbatim)

| String | Where |
| --- | --- |
| `"Local Changes"` | `LOCAL_CHANGES_CAPTION`, `sidebar.rs:35` |
| `"Local Changes (N)"` (format `"{LOCAL_CHANGES_CAPTION} ({count})"`; count omitted when 0 or unknown) | `local_changes_text`, `sidebar.rs:61-66` |
| `"All Commits"` | `ALL_COMMITS_CAPTION`, `sidebar.rs:37` |
| `"Filter"` (placeholder) | `SIDEBAR_FILTER_PLACEHOLDER`, `sidebar.rs:33` |
| `"Branches"`, `"Remotes"`, `"Tags"`, `"Stashes"` | `section_caption`, `sidebar.rs:51-58` |
| `"HEAD"` (detached HEAD row, first under Branches) | `DETACHED_HEAD_CAPTION`, `sidebar.rs:39` |
| `"Finding <name>…"` | `finding_text`, `ref_find.rs:50-52` |
| `"<name> is not in the graph"` | `not_in_graph_text`, `ref_find.rs:55-57` |
| `"<name> names no commit, so it is not in the graph"` | `no_commit_text`, `ref_find.rs:60-62` |
| `"The refs could not be read: <message>"` (notice fallback when no press notice) | `pane.rs:67-71` |
| `"stash@{n}"` (the find's name for a stash press) | `ref_find.rs:111` |
| `"n↓m↑"` ahead/behind (behind first, no space, zero side omitted, empty when level) | `counts_text`, `crates/cairn-ui/src/title_bar/status_box.rs:89-93` |

`<name>` in the find strings is `RefName::shorthand()` (e.g. `main`, `origin/main`, `v1.0`), or
`"HEAD"`, or `stash@{n}` (`ref_find.rs:90-113`).

Local Changes' count = distinct paths the last status listed; `None` (so no count) before a status,
for an unreadable index, or with no working tree (`pane.rs:39-45`).

#### Constants and dimensions

| Name | Value | Anchor |
| --- | --- | --- |
| `SIDEBAR_ROW_HEIGHT` (every row incl. the two entries) | 22.0 | `sidebar.rs:29` |
| `SIDEBAR_INDENT` (per folder level) | 14.0 | `sidebar.rs:31` |
| `FONT_SIZE` (entries, ref names, folder names) | 13.0 | `sidebar.rs:40` |
| Section caption font | 12.0, bold, `text_secondary` | `sidebar.rs:488-494` |
| Counts label font | 12.0, `DIFF_FONT_FAMILY` (IBM Plex Mono), `text_secondary` | `sidebar.rs:496-503` |
| Row left padding | `8 + depth × 14` | `sidebar.rs:462` |
| Row right padding | 8 | `sidebar.rs:471` |
| Spacing between glyphs/name/counts in a row | 5 | `sidebar.rs:472` |
| `GLYPH_SIZE` (every glyph square) | 10.0 | `ref_glyphs.rs:23` |
| Glyph `STROKE` | 1.3 (check mark 1.6) | `ref_glyphs.rs:26`, `:135` |
| Notice font | 12.0 | `sidebar.rs:317` |
| Sidebar top padding | 6 | `sidebar.rs:291` |
| Filter wrapper padding | 6 | `sidebar.rs:303` |
| `SIDEBAR_WIDTH` (default) | 240.0 px | `crates/cairn-app/src/sidebar/state.rs:24` |
| `SIDEBAR_MIN_WIDTH` | 140.0 px | `state.rs:26` |
| Max width | none set (no `.max_size`); bounded only by the main region's `MAIN_MIN_WIDTH` 240.0 | `window.rs:39`, `:305-318` |
| Splitter handle thickness | Freya `ResizableContext::HANDLE_SIZE` = 4.0 | freya `resizable_container.rs:144` |
| `FIND_PAGE_ROWS` (rows per find page) | 512 | `ref_find.rs:31` |

Splitter (`window.rs:300-319`): horizontal `ResizableContainer`; sidebar panel `PanelSize::px(width)`,
`.min_size(140)`, `on_resized` stores the dragged width in session state (`SidebarView::width`, not
persisted across runs); main panel `PanelSize::percent(100.)` `.min_pixels(240)`. **No `on_collapse`** —
the sidebar cannot be collapsed by drag, and there is no toggle to hide it.

#### What a row draws (`drawn_row`, `sidebar.rs:349-434`; `build_row`, `sidebar.rs:444-510`)

Row = horizontal flex, cross-centred: [disclosure triangle (folders only)] [glyph] [name, flex 1,
ellipsis] [counts] [warning triangle]. Chosen row background `surface_secondary`. Glyph colour
`text_secondary`, except `Gone` in `warning`.

| Row kind | Glyph | Disclosure | Text | Depth/indent | Bold | Extras |
| --- | --- | --- | --- | --- | --- | --- |
| Section | ▼ `Opened` / ▶ `Closed` *as its glyph* (no separate triangle) | yes | caption, 12 px bold `text_secondary` | 0 → 8 px | yes | — |
| Folder | `Folder` outline, preceded by ▼/▶ triangle | yes | the path part at that depth (`folder_name`) | depth → `8+14d` | no | — |
| Detached HEAD | `Current` (check) | — | `"HEAD"` | 0 | yes | — |
| Stash | `Stash` (box) | — | the stash reflog message, e.g. `On main: wip` / `WIP on main: 1234567 subject` (`refs.rs:119`) | 0 | no | — |
| Local branch, current | `Current` (check mark replaces branch glyph) | — | last `/` segment | folder depth | **yes** | if upstream gone: warning triangle at the row's right (check stays) |
| Local branch, upstream exists | `Branch` (two commits + fork) | — | last segment | depth | no | `n↓m↑` counts at right, monospace 12 px (omitted when level or not yet counted) |
| Local branch, upstream gone | `Gone` (warning triangle, `warning` colour) | — | last segment | depth | no | no counts |
| Local branch, no upstream | `LocalOnly` (one line, newest hollow) | — | last segment | depth | no | — |
| Remote-tracking | `Remote` (filled cloud) | — | last segment; a symbolic one (`origin/HEAD`) keeps `origin/HEAD` whole | depth | no | — |
| Tag | `Tag` (label outline with hole) | — | full shorthand (tags have no folders) | 0 | no | — |

Note: refs directly under a section sit at indent 8 — the same x as the section caption's
triangle; only folder contents step in. The current branch's counts *are* drawn (the `counts`
logic does not exclude current); only Gone suppresses them (`sidebar.rs:409-415`).

Glyph shapes (`ref_glyphs.rs:85-228`, all in a 10×10 square, one colour, anti-aliased): Tag — pentagon
pointing left (0.8,5)→(4,1.2)→(9.2,1.2)→(9.2,8.8)→(4,8.8) + filled hole r 1.1 at (4.6,5); Remote —
three filled circles (r 2.4, 3.0, 2.0) on a filled base rect; Stash — outlined lid + body + slot line;
Current — check (1.2,5.4)→(4,8.2)→(8.8,1.8), stroke 1.6; Branch — vertical line, cubic fork, three
filled dots r 1.4; LocalOnly — vertical line, hollow top dot r 1.4, filled bottom dot r 1.5; Folder —
outlined tab+body; Gone — outlined triangle, line + dot r 0.8; Opened — filled down triangle; Closed —
filled right triangle; Submodule — outlined box, filled inner box. Pinned distinct by
`every_glyph_paints_a_shape_no_other_glyph_paints` (`ref_glyphs.rs:257`).

#### Ordering and filtering (`sidebar_rows.rs`)

- Sections always in order Branches, Remotes, Tags, Stashes (`sidebar_rows.rs:36`); each caption is
  always drawn (even an empty section) unless filtering.
- Branches and Remotes are split into folders at `/` (`has_folders`, `:50-55`); Tags never; Stashes in
  stash-list order.
- At each level: folders first, then refs, each in Fork's natural order — case ignored, digit runs as
  numbers (`natural_order`, `:176-208`).
- Remotes: the remote name (`origin`) is itself a folder (`folder_name` doc, `:137-139`) — so remotes
  appear as Remotes ▸ 📁 origin ▸ refs.
- Disclosure defaults: **a section is open until closed; a folder is closed until opened**
  (`Disclosure`, `:58-65`). The current branch's folders are opened automatically whenever `HEAD`
  changes (first snapshot and checkout), not on every refresh (`state.rs:117-125`; test
  `the_current_branchs_folders_open_when_it_becomes_current`).
- Filter: substring anywhere in the shorthand name (refs) or the message (stashes), case-insensitive
  Unicode-lowercased, no wildcards (`text_filter.rs:1-3`, `refs.rs:198-224`). While text is present
  every section/folder is drawn open and a section with no match loses its caption
  (`sidebar_rows.rs:255`, `:312-315`). Detached `HEAD` row is listed only if `"HEAD"` matches the filter
  (`:271-276`). Filtering runs on the repository thread (`Request::FilterRefs`); the previous rows stay
  drawn until the answer arrives (`state.rs:5-9`).

#### Interactions

| Gesture | Effect | Anchor |
| --- | --- | --- |
| Press "Local Changes" / "All Commits" | switches the main region (`MainView`) | `sidebar.rs:261`, `pane.rs:83` |
| Press a section caption or folder row (anywhere on the row) | toggles open/closed, rows re-asked | `ref_find.rs:84-87`, `state.rs:139-152` |
| Press a branch / remote / tag / detached HEAD / stash | switches to All Commits; marks the row chosen; selects its commit's row in the history and scrolls to it (`reveal_row`); if not loaded, pages the walk forward (`Request::FindRow`, 512 rows/page) showing "Finding <name>…"; if the walk ends without it, "<name> is not in the graph" (a stash's changes are still shown, no row selected); a ref naming no commit: "<name> names no commit, so it is not in the graph" | `ref_find.rs:79-137`, `:153-202`, `:333-345` |
| Next press / scroll of the history / a row chosen in the history | supersedes the find; choosing a history row clears the sidebar's chosen entry and notice | `ref_find.rs:256-277`, `window.rs:160-171` |
| Double-click | **NONE** (no handler; a double press = two presses, i.e. a toggle twice on a folder) | `sidebar.rs:474` only `on_press` |
| Right-click / context menu | **NONE** (no `ContextMenu` in sidebar files) | — |
| Hover | **NONE** (no hover state, no tooltip) | — |
| Keyboard navigation in the list | **NONE** (no `on_key_down`, not focusable); the filter field takes the shared text-field key policy | — |
| Drag a ref (e.g. to merge/rebase) | **NONE** | — |
| Checkout on double-click, ahead/behind actions, pin/favourite, "Show in graph" | **NONE** | — |

#### Tests pinning the sidebar

`crates/cairn-ui/tests/sidebar.rs`: `the_sidebar_draws_its_sections_in_forks_order_with_forks_marks`,
`each_entry_draws_its_kinds_glyph`, `the_entry_chosen_is_drawn_chosen_by_its_name`,
`a_press_is_reported_for_what_it_pressed`, virtualization twins `a_sidebar_of_50000_refs_builds_one_viewport`
and `a_folder_of_10000_branches_open_builds_one_viewport`. Model: `a_sidebar_row_is_eight_bytes`
(`sidebar_rows.rs:426`). App: `crates/cairn-app/src/sidebar/tests.rs`, `pane.rs:97`
(`the_count_beside_local_changes_is_of_distinct_paths`), `state.rs:219`, `:237`.

---

### (b) History list

#### Files

- `crates/cairn-ui/src/history/list.rs` — `HistoryList` (virtualized list, keys, press, context menu).
- `crates/cairn-ui/src/history/commit_row.rs` — `CommitRow` (one row), `HistoryHeader` (headings + Show Lost Commits).
- `crates/cairn-ui/src/history/graph_geometry.rs`, `graph_cell.rs`, `lane_palette.rs` — the graph.
- `crates/cairn-ui/src/history/ref_chips.rs`, `ref_glyphs.rs` — chips.
- `crates/cairn-ui/src/history/date_text.rs` — date text.
- `crates/cairn-app/src/window.rs:173-192` (list + header + placeholder), `:393-472` (`history()` wiring), `:604-656` (`notice`, `list_line`), `:536-566` (title-bar count).
- `crates/cairn-app/src/history/{selection,ref_find,lost_commits,status_text,state}.rs`.

#### Structure

`window.rs:173-192`: a column of [`HistoryHeader` (height 26)] then either a centred placeholder
(`notice`) or the `HistoryList`; under it, when a page failed with rows already drawn, a red
`list_line`. The list sits in the upper panel of a vertical splitter over the detail pane
(`window.rs:323-360`): list `percent(100)` `min_pixels(LIST_MIN_HEIGHT = 80)`, pane default
`PANE_HEIGHT = 260` px, `PANE_MIN_HEIGHT = 90`, collapsible by over-drag to `DETAIL_STRIP_HEIGHT`
(`window.rs:33-37`).

#### Columns (`commit_row.rs:111-184`; headings `commit_row.rs:258-318`)

Row: horizontal flex, padding left/right `ROW_PADDING` 10, spacing `COLUMN_GAP` 10.

| Column | Heading (verbatim) | Width | Content | Colour |
| --- | --- | --- | --- | --- |
| Graph + chips + subject | `"Graph and subject"` | flex 1 (graph `lanes × 14`, clamped 1..24 lanes, then chips + subject in the rest, clipped) | graph canvas, chips, subject (ellipsis, one line, **bold on HEAD's row**) | subject `text_primary` |
| Author | `"Author"` | `AUTHOR_WIDTH` 150 | author name, ellipsis | `text_secondary` |
| Commit | `"Commit"` | `ID_WIDTH` 72 | short id, 7 hex (`Oid::SHORT_HEX`, `oid.rs:41`), no ellipsis | `text_secondary` |
| Date | `"Date (UTC)"` | `DATE_WIDTH` 132 ("Sized for `YYYY-MM-DD HH:MM`") | `utc_minutes(author_time)` | `text_secondary` |

- Headings: 13 px (`ROW_FONT_SIZE`), `text_placeholder`, one line; header row height 26, 1 px bottom
  border in `border` (`commit_row.rs:281-286`).
- **Not resizable, not reorderable, not sortable, no column hide**: fixed `Size::px` widths, headings
  are plain labels with no handlers.
- Graph and subject share one heading; inside it graph and the chips/subject box are separated by
  `COLUMN_GAP` 10 (`commit_row.rs:127`).
- Date shown: **author** time, in **UTC**, format `YYYY-MM-DD HH:MM` (`date_text.rs:5-14`,
  e.g. `"2023-11-14 22:13"`). (`long_date` — `"25 Nov 2020 01:11:30 +01:00"`, at the commit's own
  offset — exists in the same file but is the Commit tab's, not the list's.)
- Room for chips+subject: `label_room = width − 2×10 − (150+72+132) − 3×10 − graph_width − 10`
  (`commit_row.rs:25-32`).

#### Row constants

| Name | Value | Anchor |
| --- | --- | --- |
| `ROW_HEIGHT` (row, header, virtual item size) | 26.0 | `graph_geometry.rs:5` |
| `ROW_FONT_SIZE` (subject, author, id, date, headings) | 13.0 | `commit_row.rs:20` |
| `ROW_PADDING` | 10.0 | `commit_row.rs:18` |
| `COLUMN_GAP` | 10.0 | `commit_row.rs:17` |
| `AUTHOR_WIDTH` / `ID_WIDTH` / `DATE_WIDTH` | 150 / 72 / 132 | `commit_row.rs:13-16` |
| `PREFETCH_ROWS` (rows from the end that ask for more) | 24 | `list.rs:13` |
| `PAGE_JUMP` (PageUp/PageDown) | 10 rows | `list.rs:23` |
| `PAGE_ROWS` (rows per history page) | 64 | `crates/cairn-app/src/main.rs:35` |

#### Graph geometry (`graph_geometry.rs`, `graph_cell.rs`)

| Name | Value | Anchor |
| --- | --- | --- |
| `LANE_WIDTH` (centre-to-centre) | 14.0 | `graph_geometry.rs:8` |
| `NODE_RADIUS` (dot or ring) | 4.0 | `:11` |
| `STROKE_WIDTH` (lines) | 1.8 | `:13` |
| `MAX_DRAWN_LANES` (lanes past share the last column) | 24 | `:16` |
| `RING_WIDTH` (merge ring stroke) | 2.0 | `graph_cell.rs:15` |
| `DASH` (out-of-order line: painted, gap) | [4.0, 3.0] | `graph_cell.rs:13` |
| Lane x | `(index + 0.5) × 14` | `graph_geometry.rs:67-70` |
| Node y | row middle, 13 | `:73-75` |

Node: ≤1 parent → filled dot; merge (>1 parent) → hollow ring (`graph_geometry.rs:100`,
`graph_cell.rs:39-49`), in the node's lane colour. Lines: straight when vertical; a lane change is a
cubic Bézier with control points at the vertical midpoint (`graph_cell.rs:62-71`). A line keeps the
colour of the track it runs in (not the node's) (`graph_geometry.rs:116-125`). A line to a parent
drawn *above* its child is dashed along its whole length. Graph column width = drawn lanes × 14,
minimum one lane; the list's lane count only grows (`state.rs` `Progress::lanes`).

#### Lane palette (`lane_palette.rs:6-15`) — Okabe–Ito less black, plus a grey; cycles by `lane % 8`

| Lane % 8 | RGB | Name |
| --- | --- | --- |
| 0 | (86, 180, 233) | sky blue |
| 1 | (230, 159, 0) | orange |
| 2 | (0, 158, 115) | bluish green |
| 3 | (204, 121, 167) | reddish purple |
| 4 | (240, 228, 66) | yellow |
| 5 | (0, 114, 178) | blue |
| 6 | (213, 94, 0) | vermillion |
| 7 | (190, 190, 190) | neutral grey |

#### Ref chips (`ref_chips.rs`)

| Name | Value | Anchor |
| --- | --- | --- |
| `CHIP_HEIGHT` | 18.0 | `:33` |
| `CHIP_GAP` (between chips, and last chip → subject) | 4.0 | `:35` |
| `CHIP_FONT_SIZE` | 12.0 | `:37` |
| `NAME_PADDING` (each side of name) | 5.0 | `:39` |
| `CAP_WIDTH` (glyph cap) | `GLYPH_SIZE + 6` = 16.0 | `:41` |
| `CHECK_GAP` (check mark → current name) | 3.0 | `:43` |
| `MIN_ADVANCE` (lower-bound char width for layout) | 2.0 | `:47` |
| `CORNER_RADIUS` (chip and cap) | 3.0 | `:48` |
| `TAG_INDIGO` | `Color::from_rgb(98, 92, 196)` | `:51` |
| Chip fill | tint with alpha 56 | `:263` |
| Chip border | tint, 1 px | `:264` |
| Cap fill | tint with alpha 140 | `:271` |
| Text & glyph colour | `text_primary` (250,250,250) | `:256` |

- **Tint**: the row's lane colour for branch, remote and stash chips; tags always indigo (`tint`, `:244-249`).
- **Kinds** (`ChipKind`, `:55-67`; `cap`, `:71-78`):
  - Local branch: plain chip, no cap; the **current** branch gets a check-mark glyph before its name
    (`:281`).
  - Local branch whose upstream labels the same row ("tracked"): a **Remote (cloud) cap** at its left —
    the branch and its upstream merge into **one chip**, the upstream drawing no chip of its own
    (`:211-241`, `:169-172`). Any other remote ref at that commit keeps its own chip.
  - Remote-tracking ref (untracked by a branch on the row): cloud cap + `origin/name`.
  - Tag: tag-glyph cap, indigo.
  - Stash row: one chip `"stash@{n}"` with the box-glyph cap (`Chip::stash`, `:97-102`).
- **Order** (`row_chips`, `:135-185`): the current branch first, then the row's labels in snapshot order
  (local branches, remote-tracking, tags; each bytewise by name).
- **Text**: `label.short_name()` (e.g. `main`, `origin/main`, `v1.0`) — not truncated with ellipsis;
  no "+N more" counter.
- **Compaction/clipping**: chips laid out left to right until a lower-bound width sum reaches the room;
  the chip crossing the edge is drawn and clipped by the container (`Overflow::Clip`,
  `commit_row.rs:143`); none after it is built. Many chips push the subject out entirely.
- Detached `HEAD` draws **no `HEAD` chip**; HEAD's row is marked only by its bold subject (`list.rs:470`,
  `commit_row.rs:155`).

#### Row states

| State | Drawing | Anchor |
| --- | --- | --- |
| Selected (and the second of a compared pair) | whole-row background `surface_secondary` (45,45,45); no accent bar, no text-colour change | `commit_row.rs:186-190`, `list.rs:472` |
| HEAD's commit | subject `FontWeight::BOLD` | `commit_row.rs:155` |
| Lost (Show Lost Commits, unreachable) | subject, author, id, date all `text_placeholder` (150,150,150); graph and chips keep their colours | `commit_row.rs:82-108` |
| Stash row | `CommitRow` of `stash.as_commit()`: subject = stash message, author/time of the stash, one parent (dot, never ring), its `stash@{n}` chip, a lane of its own with one line to its base; never bold, never dimmed | `window.rs:413-418`, `cairn-model/src/lib.rs:135-143` |
| Hover | **NONE** | — |
| Keyboard focus | list outlined with a 1 px `border_focus` (110,110,110) border only when focus came by keyboard | `list.rs:277-279` |

#### Interactions

| Gesture | Effect | Anchor |
| --- | --- | --- |
| Press a row | gives the list focus; selects the row (a pair returns to one); asks what it changed; supersedes a sidebar find and clears the sidebar's chosen entry | `list.rs:399-410`, `window.rs:436-443`, `selection.rs:88-99` |
| ⌘-press (macOS) / Ctrl-press (Linux) — `Action::ExtendSelection` | second commit of a comparison: exactly two, a third replaces the second; ⌘/Ctrl-press on one of the pair leaves the other alone; the lower row is the base; tip-against-tip diff | `list.rs:401-406`, `selection.rs:104-162`, `accelerators.rs:291` |
| Shift-press | **no range selection**: treated as a plain press (the list checks only `ExtendSelection`) | `list.rs:401-409` |
| ↓ / ↑ | next / previous row (clamped, no wrap); first key with nothing selected selects row 0; selection scrolled into view | `list.rs:350-363`, `:335-338` |
| PageDown / PageUp | ±10 rows | `list.rs:357-358` |
| Home / End | first / last **loaded** row | `list.rs:359-360` |
| Shift+↑/↓ | moves like ↑/↓ (another view's chord, `is_chord` leaves it to the list) | `accelerators.rs:460-470` |
| Ctrl+↑/↓ (⌘ on macOS) | not the list's: Previous/Next change in the detail pane | `list.rs:316`, `accelerators.rs:289-290` |
| Ctrl+Shift+. (⌘⇧. on macOS) — `Action::ShowLostCommits`, heard in `Scope::History` while the list has focus | toggles Show Lost Commits (reopens the history) | `list.rs:309-313`, `accelerators.rs:345`, `lost_commits.rs:12-34` |
| Right-click | selects the row unless it is already in the selection (Fork for Windows), then opens a context menu (commit rows only; a stash row has **no** menu) | `list.rs:416-449` |
| Context menu items | exactly one: `"New Branch…"` (`NEW_BRANCH_CAPTION`, `list.rs:16`) → opens Create Branch at that commit with its subject | `list.rs:439-448`, `window.rs:452-454` |
| Ctrl+Shift+B (⇧⌘B macOS) — `Action::NewBranch`, window scope | Create Branch at `HEAD` | `accelerators.rs:348` |
| Double-click | **NONE** (no checkout etc.) | — |
| Scroll near end | within the last 24 rows → `Request::MoreHistory { rows: 64 }` (debounced; not while a find pages) | `list.rs:411-413`, `window.rs:459-472` |
| Column header click / drag | **NONE** | — |
| Drag a row | **NONE** | — |
| Search/filter of commits | **NONE** in the list | — |

#### Show Lost Commits check box

- Lives in the history's heading strip, laid absolutely over the **right end of the "Graph and subject"
  heading cell**, just left of "Author" (`commit_row.rs:288-313`).
- Label verbatim `"Show Lost Commits"` (`SHOW_LOST_COMMITS_CAPTION`, `commit_row.rs:199`).
- Tooltip: the chord name from the table — Linux `"Ctrl+Shift+."`, macOS `"⌘⇧."`
  (`commit_row.rs:296-305`; `accelerators::chord_name`).
- Check box (`crates/cairn-ui/src/check_box.rs`): 14×14 mark, radius 3, 1 px inner border `border`,
  ticked fill `primary` (103,80,164) with `"✓"` 11 px `text_inverse`, unticked `surface_primary`;
  caption 13 px `text_primary`; spacing 6; pointer cursor; a11y CheckBox with toggled state.
- Pressing it or the chord flips the session toggle and reopens the history walk from every reflog
  entry of `HEAD` and each local branch (`lost_commits.rs:22-34`); not persisted.

#### Placeholder and status strings (verbatim)

| String | Where |
| --- | --- |
| `"Reading history…"` | `status_text.rs:8` (list placeholder while loading) |
| `"No commits yet."` | `status_text.rs:9` |
| `<engine failure message>` (with nothing loaded) + the repository path under it, 12 px `text_placeholder` | `status_text.rs:10`, `window.rs:604-620` (message 14 px theme colour) |
| `"the history is too large to hold"` (`HistoryFull`) | `cairn-model/src/history.rs:57-60` |
| Failure under drawn rows: message in `error` colour, 13 px, padding 0/12, min height 26 | `window.rs:641-656` |
| Title-bar count `"N commits…"` while more is coming, `"N commits"` / `"1 commit"` when done or failed | `status_text.rs:16-26`; drawn 13 px `text_placeholder` in the title bar, `window.rs:560-565` |
| `"Graph and subject"`, `"Author"`, `"Commit"`, `"Date (UTC)"` | `commit_row.rs:294`, `:315-317` |
| `"Show Lost Commits"` | `commit_row.rs:199` |
| `"New Branch…"` | `list.rs:16` |
| `"stash@{n}"` | `ref_chips.rs:100` |

#### Tests pinning the history list

`crates/cairn-ui/tests/history_list.rs`: virtualization twins
`only_a_viewport_of_rows_is_built_however_long_the_history` and
`only_a_viewport_of_labelled_rows_is_built_and_each_lays_out_a_columns_worth_of_chips`;
`an_accelerators_chord_does_not_move_the_selection`, `the_show_lost_commits_chord_is_reported_and_moves_nothing`,
`a_lost_row_is_drawn_lost_and_no_other`, `another_views_chord_is_the_history_lists_arrow`,
`a_click_selects_that_row_and_reports_its_identity`, `keys_move_the_selection_and_report_each_row_they_land_on`,
`a_selection_moved_off_screen_is_scrolled_into_view`, `a_click_gives_the_list_the_keyboard`,
`the_list_is_outlined_only_while_it_has_keyboard_focus`, `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew`,
`the_list_moves_from_the_row_its_callers_hint_names`, `every_commit_rows_menu_offers_new_branch_at_its_commit`,
`a_right_click_selects_its_row_and_keeps_a_selection_it_lands_in`. Also `tests/commit_row.rs`,
`tests/ref_chips.rs`, `tests/drawn_lanes.rs`, `tests/drawn_rows.rs`; unit tests in `list.rs`
(`a_rows_chips_are_laid_out_once_per_row_built_and_never_per_frame`, key-movement tests),
`graph_geometry.rs`, `graph_cell.rs`, `lane_palette.rs` (`the_palette_is_the_okabe_ito_set_less_black_plus_a_neutral`),
`date_text.rs`.

---

### Gaps against a Fork-like UX worth noting (observations, not recommendations)

- Sidebar: no icons on Local Changes / All Commits; no context menu, double-click checkout, drag,
  hover or keyboard navigation; the sidebar cannot be collapsed; width not persisted.
- History: fixed, non-resizable columns; one date format (UTC, minutes, author time) with "(UTC)" in
  the heading; no hover highlight; selection is a flat grey (45,45,45), identical to the sidebar's chosen
  row; no Shift range selection; one context-menu item; no double-click; no commit search/filter; no
  `HEAD` chip for a detached HEAD.

## 03 — Detail pane and diff view (as built)

Paths are relative to the repository root; `ui/` = `crates/cairn-ui/src/`, `app/` = `crates/cairn-app/src/`. The window uses Freya's built-in `dark_theme` (`app/main.rs:61`), so every "theme" colour below is `DARK_COLORS` of the pinned fork (`~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/crates/freya-components/src/theming/themes.rs:116-158`): `text_primary` #FAFAFA, `text_secondary` #D2D2D2, `text_placeholder` #969696, `text_highlight` #6091E0, `text_inverse` #FFFFFF, `border` #3C3C3C, `border_focus` #6E6E6E, `surface_secondary` #2D2D2D, `surface_tertiary` #191919, `background` #141414, `disabled` #323232, `success` #81C784, `warning` #FFD54F, `error` #E57373, `info` #64B5F6. A label with no `font_size` gets Freya's default, 16 px (`freya-core/src/style/font_size.rs:12`); `Button::compact()` is padding 3/6/3/6, radius 6, `surface_tertiary` ground, `surface_secondary` hover, 1 px `border` (`themes.rs:182-215`). The only embedded font is IBM Plex Mono Regular; everything not set to it is the system UI font, and a bold Plex weight is not embedded.

### A. History / detail splitter

| What | Value | Anchor |
| --- | --- | --- |
| Layout | vertical `ResizableContainer`: list panel `PanelSize::percent(100.)` over pane panel `PanelSize::px(height)` | `app/window.rs:323-360` (`split`) |
| Default pane height | `PANE_HEIGHT` = 260 px | `app/window.rs:33`, `app/main.rs:78` |
| Pane minimum on drag | `PANE_MIN_HEIGHT` = 90 px (`.min_size`) | `app/window.rs:35`, `:354` |
| List minimum | `LIST_MIN_HEIGHT` = 80 px (`.min_pixels`) | `app/window.rs:37`, `:349` |
| Handle | Freya `ResizableContext::HANDLE_SIZE` = 4 px | freya `resizable_container.rs:144` |
| Drag-to-collapse | dragging the pane `COLLAPSE_OVER_DRAG` = 24 px past its 90 px floor fires `on_collapse` → `pane_collapsed = true` ("Fork's gesture") | freya `resizable_container.rs:149`; `app/window.rs:356` |
| Collapsed | no splitter; list `Size::flex(1.)` over a `DETAIL_STRIP_HEIGHT` (30 px) box holding only the tab strip | `app/window.rs:329-340`, `ui/detail/tabs.rs:8` |
| Persisted | no — session `State` only (`pane_height`, `pane_collapsed`) | `app/main.rs:77-78` |
| Dragged height | written on every drag step (`on_resized` → `pane_height.set`); peeked, not read, so the window does not redraw during a drag | `app/window.rs:342-355` |

### B. Tab strip (`ui/detail/tabs.rs`)

- Tabs, in order: **"Commit"**, **"Changes"** (`DetailTab::caption`, `tabs.rs:23-28`); Commit is the default (`tabs.rs:15`). No counts. File Tree tab is issue #31 (`tabs.rs:12`).
- Right-aligned collapse control: a compact `Button` reading **"Collapse"** while open, **"Expand"** while collapsed (`COLLAPSE_CAPTION`/`EXPAND_CAPTION`, `tabs.rs:32-34`, drawn `:184-197`). Default 16 px button text (no size set).
- Strip: height 30 px (`DETAIL_STRIP_HEIGHT`), padding 0/12/0/12, spacing 2, background `surface_tertiary`, 1 px `border` top and bottom, children aligned to the bottom (`tabs.rs:120-139`).
- Each tab: full height, padding 0/12/0/12, label 12 px (`TAB_FONT_SIZE`, `tabs.rs:10`), one line. Shown tab: 2 px bottom border in `text_highlight` and text `text_primary`; others `text_placeholder`; unavailable `disabled` (`tabs.rs:141-180`). A11y role Tab.
- Unavailable state: the Commit tab while two commits are selected; drawn disabled, press ignored (`tabs.rs:62-67`, `:164-166`); the pane then forces the Changes tab, keeping the session's chosen tab for later (`app/detail/pane.rs:79-92`). Pressing a tab on a collapsed pane also opens the pane (`pane.rs:93-97`).
- Chords: Show Commit tab **Ctrl+Alt+1** / **⌘⌥1**, Show Changes tab **Ctrl+Alt+2** / **⌘⌥2** (physical digit keys; `ui/accelerators.rs:295-296`), also uncollapsing (`app/shortcuts.rs:55-56`, `:95-98`). Previous/next change **Ctrl+↑ / Ctrl+↓** (⌘↑/⌘↓), heard in `Scope::Detail` only — from inside the pane (`accelerators.rs:289-290`, `app/detail/pane.rs:105-110`).

Pane-level notices (`app/detail/pane.rs:33-38`, drawn by `notice` `:258-271`: centred, padding 12, 13 px, `text_placeholder`, or `error` when alarming):
- **"Select a commit to see its details."** (`NOTHING_SELECTED`)
- **"Reading the commit…"** (`READING`)
- **"The Commit tab describes one commit."** (`NOT_ONE_COMMIT`, a change set with no details)
- an engine failure's message, in `error` colour.

### C. Commit tab (`ui/detail/commit_tab.rs`; wired by `app/detail/pane.rs:148-232`)

One `VirtualScrollView` of equal rows, `DETAIL_ROW_HEIGHT` = **24 px** (`commit_tab.rs:50`), header + message + files + opened diffs, plus `end_room` rows (`END_ROOM` = 16+4 = 20 px, `ui/end_room.rs:15-19`). Message lines are never wrapped; wide lines scroll sideways. Keyed by commit id, so a new commit opens at the top with nothing current (`:556-558`). No avatar (`:23`).

Constants (`commit_tab.rs:69-75`): `FONT_SIZE` 13, `SUBJECT_FONT_SIZE` 15, `CAPTION_FONT_SIZE` 11, `CAPTION_WIDTH` 72, `BADGE_WIDTH` 18, `PADDING` 12 (row left/right), `GAP` 10.

Header rows in order (`header_lines`, `:171-215`; drawn `header_row`, `:777-854`):

| Row | Content | Style |
| --- | --- | --- |
| Captions | **"AUTHOR"** left half, **"COMMITTER"** right half (`:78-80`) | 11 px `text_placeholder`, two equal flex columns, ellipsis (`columns`, `:857-877`) |
| People | `Name <email>` each side (`person`, `:880-887`) | 13 px `text_primary` (one colour for name and email despite the doc comment) |
| Dates | `long_date`: `DD Mon YYYY HH:MM:SS +HH:MM` at the recorded offset, Fork Windows form (`ui/history/date_text.rs:27-47`) | 13 px `text_secondary` |
| REFS (only if chips) | caption **"REFS"** right-aligned in 72 px, then the history row's chips in its lane colour, clipped at the pane edge, never wrapped or counted (`:787-805`); chips laid out for the window's width (`app/detail/pane.rs:217-232`) | caption 11 px `text_placeholder` |
| SHA | caption **"SHA"** (right-aligned, 72 px) + full hex id (`:806-811`) | 13 px `text_primary`, IBM Plex Mono |
| PARENTS (if any) | caption **"PARENTS"** + each parent's short id as a link, underlined, pointer cursor, a11y Link (`:812-837`) | 13 px `text_highlight`, Plex Mono; press selects a loaded parent and scrolls history to it, unloaded does nothing (issue #3) (`app/detail/pane.rs:246-256`) |
| Rule | 1 px line, `border` colour (`:838-842`) | |
| Subject | message line 1 (`git log` form: trailing whitespace trimmed, leading/trailing blank lines dropped, tabs to 8 columns; `ui/detail/message_lines.rs:23-61`) | 15 px bold `text_primary` |
| Body | each further line | 13 px `text_primary` |
| Rule | 1 px `border` | |
| Rename cut short (if any) | **"Exhaustive rename detection was skipped due to too many files: set diff.renameLimit to at least {N} to pair every rename."** (`cut_short_notice`, `:93-98`) | 13 px `warning` |
| No files | **"No files changed."** (`NO_FILES`, `:89`) | 13 px `text_placeholder` |
| Files bar | right-aligned compact button **"Expand All"** / **"Collapse All"** while any file is open (`:53-55`, `files_bar` `:725-759`); left of it, when Expand All stopped at its budget: **"Expand All stopped at its line budget: {N} file{s} left collapsed."** (`budget_notice`, `:61-67`, comma-grouped) | notice 13 px `text_secondary` |

Expand All budget: `EXPAND_ALL_LINES` = 50,000 lines (`app/worker/diff/expand_all.rs:18`). Expand All never opens the entire-file context (`app/diff/actions.rs:30-35`).

File row (`file_item` `:594-623`, `file_row` `:904-923`): disclosure chevron (`Glyph::Closed` → right, `Glyph::Open` → down, `text_secondary`), then status letter **A / D / M / T / R / C** (no similarity score; `status_letter` `:102-111`) in an 18 px box, bold 13 px, coloured A `success`, D `error`, M/T `warning`, R/C `info`; then path (rename/copy **"old → new"**, `file_text` `:115-125`) 13 px `text_primary` Plex Mono. Current file background `surface_secondary`. Press: focuses the tab, makes it current, reports pressed → toggles its diff open in place (`app/diff/actions.rs:315-325`). ↑/↓ in the focused tab move the current file (and scroll it in) without opening it (`:486-519`); chords are ignored. Keyboard focus draws a 1 px `border_focus` border (`:467-469`).

File opened in place (`under_row`, `:626-689`): rows at the same 24 px pitch (not the diff's 17 px), unified or side by side per the shared setting:
- reading: **"Reading the diff…"** (`READING_DIFF`, `:57`) 13 px `text_placeholder`;
- failed: the message in `error`;
- notice: the same words as the Changes tab's notice, one row each (`notice_line`, `:693-720`: Title 13 px `DIFF_TEXT`; Muted 12 px `DIFF_MUTED`; Side "Old"/"New" caption + two spaces + text 12 px in removed/added emphasis; git lines 11 px Plex Mono; a compact **"Load Diff"** button);
- whitespace hiding: a first row **"Ignoring whitespace hides some changes"** 13 px `DIFF_MUTED` (`:670-672`);
- otherwise the diff's rows via `draw_row` (shared with the Changes tab), never at entire-file context.
Placement: `Expansion` table, binary search per row (`ui/detail/expansion.rs`). Pinned by `crates/cairn-ui/tests/commit_tab.rs` (`the_commit_tab_shows_every_field_r5_3_names`, `the_strip_reports_its_tabs_and_its_collapse`, …) and `commit_tab_expansion.rs` (`expand_all_turns_into_collapse_all_and_says_what_its_budget_left_collapsed`, `only_a_viewport_of_rows_is_built_however_many_files_are_open`).

A stash row: the Commit tab draws the stash commit's own details (every parent); no stash-specific notice in the pane (`app/history/selection.rs:21-28`).

### D. Changes tab (`app/detail/changes_tab.rs`; `ui/detail/changes_list.rs`)

Layout top to bottom: header (summary, or comparison header), then a horizontal `ResizableContainer`: file list `PanelSize::percent(35)` (`LIST_WIDTH`, `changes_tab.rs:42`; min 200 px `LIST_MIN_PIXELS`) | diff side (min 240 px `DIFF_MIN_PIXELS`, `:44-45`), splitter share kept in session `changes_list_width` (`:167-208`, `share_of` `:217-220`). Both panels proportional, so the list keeps its share on resize.

Summary line (`ChangesSummary`, `changes_list.rs:335-387`): one row 30 px (`SUMMARY_HEIGHT` = `DIFF_HEADER_HEIGHT`, `:44`), padding 0/12/0/12, spacing 12, 13 px: author name (`text_primary`), short id (`text_secondary`, Plex Mono), `long_date` (`text_secondary`), subject (`text_primary`, flex, ellipsis). No avatar, no background set.

Comparison header (two commits, `ComparisonHeader`, `:389-515`): 60 px tall (2 × 30), padding 0/12/0/12, spacing 8; two lines, **"Base"** (the lower row, removed side) then **"Tip"** (`BASE_CAPTION`/`TIP_CAPTION`, `:391-393`), each caption 32 px wide `text_placeholder`, then short id (`text_secondary`, Plex Mono), author (`text_primary`), subject (flex, ellipsis), 13 px. Right: compact button with `Glyph::Swap` (`↕`, Plex Mono 14 px), tooltip and a11y name **"Swap base and tip"** (`SWAP_LABEL`, `:395`). Drawn immediately when the pair is set; under it **"Reading the comparison…"** (`READING_COMPARISON`, `changes_tab.rs:39`) until the answer.

File list (`ChangesList`):
- Filter field at the top: `text_field(..).placeholder("Filter").compact()` full width (`FILTER_PLACEHOLDER`, `:26`), in a box padded 4/6/4/6 (`:194-204`); Freya compact input = padding 4/6/4/6, radius 4 (`themes.rs` `compact_input_layout`).
- Under an active filter: **"Showing {n} of {total} file{s}"** (`filter_count`, `:35-42`), or **"Filtering…"** while the first answer is pending (`:30`); 12 px `text_secondary`.
- No match: **"No file matches the filter."** centred, 13 px `text_placeholder` (`:28`, `:168-178`).
- Rows: virtualized, 24 px (`DETAIL_ROW_HEIGHT`), padding 0/8/0/8, the same `file_row` as the Commit tab (status letter badge + path) but no disclosure chevron; chosen file background `surface_secondary` (`build_row`, `:306-330`). ↑/↓ in the focused list choose previous/next file shown (`:271-304`). Focus border 1 px `border_focus`.
- With no file chosen the first file the list shows is chosen (Fork's default) (`changes_tab.rs:107-123`).

Diff side (`diff_side`, `changes_tab.rs:260-293`): `DiffHeader` (section E) over the body. Body notices (13 px, centred, `pane::notice`): **"Choose a file to see its diff."** (`NO_FILE_CHOSEN`, `:35`), **"Reading the diff…"** (`READING_DIFF`, `:37`), a failure in `error`; an answer with no diff → the **"No changes to show."** notice; otherwise `DiffNoticeView` or `DiffView` (`:296-319`). Pinned by `crates/cairn-ui/tests/changes_list.rs` (`the_summary_is_author_short_id_date_and_subject`, `the_summary_strip_is_as_tall_as_the_diff_bar`, `an_active_filter_says_how_many_files_it_shows_of_how_many`, `a_list_of_55184_files_builds_one_viewport_filtered_or_not`).

### E. Diff header bar (`ui/diff/header.rs`)

Height `DIFF_HEADER_HEIGHT` = 30 px (`:26`), padding 0/8/0/8, spacing 4, background `HEADER_BAR` (#1F1F1F) (`:228-236`). Order, left to right:

| # | Button / part | Glyph (`ui/diff/toggle_glyphs.rs`) | Tooltip = a11y name | Lit / enabled |
| --- | --- | --- | --- | --- |
| 1 | Previous change | 7×7 chevron up (1.5 px strokes, rotated 45°) | **"Previous change"** (Cairn's own, `:29`) | always enabled |
| 2 | Next change | chevron down | **"Next change"** (`:31`) | always enabled |
| 3 | Path | directory in `text_placeholder`, file name in `text_primary`, Plex Mono 11 px, one line, ellipsis, padding 0/8, flex (`:200-215`); for rename/copy a tooltip **"old → new"** (`:216-226`) | — | — |
| 3a | Hiding notice | **"Ignoring whitespace hides some changes"** (`HIDDEN_CHANGES_NOTICE`, `:45`), 12 px `warning`, only when a change is actually hidden and not `exact` (`:252-258`) | | |
| 3b | `said` line | Local Changes: **"The file changed — line selection cleared."** (`SELECTION_CLEARED`, `ui/diff/staging_gesture.rs:61`), 12 px `warning` (`:259-265`) | | |
| 4 | Ignore whitespace | `⎵` as a 12×5 box, open top | **"Ignore whitespaces"** (Fork's, `:33`) | lit when on; disabled (and drawn off) when `exact` (Local Changes) |
| 5 | Fewer lines | 8 px bar over two 12 px lines | **"Decrease number of visible lines"** (`:35`) | disabled at 1 line or entire file |
| 6 | More lines | `+` over two lines | **"Increase number of visible lines"** (`:37`) | disabled with entire file |
| 7 | Entire file | `↕` (Plex Mono 11 px) over two lines | **"Show entire file"** (`:39`) | lit when on |
| 8 | Side by side | 14×11 rectangle split by a 1.5 px bar | **"Side-by-side diff"** (Cairn's, `:43`) | lit when on |

All buttons are `Button::compact()` with a glyph in a 16×18 box (`GLYPH_WIDTH`/`GLYPH_HEIGHT`, `toggle_glyphs.rs:18-19`, `STROKE` 1.5 `:22`); a lit toggle colours its glyph `text_highlight`, never fills; disabled `disabled`; else `text_primary` (`header.rs:169-198`). No statistics, no file actions, no chords on the toggles (`:12`). Context starts at git's 3 (`Context::DEFAULT_LINES`) and follows the user's `diff.context` until moved (`ui/diff/settings.rs:28-84`); settings are session-only (issue #29). Toggling side-by-side asks git nothing; other toggles re-ask the file (`app/diff/actions.rs:117-130`, `:350-364`). Pinned by `crates/cairn-ui/tests/diff_view.rs` (`the_bar_reports_each_button_and_holds_fewer_lines_at_one`, `every_button_is_a_named_glyph_and_an_active_toggle_is_lit_in_the_accent`, `the_bar_says_changes_are_hidden_only_when_they_are`).

### F. Diff rows (`ui/diff/view.rs`, `row_parts.rs`, `unified_rows.rs`, `side_by_side_rows.rs`, `line_text.rs`)

| Constant | Value | Anchor |
| --- | --- | --- |
| `DIFF_ROW_HEIGHT` | 17 px (Fork Mac's 17 pt pitch; hunk header same) | `view.rs:42` |
| `DIFF_FONT_FAMILY` | "IBM Plex Mono" | `palette.rs:56` |
| `DIFF_FONT_SIZE` | 11 px (Fork Mac Menlo 11 pt) | `palette.rs:61` |
| `MONO_ADVANCE_EM` / `ADVANCE` | 0.6 → 6.6 px per column | `palette.rs:64`, `row_parts.rs:27` |
| `NUMBER_FONT_SIZE` | 9 px | `row_parts.rs:33` |
| Number column width | `digits × 5.4 + NUMBER_LEAD 2 + NUMBER_PADDING 6`, digits = file's widest number (`number_width`) | `row_parts.rs:38-41`, `:70-72` |
| `TEXT_PADDING` | 4 px after the separator | `row_parts.rs:44` |
| `SEPARATOR_WIDTH` / `CURRENT_SEPARATOR_WIDTH` | 1 px `GUTTER_SEPARATOR` / 3 px `CURRENT_CHANGE` on the change last stepped to | `row_parts.rs:45-46`, `:97-110` |
| `TEXT_END_PADDING` | 24 px after the widest line | `row_parts.rs:47` |
| `MIDDLE_WIDTH` (side-by-side rule) | 1 px `GUTTER_SEPARATOR` | `side_by_side_rows.rs:32` |
| `TAB_WIDTH` | 8 columns (`TAB_STOP`), Unicode widths | `line_text.rs:38`, `cairn-model/src/diff_shown.rs:35` |
| `LINE_CUT_BYTES` | 2,048 (= `DiffLimits::MAX_LINE_BYTES`) | `cairn-model/src/diff_shown.rs:32` |
| `LEAD_ROWS` | 1 row above a change stepped to | `view.rs:53` |
| Line height | not set; fixed 17 px row, text vertically centred (`words`, `row_parts.rs:113-122`) | |

- **Unified**: old-number and new-number gutters on the plain ground (blank on the side a line is not on), separator, then the text area tinted from the separator to the row's end (`unified_rows.rs:97-120`). No `+`/`-` marker column. Rows as wide as the view or the widest line (`content_width`, `view.rs:78-80`).
- **Hunk header**: git's `@@ -a,b +c,d @@` plus a space and git's function context, in `DIFF_MUTED`, where text starts, no band, no button (`header_words` `view.rs:84-93`; `unified_rows.rs:122-137`). Side by side it repeats in both columns.
- **No-newline marker**: its own muted row **"\ No newline at end of file"** (`NO_NEWLINE_AT_END`, `view.rs:50`); side by side in the column whose side lacked it, filler opposite.
- **Side by side**: two equal half-width columns in one virtual list, each its own number gutter + separator + clipped text; a changed line's tint covers its whole column, number included; missing side = `FILLER` (`side_by_side_rows.rs:192-235`). Sideways scroll slides both columns' text together while gutters stay (`Columns::of`, `:47-63`).
- **Intra-line highlight**: the paragraph's highlight ranges in `REMOVED_EMPHASIS`/`ADDED_EMPHASIS` over the line tint (`row_parts.rs:127-137`, `LineKind::dress` `:59-65`).
- **Control bytes**: tab expanded to 8 columns, trailing `\r` not drawn, other C0/DEL as Unicode control pictures (U+2400…, U+2421), invalid UTF-8 one U+FFFD (`line_text.rs:107-142`).
- **Long-line cut**: a line past 2,048 bytes (only in a loaded-anyway diff) drawn to the cut then **" … {N} more bytes"** in `DIFF_MUTED` (`cut_marker`, `line_text.rs:42-47`).
- **Keys** (focused view): ↑/↓ one row, PgUp/PgDn a view less a row, Home/End, ←/→ four columns (26.4 px) (`scroll_by_key`, `view.rs:397-424`); press focuses. Previous/next change (`step_change`, `view.rs:108-131`): first press finds the first change at/below the top row in view, then steps from the cursor while the view has not moved; no wrap.
- Wrap is issue #34 (`view.rs:20`).

### G. Notices in place of rows (`ui/diff/notice.rs`)

`DiffNoticeView` (`:491-573`): centred column, spacing 8, padding 12, background `GROUND`; title 13 px `DIFF_TEXT`, muted 12 px `DIFF_MUTED`, git lines 11 px Plex Mono, side captions 12 px. Strings (all verbatim):

| State | Title | Under it |
| --- | --- | --- |
| Binary | **"Binary file"** | side-by-side (spacing 32) **"Old"** (removed emphasis) / **"New"** (added emphasis), each a size: `"{n} bytes"` under 1 KiB, else `"{x.x} KiB ({n} bytes)"` (`size_text`, `:324-333`) |
| Too large | **"Changes are too large to display"** (Fork Windows) | muted reason: `"{n} bytes, more than the limit of {limit} bytes"`, `"{n} lines, more than the limit of {limit} lines"`, `"a line of {n} bytes, longer than the limit of {limit} bytes"`, + `"; too large to load"` when not loadable; past the ceiling `"{size} MiB — larger than the 64 MiB Cairn can load"` (`too_large_reason`, `:380-410`); compact button **"Load Diff"** when loadable (`:26`, `:512-517`). Limits: 1 MiB, 50,000 lines, 2,048-byte line, load-anyway 64 MiB (`cairn-model/src/diff_content.rs:19-22`) |
| LFS pointer | **"Git LFS pointer"** | "Old"/"New" stacked (spacing 12), each the pointer's lines in Plex Mono |
| Submodule | **"Submodule"** | `-Subproject commit <hex>` (removed emphasis), `+Subproject commit <hex>[-dirty]` (added emphasis) |
| Mode only | **"Mode changed"** | git's `old mode NNNNNN` / `new mode NNNNNN` |
| Rename/copy, no content | **"Renamed without changes"** / **"Copied without changes"**; with mode: **"Renamed, mode changed"** / **"Copied, mode changed"** | `similarity index N%`, `rename from …`/`rename to …` (or `copy …`) (`header_lines`, `:269-289`) |
| Other no-content (unreached) | **"No change to the file's content"** | |
| Conflicted | **"Unmerged path — conflicts must be resolved before a diff can be shown"** | |
| Unsupported | engine reason: "an external diff program is configured for this path ({command}), and Cairn never runs one" (`cairn-git/src/diff/content.rs:790`); "a bare repository has no working tree", "the index is a sparse index, which Cairn does not read yet", "a directory, which is not one file to diff", "not a regular file or a symlink (a named pipe, a socket or a device), which git does not diff" (`cairn-git/src/diff/working_tree.rs:73,100,689,692`) | |
| Nothing shown | — | muted **"No changes to show."** or **"Every change to this file is whitespace, which is being ignored."** |

Pinned by `crates/cairn-ui/tests/diff_notice.rs` (`every_state_that_is_not_text_draws_its_notice`, `every_notice_says_the_same_words_in_place_as_in_the_changes_tab`).

### H. Palette (`ui/diff/palette.rs`)

Fork's dark values are measured on Fork's #282828 ground and retuned to Cairn's #141414 by keeping each channel's offset (−20 per channel, `retuned`, `:72-89`).

| Token | Value | Fork source | Anchor |
| --- | --- | --- | --- |
| `GROUND` | #141414 rgb(20,20,20) (= Freya dark `background`) | — | `:24-25` |
| `REMOVED_TINT` | #4F2B2A | #633F3E | `:28` |
| `ADDED_TINT` | #26482B | #3A5C3F | `:30` |
| `REMOVED_EMPHASIS` | #8B2E33 | #9F4247 | `:32` |
| `ADDED_EMPHASIS` | #24702E | #388442 | `:34` |
| `DIFF_TEXT` | #DDDDDD (kept) | #DDDDDD | `:37` |
| `DIFF_MUTED` | #A0A0A0 (kept; numbers, hunk headers, EOF marker) | #A0A0A0 | `:39` |
| `GUTTER_SEPARATOR` | #373737 | #4B4B4B | `:41` |
| `FILLER` | #2E2E2E | #424242 (Windows) | `:44` |
| `HEADER_BAR` | #1F1F1F | #333333 | `:46` |
| `CURRENT_CHANGE` | #6091E0 rgb(96,145,224), copy of theme `text_highlight` (Fork uses system accent, Mac #126CFB) | — | `:52` |

Context lines have no tint (`GROUND`). There is no light palette.

### I. Staging gesture (Local Changes' diff only; `ui/diff/staging_gesture.rs`, wired at `app/local_changes/pane.rs:343-480`)

- Local Changes' bar is `exact(true)` (ignore whitespace off and disabled) and carries the `said` line (`app/local_changes/pane.rs:367-372`); its notices: conflicted path → Conflicted notice; nothing → "No changes to show.".
- **Hover** (no selection): the chunk (hunk) under the pointer is outlined: full width minus the 16 px scrollbar, 1 px `CURRENT_CHANGE` border, fill `CURRENT_CHANGE` at alpha 14/255 (`:744-754`).
- **Floating actions**: at the chunk's (or selection's) top + 2 px, right edge `SCROLLBAR_THICKNESS` 16 + 6 px, spacing 4 (`:755-760`); each a pill: padding 2/8/2/8, radius 4, background `CURRENT_CHANGE`, 12 px label `text_inverse` (white), takes no focus, does not start a drag (`floating_action`, `:798-814`).
  - Unstaged diff: **"Stage"**, **"Discard Changes…"**; staged diff: **"Unstage"** (`:54-58`, order `GestureSide::verbs` `:86-91`).
  - With a drag selection: **"Stage N Lines"** / **"Stage 1 Line"**, **"Unstage N Lines"**, **"Discard N Lines…"** (`lines_caption`, `:106-113`).
- **Drag**: press on a row arms; moving > `DRAG_THRESHOLD` 4 px starts a drag (`ui/local_changes/drag.rs:26`); tint `CURRENT_CHANGE` alpha 56/255 over the rows between ends (side by side: only the column it began in) (`:734-743`); edge auto-scroll while dragging; release selects the changed lines in range; a press without drag, Escape (`view.rs:351-357`) or focus loss clears. After an action the buttons hide until the pointer moves or the diff redraws.
- **Mode row** (file mode changed): a row above the diff, height 17+4 = 21 px, padding 0/(16+6)/0/8, `GROUND`, git's `old mode NNNNNN  new mode NNNNNN` 11 px Plex Mono `DIFF_MUTED`; hovered: 1 px `CURRENT_CHANGE` border and pills **"Stage Mode Change"** / **"Discard Mode Change…"** (unstaged) or **"Unstage Mode Change"** (staged) (`mode_caption` `:116-122`, `ModeRow` `:868-925`).
- Layer depth `GESTURE_LAYER` = 64 (`:65`). Commit and Changes tabs pass no `Gesture`, so draw none.
- Pinned by `crates/cairn-ui/tests/staging_gesture.rs` (`a_hovered_chunks_actions_take_exactly_its_changes`, `a_drag_narrows_the_actions_to_its_lines`, `side_by_side_a_drag_keeps_to_its_column`, `the_gesture_builds_one_viewport_over_a_10000_line_diff`).

### Notable gaps against Fork, read from the code

- Commit tab rows are 24 px, but its in-place diff rows are drawn at 24 px too, not the diff's 17 px (`commit_tab.rs:46-50`).
- The Commit tab draws name and email in one colour, though its doc says the address is lighter (`commit_tab.rs:879-887`).
- No avatar anywhere; no file-tree tab (#31); no line wrap (#34); settings and splitter positions are not persisted (#29, #89).
- Previous/next-change tooltips and the side-by-side tooltip are Cairn's own words, not Fork's recorded ones (`header.rs:28-43`).
- Accent is Freya's `text_highlight` #6091E0, not a system accent.

## 04 — Local Changes (as built)

Scope: the file lists (`crates/cairn-ui/src/local_changes/`, `end_room.rs`, `edge_scroll.rs`;
`crates/cairn-app/src/local_changes/`) and the commit box (`crates/cairn-ui/src/commit_box.rs`,
`check_box.rs`, `text_field.rs`; `crates/cairn-app/src/commit_box/`). Paths are relative to
`crates/`. Theme is Freya's `dark_theme` (`cairn-app/src/main.rs:61`); token RGB values below are
from the pinned fork's `DARK_COLORS` (freya rev `caa46f8`, `freya-components/src/theming/themes.rs:116`).

Theme tokens used on this surface (dark):

| Token | RGB |
| --- | --- |
| `background` | 20,20,20 |
| `surface_primary` | 60,60,60 |
| `surface_secondary` | 45,45,45 |
| `surface_tertiary` | 25,25,25 |
| `border` | 60,60,60 |
| `border_focus` | 110,110,110 |
| `disabled` | 50,50,50 |
| `text_primary` / `text_secondary` / `text_placeholder` | 250 / 210 / 150 grey |
| `text_inverse` | white |
| `primary` | 103,80,164 |
| `success` / `warning` / `error` / `info` | 129,199,132 / 255,213,79 / 229,115,115 / 100,181,246 |

---

### A. Layout of the view

The view takes the main region in place of the history when Local Changes is pressed in the sidebar
(`cairn-app/src/local_changes/pane.rs:1-6`). It is mounted only while shown.

```
┌ banner (only when the latest status read failed while lists exist) ─────────────┐
├──────────── list side (35%) ──────────┬─────────── diff side (65%) ─────────────┤
│ [Filter_______________]               │ DiffHeader (diff bar, exact)             │
│ Showing N of M files   (filter on)    │ [mode-change row, if any]                │
│ ┌ Unstaged ............ ⇊  [Stage]  ┐ │ diff rows + staging gesture / notice     │
│ │ rows (virtualised)                │ │                                          │
│ ├──── splitter (50%) ───────────────┤ │                                          │
│ ┌ Staged .............. ⇈ [Unstage] ┐ ├──────────────────────────────────────────┤
│ │ rows (virtualised)                │ │ commit box                               │
└───────────────────────────────────────┴──────────────────────────────────────────┘
```

The commit box sits **under the diff, on the right panel**, not under the lists
(`pane.rs:275-290`). Nothing is drawn under the lists (`pane.rs:225-227`).

#### Splits

| What | Value | Anchor |
| --- | --- | --- |
| List side's share, default | `LIST_WIDTH` = 35.0 % (the Changes tab's) | `cairn-app/src/local_changes/state.rs:43` → `cairn-app/src/detail/changes_tab.rs:42` |
| List side min | `LIST_MIN_PIXELS` = 200 px | `pane.rs:83` |
| Diff side min | `DIFF_MIN_PIXELS` = 240 px | `pane.rs:84` |
| Horizontal splitter | Freya `ResizableContainer`, `Direction::Horizontal` | `pane.rs:256-292` |
| Unstaged's share of the two lists, default | `LISTS_SPLIT` = 50.0 % | `cairn-ui/src/local_changes/list.rs:150` |
| Each list min (vertical drag) | `LIST_MIN_PIXELS` = 80 px | `list.rs:112` |
| Vertical splitter | `ResizableContainer`, `Direction::Vertical` | `list.rs:310-326` |
| Splitter handle | Freya `ResizableContext::HANDLE_SIZE` = 4.0 px | freya `resizable_container.rs:143` |
| Persistence | Both shares are session state only (`State<f32>`), never saved | `state.rs:50-53, 75-76` |

A dragged pixel size is turned into a percentage of the split minus the handle (`list.rs:116-119`,
`changes_tab.rs:217-220`).

#### Whole-view states (`pane.rs:63-80`, drawn by `detail::pane::notice`, `cairn-app/src/detail/pane.rs:258-271`: centred, padding 12, 13 px, `text_placeholder`, or `error` when alarming)

| State | Text (verbatim) | Alarming |
| --- | --- | --- |
| Before the first status | "Reading the working tree's status…" | no |
| First status failed | "The working tree's status could not be read: {message}" | yes |
| Bare repository | "This repository has no working tree." | no |
| Sparse index, old git | "The index is a sparse index, which the git in use cannot read (git 2.32 and later can)." | yes |

A later status failure while lists are drawn shows a **banner** over the whole view
(`pane.rs:308-323`): padding 4/8/4/8, `surface_tertiary` background, 12 px `error` text, one line,
ellipsis; same "The working tree's status could not be read: …" wording.

#### Diff side states (`pane.rs:344-404`)

| State | Text | Anchor |
| --- | --- | --- |
| Status lists nothing | "No local changes." (in the **diff** side; the two list headings still draw, empty) | `pane.rs:68, 347-354` |
| Nothing selected | "No file selected" (no full stop) | `pane.rs:66` |
| Diff on its way | "Reading the diff…" | `pane.rs:75` → `detail/changes_tab.rs:37` |
| Diff failed | the engine's message, alarming | `pane.rs:376` |
| Conflicted path | "Unmerged path — conflicts must be resolved before a diff can be shown" | `cairn-ui/src/diff/notice.rs:50-51` |
| Nothing to show | "No changes to show." | `notice.rs:54` |
| Line selection lost on refresh | "The file changed — line selection cleared." (said in the bar until the next key/press) | `cairn-ui/src/diff/staging_gesture.rs:61`, `pane.rs:244-247, 299-305` |

The diff is always the primary's alone, bar counting nothing (`pane.rs:34-39`), "exact" (ignore
whitespace drawn off and disabled, `pane.rs:360-369`).

---

### B. The filter

| Item | Value | Anchor |
| --- | --- | --- |
| Field | `text_field(..)` → Freya `Input`, `.compact()`, fill width | `list.rs:338-341` |
| Placeholder | "Filter" (`FILTER_PLACEHOLDER`, shared with the Changes tab) | `cairn-ui/src/detail/changes_list.rs:26` |
| Wrapper padding | 4 / 6 / 4 / 6 | `list.rs:336` |
| Compact input layout (Freya theme) | corner radius 4, inner margin 4/6/4/6; background `surface_tertiary`, focused `background` | freya `themes.rs:405-423` |
| Count line (only while filter text is non-empty) | "Showing {shown} of {total} file{s}" — numbers digit-grouped (`grouped`), "file" singular only when total == 1; 12 px, `text_secondary`, 1 line | `list.rs:264-275`, `changes_list.rs:35-41` |
| While the filter's rows are on their way | "Filtering…" | `changes_list.rs:30` |
| Filter leaves no row in either list | "No path matches the filter." centred in place of **both** lists, 13 px, `text_placeholder` | `list.rs:74, 288-299` |

The count counts distinct paths (a path in both lists is one), M = `LocalChanges::paths()`
(`list.rs:252-262`). Filtering runs on a worker (`Request::FilterLocalChanges`, `pane.rs:124-135`).

---

### C. Headings

| Item | Value | Anchor |
| --- | --- | --- |
| Captions | "Unstaged", "Staged" — **no count** in either heading | `list.rs:70-72, 83-88` |
| Height | `LIST_HEADER_HEIGHT` = 26 px | `list.rs:77` |
| Padding / spacing | 0/8/0/8; spacing 6; children centred vertically | `list.rs:561-562` |
| Background | `surface_tertiary` (25,25,25) | `list.rs:563` |
| Caption text | 12 px, **bold**, `text_secondary`, 1 line, flex 1 | `list.rs:565-571` |
| Order (left → right) | caption, double chevron, Stage/Unstage button | `list.rs:573-574` |

#### Heading controls (`heading_control`, `list.rs:420-449`)

Padding 1/6/1/6; corner radius 4; 1 px border, `border` when enabled, `disabled` when not; pointer
cursor only when enabled; a11y role Button with the caption as its name. Pressing does not take focus
from the list.

| Control | Caption / glyph | Enabled when | Tooltip | Anchor |
| --- | --- | --- | --- | --- |
| Unstaged's button | "Stage"; "Stage All" while the heading press is held | plain: selection is in this list; All: the list shows ≥ 1 row | **none** (a11y name only) | `list.rs:488-517`, captions `menu.rs:22-25` |
| Staged's button | "Unstage"; "Unstage All" while held | same | none | same |
| Unstaged's chevron | ⇊ `Glyph::StageAll`, two 7×7 chevrons pointing down, 1.5 px stroke, overlap −2 px, in a 16×18 box | list shows ≥ 1 row | "Stage All" | `list.rs:521-537`, `cairn-ui/src/diff/toggle_glyphs.rs:18-22, 43, 65, 153-176` |
| Staged's chevron | ⇈ `Glyph::UnstageAll`, pointing up; **always drawn** (Fork for Windows shows it only with Shift held — deviation) | same | "Unstage All" | `toggle_glyphs.rs:44-47, 66` |

Button label: 12 px, `text_primary` **even when disabled** (only its border dims) (`list.rs:502-506`).
Chevron glyph colour: `text_primary`, or `disabled` when the list is empty (`list.rs:528-532`).

Heading press (makes the buttons read "… All"): Shift on Linux, ⌥ on macOS
(`cairn-ui/src/accelerators.rs:314-328`, `Trigger::HeadingPress`). With a filter on, All takes only
the rows shown (`list.rs:484-487`; `cairn-app/src/local_changes/actions.rs:520-551`). After Stage
All / Unstage All nothing is selected (`actions.rs:548-550`).

---

### D. Rows

| Item | Value | Anchor |
| --- | --- | --- |
| Row height | `DETAIL_ROW_HEIGHT` = 24 px (the Commit tab's) | `cairn-ui/src/detail/commit_tab.rs:50`, used `list.rs:591-592` |
| Row padding | 0/8/0/8; content centred vertically; min width fill | `list.rs:685-690` |
| Badge column | `BADGE_WIDTH` = 18 px, centred | `list.rs:80, 812-815` |
| Badge → text spacing | 10 px | `list.rs:810` |
| Font size (badge letter and path) | `FONT_SIZE` = 13 px | `list.rs:79` |
| Path text | one label, the **full repo-relative path**, IBM Plex Mono (`DIFF_FONT_FAMILY`), `text_primary`, 1 line. **No dimmed directory, no bold file name.** | `list.rs:817-823`, `diff/palette.rs:56` |
| Rename / copy text | "{from} → {to}" | `list.rs:123-128` |
| Selected row | background `surface_secondary` (45,45,45) — same colour as the commit box's background; no other selection colour, no focus-dependent variant | `list.rs:711, 719` |
| Focused list | 1 px `border_focus` (110,110,110) border around the list body, on **keyboard** focus only | `list.rs:583-585` |
| Scroll | `VirtualScrollView`, `scroll_with_arrows(false)` (arrows move the selection) | `list.rs:588-595` |
| End room | extra empty rows after the last so the 16 px overlay horizontal scrollbar (`SCROLLBAR_THICKNESS`) never covers it: `END_ROOM` = 16 + 4 = 20 px, rounded up to whole rows (24 px → 1 row) | `cairn-ui/src/end_room.rs:15, 19, 23-28` |

#### Badges per `ChangeKind` (`change_badge`, `list.rs:99-109`; colours `list.rs:791-796`)

| ChangeKind | Badge | Drawn as | Colour |
| --- | --- | --- | --- |
| Modified | "M" | bold 13 px letter | `warning` (yellow) |
| TypeChanged | "M" (same as Modified) | bold letter | `warning` |
| Added (incl. untracked) | "+" | bold letter | `success` (green) |
| Deleted | "D" | bold letter | `error` (red) |
| Renamed | "R" | bold letter | `info` (blue) |
| Copied | "C" | bold letter | `info` |
| Submodule | `RefGlyph::Submodule`: 10×10 canvas, outlined 8×8 box with a filled 3.2×3.2 box inside | painted shape | `info` |
| Conflicted | `RefGlyph::Gone`: outlined warning triangle with a stroke and a dot | painted shape | `warning` |

Glyph canvas `GLYPH_SIZE` = 10 px, stroke 1.3 px (`cairn-ui/src/history/ref_glyphs.rs:23, 26, 176-218`).
The Commit tab keeps its own letters (`A`, `T`, `S`) — Local Changes deliberately differs
(`list.rs:9-10`, pinned by `each_kind_of_change_has_forks_badge`, `list.rs:837`).

---

### E. Interactions on the lists

#### Selection (`cairn-ui/src/local_changes/selection.rs`, `cairn-app/src/local_changes/actions.rs`)

One selection, in one list at a time; paths, not rows; a primary (the file clicked first, whose diff
shows) and an anchor (`selection.rs:1-18, 24-34`).

| Gesture | Effect | Anchor |
| --- | --- | --- |
| Plain press | row alone selected, primary + anchor; list takes focus | `list.rs:771-782`, `actions.rs:181-189` |
| ⌘-click (macOS) / Ctrl-click (Linux) | toggle the row in/out; primary stays the first clicked; toggled-in row becomes anchor | `selection.rs:90-111`, `actions.rs:568-574` |
| Shift-click | range from anchor to row, row becomes primary | `actions.rs:578-611` |
| ↑ / ↓ (focused list) | move from the primary; with none, first row; scrolls to keep it in view | `list.rs:647-681` |
| Shift+↑ / Shift+↓ | extend range from anchor | `list.rs:631-673`, `accelerators.rs:293-294` |
| Selecting in the other list | starts a new selection there | `selection.rs:91-93` |
| As the view opens with nothing selected | first row shown (Unstaged's, else Staged's) | `pane.rs:140-147`, `next_selection.rs:5-12` |
| After an action / refresh | one rule (`next_selection`): survivors stay; else the row that took the first path's place, else nearest above; nothing when the list empties | `cairn-app/src/local_changes/next_selection.rs:1-21` |

Modifiers on a pointer press are read from the window's held keys (`HeldKeys`), since this toolkit
build carries none on pointer events (`list.rs:206-211, 402-405`).

#### Double press (`list.rs:758-767`, `actions.rs:200-205`)

A primary double press with no extending/range modifier held: the first press made the row the
selection; the double press **stages (Unstaged) or unstages (Staged) the selection**. The second
press's release chooses nothing (`doubled` flag, `list.rs:389-390, 773`). A double press with ⌘/Ctrl
held just toggles twice.

#### Keyboard chords on a focused list (`accelerators.rs:299-343`, scope `LocalChanges` / `LocalChangesLists`)

| Action | Linux | macOS | Heard on diff too? |
| --- | --- | --- | --- |
| Stage / Unstage selection | Enter, Ctrl+S, Ctrl+Shift+S | Return, ⌘S | yes, except the bare Enter/Return (lists only) |
| Stage All / Unstage All | Ctrl+Alt+Shift+S | ⌥⇧⌘S | yes |
| Discard (unstaged side) | Backspace, Delete, Ctrl+Shift+D | ⌫, ⇧⌘D | yes |

On the diff, a chord acts on the lines a drag selected, else on the whole selection
(`actions.rs:225-268`). All actions are inert while any dialog or the popover is open
(`actions.rs:46-47, 182, 193`).

#### Drag and drop (`cairn-ui/src/local_changes/drag.rs`, `edge_scroll.rs`)

| Item | Value | Anchor |
| --- | --- | --- |
| Threshold before a press is a drag | `DRAG_THRESHOLD` = 4.0 px (Freya `DragZone`'s) | `drag.rs:24-26` |
| Drop zone | the whole other list (heading included); release over it drops | `list.rs:542-553` |
| What it carries | the selection if the drag began on a selected row; else that row, which becomes the selection | `actions.rs:207-220` |
| Effect | stage (Unstaged → Staged) or unstage (Staged → Unstaged) | `actions.rs:219` |
| **Visual while dragging** | only the target list's body gets the 1 px `border_focus` border; **no drag ghost, no cursor change, no insertion marker, no highlight of rows** | `list.rs:462, 583-585` |
| Edge auto-scroll of target list | band `EDGE_BAND` = 24 px inside an edge; tick `EDGE_TICK` = 16 ms; up to `MOST_PER_TICK` = 32 px per tick, faster deeper | `cairn-ui/src/edge_scroll.rs:27, 30, 33, 39-55` |
| Ends without drop | Escape; any press while dragging (lost release); window losing focus | `drag.rs:12-19, 116-150` |

#### Context menu (`cairn-ui/src/local_changes/menu.rs:188-229`, opened `list.rs:722-757`)

Right-press on a row outside the selection first selects that row (and focuses the list); on a row in
the selection it acts on the selection. Freya `Menu` / `MenuButton` defaults (no Cairn styling); each
item closes the menu.

| # | Unstaged list | Staged list | Disabled when |
| --- | --- | --- | --- |
| 1 | "Stage" | "Unstage" | never |
| 2 | "Discard Changes…" | "Discard Changes…" (always greyed) | Staged list always; Unstaged when nothing selected can be discarded |
| 2a | (only when 2 is greyed) a second greyed item: the reason, 12 px, wrapped in a 300 px-wide label, never cut | "Staged changes can't be discarded. Unstage them first." | — |
| 3 | "Stage All" | "Unstage All" | never |
| 4 | "Copy Path" (selected paths, one per line, to clipboard) | "Copy Path" | never |

Captions: `menu.rs:22-27`. Fork items deliberately absent: open, external diff, blame, history,
ignore, stash, save as patch (`menu.rs:1-3`).

`NoDiscard` reasons (`menu.rs:120-140`), joined by a space, submodules first:

| Selection | Reason (verbatim) |
| --- | --- |
| Staged list | "Staged changes can't be discarded. Unstage them first." |
| Only submodules | "Submodule changes can't be discarded." |
| Only conflicted | "Conflicted files can't be discarded. Resolve or stage them first." |
| Only nested repositories | "Nested repositories can't be discarded." |
| Mixed kept kinds | e.g. "Submodule changes can't be discarded. Conflicted files can't be discarded. Resolve or stage them first. Nested repositories can't be discarded." |

A selection mixing kept rows with discardable ones is offered; the confirmation adds one line
(`Left::text`, `menu.rs:77-100`): "{n} submodule(s)", "{n} conflicted file(s)", "{n} nested
repository/repositories", joined "a, b and c", then "is left as it is." (total 1) or "are left as they
are." — e.g. "1 submodule and 1 conflicted file are left as they are." The discard chord on a
selection with nothing discardable does nothing silently (`actions.rs:613-630`).

Discard confirmation title: "Discard Changes" (Fork for Windows writes "Discard changes"); a refusal
while counting becomes "Couldn't Discard Changes" with the engine's sentence and "Nothing was
changed." (`actions.rs:33-37, 71-73`). The dialog itself belongs to the dialogs section.

Pinned by (headless, `cairn-ui/tests/local_changes.rs`): `both_lists_draw_their_paths_with_their_badges`,
`every_kind_of_change_draws_its_badges_shape`, `each_list_is_in_its_paths_order`,
`a_press_and_the_arrows_choose_a_row_of_its_list`, `the_filter_says_what_it_leaves`,
`the_filters_count_is_of_distinct_paths_a_path_in_both_lists_once`,
`a_status_of_50000_paths_builds_one_viewport_filtered_or_not`,
`a_status_of_50000_paths_all_selected_builds_one_viewport`,
`scrolled_to_the_end_each_lists_last_row_is_clear_of_the_horizontal_scrollbar`,
`the_splitter_between_the_lists_drags`; unit tests in `menu.rs`, `drag.rs`, `list.rs`; app-level in
`cairn-app/src/local_changes/tests.rs`.

---

### F. The commit box (`cairn-ui/src/commit_box.rs`; wiring `cairn-app/src/commit_box/pane.rs`)

#### Container and layout (`commit_box.rs:448-466`)

Fill width; padding 8 all round; vertical spacing 6; background `surface_secondary` (45,45,45);
1 px top border in `border`. Children top → bottom:

1. the one line (optional) — 12 px, `text_secondary`, fill width, wraps (`:457-463`)
2. subject row — horizontal, spacing 8, centred: subject field (flex 1), counter, `≡` (`:384-392`)
3. description — with the column-72 ruler (`:396-419`)
4. actions row — horizontal, spacing 8: Amend check box, flexible spacer, the button (`:438-446`)

#### Subject

| Item | Value | Anchor |
| --- | --- | --- |
| Field | `text_field_recalling` (Freya `Input`, default — not compact — layout: radius 6, inner margin 8) | `commit_box.rs:364-373`, freya `themes.rs:398-402` |
| Placeholder | "Commit subject" | `commit_box.rs:43` |
| Disabled | while the box is stopped (rebase / am / sequence) | `:373` |
| Counter text | just the number `50 − chars` (chars, not bytes), negative past 50, e.g. "50", "0", "-20" | `:66-68, 101-108, 374-375` |
| Counter colour | `text_secondary`; `error` (red) only when > 70 chars (`SUBJECT_HARD_LIMIT`); no separate colour between 51 and 70 | `:69-71, 377-381` |
| Counter size | 12 px | `:376` |
| Counter a11y | "{n} characters left" | `:382` |
| Recall keys | bare ↑ / ↓ in the subject, offered first: taken only while subject+description are empty (nothing recalled) or the subject holds the last recalled one; ↑ older, ↓ newer, ↓ from newest → empty draft | `cairn-ui/src/text_field.rs:37-64`, `cairn-app/src/commit_box/pane.rs:339-372` |

#### Recent Commit Messages

| Item | Value | Anchor |
| --- | --- | --- |
| Control | `control` button whose caption is "≡"; a11y name "Recent Commit Messages"; **no tooltip** | `commit_box.rs:49, 525-548` |
| Enabled | box not stopped and at least one message | `:526` |
| Menu | Freya `ContextMenu` of `MenuButton`s, each the message's **first line**, newest first; choosing fills subject and description (split at first line + blank line) | `:531-546`, `pane.rs:326-334`, `cairn-app/src/commit_box/state.rs:382-401` |
| How many | 10 (`RECENT_MESSAGES`) commits `HEAD` reaches | `cairn-git/src/recent_messages.rs:9` |

#### Description

| Item | Value | Anchor |
| --- | --- | --- |
| Field | `text_field_in`, multiline, fill width | `commit_box.rs:402-409` |
| Placeholder | "Description" | `:45` |
| Font | IBM Plex Mono, `DESCRIPTION_FONT_SIZE` = 12 px | `:76, 399-400` |
| Height | min `DESCRIPTION_MIN_HEIGHT` = 64 px, max `DESCRIPTION_MAX_HEIGHT` = 200 px | `:77-78` |
| Ruler | 1 px wide, `border` colour, full field height, non-interactive, absolute at x = `FIELD_MARGIN` (8) + 72 × `MONO_ADVANCE_EM` (0.6) × 12 = **526.4 px** | `:72-73, 80, 411-419, 475-477`, `diff/palette.rs:64` |

#### Amend check box (`cairn-ui/src/check_box.rs`)

| Item | Value | Anchor |
| --- | --- | --- |
| Caption | "Amend" | `commit_box.rs:47` |
| Mark | 14 × 14 px, radius 3, 1 px inner border (`border`, `disabled` when disabled); fill `primary` (103,80,164) ticked else `surface_primary`; "✓" 11 px `text_inverse` when ticked | `check_box.rs:21-42` |
| Caption text | `CAPTION_SIZE` = 13 px, 1 line; `text_primary`, `text_secondary` when disabled; spacing 6 | `check_box.rs:9, 43-62` |
| A11y | role CheckBox, toggled state set | `check_box.rs:47-50` |
| Enabled when | `HEAD` born, no operation in progress, box not stopped, no commit asked | `pane.rs:118-120`, `commit_box.rs:421-425` |
| Tick | sets the draft aside, fills `HEAD`'s message into an empty untouched draft, redraws Staged as amend's staged list vs `HEAD`'s parent; untick restores the draft exactly | `pane.rs:184-213`, `state.rs:221-250` |

#### The button (`control`, `commit_box.rs:481-516`)

Padding 5/14/5/14; corner radius 6; 1 px inner border (`border`, `disabled` when disabled);
background `surface_primary` (60,60,60); label 13 px, `text_primary` or `text_secondary` when
disabled; pointer cursor when enabled. Same control is used for "≡" and "Cancel".

| State | Caption (verbatim) | Anchor |
| --- | --- | --- |
| 0 staged | "Commit" | `commit_box.rs:84-90` |
| 1 staged | "Commit 1 File" | same |
| N staged | "Commit {N} Files" (no digit grouping) | same |
| Amend ticked | "Amend Last Commit" | `:54` |

Enabled (`ready`, `pane.rs:136-143`, `commit_box.rs:353`): (Amend ticked) or (staged > 0, or a merge /
single cherry-pick / single revert in progress), **and** the subject is non-blank, **and** no commit is
asked, **and** the box is not stopped. Commit chord presses it: Ctrl+Enter (Linux), ⌘Return (macOS)
(`accelerators.rs:341-342`), claimed in either field without a newline (`text_field.rs:65-74`).

#### Busy — what stands in the button's place (`commit_box.rs:165-179, 550-620`; `pane.rs:61-95`)

All 12 px, `text_secondary`.

| State | Text |
| --- | --- |
| Queued behind another write | "Waiting for {name}…" where name is the write ahead's own name, e.g. "Stage 2 Files", "Unstage 1 File", "Stage lines of {path}", "Commit 1 file", "Commit {n} files", "Amend", "Create branch '{name}'" (`cairn-app/src/worker/local/lane.rs:274-324`). Note: the module doc's example "Waiting for the commit to finish…" is not what the code builds. |
| Running, git not started yet | "Committing" / "Amending" (no Cancel) |
| Running | "Committing 0:03" / "Amending 1:15" (`m:ss`, redrawn each second by an `async_io::Timer`) + a "Cancel" control, spacing 8 |
| Cancelled until reaped | "Cancelling…" |

#### The one line over the fields (`operation_line`, `commit_box.rs:110-153`; precedence `pane.rs:121-133`)

| Condition | Line (verbatim) | Box stopped? |
| --- | --- | --- |
| Merge | "Merging. Committing concludes the merge; Amend is unavailable." | no |
| Single cherry-pick | "Cherry-picking {short id}. Committing concludes it; Amend is unavailable." (without id: "Cherry-picking. Committing concludes it; Amend is unavailable.") | no |
| Single revert | "Reverting {short id}. Committing concludes it; Amend is unavailable." | no |
| Cherry-pick sequence | "A cherry-pick is in progress. Continue or abort it with git cherry-pick --continue or --abort." | yes |
| Revert sequence | "A revert is in progress. Continue or abort it with git revert --continue or --abort." | yes |
| Rebase | "A rebase is in progress. Continue or abort it with git rebase --continue or --abort." | yes |
| `git am` | "git am is in progress. Continue or abort it with git am --continue or --abort." | yes |
| Detached `HEAD`, no operation | "HEAD is detached: this commit will be on no branch." | no |
| Unborn branch, no operation | "No commit yet, so Amend is unavailable." | no |
| Amend ticked, amend's staged list unreadable, not detached | "Amend's staged list couldn't be read, so the lists show what is staged against HEAD: {why}" | no |

Short id is 7 hex (`abababa` in the test). "Stopped" disables both fields, the Amend toggle, the `≡`
and the button (`commit_box.rs:350-353`; `cairn-model` `refuses_commit`). For a concluding operation
git's cleaned `MERGE_MSG` fills an empty, untouched draft (`state.rs:255-268`).

#### Other commit-box strings

| Constant | Text | Anchor |
| --- | --- | --- |
| `AMEND_TITLE` | "Amend Commit" (confirmation title for an amend that must be confirmed) | `commit_box.rs:56` |
| `SKIP_HOOKS_CAPTION` | "Skip pre-commit hooks and commit" (Git Error dialog's skip) | `:64` |
| `CANCEL_COMMIT_CAPTION` | "Cancel" | `:51` |

After a successful commit the draft it took is cleared and Amend unticks; a failure keeps the draft
and opens the Git Error dialog (`pane.rs:449-507`).

Pinned by: `cairn-ui/tests/commit_box.rs` — `the_amend_toggle_reports_its_toggled_state_to_assistive_technology`,
`amend_last_commit_reports_its_press_and_draws_no_line`, `the_counter_is_drawn_and_a_stopped_box_asks_nothing`,
`a_commit_asked_stands_in_the_buttons_place`, `an_edit_in_a_field_is_reported`, plus Git Error tests;
unit tests in `commit_box.rs:622-716` (counter, caption, lines, elapsed, ruler);
app-level in `cairn-app/src/commit_box/tests.rs`.

---

### G. Gaps against Fork worth noting (observations from the code, not Fork evidence)

- Headings carry no file count.
- Paths draw whole, one style: no dimmed directory, no emphasised file name.
- The Stage/Unstage heading buttons have no tooltip; their label does not dim when disabled.
- Selection colour (`surface_secondary`) equals the commit box background; focus shows only as a 1 px
  border on keyboard focus.
- A drag has no visual beyond the target list's border.
- "No local changes." is drawn on the diff side while two empty headed lists stay on the left.
- The commit box is under the diff (right panel), full width of that panel.
- The `≡` Recent Commit Messages control has no tooltip.

## 05 — Dialogs, context menus, activity popover (as built)

Read-only audit of the source on `feature/plan-ui-review`, which is at `main` 967bb72. Paths are relative to the repo root.
UI = `crates/cairn-ui/src`, APP = `crates/cairn-app/src`, MODEL = `crates/cairn-model/src`.
Freya is pinned at `alexparlett/freya` rev `caa46f8` (`~/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/crates/freya-components/src`, abbreviated FREYA below).

### 0. Shared dialog machinery

#### 0.1 Freya `Popup` (every dialog but the activity popover uses it)
| Property | Value | Anchor |
|---|---|---|
| Backdrop | full-window rect, animated `(0,0,0,0)` → `(0,0,0,150)` over 150 ms; a press on it calls `on_close_request` | FREYA/popup.rs:185, 56-64 |
| Content animation | scale 0.85→1, opacity 0.2→1, 250 ms, Expo ease-out | popup.rs:195-204 |
| Panel | `corner_radius 12`, shadow y4 blur5 `(0,0,0,30)`, centred in window, `Layer::Overlay`, a11y role Dialog | popup.rs:240-266 |
| Theme default | width 500 px (every Cairn dialog overrides), height auto, **padding 8 all, spacing 4**, background `background`, colour `text_primary` | FREYA/theming/themes.rs:537-544 |
| Escape | `close_on_escape_key = true`: Escape → `on_close_request`, `prevent_default` | popup.rs:160, 228-237 |
| `PopupTitle` | font 18, padding 8, label width fill, role TitleBar | popup.rs:293-301 |
| `PopupContent` | font 15, padding 8 (Cairn labels set their own 12–14) | popup.rs:327-333 |
| `PopupButtons` | horizontal, width fill, `main_align End` (right-aligned), padding 8, spacing 4 | popup.rs:360-369 |

#### 0.2 Button order — `UI/dialogs/button_order.rs:22-30`
`ordered(platform, actions, cancel)`. On **Linux** it is the actions in the order given (primary first), then Cancel or Close **last**. On **macOS** it is Cancel or Close **first**, then the actions reversed, so the primary comes **last**. The row is right-aligned on both (PopupButtons). Pinned by `linux_leads_with_the_primary_and_macos_ends_with_it` (unit) and `tests/dialog_buttons.rs` (`every_dialogs_buttons_follow_the_platforms_order`, `a_confirmation_starts_on_cancel_on_both_platforms`, `a_risky_dialog_opens_on_its_safe_answer_on_both_platforms`).

#### 0.3 Answer button — `UI/dialogs/answer_button.rs`
- `answer(..)` (:79) builds a `ChoiceButton` (:172-196). It is not Freya's `Button`. Padding `Gaps(6,14,6,14)`, `corner_radius 6`, border 1 px `border` when unfocused and **2 px `border_focus` when focused**, background `surface_primary`, label font **14**. It can be focused, and `a11y_auto_focus(focused_first)` is how a dialog opens on its safe answer. A pointer press presses it, and Return or Space presses it while it has focus.
- `greyed_answer(..)` (:101-122) uses the same padding and radius, a 1 px border in `border_disabled`, and text in `text_placeholder`. It cannot be focused, is a11y-disabled, and nothing presses it.
- No answer button is "filled", so the primary and Cancel look alike in confirm, Git Error and the host-key Cancel. Create Branch and the credential prompt's primary use Freya `Button::new().filled()` instead. The Freya button layout is padding 6/12 and radius 6 (themes.rs:173-176); compact is padding 3/6 (:183).

#### 0.4 One modal state — `APP/modal.rs`
- The priority order is: `Prompt` → `Confirmation` → `CommitError` → `Ending` (oldest first) → `CreateBranch` → `Popover` (:18-31, :34-50). Only the one on top is drawn (`APP/window.rs:249-266`). A dialog under another is kept but not drawn.
- `shortcuts::keys_inert` = any modal open (`APP/shortcuts.rs:35-37`). `act` returns early, so every window chord is dead while a dialog or the popover is up (:51-53).
- `ContextMenuViewer::new()` is the host of every context menu, at the window root (`APP/window.rs:270`).

---

### 1. Confirmation dialog (destructive ops) — `UI/dialogs/confirm.rs`

| Item | Value | Anchor |
|---|---|---|
| Width | `DIALOG_WIDTH = 520.` | :41 |
| a11y | inner rect `a11y_modal(true)`, role **AlertDialog** (focus held inside; Tab stays in) | :361-362 |
| Body text | label width fill, **font 14**, wraps whole | :366 |
| "Left" line | separate label, font 14, under the body (Ready and Counting only, not Refused) | :350-353, :367-371 |
| Close request | `on_close_request` → cancel: Escape and a press on the backdrop both cancel | :357 |
| One answer | `answered` state, so every press after the first is ignored | :219-232, :278-284 |
| Key | `render_key = U64(serial)`, so a new confirmation is a new dialog with fresh focus | :378-380 |

#### 1.1 Three phases (`DialogAnswer`, :64-71)
| Phase | Title | Body | Buttons (Linux order; macOS reversed) | Focus |
|---|---|---|---|---|
| `Counting` | `<title>` | `"Counting…"` (`COUNTING_TEXT`, :47) | greyed `<title>` (button reads the dialog title), `"Cancel"` | Cancel |
| `Ready(Consequence)` | `<title>` | `Consequence::prompt()` | `Consequence::action()`, `"Cancel"` (`CANCEL_CAPTION`, :44) | **Cancel** (confirm `focused_first=false`, :294) |
| `Refused(why)` | `"Couldn't <title>"` (:246) | `"<why> Nothing was changed."` (`NOTHING_CHANGED`, :54) | `"Close"` alone (`CLOSE_CAPTION`) | Close |

The module doc explains why focus starts on Cancel: it "deviates from Fork, which styles Discard as the default (Fork Tracker #1080)" (:21-23).

#### 1.2 "Show files" disclosure (:316-349)
- It appears only when `consequence.listed() > 0`, which is a `DiscardFiles` of **more than one** file (`MODEL/consequence.rs:255-264`).
- The control reads glyph `"▸"` when closed and `"▾"` when open, then `"Show files"` (`SHOW_FILES_CAPTION`, :51). Font 13, spacing 4, focusable, a11y Button with `expanded` state.
- The list is a `VirtualScrollView`. Row height `SHOWN_FILE_HEIGHT = 20.` (:57); list height `min(SHOWN_FILES_HEIGHT = 200., 20 × length)` (:60, :343-345). Length includes `end_room`.
- Each row is horizontal, padding (0,6,0,6), spacing 12. It shows the path label (font 13, max 1 line, not cut: the list scrolls sideways) and the "what" label (font 13) (:195-207).
- Pinned by `show_files_discloses_each_file_and_what_happens_to_it`, `show_files_lists_a_path_wider_than_the_dialog_whole_and_scrolls_to_it` and `show_files_over_50000_paths_builds_one_viewport`.

#### 1.3 Titles (who opens it)
| Title | Opened by | Anchor |
|---|---|---|
| `"Discard Changes"` (`DISCARD_TITLE`) | discard of files **and** of lines (both open counting) | `APP/local_changes/actions.rs:73`, :374, :411 |
| `"Amend Commit"` (`AMEND_TITLE`) | an amend that `needs_confirming` | `UI/commit_box.rs:56`; `APP/commit_box/pane.rs:293-294` |
| `"Remove Stale Lock"` (`REMOVE_LOCK_TITLE`) | `Remove index.lock…` answer | `APP/operations/leftover_lock.rs:27`, :125 |

The window side is `APP/operations/confirming.rs`. `Confirming::new` builds a Ready dialog and `::counting` a Counting one. `arrived` and `refused` flip the phase in place under the same serial (:153-161). `leaving(left)` sets the extra line. `on_let_go` runs when the dialog is dismissed unanswered, which ends a running count. The serial comes from a static `AtomicU64` that starts at 1 (:90). It is drawn by `APP/window.rs:277-298`.

#### 1.4 The "left" line — `UI/local_changes/menu.rs:77-100` (`Left::text`)
The template is `"<what> is left as it is."` for one row, or `"<what> are left as they are."` otherwise. The kinds come in the order `"N submodule(s)"`, `"N conflicted file(s)"`, `"N nested repository/repositories"`, joined as "a, b and c". Example: `"1 submodule and 1 conflicted file are left as they are."`

#### 1.5 Consequence words — `MODEL/consequence.rs`
`UNDONE = "You can't undo this."` (:544). Paths go through `quoted()`/`shown_path()`: C-quoted only where needed, with bidi controls and line separators escaped (:609-671).

**`prompt()` (:203-250):**
| Variant | Template |
|---|---|
| DiscardLines, mode only | `"Discard the mode change of <path>? You can't undo this."` (:583) |
| DiscardLines, lines + mode | `"Discard N changed line(s) and the mode change in <path>? You can't undo this."` (:584-587) |
| DiscardLines, chunk | `"Discard this chunk (N line(s)) in <path>? You can't undo this."` (:588-591) |
| DiscardLines, lines | `"Discard N changed line(s) in <path>? You can't undo this."` (:592-595) |
| DiscardFiles | `"Discard all changes in <path \| N file(s)>? [1 untracked file will be deleted. \| N untracked files will be deleted. ]You can't undo this."` (:551-569) |
| CheckoutDiscarding | `"Discard local changes and any untracked files in the way, then check out <branch> at <short>. You can't undo this."` (:224-229). It is never drawn; Create Branch is the surface. |
| Amend, on a remote | `"<short> is already on <upstream \| a remote>. Amending it rewrites history others may have."` (:531) |
| Amend, no reflog | `"<short> can't be recovered after this: this repository keeps no reflog."` (:534) |
| Amend, both | the two sentences joined by a space (:537) |
| Amend, neither (never drawn) | `"Amend <short>? The old commit stays in Show Lost Commits."` (:539) |
| RemoveLock | `"Remove <path>? It was last changed <N second(s)/minute(s)/hour(s)/day(s)> ago and holds <N byte(s) \| x.y KiB/MiB/GiB/TiB>. Another program may still own it: removing a lock a running git holds can corrupt the index. You can't undo this."` (:242-248, `size` :674, `elapsed` :694) |

**`action()` (the confirm button's caption, :343-390):**
| Variant | Caption |
|---|---|
| DiscardLines | `"Discard Mode Change"`; `"Discard N Line(s) and Mode Change"`; `"Discard N Line(s)"` (title-case Line/Lines) |
| DiscardFiles | `"Discard Changes in N File(s)"` (1 File / N Files) |
| Amend | `"Amend"` |
| CheckoutDiscarding | `"Create and Checkout"` |
| RemoveLock | `"Remove <file name>"`, i.e. `"Remove index.lock"` |

**`listed_file()` "what" column (:272-306):** `"Restored"`, `"Mode change"`, `"N line(s) and mode change"`, `"Binary and mode change"`, `"N line(s)"`, `"Binary"`, `"Emptied"`, `"Deleted"`.

**`name()`, the activity and Couldn't name (:396-433):** `"Discard lines of <path>"`, `"Discard Changes"`, `"Amend"`, `"Create branch '<branch>'"`, `"Remove index.lock"`.

**`happened()` (:440-457):** `"Discarded local changes and any untracked files in the way, then checked out '<branch>' at <short>."`

**`amend_reported()` (:481-515):** `"Amended <short>, which was pushed to <on> while the amend ran. Sharing the amended commit needs a force push."`. Then either `"<short> can't be recovered: git kept no reflog of it."` or, alone, `"Amended <short>. It can't be recovered: git kept no reflog of it."`

---

### 2. Git Error / "Couldn't <name>" — `UI/dialogs/git_error.rs`

| Item | Value | Anchor |
|---|---|---|
| Title (git failure) | `"Git Error"` (`GIT_ERROR_TITLE`) | :32 |
| Sentence (git failure) | `"An unexpected error occurred while performing the git request."` (`GIT_ERROR_TEXT`) | :33 |
| Details caption | `"Error Details:"` (font 12, `text_secondary`) | :34, :239-243 |
| Close | `"Close"` (`CLOSE_CAPTION`) | :35 |
| Lock button | `"Remove index.lock…"` (`REMOVE_LOCK_CAPTION`) | :38 |
| Skip button | `"Skip pre-commit hooks and commit"` (`SKIP_HOOKS_CAPTION`, `UI/commit_box.rs:64`) | :311 |
| Refusal title | `"Couldn't <name>"` (`couldnt_title`) | :41-43 |
| Width | `DIALOG_WIDTH = 680.` | :45 |
| Output row | `OUTPUT_ROW_HEIGHT = 16.` | :47 |
| Output box | `OUTPUT_HEIGHT = 240.` (about 15 lines), 1 px `border`, IBM Plex Mono (`DIFF_FONT_FAMILY`) at 12, virtualized, opens **scrolled to the end** | :49, :206-209, :226-258 |
| Command line | label in mono 12, width fill (above the output box) | :245-250 |
| Show files (partial discard) | Freya `Button::new().compact()` captioned `"Show files"` (no ▸ glyph here, unlike confirm). The list is rows of 16 px, height `16 × clamp(n, 1, FILES_SHOWN = 10)`, bordered, mono 12 | :51, :260-291 |
| Sentence label | font 14, width fill, wraps | :346-351 |
| Content order | sentence → files (if any) → details (if any) | :352-353 |
| a11y | `a11y_modal`, role AlertDialog | :340-341 |
| Footer | `ordered(platform, [Remove index.lock…?, Skip…?], Close)`. On Linux: Remove index.lock…, Skip…, Close. On macOS: Close, Skip…, Remove index.lock… | :296-332 |
| Focus | **Close** (`focused_first=true`); the others false | :324-331 |
| Escape / backdrop | close (`on_close_request`) | :336 |
| Once | each button answers once (`answered`) | :210-223 |

The output row has padding (0,6,0,6) and one line with no cut marker (:374-386). Pinned by `tests/commit_box.rs::the_git_error_draws_one_viewport_of_output_and_the_skip_only_where_offered`.

#### 2.1 Wiring
- **Commit box's own error** (`APP/commit_box/pane.rs:510-541`): `GitErrorDialog::new(serial, command, lines).sentence(..).skip(skip).remove_lock(index_lock)`. Skip asks the same commit again with `--no-verify` (:546).
- **Every other ending** (`APP/operations/ending_dialog.rs`): `Endings` is a FIFO, oldest first, each entry closed by its own serial (:45-77). `dialog()` builds one from a `Shown` (:150-174). `of_failure` gives `"Git Error"`, the sentence `failure.said`, and details (command, `shown_lines(output)`) (:80-92). A failure without a command becomes `couldnt(name, said)` with no tail. `refused(name, said)` gives `"<said> Nothing was changed."` (:107-109). For `of_write` (:113-146), `Done`, `Cancelled` and `NeedsConfirming` open nothing; `Stale`, `Refused` and `NotRun` open a refusal; `Failed` opens `of_failure`; `Incomplete` opens "Couldn't" plus the kept files (via `shown_path`) plus details if git failed.
- A fetch failure opens `of_failure("Fetch '<remote>'", ..)` (`APP/session.rs:156-171`). A fetch cancel opens nothing.
- Remove index.lock… closes the dialog, then calls `leftover_lock::ask` (:157-161).
- Pinned by `every_ending_has_exactly_one_home`, `the_workers_words_are_the_dialogs` and `endings_wait_their_turn` (ending_dialog.rs tests).

#### 2.2 Ending sentences — `APP/worker/ending_words.rs`
`sentence()` appends a "." if one is missing. The early-return variants are already full.
| Error | Sentence |
|---|---|
| GitFailed | `GIT_FAILED` = `"An unexpected error occurred while performing the git request."` (:17-18, :142) |
| ChangedSinceRead | `"<path> changed after its diff was read."` (:58-60) |
| ContentReadsDisagree | `"<path> changed while it was read."` (:61-63) |
| ChangedSinceConfirmed / LockChangedSinceConfirmed | `"<path> changed since you confirmed."` (:64-66) |
| AmendChangedSinceConfirmed | `"HEAD changed since you confirmed the amend."` (:67-69) |
| NoPaths | `"No file was given."` (:71) |
| CommitRefused::InProgress | `"<Name> is in progress. Continue or abort it with <git cmd> --continue or --abort."`; `git am` stays lower-case (:22-38) |
| CommitRefused::NothingToAmend | `"There is no commit to amend yet."` (:74) |
| CommitRefused::CommitEncoding | `"i18n.commitEncoding is <enc>, and Cairn writes a message only as UTF-8."` (:75-78) |
| *::NotWhatWasConfirmed | `"What was asked is not what was confirmed."` (:180-182) |
| CheckoutRefused::InProgress | `"A merge / A cherry-pick / A revert / A rebase / An am session is in progress. Finish or abort it first."` (:42-53) |
| CheckoutMoved::Head | `"HEAD moved since you pressed Create and Checkout."` (:86) |
| CheckoutMoved::Commit | `"Commit <short> can no longer be read."` (:87-89) |
| CheckoutMoved::Name | `"A branch named '<name>' exists now."` (:90-92) |
| LockRefusal | `"There is no index.lock any more."`, `"index.lock is not a file."`, `"index.lock could not be read (<kind>)."`, `"Cairn is running git in this repository."` (:94-100) |
| ReadWorkingTree / DiffFile | `"<path> could not be read (<source>)."` (:103-108) |
| LockNotRemoved | `"<path> could not be removed (<source>)."` (:109-111) |
| MadeButGitFailed | `"The commit\|amend was made, but git failed afterwards. The history shows it."` (:114-123) |
| CommitUnconfirmed (amend) | `"Something else moved HEAD while the amend ran, so git may have amended another commit. The history shows HEAD as it is now."` (:127-130) |
| CommitUnconfirmed (commit) | `"Something else moved HEAD while the commit ran. The history shows HEAD as it is now."` (:132-134) |
| GitUnwatched | `"Cairn lost hold of git while it ran. The lists and history show anything it had already done."` (:137-141) |
| AmendNotMade | the inner failure's sentence (:143) |
| other | the engine's `shown()` capitalised, e.g. `"The commit was cancelled before git ran."` (:144-150) |
| Refusal (per path, :160-178) | `"Nothing is selected."`; `"<path> is staged, unstaged or discarded only as a whole file."`; `"<path> is conflicted, so it can only be staged whole."`; `"<path> is a submodule; submodule changes can't be discarded."`; `"<path> is a nested repository; nested repositories can't be discarded."`; `"<path> is not a file."`; `"A directory on <path> is a file or a symlink."`; `"<path> has no unstaged change to discard."` |

Other sentences from the local lane (`APP/worker/local/lane.rs`):
- `DiscardIncomplete`: `"Every file was discarded, but git failed afterwards."`, `"1 file was left as it was; the rest were discarded."` or `"N files were left as they were; the rest were discarded."` (:1011-1015).
- `NotRun` at close: `"The repository was closing."` (:1316).

---

### 3. Create Branch — `UI/dialogs/create_branch.rs` (window side `APP/operations/create_branch.rs`)

**Constants (:38-70):**
- Title `"Create Branch"`.
- Subtitle `"Use '/' as a path separator to create folders"`.
- Captions `"Create branch at:"` and `"Branch name:"`.
- Placeholder `"Enter branch name"`.
- Check box `"Check out after create"`.
- `"Local changes:"` with radios `"Don't change"`, `"Stash and reapply"` (always greyed) and `"Discard"`; the greyed radio's reason `"Comes with stashing."`.
- Buttons `"Create"` / `"Create and Checkout"` and `"Cancel"`.
- `DIALOG_WIDTH = 520.0`, `CAPTION_WIDTH = 130.0` (captions right-aligned, `text_secondary`, max 1 line), `FONT_SIZE = 13.0`.

**Layout, top to bottom (:461-480):**
1. `PopupTitle "Create Branch"`.
2. Subtitle: font 12, `text_secondary`, padding (0,8,4,8).
3. `PopupContent` rows. Each row is horizontal, spacing 8: a 130 px caption, then the value (:310-318).
   - **Create branch at:** a commit glyph (8×8 diamond, border 1.5, rotated 45°), then `"<short id> <subject>"`, read-only, max 1 line, ellipsis, width `520-130-60 = 330` px (:319-332).
   - **Branch name:** `text_field`, width 330, placeholder, **auto_focus**, Return → `create()` (:333-340).
   - (blank caption) the check box `"Check out after create"`, always enabled (:344-352).
   - **Local changes:** drawn only while the box is ticked **and** a tracked path has a change (`APP/operations/create_branch.rs:174-182`). It holds a column (spacing 6) of `Don't change`, `Stash and reapply` + `"Comes with stashing."` (font 12, secondary) and `Discard`, with the ⚠ glyph (`RefGlyph::Gone`, `GLYPH_SIZE`, `warning` colour) beside it while Discard is chosen (:353-396). Each opening starts on Don't change.
4. Buttons row. This is custom, not `PopupButtons`: horizontal, padding 8, spacing 4 (:433-459).
   - A flex `said` area on the left holds ⚠ (only when refused) and the refusal, or the waiting text (font 12, `text_secondary`, wraps) (:404-432).
   - Then `ordered(platform, [Button.filled.enabled(ready) primary], Button Cancel)`.
   - Primary = `"Create and Checkout"` if the box is ticked, else `"Create"` (:397-401).

**Radio** (:500-556): 14×14 circle with a 1 px `border`, a 6 px `primary` dot when selected, spacing 6, caption at 13 (`text_secondary` when disabled). a11y RadioButton with toggled state. Pointer cursor. Space or Return while focused.

**Check box** (`UI/check_box.rs:13-74`): 14×14, radius 3, `"✓"` at 11 in `text_inverse` on `primary` when ticked. Caption 13.

**Behaviour:**
- Enabled only when `ready`: the name is non-empty, the engine answered `Free` for the exact text, no write of this dialog is running, and with Discard chosen the consequence has arrived (`APP/operations/create_branch.rs:264-270`).
- A press with Discard chosen builds `Confirmed::by_user(consequence)` inside the dialog. No second dialog opens (UI :282-299).
- Focus: the **name field** (auto_focus). Neither Freya Button opens focused, so this is not the safe-answer pattern.
- a11y role is **Dialog** (not AlertDialog), `a11y_modal`.
- Escape, Cancel and the backdrop all call `on_cancel` → `cancel(view)`, ignored unless Create Branch is on top (APP :410-416).
- The dialog stays open while its write runs and closes on `Done`. A failure opens Git Error or "Couldn't Create branch '<name>'" over it (APP :1-25, :422-448).
- `"Check out after create"` is sticky for the session only (APP :8-11).

**Opened by:**
- The history row's context menu `"New Branch…"`.
- The `Action::NewBranch` chord = **Ctrl+Shift+B / ⌘⇧B** (`UI/accelerators.rs:348`; `APP/shortcuts.rs:84` → `open_at_head`). It does nothing on an unborn HEAD, before refs are read, or while the dialog is already up (APP :125-160).

**`name_refusal` (UI :86-98):**
| NameRefusal | Text |
|---|---|
| AtBrace | `"A branch name can't contain '@{'"` |
| Invalid { reason } | git's reason verbatim |
| Taken | `"Branch <name> already exists"` |
| InsideABranch / HoldsABranch | `"cannot lock ref 'refs/heads/<name>': '<branch>' exists; cannot create 'refs/heads/<name>'"` |

A failed name check shows the engine's failure string (APP :286).

**`discard_refusal` (UI :104-115):** `"<A merge | A cherry-pick | A revert | A rebase | An am session> is in progress. Finish or abort it first."` A failed consequence read shows its string (APP :292-294).

**Waiting text:** `"Waiting for <write name>…"`, e.g. `"Waiting for Commit 1 file…"` (APP :301-311).

Pinned by `tests/create_branch_dialog.rs` (`the_dialog_is_forks_its_button_following_the_box`, `a_refusal_is_said_beside_the_buttons_behind_the_warning_glyph`, `a_long_refusal_is_drawn_whole_and_wrapped`, `a_check_waiting_behind_a_write_is_said_beside_the_buttons`, `local_changes_are_forks_three_in_forks_order_stash_greyed_and_a_chosen_discard_warned`, `discards_press_builds_the_token_from_the_consequence_it_was_handed`, `refusals_are_worded_by_the_view`).

---

### 4. Credential prompt — `UI/dialogs/credential_prompt.rs` (wired `APP/window.rs:363-393`)

- **Width:** `DIALOG_WIDTH = 520.0` (:26).
- **Title:** `"<asking> is asking for a credential"` (:200-203). `asking` is the fetch's remote, or the local write's name (e.g. `"Commit"`), or `"git"` when the token named none (`APP/window.rs:371`).
- **Body:** line 1 is `asking(kind, text)` at font 14 (:135-140). Line 2 is the raw prompt text, trimmed, font 12, `text_secondary`.

`asking()` (:220-235):
| Kind | With subject | Without subject |
|---|---|---|
| Username | `"Username for <url>"` | `"Username"` |
| Password | `"Password for <url>"` | `"Password"` |
| Passphrase | `"Passphrase for the key <key>"` | `"Passphrase"` |
| Confirmation (ssh host key) | `"The host <host> is not yet known. Connect to it anyway?"` | `"Connect anyway?"` |
| Other | `"A secret is being asked for"` | `"A secret is being asked for"` |

**Field kinds (username, password, passphrase, other):**
- A `text_field`, width fill, **auto_focus**, Return submits. Mode is `Shown` for a username and masked (`InputMode::new_password()`) otherwise (:165-186).
- Primary is a Freya `Button.filled` captioned `"Continue"` (:187-191).
- Cancel is an answer button `"Cancel"`, not focused first (:123-133).

**Confirmation (host key):**
- No field. Instead an extra line, font 12, secondary: `"Accepting adds this key to your known hosts. ssh will trust it from now on without asking again."` (`ACCEPTING_IS_PERMANENT`, :216-217).
- Primary is `"Yes, connect"` (Button.filled), which sends `"yes"` (`PromptKind::ACCEPTED`, `MODEL/prompt.rs:53`).
- **Cancel is focused first** (:127).

**Buttons and keys:**
- `ordered(platform, [primary], Cancel)`: on Linux `Continue`/`Yes, connect` then `Cancel`; on macOS the reverse (:194-195).
- No `a11y_modal` or AlertDialog wrapper; only the Popup's Dialog role.
- Escape and the backdrop cancel (`on_close_request`, :199).

Pinned by `tests/credential_prompt.rs` (7 tests: masking, username shown, passphrase, host-key yes, cancel/escape, unknown masked, press outside) plus the unit tests `every_kind_of_prompt_is_stated_with_its_subject` and `a_prompt_that_quotes_nothing_is_still_stated`.

---

### 5. Leftover lock flow — `APP/operations/leftover_lock.rs`

- **Banner:** `LockBanner` text `"index.lock is in this repository, so git can't change the index. If no other git program is running, it was left behind."`. It shows after `LOCK_STALE_AFTER = 10 s` (`UI/title_bar/lock_banner.rs:16-19`), and only while no Cairn git runs or waits (APP :142-155). Its button is `"Remove index.lock…"`.
- **Press**, from the banner or the Git Error: `ask` sends `Request::LockConsequence` (:79-87). The answer then goes one of two ways:
  - Ready: a confirmation titled `"Remove Stale Lock"` with the RemoveLock prompt and `"Remove index.lock"` button (:112-138).
  - Refused: the ending `"Couldn't Remove index.lock"` (`REMOVE_LOCK_NAME`) with `"<sentence> Nothing was changed."` (:92-106).

---

### 6. Context menus (every one)

There are three menus. Each item closes the menu (`ContextMenu::close()`) before it acts. Freya defaults (FREYA/menu.rs, themes.rs:600-621):
- **Container:** padding 4, radius 8, `background`, border `surface_primary`, shadow (0,4,10).
- **MenuButton:** padding (8,14), min width 105, radius 10, hover `surface_secondary`.
- **Escape** closes; so does a press outside.

**No menu uses a separator.**

#### 6.1 History commit row — `UI/history/list.rs:414-450`
- **Opens on:** `on_pointer_down` with `MouseButton::Right`, via `ContextMenu::open_from_down`. A right-click outside the selection first selects that row and moves the cursor (:420-424).
- **Items:** 1. `"New Branch…"` (`NEW_BRANCH_CAPTION`, :16). It is always enabled and opens Create Branch at that commit with its subject.
- **Stash rows:** no menu (returns at :427-428). Lost-commit rows get it, as they are commit rows.
- Pinned by `tests/history_list.rs::every_commit_rows_menu_offers_new_branch_at_its_commit`.

#### 6.2 Local Changes file rows (Unstaged and Staged) — `UI/local_changes/list.rs:720-755`, menu built in `UI/local_changes/menu.rs:188-229`
- **Opens on:** right pointer-down, `open_from_down`. The menu acts on the selection the row is in; a row outside it becomes the selection first (:723-733).

| # | Unstaged list | Staged list |
|---|---|---|
| 1 | `"Stage"` → `Action::StageOrUnstage` | `"Unstage"` |
| 2 | `"Discard Changes…"` → `Action::Discard`, or greyed (see below) | `"Discard Changes…"` **always greyed** |
| 2b | (only when greyed) a disabled item holding the reason (label width 300, font 12, wraps) | `"Staged changes can't be discarded. Unstage them first."` |
| 3 | `"Stage All"` → `Action::StageOrUnstageAll` | `"Unstage All"` |
| 4 | `"Copy Path"`: the selected paths, `\n`-joined, to the clipboard (`APP/local_changes/actions.rs:653-673`) | `"Copy Path"` |

- **Discard greyed on Unstaged:** this happens only when **every** selected row is a submodule, a conflict or a nested repository (`no_discard`, :149-173). The reason is one sentence per kind present, joined by a space (:120-140):
  - `"Submodule changes can't be discarded."`
  - `"Conflicted files can't be discarded. Resolve or stage them first."`
  - `"Nested repositories can't be discarded."`
- **Captions** are constants at :22-27.
- **Not offered** (module doc :1-3): open, external diff, blame, history, ignore, stash, save as patch.
- Pinned by `tests/local_changes_actions.rs::each_rows_menu_offers_forks_items_and_no_discard_where_none_is_allowed` and `a_right_click_outside_the_selection_chooses_its_row_before_the_menu_opens`.

#### 6.3 Commit box "Recent Commit Messages" — `UI/commit_box.rs:524-547`
This is not a right-click menu. A `≡` control (a11y name `"Recent Commit Messages"`, padding 5/14, radius 6, font 13) opens `ContextMenu::open(menu)`.
- **Items:** the first line of each of the last ten messages, newest first. Choosing one recalls that message.
- **Disabled** when the box is stopped (an operation in progress refuses the commit) or there are no messages (`enabled = open && !recent.is_empty()`, :526; `open = !self.stopped`, :350).

#### 6.4 Surfaces with **no** context menu
A grep of `MouseButton`, `on_pointer_down`, `on_secondary` and `ContextMenu` over UI and APP non-test code finds only the three above. None on:
- sidebar rows (Local Changes, All Commits, branches, remotes, tags, stashes, folders)
- ref chips
- the diff view (`staging_gesture.rs:549` handles the primary button only)
- the Changes tab file list
- Commit tab files and files opened in place
- the commit tab header / parents
- the title bar and status box
- activity popover rows
- stash history rows

A test-only menu lives in `APP/window.rs:4590-4641`, the host test.

---

### 7. Activity popover — `UI/title_bar/activity_popover.rs`, `APP/operations/activity.rs`

#### 7.1 Opening and closing
- **Opens from** a press on the title bar's status box (`APP/window.rs:214`, `view.activity.open()`), which selects the newest entry (`APP/operations/activity.rs` `ActivityView::open` and `ActivityLog::opened`). The box's laid-out (left, bottom) is the anchor (`APP/window.rs:215-221`). **No keyboard chord opens it.**
- **Closes on** Escape (its own `on_global_key_down`, :563-568), a press on the full-window transparent backdrop (:530-535), or choosing "Show in Lost Commits". It is modal (`Modal::Popover`, lowest priority), so window chords are inert while it is open.

#### 7.2 Geometry (UI constants :47-61)
| Const | Value | Meaning |
|---|---|---|
| `WIDTH` | 860 | panel width, `min(860, room-16)`, at least 260 (`hung`, :605-610) |
| `HEIGHT` | 460 | panel height |
| `LIST_WIDTH` | 260 | left column |
| `MARGIN` | 8 | kept from the window's edges and the panel's corners |
| `ARROW` | 12 | rotated square under the panel, upper half shows |
| `ARROW_INSET` | 18 | arrow tip, in from the box's left |
| `ENTRY_ROW_HEIGHT` | 38 | left row |
| `LINE_ROW_HEIGHT` | 16 | right line |
| `TOP` | 44 | fallback anchor before the box has been laid out |

- **Panel:** `corner_radius 8`, `background`, 1 px `border`, shadow y4 blur5 `(0,0,0,30)`, a11y Dialog plus `a11y_modal`. `panel_top = box_bottom + ARROW/2 + 2` (:524).
- **Header:** `"Activity"` (`ACTIVITY_TITLE`), font 13 **bold**, padding (8,10,4,10) (:570-578).
- Pinned by `the_panel_hangs_under_the_box_inside_the_window` (unit) and `tests/activity_popover.rs::the_popover_hangs_from_the_status_box_inside_the_window`.

#### 7.3 Left list, one row per operation (newest first, virtualized; `entry_row` :261-321)
- **Row:** horizontal, padding (2,8,2,8), spacing 6, `surface_secondary` background when selected. A press selects it.
- **Column 1 (flex):**
  - name: font 13, `text_primary`, 1 line, ellipsis
  - result line under it, optional: font 11, `text_secondary`, 1 line, ellipsis
- **Then:**
  - started time: font 11, secondary
  - glyph: `"⚠"` for failed (`FAILED_GLYPH`), `"⊘"` for cancelled (`CANCELLED_GLYPH`), font 12; nothing for running or succeeded (:92-99)
  - `"×"` (`CANCEL_OPERATION_CAPTION`), a compact Freya Button, only while running and cancellable (a commit, an amend or a fetch)
- **No status word on the row.**
- **Empty state:** centred `"Nothing has run in this window yet."` (`NO_ACTIVITY`), font 12, secondary (:425-436).

#### 7.4 Right pane, the selected entry (:443-517; padding 8, spacing 6)
1. Name: font 14, **bold**, 1 line.
2. Header line: `"<word> <started>"`, or `"<word> <started> · took <took>"` (`header_line`, :407-412), font 12, secondary.
   - The four `ActivityStatus` words (:83-90): **`"running"`, `"succeeded"`, `"failed"`, `"cancelled"`**.
3. Sentence, if any: font 12, wraps.
4. Prompt, if it ran: `"Confirmed:"` (`CONFIRMED_CAPTION`, font 11, secondary), then the prompt (font 12, wraps).
5. `"Show in Lost Commits"` (`SHOW_IN_LOST_COMMITS_CAPTION`, compact Button), offered for a succeeded amend. It turns Show Lost Commits on and finds the replaced commit (APP :667-674).
6. A bordered, virtualized lines box (flex height). Each line row has padding (0,6,0,6) and mono 12, 1 line (`line_row`, :380-404):
   - `$ git …` in `text_primary`
   - output and endings in `text_secondary`
   - if cut, the first line reads `"Earlier output not kept."` (`EARLIER_NOT_KEPT`)
   - a running entry opens at its end, a finished one at its top, never following new lines (:345-373)

#### 7.5 Strings and bounds the window feeds it (`APP/operations/activity.rs`)
**Bounds:**
- `ACTIVITY_ENTRIES = 200` (:50).
- `LOG_BYTES = 4 × OUTPUT_BYTES` = 4 MiB (:53). Per entry the tail is `OUTPUT_LINES = 10_000` lines and `OUTPUT_BYTES = 1 MiB` (`APP/operations/output_tail.rs:18-20`).
- Lines are cut at `LINE_CUT_BYTES` with the diff's cut marker.

**Started time:** `"HH:MM:SS UTC"`, time of day (:574-585). **Took:** `"N ms"` (<1 s), `"N.N s"` (<60 s), or `"N min N s"` (:588-598).

**Names:**
- Fetch: `"Fetch '<remote>'"` (:157-165).
- Writes (`APP/worker/local/lane.rs:274-326`):
  - `"Stage lines of <path>"`, `"Unstage lines of <path>"`
  - `"Stage 1 File"` / `"Stage N Files"`, `"Unstage …"` (title case)
  - `"Commit"` (0 files), `"Commit 1 file"`, `"Commit N files"` (lower case)
  - `"Amend"`, `"Create branch '<name>'"`
  - the destructive writes use `Consequence::name()`
  - `"A write"` when the write was never seen asked (APP activity :389)

**Result lines:**
- Writes (`lane.rs:386-418`):
  - success: `"staged"`, `"unstaged"`, `"discarded"`, `"Switched to branch '<name>'"` (checkout); none for commit, amend, a plain create or a lock removed
  - failure: `"<verb> failed"` — `"stage failed"`, `"unstage failed"`, `"discard failed"`, `"commit failed"`, `"amend failed"`, `"create branch failed"`, `"remove failed"`
  - cancel: `"cancelled"`
- Fetch:
  - success: `"Already up to date"`, `"Fetched 1 branch"` or `"Fetched N branches"`, counted from git's ref-update lines (:188-196, :522-541)
  - failure: `"fetch failed"`
  - cancel: `"cancelled"`

**Sentences:**
- `WRITE_CANCELLED = "Cancelled before git made it. Nothing was changed."` (:56).
- `FETCH_CANCELLED = "Cancelled while git ran. Whatever it had already fetched stays."` (:59).
- A failure shows its `said` (the ending words in §2.2).
- A success shows `amend_reported` or `happened` (§1.5).

**Command lines:**
- `"$ git <shell-quoted args>"` (:496-519; quoting at :559-570).
- Ending line: `"cancelled"`, `"exit code N"` or `"killed by signal N"`; nothing for exit 0 (:545-556).

**Interactions:**
- Selecting a row → `log.choose(key)`.
- `×` on a write → `Request::CancelWrite`.
- `×` on a fetch → `FetchStatus::cancelling()` + `Request::CancelFetch` (APP :638-652).

Pinned by `tests/activity_popover.rs`: `the_popover_builds_one_viewport_of_operations_and_of_lines` (200 entries, 100k lines), `each_control_reports_what_it_is_for`, `each_row_is_forks_name_result_time_and_glyph`, `the_prompt_is_whole_above_fixed_lines_and_a_cut_is_said`, `a_running_entry_opens_at_its_end_and_a_finished_one_at_its_top`.

---

### 8. Cross-surface observations (facts, not proposals)
- **Three dialog widths:** 520 (confirm, Create Branch, credential), 680 (Git Error), 860 (popover). The Freya default of 500 is never used.
- **Two kinds of button:** the answer `ChoiceButton` (unfilled, its focus ring shown by border) in confirm, Git Error and the credential Cancel; and Freya `Button.filled` in Create Branch's primary and the credential primary. Create Branch's Cancel is an unfilled Freya Button. So the primary is visually filled only in Create Branch and the credential prompt.
- **Focus default:**
  - Cancel: confirm (Counting and Ready)
  - Close: Git Error, Refused
  - name field: Create Branch
  - text field: credential field prompts
  - Cancel: host-key prompt
- **Escape:** every surface closes or cancels on Escape (Popup's handler, or the popover's own). In the confirmation, Return or Space presses the focused button, which is Cancel.
- **Two "Show files" styles:** confirm uses a `▸`/`▾` text disclosure (font 13); the Git Error uses a compact Freya Button with no glyph.
- **a11y roles differ:** confirm and Git Error are AlertDialog + `a11y_modal`; Create Branch is Dialog + modal; the credential prompt has only the Popup's Dialog role and no `a11y_modal`; the popover is Dialog + modal.
- **No dialog** carries an icon, apart from Create Branch's ⚠ glyphs (the refusal and the chosen Discard).

## 06 — Keyboard, palette and typography (as built)

Read-only audit of `main` at `967bb72` (branch `feature/plan-ui-review`). Production code only; test
modules skipped. Anchors are `path:line`. `UI` = `crates/cairn-ui/src`, `APP` = `crates/cairn-app/src`.

### (a) Keyboard

#### The table's shape

- One table: `UI/accelerators.rs`. `Action` (20 variants, `:46-92`, `Action::ALL` `:96`), `Scope` (6, `:123-143`),
  `FieldScope` (1, `:162-165`), `Os` (`Linux` | `MacOs`; every non-macOS platform takes the Linux row, `:179-196`).
- `chords(action, os) -> Chords` (`:278-355`): up to `MOST_CHORDS = 3` (`:225`) per action per OS. "Command" =
  Ctrl on Linux / ⌘ (META) on macOS; "option" = Alt / ⌥ (`:280-283`).
- Triggers (`:200-214`): `Named(NamedKey)` (matched by key), `Physical(Code)` (matched by position — digits,
  letters, `.`, so ⌥ or Shift do not change them), `Press` (pointer on a row), `HeadingPress` (pointer on a Local
  Changes list heading's Stage/Unstage button).
- Lock keys are ignored: `chord_modifiers` masks to Ctrl|Alt|Shift|Meta (`:272-274`); a chord needs exactly its modifiers.
- `heard_in(action)` (`:372-391`) gives each action one scope; `chord_heard_in` (`:360-369`) has exactly one
  exception: a **bare Enter** of a `LocalChanges` action is heard in `LocalChangesLists` only (never the diff).
- Resolution: `resolve_key(event, scope)` (`:450`) / `resolve_key_on` (`:473-485`) — first action in `ALL` order
  whose chord is heard in that scope and matches. `is_chord(event, own)` (`:465-470`) = "a chord of Window, Detail,
  or the view's own scopes" — views use it to leave such keys alone (e.g. Ctrl+↓ is never "next commit").

#### Every `Action`

Chords exactly as the table lists them, in order (first = the one `chord_name` spells).

| Action | Linux chords | macOS chords | Scope heard in | What it does (handler) |
|---|---|---|---|---|
| `PreviousChange` | Ctrl+↑ | ⌘↑ | `Detail` | `diff::actions::step(view,false)` (`APP/shortcuts.rs:57`); heard by `APP/detail/pane.rs:106` and Local Changes' diff `APP/local_changes/pane.rs:239` |
| `NextChange` | Ctrl+↓ | ⌘↓ | `Detail` | `diff::actions::step(view,true)` (`shortcuts.rs:58`); same hearers |
| `ToggleSideBySide` | — none | — none | `Window` | `diff::actions::toggle_side_by_side` (`shortcuts.rs:69`); reached only from the diff bar button via `of_header` (`shortcuts.rs:19-29`) |
| `ToggleIgnoreWhitespace` | — | — | `Window` | `change_settings(toggle_ignore_whitespace)` (`shortcuts.rs:59-62`); button only |
| `MoreLines` | — | — | `Window` | `change_settings(more_lines)` (`shortcuts.rs:63`); button only |
| `FewerLines` | — | — | `Window` | `change_settings(fewer_lines)` (`shortcuts.rs:64`); button only |
| `EntireFile` | — | — | `Window` | `change_settings(toggle_entire_file)` (`shortcuts.rs:65-68`); button only |
| `ExtendSelection` | Ctrl+press (`Press`) | ⌘+press | `Window` (press resolved where it lands) | no-op in `shortcuts.rs:72-75`; resolved by the list via `HeldKeys::press()` — history second commit (`UI/history/list.rs:403`), Local Changes toggle (`UI/local_changes/list.rs:403-404`) |
| `SelectRange` | Shift+press | Shift+press | `Window` | no-op in shortcuts; Local Changes range press (`UI/local_changes/list.rs:403`) |
| `ExtendSelectionUp` | Shift+↑ | Shift+↑ | `LocalChangesLists` | no-op in shortcuts; list extends selection from anchor (`UI/local_changes/list.rs:631-668`) |
| `ExtendSelectionDown` | Shift+↓ | Shift+↓ | `LocalChangesLists` | same |
| `ShowCommitTab` | Ctrl+Alt+1 (physical `Digit1`) | ⌘⌥1 | `Window` | sets `DetailTab::Commit` and un-collapses the pane (`shortcuts.rs:55,95-98`) |
| `ShowChangesTab` | Ctrl+Alt+2 | ⌘⌥2 | `Window` | sets `DetailTab::Changes`, un-collapses (`shortcuts.rs:56`) |
| `Refresh` | F5 (bare) | ⌘R (physical `KeyR`) | `Window` | `submit(Request::Refresh)` (`shortcuts.rs:86-90`) |
| `StageOrUnstage` | Enter (bare) · Ctrl+S · Ctrl+Shift+S | Enter (bare; spelled "Return") · ⌘S | `LocalChanges`; bare Enter → `LocalChangesLists` (`chord_heard_in`) | no-op in shortcuts (`:78-82`); lists → `ListIntent::Act` → `local_changes::actions::act` → `stage_or_unstage` (`APP/local_changes/actions.rs:206,427`); on the diff → `actions::on_the_diff` (`APP/local_changes/pane.rs:391`, gesture verb `actions.rs:248-249`) |
| `StageOrUnstageAll` | Ctrl+Alt+Shift+S · Shift+heading-press | ⌘⌥⇧S · ⌥+heading-press | `LocalChanges` | `everything(list)` for the focused list or the selection's (`actions.rs:428-439`); heading reads `held.heading_press()` (`UI/local_changes/list.rs:491`) |
| `Discard` | Backspace · Delete · Ctrl+Shift+D | Backspace (spelled "⌫") · ⇧⌘D | `LocalChanges` | `discard_rows` → confirmation (`actions.rs:440-444`); on diff → gesture `Discard` (`actions.rs:250`) |
| `Commit` | Ctrl+Enter | ⌘Enter ("⌘Return") | `CommitBox` | claimed by the commit box's fields (`FieldKey::Own`) and pressed as the button when ready (`UI/commit_box.rs:355-367`); no-op in shortcuts |
| `ShowLostCommits` | Ctrl+Shift+. (physical `Period`) | ⌘⇧. | `History` | history list hears it (`UI/history/list.rs:309`) → `lost_commits::history_action` → `toggle` (`APP/history/lost_commits.rs:12-20`) |
| `NewBranch` | Ctrl+Shift+B | ⌘⇧B | `Window` | `operations::create_branch::open_at_head(view)` (`shortcuts.rs:84`) |

Gatekeeping: `shortcuts::act` returns early while `keys_inert(view)` (any modal — credential prompt, confirmation,
Git Error/"Couldn't", Create Branch, activity popover; `shortcuts.rs:35-37,51`). `history_action` checks the same
(`lost_commits.rs:13`). Window scope is heard by `on_global_key_down` on the window root (`APP/window.rs:199-205`).

#### Scopes

| `Scope` | Heard where (who calls `resolve_key` for it) | Actions in it |
|---|---|---|
| `Window` (`:126`) | window root global listener, `APP/window.rs:202` | Toggle*/More/Fewer/EntireFile (no chords), ExtendSelection, SelectRange (presses), ShowCommitTab, ShowChangesTab, Refresh, NewBranch |
| `Detail` (`:130`) | detail pane `on_key_down`, `APP/detail/pane.rs:106`; Local Changes' diff, `APP/local_changes/pane.rs:239` | PreviousChange, NextChange |
| `LocalChanges` (`:133`) | Local Changes lists `UI/local_changes/list.rs:629`; its diff `APP/local_changes/pane.rs:391` | StageOrUnstage (modified chords), StageOrUnstageAll, Discard (incl. bare Backspace/Delete) |
| `LocalChangesLists` (`:136`) | lists only, `UI/local_changes/list.rs:630` | StageOrUnstage's bare Enter; ExtendSelectionUp/Down |
| `CommitBox` (`:140`) | commit box fields via `FieldScope::CommitBox` (`UI/commit_box.rs:366,402`) | Commit |
| `History` (`:142`) | history list `UI/history/list.rs:309,316` | ShowLostCommits |

`FieldScope` (`:162-177`): only `CommitBox` → `Scope::CommitBox`. Rule: a field's own scope holds no bare chord
(`a_fields_own_scope_holds_no_bare_chord`).

#### What a text field does with a key (`field_key_on`, `UI/accelerators.rs:534-564`; applied by `UI/text_field.rs:50-89`)

Order of decision:
1. (subject only) bare ↑/↓ offered to `recall` first; taken → stop + prevent default (`text_field.rs:57-64`).
2. A chord of the field's own `FieldScope` → `FieldKey::Own(action)`: stop propagation, prevent default, call
   `on_action` — so Ctrl/⌘+Enter commits without a newline (`:541-545`; `text_field.rs:66-74`).
3. A `Window`-scope chord → `Unclaimed`: not typed, stop propagation (window global listener still hears it) (`:546-548`).
4. Shift alone → `Edit { bubbles: true }` (a scroll view behind reads Shift) (`:550`).
5. Ctrl / Alt / AltGraph / Meta key alone → `Unclaimed` (so `HeldKeys` sees it) (`:551-553`).
6. Ctrl/Meta + character that is not a Freya `EditBindings` default (copy/paste/undo…) → `Unclaimed`, types nothing (`:556-561`).
7. Everything else → `Edit { bubbles: false }`: editor handles it (`Input::key_down_default`), kept from surrounding
   views — a bare Enter/Backspace in a field never reaches a list chord (`:562`; `text_field.rs:81-86`).

Builders: `text_field` (filters; no own scope, `text_field.rs:23`), `text_field_in` (commit description, `:29`),
`text_field_recalling` (commit subject, `:41`). Note `Detail`-scope chords are not passed through by fields
(only `Window`'s), by design (⌘↑ is start-of-document on macOS).

#### `recall_step` (`UI/accelerators.rs:566-593`)

Commit subject only: bare ↑ → `RecallStep::Older`, bare ↓ → `Newer`; any modifier held → `None` (Shift+↑ selects).
Not a table chord. The box decides whether it takes the step (only while the subject is empty or holds the last
recalled message): `APP/commit_box/pane.rs:170,339` (`recall_step(step, view) -> bool`).

#### `HeldKeys` and pointer-press modifiers (`UI/accelerators.rs:595-647`)

- This Freya build carries no modifiers on a pointer event, so modifiers for ⌘/Ctrl-click, Shift-click and the
  heading's Shift/⌥-press are read from `HeldKeys`, kept from key down/up events the window hears
  (`APP/window.rs:199-208`; `heard()` counts the modifier key itself in on down/out on up, `:615-625`).
- Reset to empty when the app loses focus (`APP/window.rs:144-151`), since releases elsewhere are unheard.
- Public answers are actions only: `press()` → `resolve_press_on` (`:489-495`), `heading_press()` →
  `resolve_heading_press_on` (`:499-505`). No equality, opaque `Debug` (`:601-608`).
- Consumers: `UI/history/list.rs:403` (ExtendSelection → compare), `UI/local_changes/list.rs:403-404,491`.
- `Chord::key_press` / `press_hold` (`:418-446`) exist for headless tests only.

#### Tooltips: `chord_name` (`UI/accelerators/chord_names.rs:19-55`)

- Spells the action's **first** chord; `None` for no chord or a pointer chord.
- Linux: Fork-Windows form, held keys in order Ctrl, Alt, Shift, Super joined with `+` (`:31-36,48-50`).
  macOS: symbols ⌘ ⌥ ⇧ ⌃ run together, then the key (`:37-42`).
- Key names: Enter→"Return", Backspace→"⌫", Delete→"⌦" on macOS only; arrows ↑↓←→; Escape→"Esc"; other named
  keys by Debug name (e.g. "Enter", "Backspace", "F5"); physical keys by cap (`KeyS`→"S", `Digit1`→"1", `Period`→".") (`:58-100`).
- Strings it produces for the table today:

| Action | Linux | macOS |
|---|---|---|
| PreviousChange / NextChange | `Ctrl+↑` / `Ctrl+↓` | `⌘↑` / `⌘↓` |
| ShowCommitTab / ShowChangesTab | `Ctrl+Alt+1` / `Ctrl+Alt+2` | `⌘⌥1` / `⌘⌥2` |
| Refresh | `F5` | `⌘R` |
| StageOrUnstage | `Enter` | `Return` |
| StageOrUnstageAll | `Ctrl+Alt+Shift+S` | `⌘⌥⇧S` |
| Discard | `Backspace` | `⌫` |
| Commit | `Ctrl+Enter` | `⌘Return` |
| ShowLostCommits | `Ctrl+Shift+.` | `⌘⇧.` |
| NewBranch | `Ctrl+Shift+B` | `⌘⇧B` |
| Extend/SelectRange, diff toggles | none | none |

- **Only one call site renders a chord**: the Show Lost Commits check box tooltip in the history header
  (`UI/history/commit_row.rs:296-305`). The diff bar's tooltips are names only (`UI/diff/header.rs:182`, labels
  `:29-43`); the Local Changes chevron tooltip is its caption (`UI/local_changes/list.rs:526`); no menu item,
  button or the commit button shows its chord.

#### Keys handled outside the table (views' own keys, not chords)

- History list: ↑/↓, PageUp/PageDown (±`PAGE_JUMP`=10, `UI/history/list.rs:23`), Home/End (`:355-360`).
- Commit tab file list ↑/↓ (`UI/detail/commit_tab.rs:504-505`); Changes tab list ↑/↓ (`UI/detail/changes_list.rs:289-290`);
  Local Changes lists ↑/↓ (`UI/local_changes/list.rs:660-663`).
- Diff view: ↑/↓ one row, PageUp/PageDown, Home/End, ←/→ by 4 advances (`UI/diff/view.rs:411-418`); Escape clears the
  gesture host's state (`:352`).
- Escape: cancels a Local Changes drag (`UI/local_changes/drag.rs:146`); closes the activity popover (`UI/title_bar/activity_popover.rs:564`).
- Tab/Shift-Tab focus movement is Freya's (named in the module doc, `accelerators.rs:33-35`).
- Sidebar: no key handling found in `UI/sidebar.rs` (pointer only).

#### Common Fork shortcuts with NO `Action` (absent from `Action` at `UI/accelerators.rs:46-92`)

Fetch (the title bar's "Fetch <remote>" button only, `APP/window.rs:575-602`), Pull, Push, Stash / Stash pop,
Find/Search commits, focus the filter fields, Preferences, Open/Clone repository, Close/Quit, switching to
Local Changes vs All Commits (sidebar presses only), File Tree tab (⌘⌥3 reserved, `accelerators.rs:19`),
Checkout, Merge, Rebase, Cherry-pick, Revert, Reset, Create tag, Copy SHA/path (menu item "Copy Path" exists,
no chord), Amend toggle, Select all in lists, Undo, Blame/File history, Toggle sidebar/pane collapse.
Present: Commit tab vs Changes tab (`ShowCommitTab`/`ShowChangesTab`).

### (b) Palette and typography

#### Theme

- `use_init_theme(dark_theme)` unconditionally (`APP/main.rs:61`). No light theme, no `PreferredTheme`/system
  dark-mode following, no theme switch anywhere in `UI`/`APP` (only a test reads `LIGHT_COLORS`). Window root
  paints `.theme_background()` (`APP/window.rs:196`).
- Freya `DARK_COLORS` (pinned fork `caa46f8`, `freya-components/src/theming/themes.rs:116-157`); `build_theme`
  replaces `primary/secondary/tertiary` with the platform accent if Freya can read one (`themes.rs:712-730`).

| Freya token | Value (dark) | Cairn uses (count) — what it paints |
|---|---|---|
| `background` | rgb(20,20,20) | 4 — window ground, dialog/popover bg (`UI/dialogs/answer_button.rs:47,123`, `UI/title_bar/activity_popover.rs:547,560`) |
| `surface_primary` | (60,60,60) | 3 — check box unticked fill (`UI/check_box.rs:40`), commit button bg (`UI/commit_box.rs:499`), answer button bg (`answer_button.rs:76`) |
| `surface_secondary` | (45,45,45) | 7 — **selection** background: history row (`UI/history/commit_row.rs:109`), sidebar row (`UI/sidebar.rs:473`), Local Changes row (`UI/local_changes/list.rs:711`), Changes list (`UI/detail/changes_list.rs:322`), popover entry (`activity_popover.rs:279`); also `sidebar.rs:259`, `commit_box.rs:452` |
| `surface_tertiary` | (25,25,25) | 5 — detail tab strip (`UI/detail/tabs.rs:114`), Local Changes list headings (`UI/local_changes/list.rs:563`), banners (`APP/window.rs:629`, `UI/title_bar/lock_banner.rs:97`), `APP/local_changes/pane.rs:313` |
| `border` | (60,60,60) | 16 — separators, box/field outlines, history header rule (`commit_row.rs:262`) |
| `border_focus` | (110,110,110) | 4 — focused list outline (`UI/history/list.rs:267`, `UI/local_changes/list.rs:584`, `UI/detail/changes_list.rs:213`), dialog focus ring (`answer_button.rs:75`) |
| `border_disabled` | (80,80,80) | 2 — `answer_button.rs:43,77` |
| `text_primary` | (250,250,250) | 33 — body text, subjects, chip text (`UI/history/ref_chips.rs:256`) |
| `text_secondary` | (210,210,210) | 38 — author/id/date columns (`commit_row.rs:106`), counts, captions, glyphs |
| `text_placeholder` | (150,150,150) | 12 — muted text, history headings, lost (dimmed) rows (`commit_row.rs:100`), notices |
| `text_highlight` | (96,145,224) | 3 — **accent**: active detail tab (`tabs.rs:113`), active diff toggle (`UI/diff/header.rs:162`), parent link (`UI/detail/commit_tab.rs:829`) |
| `text_inverse` | WHITE | 2 — check mark (`check_box.rs:42`), gesture action text (`UI/diff/staging_gesture.rs:812`) |
| `primary` | (103,80,164) Freya purple, or platform accent | 2 — ticked check box fill (`check_box.rs:38`), Create Branch radio dot (`UI/dialogs/create_branch.rs:520`) |
| `success` (129,199,132) / `error` (229,115,115) / `warning` (255,213,79) / `info` (100,181,246) | | change-kind badges: Added/Deleted/Modified·TypeChanged·Conflicted/Renamed·Copied·Submodule (`UI/local_changes/list.rs:792-795`, `UI/detail/commit_tab.rs:907-910`); `error` also failure lines (`APP/window.rs:653`, `APP/detail/pane.rs:261`, `UI/commit_box.rs:378`); `warning` also "upstream gone" (`UI/title_bar/status_box.rs:158`), gone-glyph (`UI/sidebar.rs:465,507`), whitespace notice (`header.rs:161`), create-branch warnings (`create_branch.rs:384,416`) |
| `disabled` | (50,50,50) | 6 — disabled glyphs/borders (`check_box.rs:32`, `tabs.rs:115`, `header.rs:163`, `local_changes/list.rs:435,531`, `commit_box.rs:494`) |
| unused | `secondary`, `tertiary`, `surface_inverse*`, `focus`, `active`, `overlay`, `shadow` | not read by Cairn code (Freya's own components may) |

#### Cairn's own colour tokens (named consts)

| Name | Value | file:line | Paints |
|---|---|---|---|
| `FORK_GROUND` (private) | #282828 | `UI/diff/palette.rs:20` | reference for retuning only |
| `GROUND` | rgb(20,20,20) (= Freya dark background) | `UI/diff/palette.rs:24-25` | diff view ground under rows |
| `REMOVED_TINT` | Fork #633F3E → (79,43,42) | `palette.rs:28` | removed line bg |
| `ADDED_TINT` | Fork #3A5C3F → (38,72,43) | `palette.rs:30` | added line bg |
| `REMOVED_EMPHASIS` | Fork #9F4247 → (139,46,51) | `palette.rs:32` | intra-line range, removed |
| `ADDED_EMPHASIS` | Fork #388442 → (36,112,46) | `palette.rs:34` | intra-line range, added |
| `DIFF_TEXT` | #DDDDDD | `palette.rs:37` | diff text (also notices `UI/diff/notice.rs:466`) |
| `DIFF_MUTED` | #A0A0A0 | `palette.rs:39` | line numbers, hunk headers, EOF marker |
| `GUTTER_SEPARATOR` | Fork #4B4B4B → (55,55,55) | `palette.rs:41` | gutter/text rule |
| `FILLER` | Fork #424242 → (46,46,46) | `palette.rs:44` | side-by-side filler |
| `HEADER_BAR` | Fork #333333 → (31,31,31) | `palette.rs:46` | diff header bar |
| `CURRENT_CHANGE` | rgb(96,145,224) (copy of `text_highlight`, pinned by test) | `palette.rs:52` | outline of change moved to; gesture outline `UI/diff/staging_gesture.rs:893` |
| `LANE_COLOURS` (8) | (86,180,233) sky, (230,159,0) orange, (0,158,115) green, (204,121,167) purple, (240,228,66) yellow, (0,114,178) blue, (213,94,0) vermillion, (190,190,190) grey — Okabe–Ito less black + neutral | `UI/history/lane_palette.rs:6-15` (fallback grey `:22`) | graph lines/nodes (`UI/history/graph_cell.rs:38,55`), branch/remote/stash chip tint |
| `TAG_INDIGO` | rgb(98,92,196) | `UI/history/ref_chips.rs:51` | tag chips |
| chip alpha | tint `.with_a(56)` fill, `.with_a(140)` cap, 1px tint border | `ref_chips.rs:263-271` | ref chips |
| gesture alpha | accent `.with_a(56)`, `.with_a(14)` | `UI/diff/staging_gesture.rs:741,751` | drag-selection / hovered chunk tint |
| `Color::WHITE` | | `UI/history/ref_glyphs.rs:242` | glyph rasterising (mask) |

Files that define colours: `UI/diff/palette.rs` (diff), `UI/history/lane_palette.rs` (lanes), `UI/history/ref_chips.rs`
(`TAG_INDIGO`). Everything else reads Freya's theme via `get_theme_or_default().read().colors()` at the call site
(~25 files). **No central Cairn theme/palette module** for the chrome; the diff palette is the only token module.

#### Font families

| Family | Where | Applies to |
|---|---|---|
| IBM Plex Mono Regular (`DIFF_FONT_FAMILY = "IBM Plex Mono"`, `UI/diff/palette.rs:56`) | embedded: `APP/assets/fonts/IBMPlexMono-Regular.ttf` (+ `IBMPlexMono-LICENSE.txt`), `include_bytes!` `APP/main.rs:40`, registered `LaunchConfig::with_font(DIFF_FONT_FAMILY, DIFF_FONT)` `main.rs:55` | diff rows and numbers (`UI/diff/row_parts.rs:85,116`), diff header path (`header.rs:210`), notices (`notice.rs:478`), gesture (`staging_gesture.rs:902`), toggle glyph text (`toggle_glyphs.rs:59,78`); Commit tab id/parents/file paths/message lines (`commit_tab.rs:717,809,831,921`); Changes tab compare ids (`changes_list.rs:379,473`); Local Changes paths (`local_changes/list.rs:823`); sidebar counts (`sidebar.rs:500`); title-bar ahead/behind counts (`status_box.rs:171`); Git Error output (`git_error.rs:247,255,287`); popover lines (`activity_popover.rs:399`); commit description field (`commit_box.rs:399`) |
| Freya default (no `font_family` set) | `freya-core/src/style/default_fonts.rs:3-17`, set as `fallback_fonts` by `freya-winit/src/config.rs:348` | **all other UI text**. Linux: "Ubuntu", "Adwaita Sans", "Noto Sans", "Arial"; macOS ".AppleSystemUIFont", "Noto Sans", "Arial". Includes the history list's id/date columns (not mono: `commit_row.rs:166-182`) and the commit subject field |

- Only Regular weight of Plex Mono is embedded; bold is `FontWeight::BOLD` on the UI face (HEAD subject `commit_row.rs:155`,
  sidebar current/section `sidebar.rs:489`, status box repo name `status_box.rs:192`, list headings
  `local_changes/list.rs:570,801`, commit tab `:844,919`, popover `:467,575`). No IBM Plex Sans. No tabular-nums/font features.
- Freya default font size is 16 (`freya-core/src/style/font_size.rs:10-13`); Freya typography theme body 16, caption 12.

#### Font sizes by surface

| Surface | Size | Anchor |
|---|---|---|
| Diff text, diff header path, Commit tab message lines | 11 (`DIFF_FONT_SIZE`) | `UI/diff/palette.rs:61` |
| Diff line numbers | 9 (`NUMBER_FONT_SIZE`) | `UI/diff/row_parts.rs:33` |
| History rows (subject/author/id/date/headings) | 13 (`ROW_FONT_SIZE`) | `UI/history/commit_row.rs:20` |
| Ref chips | 12 (`CHIP_FONT_SIZE`) | `UI/history/ref_chips.rs:37` |
| Sidebar rows / section headings / counts | 13 (`FONT_SIZE`) / 12 / 12 | `UI/sidebar.rs:40,488,501` |
| Local Changes rows / headings, filter | 13 / 12 | `UI/local_changes/list.rs:79,504,569,273` |
| Changes tab list / summary | 13 / 12 | `UI/detail/changes_list.rs:46,165` |
| Commit tab body / subject / captions (AUTHOR…) | 13 / 15 / 11 (+1 at `:708`) | `UI/detail/commit_tab.rs:69-71` |
| Detail tab strip | 12 | `UI/detail/tabs.rs:10` |
| Diff header notice | 12 | `UI/diff/header.rs:47` |
| Title bar "Cairn" / loaded count / status box | 16 / 13 / 13 | `APP/window.rs:553,564`; `UI/title_bar/status_box.rs:29` |
| Window notices (message / path) / banner / list line | 14 / 12 / 13 / 13 | `APP/window.rs:610,616,633,651` |
| Commit box description / small texts / button | 12 / 12 / 13 | `UI/commit_box.rs:76,376,461,500,557,611` |
| Check box caption / mark | 13 / 11 | `UI/check_box.rs:9,42` |
| Dialogs body / buttons / details | 14 / 14 / 12–13 | `UI/dialogs/confirm.rs:204-369`, `answer_button.rs:51,128`, `git_error.rs:241-349`, `credential_prompt.rs:139-170`, `create_branch.rs:70,369,429,475` (13 / 12) |
| Activity popover | 11–14 | `UI/title_bar/activity_popover.rs:289-574` |
| Context menu | 12 | `UI/local_changes/menu.rs:220` |
| Lock banner | 13 | `UI/title_bar/lock_banner.rs:102` |

Sizes in use: 9, 11, 12, 13, 14, 15, 16. Named consts: 15 size consts; ~55 of 95 `font_size(` calls are inline literals.

#### Row heights (virtualized lists and bars)

| List / bar | Height | Anchor (const; list) |
|---|---|---|
| History rows and header | 26 `ROW_HEIGHT` | `UI/history/graph_geometry.rs:5`; `UI/history/list.rs:283` |
| Sidebar rows | 22 `SIDEBAR_ROW_HEIGHT` (indent 14/level, base 8) | `UI/sidebar.rs:29,31,283,462` |
| Commit tab files / Changes tab files / Local Changes rows | 24 `DETAIL_ROW_HEIGHT` | `UI/detail/commit_tab.rs:50,473`; `changes_list.rs:258`; `local_changes/list.rs:592` |
| Local Changes list heading | 26 `LIST_HEADER_HEIGHT` | `UI/local_changes/list.rs:77` |
| Diff rows | 17 `DIFF_ROW_HEIGHT` | `UI/diff/view.rs:42,333` |
| Diff header bar / detail tab strip / Changes summary | 30 / 30 / 30 (60 when comparing) | `UI/diff/header.rs:26`; `UI/detail/tabs.rs:8`; `changes_list.rs:44,487` |
| Ref chip | 18 | `UI/history/ref_chips.rs:33` |
| Git Error output & files | 16 `OUTPUT_ROW_HEIGHT` | `UI/dialogs/git_error.rs:47,232,265` |
| Confirm "Show files" | 20 `SHOWN_FILE_HEIGHT` (box 200) | `UI/dialogs/confirm.rs:57,60,341` |
| Activity entries / lines | 38 / 16 | `UI/title_bar/activity_popover.rs:57,59,439,368` |
| Scrollbar / end room | 16 / 20 | `UI/end_room.rs:15,19` |

#### Layout sizes, paddings, radii, borders

- Panels: sidebar 240 (min 140) `APP/sidebar/state.rs:24,26`; detail pane 260 (min 90) `APP/window.rs:33,35`; history min 80
  `:37`; main min width 240 `:39`; Changes-tab list 35% (min 200, diff min 240) `APP/detail/changes_tab.rs:42-45`;
  Local Changes 200/240 mins `APP/local_changes/pane.rs:83-84`; Unstaged/Staged split 50% (min 80) `UI/local_changes/list.rs:150,112`.
- History columns: author 150, id 72, date 132, gap 10, row padding 10 (`UI/history/commit_row.rs:13-18`); lane 14,
  node radius 4, stroke 1.8, ring 2, max lanes 24 (`graph_geometry.rs:8-16`, `graph_cell.rs:15`).
- Diff gutters: number padding 6, lead 2, text padding 4, separator 1 / current 3, end padding 24 (`UI/diff/row_parts.rs:38-47`).
- Dialog widths: 520 (confirm, create branch, credential) `confirm.rs:41`, `create_branch.rs:67`, `credential_prompt.rs:26`;
  Git Error 680, output 240 `git_error.rs:45,49`; popover 860×460, list 260 `activity_popover.rs:47-49`; menu 300 `menu.rs:219`.
- Corner radii (all inline but chips): chips 3 (`ref_chips.rs:48`), check box 3 (`check_box.rs:24`), commit button 6
  (`commit_box.rs:488`), answer buttons 6 (`answer_button.rs:40,121`), create-branch radio 7 / dot 3
  (`create_branch.rs:506,519`), gesture actions 4 (`staging_gesture.rs:802`), list heading button 4 (`local_changes/list.rs:429`),
  popover 8 (`activity_popover.rs:559`). Freya buttons: 6, padding 6/12 (compact 3/6) (`themes.rs:173-187`).
- Borders: 1px everywhere except focus ring 2 (`answer_button.rs:111`), radio 1.5 (`create_branch.rs:494`); bottom-only
  rules on history header (`commit_row.rs:281`), detail tabs (`tabs.rs:132,154`), commit box (`commit_box.rs:453`).
- Paddings (48 calls): mostly inline `Gaps::new(0,8,0,8)` / `(0,12,0,12)` / `(4,6,4,6)` / `(6,12,6,12)` / `(8,12,8,12)`
  title bar (`APP/window.rs:552`); named only in history (`ROW_PADDING`), commit tab (`PADDING=12`, `GAP=10`,
  `commit_tab.rs:74-75`), chips (`NAME_PADDING=5`), diff gutters. Spacing: 55 calls, values 2–32, mostly inline.

Inline (unnamed) numeric style literals — font_size / padding / spacing / radius / px / border width — per file:
create_branch.rs 23, activity_popover.rs 22, commit_box.rs 14, local_changes/list.rs 12, changes_list.rs 11,
APP/window.rs 11, toggle_glyphs.rs 10, notice.rs 10, git_error.rs 10, confirm.rs 10, sidebar.rs 9, answer_button.rs 9,
staging_gesture.rs 8, check_box.rs 6, commit_tab.rs 5, credential_prompt.rs 4, lock_banner.rs 3, header.rs 3,
tabs.rs 3, status_box.rs 2, menu.rs 2, ref_chips.rs 2, commit_row.rs 2, unified_rows.rs 2, APP local_changes/pane.rs 2,
APP detail/pane.rs 2, history/list.rs 1, side_by_side_rows.rs 1, row_parts.rs 1 (approximate grep count, ~200 total).

#### `docs/design/ui.md` "## Palette and type" (`docs/design/ui.md:403-418`) — what it prescribes vs built

| Prescribed | Built |
|---|---|
| Dark-first (because Freya `dark_theme`) | Yes — `dark_theme` only (`APP/main.rs:61`) |
| Cool graphite neutrals with slight blue bias | No — Freya's neutral greys (20/25/45/60), no blue bias |
| One accent, desaturated steel blue, for HEAD, selection, primary action | Partial — accent `text_highlight` (96,145,224) on active tab/toggle/current change/parent link; selection is grey `surface_secondary`; HEAD is bold only; check box/radio use Freya `primary` purple (or platform accent) |
| Red/green reserved for diffs, semantic | Mostly — but success/error also tint change-kind badges |
| Lane colours: six-step CVD-safe set; identity not colour-alone | Eight-step Okabe–Ito + grey (`lane_palette.rs:6-15`) |
| IBM Plex Sans for interface, Mono for ids/paths/diffs | Mono yes (Regular only); **no Plex Sans** — UI text is Freya's system fallback (Ubuntu/Adwaita Sans/Noto Sans); history id column not mono |
| `tabular-nums` where digits align | Not found |
| Light theme is an obligation (Open: "A light theme, with its own diff palette", `ui.md:436-438`) | Absent |

Mockup tokens (`docs/design/mockups/cairn-ui.html:5-13`) for comparison: bg #1B1D22, surface #23262D, raised #2C3038,
line #353A44, fg #E6E8EC, muted #8B919C, accent #6FA3D6, add #4FAE6E, del #E06C6C, warn #D9A441; lanes `--l0..l5`
##6FA3D6 #D9A441 #B57BD6 #4FAE6E #E08A5A #62C2C2; sans "IBM Plex Sans", mono "IBM Plex Mono". None of these values appear in code.

## Fork surfaces Cairn does not have at all

Read-only audit of `docs/design/ui.md`, `docs/design/feature-inventory.md`, `docs/design/cairn.md`,
`docs/work/daily-loop/roadmap.md` and `docs/design/mockups/cairn-ui.html`, checked against
`crates/cairn-ui/src/` and `crates/cairn-app/src/` (outside `worker/` except where a string is
built there). "String scan" means a scan of production string literals (test modules,
`tests.rs` and comment lines skipped) in both crates for the term given. All `path:line` anchors
are as of `feature/plan-ui-review` @ 967bb72.

### A. Described as Fork's layout, kept (`ui.md`, "The layout model, kept from Fork", 15-54, and its table 58-91)

| Surface | Where described | Absent — evidence |
| --- | --- | --- |
| **Toolbar** with `Fetch · Pull · Push · Stash │ repo · branch │ actions` | `ui.md:21` (diagram); `ui.md:88` (drops Fork's Appearance / Workspace / Feedback, adds *Open in terminal / editor* and a command palette); mockup `cairn-ui.html:244-258` (Fetch with a `2` badge, Pull, Push, Stash; centre "cairn main · ~/Development/cairn"; right Branch, Worktree, Terminal, Palette) | No toolbar element. The title bar row holds only "Cairn", the status box, the count and `Fetch <remote>` (`window.rs:538-574`). String scan: 0 literals matching `\bPull`, `\bPush`, `Palette`, `Terminal`, `[Ee]ditor`; `[Ss]tash` hits only the sidebar section, the greyed "Stash and reapply", and `stash@{n}` chips. |
| Toolbar's Pull/Push ahead-behind badges (Fork 2.62/2.16) | `docs/research/refs-and-status/fork-refs-and-status-ui.md:578-579` (evidence; mockup draws `Fetch 2`) | No Pull/Push buttons, so no badges; counts appear only in the status box (`title_bar/status_box.rs:89-95`). |
| Status box's **branch dropdown** (Fork's "branch drop down selector on toolbar") | `fork-refs-and-status-ui.md:568-574` | `StatusBox` is labels only (`status_box.rs:183-215`); the only handler on it opens the popover (`window.rs:209`). |
| **Repository tabs** across the top | `ui.md:41-42`; mockup tab strip `cairn ● / freya / gitoxide` (`cairn-ui.html:259`, `337`, `418`, `564`) | No tab strip; one repository from argv/cwd (`crates/cairn-app/src/repository_path.rs`); string scan: 0 literals matching `Repositor` or `\bOpen`. Window title is the constant "Cairn" (`main.rs:50`), never the repo. |
| **Worktrees sidebar section**, worktree chip on a branch, disabled checkout | `ui.md:83`, `ui.md:97`; mockup `cairn-ui.html:554-610` | `SidebarSection` is `Branches, Remotes, Tags, Stashes` only (`crates/cairn-model/src/sidebar_rows.rs:27-36`); `docs/systems/sidebar.md:215` lists Worktrees as not built. String scan: `[Ww]orktree` hits only `window_check.rs` fixtures. |
| **Submodules sidebar section** | `ui.md:98` | Same enum; only a submodule badge/notice in Local Changes (`local_changes/list.rs:6-9`, `diff/notice.rs:32`). |
| **Forge links** in branch/commit menus (create PR, open on forge, copy permalink) and the post-push toast | `ui.md:86`; mockup `cairn-ui.html:596-609` | String scan: 0 for `[Pp]ull [Rr]equest`; no menu on any ref (sidebar has no context menu; history row menu is "New Branch…" only, `history/list.rs:439-447`). |
| **Three-way, region-level conflict resolution** | `ui.md:87`, `ui.md:207-210`; mockup "Conflict" `cairn-ui.html:469-553` | A conflicted path draws the notice "Unmerged path — conflicts must be resolved before a diff can be shown" (`diff/notice.rs:50-51`) and stages whole; no resolver, no "Use ours/theirs", no merge bar. |
| **Command palette**, **Open in terminal / editor** | `ui.md:88` | String scan 0 (`Palette`, `Terminal`, `[Ee]ditor`); no `Action` for either (`accelerators.rs:46-92`). |
| **Linux-native chrome: client-side decorations, controls at the right** | `ui.md:91`; mockup `Cairn — cairn — ▢ ✕` (`cairn-ui.html:243`) | No `with_decorations`/CSD code anywhere in either crate (grep `decorations` = 0); Freya's default `decorations: true` (`~/.cargo/git/checkouts/freya-*/caa46f8/crates/freya-winit/src/config.rs:148`) — the OS/winit frame, with Cairn's row drawn under it. |
| **A menu bar** | `cairn.md:119` ("Still open", `platform.md`) | Nothing; no `MenuBar` (only `Menu`/`MenuButton` for context menus). |

### B. Planned for a later packet or tier (`feature-inventory.md`; `roadmap.md` 5b-8 and "After D7")

| Surface | Where described | Absent — evidence |
| --- | --- | --- |
| Repository manager / recent repositories / multi-repo / open-repository picker / Init / Clone | `feature-inventory.md:26`, `:112`, `:121-123`; `cairn.md:108-111`; `roadmap.md:358` | String scan: `Clone` 0, `Repositor` 0, `\bOpen` 0. |
| File tree at a revision (detail pane's File Tree tab) | `feature-inventory.md:39`; mockup tab `Commit / Changes 8 / File Tree` (`cairn-ui.html:298`, `:594`) | `DetailTab::ALL = [Commit, Changes]` (`crates/cairn-ui/src/detail/tabs.rs:21`); "a File Tree tab is issue #31" (`tabs.rs:12`); the ⌘⌥3/Ctrl+Alt+3 chord is reserved and unbound (`accelerators.rs:19`, `:1450`). |
| Blame | `feature-inventory.md:40`; `roadmap.md:356` | String scan `[Bb]lame` 0; Local Changes menu omits Fork's Blame (`local_changes/menu.rs:1-3`; `docs/systems/local-changes.md:812-815`). |
| File / path history | `feature-inventory.md:41`; `roadmap.md:356` | No literal "History" but type names; menu omits Fork's History (`menu.rs:1-3`). |
| Commit search (`--grep`, author, pickaxe); Fork's separate search-results list | `feature-inventory.md:42`; Fork evidence `docs/research/history-graph/what-clients-show.md:184-197` | String scan `[Ss]earch` 0; no Find/Search `Action` (`accelerators.rs:46-92`); the only "Find" is the sidebar's ref find "Finding <ref>…" (`history/ref_find.rs:51`). Sidebar "search" listed not built (`sidebar.md:215`). |
| Compare a branch against its upstream (arbitrary revisions beyond two pressed commits) | `feature-inventory.md:43` | Only ⌘/Ctrl-press of two history rows (`history/selection.rs`); `sidebar.md:216` "⌘-click compare from the sidebar … not built". |
| Image diffs | `feature-inventory.md:47` | `diff/notice.rs:24-60` states: too large, binary, LFS pointer, submodule, mode/rename-only, conflicted — no image; grep `image` 0. |
| Repository summary / statistics | `feature-inventory.md:49` | Nothing. |
| Stash create / apply / pop / drop / Save Snapshot; "Stash N Files…"; Create Branch's "Stash and reapply" | `feature-inventory.md:65-66`; `roadmap.md:217-271`; `ui.md:81` | Stashes are read-only rows/chips; "Stash and reapply" drawn greyed with "Comes with stashing." (`dialogs/create_branch.rs:54-57`). |
| `.gitignore` editing (`Ignore ›` submenu) | `feature-inventory.md:64`; `roadmap.md:223-226` | String scan `Ignore` hits only the diff tooltip "Ignore whitespaces" (`diff/header.rs:33`). |
| Rename / delete branch, checkout / switch, set upstream, create / delete tag | `feature-inventory.md:72-78`; `roadmap.md:313-330` | String scan `Rename` 0 (only diff notices "Renamed…"), `Delete` 0, `\bTag` only "Tags" heading; `[Cc]heckout` only "Create and Checkout" in Create Branch. No double-click checkout (`sidebar.md:215`). |
| Merge, rebase, interactive rebase, cherry-pick, revert, reset, squash/fixup | `feature-inventory.md:85-102`; `roadmap.md:353-358` | Strings exist only as *operation-in-progress* lines/refusals (`commit_box.rs:126-132`, `dialogs/create_branch.rs:106-112`); string scan `\bReset` 0. A single cherry-pick/revert/merge started elsewhere can be concluded by Commit. |
| Pull, Push, force push, remote add/edit/remove, prune, push/delete tags, Commit and Push | `feature-inventory.md:109-115`; `roadmap.md:273-311`; `ui.md:328-329` | Only `Fetch <remote>`, first remote only (`window.rs:583-584`), and no Fetch chord (`accelerators.rs:46-92`). |
| Submodule init / update / sync; submodule status beyond a badge | `feature-inventory.md:46`, `:125` | Only `RefGlyph::Submodule` badge and the "Submodule" notice. |
| `git config` editing, Preferences, light theme, external diff/merge tool, update mechanism | `feature-inventory.md:127`, `:146-152`; `ui.md:417-418`, `:438`; `cairn.md:120` | String scan `Preferences|Settings` 0, `Theme|Light` 0; `use_init_theme(dark_theme)` fixed (`main.rs:61`); no persisted settings store (`roadmap.md:358-361`, issue #89). |
| Avatars | not described in ui.md/feature-inventory (Fork draws them) | String scan `Avatar|Gravatar` 0; the history row draws author text only (`history/commit_row.rs:1-3`). |
| Commit graph options (branch scoping / "Filter by active branch", column chooser, collapse) | Fork evidence `fork-refs-and-status-ui.md:49`, `:222` (toolbar button, ⌘⇧A/Ctrl+Shift+A) | History header is fixed: "Graph and subject", "Author", "Commit", "Date (UTC)" with fixed widths (`history/commit_row.rs:13-17`, `:294-317`); the one view option is Show Lost Commits. |

Fork surfaces the systems docs themselves list as not built (useful for the redesign, all
verified absent by the files cited):

- Sidebar: Pinned, Worktrees, Submodules, the tab strip of refs and search, double-click checkout,
  ⌘-click compare, keyboard navigation (`docs/systems/sidebar.md:215-216`). The sidebar's refs
  carry **no context menu** at all (no `MouseButton::Right`/`ContextMenu::open` in
  `crates/cairn-ui/src/sidebar.rs`).
- Local Changes: tree layout and layout menu, collapse-all, Hide Untracked Files, Show Ignored
  Files, the quick-look eye (`local-changes.md:758-764`); menu items Open, External Diff, Show in
  Files, Blame, History, Ignore, LFS, Stash, Save as Patch (`local-changes.md:812-815`,
  `menu.rs:1-3`); no keyboard way to open the menu.
- Commit box: limit setting, monospace toggle, Wrap paragraph at ruler, spell check, autocomplete,
  commit template, `prepare-commit-msg`, AI drafts, sign-off, Commit and Push, the ⌘⇧A amend chord
  (`local-changes.md:770-772`).
- History commit-row menu: Fork's checkout, merge, rebase, cherry-pick, revert, reset, tag, copy
  SHA etc. — Cairn's menu is the single "New Branch…" (`history/list.rs:16`, `:439-447`); a stash
  row has no menu (`list.rs:425-428`). Commit-tab and Changes-tab file rows have no menu.
- Diff bar: word wrap and invisibles (`docs/systems/diff.md`, "The diff view", its table row
  "The bar's buttons"); the five diff-bar toggles have **no chord** (`accelerators.rs:349-353`
  map them to `Chords::of([])`).

### C. Explicitly out of scope (`feature-inventory.md:194-203`; `cairn.md:34-43`)

Reviewing PRs / issues, CI status, creating or deleting platform repositories, being a text
editor, Git-flow, a tutorial, Windows, `git bisect` / `filter-branch`. Also out by design in
`ui.md`: an Accounts sidebar section (`ui.md:89`), a Clean command and Discard All (`ui.md:259`,
`:270-271`), a combined multi-file diff (`ui.md:167-168`), hunk-header buttons, a checkbox column
and a selection gutter (`ui.md:214`, `:222-224`), a backup on discard (`ui.md:272-273`), author
override / empty commits / up-front skip-hooks / sign-off controls in the commit box
(`ui.md:326-328`), a list of reflog entries (`ui.md:342`). All confirmed absent (none appear in
the string scan or the component tree).

---

## Where the code and `docs/design/ui.md` disagree

Legend: **AGREES** (only where notable), **DISAGREES** (doc vs code), **NOT BUILT**.

### Preamble and the layout model (`ui.md:1-54`)

| ui.md claim | Verdict | Code |
| --- | --- | --- |
| "Three regions, always", toolbar row on top (`ui.md:17-27`) | DISAGREES / NOT BUILT | No toolbar. Top row is the title bar: wordmark "Cairn" (`window.rs:553`), status box, "N commits…" (`window.rs:560-565`), `Fetch <remote>` (`window.rs:566`, `:576-602`). Local Changes replaces graph **and** detail pane (`window.rs:240-243`), as ui.md:23-25 allows. |
| Local Changes (N) first sidebar entry (`ui.md:31-32`) | AGREES | `sidebar.rs:60-66` (`Local Changes (N)`, absent at 0). |
| Graph is the main view on open (`ui.md:33`) | AGREES | `MainView::AllCommits` is `#[default]` (`sidebar.rs:42-47`). |
| Stashes inline and in the sidebar; an unreachable one sidebar-only (`ui.md:36-40`) | AGREES | `RowContent::Stash` rows (`window.rs:409-415`); `sidebar.md`. |
| Repository tabs (`ui.md:41-42`) | NOT BUILT | — |
| Detail pane under the graph, draggable splitter, Commit and Changes tabs (`ui.md:43-44`) | AGREES | `window.rs:323-361` (`PANE_HEIGHT` 260, min 90, collapse to a strip); `detail/tabs.rs:21`. |
| Dialog buttons in platform order (`ui.md:45-50`) | AGREES | `dialogs/button_order.rs`, `dialogs/answer_button.rs`. |
| "A right-click selects the row … Every list row's menu does this" (`ui.md:51-54`) | AGREES where menus exist; misleading as a whole | History rows (`history/list.rs:416-424`) and Local Changes rows (`local_changes/list.rs:722-747`) select on right-click. Sidebar rows, Commit-tab and Changes-tab file rows have no menu at all. |

### "What changes, and why" table (`ui.md:58-91`)

| Row | Verdict | Code |
| --- | --- | --- |
| One confirmation dialog, Cancel focused, one-sentence prompt, Show files, Title Case (`:60-62`) | AGREES | `dialogs/confirm.rs:44-57`, `:318`; prompts `cairn-model/src/consequence.rs:544-597`. |
| Mixed discard says in one line what is left (`:63`) | AGREES | `menu.rs:120-135`, confirm `left`. |
| One home per message; nothing under the lists (`:64`) | AGREES, one gap | Git Error / "Couldn't <name>" (`dialogs/git_error.rs:32-42`). "A refusal known in advance as a greyed control with its reason beside it": the Fetch button is **omitted**, not greyed, when there is no remote (`window.rs:582-583`). |
| No text a person needs is cut (`:65`, again `:154`) | DISAGREES | The title bar clips: status box in an `Overflow::Clip` flex cell (`window.rs:555-557`), every status-box label `max_lines(1)` (`status_box.rs:189-201`, running text `:143`), count `max_lines(1)` (`window.rs:563`); the opening notice's path ellipsised (`window.rs:615`); history chips cut at the column edge (`history/ref_chips.rs`, documented in CLAUDE.md as Fork's). |
| Activity popover quotes the prompt, has durations, "Show in Lost Commits", UTC times (`:66`) | AGREES | `title_bar/activity_popover.rs:30-42`, `:85-88`; `operations/activity.rs:651-660` ("HH:MM:SS UTC"). |
| Ignore Whitespace disabled in Local Changes (`:67`) | AGREES | `diff/header.rs:269` (`ignore_whitespace() && !self.exact`). |
| Visible double chevrons in both headers (`:68`) | AGREES | ⇊ / ⇈ (`local_changes/list.rs:24-27`). |
| Line selection survives an unchanged refresh; one notice when cleared (`:69`, `:226-230`) | AGREES | `diff/staging_gesture.rs:61` "The file changed — line selection cleared." |
| Floating actions stay at the list's top while the chunk top is scrolled above (`:70`) | AGREES | visible stretch clipped to the list (`staging_gesture.rs:601-605`); `local-changes.md:784-787`. |
| Press without drag / Escape lets a selection go; `Discard 2 Lines…` (`:71`) | AGREES | `staging_gesture.rs:103-111`. |
| Amend asks first only when published or unlogged; button "Amend Last Commit" (`:72`) | AGREES | `commit_box.rs:54-56`. |
| Cancel beside a running commit (`:73`) | AGREES | `commit_box.rs:51`, `:550-616`. |
| Skip pre-commit hooks always offered on a failed commit/amend (`:74`) | AGREES | `commit_box.rs:64`. |
| Merge message as git's editor leaves it (`:75`) | AGREES (engine) | `reads::as_the_editor_leaves`. |
| Leftover lock is a banner + confirmed removal (`:76`) | AGREES | `title_bar/lock_banner.rs:16-19` (stale after **10 s**, ui.md "a few seconds"); `operations/leftover_lock.rs:27` "Remove Stale Lock". |
| Nested repo / submodule never discarded, greyed with why (`:77-78`) | AGREES | `menu.rs:122-133`. |
| Commit box disabled during rebase / am / sequence, naming git's command (`:79`) | AGREES | `commit_box.rs:126-132`, tests `:680-690`. |
| Show Lost Commits check box at right of "Graph and subject", tooltip the chord (`:80`) | AGREES | `history/commit_row.rs:288-312`. |
| "Stash and reapply" greyed (`:81`) | AGREES | `dialogs/create_branch.rs:54-57`. |
| Show Lost Commits reaches HEAD's and local branches' reflogs (`:82`) | AGREES | `docs/systems/history-graph.md`, "Show Lost Commits". |
| Worktrees sidebar section, chip, disabled checkout (`:83`) | NOT BUILT | — |
| Pressing a ref finds it however deep (`:84`) | AGREES | `history/ref_find.rs`. |
| Generic remote glyph (`:85`) | AGREES | `RefGlyph::Remote` (`history/ref_glyphs.rs:30-52`). |
| Forge links; post-push toast (`:86`) | NOT BUILT | — |
| Three-way conflict resolution (`:87`) | NOT BUILT | — |
| Toolbar drops Appearance/Workspace/Feedback, adds terminal/editor/palette (`:88`) | NOT BUILT | — |
| No Accounts section (`:89`) | AGREES | trivially. |
| Loading state distinct from empty (`:90`) | AGREES | "Reading history…" vs "No commits yet." (`history/status_text.rs:6-13`); "Reading the working tree's status…" (`local_changes/pane.rs:64`). |
| Linux-native chrome: no traffic lights, **client-side decorations**, keyboard-first throughout (`:91`) | DISAGREES / NOT BUILT | OS decorations (Freya default), no CSD. Keyboard gaps: no chord for Fetch, for switching Local Changes / All Commits, for the five diff-bar toggles (`accelerators.rs:349-353`); sidebar has no keyboard navigation (`sidebar.md:216`); Local Changes' menu cannot be opened from the keyboard (`local-changes.md:815`). |

### The sidebar, the toolbar and Local Changes (`ui.md:93-131`)

| Claim | Verdict | Code |
| --- | --- | --- |
| Sidebar order: Local Changes, All Commits, filter, **Worktrees**, Branches, Remotes, Tags, Stashes, **Submodules** (`:95-98`) | DISAGREES / NOT BUILT | Four sections only (`sidebar_rows.rs:36`); filter placeholder "Filter" (`sidebar.rs:33`). |
| Folders at `/`, current branch ✓ and bold, counts or gone upstream; press selects and finds, never filters; every section virtualized (`:98-103`) | AGREES | `sidebar.rs:8-13`, `sidebar.md`. Note: one virtualized list for all sections, not "every section a virtualized list" — same effect. |
| Title bar names repo, `*`, branch, behind/ahead "as Fork's central status box" (`:105-107`) | AGREES | `status_box.rs:1-7`, `:183-203`. |
| "While an operation runs, the status box shows it **instead**" (`:107-109`) | DISAGREES | Running lines are **appended after** name, branch and counts in the same row (`status_box.rs:183-215`, `.children(self.running…)` after `.maybe_child(tracked)`); spinner + name + " · N waiting" (`status_box.rs:49-61`, `:258-268`) agrees. |
| A fetch adds git's progress line; Fetch greyed while one runs; box press opens popover (`:109-110`) | AGREES | `window.rs:506-536`, `:587-588`, `:209`. Caption is `Fetch <remote>` ("Fetch origin"), first remote only (`window.rs:583-584`) — ui.md does not say. |
| Local Changes layout: filter over Unstaged above Staged, headers with Stage/Unstage + chevron, diff right, commit box under it (`:112-117`) | AGREES | `local_changes/list.rs:1-27`, `local-changes.md:18-62`. |
| Refresh on focus, after operations, on the chord; nothing watches the filesystem (`:120-126`) | AGREES | `refresh/focus.rs`; F5 / ⌘R (`accelerators.rs:299-302`). |
| Two banners under the title bar; closing text (`:128-131`) | AGREES | `window.rs:236-237`; `operations/local_writes.rs:127-134`. |

### Messages (`ui.md:133-157`)

| Claim | Verdict | Code |
| --- | --- | --- |
| Operation names, e.g. "Stage 2 files" (`:136`) | DISAGREES (casing) | `Stage 2 Files` / `Unstage 1 File`, title case on purpose ("Fork's Stage and Unstage say "File" in title case, its Commit "file" in lower case", `worker/local/lane.rs:275-295`); "Commit 1 file" (`lane.rs:314`); "Create branch 'topic'" (`lane.rs:317-318`) agrees. |
| Git Error: title, sentence, `Error Details:`, command and output scrolled to end (`:141-143`) | AGREES | `dialogs/git_error.rs:32-35`. |
| "Couldn't <name>", one sentence + "Nothing was changed.", no Error Details (`:144-147`) | AGREES | `git_error.rs:40-42`; `confirm.rs:54`. |
| Known refusal is a greyed control with reason under (menu) / beside (button) (`:148-150`) | AGREES for menu/Amend/Stash-and-reapply; Fetch omitted rather than greyed (above). |
| Keys inert while a dialog or the popover is open (`:156-157`) | AGREES | `shortcuts::keys_inert`, `modal.rs`. |

### Staging — Files (`ui.md:161-210`)

| Claim | Verdict | Code |
| --- | --- | --- |
| One selection, primary, ⌘/Ctrl-click, Shift-click, Shift+↑/↓ (`:163-165`) | AGREES | `local_changes/selection.rs`, `accelerators.rs:291-294`. |
| Five routes; stage chord Return / ⌘S (mac), Enter / Ctrl+S / Ctrl+Shift+S (Linux); Return in the diff does nothing (`:172-177`) | AGREES | `accelerators.rs:306-313`, `:357-362`. Code also has Stage/Unstage All on ⌥⇧⌘S / Ctrl+Alt+Shift+S (`accelerators.rs:319-320`) — not in ui.md. |
| Menu: Stage or Unstage, Discard Changes…, Stage All, Copy Path (`:179`) | AGREES | `menu.rs:215-219` (Stage All **or Unstage All** by list). |
| Shift (Linux) / ⌥ (mac) turns header buttons into Stage All / Unstage All (`:180-183`) | AGREES | `accelerators.rs:319-327`. |
| Discard keys ⌫ / ⇧⌘D; Backspace, Delete, Ctrl+Shift+D (`:188-189`) | AGREES | `accelerators.rs:330-340`. |
| Selection rule after action/refresh (`:191-196`) | AGREES | `local_changes/next_selection.rs`. |
| Conflicted row staged whole, no gesture, no discard (`:207-210`) | AGREES | `menu.rs:5-7`, `diff/notice.rs:50-51`. |

### Staging gestures, Discard, the commit box (`ui.md:212-330`)

| Claim | Verdict | Code |
| --- | --- | --- |
| Hover outlines a chunk; `Stage` / `Discard Changes…` / `Unstage`; `Stage 2 Lines`; no hunk-header actions (`:214-224`) | AGREES | `staging_gesture.rs:1-4`, `:53-58`, `:103-120`. Code also has mode-change captions ("Discard Mode Change…", `:120`) — ui.md says only "a row of its own with its own actions". |
| Discard dialog: "Counting…" with greyed "Discard Changes"; prompt "Discard all changes in 31 files? 2 untracked files will be deleted. You can't undo this."; button `Discard Changes in 31 Files`; lines "Discard 2 changed lines in src/main.rs?" under `Discard 2 Lines` (`:244-251`) | AGREES, plus unlisted forms | `consequence.rs:554-568`, `:356-368`, `:583-594`. Code also renders "Discard this chunk (6 lines) in src/main.rs?" for a hovered chunk's discard and "Discard the mode change of run.sh?" (`consequence.rs:583-590`; also in `local-changes.md:788-792`) — ui.md does not mention the chunk form. |
| Show files ▸/▾, per-row "14 lines", "Binary", "Restored", "Deleted", "Emptied", "Mode change" (`:253-255`) | AGREES | `confirm.rs:318`; `consequence.rs:266-303` (also "Binary and mode change"). |
| Refusal texts for staged / submodule / conflicted / nested (`:260-268`) | AGREES verbatim | `menu.rs:122-133`. |
| Commit box: counter grey to 50, negative, red past 70; Recent Commit Messages at subject's right; ruler at 72; Amend left; `Commit N Files` right; ⌘Return / Ctrl+Enter (`:277-283`) | AGREES, one detail | `commit_box.rs:43-73`, `:86-88`, `accelerators.rs:342`. With 0 files the button reads plain "Commit" (`commit_box.rs:86`), and "Commit 1 File" in the button vs "Commit 1 file" as the operation name (`lane.rs:314`). Recent messages open from a `≡` glyph (`commit_box.rs:1-3`). |
| Merge line "Merging. Committing concludes the merge; Amend is unavailable." (`:296`); detached line (`:300-301`) | AGREES verbatim | `commit_box.rs:126`, `:62`. Unborn line "No commit yet, so Amend is unavailable." (`:60`) is code-only wording. |
| Amend confirm "Amend Commit" with the published / no-reflog sentences (`:315-317`) | AGREES | `commit_box.rs:56`; `consequence.rs` `amend_prompt`. |
| Amend staged-list fallback says lists show what is staged against HEAD (`:308-311`) | AGREES | `commit_box/pane.rs:127`. |
| "Committing 0:03" with Cancel; Git Error with Skip beside Close (`:320-326`) | AGREES | `commit_box.rs:587-616`, `:64`. |

### Show Lost Commits, Create Branch, Activity, leftover lock (`ui.md:332-401`)

| Claim | Verdict | Code |
| --- | --- | --- |
| ⌘⇧. / Ctrl+Shift+.; check box at the right of "Graph and subject"; dimmed rows (`:334-341`) | AGREES | `accelerators.rs:345`; `commit_row.rs:288-312`. |
| `New Branch…` on any commit row; Ctrl+Shift+B / ⇧⌘B at HEAD; subtitle "Use '/' as a path separator to create folders"; name refused beside the buttons; "Waiting for <name>"; "Check out after create" remembered; "Local changes:" Don't change / Stash and reapply / Discard ⚠; button "Create and Checkout" (`:348-357`) | AGREES | `history/list.rs:16`; `accelerators.rs:348`; `dialogs/create_branch.rs:38-65`, `:88-112`; `operations/create_branch.rs:309` ("Waiting for {name}…"). Button reads "Create" when the box is unticked (`create_branch.rs:61`). "Remembered" is for the session only (`roadmap.md:358-361`). |
| Activity: rows name, result line, start time, ⚠; four words; duration; prompt; "Show in Lost Commits"; bounded (`:373-385`) | AGREES | `activity_popover.rs:1-42`, `:85-88`. Code adds an × cancel on a running row (`:34`) and "Nothing has run in this window yet." (`:36`) — not in ui.md. |
| Lock banner text verbatim; retry ~1 s once or twice; Git Error carries `Remove index.lock…`; "Remove Stale Lock" (`:389-400`) | AGREES | `lock_banner.rs:16-17`; `worker/local/lane.rs:1566-1568` (2 retries, 1 s); `git_error.rs:38`; `leftover_lock.rs:27`. |

### Palette and type (`ui.md:403-418`)

| Claim | Verdict | Code |
| --- | --- | --- |
| Dark-first via Freya's `dark_theme` (`:405`) | AGREES | `main.rs:61`. |
| "Cool graphite neutrals with a slight blue bias" (`:405-407`) | DISAGREES | Stock Freya ground `rgb(20, 20, 20)`, neutral (`diff/palette.rs:19-22`); no custom theme tokens anywhere. |
| One desaturated steel-blue accent for HEAD, selection and the primary action (`:407-409`) | DISAGREES | Accent is the theme's `text_highlight` `rgb(96, 145, 224)` (`palette.rs:48-53`), used for the current change, active diff toggles and the detail tab underline (`detail/tabs.rs:113`). Selection is the theme's grey `surface_secondary` (`history/commit_row.rs:109`, `:189`; `sidebar.rs:473`; `local_changes/list.rs:711-719`). HEAD is marked by a **bold subject**, not colour (`commit_row.rs:1-3`). Primary buttons are Freya's default `Button`. |
| Lane colours "a six-step set" (`:409-411`) | DISAGREES | **Eight**: Okabe-Ito's seven non-black plus a neutral grey (`history/lane_palette.rs:6-15`, pinned by `the_palette_is_the_okabe_ito_set_less_black_plus_a_neutral`); `docs/systems/history-graph.md:746-749` agrees with the code, not ui.md. Tags are a fixed indigo `rgb(98, 92, 196)` (`ref_chips.rs:50-51`) — not in ui.md. |
| Type is IBM Plex — Sans for the interface, Mono for ids, paths, diffs (`:413-414`) | DISAGREES (half) | Only IBM Plex Mono is embedded (`main.rs:37-40`; `assets/fonts/` holds `IBMPlexMono-Regular.ttf` + licence only). The interface is Freya's default font; no `font_family` for Sans anywhere. Mono is used for diffs, ids, paths in the Changes list, the status box counts, Git Error output, popover lines, the description field (`palette.rs:56` and its users). Local Changes' paths: `local_changes/list.rs` imports `DIFF_FONT_FAMILY`. |
| `tabular-nums` wherever digits align (`:415`) | NOT BUILT | grep `tabular` 0; digit alignment relies on Mono's fixed advance. |
| Light theme with its own diff palette (`:417-418`, `:438`) | NOT BUILT | `use_init_theme(dark_theme)` only. |

### The screens (`ui.md:420-434`) and the mockup

| Screen | Verdict |
| --- | --- |
| 1 History (`:422-424`) | BUILT, but differs from the mockup (below). |
| 2 Local changes (`:425-427`) | BUILT; ui.md already disowns the mockup's gesture. |
| 3 Discard "over the history view" (`:428-430`) | DISAGREES (minor) — discard is reachable only from Local Changes (its lists, menu, chords and gesture), so the dialog is drawn over Local Changes, never over the history. |
| 4 Conflict (`:431`) | NOT BUILT. |
| 5 Worktrees and forge links (`:432-434`) | NOT BUILT. |

**`docs/design/mockups/cairn-ui.html`** mocks five boards — History (`:239-326`), Local changes
(`:327-407`), Discard (`:408-468`), Conflict (`:469-553`), Worktrees and forge links (`:554-623`).
`ui.md:3-9` already disowns its fixed-height pane, tab count, `+N` file counts, hunk-header buttons,
checkbox column, selection gutter, transparent washes, operation-log drawer and stash-first discard.
Major differences from the code **not** covered by that disclaimer:

- Every board draws the **toolbar** (Fetch `2` / Pull / Push / Stash; repo · branch · path; Branch /
  Worktree / Terminal / Palette, `:244-258`), **repository tabs** (`:259`), and **client-side window
  controls** "▢ ✕" (`:243`). None built.
- Sidebar: a **Worktrees** section with paths and "this window" and "+ Add worktree…" (`:575-581`),
  `worktree` chips on branches, "Tags: none yet", a count on "Stashes 1", filter "Filter refs" (`:264`). Code:
  four sections, no counts on section headers, placeholder "Filter".
- History columns: mockup "Description / Author / Commit / Date" with relative/short dates ("18:20",
  "Sep 14"); code "Graph and subject / Author / Commit / Date (UTC)" with `YYYY-MM-DD HH:MM`
  (`commit_row.rs:13-17`, `:294-317`) and the Show Lost Commits check box the mockup lacks.
- Loading: mockup "Loading older commits — 9 of ~9 shown" under the list; code puts "N commits…" in
  the title bar (`status_text.rs:17-27`).
- Detail pane: mockup has a **File Tree** tab and a Refs row in a two-column grid; code has Commit /
  Changes only, with a REFS row of chips.
- Local changes: mockup groups paths by **directory tree** with `Stage all` / `Unstage all` text
  buttons and "2 files" counts; code is flat lists with `Stage`/`Unstage` buttons and ⇊/⇈ glyphs and
  no per-list count (count is in the sidebar and "Showing N of M files"). Mockup diff bar has text
  buttons "Unified / Side by side / Ignore whitespace" and a `+9 −3` stat; code has Fork's glyph
  buttons (prev/next, ⎵, −, +, ↕, split) and no stat. Mockup commit box has **Sign-off** and "pre-commit
  hook will run", subject counter "46 / 72"; code has neither and Fork's characters-left counter.
- Discard: mockup's "What is lost / How much / Recoverable" three-row prompt and "2 files selected"
  bar; code is the one-sentence prompt + Show files.

### Docs in `docs/systems/` that conflict with `ui.md`

- **Lane palette**: `history-graph.md:746-749` (eight hues) vs `ui.md:409` (six-step). Code = systems.
- **Discard chunk prompt**: `local-changes.md:788-792` names "Discard this chunk (6 lines) in
  src/main.rs?" — ui.md:249-251 gives only the "changed lines" form for lines from the diff.
- **Sidebar sections**: `sidebar.md:215` says Worktrees and Submodules are not built; ui.md:97-98
  describes them as the sidebar's order (design intent, but no pointer says so).
- **Status box while running**: `history-graph.md:843-866` ("The title bar") and `status_box.rs:8-12`
  describe progress shown in the box; neither says "instead", which is ui.md:107's word.
- No conflicts found between ui.md and `diff.md` "The diff view" (`docs/systems/diff.md:1804-2157`);
  it documents what ui.md omits: the bar's glyph buttons, "Ignore whitespaces" tooltip
  (`diff/header.rs:33`), Fork-measured 11 px Mono / 17 px rows, the accent as theme `text_highlight`.

### Drawn by the code, not mentioned in `ui.md`

- The wordmark "Cairn" at the left of the title bar (`window.rs:553`) and the "N commits…" count
  (`window.rs:560-565`).
- `Fetch <remote>` as the one top-level action, first remote only (`window.rs:576-602`).
- History column headings and the `Date (UTC)` label (`commit_row.rs:294-317`).
- Tag chips in fixed indigo (`ref_chips.rs:50-51`); lane-tinted chips with painted glyphs
  (`ref_glyphs.rs`).
- Detail pane's Collapse / Expand control (`detail/tabs.rs:31-34`) and ⌘⌥1/2 tab chords
  (`accelerators.rs:295-296`).
- "Showing N of M files" / "Filtering…" under the filters (`detail/changes_list.rs:26-40`).
- Stage All / Unstage All chord ⌥⇧⌘S / Ctrl+Alt+Shift+S (`accelerators.rs:319-320`).
- Unborn-branch line "No commit yet, so Amend is unavailable." (`commit_box.rs:60`); "Cancelling…"
  (`:58`); "Waiting for <name>…" in the commit button's place (`commit_box/pane.rs:89`).
- Activity row × cancel and the empty "Nothing has run in this window yet." (`activity_popover.rs:34-36`).
- Diff notices "Changes are too large to display" + Load Diff, "Binary file", "Git LFS pointer",
  "Submodule", "Renamed without changes", "Mode changed", "No changes to show." (`diff/notice.rs:24-60`).

## The structural cost of changing layout

### Who assembles the window

- **`crates/cairn-app/src/main.rs`** launches the one window: `WindowConfig::new(..).with_title("Cairn")` (`main.rs:46-52`) with no size, minimum size or decorations set, so the toolkit's defaults apply; the close hook is `closing.requested()` (`main.rs:52`). `LaunchConfig::with_font(DIFF_FONT_FAMILY, DIFF_FONT)` registers IBM Plex Mono, the only embedded font (`main.rs:39`, `main.rs:55`). `app` calls `use_init_theme(dark_theme)` (`main.rs:61`): Freya's built-in dark theme is the whole window's theme, and there is no light theme or system-theme following. `app` owns every `use_state` the `View` handles point at (`main.rs:63-…`) and hands them to `window::window`.
- **`crates/cairn-app/src/window.rs`** is the window's tree, production code lines 1-657 and tests from `window.rs:658` to the end (4,766 lines, about 58 tests drawing the window from view states). `window()` (`window.rs:123-272`) builds, top to bottom: the global key handlers (`window.rs:199-209`); `title_bar` (`window.rs:538-569`, the "Cairn" word, the status box, the loaded count, `Fetch <remote>`); the lock banner and the closing banner (`window.rs:236-237`); `beside` (`window.rs:300-319`, a horizontal `ResizableContainer` with the sidebar panel at `SIDEBAR_MIN_WIDTH` and the main panel at `MAIN_MIN_WIDTH` 240 px); the main region, a `match` on `MainView` (`window.rs:240-243`): `AllCommits` gives `split` (`window.rs:323-360`, a vertical `ResizableContainer` with the list at least `LIST_MIN_HEIGHT` 80 px over the detail pane at `PANE_HEIGHT` 260 px, collapsing below `PANE_MIN_HEIGHT` 90 px to a `DETAIL_STRIP_HEIGHT` strip), and `LocalChanges` gives `LocalChangesPane`; the one modal child chosen by `modal::drawn` (`window.rs:252-268`); and `ContextMenuViewer` (`window.rs:270`), the host every context menu needs.
- The **panes inside the main region are assembled in `cairn-app`, not `cairn-ui`**: `crates/cairn-app/src/sidebar/pane.rs`, `detail/pane.rs` and `detail/changes_tab.rs` (the Changes tab's file list beside its diff, a `ResizableContainer` at `changes_tab.rs:188`), `local_changes/pane.rs` (lists beside the diff and the commit box, a `ResizableContainer` at `local_changes/pane.rs:257`) and `commit_box/pane.rs`. `cairn-ui` supplies the components each composes (and one splitter of its own, Unstaged over Staged, `crates/cairn-ui/src/local_changes/list.rs:310`). So moving a panel means editing `window.rs` and one or more `cairn-app` pane files, and the components' props, which are `cairn-ui`'s.
- **Session state is the `View` struct** (`window.rs:44-110`): the pane's height and collapse, the detail tab, the sidebar width (`view.sidebar.width`), the Changes list's share (`changes_list_width`), the diff settings. None is persisted beyond the session; there is no settings store (issue #89 is the named future one, `FILESYSTEM_MUTATION_EXCEPTIONS` in the guards).

### How layout constants are spread

- **No central theme or layout module.** Each component file declares its own `const`s (about 30 files in `crates/cairn-ui/src` and `crates/cairn-app/src` declare a size or colour constant; the most in `diff/palette.rs` 15, `title_bar/activity_popover.rs` 9, `history/ref_chips.rs` 9, `diff/row_parts.rs` 9, `detail/commit_tab.rs` 8, `window.rs` 8). Named palettes exist only for the diff (`crates/cairn-ui/src/diff/palette.rs`, re-exported as `cairn_ui::diff_palette`) and the graph's lanes (`crates/cairn-ui/src/history/lane_palette.rs`).
- **Inline literals are common**: outside `worker/` and tests, about 60 `font_size(<number>)` calls and about 48 `padding(<number>)`/`Gaps::new(<number>…)` calls carry their value in place (e.g. `window.rs:552-553`, `window.rs:564`, `window.rs:610-616`, `window.rs:628-633`).
- **Everything else is Freya's theme**: about 45 reads of `get_theme_or_default()`, `theme_color()` or `theme_background()` (for example `text_placeholder`, `surface_tertiary`, `text_secondary` and `error` at `window.rs:565`, `window.rs:629`, `window.rs:635`, `window.rs:653`). A Freya component used unstyled (`Button`, `ResizableContainer`, `Popup`, `Menu`) takes its look from `dark_theme`.
- **Cross-crate constants** the app reads from `cairn-ui` (`crates/cairn-ui/src/lib.rs`): `ROW_HEIGHT`, `LANE_WIDTH`, `MAX_DRAWN_LANES`, `graph_width` (`lib.rs:87`), `DETAIL_STRIP_HEIGHT`, `END_ROOM`, `SCROLLBAR_THICKNESS` (`lib.rs:82`), `NUMBER_FONT_SIZE`, `NUMBER_PADDING`, `TEXT_PADDING` (`lib.rs:68`), `GLYPH_SIZE` (`lib.rs:93`), `DRAG_THRESHOLD` (`lib.rs:95`), `PREFETCH_ROWS`, `NEW_BRANCH_CAPTION` (`lib.rs:88`), `LOCK_BANNER_TEXT`, `LOCK_STALE_AFTER` (`lib.rs:116`). `window_check.rs` and the app's tests read several of these, so a renamed or regrouped constant reaches tests in both crates.

### The tests a layout change moves

- **Headless component tests**: 23 files under `crates/cairn-ui/tests/` (about 11,300 lines), each rendering a component with `freya-testing` and many asserting its text and the rows built. The virtualization twins are among them (see below).
- **Window tests**: the tests module of `crates/cairn-app/src/window.rs` (`window.rs:658` on) draws the whole window from view states; the app's other `tests.rs` files (`local_changes/tests.rs`, `commit_box/tests.rs`, `operations/tests.rs`, `operations/create_branch/tests.rs`, `sidebar/tests.rs`, `local_changes/actions/tests/*`) drive panes through the window.
- **`crates/cairn-app/src/window_check.rs`**: `#[ignore]`d measurements of the real window over the bench repository, whose figures in `docs/research/` would need re-measuring after a layout change that alters what a frame builds.

### Guards and invariants a layout redesign brushes against

| Invariant (CLAUDE.md) | Twin (`crates/cairn-guards/tests/invariants.rs`) | What a redesign must keep |
| --- | --- | --- |
| No unbounded list renders without virtualization | `a_history_sized_list_renders_through_a_virtualizing_view` (`invariants.rs:5044`); `UNBOUNDED_VIEW_EXCEPTIONS` is empty (`invariants.rs:5118`) | No render file in `cairn-ui/src` or `cairn-app/src` may name `ScrollView`. Today ten files use `VirtualScrollView` (`end_room.rs`, `sidebar.rs`, `detail/changes_list.rs`, `detail/commit_tab.rs`, `dialogs/confirm.rs`, `dialogs/git_error.rs`, `diff/view.rs`, `local_changes/list.rs`, `title_bar/activity_popover.rs`, `history/list.rs`). Any new scrolling panel, even a bounded one such as a Preferences page or a long dialog body, must be virtualized or added to the exceptions roster with a reason (a reviewed change). The behavioural twins (`only_a_viewport_of_rows_is_built_however_long_the_history` and the per-list siblings in `crates/cairn-ui/tests/`) pin each list's viewport count. |
| No component names a literal modifier | `no_component_names_a_literal_modifier` (`invariants.rs:5180`), `CHORD_NAMES` (`invariants.rs:5166`) | A toolbar button's or menu item's tooltip naming its shortcut must come from `accelerators::chord_name` (`crates/cairn-ui/src/accelerators/chord_names.rs`), never a typed "Ctrl+…" or "⌘". A menu bar listing chords would need the same route. |
| The accelerator table holds data and resolution only | `the_accelerator_table_holds_data_and_resolution_only` (`invariants.rs:5516`), roster `ELEMENT_BUILDERS` (`crates/cairn-guards/src/lib.rs:867`) | A menu bar or command palette must not be built inside `accelerators.rs`; new actions are rows there, views elsewhere. The chord rule (`chords_are_distinct_and_every_bare_one_is_a_function_key_or_local_changes_own`) refuses a bare letter or a chord shared by two actions. |
| Every text field takes the one key policy | `every_text_field_takes_the_shared_key_policy` (`invariants.rs:5849`), roster `TEXT_FIELD_IDENTS` (`crates/cairn-guards/src/lib.rs:902`), exceptions `TEXT_FIELD_EXCEPTIONS` (`invariants.rs:5846`) | Any new search box (history search, sidebar or toolbar), Preferences field or dialog field must be built with `cairn_ui::text_field`/`text_field_in`; naming `Input` or `on_pre_key_down` anywhere else fails. |
| A `RowContent`, `DiffContent`, `UnifiedRow`, `SideBySideRow` is read by naming every variant | `every_view_of_a_row_names_every_kind_of_row` (`invariants.rs:394`) and its diff siblings | A new column or row layout that matches these enums must match exhaustively (see `window.rs:405-420`). |
| The UI thread never waits on repository work | `the_ui_thread_never_waits_on_repository_work` (`invariants.rs:4976`) | A new pane in `cairn-app` outside `worker/` may not name `cairn_git`, a channel, a mutex or a waiting call; new data must arrive as a `Request`/`Update` through the worker. |
| `cairn-ui` never names `gix` or `cairn_git`, never touches the filesystem | `layers_never_name_the_crates_they_are_sealed_from`; `only_the_ops_module_changes_the_filesystem` | A repository picker, recent-repositories list or Preferences store cannot read or write files from a component; it needs engine calls or a filesystem exceptions row (issue #89). |
| Every embedded font is a user decision, beside its licence | `the_embedded_fonts_are_the_roster_each_with_its_licence`, `EMBEDDED_FONTS` (`invariants.rs:9655`, IBM Plex Mono Regular alone) | A UI typeface other than the system default, or a bold/italic Plex weight, is a new roster row and a user decision. |
| Only a confirmation surface builds `Confirmed` | `destructive_operations_are_sealed_behind_the_confirmation_token`, `CONFIRMATION_SURFACES` (`dialogs/confirm.rs`, `dialogs/create_branch.rs`) | Moving a destructive action into a toolbar or menu must still open one of those dialogs; a new dialog that builds a token is a roster row. |
| Comments describe the code, not its history | `comments_describe_the_code_not_its_history` (`invariants.rs:9554`) | A redesign's comments may cite Fork's behaviour, never "per the UI review packet". |
