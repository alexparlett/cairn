# Sidebar

How the window's sidebar is built: what it draws, where its rows are laid out, and
what pressing an entry does — including finding a ref whose row is not loaded yet by paging
the history's walk. Spec: `docs/prd/refs-and-status.md` R8 (criterion C8). Fork is the
standard (`docs/research/refs-and-status/fork-refs-and-status-ui.md`, section 5); every place
Cairn's sidebar is not Fork's is named under "Where it is not Fork's".

## What it draws

Left of the main region, behind a draggable splitter (`window::beside`, Freya's
`ResizableContainer`; its width kept for the session in `SidebarView::width`), top to bottom:

- **Local Changes**, with the count of distinct paths the last status listed — `Local
  Changes (N)`, the count left out when there is none or no status has been read
  (`sidebar_pane::local_changes_count`, `LocalChanges::paths`, `cairn_ui::local_changes_text`)
  — and **All Commits**. They choose what the main region shows (`cairn_ui::MainView`,
  `SidebarView::main`): the history with its detail pane, or the Local Changes view
  (`local-changes.md`). Pressing a ref returns the main region to the history.
- **A filter box** (`cairn_ui::SIDEBAR_FILTER_PLACEHOLDER`), and under it one line saying
  what a press is doing — "Finding <ref>…" — or why it found nothing, or, when the last
  refresh could not read the refs, why (`RefreshState::failure`).
- **One virtualized list** of the refs (`cairn_ui::Sidebar`, `crates/cairn-ui/src/sidebar.rs`):
  the sections **Branches**, **Remotes**, **Tags** and **Stashes**, in that order, each a
  caption that opens and closes it. Branches and remote-tracking refs are grouped into
  folders split at `/` — a remote's refs under a folder named for the remote, its symbolic
  `HEAD` listed as `origin/HEAD` rather than a bare `HEAD` — each folder a row that opens
  and closes; tags are listed whole (`release/2.0`); every stash entry is listed by its
  message, a stash commit filed twice included. A detached `HEAD` is the first row under
  Branches, as Fork draws it. At each level folders come first and then the refs, each in
  Fork's natural order — case ignored, a run of digits read as its number, so `b2` before
  `b10` (`cairn_model::natural_order`) — and tags in the same order; stashes keep the stash
  list's. (Among Fork's sorting changes its release notes list `main` "treated as
  `master`"; the research record does not say what Fork does with `master`, so nothing is
  done for it.)

Each entry's glyph is its kind's shape, painted (`cairn_ui::RefGlyph`; the shapes differ
pixel for pixel, `every_glyph_paints_a_shape_no_other_glyph_paints`): a branch, a remote,
a tag, a stash's box, a folder; the current branch a check mark with its name bold. A
local branch draws Fork's three upstream states, each its own shape: with an upstream, the
branch glyph; with none, a single line of commits, its newest hollow (`RefGlyph::LocalOnly`,
Fork's local-only icon, told by shape rather than Fork's grey); with its upstream configured
but gone, Fork's warning triangle in place of its glyph — the current branch's at its right,
beside the check mark, as Fork marks an active branch with an invalid upstream. A branch
with an upstream draws its behind and ahead counts at its right, unspaced as Fork prints
them and as the title bar prints them (`counts_text`: `18↓1↑`, a zero left out).
What one row draws is `cairn_ui::drawn_row`, read out of the snapshot for that row alone.
The entry last pressed is drawn chosen, by identity (a ref by its name, a stash by its
place and commit), and is let go of when a row is chosen in the history.

## Where its rows are laid out

