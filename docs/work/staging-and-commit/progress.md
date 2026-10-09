# Progress — staging-and-commit

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

## 2026-10-09 — phase 08 QA, adjudicated and fixed; the user's decisions applied

Four fresh reviewers and a fresh qa-confirm adjudicated phase 08; the user decided the batched
items (2026-10-09, relayed by the coordinator). Commits after b442dc6 below.

Fixed:

- **1 (critical) — a stale line selection applied to a re-read diff.** The files drawn together
  kept one number per ask, so a selection made over a re-ask's old diffs named the new diffs'
  lines once their page arrived; and an act built before a redraw trusted the number it was
  drawn under. Now the files drawn together are renumbered whenever a page replaces a diff that
  was drawn, every `GestureAct` carries its number, and `on_gesture` refuses one no longer drawn
  — single path and stacked alike. Test-first:
  `a_selection_over_files_drawn_together_is_nothing_once_a_page_replaces_its_diff` failed on the
  old code (a `DiscardLinesConsequence` sent against the re-read diff), and
  `an_act_made_under_an_answer_no_longer_drawn_asks_nothing` fails with the check removed.
- **2** — `ShownDiff` holds `Arc<FileDiff>` (`shared_diff`; `the_answer_drawn_is_shared_not_copied`),
  and the lines writes and the lines discard carry the shared answer: no copy per action.
- **8** — a release and a press read the drag in place, copying three fields, never the phase.
- **11** — `part_of_a_new_files_lines_is_discarded_as_lines`. **12** —
  `after_an_action_a_moved_pointer_brings_the_actions_back`.
- **13 (probe)** — flipping the budget's `>=` to `>` survived the real-git test; it gained a path
  costing exactly the budget, which now catches the flip. **14 (probe)** — the real-git test read
  untracked paths only; `paths_drawn_together_are_each_read_as_their_own_side` reads an unstaged,
  a staged and an untracked path through the lane's own stage and commit.
- **#3 residual** written into the root `CLAUDE.md`'s virtualization obligations.

The user's decisions (2026-10-09):

