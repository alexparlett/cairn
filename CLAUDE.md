# Cairn

Cairn is a native git client for Linux (and, where the toolkit allows, macOS),
aiming at what Fork and Sourcetree do well — a readable history graph, a diff you
can stage by hunk, and destructive operations that tell you what they will cost
before they cost it — without the Electron tax. The load-bearing bet is a hard
seam: **the UI describes what it wants, never how a repository is read, and every
repository access is a value-returning call the view layer cannot make itself.**
Testable form: `cairn-ui` compiles with neither `gix` nor `cairn-git` in its
dependency graph, and nothing outside `cairn-git::ops` can mutate a repository.

Status today: the workspace, the seam, the gate and the guard suite exist and are
green. The first repository read exists — `cairn-git`'s bounded, resumable history
query, feeding the lane assigner in `cairn-model` — and it is wired to the window
through the worker boundary in `crates/cairn-app/src/worker/`. The application
opens the repository named on its command line (or the working directory), draws
its history as a virtualized graph with lanes, edges and four columns, pages as
you scroll, and does all of it off the UI thread. It can fetch its default
remote — the one `git` verb built so far, with git's own progress, a cancel,
and a credential dialog fed by the askpass helper (`docs/systems/credentials.md`).
Every `git` process runs on one runner in `cairn-git`'s private `process/`
module (`docs/systems/git-processes.md`): `git` is found once, as the
application starts; fetch runs in the network lane, which refuses a second
fetch with a reason the window draws; each repository keeps a registry of the
`git` it is running and a bounded command log, which the worker answers as
values and no view draws yet; and closing the window closes its repository,
ending and reaping every `git` Cairn started in it before the window goes
(what git itself detaches from the group, such as auto-maintenance, is not
Cairn's to end).
The engine can also answer what a commit or a pair of commits changed — `git
diff-tree`'s answer, run as a read — and what one of those files' change is, line
by line, and one path's staged, unstaged or untracked diff in the working tree
(`docs/systems/diff.md`); the worker asks those
queries on a diff thread of its own, numbered per lane so a scroll and a diff
never cancel each other, and the window keeps each answer only for the
selection it names. Choosing a commit asks what it changed, and the detail pane
under the list — behind a draggable, collapsible splitter, with Commit and
Changes tabs — draws its author, committer, id, parents, message and changed
files in the Commit tab; a file pressed there opens its diff in place under its
row, as Fork does, and Expand All opens the files in order until a budget of
fifty thousand lines is spent, then says how many it left collapsed — read a
page at a time on the diff thread, each file decided before its blobs are read,
a file that fails failing alone. The Changes tab is Fork's: a one-line summary,
the changed files behind a filter answered on a worker, the first file chosen,
and one file's diff, as the unified rows `git diff` prints — Fork's small
line-number gutters and no marker column, hunk headers with git's function
context, intra-line ranges, in IBM Plex Mono — or side by side, under Fork's bar of previous and next change and
the ignore-whitespace, context, entire-file and side-by-side buttons, the
context starting at the user's `diff.context` (`docs/systems/diff.md`, "The
diff view"); or the notice of a state that is not text, with Load Diff for a
file past the limits and every line past the long-line limit drawn cut. A
second commit pressed with ⌘ or Ctrl compares the two, tip against tip with
the lower row the base, in the Changes tab under a header naming both and a
swap, the Commit tab unavailable while two are selected. Keyboard shortcuts —
and a press's modifiers, which this toolkit build does not carry on a pointer
event — resolve through one accelerator table (`cairn_ui::accelerators`), each
action a list of chords heard in its own scope, and every text field hands the window
its chords and held keys (`cairn_ui::text_field`).
The engine also reads a repository's refs, upstreams, stash list and ahead/behind
counts as git lists them (`docs/systems/refs.md`), and its working tree's status as
`git status` answers it (`docs/systems/status.md`). The window
walks the history from every ref, each row labelled and each stash a row of its own
(`docs/systems/history-graph.md`): each ref is Fork's chip between the graph and the
subject — lane-tinted, a tag indigo, each kind told by a glyph painted as a shape, the
current branch first with a check mark, a branch and its upstream at one commit one
chip, chips cut at the column's edge and built only as far as it — a stash's row a
`stash@{n}` chip, `HEAD`'s subject bold; the Commit tab draws the same chips in a REFS
row; selecting a stash lists what `git stash show` lists, untracked files included where
the user's `stash.showIncludeUntracked` says so; and the title bar names the repository,
starred while status lists a change, the current branch and how far it is behind and
ahead of its upstream, or that the upstream is gone, a detached or unborn `HEAD` in
git's words. It refreshes — the refs on the history thread,
status and ahead/behind on a refresh thread of their own, the refs and ahead/behind
superseding the refresh before them and a running status left to finish, one follow-up
behind it — when it gains focus, when a fetch ends and on the Refresh action (F5, ⌘R on macOS), reopening
the history only when what it draws changed, its old rows freed on a worker.
Left of the history, behind a draggable splitter, is Fork's sidebar
(`docs/systems/sidebar.md`): Local Changes with its count of distinct paths and All Commits,
which switch the main region; a filter; and
Branches, Remotes, Tags and Stashes in one virtualized list, branches and remotes in
folders split at `/`, the current branch checked and bold, each branch's counts or its
gone upstream — the rows laid out on the repository thread for the filter's text and what
is open. Pressing a ref selects its row and brings it into view, or finds it by paging the
history's walk forward ("Finding <ref>…"), every page kept as a scroll's would be, the
next press, a scroll or a row chosen superseding the find; a tag on a tree, and a stash
whose base no ref reaches (its changes shown), say they are not in the graph.
Local Changes is Fork's view in the main region (`docs/systems/local-changes.md`):
a filter answered on a worker over Unstaged above Staged behind a splitter, each a virtualized
list of paths in Fork's natural order with Fork's badges, told apart by shape, laid out on the
refresh thread as the status is read, a path with both
changes in both; the first path chosen as the view opens and its staged, unstaged or untracked
diff asked through the working-tree query by its list, in the file-diff lane the Changes tab
and the files opened in place share, and drawn under the diff view's bar for that exact query
alone; a conflicted path's notice in place of a diff; and the path chosen followed through
each refresh — asked again while listed, its diff drawn meanwhile, another chosen once the
status no longer lists it, a path the filter hides kept as the Changes tab keeps one. Its
lists act on files as Fork's do: a multi-selection (⌘- or Ctrl-click, Shift-click, Shift+↑/↓),
staged or unstaged by Fork's five routes — a double press, the table's chords on a focused list
or the diff, a drag from one list dropped on the other (tracked by the lists, so it survives its
row, the list it would drop on scrolling at its edges), each list's button (Stage All or Unstage
All with ⌥ held) and the double chevron, and the context menu — the selection then moving to
the nearest path left; a discard on the unstaged side only, never of a submodule or a conflict
(each says why), asked of the engine on the local lane before the confirmation drawn from what
it would lose; a conflicted row staged whole by `git add`; the diff always the exact one; and
each write drawn under the lists, queued, running or how it failed. Several paths selected draw
their diffs together, as Fork does, each under its own row, read on the diff thread under Expand
All's line budget. Over the diff is Fork's staging gesture, in Local Changes alone: a hovered
chunk outlined, floating Stage and Discard Changes… (Unstage over the staged diff), a
drag-selection across lines — tracked by the list, so it survives the rows it scrolls past, side
by side kept to its column — narrowing them and the chords to its lines, a mode change's own row,
every line of a new file discarded as the file. Under the diff is Fork's commit box: a subject
with its characters-left counter and Recent Commit Messages (a menu, and ↑/↓ in an empty or
recalled subject), a description with a ruler at column 72, the draft the window's for its life,
`Commit N Files`, and Amend — which sets the draft aside, fills an empty one with `HEAD`'s
message, draws amend's staged list in Staged (diffed against `HEAD`'s parent and unstaged back to
it), and is confirmed by its own button or the commit chord where no remote has `HEAD` and by the
confirmation dialog where one has, a failed amend's skip amending at once under the line confirmed
— each token built by a confirmation surface alone; a commit running drawn busy
with its elapsed time and a Cancel; a failed one opening Fork's Git Error dialog over its output,
ANSI stripped, the skip offered only where a hook exists; a merge filling an empty draft with
`MERGE_MSG` as git wrote it; and the box disabled during a rebase, `git am`, a cherry-pick or a
revert, naming it (`docs/systems/local-changes.md`, "The commit box"). The engine
(`docs/systems/staging.md`): lines and files staged and unstaged through git's own verbs,
each patch checked against the index and the file before it applies, and the two discards —
lines, and whole files with untracked ones deleted — sealed behind a confirmation the engine
computes and re-checks; and commit and amend through `git commit -F -`, the message byte for
byte as git's own `-F` stores it, amend sealed behind a confirmation naming `HEAD`, whether a
remote has it and whether git will log it, refused during a rebase, `git am`, a cherry-pick or
a revert, with the hooks git would run, the last ten messages and amend's staged list read
beside it; and the worker runs them on a local write lane of its own
(`docs/systems/git-processes.md`, "The local write lane"): one at a time in the order asked,
each with an askpass token whose prompt the window shows titled by its own operation, a status
read across a write never drawn, the read after each write what it left stale, a commit
cancellable by its id, and a close that waits on the write running and names a lock left
behind the next time the repository opens.
Nothing else mutates a repository, and there is no repository picker: one
repository, named on the command line.

## Repo map