The rows are `cairn_model::SidebarRow`s — eight bytes each (`a_sidebar_row_is_eight_bytes`), a section, a folder, a ref, a
stash or a detached `HEAD`, a ref, folder or stash naming its place in the snapshot — laid
out by `RefsSnapshot::sidebar_rows(text, &Disclosure, keep_going)`
(`crates/cairn-model/src/sidebar_rows.rs`): every section's caption and, under each open
one, the refs whose names hold the filter's text (`RefsSnapshot::matching`'s rule, the name
past its namespace, case ignored) and the stashes whose messages do, folders opened as the
`Disclosure` says. While the filter holds text every section and folder is drawn open, a
closed one too, so a match is never hidden, and a section nothing matched in draws no
caption. The pass asks `keep_going` before its first entry and every few thousand after.

It runs on the repository thread, in the ref-filter lane: `Request::FilterRefs { refs,
text, disclosure }` is answered by `Update::FilteredRefs { refs, text, rows }`, the
snapshot handed back with the rows so the window keeps them together
(`cairn_ui::SidebarRefs`). The window asks whenever what they are laid out from changes —
a refresh's snapshot arrives (`session::apply`, `SidebarState::refs_arrived`), the filter's
text changes (a side effect in `sidebar_pane::SidebarPane`, `SidebarState::filter`), a
section or folder is pressed (`SidebarState::toggle`) — and each ask supersedes the one
before it, so the answer drawn is the latest asked; until it arrives the rows before it
stay drawn. The rows an answer replaces, and the snapshot they may be the last hold on, go
to the repository thread to free (`Retired::sidebar`), and so does a superseded answer
(`Update::into_retired`). The UI thread lays nothing out and searches no ref: it keeps the
rows answered and reads a row's ref by index as the list builds that row.

What is open lives in `SidebarState`'s `Disclosure`: a section is open until closed, a
folder closed until opened, and the current branch's folders are opened when it becomes
current — the first snapshot, and a checkout — as Fork reveals the current branch; a folder
the user closed stays closed while the current branch is the same.

The list is a `VirtualScrollView` over the rows, so it builds one viewport whatever the
snapshot holds: 50,000 refs with every folder open, or one folder of 10,000 branches
opened, build only the rows on screen (twins below).

## Pressing an entry (`crates/cairn-app/src/ref_find.rs`)

A section's or folder's row opens or closes it and asks the rows again. A ref, a detached
`HEAD` or a stash selects its row and brings it into view (`selection::choose`,
`cairn_ui::reveal_row`):

