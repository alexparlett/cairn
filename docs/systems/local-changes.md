# Local Changes

How the window's Local Changes view is built: the two lists it draws over the working
tree's status, how a path is chosen and its diff asked, what keeps a diff from being drawn
for any path but the one chosen, how several paths selected draw their diffs together, how the
view stages, unstages and discards files, the diff's staging gesture over lines, and the commit
box under the diff. Spec: `docs/prd/refs-and-status.md` R9 (criterion C9) for the view, and
`docs/prd/staging-and-commit.md` R8 (criteria C8's view half, C18 and C24's conflicted rows)
for its actions, R9 (criterion C19) for the gesture and R10 (C13, C14 and C24's view halves)
for the commit box. Fork is the standard (`docs/research/refs-and-status/fork-refs-and-status-ui.md`,
section 6, and `docs/research/staging-and-commit/fork-staging-and-commit.md`, sections 1-2);
every place Cairn's view is not Fork's is named under "Where it is not Fork's". The status
itself — what `git status` lists and how it is read — is `status.md`; the working-tree query
that answers a path's diff is `diff.md`, "The working-tree query"; the verbs the actions run
are `staging.md`, on the local write lane of `git-processes.md`, and so are commit and amend,
which the commit box asks.

## What it draws

Pressing **Local Changes** in the sidebar (`sidebar.md`) puts the view in the main region in
place of the history and its detail pane (`window::window`, `MainView::LocalChanges`,
`crates/cairn-app/src/local_changes_pane.rs`), as Fork lays it out:

- **Left**, behind a draggable splitter (its share kept for the session,
  `LocalChangesView::list_width`): a filter field — the only control above the lists — then
  **Unstaged** above **Staged**, each under its heading — its name, its Stage or Unstage
  button, and over Unstaged Fork's double chevron, Stage All — and each one flat, virtualised list
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
- **Under the lists**: one line saying what the writes asked from the view are doing — the
  write running and how many wait behind it, or the first queued — or why the last action
  asked nothing, or how the last write ended when it did not do what was asked
  (`local_changes_actions::acting_line`, staging-and-commit R8.6).
- **Right**: the chosen path's diff under the diff view's bar — previous and next change,
  ignore whitespace (drawn off and disabled here: the diff is always the exact one,
  staging-and-commit R8.5), context, entire file, side by side, the shared settings — unified
  or side by side; or the notice of a state that is not text, with Load Diff for a file past the
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
M files", both counts in distinct paths, as the sidebar's count is (the user's decision,
2026-10-07): M is the status's (`LocalChanges::paths`) and N the rows the filter left, counted
by the same pass that left them (`MatchedRows::paths`, a merge of the two lists' rows on the
worker), so a path left in both lists is one file; the window keeps N with the rows
(`local_changes_state::shown_paths`) and hands it to the list (`LocalChangesList::shown_paths`).
"No path matches the filter." when it leaves none.

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

## Acting on files

What a person does to the lists is reported by the component as a choice or a
`cairn_ui::ListIntent`, and decided by `crates/cairn-app/src/local_changes_actions.rs`; the
component decides nothing about what is staged.

**The selection** (R8.1, `cairn_ui::ListSelection`): the paths selected in one list, sorted by
their bytes, with the path a range extends from. A plain press, or ↑ or ↓, selects one path and
chooses it; a press with the table's extending press held (⌘- or Ctrl-click,
`Action::ExtendSelection`, resolved from `HeldKeys` since a pointer event carries no modifiers
here) toggles a path — a selection made of the path chosen when none is made yet — and a press
with the range press held (Shift-click), or Shift+↑/↓ on the focused list, selects every row the
list shows from the anchor to it. With one path selected the diff shows it; with several it
draws their diffs together ("Several paths, drawn together", below), the path last pressed in —
or, once it is toggled out, another the selection holds — staying the path chosen, the anchor a
range extends from; a toggle that takes out the last path leaves nothing
selected in that list, so nothing there is drawn selected or acted on. A row is drawn selected
by a binary search as it is built; the path chosen is drawn selected only while no selection is
made in its list. Selecting in the other list starts a selection there. A refresh that keeps
the path chosen costs the selection nothing — a path it names that the lists no longer list is
left out where it is acted on — and one that moves or lets go of the path chosen
(`local_changes_pane::follow_the_lists`) moves the selection with it, so a path the status took
away never comes back selected unseen.

**Which paths an action takes**: the selection's paths the lists drawn still list — each found
by a binary search — or, with no selection made in that list, the path chosen; a whole-file
action names a rename's source beside its path (`LocalChanges::whole_file_paths`), so a staged
rename unstages whole (`git reset -q -- <old> <new>`) while a rename's source listed as a row of
its own resets alone. Every path is one `git status` listed: the engine trusts its caller with
that (`staging.md`, Residuals). A collapsed untracked-directory row — the only one status lists
whole, a nested repository — stages as `git add` takes it, a gitlink at its commit, and nothing
under it (`a_nested_repositorys_row_stages_exactly_what_git_add_adds_for_it`).

**Fork's five routes** (R8.2), each asking `LocalWrite::StageFiles` from Unstaged or
`LocalWrite::UnstageFiles` (back to `HEAD`) from Staged through `local_writes::ask`:

