# Sidebar

How the window's sidebar is built: what it draws, where its rows are laid out, and
what pressing an entry does — including finding a ref whose row is not loaded yet by paging
the history's walk. Spec: `docs/prd/refs-and-status.md` R8 (criterion C8) and
`docs/prd/fork-alignment.md` R6, R9.2-R9.3, R10.3. Fork is the standard
(`docs/research/refs-and-status/fork-refs-and-status-ui.md`, section 5, and
`docs/research/ui-review/`); every place Cairn's sidebar is not Fork's is named under "Where it is
not Fork's".

## What it draws

Left of the main region, behind a draggable splitter (`window::beside`, Freya's
`ResizableContainer`; Fork's 300 px as it opens, `metrics::SIDEBAR_WIDTH`, its width kept for the
session in `SidebarView::width`), top to bottom, as Fork for Windows draws it
(`docs/research/ui-review/fork-window-survey-2026-10-16.md` §3, `fork-window-survey-2-2026-10-16.md`
§2, `fork-sidebar-2026-10-20.md`):

- **A header row**: the repository's name, bold (`Update::Opened`'s `name`), and a gear at its
  right whose menu holds **Copy Repository Path** (`cairn_ui::GearChoice`), which puts the
  repository's root, made absolute by the worker (`Update::Opened`'s `path`, `View::repository_path`),
  on the clipboard (`sidebar::menu::gear`). Fork's Rename Repository and its settings are not drawn.
- **Local Changes**, with its icon and the count of distinct paths the last status listed —
  `Local Changes (N)`, the count left out when there is none or no status has been read
  (`sidebar::pane::local_changes_count`, `LocalChanges::paths`, `cairn_ui::local_changes_text`)
  — and **All Commits**, with its icon. The one the main region shows is Fork's rounded grey band
  (`tones::SIDEBAR_CHOSEN`), standing in 5 px from either edge with ~3 px corners
  (`metrics::SIDEBAR_BAND_INSET`, `SIDEBAR_BAND_RADIUS`), and stays drawn while a ref is chosen
  below. They choose what the main region shows (`cairn_ui::MainView`, `SidebarView::main`): the
  history with its detail pane, or the Local Changes view (`local-changes.md`). Pressing a ref
  returns the main region to the history. Fork's refs/search switch is not drawn.
- **A filter field** with its magnifier (`cairn_ui::SIDEBAR_FILTER_PLACEHOLDER`,
  `tones::FILTER_FIELD`), and under it one line saying what a press is doing — "Finding <ref>…" —
  or why it found nothing, or, when the last refresh could not read the refs, why
  (`RefreshState::failure`).
- **One virtualized list** of the refs (`cairn_ui::Sidebar`, `crates/cairn-ui/src/sidebar.rs`):
  the sections **Branches**, **Remotes**, **Tags** and **Stashes**, in that order, each a bold
  heading with its chevron and no count, Tags closed as the sidebar opens and the rest open
  (`SidebarSection::opens_closed`, `Disclosure`'s default). Branches and remote-tracking refs are
  grouped into folders split at `/` — a remote's refs under a folder named for the remote, its
  symbolic `HEAD` listed as `origin/HEAD` rather than a bare `HEAD` — each folder a row that opens
  and closes, every folder closed until opened, a remote's own among them; each row draws its
  leaf name (`crash` under `bugfix`). Tags are listed whole (`release/2.0`), **newest first by the
  date of the commit each identifies** (`Ref::commit_time`, read by the refs query; two tags of one
  date by name, last first — the one tie observed, not a rule observed — and a tag naming no commit
  last), never by name or by a tag's own date, as Fork lists them (sb14). Every stash entry is
  listed by its message after git's `On <branch>: ` ("WIP on parser"), a message of any other form
  — git's default `WIP on main: <id> <subject>` — whole (`cairn_ui::stash_text`), a stash commit
  filed twice included. A detached `HEAD` is the first row under Branches, as Fork draws it. At each
  level of Branches and Remotes folders come first and then the refs, each in Fork's natural order
  — case ignored, a run of digits read as its number, so `b2` before `b10`
  (`cairn_model::natural_order`); stashes keep the stash list's. (Among Fork's sorting changes its
  release notes list `main` "treated as `master`"; the research record does not say what Fork does
  with `master`, so nothing is done for it.)

