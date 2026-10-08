# The staging surface as built

Date: 2026-10-07
Commissioned by: staging-and-commit planning

**Method.** Read-only audit of the checkout at `36c0d5c` (branch
`feature/plan-staging-and-commit`, equal to `origin/main`): the root `CLAUDE.md`; the
as-built docs `docs/systems/diff.md`, `status.md`, `local-changes.md`, `sidebar.md`,
`refs.md`, `history-graph.md` ("Refresh", "From every ref, labelled, with stash rows"),
`git-processes.md` ("The network lane"); the design docs `docs/design/diff.md`, `ui.md`,
`feature-inventory.md`, `concurrency.md`, and the mockup `docs/design/mockups/cairn-ui.html`;
the roadmap brief (`docs/work/daily-loop/roadmap.md`, section 5); the evidence in
`docs/research/diff-engine/fork-detail-and-diff-ui.md` (Findings 13, 15, 23) and
`docs/research/process-manager/consumer-invocations.md` (rows P1-P13); and the code those
docs name, read directly — `cairn-model` (`line_selection.rs`, `patch.rs`, `patch_apply.rs`,
`diff_shown.rs`, `diff_rows.rs`, `local_changes.rs`, `status.rs`, `confirm.rs`,
`command_log.rs`), `cairn-git` (`diff/working_tree.rs`, `reads/working_tree.rs`,
`ops/mod.rs`, `refs/stash.rs`, `process/`), `cairn-ui` (`local_changes.rs`, `diff_view.rs`,
`unified_rows.rs`, `side_by_side_rows.rs`, `diff_row_parts.rs`, `accelerators.rs`,
`diff_header.rs`), `cairn-app` (`local_changes_pane.rs`, `diff_actions.rs`,
`diff_state/working.rs`, `shortcuts.rs`, `session.rs`, `worker/`), and the tests named below
(`crates/cairn-model/tests/diff_patch.rs`, `crates/cairn-git/tests/diff/patches.rs` and
`scratch.rs`, `crates/cairn-git/tests/diff/working_tree.rs`, `crates/cairn-ui/tests/local_changes.rs`,
`crates/cairn-app/src/local_changes_tests.rs`). The vendored Freya at the linked rev
(`~/.cargo/git/checkouts/freya-*/caa46f8`) was read for pointer events and text input. No
test was run and no file but this one was written. Claims about git's own behaviour that
were not read from Cairn's tests are marked **(unverified)**.

---

## 1. The diff model: what a selection can say and what a patch carries

### 1.1 `Selection` (`crates/cairn-model/src/line_selection.rs`)

- Two `BTreeSet<LineNumber>`s: `removed` (old-side numbers) and `added` (new-side numbers).
  Nothing about hunks, context, view or overlay (R1.3). A removed line and an added line of
  the same number are distinct selections
  (`a_removed_line_and_an_added_line_of_the_same_number_are_two_selections`).
- Constructors and edits: `empty()`, `with_every_change(&TextDiff)` (whole file),
  `select_change(&ChangedRange)` ("what staging a hunk selects" — note: one *change*, a run
  of removals and the additions replacing them, not a drawn hunk), `select_removed`,
  `select_added`, `unselect_removed`, `unselect_added`. Queries: `holds_removed`,
  `holds_added` (O(log n), cheap per row), `holds_every_change` (a pass over every changed
  line — documented as off-UI-thread or kept-and-updated), `len`, `is_empty`, iterators.
- **What it can express:** any subset of changed lines, on both sides independently — a whole
  file, a whole change, a whole hunk (by `select_change` over each `hunk.changes`), arbitrary
  scattered lines, removals only, additions only. It cannot express a context line (selecting
  one is meaningless and ignored by the emitter), and it carries no notion of *which diff* it
  was made against (no blob id, no side) — the caller must keep that pairing.
- **Map from a drawn hunk to a selection.** `Hunks::of`/`of_ranges` (`diff_hunks.rs`) give
  each `Hunk` a `changes: Range` of indexes into the ranges grouped. For `DrawnRanges::Exact`
  those are indexes into `TextDiff::changes()`, so "stage this drawn hunk" is
  `select_change` over them. Under ignore-whitespace the layout groups the overlay's ranges
  (`UnifiedLayout::shown` → `DisplayOverlay::changes_to_draw`), and those indexes are into
  `DisplayOverlay::changes_ignoring_whitespace()`, **not** into the exact changes — there is
  no mapping from a whitespace-ignoring change to exact lines today (see section 9).
- **Row → change lookup.** `ChangeStops` (`diff_rows.rs`) maps a drawn change to its rows
  (`rows(change)`, `start`, `first_from(row)`, `last_before(row)`, binary searches); nothing
  maps a row to its hunk directly (derivable from `Hunks` and the row index, not built).

### 1.2 `emit_patch(file, text, selection) -> Patch` (`crates/cairn-model/src/patch.rs`)

Bytes for `git apply --cached` against the OLD side. Rules as built and pinned by unit tests
in the same file:

| Shape | What it emits | Pinned by |
| --- | --- | --- |
| Context | Always `PATCH_CONTEXT` = 3, whatever the view; no argument can change it (R1.6). Hunks grouped by `Hunks::of(text, 3)` (git's merge rule, no `interHunkContext`). | `a_patch_carries_three_lines_of_context_and_takes_no_say_in_it`, `a_patch_is_the_same_bytes_whatever_the_view_was_built_at` |
| Unselected lines | **Forward rule only:** an unselected removal becomes context (old line), an unselected addition is dropped. | `an_unselected_removal_becomes_context_and_an_unselected_addition_is_dropped` |
| Counts / starts | Recounted from emitted lines; new start = old start + net of earlier emitted hunks; a hunk with nothing selected is omitted. | `a_hunk_with_nothing_selected_is_left_out_and_the_next_hunks_new_side_shifts` |
| Header spelling | Empty range names the line before; count of one omitted (`@@ -1 +1 @@`, `-0,0`). | `a_header_spells_what_git_spells` |
| No newline at EOF | `\ No newline at end of file` follows the line it belongs to, whichever marker it ended up with. | `a_line_that_never_ended_carries_its_marker_on_whichever_side_it_is_on`, `the_marker_follows_a_removal_that_was_turned_into_context` |
| New file | `new file mode`, `--- /dev/null`; stays an addition however little is selected. | `an_addition_with_a_line_left_out_is_still_a_new_file` |
| Deleted file | `deleted file mode` + `+++ /dev/null` only when every removal is selected; otherwise a modification. | `a_deletion_with_a_removal_left_out_is_a_modification` |
| Mode change | `old mode`/`new mode` headers; a mode change alone is headers with no hunks (callers pass an empty `TextDiff` and `Selection::empty()`). | `a_rename_and_a_mode_change_are_headers_with_no_hunks` |
| Rename / copy | `similarity index`, `rename from/to` or `copy from/to`; `---`/`+++` name each side. | `a_rename_with_an_edit_names_the_old_side_and_the_new_side_apart` |
| `index` line | Written for a whole selection when the blob moved (abbreviated ids, mode when unchanged); omitted for a partial selection. | `the_index_line_follows_gits_own_rule`, `a_partial_selection_claims_no_blob_id` |
| Type change | Two sections at one path (delete old kind, add new kind); all or nothing — a partial selection emits nothing. | `a_type_change_is_written_as_a_deletion_and_an_addition`, `a_type_change_that_is_not_wholly_selected_emits_nothing` |
| Binary, too large, LFS pointer, submodule, conflicted, unsupported | **No patch at all** — `DiffContent` holds no `TextDiff`. The C1 harness stages these "directly" (`update-index --cacheinfo`). | `whole_patch` in `crates/cairn-git/tests/diff/patches.rs` |
| Paths | Written as raw `RepoPath` bytes in `diff --git a/.. b/..`, `---`, `+++`, `rename from` lines. **No C-quoting.** | `a_path_goes_out_as_the_bytes_git_stores` (a `0xff` byte only; never through real `git apply`) |
| Whitespace overlay | Unreachable by construction: `emit_patch` takes `&TextDiff`, which has no room for the `-w` ranges (`compile_fail` doctest in `diff_text.rs`). | R1.7 |

**The emitter has no caller in production** (`docs/systems/diff.md`, top: "the patch emitter
still ships with no caller, deliberately"). `apply_patch` / `apply_patch_in_reverse`
(`patch_apply.rs`) are a pure test oracle, public only because the round-trip tests live in
another crate; the doc says a later phase "may put them behind `#[doc(hidden)]` or a
test-support feature once there is a caller to gate against".

### 1.3 `ShownDiff` (`crates/cairn-model/src/diff_shown.rs`)

The window's prepared form of one `FileDiff`, built once on the diff thread
(`worker/diff_lane.rs`): the `FileDiff`, the `Context` asked at, both `UnifiedLayout` and
`SideBySideLayout` (built by `::shown`, i.e. over the overlay's ranges when ignore-whitespace
is on), `hides_changes`, widest drawn columns, cut-line flag, gutter digits. `diff()` returns
the `FileDiff` whose `text()` is the exact `TextDiff` — so a staging gesture on the window
side has the exact diff in hand to build a `Selection` and (on a worker) a patch, without a
second query. It carries no record of the blob ids the answer was read against beyond what
`ChangedFile` holds (`old_id`, `new_id`; for the unstaged side `new_id` is the id of git's
form of the working-tree content, `diff/working_tree.rs` `named`).

### 1.4 Round trips through real git — what is covered and what is not

Two layers:

**Model-only** (`crates/cairn-model/tests/diff_patch.rs`, corpus in `tests/diffs/mod.rs`):
`a_patch_gives_what_the_selection_means_over_every_awkward_case` (seeded selections over
missing-final-newline on each side and both, first/last-line hunks, merging and non-merging
hunks, empty and one-line files, CRLF, added, deleted, rename-with-edits, mode change — each
with and without ids), judged against `apply_patch` **and** `diffs::expected_result` (the
selection rule stated with no hunks); `selecting_every_change_reproduces_the_new_version`;
`the_same_patch_backwards_undoes_exactly_what_it_did`; the negative
`a_patch_for_one_selection_does_not_give_what_another_one_means`; and C4,
`a_selection_and_its_patch_are_the_same_in_every_view` (six contexts, both views — **built
with `UnifiedRows::new`/`SideBySideRows::new`, the exact projections, never `::shown` over a
whitespace overlay**).

**Real git** (`crates/cairn-git/tests/diff/patches.rs`; `Scratch` in `tests/diff/scratch.rs`
runs `git apply --cached --whitespace=nowarn [--reverse]`, `--check` first, with
`GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES` pointed at a
scratch dir, so the repository is only read):

- **C1** — every file of a commit, every line selected, applied to the parent tree; the
  `write-tree` must equal the commit's tree: `every_crafted_commit_round_trips_through_real_git_apply`,
  `every_rewrite_commit_round_trips_through_real_git_apply` (renames, copies),
  `every_commit_of_this_repository_round_trips_through_real_git_apply`.
- **C2** — `a_seeded_selection_stages_what_its_patch_says_it_does`: twelve selections per file
  (every, none, additions only, removals only, first and last line of every change, eight
  seeded), staged blob compared with `apply_patch`, plus mode, source-path survival and
  whether anything is staged; required shapes: added, deleted, modified, rename, mode change,
  two hunks, CRLF context, removal only, one line, one line without newline.
  `the_last_line_of_a_file_with_no_newline_stages_on_its_own` covers the EOF edge by hand.
- **C3** — the **whole** patches reversed (`--reverse`) onto the commit tree must give the
  parent's; copies undone by hand (`gits_own_copy_patch_cannot_be_reversed_either` shows
  git's own copy patch refuses to reverse too).

**Not covered by any test today:**

1. **Partial selections applied in reverse** (unstage lines, discard lines). C3 reverses
   whole-file patches only, and the model test reverses a partial patch only onto its own
   forward result. Applied in reverse to the diff's *new* side (the index for unstage, the
   working tree for discard), a partial forward-rule patch is the wrong shape: an unselected
   added line exists in that preimage and must be *context*, and an unselected removed line
   does not and must be *dropped* — the mirror of the forward rule (git-gui's rule for
   reverse line application **(unverified here)**). `emit_patch` has no reverse mode. With
   the forward rule, `git apply -R` would see context lines that are not in its preimage and
   either refuse or, with a hunk ending in a selected line (no trailing context), be forced
   to match at end of file **(unverified)**. This is the single largest model gap for the
   packet.
2. **Working-tree diffs as the patch source.** Every C1-C3 patch comes from a commit's
   content query. None is built from `Repository::working_tree_diff` (staged, unstaged,
   untracked) and applied to a real index, nor to a real working tree (`git apply` without
   `--cached`, the discard path). So the following are unexercised through real git:
   a filtered path (clean filter; the unstaged side is git's form, which is correct for an
   index apply and is the question for a working-tree apply), `text=auto`/`core.autocrlf`
   CRLF checkouts, an untracked file's partial stage (the `--no-index` answer is
   `ChangeStatus::Added` with `old_path == new_path`, so `emit_patch` writes a new-file patch
   — plausible, untested), an intent-to-add entry's unstaged side (a new-file patch against
   an index that already has the i-t-a entry; git learned to accept that around 2.29
   **(unverified)**, inside the 2.30 floor), and an unborn branch (`a_staged_file_on_an_unborn_branch_...`
   is a read test only).
