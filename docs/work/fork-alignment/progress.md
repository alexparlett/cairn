# Progress — fork-alignment

Running log, newest first. Each phase's dismissed QA findings are logged here with their
reasons, for the final QA phase to audit.

## 2026-10-20 — phase 08, context menus, keys, focus and dialogs (packet mode)

Committed onto `feature/fork-alignment` over phase 07's a88367f.

- **Fork observed first** (`/observe-fork`, fork-observer agent, shots `cp1`-`cp88`):
  `docs/research/ui-review/fork-copy-and-dialogs-2026-10-20.md`. VERIFIED: Copy Commit Info is
  `<7-char id> - <subject>` on one line, no trailing newline, the same from Ctrl+Shift+C; Copy
  Commit SHA and Ctrl+C the full id; with two rows selected the menu keeps the copies (beside range
  items Cairn lacks), each line a commit in list order, the upper first; Ctrl+C in the sidebar the
  ref's name past its namespace (`origin/main`, `v0.2`), Ctrl+Shift+C nothing; Copy Path the
  repository-relative path in the platform's separators, Copy Full Path joined to the root, the
  keys the same; the Commit tab's file menu holds Copy Path and Copy Full Path with no chords;
  a field's Ctrl+C copies its selection; dialogs #2B2B2B in a 1 px #3BACED border, square, title
  white semibold, body #CAC6C6, the default button #373737 bordered 1 px #0078D7 (not filled),
  Cancel bordered #525252, a disabled Create #343434 with #707070 text, focus a dashed inset
  rectangle only after Tab. NOT OBSERVED: whether Copy Commit Info carries a body (no fixture
  commit has one), CRLF or LF between lines, the Git Error dialog's look, which control a dialog
  focuses on open.
- **What shipped:**
  - C17 — every context menu holds R9.2's working items in Fork's groups with Fork's chords,
    through the one component: a commit row (`history::row_menu`: New Branch… — Copy Commit SHA,
    Copy Commit Info; a comparison the copies alone), a ref (Copy Branch/Tag Name now showing
    Ctrl+C), the gear, Unstaged (Stage, Discard Changes… — Stage All — Copy Path, Copy Full
    Path), Staged (Unstage — Unstage All — the copies, no discard), a commit's file in the
    Changes tab (with chords) and the Commit tab (without). Copy and Copy Full are two actions
    in `Scope::List`, each list choosing its own menu's item showing the chord
    (`menu::chosen_by`). Tests: **`every_context_menu_holds_its_rows_working_items`**,
    **`copy_and_copy_full_copy_what_the_focused_view_names`** (history, Unstaged, Staged),
    `the_copy_chord_copies_the_walked_refs_name` (branch, tag), `a_files_menu_and_the_copy_chords_choose_its_copies`,
    `a_files_right_press_opens_its_menu_without_opening_the_file`, `the_copy_texts_are_forks`,
    `a_menus_commits_are_in_the_lists_order`, `a_chord_chooses_the_item_that_shows_it_when_it_can_be_chosen`,
    `each_rows_menu_offers_forks_items_and_no_discard_where_none_is_allowed` (expectation changed
    to Fork's: no discard on the staged side, uv15).
  - C18 — new chords: Copy Ctrl+C / ⌘C, Copy Full Ctrl+Shift+C / ⇧⌘C (macOS unverified against
    Fork for macOS, as the other ⌘ columns of phases 02-08, marked in the table; ToggleDetails
    still none on macOS). Linux Discard now Delete then Backspace, so menus show Fork's
    "Delete"; Linux chord names in Fork's menus' order, Ctrl+Shift+Alt+S. The pin holds: no
    bare chord in `Scope::List`, a new `copy_twice` example; a text field's Ctrl+C is the
    field's (`a_text_field_hands_on_the_windows_chords_and_keeps_its_own_keys`). No chord
    collides with a desktop's or a field's binding on Linux.
  - C19 — one focus treatment, `style::focus`: a 1 px ring in Fork's blue on keyboard focus, drawn
    inset in a bordered button; every focusable control applies it (lists, bar buttons, Expand
    All, check boxes, radios, the commit button, Show files, the diff view, every push button).
    Ctrl+1 leaves focus in Unstaged, whether Local Changes was shown or not, a press on the
    sidebar's row leaving focus alone (uw38, uw6). Tests: **`every_tab_stop_shows_focus`** (All
    Commits and Local Changes), `every_dialogs_tab_stop_shows_focus`,
    **`ctrl_1_opens_local_changes_with_unstaged_focused`**.
  - C20 — every dialog in Fork's frame (`dialogs::frame`), its title 14 px bold white, its primary
    Fork's blue-bordered default button (`PushButton`), About's Close the default, Git Error with
    none; the worker-lost notice reworded. Tests: **`every_dialogs_primary_button_is_forks_default_in_forks_frame`**,
    **`no_message_asks_to_reopen_the_repository`**, `about_cairn_names_the_version_and_the_git` (kept).
  - C21 menus half — **`a_long_menu_builds_one_viewport`** (1,000 and 100,000 items),
    `a_long_menus_reason_and_submenu_are_drawn_beside_its_rows`.
  - Deferred items closed: dialog titles off Freya's 18 px with Fork's border; `MENU_ITEM` read;
    Ctrl+1 into Unstaged; menus virtualized; the macOS columns; the sidebar's Ctrl+C; Expand All
    and the 1 px ring now inside `every_tab_stop_shows_focus`'s walk. A greyed bar button keeps
    its ring if it held keyboard focus as it greyed (the ring follows focus, not the state);
    probed in the code, not in a test.
- **Evidence over the PRD's paraphrase** (qa-checklist item 1): R11.1/C20 say the primary is
  "filled"; Fork's default button is bordered in blue over the button's fill (cp60-cp76) — built
  as Fork's, the test named for what it checks, batched below. R9.2 lists chords on the Commit
  tab's file menu; Fork's shows none (cp52-cp59) — built as Fork's. R9.2 lists no chord beside
  Unstage All; Fork's shows Ctrl+Shift+Alt+S (uv15) — built as Fork's. R9.2 keeps "Discard" on the
  staged menu implicitly ("as today"); Fork's staged menu has none (uv15) — removed. Fork's
  two-row menu holds range items Cairn lacks; Cairn keeps the copies alone.
- **Real window — NOT RUN in the compositor** (qa-checklist items 6-7): Hyprland's command socket
  is still missing (`/run/user/1000/hypr/<instance>/` holds only `hyprland.log`). **Substitute:**
  the real `window()` with the real worker over the fixture (built on the host by the recipe,
  the upstreams set by fetching into the bare remote rather than pushing), Fork Dark's launch
  theme, rendered headlessly in a release build at 1024×768 and 880×620 (a scratch `#[ignore]`
  test, not committed): All Commits, a commit row's menu, Create Branch (and five Tab stops with
  keyboard navigation set), Local Changes, a file's menu, the discard confirmation. Compared with
  uv22, uv14, uw10 and cp61: the menus' items, groups, rules and chords, the dialogs' square frame
  in its blue border, the bold white title and the blue-bordered default button match; the
  focus rings show at each stop. Differences: Fork's logo and ✕ not drawn (a `ui.md` row);
  Fork's buttons squarer; the subtitle stood left of the title (fixed, 95d9243); a field's
  keyboard focus is the toolkit's 2 px border and ring, not Fork's 1 px (`docs/systems/style.md`).
  It cannot show the compositor's fonts, hover, tooltips or real input.