Each row is a chevron's slot — a heading's or a folder's chevron, open or closed, a leaf's slot
left empty so its icon stands where its folders' do — then its icon and its name, each level
12 px further in (`metrics::SIDEBAR_INDENT`, `cairn_ui::row_indent`). The icons are Lucide's
(`style::icons`, `cairn_ui::SidebarGlyph`): an open folder; a remote's own folder its cloud; a
branch, and a remote-tracking ref, the branch glyph; the current branch a check mark in place of
it, its name bold; a tag's outline; a stash's archive box. A local branch draws Fork's upstream
states: with an upstream, the branch glyph and, at the row's right, its ahead then behind counts
as Fork for Windows draws them — each count with its arrow after it, a space between, a zero left
out (`counts_text`: `1↑`, `1↓`, `1↑ 2↓`), in the body's tone; with its upstream configured but
gone, the branch glyph with Fork's yellow warning on its bottom right (`tones::UPSTREAM_GONE`) and
no counts — the current branch's warning at its right, beside the check mark; with none, a glyph
of its own shape in a quieter tone (`SidebarGlyph::LocalOnly`), where Fork greys its branch glyph.
What one row draws is `cairn_ui::drawn_row`, read out of the snapshot for that row alone.

The entry last pressed is drawn chosen, by identity (a ref by its name, a stash by its place and
commit), and is let go of when a row is chosen in the history. A press on a row gives the sidebar
focus, so the chosen row is drawn as a selection says its list's focus (`docs/systems/style.md`,
"The selection"): Fork's blue while the sidebar has focus, its grey once focus is elsewhere, in
Fork's rounded band (sb22).

**Its keys** (`crates/cairn-ui/src/sidebar/keys.rs`), Fork's (sb22-sb72), heard while the sidebar
has focus: ↑ and ↓ walk every row of the list — each heading, folder, ref and stash — one step,
stopping at either end and never reaching the filter, All Commits or Local Changes; a ref, a
stash or a detached `HEAD` walked to is chosen at once, as a press chooses it, so the history
selects its commit while the sidebar keeps focus, and a heading or folder walked to only holds
the walk, drawn as the selection. ← closes an open heading or folder and from any other row goes
to the row it stands in; → opens a closed one and from an open one goes to its first row. Return
does nothing, as in Fork. Where the walk stands is the sidebar's own state (`keys::Walk`): it holds
while the entry chosen is the one it chose — a row chosen in the history lets it go — and while
the rows are the ones it stood among, but for the rows laid out again for a heading or folder it
or a press opened or closed. With no walk standing, a key first looks for the chosen entry's row
among the rows, once per press.

**A right press** on a ref selects its commit as a press does (FS1 §8) and opens its menu through
the one menu component (`cairn_ui::ref_menu`): a branch's or a remote-tracking ref's **New
Branch…** then **Copy Branch Name**, a tag's **New Branch…** then **Copy Tag Name** — R9.2's working
items, New Branch… greyed on a tag that names no commit. New Branch… opens Create Branch at the
ref's commit, its subject the loaded row's (`sidebar::menu::chosen`); Copy puts the ref's name past
its namespace on the clipboard — `feature/login`, `origin/main`, `v0.2` — as Fork's copies
(`sidebar::menu::ref_name_text`, sb84-sb86). A right press on a stash or a detached `HEAD` chooses
it and opens nothing; on a heading or folder it does nothing.

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
text changes (a side effect in `sidebar::pane::SidebarPane`, `SidebarState::filter`), a
section or folder is pressed (`SidebarState::toggle`) — and each ask supersedes the one
before it, so the answer drawn is the latest asked; until it arrives the rows before it
stay drawn. The rows an answer replaces, and the snapshot they may be the last hold on, go
to the repository thread to free (`Retired::sidebar`), and so does a superseded answer
(`Update::into_retired`). The UI thread lays nothing out and searches no ref: it keeps the
rows answered and reads a row's ref by index as the list builds that row.

