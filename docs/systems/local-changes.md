# Local Changes

How the window's Local Changes view is built today: the two lists it draws over the working
tree's status, how a path is chosen and its diff asked, and what keeps a diff from being drawn
for any path but the one chosen. Read only: nothing in it stages, unstages, discards or commits
(refs-and-status R9.6; packet 5 adds that). Spec: `docs/prd/refs-and-status.md` R9 (criterion
C9). Fork is the standard (`docs/research/refs-and-status/fork-refs-and-status-ui.md`, section
6); every place Cairn's view is not Fork's is named under "Where it is not Fork's". The status
itself — what `git status` lists and how it is read — is `status.md`; the working-tree query
that answers a path's diff is `diff.md`, "The working-tree query".

## What it draws

Pressing **Local Changes** in the sidebar (`sidebar.md`) puts the view in the main region in
place of the history and its detail pane (`window::window`, `MainView::LocalChanges`,
`crates/cairn-app/src/local_changes_pane.rs`), as Fork lays it out:

- **Left**, behind a draggable splitter (its share kept for the session,
  `LocalChangesView::list_width`): a filter field — the only control above the lists — then
  **Unstaged** above **Staged**, each under its heading and each one flat, virtualised list
  (`cairn_ui::LocalChangesList`, `crates/cairn-ui/src/local_changes.rs`), with a draggable
  splitter between the two (the user's decision, 2026-10-07; its share kept for the session and
  never beyond it, `LocalChangesView::lists_split`, `LocalChangesList::split`). A row is a badge
  and a path, a rename's or a copy's source before it (`old → new`, `cairn_ui::change_text`).
  The badges are Fork's, told apart by their shape (the user's decision, 2026-10-07;
  `cairn_ui::change_badge`): `M` for a modified file and a type change alike, `+` for an added,
  intent-to-add or untracked one, `D` deleted, `R` renamed, `C` copied, and, painted as shapes
  rather than set in a font, a submodule's box (`RefGlyph::Submodule`) whatever changed in it
  and Fork's warning triangle for a conflicted path (`RefGlyph::Gone`). Colour only reinforces
  the shape. The Commit tab keeps its own letters.
- **Right**: the chosen path's diff under the diff view's bar — previous and next change,
  ignore whitespace, context, entire file, side by side, the shared settings — unified or side
  by side; or the notice of a state that is not text, with Load Diff for a file past the
  limits; or, for a conflicted path, the conflict's notice in place of a diff (R9.4,
  `DiffNotice::Conflicted`). Previous and next change are heard from inside the view and move
  its own diff (`diff_actions::step`), whose scroll and change cursor are the view's
  (`LocalChangesView::scroll`, `cursor`), not the Changes tab's.
- **Instead of the lists**: "Reading the working tree's status…" before the first status;
  the read's failure when no status has been read (`RefreshState::failure(Refreshed::Status)`,
  said over the lists kept when one has); that the repository has no working tree; that the
  git in use cannot read a sparse index. With no change, the lists are empty and the right says
  "No local changes."

The count beside the sidebar's entry is the number of distinct paths the status lists
(R9.2, `sidebar_pane::local_changes_count`, `LocalChanges::paths`): a path in both lists once,
and a staged deletion with an untracked file of the same name — two entries git lists apart —
once; none is shown with no change.

## Which list, in what order (`cairn_model::LocalChanges`)

A status arrives laid out already: `LocalChanges::new` runs on the refresh thread when the
status is read (`worker/refresh_lane.rs`), and `Update::Status` carries it, so the window only
keeps it. A tracked path is in Staged when it has a staged change and in Unstaged when it has
an unstaged one — in both when it has both (R9.1) — or when git listed it for a submodule's
state alone; a conflicted path and an untracked one are in Unstaged, where Fork puts them.
Each list is an index into the status's entries, in Fork's natural order of its paths (the
user's decision, 2026-10-07; `cairn_model::path_order`, the sidebar's `natural_order`: case
ignored, each run of digits read as its number, two paths that read alike told apart by their
bytes), so untracked paths are mixed with tracked ones by name, as Fork mixes them. Untracked files
are listed one per file, a new directory never a row of its own (the status read's own rule,
`status.md`). A row is read by its index as it is built (`LocalChanges::get`), and a path is
found in a list by a binary search in the same order (`LocalChanges::row_of`); nothing on the
UI thread walks the status.

## The filter