- **QA.** Fresh `qa-checklist`, `responsiveness-reviewer`, `test-coverage-auditor`,
  `gate-integrity-reviewer` and `destructive-ops-reviewer` in the foreground; a fresh `qa-confirm`
  adjudicated 21 raw findings (Q2 and T4 merged):
  - Confirmed and fixed: D1 (critical by the letter: `PushButton` compared equal on what it drew,
    so Create Branch's button kept the press of the choice it was first drawn with — a dead press
    after switching Don't change and Discard; the window's re-checks refused every stale token —
    482ac9c, `the_button_presses_for_the_choice_shown_now`); Q2/T4, Q3, Q4, R1 (CLAUDE.md's
    residual list names the comparison's scans), G1 (the guard requires `copy_twice`,
    `heading_press`, `shift_d`), T1, T2, T3 (Copy Full's chord; the no-current case not added),
    T5, T6 (doc), T7, T8, T9 (29bd246, 4d012cc). G2 was escalated as a policy question (a test
    helper compiled into cairn-app by `#[path]` from cairn-ui, outside cairn-app's scanned files);
    resolved without a policy change by giving cairn-app its own copy. A fresh `qa-confirm`
    verified all 16 and the mutations it ran each failed their test; its one new cosmetic note
    (a doc line past 100 columns) fixed.
  - Dismissed, with reasons: R2 (the panel compares groups by value and rebuilds an open
    submenu's layout per highlight — proportional to a menu's items, and every production menu
    is a short fixed list); R3 (Copy Full Path's joins and the clipboard write once per press,
    inherited and within the per-press budget the root CLAUDE.md states); D2 (the confirmation's
    button cannot go stale: one fixed consequence per serial, and greyed-to-ready re-renders it);
    D3 (a double press's second token is dropped by `discard_confirmed`'s `writing` check).

**Batched for the user — PENDING, not approved:**
1. **C20's "filled" primary is built as Fork's bordered default** (cp60-cp76): `#373737` in a
   1 px `#0078D7` border, other answers `#525252`, Git Error with no default as Fork's. The PRD's
   C20 names `every_dialogs_primary_button_is_filled`; the test is
   `every_dialogs_primary_button_is_forks_default_in_forks_frame`. Options: (a) keep Fork's look
   and amend R11.1/C20 at teardown (recommended); (b) fill the primary as the PRD says.
2. **The Commit tab's file menu shows no chord and hears none; Unstage All shows its chord; the
   staged menu has no discard** — each Fork's evidence over R9.2's table.
3. **New chords:** Copy Ctrl+C / ⌘C and Copy Full Ctrl+Shift+C / ⇧⌘C in a list-only scope; the ⌘
   column of every action phases 02-08 added stays unverified against Fork for macOS.
4. **Discard's Linux chords reordered** (Delete first) and **Linux chord names spelled
   Ctrl+Shift+Alt** — what menus and tooltips show changes; no binding changes.
5. **Focus:** a ring at every non-list stop and on lists (Fork draws none on lists; R10.2's ring),
   inset in buttons; a field's ring is the toolkit's 2 px.
6. **New metrics and tones:** `MENU_LONG_WIDTH` 360, `DIALOG_BUTTON_PADDING_Y/X` 6/14,
   `COMPACT_BUTTON_PADDING_Y/X` 3/6, `focus::RING_INSET` 2; `DIALOG_TITLE` #FFFFFF,
   `DIALOG_TEXT` #CAC6C6 (cp60).
7. **Every Freya `Button` replaced by one `PushButton`** (dialogs, Load Diff, Remove index.lock…,
   the popover's buttons, the comparison's swap) and every dialog by one frame.
8. **The real-window check** for this phase is owed, as phases 05-07's, or the headless release
   render accepted in its place.

**Deferred:** whether Copy Commit Info carries a body (no fixture commit has one); the
Changes tab's chord with no current file untested; Fork's dialog logo and ✕; a stop dropped
from the window's focus walk while its count stays above the floor is not caught. macOS
unverified.

## 2026-10-20 — phase 07, Local Changes and the commit box (packet mode)

Committed onto `feature/fork-alignment` over phase 06's 7bbbc5c.

- **What shipped** (R8, C16):
  - The bar over both lists (R8.1): Fork's 17 px bar and its rule (`metrics::LISTS_BAR`), the
    filter a magnifier and a field with no placeholder, and at its right Stage All's ⇊, the eye
    and View Mode, whose menu holds "View as List" alone, ticked, choosing it doing nothing
    (`crates/cairn-ui/src/local_changes/bar.rs`). Stage All reports the same intent the
    Unstaged heading's chevron did (`ListIntent::Act(Unstaged, StageOrUnstageAll)`), greyed
    while Unstaged shows no row. Tests: **`local_changes_bar_and_headings_are_forks`** (C16),
    `the_bars_controls_report_their_presses_and_say_what_they_are`,
    `stage_all_is_greyed_while_unstaged_shows_no_row`.
  - The headings (R8.2): bold at 14 px on Fork's 32 px band and rule, the bordered Stage or
    Unstage button 22 px tall, greyed while nothing is selected in its list; Unstaged's chevron
    gone, Staged's ⇈ Unstage All kept. Test: `local_changes_bar_and_headings_are_forks`.
  - The commit box (R8.3): one bordered field of two parts (`text_field::field_part`), the
    subject 22 px and bold once typed, the counter shown only then, the Recent Commit Messages
    glyph at the subject's right end, a rule, the description one line while empty and growing
    a line at a time; Amend at the left; the split button — the caption, then its ⌄, a segment
    of its own, always greyed with "Comes with pull and push.", no Tab stop, opening nothing.
    Tests: **`the_commit_box_is_one_field_and_a_split_button`** (C16, both states),
    `the_counter_and_the_bold_subject_appear_once_the_subject_has_text` (and red past 70).
  - Rows 20 px (R8.4, `LOCAL_CHANGES_ROW_HEIGHT`). The diff bar's button became
    `src/bar_button.rs`, shared with the new bar; the diff bar's tests are unchanged and green.
- **Staging routes**: every suite passes; expectations changed only where Stage All's chevron
  moved into the bar (FS1 §7, uv12a): `the_buttons_stage_and_unstage_the_selection_or_with_the_press_held_everything`
  (Stage All pressed in the bar, Unstage All on Staged's heading, by accessible name),
  `a_staged_selection_unstages_with_its_renames_source_and_all_takes_the_list` and
  `stage_all_and_unstage_all_take_the_rows_the_filter_shows` (the bar's Stage All by name,
  the writes asserted as before). Geometry only: the drag test's edge point moved clear of the
  taller heading's splitter; the filter's click point and the 50,000-path twins' viewport read
  the bar and row constants; the window's highlighted-row finder reads the 20 px row; Recent
  Commit Messages is pressed by name (`recent_messages_fill_both_fields_from_the_menu_and_the_arrows`).
  No discard route's wiring changed, so no destructive-ops reviewer.
- **Fork observed** (`/observe-fork`, fork-observer agent, shots `lc6`-`lc33`):
  `docs/research/ui-review/fork-local-changes-2026-10-20.md`. VERIFIED: the eye opens Quick Look
  — a window over Fork's showing the file's diff side by side, "Space bar: close, Up/Down: move
  file selection" — and leaves the main diff as it was; Space on a focused list does the same;
  View Mode's glyph does not change with the mode and its tooltip stays "View Mode"; with
  nothing left to stage the bar's chevron turns into ⌃⌃ "Unstage All"; the description is one
  22 px line while empty and grows upward about 15 px a line; the selected band is inset 4 px
  each side, rounded ~4 px, adjacent bands merge. NOT OBSERVED: whether Quick Look remembers its
  own side-by-side toggle.
- **Evidence over the PRD's paraphrase** (qa-checklist item 1): R8.1 lists "the eye 'Show diff
  side-by-side'" without saying what it does; built first as a toggle of the shared side-by-side
  setting, which the observation showed is not Fork's (b6fdcc5) — now greyed, "Comes with Quick
  Look." (#35), batched below. R8.1's eye tooltip drops Fork's "(space bar)": Cairn binds no
  key to it. Fork's typed subject is bold (uw36) and its description grows from one line (lc28):
  both built, though R8.3 says "the rest of the box is unchanged". Fork's Stage All turning into
  Unstage All is not built (R8.1 names only Stage All): greyed instead, a `ui.md` row, batched.
- **Real window — NOT RUN in the compositor** (qa-checklist items 5-7): Hyprland's command
  socket is still missing (`.socket.sock` absent; `.socket2.sock` and `.hyprsunset.sock` only),
  so the window cannot be sized and `wtype` would type into the user's desktop. **Substitute:**
  the real `window()` with the real worker over the fixture (`fx/t`, `fixture-recipe.md`'s
  state), Fork Dark's launch theme, rendered headlessly in a release build at 1024×768 and
  880×620 (a scratch `#[ignore]` test in `window_check.rs`, not committed), Local Changes empty
  and with a subject and three description lines. Compared with uw6, uw6a, uw36a, uv12a-c and
  the lc shots: the bar (17 px + rule, measured), the controls' order and place, the headings
  (32 px + rule), the bordered buttons, the 20 px rows, the field (subject 22 px, rule, one-line
  description), the counter "49" before the glyph, the bold subject, the split button and its ⌄
  match. Differences seen: the splitters' 4 px handles (an existing `ui.md` row); the selection
  band full width and square where Fork's is inset 4 px and rounded (R8.4 keeps the selection;
  deferred); the badges coloured letters, no document glyph, paths in IBM Plex Mono (R1.6; the
  badges deferred since phase 06); Lucide glyphs for Fork's artwork (L4; View Mode's notched
  square drawn as Lucide's list); the description's lines 21 px apart to Fork's ~15 (its
  monospace line). It cannot show the compositor's fonts, hover, tooltips, focus rings or input.
- **QA.** Fresh `qa-checklist`, `responsiveness-reviewer` and `test-coverage-auditor` in the
  foreground (no guard, script, CI or discard-route change); a fresh `qa-confirm` adjudicated 12
  raw findings (Q-F1 and T1 merged) into 4 confirmed, 7 dismissed:
  - Confirmed and fixed: Q-F1/T1 (the eye's window wiring untested — dissolved: the eye is greyed
    and has no wiring, b6fdcc5); Q-F3 (a find-by-name helper in three test binaries — now
    `crates/cairn-ui/tests/support/named.rs`); T2 (the ⌄'s greyed fill and no Tab stop
    unchecked); T3 (the drawn counter's red past 70 unchecked; 04a2b7d). A fresh `qa-confirm`
    verified all four; the mutations it named were run here and each failed its test (the eye
    enabled, the ⌄ focusable, the ⌄ filled as a button, the counter grey past 70). It also
    noted the empty description stood 31 px, not one line: fixed and pinned (0917c69).
  - Dismissed, with reasons: Q-F2 (Stage All as a focusable glyph button taking focus from the
    list on a press — in the vendored Freya a press focuses nothing that does not request it,
    `accessibility/focus.rs`, so focus stays on the list); Q-F4 (survey shot ids in comments —
    they cite the Fork behaviour matched, which the comment rule allows; precedent in the base
    tree; the guard passes); Q-item4 (the Input builders — checked against the vendored source,
    they behave as used); Q-item6 (a transient failure — caused by the auditor's concurrent
    mutations; clean reruns pass); T4 (heights checked against the metric constants — R8 gives a
    number only for the rows, pinned as a literal in `tests/style.rs`; the surveyed constants
    are batched below); T5 (the heading size read from `metrics::HEADING` — R8.2 asks for bold,
    which is pinned; the size is older and shared); T6 (the eye's lit tone through one toggle
    field — by design, and the eye is now greyed). The responsiveness reviewer found nothing.

**Batched for the user — PENDING, not approved:**
1. **The eye is greyed, "Comes with Quick Look."** Fork's eye opens Quick Look (lc9), not a
   side-by-side toggle; R8.1 names the eye but not what it does. Options: (a) greyed until #35
   (built, recommended: L2's form, nothing claims to be Fork's that is not); (b) the eye toggles
   the shared side-by-side setting (the first build, a `ui.md` row as a deviation); (c) the eye
   absent until Quick Look.
2. **Stage All greys when nothing is left to stage**, where Fork's turns into ⌃⌃ Unstage All
   (lc21). Recorded in `ui.md`'s Unstage All row. Option (b): flip it as Fork does (a second
   meaning on one control, and a staging route added).
3. **The typed subject bold and the description growing from one line**, built on Fork's
   evidence though R8.3 says the rest of the box is unchanged; the description's old 64 px floor
   is gone.
4. **New metrics:** `LISTS_BAR` 18, `LIST_HEADING` 33, `LIST_HEADING_BUTTON` 22,
   `LIST_HEADING_BUTTON_PADDING` 12, `COMMIT_BOX_INSET` 15, `COMMIT_SUBJECT` 22,
   `COMMIT_BUTTON` 22, `COMMIT_CHEVRON` 16. No new tones.
5. **Lucide glyphs chosen:** the eye `eye`, View Mode `list` (Fork's is a notched square that
   never changes with the mode), Recent Commit Messages `text-align-justify`.
6. **The real-window check** for this phase is owed, as phases 05's and 06's, or the headless
   release render accepted in its place.

**Deferred:** the selection band inset 4 px and rounded, adjacent bands merged (lc8, lc23); the
badges as Fork's filled squares and a document glyph on each row (with phase 06's); Quick Look
(#35); the tree, combined list, Hide Untracked Files and Show Ignored Files (#36, #70, #62);
Commit Options and Commit and Push in the ⌄ (remote-sync); the description's line pitch (Fork's
~15 px, Cairn's monospace 21 px). macOS unverified.

## 2026-10-20 — phase 06, the detail pane and the diff bar (packet mode)

Committed onto `feature/fork-alignment` over phase 05's d9265cd.

- **Fork observed first** (`/observe-fork`, fork-observer agent, Fork 2.23.2.0 Dark, shots `cm*`):
  `docs/research/ui-review/fork-committer-2026-10-20.md`. VERIFIED: a COMMITTER block is drawn for
  a committer differing by name alone, address alone, an hour or one second later, and not for the
  same instant written at another offset nor for the same committer — so Fork compares name,
  address and the instant to the second; the block stands beside AUTHOR on the same rows, its
  42 px avatar the committer's, its date in the machine's offset; the Changes summary and the
  history row are the author's; the bar's seven tooltips verbatim; a toggled-on glyph is #419BF9
  with nothing behind it; Show entire file greys both line buttons, Decrease never greys (at one
  line a press does nothing); hover is a #242424 16 px square; buttons repeat every 20 px; the bar
  is 17 px between rules. NOT OBSERVED: whether Fork compares the instant or the date as shown
  (the fixture cannot tell them apart), the hover look of a toggled button, Fork's starting
  context.
- **What shipped** (R7, C15):
  - The tab strip (R7.1): Fork's 31 px strip between rules (`metrics::DETAIL_STRIP`), 96 px tabs
    from the pane's edge with the full-width underline, the hide-details icon at the right
    (phase 03's). Test: `the_strip_reports_its_tabs_and_its_collapse` (kept).
  - The Commit tab (R7.2): AUTHOR caps label over the author's block — 42 px avatar
    (`cairn_ui::AvatarSize::Detail`, `avatar_of`), bold name and the address, the date with its
    seconds and offset — the COMMITTER block beside it only where it differs
    (`cairn_ui::committer_differs`), REFS absent without refs, SHA, PARENTS (standing empty for a
    root commit, as Fork's, cm13), the message and the paths in the values' column, Expand All its
    caption alone, a document glyph on each file row. Every row is the diff's 17 px
    (`COMMIT_TAB_ROW_HEIGHT`), so a file opened in place draws its diff at the diff's row height;
    the avatar spans three rows as overlapping bands. Tests: **`the_committer_is_shown_only_where_it_differs`**
    (C15), `the_people_block_draws_each_persons_avatar_whole_across_its_rows`,
    `the_message_and_the_paths_stand_in_the_values_column`, and the twins
    `only_a_viewport_of_rows_is_built_however_many_files_are_open` and
    `only_a_viewport_of_files_is_built_however_many_the_commit_touched`, held at the new pitch
    (the expansion twin's `rows_built` now counts full-width one-pitch row rects, since the rows
    under the avatar and the rules hold no text; its end-room expectation read from
    `with_end_room`). The pitch change did not break `cairn_ui::Expansion`'s placement — the
    phase's stopping rule — since every row moved to the one pitch together.
  - The Changes summary (R7.3): the author's 18 px avatar, the name, the id's first nine
    characters and the short date in the secondary tone, the subject bold, on Fork's 23 px strip
    (`metrics::DETAIL_SUMMARY`). Test: **`the_changes_summary_opens_with_the_avatar`** (C15),
    `the_summary_is_author_short_id_date_and_subject` (expectation changed to Fork's nine
    characters and short date, uw5/cm14). The file list's filter now stands in a bar level with
    the diff's (a magnifier, Fork's tone and rule), keeping Cairn's "Filter" placeholder.
  - The diff bar (R7.4): Fork's order, the path centred with its document glyph, Lucide glyphs
    (`chevron-up`, `chevron-down`, `space`, `list-minus`, `list-plus`,
    `list-chevrons-up-down` — the three list glyphs mirrored so the sign stands left as Fork's —
    `columns-2`), Fork's tooltips verbatim with the chord read from the table
    (`cairn_ui::bar_tooltip`, `HeaderAction::action`, which `shortcuts::of_header` now
    delegates to), the toggled glyph #419BF9, the hover square, the bar 17 px + rule
    (`metrics::DIFF_BAR`). `toggle_glyphs.rs` retired: every glyph it drew is Lucide's (the
    Commit tab's chevrons, the comparison's swap `arrow-up-down`, Stage All / Unstage All
    `chevrons-down` / `chevrons-up`). Tests: **`the_diff_bar_is_forks_with_its_tooltips`** (C15),
    `every_button_is_a_named_glyph_and_an_active_toggle_is_lit_in_forks_blue`,
    `each_bar_button_draws_its_forks_glyph`,
    `the_bar_reports_each_button_and_greys_the_line_buttons_for_the_entire_file` (expectation
    changed to Fork's: Decrease stays pressable at one line, cm38),
    `a_hovered_bar_button_stands_on_its_square_unless_it_is_disabled`,
    `every_bar_button_that_can_be_pressed_is_a_tab_stop`, `each_button_does_its_own_action`.
  - The bands and hunk header (R7.5): already Fork Dark's since phase 01
    (`each_tint_is_forks_measured_value_on_forks_ground`); nothing left.
- **Evidence over the PRD's paraphrase** (qa-checklist item 1): R7.2 leaves to the observation
  which fields count — built as observed (name, address, instant to the second, not offset); it
  does not say where the block stands (beside AUTHOR, cm7z) nor that the committer's date is the
  machine's offset (Cairn draws both dates in the zone the window is handed, which is the same
  thing); R7.4 does not say Decrease never greys (cm38) — built as Fork's; FS1 §6 calls the
  summary's id "ten characters" where its own quoted `f93015d03` and cm14's `1c63ef316` are nine —
  built nine; R7.3 says nothing of the summary's date form, Fork's is the history's
  `16 Oct 2026 12:12` — built so. Two earlier user decisions recorded in `docs/systems/diff.md`
  are superseded by R7.2 as written: the Commit tab's in-place pitch kept at 24 px (2026-10-04,
  which named "the whole Commit tab at 17 px" as the alternative) and the committer drawn always.
  Both are in the batch below.
- **Committer column x** (QA Q2): Cairn's committer avatar starts at the row's middle + 20 px,
  Fork's at middle + 17 at the one pane width measured; left as is.
- **Real window — NOT RUN in the compositor** (qa-checklist items 5-7). Hyprland's command socket
  is still missing (`.socket.sock` absent under `/run/user/1000/hypr/<instance>/`; `hyprctl`
  cannot connect). `grim` captures and the session is unlocked, but with no `hyprctl` the window
  cannot be sized to 1024×768 or 880×620, and `wtype` would type into whatever the user has
  focused on the desktop they were using, so the release build was not launched there.
  **Substitute:** the real `window()` with the real worker over the fixture
  (`fixture-recipe.md`, built on the host, its "Local only commit" given Grace Hopper as committer
  so the COMMITTER block shows), Fork Dark's launch theme, rendered headlessly in a release build
  at 1024×768 and 880×620 (a scratch `#[ignore]` test, not committed), the merge, the local-only
  commit and the root commit each in the Commit and Changes tabs. Compared with uv2f, uv7b, uv9z,
  uv10z, uw5a and cm7z: the strip, tabs and underline, the summary line, the bars (both 17 px,
  level), the glyphs and their order, the author and committer blocks and their columns, REFS,
  SHA, PARENTS, the message and file columns match. Differences seen: Lucide's glyphs and Fork's
  own artwork (L4; Fork's −≡ has four lines to Lucide's three); the header's and file rows' 17 px
  pitch against Fork's ~20 and 22 (batched); the file badges are coloured letters where Fork draws
  filled squares and the paths IBM Plex Mono where Fork's are its UI face (R1.6 keeps the mono);
  Fork's ⌄ beside an address and its pop-out icon not drawn; Fork's Changes list rows ~20 px to
  Cairn's 24 (no requirement of this phase). It cannot show the compositor's fonts, real hover,
  tooltips, focus rings or input.
- **QA.** Fresh `qa-checklist`, `responsiveness-reviewer` and `test-coverage-auditor` in the
  foreground (no guard, script or CI change, so no gate-integrity reviewer); a fresh `qa-confirm`
  adjudicated 20 raw findings into 11 confirmed (4 merged), 1 escalated, 4 dismissed:
  - Confirmed and fixed: T1 (the lit-glyph check accepted the accent's blue — now held to Fork's
    by its red), Q5/T2/T10/T11 merged (the Commit tab's avatar unpinned — bands, offset, overlap,
    size, weight, the author's own), T3 (the committer's own date), T4 (the greyed line buttons'
    look and disabled flag), T5 (a test doc claiming to catch a typed chord, which is the
    modifier guard's), T6/Q4 merged (the glyph per button and its mirroring — pixel-exact against
    the style module's icon), T7 (the bar's action mapping), T8 (the values' column), T9 (the hover
    square). Q2 and Q3: recorded above. Glyph mutations (Increase unmirrored, Decrease given
    Increase's glyph) were run and fail `each_bar_button_draws_its_forks_glyph`.
  - Escalated, decided and batched: Q1 — drawn as plain shapes the bar's five chordless toggles
    and Expand All had lost keyboard reach (they were the toolkit's focusable buttons before this
    phase); Fork's Tab cycle (FS2 §3) reaches neither. Decided: kept as Tab stops (0d637de), a
    `docs/design/ui.md` row — batched for the user.
  - A fresh `qa-confirm` verified the fixes by one mutation each (10 of 10 killed). Left open, for
    phase 08's focus work: Expand All's Tab stop and the bar's 1 px focus ring have no test, and
    a greyed bar button holding keyboard focus may keep its ring (unprobed).
  - Dismissed, with reasons: Q6 (the exhaustive-read invariant over `DiffContent` and the diff's
    row enums scopes production `src/` and leaves `tests/` out by design); R1, R2, R3 (the
    responsiveness reviewer's own notes, no defect claimed: SVG rasters cached per icon, size and
    tone; the avatar's bands bounded per built row; `bar_tooltip` constant per button render).

**Batched for the user — PENDING, not approved:**
1. **The Commit tab's one 17 px pitch.** R7.2/C15's "an opened file's diff at the diff's row
   height" in a single fixed-pitch virtualized list moves every row to 17 px — header rows (Fork
   ~20) and file rows (Fork 22) too. It reverses the 2026-10-04 decision that kept 24 px. Options:
   (a) keep (recommended: C15 as written, the virtualization invariant intact, a ui.md row);
   (b) back to 24 px everywhere, C15 amended.
2. **COMMITTER only where it differs** reverses the earlier "both always, as git's `fuller`"
   decision; R7.2 asks for it. Ratify or not.
3. **Q1: the bar's buttons and Expand All stay Tab stops**, against Fork's three-stop Tab cycle
   (a ui.md row). Option (b): Fork's, the five chordless toggles reached by pointer alone.
4. **Decrease is never greyed at one line**, as Fork's (cm38); Cairn greyed it before.
5. **Two small deviations as ui.md rows:** the Changes list's filter keeps its "Filter"
   placeholder (Fork's is blank); Fork's ⌄ beside an address is not drawn.
6. **New metrics:** `DETAIL_STRIP` 33, `DETAIL_TAB` 96, `DETAIL_SUMMARY` 24, `DIFF_BAR` 18,
   `DIFF_BAR_ICON` 14, `DIFF_BAR_BUTTON` 16, `DIFF_BAR_GAP` 4, `DETAIL_AVATAR` 42,
   `DETAIL_AVATAR_TEXT` 20, `SUMMARY_AVATAR` 18, `SUMMARY_AVATAR_TEXT` 9, `COMMIT_TAB_INSET` 20,
   `COMMIT_TAB_CAPTIONS` 62, `COMMIT_TAB_GAP` 16; `DETAIL_SUBJECT` 15 → 14; `SWAP_GLYPH` and
   `ENTIRE_FILE_GLYPH` removed. **New tones:** `DIFF_BAR_ICON` #CFCFCF, `DIFF_BAR_ON` #419BF9,
   `DIFF_BAR_HOVER` #242424.
7. **Lucide glyphs chosen** for the bar (above), the comparison's swap (`arrow-up-down`) and Stage
   All / Unstage All (`chevrons-down` / `chevrons-up`); the stopping rule (no close Lucide
   equivalent) did not trigger — each reads as Fork's, the list glyphs mirrored to put the sign
   left. Fork's swap glyph was not observed.
8. **The real-window check** (qa items 5-7) for this phase is owed, as phase 05's, or the
   headless release render accepted in its place.

**Deferred:** the file rows' badges as Fork's filled squares (Commit tab and Changes list) and the
Changes list's ~20 px rows; Fork's ⌄ menu beside an address; the pop-out window; File Tree (#31),
Quick Look (#35), hidden symbols (#33), word wrap (#34). macOS unverified.

## 2026-10-20 — phase 05, the sidebar (packet mode)

Committed onto `feature/fork-alignment` over phase 04's 4eea6ae.

- **Fork observed first** (`/observe-fork`, fork-observer agent, Fork 2.23.2.0 Dark, shots `sb*`):
  `docs/research/ui-review/fork-sidebar-2026-10-20.md`. VERIFIED: tags newest first by the
  **tagged commit's date** (m-light, z-annot-old, b-light, a-annot-new — not name, not tagger
  date, not creation order); counts ahead then behind, `1↑ 2↓`, body tone; a gone upstream draws
  no count but a yellow (#FFC501) warning on its branch glyph's bottom right; a stash drops
  `On <branch>: ` and keeps git's default `WIP on …` whole; ↑/↓ walk every heading, folder, ref
  and stash, never reaching the filter, All Commits or Local Changes; the history follows each ref
  walked to at once; Return does nothing anywhere observed; ←/→ are a tree's; the band is rounded
  (~3 px), 22 px, inset ~5 px each side; "Create branch at:" names the ref with its glyph; Copy
  Branch Name copies `origin/main` for a remote. NOT OBSERVED: Return on Local Changes / All
  Commits (unreachable by arrows), a tie between tags of one date beyond one case, a local branch
  in a folder. The tag order is by a date, so the phase's stopping rule did not trigger.
- **The sidebar** (R6, C14): header with the repository's name and the gear's Copy Repository Path
  (`Update::Opened { path }`, `View::repository_path`, `sidebar::menu`); Local Changes and All
  Commits with Lucide icons (`file-diff`, `file-stack`) in Fork's rounded band; the filter's
  magnifier and #292929 fill; Tags closed as it opens (`Disclosure`'s default,
  `SidebarSection::opens_closed`); 12 px a level with a chevron slot so a leaf's icon lines up with
  its folders'; Lucide glyphs (`SidebarGlyph`), a remote's folder its cloud; tags by their commits'
  dates (`Ref::commit_time`, read by the refs query through git's substitutions); stash wording
  (`stash_text`); counts `1↑ 2↓` (`counts_text`, was behind-first unspaced); the sidebar 300 px
  (`metrics::SIDEBAR_WIDTH`, was 240). `RefGlyph` lost the five sidebar-only shapes.
- **Keys** (R10.3, the deferred item): Fork's tree walk (`cairn_ui::sidebar::keys`), a ref or
  stash walked to chosen at once as a press chooses it.
- **Ref menus** (R9.2, R9.3): a right press on a ref selects its commit and opens New Branch…
  (Ctrl+Shift+B shown) then Copy Branch Name / Copy Tag Name; greyed New Branch… on a tag naming
  no commit; nothing on a stash, heading or folder.
- **The menu component** (phase 02's) fixed: the release render showed a two-group menu stretched
  across the window by its rule, and once fixed, its chord drawn past its edge — a row's
  space-between alignment set the chord against the room the window offered before the menu was
  measured. The rule is now the panel's own fill-minimum child, and an unseen copy of the chord
  (`tones::UNSEEN`) holds its room while the chord is placed against the row's right edge.
  Every two-group context menu phase 08 adds relies on it.
- Acceptance criteria and their tests:
  - C14: `the_sidebar_is_forks`, `each_entry_draws_its_kinds_glyph`,
    `a_right_press_on_a_ref_selects_its_commit` (cairn-ui) and
    `a_right_press_on_a_ref_selects_its_commit_in_the_history` (cairn-app),
    `the_sidebar_walks_by_key_as_forks_tree`, `the_walk_stays_on_a_folder_opened_or_closed_by_key`,
    `the_arrows_walk_the_sidebar_and_select_each_refs_commit` (cairn-app),
    `tags_are_listed_newest_first_by_their_commits_date`,
    `tags_and_every_folder_start_closed_and_the_other_sections_open` (cairn-model),
    `a_tag_is_dated_by_its_commit_never_by_its_tagger`,
    `a_tags_date_is_read_through_a_replacement_as_git_reads_it` (cairn-git, real git).
    **Except** C14's last clause, "Return opens Local Changes or All Commits", which Fork's
    evidence contradicts — see the batch below; the test is named for what it decides.
  - C21 (sidebar half): `a_sidebar_of_50000_refs_builds_one_viewport` and
    `a_folder_of_10000_branches_open_builds_one_viewport`, kept, each now requiring 22 px row pitch.
  - Model changes with tests in the same commit: `Ref::commit_time` (edafd00, with the refs
    oracle and the tagger-date test), `Disclosure`'s default and the tag order (86f9179).
- Tests whose expectations changed: the sidebar suites (counts form, Tags closed, stash wording,
  glyph kinds); the cairn-git poll count and cost (one poll and one object read per tag); the app's
  tag-on-a-tree test opens Tags first.
- Measured (`docs/research/refs-and-status/measured.md`, "A tag's commit date"): 10,000 tags on
  commits of their own, 19.8 ms with the dates against 4.1 ms without — ~1.6 µs a tag on the
  repository thread every refresh. Reusing the last snapshot's dates is a possible follow-up.

**Real window — NOT RUN** (qa-checklist items 5, 6, 7 owed). After the host's reboot Hyprland's
command socket (`.socket.sock`) is missing and the session is locked, so `hyprctl`,
`send_shortcut` and `grim -T` had nothing to drive or capture, and `wtype` would type into the
lock screen; the release build was not run in a real window. **Partial substitute:** the real
`window()` with the real worker over the fixture (`fixture-recipe.md`, built on the host), Fork
Dark's launch theme installed, rendered headlessly in a release build with freya-testing's
`render_to_file` at 1024×768 and 880×620 (a scratch `#[ignore]` test, not committed), pressing,
right-pressing and keying by the runner's events. It showed: the sidebar beside uw27/uv4z — rows,
order, indents, Tags closed, `1↑`, the stash's wording, the band; ↓ from `main` landing on the
Remotes heading in blue; the ref menu (which is how the menu component's two bugs were found and
then seen fixed); at 880×620 every history column kept, the subject ~160 px (the 300 px sidebar
takes 60 px more than phase 03's 240). It cannot show the compositor's fonts, a focus ring, hover,
tooltips, or real keyboard and pointer input. Differences seen against FS1/FS2: Fork's own icon
set against Lucide (L4); the local-only glyph's shape (a ui.md row); Fork's refs/search switch and
divider not drawn (R6.3); Fork's filter field less rounded.

**QA.** Fresh `qa-checklist`, `responsiveness-reviewer` and `test-coverage-auditor` in the
foreground (no guard, script or CI change, so no gate-integrity reviewer); `qa-confirm`
adjudicated 16 raw findings (two pairs merged), and a fresh `qa-confirm` verified the fixes:
- Confirmed and fixed: Q3/R2 (the key's row search copied a name per row and was unstated —
  `SidebarTarget::is`, a CLAUDE.md residual line), Q4/R1 (tag dates unmeasured — measured),
  Q8's docs half (sidebar.md cited R11/R10.2 for things they do not ask), T1's test half (the
  sidebar test could read the chord's unseen copy), T2 (a heading's first level unpinned), T3
  (the walk kept across a toggle untested, and it trusted a stale index over later layouts —
  `Walk` now keeps its row), T4 (greyed New Branch… untested), T5 (a remote ref's menu
  untested), T6 (a tag's date through a replacement untested). T2, T3 and T4 mutation-checked.
- Confirmed, batched for the user (PENDING): Q1, Q2, Q7, Q8's assignment (below).
- Dismissed, with reasons: Q5 (`Clipboard::set` on the UI thread is no repository work, and the
  same call already ships in Local Changes' Copy Path); R3 (a walk step is a press, whose cost is
  a stated bounded residual, and superseding under key repeat is the design and Fork's); T1's
  mutation half (the auditor reported that restoring space-between and a margin beside the new
  rule passes the menu tests; re-run here twice from the committed tree, touched and rebuilt, it
  fails `a_long_chord_stands_inside_the_menu` — "the chord ends at 395.8203, not at the menu's
  236.91016 less its inset". The auditor's copies built into the shared `target/`, where an
  artifact newer than its sources is reused: the same stale cache misled this phase once, when a
  scratch worktree built into `target/` left a `cairn-model` that no source here held).

**Batched for the user — PENDING, not approved:**
1. **C14 / R10.3's Return.** The PRD says the sidebar's rows walk by ↑/↓ and Return opens Local
   Changes or All Commits (test `the_sidebar_walks_by_key_and_opens_with_return`). Fork's arrows
   never reach those rows and its Return does nothing on any row (sb55-sb68); Return on them is
   NOT OBSERVED, being unreachable. Built: Fork's. Options: (a) ratify Fork's and amend C14,
   R10.3.3 and the test name to `the_sidebar_walks_by_key_as_forks_tree` (recommended — Ctrl+1 and
   Ctrl+2 already reach both from the keyboard); (b) a Cairn extension, ↑ past the Branches
   heading to All Commits and Local Changes and Return there switching, as a ui.md deviation row.
2. **The real-window check** (qa items 5-7) for this phase is owed — before phase 09 or by the
   user — or the headless release render accepted in its place.
3. **Tags of one date**: ordered by name, last first, from the one tie observed; a second
   `/observe-fork` run with several ties would settle it.
4. **Two deviations no requirement owns**: "Create branch at:" shows the commit where Fork names
   the ref with its glyph (sb78-sb82; the chord's "main" too, FS2 §4), and pressing All Commits
   does not move focus to the history (sb73). Assign each to a later phase (08 is the nearest),
   or keep as ui.md rows.
5. **New tones**: `SIDEBAR_ICON` #CFCFCF, `SIDEBAR_DIM_ICON` #777777, `FILTER_FIELD` #292929
   (FS2's own), `UPSTREAM_GONE` #FFC501, and `UNSEEN` (transparent); **new metrics**:
   `SIDEBAR_ICON` 14, `SIDEBAR_CHEVRON` 12, `SIDEBAR_GAP` 6, `SIDEBAR_INSET` 8,
   `SIDEBAR_BAND_INSET` 5, `SIDEBAR_BAND_RADIUS` 3, `SIDEBAR_BADGE` 8.
6. **The local-only glyph** keeps a shape of its own (Lucide `git-commit-vertical`, dim) where Fork
   greys its branch glyph — kept from refs-and-status and now a ui.md row; Fork's own would be the
   branch glyph in `SIDEBAR_DIM_ICON`.
7. **The 300 px sidebar at the 880×620 minimum** leaves the history's subject ~160 px (it was
   ~218 px with 240 px); the minimum size stays as approved.
8. **Icons chosen from Lucide** for Local Changes (`file-diff`) and All Commits (`file-stack`, Fork's
   is a tray of pages).

**Deferred:** Copy Branch/Tag Name's Ctrl+C (R10.1's `Copy`, phase 08); C21's menu half
(`a_long_menu_builds_one_viewport`, phase 08); a branch name's "Local branch / Tracked branch"
tooltip and the hover icons (#2) not drawn; reusing the last snapshot's tag dates.

## 2026-10-20 — phase 04, the history (packet mode)

Committed onto `feature/fork-alignment` over phase 03's 1573b07.

- **`HEAD`'s reach** (R5.6, closes #67): every date-order walk from refs marks each row `HEAD`
  cannot reach as it comes off (`cairn-git/src/history/head_reach.rs`), final at emission since
  5c's date order yields no child after its parent; `cairn-model` carries it as a row flag
  (`HistoryRow::head_reaches`); the list greys such a row's texts, a stash's row always. Other
  orders and walks from given commits mark nothing.
- **The stash lane** (R5.7): `push_stash` puts a stash on the lowest lane awaiting its base, else
  the lowest free lane. The frozen compact-rows oracle is untouched (byte-identical) and walks
  no stash, so what it compares is unchanged.
- **The row** (R5.1-R5.5): no heading, 23 px, columns in Fork's order, initials avatars (history
  only; the Commit tab and Changes summary are phase 06's), `HEAD`'s row bold throughout, Fork's
  17 px chips with a tracked branch's upstream as a cloud-only chip, the remote cloud grey, the
  current branch bold. Ctrl+0 (Show HEAD) pinned.
- **Local dates** (R5.4, closes #103): jiff a direct dependency of cairn-ui and cairn-app, the zone
  read once on the repository thread (`worker/zone.rs`) and sent with the git found
  (`Update::Started`), handed down as `cairn_ui::LocalZone`; history `d Mon yyyy HH:mm`, the Commit
  tab `dd Mon yyyy HH:mm:ss ±HH:MM` in the zone, an unreadable zone said once as " (UTC)"; the
  activity popover's times local. Guard `the_render_layer_is_handed_its_time_zone`. Lockfile:
  two dependency-edge lines only, no new crate, features unchanged.
- **The graph's geometry** (R5.8), measured from uv2/uv7: lanes 12 px apart, a 5 px dot, a 13 px
  hollow merge circle, 2 px lines, curved joins (already curves).
- **Show Lost Commits** (R5.9): the header and its check box are gone; the View item and its
  chord turn it; while on, a strip over the history. **Decision taken here, batched for the
  user:** the strip uses Fork's observed words and place — "Reflog mode enabled" with an "Exit"
  link, across the top of the history column (`fork-menu-toggles-2026-10-19.md` §2) — not PRD
  R5.9's "Showing commits only the reflog keeps." / "Hide Lost Commits" under the toolbar (R4.1).
  The old `ui.md` row claiming Fork shows no sign of the mode is removed. The PRD text is unchanged.
- Acceptance criteria and their tests:
  - C10: `a_history_row_is_forks`, `a_rows_chips_are_forks`, `the_render_layer_is_handed_its_time_zone`,
    `show_head_selects_heads_row`.
  - C11: `the_walk_marks_what_head_reaches_as_git_rev_list_does` and
    `the_head_reach_counts_what_git_substitutes_and_a_head_no_ref_names` (real git, git-floor too),
    `a_row_beyond_heads_reach_reads_back_so_and_no_other_does`.
  - C12: `a_stash_is_drawn_on_its_base_commits_lane` (cairn-model and `drawn_lanes.rs`),
    `a_stash_whose_base_no_line_awaits_takes_a_new_lane`,
    `a_stash_already_awaited_keeps_its_line_and_a_second_takes_the_lower_waiting_lane`,
    `the_graph_is_drawn_at_forks_measured_sizes`, `a_merges_node_is_the_larger_hollow_circle`.
  - C13: `show_lost_commits_is_the_view_menus_and_names_itself_while_on`.
- Tests whose expectations changed: `drawn_rows.rs` and the window's checkout table keep their
  frozen oracles verbatim, reading each oracle date in Fork's form (`fork_date`) and the
  avatar's initials; `every_ref.rs`'s stash test follows the new lane rule (two stashes on one
  commit share a line); the Commit tab and Changes summary tests hand a zone; the activity rows'
  times lose " UTC"; heading-based window helpers read the main region instead.
- The stash row's `.focused(..)` wiring is now pinned
  (`rows_head_cannot_reach_are_grey_and_a_selected_stash_is_drawn_focused`, mutation-checked).

**Real window** (release build, the fixture of `fixture-recipe.md` built on the host — every
commit in one second, as FS2 §6's uw63 — Hyprland, captured with `grim -T`, driven with
`hl.dsp.send_shortcut`, resized with `hl.dsp.window.resize`).
- Verified at 1024×768: no heading; rows in uw63's order and lanes (`bugfix/crash` lane 1, the
  stash on `main`'s lane above it, the merge's hollow circle, curved joins); `bugfix/crash`'s row
  and the stash's in #A9A9A9 (sampled), reached rows #EAE8E8; the cloud-only chip before
  `feature/login`; `origin/main` and tags with their caps; avatars; dates local
  ("20 Oct 2026 07:23", BST). Ctrl+Shift+. drew "Reflog mode enabled  Exit" over the rows, and
  again took it down.
- Verified at 880×620: every column visible, the subject cut first with an ellipsis; Ctrl+0
  selected `HEAD`'s row in Fork's blue; the Commit tab's dates "+01:00"; Expand All visible.
- Differences, and why: Fork starts each row's chips after its own lanes (Cairn's graph column is
  the widest row's — a `ui.md` row); Fork's blue dot on `HEAD`'s row, its merge chevron and
  filled selected merge are not drawn (a `ui.md` row; Collapse Merges is #1); Fork's selection
  band has rounded ends, Cairn's is square (not recorded as a row yet — batched); Fork's reflog
  mode also draws the stash reflog's commits (existing `ui.md` row 84).
- UNVERIFIED (no pointer reaches a Wayland window from this host): pressing the strip's Exit, a
  row press, the View menu by pointer — covered headlessly. macOS unverified.

**QA.** Fresh `qa-checklist`, `responsiveness-reviewer`, `test-coverage-auditor` and
`gate-integrity-reviewer` in the foreground; `qa-confirm` adjudicated 18 raw findings:
- Confirmed and fixed: G1 (the zone guard missed `in_tz` and `TimeZoneDatabase`), G2 (a braced
  `use jiff::tz::{self, db}`), G3 (unstated jiff residuals in CLAUDE.md), G5 (catch-all label in
  the matcher), R1 (`LocalZone` compared by zone content, now by read serial), T1 and T2 (two
  `push_stash` arms survived mutation; now pinned and mutation-checked), T3 and T4 (shared ids and
  dates made two claims unfalsifiable), Q2 (graft, shallow and unnamed-`HEAD` reach cases), Q3
  (two test comments told history). A fresh `qa-confirm` verified all ten fixed and caught one
  unformatted line, fixed.
- Escalated, batched for the user: G4 — clippy's `disallowed-methods` (linter tier) was not used
  for the zone rule; CLAUDE.md now records why a guard holds it.
- Dismissed, with reasons: G2's `"/etc/localtime"` half (a `std::fs` read, inside the stated
  residual); R2 (the zone read placed as designed on the repository thread, once per launch, off
  the UI thread); T6 (`ChipKind` has no `HEAD` variant, and the detached branch is exercised
  elsewhere); Q4 (manifest comments cite decision ids as every other manifest comment does; the
  comments rule binds code comments); Q5 (the strip is Fork's own, so no `ui.md` deviation row is
  owed; the PRD wording stays the batched question); Q6 (layout widths stay in the views, as
  `style.md` says; the guard holds font sizes and tones); Q7 (no production walk is out of date
  order or from given commits).

**User decisions relayed by the coordinator on 2026-10-20** (from earlier phases' batches):
- The splitters stay 4 px handles rather than Fork's 1 px rules, for now (a `ui.md` row added).
- The 880×620 minimum window size is approved.
- Phase 03's new tones, the status box #303030 and the badge #606060, are approved.
- Phase 01's 10 extra tones sampled from FS2 are approved.
- Phase 03's metrics (20 px icons, 44 px minimum button width, 14 px toolbar group gap) are approved.
- Every other batched item from earlier phases stays PENDING.

## 2026-10-20 — phase 03, the toolbar, the status box and the window's regions (packet mode)

Committed onto `feature/fork-alignment` over phase 02's f998176.

- `title_bar` is now `toolbar`. No guard named the old path, so there was no guard path to move
  or to show failing.
- `cairn_ui::Toolbar`: Fork's buttons in Fork's order, each an icon over its label.
  - Quick Launch, Pull, Push, Stash (with its ⌄) and Appearance are greyed with L2's sentences
    and report nothing to a press or a key.
  - Fetch and Branch go through the menu bar's refusals and `shortcuts::act`.
  - Branch's ⌄ is a menu of New Branch… alone.
  - The sidebar toggle hides and shows the sidebar.
  - Badges on Push and Pull show the commits ahead and behind, absent at zero.
- The status box sits in the toolbar's middle, Fork's 300 px, with Branch beside it.
  - It draws the repository's name, bold, over the branch, with git's words for a detached or
    unborn `HEAD` and for "upstream gone".
  - Only its list glyph opens the popover.
  - A running operation takes the repository's place.
  - The title row's count and star are gone.
- **Stopping rule hit, and decided by the user: option A.** git's longer progress lines (about
  290-360 px) don't fit the box's 248 px of text room at any window size. The box's second line
  now shows the line up to and including its percentage.
  - A line with no percentage is kept up to its first comma, or whole if it has none. I chose
    that rule.
  - The whole line stays in the activity popover.
  - An operation name wider than the box is cut with "…". That cut is a `ui.md` row.
- The regions:
  - The banners stand under the toolbar.
  - The history and the detail pane split the height half each and grow together. A drag moves
    the proportion (`PaneSplit`).
  - Fork's "Hide commit details" icon replaces Collapse, lit while the pane is hidden.
  - The subject column gives way first as the window narrows.
- Ctrl+Shift+D is ToggleDetails on Linux, and Discard keeps Backspace and Delete. macOS keeps ⇧⌘D
  for Discard and gives ToggleDetails no chord.
- The window opens at 1024×768 and has a minimum of **880×620**, measured headlessly and in the
  real window. The toolbar needs about 780 px. At 880 px the history keeps 218 px for a subject.
  At 620 px the pane's even share (276 px) reaches Expand All. Recorded in `window-frame.md`.
- Acceptance criteria and the tests that decide them:
  - C7: `the_toolbar_is_forks_with_the_unbuilt_buttons_greyed`,
    `ahead_and_behind_are_badges_on_push_and_pull`,
    `fetch_and_branch_are_greyed_with_their_reasons`,
    `the_greyed_sentences_and_the_icons_name_are_the_specs_words`,
    `each_refresh_redraws_the_badges_on_push_and_pull`,
    `the_toolbars_branch_and_fetch_act_and_say_why_when_they_cannot`.
  - C8: `the_status_box_names_the_repository_over_the_branch`,
    `the_box_draws_the_name_bold_over_the_branch`,
    `the_status_box_shows_an_operation_only_once_it_has_run_250_ms` (kept, now checking the name
    over the progress line in the repository's place),
    `a_long_progress_line_is_kept_to_its_percentage_and_only_an_overlong_name_is_cut`,
    `a_progress_line_is_kept_to_its_percentage`,
    `the_status_box_opens_the_operations_with_their_git_and_no_token` (the glyph opens it, the
    centre does not), and `progress_and_remotes_land_in_their_states` (the line is kept whole).
  - C9: `the_detail_pane_grows_with_the_window`,
    `expand_all_is_visible_at_the_minimum_size_and_at_1024_by_1200`,
    `hiding_the_details_leaves_the_tab_strip`, `the_banners_stand_under_the_toolbar`,
    `the_subject_column_shrinks_first`, `the_window_keeps_every_region_whole_at_its_minimum_size`,
    `a_drag_against_the_panes_floor_keeps_the_proportion`,
    `the_sidebar_toggle_hides_and_shows_the_sidebar`.
  - C18 (the Ctrl+Shift+D half): the table's rows test and its pin (the new Shift+D example), and
    `the_menu_bar_holds_only_what_works_in_forks_order`.
- Tests whose expectations changed:
  - The fetch test now presses the toolbar's Fetch.
  - The tab-order test makes one Tab stop fewer: the strip's icon is no Tab stop.
  - `a_text_given_no_size_is_drawn_at_the_bodys` now measures the caption's title and the root's
    size.
  - The popover-position test allows the panel to move left to fit, since the box is centred.
  - The no-chord test presses outside the dialog on a row's date.
- Deferred check: a Lucide icon rasterizes once per icon, size, tone and scale factor (the
  `AssetCacher`). After that a re-render is a hash and a lookup (responsiveness-reviewer).

**Real window** (release build, the fixture, Hyprland, session locked; captured with `grim -T`,
driven with `hl.dsp.send_shortcut`, resized with `hl.dsp.window.resize`).
- Verified:
  - At 1024×768 the toolbar matches FS2's uw3b: the status box at x 367-667 against Fork's
    362-662, the Push badge "1", greyed labels, Branch beside the box, Appearance at the right.
  - The pane is half the main region, as in uw3.
  - Ctrl+0 selects HEAD, and the Commit tab draws down to Expand All.
  - Hyprland holds the window at 880×620 when asked for 700×500.
  - At 880×620 Expand All is visible at the pane's bottom.
  - Ctrl+Shift+D hides the pane to its strip with the icon lit blue (Fork's uw41), and brings it
    back at its share.
- Differences, and why:
  - Fork's tab strip and .gitignore banner are not drawn: no tabs (L8), no suggestion banners
    (`ui.md`).
  - Fork's Open in, Console and Work are not drawn (L2).
  - The history header and its Show Lost Commits box still stand. They overlap at 880 px, and
    phase 04 removes the header.
  - The splitters are still 4 px handles (below).
- UNVERIFIED (no pointer reaches a Wayland window from this host): tooltips, button presses, the
  Branch ⌄ menu, hover boxes and splitter drags in the real window. These are covered headlessly.
  macOS is unverified too: nothing was built for it.

**QA.** Fresh reviewers ran in the foreground: `qa-checklist`, `responsiveness-reviewer`,
`test-coverage-auditor` and `gate-integrity-reviewer`. Responsiveness and gate-integrity found
nothing; the others raised 14 findings between them. `qa-confirm` adjudicated all 14:
- Confirmed and fixed:
  - Q1: diff.md said there was no collapse chord.
  - Q2: a one-sided drag weighed pixels against the default share. Probed: at the floor only one
    panel reports. `PaneSplit` now keeps the pane's share of the measured room, pinned by
    `a_drag_against_the_panes_floor_keeps_the_proportion`.
  - Q4: the toolbar's paddings and radii now come from `style::metrics`.
  - Q6: the real window is recorded above.
  - T1: the tests now hold the pane at exactly half the room at the minimum height. Shown to fail
    with the minimum set to 880×450.
  - T2: an overlong name must stay one line, and a short one its whole width.
    - Deleting `max_lines` alone doesn't wrap in this Freya: the ellipsis keeps one line.
    - Deleting both `max_lines` and the ellipsis fails the test.
  - T3: the whole progress line is kept in the fetch state and the activity entry.
  - T4: the toolbar's Branch and Fetch reasons are tested through the window.
  - T5: the banner must also stand above the main region.
  - T6 in part: the stale menu-bar test doc is fixed. The sentences are now spelled out apart
    from the constants.
- Dismissed, with reasons:
  - Q3: Ctrl+Shift+D can't discard. Linux Discard holds only Backspace and Delete, and the
    resolver tests pin each chord to its action.
  - Q5: the status box glyph was no Tab stop before this phase either. Fork's toolbar isn't
    reached by Tab (FS2 §3), and focus at every stop is phase 08's (C19).
  - R1: the root already re-rendered per fetch progress line before this phase.
  - R2: the toolbar's handlers capture only Copy state handles and the window's one submitter.
  - T6's duplicate assertion: redundancy is not a defect.

Batched for the user (PENDING, none approved):
- Draw the splitters as Fork's 1 px rules? Freya's `ResizableHandle` has no separate hit zone,
  so a 1 px rule is also a 1 px grab target. Kept at 4 px for now.
- The minimum size 880×620 and the opening size 1024×768.
- New metrics and tones:
  - the status box's #303030 and the badge's #606060, both sampled in FS2;
  - the toolbar's 20 px icons, its 44 px least button width, its 14 px group gap.
- The status box keeps its 300 px at every width the minimum allows. Fork's shrinks and overlaps
  Push below about 800 px.
- The no-percentage rule for progress lines (up to the first comma).
- The Lucide icons chosen for Fetch, Pull, Push, Branch, Appearance and the list glyph.
- The pointer and macOS checks above, UNVERIFIED.

## 2026-10-19 — phase 02, the caption row, the menus and About Cairn (packet mode)

Committed onto `feature/fork-alignment` over phase 01's 082b3b6.

- `cairn_ui::menu`: one component that draws every menu from data. It draws groups split by
  rules, chords from `chord_name`, greyed items with their reason, the ✓ gutter and submenus.
  It walks by Fork's keys and holds the keyboard while open. The three context menus moved onto
  it with their items unchanged. A greyed item's reason is now drawn under it while the pointer
  is on it, where it used to be a second row: Freya's tooltip hides while a context menu is open.
- `cairn_ui::CaptionRow`: Fork's caption row on Linux, the same menus as a row on macOS. Each
  frame gesture goes to `crate::frame::ask` as a `FrameRequest`. Decorations are off and
  `BorderlessPlugin` handles the resize edges.
- The menu bar holds exactly R2.5's items, in Fork's order and groups (`crate::menu_bar`).
- Seven new accelerator actions; Show Lost Commits' chord now works window-wide.
- About Cairn is the last of the one modal state. `Update::GitFound` reports the git that was
  found.
- `deny.toml` pins `titlebar` and `borderless`; the lockfile gains `freya-borderless-plugin`.
- Acceptance criteria and the tests that decide them:
  - C5: `the_caption_row_draws_the_menus_the_title_and_the_controls`,
    `a_drag_on_the_caption_row_moves_the_window`.
  - C6: `the_menu_bar_holds_only_what_works_in_forks_order`, `the_menus_walk_by_key_and_hover`,
    `the_window_title_names_the_repository`, `zoom_scales_the_window_in_steps`.
  - C20 (About half): `about_cairn_names_the_version_and_the_git`.
  - C2 (borderless arm): `gate.sh --step deps`.

**Fork observed first.** `/observe-fork` checked these, all VERIFIED
(`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`):
- Hide Commit Details and Show Lost Commits show a ✓ while on, with their labels unchanged; the
  ✓ gutter shifts every label by 18 px.
- Hovering switches between menus, and ←/→ wrap. A menu opened by a press highlights nothing
  until ↓ is pressed. That differs from R9.1's literal "first item focused on open": context
  menus do highlight their first item, but the bar follows Fork.
- About Fork offers only a ✕ and ignores Escape. Cairn keeps R11.2's Close and Escape; the
  deviation row is in `ui.md`.
- The caption row maximizes on a double click and opens the system menu on a right click.

**Real window** (release build, the fixture, 1024×768). The session was screen-locked for most
of the run; the window was captured from its own surface (`grim -T`) and keys were sent with
Hyprland's `send_shortcut`.
- Verified:
  - The caption row draws the menus, "Cairn" centred and three controls, matching FS2's band of
    #1A1A1A.
  - The OS title reads `t — Cairn`.
  - Ctrl+1, Ctrl+2, Ctrl+0 and Ctrl+Shift+. act. Ctrl+= and Ctrl+- scale the window in steps:
    1.1 and 1.5 were measured from the row band.
  - Decorations, on Wayland: `WAYLAND_DEBUG=1` shows winit binding `zxdg_decoration_manager_v1`
    and sending `set_mode(client_side)` before the window first shows; Hyprland answers
    `configure(server_side)`.
  - The host's `hyprbars` draws no bar on Cairn. Its source hides the bar for a window whose
    latest request was client-side. Seen on screen once the session was unlocked: nothing above
    the caption row. `hyprctl decorations` still lists the decoration, hidden. No window rule is
    needed. An earlier note in this phase claimed a rule was needed; it misread that listing and
    is withdrawn.
  - Under X11 (Xwayland): `_MOTIF_WM_HINTS` has decorations 0, `WM_CLASS` is `cairn`, and the
    title is right. hyprbars draws no bar here either, as seen on screen.
- **UNVERIFIED** (batched for the user):
  - drag, double-press maximize, the window menu, resizing from the edges;
  - opening a menu by pointer, and hover moving along the bar;
  - a greyed reason shown on hover;
  - About Cairn and the menus' 12 px, by eye.
  
  No tool on the host moves the pointer into a Wayland window, and xdotool's pointer did not
  reach Xwayland while the session was locked.
- **UNVERIFIED**: macOS. No Apple target is installed, so nothing was compiled for macOS; the
  `Frame::Native` arm is checked only headlessly.
- Not compared shot by shot with FS1/FS2 beyond the band's tones. The toolbar and regions are
  phase 03's.

**QA.** First passes ran from the background and their reports arrived late. A second, fresh set
was run in the foreground: `qa-checklist`, `responsiveness-reviewer`, `test-coverage-auditor` and
`gate-integrity-reviewer`. `qa-confirm` adjudicated 26 raw findings; 15 were confirmed, in whole
or in part. Fixed:
- A, Q: `window-frame.md` now records the macOS gap and what headless tests cannot see.
- B, E: recorded above.
- F: the menus read `refreshed` rather than peeking it.
- G: `keys_inert`'s doc and CLAUDE.md now name About.
- I: `described`'s doc comment is back above its own function.
- K: `MenuKey::Held` is renamed `Unclaimed`, with AltGraph added.
- M: Alt added to `menu_key`'s test.
- N: the ZoomIn comment reworded.
- O: Show HEAD and New Branch… are asserted enabled once the refs are read.
- P: an unborn `HEAD`, and Fetch with no repository.
- R: the bar's Escape and Return, and no key reaching the window behind it. This needed a fix:
  the panel now closes the menu itself on Escape (`Panel::on_dismiss`), so Escape no longer
  reaches the window.
- S: a reason absent before hover and after the pointer leaves.
- T: About does not open under the popover.

Dismissed, with reasons:
- C, a greyed reason unreachable from the keyboard: R2.6 and R9.1 put the reason in a tooltip,
  and skipping disabled items is how standard menus behave. Batched as a design question.
- D, Hide Commit Details in Local Changes: the setting persists. Its tick is drawn, and the pane
  collapses when All Commits is shown again.
- H, the bar reachable under a dialog: every dialog is a Freya `Popup`, whose window-wide scrim
  covers the caption row, and the popover closes on an outside press.
- J, "fork-alignment L10" in manifests: the PRD keeps the citation after teardown, the same
  form was already in cairn-ui's manifest, and the comment guard does not read manifests.
- K in part, listing `menu_key`/`recall_step`/`field_key` among CLAUDE.md's exceptions: they
  answer what to do with a key, not which modifier is held.
- N in part, adding a Ctrl+Shift+= chord: that would be a new chord against Fork's Ctrl+=, so it
  is the user's call.
- S in part, "only the hovered item's reason": the panel keeps a single `pointed_greyed`.
- U, the Show Lost Commits chord from Local Changes: it goes through the same window-scope path
  as F5, which the table's scope tests pin.
- V, About's version compared through `version_line`: the test pins that the crate version
  reaches the dialog, and the wording is not specified.
- W, zoom refusal texts checked only for presence: R2.6 requires a reason, not its words.
- X, the empty ToggleDetails chord: phase 03 changes it.
- Y, CaptionRow cloning the menus: about twelve items, bounded.
- Z, `drag_window`'s modal loop on Windows: Windows is not a target.
- L, the untwinned "one component" sentence in CLAUDE.md: it sits in the status text and the
  repo map as a description, and the residual it supports names its reviewer. Not adjudicated by
  qa-confirm; left as written.

Batched for the user (PENDING, none approved):
- the lockfile's `freya-borderless-plugin` and the widened freya pin, with `titlebar` on
  cairn-ui rather than cairn-app;
- the `ui.md` rows for About's Close and Escape, and for no Alt access keys;
- greyed reasons drawn under the item instead of in a tooltip, and whether the keyboard should
  reach them (C);
- the zoom steps 0.5-2;
- the app id `cairn`;
- `Update::GitFound`;
- a Ctrl+Shift+= alternate for Zoom In (N);
- the pointer and macOS checks above, UNVERIFIED.

## 2026-10-19 — phase 01, the style module (packet mode)

Built on `feature/fork-alignment`, cut from `main` at 934d5e1 (history-topo-order, #142).

- `cairn_ui::style` (`metrics`, `tones`, `icons`) holds R1.2's sizes and R1.3's tones, the
  launch theme is `tones::theme()` (Freya's `dark_theme` over Fork Dark's sheet), every view is
  re-toned through it with its layout unchanged, and the body size (12 px) is set at the window
  root. Text that was 13 px is now the body's 12 px; other sizes kept and moved to `metrics`.
- A selection is drawn by its list's focus, read from Freya's `use_focus` (no render effect,
  so the stopping rule did not trigger): the history, the sidebar (now focusable, taking focus
  on a press), both Local Changes lists, the Changes tab's files and the Commit tab's files.
- Lucide through `icons-lucide`; Cargo.lock gains `freya-icons` alone (stopping rule not
  triggered); `deny.toml` pins freya's features exactly and records Lucide's ISC licence.
- Guards `chrome_sizes_and_tones_come_from_the_style_module` and
  `every_icon_is_named_through_the_icon_module`, with self-tests; `STYLE_EXCEPTIONS` holds only
  the two palette files the plan expected.
- The diff palette no longer retunes: Cairn's diff ground is Fork's `#282828`, so the bands are
  Fork Dark's values (R7.5 met here). `ADDED_EMPHASIS` stays Fork's `#388442` less 20 a channel:
  at Fork's value the text reads under WCAG AA. Chips: tag and stash by kind, branch and remote
  bordered by lane and filled with a dark shade of it.
- C1 `chrome_sizes_and_tones_come_from_the_style_module` (+ self-test); C2
  `every_icon_is_named_through_the_icon_module` (+ self-test) and `gate.sh --step deps` — the
  `borderless` arm and `freya-borderless-plugin` are phase 02's (exact refuses an allowed feature
  not enabled); C3 `the_style_module_holds_forks_metrics_and_fork_darks_tones`; C4
  `a_selection_is_blue_while_its_list_has_focus_and_grey_when_not`.

**Real window** (release build, the fixture of `fixture-recipe.md`, Hyprland, 1024×768, driven
with `wtype` and `hl.dsp.send_shortcut`): All Commits, a row selected with ↓ (Fork's blue,
white bold text, as uw3), focus moved with Tab (grey `#525252`, bold, as uw4), Create Branch by
Ctrl+Shift+B (`#2B2B2B` ground, `#0078D7` focused field, disabled Create `#343434`/`#707070`,
buttons at 12 px), the Changes tab by Ctrl+Alt+2 (file list `#212121`, chosen file grey, diff
`#282828`, added band `#3A5C3F`, hunk header `#959595`, chosen tab in blue, as uw5). Surfaces
read as Fork Dark's: chrome `#1A1A1A`, sidebar `#2C2C2D`, history `#212121`, detail `#333333`.
Differs from FS1/FS2, each a later phase's: the splitters are Freya's ~4 px handles in `#525252`
where Fork draws 1 px rules (03); the history keeps its header, 26 px rows and UTC dates (04);
dialog titles are Freya's 18 px `PopupTitle` and the dialog has no `#3BACED` border (08, R11);
the sidebar keeps its 14 px indent and has no header (05). Found in the window and fixed: a
dialog's text field drew at `PopupContent`'s 15 px. **Not verified:** menus and tooltips by eye
— no pointer reached the window (`hl.dsp.cursor.move` warps without a hover reaching it, and
`xdotool` serves XWayland only), and no menu opens from the keyboard yet; their 12 px rests on
the root size and the tooltip theme, pinned by `a_text_given_no_size_is_drawn_at_the_bodys` and
`the_launch_theme_carries_fork_darks_tones_into_freyas_components`. The minimum size was not
checked: the window has none until phase 03 sets it.

**QA.** Fresh `qa-checklist`, `responsiveness-reviewer`, `test-coverage-auditor` and
`gate-integrity-reviewer`; `qa-confirm` adjudicated 18 findings (22 raw, 4 merged). Fixed: the
invariant's claim narrowed to font sizes and tones with layout sizes stated unguarded (A); menus
take `MENU_SHADOW` (B); the icon test's comment moved (E); disabled buttons take FS2's `#383838`
border (G); a fractional-alpha tuple is caught, and the unseen routes — a computed tuple, a
`u32` or variable colour, a literal `with_a`, a size through a binding or a parameter — are
stated residuals (H, I, J); a blind-matcher pin for the size half (K); exceptions excuse tones
only (L); the residual's owner is the permanent `qa-checklist` item 1 (M); window tests pin the
history rows' focus wiring and the root's body size (N, O). Deferred: dialog titles at 18 px
(C → phase 08); C2's `borderless` (D → phase 02); after a press the sidebar holds focus with no
keys, so ↑/↓ reach no list until the history is pressed (Q → phase 05, C14, which gives the
sidebar Fork's ↑/↓).

Dismissed, with reasons:
- F, "user-approved, fork-alignment L10" in `crates/cairn-ui/Cargo.toml` is history: the
  manifest records every dependency's approval this way (its other two comments), the
  comment-history guard reads `.rs` comments only, and the meta-invariant wants each
  dependency's reason recorded.
- P, C4 never moves focus straight between Unstaged and Staged: each list reads its own focus
  node and a `LocalSelection` holds one list, so pressing the other list moves selection and
  focus together; nothing hand-tracked could carry a stale tone.
- R, the icons' `SvgViewer` cost: no view draws an icon yet; the phase that first does (03, the
  toolbar) checks it.

Batched for the user (PENDING, none approved): the lockfile's new crate `freya-icons` and the
`deny.toml` freya pin (L10 decided the crate; the pin's exact list is new); the extra FS2 tones
beyond R1.3's table; `ADDED_EMPHASIS` kept off Fork's value for contrast; the Lucide ISC notice
a package of the binary must carry beside it.

## 2026-10-16 — planned

Planned with `/feature-plan`.

Evidence in `docs/research/ui-review/`:
- two first-hand Fork for Windows 2.23.2 surveys (light, then dark), at 1024×768 over the fixture
  in `fixture-recipe.md`;
- Cairn's window over the same fixture;
- Cairn's UI as built;
- the earlier Fork evidence graded;
- the Freya chrome APIs.

The user locked S1-S5 and L1-L9 (`brainstorm.md`). They rejected the recommendation once, on the
menu bar: they chose Fork's caption row over the OS frame.

The surveys also found a git-parity bug in the graph's order. It became packet 5c,
`history-topo-order`, which runs first.

The design docs were rewritten to this end state:
- `docs/design/ui.md`: the chrome, the deviations, the sidebar, Local Changes, Show Lost Commits,
  the palette and type;
- `history-graph.md`: columns, `HEAD`'s reach, the stash lane;
- `diff.md`: the detail pane, avatars, the diff bar.

The roadmap gained 5c-5h.