What is open lives in `SidebarState`'s `Disclosure`: Tags starts closed and the other sections
open, a folder closed until opened, and the current branch's folders are opened when it becomes
current — the first snapshot, and a checkout — as Fork reveals the current branch; a folder
the user closed stays closed while the current branch is the same.

The list is a `VirtualScrollView` over the rows, so it builds one viewport whatever the
snapshot holds: 50,000 refs with every folder open, or one folder of 10,000 branches
opened, build only the rows on screen (twins below).

## Pressing an entry (`crates/cairn-app/src/history/ref_find.rs`)

A section's or folder's row opens or closes it and asks the rows again. A ref, a detached
`HEAD` or a stash selects its row and brings it into view (`selection::choose`,
`cairn_ui::reveal_row`):

- **A ref that names no commit** — a tag on a tree or a blob — says so at once ("<tag>
  names no commit, so it is not in the graph") and walks nothing.
- **A row already loaded** is looked up among the rows refs label, every stash's and
  `HEAD`'s (`History::labelled_position`): a pass that grows with the labelled rows, never
  with every loaded row — a few hundred for the whole of rust-lang/rust, a fraction of a
  millisecond at 50,000 refs. It is right because every ref the sidebar lists labels its
  commit's row in the walk the window holds — but while a reopen reads, when the sidebar's
  rows for the refresh's snapshot (`Request::FilterRefs`, on the repository thread) may
  arrive before the reopened walk's first page (`Request::OpenHistory`, on the history
  thread) and the rows drawn are still the old walk's. A ref that moved then labels no
  loaded row, or labels one by its old place: its press finds nothing among them, or a
  commit both walks hold; the end the old rows reached is forgotten at the reopen
  (`Progress::reopen`), so a press that finds nothing pages the new walk, which is from the
  sidebar's snapshot, and its first page replaces the old rows. The row found is told to the
  history list's cursor hint (`View::history_cursor`), so the next arrow key starts from it
  without searching.
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
a reopen replaced are dropped. An open is the one history-lane request no later press,
scroll or stop supersedes: whose walk is still the window's, it runs — its opening pass and
its first page under the walk's number — whatever the query lane asked since, so a find asked
straight after a reopen pages the new walk on from its first page, and a stop asked while a
walk opens leaves it opening (`a_find_or_its_stop_never_supersedes_a_walk_still_opening`).