Typing in the filter asks a worker which rows hold the text in a path or a source — the
Changes tab's rule, case ignored (`LocalChanges::matching`) — as
`Request::FilterLocalChanges { changes, text }`, `changes` the window's own lists, shared. It
is numbered in a lane of its own (`QueryLane::LocalChangesFilter`), routed to the repository
thread, and superseded by the next keystroke or by a newer status's ask, nothing else; a pass
superseded mid-way stops within a few thousand paths and sends nothing. Its answer,
`Update::FilteredLocalChanges { changes, text, rows }`, is kept only for the newest lists and
the text as it is (`LocalChangesState::filtered`). With a filter on, a new status is not drawn
until its rows come: the lists before it stay drawn with their own rows meanwhile, so a row of
one status is never read by an index into another, and a refresh never empties the view while
a filter is typed (`crates/cairn-app/src/local_changes_state.rs`). Until the first answer for a
text, the lists say "Filtering…" rather than counting every row as matched; then "Showing N of
M files" (rows, so a path in both lists counts twice); "No path matches the filter." when it
leaves none.

## Choosing a path, and asking its diff

A row pressed, or reached with ↑ or ↓ in its focused list, is chosen by its list and its path
(`diff_actions::choose_working`), and its diff is asked through the working-tree query as its
list says (R9.3, `diff_actions::working_query`): its staged diff (`WorkingSide::Staged`) from
Staged, its unstaged diff (`WorkingSide::Unstaged`) from Unstaged, its untracked diff
(`WorkingSide::Untracked`) for a path git does not track; a conflicted path asks nothing. It is
asked at the Changes tab's options — the shared context and whitespace setting, and the entire
file — since the view is one file's diff under the same bar. With nothing chosen, the first
path the lists show is chosen as the view opens — Unstaged's first row, else Staged's (R9.3).

The path chosen and its diff live in the diff selection beside the commit's
(`crates/cairn-app/src/diff_state/working.rs`, `DiffState::choose_working`), because the three —
the Changes tab's file, the files opened in place in the Commit tab, and this path — share the
file-diff lane: whichever asks takes it, and the others are asked again, whole, when their view
is shown (`working_needs_asking`, `reask_working`; `diff.md`, "The file-diff lane is shared").
A change of the settings with Local Changes shown asks its path again at once and the others
when their tab is shown (`Asking::Working`). A changes query supersedes the lane whoever holds
it.

**An answer is drawn only for the query it names.** `Update::FileDiff` for a working-tree
query is kept only when it names exactly the query the path chosen is asked as now — its path,
its side and its options (`DiffState::working_arrived`) — so a path in both lists draws each
list's diff and an answer for the list chosen before, arriving late, is not drawn; one not kept
is handed to a worker to free.

**A refresh follows the path.** Each status drawn is numbered (`LocalChangesState::serial`), and
the path chosen remembers the lists it was chosen or last found in. When other lists are drawn,
the view looks for it in them by its list and path (`local_changes_pane::follow_the_lists`):
still listed, it is asked again as it is listed now — the file may have changed under the same
status — and its last diff stays drawn until the new one comes (`DiffState::refresh_working`);
gone from the status, the first path shown is chosen in its place, so no diff is drawn for a
path the status no longer lists; with no path shown, a path chosen is let go of. The filter
follows the Changes tab's rule: a path it hides stays chosen — its diff drawn, no row
highlighted — and is asked again on each refresh; only the status taking it away chooses
another. What is done is decided by a pure function (`local_changes_pane::follow`: keep, ask
again, choose the first, let go, or nothing), and the effect that runs it writes only when
there is something to write, so it never wakes itself for good. This runs only while the view
is shown: a status that arrives while the history is shown asks nothing until Local Changes
is.

Every status, diff and set of lists the window lets go of is handed to a worker to free
(`Request::Retire`): the refresh's answers and the lists drawn each hand their own hold over,
so the last hold on a status of tens of thousands of paths is never dropped on the UI thread.

## What enforces this

- Layout (`crates/cairn-model/src/local_changes.rs`):
  `each_path_is_in_the_lists_its_changes_put_it_in_ordered_by_name`,
  `each_list_is_in_natural_order_and_found_in_it`,
  `each_change_is_drawn_as_its_kind_with_a_sources_path`, `the_count_is_of_distinct_paths`,
  `a_path_is_found_by_its_list_and_name`, `a_filter_leaves_each_lists_rows_that_hold_its_text`,
  `fifty_thousand_paths_are_laid_out_in_order`.
- Drawing (`crates/cairn-ui/tests/local_changes.rs`):
  `both_lists_draw_their_paths_with_their_badges`, `each_list_is_in_its_paths_order`,
  `a_press_and_the_arrows_choose_a_row_of_its_list`, `the_filter_says_what_it_leaves`,
  `every_kind_of_change_draws_its_badges_shape` (each painted badge's cell read pixel for pixel
  against its glyph), `the_splitter_between_the_lists_drags`, the badge rule
  `each_kind_of_change_has_forks_badge` and `every_glyph_paints_a_shape_no_other_glyph_paints`
  (`ref_glyphs.rs`), and the viewport twin
  `a_status_of_50000_paths_builds_one_viewport_filtered_or_not` (top, deep and end, filtered and
  not), named in the root `CLAUDE.md`'s virtualization invariant.