| Path | What lives there |
| --- | --- |
| `docs/` | `qa-gate.md` (QA contract), `design/` intent, `prd/` per-packet specs, `systems/` as-built, `work/` in-flight dirs, `research/` evidence (deferred work goes to GitHub issues; `backlog/` is the no-remote fallback) — findings promote research → brainstorm → design/prd → systems (contract: `docs/CLAUDE.md`) |
| `crates/cairn-model/` | The vocabulary crossing the seam: `Oid`, `RefName`, `RefsSnapshot` (refs, `HEAD`, the stash list, upstreams; with `AheadBehind`), `WorkingTreeStatus` (per path what `git status` lists — a staged and an unstaged change, a conflict's kind, a submodule's state, or untracked — or that git cannot read the index) and `LocalChanges` (a status laid out as Local Changes' Unstaged and Staged lists, in Fork's natural order of their paths (`path_order`), with each row's `ChangeKind`, the count of distinct paths, a path's row found by a search and the filter's pass, `MatchedRows`, and — while Amend is ticked — Staged as amend's staged list against `HEAD`'s parent (`LocalChanges::amending`, `StagedAgainst`); `src/local_changes.rs`), `History` (the rows a reader keeps, slim and owning no heap allocation of their own — an id, a parent count, a subject and an author held in the history's shared text store and author table, a date, a lane and its lane changes — in stores that grow in fixed chunks, never by doubling (`src/chunked_store.rs`), beside an index of authors by name that is a standard hash map and does double; a row is a commit's or a stash's (`RowContent::Stash`, `RowId::Stash`), and the refs labelling a row's commit (`RowLabels`, `Label`; `src/row_labels.rs`) and what a stash's row keeps sit beside the rows in stores of their own; a page arrives as a `RowsPage` and a row is read as a `HistoryRow`; a row's labels are kept by name, so `RowLabels::find` searches them, and each history is numbered, `History::serial`, so a place found in one is never trusted in the next; `History::labelled_position` finds a pressed ref's row among the labelled, stash and `HEAD` rows alone), the sidebar's rows (`SidebarRow`, `SidebarSection`, `Disclosure`, laid out by `RefsSnapshot::sidebar_rows`; `src/sidebar_rows.rs`), `CommitSummary` (a row as the list draws it), `StashSummary` (a stash's row), `CommitDetails`, `ChangeSet`, the `Confirmed` token and the `Consequence` it carries (what a destructive operation will destroy, with the prompt and button rendered from it — an amend's two texts apart too, `force_push_warning` and `replaces`, the prompt exactly the two joined, and `needs_force_push` deciding its dialog; `src/consequence.rs`), `OperationInProgress` and `CommitHooks` (what the commit box reads). Plain data, plus the pure algorithms that produce some of it — the layout one (`LaneAssigner`, whose `GraphRow` keeps only a commit's lane and the lane changes at it (a stash's row, `LaneAssigner::push_stash`, one line in a lane of its own to the commit it was made on), every `LaneAssigner::SNAPSHOT_EVERY`th row a lane snapshot, and `row_edges`, which derives the edges a drawn row crosses from any `LaidOutRows` — a slice of `GraphRow`s or a `History`; `docs/systems/history-graph.md`) and the diff model (`TextDiff` and the hunk, row and patch projections of it, `ShownDiff` — one answer prepared for the views, built on the worker — `Selection`, `emit_patch` and the reference `apply_patch`, and what staging, unstaging and
discarding a selection emits — `action_patch` with a `PatchAction`, over the inversions
`TextDiff::inverted`, `Selection::inverted` and `ChangedFile::inverted`, so nothing is applied
with `-R`; and what the staging gesture selects of the rows drawn — a drawn hunk's changed lines
and the changed lines in a span of rows, one column side by side (`hunk_at`, `hunk_selection`,
`selection_in`, `SideColumn`; `src/row_selection.rs`); `docs/systems/diff.md`) — and `Secret`, the one type that holds a credential. Depends on nothing but `zeroize` (for that type) — not `gix`, not `freya`, not the other crates. |
| `crates/cairn-git/` | The repository engine: gitoxide-backed reads — the history walk (from `HEAD`, given commits, or a refs snapshot: every ref, each row labelled, each stash whose base is walked a row of its own, merged in by `src/history/stream.rs`; with Show Lost Commits, every reflog entry of `HEAD` and each local branch too, each log read whole by git's rules (`src/history/reflogs.rs`), the commits no ref reaches marked as the walk goes (`src/history/reach.rs`)), the refs snapshot and each branch's upstream (`src/refs.rs`, `src/refs/`), ahead and behind (`src/ahead_behind.rs`), the ref-storage refusal at open (`src/ref_storage.rs`; `docs/systems/refs.md`), the working tree's status (`src/status.rs`, asked of `git status` through `src/reads/`; `docs/systems/status.md`), and under `src/diff/` the queries answering what a commit changed (asked of `git diff-tree` through `src/reads/`), what one file's change is, and one path's working-tree diff — and under `src/ops/` every write, delegating to the `git` binary per design decision D1. Every `git` process is built in the crate-private `src/process/` — `GitBinary` (startup discovery and the 2.30 floor), `GitEnvironment` (the explicitly built environment, the only place a `Command` is built), `Askpass` (where git and ssh are sent for a secret), the runner, which streams and can kill a process and collects under a ceiling only for a read (`collect` and `finish_within` exist on `Invocation<Read>` alone, pinned by `the_bounded_output_helpers_exist_on_a_read_alone`), and each repository's registry of running invocations and its command log — and an invocation is typed a read or a write, a write needing the `WriteAuthority` only `ops/` can construct. `src/ops/` holds `fetch` (not destructive, so it takes no `Confirmed`), and the local write verbs (`docs/systems/staging.md`): `stage_lines` and `unstage_lines` (`git apply --cached`), `stage_files` (`git add`) and `unstage_files` (`git reset -q`, or `git rm --cached -f -q` out of a root commit's amend) in `src/ops/stage.rs`, and the destructive `discard_lines` (`git apply`) and `discard_files` (`git restore --worktree`, then `git clean -f -q --` in batches of `CLEAN_ARGUMENT_BYTES`) in `src/ops/discard.rs`, each taking `Confirmed` and beside the builder of its `Consequence` — every verb run with `--literal-pathspecs` before it (`src/ops/local_write.rs`), each patch checked against the index (gix) and the file first (`src/ops/fresh_state.rs`) — and `commit` and the destructive `amend` (`git commit -q [--amend] -F -`, the message on stdin, no `--literal-pathspecs`) in `src/ops/commit.rs`, amend taking `Confirmed` from `amend_consequence` (`src/ops/amend.rs`: `HEAD`, whether a remote has it, whether git will log it); `create_branch` (`git branch -- <name> <commit>`, `src/ops/branch.rs`) and, in `src/ops/checkout.rs`, `create_branch_and_checkout` (`git checkout -q -b`) and the destructive `create_branch_discarding` (`git checkout -q -f -b`, taking `Confirmed` from `checkout_discarding_consequence`, which counts the untracked files in the way through `reads::untracked_paths`), Create Branch's verbs and Show Lost Commits' way back, with `Repository::branch_name` (`src/branch_names.rs`) asking whether a name can be a new branch's — refusing one a branch has or one a branch's directory holds; beside them the reads the commit box needs — the operation in progress as git's status reads it (`src/operation_in_progress.rs`), the hooks git would run (`src/commit_hooks.rs`), the last ten messages (`src/recent_messages.rs`), and amend's staged list and the parent it is against (`src/diff/amend.rs`: `amend_staged`, `amend_parent`), one file of which `WorkingTreeDiff::Amending` diffs — and re-exports what the application needs of `process/`; `src/reads/` is where each read `git` answers lives, one named function each: today `changes`, `git diff-tree` for the changes query, whose rename and copy pairs gix and git disagree on; `patches`, `git diff-tree -p` for the content query's changed lines and function context, whose line diff gix and git disagree on too; `diff_attributes`, `git check-attr`, which says whether a path's diff driver names its own algorithm; `working_tree_patch`, one path's staged, unstaged or untracked diff (`git diff-index --cached`, `git diff-files`, `git diff --no-index`), which reads the working tree through git so its side is git's form of the file, with `staged_pairing` beside it, `git diff-index --cached --raw` over the whole index, the rename or copy the user's `git diff --cached` pairs a path into, and `staged_since`, the same over the index against `HEAD`'s parent, amend's staged list; `hash_object`, `git hash-object --path=<p> -- <p>` without `-w`, a working-tree file's id in git's form, for a discard's stale check; `hooks_path`, `git rev-parse --git-path hooks`, where git runs hooks from, for `Repository::commit_hooks`; `fetch_settings`, `git config` in query form, what a fetch of a remote will read, for fetch's refspec check (`src/ops/refspec_policy.rs`), which must decide on exactly what the fetch's own git reads; `status`, `git status --porcelain=v2 -z` — read again with `--untracked-files=all` only where the first answer collapsed an untracked directory, so git itself reads `status.showUntrackedFiles` — the working tree's status, which gix answers differently wherever status is hard; and `stash_changes`, `git stash show --raw`, what a stash changed, its untracked files paired with the rest as only git pairs them, git reading `stash.showIncludeUntracked` itself. Speaks `cairn-model` types at its boundary; `gix` types never appear in a public signature. Must never depend on `freya` or `cairn-ui`. |
| `crates/cairn-askpass/` | The askpass helper binary `git` and `ssh` run to ask for a secret, and the library half — the `Channel` the application listens on. Links `cairn-model` and `zeroize` only: it runs in a process holding a plaintext secret. Never names the engine, the toolkit or a logging crate. |
| `crates/cairn-ui/` | Freya components. Render `cairn-model` values, report intent through `EventHandler` props. `src/accelerators.rs` is the accelerator table, the one render file that names a modifier; `src/diff_view.rs` the diff view, drawing `src/unified_rows.rs` or `src/side_by_side_rows.rs` (each from `src/diff_row_parts.rs`), `src/diff_notice.rs` what stands in place of rows, `src/changes_list.rs` the Changes tab's filtered file list and summary, `src/diff_header.rs` its bar (whose glyphs `src/toggle_glyphs.rs` draws), `src/columns.rs` the terminal column widths tabs stop by, `src/diff_settings.rs` the diff settings — context, ignore-whitespace and side-by-side shared by every diff view, Entire File the Changes tab's and Local Changes', never a file opened in place, `src/diff_palette.rs` the diff's colour tokens and typeface, `src/commit_tab.rs` the Commit tab and `src/expansion.rs` where each file opened in place under its row falls in that tab's one list; `src/ref_chips.rs` a row's chips — which, in what order, compacted, and built only as far as the column's edge — and `src/ref_glyphs.rs` their glyphs, painted as shapes; `src/status_box.rs` the title bar's status box; `src/sidebar.rs` the sidebar — Local Changes and All Commits, its filter box, and its one virtualized list of sections, folders and refs, each row drawn from the snapshot by index (`drawn_row`); `src/local_changes.rs` Local Changes' filter field and its Unstaged and Staged lists, each virtualized, each row a badge and a path read by index, each heading its Stage or Unstage button, every press, chord, drop and menu choice reported as a `ListIntent`, with `src/list_selection.rs` the paths selected (`ListSelection`) and the nearest row left after an action, `src/local_changes_drag.rs` the drag between the lists, kept by the lists rather than a row, and `src/local_changes_menu.rs` the context menu and why a selection offers no discard (`NoDiscard`); `src/end_room.rs` the empty rows after a list's last, so the horizontal scrollbar Freya draws over a list's bottom never covers it; `src/text_field.rs` the one key policy every text field is built with (`text_field`, `text_field_in`, and the commit box's subject, `text_field_recalling`, which offers a bare ↑ or ↓ — `accelerators::recall_step` — to the box first); `src/confirm_dialog.rs` the confirmation a destructive operation waits on — a `Consequence`'s prompt and button, focus on Cancel, modal, the token built on its button; `src/commit_box.rs` the commit box (`CommitBox`, `CommitButton`, Fork's counter, the Amend toggle reporting its toggled state, the busy line and its timer), the second confirmation surface — `AmendButton`, the amend button drawn with the line it confirms, which the commit chord presses too and which refuses an amend a remote has, and `AmendSkip`, a failed amend's skip drawn in the Git Error dialog with the prompt the amend was confirmed with and `src/git_error_dialog.rs` Fork's Git Error dialog, its output through a virtualizing view; `src/edge_scroll.rs` the edge auto-scroll of a drag over a virtualized list, the drag tracked by the list and paced by `async-io`'s timer, and ended by a press heard while it is on (its release lost outside the window) or the window losing focus; `src/staging_gesture.rs` the diff's staging gesture (`Gesture`, `LineDrag`, `ModeRow`) — a hovered chunk's outline and floating actions, a drag-selection's tint, one layer laid over the recycled rows, the drag heard by the view's root — which a diff view draws only when handed one; `src/stacked_diff.rs` several files' diffs drawn together (`StackedDiff`) on the Commit tab's `Expansion`; `src/check_box.rs` the one check box (Amend, Show Lost Commits, "Check out after create"); `src/accelerators/chord_names.rs` a chord spelled for a tooltip from the table; `src/button_order.rs` where every dialog's buttons sit on each platform (the primary first on Linux, last on macOS), and `src/answer_button.rs` the answer a risky dialog opens focused on (Cancel, Close); `src/create_branch_dialog.rs` Fork's Create Branch dialog, opened by `New Branch…` on a commit row (`HistoryList::on_new_branch`) or at `HEAD` by Fork's New Branch chord (`Action::NewBranch`). Must never depend on `gix` or `cairn-git`, and must never touch the filesystem. |
| `crates/cairn-app/` | The binary. Owns the window, the worker threads, and the wiring between engine and UI — the only crate where the two layers meet. `src/worker/` is everything that may wait: `git` found once per application (`discovery.rs`), each repository's threads (`pool.rs`), the routing table from query lane to thread (`routing.rs`) and the per-lane epochs (`epoch.rs`), the history lane's walk, the refs it walks from and a find's pages (`history_lane.rs`), the refresh thread's status and ahead/behind (`refresh_lane.rs`), the diff thread (`diff_lane.rs`) and Expand All's line budget (`expand_all.rs`), the network lane (`network_lane.rs`), the local write lane (`local_lane.rs`: every write to the index, the working tree and a local ref, one at a time in the order asked, each under an `OperationId` and with an askpass token of its own, the write clock a status read across a write is dropped by, refreshes kept back while a commit runs, the close that waits on the write running, and, in the lane's order, what a discard would lose and what the commit box reads — its reads and an amend's, each numbered in a lane of its own; `docs/systems/git-processes.md`, "The local write lane") and the askpass acceptor; `src/local_writes.rs` the local writes as the window keeps them — queued, running, ended, the lock files last listed and the write a close waits on; `src/refresh.rs` asks for a refresh when the window gains focus and `src/refresh_state.rs` keeps what the last refresh answered; `src/sidebar_state.rs` the sidebar's rows as the window keeps them, what is open and the entry pressed, `src/sidebar_pane.rs` draws it and `src/ref_find.rs` is what a press does — its row selected, or found by paging the walk; `src/local_changes_state.rs` Local Changes' lists as the window keeps them and the filter's rows, `src/local_changes_pane.rs` draws the view and follows the path chosen through each refresh, `src/commit_box_state.rs` the commit box as the window keeps it — the draft (`LocalChangesView::commit`), Amend and the draft it set aside, what was read for it, the commit asked, its output's bounded tail and the Git Error dialog — and `src/commit_box_pane.rs` draws it and is what its presses and its updates do; `src/local_changes_actions.rs` is what the lists' intents and the diff's gesture do — the selection, the writes each route asks, the selection moved on, a discard's consequence asked and its confirmation opened, lines staged, unstaged and discarded, and what is said under the lists — and `src/diff_state/working.rs` is that path and its working-tree diff in the diff selection, `src/diff_state/together.rs` the paths selected drawn together; `src/diff_state.rs` is the diff selection and the answers kept for it; `src/selection.rs` chooses a row, or two to compare, and asks what they changed; `src/row_finder.rs` finds the selected row once per history, for the Commit tab's REFS row; `src/detail_pane.rs` draws the pane for the selection now and `src/changes_tab.rs` its Changes tab; `src/file_filter.rs` the Changes tab's filter as the window keeps it; `src/diff_actions.rs` chooses a file, changes the shared diff settings and moves between changes; `src/shortcuts.rs` is what each accelerator, and each button of the diff's bar, does — nothing while a credential prompt, a confirmation or the Git Error dialog is up; `src/create_branch.rs` Create Branch as the window keeps it — the dialog, its name asked of the engine, the sticky check box, Discard counted then confirmed, a failure's Git Error dialog and the name kept; `src/confirming.rs` the confirmation the window keeps open (`View::confirming`: the `Consequence` and where its token goes, never a token), drawn over everything with a context-menu host at the window's root; `assets/fonts/` holds the embedded IBM Plex Mono and its licence; `src/closing.rs` is the window's close hook, which asks the worker to close and never waits, and tells the window which write it is finishing; `src/window_check.rs` is C14's window check, an `#[ignore]`d measurement of the real window over the bench repository. |
| `crates/cairn-guards/` | Test-only. The deterministic enforcement twins for the Invariants below; nothing depends on it. |
| `scripts/`, `.githooks/`, `.github/` | The enforcement layer (contract: `docs/qa-gate.md`). |

Directories with their own CLAUDE.md carry local conventions; read it when you work
there.

## Commands

- `scripts/gate.sh` — the pre-merge gate: format, lint, typecheck, guards,
  dependency policy, full test suite, doctests, and `git-floor` (`cairn-git`'s
  real-git diff, status and lost-commits tests against git 2.30.9 and 2.32.7, built from source by
  `scripts/git-floor.sh` into `~/.cache/cairn/git-floor` on the first run, which
  needs the network, a C compiler, make and zlib's headers, and fails naming what
  is missing rather than skipping). Every `--step` but `test-fast` runs in it
  (`the_local_full_gate_runs_every_step_but_the_day_loops`). Exit-code safe; run it
  before calling a change done instead of an ad-hoc `&&` chain (piping test output
  through `tail`/`head` masks the exit code).
- `scripts/gate.sh --fast` — day-loop subset. Never the merge bar; deliberately
  skips network-dependent checks (`deps`, `git-floor`) so the day loop stays usable
  offline.
- `scripts/gate.sh --step <name>` — one named gate component. CI uses this
  interface so CI and local runs share the same command implementation.
- `cargo run -p cairn-app` — run the app. `cargo run -p cairn-app --release` for
  anything where frame time or a large repository is the point; the dev profile
  builds dependencies at `opt-level = 3` but Cairn's own crates at 1. The askpass
  helper is a second binary that `-p cairn-app` alone does not build: run
  `cargo build --workspace` first (or `-p cairn-askpass`), or fetches needing a
  prompt fail with a message saying so.
- Toolchain is pinned in `rust-toolchain.toml`; `cargo deny` is the one Rust tool the
  gate needs that rustup does not ship (`cargo install cargo-deny --locked`), and
  `git-floor` needs a C toolchain and zlib's headers (`build-essential zlib1g-dev`
  on Debian and Ubuntu).