- a double press on a row — the first press makes the row the selection, so it stages that row;
- the table's chords on a focused list (`Scope::LocalChanges`): Enter or Ctrl+Shift+S (Return
  or ⌘S), and, for the whole list, Ctrl+Alt+Shift+S (⌥⇧⌘S) — heard by the list's own key
  handler, so nowhere else, and never from a text field, which keeps Enter and Backspace;
- a drag from one list released over the other (`cairn_ui`'s `local_changes_drag`): tracked
  by the two lists together, never a row, with the path it began on, so it survives its row
  unmounting or a status replacing the lists; a release over the same list, outside both, or
  after Escape drops nothing; a press heard while a drag is on (its release made where the
  window could not hear it) or the window losing focus ends it — a press over the lists or
  their filter by the platform's mouse-down, which a text field cannot cancel, and elsewhere
  by the global pointer-down; while it is on, the list it
  would drop on scrolls at its edges (`EdgeScroll`); it drops the selection it began in, or
  the path it began on alone;
- each list's button — Stage or Unstage, dimmed with nothing to act on, and Stage All or
  Unstage All while the table's Stage All press is held (⌥, Alt on Linux) — and the double
  chevron's Stage All in Unstaged's heading;
- the context menu (`local_changes_menu`): Stage or Unstage, Discard Changes… on the unstaged
  side, Stage All or Unstage All, Copy Path; a right-press on a row outside the selection
  chooses it first; each item closes the menu as it is chosen.

Stage All and Unstage All ask `LocalWrite::StageAll` or `LocalWrite::UnstageAll` with the
lists drawn, shared, and — with a filter on — the rows it shows (the user's decision,
2026-10-09): every row with no filter, and only the rows shown with one, so a change the filter
hides, a conflicted one among them, is never staged or unstaged unseen; nothing while the
filter's rows are on their way. Their paths are gathered on the local lane, never on the UI
thread, which copies at most the filter's row indices.

**After a stage or an unstage** (R8.3) the selection moves to the nearest path left in the list
it left — the row that slides into the first acted row's place, else the nearest above it
(`cairn_ui::nearest_remaining`) — and its diff is asked at once; with none left, nothing is
selected. For a selection with gaps it is measured from where the selection began: the row in
the first acted row's place, else the nearest above it — the rule the user ratified on
2026-10-09. The same holds after a confirmed discard.

**A discard** (R8.4) is offered on the unstaged side only. A selection holding a submodule
(R8.8) or a conflicted path (R8.7), and any discard on the Staged list or a staged diff, asks
nothing: the view says why (`cairn_ui::NoDiscard`), and the menu draws its Discard disabled with
the reason beside it. Otherwise the discard chord (Backspace, Delete or Ctrl+Shift+D; ⌫ or ⇧⌘D)
or the menu asks the engine what it would lose — `Request::DiscardConsequence`, computed by
`ops::discard_files_consequence` on the local lane, in the order asked, so after every write
asked before it — and the view says it is counting. The count is per path (a `git diff-files`
read for each tracked file) and holds the local lane while it runs, so it is numbered in a lane
of its own: a newer discard asked, the view being let go of (`Request::StopCounting`, asked as
Local Changes unmounts) and a close each end it, between paths or by ending its read, and it
then answers nothing. The answer for the discard asked last opens
the confirmation (`Confirming::new`, `cairn_ui::ConfirmDialog`: the prompt and the button drawn
from the `Consequence`, Cancel focused), and its token asks `LocalWrite::DiscardFiles`; an
answer for an earlier ask, or one arriving once the view is no longer shown, is dropped. A
refusal the engine makes before any dialog — a nested repository among untracked rows, a path
that is not a file — is said. No modifier, setting or route reaches a discard without the
dialog.

**A conflicted row** (R8.7, C24) stages whole by every route — `git add`, which marks it
resolved — and is never discarded. As in Fork, nothing more is said before it is staged (the
user's decision, 2026-10-09): staging it is the one way Cairn resolves a conflict, and Cairn
offers no way back to the conflicted state — git's own `git checkout -m <path>` is it.

**While a dialog is open** — a confirmation or a credential prompt — nothing the lists or the
diff report is acted on, and the window's chords are inert (`shortcuts::act`). A credential
prompt that arrives while a confirmation is open (a write running behind it asked for a secret)
sets the confirmation aside, kept unanswered, and the confirmation is drawn again once the
prompt is answered or refused (`window::window`).

**The diff hears the same chords** (`Scope::LocalChanges`): on the lines a drag across it
selected ("The diff's staging gesture", below), or with none on the file it shows — or the paths
drawn together — whole, in the list it was chosen from (`local_changes_actions::on_the_diff`).
Hovering is not selecting (Fork, Tracker #103): a chunk under the pointer changes nothing the
chords do.

## Several paths, drawn together

With two or more paths selected in one list, the diff draws their diffs together, one under
another, as Fork does (R8.1, the user's decision of 2026-10-09). What the selection asks is
worked out as the view renders, whenever the lists, the selection or the settings move
(`local_changes_pane::together_wanted`, `draw_together`): the paths the lists drawn still list —
each found by a binary search — in the order the lists show them, each with the side it is
asked as (`diff_actions::working_query`), a conflicted path asking nothing and drawn as its
notice. The paths are asked in one ask of the diff thread, in the file-diff lane
(`Request::Together`, `DiffState::show_together`, `crates/cairn-app/src/diff_state/together.rs`):
it reads each path's working-tree diff in order, as a single path's is, prepares each for the
views there, and answers a page at a time (`Update::Together`, at most `TOGETHER_PAGE_FILES`
paths or `cairn_git::PAGE_LINES` lines a page) under the ask's own number, so a page of another
ask is never kept. It reads under Expand All's line budget (`EXPAND_ALL_LINES`: each path costs
one and every line of both its sides): once spent, the paths after are not read, and each says
so and to choose it alone. A path's failure is that path's line; a whole ask's failure is said
under every path still awaited.

The files are drawn by `cairn_ui::StackedDiff` (`crates/cairn-ui/src/stacked_diff.rs`): one
virtualised list of rows of the diff's height — each path's own row, then its diff's rows,
unified or side by side, or the notice standing in their place — laid out by the Commit tab's
table of files opened in place (`cairn_ui::Expansion`), so a row is placed by a binary search
over the files. The bar over them names how many files are drawn; previous and next change are
inert there.

With no lines selected the chords act on the files read and drawn — a diff, its notice or its
failure — never one still being read or one the line budget left unread
(`DiffState::together_read_paths`; the user's decision, 2026-10-09,
`the_chords_over_files_drawn_together_take_only_the_files_drawn`); a discard of several files
names them in its dialog, the first three and how many more (`Consequence::prompt`).

The same paths asked again — a refresh, the settings moved — keep what they draw until their
answers arrive, under a new number; other paths are drawn afresh, from the top. While paths are
drawn together the path chosen stays chosen but is not asked (`choose_working` asks nothing then,
and `working_needs_asking` is false), so the two never take the lane from each other; once one
path, or none, is selected, the paths drawn together are let go of — handed to a worker to free —
and the path chosen is asked again as the view renders. An ask that lost the lane before its
last page — to the Changes tab's file, the files opened in place or a changes query — is asked
again as Local Changes is shown (`together_needs_asking`, `reask_together`).

## The diff's staging gesture

In Local Changes' diff alone (R9), Fork's gesture (`fork-detail-and-diff-ui.md`, Finding 23),
built in `crates/cairn-ui/src/staging_gesture.rs` and handed to the diff view — the single
file's `DiffView` or the files drawn together's `StackedDiff` — as a `cairn_ui::Gesture`: the
side it is over (`GestureSide`, from the list the path was chosen in), the number of the answer
drawn (`DiffState::working_drawn`, or the ask drawn together, `together_drawn`), the drag the
window keeps (`LocalChangesView::lines`, a `cairn_ui::LineDrag`) and where its actions go
(`local_changes_actions::on_gesture`). The Commit and Changes tabs hand their diff view none,
so they draw no action (R9.5); a conflicted path's diff (R8.7) and a submodule's, binary,
LFS-pointer and too-large changes are notices with no rows, so they draw none either.

**Hover** (R9.1): the chunk under the pointer — the drawn hunk its row is in
(`UnifiedLayout::hunk_at`, `hunk_rows`) — is outlined and floats `Stage` and `Discard
Changes…` over the unstaged diff, `Unstage` over the staged one, at its top right (at the
list's top while its top is scrolled above). Its actions take every changed line the outline
holds (`hunk_selection`): at any context, every exact change the hunk groups, so at context
ten one chunk can be several of git's changes, and what is taken is what is outlined. The
outline, the selection's tint and the actions are one layer laid over the rows, outside the
rows the virtualising view recycles (`GestureLayer`), lifted above them; the outline and tint
take no pointer and the actions take no focus, so the diff keeps its chords. The pointer is
heard by the view's root (global moves, read against the list's frame), so the hover follows
the rows under a still pointer when they change.

**A drag** (R9.2): a primary press on a line row arms a drag; past `DRAG_THRESHOLD` it selects
from that row to the row under the pointer, kept to the file it began in, and — side by side —
to the column it began in (R9.3, `SideColumn`). It is the list's, never a row's: the root hears
every move and the release, so it carries on across rows the virtual list unmounts, and the
list scrolls itself at its edges (`EdgeScroll`). What it selects is read from the layout when it
ends (`UnifiedLayout::selection_in`, `SideBySideLayout::selection_in`): the changed lines in the
rows spanned and no others, found from the change stops, so its cost is the lines selected and
never the rows spanned, and a selection reaching thousands of rows below is whole. A press
without a drag selects nothing and lets a selection go, as does Escape. A release the window
never hears ends the drag at the next press; losing focus lets it go. The selection's actions
are drawn at its top right, counted (`Stage 2 Lines`, `Unstage 1 Line`, `Discard 2 Lines…` —
whose confirmation's button reads `Discard 2 Lines`; the user's decision, 2026-10-09), and
the chords act on it too. A selection belongs to the answer it was made under: another answer
drawn — a refresh, a write's re-read — makes it nothing. The number is the answer's: a working-tree
answer kept is numbered afresh (`DiffState::working_drawn`), and the files drawn together are
renumbered whenever a page replaces a diff that was drawn (`together_drawn`) — never by a page
filling a path still being read — so a selection made over a re-ask's old diffs is nothing once
its new ones arrive. Every act carries the number it was made under (`GestureAct::drawn`), and
`local_changes_actions::on_gesture` refuses one whose number is no longer drawn, so an action
built before a redraw never sends old rows' lines against a new diff
(`a_selection_over_files_drawn_together_is_nothing_once_a_page_replaces_its_diff`,
`an_act_made_under_an_answer_no_longer_drawn_asks_nothing`). The diff handed to a write is the
answer drawn, shared (`ShownDiff::shared_diff`), never copied on the UI thread.

**After an action** the actions hide until the diff is drawn again or the pointer moves, so a
second press cannot act twice on rows the first already took (Fork, Tracker #480); then they
follow whatever the new rows put under the pointer, or none.

**What each action asks** (`local_changes_actions::on_gesture`): the diff drawn, copied, with
the selection — `LocalWrite::StageLines` from the unstaged or untracked diff, `UnstageLines` from
the staged one, applied by the engine's patch (`staging.md`); the selection then goes. A
discard is refused in the view for a submodule or a conflict (`cairn_ui::no_discard`); every
line of a new file is the file — its discard deletes it — so it asks what discarding the file
would lose (`Request::DiscardConsequence`, the files' route, phase 03's `WholeFileOnly`);
otherwise it asks what discarding the lines would lose (`Request::DiscardLinesConsequence`,
`ops::discard_lines_consequence` on the local lane, after the writes asked before it, not read
once a newer ask superseded it), and the answer opens the confirmation, whose token asks
`LocalWrite::DiscardLines`. A staged rename's source listed as a row of its own draws the
rename's staged diff, so its lines unstage at the new path.

**The mode row** (R9.4): a file whose mode changed draws git's `old mode` and `new mode` as a
row of its own over its diff — over the notice, for a change of the mode alone — with its own
actions while hovered (`Stage Mode Change`, `Discard Mode Change…`, `Unstage Mode Change`), each
taking the mode change alone (`Selection::select_mode`), never a line (`cairn_ui::ModeRow`).

## The commit box

Under the diff, the width of the diff's side (R10.1), Fork's box (`fork-staging-and-commit.md`
§5), drawn by `cairn_ui::CommitBox` (`crates/cairn-ui/src/commit_box.rs`) from the window's
state (`crates/cairn-app/src/commit_box_state.rs`) by `commit_box_pane::CommitBoxPane`, which
also holds what each press does: a one-line subject with Fork's counter — the characters left
before 50, negative past it, red past 70 (`subject_count`) — and the `≡` that opens Recent
Commit Messages; a description in IBM Plex Mono with a ruler at column 72; `Amend` at the left;
and the button at the right, `Commit N Files` (`commit_caption`: plain `Commit` with none).

**The draft is the window's** (R10.7). The subject and the description are two `State<String>`s
kept by `LocalChangesView::commit` for the window's life, bound two ways to the fields — so a
refresh, Local Changes hidden and shown again, a failed hook and Amend's toggling leave it as it
was, and what the window writes into it (a recalled message, `HEAD`'s, `MERGE_MSG`) is what the
fields show. Both fields are built by the one key policy heard in the commit box's scope
(`text_field_in`, and for the subject `text_field_recalling`, `FieldScope::CommitBox`): the
commit chord (⌘Return, Ctrl+Enter) commits without a new line, the window's chords reach the
window, and no list or staging chord fires (R7.3 as amended). Tab moves from the subject to the
description. A commit made clears the draft; one that fails keeps it.

**What is asked, and where** (`docs/systems/git-processes.md`, "The local write lane"). As the box
is shown, and as each refresh's refs arrive while Local Changes is shown, the box asks its reads
(`Request::CommitReads`, answered by `Update::CommitReads`): the operation in progress, the hooks
git would run and the last ten messages — on the local lane, after the writes asked before them,
numbered in the commit-box lane so the next ask supersedes the last. Nothing is read on the UI
thread.

**Commit** asks `LocalWrite::Commit` with the draft composed as typed — the subject, then a blank
line and the description when there is one (`compose_message`; git's own cleanup is git's,
R6.1). The button needs a subject (Fork's rule) and something staged, or a merge in progress.
While it waits its turn the box says `Committing (waiting)…`; once git runs it says
`Committing…` with the time it has run, redrawn each second by a task of its own line's
(`Elapsed`, on `async-io`'s timer), and a Cancel that asks `Request::CancelWrite` for that
commit's id — never for one still queued (R4.3, R10.4).

**A failure** of git's own (`WriteEnding::Failed` with the command that ran) opens Fork's Git Error
dialog (`cairn_ui::GitErrorDialog`, R10.5) over the window: the command, and the commit's output
as it streamed (`Update::WriteOutput`) — or, if nothing streamed, the output the engine kept —
with ANSI sequences and control characters stripped (`strip_ansi`), the latest 10,000 lines or
1 MiB kept (`OutputTail`), drawn one row per line through a virtualizing view opened at its end.
`Skip pre-commit hooks and commit` is offered only where the last read found a `pre-commit` or
`commit-msg` hook git would run, and never for a commit whose hooks were already skipped — for
any failure git reports while such a hook exists, since a hook's failure and git's own cannot
be told apart without parsing git's localized words. For a commit it asks the same message
again with `skip_hooks`, `--no-verify` for that one commit; for an amend it is the commit box's
`cairn_ui::AmendSkip`, drawn in the dialog with the prompt the amend was confirmed with, whose
one press builds the skipped amend's token from that consequence and asks it at once — no
second dialog (the user's decision, 2026-10-09). The next commit runs its hooks. Close, Escape or a press outside closes it. The window's chords and Local
Changes' actions are inert while it is open, as under a confirmation. A refusal, a stale amend,
a cancel or a commit not run is said under the lists, as any write's ending is.

**Amend** (R10.3, R10.6, L12). Ticking it sets the draft aside and asks what an amend would
replace (`Request::Amending`, on the local lane, numbered in the amending lane): the
`Consequence` (`ops::amend_consequence`), `HEAD`'s message — of the commit the consequence names
— and amend's lists over the status the window drew, Unstaged as the status lists it and Staged
as amend's staged list against `HEAD`'s parent (`LocalChanges::amending`, laid out on the lane).
The lists that answer are drawn as a status's are; each status that arrives while Amend is
ticked asks again over it, superseding the read before, and every write asked supersedes it too,
so a stage queued behind it never waits on its walk; and the box shows `Reading what Amend
would replace…` until the first answer, the button waiting for each newer one. An empty draft is
filled with `HEAD`'s message once it arrives; a draft with text is kept as the amend's message.
Unticking puts the draft set aside back exactly as typed, ends the read in flight
(`Request::StopAmending`) and draws the status's own lists again. Amend is disabled on an
unborn branch and while any operation is in progress (R10.1).

While amending, a path chosen in Staged asks its diff against `HEAD`'s parent
(`WorkingSide::Amending`, `WorkingTreeDiff::Amending`: `git diff --cached HEAD^`'s answer), and an
unstage of whole files from Staged — each route, Unstage All among them — puts the entries back
to the parent's (`UnstageTarget::Commit`, `git reset -q <parent> --`) or, for a root commit's
amend, removes them (`UnstageTarget::Nothing`, `git rm --cached -f -q --`): the file leaves the
amended commit (`local_changes_actions::unstage_target`). Lines unstaged by the gesture are
applied against the index as from any staged diff. Where amend's staged list cannot be read —
a partial clone's missing blob on git 2.44 and later — the box says so and why, the lists stay
the status's against `HEAD`, and the amend itself is still offered.

The button reads the consequence's own words: `Amend <short id>` above the line it renders,
"Replaces <short id> '<subject>'." and whether the old commit stays in Show Lost Commits or
cannot be recovered (`Consequence::replaces`) — Fork's way, as the user decided it
(2026-10-09). **The commit box is the second confirmation surface** (R1.1,
`CONFIRMATION_SURFACES`, beside the dialog): an amend no remote has is confirmed by the box's
button, or by the commit chord from either field, which does exactly what the button does —
`cairn_ui::AmendButton`, one component drawing the button and the line it confirms, builds the
token from the consequence it draws, once per consequence, and hands it to the window to ask
`LocalWrite::Amend` at once. It refuses a consequence a remote has
(`Consequence::needs_force_push`): that amend reads `Amend <short id>…` and, by button or chord,
opens the confirmation dialog first, its words the whole prompt — the force push
(`Consequence::force_push_warning`) and the line — and its own button the one that builds the
token. Whichever builds it, the engine re-checks the token's consequence before git runs, and
refuses an amend whose `HEAD` moved or was pushed since. A commit made — or an amend — clears the
draft only where it still holds the message the commit took, so a draft typed while it ran is
kept; an amend unticks Amend.

**An operation in progress** (R10.8, L25): with a merge, an empty draft is filled once per merge
with git's `MERGE_MSG` as git wrote it — its `# Conflicts:` lines kept visible, which a commit
under `-F` keeps unless the person deletes them (the user's decision E) — and the commit is the
merge commit; Amend is disabled. During a rebase, `git am`, a cherry-pick or a revert the fields
and the buttons are disabled and the box says "Committing is unavailable while a rebase is in
progress."

**Recent Commit Messages** (R10.2): the `≡` opens a menu of the last ten messages' subjects,
newest first; choosing one fills both fields (`split_message`: the first line, and the rest after
its blank line). In an empty subject, or one holding the message last recalled, a bare ↑ recalls
the next older and ↓ the next newer, ↓ from the newest emptying the draft
(`accelerators::recall_step`, offered first by `text_field_recalling`); in a subject the person
typed, the arrows are the editor's.

## What enforces this

- The commit box, at the component (`crates/cairn-ui/tests/commit_box.rs`):
  `the_amend_toggle_reports_its_toggled_state_to_assistive_technology`,
  `the_amend_button_in_place_builds_one_token_from_the_consequence_it_draws`,
  `a_published_amends_button_asks_rather_than_confirms`,
  `the_amend_button_refuses_a_pushed_consequence_and_builds_nothing_unready`,
  `a_failed_amends_skip_confirms_the_prompt_it_draws_once`,
  `the_counter_is_drawn_and_a_stopped_box_asks_nothing`,
  `the_git_error_draws_one_viewport_of_output_and_the_skip_only_where_offered`; in the window
  (`crates/cairn-app/src/commit_box_tests.rs`):
  `a_draft_typed_then_amend_ticked_and_unticked_is_back_exactly`,
  `the_amend_button_its_line_and_its_token_name_one_head`,
  `a_published_amend_and_the_commit_chord_ask_the_dialog_first`,
  `the_commit_chord_amends_an_unpublished_commit_as_the_button_does`,
  `the_commit_chord_asks_nothing_while_the_box_is_not_ready`,
  `a_newer_amend_read_names_its_own_head`,
  `a_failed_amends_skip_amends_at_once_under_the_line_confirmed`,
  `a_draft_typed_while_a_commit_runs_outlives_its_ending`,
  `a_cancel_while_the_commit_is_queued_asks_nothing`,
  `a_failed_hooks_skip_commits_once_without_hooks_and_the_next_runs_them`,
  `with_no_hook_the_git_error_offers_no_skip`, `cancel_reaches_only_the_running_commit`,
  `a_merge_fills_the_draft_with_merge_msg_as_git_wrote_it_and_disables_amend`,
  `during_a_rebase_am_cherry_pick_or_revert_the_box_is_disabled_and_names_it`,
  `amend_is_disabled_on_an_unborn_branch`,
  `amends_staged_list_is_drawn_diffed_against_heads_parent_and_unstaged_to_it`,
  `amends_staged_list_unread_is_said_and_the_status_lists_stay`,
  `recent_messages_fill_both_fields_from_the_menu_and_the_arrows`; the window's state
  (`commit_box_state.rs`): `a_message_split_into_the_fields_composes_back_unchanged`,
  `ansi_sequences_and_control_characters_are_stripped`,
  `the_output_kept_is_its_latest_bounded_tail`,
  `amend_sets_the_draft_aside_and_fills_only_an_empty_one`, `a_merge_fills_an_empty_draft_once`,
  `a_failure_offers_the_skip_only_where_a_hook_exists`; through the real boundary
  (`worker/local_lane_tests.rs`): `the_commit_boxs_reads_and_an_amends_read_come_through_the_lane`,
  and a write superseding the amend read (`a_write_supersedes_the_amend_read_and_nothing_else`,
  `worker/pool.rs`),
  `a_failing_hook_fails_a_commit_and_the_skip_commits_past_it` (its command and output),
  `a_chunk_drawn_at_context_ten_stages_exactly_as_drawn`; the model:
  `amending_lists_amends_staged_list_beside_the_status_unstaged_one`,
  `an_amends_parts_are_its_prompt_and_only_a_published_one_needs_the_dialog`; the engine:
  `a_file_of_amends_staged_list_reads_as_git_diff_cached_against_heads_parent`
  (`crates/cairn-git/tests/diff/working_tree.rs`); the table:
  `only_a_bare_arrow_steps_through_the_recent_messages`.
- Layout (`crates/cairn-model/src/local_changes.rs`):
  `each_path_is_in_the_lists_its_changes_put_it_in_ordered_by_name`,
  `each_list_is_in_natural_order_and_found_in_it`,
  `each_change_is_drawn_as_its_kind_with_a_sources_path`, `the_count_is_of_distinct_paths`,
  `a_path_is_found_by_its_list_and_name`, `a_filter_leaves_each_lists_rows_that_hold_its_text`,
  `what_a_filter_leaves_is_counted_in_distinct_paths`,
  `fifty_thousand_paths_are_laid_out_in_order`.
- Acting, at the component (`crates/cairn-ui/tests/local_changes_actions.rs`):
  `a_held_press_toggles_or_ranges_and_shift_arrows_range`,
  `every_chord_of_the_lists_actions_is_reported_on_its_list`,
  `a_double_press_stages_or_unstages_its_row`,
  `the_buttons_stage_and_unstage_the_selection_or_with_the_press_held_everything`,
  `a_drag_drops_on_the_other_list_only_and_survives_its_row`,
  `a_drag_between_the_lists_scrolls_the_one_it_would_drop_on`,
  `each_rows_menu_offers_forks_items_and_no_discard_where_none_is_allowed`,
  `a_press_heard_mid_drag_or_focus_lost_ends_the_drag_without_a_drop`, and with every one of
  50,000 paths selected `a_status_of_50000_paths_all_selected_builds_one_viewport`
  (`tests/local_changes.rs`); the pure parts
  (`a_selection_toggles_spans_and_holds_paths_of_one_list`,
  `the_nearest_row_left_is_the_one_that_takes_the_first_acted_rows_place`,
  `a_press_becomes_a_drag_past_the_threshold_and_drops_on_the_other_list_only`,
  `a_discard_is_refused_for_staged_changes_submodules_and_conflicts_only`), and the edge
  scroll's lost release (`a_release_the_window_never_heard_ends_the_drag_at_the_next_press_or_focus_lost`).
- The gesture: what a chunk and a drag select, from the layout
  (`crates/cairn-model/src/row_selection.rs`):
  `a_chunk_at_context_ten_takes_every_change_it_draws_and_no_other`,
  `a_drag_selects_the_changed_lines_in_its_rows_alone`,
  `side_by_side_a_drag_selects_its_own_column`,
  `rows_drawn_ignoring_whitespace_select_nothing`,
  `a_long_span_selects_exactly_the_changes_between_its_ends`; at the component
  (`crates/cairn-ui/tests/staging_gesture.rs`):
  `a_hovered_chunks_actions_take_exactly_its_changes`,
  `a_view_handed_no_gesture_draws_no_action`, `a_drag_narrows_the_actions_to_its_lines`,
  `a_drag_across_unmounted_rows_selects_every_line_between_its_ends`,
  `side_by_side_a_drag_keeps_to_its_column`,
  `after_an_action_the_actions_follow_the_new_rows_or_none`,
  `after_an_action_a_moved_pointer_brings_the_actions_back`,
  `files_drawn_together_each_take_their_own_gesture`, and the viewport twins
  `the_gesture_builds_one_viewport_over_a_10000_line_diff` and
  `files_drawn_together_build_one_viewport`, named in the root `CLAUDE.md`'s virtualization
  invariant; in the window (`crates/cairn-app/src/local_changes_gesture_tests.rs`):
  `a_hovered_chunk_stages_and_unstages_exactly_its_lines`,
  `the_chords_act_on_the_selection_and_on_the_whole_file_without_one`,
  `a_chunks_discard_confirms_its_lines_and_a_new_files_every_line_is_its_file`,
  `the_mode_row_stages_the_mode_alone`, `nothing_acts_on_lines_while_a_confirmation_is_open`,
  `a_selection_over_files_drawn_together_is_nothing_once_a_page_replaces_its_diff`,
  `an_act_made_under_an_answer_no_longer_drawn_asks_nothing`,
  `part_of_a_new_files_lines_is_discarded_as_lines`,
  `the_chords_over_files_drawn_together_take_only_the_files_drawn`,
  `several_paths_selected_draw_their_diffs_together`,
  `a_renames_source_row_unstages_its_lines_at_the_new_path`; the paths drawn together as kept
  (`crates/cairn-app/src/diff_state/together.rs`):
  `the_paths_drawn_together_keep_only_their_own_asks_pages`,
  `the_same_paths_asked_again_keep_their_diffs_meanwhile`; and through the real boundary,
  `paths_drawn_together_are_answered_by_place_under_their_budget` (`worker/diff_tests.rs`, a path
  costing exactly the budget among its cases) and
  `paths_drawn_together_are_each_read_as_their_own_side` (`worker/local_lane_tests.rs`: an
  unstaged, a staged and an untracked path, against real `git`); the prompt naming the files,
  `a_discard_of_several_files_names_the_first_three_and_counts_the_rest`
  (`crates/cairn-model/src/consequence.rs`).
- Acting, in the window (`crates/cairn-app/src/local_changes_actions_tests.rs`):
  `the_chord_and_the_button_stage_a_selection_and_the_selection_moves_on`,
  `a_drag_the_menu_and_a_double_press_stage_what_is_selected`,
  `a_staged_selection_unstages_with_its_renames_source_and_all_takes_the_list`,
  `a_staged_selection_unstages_by_the_drag_the_button_and_the_menu`,
  `the_selection_moves_to_the_row_that_takes_the_acted_rows_place`,
  `stage_all_and_unstage_all_take_the_rows_the_filter_shows`,
  `a_refresh_or_an_emptying_toggle_leaves_nothing_selected_unseen`,
  `leaving_local_changes_ends_a_discards_count`,
  `a_discard_asks_what_it_would_lose_and_confirms_exactly_that`,
  `no_discard_reaches_a_staged_change_a_submodule_or_a_conflict_and_each_says_why`,
  `a_conflicted_row_stages_whole_by_every_route`,
  `local_changes_acts_on_nothing_while_a_confirmation_is_open`,
  `backspace_in_the_filter_edits_the_filter_and_discards_nothing`,
  `a_credential_prompt_sets_an_open_confirmation_aside_until_it_is_answered`,
  `local_changes_diff_is_always_exact_and_leaves_the_setting_alone`,
  `a_write_is_drawn_queued_running_and_stale_where_it_was_asked`,
  `a_discard_names_only_paths_the_lists_drawn_still_list`,
  `the_extending_press_toggles_out_and_the_diff_follows_the_selection`; through the real
  boundary (`worker/local_lane_tests.rs`):
  `a_discards_consequence_is_counted_after_the_writes_asked_before_it`,
  `the_dialogs_count_is_what_the_discard_then_does_to_a_mixed_selection` and
  `a_newer_ask_or_a_stop_ends_a_discards_count_and_its_read`, with the lane's
  `an_all_gathers_the_rows_the_filter_shows_or_every_row`; the paths a
  whole-file action names, `a_whole_file_action_names_each_rows_path_and_a_renames_source`
  (`cairn-model`); and against real git,
  `a_nested_repositorys_row_stages_exactly_what_git_add_adds_for_it`
  (`crates/cairn-git/tests/diff/write_verbs.rs`).
- Drawing (`crates/cairn-ui/tests/local_changes.rs`):
  `both_lists_draw_their_paths_with_their_badges`, `each_list_is_in_its_paths_order`,
  `a_press_and_the_arrows_choose_a_row_of_its_list`, `the_filter_says_what_it_leaves`,
  `the_filters_count_is_of_distinct_paths_a_path_in_both_lists_once`,
  `every_kind_of_change_draws_its_badges_shape` (each painted badge's cell read pixel for pixel
  against its glyph), `the_splitter_between_the_lists_drags`, the badge rule
  `each_kind_of_change_has_forks_badge` (a unit test in `crates/cairn-ui/src/local_changes.rs`)
  and `every_glyph_paints_a_shape_no_other_glyph_paints` (`ref_glyphs.rs`), and the viewport twin
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
  `the_filters_count_is_the_sidebars_distinct_paths_a_path_in_both_lists_once`,
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
  the lists (the user's decision, 2026-10-07): the layout menu and its
  collapse-all chevron (#71), Hide Untracked Files (#70), Show Ignored Files (#62) and the eye
  (Fork's side-by-side quick look, #35) are not drawn.
- The commit box: the subject is required, as Fork's is, and a merge in progress commits with
  nothing staged; Fork's limit setting, monospace toggle, Wrap paragraph at ruler, spell
  checking, autocomplete, commit template, `prepare-commit-msg` and AI drafts, sign-off, Commit
  and Push and the ⌘⇧A amend chord are not built (Commit and Push waits on push, L9). The ruler
  stands at column 72 of IBM Plex Mono's advance after the field's margin. Fork shows a commit's
  output live in its Activity Manager; Cairn shows it in the Git Error dialog when the commit
  fails, and draws no activity log beside the box.
- The amend button, and the commit chord while amending, confirm in place only where no remote
  has `HEAD`; a commit a remote has goes through the confirmation dialog first, where Fork for
  Windows warns that the commit is already pushed (its exact words are not recorded).
- A double press acts on its row alone, since its first press makes the row the selection (the
  user's decision, 2026-10-09).
- Paths drawn together are read under Expand All's line budget, and past it are not drawn — Fork's
  own bound, if it has one, is not recorded. Drawn together, a file past R2.6's limits offers no
  Load Diff and a file whose mode changed no mode row: either is reached by choosing the path
  alone. Previous and next change step through one file's changes, so they are inert over paths
  drawn together. (Each ratified by the user, 2026-10-09.)
- Adding a path to a selection drawn together asks every path again, so the files already drawn
  are read afresh and flash as they arrive: interim, until each file's answer is kept across a
  change of the selection (carried to phase 11, with measurements).
- Fork's floating actions sit at the chunk's top right; Cairn's also stand at the list's top while
  the chunk's top is scrolled above it, so a tall chunk's actions stay in reach. A selection's
  actions are drawn at its top, not at the chunk's. A press without a drag, or Escape, lets a
  selection go, and the window losing focus cancels a drag. (Each ratified by the user,
  2026-10-09.)
- The discard dialog names the files of a discard of several — the first three and how many
  more — where Fork's counts them (the user's decision, 2026-10-09).
- Stage All's double chevron sits in Unstaged's heading, as Fork for Windows draws it (Linux
  follows Fork's Windows rows); Fork for Mac draws it above the lists and flips it to Unstage
  All once nothing is left to stage. Unstage All is the Staged button with the press held, the
  menu's item and the chord.
- The Stage All press is ⌥ on macOS and Alt on Linux — Fork for Windows' own is not recorded.
  xfwm, openbox and Plasma 5 take Alt with the first button to move a window, so there the press
  never reaches Cairn and the button stays Stage or Unstage: it fails safe, and the chevron, the
  menu and the chord still reach Stage All and Unstage All (the user's decision, 2026-10-09).
- A discard asked on the staged side says "Staged changes can't be discarded: unstage them
  first.", where Fork does nothing (the user's decision, 2026-10-09).
- A drag whose release was made outside the window, where it could not be heard, ends at the
  next press — but a press on another view's text field (the sidebar's filter) is not heard,
  since the field cancels the global pointer-down as it takes it, so a release over the other
  list after it still drops.
- The menu offers Stage or Unstage, Discard Changes…, Stage All or Unstage All and Copy Path:
  Fork's Open, External Diff, Show in Files, Blame, History, Ignore, LFS, Stash and Save as
  Patch are not built; its items name no chord, since a chord spelled in a label is a modifier
  named in a component; nothing opens the menu from the keyboard.
- A submodule's row offers no discard (Fork's Discard Submodule Changes) and says why; a
  nested repository among untracked rows is refused before any dialog; the confirmation's
  default is Cancel (staging-and-commit L8, L24).
- Ignore Whitespace is disabled in Local Changes and the diff drawn is always the exact one,
  where Fork lets it hide changes it then stages (L6).
- The submodule's badge is a shape of Cairn's drawing (a box in a box); Fork's own submodule
  icon is not recorded.
- The count beside Local Changes is the distinct paths status lists (R9.2's wording), where
  Fork's is said to equal the entries `git status` lists; whether Fork counts a path with both
  staged and unstaged changes twice is OPEN in the research (its OPEN 9). "Showing N of M
  files" under the filter counts distinct paths on both sides too, so it agrees with the
  sidebar's count (the user's decision, 2026-10-07), though the lists draw a path in both of
  them as two rows.
- Entire File applies here, shared with the Changes tab, as it does to every view of one file
  under the bar.
- A status that could not be read — a partial clone's staged rename on git 2.44 and later among
  the causes — is said for the whole view, where a failed diff fails one path; the lists of
  the last status read stay drawn under it. Naming that state is #74.
- Pressing a ref in the sidebar returns the main region to the history; Fork's own behaviour
  on that press from Local Changes is not recorded.