3. **Awkward paths through real `git apply`.** Crafted fixtures use plain names. A path with
   a space, tab, newline, double quote, backslash or a non-UTF-8 byte has never been put
   through `git apply`; git quotes such paths (`core.quotePath` rules) in `diff --git`,
   `---`/`+++` and `rename from` lines and its parser relies on that quoting **(unverified for
   each character)**. `RepoPath` is the bytes `-z` gave, so status and the diff hold them
   correctly; the emitter writes them raw.
4. **The user's `apply.*` configuration.** `Scratch` forces `--whitespace=nowarn`. A user's
   `apply.whitespace=fix` or `error` (which the user's own `git add -p` honours **(unverified)**)
   would change or refuse a stage. Fork found it had been passing `--whitespace=nowarn` where
   `--ignore-whitespace` was wanted (Finding 23, TrackerWin #202).
5. **Renames in the working tree.** The working-tree query runs `diff-index --cached
   --no-renames`, so a **staged rename's** diff is the new path's *addition* alone; the old
   path's deletion is not in the answer. A line-level unstage of a renamed file built from it
   touches only the new path (Fork has the same class of bug: "Partially unstaging a renamed
   file also unstages the rename", Tracker #1872). Whole-file unstage of a rename must name
   both paths.
6. **Stale selections.** `git apply` matches context, with offset search **(unverified:
   default fuzz 0, offsets allowed)**; a selection made against an answer that has since gone
   stale fails `--check` in most cases, but repetitive content can let a hunk land at an
   offset. Nothing checks that the index entry still holds the diff's `old_id` before an
   apply (both are available: `ChangedFile::old_id`, and gix can read the index entry).
7. **Binary, submodule, LFS pointer, too-large, mode-only staging.** No patch exists for the
   first four; whole-file stage/unstage must be `git add` / `git restore --staged` (rows P1,
   P2). Mode-only is a headers-only patch (covered by C1).
8. **Under ignore-whitespace.** No test builds a selection from the `-w` layout. See section 9.

---

## 2. The status snapshot (`crates/cairn-model/src/status.rs`, `crates/cairn-git/src/reads/status.rs`)

**What runs:** `git --git-dir=<dir> --work-tree=<top> status --porcelain=v2 -z`, as a read
(`GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`), and a second `--untracked-files=all` read
only when the first collapsed an untracked directory (nested repositories always collapse,
#78). No `--ignored`, no rename flags, no `--ignore-submodules` (`docs/systems/status.md`).

**What it holds per path** — `WorkingTreeStatus::Listed(Vec<StatusEntry>)` in git's order, or
`IndexUnreadable(Sparse)` (git < 2.32 over a sparse index), or `NoWorkingTree`:

- `StatusEntry::Changed(ChangedEntry { path, staged: Option<StagedChange>, unstaged:
  Option<UnstagedChange>, submodule: Option<SubmoduleState> })`.
  `StagedChange`: `Added | Modified | Deleted | TypeChanged | Renamed { from, similarity } |
  Copied { from, similarity }`. `UnstagedChange`: `Modified | Deleted | TypeChanged |
  IntentToAdd | Renamed { from, similarity } | Copied { from, similarity }` — the last two
  are i-t-a pairs, and **`from` is listed nowhere else** (a tracked path gone from the
  working tree that has no row of its own). `SubmoduleState { new_commits,
  modified_content, untracked_content }`.
- `StatusEntry::Conflicted(ConflictedEntry { path, kind: ConflictKind, submodule })` — one
  of git's seven, from stages (`ConflictKind::from_stages`, `code()`).
- `StatusEntry::Untracked(RepoPath)` — one per file; a nested repository as `dir/`.
- **Not held:** modes, object ids (porcelain v2's `mH mI mW hH hI` fields are parsed and
  validated, then dropped), ignored paths (never listed; #62), the branch header, the stash
  count. A write that wants to verify "the index still holds what I diffed" must ask the
  diff's `ChangedFile` or read the index.

**Layout for the view:** `cairn_model::LocalChanges::new(status)` (`local_changes.rs`), run on
the refresh thread: two `Vec<u32>` index lists — Staged (has a staged change) and Unstaged
(has an unstaged change, or a submodule state alone, or conflicted, or untracked) — each in
`path_order` (Fork's natural order, case ignored, digits as numbers, bytes to break ties);
`paths()` counts distinct paths; `get(list, row) -> LocalChange { path, from, kind:
ChangeKind, state: PathState }`; `row_of(list, path)` by binary search; `matching(text, ..)`
for the filter, on a worker (`Request::FilterLocalChanges`, lane `LocalChangesFilter`).
`ChangeKind`: `Modified | Added | Deleted | Renamed | Copied | TypeChanged | Submodule |
Conflicted`; `PathState`: `Tracked | Untracked | Conflicted`.

**Freshness and refresh triggers** (`docs/systems/history-graph.md`, "Refresh"):
`Request::Refresh` reads refs (repository thread), ahead/behind, and status (refresh
thread). It is submitted on exactly three occasions: window focus gained
(`crates/cairn-app/src/refresh.rs`, `gaining_focus_asks_for_a_refresh_and_nothing_else`),
**any fetch ending** (`session::apply`, `crates/cairn-app/src/session.rs` ~l.256;
`every_fetch_ending_asks_for_a_refresh_and_touches_no_row`), and the Refresh action (F5 /
⌘R, `shortcuts::act`). Nothing watches the file system (R10.2, #65). **A status is never
superseded** (R10.3 as amended, the user's decision of 2026-10-07): a running `git status`
finishes and its answer is drawn; every refresh asked meanwhile becomes **one** follow-up
status (`serve_refreshes`). Only a close cancels one. The working-tree diff answer is never
cached (`worker/diff_answers.rs`, `a_working_tree_answer_is_never_kept`); the engine reads the
index fresh for each working-tree query (`fresh_index`).

**How a write would trigger a refresh today:** there is no local write yet. The pattern is
fetch's: the operation returns `cairn_git::ops::Performed { description, acknowledged,
invalidated: Invalidated { refs, index, objects, working_tree } }` (`crates/cairn-git/src/ops/mod.rs`),
the worker honours the flags, and the window submits `Request::Refresh` when the operation's
outcome arrives. Two consequences of the as-built refresh for a stage click:

- If a status started *before* the write is still running, its (pre-write, or torn) answer is
  drawn first and the post-write status only after it — the row can bounce back into
  Unstaged for one status's duration (736 ms on a stat-dirty rust-lang/rust). The follow
  rule then re-asks the chosen path's diff, which reads the post-write index, so lists and
  diff can disagree for that interval.
- `Invalidated::index` is documented as "re-run any query that read it"; the diff thread's
  `StagedInputs` stamp compares the index by value and drops kept answers only when its
  attribute entries changed (`diff_freshness.rs`), which is correct because working-tree
  answers are never kept. The gix shared index snapshot is re-read on mtime change — a
  write and a read in the same timestamp tick is the documented residual.

---

## 3. Local Changes as built (`docs/systems/local-changes.md`)

**Code:** `crates/cairn-ui/src/local_changes.rs` (`LocalChangesList`, `ListSection`,
`change_badge`, `change_text`), `crates/cairn-app/src/local_changes_pane.rs`
(`LocalChangesPane`, `follow`, `follow_the_lists`, `diff_side`, `diff_body`),
`crates/cairn-app/src/local_changes_state.rs` (the kept lists, filter, `serial`),
`crates/cairn-app/src/diff_state/working.rs` (`WorkingChoice`, `WorkingShown`,
`choose_working`, `refresh_working`, `working_arrived`), `crates/cairn-app/src/diff_actions.rs`
(`choose_working`, `working_query`, `step`), mounted from `window::window` when
`MainView::LocalChanges`.

**Layout.** Left (draggable, `LocalChangesView::list_width`): one filter `Input` (the only
control above the lists — the user's decision 2026-10-07), then a vertical
`ResizableContainer` of **Unstaged** above **Staged** (`LocalChangesView::lists_split`), each
a heading (`LIST_HEADER_HEIGHT` 26 px, caption only — **no Stage all / Unstage all button**)
and a flat `VirtualScrollView` of `DETAIL_ROW_HEIGHT` rows. A row: badge (`M`, `+`, `D`, `R`,
`C` letters; painted `RefGlyph::Submodule` and `RefGlyph::Gone` for submodule and conflict;
colour reinforces shape) and the path, `old → new` for renames/copies, in IBM Plex Mono.
Right: `DiffHeader` (previous/next change, ignore whitespace, fewer/more lines, entire file,
side by side — `HeaderAction`) over the diff, or a notice (`DiffNotice::Conflicted`,
"Reading the diff…", a failure, `NothingShown`, Load Diff past the limits). No commit box,
no Stage/Discard affordance anywhere ("Fork's Stage and Unstage buttons on the headings, and
the commit box under the diff, are not built (R9.6)").

**Selection model.** **One** path at a time: `WorkingChoice { list: ChangeList, path:
RepoPath, lists: u64 }` (the `serial` of the lists it was chosen in). The list component takes
`chosen: Option<(ChangeList, usize)>` and reports `on_choose((list, row))`. No multi-select,
no range select, no select-all. A path in both lists is two rows and two separate choices,
each asking its own side (`a_path_in_both_lists_keeps_only_the_answer_for_the_list_chosen`).

**Which diff is asked** (`diff_actions::working_query`): `Untracked` → `WorkingSide::Untracked`;
tracked in Staged → `Staged`; tracked in Unstaged → `Unstaged`; conflicted → nothing. At the
Changes tab's options (shared context, whitespace, entire file). It shares the file-diff lane
with the Changes tab and the Commit tab's opened files (`working_needs_asking`, `reask_working`).

**"The follow through each refresh"** (`local_changes_pane::follow`, a pure decision: keep,
ask again, choose the first, let go, nothing): when a newer `serial` is drawn, the chosen
path is looked up by list and path; still listed → asked again, last diff drawn meanwhile
(`DiffState::refresh_working`); gone → **the first path shown** (Unstaged's first row, else
Staged's) is chosen; no path → let go. A filter-hidden path stays chosen. This runs only while
the view is shown. Relevance to staging: staging a whole file from Unstaged makes it leave
that list, so the follow jumps to the *first* path, not the neighbour and not the same path
in Staged — whatever Fork does after a stage is not recorded here (section 9).

**Keyboard.** The focused list hears ↑/↓ only (`keyboard` in `local_changes.rs`; every chord
left alone via `accelerators::is_chord`). The pane's root hears `Scope::Detail` chords
(previous/next change, `shortcuts::act`). `DiffView` hears arrows, Page Up/Down, Home/End.
No Enter, Space, Backspace, Delete or Tab handling for staging; Tab focus cycling between the
lists and the diff is not stated as built here.

**Read-only today, actionable in this packet:** the two list headings (Fork's Stage /
Unstage buttons), each row (stage/unstage/discard a file, context menu), the diff (hunk and
line stage, unstage, discard), the space under the diff (commit box: subject, body, amend,
the commit button), and the filter's matched rows (a "stage all shown" is Fork-like but not
recorded).

**Where a commit box would go.** `diff_side` returns a column (`rect().expanded()
.content(Content::Flex)`: header, then the body at `Size::flex(1.)`); the right
`ResizablePanel` holds it. Fork and the mockup put the commit box under the diff on the right
side; the natural insertion is a third child of that column, or a vertical splitter inside the
right panel. `docs/design/ui.md` ("Staging is one screen: … the commit box, without
navigating away").

**What enforces it today** — the tests listed at the end of `docs/systems/local-changes.md`,
of which the pattern-setters are `crates/cairn-ui/tests/local_changes.rs`
(`a_press_and_the_arrows_choose_a_row_of_its_list`,
`a_status_of_50000_paths_builds_one_viewport_filtered_or_not`) and
`crates/cairn-app/src/local_changes_tests.rs`
(`local_changes_draws_its_lists_and_the_first_paths_diff_from_the_working_tree_query`,
`a_refresh_that_removes_the_chosen_path_draws_no_stale_diff_under_no_row`).

---

## 4. The diff view, and how gestures could attach

**Rows** (`crates/cairn-ui/src/diff_view.rs`): `DiffView { shown: Readable<ShownDiff>,
scroll: ScrollController, current: Option<Range<usize>>, side_by_side: bool }` →
`VirtualScrollView::new_with_data_controlled(RowsData { shown, rows, geometry: RowGeometry },
build_row, scroll)` with a fixed `DIFF_ROW_HEIGHT` (17 px; `ROW_PIXELS` integer arithmetic for
deep scrolls). `draw_row` dispatches to `unified_rows::build` or `side_by_side_rows::build`,
each naming every `UnifiedRow` / `SideBySideRow` variant (guarded by
`every_view_of_a_diff_row_names_every_kind_of_row`). `draw_row` and `RowGeometry` are shared
with the Commit tab's files opened in place (`commit_tab.rs`, `expansion.rs`), where Fork has
no chunk actions (Finding 13) — any interactive state added to `RowGeometry` must be off there.

**Per-row state today:** only `current` (the change last moved to by previous/next, drawn as a
3 px accent separator, `diff_row_parts::separator`). **No hover, no selection, no per-row
press, no pointer handler on rows.** The only pointer handler is the view's root
`on_press(|_| focus_id.request_focus())`. Unified row anatomy: old number, new number
(`number`, `number_width`), 1 px separator, tinted text area; header and EOF marker are
`note_row` (muted words, no numbers). No marker column (the user's decision 2026-10-04). The
hunk header has "no band and no button" (Fork, Finding 13; `docs/design/diff.md`).

**Side by side** (`side_by_side_rows.rs`): two equal columns inside one virtualizing view,
`Cell::{Line { at, kind, bytes, ranges }, Note, Filler}`; `SideBySideRow::Replaced` pairs the
i-th removed with the i-th added line; leftovers are `Removed`/`Added` with filler; the header
is repeated in each column; the EOF marker sits in the column of the side that did not end.
Line identity is per column: the left cell is an old line (removed or context), the right a
new line (added or context), so a click on a column selects one side's line — Fork allows
"lines … selected on one side at a time only" (Finding 23). C4 already proves side-by-side
offers every changed line exactly once in the exact projection.

**Pointer events and modifiers.** The linked Freya (`caa46f8`) has element events
`on_press`, `on_pointer_down`, `on_pointer_enter`, `on_pointer_leave`, `on_pointer_move`,
`on_pointer_over`, `on_mouse_move`, and global/capture pointer moves
(`freya-core/src/elements/extensions.rs`). `MouseEventData` carries `global_location`,
`element_location` and `button` — **no modifiers** (`freya-core/src/events/data.rs`). So
Shift-click range and ⌘/Ctrl-click toggle must go through `HeldKeys`
(`accelerators.rs`): the window keeps it from every key down/up (`window.rs`, `held_keys`),
lets go on focus loss, and `HeldKeys::press()` answers an `Action`. Only one pointer chord
exists today, `Action::ExtendSelection` (command + press), used by the history list
(`history_list.rs`, `held.peek().press() == Some(Action::ExtendSelection)`). A range action
(Shift + press) would be a new table row.

**Accelerator table constraints** (`docs/systems/diff.md`, "The accelerator table";
`crates/cairn-ui/src/accelerators.rs`): `Action::ALL` has 11 actions; scopes are `Window`
and `Detail` only; "every chord holds a modifier but a function key's"
(`chords_are_distinct_and_every_bare_one_is_a_function_key`), and the chords are "Fork's and
only Fork's" (`the_table_is_forks_chords_and_no_others`). Fork's staging keys are Return or
⌘S (Mac) / Enter or Ctrl+Shift+S (Windows) to stage, Backspace or ⌘⇧D / Ctrl+Shift+D to discard
(Finding 23). Bare Enter/Backspace cannot be chords under the table's rule; they would have
to be keys of the focused view (as the lists' arrows are), and a commit-message field must
keep them. `shortcuts::act` returns early while a credential prompt is up; a confirmation
dialog would want the same ownership of keys.

**Where a gesture could attach, mechanically:**

- *Fork's hover chunk + floating buttons* (Finding 23: hover outlines the hunk including its
  header row, floats `Stage` / `Discard…` — `Unstage` in Staged — at its top right; a
  drag-selection of text narrows them to whole lines; Discard on unstaged only). Needs a
  hovered-hunk state (pointer enter/leave on rows, or pointer move on the view mapped to a
  row by `-scroll_y / ROW_PIXELS`), a row→hunk lookup (`Hunks` + `UnifiedLayout::hunks()`;
  not built), an overlay element positioned at the hunk's first row inside the scroll content
  (rows are recycled by the virtualizer, so the buttons cannot live in a row that scrolls
  away), and a drag selection tracked in view coordinates.
- *A selection gutter* (the mockup's candidate, without its header buttons): a third gutter
  column or a press target on the existing number gutter; selection state as
  `Readable<Selection>` handed to the rows (cheap `holds_*` per row). `RowGeometry`'s
  hand-written `PartialEq` would need to include it or the rows would not redraw.
- Either way: drawn rows map to exact line identities only in the Exact layout; under
  ignore-whitespace the drawn change does not map to exact lines (section 9).
- Entire File (`Context::EntireFile`) is one hunk; Fork's 2019 buttons then applied to the
  whole file, and Mac 2.19 added sub-chunk staging there (Finding 23).

---

## 5. What the design docs and the mockup say

**`docs/design/ui.md`.** "Staging is one screen: unstaged and staged lists, the diff of the
selected file, and the commit box". "Pressing Local Changes puts the staging screen … the diff
of the chosen path with the stage and discard gestures on it and the commit box under it".
**Staging gestures** section: Fork puts no actions on the hunk header; the mockup's header
actions + line gutter are ruled out "as drawn" by the button-free header; "a selection gutter
without header buttons remains a candidate". **Open:** "The staging gesture: Fork's hover
outline with floating Stage and Discard, or a line-selection gutter. Decided before hunk
staging is built." Cairn's deviations table: the destructive dialog says what is lost, how
much, whether recoverable, and its text IS the `Confirmed` prompt; an **operation log
drawer** quoting acknowledged prompts; the discard dialog offers **stash first**; refresh
after Cairn's own operations.

**`docs/design/diff.md`.** "the same patch in reverse removes exactly the selection" (stated
intent — not as built for partial selections, section 1.4); "What a user stages is the exact
diff, never the displayed one"; header has no buttons — "staging acts on a selection, not on
the header".

**`docs/design/mockups/cairn-ui.html`** (the page's own TRAP banner: the detail pane and the
diff are superseded — "a banded hunk header carrying buttons, a checkbox column and
transparent colour washes; none of those survive"):

- *Screen 2, Local changes* ("Stage by file, hunk and line on one screen"): Unstaged and
  Staged **trees** (directory rows), headings with counts and **Stage all** / **Unstage all**
  buttons; diff header with path, `+9 −3`, Unified / Side by side / Ignore whitespace; hunk
  headers carrying **Stage hunk** and **Discard hunk…** (ellipsis: opens the dialog); a
  checkbox column per line (`.cb`), picked lines highlighted; a **commit box** under the
  diff: subject with a `46 / 72` counter, body, **Amend** and **Sign-off** checkboxes, the
  note "pre-commit hook will run", and a **Commit 1 file** button. Notes: hunk actions on the
  header (superseded), line selection as a checkbox gutter (superseded as drawn), "pre-commit
  hook will run" (D1).
- *Screen 3, Discard*: "2 files selected" (multi-select implied), a modal "Discard changes in
  2 files?" with **What is lost** (counts of lines added/removed and the paths), **How much**
  ("since `7f9c648`, 2 hours ago"), **Recoverable: No** ("The reflog does not help here"), an
  option **Stash them first, then discard** (on by default, "Recoverable later as
  `stash@{1}`"), "This text is recorded in the operation log", and the button **Stash, then
  discard 3 changes**. Under it an **Operation log** drawer: time, description (push, commit,
  "Deleted branch … · acknowledged" with the quoted prompt).
- *Screen 1*: a **Stash** toolbar verb; a `stash@{0}` row inline in the history, dashed node;
  a sidebar **Stashes** section.
- No reflog screen, no `.gitignore` UI, no clean dialog, no stash apply/pop/drop UI, no amend
  flow beyond the checkbox.

**`docs/design/feature-inventory.md`.** Tier 2: Stage/unstage file, hunk, line ("Fork
headline. Requires the patch-capable diff model"); Discard file **[D]**, hunk/line **[D]**;
Clean untracked **[D]**; Commit ("Must run hooks"); Amend **[D]** ("the old one survives only
in the reflog"); `.gitignore` editing (no note); Stash create/apply/pop; Stash drop **[D]**.
Tier 1: Reflog view ("Fork sells this as 'restore lost commits'"), Stash contents. Tier 7:
Operation log ("Fed by `ops::Performed`"). **Recovery**: committed work is recoverable via the
reflog, so "the reflog view ships alongside the first commit-level destructive operation";
uncommitted work (discard file/hunk/line, clean, `reset --hard` over a dirty tree) has **no
recovery**; auto-stash before a destructive working-tree operation is open (`cairn.md`, "Still
open"; roadmap O3/L5).

**Roadmap brief** (`docs/work/daily-loop/roadmap.md`, section 5) adds: the
`destructive-ops-reviewer` stops being theoretical; the reflog view ships here (L4); the
operation log becomes visible; O3 decided deliberately; "this packet adds the local write lane
[packet 2a] designs"; "a patch is always emitted at three lines of context from the exact
diff, never from a whitespace-ignoring view, which is what Fork gets wrong".

**Prior evidence on the verbs**: `docs/research/process-manager/consumer-invocations.md`
rows P1-P13 (stage `git add`, unstage `git restore --staged`/`git reset`, hunk/line `git apply
--cached [--check] -`, unstage lines `git apply --cached -R -`, discard lines `git apply -R -`,
discard file `git restore --worktree`, clean `git clean -f [-d]` with `-n` to list, commit
`git commit -F -`, amend, stash push/apply/pop/drop; P13: `.gitignore` editing is a file write
"whether it counts as an `ops` mutation is unstated", reflog and stash list are gix reads,
the operation log is fed by `Performed`).

---

## 6. Sidebar, stashes, and where a reflog view could live

**Stashes today.** Read by `crates/cairn-git/src/refs/stash.rs` (the `refs/stash` reflog via
gix `log_iter().all()`, reversed — `rev()` stops at a line over 4 KiB), as
`StashEntry { index, message, commit, base }` in the `RefsSnapshot`. They appear (a) in the
sidebar's **Stashes** section (`docs/systems/sidebar.md`; every entry by message, list order,
a stash commit filed twice included), and (b) **inline in the history** as `RowContent::Stash`
rows placed by date above their base, one lane into the base, only when the walk reaches the
base (`docs/systems/history-graph.md`; `crates/cairn-git/src/history/stream.rs`). Pressing a
stash selects its row (or, with no row, shows its changes anyway via `RowId::Stash`), and the
detail pane shows what it changed via `ChangesRequest::stash` → `git stash show --raw -z …`
(`reads/stash_changes.rs`, the third porcelain read). A refresh reopens the history when the
stash list changed (`RefsSnapshot::walks_as`), so a stash push/pop/drop is drawn on the next
refresh. **No action exists on a stash** (no context menu anywhere in the application; Freya's
`context_menu.rs`, `popup.rs`, `checkbox.rs` exist in the linked rev).

**Where a reflog view would naturally live — options, none built:**

- Fork's own: "Show lost commits (Reflog)", a history-mode toggle the vendor equates to `git
  log --all --reflog` (`docs/research/refs-and-status/fork-unreachable-stash-base.md`, section
  5; ⌘⇧. / Ctrl+Shift+.). In Cairn that is extra seeds for `HistoryRequest::from_refs`
  (`crates/cairn-git/src/history/seeds.rs`), with the same walk, rows and chips — the lowest
  new-surface option, and it reuses the reflog reader pattern `refs/stash.rs` already proves
  against gix's 4 KiB window bug.
- A sidebar section (between Tags and Stashes, or a `HEAD` reflog under the detached-`HEAD`
  row), virtualized like the others (`cairn_model::SidebarRow`, `RefsSnapshot::sidebar_rows`).
- A detail-pane tab (the `3` chord slot is reserved for File Tree, `accelerators.rs`).
- The operation log drawer (ui.md) is a separate surface: `Performed` exists but only fetch
  produces one, and the command log (`cairn_model::CommandRecord`, `Request::CommandLog` →
  `Update::CommandLog`, answered by the repository thread) is **answered and drawn nowhere**
  (`session.rs`: `Update::CommandLog { .. } => {}`). Neither persists across sessions.

---

## 7. Test patterns the packet will extend

- **Model unit tests** beside the code (`patch.rs`, `line_selection.rs`, `local_changes.rs`),
  each with a "Caught by:" mutation note; model integration tests with an independent oracle
  (`crates/cairn-model/tests/diff_patch.rs`, `diffs::expected_result`, seeded selections,
  `is_a_well_formed_file`). A reverse emitter would want its own independent statement of the
  rule there, and C4 extended to the `::shown` layouts if staging from `-w` is allowed.
- **Engine against real git** (`crates/cairn-git/tests/diff/`): `Repo::new(name)` fixtures
  (`write`, `remove`, `symlink`, `chmod`, `commit`, `config`, `git`, `try_git`),
  `repositories::crafted()` / `rewrites()` / `submodules()`; `Scratch` for isolated index and
  objects; `answers_writing_nothing` / `snapshot` (byte-identical git directory) for reads;
  `same_as_git` parity against porcelain. Every file runs again under git 2.30.9 and 2.32.7
  (`scripts/git-floor.sh`, the `git-floor` gate step). Stub `git` (`process/stub_git.rs`) for
  argv and environment pins (`ops/fetch.rs`, `ops/authority.rs`, `process/cli.rs`).
  Writes must run in throwaway repositories, never the bench (memory: the bench is read only,
  even metadata) and never this checkout.
- **Status parity** (`crates/cairn-git/tests/status.rs`): oracles that are not `git status`
  (`git diff --cached --name-status`, `git ls-files --others --exclude-standard`, `ls-files
  -u`) — the model for "after this stage, the index is what `git add -p` would make".
- **Worker boundary** (`crates/cairn-app/src/worker/*_tests.rs`): a real repository opened
  through `RepositoryHandle` / `Updates`, `handle.submit(..)`, `collect_until`,
  `one_refresh`; `fetch_tests.rs` for an operation's lifecycle and lane refusal;
  `lifecycle_tests.rs` for the command log.
- **Window, headless** (`crates/cairn-app/src/local_changes_tests.rs`, `window.rs`,
  `sidebar_tests.rs`): `TestingRunner` over `window(..)` with a constructed `View`, requests
  captured in `Submitted`, updates applied through `session::apply` exactly as the stream
  applies them, `test.sync_and_update()` loops.
- **Components, headless** (`crates/cairn-ui/tests/local_changes.rs`, `diff_view.rs`,
  `commit_tab_expansion.rs`): a `Fixture` of `State`s provided as root context, labels read
  with positions, painted glyphs read pixel for pixel, viewport-count twins (50,000 paths,
  100,000 lines) named in the root `CLAUDE.md` virtualization invariant — any new list
  (multi-selection, a reflog list, an operation log) inherits that obligation.
- **Guards** (`crates/cairn-guards/tests/invariants.rs`): only `ops/` mutates; the porcelain
  read roster (`the_porcelain_reads_are_the_two_named_queries`); the runner seal; no literal
  modifier outside the accelerator table; diff-row and `DiffContent` readers name every
  variant; no `ScrollView` on a render path; `Confirmed` seal
  (`destructive_operations_are_sealed_behind_the_confirmation_token`).

---

## 8. What the write path already has

- `ops/` with `WriteAuthority` (the only constructor of a write invocation), `Performed`,
  `Invalidated`, `stranded_locks` (a stale `*.lock` reported on the cancel that made it), the
  placeholder `describe_destructive` taking `Confirmed` (to be replaced by the first real
  destructive operation).
- The runner feeds **stdin** (`GitCommand` input written on a thread, `process/cli.rs`,
  `pipes.rs`), so `git apply --cached -` and `git commit -F -` need no new process plumbing.
- `GIT_EDITOR=false` / `GIT_SEQUENCE_EDITOR=false` in `ALWAYS`, so a verb that wants an editor
  fails promptly; `a_commit_cancelled_inside_a_sleeping_hook_leaves_no_index_lock`
  (`docs/systems/git-processes.md`) already exercises a `commit -a` under a hook.
- **One write lane only**: `worker/network_lane.rs` `Operation { Fetch }` and `Lane { Network }`
  ("One, until the first local write adds `Local`"); the local lane is designed
  (`docs/design/concurrency.md`, "Operations": index, working tree and local refs written only
  from the local lane, queued with a "queued" state, a duplicate refused with a reason) and not
  built.
- `cairn_model::Confirmed::by_user(prompt)` / `acknowledged()`; the credential dialog
  (`credential_prompt.rs`, `window::dialog`) as the as-built modal pattern that owns the keys.
- `cairn-ui` must never touch the filesystem; only `ops/` mutates a repository — so
  `.gitignore` editing has no sanctioned home yet (a file write is not a `git` verb; P13).

---

## Gaps and open questions for the packet

**Model and patch**

1. **No reverse emitter.** Unstaging or discarding *part* of a file needs the mirrored rule
   (unselected additions → context, unselected removals → dropped) applied with `-R`, or an
   equivalent "patch against the new side". Only whole-file reversal is proven (C3). Decide the
   API (a direction argument, a second function, or emitting a forward patch of the inverse
   diff) and give it an independent oracle and real-git C2/C3 twins.
2. **No real-git round trip from a working-tree answer.** C1-C3 use commit diffs only. Needed:
   staged→unstage, unstaged→stage, unstaged→discard (`git apply` on the working tree),
   untracked partial stage, intent-to-add, CRLF/`text=auto`/`core.autocrlf`, a clean filter,
   an unborn branch, a mode change beside edits, a type change in the working tree — each on
   2.30.9 and 2.32.7.
3. **Path quoting.** The emitter writes raw path bytes; git quotes paths with special
   characters. Verify against `git apply` with spaces, tabs, newlines, quotes, backslashes and
   non-UTF-8 bytes; likely needs git's C-quoting in every path line.
4. **Ignore whitespace.** The `-w` layout's changes do not map to exact lines. Options: refuse
   line/hunk staging while `-w` is on (Fork's vendor: "ignore whitespace is for review rather
   than staging"; since 2024 Fork proposes turning it off when a stage fails), map a drawn `-w`
   change to the exact changes it overlaps, or stage the exact lines of the hunk the drawn
   change sits in. C4 does not cover `::shown` layouts.
5. **Staged renames.** The staged diff is `--no-renames`, so a renamed path's diff is an
   addition only; whole-file unstage must name both paths, and line-level unstage of a rename
   needs a decision (refuse, or diff the pair). The unstaged i-t-a pair (`UnstagedChange::
   Renamed { from }`) has the same shape on the other side, and `from` has no row.
6. **Stale selection guard.** Should a stage verify the index still holds `old_id` (and, for
   the unstaged side, that the working tree still hashes to `new_id`) before `git apply`, or
   rely on `--check`? Offsets can land a hunk elsewhere in repetitive content.
7. **`git apply` flags and config.** `--whitespace=nowarn` vs `--ignore-whitespace` vs the
   user's `apply.whitespace`; `--recount`/`--unidiff-zero` (P3 says no, as `add -p`). Parity
   target: what `git add -p` would stage.
8. **What `apply_patch` becomes** once there is a caller: hide it (`#[doc(hidden)]` / feature)
   so nothing ships it as a write path.

**Engine and lanes**

9. **The local write lane** (concurrency.md) must be built: queueing, the "queued" state,
   duplicate refusal, and what a queued stage click does when its diff goes stale behind it.
10. **Refresh after a write vs the never-superseded status.** A status started before a stage
    is drawn after it; decide whether a local write may cancel or discard an in-flight status,
    or mark it stale, or accept one status's bounce. Also whether a write asks a full
    `Request::Refresh` or a status-only refresh.
11. **Whole-file verbs for what has no patch**: binary, LFS pointer, too large, submodule
    (stage = `git add`, unstage = `git restore --staged`), conflicted (out of scope? merge is
    Tier 4), nested repositories (`dir/` untracked rows; `git clean` needs `-ff` to remove
    one **(unverified)**).
12. **Clean untracked** needs the list it will delete named in the prompt; status lists one
    row per untracked file (collapsed directories expanded by the second read) and never
    ignored files — decide `-d`, `-x`, and nested repositories.
13. **`.gitignore` editing** is a file write with no sanctioned home (`cairn-ui` touches no
    filesystem; only `ops/` mutates). Decide whether it is an `ops` operation, which file
    (`.gitignore` at which level, `.git/info/exclude`), and how status refreshes after it.

**View**

14. **The gesture** is open by design (ui.md "Open"): Fork's hover outline + floating
    Stage/Discard narrowed by a drag selection, vs a selection gutter. Mechanics available:
    pointer enter/leave/move/down exist in the linked Freya; pointer events carry no modifiers
    (use `HeldKeys`); rows are recycled by the virtualizer, so floating buttons must live
    outside the rows; there is no row→hunk lookup yet; `RowGeometry` is shared with the Commit
    tab, which must stay action-free.
15. **Keys.** Fork's Enter/Backspace (bare) cannot be accelerator-table chords
    (`chords_are_distinct_and_every_bare_one_is_a_function_key`); Ctrl+Shift+S / Ctrl+Shift+D
    (⌘S, ⌘⇧D on macOS) can. Scope: a new scope for Local Changes so a commit-message field
    keeps Enter and Backspace? With nothing selected Fork's keys apply to the whole file.
16. **File multi-selection** (the mockup's "2 files selected", Fork's lists) does not exist:
    `WorkingChoice` is one path. Range select needs a Shift-press action in the table.
17. **Selection after a stage.** The follow rule chooses the *first* path when the chosen one
    leaves its list; what Fork does (next row, same path in the other list) is not recorded.
18. **Discard on the Staged side** — Fork refuses by design (TrackerWin #1563). Decide.
19. **List headings**: Fork's Stage/Unstage-all buttons were deliberately not drawn ("the
    filter field is the only control above the lists", the user's decision of 2026-10-07,
    recorded as a refs-and-status scope choice); this packet must revisit that line.
20. **Commit box**: placement under the diff, multi-line `Input` (`.multiline(true)` exists in
    the linked Freya `input.rs`), subject counter, Amend (prefill from `HEAD`'s message —
    `CommitDetails` is already read for the Commit tab), Sign-off, the "pre-commit hook will
    run" note, streaming hook output (P8: hooks can run for minutes and print megabytes), a
    cancel, and GPG signing's pinentry (#18).

**Recovery surfaces**

21. **Reflog view**: history mode (Fork's "show lost commits", `--all --reflog`), sidebar
    section, or tab; which reflogs (`HEAD`, branches, `refs/stash`); a gix reader must avoid
    `log_iter().rev()`'s 4 KiB failure as `refs/stash.rs` does.
22. **Operation log**: `Performed` is produced only by fetch; the command log is answered but
    never drawn; neither persists. Decide drawer vs panel, persistence, and whether
    non-destructive writes (stage, commit) are logged as `Performed` entries too.
23. **O3, auto-stash before discard/clean** (mockup: on by default) — still undecided; note
    `git stash push -- <paths>` cannot stash a partial-line selection, so "stash first" for a
    line discard is a whole-file stash **(unverified alternatives: a stash of the patch)**.
24. **Stash verbs' surfaces**: no context menu or toolbar exists; stash rows are in the
    history and the sidebar; apply/pop can stop with conflicts (P11), which Cairn cannot yet
    resolve (Tier 4).