**Version-sensitive API rule:** for any fast-moving dependency, verify APIs you are
not certain of against current docs before writing them. Never code such an API from
memory, and flag memory-coded usage in review. Here that means **`freya` (0.5 is a
release candidate on a builder API that replaced the old `rsx!` macro — anything
you remember about Freya from `rsx!` examples is wrong) and `gix` (pre-1.0, and its
feature flags gate whole modules: `default-features = false` silently produced an
empty `gix_hash::Kind` once already)**. Read the vendored source when the docs are
thin: `~/.cargo/registry/src/` for a registry dependency such as `gix`, and
`~/.cargo/git/checkouts/` for a git-pinned one such as `freya`, whose registry
copy is a different version from the fork that links.

## Default task workflow

- Branch per standalone task: `feature/<slug>` or `fix/<slug>` off `main`; keep
  `main` green. Packet phases follow the mode-specific flow below instead.
- Feature packets get ONE long-lived integration branch, `feature/<packet-slug>`,
  created in its own worktree when the packet's first implementation phase starts.
  **User-mode branch rule** (the default when the user starts one phase): create a
  runtime-owned phase branch from `feature/<packet-slug>` before editing, then raise
  a templated pull request back into that integration branch; never merge it.
  **Packet-mode exception:** only an explicitly declared packet coordinator (the
  `/orchestrate-packet` skill) and the phase agents it dispatches may commit phase
  work directly to `feature/<packet-slug>`; packet mode raises no per-phase PRs.
  **EVERY pull request — phase PRs into integration and packet or planning-doc PRs
  into `main` — is merged by the USER, never by an agent**, after a human has read
  the code. `.claude/settings.json` denies the common spellings of merging and
  pushing to `main`, but it binds Claude Code sessions only and cannot enumerate
  every route: treat the rule as the authority and the deny list as a backstop.
  Packet planning docs land on `main` via their own PR, so every session shares the
  current plan.
- For parallel or long-running tasks, use a separate git worktree per task so
  sessions cannot trample each other; a packet's branch lives in its own worktree.
- Read the relevant local CLAUDE.md and existing implementation before modifying.
- Make ALL changes the objective needs: code, data, tests, docs. No unrelated
  refactors. A change to `cairn-model`, to anything under `cairn-git/src/ops/`, or
  to `cairn-guards` requires a test in the same commit — those are the seam, the
  destructive surface, and the enforcement layer, and each is a place where a
  silent regression is expensive and invisible.
- Big or multi-session work goes through a feature packet: run `/feature-plan` to
  design it with the user first.
- Done means: `scripts/gate.sh` passes locally and `/qa` has reviewed the diff.

## Architecture (the load-bearing ideas)

- **The engine is authoritative; the view is a projection.** `cairn-git` answers
  questions and performs operations; `cairn-ui` renders answers. A component that
  needs new information asks for a new `cairn-model` type and a new engine call —
  it never reaches for a repository, a path, or a subprocess. Enforced by the
  dependency allowlist and the crate-seal guard.
- **`cairn-model` is the whole contract between them, and it is plain data.**
  Neither side may leak its own vocabulary across: no `gix::ObjectId` in a
  component, no `freya` type in the engine. That is what keeps the backend
  replaceable — the decision to bet on gitoxide is reversible exactly as long as
  this holds.
- **Reads go through gitoxide; writes go through the `git` binary.** Decision D1
  in `docs/design/engine.md`: a mutation must run the user's hooks, filters and
  credential helpers and honour their config, and gix runs none of them. The
  writes today are `git fetch` and the local verbs of `docs/systems/staging.md` —
  `git apply --cached` and `git apply` with the model's patch, `git add`, `git reset
  -q`, `git rm --cached -f -q`, `git restore --worktree` and `git clean -f -q`, each
  after git's global `--literal-pathspecs`, `git commit -q [--amend] -F -`, the
  message on stdin, `git branch -- <name> <commit>` (`ops::create_branch`), and `git
  checkout -q [-f] -b <name> <commit> --` (`ops::create_branch_and_checkout`, and the
  destructive `ops::create_branch_discarding`, Create Branch's "Discard"). A read
  runs `git` only where gix's answer differs from git's — the changes query, whose
  rename and copy detection is where they disagree (`reads::changes`,
  `git diff-tree`), and the content query's changed lines, function context and
  whitespace-ignoring lines, where their line diffs disagree (`reads::patches`,
  `git diff-tree -p`, with `reads::diff_attributes` beside it), and one path's
  working-tree diff, where only git's own read of the working tree is git's form
  of it (`reads::working_tree_patch`: `git diff-index --cached`, `git
  diff-files`, `git diff --no-index`, with `reads::staged_pairing`, `git diff-index
  --cached --raw` over the whole index, pairing a staged rename as `git diff --cached`
  does), and a working-tree file's id in git's form for a discard's stale check
  (`reads::hash_object`, `git hash-object --path=<p> -- <p>`, never `-w`), and where git
  runs hooks from (`reads::hooks_path`, `git rev-parse --git-path hooks`), and whether git
  takes a name as a branch's (`reads::branch_name`, `git check-ref-format --branch`) and the
  lines of the changes Create Branch's discard would lose (`reads::change_lines`, `git
  diff-index --cached --numstat` and `git diff-files --numstat`) and the untracked files it
  would overwrite or remove, whatever `status.showUntrackedFiles` says
  (`reads::untracked_paths`, `git ls-files --others --exclude-standard -z`) — each a
  question only git can answer — and the remote configuration fetch's
  refspec check decides on, where gix's reading of a linked worktree's
  `includeIf`, of the system file and of trust is not git's
  (`reads::fetch_settings`, `git config`), and the working tree's status, where
  gix's answer differs from git's on renames past its limit, conflicted paths,
  sparse checkouts and a lying fsmonitor hook, and gix would start clean filters
  outside `process/` (`reads::status`: `git status --porcelain=v2 -z`, and the
  same with `--untracked-files=all` only where the first answer collapsed an
  untracked directory, so that git reads `status.showUntrackedFiles` itself;
  never `--ignored`, a rename option or `--ignore-submodules`) — and each such
  read is a named function in
  `cairn-git/src/reads/`, run as a read invocation: query plumbing (never a plumbing writer such as
  `update-ref`, `update-index` or `write-tree`), `status`, or one of the three
  porcelain exceptions, each accepted by the user — for an untracked file,
  `git diff --no-index -- /dev/null <path>`, `<path>` work-tree-relative (no
  absolute, `.` or `..` component, refused before git runs) and given as `./-`
  when it is `-`, which git reads as stdin (it reads no index, so there is none
  to refresh; its presentation settings are pinned by `-c`); and, for fetch's
  refspec check, `git config --includes --null` with `--type=bool --get <key>`
  or `--get-all <key>`, query form only and never a setter
  (`reads::fetch_settings`, 2026-10-04: the check must read the remote exactly
  as the fetch's own git will, and fails closed when the read fails); and, for
  a stash's changes, `git stash show --raw -z --no-abbrev --no-color
  --no-ext-diff --no-textconv --no-relative --end-of-options <stash commit>`
  (`reads::stash_changes`, 2026-10-07, accepted by the user: with
  `stash.showIncludeUntracked` set, git pairs a stash's untracked files with
  its tracked changes in one diff no plumbing can ask without writing a tree,
  and git reads the setting itself, as the user's own `git stash show` does,
  git 2.30 and 2.31 ignoring it) — only,
  `GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`, no askpass token. Everywhere gix
  agrees with git, a read spawns no process — that is the whole reason the split
  pays. D1 is amended for the programs git itself starts on a read, each exactly
  as the user's own `git diff` starts it: the repository's `core.fsmonitor` hook,
  as git reads the index of a repository with a working tree — or, under
  `core.fsmonitor=true`, git's own fsmonitor daemon, started if none is running,
  which writes its socket and cookie directory in the git directory (the one
  thing a read leaves there: no object, ref, index or config) and, in a session
  of its own, outlives the read and the application, not Cairn's to end; and, on a read of
  the working tree, the path's clean filter driver — git-lfs, git-crypt; a
  `clean` command, or the long-running `filter.<driver>.process` git-lfs
  installs, which git sends only `command=clean` — which
  converts the file to git's form (and, for a submodule, `git status` inside it,
  with that repository's own hook and filters). The driver runs as a child of the
  read's `git`, so with the environment Cairn built for that read (the inherited
  roster, the `ALWAYS` table, `GIT_ASKPASS` and `SSH_ASKPASS` naming Cairn's
  helper and, while the application listens for it, `CAIRN_ASKPASS_SOCKET` — so
  a driver can reach the socket, but carrying no token it fails closed — the
  read's two variables, no askpass token) plus what git sets for a filter
  (`GIT_EXEC_PATH`, `GIT_PREFIX`, `GIT_CONFIG_PARAMETERS`, git's exec directory
  first on `PATH`, and `GIT_DIR` and `GIT_WORK_TREE`, since Cairn names every
  repository it opens to git).
  A status read's residuals, accepted as parity with the user's own `git status`
  (`docs/systems/status.md`): under a split index git advances
  `sharedindex.*`'s mtime and under a sparse index the loose tree objects'
  mtimes, bytes unchanged; it runs `git status` inside each submodule, with that
  repository's own hook and filters; and since a read never writes the
  refreshed stat back, a tree whose every file's stat changed is rehashed in
  full on every read until something refreshes the index, which Cairn does not;
  and in a partial clone, a staged rename whose blob only the promisor holds
  fails the whole read on git 2.44 and later (no fetch, nothing listed, where the
  user's own `git status` would fetch and answer), while git before 2.44 ignores
  `GIT_NO_LAZY_FETCH` and fetches, writing a pack
  (`in_a_partial_clone_a_status_read_fails_rather_than_fetching`).
  Residuals, stated in `docs/design/engine.md` ("Reads see git's form"): that
  environment still hands the driver the user's `PATH`, `HOME` and the rest; a
  store the driver keeps is its own to write (git-lfs's `.git/lfs/objects`); a
  driver that fails without being `required` makes git fall back to the
  unfiltered content with a stderr warning Cairn does not show; and `textconv`
  never runs on a read. No other program runs on a read — no textconv, external
  diff, driver `command` or smudge filter — pinned by
  `the_content_query_writes_nothing_and_runs_nothing`,
  `a_working_tree_query_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor`,
  `a_status_read_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor`,
  `a_stash_read_writes_nothing_and_runs_no_program_but_fsmonitor`,
  `hashing_a_file_runs_only_the_clean_filter_and_writes_nothing` (`reads::hash_object`)
  and `the_hooks_path_read_writes_nothing_and_runs_nothing` (`reads::hooks_path`),
  and the daemon's case by
  `a_read_under_the_builtin_fsmonitor_writes_only_the_daemons_own_files`.
  How every `git` process is built, run and ended is
  `docs/design/processes.md`. Consequence for free: Cairn stores no
  credentials, because git's helpers do (D2).
- **Every repository mutation lives in `cairn-git::ops`, and the destructive ones
  are sealed behind `cairn_model::Confirmed`.** The token's only constructor
  takes the engine-computed `cairn_model::Consequence` — what the operation will
  destroy — and renders the prompt from it, so a code path cannot reach a
  discard, an amend or a hard reset without having put those words in front of a
  human, the operation re-checks exactly what they were told, and the operation
  log quotes them afterwards.
- **The UI thread is never allowed to wait on a repository.** `cairn-git` is
  synchronous at its boundary and decides nothing about where work runs (the
  threads it owns read and feed a subprocess's pipes and reap it, inside the
  runner);
  `cairn-app` decides where the blocking work runs and hands results back as
  values (decision D3: one `cairn_git::SharedRepository` — gitoxide's
  `ThreadSafeRepository` — per repository, each worker thread taking its
  thread-local handle once, routed to by an explicit table — the history lane
  (with a find in the sidebar, which pages the same walk, every page answered under
  the walk's own number so a superseded find's pages still arrive), the sidebar's rows
  and a refresh's refs on the repository thread that owns the live walk, the
  changes and file-diff lanes on a diff thread of their own, status and
  ahead/behind on a refresh thread of their own — and every QUERY carrying an epoch
  numbered in its lane, so a superseded query is abandoned rather than
  rendered; a query supersedes only its own lane, except that a changes query
  also supersedes the file diff, and an operation such as fetch carries none,
  so a scroll, a diff and a fetch cannot supersede one another; and every diff
  answer names the selection it answers, which the window checks before it
  keeps it). The epoch IS the cancel signal the
  engine polls, so superseding a query stops its walk, or ends its `git`
  read's process group, rather than discarding its answer; a fetch is
  cancelled by killing its process instead. A repository is somebody's 10-year
  monorepo: any design that assumes a query is fast is wrong.
- **A scroll keeps its walk open.** gitoxide's walk cannot be resumed from a
  value, so a cursor resumes by replaying — which makes page *k* cost `k x limit`
  and does not reach the sizes the history view promises. `cairn-git` therefore
  offers a `HistorySession` that holds the walk for the life of a scroll, making
  paging O(limit); it borrows the repository and is not `Send`, so it lives on
  the worker that owns that handle and never crosses a thread. The cursor
  remains, as the cold-restart path.

## Invariants, YOU MUST keep these

Meta-invariants (keep these; they are what makes the rest durable):

- Every invariant in any CLAUDE.md gets a deterministic enforcement twin (guard
  test, ratchet, hook, or gate step) in the same change that introduces it. A rule
  without a guard is a suggestion.
- Enforcement lives at the STRONGEST tier that can express it — type-level/
  construction seal, then compiler/linter config, then a guard check, then a gate
  step — one authority per invariant. Residual gaps a check cannot express are
  STATED as review obligations, never left implied.
- Every dependency addition is a decision to surface to the user, not a default
  move; dependency policy is a blocking gate step (`gate.sh --step deps`), and
  `deny.toml` records the reason for every exception.
- Never commit secrets or `.env`; never hand-edit generated files (if one is
  introduced, add a PreToolUse deny hook in the same change, see
  `.claude/hooks/README.md`).

Project invariants:

- **Each crate depends only on its allowlist.** Twin: `layer_dependencies_are_allowlisted`
  in `crates/cairn-guards/tests/invariants.rs`. A crate with no row there fails,
  so adding a layer cannot happen by accident. Every dependency table counts —
  `[dependencies]`, `[build-dependencies]`, `[dev-dependencies]` and their
  `[target.*]` forms, renames seen through — and a dev-dependency beyond the
  crate's row needs its own `TEST_ONLY_ALLOWLIST` row (`freya-testing` in
  `cairn-ui` and `cairn-app`; `cairn-askpass` in `cairn-git`, whose fetch tests
  answer a real channel). The allowlist reads names, never features, so the
  dependencies whose features matter are pinned in `deny.toml`
  (`[[bans.features]]`, `exact`), which `gate.sh --step deps` enforces: `nix`, whose
  `process` feature compiles the exec family (`process` and `signal` only), and
  `async-io`, the edge auto-scroll's timer (none, so its optional `tracing` stays
  off).
- **Every embedded font is a user decision, and ships beside its licence.** A font
  file is a dependency `cargo deny` cannot see, so the roster is the guard's:
  `crates/cairn-app/assets/fonts/` holds exactly the files `EMBEDDED_FONTS` names
  in `crates/cairn-guards/tests/invariants.rs`, each font with its licence file
  beside it. Twin: `the_embedded_fonts_are_the_roster_each_with_its_licence`
  (matcher `embedded_font_violations`, self-test
  `the_embedded_font_matcher_catches_the_shapes_it_claims`): another font
  dropped in, a licence file deleted, or a row outliving its font fails.
  Residual review obligation, `qa-checklist`'s: the guard reads file names, so
  whether a licence file holds the right licence for its font, and whether that
  licence permits embedding it, is a judgement.
- **`cairn-ui` and `cairn-model` never name `gix` or `cairn_git`; `cairn-git`
  never names `freya` or `cairn_ui`.** Manifests alone would miss a re-export, so
  the twin reads source: `layers_never_name_the_crates_they_are_sealed_from`,
  over the whole crate directory (`src/` and `tests/` alike), matching aliased
  imports and qualified paths, with the debris hook echoing the same rule in
  milliseconds.
- **Outside `cairn-model`, a `RowContent` — and, in production code, a
  `DiffContent`, a `UnifiedRow` or a `SideBySideRow` — is read by naming every
  variant.** No
  `_ =>`, catch-all binding (`other`, `ref x`, `&_`) or `Some(_)`-beside-
  `Some(RowContent::..)` arm in a match that names it, no `if let`, `while let`,
  let-chain or `let .. else` over it, no `matches!` over it, and no `use` that
  imports its variants or renames it: each compiles once a second kind of row
  exists and silently draws nothing for it. Primary enforcement is the type (not `#[non_exhaustive]`, so an
  exhaustive match breaks when a variant lands); twin against the spellings that
  escape it: `every_view_of_a_row_names_every_kind_of_row`, over every crate but
  `cairn-model` and `cairn-guards`, with its matcher self-test
  `the_row_content_matcher_catches_the_shapes_it_claims`. Residual review
  obligation: the matcher reads spellings, so a helper that returns
  `Option<&CommitSummary>` and is then read partially, or a `type` alias for
  `RowContent`, is `qa-checklist`'s to catch. `DiffContent` is held to the same
  matcher (`reads_enum_partially`) by `every_view_of_a_file_diff_names_every_state`
  (self-test `the_diff_content_matcher_catches_the_shapes_it_claims`), over every
  crate's `src/` but `cairn-model`'s and `cairn-guards`', in production code only:
  test modules, files a parent declares under `#[cfg(test)]` at the top of the
  file and through no other declaration (`#[cfg(not(test))] mod x;` keeps `x`
  scanned; self-tested in `the_diff_content_matcher_catches_the_shapes_it_claims`),
  and `tests/` are left out, since a test asserting one state is a check rather than a view; that a test
  helper of this kind is not used to draw is the same review's. The diff's row
  enums, `UnifiedRow` and `SideBySideRow`, are held to the same matcher over the
  same files by `every_view_of_a_diff_row_names_every_kind_of_row` (self-test
  `the_diff_row_matcher_catches_the_shapes_it_claims`, every shape spelled for
  both enums), since phase 07 brought their second reader; the residuals are
  `DiffContent`'s (a helper handing out one kind of row and read partially, or a
  `type` alias for either enum, is `qa-checklist`'s).