- **5** — with no lines selected, the chords over files drawn together act only on the files
  read and drawn (`together_read_paths`;
  `the_chords_over_files_drawn_together_take_only_the_files_drawn`); and a discard of several
  files names them — the first three and how many more — in `Consequence::prompt`, for every
  caller (the lists' route too), the token's text the dialog's
  (`a_discard_of_several_files_names_the_first_three_and_counts_the_rest`; the engine's and the
  lane's literal prompts updated).
- **6** — the floating discard reads `Discard 2 Lines…`; the dialog's button stays `Discard 2
  Lines`.
- **Ratified** — the line budget for files drawn together; no Load Diff, mode row or previous and
  next change when drawn together; actions pinned at the list's top; a press without a drag or
  Escape clears a selection and focus loss cancels a drag. Recorded as PRD product rules and
  `ui.md` "What changes, and why" rows. **4'** (keep each file's answer across selection changes)
  carried to phase 11; the flash documented as interim.

Dismissed:

- **9** — the floating count and the dialog's count differ: both are `Selection::len()` over the
  same `Selection` (`staging_gesture.rs`, `consequence.rs`, `discard.rs`); a mode change is counted
  apart, and git's end-of-file marker is no line number.
- **16** — a `refresh_tests` flake: 5/5 alone and 3/3 with the whole `worker::` module, unmutated;
  the failure was a `WAIT` deadline under the mutation run's load.

Carried: see `state.md` (phase 11: #3 and #4'; phase 09: #7; phase 12, for the user: #15).

## 2026-10-09 — phase 08, the diff's staging gesture (packet mode)

Built on `feature/staging-and-commit`; QA is the coordinator's. Commits 3bfb9e9 (model: which
drawn chunk a row is in and which lines a drag selects, `row_selection.rs`), dc8a682 (ui: the
gesture layer, `LineDrag`, `ModeRow`, and `StackedDiff` for files drawn together), 59fd0ef
(app: the gesture's acts, the lines' discard consequence on the local lane, the chords narrowed,
and the paths selected drawn together through `Request::Together`), and the docs commit after.

- **No stopping rule met.** (1) Fork's gesture is built on the linked Freya alone (`caa46f8`,
  each API read in the vendored fork): global pointer listeners (`on_global_pointer_move`,
  `_down`, `_press`) on the view's root, `PointerEventData::global_location`,
  `Position::new_absolute` for the layer, `Interactive::No` for the outline and tint, and
  `async-io`'s timer through phase 06's `EdgeScroll` — no second dependency. One finding: a
  node's layer is its parent's plus one (`freya-core/src/data.rs`), so the rows, built deep in
  the virtualising view, stood above a layer added at the view's root and took its presses;
  the layer is lifted by `GESTURE_LAYER` (64), well under `Layer::Overlay`. (2) A chunk drawn at
  a non-default context maps to exactly the exact changes the drawn hunk groups — Fork's chunk
  is the hunk drawn at the current context (Finding 23: the active chunk is outlined with its
  header row; Finding 14: in entire-file mode the buttons apply to the whole file, the file
  being one chunk), so the outline is the selection and no second reading arises
  (`a_chunk_at_context_ten_takes_every_change_it_draws_and_no_other`, C19's QA risk 1).
- **Outside the recycled rows** (R9.1): one layer over the list — at most an outline, a tint and
  three actions — read from the rows' numbers and the scroll, re-rendered on pointer moves and
  scrolls alone. The selection is read from the layout when the drag ends (`selection_in`,
  costing the changed lines selected) and never copied per frame; an action copies it once.
- **A selection belongs to the answer drawn**: the view is handed a number for it
  (`DiffState::working_drawn`, kept per working-tree answer; `together_drawn`, per ask), and a
  `LineDrag` made under another number is nothing — so a refresh or a write's re-read never
  leaves a selection over other lines.
- **After an action** the actions hide until the diff is drawn again or the pointer moves (Fork's
  Tracker #480 and its fix), so a second press cannot act on rows already taken (QA risk 3).
- **The user's decision on phase 07's QA item 4(e) — several paths drawn together — is built**:
  `Request::Together` reads each selected path's working-tree diff on the diff thread, in the
  lists' order, a page at a time under the ask's number; `cairn_ui::StackedDiff` draws them on
  the Commit tab's `Expansion`, the gesture over each file's rows (a drag kept to its file); the
  path chosen stays chosen, unasked, meanwhile. PRD R8.1's note, `local-changes.md` and the root
  `CLAUDE.md` amended; `ui.md` already described the end state.
- **Every line of a new file** (phase 03's `WholeFileOnly`, phase 07's carry) is discarded as the
  file: the view checks it as the engine does and asks `Request::DiscardConsequence` for the
  path, whose count already takes its cancel; a lines discard asks
  `Request::DiscardLinesConsequence` (new, the discard-count lane), read only while still the
  newest — `discard_lines_consequence` reads one path and takes no cancel.
- **The mode row** (R9.4) selects through `Selection::select_mode` (phase 02's carry); a
  mode-only discard is allowed and named by the engine (phase 03's carry).
- **A staged rename's source row** (phase 02's QA item 8, the lines half): it draws the rename's
  staged diff, so its lines unstage at the new path
  (`a_renames_source_row_unstages_its_lines_at_the_new_path`).
- **Keys and dialogs** (phases 06 and 07): the diff's chords stay focused-only, act on a drag's
  lines when there is one, and nothing acts while a confirmation or a prompt is open
  (`nothing_acts_on_lines_while_a_confirmation_is_open`); every lines discard goes through
  `ConfirmDialog` with the engine's `Consequence::DiscardLines`, which carries its selection and
  patch.
- Test helpers that press list rows now look only in the lists' column (`in_lists`), since the
  files drawn together draw their paths too.
- `LocalWrite`'s `expect(dead_code)` names phase 09 alone.

Items for the user's ratification (none was a stopping rule; each is built as stated):

1. **The files drawn together are read under Expand All's line budget** (50,000 lines, a path
   costing one and both its sides' lines); the paths past it are not drawn, each saying to
   choose it alone. Fork's own bound is not recorded. Alternative: no bound (a 50,000-path
   selection would read every diff). Recommendation: keep.
2. **Drawn together, a file offers no Load Diff and no mode row, and previous and next change
   are inert**; each is reached by choosing the file alone. Alternative: build them over the
   stacked view (Load Diff per file, the mode row as a row of the list, stepping across files).
   Recommendation: keep for this packet; file an issue if wanted.
3. **The floating actions stand at the list's top while the chunk's top is scrolled above it**
   (Fork places them at the chunk's top right), so a tall chunk's actions stay in reach; a
   selection's actions stand at its top.
4. **A press without a drag lets a selection go, and so does Escape**; losing focus mid-drag
   lets the drag go (as the lists' drag does) rather than selecting to where it was.
5. **With nothing selected, the chords over files drawn together act on every path drawn**
   (whole files), as the lists' chords act on the selection.
6. **The selection's captions are R9.2's words** — `Stage 2 Lines`, `Unstage 1 Line`, `Discard 2
   Lines` — so the discard's has no ellipsis where the chunk's `Discard Changes…` does.

Carried forward: see `state.md`.

## 2026-10-09 — phase 07 QA, adjudicated and fixed; the user's decisions applied

Four fresh reviewers and a fresh qa-confirm adjudicated phase 07; the user decided the open
items (2026-10-09, relayed by the coordinator). Commits b49d4b4, a918810, be847b8, 4c1c891,
01942eb and this one.

Fixed:

1. **(CRITICAL) A discard's count held the local lane and the close, uncancellable.**
   `ops::discard_files_consequence` now takes a `cancel`, polled before each path and handed to
   each path's `git diff-files` read (which it ends); a cancelled count is
   `Error::ConsequenceCancelled` and answers nothing. The count is numbered in a lane of its own
   (`QueryLane::DiscardCount`): a newer ask, `Request::StopCounting` (asked as Local Changes
   unmounts, `use_drop`) and a close end it. Each path is counted once by a set rather than a
   quadratic scan. Tests: `a_cancelled_count_stops_between_paths_and_runs_no_further_read`,
   `a_path_named_twice_is_counted_once` (ops, same commit),
   `a_newer_ask_or_a_stop_ends_a_discards_count_and_its_read` (lane: the superseded count's held
   read ended), `leaving_local_changes_ends_a_discards_count`; root `CLAUDE.md`'s residuals say
   the count is per path, holds the lane, and what ends it.
2. 50,000 paths ALL selected, with `.held` set, still build one viewport
   (`a_status_of_50000_paths_all_selected_builds_one_viewport`).
3. Root `CLAUDE.md`'s cost of a large selection corrected: Shift+↑/↓ re-spans on every key repeat,
   a ⌘/Ctrl-press clones twice and inserts; 5-15 ms at 50,000, per press, never per frame.
5. **A refresh left the selection holding gone paths.** The follow now moves the selection with
   the path it chooses (`Follow::ChooseFirst`) or lets go (`Follow::LetGo`); a toggle that empties
   the selection keeps it its list's, so nothing is acted on (`acted_rows` falls back to the path
   chosen only with no selection made there). `a_refresh_or_an_emptying_toggle_leaves_nothing_selected_unseen`
   failed first on both halves.
6. **(CRITICAL) The behind-the-modal test could not fail.** Rewritten with the recording
   submitter, driving every intent, `choose` and `on_the_diff` behind the open confirmation;
   checked against M7 (`dialog_open` → `false && ..`): it now fails, five writes asked.
7. **R8.3's call site.** `the_selection_moves_to_the_row_that_takes_the_acted_rows_place`: c of
   a, b, c, d leaves d; cx of the filter's cx, dx, ex leaves dx. Checked against M6
   (`nearest_remaining(len, 0 * first, ..)`): it now fails (a.rs).
8. The drag's other ends: `a_press_heard_mid_drag_or_focus_lost_ends_the_drag_without_a_drop`.
   **It found a gap**: a press on the filter field after a lost release did not end the drag —
   Freya's `Input` cancels the global pointer-down (and its own release) as it takes a press —
   so the next release over the other list dropped. The lists' root now also hears the
   platform's mouse-down, which fires before any pointer-down handler and which a field does
   not cancel. Residual, stated in `local-changes.md`: a press on another view's text field (the
   sidebar's filter) after a lost release is still not heard.
9. The release-outside case now crosses Staged before it is released over the filter.
11. **The user's decision: Stage All / Unstage All take the rows a filter shows** (every row
   with none on): `LocalWrite::StageAll`/`UnstageAll` carry `shown: Option<Vec<u32>>`, the
   filter's indices, and the lane gathers those rows; nothing is asked while the filter's rows
   are on their way. `stage_all_and_unstage_all_take_the_rows_the_filter_shows` failed first (a
   hidden row and a hidden conflicted row were staged); lane unit test
   `an_all_gathers_the_rows_the_filter_shows_or_every_row`. PRD R8.2 noted.

The user's decisions recorded (2026-10-09, the user's):

- 4(a) R8.3's rule for a selection with gaps (the row in the first acted row's place, else the
  nearest above) RATIFIED — PRD R8.3 note, `local-changes.md`, `docs/design/ui.md`.
- 4(b) a double press acts on its own row RATIFIED — C18 amended, PRD R8.2 note.
- 4(c) "Staged changes can't be discarded: unstage them first." RATIFIED — PRD R8.4 note, a
  deviation row in `docs/design/ui.md`'s "What changes, and why", `local-changes.md`.
- 4(d) Stage All stays in Unstaged's heading with the Alt press; xfwm, openbox and Plasma 5 grab
  Alt+button-1, where the press fails safe — `docs/design/ui.md`, PRD R8.2 note, `local-changes.md`.
- 4(e) the multi-selection diff MATCHES FORK (the selected files' diffs drawn together), built in
  phase 08 on the Commit tab's layout of files opened in place (`cairn_ui::Expansion`); phase 07
  draws the path last pressed in meanwhile — PRD R8.1 note, `docs/design/ui.md`,
  `local-changes.md`, `state.md` (a phase 08 requirement).
- 10 conflicted-row staging LEFT AS FORK: no extra wording before a conflicted row is staged; the
  one-way resolution inside Cairn is a stated residual in `local-changes.md`.
- 11 above.

Dismissed, with reasons:

- 12 "no test of `whole_file_paths`": it is in ed168c2,
  `a_whole_file_action_names_each_rows_path_and_a_renames_source`.
- 13 `Err(String)` across the worker boundary: the engine's errors stay typed
  (`Error::Refused` and the rest, `ops/discard.rs`); the application formats one only to draw it
  and branches on nothing but the cancelled count, which it matches typed.
- 14 a rename row's discard counts two files: the prompt names both honestly, and that is
  `whole_file_paths`' intended rule (the rename moves back whole).
- 15 no end-to-end gesture → git test: each link is tested against its real boundary (the
  component's intents, the window's requests, the lane through the real worker, the engine
  against real git); recording requests at the application is the seam's design.

Carried: to phase 08 the Fork-matching multi-selection diff (4e); to phase 11 batching the
count's per-path reads into one multi-path `git diff-files` read in `reads/` (measure first), and
the argv size of stage, unstage and restore at 50,000 paths (destructive-ops).

## 2026-10-09 — phase 07, Local Changes acts on files (packet mode)

Built on `feature/staging-and-commit`; QA is the coordinator's. Commits ed168c2 (model:
`LocalChanges::whole_file_paths`), 4b60da0 (ui: the lists' selection, routes, drag, menu, the
exact diff's bar, the edge scroll's lost release), f059a13 (git: a nested repository's row
staged, and the engine's `Absent` arm owned by its caller), 988cee5 (app: the actions, the
discard's consequence on the local lane, the confirmation, the line under the lists).

- **No new dependency, no Fork route unbuildable** (stopping rule 1 not met): Freya's
  `EventsCombos` gives the double press, `ContextMenu`/`MenuButton` the menu (each item closes
  it, `ContextMenu::close`), `on_mouse_up` the drop — a press is reported only on the element
  the button went down on, so a drop zone hears the release, as Freya's own `DropZone` does —
  and `freya::clipboard::Clipboard` Copy Path; each read in the vendored fork at `caa46f8`.
- **The selection after a stage (R8.3) for a multi-selection** — stopping rule 2 weighed, and
  decided rather than stopped on, for the user's ratification (item 1 below): the row that
  slides into the FIRST acted row's place, else the nearest row above it
  (`cairn_ui::nearest_remaining`). For a contiguous selection that is Fork's documented
  "nearest remaining" with no other reading; it differs only for a selection with gaps.
- **Where a discard's consequence is asked** (phase 04's carry): on the local lane, as a job of
  its own (`Request::DiscardConsequence`, `LocalJob::Consequence`), so it counts what the writes
  asked before it left (`a_discards_consequence_is_counted_after_the_writes_asked_before_it`:
  a stage held in its `git add`, the consequence answered only after it ended, and refusing the
  path it staged). The answer opens `Confirming::new` from the pane (whose submit it needs);
  one arriving with Local Changes not shown, or for an earlier ask, is dropped.
- **Paths git status never listed** (phase 03's QA item 4): the caller owns it — the window
  asks only for paths of rows the lists drawn list, each found again by a search
  (`a_discard_names_only_paths_the_lists_drawn_still_list`); `staging.md`'s residual and the
  engine test's doc (`a_file_git_clean_leaves_is_named_as_kept`, the `Absent` arm) say so.
- **A collapsed untracked-directory row** is, since status lists untracked files one per file,
  only ever a nested repository (`dir/`): staged by its row it becomes a gitlink at its commit
  and nothing else (`a_nested_repositorys_row_stages_exactly_what_git_add_adds_for_it`, read
  back with `git ls-files --stage`); its discard is refused before any dialog (engine,
  phase 03).
- **Keys** (phase 06's carry): `Scope::LocalChanges` heard on each list's and the diff's own
  `on_key_down`, `Scope::LocalChangesLists` (Shift+↑/↓) on the lists; presses resolved through
  `HeldKeys::press()` (`ExtendSelection` toggles, `SelectRange` ranges). The table gained one
  chord: `StageOrUnstageAll`'s ⌥/Alt **press**, for the heading button (R8.2's "⌥-held");
  every table test and the text-field tests now skip a press where they read keys.
- **While a dialog is open** (phase 06's QA item 4): the dialog's modal holds the keys, and
  `local_changes_actions::intent` and `on_the_diff` act on nothing while a confirmation or a
  credential prompt is up (`local_changes_acts_on_nothing_while_a_confirmation_is_open`).
  **A credential prompt over a confirmation** (item 5, decided): the confirmation is set aside —
  kept, unanswered, not drawn — while the prompt is up, and drawn again after
  (`a_credential_prompt_sets_an_open_confirmation_aside_until_it_is_answered`). **Backspace in
  the filter field** (item 21): `backspace_in_the_filter_edits_the_filter_and_discards_nothing`.
  **The pointer leaving the window** (item 16): `EdgeScroll` (and the lists' drag) end on a
  press heard while a drag is on, and on focus lost
  (`a_release_the_window_never_heard_ends_the_drag_at_the_next_press_or_focus_lost`).
- **R8.5**: the working-tree query is asked with `ignore_whitespace: false` whatever the shared
  setting (`diff_actions::working_options`, and `DiffState::settings_changed` takes the working
  options apart); `DiffHeader::exact` draws the toggle off and disabled and no hiding notice.
- **R8.6**: one line under the lists (`local_changes_actions::acting_line`): why the last
  action asked nothing, a discard being counted, the write running and those queued, or a
  write's ending that did not do what was asked (a stale patch naming its path).
- `expect(dead_code)` removed from `Confirming::new`, `local_writes::ask`, `LocalWrites::{queued,
  running, last}` and `OperationId::next`; `LocalWrite`'s now names phases 08 and 09.

Decisions, and items batched for the user's ratification (none was a stopping rule):

1. **For the user's ratification — R8.3 for a selection with gaps**: the selection moves to the
   row that takes the first acted row's place, else the nearest above it. Alternatives: the row
   after the LAST acted row; or the remaining row nearest the selection's middle. Recommendation:
   keep (the eye stays where the selection began; a contiguous run walks down the list as
   Fork's does).
2. **For the user's ratification — a multi-selection's diff**: the diff shows the path last
   pressed in (or, toggled out, another the selection holds), where Fork draws the selection's
   files together. A combined view is new UI, not in R8.
3. **For the user's ratification — Stage All's placement and press**: the double chevron sits in
   Unstaged's heading (Fork for Windows' place; Linux follows the Windows rows), and the heading
   button's ⌥ press is Alt on Linux (Fork for Windows' own is unrecorded), which some window
   managers take to move a window; the chevron, the menu and the chord remain.
4. **For the user's ratification — what the view says for a staged-side discard**: Fork does
   nothing; Cairn says "Staged changes can't be discarded: unstage them first." (the chord on the
   Staged list or a staged diff), asking nothing.
5. **A double press acts on its row**: its first press makes the row the selection, so a double
   press never stages a multi-selection; C18's "each route stages and unstages a
   multi-selection" holds for the other four routes, each tested both ways, and the double
   press is tested on its row.
6. The menu names no chord beside its items (a chord spelled in a label would be a modifier
   named in a component) and nothing opens it from the keyboard.

Carried forward: see `state.md` (the amend diff against `HEAD^` to phase 09; a discard of every
line of a new file to phase 08, whose gesture is the only route that selects lines).

## 2026-10-09 — phase 06 QA, adjudicated and fixed; the user's decisions applied

Five fresh reviewers and a fresh `qa-confirm` (the coordinator's; probes P1-P3 ran as window
tests). Fixed, each test-first — the new test red on the code before the fix, or the named
mutation or bypass caught:

1. **Critical, items 1-3 as one change — the dialog kept a replaced confirmation's handlers,
   stayed deaf after a chained one, and copied the consequence on every render.** `ChoiceButton`'s
   equality ignored its handler and the dialog was unkeyed, so B set over A drew B and handed A's
   continuation a token naming A's path (P1); `answered` survived into a confirmation opened by
   the last one's answer, leaving Escape, Cancel and confirm dead under inert window chords (P2);
   and the window cloned the `Consequence` and compared it whole each render. Each confirmation
   now has a serial (`Confirming::serial`): the dialog is equal by it and keyed by it (the window's
   `.key` and its own `render_key`), so another confirmation remounts afresh; `Confirming` shares
   its consequence (`Rc`), and the dialog renders its words once as it mounts. Window tests
   `a_confirmation_replaced_in_place_hands_its_own_token_to_its_own_continuation` and
   `a_confirmation_opened_by_the_last_ones_answer_answers_afresh` were red on c74bfff ("the
   replaced confirmation got a token"; "Escape did nothing"). The "identity is stable" comment is
   gone.
2. **Item 7 — the accelerator pin's guard read only the pin's own attributes.** New
   `pin_placement_violations`: declared once, directly in the one module whose attributes are
   exactly `#[cfg(test)]`, its own attributes exactly `#[test]` (or none, for R4.8's pinned
   function), its body still holding its rule examples. R4.8's pin had the same gap (a
   `cfg(any())` module, a nested module) and takes the same check. Self-test
   `the_pin_placement_check_catches_the_shapes_it_claims` (module compiled away, `cfg(not(test))`,
   ignored, nested in a `cfg(any())` module, body emptied, an example dropped, moved into an
   `impl`, renamed) and three new R4.8 shapes.
