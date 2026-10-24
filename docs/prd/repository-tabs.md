---
status: in-flight
packet: repository-tabs
opened: 2026-10-24
---

# PRD — Repository tabs

Several repositories open in one window, one tab each, behind Fork's tab strip: opened from the
command line, File › Open Repository… and the Repository Manager tab's Recent list; closed and
restored as Fork's are; switched by the strip and Ctrl+Tab; and remembered across launches.
Underneath, the window's state is split in two — what the window owns and what each tab owns —
and every repository keeps its own worker, its own update stream and its own close.

The decisions and their rejected alternatives are in `docs/work/repository-tabs/brainstorm.md`
(T1-T12). The evidence is in `docs/research/repository-tabs/`, cited here by short names:
- FT: `fork-tabs-evidence.md`, what the Fork surveys recorded of its tabs, graded, with the
  questions no evidence answers (its §17);
- WS: `window-state-audit.md`, the window's state, per repository and per window;
- WG: `worker-and-guards-audit.md`, the worker, the engine's process-wide pieces and the guards;
- FR: `freya-tab-strip.md`, Freya's parts for a strip, state, tasks, pickers and file drops.

The design is `docs/design/repository-tabs.md`, written by this packet's planning to describe the
end state; the chrome around it is `docs/design/ui.md`.

