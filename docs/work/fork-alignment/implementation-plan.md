# Implementation plan — fork-alignment

The technical plan behind `docs/prd/fork-alignment.md`. It also holds the packet's one copy of
the review-dispatch rules.

## Branches and modes

- **Integration branch:** `feature/fork-alignment`, in its own worktree. Phase 01 creates it from
  `main`, once `history-topo-order` (roadmap 5c) has merged.
- **User mode, the default:** each phase works on a runtime-owned phase branch cut from
  integration, and raises a templated PR back into integration. It never merges.
- **Packet mode:** only under `/orchestrate-packet`. There, phases commit straight onto
  integration and raise no per-phase PR.
- Nothing reaches `main` until phase 09 passes. Every PR is the user's to merge.

## Architecture

The seam does not move. `cairn-ui` gains Freya features and `jiff`; `cairn-app` gains `jiff` and
Freya's `titlebar` and `borderless` features. The lockfile gains exactly two crates, both from the pinned Freya source: `freya-icons` (phase 01) and `freya-borderless-plugin` (phase 02), approved in brainstorm L10. No `muda`. Every new fact a view draws is a value
from the engine or the worker. The new facts:

| Fact | From | To |
| --- | --- | --- |
| `HEAD`'s reach per row | the walk (`cairn-git/src/history/`, beside `reach.rs`) | a field of the row's `cairn-model` vocabulary |
| The time zone | `TimeZone::system()`, read once on a worker in `cairn-app` | the window state, passed as a prop |
| The repository's absolute path | `crates/cairn-app/src/repository_path.rs`, which already holds it | Copy Full Path and Copy Repository Path |

The style module, `cairn_ui::style` (`crates/cairn-ui/src/style.rs` beside `style/`):
- `metrics.rs`: `pub const`s grouped by surface;
- `tones.rs`: a `Tones` value of named colours, and the theme built from Freya's `dark_theme`;
- `icons.rs`: one function per icon, returning the element.

Components read sizes and tones from these modules. The launch theme in `crates/cairn-app/src/main.rs`
comes from `style::tones`. The diff palette (`crates/cairn-ui/src/diff/palette.rs`) and the lane
palette (`history/lane_palette.rs`) stay where they are. They read the tones Fork Dark gives
(bands, hunk text) from `style::tones`.

The menu component, `cairn_ui::menu`:
- A menu is data: `MenuItem { label, chord: Option<Action>, enabled, reason, kind }`, in groups.
- One component draws it: Freya's `Menu`, `SubMenu` and `MenuButton` (FA §1-2), plus a separator
  row and a right-aligned chord column filled from `accelerators::chord_name`.
- The menu bar, the toolbar's dropdowns and every context menu are built from it.
- Rule of three: it has more than three users from phase 02 on.

The caption row (Linux) lives in `crates/cairn-app/src/window.rs`'s shell, beside the
`title_bar` parts that move into the toolbar:
- the window config built with `with_decorations(false)`;
- `BorderlessPlugin` for the resize edges;
- `window_drag` on the row's empty space;
- `TitlebarButton` ×3.

On macOS, the native frame stays and the same menu data is drawn as a row at the top of the
window, under `cfg(target_os = "macos")`; no native menu bar (no `muda`, L10). The guard rosters
must allow a `cfg` file split if one is needed.

## Guards added (each in the phase that adds its invariant, with a matcher self-test)

| Guard | Holds | Phase |
| --- | --- | --- |
| `chrome_sizes_and_tones_come_from_the_style_module` | No production file of `crates/cairn-ui/src` or `crates/cairn-app/src`, outside `style/` and an exceptions roster (`STYLE_EXCEPTIONS`, each row a file and its reason, failing when no longer needed), passes a numeric literal to `font_size`, nor builds a colour from a literal (`Color::from_rgb`, `from_argb`, `from_hex`, a `(u8, u8, u8)` tuple into a colour, `"#…"` strings). Self-test: `the_style_literal_matcher_catches_the_shapes_it_claims`. The first rows are expected to be `diff/palette.rs` and `history/lane_palette.rs`. | 01 |
| `every_icon_is_named_through_the_icon_module` | Only `crates/cairn-ui/src/style/icons.rs` names `freya::icons`, `lucide` or `SvgViewer`; it must name `lucide`. | 01 |
| `deny.toml` `[[bans.features]]` for `freya` | `exact`: today's set plus `icons-lucide` (01), `titlebar` and `borderless` (02); a new feature becomes a decision. A `[licenses]` note records Lucide's ISC licence for `freya-icons`. | 01, 02 |
| `the_render_layer_is_handed_its_time_zone` | No file of `crates/cairn-ui/src` names `TimeZone::system`, `try_system`, `tz::db` or `TZ`. Only `cairn-app/src/worker/` reads the zone. | 04 |
| The allowlist rows for `jiff` | `layer_dependencies_are_allowlisted` gains `jiff` for `cairn-ui` and `cairn-app` alone, and `deny.toml` pins its features to what gix-date already enables, so the lockfile is unchanged. | 04 |

Each guard addition is an invariant. Its phase adds it to the root `CLAUDE.md` with its twin in
the same commit.