3. **Item 8 — a field built without `Input` escaped the text-field guard.** `TEXT_FIELD_IDENTS`
   adds `use_editable`, `UseEditable`, `EditableConfig`, `text_edit`, `SelectableText`,
   `CodeEditor`, each self-tested; `TEXT_FIELD_EXCEPTIONS` excuses the accelerator table's
   `text_edit` (the editor's bindings), required to still match. Bypass reproduced: a
   `freya::text_edit::use_editable` in a render file fails the guard.
4. **Item 9** — root `CLAUDE.md` names `async-io` beside `nix` as a dependency whose features
   `deny.toml` pins.
5. **Item 10 — `EventHandler<Confirmed>` was trusted by spelling.** New
   `token_callback_shadow_violations`: no production file declares, aliases or imports-as
   `EventHandler`, `Fn`, `FnMut` or `FnOnce` (self-tested both ways); the residual — a callback
   stashing its token in a captured cell — is stated in root `CLAUDE.md`.
6. **Item 11 — `text_field_in` took any scope.** It takes a `FieldScope` (the commit box alone);
   `a_fields_own_scope_holds_no_bare_chord` holds every field scope free of a bare chord.
7. **Item 14, the user's decision — `is_chord` swallowed another view's chords.**
   `is_chord(event, own)` reads the window's, the detail pane's and the view's own scopes only;
   `another_views_chord_is_the_history_lists_arrow`, `..._file_lists_arrow`,
   `..._commit_tabs_arrow` and `..._diff_views_arrow` were each red before (Shift+↓ moved nothing)
   and green after; `a_chord_of_the_window_the_pane_or_the_views_own_scope_is_a_chord` replaces
   `a_chord_of_any_scope_is_a_chord`.
8. **Item 15** — `edge_step` answers 0 for a viewport with no height
   (`a_viewport_with_no_height_scrolls_nothing`, red before: a pointer at -1000 over `0..0`
   scrolled -32).
9. **Item 18** — root `CLAUDE.md`'s UI-thread residuals name the edge-scroll timer task.
10. **Items 19 and 20 — mutations K and L survived.** The text-field tests' view around the field
    resolves every scope, the window's included, and the window records every raw key: removing
    the unclaimed arm's `stop_propagation` (K) now fails the filter test, and removing the own
    arm's `prevent_default` (L) the commit-box test.

The user's decisions (2026-10-09, relayed by the coordinator):

- **6 — ratified**: the token-holder exemption for a callback's argument, with item 10's
  rename refusal and the stated residual. Root `CLAUDE.md` records it.
- **12 — ratified**: "every text field takes the one key policy", with item 8's widened list.
  Root `CLAUDE.md` records it.
- **13 — R7.3 amended**: "no list or staging chord fires while the commit box has focus; the
  window's chords still do" (as from the filter fields). PRD R7.3 carries a dated note;
  `Scope::CommitBox`'s doc and state.md say so.
- **14 — fix `is_chord`** (item 7 above). Range-select in the history stays a later call.

Carried (state.md): to phase 07, items 4, 5, 16 and 21; to phase 09, how amend builds its token
(through `ConfirmDialog`, the box handing the window a `Confirming`), a token-less force-push
warning if informational, and the widened guard already covering the commit box.

Dismissed:

- **17** (per-key work; the idle context-menu viewer): an observation, bounded — `field_key` is
  one table resolution per key press, `shortcuts::act` returns early in O(1) under a dialog, and
  the viewer's pointer-move write touches its own scope alone.

## 2026-10-09 — phase 06, render foundations (packet mode)

Built on `feature/staging-and-commit`; QA is the coordinator's. Commits f044c95 (the table's
lists and scopes, the text-field key policy, the confirmation dialog and the context-menu host)
and the edge auto-scroll with `async-io` after it.

- **C15 failed first, as the stopping rule requires**:
  `a_focused_filter_field_hands_the_windows_chords_and_held_keys_to_the_window` was written
  before any fix and was red on 2112f18 on both halves — the Refresh chord unheard with the
  sidebar's filter focused (0 refreshes asked), and the extending chord held while the filter
  had focus never reaching `HeldKeys` (a plain selection). Green after the policy.
- **The accelerator table (R7.2, R7.3, L22)**: `chords(action, os) -> Chords`, a list in the
  table's order, replaced `chord(..) -> Option<Chord>`; every caller and test moved. New
  actions and scopes: `StageOrUnstage`, `StageOrUnstageAll`, `Discard` in `LocalChanges`;
  `ExtendSelectionUp/Down` (Shift+↑/↓) in `LocalChangesLists`; `Commit` in `CommitBox`;
  `ShowLostCommits` in `History`; `SelectRange` (Shift+press) beside `ExtendSelection`, which
  now also names a path toggled in Local Changes' lists. No new chord collides with an
  existing Cairn chord (no stopping rule). The pin, renamed
  `chords_are_distinct_and_every_bare_one_is_a_function_key_or_local_changes_own`, checks every
  chord of every list and then shows its rule failing on a bare Enter, Backspace or Delete moved
  to each other scope, a bare letter in Local Changes, a bare press, a chord listed twice for one
  action and a chord shared by two actions. `the_accelerator_table_holds_data_and_resolution_only`
  now also requires the list signature, no single `chord`, and the pin as a plain `#[test]`.
- **The key policy (R7.1)**: `accelerators::field_key` resolves a key for a focused field
  (`FieldKey::Own`, `Unclaimed`, `Edit { bubbles }`); `cairn_ui::text_field` and
  `text_field_in` build every field with it (the three filters and the credential prompt).
  A window chord and a lone Control/Alt/Command pass to the window untyped; a primary+letter
  that is no `EditBindings` binding types nothing; the field's own scope's chord is claimed;
  every other key is kept from the views around (Shift alone still bubbles). New guard
  `every_text_field_takes_the_shared_key_policy` (matcher `builds_a_text_field`, self-test).
  `Chord::press_hold` now sends the modifier's own key, as a keyboard does.
- **The dialog (R7.4)**: `cairn_ui::ConfirmDialog`, on `CONFIRMATION_SURFACES`; the window keeps
  `View::confirming: State<Option<Confirming>>` (`crates/cairn-app/src/confirming.rs`) and
  `shortcuts::act` is inert while it is open. **The context-menu host (R7.5)**:
  `ContextMenuViewer` at the window's root. **Edge auto-scroll (R7.6)**:
  `cairn_ui::{use_edge_scroll, EdgeScroll, edge_step}`, paced by `async_io::Timer`.