- **Only `cairn-git/src/ops/` mutates a repository**, whether through gitoxide or
  a `git` subprocess. Primary enforcement is the type: a `git` invocation is
  built as a read or a write (`GitBinary::read_invocation`,
  `GitBinary::write_invocation`), and a write consumes a
  `cairn_git::ops::WriteAuthority` — crate-private, a private field, one
  constructor visible to `ops/` alone — so the compiler refuses a write built
  anywhere else, and from outside the crate neither the authority nor either
  builder can be named (the `compile_fail` doctests in
  `crates/cairn-git/src/ops/mod.rs`, with their passing scaffold). Twins
  against erosion, each with a nonzero-files assertion and a matcher
  self-test: `only_the_ops_module_mutates_a_repository` — no product file
  outside `ops/` and `process/` spawns `git` by its literal name, and no file
  of `crates/cairn-git/src` outside `ops/` names gitoxide's mutation API, a roster
  enumerated from the vendored gix 0.87.1 source with each entry's file and
  line beside it in `crates/cairn-guards/src/lib.rs` (self-test
  `the_gitoxide_mutation_matcher_catches_the_shapes_it_claims`);
  `the_runner_is_named_only_by_ops_and_reads` — no file of `crates/cairn-git/src` but
  `process/`, `ops/` and `reads/` names the runner (`GitCommand`,
  `read_invocation`, `Running`, `ProcessKill`, `Invocation`, `KillHandle`),
  none but `process/` and `ops/` names `write_invocation` or
  `WriteAuthority`, none but `ops/` constructs,
  builds a literal of or implements `WriteAuthority`, none declares or
  re-exports any of them `pub`, `process` stays a private module, the
  authority keeps its private field, its `pub(in crate::ops)` constructor and
  no `Clone`/`Copy`/`Default`, and the doctests stay, and `process/runner.rs`
  declares the bounded-output helpers `collect` and `finish_within` once each,
  in `impl Invocation<Read>`, `Invocation::drive` takes its ceiling as the
  kind's `Kind::Ceiling` — `Infallible` for a write, so no generic helper can
  give a write one — and the pin
  `the_bounded_output_helpers_exist_on_a_read_alone`, which stops compiling
  when either helper is given to a write, stays in code under no `cfg` or
  `ignore` (staging-and-commit R4.8; matcher `bounded_helper_violations`,
  self-test `the_bounded_output_helper_check_catches_the_shapes_it_claims`) (self-test
  `the_runner_matcher_catches_the_shapes_it_claims`, and
  `the_unguarded_routes_to_a_process_now_fail_a_twin` over the four routes
  `docs/research/process-manager/runner-and-worker-as-built.md` section 3
  found unguarded); and `the_retired_runner_is_gone` — the runner
  credential-prompts built and process-manager replaced stays gone: nowhere in
  `crates/cairn-git/src`, `process/` and test modules included, is `Running`
  or `ProcessKill` named, `run` or `stream` declared in an `impl` block of
  `GitCommand`, or `.stream(..)` called, and production `process/` starts a
  process by method-call syntax on exactly one line (today `.spawn()` in
  `GitCommand::start_with`) and never calls `.output()`, `.status()` or
  `.exec()` that way, so a second runner that starts its own process by those
  methods fails whatever it is called (self-test
  `the_retired_runner_matcher_catches_the_shapes_it_claims`). Residual review
  obligations: the gitoxide roster reads
  names, so a gix write behind a name it does not hold — an API added after
  0.87.1, or one reached through a trait object, a generic or a macro — is not
  seen; the runner guard reads names too, so a built or started invocation (a
  `GitCommand`, an `Invocation`) or a kill handle (`KillHandle`) handed out of
  `ops/` or `reads/` and driven elsewhere by
  inference (`crate::ops::w(&git).args(..).start()?.finish(..)`,
  `handle.kill()`) is not seen either, and that `ops/` and
  `reads/` hand out only named operation types — as `fetch` does with
  `FetchInProgress` and `FetchCancel` — is review; and the retired-runner twin
  sees a retired entry point declared through a `type` alias of the builder,
  as a free function or by a macro only if it starts a process, counts two
  spawns on one line once, does not see a second path built on `start`
  that drives an `Invocation` by rules of its own, and counts method calls
  only, so a process started in production `process/` by a path call
  (`Command::spawn(&mut c)`, `Command::output(&mut c)`) or through `nix` is
  not counted by it. The terminal-prompt twin still sees most of those —
  every product file but `process/environment.rs` may not name `Command`,
  and every product file is held to its `exec*`/`posix_spawn*` roster — so
  what no twin sees is a path-call start inside `process/environment.rs`, the
  one file allowed to name `Command`. (`nix` named outside `process/` is
  caught by the process twin, and `fork` inside it is `unsafe`, which the
  workspace forbids.) That is `qa-checklist`'s (its item 7). The three porcelain
  verbs a read runs are pinned by
  `the_porcelain_reads_are_the_three_named_queries` (self-test
  `the_porcelain_read_matcher_catches_the_shapes_it_claims`): in the production
  code of `crates/cairn-git/src/reads/`, the exact literal `"diff"` appears only
  in `reads/working_tree.rs`, once, with `"--no-index"` the next literal on its
  line and `"/dev/null"` in the file (the `diff` attribute's two lines in
  `reads/attributes.rs` excused by `DIFF_ATTRIBUTE_LINES`, each required to
  still match); the exact literal `"config"` appears only in
  `reads/fetch_settings.rs`, once, every literal there starting with `-` is one
  of `CONFIG_QUERY_OPTIONS` (`--includes`, `--null`, `--type=bool`, `--get`,
  `--get-all`, one of the last two required), none there is a
  `CONFIG_SETTER_SUBCOMMANDS` word, and no literal anywhere in `reads/` is one
  of `CONFIG_SETTER_OPTIONS` (`--add`, `--unset`, `--unset-all`,
  `--replace-all`, `--edit`, `--rename-section`, `--remove-section`); and the
  exact literal `"stash"` appears only in `reads/stash_changes.rs`, once, with
  `"show"` the literal after it, every literal there starting with `-` is one
  of `STASH_SHOW_OPTIONS` (`--raw`, `-z`, `--no-abbrev`, `--no-color`,
  `--no-ext-diff`, `--no-textconv`, `--no-relative`, `--end-of-options`), each
  of `STASH_SHOW_REQUIRED` (`--raw`, `--no-ext-diff`, `--no-textconv`,
  `--end-of-options`) among them, and no literal anywhere in `reads/` is one of
  `STASH_WRITING_SUBCOMMANDS` (`push`, `pop`, `apply`, `drop`, `store`,
  `clear`, `create`, `branch`, `save`, `export`, `import`). Whether
  a read in `reads/` really runs query plumbing, `status`, `git diff
  --no-index -- /dev/null <path>`, `git config` in query form or `git stash
  show` in raw form beyond those
  literals — a verb or option built at run time (`format!`, `concat!`, bytes)
  is not seen, and
  `GIT_OPTIONAL_LOCKS=0` covers `status` alone, so a porcelain `diff`
  built as a read still rewrites the index, and a plumbing writer built as one
  writes whatever it writes — is `destructive-ops-reviewer`'s (its check 10).