## Per-phase technical notes

- **01.**
  - The tones table is the PRD's R1.3, written once.
  - `a_selection_is_blue_while_its_list_has_focus_and_grey_when_not` needs each list's focused
    state. A list learns it from Freya's focus (`use_focus` / `a11y` focus, FA §8). Read the
    vendored source before choosing.
  - Re-tone the existing views through `style` without moving anything: this phase changes how
    the window looks, not its layout. Layout changes belong to the later phases.
- **02.**
  - Check the vendored `BorderlessPlugin` and `window_drag` on Wayland (Hyprland, the dev host) and
    X11.
  - Under a tiling compositor the controls still draw and do what the compositor allows; record
    the behaviour in `docs/systems/`.
  - The menu bar's items are R2.5's.
  - Zoom uses Freya's runtime scale factor, never the `zoom-shortcuts` feature, which hard-codes
    Ctrl.
  - About Cairn is a `Popup` under the one modal state (`modal.rs`).
- **03.**
  - The toolbar moves the status box (`cairn-ui/src/title_bar/status_box.rs`) and Fetch out of
    `window.rs`'s title row. `title_bar` becomes `toolbar`, keeping the activity popover and the
    lock banner as its children or siblings, and the guard paths named in the root `CLAUDE.md`
    move with them. Rename in one commit, with the root `CLAUDE.md` and `docs/systems/` updated.
  - The splitter between the history and the detail pane becomes a proportion.
  - The minimum size: measure the widths in the real window, then set it in the window config and
    record it.
  - Removing Ctrl+Shift+D from Discard on Linux, for ToggleDetails, changes the root `CLAUDE.md`'s
    accelerator paragraph and the table's pin examples in the same commit.
- **04.**
  - The reach mark is propagated in emission order: `HEAD` is reached; when a reached row is
    emitted, its parents are marked. This relies on 5c's order, so check it on tied times.
  - The stash lane changes `LaneAssigner::push_stash`, in `cairn-model` (a test in the same
    commit). The frozen compact-rows oracle (memory: it is frozen; C15 of history-graph compares
    against a verbatim pre-compaction assigner) must stay byte-identical. If the stash change
    alters what the oracle compares, stop and ask.
  - Dates: `history/date_text.rs` takes the zone. Read jiff's API in the vendored 0.2.37 source;
    never write it from memory.
  - Show Lost Commits: the header check box goes with the header, and its tooltip's `chord_name`
    use moves to the View item.
- **05.**
  - The sidebar is laid out by `RefsSnapshot::sidebar_rows` (`cairn-model`). Indentation and leaf
    names may be view-only, or a model change; a model change needs a test in the same commit.
  - Tag order, newest first, is the snapshot's: by the tagger date, falling back to the commit
    date, as Fork's order showed. Verify that on the fixture, with tags whose dates are not their
    commits'.
- **06.**
  - COMMITTER is shown only where name, email or time differ. Compare the values the model
    already holds.
  - The in-place diff's row height comes from `style::metrics`.
- **07.**
  - The bar over both lists moves Stage All's chevron, and the Unstaged heading loses its own.
  - Every staging route's tests must still pass unchanged, or change only where the PRD changes
    the route.
  - The split Commit button is one component, with its ⌄ greyed.
- **08.**
  - `Copy` and `CopyFull` are two actions, each interpreted by the focused view's menu data, so
    the menu's chord column and the key are one source.
  - Focus rings: one `style` treatment applied by each focusable control.

## Review dispatch (the packet's one copy)

Every phase runs `/qa` over its diff. Reviewers are spawned fresh, never the implementer.
Adjudication goes to `qa-confirm`. Dismissals are logged in `progress.md` with their reasons.

| Phase | Reviewers, beyond `qa-checklist` |
| --- | --- |
| 01 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (new guards, `deny.toml`) |
| 02 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (`deny.toml`, any roster change) |
| 03 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (the accelerator pin's examples; a guard path moving with `title_bar`) |
| 04 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (allowlist, the zone guard) |
| 05 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 06 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 07 | `responsiveness-reviewer`, `test-coverage-auditor`; `destructive-ops-reviewer` if a discard route's wiring changes |
| 08 | `responsiveness-reviewer`, `test-coverage-auditor`, `gate-integrity-reviewer` (the accelerator pin), `destructive-ops-reviewer` (the confirmation and Create Branch dialogs are `CONFIRMATION_SURFACES`) |
| 09 | the whole set, over the whole packet diff |

## Risks

- **Client-side decorations on Wayland:** resize edges, the window menu, double-press-to-maximize
  and focus behaviour vary by compositor. Phase 02 tests on Hyprland and records the rest as
  unverified. If a compositor makes the caption row unusable, stop and ask. The user chose this
  over the OS frame, so the answer may be a per-compositor fallback, and that is a decision.
- **Freya's 16 px default:** components that take no font size (menus, tooltips) need their size
  set where they are built. A missed one shows only in the real window: qa-checklist item 5.
- **Selection focus:** Freya's focus model must tell which list holds focus without a render
  effect. If it cannot, phase 01 stops and asks rather than inventing a focus tracker.
- **The compact-rows oracle (phase 04):** see above.
