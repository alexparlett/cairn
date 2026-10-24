# Implementation plan — repository-tabs

The technical plan behind `docs/prd/repository-tabs.md`. It also holds the packet's one copy of the
review-dispatch rules. File and symbol names are as they stand at `feature/fork-alignment`
(b539d9d); WS and WG (`docs/research/repository-tabs/`) map every one of them.

## Branches and modes

- **Integration branch:** `feature/repository-tabs`, in its own worktree. Phase 01 creates it from
  `main` once `fork-alignment` (PR #154) has merged; until then the packet does not start.
- **User mode, the default:** each phase works on a runtime-owned phase branch cut from
  integration, and raises a templated PR back into integration. It never merges.
- **Packet mode:** only under `/orchestrate-packet`. There, phases commit straight onto
  integration and raise no per-phase PR.
- Phase 01 needs the machine with the Windows VM (`~/VMs/fork-win10/`) for its observation; the
  later phases run anywhere the gate runs.
- Nothing reaches `main` until phase 07 passes. Every PR is the user's to merge.

## Architecture

The seam does not move: `cairn-ui` draws values, `cairn-git` is untouched (every engine piece is
already per repository, WG §1-2), and `cairn-app` is where the work is.

**The two states.** `crates/cairn-app/src/window.rs`'s `View` splits into:
- `WindowView` — the window's fields of the PRD's R2.1, built once in `app()`'s root scope;
- `TabView` — one repository's fields, built in that tab's holder's scope.

Every function that takes `view: View` today (WS summary 19: about 170) takes the half it reads,
or both. The six literal builders (WS summary 18) become one constructor per half plus one test
builder each, so the about 209 tests that build a view change in one place.

**The tab holder** (`crates/cairn-app/src/tabs/holder.rs`, the module `tabs/` beside `tabs.rs`):
- a component, one keyed child per open tab under the window's root, drawing nothing and
  registering no event handler;
- it owns the tab's `TabView` (its `State`s made in its scope, so they drop when it unmounts,
  FR §2), the tab's update task (`spawn` in its scope, cancelled on unmount, FR §3) and its
  `Replier` (held in the hook, never a struct field);
- it calls `worker::open`, the only file that does (the new guard below);
- it publishes its tab to the window's list of tabs (`WindowView::tabs`: id, name, mark, and the
  `TabView` handles) and withdraws it before it unmounts, so no handle outlives its scope.

The active tab's view is drawn by the window from that list, keyed by the tab's id, so switching
remounts the view's components and every `use_side_effect`, `use_hook` and `PartialEq` that
captured the old tab's `submit` starts on the new one (WS summary 15-17). The view's own local
state — `split_width`, `RowFinder`, `use_drop`'s `StopCounting` — either moves into `TabView` or
is allowed to reset; a counting confirmation is modal, so no switch can interrupt it (R9.2).

**A tab's id** is a `TabId`, a process-wide counter. **A prompt's id** becomes unique in the
application the same way `OperationId::next` is (a static atomic), so `PromptId` needs no tab
beside it (R9.1).

**Closing.** `closing.rs`'s `Closing` holds one stage per tab instead of one: a tab's close submits
`Request::Close` carrying the tab's large answers (or retires them just before), and the holder
moves to a "closing" list, out of the strip, until its stream ends. The window's close submits
every tab's close and closes the window when every stream has ended, or on the second request
after `CLOSE_PATIENCE`. `LocalWrites::closing_line` names the repository. Nothing the UI thread
calls waits (`RepositoryHandle::submit` is as today).