- **Every `git` subprocess runs with an environment Cairn built, and that
  environment always sets `GIT_TERMINAL_PROMPT=0`, `SSH_ASKPASS_REQUIRE=force`,
  `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false` and points `GIT_ASKPASS`
  and `SSH_ASKPASS` at Cairn's own helper; a read adds
  `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1` and never carries an
  askpass token.** A GUI has no
  terminal, so git's own credential prompt would hang the window on nothing,
  ssh would ask for a passphrase on a tty nobody is watching, and a verb that
  wants an editor would launch one nobody can see; a read must not rewrite
  the index it is looking at, nor fetch a missing object from a partial
  clone's promisor remote; and an inherited environment carries whatever
  the launching shell had — a `GIT_ASKPASS` meant for something else, a
  `GIT_DIR` pointing elsewhere. Primary enforcement is construction:
  `cairn_git::ops::GitEnvironment` has one constructor, which copies a
  spelled-out roster from the parent, applies its `ALWAYS` table, and names
  the helper from the `Askpass` it is given (there is no environment without
  one); `GitEnvironment::command` — visible to `process/` alone — is the only
  place a `std::process::Command` is built, clearing the inherited
  environment before applying that one and what the invocation's profile
  adds: the `READ_ONLY` table for a read, whose kind has no field for a
  token, or the askpass token for a write that was given one. The profile is
  chosen by the invocation's type, never by its caller, and the runner is
  crate-private, so nothing outside `cairn-git` can run a raw verb. Twins
  against erosion, over the product crates' `src/` with test modules blanked
  (a test fixture may spawn what it likes):
  `every_git_invocation_disables_the_terminal_prompt` — no production file
  but `crates/cairn-git/src/process/environment.rs` names `Command` (so an
  alias is caught on its import line), builds one, calls an
  environment-setting method (`env`, `envs`, `env_clear`, `env_remove`),
  writes a `GitEnvironment { .. }` literal or opens an `impl` block for the
  type; that file builds exactly one `Command` and one `GitEnvironment`
  literal, calls both `env_clear` and `envs`, has no `&mut self` method, its
  `ALWAYS` table — the table itself, not the file — carries
  `("GIT_TERMINAL_PROMPT", "0")`, `("SSH_ASKPASS_REQUIRE", "force")`,
  `("GIT_EDITOR", "false")` and `("GIT_SEQUENCE_EDITOR", "false")`, its
  `READ_ONLY` table carries each of `READ_ONLY_PINS` —
  `("GIT_OPTIONAL_LOCKS", "0")` and `("GIT_NO_LAZY_FETCH", "1")` — and is
  applied, its `INHERITED` roster, read with its comments, carries each of
  `INHERITED_PINS` — staging-and-commit R5.2's nine: `GNUPGHOME`, `DISPLAY`,
  `WAYLAND_DISPLAY` and `XAUTHORITY`, so a signing pinentry reaches the desktop,
  and `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME`,
  `GIT_COMMITTER_EMAIL` and `EMAIL`, so a commit is by the terminal's identity —
  each with a comment of its own beside it (its reason), carries no `*_DATE`
  variable (`INHERITED_NEVER`, `GIT_AUTHOR_DATE` and `GIT_COMMITTER_DATE`: a
  stale one would stamp every commit) and is read, and
  its production code names `"GIT_ASKPASS"`, `"SSH_ASKPASS"`, the socket
  variable and the token variable (matcher self-test
  `the_process_environment_matcher_catches_the_shapes_it_claims`); and
  `only_the_process_module_builds_or_runs_a_process` — no product file
  outside `crates/cairn-git/src/process/` names `Stdio`, `Child` or its
  pipes, `CommandExt` or `nix`, calls `.spawn()`, `.output()`, `.status()`,
  `.wait()`, `.try_wait()` or `.wait_with_output()`, or calls
  `GitEnvironment::command`, which stays `pub(super)`; `process/` itself is
  required to show `.spawn()`, `.try_wait()`, `CommandExt`, `Stdio`,
  `Child`, `ChildStdout`, `ChildStderr`, `nix` and `.command(..)`, so the
  matcher is proven to read real code (`.output()` and `.wait()`, required
  while the retired runner used them, were swapped for `CommandExt` and
  `ChildStdout` when it went, and stay banned outside `process/`), and every
  roster entry has a self-test case spelled out
  apart from the roster (matcher self-test
  `the_process_matcher_catches_the_shapes_it_claims`; its one exception row,
  `history_state::Progress::status` in `cairn-app`, fails when no longer needed). The
  VALUE is pinned behaviourally in `cairn-git`: the builder's tests spell out
  the read and write variable sets in full, the stub tests in
  `process/cli.rs`, `ops/authority.rs` and `ops/fetch.rs` run a `git` that
  prints what it was given — the probe's, a read's, a write's and fetch's —
  and real `git` proves the effect: a `status` read leaves a stale index
  byte-identical where the same `status` as a write rewrites it, a read in a
  partial clone leaves a missing object missing where the same command as a
  write fetches it (`a_read_in_a_partial_clone_does_not_fetch_a_missing_object`,
  skipped below git 2.44), and a
  `commit` or `rebase -i` with a hanging configured editor fails promptly
  without running it. Residual review obligations: git older than 2.44
  ignores `GIT_NO_LAZY_FETCH`, so on such a git a read in a partial clone may
  still lazy-fetch — a pack written and the network reached; with no token
  only a promisor that needs a prompt fails closed, and one a configured
  credential helper or the ssh agent answers fetches — which no check can refuse while the
  floor is 2.30 (the user's decision); whether a new read could touch a
  missing object, and what it does on such a git, is
  `destructive-ops-reviewer`'s (its check 10);
  whether the inherited
  roster is RIGHT — each entry is a deliberate leak of the user's
  environment to git, and a missing one breaks a credential helper that
  worked — is `destructive-ops-reviewer`'s (its check 9); and the matchers
  read identifiers, so a `Command` reached through a `type` alias, a wrapper
  crate or a macro, and a process method reached through a trait object,
  are `qa-checklist`'s to catch (its item 7).
- **No credential value is logged, Debug-printed, serialised, or stored in
  application state.** The one type that holds a credential is
  `cairn_model::Secret`: no `Debug`, `Display`, `Clone` or serialisation, no
  derive at all, one accessor (`expose_secret`), and a `zeroize`-wrapped
  buffer so the drop clears memory with writes the compiler may not remove
  (credential-prompts L11). Primary enforcement is the type: `{:?}` and `{}`
  on it, a `#[derive(Debug)]` container of it and `.clone()` do not compile,
  pinned by the `compile_fail` doctests in `crates/cairn-model/src/secret.rs`
  (which is why the full gate has a `test-doc` step). Twin against what the
  compiler cannot refuse:
  `no_credential_value_is_logged_printed_serialised_or_stored`, with matcher
  self-test `the_credential_matcher_catches_the_shapes_it_claims`. What it
  decides, over every crate but `cairn-guards`: the type's file declares one
  `Zeroizing` field, derives nothing, has exactly the public functions and
  impl blocks the guard spells out (`new`, `from_string`, `expose_secret`,
  `len`, `is_empty`; the inherent impl, `Zeroize`, `ZeroizeOnDrop`), keeps
  its four compile-fail pins and their passing twin, and no other file opens
  an impl that names `Secret` (so no `Deref`, `From<Secret>`, `AsRef` route
  around the accessor); no `struct` or `enum` that holds a `Secret` —
  directly or through another such type, in `src/` or `tests/` — derives or
  hand-implements `Debug`, `Display`, `Clone`, `Copy`, `Serialize`,
  `Deserialize`, `Encode` or `Decode`; nothing renames `Secret` itself
  (`use .. as`, a `type` alias) — an alias of a type that HOLDS one is not
  followed, and is the review's; no `struct` outside the `SECRET_HOLDERS` roster (empty on
  purpose: a secret is passed by value and consumed once, never kept) has a
  field holding one or holding a type that does; and in production code
  `expose_secret` is named only in the `SECRET_READERS` roster (the type, the
  wire encoder that hands the bytes to the helper, the helper's `main` that
  hands them to git), each of which must actually read, and never — nor is
  any container type — inside a macro that renders its arguments (`format!`,
  `format_args!`, `panic!`, the assertions, `write!`, `dbg!`, the
  `tracing`/`log` event and span macros). Residual review obligations,
  `qa-checklist`'s: the matchers read spellings, so a generic wrapper
  instantiated with `Secret` at a use site rather than in a declaration, a
  hand-written `Debug` on such a wrapper, and — inside a `SECRET_READERS`
  file — the bytes hoisted into a local that is then rendered
  (`let b = s.expose_secret(); format!("{b:?}")`) are not seen; and whether a
  prompt's text, which IS rendered, could carry a secret (git puts the prompt
  on `argv`, so it never should) is a judgement, not a token.
- **Destructive operations take `cairn_model::Confirmed` by value, the token
  carries the `Consequence` the engine computed and the prompt rendered from it,
  and only a confirmation surface builds one.** Primary enforcement is the type:
  `Confirmed`'s two fields are private, it is neither `Clone` nor `Copy`, and its
  one constructor, `Confirmed::by_user`, takes a `Consequence` alone and renders
  the prompt from it (`Consequence::prompt`, beside the type in
  `crates/cairn-model/src/consequence.rs`), pinned by the `compile_fail` doctests
  in `crates/cairn-model/src/confirm.rs` — `.clone()`, a copy, the constructor
  given text, a literal, `Default` and a conversion from text each refused, each
  the passing scaffold plus one line. Twin against what the compiler cannot
  refuse: `destructive_operations_are_sealed_behind_the_confirmation_token`
  (matcher self-test `the_confirmation_seal_matchers_catch_the_shapes_they_claim`,
  matchers `function_signatures`, `takes_by_value`, `names_a_path_into`,
  `opens_an_impl_naming` and `impl_headers` in `crates/cairn-guards/src/lib.rs`),
  each part with a nonzero-files assertion: the token's file keeps exactly its two
  private fields, derives or implements none of `Clone`, `Copy`, `Default`,
  `Deserialize`, `Decode`, `From`, `TryFrom` or `FromStr`, opens one impl block
  (attributes ahead of an `impl` on its line stepped over), declares exactly the
  functions `pub fn by_user`, `pub fn consequence` and `pub fn prompt` — no other,
  private, `const` or restricted one — and no `const`, `static`, `macro_rules!` or
  `mod` item, names `by_user` once and builds the token once, and keeps its
  doctests; the `Consequence`'s file (`consequence.rs`) derives or implements none
  of `Default`, `Deserialize`, `Decode`, `From`, `TryFrom` or `FromStr` for it or a
  part, opens one impl block, `impl Consequence`, and renders the prompt there;
  the **destructive-operation roster** (`DESTRUCTIVE_OPERATIONS`, file and
  function) — every function in `crates/cairn-git/src`'s production code whose
  signature names `Confirmed` is in `ops/`, is on the roster or is the record that
  spends the token (`CONFIRMED_RECORD`: `destructive` in `Performed`'s inherent
  impl in `ops/mod.rs`, excused by that place and not by its name), and takes it
  as exactly one by-value parameter, never behind a reference, an `Option`, a
  bound or in the return; every row names a function that exists; no `pub`
  function in `ops/` takes a `Consequence` without the token; and no type in any
  crate's production code — the application's worker scan asserted — keeps a
  `Confirmed` in a field outside `CONFIRMED_HOLDERS` (a token held in a type
  reaches an operation behind a reference; its one row,
  `crates/cairn-app/src/worker/local_lane.rs`, is `LocalWrite`, which carries a
  destructive write's token to the local lane that spends it by value — a type
  that holds a `LocalWrite`, such as `Request` or the lane's job, is not
  followed, and is the review's — and a type may name the token only as the
  argument of a callback it holds, spelled exactly as one of `TOKEN_CALLBACKS`,
  `EventHandler<Confirmed>` and `dyn Fn(Confirmed)` (or `FnMut`, `FnOnce`), the
  confirmation dialog's handler and the window's continuation, which are handed a
  token to spend and keep none — ratified by the user on 2026-10-09; a callback
  that returns one or takes it beside other arguments still counts as a holder,
  and no production file declares a type, trait or alias named `EventHandler`,
  `Fn`, `FnMut` or `FnOnce` or imports another item under one of those names
  (`token_callback_shadow_violations`), so the trusted spellings mean the toolkit's
  handler and the language's closures; residual, `destructive-ops-reviewer`'s: a
  callback that stashes the token it is handed in a cell it captured keeps one
  unseen); and the roster is never empty, asserted (the user's decision,
  2026-10-08), its rows today `ops::discard_lines` and `ops::discard_files`
  (`crates/cairn-git/src/ops/discard.rs`), which replaced the placeholder,
  `ops::amend` (`crates/cairn-git/src/ops/commit.rs`) and `ops::create_branch_discarding`
  (`crates/cairn-git/src/ops/checkout.rs`, Create Branch's "Discard", the one operation that
  discards a staged change — staging-and-commit R3.6's stated exception);
  the **confirmation-surface roster** (`CONFIRMATION_SURFACES`, staging-and-commit
  R1.1: the confirmation dialog, `crates/cairn-ui/src/confirm_dialog.rs`, which
  draws the `Consequence` and builds the token from it on its button; and the
  commit box, `crates/cairn-ui/src/commit_box.rs`, whose `AmendButton` draws the
  amend's line and builds its token on the button or the commit chord — refusing
  an amend a remote has, which the dialog confirms — and whose `AmendSkip`
  draws a failed amend's prompt in the Git Error dialog and builds the skipped
  amend's token from the same consequence) — no production file of any crate
  but the guards, `confirm.rs` aside, names `by_user` unless it is on the roster,
  and every row does, so a helper "for tests" outside a `#[cfg(test)]` module, an
  alias's `T::by_user` and a stored `Confirmed::by_user` all fail, and the scan is
  required to have read `cairn-ui`'s and `cairn-app`'s `src/`; no production
  file outside `cairn-model` and `cairn-git` (`CONSEQUENCE_BUILDERS`) spells a
  path into a `Consequence` or names its parts (`DiscardedFile`, `FileLoss`,
  `Publication`, `Reflog`, `LostChange`, `ChangeLoss`, `ChangedKind`), so the render crates hold one and ask it for its words but never
  build one; and nowhere, `src/` or `tests/`, does a file but the type's own open
  an impl naming `Confirmed`, `Consequence` or its parts, or rename one (`use ..
  as`, or a `type` alias wherever it follows another item). Residual review
  obligations, `destructive-ops-reviewer`'s: whether a `Consequence` is
  computed rightly from the repository and re-checked against it before the
  operation runs (R1.4); that an operation derives every target — each path, each
  line — from `confirmed.consequence()`, never from a parameter beside it; whether
  a prompt that IS rendered from it is honest and sufficient; which operations are
  destructive at all (an operation left off the roster that takes no token is not
  seen); a roster surface building the token from anything but the `Consequence`
  it drew; and, since `Consequence` is `Clone`, a surface building two tokens from
  one acknowledgement. `qa-checklist`'s: the matchers read spellings, so a token
  reached through a macro, a closure in `ops/` taking one, and a public function
  in `cairn-model` or `cairn-git` that builds a `Consequence` from values its
  caller passes — through which a render crate could forge one without spelling a
  path into it — are not seen. Test code (test modules, `#[cfg(test)]` module
  files, `tests/`) may build tokens, since it cannot ship.
- **No `unsafe`, anywhere.** Twin: `unsafe_code = "forbid"` in the workspace lint
  table (compiler tier, so it cannot be locally overridden).
- **Shipping code never panics on a path a user can reach**: `unwrap`, `expect`,
  `todo!`, `unimplemented!` and `dbg!` are denied by the workspace clippy table,
  with tests exempted via `clippy.toml`. A panic in a git client can cost someone
  a working tree.
- **CI runs every merge-bar gate step, and the local full gate runs every step but
  the day loop's `test-fast`.** Twins: `ci_runs_every_merge_bar_gate_step`
  compares `gate.sh`'s dispatch arms against the workflow, so a step added locally
  cannot quietly skip CI; `the_local_full_gate_runs_every_step_but_the_day_loops`
  compares them against what `gate.sh` with no arguments calls, read by a matcher
  that refuses a conditional it does not know rather than guessing (self-test
  `the_gate_sequence_matcher_catches_the_shapes_it_claims`), with its exemptions
  an explicit roster (`LOCAL_FULL_GATE_EXEMPT`) that fails when a name in it stops
  being a step; and `the_full_gate_is_the_default_and_no_merge_bar_step_is_skipped`
  holds what that reading cannot see — `FAST=0` is the default, set once before the
  arguments are read, and no merge-bar step's `*_CMD` is the literal `skip` or empty
  (self-test `the_gate_command_readers_catch_the_shapes_they_claim`).
- **A test that skips where its host cannot serve it is required wherever the host
  can.** A passing test's stderr is hidden, so a skip reads `ok`; each such test
  fails instead of skipping when its `CAIRN_REQUIRE_*` variable is set, and
  `scripts/gate.sh`'s `test-full` probes the host exactly as the test checks it and
  sets the variable where the probe succeeds, saying so on the PASS line where it
  does not: the sshd fixture (`CAIRN_REQUIRE_SSH_FIXTURE`), git's builtin fsmonitor
  daemon (`CAIRN_REQUIRE_FSMONITOR_DAEMON`, cleared by `scripts/git-floor.sh` for the
  floors' gits, which have none), a mount namespace and a second owner
  (`CAIRN_REQUIRE_MOUNT_NAMESPACE`, `CAIRN_REQUIRE_SECOND_OWNER`), and a git that can
  make a reftable repository (`CAIRN_REQUIRE_REFTABLE`, whose test no floor run
  holds); CI's `gate` job
  sets `CAIRN_REQUIRE_SSH_FIXTURE` and `CAIRN_REQUIRE_NO_LAZY_FETCH` outright in its
  own `env:`. Twins: `the_ssh_criteria_are_required_wherever_they_can_run`,
  `the_partial_clone_pin_is_required_in_ci`,
  `the_fsmonitor_daemon_pin_is_required_wherever_it_can_run`,
  `the_user_namespace_tests_are_required_wherever_they_can_run` and
  `the_reftable_refusal_is_required_wherever_it_can_run`; the last three read
  their probes whole (`gate_function_body`) and require each call as a statement
  of `run_test_full`'s own (`gate_function_calls`, self-test
  `the_gate_function_call_matcher_catches_the_shapes_it_claims`), and the CI pins
  read the `gate` job's own `env:` block alone (self-test
  `the_workflow_env_matcher_reads_only_the_jobs_own_block`). Residual review
  obligations, `gate-integrity-reviewer`'s, until #58 closes them: the probe bodies
  are matched as text with their comment lines kept, so a commented-out `export`
  still satisfies a pin, and those body checks have no self-test; and `--step
  git-floor` run alone — CI's `git floor` job — runs no probe, so the
  across-filesystem test in its `diff_engine` run is never required there. Whether
  the runner's host serves a probe at all (GitHub's Ubuntu 24.04 images refuse the
  second owner) is said by the gate's note, not decided by it.
