# Progress — refs-and-status

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

## 2026-10-07 — teardown

**The user accepted every remaining batched decision in `state.md` as built, and approved
teardown (2026-10-07).** Each "Decided by the user" item was checked against a living doc:
the merge bar's four (distinct paths in "Showing N of M", `local-changes.md`; a failed status
read kept as `Error::GitFailed`, `status.md` and `local-changes.md`; F5 and the bare-chord rule,
`diff.md`; invalid ref names skipped uncounted, `refs.md`), RR1 and RR2 (`history-graph.md`,
"Refresh"; PRD R10.3, R11.2), Q1 and Q2 (PRD R6.2, R4.2; `diff.md`, `history-graph.md`), and
phase 08's and 09's Fork decisions (`sidebar.md`, `local-changes.md`) — all present; none had
to be added. The batched open questions of phases 04-09 are described as built in
`history-graph.md`, `sidebar.md` and `local-changes.md`.

Teardown: the PRD stamped `shipped`; C11, C12, C15 and C16's numbers moved to
`docs/research/refs-and-status/measured.md`; the systems docs spot-checked against the code by
two fresh readers and their stale sentences fixed (the Local Changes filter's lane and thread
named, packet and phase tags dropped, `SidebarRow` corrected to eight bytes and pinned by
`a_sidebar_row_is_eight_bytes`, the refs read's cost cited from the measurement); the PRD's
out-of-scope items and the "Handed to phase 10" list filed as #62-#82, with #2, #4, #35 and #36
referenced rather than duplicated; the roadmap marks packet 4 shipped. This directory is then
deleted; git history holds it.

## 2026-10-07 — merge bar: the user's decisions and the drift fixed

The packet's independent QA returned MERGE BAR: READY, with documentation drift to fix first.
**The user decided, at the merge bar (2026-10-07):**

1. **Local Changes' "Showing N of M files" counts distinct paths**, matching the sidebar's
   Local Changes (N), not rows (phase 09's QC3c). Built: the filter's pass counts N on the
   worker by a merge of the two lists' rows left (`MatchedRows::paths`); M is
   `LocalChanges::paths`; the window keeps N with the rows
   (`local_changes_state::shown_paths`) and hands it to the list
   (`LocalChangesList::shown_paths`). Pins, each red under counting rows (the caption back to
   `unstaged + staged` of the rows' total, and `MatchedRows::paths` set to the rows' count):
   `what_a_filter_leaves_is_counted_in_distinct_paths` (model),
   `the_filters_count_is_of_distinct_paths_a_path_in_both_lists_once` (cairn-ui; "2 of 7"
   under the mutation) and `the_filters_count_is_the_sidebars_distinct_paths_a_path_in_both_lists_once`
   (the window through the real filter pass; "Showing 2 of 6 files" under the mutation).
2. **A failed status read stays the built `Error::GitFailed`** for the whole view, the last
   lists kept (phase 02 QA's open question). A named state for a partial-clone status failure
   is an issue to file at teardown (state.md, "Handed to phase 10").
3. **F5 stays Linux's Refresh chord**; the bare-function-key rule is kept.
4. **Invalid ref names stay skipped and uncounted** in `RefsSnapshot::unreadable` (phase 01's
   QC-F6).

Merge-bar findings fixed:

| Finding | Fix |
| --- | --- |
| W1 | `docs/design/concurrency.md`: refs and ahead/behind superseded, a running status left to finish, every refresh meanwhile one follow-up, only a close ends one |
| W2 | `docs/design/engine.md`, the status case: the two-read `--untracked-files=all` scheme (git reads `status.showUntrackedFiles`), the programs a status read starts (fsmonitor hook or daemon, clean filters, `git status` in each submodule), the partial-clone residual (fails closed on 2.44 and later, fetches before) |
| W3 | `docs/systems/refs.md` and `status.md` point at what draws them |
| W4 | `history-graph.md` names the snapshot's readers, not phases |
| W5 | state.md's phase 06 row: done |
| W6 | `the_refs_and_ahead_behind_reads_leave_the_git_directory_byte_identical` (`crates/cairn-git/tests/refs.rs`): every file under `.git` compared byte for byte around both reads, over `every_kind` with a stash. Red with `std::fs::write` of `.git/index` at the top of `Repository::refs`, and with a write of `.git/logs/HEAD` at the top of `Repository::ahead_behind`; restored after each |
| W7 | this entry, below |
| W8 | `diff.md` cites `chords_are_distinct_and_every_bare_one_is_a_function_key` |
| W9 | `reads/mod.rs`'s list of writing `git stash` subcommands has `export` and `import` (the guard's roster had them) |
| N1, N2 | PRD R3.4 and R7.1 amended inline with the user's decisions (two reads; `18↓1↑`, a zero left out) |
| N3 | `git-processes.md`'s reads table names `stash_changes.rs` |
| N4 | the stash read's residual worded "before 2.44 may lazy-fetch (2.30 and 2.31 list no untracked file)" in `reads/stash_changes.rs`, `diff.md` and `destructive-ops-reviewer.md` (wording only) |
| N5 | `scripts/git-floor.sh`'s skip roster names `a_copy_into_an_untracked_file_is_read_from_the_untracked_commit`, whose skip now prints its whole name |
| N6 | `CAIRN_C16_MIB` can only lower C16's 64 MiB (`compact_rows.rs` and `find_tests.rs`) |
| N10 | state.md: `18↓1↑` unspaced in the phase 07 item; L13's row keeps a parent count |
| N11 | `refs.md` and `git-processes.md` say as-built what was phase-dated |
| N12, N7-N9 | carried into state.md, "Handed to phase 10" |

**The user's report at the merge bar (2026-10-07):** in the Commit tab with files opened in
place (a stash row, Expand All), the last diff row sat flush against the pane's bottom, under
the horizontal scrollbar. Freya's `VirtualScrollView` (fork `caa46f8`) draws that bar as an
overlay over the viewport's bottom 16 px and lays out no padding, so the Commit tab's list,
the diff view, the Changes list and Local Changes' two lists — each can scroll sideways —
now count empty rows after their last (`crates/cairn-ui/src/end_room.rs`, `with_end_room`, at
least 20 px in whole rows), built empty and after every indexed row. Pins, each red with the
room removed: `scrolled_to_the_end_the_last_row_is_clear_of_the_horizontal_scrollbar`
(diff view, unified and side by side), `scrolled_to_the_end_the_last_file_is_clear_of_the_horizontal_scrollbar`
(Changes list), `scrolled_to_the_end_each_lists_last_row_is_clear_of_the_horizontal_scrollbar`
(Local Changes) and `only_a_viewport_of_rows_is_built_however_many_files_are_open` (the
Commit tab's end one row above the bottom); the diff view's viewport twins count the room's
empty rows at the end. The history list and the sidebar were not changed.

**W7, phase 07 QA's dismissals RR5-RR7 named with their bounds** (the phase 07 QA entry logged
them only as "bounded, no defect claimed"; that entry stands as written):

- **RR5** — a row's chips read its labels as it is built: at most 2·chips + 1 label reads
  (`ref_chips::row_chips`: the current branch's label and each upstream found by a binary
  search, the labels read in order only until the column's room is spent) — bounded by the
  column, never by the refs on the commit.
- **RR6** — `graph_geometry::row_geometry` builds a `HashSet` of the places a row's lines reach,
  once per row built: O(lanes crossing the row), and a row is built only in the viewport.
- **RR7** — the title bar's status box, built by `window::status_box` per window render: one
  `HeadState` clone and two binary searches (the current branch in the snapshot,
  `RefsSnapshot::find`, and its counts, `RefreshState::ahead_behind_of`) — O(log refs) a
  render.

## 2026-10-07 — phase 09 QA

Fresh qa-checklist (NOT READY), responsiveness-reviewer, test-coverage-auditor; adjudicated by
a fresh qa-confirm, run by the coordinator. **The user ruled on the Local Changes Fork
questions (2026-10-07):**

1. **Badges: Fork's, told apart by shape** — `+` for added and untracked, `M` for a type change
   as for a modification, a submodule glyph painted as a shape (`RefGlyph::Submodule`, a box in
   a box, no font), Fork's triangle for a conflict; colour only reinforces. Local Changes only:
   the Commit tab keeps its letters.
2. **A draggable splitter between Unstaged and Staged**, its share kept for the session and
   never beyond it (`LocalChangesView::lists_split`).
3. **Natural row order**: `cairn_model::natural_order` through `path_order` (bytes break a tie
   between paths that read alike), untracked mixed in; `row_of` searches in the same order.
4. **Toolbar: the filter field only**; the eye, Hide Untracked Files and the layout menu are
   issues to file (state.md, "Handed to phase 10").

Fixes, each pin checked against a named mutation:

| Finding / decision | Pin | Mutation it fails under |
| --- | --- | --- |
| TC1: the fixed loop, reintroduced, only hung the tests | `an_empty_status_lets_a_path_go_once_and_then_decides_nothing`, `the_path_chosen_follows_the_lists_as_decided` (`local_changes_pane::follow`, a pure `Follow` decision: Keep / ReAsk / ChooseFirst / LetGo / Nothing; the effect writes a let-go only while a path is chosen) | `None if chosen.is_some() \|\| true`: both red with a message in 0.05 s, and no window test hangs under it |
| TC2: the scratch guard missed a linked worktree or a `.git` symlink | `the_scratch_guard_refuses_whatever_shares_the_benchs_git_directory` (`window_check::scratch_refusal`, `std::fs` only: `.git` file `gitdir:`, symlink followed, `commondir`, alternates; fixtures built without git) | the git-directory and common-directory check removed (a linked worktree accepted) |
| TC3, TC4, decision 1 | `every_kind_of_change_draws_its_badges_shape` (every kind drawn; each painted badge's cell read pixel for pixel against its glyph's own paint), `both_lists_draw_their_paths_with_their_badges`, `each_kind_of_change_has_forks_badge`, `every_glyph_paints_a_shape_no_other_glyph_paints` (now with `Submodule`) | a glyph's cell an empty rect; the submodule painted as the triangle; an added path drawn `A` |
| Decision 2 | `the_splitter_between_the_lists_drags` (cairn-ui: the handle dragged 100 px, the Staged heading moves and the share is written); `a_dragged_height_becomes_unstageds_share` | the dragged share not written |
| Decision 3 | `each_list_is_in_natural_order_and_found_in_it` (`b10` after `B2`, case ignored, two non-UTF-8 paths that read alike told apart by bytes, every row found by `row_of`) | the lists sorted bytewise; `row_of` searching bytewise |
| TC5 | `a_local_changes_filter_superseded_mid_pass_stops_and_sends_nothing` (400,000 paths, the lane's function with the outbox read raw, as `a_sidebar_ask_superseded_mid_pass_stops_and_sends_nothing`; green five runs of five) | the pass's keep-going closure `\|\| true` |
| RR-note: "no diff is drawn under no row" overclaimed | `a_filter_hiding_the_path_chosen_keeps_it_chosen_and_drawn` (the filter hides the path chosen: its diff still drawn, no row highlighted, nothing else chosen, a refresh asks it again); docs say the Changes tab's rule (`local_changes_pane.rs`, `local-changes.md`) | a path the filter hides treated as gone (also red in `the_path_chosen_follows_the_lists_as_decided`) |
| RR1: the 15.76 ms F1 Load Diff frame called the existing phase's | The phase 09 entry corrected in place: no update landed in that frame and its cause is unexplained. Three more runs after the badge and splitter changes: F1 Load Diff max 6.67 ms (a file diff applied in it) / 2.96 / 2.93 ms; C14 recorded 1.12 / 3.20 ms there; run 2 gave 2.98 ms. It did not reproduce in four runs | — |
| QC2 | `diff.md` (the settings paragraph and the in-place options row), the root `CLAUDE.md` cairn-ui row, `diff_settings.rs` and `diff_actions::in_place_options` docs: Entire File is the Changes tab's and Local Changes', never a file opened in place | — |
| QC3a | `local-changes.md` "Where it is not Fork's" names the count (distinct paths, R9.2; Fork's said to be git status's entries, its doubling OPEN 9), the rows count under the filter, and Entire File shared | — |

**C12 re-run** after the badge and splitter changes (three runs, a fresh scratch clone of
11,000 paths, release, the bench untouched by `find -newer` after the clone and all three
runs): every frame under 16.7 ms. Scratch: opening max 6.60 / 6.87 / 8.35 ms; Local Changes
shown 6.72 / 6.92 / 6.90 ms; Unstaged scrolled 1.29 / 1.17 / 1.15 ms; a filter typed 0.72 /
0.84 / 1.17 ms; a refresh landed 0.73 / 0.81 / 0.60 ms. Bench: opening 7.50 / 6.63 / 6.65 ms;
the largest other frame the first Commit tab draw, 6.44 / 6.32 / 6.23 ms.

**For the user's batch (not decided), QC3c:** "Showing N of M files" counts rows (a path in
both lists twice) while the sidebar counts distinct paths and the caption says "files": should
M match the sidebar's count, or the caption say rows?

**Dismissed** (qa-confirm): QC1 — the `|| true` in the worktree was a reviewer's mutation, now
reverted; QC3b — Entire File shared under Local Changes' own bar is the Fork-like case, so only
QC2's rewording; QC4 — every Freya API used (`VirtualScrollView::new_with_data_controlled`,
`ResizableContainer`/`ResizablePanel::on_resized`, `use_a11y`, `use_focus`,
`scroll_with_arrows`) is present in the fork at `caa46f8`, verified; the inherited RR-notes —
a failed send after the worker ended dropping a `Retire`'s last hold on the UI thread, the
filter sharing the repository thread with history pages (bounded, cancellable), and the re-ask
on each refresh (R9.3) — acknowledged acceptable.

## 2026-10-07 — phase 09: Local Changes, read only, and the window check

Packet mode, on `feature/refs-and-status`. QA pending (the coordinator's, fresh reviewers).

**Shipped.** Local Changes in the main region (`docs/systems/local-changes.md`): a filter over
Unstaged above Staged, each a flat `VirtualScrollView` of paths with a badge (`M`, `A` for added,
intent-to-add or untracked, `D`, `R`/`C` with the source before the path, `T`, `S` for a
submodule, Fork's warning triangle for a conflict), the chosen path's diff on the right under
the diff view's bar. The lists are `cairn_model::LocalChanges` — the status's entries indexed
into two lists sorted by path bytes, built on the refresh thread (`Update::Status` now carries
`Arc<LocalChanges>`), with the count of distinct paths (`paths`, now the sidebar's count), a
path's row by binary search (`row_of`) and the filter's pass (`matching`, a tenth lane
`QueryLane::LocalChangesFilter`, `Request::FilterLocalChanges`, `Update::FilteredLocalChanges`,
on the repository thread). The window keeps the lists drawn and the filter's rows
(`local_changes_state.rs`); with a filter on, a new status is drawn only once its rows arrive.
The path chosen lives in `DiffState` (`diff_state/working.rs`) as a third holder of the
file-diff lane: asked as its list says (`WorkingSide::Staged`, `Unstaged`, `Untracked`; a
conflict asks nothing and draws `DiffNotice::Conflicted`), kept only for that exact query,
re-asked when the view is shown after losing the lane, by a setting moved with the view shown
(`Asking::Working`), and followed through each status drawn (`follow_the_lists`): still
listed, asked again with its last diff drawn meanwhile; gone, the first path chosen; none, none.
`FileTarget::WorkingTree` and `WorkingSide` lost their `expect(dead_code)`. The view hears
previous and next change itself (its own scroll and cursor, `LocalChangesView`) and says a
failed status read in place of the lists, or over the lists kept.

**Decided, batched for the user (not approved):**

- **The filter field is built** — Fork's Local Changes has one above the lists, and the Changes
  tab's precedent draws its filter — but **not the eye (quick look), the layout menu or the
  collapse-all chevron**, as the Changes tab draws none of them; the layout menu's items
  (tree, combined list, hide untracked, show ignored) are out of scope by the PRD (#36, R3.4,
  R3.5). Named in `local-changes.md`.
- **Badges are the Commit tab's letters** (`A` for added and untracked alike, `T` for a type
  change as R9.1 names it a kind, `S` for a submodule), where Fork draws a green `+` for
  untracked/added, `M` for a type change and a submodule icon; the conflict is Fork's triangle.
- **Rows in path-byte order**, untracked mixed with tracked (Fork declined tracked-first; its
  exact list order is not recorded).
- **The count is distinct paths** (R9.2's letter), where phase 08 counted entries: a staged
  deletion and an untracked file of the same name count once (Fork's OPEN 9 unsettled).
- **Local Changes' diff takes the Changes tab's options**, Entire File included (one file's diff
  under the same bar); the setting is shared with the Changes tab.
- **A refresh re-asks the path chosen** even when the status is unchanged (the file may have
  changed under the same `M`), drawing its last diff meanwhile; a path gone chooses the first.
- **The status failure** (the user's open question from phase 02) is drawn as built: the
  `GitFailed` message for the whole view, the lists of the last status kept under it.
- Unstaged and Staged split the left side in two equal halves (no splitter between them).
- "Showing N of M files" counts rows, so a path in both lists counts twice there.

**C9** passes: `local_changes_draws_its_lists_and_the_first_paths_diff_from_the_working_tree_query`
(lists, count, first path asked as its side, answer drawn for that query alone, an untracked
path asked as untracked), `both_lists_draw_their_paths_with_their_badges`,
`a_conflicted_path_draws_its_notice_and_asks_nothing`, and the viewport twin
`a_status_of_50000_paths_builds_one_viewport_filtered_or_not` (named in the root `CLAUDE.md`).

**The QA brief, each pinned:**

- A refresh removing the chosen path: `a_refresh_that_removes_the_chosen_path_draws_no_stale_diff_under_no_row`
  (window) and `a_refresh_asks_the_path_again_and_draws_its_last_diff_meanwhile` (state).
- A path staged and unstaged: `a_path_in_both_lists_draws_each_lists_diff_and_the_answers_cannot_cross`
  (window) and `a_path_in_both_lists_keeps_only_the_answer_for_the_list_chosen` (state).
- Untracked files under a new directory, one per file and counted:
  `untracked_files_under_a_new_directory_are_listed_one_per_file_and_counted` (real boundary,
  a repository written with `std::fs`), `each_path_is_in_the_lists_its_changes_put_it_in_ordered_by_name`,
  `the_count_beside_local_changes_is_of_distinct_paths`.
- window_check's status is large and only the scratch clone was dirtied: the check refuses a
  scratch path inside the bench (or the bench inside it) and a clone with
  `objects/info/alternates`, and asserts at least 1,000 paths; it landed 11,000 (below), and
  `find ~/Development/bench/rust/.git -newer <marker>` printed nothing after either run (and
  after the clone, made with `GIT_OPTIONAL_LOCKS=0 git clone --local --no-hardlinks`).

**Every pin checked against a named mutation:**

| Pin | Mutation it fails under |
| --- | --- |
| `each_change_is_drawn_as_its_kind_with_a_sources_path` | Unstaged takes `unstaged.is_some()` alone (a submodule listed for its state alone dropped) |
| `the_count_is_of_distinct_paths`, `each_path_is_in_the_lists_its_changes_put_it_in_ordered_by_name` | the distinct-path merge advancing one list on equal paths (a path in both counted twice) |
| `a_status_of_50000_paths_builds_one_viewport_filtered_or_not` | `item_size(1.)` (514 rows built for a viewport of about 22); `length(rows.min(1000))` (the end is not the last row) |
| `a_path_in_both_lists_keeps_only_the_answer_for_the_list_chosen` | `working_arrived` keeping any answer |
| `the_working_path_and_the_commits_file_take_the_lane_from_each_other` | choosing the path not taking the lane from the commit's file; `select_changes` leaving the path's lane |
| `a_refresh_asks_the_path_again_and_draws_its_last_diff_meanwhile` | `refresh_working` dropping the drawn diff |
| `a_refresh_that_removes_the_chosen_path_draws_no_stale_diff_under_no_row` | a path gone treated as still chosen |
| `a_path_in_both_lists_draws_each_lists_diff_and_the_answers_cannot_cross` | a Staged row asked as unstaged |
| `a_conflicted_path_draws_its_notice_and_asks_nothing` | a conflict asked as unstaged |
| `local_changes_draws_its_lists_and_the_first_paths_diff_from_the_working_tree_query` | no first path chosen on open |
| `local_changes_filter_is_answered_on_a_worker_for_the_windows_own_lists` | the request numbered in no lane (never served: no answer) |
| `untracked_files_under_a_new_directory_are_listed_one_per_file_and_counted` | the status read's second `--untracked-files=all` read skipped (`new/` one row) |
| `the_count_beside_local_changes_is_of_distinct_paths` | the count as entries |
| `next_change_moves_local_changes_own_diff` | the view's `Scope::Detail` chords unheard |
| `a_status_that_could_not_be_read_is_said` | the failure line over the kept lists dropped |

A bug the window tests found first: a status listing nothing, with a path chosen, let it go and
then let go of nothing in a loop (each write woke the effect again); `follow_the_lists` now
writes only when a path is to be let go of.

**C12 — `window_check`, measured** (release, warm, 1440×900, AMD Ryzen 7 9800X3D, two runs;
frames are the UI thread's work: every update applied in the frame plus its
`sync_and_update`). The window now opens as the application does — `Request::Refresh`, the refs
opening the history from every ref — and lands the decorated history, the sidebar's rows, the
refs and the status before its first phase ends:

| Phase | Frames | Median | p99 | Max (run 1 / run 2) | Over 16.7 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| bench: opening — refs (175), first page (8 rows labelled), sidebar rows, status | 35 / 33 | 0.02 ms | 7.5 ms | 7.51 / 7.01 ms | 0 |
| bench: the history scrolled | 62 | 1.43 / 1.34 ms | 2.1 ms | 2.12 / 2.09 ms | 0 |
| bench: S1, M1, F1 phases (as before) | — | — | — | 15.76 (F1 Load Diff, run 1) / 7.52 ms | 0 |
| scratch clone (11,000 paths): opening, its status among what lands | 37 | 0.02 ms | 8.3 ms | 6.70 / 8.25 ms | 0 |
| Local Changes shown: lists (Unstaged 10,750, Staged 500) and the first path's diff | 24 / 25 | 0.03 ms | 8.6 ms | 7.26 / 8.61 ms | 0 |
| Unstaged scrolled, 100 rows a frame | 122 | 0.68 / 0.75 ms | 1.3 ms | 1.34 / 1.47 ms | 0 |
| a filter typed, its rows answered on a worker | 13 | 0.02 ms | 1.0 ms | 1.02 / 0.94 ms | 0 |
| a refresh landed with the view shown | 28 | 0.02 ms | 1.2 ms | 1.15 / 0.60 ms | 0 |

Applying a status of 11,000 paths costs 0.004-0.014 ms (the lists arrive laid out; the window
moves an `Arc`). The status landed after 77 ms on the bench (clean) and about 110 ms on the
scratch clone (11,000 paths, two reads); the first path's diff about 60-80 ms after the view was
shown. The F1 Load Diff frame at 15.76 ms in run 1 (2.98 ms in run 2) is unexplained — no
update landed in it — and did not reproduce (corrected after QA, RR1; see "phase 09 QA"); no
frame crossed 16.7 ms. The slowest scratch frames (7-9 ms) are the view's first draw.

**C11, complete** — every number on the machine in `measured-baseline.md`, release, warm,
median of seven, measured in the phases named; nothing here was re-measured:

| C11 item | Bar | Measured | Where |
| --- | --- | --- | --- |
| status, clean bench | 100 ms | 29.1 ms | phase 02 |
| status, 1,000 modified + 10,000 untracked (scratch clone) | 250 ms | 34.6 ms (one read); 66.7 ms with the untracked in 1,000 new directories (two reads) | phase 02 |
| refs snapshot with every ahead/behind, bench | 100 ms | 0.13 ms (175 refs) | phase 01 |
| the same, 10,000-ref fixture | recorded | 28.3 ms (10,001 refs) | phase 01 |
| first page from every ref | 200 ms | 7.27 ms (8.56 ms with the snapshot's read) | phase 05 |
| the same from `HEAD`, beside it | recorded | 7.69 ms | phase 05 |
| status with every file's stat changed | recorded, not barred | 758.9 ms | phase 02 |

## 2026-10-07 — phase 08 QA

Fresh qa-checklist (NOT READY on QC-B1), responsiveness-reviewer, test-coverage-auditor;
adjudicated by a fresh qa-confirm, run by the coordinator. **The user ruled on the Fork
questions (2026-10-07):**

1. **Sort: Fork's natural order, folders first** — case ignored, digit runs read as numbers
   (`b2` before `b10`), at every level and for tags (`cairn_model::natural_order`). `main`
   treated as `master` (Fork Mac RN 1.0.97) is NOT done: the research record lists it among
   Fork's sorting changes but does not say what Fork does with `master`.
2. **A distinct no-upstream glyph**: `RefGlyph::LocalOnly`, a single line of commits with its
   newest hollow, beside the branch glyph (an upstream) and the warning (gone) — shape, not
   colour.
3. **Expansion is not remembered across sessions** (as built); `sidebar.md` now cites the one
   ambiguous research line (section 2, Windows 2.23's "collapse state" per worktree). The
   coordinator files an issue at teardown.
4. **(a)** counts unspaced, `18↓1↑`, in the sidebar and the title bar; **(b)** while filtering,
   a closed section opens too; **(c)** while filtering, a section with no match draws no
   caption; **(d)** the symbolic remote `HEAD` listed as `origin/HEAD` under its remote.

Fixes, each pin checked against a named mutation:

| Finding / decision | Pin | Mutation it fails under |
| --- | --- | --- |
| TC1 / QC-B1: the cancel pin could not fail | `a_page_asked_under_a_superseded_number_walks_nothing_and_the_next_takes_the_walk_up` (the lane, deterministic: an open of 2 under e1, two bumps, `More { 1_000_000 }` under the stale number sends nothing and keeps the session, `More { 2 }` under the current one answers `commits[2..4]`, not complete); `superseding_a_request_stops_the_walk_that_is_serving_it` reworked (one queued `More { whole }` of a 20,000-commit line, superseded by `MoreHistory { 2 }` after a fifth of the unsuperseded walk's time; passes only on the walk's third and fourth rows, a page starting at its first row a failure outright; a round whose walk finished first retried) | `session.next_page(rows, &CancelSignal::new())` (both red); `watch(self.walk.unwrap_or(epoch))` in `page`/`answer` (both red) |
| The fixture for both | `a_written_line_is_read_back_as_written` | (SHA-1's published vectors; the engine reads the line back in order) |
| TC2: a stop alone | `a_stop_alone_ends_the_find_and_leaves_the_walk_where_it_stood` (a find of one row a page over a line three windows long, stopped, `ListRemotes` behind the stop answered, no find page after it within 200 ms and none `complete`, then `MoreHistory { 3 }` continues the walk) | the stop numbered in no lane AND its arm ending nothing; the stop routed to another thread and numbered in no lane. Each mechanism alone is covered by the other (the lane's bump cancels the find's next page; the arm clears it), so a single one of the two passes, by design |
| RR1: a sidebar press left the list's hint stale | `the_list_moves_from_the_row_its_callers_hint_names` (cairn-ui: a press reaches the caller's hint; with one commit at rows 5 and 500 the hint at 500 moves ↓ to 501, a search would give 6); window `pressing_a_loaded_ref_selects_its_row_and_brings_it_into_view`, `pressing_a_ref_past_the_loaded_rows_finds_it_by_paging`, `a_parent_link_selects_a_loaded_parent_and_ignores_an_unloaded_one` now require `View::history_cursor` at the row | the list keeping its own hint; `bring_into_view` and `follow_parent` not setting it (all three window tests red) |
| Decision 1 | `each_level_is_in_natural_order_folders_first`, `natural_order_reads_numbers_as_numbers_and_ignores_case` | leaves unsorted; `natural_order` as a bytewise compare |
| Decision 2 | `each_entry_draws_its_kinds_glyph` (main Branch, login LocalOnly, topic Gone); `every_glyph_paints_a_shape_no_other_glyph_paints` | no upstream drawn as `Branch` |
| Decision 4a | `counts_are_behind_then_ahead_and_a_zero_is_left_out`; the title-bar test; the sidebar order test | counts joined with a space (status box and title bar red) |
| Decisions 4b, 4c | `a_filter_keeps_what_matches_with_every_section_and_folder_open` | a closed section left closed while filtering; an empty caption kept |
| Decision 4d | `the_sidebar_draws_its_sections_in_forks_order_with_forks_marks` | a symbolic remote ref drawn by its last part |
| QC-D1, TC3, QC-D2, RR2, RR3, TC-residual, QC-note | docs: `sidebar.md` (pins, the order `FilterRefs` before `OpenHistory` that keeps `labelled_position` right — pinned by `a_reopen_frees_the_old_rows_on_a_worker_and_keeps_the_selection` — the stash with no row paging the whole history, what waits behind a find page, `labelled_position`'s growth, the expansion line), `history-graph.md`, the phase-08 entry's pin row and equivalent-mutant reason corrected in place, `ref_find.rs`'s doc | — |

## 2026-10-07 — phase 08: the sidebar, its filter, and finding a ref

Packet mode, on `feature/refs-and-status`. QA pending (the coordinator's, fresh reviewers).

**Shipped.** The sidebar left of the main region behind a draggable splitter: Local Changes
(N) and All Commits, which switch the main region (Local Changes a placeholder for phase 09);
a filter box; Branches, Remotes, Tags and Stashes in one `VirtualScrollView`, branches and
remotes in folders split at `/`, folders first, the current branch's folders revealed when
it becomes current, a detached `HEAD` the first branch row; the current branch's check mark
and bold, each branch's counts (`counts_text`), Fork's warning icon for a gone upstream
(beside the check mark on the current branch); a press's notice under the filter. The rows
are `cairn_model::SidebarRow`s laid out on the repository thread (`RefsSnapshot::sidebar_rows`,
`Request::FilterRefs` now carrying the `Disclosure`, `Update::FilteredRefs` the rows and their
snapshot); the window keeps them (`sidebar_state.rs`), freeing replaced ones on a worker
(`Retired::sidebar`). Pressing a ref (`ref_find.rs`): its row selected and revealed — looked up
among the labelled, stash and `HEAD` rows (`History::labelled_position`) — or found by
`Request::FindRow`, paging the held walk 512 rows a page on the repository thread, one page
whenever no other job waits; `Request::StopFinding` for a scroll (a side effect on the list's
scroll position), a row chosen, or a row found in a page; a reopen re-asks the find. Pages are
answered under a new lane's number, `QueryLane::Walk`, which only an open moves, so a
superseded find's pages still arrive; an open superseded before it started still lets go of
the old walk (`HistoryLane::replace_walk`). A tag on a tree says so at once; a stash with no
row, at the walk's end, shows its changes with no row selected. `RefreshState` keeps
ahead/behind shared (`BranchCounts`), and both its `expect(dead_code)`s are gone.

**C8 and the QA brief, each pinned, each pin checked to fail under a named mutation** (one
edit, the named tests run, the file restored):

| Case | Pin | Mutation it fails under |
| --- | --- | --- |
| Sections in Fork's order, folders at `/`, folders first, closed folders hide | `sections_come_in_forks_order_and_branches_and_remotes_fold_at_slashes`; headless `the_sidebar_draws_its_sections_in_forks_order_with_forks_marks` | leaves laid out before folders; a closed folder's refs laid out |
| The filter opens every folder, a closed section stays closed | `a_filter_keeps_what_matches_with_every_folder_open` | `filtering` dropped from the folder's `open` |
| Current ✓ and bold, counts, gone, kinds' glyphs | `each_entry_draws_its_kinds_glyph`; the headless order test | gone drawn as a branch; `bold: false`; the current branch's `warning: false` |
| 50,000 refs build one viewport (C8, the twin) | `a_sidebar_of_50000_refs_builds_one_viewport` | the list a `ScrollView` of every row (50,000 built) |
| A folder of 10,000 open builds one viewport (QA brief) | `a_folder_of_10000_branches_open_builds_one_viewport` | the same (10,000 built) |
| A loaded ref selected and revealed, nothing paged | `pressing_a_loaded_ref_selects_its_row_and_brings_it_into_view` | the loaded lookup skipped; `labelled_position` answering `None` |
| A ref past the loaded rows found by paging, "Finding…", stopped once found | `pressing_a_ref_past_the_loaded_rows_finds_it_by_paging`; worker `a_find_pages_the_walk_until_a_page_holds_the_row_and_no_further` | no `StopFinding` on found; the worker paging on past a found row |
| Two quick presses draw only the second (QA brief) | `two_quick_presses_draw_only_the_second`; worker `a_second_find_supersedes_the_first_and_no_page_is_lost` | the first find kept over the second |
| A scroll supersedes a find (QA brief) | `a_scroll_of_the_list_supersedes_a_find`; worker `a_stopped_find_leaves_the_walk_for_the_next_page` | the scroll effect never stopping |
| A row chosen supersedes a find | `a_row_chosen_during_a_find_supersedes_it` | `row_chosen` removed from `on_select` |
| The list's end asks no page during a find | `a_find_is_not_superseded_by_the_list_reaching_its_end` | the `on_reach_end` guard removed |
| A superseded find's laid-out pages still arrive | `the_pages_a_find_laid_out_before_it_was_stopped_still_arrive` | pages sent under the query's epoch |
| An open superseded before it ran replaces the walk | `a_find_pages_the_walk_until_a_page_holds_the_row_and_no_further` and three more | `replace_walk` not called (four find tests time out) |
| A reopen re-asks the find | `a_reopen_during_a_find_asks_it_again_of_the_new_walk` | `ref_find::reopened` not called |
| A tag on a tree says so, pages nothing | `a_tag_on_a_tree_says_it_is_not_in_the_graph` | (asserts no `FindRow` and the notice) |
| A stash with no row: changes shown, says so | `a_stash_with_no_row_shows_its_changes_and_says_it_is_not_in_the_graph`; worker `a_find_for_a_row_the_walk_never_reaches_ends_with_the_walk` | the stash's `selection::choose` skipped; the complete check skipped |
| The filter on a worker, the answer drawn, replaced rows freed off the UI thread (QA brief) | `typing_in_the_sidebars_filter_asks_a_worker_and_draws_its_answer` | the pane's side effect never submitting |
| TC3: an answer arrives, a superseded one is dropped, one superseded mid-pass stops | `the_sidebars_rows_are_answered_on_a_worker_and_a_newer_ask_supersedes_the_older`; `a_sidebar_ask_superseded_mid_pass_stops_and_sends_nothing` | the answer sent under no epoch; `|| true` keep-going |
| The find's cancel is the epoch | `superseding_a_request_stops_the_walk_that_is_serving_it` — **false at 87ebb03** (phase 08 QA's TC1): with queued superseded opens each replacing the walk, the test passed under a fresh `CancelSignal` too; replaced in QA by `a_page_asked_under_a_superseded_number_walks_nothing_and_the_next_takes_the_walk_up` and a reworked pool test, both red under that mutation (see "phase 08 QA") | `next_page` given a fresh `CancelSignal` |
| The current branch's folders revealed only when it becomes current | `the_current_branchs_folders_open_when_it_becomes_current` | revealed on every refresh |
| `labelled_position` covers stashes and `HEAD`, scans no unlabelled row | `a_labelled_row_is_found_among_the_labelled_rows_alone` | the stashes' pass removed; the `HEAD` row removed |

Equivalent mutants, noted: the serve loop's `is_current` check before each find page and
`Page::Stop` clearing `finding` — each stands in for the other while both are in place, and
without the first a stale find's page is cancelled by its own epoch at its first poll and
sends nothing, except on a walk already exhausted, which polls no cancel and sends one empty
`complete: true` page, harmless (corrected in phase 08 QA's TC3; this said "sends nothing"
outright); the ref filter's pre-pass `is_current` (`keep_going` is asked before the first
entry).

**Measured** — `measures_a_find_through_the_boundary` (`#[ignore]`d,
`crates/cairn-app/src/worker/find_tests.rs`), release, warm, median of seven after a
warm-up, `~/Development/bench/rust` at `c999cef531e` (read only: its `.git` listing and the
two directories' mtimes identical before and after every run), AMD Ryzen 7 9800X3D, Linux
7.2.8-2-cachyos. Through the real boundary — the repository thread paging the held walk 512
rows a find page, every page appended to one `History` as the window appends it:

| Find | Row | Median | Range | Rows kept | Retained (capacity) |
| --- | ---: | ---: | ---: | ---: | ---: |
| the oldest commit, `c01efc669f0` (the walk's last row) | 345,544 | **2,227-2,250 ms** over three runs | 2,213-2,285 | 345,545 | **52.67 MiB** (159 B/row) |
| `refs/tags/release-0.1` (early history) | 337,965 | **2,208 ms** | 2,191-2,214 | 337,984 | 51.54 MiB |

Against C15's 10% over 2.4 s (2.64 s) and C16's 64 MiB: met, neither stopping rule hit; the
same as the engine-only reporter's 2,265 ms / 52.67 MiB (phase 05), so the boundary adds
nothing measurable. **Cancel**: a find stopped halfway (at 1.1 s, 155-160k rows kept), as a
scroll stops it: no row arrived after the stop — the page being walked was cancelled at its
next commit and kept in the session — and the next page asked began exactly at the row after
the last one kept. The press's lookup among the labelled rows of a 345,545-row history: under
a microsecond (a whole-history id scan, the first version, was 4.75 ms on the UI thread, and
was replaced by `History::labelled_position`).

**Decided and batched for the user (not approved):** folders first at each level, each in the
snapshot's (bytewise) order — Fork's "alphabetically, folders first" option, its default not
recorded; folders open closed, the current branch's opened when it becomes current (first
snapshot, a checkout) and expansion not remembered across sessions; while filtering every
folder is open, a closed section stays closed, and every section's caption is drawn even when
empty; a detached `HEAD` is a `HEAD` row first under Branches with the check mark, bold; the
counts printed as the title bar prints them (Fork's Mac order, behind first); the gone
upstream as Fork's warning icon alone (in place of the branch icon; beside the check mark on
the current branch); Fork's greyed icon for a branch with no upstream not drawn (colour
alone); `Local Changes (N)` with N the paths status listed, the count left out at zero or
before a status; stash entries by their message; `origin/HEAD` listed as `HEAD` under its
remote, as git lists it; the notices' wording ("Finding <ref>…", "<ref> is not in the
graph", "<tag> names no commit, so it is not in the graph") and place (under the filter); the
refs read's failure said there too; a press returns the main region to the history; any move
of the list's scroll position supersedes a find, and a row chosen in the list lets go of the
entry pressed; a find's page is 512 rows (up to 511 rows kept past the target); a stash with
no row is known so only at the walk's end (its press pages the whole history first, 2.2 s on
the bench); a reopen during a find asks it again of the new walk; a ninth query lane,
`QueryLane::Walk`; the sidebar 240 px wide until dragged, at least 140, the main region at
least 240; and pressing looks a row up among labelled rows only (a ref moved since the walk
began, until the refresh's reopen, pages the walk).

## 2026-10-07 — phase 07 QA

Fresh qa-checklist (NOT READY on QC1), test-coverage-auditor, responsiveness-reviewer,
destructive-ops-reviewer, gate-integrity-reviewer; adjudicated by a fresh qa-confirm (20
confirmed, 6 dismissed, none escalated), run by the coordinator. Fixes, each pin checked
against a named mutation:

- **QC1 (must fix).** The title bar named the command line's path as given (`.`, `..`, a
  subdirectory, `.git`). The repository thread now sends `Update::Opened { name }` once
  discovery has resolved the repository — its working tree's last component, its git
  directory's when bare — kept in `View::repository`; nothing is named until then.
  `an_open_names_the_repositorys_folder_whatever_path_it_was_opened_at` opens a fixture at
  itself, `/.`, `sub/..`, `sub`, `sub/deeper` and `.git`, and the checkout at `.`, `src`,
  `./src/..` and `../..` relative to the test's directory (red when the name is the git
  directory's); the title test applies the update and requires the name, and nothing before
  it (red when `Opened` sets nothing). Canonicalising the root was dropped: discovery already
  answers it absolute and resolved, and the mutation removing it survived. Every boundary test
  helper that opens a repository now reads `Opened` first (`fetch_tests::opened_as`).
- **GI1.** `export` and `import` are on `STASH_WRITING_SUBCOMMANDS`; the self-test spells
  the eleven out apart from the roster (red when one is dropped from it).
- **GI2/DO3.** `STASH_SHOW_OPTIONS` and `STASH_SHOW_REQUIRED`: every `-` literal of the
  stash read must be listed, the four required present. Self-test refuses `-p`, `--patch`,
  `--ext-diff`, `--textconv`, `-u`, `--include-untracked`, `--only-untracked`, `--stat` and
  each required flag dropped; red with either check disabled, and the real tree red with
  `--no-textconv` dropped or `-u` added.
- **GI3, GI4, DO4.** The reviewer, the guard's doc comment and `docs/design/engine.md`
  (a fifth case, the stash) say three exceptions; the reviewer says that argv alone, any
  revision but a full stash-commit id, the full subcommand list and the 2.32-2.43 lazy fetch.
- **DO1.** `reads/stash_changes.rs` and `diff.md` state that git 2.32-2.43 lazy-fetch in a
  partial clone and 2.44+ fail closed.
- **DO2.** `a_stash_read_writes_nothing_and_runs_no_program_but_fsmonitor`: a caching
  textconv, a driver `command`, `diff.external`, clean and smudge filters, `core.pager`, the
  untracked cache and an fsmonitor hook; no marker, the git directory byte-identical, each
  program run by hand. Red with `--no-ext-diff` swapped for `--ext-diff -p` (the answer no
  longer parses). Named in the root CLAUDE.md's pin list and `diff.md`; git-floor's
  `diff_engine` floor 118.
- **TC1.** The paint-once test adds a line past the cap into the node beside one passing it,
  read by coordinates; red when the end is dropped from `place`'s key.
- **TC2.** The gone-upstream title case supplies counts; red when a gone upstream is answered
  by its counts.
- **TC-obs a, b.** The writes-nothing test requires every file read; the parity test is named
  and worded off/on (a true unset would read the developer's global configuration through the
  engine's git, which the oracle's isolated git does not).
- **QC2.** The ahead/behind parity test requires bytewise order by name; red when the
  engine counts the branches in reverse.
- **RR1, RR3, QC5, QC6.** Docs: a sideways resize rebuilds the visible rows each frame,
  bounded by viewport × lanes; the REFS chips are laid out each Commit-tab render, bounded
  by width, only the header cached; the chips' cost cites its test; a comma.

Notes (no action): RR2 — `row_finder`'s pass is stated accurately; a later O(1) improvement
is to check the list's index hint first. RR4 — one chipless first frame; a stash chip
ignores the width by design (one chip).

Dismissed, with reasons: QC3 — a false hit on `"branch"` in `reads/` fails loudly, and its
remedy would be scoping like `CONFIG_SETTER_SUBCOMMANDS`; QC4 — the window's slimmed-rows
test compares unordered on purpose (explained there), and chip order is pinned in
`ref_chips.rs`; RR5-RR7 — bounded, no defect claimed; TC-obs c — the stub argv test's literal
argv is pinned by its sibling.

## 2026-10-07 — phase 07: chips on rows, stash rows, REFS and the title bar

Packet mode, on `feature/refs-and-status`. QA pending (the coordinator's, fresh reviewers).

**The user's decisions, relayed by the coordinator (2026-10-07):**

- **Q1 — option A: a third porcelain read.** A stash's changes are `git stash show --raw -z
  --no-abbrev --no-color --no-ext-diff --no-textconv --no-relative --end-of-options <stash>`
  (`reads/stash_changes.rs`), git reading `stash.showIncludeUntracked` itself, because with
  it set git pairs a stash's untracked files with its tracked changes in one diff — verified:
  a tracked `a` deleted beside an untracked `b` of its content is `R100 a b` in `git stash
  show --include-untracked`, where two plumbing diffs print `D a`, `A b` — and no plumbing
  can ask that without writing a tree. Guard renamed `the_porcelain_reads_are_the_three_named_queries`.
- **Q2 — option (i): one row per stash commit, the newest entry's.** `git stash store` can
  file a commit twice; `seeds::resolve` keeps the first (newest) entry of each commit.

**Shipped.** Chips on rows (`ref_chips.rs`, `ref_glyphs.rs`), the bold `HEAD` subject and
a stash row's `stash@{n}` chip (R5); the REFS row (R6.1); a stash's changes as `git stash
show` lists them, each untracked-side file read from the third parent (R6.2); the title bar's
status box (R7.1); and each place a row's lines reach painted once. Model: `RowLabels::find`,
`Label::short_name`, labels kept by name, `History::serial`.

**C7 and the QA brief, each pinned, each pin checked to fail under a named mutation**
(`mutate.py`: one edit, the named tests run, the file restored and touched):

| Case | Pin | Mutation it fails under |
| --- | --- | --- |
| Twenty refs clip, no wrap, row height kept | `twenty_refs_are_clipped_at_the_columns_edge_and_the_row_keeps_its_height` | the column's `overflow(Clip)` removed |
| Chips stop being built at the edge | `chips_stop_being_built_at_the_rooms_edge_however_many_refs_the_commit_has`; `only_a_viewport_of_labelled_rows_is_built_and_each_lays_out_a_columns_worth_of_chips` | the loop's `if laid.full() { break }` removed (each) |
| Compact: local+upstream one chip with glyph; a second remote keeps its chip; an upstream elsewhere not folded | `a_branch_and_its_upstream_at_one_commit_are_one_chip_and_nothing_else_is_folded` | `folded.contains` skip off; the current-first push off |
| Only a remote-tracking upstream folds; none without a snapshot | `only_a_remote_tracking_upstream_is_folded_and_only_with_a_snapshot` | the `kind == RemoteTracking` filter off |
| Current branch first, then the snapshot order | `the_current_branch_leads_then_branches_remotes_and_tags_in_order` | the current-first push off |
| Each kind its glyph, before its name | `each_kind_of_chip_draws_its_glyph_before_its_name_and_a_plain_branch_none`; `every_glyph_paints_a_shape_no_other_glyph_paints` | a tag's cap removed |
| Bold `HEAD` subject | `the_head_rows_subject_is_bold_and_no_other`; through the window, `a_rows_chips_and_its_refs_row_are_drawn_against_the_refreshs_snapshot` | the bold removed; the window's `.head(..)` removed |
| The room spent by a lower bound | `the_room_a_chip_is_counted_by_never_exceeds_what_it_is_drawn_at` | `MIN_ADVANCE` 2 → 7 |
| Laid out once per row built, not per frame | `a_rows_chips_are_laid_out_once_per_row_built_and_never_per_frame` | `same_refs` answering false; `room` compared unequal |
| The window hands the list the snapshot | `a_rows_chips_and_its_refs_row_are_drawn_against_the_refreshs_snapshot` | `.refs(refs)` removed |
| REFS row above SHA, none for a commit with no ref | that test; `the_refs_row_stands_above_the_id_and_only_when_a_ref_points_at_the_commit` | `.refs(refs_of(..))` removed |
| The selected row found once per history | `a_row_is_looked_for_once_per_history_and_found_where_it_arrives` | the finder keyed by row alone, not history |
| Stash row: chip before its message; asks `git stash show` | `a_stash_row_draws_its_message_and_asks_what_it_changed_on_its_base` | (was `Comparison::Commit`; red until the stash comparison) |
| A stash lists what `git stash show --name-status` lists, unset and set | `a_stash_lists_what_git_stash_show_lists_with_the_setting_unset_and_set` (host, 2.30.9, 2.32.7) | the stash branch of `changes` skipped; `--include-untracked` added |
| Untracked-side files read from the third parent | `each_file_of_a_stash_is_read_from_the_side_it_is_on`, `a_copy_into_an_untracked_file_is_read_from_the_untracked_commit` | `Trees::untracked` `None`; the copy-as-rename adaptation off (the fixture first had unedited copies and renames, which ask git nothing, and let the first survive; both now edit a line) |
| One row per stash commit | `a_stash_commit_filed_twice_is_one_row_its_newest_entrys` | the `seen.insert` filter off |
| Title bar: dirty, counts, gone, detached, unborn | `the_title_bar_names_the_repository_the_branch_and_how_far_it_is_from_its_upstream` | `*` always; gone answered as counts; the detached text |
| The porcelain guard | `the_porcelain_read_matcher_catches_the_shapes_it_claims`, `the_porcelain_reads_are_the_three_named_queries` | the stash half unread; `show` unchecked; writing subcommands unchecked; `"pop"` in the read |
| Lines painted once a place | `lines_sharing_the_last_column_are_painted_once_each_place` | (asserts the stroke count) |

**Measured** (`measures_drawing_a_viewport_over_many_open_lanes_and_a_crowded_rows_chips`,
`#[ignore]`d in `crates/cairn-ui/tests/drawn_lanes.rs`; release, median of seven, 1,400 ×
900 window, the CPU raster with PNG encoding — about 26 ms of every paint figure is that
baseline). RR2's fixture laid out as the walk lays it out, each branch tip labelled:

| Open lines | Where | Build and lay out | Paint (before the dedupe) | Paint (after) |
| --- | --- | --- | --- | --- |
| 100 | under every line | 0.22 ms | 28.4 ms | 26.8 ms |
| 1,000 | under every line | 0.39 ms | 50.5 ms | 26.8 ms |
| 5,000 | under every line | 1.05 / 0.76 ms | 146.2 ms | 26.2 ms |
| 5,000 | mid-line | 0.27 ms | 27.4 ms | 26.4 ms |

Under 5,000 open lines a viewport painted 175,000 strokes, almost all onto the last column
the lanes past `MAX_DRAWN_LANES` share: `row_geometry` now paints each place once, by the
line painted there last. Chips: a commit with 10,000 refs lays out 15 chips for 600 px in
0.3 µs. Labels are not the stopping rule's threat to the frame.

**Decided and batched for the user (not approved):** label order (current branch, then
local, remote-tracking, tags — Fork's local-before-remote; tags last where git puts them
before remotes, Fork OPEN 3); compaction by the configured upstream only, read from the
refresh's snapshot (Fork OPEN 4); chips cut by building only what a lower bound fits;
glyphs painted as paths (generic remote a cloud); title-bar counts as Fork prints them
(`18↓ 1↑`, a zero left out, nothing when level) where R7.1 wrote `↓n ↑m`; `HEAD detached
at <short>` and `<name> (no commits yet)`; `upstream gone`; the repository's name the last
component of the opened path (a subdirectory opened names the subdirectory); no `*` for an
unreadable index; the REFS row's chips the row's, in its lane colour, laid out for the
window's width; the title bar keeps "Cairn" and drops the full path (the notice still
shows it); a stash's REFS row its `stash@{n}` chip; and painting each place once.

## 2026-10-07 — phase 06 QA

Fresh qa-checklist (NOT READY on QC1 = TC1), responsiveness-reviewer, test-coverage-auditor,
destructive-ops-reviewer; adjudicated by a fresh qa-confirm. Fixes:

- **TC1 (must fix; CI run 37592390981 failed on it).** The fetch test submitted its fetch after
  reading only the first refresh's refs, so that refresh's count or status could arrive among
  the fetch's updates and come first. It now drains the whole refresh (`one_refresh`, lifted
  to `pub(super)`). Held 40 of 40 runs with 16 busy loops loading the machine.
- **TC2.** The flapping test now counts what each lane drew. Under the amended R10.3 it holds
  refs and ahead/behind to one each and status to one or two; red when status is answered under
  the ahead/behind lane's number (a superseded status drawn twice) and, before the amendment,
  when a refresh was not numbered in the status lane.
- **RR2, the user's decision: status coalesced.** A refresh is numbered in the refs and
  ahead/behind lanes only; a status is sent under the status lane's unmoved number; the refresh
  thread drops every status job queued behind the one it takes up (`refresh_lane::coalesced`).
  Pinned by `a_refresh_leaves_a_running_status_to_finish_and_asks_one_more_after_it` (three
  refreshes during a held status: it is not ended, it is drawn, exactly one follow-up runs) —
  red under "a refresh bumps the status lane again" (the first `git` ended), "no follow-up"
  (one status) and "a follow-up per refresh" (four); and `a_close_ends_a_running_status` (a
  close during it ends the stream within the bound, no status drawn, the process group gone,
  the queued status never run) — red only when both the stopping cancel and the registry's
  `end_invocations` are removed: each alone still ends the status, so either is enough.
- **DO1.** Those two tests are the refresh superseded (now coalesced) and the close during a
  running status, with the process group checked from `/proc`.
- **QC4.** The focus test asserts the chosen commit is among the reopened rows;
  history-graph.md says what the window shows when it never comes back (chosen, no row drawn
  chosen, the pane keeping its answer).
- **RR3.** The root CLAUDE.md names the `Retired` constructors among the worker functions the
  UI thread calls, and the Refresh arm's two bumps.
- **DO2.** git-processes.md says the close ends the refresh thread's status and bounds its reap.
- **RR4.** A reopen asks the new history before handing the old to the worker to free.
- **Follow-up review of the coalescing (fresh responsiveness-reviewer):** a refresh's count
  asked during a running status was served after the follow-up status, behind two; the jobs
  coalescing takes off the queue are now served before the follow-up starts. The coalescing
  test releases each status alone (`$DIR/release.<pid>`) and requires the newest count before
  the follow-up is released, and exactly one count in all — red under "the taken jobs served
  after the follow-up" and "coalescing drops the jobs it takes"; S1-S3 above re-checked.
  Stale docs fixed: `routing.rs`'s refresh comments, history-graph.md's lane rule and freeing
  paragraph, PRD C10's freeing clause, and status.md's cancellation (only a close, in the app).
- **RR1, the user's decision: kept.** The refs stay on the repository thread; the cost is in
  history-graph.md, "Refresh".
- **TC3** handed to phase 08 (state.md).

Dismissed, with reasons:

- TC-note (`rows_answer_or_failure` skips only an open's own refs): no Refresh is submitted in
  the three tests that use it today.
- QC2 (two drawn `reopen: true` answers): cannot occur. A refresh submitted before the window
  reads the first's answer makes that answer non-current, dropped in `Updates::next`; a drawn
  answer submits `OpenHistory` in the same pass, which sets `walked_from` before any later
  refresh compares.
- QC5: information; the first refs failure shows and recovers on focus or F5, as documented.
- QC-VERIFY: ⌘R is Fork's macOS Refresh (`fork-refs-and-status-ui.md`, section 7, and
  `fork-dev/Docs`).

## 2026-10-07 — phase 06: the new lanes, and refresh

Packet mode, on `feature/refs-and-status`. What landed (`docs/systems/history-graph.md`,
"Refresh"):

- **Lanes and threads (R11).** Four new lanes — refs, ahead/behind, status, ref filter — each
  superseding only itself (`each_lane_supersedes_itself_and_a_changes_query_the_file_diff_too`).
  `Request::Refresh` is numbered in three of them as it is submitted, so it supersedes the
  refresh before it lane by lane and no page, diff or filter. Refs are read on the repository
  thread (`worker/history_lane.rs`, which replaces `pool.rs`'s `Scroll`), status and
  ahead/behind on a new `cairn-refresh` thread (`worker/refresh_lane.rs`), ahead/behind handed
  there by the repository thread once its refs are read. The sidebar's filter is the ref-filter
  lane's `Request::FilterRefs` on the repository thread (`RefsSnapshot::matching`); phase 08
  wires its box. A ref's find stays history-lane work, so nothing in the lane table had to
  change shape (the stopping rule did not trigger: four entries were added).
- **Refresh (R10).** On focus gained (`refresh.rs`, a side effect on `Platform::is_app_focused`
  that submits on a false-to-true change — read in the fork at `caa46f8`,
  `crates/freya-core/src/platform.rs`, set by `freya-winit` on `WindowEvent::Focused` and settable
  in `freya-testing`), on every fetch ending, and on `Action::Refresh` — F5 on Linux, ⌘R on
  macOS, Fork's own (`fork-dev/Docs` `keyboard-shortcuts-windows.md` "F5 - Refresh",
  `keyboard-shortcuts-mac.md` "⌘R - Refresh", read 2026-10-06). F5 is no GNOME, KDE or Xfce
  global chord and no Cairn chord, so no stopping rule; the table's every-chord-has-a-modifier
  test now allows a bare function key (batched for the user, state.md). The window's first
  refresh, at open, is what opens the history.
- **Reopen (R10.4, R10.5, R11.3).** `RefsSnapshot::walks_as` compares the refs read with the
  snapshot the walk began from (refs, targets, symbolic targets, `HEAD`, stash list; not
  upstreams or the unreadable count). `Update::Refs { snapshot, reopen }`; on `reopen` the
  window moves its `History` out and retires it (#52), builds
  `History::with_author_capacity(old.author_count())`, asks `OpenHistory`, and keeps the
  selection. `session::reload_if` and the network lane's `ref_tips` comparison are gone, and so
  is `Repository::ref_tips`. Every refresh answer the window replaces, or that was superseded
  unread, is freed on the repository thread.
- **Hand-offs done.** `from_refs` wired; the unborn `HEAD` with no ref answers the empty,
  complete page through the walk itself (`no_walk` and `from_head`'s use gone); a walk error
  arrives from the first page (`history_lane::next_page`); a stale tip — commit gone between
  refresh and open — re-reads the refs and opens once more from them when they differ
  (`an_open_from_refs_gone_stale_reads_them_again_and_opens_from_those`); the author-capacity
  model test; RR2 below.

Pins and the mutation each was checked against (each test red under it, green without; run by
a scratch script, the file restored after each):

| Mutation | Red test |
| --- | --- |
| a refresh also bumps the history lane | `a_refresh_cancels_neither_a_page_being_walked_nor_a_diff_being_read` |
| a refresh also bumps the changes lane | the same |
| a refresh not numbered in the status lane | `a_refresh_asked_twice_at_once_draws_one_answer_of_each` |
| the comparison ignores the stash list / `HEAD` | `a_refresh_reopens_for_a_stash_a_checkout_and_a_moved_ref_and_for_nothing_else` |
| every refresh reopens | the same |
| snapshots compared whole | `a_walk_is_the_same_unless_what_it_draws_changed` |
| a reopen frees the old rows in place | `a_reopen_frees_the_old_rows_on_a_worker_and_keeps_the_selection` |
| a reopen builds `History::new` | the same |
| no re-read on a stale open | `an_open_from_refs_gone_stale_reads_them_again_and_opens_from_those` |
| focus refreshes on any change / never | `gaining_focus_asks_for_a_refresh_and_nothing_else` / `focus_gained_after_a_ref_moved_reopens_the_history_keeping_the_chosen_row` |
| the chord's arm does nothing | `the_refresh_chord_asks_for_a_refresh` |
| a superseded snapshot dropped, not retired | `a_superseded_refresh_comes_back_to_be_freed_on_a_worker` |
| a fetch ending does not refresh / still clears the rows | `every_fetch_ending_asks_for_a_refresh_and_touches_no_row` |
| a replaced status freed in place; a first refs failure not shown | `a_replaced_refresh_answer_is_freed_on_a_worker_and_a_failure_keeps_the_last` |
| `with_author_capacity` ignores its count | `a_history_sized_for_its_authors_appends_them_without_growing_its_index` |
| a reopen while closing | `a_refresh_answered_while_closing_reopens_nothing` |
| an open reads its own refs after a refresh read them | `a_refresh_answers_the_refs_ahead_behind_and_status_each_once` |
| status run on the repository thread before the refs | `a_slow_status_delays_neither_a_page_nor_a_diff` |
| the routing table's status lane on the repository thread | `every_query_is_served_on_the_thread_its_lane_is_routed_to` |

RR2, measured (`measures_layout_over_unmerged_refs`, `#[ignore]`d in
`crates/cairn-git/tests/every_ref.rs`; release, warm, median of seven, the machine in
`docs/research/diff-engine/measured-baseline.md`): a 12,000-commit main line with N one-commit
branches forked at distinct commits down its upper half, each dated after main's tip, so the
walk meets every branch first and holds all N lines open — the widest the graph gets.

| Branches | Rows | Widest | First page (refs read included) | Whole walk | 40 rows' edges: top / under every line / middle |
| --- | --- | --- | --- | --- | --- |
| 100 | 12,100 | 100 | 3.12 ms | 36.3 ms | 0.012 / 0.029 / 0.018 ms |
| 1,000 | 13,000 | 1,000 | 8.37 ms | 61.8 ms | 0.012 / 0.112 / 0.025 ms |
| 5,000 | 17,000 | 5,000 | 25.2 ms | 208 ms | 0.013 / 0.485 / 0.211 ms |

Layout grows with the open lanes as known (the assigner scans them per row): 25 ms for the first
page at 5,000 open lines, against 3 ms at 100 — on the history thread, inside C11's 200 ms. Edge
derivation for a viewport, the UI thread's per-frame share, is under half a millisecond at 5,000
lines crossing it; drawing those lines is phase 07's to measure.


## 2026-10-06 — phase 05 QA

Fresh reviewers (`qa-checklist` READY, `test-coverage-auditor`, `responsiveness-reviewer`)
and a fresh `qa-confirm`, run by the coordinator. Fixed, each pin checked to fail under the
mutation named (run by hand on `17396d5`'s tree):

- RR1, RR4 (`5b81a62`): opening a walk in date order read every tip in gitoxide's
  `Simple::sorting`, one uncancellable call. `walk::open` now reads the tips in its own loop,
  polling the cancel before each but the first, and answers gitoxide's read of each tip from
  the dates read (`Seeded`: a commit holding the date and nothing else, while the walk
  opens; every later read is the object's). A session opens its walk on its first page,
  under that page's cancel. The labels are an `Arc` shared by the request and the walk, not
  copied; whether `HEAD` is already a tip is a map lookup. Pin:
  `opening_a_walk_from_many_refs_is_cancelled_between_tips` (fifty tags, cancelled at the
  sixth poll: `Cancelled { walked: 0 }`, six polls, then the same rows) — fails with the
  open's poll made dead. **Measured** (`measures_the_first_page_from_every_ref`, release,
  warm, median of seven, same machine) on a synthetic repository in the scratchpad: a
  50,000-commit chain from `git fast-import`, a tag on every commit. First page of 64 from
  every ref, snapshot already read: **103 ms** [102–107] (91 ms before, gitoxide reading
  the tips itself, cancellable at no point); with the snapshot's read, 123 ms from packed
  refs and 235 ms from 50,000 loose refs; from `HEAD` 2.6 ms; a first page whose cancel is
  already set answers cancelled in **6.3 ms** (91 ms before). The bench is unchanged: 7.3 ms
  from every ref, 8.6 ms with the snapshot, cancelled in 0.04 ms; a find of its oldest
  commit 2,250-2,257 ms (three runs). C11's 200 ms holds for the walk; the 235 ms with
  50,000 loose refs is the refs read's (phase 01), not a stopping rule.
- QC1 (`2c96d4d`): a stash going directly above the walk's next commit was emitted before a
  newer stash whose date had come. The stash going above now waits for every newer stash
  to be placed or held (`Stream::due` compares against it), newest first and `stash@{0}`
  first on equal dates; and a stash dated the same second as a commit now goes first, as
  git's date order puts a stash above the index commit it was made with. Pins:
  `stash_rows_are_in_gits_date_order` (against `git log --date-order` over the refs and the
  stash commits, on a fixture with an older stash on the walk's first commit and a newer one
  on an older branch, and on `everything()`) — fails with the stash above emitted first; and
  its tie assertion (`17396d5`) — fails with commits first on equal dates (`committed <= time`).
- QC3 (`76ab666`): a stash whose commit cannot be read when the walk resolves has no row,
  the walk going on. Pin: `a_stash_whose_commit_is_gone_since_the_snapshot_has_no_row`
  (the loose object removed after the snapshot) — fails with the read's error propagated.
- TC1 (`9fcd876`): `a_stash_commit_met_while_looking_ahead_is_a_commits_row_and_no_stashs`
  (a branch commit on the stash commit, dated between base and stash; against `git
  rev-list --date-order`) — fails with the look-ahead's check of a pulled commit against the
  stash commit dead; the check over commits already ahead fails
  `a_stash_commit_a_branch_reaches_is_a_commits_row_and_no_stashs`.
- TC2: the deep-base test derives the distance (`AFTER + 1`): at it, the row is at its date;
  one less, above its base — fails with `<` → `<=` in the look-ahead's bound. The module doc
  says the bound is on commits ahead of the next one, in total.
- TC3: `a_page_cancelled_while_looking_ahead_stops_and_resumes_to_the_same_rows` — fails with
  the look-ahead's poll dead (the page lays a row out before the cancel lands).
- TC4: `looking_ahead_stops_once_the_walk_passes_the_bases_date` (a deleted branch's stash;
  the first page pulls the five commits newer than its base and one more) — fails with the
  early stop dead.
- TC5: the deep-base test runs the cold route too (look-ahead 3 and the distance) — fails
  with `resume` taking the default look-ahead.
- TC6: `in_graph_order_a_stash_is_directly_above_its_base` requires stashes on one base
  newest first — fails with `by_base` filed oldest first.
- QC2: the paging test requires the cold route's lanes equal one page's.
- Docs: `docs/systems/history-graph.md` says cancellation is polled per tip read to open
  (and how), the 50,000-tag numbers, the stash order rules, the unreadable stash, and two
  known limits — layout and drawing grow with open lanes and every unmerged ref can open one
  (RR2), and one stash commit twice in the list draws two rows with one identity (QC4).
  Hand-offs in `state.md`: RR2 (measure a wide fixture, 06/07), RR3 (chips clipped at the
  column, 07), QC7 (no seed and an unborn `HEAD` → the `no_walk` page, 06), a ref tip gone
  stale since the snapshot (06), the session's walk errors now arriving from its first page
  (06), QC4 (the user's batch, 07/08).

Dismissed: RR5 — the root `CLAUDE.md` residual names `History::position`, which
`selection::extend` calls (`crates/cairn-app/src/selection.rs`), so it is current; QC6 — the
stash's diff against its first parent with `stash.showIncludeUntracked` unread is already a
batched user decision.

Tooling note: the mutation runner restored each file with its pre-mutation mtime, older than
the mutated build, so a run that changed nothing else in that crate could test the mutated
build. Every mutation run above rebuilt its crate (each wrote a newer file); the runner now
touches the restored file, and every source was touched before the final gate.

## 2026-10-06 — phase 05: the history from every ref, labelled, with stash rows

Packet mode, on `feature/refs-and-status`. `HistoryRequest::from_refs(&RefsSnapshot)`
seeds a walk from every local branch, remote-tracking ref and tag that identifies a
commit, and `HEAD` (a detached `HEAD` that is not a commit seeds nothing); tags on trees
and blobs are filtered out before `walk_tips`; nothing about a stash seeds the walk.
Each row carries the refs pointing at its commit from the same snapshot (`RowLabels`,
`Label`: full name, kind, whether it is the branch `HEAD` is on; and whether the row is
`HEAD`'s). A stash whose base the walk reaches is a row of its own: `RowContent::Stash`
(`StashSummary`: stash commit, `stash@{n}`, base, the stash list's message, author,
author date), `RowId::Stash(stash commit)`, laid out by `LaneAssigner::push_stash` with
its base as its one parent, in a lane of its own. Every `RowContent` reader names the
variant: the window draws a stash's row as its stash commit's row with its message as
subject (phase 07 gives it a chip); `selection::comparison_of` compares a stash's row as
the stash commit against its first parent (`Comparison::Commit(stash)` — R6.2's
`stash^1..stash`); `Pair` keeps both rows' identities, since a stash's row and a
commit's of the same id differ. The test-only `NotACommit` variants are gone. **The
application still walks from `HEAD`** (`Scroll::page`'s `from_head`): reading the
snapshot on the history thread is phase 06's.

**How a stash's base is known to be reached: the walk looks ahead at itself.** The two
history routes lay out one stream (`crates/cairn-git/src/history/stream.rs`). When a
stash's committer date comes up among the walk's commits, the commits already pulled and
not handed on are searched for its base, and up to `LOOKAHEAD` (4,096) more are pulled,
stopping early once the walk passes the base's own committer date. Found: the row goes
at its date. Not found: held, and placed directly above its base when the base is the
next commit to hand on — as is a stash dated older than its base (clock skew). Never
reached: no row. Exact (a row if and only if the walk reaches the base), no separate
walk, nothing pulled in: the commits looked ahead at are the next ones handed on. A
stash commit the walk reaches through a ref is a commit's row, never a stash's. Graph
order has no dates, so every stash goes directly above its base. The cursor carries the
resolved labels and stashes beside its tips, so a cold page resumed from it is the same
stream. Cancellation is polled per row as before and per commit looked ahead at.

**Measured — C11's first page** (`measures_the_first_page_from_every_ref`, release,
warm, median of seven after a warm-up, each run a fresh open; AMD Ryzen 7 9800X3D,
60 GiB, Linux 7.2.8-2-cachyos; `~/Development/bench/rust` at `c999cef531e`, which has no
commit-graph file and no stash):

| Seed | First page (64 rows) [min–max] | Rows laid out | Commits pulled |
| --- | ---: | ---: | ---: |
| `HEAD` | 7.69 ms [7.66–7.83] | 1,088 | 1,088 |
| every ref, snapshot already read | 7.27 ms [7.24–7.34] | 1,088 | 1,088 |
| every ref, with the snapshot's read | 8.56 ms [8.50–8.60] | 1,088 | 1,088 |

Bar 200 ms: met. The stash mechanism, on a `--shared` scratch clone of the bench in the
scratchpad (166 refs), every ref with the snapshot already read: no stash 7.40 ms; a
stash on `HEAD` 7.40 ms (1,087 commits pulled, the row at its date); plus a stash whose
base is `HEAD~20000` (2017) 21.60 ms (4,096 pulled; the row held and drawn directly
above its base); plus a stash on a deleted branch's commit dated 2015 21.53 ms (4,096
pulled; no row). With the clone's own commit-graph (written by git's `gc --auto` there)
the worst case is 3.09 ms. The stream never holds more than 4,096 commits ahead of the
next row, so that is the most looking ahead adds to a page.

**C16 re-measured** (`measures_compact_rows_over_a_named_repository`, mode `find`, same
machine, release, a fresh process per run, median of seven after a warm-up), paging all
of rust-lang/rust: seed `snapshot` (the new walk: labels, stash rows) **52.67 MiB**
retained, 159 B/row over 345,545 rows (rows 23.77, text 16.01, lane changes 12.25,
snapshots 0.25, authors 0.371, labels 0.006, stashes 0.000 MiB), `RssAnon` +83.2 MiB;
seed `refs` 52.66 MiB; `HEAD` 51.87 MiB over 340,228. Against 64 MiB: met. Find times:
`snapshot` 2,265 ms [2,241–2,275], `refs` 2,253 [2,245–2,264], `HEAD` 2,211
[2,205–2,221] — about 2% above phase 04's 2,206 / 2,169, inside C15's 10% of 2.4 s. On the
scratch clone with its two drawn stashes: 52.67 MiB, stashes 0.003 MiB. **C15**
equivalence on the bench still holds: seeds `refs` and `snapshot` 345,545 rows (49,609
repainted, 627 late lines), `HEAD` 340,228 (48,988, 624), every row's derived edges the
frozen assigner's.

**Bench hygiene — two metadata-only touches, reported.** (1) The scratch clone was made
with `git clone --shared`, so its objects live in the bench's pack through alternates;
when `git stash` and `git commit` in the clone wrote objects the bench's pack already
holds, git freshened that pack's mtime (`.git/objects/pack/pack-9359bf25….pack`, now
2026-10-06 10:19:46; content unchanged). (2) A plain `git status` run on the bench to
check it was clean created and removed `index.lock`, moving the `.git` directory's mtime
(10:27:19); the index itself is unchanged (2026-09-17), and no lock was left. No object,
ref, index or config of the bench changed. Next time: a scratch clone with no
alternates (a clone across filesystems, as phase 02's on tmpfs), and
`GIT_OPTIONAL_LOCKS=0 git status`.

Pinned (mutation each was checked to fail, run by hand on `51b2a21`):

- `the_commits_walked_from_every_ref_are_git_rev_lists` (C6): stash bases seeded
  (also fails 3 other C6 tests); stash commits seeded (`refs/stash`, fails 6); a tag's
  object rather than its commit seeded (fails 5); never placing a stash directly above
  its base.
- `every_rows_labels_are_what_git_log_decorates_it_with` (C6): symbolic refs dropped from
  labels; `current` never set; `HEAD` never labelled (also
  `a_detached_head_no_ref_reaches_is_walked_and_labelled`).
- `each_stash_on_a_walked_commit_is_one_row_with_one_line_to_it` (C6): a stash laid out
  with `push` (lines join, both stashes on one commit in one lane); never at its date
  (always held; also the deep-base test); a stash placed above its base only once held.
- `a_walk_paged_any_way_draws_the_rows_one_page_does`: a cursor that forgets the
  decoration (cold pages lose their stash rows and labels).
- `a_stash_commit_a_branch_reaches_is_a_commits_row_and_no_stashs`: the walked-as-a-commit
  check dropped.
- Assigner (C6's "assigner tests"): `two_stashes_on_one_commit_each_keep_a_lane_down_to_it`
  and `rows_with_stashes_derive_the_edges_the_assigner_kept` fail with `push_stash`
  joining as `push` does; `a_stash_draws_one_line_down_to_the_commit_it_was_made_on`
  pins the one line and its derived edges.
- Model: `a_row_held_after_one_that_could_not_be_reads_its_own_labels_and_stash` (the
  first entry filed under a row number read rather than the last);
  `each_row_reads_back_its_own_labels_and_no_others` (labels read from the row before);
  `a_stash_row_reads_back_as_its_stash_and_is_found_by_its_identity` and
  `a_consumer_reads_a_row_by_matching_on_its_content` (the stash flag dropped); the kept
  row is still 72 B (`a_kept_row_is_seventy_two_bytes`: the flags byte fills the one byte
  of padding).
- Window: `a_stash_row_draws_its_message_and_asks_what_it_changed_on_its_base` fails
  with a stash compared against itself, with `Pair` keying rows as commits, and with the
  stash's row drawn without its message.

Every walk in C6 is paged to its end over fixtures of eight to twenty-odd commits,
git run in the same fixture at test time (`rev-list --branches --remotes --tags HEAD`,
`log --decorate=full --format=%H%x1f%D`, `stash list --format=…`, `rev-list
--date-order`).

Decided here, batched for the user (not approved):

- **Looking ahead, 4,096 commits.** A stash's row is at its date when the walk meets its
  base within 4,096 commits of that date (or before passing the base's date); otherwise
  directly above its base. R4.2's "merged by its commit time" is therefore exact only
  for a near base; the QA brief's "at its date or directly above its base" is what holds
  for a deep one. The alternative — a reachability walk per stash from the seeds — costs
  a whole-history walk for every stash whose base is unreachable (a deleted branch's,
  the common case Fork's rule exists for).
- **Each stash keeps its own lane down to its base**, never joining a line already
  descending to it, so several stashes on one commit each take a lane (Fork's
  screenshots), where a commit's line joins at once.
- **A stash commit some ref reaches is a commit's row and no stash's**, labelled by that
  ref; its index and untracked commits are then rows too, as git walks them.
- **A stash's row's subject is the stash list's message, its date the stash commit's
  author date; it is placed by its committer date**, the walk's own order.
- **Labels are in the snapshot's order** (local branches, remote-tracking refs, tags,
  each by name); a row carries full names, its kind and whether it is current. Upstream
  pairs for compact labels (R5.2) are phase 07's to read from the snapshot.
- **A stash's row is drawn as a commit row with its message as subject** until phase
  07's chip, and selected it asks the stash commit's changes against its first parent;
  `stash.showIncludeUntracked` (R6.2) is not read yet.

## 2026-10-06 — phase 04 QA

Fresh reviewers (`qa-checklist` READY, `test-coverage-auditor`,
`responsiveness-reviewer`) and a fresh `qa-confirm`, run by the coordinator: nine
confirmed, none dismissed. Fixed:

- TC1: `authors_whose_keys_collide_are_told_apart_by_name` files three names under one
  key through `author_of`'s own path (`author_under`, the key given); fails with a new
  author not chained to the one already under its key (`next: NO_AUTHOR`), with the name
  check dropped and with the chain not walked.
- TC2, QC3: `what_a_history_retains_counts_every_chunk_whole` asserts the snapshot and
  author terms exactly, over a snapshot with an open lane and a late line; fails with the
  author index, the open-lane store or the late-line store dropped. The index's share is
  now an estimate of hashbrown's table (buckets from its capacity, an entry and a control
  byte each, one group past the end; `the_author_index_is_estimated_by_its_buckets`), and
  `RetainedBytes` says it is an estimate. Re-measured on rust-lang/rust from every ref
  (same machine, release, one run): authors 0.371 MiB (was 0.324), total 52.66 MiB
  against C16's 64.
- TC3: a run of exactly a chunk now takes a chunk of its own (`len >= CHUNK`) and leaves
  the chunk being filled to the runs after it, the boundary chosen and pinned by
  `a_run_exactly_a_chunk_long_leaves_the_chunk_being_filled_alone` (fails with `>`).
- QC1, TC4: a full history ends the scroll. `Progress::appended` sets the stream ended,
  so nothing more is asked and a later page cannot flip the window back to ready
  (`a_full_history_stops_asking_and_keeps_saying_so`, failing without either). The
  model's partial-page contract is reached by naming every address of the text store
  (`a_page_past_the_historys_limit_keeps_the_rows_before_it_and_says_so`, failing with a
  row pushed before its text was held).
- TC5, QC2: the window's table is said to be committed with the slimming (`1b0ca9f`)
  from a run against `4205d5d`, and
  `the_windows_table_agrees_with_the_engines_table_captured_before` holds it to
  `CAIRN_BEFORE`, committed at `4205d5d` (fails with a subject edited).
- QC5: the window's check pages as the window does until every commit of `main`'s is
  held or the history ends; `drawn_rows.rs`'s module comment wrapped.
- QC4: `a_kept_row_is_seventy_two_bytes` cited in `src/history.rs`.
- RR1 (docs): the root `CLAUDE.md`'s `cairn-model` row and residual, and the systems
  doc, say the author index is a standard hash map that doubles, rehashing every author
  so far at each doubling (0.48 ms at 57,000), bounded by distinct authors. Pre-sizing a
  reopened history is handed to phase 06 in `state.md`; the doubling itself is in the
  user's batch.
- RR2 (docs): the root `CLAUDE.md` no longer calls `index_of`'s fallback unreachable —
  after a parent link or a pair let go, the next arrow key scans every loaded id once.

## 2026-10-06 — phase 04: slim rows

A kept row is 72 bytes of plain data (`StoredRow`, private to
`crates/cairn-model/src/history.rs`): its commit's id once, a parent count (`u16`,
saturating), a subject span of the history's text store, an author number in its
author table, the author date, the lane, the span of its lane changes and, on a row
with one, its snapshot's number. It is `Copy`, asserted at compile time beside it, so
no kept row owns a heap allocation. The window keeps a `History` whose stores grow in
fixed chunks, never by doubling (`crates/cairn-model/src/chunked_store.rs`: rows 1,024
to a chunk, text 64 KiB, lane changes 4,096; a run never splits across chunks, a run
longer than a chunk gets its own; the list of chunks grows by one). A page crosses as a
`RowsPage` (flat vectors: its rows, its own text, its authors once each, its lane
changes and snapshots) and is appended in `session::apply`, where pages were applied
before. Every reader reads through the history (`HistoryRow`); the list hands a row's
callback only its content and edges (`RowRender { content, graph, .. }`), which closes
RR1. `Lane` is held in 32 bits, so a `LaneChange` is 16 B rather than 24. No production
view needed a row's full parents or its author's address — the Commit tab reads both
from the details query — so no stopping rule fired. Packet mode, on
`feature/refs-and-status`.

**C16's comparison, written before the old fields went.** Commit `4205d5d` pinned what
the rows drew while every row still carried its full parents and its author's address:
`crates/cairn-git/tests/slim_rows.rs` (a crafted fixture of ten `git commit-tree`
commits — an empty subject, one longer than a text chunk, one in ISO-8859-1, one of
bytes that are not UTF-8, a name past ASCII, a merge, an octopus, a root — paged two at
a time so pages bring new and known authors; and the Cairn checkout's walk from
`0cfd746`, `main` as it stood) and `crates/cairn-ui/tests/drawn_rows.rs` (the list's
labels and node centres over crafted rows). The window's own check over the Cairn
checkout, `the_cairn_checkouts_rows_draw_what_they_drew_before_rows_were_slimmed`
(`crates/cairn-app/src/window.rs`), has its table read by the same test built at
`4205d5d` in a scratch worktree. `1b0ca9f` then slimmed the rows; every table is now
held against the slimmed rows, beside live oracles (`git log`'s `%s %an %at %P` over the
Cairn checkout from `HEAD` and a braided fixture; the details query's author, date and
parents). The details query's subject is the message's first line by design
(`CommitDetails::subject`), where a row's folds the first paragraph as `%s` does, so it
is not used as a subject oracle.

**C16, measured** — release build, warm, median of seven after a warm-up, fresh process
per run; `~/Development/bench/rust` at `c999cef531e` (read only: its `.git` listing
identical before and after, nothing newer than a marker file, `git status` clean); AMD
Ryzen 7 9800X3D, 60 GiB, Linux 7.2.8-2-cachyos. Reporter
`measures_compact_rows_over_a_named_repository`, mode `find`, paging the held session 64
rows at a time and appending every page to one `History`:

| Seed | Rows | Retained (capacity, every chunk whole) | Rows | Text | Lane changes | Snapshots | Authors and index | `RssAnon` growth | Find |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| every ref | 345,545 | **52.61 MiB** (159 B/row) | 23.77 MiB | 16.01 MiB | 12.25 MiB | 0.25 MiB | 0.32 MiB (8,424) | 83.1 MiB | 2,206 ms |
| `HEAD` | 340,228 | 51.82 MiB (159 B/row) | 23.42 MiB | 15.76 MiB | 12.07 MiB | 0.25 MiB | 0.32 MiB (8,423) | 82.0 MiB | 2,169 ms |

Against phase 03's 167.9 MiB from every ref (row vector 108.0, lane changes and
snapshots 18.6, parents 14.3, text 27.0) and C16's 64 MiB: met, with the reporter
asserting it. Find runs (ms, warm-up excluded): every ref 2198.6-2220.9, `HEAD`
2157.3-2178.1 — faster than phase 03's 2,263 / 2,210 and well inside C15's 10% of
2.4 s. C15's equivalence over the bench, mode `equivalence` (the walk's parents now read
from the details query, as anything needing parents does): every ref 345,545 rows,
49,609 repainted, 627 late lines; `HEAD` 340,228, 48,988, 624 — every row's derived
edges equal to the frozen assigner's, as in phase 03.

**Reading rows through the stores, per frame.** `window_check` (release, the same
session, three runs each interleaved with `4622a76`, phase 03's last commit), the
history scrolled 9 rows a frame: medians 1.30 / 1.33 / 1.31 ms before and 1.34 / 1.37 /
1.38 ms after, maxima 1.95 / 2.14 / 1.90 and 1.86 / 1.86 / 2.18 ms; no frame over
16.7 ms either way. About 0.06 ms a frame more at the median, 0.4% of a frame's budget,
and no frame lost: judged not to be the stopping rule's "costs a frame measurably", and
batched for the user. Applying a page: median 0.007-0.008 ms before, 0.008-0.009 ms
after.

Pinned (mutation each fails, run by hand on `1b0ca9f`):
`every_row_draws_what_it_drew_before_rows_were_slimmed` and
`the_cairn_checkouts_rows_draw_what_they_drew_before_rows_were_slimmed` (fail with
`CommitRow` handing the node a parent count of 0, and with the list handing a row's
callback the next row's content); `every_crafted_row_draws_what_it_drew_before_rows_were_slimmed`
and `every_row_of_the_cairn_checkout_draws_what_it_drew_before_rows_were_slimmed` (fail
with `History::append` keeping a page's author number rather than the history's, with
every author added again on every page, and with a subject read a byte short);
`a_page_of_new_authors_and_a_page_of_known_ones_both_draw_and_each_is_named_once` (the
page's author number kept; every author added again);
`authors_whose_keys_collide_are_told_apart_by_name` (the name check dropped; the chain
not walked); `what_a_history_retains_counts_every_chunk_whole` and the store tests
(chunks counted by length rather than capacity);
`appending_ten_thousand_rows_allocates_per_chunk_never_per_row` (39 allocations for
9,936 rows; fails with a new chunk for every run);
`a_history_draws_what_the_rows_it_was_given_draw`,
`the_cairn_checkouts_rows_draw_what_the_assigner_drew_before_compaction`,
`each_cold_page_and_resumed_session_draws_on_its_own_what_the_assigner_drew_for_it` and
`rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew` (a page's snapshots
dropped on the way into the history). The `Copy` assertion stops compiling with a
`String`, `Vec` or `Box` in the row.

Decided here, batched for the user (not approved):

- **Authors are numbered on the window's side.** A page names each of its own authors
  once; `History::append` finds each in the history's table (a hash of the name, names
  that hash alike chained) and adds only those it lacks. R4.7's letter is a page that
  carries "any authors new to the history"; numbering on the worker would make every
  page depend on the window having applied every page before it, which a superseded page
  or a cold restart could break silently. The cost is hashing at most a page's authors
  on the UI thread per page, and the index (counted in the 0.32 MiB above).
- **A parent count saturates at 65,535**, which only decides whether a ring is drawn.
- **A history too large for its 32-bit addresses** (about 4 GiB of subjects, or four
  billion rows or lane changes) answers `HistoryFull`, which the window shows as the
  page's failure; the rows appended before it stay.

## 2026-10-06 — phase 03 QA

Fresh reviewers (`qa-checklist` READY, `responsiveness-reviewer` with no findings,
`test-coverage-auditor`) and a fresh `qa-confirm`, run by the coordinator.

Confirmed findings fixed:

- TC1: `a_rows_heap_bytes_count_what_its_snapshot_holds` — a row carrying a snapshot of
  three words and a late line, its exact heap bytes pinned; fails with the snapshot's own
  `heap_bytes` multiplied by zero in `GraphRow::heap_bytes`, which every earlier test (empty
  snapshots only) let through.
- TC2: `the_reference_assigner_is_the_one_compact_rows_were_checked_against` — an FNV-1a
  fingerprint of C15's reference below its module documentation, taken while it was still
  `f34631c`'s verbatim (no change to it since `f94fbdf`); fails on a one-character edit to
  its code, and not on an edit to its documentation. No dependency added.
- TC3 (reporter half): `measures_layout_over_every_ref_of_a_named_repository` counts rows
  whose edges could not be derived, prints the count and fails on any; fails with the
  reporter asking for the row below.
- QC-A3: the systems doc's twins table names the cold-page and resumed-session test; its
  opening paragraph is re-wrapped.
- RR1: deferred to phase 04 (`state.md`): `RowRender` clones the whole row per built row.

Dismissed, with reasons:

- QC-A2: the `u32` saturation in `connect_upward` cannot be reached — both rows of a late
  line are in the window's `VecDeque`, production never calls `with_window`, and the
  default window is 1024 rows.
- RR2: deriving O(viewport × K) per frame is the design, measured at about 0.5 ms a frame
  at worst.
- RR3: the 100,000-row viewport twin counts rows built, by design; derivation at real
  snapshot spacing, scrolled deep and back, is pinned by
  `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew`.
- TC2's `EdgeSegment`-constructor half: the constructors have their own unit tests, and the
  literal-edge layout tests are independent of the oracle.
- TC3's UI-fallback half: the node-alone fallback in `build_row` is commented, required by
  the no-panic rule, and its precondition (a snapshot within reach on every row the
  assigner lays out and a reader keeps) is pinned by the model's tests.

## 2026-10-06 — phase 03: compact rows

A row keeps its id, lane and only the lane changes at it (`LaneChange`: a line ends at
its commit, starts from it, or starts late — out of order — to a child laid out below);
every 64th row, and the first row a reader keeps, carries a `LaneSnapshot` of the lines
crossing into it; `cairn_model::row_edges` derives a drawn row's edges from the nearest
snapshot above it, advanced through at most 63 rows. Packet mode, on
`feature/refs-and-status`. No stopping rule fired: no row's derived edges differ from
the old assigner's anywhere they were compared.

**C15's oracle and how it was written first.** `crates/cairn-model/tests/layout_before_compaction/mod.rs`
is `f34631c`'s lane assigner verbatim (two names changed, checked by a `diff`), kept as
test code. Commit `f94fbdf` added the compact form while the old `edges` field still
rode on every row, and its equivalence test compared each row's derived edges with that
field and with the frozen copy; `7c23805` ran the same over the Cairn checkout and, in the
`#[ignore]`d reporter, over every row of the bench repository from every ref (345,545
rows, 49,609 of them repainted, 627 late lines) and from `HEAD` (340,228 rows, 48,988
repainted) — every row equal to the old storage and to the frozen copy, edge for edge and
in order. `097af10` then removed the field; the tests keep comparing against the frozen
copy. The assigner still keeps every edge for the rows inside its window (a repaint's
lane is chosen from them), so its layout is today's; only what it hands out changed.

**Where derivation runs, and K: at draw time, K = 64.** `HistoryList` derives each row
it builds (`row_edges` in `build_row`), from at most `K - 1` rows above it. Measured
with the reporter's `derive` mode (`measures_compact_rows_over_a_named_repository`,
release build, warm, one process, every row exactly `K - 1` below its snapshot over the
whole bench history from every ref; mean 106 edges a row):

| K | Rows timed | Per row p50 | p99 | Max | Snapshots held |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 345,545 | 0.23 µs | 0.41 µs | 9.9 µs | 19.81 MiB |
| 8 | 43,193 | 0.29 µs | 0.65 µs | 15.1 µs | 2.48 MiB |
| 16 | 21,596 | 0.38 µs | 0.86 µs | 15.0 µs | 1.24 MiB |
| 32 | 10,798 | 0.55 µs | 1.34 µs | 13.7 µs | 0.62 MiB |
| **64** | 5,399 | **0.76 µs** | **2.0 µs** | **14.1 µs** | **0.31 MiB** |
| 128 | 2,699 | 1.26 µs | 3.4 µs | 14.8 µs | 0.15 MiB |
| 256 | 1,349 | 2.82 µs | 7.1 µs | 21.0 µs | 0.08 MiB |
| 1024 | 337 | 8.34 µs | 24.4 µs | 36.8 µs | 0.02 MiB |

A 1440×900 window shows 35 rows at 26 px; even with every one at the worst row timed
(14.1 µs) a frame's derivation is 0.5 ms of the 16.7 ms budget, and 0.07 ms at the p99.
K = 64 keeps snapshots at 0.31 MiB, which leaves C16's 64 MiB to phase 04, and matches
the application's page (`PAGE_ROWS`), so every page from the first starts on a snapshot.
Deriving on the worker per viewport would add a round trip to every scroll for work that
costs microseconds on the UI thread; not done. The real window agrees: `window_check`
(release, one run each, same session) scrolled the history 9 rows a frame in 1.31 ms
median, 1.89 ms max with compact rows, against 1.27 / 1.99 ms at `f34631c`.

**C15 numbers** — release build, warm, median of seven after a warm-up, fresh process per
run, interleaved with the old rows' runs; `~/Development/bench/rust` at `c999cef531e`
(read only: its `.git` listing identical before and after, nothing newer than a marker
file, `git status` clean); AMD Ryzen 7 9800X3D, 60 GiB, Linux 7.2.8-2-cachyos. A find of
the oldest commit (`c01efc669f0`, the last row) pages the held session 64 rows at a time
keeping every row, as the window does. "Before" is `deep-find-measured.md`'s appendix
reporter built at `f34631c` and run in the same session:

| Seed | Rows | Before: find | After: find | Change | Before: retained (capacity) | After: retained (capacity) | `RssAnon` growth before / after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| every ref | 345,545 | 2,495 ms | 2,263 ms | −9.3% | 1,427 MiB | 167.9 MiB | 1,446 / 185.7 MiB |
| `HEAD` | 340,228 | 2,453 ms | 2,210 ms | −9.9% | 1,392 MiB | 167.0 MiB | 1,410 / 184.4 MiB |

Runs (ms, warm-up excluded): every ref before 2486.7-2508.7, after 2255.8-2269.7; `HEAD`
before 2430.4-2461.6, after 2204.7-2227.1. C15's bar — no more than 10% slower than
2.4 s — holds; it is faster, since a row no longer allocates its edges. The retained
figure counts capacity: the row vector's capacity (524,288 rows × 216 B = 108.0 MiB,
its growth slack included), lane changes and snapshots 18.61 MiB (5,400 snapshots),
parents 14.3 MiB, text 27.0 MiB. That is the figure phase 04 starts from (the plan
expected about 160 MiB). A `LaneChange` is 24 B; nothing shrinks it in this phase.

Pinned (mutation each fails, run by hand): `every_row_draws_the_edges_the_assigner_retained_before_compaction`
(crafted fixtures and 1,590 generated histories, windows 1-1024, K 1-64; fails with ended
late lines never retired, ending lanes never closed, the late-line merge reversed,
late lines ranked by end row only, snapshots without late lines, snapshots taken after the
row, a late span one short, ending lanes also drawn passing, and late ranks not recorded),
`two_late_lines_cross_a_row_in_their_childs_parent_order`,
`a_snapshot_falls_every_k_rows_and_on_the_first_row_kept` (fails with no snapshot on the
first kept row), `rows_kept_from_any_row_on_draw_without_the_rows_above_them`,
`a_row_with_no_snapshot_within_reach_draws_nothing_rather_than_guessing`,
`a_row_further_below_its_snapshot_than_any_interval_draws_nothing`,
`the_lanes_kept_rows_name_are_the_lanes_their_edges_reach` (fails with a snapshot's lanes
uncounted), `the_cairn_checkouts_rows_draw_what_the_assigner_drew_before_compaction` and
`each_cold_page_and_resumed_session_draws_on_its_own_what_the_assigner_drew_for_it`
(fails with `drawn_from` dropped from either the cold page or the session), and in the
list `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew` (fails deriving from
the row above). The viewport twin, `only_a_viewport_of_rows_is_built_however_long_the_history`,
still holds.

## 2026-10-06 — phase 02 QA

Fresh reviewers (`qa-checklist`, `test-coverage-auditor`, `destructive-ops-reviewer`,
`gate-integrity-reviewer`) and a fresh `qa-confirm`, run by the coordinator.

**The user accepted the two-read scheme for `status.showUntrackedFiles` (2026-10-06):** a
first `git status` with no `--untracked-files`, a second with `--untracked-files=all` only
where the first collapsed an untracked directory. Now a locked decision (`state.md`).

Confirmed findings fixed:

- DO1: `in_a_partial_clone_a_status_read_fails_rather_than_fetching` — a blob-less clone
  whose sparse checkout left a blob with the promisor, and a staged inexact rename of it:
  on git 2.44+ `Error::GitFailed`, nothing listed, no pack written; below 2.44 the read
  fetches and answers, pinned as the floor's residual. Fails with the read's
  `GIT_NO_LAZY_FETCH` set to `0`. The docs (`reads/status.rs`, `docs/systems/status.md`)
  say the whole read fails; the residual is in PRD R3.8 and the root `CLAUDE.md`. No new
  model variant, retry or fetch (the read policy of 2026-10-02).
- DO2: `a_status_read_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor` — an
  untracked cache, an fsmonitor token and a split index, each one a locked status
  rewrites, and a caching textconv, an external diff, a driver `command`, a smudge filter
  and a `post-index-change` hook: the git directory byte-identical (the shared index's
  mtime excepted, as documented), only the clean filter and the hook's token answer ran.
  Fails with the read's `GIT_OPTIONAL_LOCKS` set to `1`. Found building it: git 2.30.9 and
  2.32.7 merge a split index when a locked status under `core.fsmonitor` rewrites the index.
  Named in `CLAUDE.md`'s pin list and `docs/systems/status.md`.
- TC1: `a_status_read_superseded_during_its_second_read_ends_it` — fails (after the
  stub's 30 s) when the second read is given a cancel that never fires.
- TC2: rename and unmerged records with submodule fields; fails with the field dropped
  from either; the integration tests compare a conflicted entry's submodule state too.
- TC3: the index walk panics on an unreadable directory, and each read requires the
  superproject's index and each named submodule's among those compared (fails with the
  walk skipping `modules/`, or matching no index).
- QC-F2: the docs say a setting changed between the two reads is not seen.
- QC-D1/GI1, QC-D2: "real-git diff and status tests" in `CLAUDE.md`, `docs/qa-gate.md`,
  `scripts/gate.sh` and `scripts/git-floor.sh`; the banner reflowed; floors raised.
- QC-F4: commit 3b56314 (`fix(app)`) belongs to phase 02 though it touches `cairn-app`:
  `an_identical_ask_is_answered_from_what_is_kept` took the newest commit's first
  modified file, which phase 02's docs commit made `CLAUDE.md` — past the size limits, so
  answered without a patch read — and the test failed on the checkout's history, not on
  the worker. It now searches for a file whose diff asks git, on a handle of its own.

Dismissed, with the adjudicator's reasons:

- QC-F3 (and the destructive-ops note on it): a nested repository makes every status read
  twice — the documented, measured cost of the scheme the user accepted; phase 06 may
  weigh it.

For the user's end batch: what a failed status read does to the view (`state.md`).

## 2026-10-06 — phase 02: the status engine

`Repository::status` — `git status --porcelain=v2 -z` as a read, parsed into
`cairn_model::WorkingTreeStatus` (`docs/systems/status.md`). Packet mode, on
`feature/refs-and-status`. No stopping rule triggered.

**`status.showUntrackedFiles`, read by git itself (decided here; for the user's
review).** The first read passes no `--untracked-files`, so git reads the setting; only
where its answer collapsed an untracked directory (a `?` record ending `/`) is the status
read again with `--untracked-files=all`, whose answer is the answer. No `git config` read,
so no porcelain read beyond `status` was added and the stopping rule did not apply. Why
not gix's reading: besides the `includeIf` and system-file divergences behind
`reads::fetch_settings`, the value's language differs by git version — git 2.56 takes
`false`/`true`, 2.30.9 dies on both as a bad config (reproduced). Cost: a second read
wherever an untracked directory or a nested repository exists (C11 below: 66.7 ms
against 34.6 ms for one read).

**Found while building:**

- The sparse index (`sdir`) is read by git from **2.32.0**, not 2.31: `read-cache.c` at
  v2.31.0 has no `CACHE_EXT_SPARSE_DIRECTORIES` (checked in git's source). R3.7's state is
  a failed read on a git before 2.32 over an index gix reads as sparse.
- `git diff --name-status` is not an oracle for submodules: on 2.32.7 and 2.56.0 it leaves
  out a submodule whose only change is untracked content, which status reports as `S..U`
  (2.30.9's `git diff` includes it). The submodule oracle is the submodule's own state.
- An oracle `git diff -C` under `diff.renames=copies` is "find copies harder"; the oracle
  runs with `-c diff.renames=false` so its flag alone decides.
- git does not verify the index checksum on a read, so an index cut short can be read as
  garbage (a record with an empty path, which the parser refuses) rather than failing; the
  failing-read test breaks the `DIRC` signature instead.
- Porcelain v2 prints no `#` header for `status.branch` or `status.showStash` set in
  configuration (2.30.9, 2.32.7, 2.56.0); the parser skips a header anyway.

**C11, status half** — release build, warm, median of seven [min–max], through
`Repository::status` with the application's `git` (2.56.0), reporter
`measures_the_status_read`; same machine as phase 01. The bench on btrfs, read only (its
index and locks checked unchanged); the clone on tmpfs. Beside the spike's `git` (spawn +
parse, `status-agreement-spike.md`):

| Scenario | Entries | Reads | Cairn | Spike's git |
| --- | --- | --- | --- | --- |
| bench, clean (bar 100 ms) | 0 | 1 | 29.1 ms [28.9–30.0] | 28.8 ms (`-unormal`) |
| scratch clone, clean | 0 | 1 | 24.3 ms [23.8–25.6] | 27.2 ms (`-unormal`) |
| 1,000 modified + 10,000 untracked in 1,000 tracked directories (bar 250 ms) | 10,980 | 1 | 34.6 ms [33.8–43.8] | 26.3 (1,000 modified) and 34.1 (10,000 untracked), measured apart |
| the same, untracked in 1,000 new directories (`-uall` second read; new data) | 11,000 | 2 | 66.7 ms [64.5–78.0] | — |
| every tracked file's stat changed (recorded, not barred) | 0 | 1 | 758.9 ms [756.1–761.3] | 736.2 (`-uno`), 765.6 (`-unormal`) |
| that read superseded at 100 ms | — | 1 | answered cancelled at 100.9 ms; registry empty; logged cancelled | — |

Both bars met. 20 of the 10,000 untracked files fall in ignored directories.

## 2026-10-06 — phase 01 QA

Four fresh reviewers (`qa-checklist`, `test-coverage-auditor`,
`destructive-ops-reviewer`, `gate-integrity-reviewer`); a fresh `qa-confirm`
adjudicated. Confirmed findings fixed:

- GI1, TC10: the reftable twin reads each test only to its closing brace, through a
  matcher (`required_skip_violations`) with a self-test spelling the deleted SKIPPED
  line, the requirement moved to the next test, and an idle `if .. {}`.
- TC1, TC16: the cancelled ahead/behind test cancels inside a 2,000-commit frontier
  paint and requires reads to stop there (`AheadBehindCancelled` now carries
  `commits_read`); it fails with `Polled`'s poll removed (4,023 commits read). A cancel
  between two branches answers `branches: 1`.
- TC2: a loose chain of three tags, peeled object by object; a one-hop peel fails it.
- GI3, GI4, GI6: the setting test's git half skips aloud and is required too; the gate's
  note and `docs/qa-gate.md` name the probe and its twin; "exactly as the test does"
  reworded.
- TC3, TC4, TC5, TC6, TC8, TC9, TC12, TC13, TC14, TC15: the retarget is the only change
  between two reads; chains at and past git's depth; a cancel at every poll of a refs
  query, with the poll count exact; exact costs on a small fixture; a damaged stash
  reflog; ahead/behind in a shallow clone; version 0 with `reftable` and a linked
  worktree of a reftable repository; a merge on the upstream side; the one global key
  read pinned locally in fixtures; the vacuous worktree claim dropped.
- QC-F3: the upstream pass polls the cancel per branch.
- QC-F5: committer dates running against the graph — gix's count equals git's there.
- Probes, both confirmed and fixed against git: QC-F1, a repository with no `config`
  file opens as git and gix open it; QC-F4, `HEAD` through a symbolic branch is the end
  of the chain, unborn when that end is missing (git's `symbolic-ref` follows every
  level). Found while fixing TC4: `%(symref)` names the END of a symbolic chain, and
  git reads at most five refs, the ref itself included — the snapshot read one hop too
  many and named the first hop; both fixed, pinned against git.
- Found while fixing TC8: git numbers a stash list so that an unparseable reflog line
  takes no number while a missing commit keeps its own; the snapshot gave the bad line
  a number; fixed, and the oracle now reads git's own `%gd`.
- DO1, DO2, QC-F2: docs say what `ref_tips` compares, that a gix configuration re-read
  restores the namespace, and that gix's open reads `HEAD` before the refusal.

Dismissed, with the adjudicator's reasons:

- GI2: an inherited residual already stated in the root CLAUDE.md, tracked by open
  issue #58, whose generic fix covers this probe too.
- GI5: CI's `test-full` runs the gate's probe, matching the fsmonitor precedent.
- GI7, TC11, and QC-F7's floor half: every floor-run pattern the twin misses fails
  loudly (a floor git cannot make reftable, so the required test goes red), never
  silently.
- DO3: the gitoxide-mutation twin already scans these files, and they only read.
- DO4: `ref_tips` was uncancellable before, runs off the UI thread, costs 28.4 ms at
  10,000 refs, and phase 06 replaces it.
- TC7: a deliberate two-read determinism check.

For the user's end batch: QC-F6 (invalid ref names not counted), in `state.md`.

## 2026-10-06 — phase 01: the refs engine

The refs snapshot, upstreams, the stash list and ahead/behind in `cairn-git`, the
reftable refusal at open, and their vocabulary in `cairn-model`
(`docs/systems/refs.md`). Packet mode, on `feature/refs-and-status`.

**Stopped once for the user** (a phase-01 stopping rule): gix's upstream API
disagreed with `%(upstream)` on three cases the four parity rules did not cover —
two `branch.<n>.merge` values (gix takes the last, git the first), a short merge with
a named remote (gix expands and maps it, git maps nothing), and two fetch refspecs
both mapping the merge (gix does not take the first). The user chose resolving every
upstream by hand from the configuration (a fifth parity rule, PRD R1.4, L3); kept a
ref naming a missing object skipped and counted (a divergence from git's `fatal:
missing object`, R1.3); and had a format-version-0 repository with
`extensions.refStorage` refused at open, as git refuses it (R1.9).

**C11, refs half** — release build, warm, median of seven, on the machine in
`docs/research/diff-engine/measured-baseline.md` (AMD Ryzen 7 9800X3D), reporter
`measures_the_refs_snapshot_and_ahead_behind`:

| Repository | Refs | Refs snapshot | Snapshot + every ahead/behind |
| --- | --- | --- | --- |
| rust-lang/rust at `c999cef531e` (`main` equal to `origin/main`) | 175 | 0.14 ms | 0.13 ms (one branch, no walk) |
| generated: 3,000 branches, 3,000 remote-tracking, 4,000 tags (1,000 annotated), packed, 500 loose | 10,001 | 28.4 ms | 28.3 ms (ten branches, equal) |

Within C11's 100 ms on the bench. Beyond C11, a divergence built over the bench's
real history (a branch at `HEAD` whose upstream is `HEAD~n`), each count equal to
`git rev-list --left-right --count`:

| Upstream | Ahead | Cairn | Commits read | git |
| --- | --- | --- | --- | --- |
| `HEAD~100` | 1,957 | 189 ms | 39,209 | 82-94 ms |
| `HEAD~1000` | 22,649 | 1.32 s | 290,337 | 547-622 ms |
| `HEAD~10000` | 169,679 | 3.02 s | 732,393 | 1.14 s |

About two to three times git: gix's hidden frontier reads far more than the
divergence, and no commit-graph is used so that every read can be cancelled (the
bench has none either). It runs on the refresh thread (phase 06) and is cancellable,
so it holds up nothing; a faster count is a follow-up, not a C11 failure.

## 2026-10-05 — slim rows added

The user locked L14: the rest of a row is slimmed (one id, a parent count, shared
text and author stores, no email, no per-row allocation), as a phase 04 of its
own; later phases renumbered 05-10. C16 bars all of rust-lang/rust at 64 MiB of
retained rows; C15 keeps the edge equivalence and the find's time.

## 2026-10-05 — plan revised before merge

At the user's request, two audit-settled choices were checked against evidence
before the planning PR merged. Fork draws a stash only on a commit its ref walk
reaches (`fork-unreachable-stash-base.md`), so the plan follows that rather than
seeding the walk with stash bases. A deep find was measured on rust-lang/rust
(`deep-find-measured.md`: 2.4 s, 1.4 GiB retained, 89% edges) and Fork's own
approach researched (`fork-deep-history.md`: a 50,000/100,000-commit cap, layout
of the visible area only); the user locked L13 — compact rows, no cap — and a new
phase 03 for it, renumbering later phases 04-09. The systems doc's per-row memory
figure was corrected. A fresh audit caught L13's 60 MB estimate (it is about
160 MiB); C15's bar is 192 MiB.

## 2026-10-05 — planned

`/feature-plan` from the roadmap's packet 4 brief. Four evidence records saved
under `docs/research/refs-and-status/`: a code audit, gix's refs and status APIs
read from the vendored source and probed, a 38-fixture status agreement spike
against git 2.56.0 and 2.30.9 with costs on rust-lang/rust, and a study of Fork's
labels, sidebar, Local Changes and refresh. The user locked L1-L12 as
recommended, closing program O2 for `git status`. PRD, design rewrites
(`engine.md`, `history-graph.md`, `ui.md`, `concurrency.md`,
`feature-inventory.md`) and this directory written on
`feature/plan-refs-and-status`, for a planning PR into `main`.