- **A ref that names no commit** — a tag on a tree or a blob — says so at once ("<tag>
  names no commit, so it is not in the graph") and walks nothing.
- **A row already loaded** is looked up among the rows refs label, every stash's and
  `HEAD`'s (`History::labelled_position`): a pass that grows with the labelled rows, never
  with every loaded row — a few hundred for the whole of rust-lang/rust, a fraction of a
  millisecond at 50,000 refs. It is right because every ref the sidebar lists labels its
  commit's row in the walk the window holds: `session::apply` asks the sidebar's rows for a
  refresh's snapshot (`Request::FilterRefs`) before it asks the reopen that snapshot's moved
  refs need (`Request::OpenHistory`), both on the repository thread, which serves them in
  order (`a_reopen_frees_the_old_rows_on_a_worker_and_keeps_the_selection` pins the order),
  so the rows a press is read from never name a ref the history is not walked from. The row
  found is told to the history list's cursor hint (`View::history_cursor`), so the next
  arrow key starts from it without searching.
- **A row not loaded yet** is found by paging the held walk forward:
  `Request::FindRow { target, rows }` — `ref_find::FIND_PAGE_ROWS` rows a page — while the
  sidebar says "Finding <ref>…". Every page is answered as `Update::Rows`, as a scroll's
  is, and appended to the history, so the window keeps what scrolling there would and no
  more (R8.6); the window looks through each page as it arrives, never a page twice, and
  when one holds the row selects it, brings it into view and sends `Request::StopFinding`
  (the find may have walked on past a page laid out before it began). The worker stops by
  itself after the page that holds the row.
- **The walk ends without the row** — a stash whose base no ref reaches (R4.2), or a ref
  moved since the walk began — and the sidebar says it is not in the graph; a stash's
  changes are shown all the same, `selection::choose` given its `RowId::Stash`, which no
  loaded row has, so none is drawn selected. With the history already complete it says so
  at once, walking nothing. Such a stash is known to have no row only at the walk's end, so
  its press pages the whole history first, every page kept — bounded by the history and
  cancellable by the next press or scroll like any find (2.2 s on rust-lang/rust); stopping
  early, at the base's commit date, is #73.

**What supersedes a find.** A find is history-lane work: the next press, a scroll of the
list, a row chosen in it, or its own row found supersedes it, and nothing else does. A
press sends its own `FindRow`, or a `StopFinding` when it needs no walk; a scroll is seen
by a side effect on the list's scroll position (`window`, `ref_find::scrolled_away`) and a
row chosen by the list's handlers (`ref_find::row_chosen`), each sending `StopFinding`. The
list's own "reach the end, ask a page" is held off while a find looks, since a page asked
there would supersede it. A refresh that reopens the history while a find looks asks the
find again of the new walk (`ref_find::reopened`). Two quick presses draw only the second:
the first's row, arriving in a page, is passed over.

**Pages are the walk's, not the query's.** The history has two numbers in the worker
(`worker/epoch.rs`): its query lane, `QueryLane::History`, which an open, a scroll's page, a
find and a stop are numbered in, each superseding — cancelling — the walking the last one
asked for; and its walk lane, `QueryLane::Walk`, moved by an open alone. Every page of rows
is answered under the walk's number (`HistoryLane::send_rows`), so a page laid out before a
press, scroll or stop superseded the find that walked it still arrives and is appended —
dropping it would leave a hole the next page is appended after — while the pages of a walk
a reopen replaced are dropped. An open superseded in the query lane before it started
(a find asked straight after it) still lets go of the walk it replaces
(`HistoryLane::replace_walk`), so the find pages the new walk from its start.

**A find shares its thread.** On the repository thread a find walks one page whenever no
other job is waiting (`pool::serve`'s `finding`), so a filter keystroke, a refresh's refs or
a free asked meanwhile is served between its pages; the next history-lane request ends it.
A repository-thread job that is not history-lane work — the sidebar's rows
(`FilterRefs`), the Changes tab's filter (`FilterFiles`), a refresh's refs, a free — waits
for at most the one 512-row page being walked (a few milliseconds warm, tens cold), eight
times a scroll's 64-row page; a history-lane request does not wait, since its number cancels
the page mid-walk.
Cancelling is the walk's own: the epoch is the cancel signal the engine polls once per
commit, so a superseded find stops mid-page and the walk keeps what it laid out for the next
page asked.

## What enforces this

- Layout: `sections_come_in_forks_order_and_branches_and_remotes_fold_at_slashes`,
  `a_filter_keeps_what_matches_with_every_section_and_folder_open`,
  `each_level_is_in_natural_order_folders_first`,
  `natural_order_reads_numbers_as_numbers_and_ignores_case`,
  `a_detached_head_is_the_first_branch_row`,
  `a_folder_is_named_by_its_path_and_a_branch_reveals_its_folders`, `a_stop_is_honoured_mid_pass`
  (`crates/cairn-model/src/sidebar_rows.rs`);
  `a_labelled_row_is_found_among_the_labelled_rows_alone` (`crates/cairn-model/src/history.rs`).
- Drawing (`crates/cairn-ui/tests/sidebar.rs`): `the_sidebar_draws_its_sections_in_forks_order_with_forks_marks`,
  `each_entry_draws_its_kinds_glyph`, `the_entry_chosen_is_drawn_chosen_by_its_name`,
  `a_press_is_reported_for_what_it_pressed`, and the viewport twins
  `a_sidebar_of_50000_refs_builds_one_viewport` and
  `a_folder_of_10000_branches_open_builds_one_viewport`, named in the root `CLAUDE.md`'s
  virtualization invariant.
- The worker (`crates/cairn-app/src/worker/find_tests.rs`, through the real boundary):
  `a_find_pages_the_walk_until_a_page_holds_the_row_and_no_further`,
  `a_second_find_supersedes_the_first_and_no_page_is_lost`,
  `a_stopped_find_leaves_the_walk_for_the_next_page`,
  `a_stop_alone_ends_the_find_and_leaves_the_walk_where_it_stood`,
  `the_pages_a_find_laid_out_before_it_was_stopped_still_arrive`,
  `a_find_for_a_row_the_walk_never_reaches_ends_with_the_walk`,
  `the_sidebars_rows_are_answered_on_a_worker_and_a_newer_ask_supersedes_the_older`,
  `a_sidebar_ask_superseded_mid_pass_stops_and_sends_nothing`; and, for the cancel a find
  shares with a scroll, over lines of commits written object by object for the test
  (`worker/written_repository.rs`, not this checkout's history):
  `a_page_asked_under_a_superseded_number_walks_nothing_and_the_next_takes_the_walk_up`
  (`worker/history_lane.rs`, deterministic) and
  `superseding_a_request_stops_the_walk_that_is_serving_it` (`worker/pool.rs`, one queued
  page of a long line superseded mid-walk).
- The window (`crates/cairn-app/src/sidebar_tests.rs`, headless, updates applied through
  `session::apply`): `pressing_a_loaded_ref_selects_its_row_and_brings_it_into_view`,
  `pressing_a_ref_past_the_loaded_rows_finds_it_by_paging`,
  `two_quick_presses_draw_only_the_second`, `a_scroll_of_the_list_supersedes_a_find`,
  `a_row_chosen_during_a_find_supersedes_it`,
  `a_find_is_not_superseded_by_the_list_reaching_its_end`,
  `a_reopen_during_a_find_asks_it_again_of_the_new_walk`,
  `a_tag_on_a_tree_says_it_is_not_in_the_graph`,
  `a_stash_with_no_row_shows_its_changes_and_says_it_is_not_in_the_graph`,
  `typing_in_the_sidebars_filter_asks_a_worker_and_draws_its_answer`,
  `local_changes_and_all_commits_switch_the_main_region`; and
  `SidebarState`'s `the_current_branchs_folders_open_when_it_becomes_current` and
  `a_filter_or_a_toggle_asks_the_rows_again`.
- Measured: `measures_a_find_through_the_boundary` (`#[ignore]`d, driven by
  `CAIRN_BENCH_REPO`, with `CAIRN_FIND_TARGET` or `CAIRN_FIND_REF`) times a find of the
  repository's oldest commit, or of a named ref, through the real boundary (median of
  seven), reads what it retains against C16's ceiling, stops one halfway as a scroll does
  and checks the next page takes the walk up where it stopped.

## Where it is not Fork's, and other limits

- Fork caps its history and does nothing when a pressed ref is past it; Cairn finds it
  however deep it is (the PRD's named deviation).
- Folders open closed, but for the current branch's, and what is open is not kept across
  sessions (the user's decision, 2026-10-07). Whether Fork keeps it is not settled: the
  research records only that Fork for Windows 2.23 stores "collapse state" per worktree
  (`fork-refs-and-status-ui.md`, section 2), which may mean the sidebar's folders or
  something else; finding out is #72.
- Fork's local-only icon is drawn as a shape of its own rather than Fork's grey, since a
  colour alone may not tell two kinds apart.
- Pinned, Worktrees and Submodules, the tab strip of refs and search, double-click checkout,
  ⌘-click compare from the sidebar, and keyboard navigation of the sidebar are not built.
- A find's page is `FIND_PAGE_ROWS` rows, so the page holding the row may carry up to that
  many rows past it — a few dozen kilobytes kept beyond what a scroll's page would.
- A stash whose base no ref reaches is known to have no row only when the walk ends: its
  press pages the whole history first.
- A ref moved since the walk began labels no loaded row until the refresh that saw it
  reopens the history, which it does at once; a press in between pages the walk and finds
  the commit only in a page still to come.