**A find has the history thread.** On the history thread, which runs the history lane and
nothing else, a find walks one page whenever no other job is waiting (`lane::serve_history`'s
`finding`), so the next history-lane request is taken up between its pages and ends it. The
sidebar's rows (`FilterRefs`), the Changes tab's filter (`FilterFiles`), a refresh's refs and
a free run on the repository thread and wait on no page of a find, nor on an opening pass;
a history-lane request does not wait either, since its number cancels the page mid-walk.
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
- Drawing (`crates/cairn-ui/tests/sidebar.rs`): `the_sidebar_is_forks`,
  `each_entry_draws_its_kinds_glyph`, `the_entry_chosen_is_drawn_chosen_by_its_name`,
  `a_press_is_reported_for_what_it_pressed`, `a_right_press_on_a_ref_selects_its_commit`,
  `the_gear_copies_the_repository_path`, `the_sidebar_walks_by_key_as_forks_tree`, and the
  viewport twins `a_sidebar_of_50000_refs_builds_one_viewport` and
  `a_folder_of_10000_branches_open_builds_one_viewport` (each requiring Fork's 22 px rows), named
  in the root `CLAUDE.md`'s virtualization invariant; the walk's rule
  (`the_arrows_walk_every_row_and_stop_at_the_ends`, `left_and_right_open_close_and_climb_as_a_tree`,
  `sidebar/keys.rs`); the words (`counts_are_ahead_then_behind_and_a_zero_is_left_out`,
  `a_stash_reads_its_message_after_on_branch`, `sidebar.rs`).
- The layout's order and disclosure (`crates/cairn-model/src/sidebar_rows.rs`):
  `tags_and_every_folder_start_closed_and_the_other_sections_open`,
  `tags_are_listed_newest_first_by_their_commits_date`; a tag's date
  (`a_tag_is_dated_by_its_commit_never_by_its_tagger` and the refs oracle, which compares each
  tag's date with `git log --no-walk`'s, `crates/cairn-git/tests/refs.rs`).
- The worker (`crates/cairn-app/src/worker/history/tests.rs`, through the real boundary):
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
  (`worker/history/lane.rs`, deterministic) and
  `superseding_a_request_stops_the_walk_that_is_serving_it` (`worker/pool.rs`, one queued
  page of a long line superseded mid-walk).
- The window (`crates/cairn-app/src/sidebar/tests.rs`, headless, updates applied through
  `session::apply`): `pressing_a_loaded_ref_selects_its_row_and_brings_it_into_view`,
  `pressing_a_ref_past_the_loaded_rows_finds_it_by_paging`,
  `two_quick_presses_draw_only_the_second`, `a_scroll_of_the_list_supersedes_a_find`,
  `a_row_chosen_during_a_find_supersedes_it`,
  `a_find_is_not_superseded_by_the_list_reaching_its_end`,
  `a_reopen_during_a_find_asks_it_again_of_the_new_walk`,
  `a_tag_on_a_tree_says_it_is_not_in_the_graph`,
  `a_stash_with_no_row_shows_its_changes_and_says_it_is_not_in_the_graph`,
  `typing_in_the_sidebars_filter_asks_a_worker_and_draws_its_answer`,
  `local_changes_and_all_commits_switch_the_main_region`,
  `a_right_press_on_a_ref_selects_its_commit_in_the_history`,
  `the_arrows_walk_the_sidebar_and_select_each_refs_commit`; `sidebar::menu`'s
  `a_refs_name_is_copied_past_its_namespace`; and
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
- A branch with no upstream is drawn with a glyph of its own shape rather than Fork's greyed
  branch glyph, since a colour alone may not tell two kinds apart (a `ui.md` row).
- Pinned, Worktrees and Submodules (#69), the tab strip of refs and search, the hover icons
  (pin, show only, hide: #2), a branch name's tooltip ("Local branch / Tracked branch"),
  double-click checkout and ⌘-click compare from the sidebar are not built.
- New Branch… from a ref opens Create Branch at the ref's commit, its short id and subject in
  "Create branch at:"; Fork's names the ref there with its glyph (sb78-sb82). No requirement
  asks for it yet.
- Copy Branch Name and Copy Tag Name show no chord: Fork's Ctrl+C is the `Copy` action, still
  to come with the focused view's copy items (`docs/prd/fork-alignment.md` R10.1).
- Pressing All Commits does not move focus to the history, where Fork's does (sb73). No
  requirement asks for it yet.
- `docs/prd/fork-alignment.md` C14 and R10.3 ask for Return to open Local Changes or All
  Commits from the sidebar's rows; Fork's arrows never reach those two and its Return does
  nothing on any row (`fork-sidebar-2026-10-20.md` §4), so the sidebar's keys are Fork's.
- A find's page is `FIND_PAGE_ROWS` rows, so the page holding the row may carry up to that
  many rows past it — a few dozen kilobytes kept beyond what a scroll's page would.
- A stash whose base no ref reaches is known to have no row only when the walk ends: its
  press pages the whole history first.
- A ref moved since the walk began labels no loaded row until the refresh that saw it
  reopens the history, which it does at once; a press in between pages the walk and finds
  the commit only in a page still to come.