- **The UI thread never waits on repository work.** `cairn-app` is partitioned by
  FILE: `crates/cairn-app/src/worker/` runs repository work and may block; every
  other file in the crate renders, and may name neither `cairn_git`, `gix` nor
  `cairn_askpass` (whose `accept` blocks) nor any waiting primitive — the types (`Receiver`, `Mutex`, `Condvar`, `JoinHandle`),
  the channel constructors (`channel`, `unbounded`, ...), and the nullary waiting
  calls (`recv()`, `join()`, `lock()`, `wait()`), plus `sleep`, `park`,
  `block_on`. Naming the constructor is what catches a receiver held by
  inference. Twin: `the_ui_thread_never_waits_on_repository_work`, matching
  aliased imports and calls whose parentheses wrapped, ignoring string literals,
  and asserting a nonzero file count per directory on BOTH sides.

  **Residual obligations the guard structurally cannot express** — stated here
  rather than implied, and owned by `responsiveness-reviewer`: a file partition
  cannot decide which THREAD a function runs on, so the handful of `worker/`
  functions the UI thread itself calls (`RepositoryHandle::submit`, through the
  closure `RepositoryHandle::into_submitter` builds and through the window's
  close hook — `Closing::requested`, which lives in the render-side
  `crates/cairn-app/src/closing.rs` and is scanned, but calls `submit` — whose
  `Request::Close` arm stops the epochs with an atomic store, marks the local
  write lane closing under `LaneState`'s mutex and queues the close, whose query arms bump their lanes' atomic counters and send to the
  thread the routing table names over an unbounded channel, whose operation
  arms (`Request::Retire`, a replaced change set handed to the repository
  thread to free, among them) only send over one, whose `Request::Refresh`
  arm bumps two lanes' counters, reads a third's, and sends once to the
  repository thread and once to the refresh thread, whose `Request::OpenHistory`
  arm bumps the history and walk lanes' counters and sends once, and whose
  `CancelFetch` arm takes `FetchControl`'s mutex and calls
  `KillHandle::kill`; whose `Request::Write` arm bumps the amending lane's counter (ending an amend's read
  a stage would wait on) and boxes the write and sends it
  straight to the local write lane's thread, as its `Request::DiscardConsequence`
  arm bumps the discard-count lane's counter and sends that lane the paths a discard
  would take (its `Request::DiscardLinesConsequence` arm likewise, the diff and the lines), and its `Request::StopCounting` arm only bumps that counter, whose `Request::CommitReads` and
  `Request::Amending` arms bump the commit-box and amending lanes' counters and send the local lane one job
  (`Amending` the window's status, shared), and its `Request::StopAmending` arm only bumps the amending counter, whose `Request::CancelWrite` arm
  takes `LaneState`'s mutex (`worker/local_lane.rs`) and, when the write it
  names is a running commit, calls its cancel — a `KillHandle::kill`, through
  `ops::CommitCancel` — and whose `Request::Refresh` and `Request::RefreshStatus` arms
  first take that mutex to keep the refresh back while a commit runs, the
  latter then reading the status lane's counter and sending once to the
  refresh thread (staging-and-commit R4); `LaneState`'s mutex is otherwise
  held by worker threads only for an assignment and one unbounded send —
  the lane announcing a write's start and end, the refresh thread sending a
  status it read — for a commit's cancel installed as its `git` starts
  (`LaneState::install`), which calls that cancel under the lock when one
  came first — `KillHandle::kill`, an atomic mark, a `try_lock` and a
  `killpg`, never a wait — and for a read of whether a cancel came, which an
  amend's checks make once per object their walk reads (`BeforeRunning`); so
  the UI thread waits behind it no longer than one of those;
  `OperationId::next`, an atomic increment the window asks a write under
  (`local_writes::ask`), and `LocalWrite::what`, which formats a path or a
  count; the `Retired` constructors the window builds a
  retirement with (`Retired::of`, `Retired::history`, `Retired::refs`,
  `Retired::ahead_behind`, `Retired::status`, `Retired::sidebar`, in `worker/request.rs`), which
  only box what they are given; `worker::open`, called from `main.rs`'s `use_hook`,
  and the `Replier` closure it returns; `Updates::next`, `Wake::poll`, `Drop for Updates` (an atomic
  store, when the stream's task is dropped); and
  `Discovery::start`, which `main` calls on the main thread before the window
  exists and which only spawns the thread that runs `git --version` — all in
  `crates/cairn-app/src/worker/`) are exempt from the matcher while running
  on the UI thread, and that they never block is a review judgement. So is
  the close's shape: that the hook only asks (and calls what
  `Closing::when_requested` was given, which writes the window's
  `LocalWrites` state), that what waits for the reaps
  (`SharedRepository::end_invocations`, in `Threads::drop`) runs on the
  repository thread — after it has joined the local lane, which waits on the
  write it runs however long that takes (staging-and-commit R4.9) — that the
  window closes on the update stream's end, or
  on a second request after `worker::CLOSE_PATIENCE`, never by waiting, and
  that the stream's end still depends on the window refusing a prompt left
  open when the ended fetch's outcome arrives (`session::apply`'s
  `withdraw`); and `main.rs`'s wiring of it (`Closing::opened` given the
  handle, the hook installed, `Closing::is_requested` passed to
  `session::apply`, the window closed past the hook on the stream's end),
  which no test drives. (The spinning spellings — `try_recv`, `try_iter`,
  `try_lock`, `spin_loop`, `yield_now` — ARE on the roster, so a busy poll loop
  on a render path is caught; one written inside `worker/` is not.) The matcher is
  also FILE-scoped, which is what "naming the constructor" buys and all it buys:
  a receiver constructed inside `worker/` and handed OUT, then iterated on a
  render path — `for update in rx {}`, `rx.into_iter()`, or a blocking method
  with a project-specific name — names no rostered spelling and is not caught.
  `crates/cairn-app/src/main.rs` holds exactly such a value today; that it is
  awaited rather than iterated is a review judgement, not a guarded fact. So is
  `cairn-ui`'s edge auto-scroll (`src/edge_scroll.rs`): a task spawned on the
  toolkit's executor that awaits `async_io::Timer` each tick and scrolls a list
  one step, never a sleep, ending when the drag ends or the pointer leaves the
  edge — the guard's partition covers `cairn-app` alone; and so is the commit box's
  elapsed time (`src/commit_box.rs`'s `Elapsed`): a task of its line's scope that awaits
  `async_io::Timer` each second and bumps a counter so the line is drawn again, ending
  when the line unmounts with the commit's ending. Also
  the reviewer's: whether a page is small enough that the work between yields is
  short, and whether a list is virtualized.

- **No component names a literal modifier** (decision D5, PRD R8.3). Every
  keyboard shortcut is an `Action` mapped to a list of distinct chords per
  platform (`accelerators::chords`, staging-and-commit L22: Fork keeps alternates,
  stage on Return and ⌘S, discard on Backspace, Delete and Ctrl+Shift+D), and to
  the scope it is heard in, in the accelerator table,
  `crates/cairn-ui/src/accelerators.rs`; a component asks
  `accelerators::resolve_key` which action a key press is (or `is_chord`, to leave
  one alone) and never reads the held keys itself, so a `Ctrl` that is wrong on
  macOS cannot be written into a component. **No chord is listed twice on a
  platform — within one action's list or across two actions, whatever their
  scopes — and every bare chord is a function key's, but for Enter, Backspace and
  Delete in Local Changes' list-and-diff scope (`Scope::LocalChanges`) alone**, the
  one scope no text field is in. Twin: the table's pin
  `chords_are_distinct_and_every_bare_one_is_a_function_key_or_local_changes_own`,
  which checks the table and then shows the rule failing on the shapes it refuses
  (a bare Enter, Backspace or Delete in every other scope, a bare letter in Local
  Changes, a bare press, a chord listed twice for one action, one shared by two
  actions); that the table answers a list (`pub fn chords(..) -> Chords`, no single
  `chord`) and that the pin stays a plain `#[test]` are held by
  `the_accelerator_table_holds_data_and_resolution_only`, which also holds the pin
  where it runs (`pin_placement_violations`, self-test
  `the_pin_placement_check_catches_the_shapes_it_claims`: declared once, directly
  in the one module whose attributes are exactly `#[cfg(test)]`, its own exactly
  `#[test]`, its body still showing the rule failing — the same check holds R4.8's
  `the_bounded_output_helpers_exist_on_a_read_alone`). A view leaves alone only the
  chords of the window, the detail pane and the scopes it is in
  (`accelerators::is_chord(event, own)`), so another view's chord — Local Changes'
  Shift+↓ — is its arrow. **Every text field takes the one key policy**
  (staging-and-commit R7.1; ratified by the user on 2026-10-09): Freya's `Input` claims every key
  but four, which cancels the window's global key event, so a focused field hid the
  window's chords and the held modifiers a press is resolved against; every field
  is built by `cairn_ui::text_field` or `text_field_in`
  (`crates/cairn-ui/src/text_field.rs`), whose pre-key handler asks the table
  (`accelerators::field_key`) and hands a window chord and a lone modifier key to
  the window untyped, types nothing for a primary+letter press that is no editing
  binding, claims its own scope's chord (the commit box's commit) without a new
  line, and keeps every other key from the views around it; a field's own scope is a
  `FieldScope` — the commit box alone — none with a bare chord
  (`a_fields_own_scope_holds_no_bare_chord`). Twin:
  `every_text_field_takes_the_shared_key_policy` — no file of `crates/cairn-ui/src`
  or `crates/cairn-app/src` but that one names, in production code, any of
  `TEXT_FIELD_IDENTS`: `Input`, `on_pre_key_down` and every other way the toolkit
  edits text (`use_editable`, `UseEditable`, `EditableConfig`, `text_edit`,
  `SelectableText`, `CodeEditor`), save the rows of `TEXT_FIELD_EXCEPTIONS` (the
  accelerator table's `text_edit`, for the editor's own bindings), each required to
  still match; and that one must name `Input` and `on_pre_key_down` (matcher
  `builds_a_text_field`, self-test
  `the_text_field_matcher_catches_the_shapes_it_claims`). Twin of the
  modifier rule:
  `no_component_names_a_literal_modifier`, over every file of `crates/cairn-ui/src`
  and `crates/cairn-app/src` (test modules blanked) but the table, which must exist
  and in which the matcher must find a modifier (so a blind matcher fails); matcher
  `names_a_literal_modifier` in `crates/cairn-guards/src/lib.rs`, whose rosters are
  read from the vendored `keyboard-types` (the version `Cargo.lock` pins) and
  Freya's `ModifiersExt`: the type `Modifiers`, the trait and its
  `ctrl_or_meta`/`ctrl_or_alt`, the event's `modifiers` field, the type's
  constants (`CONTROL`, `META`, ..., and the locks `CAPS_LOCK`, `NUM_LOCK`,
  `SCROLL_LOCK`, `FN_LOCK`, `SYMBOL_LOCK`), the modifier and lock keys of
  `NamedKey` and `Code` (`Control`, `ShiftLeft`, `CapsLock`, ..., and `::Fn`),
  the nullary predicates `.ctrl()`/`.alt()`/`.shift()`/`.meta()`, and a string
  or char literal spelling a chord for a person once its `\u{..}` and `\x..`
  escapes are read (`spells_a_chord`: `Ctrl`, `Cmd`, `⌘`, `⌥`, `Shift+`,
  `Shift-`, `Command+`, `Opt+`, ...). Matcher self-test:
  `the_modifier_matcher_catches_the_shapes_it_claims` — an aliased import of the
  type, a qualified path, a constant defined beside a component, a glob of the
  key type's variants, a predicate reached by inference, `'⌘'`, an escaped
  `⌘`, each hyphenated spelling. **The table itself holds data and the
  resolution of a press, nothing a person reads**: an element built there or a
  chord spelled out in a literal would be a view inside the one file the
  modifier guard exempts. Twin: `the_accelerator_table_holds_data_and_resolution_only`
  — the table's production code names none of `ELEMENT_BUILDERS` (`rect`,
  `label`, `Element`, `Component`, `Button`, ...; matcher `names_an_element`,
  self-test `the_element_matcher_catches_the_shapes_it_claims`) and no literal of
  it spells a chord. **One module spells a chord for a person, from the table's data**
  (the user's decision, 2026-10-09): `crates/cairn-ui/src/accelerators/chord_names.rs`
  (`accelerators::chord_name`, a tooltip's text — Linux `Ctrl+Shift+.`, macOS `⌘⇧.`, as
  Fork's own lists write them) renders an action's first chord from the table, and is the
  one row of `CHORD_NAMES` that `no_component_names_a_literal_modifier` exempts: exactly that
  path, the row failing when its file spells no chord or is not read, each literal there one
  held key's name alone (`MODIFIER_NAMES`; matcher `hand_typed_chords` — a chord typed out
  whole fails), and no element built there (self-test
  `the_chord_name_roster_and_its_matcher_catch_the_shapes_they_claim`). Residual review
  obligations, `qa-checklist`'s (its item
  11): a name put together there from single-key literals rather than from the chord the
  table lists is not seen; the matchers read spellings, so a modifier reached through a `type` alias
  declared outside the render crates, a macro, or a raw bit pattern compared
  without naming the type is not seen, nor is an element the table builds
  through a helper named otherwise; the table's
  public surface must keep speaking actions and chords, never a modifier or a
  "held" predicate a component could branch on under another name — the
  exceptions, `Chord::key_press` and `Chord::press_hold`, hand a chord's keys to
  headless tests, and a render path calling either is a finding; `HeldKeys`, the
  keys the window hears held so a pointer press can be resolved (this toolkit
  build carries no modifiers on one), answers an `Action` and never which key is
  down; a key event
  handled by a test in `crates/cairn-ui/tests/` is not scanned; whether each
  chord is right for its platform — clear of the desktop's and of Fork's — is a
  judgement the table's tests do not make; and the text-field matcher reads the
  name `Input`, so a field reached through a wrapper crate or a macro is not
  seen.