**Depends on:** `fork-alignment` (the daily-loop roadmap's 5d) merged to `main`: the caption row,
the menu component, the toolbar and its status box, and the window regions this packet puts a strip
between. Every file and symbol named below is as it stands at `feature/fork-alignment` (b539d9d).

## What this packet delivers

- The window's state split into a window part and a tab part, and every repository opened,
  served and closed by its tab.
- Fork's tab strip: names told apart, the mark of uncommitted changes, ×, "+", switching,
  reordering and an overflow list.
- The Repository Manager tab, with Recent and Open.
- Open Repository…, Close Tab, Restore Closed Tab, and the window's close across every tab.
- A credential prompt that comes forward from any tab and names its repository; every other
  dialog waiting on its own tab.
- The open tabs and the Recent list remembered across launches.

## Markers

A requirement marked **[observe]** is Cairn's default where Fork's behaviour is unrecorded (FT
§17), and every requirement one of R1.1's questions bears on is settled the same way, marked or
not. R1's observation runs before anything it covers is drawn. Where Fork does otherwise, the
phase that observed it amends that requirement and its criterion to Fork's behaviour, or keeps the
default as a deviation row in `docs/design/ui.md`. Either way the user approves the change before
it is built, and the amendment is logged in `docs/work/repository-tabs/progress.md`.

## Requirements

### R1 — Fork observed first (T4)

1. Before any strip, Manager or menu item is drawn, `/observe-fork` answers FT §17's questions
   1-19, 22-24, 26-39, 41-46 and 48, on the machine with the Windows VM. Each answer is
   VERIFIED with a screenshot or NOT OBSERVED, in
   `docs/research/repository-tabs/fork-observed-<date>.md`.
2. Each [observe] requirement is then settled as the Markers section says, before the phase that
   builds it starts.

### R2 — The window's state in two (T3)

1. The window holds one `WindowView` and one tab state per tab (`TabView`); every field of today's
   `View` (WS §2) goes to exactly one of them:

   | `WindowView` | `TabView` |
   | --- | --- |
   | `held_keys`, `zone`, `frame` (zoom, About, the `git` found) | the history: `rows`, `progress`, `selected`, `pair`, `history_scroll`, `history_cursor` |
   | `pane_collapsed`, `pane_split`, `changes_list_width` | `detail_tab`, `diff`, `diff_scroll`, `change_cursor`, `filter_text` |
   | `diff_settings`' toggles and the context chosen (R2.3) | the context its repository's `diff.context` seeds (R2.3) |
   | | `fetch`, `remotes`, `refreshed`, `lock`, `writes`, `endings`, `confirming`, `prompt` (R9) |
   | the sidebar's `width` and `shown` | the sidebar's `state`, `filter_text`, `main`, `finding` |
   | Local Changes' `list_width` and `lists_split` | Local Changes' `state`, `filter_text`, focus, `scroll`, `cursor`, `acting`, `lines`, `commit` |
   | Create Branch's sticky `checkout` | Create Branch's `open`, `asked`, `next_serial`, `name` |
   | the activity popover's `open` and its anchor | the activity `log`'s entries and selection |
   | the list of tabs: each tab's id, name, mark, kind, submitter and `TabView` (never its `Replier`) | `show_lost`, the repository's name and path, the scrolls of R3.3 |

   `CreateBranchState` and `ActivityLog` each hold fields of both halves today and are split.
   The one modal state stays derived (`modal::modal`), from the shown tab's state, the window's,
   and the prompts of R9.1.

2. With one tab open the window behaves exactly as it does before the split: every existing test
   passes, changed only where it builds the state.
3. The diff's toggles are the window's. Each tab keeps the context its repository's
   `diff.context` seeds (`Update::ConfiguredContext`), and draws it until the person changes the
   context in any tab; from then on the window's chosen context is drawn in every tab, and a
   background tab's seed never changes the shown tab's diff. A toggle changed redraws the shown
   tab's diff and marks every other tab's diff to be asked again when that tab is next shown.
4. The time zone and the `git` description are read once per application, never once per open,
   and reach the window on an application stream of their own (`Update::Started` no longer
   carries them), so no tab's open writes window state.

### R3 — Each tab's repository (T2, T3)

1. Each repository tab opens its own repository through `worker::open`, which only the tab's
   holder calls (R3.6); it has its own `RepositoryHandle`, `Updates`, `Replier`, `Epochs` and
   closing, and its update stream is driven by one task of its own, which applies each update to
   that tab's state alone.
2. Only the active tab is drawn. Every other tab keeps its worker and its state as values the
   window holds, and its update stream is still applied. No hidden view of a tab is mounted.
3. Switching tabs reads nothing from a repository but the refresh of R5.2, and the tab's state is
   drawn as it was left: its main view, selections, filters, detail tab, Show Lost Commits, commit
   draft, and the scroll of every list it draws — the history, the diff, the Commit tab, the
   Changes tab's files, the sidebar and Local Changes' lists — each list's scroll kept in `TabView`
   rather than in a component of its own.
4. Every hook that captured one tab's view or submitter for the window's life — today the
   desktop title, the lock's confirmation, the find's supersede and the focus refresh, in
   `window()`'s root scope — either lives in the shown tab's view, remounted with it on every
   switch, or reads the shown tab when it fires.
5. `git` is found once per application (`Discovery`), whatever the number of tabs.
6. Each tab's state, its update task and its `Replier` belong to one per-tab holder, which is
   mounted for every open tab, draws nothing, and is the only place a repository is opened. When
   its tab closes, the holder hands the tab's large answers to that repository's worker to free
   (R8.3) and unmounts. No struct field keeps a `Replier`; the holder draws its own tab's
   credential prompt when the window shows it (R9.1), so the `Replier` never leaves its hook.

### R4 — The strip (T5, T6)

1. A strip under the toolbar and above the banners, with one tab per open tab in their order and a
   "+" at its right end (FT §1-2). The active tab is drawn in the sidebar's surface tone, the others
   in the strip's darker tone, every tone and size from `cairn_ui::style`.
2. Tabs share the strip's width equally, down to a minimum tab width; a tab is pressed to show it.
3. **[observe]** Past that minimum, the strip ends in a button whose menu lists every tab by its
   whole name; choosing one shows it. No tab is out of reach, and a name cut in its tab is shown
   whole in that menu and in its tooltip.
4. **[observe]** Each tab has a × on the active tab and on the tab under the pointer, and a middle
   press on a tab closes it (R8.1).
5. **[observe]** A tab is reordered by dragging it along the strip; the order is the one R11
   remembers and Ctrl+Tab follows.
6. **[observe]** A tab's context menu offers Close Tab and Close Other Tabs, with their chords
   where they have one, built with `cairn_ui::menu`.
7. A tab's name is its repository's folder. When two open tabs share a name, each is named with
   the folder that holds it in parentheses (`t (uirev)`, `t (uirev2)`, FT §5); when those match
   too, with as many folders up as tell them apart, nearest first. Names are worked out again on
   every open and every close, so a tab left alone under its name is the plain folder again (Fork
   keeps the qualified name, FT §5, a bug not copied). **[observe]** The status box, the
   sidebar's header and the desktop title use the tab's name.
8. **[observe]** A tab's mark at its left is drawn when its last status lists an uncommitted
   change (staged, unstaged, untracked or conflicted: Local Changes' count above zero) and is absent
   on a clean tree. A tab with a dialog waiting (R9.3) draws a mark of another shape in its place.
   The marks differ by shape, never by colour alone.
9. Keyboard Tab never reaches the strip (FT §1).

### R5 — Refreshes across tabs (T8)

1. When the window gains focus, the active tab refreshes as a refresh does today (refs, ahead and
   behind, status), and every other repository tab asks its status alone (`Request::RefreshStatus`),
   each on its own refresh thread.
2. A tab made active refreshes as the active tab does on focus.
3. A tab whose commit runs starts no refresh, as today (the local lane's `defer_refresh`); the rule is per tab.
4. A background tab drawn by no component runs no timer of the view's and hears no key.

### R6 — The Repository Manager tab (T1)

1. File › New Tab, its chord and the "+" open a Repository Manager tab at the end of the strip and
   show it (FT §3). **[observe]** With one already open, they show that one.
   **[observe]** Recent's order and cap, the empty Manager's words and the Manager's keys (R6.2,
   R6.3) are settled by R1's questions 13-15.
2. Its left holds a "Repository Manager" heading and a Recent list: the repositories R11 remembers,
   newest first, each a name and its path, virtualized. Its right, for the row selected, holds the
   repository's name, its path and an Open button; with nothing in Recent it says how to open one.
   Its Open Repository… button asks R7.2's picker.
3. Open, a double press or Enter on a row opens that repository in place of the Manager tab, or
   shows its tab when it is already open (R7.3).
4. A Recent row whose folder is gone or no longer a repository says so on its row; its Open is
   greyed with the reason, and its context menu's "Remove from Recent" removes it. Whether each
   row's folder is there is asked off the UI thread, on the store's thread, when the Manager is
   shown, and drawn when it answers.
5. While the Manager tab is shown: the menu bar holds File, Window and Help alone; the toolbar's
   repository buttons are greyed with the reason "Open a repository first."; the status box says
   that no repository is open (FT §3, §11).

### R7 — Opening (T1, T7, T10)

1. Each path on the command line opens a tab, the last one named shown. `repository_path::chosen`
   takes every argument, not the first.
2. File › Open Repository…, its chord and the Manager's button ask the desktop's folder picker
   (`rfd`'s `AsyncFileDialog::pick_folder`, parented to the window) for one folder. Its future is
   awaited in a task on the UI executor, rfd running the portal call on a thread of its own (FR
   §5); the UI thread waits on nothing and no render file names a waiting primitive. A cancelled
   picker does nothing.
3. Opening a repository whose root is already open in a tab shows that tab. The root is known
   only once the new tab's worker has discovered it (`Update::Opened`); a tab whose root another
   open tab already has closes itself before drawing anything and the window shows the other, so
   of two opens of one root racing, the one answered second goes. A path inside a repository opens
   the repository that contains it, as `SharedRepository::discover_for` finds it; two worktrees of
   one repository are two roots and two tabs. **[observe]**
4. **[observe]** A repository that cannot be opened still opens its tab, whose body shows why in
   the words a failed open shows today, with Close Tab. It is not added to Recent.
5. A repository that opens is moved to the top of Recent.

### R8 — Closing and restoring (T9)

1. File › Close Tab, its chord, the tab's ×, a middle press and Close Tab in its menu close the
   active (or pressed) tab. The tab leaves the strip at once; the tab shown next is its left
   neighbour, or its right when it had none. **[observe]** Fork's choice of neighbour.
2. Closing a repository tab asks its worker to close (`Request::Close`) and never waits: a write
   running in it finishes, its other work ends as a window's close ends it today, a credential
   prompt it had open or waiting is refused (as `session::apply`'s `withdraw` refuses one at the
   window's close), and its holder stays mounted, out of the strip, until that repository's update
   stream ends.
   An ending that arrives for a tab that has closed — the finished write failed — comes forward
   over the shown tab, titled with the closed repository's name, with Close alone and no skip,
   since there is no tab for it to wait on.
3. Nothing a tab held is freed on the UI thread: its history, refs, status, sidebar rows, Local
   Changes lists and diff answers are handed to its repository thread to free, before or with its
   close, since a `Request::Retire` sent after the close is dropped on the UI thread (WG §3.2).
4. Closing the last tab leaves the window on a Repository Manager tab. **[observe]**
5. File › Exit and the window's own close close every tab's repository. The window closes when
   every update stream has ended, or, on a second request after `CLOSE_PATIENCE`, at once. While a
   write still runs, the closing line under the toolbar names the repository and the write
   ("Closing after Commit 1 file in cairn finishes. Close again to quit now.").
6. **[observe]** File › Restore Closed Tab and its chord open the most recently closed repository
   again, at its tab's old place in the strip, as a fresh open. The window keeps the last 20 closed in this
   session, newest first; a closed Manager tab is not among them. Greyed with its reason when there
   is none.

### R9 — Dialogs across tabs (T11)

1. A credential prompt from any tab comes forward over the tab shown, titled with the asking tab's
   name, without switching tabs. Each tab keeps its own prompt (`TabView`); the window shows the
   one that arrived first, drawn by its own tab's holder, and a second waits behind it. A prompt's
   id is unique in the application, so an answer can never reach another tab's `git`.
2. The one modal state decides which dialog is drawn and that keys are inert; the strip takes no
   press while a dialog is drawn.
3. A Git Error, a "Couldn't <name>", any other ending, and a confirmation whose answer arrives
   after its tab stopped being shown (`Remove index.lock…`'s, which opens when the engine answers,
   not at the press), wait on their tab, whose waiting mark (R4.8) is drawn until the tab is
   shown, when the dialog opens.
4. The status box and the activity popover show the shown tab's operations and log. A background
   tab's operation runs on and ends into its own tab's state.

### R10 — Menus and chords (T1)

1. File, in Fork's order with Fork's separators (FT §9), listing only what works: New Tab,
   Open Repository…, Close Tab, Restore Closed Tab — Exit.
2. Window: Select Previous Tab, Select Next Tab — Zoom In, Zoom Out.
3. Six actions join the accelerator table, each heard in the window's scope:

   | Action | Linux | macOS |
   | --- | --- | --- |
   | `NewTab` | Ctrl+T | ⌘T |
   | `OpenRepository` | Ctrl+O | ⌘O |
   | `CloseTab` | Ctrl+W | ⌘W |
   | `RestoreClosedTab` | Ctrl+Shift+T | none (⇧⌘T is Fork for macOS' New Tag, FT §8) |
   | `NextTab` | Ctrl+Tab | ⌃Tab |
   | `PreviousTab` | Ctrl+Shift+Tab | ⌃⇧Tab |

   **[observe]** `NextTab` and `PreviousTab` follow the strip's order and wrap at its ends. Each is
   a window chord a text field hands to the window (`cairn_ui::text_field`), so they work from the
   commit subject and every filter. No chord is listed twice, and the table's pin keeps
   its rule examples.

### R11 — Remembering the tabs (T12)

1. Cairn keeps, in `$XDG_STATE_HOME/cairn/tabs.toml` (`~/.local/state/cairn/tabs.toml` when the
   variable is unset or empty), the open repository tabs in strip order, the one shown, and the
   Recent list, at most 30 entries, newest first; with a format version.
2. The file holds absolute paths alone, as TOML through `serde`'s derive and the `toml` crate.
   A path that is not valid UTF-8 cannot be a TOML string: its tab opens and works, but it is
   neither remembered nor put in Recent.
3. It is read once as the application starts, before the window opens, its size capped at 1 MiB;
   a missing file, an unreadable one, one over the cap, one with another format version or one
   that does not parse opens no remembered tab, says nothing, and is replaced on the next write.
4. It is written whole each time the open tabs, their order, the one shown or Recent change: on an
   application thread of its own, to a temporary file in the same directory renamed into its
   place, the latest asked state winning when writes queue. The directory is created `0700` when
   missing. A failed write is tried again with the next change; it opens no dialog and never stops
   the window.
5. **[observe]** On launch: the remembered tabs open in order, then each command-line path (R7.1, switching to
   its tab when already open); the shown tab is the last path named, or the remembered one shown.
   With no path and nothing remembered, the working directory's repository opens if it is in one,
   and otherwise a Repository Manager tab.
6. The store's writes are the one row it adds to `FILESYSTEM_MUTATION_EXCEPTIONS`, its reason
   that it writes Cairn's own state outside every repository; no other file of `cairn-app` writes
   the filesystem.

### R12 — Dependencies (T7, T12)

1. `rfd` becomes a direct dependency of `cairn-app` at the version `freya-winit` already locks,
   adding no package to `Cargo.lock`: a `layer_dependencies_are_allowlisted` row, and an `exact`
   `[[bans.features]]` pin in `deny.toml` to the features already enabled.
2. `serde` (with `derive`) and `toml` become direct dependencies of `cairn-app`, each at a version
   already in `Cargo.lock`, adding no package: allowlist rows and `exact` feature pins.
3. If any of them adds a package to `Cargo.lock`, the phase stops and asks the user.

### R13 — Cost, measured (T2, T8)

On the bench machine (the window checks, `crates/cairn-app/src/window_check.rs`, over clones of
the bench repository, never the bench itself):
1. Switching between two open tabs over the bench repository draws the shown tab's first frame
   within 100 ms.
2. Ten tabs open and idle for 60 s use less than 1 % of one core, all threads together, and wake
   no thread of theirs.
3. Opening ten tabs at launch delays the window's first frame by nothing a tab's open does: the UI
   thread waits on none of them.
4. With ten tabs, a focus refresh's status for the active tab arrives within 1.5 times its time
   with one tab open, the other nine reading theirs at once.
5. The resident memory and threads each idle tab adds are measured and recorded in
   `docs/research/repository-tabs/measured.md`; they carry no bar.

## Acceptance criteria

Each criterion names the test that decides it, written by the phase that builds it. A test named
here is a commitment; a rename is recorded in `state.md`.

| Id | Criterion | Decided by |
| --- | --- | --- |
| C1 | R1: the observation file exists, every listed question answered VERIFIED or NOT OBSERVED, and every [observe] requirement settled and approved before its phase built it | the final QA's audit of `progress.md` and the file |
| C2 | R2.1-R2.2: every `View` field is in exactly one of `WindowView` and `TabView`, and one tab behaves as the window did | `the_window_and_its_tabs_hold_every_field_once`; the existing suites unchanged but for their builders |
| C3 | R2.3: the context starts at each repository's `diff.context` until changed, then the chosen value holds in every tab; a background tab's seed never moves the shown tab's; a toggle marks other tabs' diffs to be asked again | `the_diff_context_is_each_repositorys_until_the_person_chooses_one`, `a_toggle_asks_a_background_tabs_diff_again_when_it_is_shown` |
| C4 | R2.4, R3.5: `git` found once and the zone and `git` description read once for three tabs opened together | `three_tabs_find_git_and_read_the_zone_once` |
| C5 | R3.1, R3.6: only the tab holder names `worker::open`; each tab's updates reach that tab alone; no struct field keeps a `Replier` | `only_the_tab_holder_opens_a_repository` (guard, with self-test); `an_update_reaches_only_its_own_tab`; the credential guard following the `Replier` alias, with its self-test case |
| C6 | R3.2-R3.4, R5.4: only the active tab is drawn; switching keeps every per-tab value, every list's scroll among them, and the title, lock confirmation and find follow the shown tab; a background tab hears no key | `only_the_active_tab_is_drawn`, `switching_back_draws_the_tab_as_it_was_left`, `the_windows_hooks_follow_the_shown_tab`, `a_background_tab_hears_no_key` |
| C7 | R4.1-R4.3, R4.9: the strip's place, equal widths, "+", tones from style; past the minimum width the overflow list holds every tab, and no name is cut without its whole name a press away; keyboard Tab never reaches it | `the_strip_shares_its_width_and_lists_what_does_not_fit` (headless, 1, 4 and 30 tabs), `keyboard_tab_never_reaches_the_strip` |
| C8 | R4.4-R4.6: ×, middle press, drag reorder and the tab menu, as settled by R1 | `a_dragged_tab_takes_its_new_place`; `a_tab_closes_by_its_cross_its_middle_press_and_its_menu` |
| C9 | R4.7: names told apart by as many folders as needed, plain again once alone, and used by the status box, the sidebar's header and the title | `tab_names_are_told_apart_and_plain_again_when_alone` (pure, `cairn-model` or `cairn-ui`), `the_status_box_sidebar_and_title_use_the_tabs_name` |
| C10 | R4.8: the mark follows the status and the waiting dialog, by shape | `a_tabs_mark_follows_its_status_and_its_waiting_dialog` |
| C11 | R5.1-R5.3: focus refreshes the active tab whole and each other its status alone; activation refreshes; a commit defers its own tab's refresh only | `focus_refreshes_the_shown_tab_and_the_status_of_the_others`, `a_commit_holds_back_only_its_own_tabs_refresh` |
| C12 | R6: the Manager tab's lists, Open in place, the already-open switch, New Tab showing a Manager already open, a missing folder's row, the reduced menu bar and greyed toolbar | `new_tab_shows_the_manager_already_open`, `the_manager_opens_a_recent_repository_in_its_place`, `a_missing_recent_repository_says_so_and_cannot_be_opened`, `the_manager_tab_greys_the_repository_chrome` |
| C13 | R6.2: the Recent list is virtualized | `a_recent_list_of_10000_builds_one_viewport` |
| C14 | R7.1, R7.3-R7.5: every command-line path a tab, a repository already open switched to (two racing opens of one root leaving one tab), a path inside one opening its root, a failed open a tab of its own | `every_path_named_opens_a_tab_and_the_last_is_shown`, `opening_an_open_repository_shows_its_tab`, `two_opens_of_one_root_leave_one_tab`, `a_repository_that_fails_to_open_has_a_tab_saying_why` |
| C15 | R7.2: the picker's future is awaited in a task and its answer arrives as a value; a cancel does nothing | `the_folder_picker_answers_as_a_value` (with the picker behind a seam the test answers) |
| C16 | R8.1-R8.3: the neighbour shown, a running write finishing after its tab left, its failure coming forward with Close alone, its prompt refused, the tab's answers freed on its worker | `closing_a_tab_shows_its_neighbour`, `a_tab_closed_during_a_commit_lets_the_commit_finish`, `a_closed_tabs_failed_write_comes_forward_without_its_skip`, `closing_a_tab_refuses_its_prompt`, `a_closed_tabs_answers_are_freed_off_the_ui_thread` |
| C17 | R8.4-R8.5: the last tab leaves a Manager tab; the window's close ends every tab, names the repository it waits on, and closes on the second request after its patience | `closing_the_last_tab_leaves_the_manager`, `the_window_closes_when_every_tab_has_ended` |
| C18 | R8.6: Restore Closed Tab reopens at the old place, 20 deep, greyed when empty | `restore_closed_tab_reopens_where_the_tab_was` |
| C19 | R9.1-R9.2: a background tab's prompt comes forward titled with its name; two wait in turn; a prompt id never reaches another tab; the strip takes no press under a dialog | `a_background_tabs_prompt_comes_forward_with_its_name`, `prompt_ids_are_unique_across_tabs`, `the_strip_takes_no_press_under_a_dialog` |
| C20 | R9.3-R9.4: a background tab's ending, and a confirmation answered after its tab was left, wait on their tab under the mark and open when shown; the status box is the shown tab's | `a_background_tabs_git_error_waits_for_its_tab`, `a_lock_confirmation_answered_after_a_switch_waits_for_its_tab` |
| C21 | R10: the File and Window menus' items and order; the six chords on each platform, heard from a text field, the table's pin green; Ctrl+Tab's order and wrap | `the_file_and_window_menus_list_the_tab_items`, `the_tab_chords_reach_the_window_from_a_text_field`, `next_tab_wraps_at_the_strips_ends`, the table's pin |
| C22 | R11.1-R11.5: the store round-trips; a non-UTF-8 path is left out; a bad file opens nothing and is replaced; a write is atomic and latest-wins; the launch rules | `the_tabs_file_round_trips`, `a_path_that_is_not_utf8_is_not_remembered`, `a_bad_tabs_file_opens_no_tab_and_is_replaced`, `a_write_replaces_the_file_whole_and_the_latest_wins`, `launch_opens_the_remembered_tabs_then_the_paths_named` |
| C23 | R11.6: the store's row in `FILESYSTEM_MUTATION_EXCEPTIONS`, pinned to its kinds of write | `only_the_ops_module_changes_the_filesystem` with the new row |
| C24 | R12: the lockfile gains no package; the allowlist and feature pins hold | `gate.sh --step deps`; `layer_dependencies_are_allowlisted` |
| C25 | R13.1-R13.5: the bars met and the numbers recorded | `#[ignore]`d window checks `tabs_switch_check`, `tabs_idle_check`, `tabs_launch_check`, `tabs_refresh_check`, run in the final QA |
| C26 | Every list the packet adds builds one viewport; the UI thread waits on nothing; the full gate green | the twins named above and in the root `CLAUDE.md`; `scripts/gate.sh` |

## Out of scope

- Clone, Init, the Repository Manager's summary card and its Repositories list of a source folder
  (the roadmap's `repository-manager`, slimmed to them).
- Quick Launch (`quick-launch`) and Preferences with their settings store (`preferences`): the diff's
  settings, the splitters and the theme across sessions stay the session's.
- A second window, dragging a tab out to one, and a second launch handing its paths to the running
  window: two processes each keep their own tabs, the last to write deciding the next launch.
- Fork's workspaces.
- One local write lane for two worktrees of one repository open in two tabs (the roadmap's
  `worktrees`): each tab keeps its own lane until that packet keys the lane by the common directory.
- Freeing a background tab's loaded history (issue #4).
- Dropping a folder from the file manager onto the window: winit delivers no drop on native
  Wayland (FR §6), so it would work on X11 alone.