- The selection (`crates/cairn-app/src/diff_state/working.rs`):
  `a_path_in_both_lists_keeps_only_the_answer_for_the_list_chosen`,
  `a_refresh_asks_the_path_again_and_draws_its_last_diff_meanwhile`,
  `a_conflicted_path_asks_nothing_and_draws_its_notice`,
  `the_working_path_and_the_commits_file_take_the_lane_from_each_other`,
  `a_setting_asks_the_working_path_when_local_changes_is_shown`; the lists as kept
  (`crates/cairn-app/src/local_changes_state.rs`):
  `with_no_filter_a_status_is_drawn_as_it_arrives`,
  `with_a_filter_a_status_is_drawn_once_its_rows_arrive`,
  `clearing_the_filter_draws_the_newest_lists_whole`; following the lists
  (`local_changes_pane.rs`): `an_empty_status_lets_a_path_go_once_and_then_decides_nothing`,
  `the_path_chosen_follows_the_lists_as_decided`; the count
  (`the_count_beside_local_changes_is_of_distinct_paths`, `sidebar_pane.rs`).
- The window (`crates/cairn-app/src/local_changes_tests.rs`, headless, updates applied through
  `session::apply`):
  `local_changes_draws_its_lists_and_the_first_paths_diff_from_the_working_tree_query`,
  `a_conflicted_path_draws_its_notice_and_asks_nothing`,
  `a_path_in_both_lists_draws_each_lists_diff_and_the_answers_cannot_cross`,
  `a_refresh_that_removes_the_chosen_path_draws_no_stale_diff_under_no_row`,
  `typing_in_the_filter_asks_a_worker_and_the_lists_draw_its_answer`,
  `next_change_moves_local_changes_own_diff`, `a_status_that_could_not_be_read_is_said`,
  `a_filter_hiding_the_path_chosen_keeps_it_chosen_and_drawn`; and
  `local_changes_and_all_commits_switch_the_main_region` (`sidebar_tests.rs`).
- The worker, through the real boundary (`crates/cairn-app/src/worker/refresh_tests.rs`):
  `untracked_files_under_a_new_directory_are_listed_one_per_file_and_counted`,
  `local_changes_filter_is_answered_on_a_worker_for_the_windows_own_lists` and
  `a_local_changes_filter_superseded_mid_pass_stops_and_sends_nothing`; the working-tree
  query itself, `a_working_tree_diff_answers_through_the_boundary` (`diff_tests.rs`).
- Measured: `window_check` (`#[ignore]`d, `crates/cairn-app/src/window_check.rs`) lands the
  view over a scratch clone of the bench repository with a status of thousands of paths
  (`CAIRN_SCRATCH_REPO`): the lists drawn and the first path's diff, Unstaged scrolled, a
  filter typed, a refresh landed. Its guard (`scratch_refusal`, pinned by
  `the_scratch_guard_refuses_whatever_shares_the_benchs_git_directory`) refuses, with
  `std::fs` alone, a scratch that is the bench, inside it or holding it, one whose git
  directory — `.git`, a `.git` file's `gitdir:`, a symbolic link followed — or `commondir` is
  inside the bench, and one borrowing objects through `alternates`. That only the scratch was
  dirtied is a manual step the check does not take: touch a marker before the clone and the
  run, and `find <bench>/.git -newer <marker>` must print nothing after.

## Where it is not Fork's, and other limits

- Fork draws each list as a tree by default, with a layout menu offering a tree, a list and a
  combined list, Hide Untracked Files and Show Ignored Files; Cairn draws flat lists only (the
  tree view is #36), lists untracked files unless the user's `status.showUntrackedFiles` says
  no, and never lists ignored files (R3.4, R3.5). The filter field is the only control above
  the lists (the user's decision, 2026-10-07): the layout menu, Hide Untracked Files, the
  collapse-all chevron and the eye (Fork's side-by-side quick look) are not drawn, and issues
  are to be filed for them.
- Fork's Stage and Unstage buttons on the headings, and the commit box under the diff, are
  packet 5's (R9.6).
- The submodule's badge is a shape of Cairn's drawing (a box in a box); Fork's own submodule
  icon is not recorded.
- The count beside Local Changes is the distinct paths status lists (R9.2's wording), where
  Fork's is said to equal the entries `git status` lists; whether Fork counts a path with both
  staged and unstaged changes twice is OPEN in the research (its OPEN 9). "Showing N of M
  files" under the filter counts rows, so a path in both lists counts twice there.
- Entire File applies here, shared with the Changes tab, as it does to every view of one file
  under the bar.
- A status that could not be read — a partial clone's staged rename on git 2.44 and later among
  the causes — is said for the whole view, where a failed diff fails one path; the lists of
  the last status read stay drawn under it. Whether to name that state, retry without rename
  detection, or let status fetch is open for the user.
- Pressing a ref in the sidebar returns the main region to the history; Fork's own behaviour
  on that press from Local Changes is not recorded.