**Startup.** `Update::Started` stops carrying the zone and the `git` description; both are read
once per application, the zone still in `worker/zone.rs` (the zone guard's `ZONE_READER`), on a
thread `main` starts beside `Discovery::start`.

**The picker** (`crates/cairn-app/src/worker/folder_picker.rs`): `rfd::AsyncFileDialog::pick_folder`,
spawned on the UI executor, rfd itself running the portal call on its own thread (FR §5); the
answer arrives as a value. A seam (a trait or a function value) lets the window's tests answer it.

**The store** (`crates/cairn-app/src/worker/tab_store.rs`): the format types with `serde` derives,
read synchronously in `main` before the window launches (capped at 1 MiB), written by one
application thread fed by an unbounded channel whose newest request wins. The write is
`create_dir_all` with mode `0700`, a write of the temporary file and `rename`; the exact kinds go
in its `FILESYSTEM_MUTATION_EXCEPTIONS` row.

**The strip** (`crates/cairn-ui/src/tab_strip.rs`, beside `tab_strip/`): built from `rect`s and
labels, as the detail pane's tab strip is, with `DragZone`/`DropZone` for reordering (FR §1), every
size and tone from `cairn_ui::style`; its overflow list built with `cairn_ui::menu`. A tab's middle
press uses `on_all_press`/`on_pointer_down`, since `on_press` is the left button alone (FR §7).
The names rule is a pure function (where the names live — `cairn-model` or `cairn-ui` — is the
phase's call; a `cairn-model` change needs its test in the same commit).

**The Repository Manager tab** (`crates/cairn-ui/src/repository_manager.rs`): a virtualized Recent
list and the selected row's name, path and Open, drawn from values; it reads nothing itself.

## Guards added or changed (each in the phase that adds its invariant, with a matcher self-test)

| Guard | Holds | Phase |
| --- | --- | --- |
| `only_the_tab_holder_opens_a_repository` | No production file of `crates/cairn-app/src` but `tabs/holder.rs` names `worker::open` or imports `open` from `worker` (aliases included); that file must. Self-test: `the_repository_open_matcher_catches_the_shapes_it_claims`. | 02 |
| The secret holders' containers | The credential guard's container scan follows the alias `Replier` too (WG §6.1), so no struct field keeps one. | 02 |
| `FILESYSTEM_MUTATION_EXCEPTIONS` | A row for `worker/tab_store.rs`, pinned to exactly its kinds of write, its reason that it writes Cairn's own state outside every repository. | 06 |
| `layer_dependencies_are_allowlisted` and `deny.toml` | `rfd` (04), `serde` and `toml` (06) in `cairn-app`'s row, each with an `exact` `[[bans.features]]` pin. | 04, 06 |
| Path rosters that move | `CONFIRMED_HOLDERS`, `WINDOW_TOKEN_FIELDS` and `PROCESS_CALL_EXCEPTIONS`' text, if `commit_box/state.rs`'s `GitError` or `history::state::Progress` move with the split; the root `CLAUDE.md`'s UI-thread residuals and `.claude/agents/responsiveness-reviewer.md`, which name `main.rs`'s `use_hook` wiring and `Closing::*` (WG §6.1). | 01, 02, 04 |

Each guard addition is an invariant. Its phase writes it into the root `CLAUDE.md` with its twin, in
the same commit.

## Per-phase technical notes

- **01.**
  - The observation first, with the questions R1.1 lists; the amendments to the [observe]
    requirements are put to the user before the phase ends, and recorded.
  - The split is mechanical but wide. Move fields; change no behaviour. One tab is still opened
    by `main.rs`, as today. Expect `window.rs`'s 5,700 lines (mostly tests) to change mostly in
    their builders.
  - Read the vendored Freya source for `State`'s lifetime before deciding where `WindowView`'s
    handles are made: the root scope, as today.
- **02.**
  - Prove the holder's lifetime in a headless test before building on it: a keyed holder unmounted
    drops its `State`s and cancels its task (FR §2-3 verify the mechanism, not this use).
  - `main.rs` stops opening the repository; the holder does. `repository_path::chosen` returns
    every argument.
  - Refresh: `refresh::focus::on_focus_gained` is installed once by the window and asks every tab
    — the shown one `Request::Refresh`, the rest `Request::RefreshStatus`. Activation submits
    `Request::Refresh`.
  - The window checks (`#[ignore]`d, R13) over N tabs need a worker-side helper that waits on
    several `Updates` (`worker/window_check_updates.rs` has one for one stream; render-side files
    may not wait).
- **03.**
  - The strip's metrics and tones join `style/metrics.rs` and `style/tones.rs` (Fork Dark: strip
    #101010, active tab #2C2C2D, FT §1), plus whatever the observation measured.
  - Overflow: the minimum tab width is a metric; the list button appears only when the tabs at
    their minimum do not fit.
  - The six chords join the accelerator table with their pin's rule examples kept; Ctrl+Tab must
    reach the window from every text field (`accelerators::field_key`).
- **04.**
  - `Request::Close`'s doc ("What closing the window asks for") and `WORKER_LOST`'s text ("Close
    Cairn and start it again") change with it; the root `CLAUDE.md`, `docs/systems/git-processes.md`
    ("Closing") and `credentials.md` ("Closing the window closes the repository first") are
    rewritten in the same change.
  - The `Retire`-after-`Close` hole (WG §3.2): either the close carries the tab's answers, or the
    tab retires them first and closes second; a test proves nothing is dropped on the UI thread.
  - `rfd`'s features: pin exactly what `freya-winit` enables (`xdg-portal`, `wayland` and the
    defaults); check `cargo deny` accepts the pin while `freya-winit` enables the same.
  - The File and Window menus are `menu_bar.rs`'s data; the Manager tab's reduced menu bar and
    greyed toolbar read the active tab's kind.
- **05.**
  - The prompt's title names the tab; the window's one `prompt` becomes a queue of prompts, each
    with its tab, answered through that tab's `Replier`. `session.rs`'s refusal of a prompt unless
    this tab's fetch or write is in flight stays per tab.
  - A background tab's `endings` wait; the mark reads "has an ending waiting".
- **06.**
  - The store's types are versioned; parsing is lenient about unknown keys and strict about the
    version.
  - Writes are triggered by the window's list of tabs changing (order, active, open, close) and by
    Recent changing, never per frame.
  - `worker/tab_store.rs` reads `$XDG_STATE_HOME` and `$HOME` itself: the store is not a `git`
    process, so `GitEnvironment`'s roster has nothing to do with it.

## Review dispatch (the packet's one copy)

Every phase runs `/qa` over its diff. Reviewers are spawned fresh, never the implementer.
Adjudication goes to `qa-confirm`. Dismissals are logged in `progress.md` with their reasons.

| Phase | Reviewers, beyond `qa-checklist` |
| --- | --- |
| 01 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (any roster path moved), `destructive-ops-reviewer` (the confirmation and Git Error state move into `TabView`) |
| 02 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (the new guard, the secret guard's alias, the UI-thread residual text) |
| 03 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (the accelerator pin) |
| 04 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (`deny.toml`, the allowlist), `destructive-ops-reviewer` (a tab closed while its write runs; the close's waits) |
| 05 | `responsiveness-reviewer`, `test-coverage-auditor`, `destructive-ops-reviewer` (a prompt's answer reaching only its own `git`) |
| 06 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (the filesystem exception row, the allowlist, `deny.toml`), `destructive-ops-reviewer` (a filesystem write outside `ops/`) |
| 07 | the whole set, over the whole packet diff |

## Risks

- **The split's width.** About 170 functions and 209 tests touch the state. Phase 01 may need to
  stop part-way and split itself; that is a scope call for the user, not a reason to change
  behaviour in the same phase.
- **A handle outliving its holder.** A `TabView` handle read after its holder unmounted panics.
  The window's list must drop a tab's entry before the holder goes, and a test closes a tab while
  its view is drawn.
- **Freya's keyed remount.** Switching remounts the tab's view; anything a component kept in its
  own hooks resets. If one of them cannot reset without losing what R3.3 keeps, it moves into
  `TabView`.
- **Ten `git status`es on one focus (R5.1).** On a slow disk they contend; R13.4 measures it. If the
  bar fails, the fallback (background statuses one after another) is a user decision.
- **`rfd`'s zenity fallback** starts a process the process guards do not see; recorded as a review
  point in `docs/systems/`, and the PRD's dependency rows.
