# Progress — fork-alignment

Running log, newest first. Each phase's dismissed QA findings are logged here with their
reasons, for the final QA phase to audit.

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
