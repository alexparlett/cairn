# Sidebar

How the window's sidebar is built today: what it draws, where its rows are laid out, and
what pressing an entry does — including finding a ref whose row is not loaded yet by paging
the history's walk. Spec: `docs/prd/refs-and-status.md` R8 (criterion C8). Fork is the
standard (`docs/research/refs-and-status/fork-refs-and-status-ui.md`, section 5); every place
Cairn's sidebar is not Fork's is named under "Where it is not Fork's".

## What it draws

Left of the main region, behind a draggable splitter (`window::beside`, Freya's
`ResizableContainer`; its width kept for the session in `SidebarView::width`), top to bottom:

- **Local Changes**, with the count of paths the last status listed — `Local Changes (N)`,
  the count left out when there is none or no status has been read
  (`sidebar_pane::local_changes_count`, `cairn_ui::local_changes_text`) — and **All
  Commits**. They choose what the main region shows (`cairn_ui::MainView`,
  `SidebarView::main`): the history with its detail pane, or Local Changes, which draws a
  placeholder until its lists are built (`window::LOCAL_CHANGES_PLACEHOLDER`). Pressing a
  ref returns the main region to the history.
- **A filter box** (`cairn_ui::SIDEBAR_FILTER_PLACEHOLDER`), and under it one line saying
  what a press is doing — "Finding <ref>…" — or why it found nothing, or, when the last
  refresh could not read the refs, why (`RefreshState::failure`).
- **One virtualized list** of the refs (`cairn_ui::Sidebar`, `crates/cairn-ui/src/sidebar.rs`):
  the sections **Branches**, **Remotes**, **Tags** and **Stashes**, in that order, each a
  caption that opens and closes it. Branches and remote-tracking refs are grouped into
  folders split at `/` — a remote's refs under a folder named for the remote — each folder
  a row that opens and closes; tags are listed whole (`release/2.0`); every stash entry is
  listed by its message, a stash commit filed twice included. A detached `HEAD` is the first
  row under Branches, as Fork draws it. At each level folders come first and then the refs,
  each in the snapshot's order (bytewise by name).

Each entry's glyph is its kind's shape, painted (`cairn_ui::RefGlyph`; the shapes differ
pixel for pixel, `every_glyph_paints_a_shape_no_other_glyph_paints`): a branch, a remote,
a tag, a stash's box, a folder; the current branch a check mark with its name bold; a
branch whose upstream is configured but gone Fork's warning triangle in place of its glyph
— the current branch's at its right, beside the check mark, as Fork marks an active branch
with an invalid upstream; any other branch with an upstream its behind and ahead counts at
its right, printed as the title bar prints them (`counts_text`: `18↓ 1↑`, a zero left out).
What one row draws is `cairn_ui::drawn_row`, read out of the snapshot for that row alone.
The entry last pressed is drawn chosen, by identity (a ref by its name, a stash by its
place and commit), and is let go of when a row is chosen in the history.

## Where its rows are laid out

The rows are `cairn_model::SidebarRow`s — twelve bytes each, a section, a folder, a ref, a
stash or a detached `HEAD`, a ref, folder or stash naming its place in the snapshot — laid
out by `RefsSnapshot::sidebar_rows(text, &Disclosure, keep_going)`
(`crates/cairn-model/src/sidebar_rows.rs`): every section's caption and, under each open
one, the refs whose names hold the filter's text (`RefsSnapshot::matching`'s rule, the name
past its namespace, case ignored) and the stashes whose messages do, folders opened as the
`Disclosure` says. While the filter holds text every folder is drawn open, so a match is
never hidden behind a closed one; a closed section stays closed. The pass asks
`keep_going` before its first entry and every few thousand after.

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
  `HEAD`'s (`History::labelled_position`): a pass over a few hundred rows for the whole of
  rust-lang/rust, never every loaded row, since every ref labels its commit's row in a walk
  from the snapshot the sidebar lists.
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
  at once, walking nothing.

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
Cancelling is the walk's own: the epoch is the cancel signal the engine polls once per
commit, so a superseded find stops mid-page and the walk keeps what it laid out for the next
page asked.

## What enforces this

- Layout: `sections_come_in_forks_order_and_branches_and_remotes_fold_at_slashes`,
  `a_filter_keeps_what_matches_with_every_folder_open`, `a_detached_head_is_the_first_branch_row`,
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
  `the_pages_a_find_laid_out_before_it_was_stopped_still_arrive`,
  `a_find_for_a_row_the_walk_never_reaches_ends_with_the_walk`,
  `the_sidebars_rows_are_answered_on_a_worker_and_a_newer_ask_supersedes_the_older`,
  `a_sidebar_ask_superseded_mid_pass_stops_and_sends_nothing`; and
  `superseding_a_request_stops_the_walk_that_is_serving_it` (`worker/pool.rs`) for the
  cancel a find shares with a scroll.
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
- Fork's sorting default is not recorded; Cairn lists folders first, each level in the
  snapshot's order (Fork's "alphabetically, folders first" option). Folders open closed,
  but for the current branch's; Fork's remembered expansion is not kept across sessions.
- Fork's greyed icon for a branch with no upstream is not drawn: it would be told from a
  pushed branch's by colour alone, which the product's rules forbid.
- Pinned, Worktrees and Submodules, the tab strip of refs and search, double-click checkout,
  ⌘-click compare from the sidebar, and keyboard navigation of the sidebar are not built.
- A find's page is `FIND_PAGE_ROWS` rows, so the page holding the row may carry up to that
  many rows past it — a few dozen kilobytes kept beyond what a scroll's page would.
- A stash whose base no ref reaches is known to have no row only when the walk ends: its
  press pages the whole history first.
- A ref moved since the walk began labels no loaded row until the refresh that saw it
  reopens the history, which it does at once; a press in between pages the walk and finds
  the commit only in a page still to come.