- **No unbounded list renders without virtualization.** A history is however long
  somebody's repository is, so a view that builds one element per row of it is
  unbounded work per frame. Twin:
  `a_history_sized_list_renders_through_a_virtualizing_view`. What it decides,
  stated at the strength it actually holds: **no file on a render path may name
  `ScrollView`** — the view that lays out every child — except through an
  explicit exceptions roster that is empty today, and **some render file must use
  `VirtualScrollView` over `HistoryRow`s**. So swapping the list for the
  unbounded view, adding a second unbounded one anywhere, and deleting the
  virtualized one all fail; the roster is what turns a bounded panel's
  legitimate `ScrollView` into a review rather than a silent precedent.

  **Residual obligations the guard structurally cannot express**, stated rather
  than implied and owned by `responsiveness-reviewer` (whose dispatch row in
  `docs/qa-gate.md` names this twin):

  - *Whether a given iteration is over a history at all.* Tokens cannot tell an
    iteration over a repository's commits from one over three tabs, so the guard
    does not pretend to: it checks which VIEW a file reaches for, not what is put
    in it. A hand-rolled viewport that never names either view is the reviewer's
    to catch, and so is a `VirtualScrollView` handed a TRUNCATED length — the
    positive arm decides that the virtualizing view and `HistoryRow` meet in one
    production file, not that it is given the whole history.
  - *Which unbounded views an exception excuses.* The roster is keyed by FILE
    and the matcher reports only the first hit, so excusing one file excuses
    every plain `ScrollView` in it, then and later. Empty today; if a row is
    ever added, reviewing what else that file grows is the reviewer's.
  - *Whether work bounded by the VIEWPORT is bounded by the history anyway.*
    `cairn_ui::HistoryList`'s `index_of` keeps a cursor hint and falls back to
    `History::position` when it misses — a scan of every loaded row's id,
    inside the key handler, on the UI thread. It is the correctness fallback by
    design, and it is reached. The hint is the window's (`View::history_cursor`,
    handed to the list by `HistoryList::cursor`): an arrow key and a plain press move
    it, and so do the rows chosen outside the list — a parent link's
    (`detail_pane.rs`'s `follow_parent`) and a ref pressed in the sidebar, found loaded
    or by a find (`ref_find.rs`'s `bring_into_view`) — so their next arrow key starts
    there (`the_list_moves_from_the_row_its_callers_hint_names`). It still misses after a
    compared pair is let go (`selection::extend`, which selects the other row without
    knowing its index) and after a reopen keeps the selection by id while the rows
    under the hint changed: the next arrow key then scans the loaded ids, from the top
    to the selected row — once per key press, never per frame. A row arriving ABOVE another
    would enter it too; no row kind does today — a stash's row is laid out in the
    walk's stream and appended like any other — and there is to be no
    working-tree row (`docs/design/history-graph.md`). Named
    here rather than left implicit, because a token scan cannot tell this
    iteration from any other. Its sibling: a Commit-tab parent link finds its
    parent with `selection::loaded_row` (`History::position`), a scan of every
    loaded row's id, once per
    press on the UI thread, and so does a second commit pressed with the
    extending chord (`selection::extend`, to find which of the two rows is lower)
    — and the Commit tab builds its header (proportional
    to the commit's message, never to its files) once per commit, cached on its id.
    Files opened in place in the Commit tab are placed by a binary search over a
    table of their row counts (`cairn_ui::Expansion`), rebuilt on the UI thread
    from the first file a change touched — a page of Expand All appended costs the
    page, but opening or closing an early file re-places every file open after it,
    which Expand All's line budget bounds.
    A file's diff is prepared once, on the diff thread that answered it
    (`cairn_model::ShownDiff::new`, in `worker/diff_lane.rs`; the window only keeps
    the value, `DiffState::file_arrived`, and each file of a page of files opened in
    place alike, `DiffState::expansion_arrived`): both rows' indexes, proportional to its
    changes, and one pass over its drawn bytes for the widest line — a line past
    the long-line limit counted to its cut — bounded by R2.6's ceilings by default,
    and by the 64 MiB load-anyway ceiling for a file loaded past them. The Changes
    tab's filter is a pass over every path of a change set, run on the repository
    thread in a lane of its own (`Request::FilterFiles`), never on the UI thread;
    the window keeps the indices it answers (`file_filter.rs`), and the list reads
    the chosen file's index from `DiffState` rather than searching the change set
    for it; the sidebar's rows over a refs snapshot likewise — the filter's matches, its
    folders and what is open laid out on the repository thread (`Request::FilterRefs`,
    `RefsSnapshot::sidebar_rows`, the ref-filter lane), the window keeping the rows it
    answers (`sidebar_state.rs`) and reading each drawn row's ref by index; and Local
    Changes' lists — a status laid out as its two lists, in natural order, on the refresh
    thread as it is read (`LocalChanges::new`, `worker/refresh_lane.rs`), its filter a pass on
    the repository thread in a lane of its own (`Request::FilterLocalChanges`), the window
    keeping the lists and the rows answered (`local_changes_state.rs`): a row is read by index
    as it is built, and the path chosen is found in the lists by a binary search
    (`LocalChanges::row_of`) on every render of the view and each time other lists are drawn
    (`local_changes_pane::follow_the_lists`) — proportional to the logarithm of the paths,
    never to the paths. A status is held twice, by the refresh's answers and by the lists
    drawn (which may be the older while a filter's rows for the newer are on their way), and
    each holder hands its hold to the repository thread to free when it lets go, so the last
    is never dropped on the UI thread. Local Changes' actions
    (`local_changes_actions.rs`) work on the UI thread in proportion to what the person
    selected, never to the lists: an action finds each selected path by a binary search
    (`LocalChanges::row_of`), a range press clones the paths of the rows it spans — and
    Shift+↑/↓ spans them afresh on every key and key repeat — a ⌘- or Ctrl-press clones the
    selection twice and inserts into it (a shift of the paths after it), the row the selection
    moves to after a stage is found by stepping past the acted rows, and the menu asks of each
    selected path whether it is a submodule or a conflict (`cairn_ui::no_discard`) — so with
    fifty thousand rows selected each such press, key repeat or action costs some 5-15 ms, once
    per press and never per frame; Stage All and Unstage All gather their
    paths on the local lane, never here. Off the UI thread but in its way: what a discard would
    lose is counted per path on the local lane (`ops::discard_files_consequence`: a gix index
    read, two reads of each file and a `git diff-files` per tracked path), so it holds that
    lane — and a close, which waits on the lane — for as long as the selection is wide; it is
    numbered in the discard-count lane and ends, between paths or by ending its `git` read,
    on a newer ask, on Local Changes being let go of (`Request::StopCounting`) and on a close.
    So does what the commit box asks of the lane: its reads (`Request::CommitReads`: a `git
    rev-parse --git-path hooks` and a ten-commit walk) on every refresh's refs while Local
    Changes is shown, and — while Amend is ticked — an amend's read over every status that
    arrives (`Request::Amending`: the pushed check's walk, `ops::amend_consequence`, and a `git
    diff-index --cached` against `HEAD`'s parent), each numbered in a lane of its own and ended
    by a newer ask, the amend's also by Amend unticked and by any write asked (phase 11
    measures both, and splits the amend's read). The diff's
    staging gesture (`cairn_ui::staging_gesture`) draws one layer over the rows — an
    outline, a tint and at most three actions — re-rendered on each pointer move over the
    diff and each scroll, its chunk found by a binary search of the hunks and, for files
    drawn together, of the files; a drag's selection is read from the layout once, when it
    ends (`selection_in`, costing the changed lines selected), and copied once per action,
    never per frame — the layer reads it in place. The files drawn together cost a pass over
    the files per render of their view (`StackedDiff`'s widest row) — every frame while the
    window is resized sideways — and what the selection asks a binary search per path
    selected (`local_changes_pane::together_wanted`), run as the selection, the lists or the
    settings move; and each page of them that arrives rebuilds their `Expansion` table from
    the page's first file to the last (`DiffState::together_arrived`), so reading them costs
    the paths times the pages, bounded by the selection a list holds, on the UI thread
    (a residual for `window_check` to measure). A press in
    the sidebar looks for its row among the labelled, stash and `HEAD` rows alone
    (`History::labelled_position`), never every loaded row, and a find then looks
    through each page as it arrives (`ref_find::pages_arrived`), never a page twice. A refresh asks the UI thread for
    a submit and nothing else — on focus gained (`refresh::on_focus_gained`, a side
    effect on the toolkit's focus state), after a fetch ends, on the Refresh chord —
    and a refresh's answers are kept by moving them in (`RefreshState`), the one
    each replaces handed to the repository thread to free (`Request::Retire`). A
    reopen (`session::apply`'s `reopen_history`) moves the loaded `History` out,
    builds an empty one whose author index is allocated for the old one's author
    count (`History::with_author_capacity`: one allocation, proportional to the
    history's distinct authors, so its first pages do not rehash through every
    doubling), and hands the old one to the repository thread to free, every row of
    it (#52) — the selection kept as an id, never searched for. A history row is read through the history's stores as the list builds
    it (`render_of`, in `HistoryList`'s `build_row`): its subject and author copied
    out, and its edges derived — one lane snapshot copied and advanced through at
    most `LaneAssigner::SNAPSHOT_EVERY - 1` rows of lane changes per drawn row,
    bounded by the snapshot interval, never by the history (measured in
    `docs/systems/history-graph.md`, "What a row keeps"). Its chips are laid out there
    too, once per row built (`ref_chips::row_chips`): the current branch's label and each
    upstream found by a search, the labels read in order only until the column's room is
    spent, so a commit with thousands of refs costs a column's worth
    (`only_a_viewport_of_labelled_rows_is_built_and_each_lays_out_a_columns_worth_of_chips`,
    `a_rows_chips_are_laid_out_once_per_row_built_and_never_per_frame`); and a row's lines
    paint each place once, however many lanes share the last column
    (`graph_geometry::row_geometry`), though deriving them still walks every line crossing
    the row. The Commit tab's REFS row finds the selected row once per history and
    selection (`crates/cairn-app/src/row_finder.rs`): one pass over the loaded rows' ids,
    spread over the pages as they arrive and begun again only for a reopened history
    (`History::serial`), on the UI thread; its chips are laid out for the window's width
    on every render of the tab's body (a page arriving, a frame of a resize), bounded by
    that width, and only the header built from them is cached. While the window is
    resized sideways the list's room changes every frame, so every visible row is built
    again each frame — edges derived, chips laid out — bounded by the viewport's rows and
    the lanes crossing them. A page of rows is
    appended to the history where pages were applied before (`session::apply`,
    `History::append`): a copy proportional to the page, plus, at each doubling
    of the author index (a standard hash map, keyed by a fixed-key SipHash of the
    name), a rehash of every author so far — bounded by the history's distinct
    authors, not its rows (0.48 ms measured at 57,000 authors).
    One free still happens on the UI thread rather than through
    `Request::Retire`: `Updates::next`
    (`crates/cairn-app/src/worker/pool.rs`) drops each superseded answer that
    `Update::into_retired` does not retire — a page of rows, a filter's index
    list, a failure — each bounded by a page or by the change set's file list or
    the refs snapshot's ref list. (`session::reload_if`, which freed the whole
    history on the UI thread when a fetch moved refs, is gone: a reopen retires it.)

  Whether the virtualizing view really builds only what its viewport shows is
  pinned by a second, behavioural twin:
  `only_a_viewport_of_rows_is_built_however_long_the_history`
  (`crates/cairn-ui/tests/history_list.rs`) renders `HistoryList` headlessly over
  1,000 and 100,000 rows and requires one viewport's worth of rows, the same at
  the top and scrolled deep at both lengths, and so does its labelled twin
  `only_a_viewport_of_labelled_rows_is_built_and_each_lays_out_a_columns_worth_of_chips`
  (100,000 rows, forty labelled by 5,003 refs each, no row laying out more chips than
  its column holds); its twins for the Commit tab's files,
  `only_a_viewport_of_files_is_built_however_many_the_commit_touched`, and for the
  Commit tab with files opened in place under their rows — the longest list in the
  application — `only_a_viewport_of_rows_is_built_however_many_files_are_open`
  (`crates/cairn-ui/tests/commit_tab_expansion.rs`, three files of 10,000 lines open
  among 55,184, unified and side by side, top, deep and end), and for a
  file's diff rows, `only_a_viewport_of_diff_rows_is_built_however_long_the_file`
  (`crates/cairn-ui/tests/diff_view.rs`, which also scrolls to the end and requires
  the projection's last row there) and its side-by-side twin
  `only_a_viewport_of_side_by_side_rows_is_built_however_long_the_file`, and for the
  Changes tab's files, `a_list_of_55184_files_builds_one_viewport_filtered_or_not`
  (`crates/cairn-ui/tests/changes_list.rs`), and for the sidebar's refs,
  `a_sidebar_of_50000_refs_builds_one_viewport` (every folder open; top, deep and end) and
  `a_folder_of_10000_branches_open_builds_one_viewport` (folders over a flat list stay
  virtualized; `crates/cairn-ui/tests/sidebar.rs`), and for Local Changes' two lists,
  `a_status_of_50000_paths_builds_one_viewport_filtered_or_not` (50,000 rows in Unstaged and
  25,000 in Staged, top, deep and end, filtered and not; `crates/cairn-ui/tests/local_changes.rs`),
  and for a failed commit's output in the Git Error dialog,
  `the_git_error_draws_one_viewport_of_output_and_the_skip_only_where_offered` (100,000 lines,
  opened at its end; `crates/cairn-ui/tests/commit_box.rs`), the window keeping at most its
  latest 10,000 lines or 1 MiB (`commit_box_state::OutputTail`),
  hold the other lists to the same, and so do the staging gesture's twins
  (staging-and-commit C19, `crates/cairn-ui/tests/staging_gesture.rs`):
  `the_gesture_builds_one_viewport_over_a_10000_line_diff` (a 10,000-line diff with the
  gesture hovered and a selection made, top, deep and end, the layer drawing at most its
  two actions) and `files_drawn_together_build_one_viewport` (fifty files of 10,000 lines
  drawn together, top, deep and end), with
  `a_drag_across_unmounted_rows_selects_every_line_between_its_ends` holding a drag whole
  across the rows it unmounts. They count
  rows built, not work done: whether per-frame work grows with scroll depth while
  that count stays flat stays `responsiveness-reviewer`'s.

## Conventions

- Rust 2024 edition, toolchain pinned in `rust-toolchain.toml`. `cargo fmt` with
  the repo `rustfmt.toml` (100 columns). Clippy at `-D warnings` over
  `--workspace --all-targets --all-features`; the workspace lint table in the root
  `Cargo.toml` is the single place lint levels are set.
- Tiny dependency set. Adding a dependency is a user decision.
- Conventional Commits with a scope AND a body (1-4 sentences of why, not what).
  Scopes track the crates: `model`, `git`, `ui`, `app`, `guards`, `gate`, `docs`.
- **No commit message, pull-request title or description, or GitHub comment
  carries a Claude Code session link** (the `claude.ai` session URL or a
  `Claude-Session:` trailer), whatever an environment's attribution guidance
  says. The repository is public, and a link published in history cannot be
  taken back. Twin: `.githooks/commit-msg` refuses such a message, and
  `.githooks/pre-push` runs the same check over every outgoing commit, so a
  commit made with `--no-verify` is still stopped before it leaves the machine;
  pinned by `crates/cairn-guards/tests/session_link_hook.rs`. Residual review
  obligation: no local hook sees a pull request's text, and GitHub fills a new
  pull request's description from the first commit's message — whoever opens
  one reads its description before submitting.
- Name modules for behavior, never for layer: no `helpers`, no `utils`, no `misc`.
  `cairn-git/src/repository.rs`, not `cairn-git/src/core.rs`.
- Errors are `thiserror` enums whose variants name what the CALLER must handle;
  never re-export a dependency's error type across the seam.
- Keyboard shortcuts resolve through one accelerator table mapping a logical
  action to a list of per-platform chords (`crates/cairn-ui/src/accelerators.rs`). Never a
  literal `Ctrl` inside a component — it is the cheap half of keeping macOS
  reachable (decision D5), and an invariant with a guard (above).
- Docs follow the anchor rule: cite stable paths, exported symbols, and pinned
  tests; never literal counts or line numbers that rot.

## Modularity

Module-first is the default for ALL new code. The deciding question: does this code
need the coordinator's private mutable state? If no, it is a sibling module, every
time. Entry-point files are firewalls that assemble modules, not homes.

- Extract on the rule of three, not before. Never abstract for one use or a
  hypothetical future need.
- Data-as-code is exempt from size pressure: content tables are correctly big.
- Fix bugs test-first: reproduce with a failing test on the real code path, then
  the smallest change that turns it green. Detail: the `extract-and-test` skill.
- Optional but recommended once files grow: a file line-count ratchet — every
  walked file gets a pinned ceiling, growth past it fails, extraction lowers the
  ceiling in the same change, and a raise is a reviewed user decision.

## Testing & verification

Layers, cheapest boundary first (full contract: `docs/qa-gate.md`):

1. Stop hook: instant debris scan every turn (`.claude/hooks/qa-stop.sh`).
2. `.githooks/commit-msg` on every commit, then `.githooks/pre-push`: the
   session-link check over outgoing commits, format check, `cargo check`, guard
   suite.
3. `scripts/gate.sh --fast` while iterating; `scripts/gate.sh` is the merge bar.
4. CI (`.github/workflows/ci.yml`): the same checks as named `scripts/gate.sh
   --step` invocations on every PR and every push to `main` or a `feature/**`
   branch — `git-floor` in a job of its own — so CI runs what the local full gate
   runs (`ci_runs_every_merge_bar_gate_step` one way,
   `the_local_full_gate_runs_every_step_but_the_day_loops` the other).
5. `/qa` at end of contribution: dispatches the `qa-checklist` agent plus the
   domain reviewers matching the diff surface. Spawn reviewers FRESH; never have
   the implementer review its own work.

Engine tests run against real repositories, not mocks: `cairn-git` opens the Cairn
checkout itself in its unit tests, and fixture repositories are built by running
real git operations. A fake object database proves nothing about gitoxide.
Component tests use `freya-testing`'s headless runner (a dev-dependency, from the
same fork and rev as `freya`): `crates/cairn-ui/tests/` for components, and
`crates/cairn-app/src/window.rs` for the window drawn from each view state.

## Working style by model capability

- Baseline tier: small verifiable steps, checkpoint with the user, one
  investigation subagent at a time.
- Frontier tier: work autonomously end to end; front-load the spec; fan out
  parallel subagents across independent files or subsystems (these models
  under-spawn by default); before declaring done, have a FRESH subagent review the
  diff for COVERAGE (report every gap with confidence and severity), not filtering.
- State rule scope literally: models follow instructions literally and will not
  generalize a rule across cases unless told; say "every" or "all" when you mean it.
- Never gate the Invariants, safety, or correctness on which model you are. Anchor
  every autonomous step on a check you can actually run, never on "looks done."

## Pointers

- `docs/design/cairn.md` — the design spine: what Cairn is for, what it is not,
  the first milestone, and the map to one design doc per feature (`engine.md`,
  `credentials.md`, `concurrency.md`, `processes.md`, `history-graph.md`, `diff.md`,
  `conflicts.md`, `worktrees.md`, `forge-links.md`, `platform.md`), with the
  index of decisions D1-D9 that the architecture above implements.
- `docs/design/feature-inventory.md` — the full feature surface, tiered by risk,
  with the out-of-scope list and its reasons. Intent, not as-built.
- `docs/design/ui.md` — the UI design: Fork's layout model kept, every deviation
  named with the doc that drives it; mockups in `docs/design/mockups/cairn-ui.html`.
- `docs/work/daily-loop/roadmap.md` — the build order to the D7 milestone and
  each packet's status.
- `docs/qa-gate.md` — the QA layer contract and reviewer dispatch table.
- `docs/CLAUDE.md` — the docs layer contract (tenses, promotion, teardown).
- `docs/work/<packet>/` — in-flight packet dirs, created by `/feature-plan`, torn
  down when the work merges.
- `docs/systems/` — as-built descriptions, written when a system exists.
  `history-graph.md`: how the history view reads, lays out and draws a
  repository today, with the twin that pins each rule. `credentials.md`: the
  askpass helper and its channel, fetch end to end, and the decisions the
  packet locked. `git-processes.md`: where every `git` process is built, the
  read/write seal, the environment each kind of invocation runs with, the
  runner — its pipes, how an invocation is cancelled, and what it reports —
  each repository's registry and command log, and, in the application, `git`
  found once, the network lane and its refusal, and closing.
  `refs.md`: the refs snapshot, the five rules that make gix's refs git's,
  upstreams, the stash list, ahead and behind, and the ref storages refused at
  open. `status.md`: the working tree's status — what `git status` lists, the
  read that asks it, and the oracles that check it. `local-changes.md`: the Local
  Changes view — its two lists laid out over a status on a worker, the filter, a
  path chosen and its working-tree diff drawn for that path alone, how the
  path chosen follows each refresh, its actions, the staging gesture and the commit box. `staging.md`: the engine's write verbs —
  staging, unstaging and discarding lines and files, their arguments, the stale
  check before every patch, and the discards sealed behind a confirmation and
  re-checked before they run. `diff.md`: how a change to a file is described — one exact answer, its hunk,
  row and patch projections, the reference applier that checks the emitter,
  and the engine queries that fill it from a repository; the diff thread and
  its lanes; the detail pane, its Commit and Changes tabs, files opened in
  place and two commits compared; the diff view, with what was measured from
  Fork and what Cairn chose; and the accelerator table's contract.
- `docs/research/<slug>/` — the evidence behind decisions, kept after teardown;
  `docs/research/diff-engine/c14-measured.md` is Cairn's measured diff and
  window numbers on the bench repository, against git's own in
  `measured-baseline.md` beside it, and `docs/research/refs-and-status/measured.md`
  its refs, status, retained-row and window numbers on the same repository.