- **Dependency**: `async-io = "2.6.0"` in `cairn-ui` (L5, the user's decision): the allowlist
  row, the workspace manifest's reason, and `deny.toml`'s reason with a `[[bans.features]]`
  pin of no features (`exact`, `allow = []`); `Cargo.lock` gained only the edge from `cairn-ui`
  — no crate, no feature (no stopping rule).
- Each new test was checked against the mutation it claims (the dialog without `a11y_modal`,
  without its Cancel focus; the window without the confirmation check or the menu host; the
  field policy replaced by Freya's default; the timer looping once).

Decisions, and items batched for the user's ratification (none is a stopping rule):

1. **For the user's ratification — the token-holder guard excuses a callback's argument.**
   `CONFIRMED_HOLDERS`' rule refused `ConfirmDialog`'s `EventHandler<Confirmed>` and the
   window's `Rc<dyn Fn(Confirmed)>`, which hand a token on and keep none. Rather than roster
   both files (which would excuse any later field there), the matcher blanks exactly the four
   `TOKEN_CALLBACKS` spellings first; a callback that returns a token, takes it beside other
   arguments, or is named otherwise still fails (self-tested). Root `CLAUDE.md` states it.
2. **For the user's ratification — a new invariant line and twin**: "every text field takes
   the one key policy" in root `CLAUDE.md`'s modifier invariant, with
   `every_text_field_takes_the_shared_key_policy`. Its one cost: `window_check.rs`'s private
   test enum `Input` was renamed `Stimulus`, since the guard reads the name.
3. **For the user's ratification — window chords still pass from the commit box.** R7.3 says
   "no chord but commit's fires while the commit box holds focus"; C16 names the stage, unstage,
   discard and Show Lost Commits chords. Built as C16 and R7.1 read together: none of R7.3's
   other actions resolves in `CommitBox`, but a window chord (F5, the tab chords) still reaches
   the window from the box as from any field. If R7.3 means every chord, the box's
   `text_field_in` would claim window chords too — a one-line change in `field_key_on`.
4. **Shift+↑/↓ no longer move the selection in the history, the Changes tab's files, the Commit
   tab or the diff**: they are a chord now (`is_chord`), and those views leave chords alone, as
   they do Ctrl+↓. Before, Shift was ignored and Shift+↓ moved as ↓ did.
5. **`deny.toml` pins async-io's features** (none) beyond the phase's ask, so a later crate
   turning on `tracing` is a decision.
6. The dialog's title is the caller's (`Confirming::new("Discard changes", ..)`, Fork's Windows
   title); `Consequence` renders no title. Its answers are focusable `rect`s, not Freya's
   `Button`, whose focus cannot be given on open.
7. Dismissed: `NamedKey::Super` in the lone-modifier list — deprecated in keyboard-types; Meta
   covers it.

## 2026-10-09 — phase 05, the user's decisions A-F applied

The user decided the six batched items (relayed by the coordinator, 2026-10-09):

- **A — ratified**: `git rm --cached -f -q` out of a root commit's amend. R3.4's note now
  records the ratification (and R6.3 names `-f` too).
- **B — ratified with the fix**: the user accepted QA's verdict that asking the upstream alone
  was a defect (fixed in 44ccdb1); R6.4's amendment records the ratification.
- **C — ratified**: commit and amend are refused while `git am` is in progress. R6.9, R10.8 and
  C24 now list it.
- **D — seed Show Lost Commits from every reflog entry's old and new ids; keep the prompt's
  wording.** R11.1 and C20 amended (the walk's tips are each entry's old and new ids, as `git
  rev-list --reflog` and `git fsck` read a reflog; C20's fixture adds an amend whose log it
  created itself); "The old commit stays in Show Lost Commits." stays; the 30-day
  `gc.reflogExpireUnreachable` expiry is a stated residual in R11.1. A phase 10 requirement that
  blocks its QA (state.md).
- **E — keep `MERGE_MSG`'s `# Conflicts:` lines visible**: the box prefills the file as git
  wrote it and the user deletes them by hand; under `-F` they are committed if left. R10.8 and
  C24 amended; recorded for phase 09 (state.md).
- **F — ratified**: `git commit -q`, and no `--literal-pathspecs` on a commit. R6.1's note
  records the ratification.

Phase 05 is done: QA adjudicated, its fixes and these decisions applied, the full gate green
at c07c076 (this entry's change is documentation alone; `gate.sh --fast` and `qa-stop.sh` run
over it).

## 2026-10-09 — phase 05 QA, adjudicated and fixed

Four fresh reviewers and a fresh `qa-confirm` (the coordinator's). Fixed, each test-first —
the new test red on the code before the fix, or the named mutation caught:

1. **An upstream behind `HEAD` answered "unpublished" without asking other remotes** (= batched
   B, a defect against `Publication::Unpublished`'s "no remote-tracking ref reaches it"): the
   pushed check now falls through to `HEAD --not --remotes` when the upstream does not hold
   `HEAD`. `the_dialog_is_asked_exactly_when_a_remote_has_head` gains the case (an upstream
   behind, another remote branch at `HEAD`, checked against `git branch -r --contains`), red
   before the fix. R6.4's method sentence amended for the user's ratification. (`fix(git)`.)
2. **No test checked the argv `git commit` actually ran**: a recording `git` now pins both
   verbs' argv, stdin, environment and the command log
   (`commit_and_amend_run_git_commit_with_the_message_on_stdin_and_nowhere_else`); the `-m
   <message>` mutation of `run` fails it. (`test(git)`.)
5. **The CLAUDE.md lane-lock residual** omitted `install`'s kill and the amend walk's poll
   under the lock; both named. (`docs(docs)`.)
6. **Amend's staged list in a partial clone**: fails closed on git 2.44+, fetches below it —
   stated as status's residual is, and pinned on the host and both floors
   (`in_a_partial_clone_amends_staged_list_fails_rather_than_fetching`). (`test(git)`.)
7. **The reflog rule was never tested one log at a time**: a detached `HEAD` with only its own
   log, and a branch with only its own, under `false`; dropping either term of the rule fails
   the test (both mutations run). (`test(git)`.)
8. **The lane's hook-skip mapping was never exercised**:
   `a_failing_hook_fails_a_commit_and_the_skip_commits_past_it`; inverting the mapping, or
   fixing it either way, fails it. (`test(app)`.)
9. **The re-check was never tested against a reflog change**:
   `an_amend_refuses_when_the_reflog_it_promised_is_gone_since_it_was_confirmed`; a re-check
   comparing `HEAD` alone fails it. (`test(git)`.)
10. **R6.1's amendment** now says "for the user's ratification" and reads `-q` as by analogy
    with decision 12; batched as F below. (`docs(docs)`.)

Carried (state.md): 3 and 11 to phase 11; 4 to phases 09 and 11; 6's and 8's view halves to
phase 09; 12 (= D) to phase 10, pending the user.

Dismissed, with its reason:

- 13 — **a fetch's askpass title could carry a URL with credentials**: the network lane names
  the token after `Operation::Fetch { remote }`, the default remote's NAME; fetch never takes a
  URL.

The bench repository: its `.git` directory's mtime moved at 2026-10-08 20:35:34 — an entry
created and removed inside it, most likely a lock from a `git` run there without
`GIT_OPTIONAL_LOCKS=0`; no file inside changed, and the agent is unknown. The coordinator's
earlier "untouched" check had used a time format `bfs` rejects, with its stderr hidden. (This
phase's own reads of the bench, on 2026-10-09, ran with `GIT_OPTIONAL_LOCKS=0`; its
`find -newer <marker>` checks were empty.)

Batched for the user, added to A-E below:

- **F.** Ratify R6.1's amendment: `git commit -q`, by analogy with decision 12, and no
  `--literal-pathspecs` on a commit.

B is no longer batched: QA found it a defect, fixed above (item 1), and R6.4 is amended for
ratification.

## 2026-10-09 — phase 05, the commit engine (packet mode)

Built on `feature/staging-and-commit`; full gate green; QA pending (the coordinator dispatches
the reviewers). No stopping rule was met:

- **`-F -` against `git commit -F <file>`**: byte-identical under every `commit.cleanup` value
  and none, `core.commentChar` unset and `;`, with `#`/`;` lines, CRLF, trailing spaces and
  blank lines, a scissors line and non-ASCII text, by commit and by amend, on git 2.30.9,
  2.32.7 and 2.56.0 (checked by hand first, then pinned:
  `a_message_is_stored_as_git_commit_f_stores_it_under_every_cleanup`).
- **The floors amend as the host does**: every test of `crates/cairn-git/tests/diff/commit.rs`
  passes on 2.30.9 and 2.32.7 (amend, root amend, merge, rebase, `git am`, cherry-pick and its
  sequence, revert, the reflog arms, the publication cases).
- **The pushed check stays bounded on rust-lang/rust** (a plain `git clone --no-hardlinks` of
  the bench at `c999cef531e` in `/tmp`, its 13 remote-tracking refs fetched from the bench's
  own, deleted after; release build, warm, the `the_pushed_check_on_a_large_repository`
  reporter, three rounds each):

  | `HEAD` | Publication | Time |
  | --- | --- | --- |
  | `main` at the tip, upstream `origin/main` | upstream | 0.2-2.7 ms |
  | `main` at the tip, no upstream | some remote | 0.2-2.2 ms |
  | a new commit on the tip, no upstream | unpublished | 91-99 ms |
  | a new commit, upstream behind it | unpublished | 0.2-2.4 ms |
  | detached at `main~2000` | some remote | 1.10-1.12 s |
  | detached at `main~30000` (2013) | some remote | 1.30-1.31 s |

  git's own `rev-list -1 HEAD --not --remotes` in the last state: 1.14 s without a
  commit-graph, 0.08-0.10 s with it (`for-each-ref --contains` 0.01 s). The walk reads no
  commit-graph so it stays cancellable at every object read; bounded by the history, as git's
  is without a graph, and on a worker. Judged bounded, not stopped for; the cost and the
  commit-graph option are carried to phase 11.
- The bench was read only: `find ~/Development/bench/rust/.git -newer <marker>` empty before and
  after the clone and the fetch from it.

What shipped, by acceptance criterion (engine halves; the views are phases 06-09):

- **C13**: the message test above; `a_non_utf8_commit_encoding_is_refused_before_git_runs`;
  `a_failing_pre_commit_hook_fails_the_commit_with_its_output_and_the_skip_commits` (output on
  the failure and streamed, nothing committed, no lock, the skip commits; `.git/hooks`,
  `core.hooksPath`, a non-executable hook not counted);
  `amends_staged_list_is_the_index_against_heads_parent` (against `git diff --cached
  --name-status HEAD^`, a rename paired) and the root commit's against the empty tree;
  `a_root_commits_amend_works_and_unstages_with_rm_cached`;
  `amend_is_unavailable_on_an_unborn_branch`; `with_no_identity_gits_own_error_is_the_outcome`;
  `recent_messages_are_git_logs_last_ten`.
- **C14**: `the_dialog_is_asked_exactly_when_a_remote_has_head`, `the_amend_button_and_prompt_name_head`.
- **C24**: `a_merge_in_progress_commits_the_merge_and_refuses_an_amend`,
  `a_rebase_am_cherry_pick_or_revert_in_progress_refuses_commit_and_amend` (every state made by
  real git; `git status`'s words checked beside the engine's answer).
- **C2 (amend)**: `an_amend_refuses_when_head_moved_or_was_published_since_it_was_confirmed`.
- **R6.4's reflog** (the user's decision on phase 01's item 13):
  `whether_the_reflog_is_written_is_what_git_then_does`, each arm against the entry git then
  writes. Found doing it: git's own `git reflog` lists each entry's NEW id, so with the logs
  removed and the default setting, the amend writes an entry whose OLD id is the replaced
  commit, which `git reflog show --format=%H` (C20's seed) never lists. Carried to phase 10.
- **C12 (identity)**: `a_commit_is_by_the_identity_in_cairns_environment`.
- **C10-C12's commit halves on real `git commit`** (`worker/local_lane_tests.rs`, a commit held
  in its `pre-commit` hook): `a_commit_keeps_refreshes_back_and_a_stage_asked_meanwhile_waits_for_it`,
  `a_refresh_asked_before_a_commit_draws_nothing_while_it_runs`,
  `a_cancel_names_its_commit_and_never_reaches_the_one_queued_behind_it`,
  `a_close_ends_a_fetch_at_once_while_it_waits_on_a_commit`,
  `a_close_during_a_commit_waits_for_it_and_ends_nothing`,
  `a_signed_commits_prompt_is_titled_by_the_commit_while_a_fetch_runs` (a real `gpg.format=ssh`
  commit, its signing program a stand-in `ssh-keygen` that asks through `SSH_ASKPASS` — no real
  key, agent or `~/.gnupg` touched), and `an_amend_through_the_lane_quotes_its_prompt_and_keeps_refreshes_back`.
  Each ran five times green.

The carries from phases 03 and 04: `hooks_path` re-exported; `UnstageTo::Commit`/`Nothing`
exercised through amend; the root amend's refusal met (`-f`, below); `HeldCommit` deleted; the
`deferred` override deleted (item 7), every commit's ending reading everything
(`a_commits_ending_reads_everything_whatever_it_did`); the fixed 500/700 ms windows and the
`< 100 ms` bound replaced (item 9) by the repository thread's answer to a later
`Request::CommandLog`, a held process's mark, or a held `git add` the asks must return before —
the one time waited out is the close's patience, which is what that test tests; a prompt titled
by its own operation (item 13: `Channel::begin_for`, `Prompt::operation`, `Update::Prompt`'s
`asking`); `install()`'s kill under the lane's lock documented as a `try_lock` and a `killpg`;
`-q` on `git clean` with a test that `restore` and `apply` are silent on success
(`the_destructive_verbs_say_nothing_on_success_so_an_orphan_finishes`); the banner. Re-carried:
`Request::CancelWrite`'s `expect(dead_code)` (phase 09 first constructs it).

Decisions taken here (each stated in the PRD as amended or in `docs/systems/staging.md`):

1. **`git rm --cached -f -q`** out of a root commit's amend: without `-f`, git refuses a path
   whose staged content differs from both the file and `HEAD` — exactly what an amend's list
   shows edited. `-f` drops the staged blob from the index as `git reset` would; `--cached`
   keeps the file. R3.4 amended, for the user's ratification.
2. **`git commit -q`**, and no `--literal-pathspecs` on a commit (git exports it to the hooks,
   where a `pre-commit` hook's globbed pathspec would match nothing). R6.1 amended.
3. **`git am` refuses a commit and an amend** as a rebase does; R6.9 names a merge, a rebase, a
   cherry-pick and a revert, and `git status` reports an `am` session as one of them. Batched.
4. **The operation in progress is read as git's `wt_status` reads it**, not through gix's
   `Repository::state`, whose order differs and which misses a cherry-pick sequence whose
   stopped pick was committed (the API checked against the vendored gix 0.87.1).
5. **An upstream that is a local branch is no remote** (`branch.<b>.remote = .`): the pushed
   check falls back to every remote-tracking ref, as for none.
6. **A cancel before git runs stops the commit's checks** (an amend's walk can be long) and
   ends `NotRun`; once git runs, the lane's `install` path the phase doc names.
7. **The commit's output reaches the window as `Update::WriteOutput`** per line, as a fetch's
   progress does; and a failure carries stdout's tail ahead of stderr's, since git says why on
   stdout for "nothing to commit" and an amend that would be empty.

Batched for the user (no stopping rule; none blocks the packet):

- **A.** Ratify decision 1 (`-f` on the root amend's unstage) — or prefer refusing that path
  with git's words.
- **B.** R6.4 as written asks only the upstream when there is one: with the upstream behind
  `HEAD` and another remote branch holding it, the amend reads as unpublished and no dialog is
  asked. Recommendation: fall through to `HEAD --not --remotes` when the upstream does not hold
  it (a second walk only in that case).
- **C.** Decision 3 (`git am` refused). Recommendation: keep.
- **D.** Phase 10's seed for Show Lost Commits (the reflog finding above). Recommendation: seed
  from each entry's old and new ids, as `git rev-list --reflog` does, so `Reflog::Written`
  always means findable.
- **E.** R10.8's prefill keeps git's `# Conflicts:` comment lines from `MERGE_MSG`, which a `-F`
  commit under the default cleanup stores. Recommendation: the box strips git's comment lines
  from the prefill as git's own editor cleanup would (phase 09).

## 2026-10-09 — phase 04 QA, adjudicated and fixed; the user's decisions 12 and 14 applied

Five fresh reviewers (`responsiveness-reviewer`, `destructive-ops-reviewer`,
`gate-integrity-reviewer`, `test-coverage-auditor`, `qa-checklist`) and a fresh
`qa-confirm`. Fixed, each test-first (the new test, or the guard with the bypass planted,
red on the code before the fix):

1. **The inherited roster was cut at the first `];` in its raw text**, so a comment or a
   string holding one hid a following `GIT_AUTHOR_DATE`, and the twin passed (planted, and
   it did). `commented_table` now finds the closing with comments and strings blanked; the
   self-test spells both shapes. (`fix(guards)`.)
2. **A close waited on the local lane before ending a fetch**, so a fetch reached the network
   for as long as a commit's hooks ran. `Threads::drop` cancels the fetch through
   `FetchControl` first (`a_close_ends_a_fetch_at_once_while_it_waits_on_a_commit`, which
   waited the stream out before the fix). (`fix(app)`.)
3. **The open's lock listing walked `refs/` on the repository thread before its first
   answer.** The local lane lists them before any write as `Update::LocksAtOpen`, and the
   walk polls the lane's closing before each directory (`stranded_locks_until`,
   `a_cancelled_search_answers_nothing_and_an_uncancelled_one_everything`;
   `SharedRepository::lock_files` takes a `Cancel`). (`fix(app)`.)
4. **`CONFIRMED_HOLDERS` read the engine alone**, so the application's `LocalWrite` held a
   `Confirmed` with no row. The scan reads every crate's production code, requires the
   worker among it, and rosters `crates/cairn-app/src/worker/local_lane.rs`; emptying the
   roster turns the twin red, and the self-test refuses an unrostered application holder.
   Root CLAUDE.md updated. (`fix(guards)`.)
5. **The lane was marked closing only in `Threads::drop`.** `submit`'s close arm marks it too
   (`a_close_marks_the_local_lane_closing_as_it_is_submitted`); the CLAUDE.md residual for the
   close arm says so. (`fix(app)`.)
6. **R4.6's thread-side gates were never reached by a test.**
   `a_refresh_asked_before_a_commit_draws_nothing_while_it_runs` queues a refresh's
   ahead/behind behind a held status as the commit starts; dropping the refresh thread's
   ahead/behind gate turns it red. The repository thread's gate on refs is not driven (its
   refs are read before a test can start a commit behind them), stated in
   `docs/systems/git-processes.md`. (`test(app)`.)
10. **The roster's doc counted four identity variables**; it names all five. (`docs(git)`.)

The user's decisions (2026-10-09, relayed by the coordinator):

- **12 — the second close during a write: keep R4.9 and harden it.** A second close past
  the patience still closes the window and never ends the write. `docs/design/processes.md`
  ("Lifecycle") now says so instead of "ends it as a cancel would", with the orphan residual
  (an orphaned `git` dies of `SIGPIPE` at its next line of output, which `-q` keeps `git
  clean` from writing), stated as built in `docs/systems/git-processes.md` ("Closing"). The
  hardening is phase 05's (state.md).
- **14 — reads inherit the same roster as writes: keep one roster.** No code change; the
  reason — it is what the user's own `git` gives a filter or hook from a shell — is beside
  `INHERITED` in `crates/cairn-git/src/process/environment.rs`.

Filed: #87 (every local write walks all of `refs/` twice, `ops/local_write.rs`'s
`locks_now`/`locks_around`, phase 03 code — for phase 11's measurement) and #88 (`gate.sh
--fast` does not build the askpass helper though `built_helper`'s comment says the gate
does).

Dismissed, with reasons:

- 15 — **the identity variables are inherited**: the user's decision (R5.2, L26); the date
  variables are pinned out twice, in the builder's tests and the twin.
- 16 — **a write is dropped if `cairn-local` fails to spawn**: fail-closed by design, as for
  the diff and refresh threads (`Threads::start`); a dropped `Confirmed` means nothing
  destructive runs.
- 17 — **the UI thread takes `LaneState`'s mutex**: stated in the root CLAUDE.md residuals;
  every hold is bounded (an assignment and at most one unbounded send), with no lock taken
  inside it that the UI thread holds.
- 18 — **`refs/` walked twice per write**: phase 03's code, unchanged here; filed as #87.
- 19 — **the gate failed on a missing askpass helper**: a cold target; `--fast` runs `--lib
  --bins` only; filed as #88.

## 2026-10-08 — phase 04, the local write lane (packet mode)

Built on `feature/staging-and-commit`; gate green; QA pending (the coordinator dispatches the
fresh reviewers implementation-plan.md names: `responsiveness-reviewer`,
`destructive-ops-reviewer`, `gate-integrity-reviewer`, `test-coverage-auditor`).

What shipped (`docs/systems/git-processes.md`, "The local write lane"):

- **The lane** (R4.1-R4.3): `crates/cairn-app/src/worker/local_lane.rs`, the `cairn-local`
  thread. `Request::Write { id, write: LocalWrite }` routed by `submit` straight to its
  queue; FIFO; `OperationId` taken by the window as it asks (`OperationId::next`,
  `local_writes::ask`); `Update::WriteStarted` / `WriteEnded { id, ending, read_again }`;
  `Request::CancelWrite { id }` reaching `LaneState` directly, ending only a running
  commit with that id. `LocalWrite` carries the six phase-03 verbs, a discard its
  `Confirmed`; `UnstageTarget` is `UnstageTo` in the window's words.
- **Freshness** (R4.4-R4.6): `LaneState`'s write clock ticks at a write's start and end
  under the lock the lane announces under; the refresh thread reads no status while a
  write runs and sends one only if the clock did not tick while it was read (same lock).
  A write's ending names what to read again (`ReadAgain::Status` →
  `Request::RefreshStatus`, new, status alone; `ReadAgain::Everything` →
  `Request::Refresh`); the window asks exactly that. While a commit runs, a refresh is
  kept back at `submit` and at the repository thread's refs, the refresh thread's status
  and ahead/behind; the commit's ending says `Everything`.
- **Endings** (R4.7, R4.9): `WriteEnding::{Done, Stale, Refused, Failed,
  MayHaveTakenEffect, Incomplete, NotRun}`, each with its lock files. Closing marks the
  lane closing, joins it (the write running finishes; ones queued end `NotRun`), then ends
  the rest; the window says "Finishing <write>…" (`Closing::when_requested`,
  `status_text::closing_line`). `Update::Opened` carries `SharedRepository::lock_files`
  (new), and the window names the locks last listed (`status_text::locks_line`).
- **Prompts and the roster** (R5): one askpass token per write; the window shows a prompt
  while a fetch or a write is in flight, titled by the write when no fetch runs. R5.2's nine
  joined `INHERITED`, each with its reason, and the environment twin reads the roster
  (`INHERITED_PINS`, `INHERITED_NEVER`). R5.3's residual was already stated in
  `docs/design/processes.md`; filed as #86 and cited there.
- Window state: `crates/cairn-app/src/local_writes.rs` (`LocalWrites`), `View::writes`.

Tests: C10 — `writes_asked_faster_than_they_run_run_in_order_each_with_its_own_ending`,
`a_status_begun_before_a_write_ended_is_never_drawn`,
`a_commit_keeps_refreshes_back_and_a_stage_asked_meanwhile_waits_for_it`,
`a_cancel_names_its_commit_and_never_reaches_the_one_queued_behind_it`,
`a_writes_ending_reads_again_what_it_says_and_no_more`; C11 —
`a_close_during_a_commit_waits_for_it_and_ends_nothing`,
`a_lock_left_behind_is_named_as_the_repository_opens_and_by_the_write_it_fails`,
`the_window_is_told_once_as_the_first_close_is_asked`,
`the_window_names_the_write_it_waits_on_its_prompt_and_the_locks_found`; C12 —
`a_prompt_a_stages_hook_raises_is_shown_and_answered` (a real `git add` whose
`post-index-change` hook asks through the helper),
`a_prompt_a_commit_raises_is_shown_and_answered`,
`a_prompt_is_shown_while_a_write_runs_and_its_ending_takes_it_down`, and the twin's
roster check with its self-test. Mutations that redden them, checked by hand: sending a
status whatever the clock (the stale-status test), never keeping a refresh back (the
commit test), not joining the lane on close (the close test).

Decisions and deviations:

- **The commit-dependent halves run against `LocalWrite::HeldCommit`**, a variant compiled
  for tests alone: `ops::fetch` through a stub `git` whose `fetch` is held, asks or ends as
  the test says — the one long-running, cancellable write the engine has before phase 05,
  which the lane treats as a commit (`is_commit`, `ReadAgain::Everything`). Phase 05 adds
  `Commit`/`Amend` to `LocalWrite` and re-runs those tests against `git commit`.
- **`Request` is no longer `Clone`**: a destructive write carries its `Confirmed`, spent
  once. Tests keep `Clone` through a test-only impl on `LocalWrite` that refuses to copy a
  destructive write.
- **A status skipped or dropped is always replaced**: a write's ending always names status
  at least (every local verb's `Invalidated` has the index or the working tree), and the
  window asks it unless it is closing. So the stopping rule (a dropped status no later read
  replaces) did not trigger.
- **Fetch's types and behaviour are unchanged**: the local lane is a sibling, not reached
  through `Threads::perform`; the one new fetch behaviour arises only beside a running local
  write (a fetch's ending leaves a prompt up while a write runs). Not a stopping-rule change.
- **No roster variable carries a secret**: `XAUTHORITY` is a path to the cookie, never it.
- **The open's lock listing is on `Update::Opened`**, made on the repository thread before
  its first answer, so a stream's order is fixed (a separate update from another thread
  would land anywhere in a test reading in order). It walks `refs/` once per open; not
  measured on the bench repository (phase 11 may).
- **A cancel for a queued commit does nothing** (R4.3: only while it runs); a Cancel the
  commit box draws (phase 09) must be for the running one.

Batched for the user (no stopping rule; recorded for the packet's end):

1. **The second close during a write.** `docs/design/processes.md`, "Lifecycle", says a
   second close past the patience "ends it as a cancel would"; PRD R4.9 says it "closes
   anyway, as today". Built per the PRD: the window closes and the write's `git` is left to
   run on, orphaned — it finishes, or its next write to a pipe nobody reads ends it — and a
   lock it leaves is named the next time the repository opens. Ending it with `SIGTERM`
   first needs a non-waiting "end everything" the UI thread can call (the registry's
   `end_all` waits). Recommendation: accept the PRD's behaviour and amend the design's
   sentence; or file the non-waiting end as a follow-up.
2. **A prompt carries no owner.** With a fetch and a local write in flight at once, the
   ending of one leaves a prompt up that may have been the other's; it stays until answered
   or cancelled, and the acceptor serves nothing else meanwhile. Tagging a prompt with its
   operation needs `cairn-askpass`'s `Prompt` to say which token asked. Recommendation:
   accept for now (rare: two writes asking at once); file if phase 11 shows it.

Carried forward:

- **Phase 05**: `LocalWrite::Commit`/`Amend` (`is_commit` true, `ReadAgain::Everything`,
  `perform` installing the cancel with `LaneState::install`, whose and
  `Request::CancelWrite`'s `expect(dead_code)` then go); re-run the `HeldCommit` tests
  against `git commit` with a slow hook (C10, C11, C12's halves).
- **Phase 07**: asking writes (`local_writes::ask`, whose `expect(dead_code)` then goes),
  drawing a write queued where the user acted and its outcome (`LocalWrites::queued`,
  `last`); where the window asks `discard_*_consequence` before a dialog (they run `git`,
  so a worker's call; the local lane orders them after the writes ahead, which a
  consequence computed while a stage is queued would otherwise race).
- **Phase 11**: the activity popover reads `WriteEnding` and `Done`; the open's lock listing
  measured on the bench repository.

## 2026-10-08 — phase 03 QA, adjudicated and fixed; the user's four decisions applied

Four fresh reviewers, adjudicated by a fresh `qa-confirm`
(`scratchpad/qa03/adjudication.md`). Confirmed findings fixed in focused commits, behaviour
changes test-first (each new test seen to fail on the code before its fix):

- 1 (critical): a deleted tracked `d/a` with an untracked file `d` where its directory
  was read as absent, so `git restore` would unlink `d`; a symlinked `d` was hashed outside
  the working tree. Every directory on a path is now looked at without following links, and
  an obstructed path is refused before any prompt (`Refusal::Obstructed`) and by the
  re-check (`a_file_where_a_deleted_files_directory_was_is_never_destroyed`,
  `a_symlinked_parent_is_never_followed_out_of_the_working_tree`).
- 2 (engine half): the executable bit is recorded in every discard's `Consequence` and
  re-checked, so a `chmod` after the confirmation refuses (the mode moves in both C2 tests).
- 3, 4: a discard of files reads every confirmed file again after the run and answers
  `Error::DiscardIncomplete { performed, kept, failure }` when it did not take every one —
  `git clean` failing part way after `git restore` ran, or leaving an ignored file
  (`a_discard_that_fails_part_way_says_what_it_did_and_what_is_left`,
  `a_file_git_clean_leaves_is_named_as_kept`). Item 4's refusal of paths status never
  listed is carried to phase 07 (state.md).
- 5: `Consequence::DiscardLines` carries the patch emitted from the confirmed diff, and
  `discard_lines` applies exactly it and takes no diff
  (`a_discard_applies_the_patch_it_was_confirmed_with`; the model's
  `the_confirmed_patch_is_part_of_the_value`).
- 6: the git-form comparison is decisive — a filter attribute added after the
  confirmation moves git's form with the bytes unchanged; with the comparison removed the
  test fails (checked by hand).
- 7: `hashing_a_file_runs_only_the_clean_filter_and_writes_nothing` and
  `the_hooks_path_read_writes_nothing_and_runs_nothing`, named in the root `CLAUDE.md`'s
  list of reads held to the clean filter and fsmonitor.
- 8: git-floor's floors raised to 128, 166 and 16 (counts 129, 167, 17).
- 9: `reads/hash_object.rs`'s anchor fixed. 10: git-floor's skip count is seven.

The user's decisions of 2026-10-08 (relayed by the coordinator), applied — batched items
2, 3, 5 and 6 of the phase 03 entry closed:

- **Decision 2** (batched 2): R3.9's amendment ratified, reworded to "the bytes and the
  executable bit"; `docs/design/engine.md` matches.
- **Decision 3** (batched 3): C2, R3.5 and C8 amended to what is built — a file added
  beside a confirmed one is never taken and the discard proceeds, a path behind a file or a
  symlink is refused, `-d` is never passed.
- **Decision 5** (batched 5): an intent-to-add file's discard keeps `git restore`'s
  behaviour, worded honestly: `FileLoss::Emptied`, "1 new file emptied (20 lines)"
  (`an_intent_to_add_file_is_named_as_emptied`,
  `an_intent_to_add_files_discard_empties_it_and_says_so`); R1.2 amended.
- **Decision 6** (batched 6): a whole file's mode change is named — `FileLoss::Modified`
  carries `mode`, "1 modified (2 lines and the mode change (100644 to 100755))", and alone
  never "0 lines" (`a_whole_files_mode_change_is_named_never_counted_as_no_lines`,
  `a_whole_files_mode_change_is_named_and_put_back`); R1.2 amended.

Still batched for the user: 4 (pinning `apply.ignoreWhitespace` anyway) and 7 (a discard
of files reads one diff per tracked file to count its lines).

Dismissed, with the adjudicator's reasons:

- 11 (`IndexNow` freshness): `IndexNow::read` calls `open_index()`, which in gix 0.87.1
  (`src/repository/index.rs`) builds a fresh `File::at` each call — no cache, no mtime gate.
- 12 (`hash-object` lazily fetching through `.gitattributes` on git before 2.44 in a sparse
  partial clone): the accepted residual the root `CLAUDE.md`'s environment invariant already
  states; the 2.30 floor is the user's decision.
- `write_verbs`' 24 tests in 0.04 s looked too fast: 0.21 s single-threaded, a `git` spawn
  takes under a millisecond on this host, and no test returns early. Not a defect.

## 2026-10-08 — phase 03, C21's margin decided

The user chose C21's margin on 2026-10-08, relayed by the coordinator: a flat 50 ms per
verb, added to git's own time plus one status read (option A of the phase 03 report). From
`measured-baseline.md`'s highest sums the bars are about 93 ms to stage (42.9 + 50), 93 ms
to unstage (42.8 + 50), 76 ms to discard (25.8 + 50) and 87 ms to commit (37.4 + 50).
PRD C21 and R13.2 amended. Batched item 1 is closed; items 2-7 stay batched.
`measured-baseline.md` is evidence and not retro-edited (docs/CLAUDE.md); its section 4
already set out the 50 ms option, and the PRD records the decision.

## 2026-10-08 — phase 03, the write verbs (packet mode; stopped for C21's margin)

Built on `feature/staging-and-commit`; QA pending (the coordinator dispatches the fresh
reviewers). Stopped at R13.2's stopping rule: git's baseline exists
(`docs/research/staging-and-commit/measured-baseline.md`), and C21's margin is the user's.

What shipped (`docs/systems/staging.md`):

- `ops::stage_lines`, `unstage_lines` (`git apply --cached --whitespace=nowarn -`),
  `discard_lines` (`git apply --whitespace=nowarn -`), `stage_files` (`git add`),
  `unstage_files` with `UnstageTo::{Head, Commit(id), Nothing}` (`git reset -q`, `git
  reset -q <id>`, `git rm --cached -q`), `discard_files` (`git restore --worktree`, then
  `git clean -f --` in batches) — every one a write with `--literal-pathspecs` before the
  verb, many paths in a NUL pathspec file on stdin, the locks listed before and after on
  `Performed::locks` (R3.8), taking `Option<&AskpassToken>` for phase 04.
- The stale check (R3.7): the index read fresh with gix (`IndexNow`, once per call), the
  working tree's git form by `reads::hash_object` (`git hash-object --path=<p> -- <p>`,
  never `-w`) and its bytes hashed in process; `reads::hooks_path` (`git rev-parse
  --git-path hooks`) landed for phase 05, module-private until then.
- `discard_lines_consequence` and `discard_files_consequence` build each `Consequence`;
  `discard_lines` and `discard_files` take `Confirmed` by value, re-check before the first
  write and replaced `describe_destructive` on the roster in the same change.
- `Consequence::DiscardLines` gained `on_disk` (the bytes' hash) and `mode` (named in the
  prompt with both modes); `FileLoss`'s working-tree id is the bytes' hash.
- New errors: `Error::Refused { path, why: Refusal }`, `NoPaths`, `ChangedSinceRead`,
  `ChangedSinceConfirmed`, `ReadIndex`, `ReadWorkingTree`.
- C3 now runs through the verbs; `tests/diff/write_verbs.rs` holds C2's discard halves,
  C5, C6, C8's engine half and C9's effect; the recording `git` (`ops/recording_stub.rs`)
  holds C9's argv. `scripts/git-floor.sh` runs `ops::` too, floors 126/158/16.

Measurements and decisions:

- **`git clean`'s argv bound** (R3.5): `ops::CLEAN_ARGUMENT_BYTES`, 64 KiB per invocation,
  each path counted as its bytes, its NUL and its 8-byte pointer. Measured on this host:
  `getconf ARG_MAX` 2,097,152; `git --literal-pathspecs clean -n -- <names>` took 1,536 KiB
  of 100-byte names and the kernel refused 1,900 KiB (`E2BIG`, the pointers and the
  environment counted); one argument of 128 KiB is refused (`MAX_ARG_STRLEN`). 64 KiB is
  half Linux's smallest `ARG_MAX` (32 pages), a sixteenth of macOS's 1 MiB. A list past it
  runs as several `git clean`s after one re-check
  (`a_long_list_is_deleted_in_batches_after_one_recheck`: 700 paths, 2 invocations).
- **`apply.ignoreWhitespace`**: measured on 2.30.9, 2.32.7 and 2.56.0 over 400
  selections of a whitespace-heavy diff, staged with and without `-c
  apply.ignoreWhitespace=change` (and `=false`): every staged blob identical, every apply
  succeeded. The setting only lets a patch land on context that moved by whitespace (a
  stale index: exit 0 with it, exit 1 without), which the stale check refuses first. Not
  pinned; C5 holds the case; PRD's filed list amended.
- **`hash-object` reproduces the diff's git form** of a CRLF file under `core.autocrlf`
  and of a rot13-filtered file, with `--path` and with several paths at once, on 2.30.9,
  2.32.7 and 2.56.0, and writes nothing — so that stopping rule did not fire. A symlink
  is hashed in process (as its target), since `hash-object` reads the file it points to.
- **No verb behaved differently on 2.30.9** in what Cairn passes: `--end-of-options`
  before a commit after `--pathspec-from-file` is refused by 2.30's `reset` ("must come
  before non-option arguments") where 2.56 takes it, so the commit is passed as its hex
  id with no `--end-of-options` (an id cannot be an option); every verb as built passes
  the same tests on 2.30.9, 2.32.7 and 2.56.0.
- **Hashing for the re-check** (phase 01's QA item 25): every discard compares the
  file's bytes hashed with no filter, so a line ending changed after the confirmation
  refuses; a discard of lines also compares git's form (R3.7). PRD R3.9 amended — for the
  user's review.
- **The mode in a discard of lines** (QA item 19): `Consequence::DiscardLines.mode`, the
  prompt "discard 2 lines and the mode change (100644 to 100755)", the label "Discard 2
  Lines and Mode Change" / "Discard Mode Change"; a selection of the mode alone with no
  mode change is refused as nothing selected (QA item 18's zero-line prompt).
- **Every line of a new file discarded** is refused as `WholeFileOnly` and left to
  `discard_files`, whose prompt says "1 untracked file deleted (N bytes). You can't undo
  this action." (phase 02's carry-forward).
- **The rename source row** (phase 02's QA item 8): its staged diff is the rename, and
  its lines unstage at the new path, the source's entry untouched; its own whole-file
  unstage resets the source alone; both paths named unstage the rename whole.
- **R3.8 as built**: a success carries the locks before and after; a failure carries
  git's own report of those present after it (`GitFailed.present_locks`,
  `GitCancelled`/`GitUnwatched`'s `stranded_locks`), which names any lock that was there
  before and still is.
- The bench repository's `.git` was unchanged by everything: `find .git -newer <marker>`
  listed nothing before the clone, after it and after every baseline run.

Batched for the user's review — not decided:

1. **C21's margin** (the stopping rule): proposed below in the phase's report.
2. **R3.9's amendment**: a discard of files re-checks the bytes on disk hashed in process,
   not `git hash-object` (which cannot hash a symlink as git stores it and would not see a
   line ending changed under `core.autocrlf`).
3. **C2's "an untracked file added to a deletion's directory"**: Cairn never deletes a
   directory (status lists files one per file; the one directory it lists whole, a nested
   repository, is refused), so such a file is never taken and the deletion of the
   confirmed files proceeds (`a_file_added_beside_a_confirmed_deletion_is_never_taken`) —
   not refused, as C2's wording has it. A confirmed file replaced by a directory is refused.
4. **`apply.ignoreWhitespace`**: not pinned, as measured; pinning `-c
   apply.ignoreWhitespace=false` anyway would close the window between the stale check
   and `git apply` for a whitespace-only move.
5. **An intent-to-add file's discard of files** runs `git restore --worktree`, which
   leaves the file EMPTY (git's own behaviour, verified on 2.30.9 and 2.56.0) and the
   intent-to-add entry in place; the prompt counts its lines as modified. Deleting it
   instead (as an untracked file) would be two writes.
6. **A mode change in a discard of whole files** is restored without the prompt naming
   it (`FileLoss::Modified` counts lines only).
7. **A discard of files reads each tracked file's unstaged diff to count its lines** —
   exact, but one read per file selected.

## 2026-10-08 — phase 02 QA, adjudicated and fixed

Four fresh reviewers, adjudicated by a fresh `qa-confirm`. Confirmed findings fixed in
focused commits, behaviour changes test-first (each new test seen to fail before its fix):

- 1: `action_patch` returns the empty patch for an empty selection before anything else;
  an empty untracked file's stage, unstage and discard, and a `ModeChangeOnly` type
  change, wrote headers or `deleted file mode` before
  (`a_selection_of_nothing_makes_no_patch_by_any_action`).
- 2: the whole-index pairing read runs only for a path absent from `HEAD` or the index
  (`only_a_path_that_can_be_in_a_pair_asks_the_whole_index`, which counts the reads in
  the command log; it failed with the read running for a copy's modified source).
- 3: `scripts/git-floor.sh`'s floors raised to one under the runs' counts (84 and 134).
- 4: C3 decides a deletion from the case and checks the patch's header against it; an
  unborn branch's addition unstaged whole leaves no entry.
- 5: the partial-clone residual of the staged pairing stated in `docs/systems/diff.md` and
  pinned on both arms (`in_a_partial_clone_a_staged_pairing_fails_rather_than_fetching`).
- 6: the write-nothing pin asks a staged `git mv` with an edit, so both pairing reads
  run under it, and requires that they did.
- 7: C3's awkward names gain CR, DEL, BEL, BS, VT and FF.
- Optional, done: C3 gains a staged copy under `diff.renames=copies`, unstaged as content
  at the copy's path with real git (`lines_of_a_staged_copy_unstage_as_content_at_its_path`).

Carried forward to phases 03 and 07 in state.md: 8, 9 and the destructive carry list.

Batched for the user's review at the end of the packet — not decided:

- 10: PRD R2.1's text against the insertion-then-removal refinement of
  `TextDiff::inverted` (no reviewer found the refinement wrong).
- 11: whether C21 gains a selection-to-diff row for a staged rename (the cost of the
  pairing read under `diff.renameLimit=0`).

Dismissed, with the adjudicator's reasons:

- 12 (the `ContentReadsDisagree` retry is unbounded): it is not —
  `worker/diff_lane.rs`'s `READ_ATTEMPTS` is 3, and `asking_again` stops at 3 and on
  cancel.
- 13 (C4 derives its expected mode from `drawn.file.inverted()`): the `assert_ne`
  beside it is decisive whatever the expected value says; style only.

## 2026-10-08 — phase 02, the patch engine (packet mode)

Built on `feature/staging-and-commit`; full gate green (git-floor included); QA
pending — the coordinator dispatches the fresh reviewers. What shipped:
`TextDiff::inverted`, `Selection::inverted`, `ChangedFile::inverted` (R2.1);
`Selection`'s mode item (R2.4); C-quoted path lines (R2.5); `action_patch`, what
stage, unstage and discard emit, refusing part of a whole-file-only change (R2.2,
R2.3); the staged side of the working-tree query pairing renames and copies as `git
diff --cached` does (R2.6), unstaging lines of a rename as content at its new path.
C3, C4 and C7 pass against real git on 2.56.0, 2.30.9 and 2.32.7
(`crates/cairn-git/tests/diff/staging.rs`, `staged_renames.rs`); the intent-to-add
question is settled (state.md). No stopping rule fired: every floor git applied
every case as the host's did; the staged pairing is query plumbing (`diff-index`),
no porcelain read; no case needed `-R`.

Decisions taken in the phase (none is a stopping rule; the first is batched for the
user's review, as a refinement of R2.1's wording):

- **An inverted replacement is two changes, its insertion then its removal.** R2.1
  says "each change's two spans swapped". Swapped as one change, the forward rule
  leaves what stays of a partial selection inside one replacement AHEAD of what it
  restores (`B b`), where git's mirrored patch applied with `-R` — E1b, and what git's
  own `reset -p`/`checkout -p` edits do — leaves `b B`; the two oracles C3 names then
  disagree on every mixed selection. With the split, the inversion, the reference
  applier, the mirrored rule as the model states it and real `git apply -R` all agree
  on every case and seed; the unsplit inversion fails both the model's property test
  and C3 (checked by mutation). Visible cost: a whole inverted replacement is written
  `+old` before `-new`, which git applies identically. Still one forward rule, still
  no `-R`.
- **The C3 mirrored oracle is grounded in real git**: written in the test from the
  forward diff (it inverts nothing), applied in reverse both by real `git apply -R`
  on a scratch file outside any repository and by the reference applier, which must
  agree; the reference applier then reads Cairn's own patch, and git applies it.
- **The staged pairing reads the whole index once** (`git diff-index --cached --raw
  -z -M|-C -l<n> --diff-filter=RC --ita-invisible-in-index`), keeping only the
  records naming the path as they stream, then reads the pair's lines across both
  paths with the same detection; a record that moved between the two reads is
  `ContentReadsDisagree`. `--ita-invisible-in-index` was found needed: plumbing
  otherwise pairs a deleted empty file with an intent-to-add one, which `git diff
  --cached` never shows (C7's fixture holds that case; dropping the flag fails C7).
  `--diff-filter=RC` also drops an unmerged entry's `U` record, which the raw parser
  would refuse.
- **A staged rename's source path answers the rename record too**, since that
  record is the only thing `git diff --cached` shows for it; a copy's source keeps
  its own record.
- **The mode item is selected by nothing but `select_mode`**: whole-file round
  trips (`tests/diff/patches.rs`, diff-engine's C1-C3) now select it explicitly.
- **Path lines carry git's trailing tab** after a `---`/`+++` label holding a
  space (`diff.c`), so the emitted headers equal `git diff`'s byte for byte, which
  C3 compares for every awkward name.
- The root `CLAUDE.md` was not edited (state.md, carried forward).

## 2026-10-08 — phase 01, the user's three decisions applied

The user decided the three items the adjudication left with them (relayed by the
packet coordinator):

- **Item 6, the empty roster**: assert it non-empty. The guard now asserts
  `!DESTRUCTIVE_OPERATIONS.is_empty()`, its doc comment says so, and emptying the
  roster was seen to fail the guard. The placeholder row satisfies it until phase
  03.
- **Item 13, the amend prompt**: conditional wording. `Consequence::Amend` carries
  `reflog: Reflog`; the prompt says "The old commit stays in Show Lost Commits."
  only for `Reflog::Written`, and "The old commit can't be recovered afterwards:
  this repository keeps no reflog." for `NotWritten`, with full-literal tests for
  both arms. What decides it was checked against git 2.56: an amend writes the entry
  under `core.logAllRefUpdates=true` (the non-bare default), writes none under
  `false` set from the start, and still appends under `false` set after the logs
  exist; a bare repository leaves the setting unset, and git's default there is
  `false`. R10.6 and R6.4 are amended with a dated note; the engine's computation
  is phase 05's (state.md).
- **Item 34a, R1.2's bytes**: R1.2 amended to Fork's wording — lines per modified
  path and bytes per untracked file (L8) — with a dated note; no code change.

## 2026-10-08 — phase 01 QA, adjudicated and fixed

Eight fresh reviewers, adjudicated by a fresh `qa-confirm`. Confirmed findings
fixed in focused commits (`fix(model)`, `fix(git)`, `fix(guards)`, `docs(docs)`);
each guard fix was mutation-tested — the hole reopened in the real tree, the guard
seen to fail, the tree restored:

- 1: the token-file check reads every function (`function_signatures`), every
  impl header with attributes stepped over (`impl_headers`), refuses `const`,
  `static`, `macro_rules!` and `mod` items, and requires one `by_user` and one
  literal. The Secret guard's same idiom is filed as #85.
- 2: `CONFIRMED_HOLDERS` (empty) refuses an engine type that keeps a token in a field.
- 3: `renames_type` starts a statement after `}` and `{` as well as `;`.
- 4: the ceiling `drive` takes is `Kind::Ceiling`, `Infallible` for a write; a
  generic helper passing `Some(1)` no longer compiles (checked).
- 5: the R4.8 check reads code (comments out), refuses `cfg` or `ignore` on the
  pin, with a self-test (`the_bounded_output_helper_check_catches_the_shapes_it_claims`).
- 7: `RemoveLock` carries `modified`, `read_at`, `bytes`, `device`, `inode`.
- 8: `DiscardLines` carries the `Selection`; the "every target from
  `confirmed.consequence()`" contract is a residual in `CLAUDE.md` and in
  `destructive-ops-reviewer`'s check 4.
- 9: `consequence.rs`, `confirm.rs` and every `CONFIRMATION_SURFACES` file route to
  `destructive-ops-reviewer` (its scope gate, `docs/qa-gate.md`, checklist item 7).
- 10: fixture strings no longer trip `.claude/hooks/qa-stop.sh`.
- 11: state.md's phase 01 row corrected.
- 12: paths and subjects escaped as git C-quotes a path, plus line separators and
  bidirectional controls, with tests.
- 14: the surface scan must have read `cairn-ui/src` and `cairn-app/src`.
- 15: each roster self-test fixture breaks one rule and asserts that rule's
  message; disabling the by-value check fails it (checked).
- 16: `consequence.rs` is held to no `Default`/`From`/`TryFrom`/`FromStr`/
  `Deserialize`/`Decode` and one impl block.
- 17, 31, 32: residuals stated in `CLAUDE.md` and the reviewers' definitions.
- 20: the prompt tests pin literal text per variant, sum several untracked files'
  sizes, use an asymmetric selection, and cover singular hour/day, GiB/TiB, the
  label's fallback and a lock from the future.
- 24: a file deleted in the working tree is named "deleted file restored".
- 28: `.github/workflows/enforcement-review.yml` and the mockup reworded.
- 29: `CONFIRMED_RECORD` is keyed on `Performed`'s inherent impl, not the name.
- 39: `Deserialize` and `Decode` have negative self-test cases.

Carried forward to the owning phases in state.md: 18 and 19 and 25 (phase 03), 21
(phase 09), 23 (phase 11). With the user: 6, 13, 34a.

Dismissed, with the adjudicator's reasons:

- 22 (an amend re-check needs the publication): the shape already carries
  `published`; comparing it is phase 09's re-check.
- 26 (a refused operation leaves no record): R1.6 binds `Performed`, which records
  completed operations; the refusal outcome is R1.4's, phase 03's.
- 27 (added and removed lines both counted as discarded): matches Fork.
- 30 (`<Consequence>::Variant { .. }` escapes `names_a_path_into`): a qualified
  path in a struct expression is unstable, and every variant is a struct variant.
- 33 (unchecked addition could overflow): the counts are bounded by an in-memory
  diff, and only the engine builds the values.
- 34b (merging discard files and delete untracked was unapproved): R1.5 lists them
  as one requirement.
- 35 (the copy doctest cannot flip): harmless; it fails for the reason it names.
- 36 (widening the helpers to every kind is not caught): the check requires the
  declaration inside `impl Invocation<Read>`.
- 37 (a `&mut self` helper on a write escapes the pin): the declared-once check
  catches any second declaration.
- 38 (a scratch copy reported `describe_destructive` off the roster): not
  reproducible on a fresh copy or with a fresh target directory.
- 24b (a collapsed untracked directory is not modelled): the status read expands
  it with `--untracked-files=all`.

## 2026-10-08 — phase 01, the seal (packet mode)

Built on `feature/staging-and-commit`. `Confirmed` lost `Clone`, carries an
engine-computed `Consequence` and the prompt rendered from it, and has one
constructor that takes the `Consequence` alone; `Performed::destructive` spends the
token by value and records its prompt (R1.6); the bounded-output helpers moved to
`impl Invocation<Read>` (R4.8, #45 item 2). The guard was rewritten around two
rosters and the forging and route checks; the root `CLAUDE.md` invariant names
them and their residuals (C22).

Decisions taken in the phase (none is a stopping rule):

- **One `DiscardFiles` variant for discarding files and deleting untracked files**,
  not two: Fork confirms a mixed selection in one dialog with one prompt (L8), and
  one prompt must be one `Confirmed`. Four variants, not the phase doc's five.
- **R4.8's "compile_fail pin" is an in-crate type-check pin**, not a doctest: a
  doctest cannot name the crate-private `Invocation`, and stable Rust has no
  in-crate `compile_fail`. `the_bounded_output_helpers_exist_on_a_read_alone`
  (`process/runner.rs`) is a function that compiles only while a write's calls of
  the helpers' names resolve to a fallback trait (answering `Absent`) and a read's
  to the real helpers; checked by hand that widening the impl to every kind fails
  to compile (E0308 on both write calls). The runner guard requires the pin and
  the helpers' place.
- **The placeholder `describe_destructive` is the destructive roster's one row**:
  the rule "every function naming `Confirmed` is rostered" covers it, so the roster
  is not empty, and the row proves the by-value check on real code until phase 03
  replaces it.
- **The `Consequence` cannot be spelled outside the engine and the model** in
  production code (a forged one would otherwise reach a surface's `by_user`); the
  render crates hold one and ask it for its words. Phase 09 adds a method for the
  amend dialog's decision rather than matching variants.
- **Test code may build tokens** (test modules, `#[cfg(test)]` module files,
  `tests/`): it cannot ship, and phase 03's operations need tokens in their tests.
  A helper under any other `cfg`, or named for tests in production code, fails.
- `Error::GitOutputTooLarge` lost `stranded_locks`, and
  `a_write_the_runner_ends_lists_the_locks_present` its ceiling half: no write can
  cross a ceiling now.
- Each of the six `compile_fail` doctests in `confirm.rs` was checked by hand to
  fail for its own reason (E0599 `clone`, E0382 move, E0308 text, E0451 private
  fields, E0599 `default`, E0277 `From<String>`), the scaffold compiling.

## 2026-10-07 — planned

Planned with `/feature-plan` on `feature/plan-staging-and-commit`. Seven evidence
records under `docs/research/staging-and-commit/`; the user locked L1-L21 over five
rounds, asking for mechanism detail on the patch engine, which became
`patch-mechanics-spike.md` (git 2.30.9 and 2.56.0 agree on every case). The brief
was split: stash and `.gitignore` go to a new packet 5b, `stash-and-ignore`.
Program O3 closed: no backup before a discard, as Fork.

Recon found, and the plan answers: `Confirmed` derives `Clone` and its
constructor is callable from any crate (R1); the runner already feeds stdin, so
no runner change is needed for `apply` or `commit -F -`; a status begun before a
write is drawn after it today (R4.4); a focused text field likely hides the
window's chords and held modifiers today (R7.1, C15 fails first).

Side effect, reported to the user at the time: the git-verbs recon agent's GPG
experiment reached the user's real `gpg-agent` through its socket, wrote two test
private keys into `~/.gnupg/private-keys-v1.d/` and restarted the agent twice; the
agent was refused removing them, and the user was given the commands. No further
GPG experiments were run.
