# Local Changes

How the window's Local Changes view is built: the two lists it draws over the working
tree's status, how a path is chosen and its diff asked, what keeps a diff from being drawn
for any path but the one chosen — the primary's alone, however many are selected — how the
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
`crates/cairn-app/src/local_changes/pane.rs`), as Fork lays it out:

- **Left**, behind a draggable splitter (its share kept for the session,
  `LocalChangesView::list_width`): a filter field — the only control above the lists — then
  **Unstaged** above **Staged**, each under its heading — its name, its Stage or Unstage
  button, and a small double chevron, ⇊ Stage All over Unstaged (Fork's) and ⇈ Unstage All over
  Staged — and each one flat, virtualised list
  (`cairn_ui::LocalChangesList`, `crates/cairn-ui/src/local_changes/list.rs`), with a draggable
  splitter between the two (the user's decision, 2026-10-07; its share kept for the session and
  never beyond it, `LocalChangesView::lists_split`, `LocalChangesList::split`). A row is a badge
  and a path, a rename's or a copy's source before it (`old → new`, `cairn_ui::change_text`).
  The badges are Fork's, told apart by their shape (the user's decision, 2026-10-07;
  `cairn_ui::change_badge`): `M` for a modified file and a type change alike, `+` for an added,
  intent-to-add or untracked one, `D` deleted, `R` renamed, `C` copied, and, painted as shapes
  rather than set in a font, a submodule's box (`RefGlyph::Submodule`) whatever changed in it
  and Fork's warning triangle for a conflicted path (`RefGlyph::Gone`). Colour only reinforces
  the shape. The Commit tab keeps its own letters.
- **Nothing under the lists** (staging-and-commit R14.6): a write running or queued shows in
  the title bar's status box, and its ending, when it is not a success or a cancel, in its Git
  Error or "Couldn't <name>" dialog — a stale patch naming its path once
  (`docs/systems/git-processes.md`, "Where an ending goes";
  `a_write_is_said_nowhere_under_the_lists_and_a_stale_one_in_its_dialog`).
- **Right**: the selection's primary's diff under the diff view's bar — previous and next change,
  ignore whitespace (drawn off and disabled here: the diff is always the exact one,
  staging-and-commit R8.5), context, entire file, side by side, the shared settings — unified
  or side by side; or the notice of a state that is not text, with Load Diff for a file past the
  limits; or, for a conflicted path, the conflict's notice in place of a diff (R9.4,
  `DiffNotice::Conflicted`). Previous and next change are heard from inside the view and move
  its own diff (`diff::actions::step`), whose scroll and change cursor are the view's
  (`LocalChangesView::scroll`, `cursor`), not the Changes tab's.
- **Instead of the lists**: "Reading the working tree's status…" before the first status;
  the read's failure when no status has been read (`RefreshState::failure(Refreshed::Status)`,
  said over the lists kept when one has); that the repository has no working tree; that the
  git in use cannot read a sparse index. With no change, the lists are empty and the right says
  "No local changes."

The count beside the sidebar's entry is the number of distinct paths the status lists
(R9.2, `sidebar::pane::local_changes_count`, `LocalChanges::paths`): a path in both lists once,
and a staged deletion with an untracked file of the same name — two entries git lists apart —
once; none is shown with no change.

## Which list, in what order (`cairn_model::LocalChanges`)

A status arrives laid out already: `LocalChanges::new` runs on the refresh thread when the
status is read (`worker/refresh/lane.rs`), and `Update::Status` carries it, so the window only
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
thread — which walks no history, so a keystroke never waits on an open of it — and superseded
by the next keystroke or by a newer status's ask, nothing else; a pass
superseded mid-way stops within a few thousand paths and sends nothing. Its answer,
`Update::FilteredLocalChanges { changes, text, rows }`, is kept only for the newest lists and
the text as it is (`LocalChangesState::filtered`). With a filter on, a new status is not drawn
until its rows come: the lists before it stay drawn with their own rows meanwhile, so a row of
one status is never read by an index into another, and a refresh never empties the view while
a filter is typed (`crates/cairn-app/src/local_changes/state.rs`). Until the first answer for a
text, the lists say "Filtering…" rather than counting every row as matched; then "Showing N of
M files", both counts in distinct paths, as the sidebar's count is (the user's decision,
2026-10-07): M is the status's (`LocalChanges::paths`) and N the rows the filter left, counted
by the same pass that left them (`MatchedRows::paths`, a merge of the two lists' rows on the
worker), so a path left in both lists is one file; the window keeps N with the rows
(`local_changes::state::shown_paths`) and hands it to the list (`LocalChangesList::shown_paths`).
"No path matches the filter." when it leaves none.

## Choosing a path, and asking its diff

The diff shown is the selection's primary's (staging-and-commit R8.1, rule 8; "The selection",
below): never chosen on its own, but derived from the primary and the lists drawn and asked by
one function (`diff::actions::show_primary`) whenever the selection or the lists move. It is
asked through the working-tree query as its list says (R9.3, `diff::actions::working_query`), and
it is the one diff drawn however many paths are selected (R8.1 as amended 2026-10-10, the redesign's L1;
observed in Fork for Windows, `lc2`): the primary's — the file clicked first — with everything
one file's diff has, Load Diff, the mode row, previous and next change and the gesture, under its
own bar, which counts nothing (the user's answer of 2026-10-10: the lists' highlighted rows show
the selection); no other file's diff is asked or kept. With nothing selected the diff says "No
file selected" (`local_changes::pane::NO_FILE_SELECTED`). Its staged diff (`WorkingSide::Staged`) from
Staged, its unstaged diff (`WorkingSide::Unstaged`) from Unstaged, its untracked diff
(`WorkingSide::Untracked`) for a path git does not track; a conflicted path asks nothing. It is
asked at the Changes tab's options — the shared context and whitespace setting, and the entire
file — since the view is one file's diff under the same bar. With nothing selected, the first
path the lists show is selected as the view opens — Unstaged's first row, else Staged's (R9.3)
— and as the lists' first status, or files appearing after none, is drawn. It is asked at once
while Local Changes is shown, and as the view is shown otherwise, so a status arriving while
the history is shown never takes the file-diff lane from the commit's file.

The primary's diff lives in the diff selection beside the commit's
(`crates/cairn-app/src/diff/state/working.rs`, `DiffState::choose_working`), keyed by what it was
asked for (`WorkingChoice`: the primary's list and path and the lists it was asked in, written
by `show_primary` alone), because the three —
the Changes tab's file, the files opened in place in the Commit tab, and this path — share the
file-diff lane: whichever asks takes it, and the others are asked again, whole, when their view
is shown (`working_needs_asking`, `reask_working`; `diff.md`, "The file-diff lane is shared").
Who holds the lane is one value (`DiffState::lane`, a `LaneHolder`: `File`, `Working` or
`Expansion`), taken by one call (`take_lane`) and let go of by its holder alone
(`let_go_of_lane`), never a flag per holder that each other holder must reset
(`docs/research/staging-and-commit/review-code-app-ui.md`, H1).
A change of the settings with Local Changes shown asks its path again at once and the others
when their tab is shown (`Asking::Working`). A changes query supersedes the lane whoever holds
it.

**An answer is drawn only for the query it names.** `Update::FileDiff` for a working-tree
query is kept only when it names exactly the query the primary is asked as now — its path,
its side and its options (`DiffState::working_arrived`) — so a path in both lists draws each
list's diff and an answer for the list chosen before, arriving late, is not drawn; one not kept
is handed to a worker to free.

**A refresh moves the selection by one rule.** Each status drawn is numbered
(`LocalChangesState::serial`). When other lists are drawn, or the filter's rows change, the
selection is moved by the one pure rule (`crate::next_selection`, "Where the selection goes",
below) inside `LocalChangesState` — as the status and the filter's answers arrive, in
`session::apply`, never from a render effect — and its primary's diff is asked there and then
(`a_refresh_moves_the_selection_and_asks_its_diff_without_a_render`). A primary still listed and
shown is asked again as it is listed now — the file may have changed under the same status —
and its last diff stays drawn until the new one comes (`DiffState::refresh_working`) — and with
it a finished line selection over it, kept while the new answer's text is the same and let go
of, said once, when it differs ("The diff's staging gesture", below); one the
status took away, or the filter hides (the redesign's L3, superseding the Changes tab's rule of
keeping a hidden path chosen), gives its place to the row that took it, so no diff is drawn for
a path the lists do not show; with nothing selected, the diff is let go of. Another primary's
diff starts at its top, with no change moved to and no lines selected, silently, as does the
primary asked at other options (`diff::actions::reset_local_view`, `ask_again`).

Every status, diff and set of lists the window lets go of is handed to a worker to free
(`Request::Retire`): the refresh's answers and the lists drawn each hand their own hold over,
so the last hold on a status of tens of thousands of paths is never dropped on the UI thread.

## Acting on files

What a person does to the lists is reported by the component as a choice or a
`cairn_ui::ListIntent`, and decided by `crates/cairn-app/src/local_changes/actions.rs`; the
component decides nothing about what is staged.

**The selection** (R8.1, the redesign's rule 8 and L1; `cairn_ui::LocalSelection`, owned by
`LocalChangesState` alone): one value — the list it is in, its paths (never none, kept in the
lists' own order, `cairn_model::path_order`), a primary among them and an anchor — or none,
which is the only way nothing is selected. A plain press, or ↑ or ↓ (from the primary), makes
its row the selection, primary and anchor; a press with the table's extending press held (⌘- or
Ctrl-click, `Action::ExtendSelection`, resolved from `HeldKeys` since a pointer event carries no
modifiers here) toggles a row: toggled in, it is where a range extends from next and the primary
stays **the file clicked first** (observed in Fork for Windows 2.21.1, `lc2`); toggled out, the
primary or the anchor moves to the nearest path still selected — the one after it, else the one
before — and the last path toggled out leaves nothing selected. A press with the range press
held (Shift-click), or Shift+↑/↓ on the focused list, selects every row the list shows from the
anchor to it, that row the primary. However many paths are selected the diff shows the
primary's alone. A row is drawn selected by a binary search as it is built;
nothing else is drawn selected. Selecting in the other list starts a selection there.

**Where the selection goes** (R8.3, the redesign's L5 and L3, C25; `crate::next_selection`, a
pure function): the selected paths the lists still show, and an action did not take, stay
selected, the primary kept or moved to the nearest of them and the anchor kept or moved to the
primary — so a refresh with nothing changed leaves the selection, its primary and its anchor
exactly as they were, neither copied nor written (`next_selection::Next::Unchanged`). When none stays, the row that took the first selected path's place is
selected — the first row shown at or after it in the lists' order, past any the action took,
found by a binary search (`LocalChanges::row_from`, `ShownFiles::row_from`), so it is the right
row however many rows above it went too — else the nearest row above; with no row left in that
list, nothing, as Fork leaves the diff blank once the last file is staged (Tracker #1663). With
nothing selected, the first row shown is selected only when the lists before showed none — the
first status, files appearing. It runs after an action at once, with the paths the action took
leaving (`LocalChangesState::acted`, `next_selection::Leaving`; Stage All and Unstage All as
every row of their list, named without walking it), so the next file's diff is asked before the
status that shows the write arrives — whose own turn of the rule then changes nothing — and as
each status and each filter answer arrives, with none leaving. Whoever moved the files — a
stage, a commit made in a terminal (observed `lc5`: f5 committed, f6 selected), a filter — it
is the same rule. The filter prunes the selection to the rows it shows (the redesign's L3), so nothing hidden
is ever acted on; while the filter's first rows are on their way the selection is left as it
was, and nothing is acted on meanwhile, since an action takes only the rows shown.

**Which paths an action takes** (rule 8: every route acts on the selection, wherever focus
is): the selection's paths the lists drawn still show — each found by a binary search —
and nothing else; nothing while nothing is selected. A whole-file
action names a rename's source beside its path (`LocalChanges::whole_file_paths`), so a staged
rename unstages whole (`git reset -q -- <old> <new>`) while a rename's source listed as a row of
its own resets alone. Every path is one `git status` listed: the engine trusts its caller with
that (`staging.md`, Residuals). A collapsed untracked-directory row — the only one status lists
whole, a nested repository — stages as `git add` takes it, a gitlink at its commit, and nothing
under it (`a_nested_repositorys_row_stages_exactly_what_git_add_adds_for_it`).

**Fork's five routes** (R8.2), each asking `LocalWrite::StageFiles` from Unstaged or
`LocalWrite::UnstageFiles` (back to `HEAD`) from Staged through `local_writes::ask`:

- a double press on a row — the first press makes the row the selection, and the double press
  acts on the selection, so it stages that row;
- the table's chords on either focused list: Enter, Ctrl+S or Ctrl+Shift+S (Return or ⌘S) on
  the selection, wherever it is, and, for the list heard, Ctrl+Alt+Shift+S (⌥⇧⌘S) — heard by
  the lists' own key handler (`Scope::LocalChanges`, and the bare Enter from
  `Scope::LocalChangesLists`, the lists' alone) and, but for Enter, the diff's, so nowhere else,
  and never from a text field, which keeps Enter and Backspace;
- a drag from one list released over the other (`cairn_ui`'s `local_changes::drag`): tracked
  by the two lists together, never a row, with the path it began on, so it survives its row
  unmounting or a status replacing the lists; a release over the same list, outside both, or
  after Escape drops nothing; a press heard while a drag is on (its release made where the
  window could not hear it) or the window losing focus ends it — a press over the lists or
  their filter by the platform's mouse-down, which a text field cannot cancel, and elsewhere
  by the global pointer-down; while it is on, the list it
  would drop on scrolls at its edges (`EdgeScroll`); begun on a selected row it drops the
  selection, and begun on a row the selection does not hold it makes that row the selection
  as it drops, as a native list's drag carries the row it began on, and drops it;
- each list's button — Stage or Unstage on the selection, greyed while the selection is in the
  other list or there is none (rule 8, Fork's TrackerWin #2429) — which, while the table's
  heading press is held (`Trigger::HeadingPress`, `HeldKeys::heading_press`: Shift on Linux, as
  Fork for Windows, TrackerWin #2429 and #2466, and ⌥ on macOS; the redesign's L2, superseding
  Alt, which xfwm, openbox and Plasma 5 take with a click), reads Stage All in Unstaged and
  Unstage All in Staged, both enabled whatever is selected while the list shows a row; and the
  small double chevrons always drawn, ⇊ Stage All in Unstaged's heading and ⇈ Unstage All in
  Staged's (a deviation: Fork for Windows shows Unstage All only while Shift is held). The
  heading press is told apart from a row's: Shift on a row is still the range press, and a held
  Alt does nothing on Linux;
- the context menu (`local_changes::menu`): Stage or Unstage, Discard Changes… (greyed on the
  staged side, its reason beneath it), Stage All or Unstage All, Copy Path; a right-press on a
  row outside the selection chooses it first; each item closes the menu as it is chosen.

Stage All and Unstage All ask `LocalWrite::StageAll` or `LocalWrite::UnstageAll` with the
lists drawn, shared, and — with a filter on — the rows it shows (the user's decision,
2026-10-09): every row with no filter, and only the rows shown with one, so a change the filter
hides, a conflicted one among them, is never staged or unstaged unseen; nothing while the
filter's rows are on their way. Their paths are gathered on the local lane, never on the UI
thread, which copies at most the filter's row indices.

**After a stage or an unstage** (R8.3, the redesign's L5) the selection moves by the rule above, the acted
paths leaving — the row that slides into the first acted row's place, else the nearest above it
(observed `lc4`: f2 and f4 staged, f3 selected) — and its diff is asked at once; with none left,
nothing is selected. After Stage All or Unstage All a selection in that list goes, and one in
the other stays. The same holds after a confirmed discard.

**A discard** (R8.4) is offered on the unstaged side only, and never takes a submodule (R8.8),
a conflicted path (R8.7) or a nested repository — an untracked row git lists as its directory,
with a trailing `/` (`LocalChange::is_nested_repository`; the user's decision of 2026-10-10,
where Fork deletes one and its history, `fork-observed-nested-repo-2026-10-10.md`). A
selection with nothing that can be discarded — any on the Staged list or a staged diff, or
every row one of those three — asks nothing, and the discard chord over it does nothing, as
Fork's does, with no line under the lists (the redesign's rule 4); the menu draws its Discard
Changes… greyed with the reason wrapped beneath it, never cut (`cairn_ui::NoDiscard`): "Staged
changes can't be discarded. Unstage them first.", or one sentence per kind the selection holds,
submodules first — "Submodule changes can't be discarded.", "Conflicted files can't be
discarded. Resolve or stage them first.", "Nested repositories can't be discarded." A
selection mixing them with paths that can be discarded asks for those paths alone, and the
confirmation says in one line under its prompt what is left — "1 submodule and 1 conflicted
file are left as they are.", "1 nested repository is left as it is." (`cairn_ui::kept`,
`cairn_ui::Left`, `Confirming::leaving`; the redesign's D1, where Fork asks twice) — the button
counting only what it discards.

**The dialog opens at the press** (R7.4, the redesign's rule 3): the discard chord (Backspace,
Delete or Ctrl+Shift+D; ⌫ or ⇧⌘D), the menu or a chunk's or a selection's Discard… opens the
confirmation at once, counting (`Confirming::counting`, `cairn_ui::DialogAnswer::Counting`):
"Counting…", its button greyed and reading "Discard Changes", Cancel focused — and asks the
engine what it would lose: `Request::DiscardConsequence`, computed by
`ops::discard_files_consequence` on the local lane, in the order asked, so after every write
asked before it, or `Request::DiscardLinesConsequence` for lines. The count is per path (a `git
diff-files` read for each tracked file) and holds the local lane while it runs, so it is
numbered in a lane of its own: a newer discard asked, Cancel while it counts
(`Confirming::on_let_go`, which asks `Request::StopCounting`), the view being let go of and a
close each end it, between paths or by ending its read, and it then answers nothing. The answer
for the discard asked last fills the same dialog (`local_changes::actions::consequence_arrived`,
by the confirmation's serial): the prompt, the line of what is left, "Show files" for a discard
of several — its caption kept, its glyph turning ▸ to ▾, a virtualized list of each path and
what happens to it (`Consequence::listed_file`: "14 lines", "Binary", "Restored", "Deleted",
"Emptied", "Mode change", "14 lines and mode change") — and the button its own caption; its
token asks `LocalWrite::DiscardFiles` or `LocalWrite::DiscardLines`. An answer for an earlier
ask, or for a confirmation let go of, is dropped. A refusal the engine makes while counting —
a file that changed while it was read, a path that is not a file — makes the same dialog the
refusal (rule 4): "Couldn't Discard Changes", the engine's one sentence
(`worker::local::lane::refused_while_counting`, worded as every ending is, its path C-quoted by
`cairn_model::shown_path`) and "Nothing was changed.", Close alone. No modifier, setting or route reaches a discard without the dialog.

**A conflicted row** (R8.7, C24) stages whole by every route — `git add`, which marks it
resolved — and is never discarded. As in Fork, nothing more is said before it is staged (the
user's decision, 2026-10-09): staging it is the one way Cairn resolves a conflict, and Cairn
offers no way back to the conflicted state — git's own `git checkout -m <path>` is it.

**While a dialog is open** — a credential prompt, a confirmation, a Git Error or "Couldn't
<name>", Create Branch, or the activity popover, whichever the one modal state puts on top
(`crate::modal`) — nothing the lists or the diff report is acted on, and the window's chords are
inert (`shortcuts::keys_inert`). A credential prompt that arrives while a confirmation is open
(a write running behind it asked for a secret) is drawn over it, the confirmation kept
unanswered and undrawn, and drawn again once the prompt is answered or refused.

**The diff hears the same chords but Return** (`Scope::LocalChanges`; R7.3 as amended
2026-10-10, C25): on the lines a drag across it selected ("The diff's staging gesture", below),
or with none on the selection, whole — the paths of it the lists show, as the chord on a list
takes them (`local_changes::actions::on_the_diff`, `selected_rows`): Ctrl+S (⌘S) from the diff
stages every selected file and the selection moves to the row in the first acted row's place
(observed `lc4`), while a bare Return there does nothing (`lc3`), the lists alone hearing it
(`accelerators::chord_heard_in`). Hovering is not selecting (Fork, Tracker #103): a chunk under
the pointer changes nothing the chords do.

## The diff's staging gesture

In Local Changes' diff alone (R9), Fork's gesture (`fork-detail-and-diff-ui.md`, Finding 23),
built in `crates/cairn-ui/src/diff/staging_gesture.rs` and handed to the primary's `DiffView` as a
`cairn_ui::Gesture`: the side it is over (`GestureSide`, from the list the path was chosen in),
the content of the diff drawn (`ShownDiff::content`), the drag the window keeps
(`LocalChangesView::lines`, a `cairn_ui::LineDrag`) and where its actions go
(`local_changes::actions::on_gesture`). The Commit and Changes tabs hand their diff view none,
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
from that row to the row under the pointer, and — side by side — to the column it began in
(R9.3, `SideColumn`). It is the list's, never a row's: the root hears
every move and the release, so it carries on across rows the virtual list unmounts, and the
list scrolls itself at its edges (`EdgeScroll`). What it selects is read from the layout when it
ends (`UnifiedLayout::selection_in`, `SideBySideLayout::selection_in`): the changed lines in the
rows spanned and no others, found from the change stops, so its cost is the lines selected and
never the rows spanned, and a selection reaching thousands of rows below is whole. A press
without a drag selects nothing and lets a selection go, as does Escape. A release the window
never hears ends the drag at the next press; losing focus lets it go. The selection's actions
are drawn at its top right, counted (`Stage 2 Lines`, `Unstage 1 Line`, `Discard 2 Lines…` —
whose confirmation's button reads `Discard 2 Lines`; the user's decision, 2026-10-09), and
the chords act on it too.

**A selection belongs to the text it was made over** (R9.6, the redesign's L4, C26 — a deviation from Fork,
which drops it on every refresh, observed `ln2`). Every working-tree answer is keyed on the diff
thread with the question it answers (`ShownDiff::keyed` with the `FileQuery` — the path, the side
and the options — in `worker/diff/lane.rs`): its content (`ShownDiff::content`) is a hash of that
question, the context and the whole answer — the paths, modes and ids, every line of both sides
and the ranges that differ — so two answers with the same content draw the same rows over the
same text. A selection records the content it was made over (`LineDrag`), the view is handed the
content it draws, and a selection over other content is nothing. So a refresh — switching to the
editor and back with the file untouched, which reads the status and the primary again — draws
a re-read of the same content and keeps the selection, its tint and its actions, its old diff
drawn meanwhile; one whose text differs by a byte, anywhere in the file, leaves it nothing, and
the window lets it go as the answer arrives, the diff's bar saying "The file changed — line
selection cleared." (`cairn_ui::SELECTION_CLEARED`, `session::apply`'s `lines_follow`) until the
next key or press anywhere in the view (`local_changes::pane`'s global listeners). Another path,
or the same at other options, lets a selection go silently. A drag still in progress is
cancelled when the window loses focus, as before. Every act carries the content it was made
over (`GestureAct::content`), and `local_changes::actions::on_gesture` refuses one whose content
is no longer drawn, so an action built before a redraw never sends old rows' lines against a
new diff (`an_act_made_under_an_answer_no_longer_drawn_asks_nothing`); the engine's stale check
backs it. The diff handed to a write is the answer drawn, shared (`ShownDiff::shared_diff`),
never copied on the UI thread.

**After an action** the actions hide until the diff is drawn again or the pointer moves, so a
second press cannot act twice on rows the first already took (Fork, Tracker #480); then they
follow whatever the new rows put under the pointer, or none.

**What each action asks** (`local_changes::actions::on_gesture`): the diff drawn, copied, with
the selection — `LocalWrite::StageLines` from the unstaged or untracked diff, `UnstageLines` from
the staged one, applied by the engine's patch (`staging.md`); the selection then goes. A
discard is refused in the view for a submodule or a conflict (`cairn_ui::no_discard`); every
line of a new file is the file — its discard deletes it — so it asks what discarding the file
would lose (`Request::DiscardConsequence`, the files' route, since the engine refuses it as lines,
`Refusal::WholeFileOnly`);
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
state (`crates/cairn-app/src/commit_box/state.rs`) by `commit_box::pane::CommitBoxPane`, which
also holds what each press does: a one-line subject with Fork's counter — the characters left
before 50, negative past it, red past 70 (`subject_count`) — and the `≡` that opens Recent
Commit Messages; a description in IBM Plex Mono with a ruler at column 72; `Amend` at the left;
and one button at the right — `Commit N Files` (`commit_caption`: plain `Commit` with none), or
Fork's "Amend Last Commit" while Amend is ticked (`AMEND_BUTTON_CAPTION`, the user's answer of
2026-10-10) — with nothing under it (R10.6, the redesign's C6); the commit chord presses it, in every state. At
most one line is drawn, over the fields, wrapped and never cut (`operation_line`, R10.8, the redesign's C8
and rule 6).

**The draft is the window's** (R10.7). The subject and the description are two `State<String>`s
kept by `LocalChangesView::commit` for the window's life, bound two ways to the fields — so a
refresh, Local Changes hidden and shown again, a failed hook and Amend's toggling leave it as it
was, and what the window writes into it (a recalled message, `HEAD`'s, `MERGE_MSG`) is what the
fields show. Both fields are built by the one key policy heard in the commit box's scope
(`text_field_in`, and for the subject `text_field_recalling`, `FieldScope::CommitBox`): the
commit chord (⌘Return, Ctrl+Enter) presses the button without a new line, the window's chords
reach the window, and no list or staging chord fires (R7.3 as amended). Tab moves from the
subject to the description. A key that changes either field's text is the person's edit
(`CommitBox::on_edit`, heard through the field's validator), and so is a recalled message: the
draft is then theirs (`commit_box::state::CommitBox::touch`). **One rule fills it** (the redesign's C7, Fork's
developer's rule, Tracker #61): `HEAD`'s message as Amend is ticked, and a merge's, single
cherry-pick's or revert's message, go only into an empty draft the person has not touched — so a
fill the person cleared stays cleared. A commit made clears the draft it took, untouched again;
one that fails keeps it.

**What is asked, and where** (`docs/systems/git-processes.md`, "The local write lane"). As the box
is shown, and as each refresh's refs arrive while Local Changes is shown, the box asks its reads
(`Request::CommitReads`, answered by `Update::CommitReads`): the operation in progress, the
message git prepared for the commit that concludes a merge, a cherry-pick or a revert — its
`MERGE_MSG` as git's own editor session would leave it under the repository's `commit.cleanup`
and comment character — and the last ten
messages — on the local lane, after the writes asked before them,
numbered in the commit-box lane so the next ask supersedes the last. What an amend costs is
never read here, nor on a status: it is read at its press (rule 3). Nothing is read on the UI
thread.

**Commit** asks `LocalWrite::Commit` with the draft composed as typed — the subject, then a blank
line and the description when there is one (`compose_message`; git's own cleanup is git's,
R6.1). The button needs a subject (Fork's rule) and something staged, or a merge, single
cherry-pick or revert in progress, which it concludes. **While it is asked** the button's place
says what it is doing (`cairn_ui::Busy`, R10.4, the redesign's C5): queued behind another write, "Waiting for
<that write's one name>…" ("Waiting for Stage 1 File…") — the one form every wait uses; running,
"Committing 0:03" (or "Amending"), redrawn each second by a task of its own line's scope
(`Elapsed`, on `async-io`'s timer; freya drops a scope's tasks as it unmounts), with a Cancel that
asks `Request::CancelWrite` for that commit's id — never for one still queued (R4.3); asked with
no write ahead of it but not yet started (the lane busy with a read), its verb alone, with no
Cancel until git runs; cancelled, "Cancelling…" until git is reaped. "Waiting for" names only a
write ahead of it, never one asked after. A cancel opens no
dialog and keeps the draft (rule 4).

**A failure** of git's own (`WriteEnding::Failed` with the command that ran) opens Fork's Git Error
dialog (`cairn_ui::GitErrorDialog`, R10.5) over the window — the commit box's own, its skip and
token kept with it, and first among the endings' dialogs in the one modal state: the command, and the commit's output
as it streamed (drained from the local lane's tap on each `Update::WriteOutput`) — or, if nothing
streamed, the output the engine kept — with ANSI sequences and control characters stripped
(`strip_ansi`), the latest lines kept in the one bounded tail (`crate::output_tail::OutputTail`,
`OUTPUT_LINES` and `OUTPUT_BYTES`), drawn one row per line through a virtualizing view opened at its end.
In its footer, in the platform's order, are `Skip pre-commit hooks and commit` and Close — and
`Remove index.lock…` where git could not create the lock — one button component for all
(`answer_button`), focus on Close — the skip offered on every failed
commit or amend (the redesign's C4: Cairn keeps no model of which hooks git would run, and where no hook ran
the skip fails the same way again, harmlessly) and never for one whose hooks were already
skipped. It answers once, asking the same message again with `--no-verify` for that one commit
or amend: a commit's as a commit; a confirmed amend's under the token its failure handed back —
the engine's `Error::AmendNotMade`, carried on the ending as `worker::Unspent` and kept by the
Git Error state (`GitError`) until the skip spends it or Close lets it go (R1.1's option (a), the
user's answer of 2026-10-10) — so nothing asks twice; an amend at the press's at the press again,
which reads its cost anew. No prompt is drawn in the dialog. The next commit runs its hooks.
Close, Escape or a press outside closes it. The window's chords and Local Changes' actions are
inert while it is open, as under a confirmation. A refusal, a stale amend, a failure beneath git
or a commit not run opens "Couldn't <its name>" ("Couldn't Commit 1 file", "Couldn't Amend")
with its sentence, the window's endings' dialog; a cancel opens nothing.

**Amend** (R10.3, R10.6, the redesign's C1, PRD L12). Ticking it sets the draft aside, with whether the person had
touched it, and makes amend's staged list an overlay over the lists (`LocalChangesState`;
`docs/research/staging-and-commit/review-code-app-ui.md`, M3): every status still lands in the lists' state, and while Amend is ticked each asks
amend's lists over it (`Request::Amending`, on the local lane, numbered in the amending lane):
Unstaged as the status lists it and Staged as amend's staged list against `HEAD`'s parent
(`LocalChanges::amending`, laid out on the lane), with `HEAD`'s message. Until they arrive the
lists drawn before stay drawn, so the selection never passes through the status's own Staged
list; a newer status supersedes the read before. An empty, untouched draft is filled with
`HEAD`'s message; a draft with text is kept as the amend's message. Unticking puts the draft set
aside back exactly as it was, its touched bit with it, ends amend's read in flight
(`Request::StopAmending`), and draws the newest status's own lists wherever they are not drawn
already (`unticking_amend_ends_its_read_and_draws_the_newest_status`).
Amend is greyed while any operation is in progress, the operation's line saying why, and on an
unborn branch, under "No commit yet, so Amend is unavailable." (`UNBORN_LINE`; R10.1, rule 4).

While amending, a path chosen in Staged asks its diff against `HEAD`'s parent
(`WorkingSide::Amending`, `WorkingTreeDiff::Amending`: `git diff --cached HEAD^`'s answer), and an
unstage of whole files from Staged — each route, Unstage All among them — puts the entries back
to the parent's (`UnstageTarget::Commit`, `git reset -q <parent> --`) or, for a root commit's
amend, removes them (`UnstageTarget::Nothing`, `git rm --cached -f -q --`): the file leaves the
amended commit (`local_changes::actions::unstage_target`). Lines unstaged by the gesture are
applied against the index as from any staged diff. Where amend's staged list cannot be read —
a partial clone's missing blob on git 2.44 and later — the box's one line says so and why
("Amend's staged list couldn't be read, so the lists show what is staged against HEAD: …"; on a
detached `HEAD` the detached line wins, saying what the press will do), the
lists are the status's against `HEAD`, and the amend itself is still offered.

**Amend Last Commit** is ready with a subject even with nothing new staged. Its press — or the
commit chord, which does exactly the same — asks `LocalWrite::AmendAtPress`
(`docs/systems/git-processes.md`, "The local write lane"): one job that reads what the amend costs
and runs an amend git logs and no remote has at once, with no token (the old commit stays in Show
Lost Commits), or ends `WriteEnding::NeedsConfirming` with the `Consequence`. On that ending the
box opens the confirmation dialog (`use_amend_confirmation`, the box holding the submitter its
answer needs) — once any confirmation already open is answered, never replacing it — titled "Amend Commit" (`AMEND_TITLE`), its words the consequence's fixed sentences
— "3f2a1c9 is already on origin/main. Amending it rewrites history others may have.", "3f2a1c9
can't be recovered after this: this repository keeps no reflog." — and its button "Amend", the
one confirmation surface for it (`CONFIRMATION_SURFACES`; the commit box builds no token):
Cancel returns to the box unchanged, and Amend asks `LocalWrite::Amend` of the message pressed,
under the token it built, which the engine re-checks before git runs. A commit made — or an
amend — clears the draft only where it still holds the message it took, so a draft typed while
it ran is kept; an amend unticks Amend. The way back to the replaced commit is the one its run
answered (`Done::replaced`), never one read from the request.

**An operation in progress** (R10.8, PRD L25, the redesign's C3 and C8): with a merge, a single cherry-pick or a single
revert, an empty, untouched draft is filled with git's `MERGE_MSG` as git's own editor session
would leave it under the repository's `commit.cleanup` and comment character (R6.10: under the
default, no `# Conflicts:` block and no scissors section), so what it shows is what is committed
— the merge commit, or the pick or revert concluded — under one line: "Merging. Committing
concludes the merge; Amend is unavailable.", "Cherry-picking a1b2c3d. Committing concludes it;
Amend is unavailable." (or "Reverting …"). During a rebase, `git am` or a sequence of picks or
reverts the fields and the button are disabled under one line naming git's command — "A rebase
is in progress. Continue or abort it with git rebase --continue or --abort.". With none, on a
detached `HEAD` one line reads "HEAD is detached: this commit will be on no branch.", asking
nothing (rule 2: the commit is recoverable).

**Recent Commit Messages** (R10.2): the `≡` opens a menu of the last ten messages' subjects,
newest first; choosing one fills both fields (`split_message`: the first line, and the rest after
its blank line). In an empty subject, or one holding the message last recalled, a bare ↑ recalls
the next older and ↓ the next newer, ↓ from the newest emptying the draft
(`accelerators::recall_step`, offered first by `text_field_recalling`); in a subject the person
typed, the arrows are the editor's.

## What enforces this

- The commit box, at the component (`crates/cairn-ui/tests/commit_box.rs`):
  `the_amend_toggle_reports_its_toggled_state_to_assistive_technology`,
  `amend_last_commit_reports_its_press_and_draws_no_line`,
  `the_counter_is_drawn_and_a_stopped_box_asks_nothing` (the line uncapped),
  `a_commit_asked_stands_in_the_buttons_place`, `an_edit_in_a_field_is_reported`,
  `the_git_error_draws_one_viewport_of_output_and_the_skip_only_where_offered`,
  `the_skip_sits_in_the_footer_beside_close_and_focus_starts_on_close`, and in
  `src/commit_box.rs` `one_line_names_the_operation_or_a_detached_head` and
  `the_elapsed_time_reads_as_the_button_says_it`; in the window
  (`crates/cairn-app/src/commit_box/tests.rs`):
  `a_draft_typed_then_amend_ticked_and_unticked_is_back_exactly`,
  `amend_last_commit_asks_the_amend_at_the_press_by_button_and_chord`,
  `an_amend_that_must_be_confirmed_opens_the_dialog_and_cancel_returns_unchanged`,
  `the_commit_chord_asks_nothing_while_the_box_is_not_ready`,
  `a_detached_head_commits_under_one_line`,
  `a_failed_amends_skip_amends_once_without_asking_again`,
  `a_draft_typed_while_a_commit_runs_outlives_its_ending`,
  `a_cancel_while_the_commit_is_queued_asks_nothing`,
  `a_failed_hooks_skip_commits_once_without_hooks_and_the_next_runs_them`,
  `every_failed_commit_offers_the_skip`, `cancel_reaches_only_the_running_commit`,
  `a_merge_fills_the_draft_with_the_cleaned_message_and_disables_amend`,
  `during_a_rebase_am_or_sequence_the_box_is_disabled_and_names_it`,
  `amend_is_disabled_on_an_unborn_branch`,
  `amends_staged_list_is_drawn_diffed_against_heads_parent_and_unstaged_to_it`,
  `amends_staged_list_unread_is_said_and_the_status_lists_stay`,
  `recent_messages_fill_both_fields_from_the_menu_and_the_arrows`; the window's state
  (`commit_box/state.rs`): `a_message_split_into_the_fields_composes_back_unchanged`,
  `the_output_kept_is_its_latest_bounded_tail`,
  `amend_sets_the_draft_aside_and_fills_only_an_empty_untouched_one`,
  `a_merge_fills_an_empty_untouched_draft`,
  `a_failure_offers_the_skip_unless_it_was_skipped`; through the real boundary
  (`worker/local/tests.rs`): `the_commit_boxs_reads_and_an_amends_read_come_through_the_lane`
  (no cost read for amend's lists, C14),
  `amend_pressed_runs_a_recoverable_amend_and_answers_any_other_for_the_dialog` (the replaced
  commit and the cost read after the reap on the ending),
  `a_confirmed_amend_a_hook_fails_carries_its_token_back_for_the_skip`,
  `a_failing_hook_fails_a_commit_the_skip_commits_past_it_and_the_next_runs_it_again` (its
  command and output, and the hook run again on the next commit); the seal's window rows,
  `the_window_holds_a_token_only_for_the_git_errors_skip` (`crates/cairn-guards`); the model:
  `amending_lists_amends_staged_list_beside_the_status_unstaged_one`,
  `an_amend_says_why_it_is_confirmed_in_fixed_sentences`,
  `only_an_amend_git_logs_and_no_remote_has_runs_unconfirmed`; the engine:
  `a_file_of_amends_staged_list_reads_as_git_diff_cached_against_heads_parent`
  (`crates/cairn-git/tests/diff_engine/diff/working_tree.rs`); the table:
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
  `only_the_platforms_key_turns_the_headings_into_stage_all`,
  `a_drag_drops_on_the_other_list_only_and_survives_its_row`,
  `a_drag_between_the_lists_scrolls_the_one_it_would_drop_on`,
  `each_rows_menu_offers_forks_items_and_no_discard_where_none_is_allowed`,
  `a_press_heard_mid_drag_or_focus_lost_ends_the_drag_without_a_drop`, and with every one of
  50,000 paths selected `a_status_of_50000_paths_all_selected_builds_one_viewport`
  (`tests/local_changes.rs`); the pure parts
  (`a_toggle_adds_a_path_and_the_file_clicked_first_stays_primary`,
  `the_primary_toggled_out_moves_to_the_nearest_path_and_the_last_leaves_none`,
  `a_range_spans_from_the_anchor_and_its_row_is_primary`,
  `a_selection_keeps_its_primary_and_anchor_while_they_stay`,
  `the_nearest_row_left_is_the_one_that_takes_the_first_acted_rows_place`,
  `a_press_becomes_a_drag_past_the_threshold_and_drops_on_the_other_list_only`,
  `a_discard_is_refused_only_where_nothing_can_be_discarded`,
  `what_a_mixed_discard_leaves_is_said_in_one_line`), and the edge
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
  `a_selection_survives_a_redraw_of_the_same_text_and_no_other` (C26's headless half; the
  content itself, `a_diffs_content_is_its_question_and_every_byte_of_its_answer` in
  `cairn-model`), and the viewport twin
  `the_gesture_builds_one_viewport_over_a_10000_line_diff`, named in the root `CLAUDE.md`'s
  virtualization invariant; in the window (`crates/cairn-app/src/local_changes/actions/tests/gesture.rs`):
  `a_hovered_chunk_stages_and_unstages_exactly_its_lines`,
  `the_chords_act_on_the_selection_and_on_the_whole_file_without_one`,
  `a_chunks_discard_confirms_its_lines_and_a_new_files_every_line_is_its_file`,
  `the_mode_row_stages_the_mode_alone`, `nothing_acts_on_lines_while_a_confirmation_is_open`,
  `an_act_made_under_an_answer_no_longer_drawn_asks_nothing`,
  `part_of_a_new_files_lines_is_discarded_as_lines`,
  `several_paths_selected_show_the_primarys_diff_and_its_chord_stages_them_all` (C25's diff
  half: the file clicked first, no count, Return inert in the diff, Ctrl+S staging the whole
  selection and the selection moved to the row in the first acted row's place),
  `a_line_selection_is_kept_across_a_refresh_of_the_same_text_and_cleared_once_otherwise` (C26
  through the window: a refresh's status, the re-read asked and answered, the same text and
  then other text) and its twin through a real refresh,
  `a_line_selection_survives_a_real_refresh_of_the_same_text_and_clears_once_otherwise` (the
  window over a real worker, `worker::StagedFile`'s a.rs edited in its working tree: a
  `Request::Refresh` of the same text keeps the selection, one after an edit clears it with one
  notice, each answer keyed by the diff thread itself),
  `a_renames_source_row_unstages_its_lines_at_the_new_path`; through the real boundary
  (`worker/local/tests.rs`), `a_chunk_drawn_at_context_ten_stages_exactly_as_drawn`; the
  prompt's one sentence
  frame, `a_discard_of_files_asks_in_the_one_sentence_frame`,
  `the_worst_loss_is_said_whenever_an_untracked_file_is_deleted` and
  `a_discard_of_lines_names_the_lines_the_chunk_or_the_mode`
  (`crates/cairn-model/src/consequence.rs`).
- Acting, in the window (`crates/cairn-app/src/local_changes/actions/tests/files.rs`):
  `the_chord_and_the_button_stage_a_selection_and_the_selection_moves_on`,
  `a_drag_the_menu_and_a_double_press_stage_what_is_selected`,
  `a_staged_selection_unstages_with_its_renames_source_and_all_takes_the_list`,
  `a_staged_selection_unstages_by_the_drag_the_button_and_the_menu`,
  `the_selection_moves_to_the_row_that_takes_the_acted_rows_place`,
  `stage_all_and_unstage_all_take_the_rows_the_filter_shows`,
  `a_refresh_or_an_emptying_toggle_leaves_nothing_selected_unseen`,
  `leaving_local_changes_ends_a_discards_count`,
  `a_discard_asks_what_it_would_lose_and_confirms_exactly_that`,
  `a_refusal_while_counting_is_said_in_the_dialog_and_cancel_ends_the_count`, the dialog's own
  (`crates/cairn-ui/tests/confirm_dialog.rs`:
  `while_counting_the_dialog_says_so_and_its_greyed_button_builds_nothing`,
  `a_refusal_is_the_same_dialog_with_close_alone`,
  `what_is_left_is_one_line_under_the_prompt_and_not_in_the_token`,
  `show_files_discloses_each_file_and_what_happens_to_it` and the viewport twin
  `show_files_over_50000_paths_builds_one_viewport`),
  `no_discard_reaches_a_staged_change_a_submodule_or_a_conflict_and_each_says_why`,
  `a_conflicted_row_stages_whole_by_every_route`,
  `local_changes_acts_on_nothing_while_a_confirmation_is_open`,
  `backspace_in_the_filter_edits_the_filter_and_discards_nothing`,
  `a_credential_prompt_sets_an_open_confirmation_aside_until_it_is_answered`,
  `local_changes_diff_is_always_exact_and_leaves_the_setting_alone`,
  `a_write_is_said_nowhere_under_the_lists_and_a_stale_one_in_its_dialog`,
  `a_discard_names_only_paths_the_lists_drawn_still_list`,
  `the_extending_press_toggles_and_the_diff_stays_on_the_file_clicked_first`,
  `every_route_takes_the_selection_and_nothing_the_person_did_not_select`,
  `nothing_is_acted_on_while_the_filters_rows_are_on_their_way`; the table's held key
  (`crates/cairn-ui/src/accelerators.rs`): `a_press_resolves_against_the_keys_the_window_heard`
  and `the_table_is_forks_chords_and_no_others`; through the real
  boundary (`worker/local/tests.rs`):
  `a_discards_consequence_is_counted_after_the_writes_asked_before_it`,
  `the_dialogs_count_is_what_the_discard_then_does_to_a_mixed_selection` and
  `a_newer_ask_or_a_stop_ends_a_discards_count_and_its_read`, with the lane's
  `an_all_gathers_the_rows_the_filter_shows_or_every_row`; the paths a
  whole-file action names, `a_whole_file_action_names_each_rows_path_and_a_renames_source`
  (`cairn-model`); and against real git,
  `a_nested_repositorys_row_stages_exactly_what_git_add_adds_for_it`
  (`crates/cairn-git/tests/diff_engine/diff/write_verbs.rs`).
- Drawing (`crates/cairn-ui/tests/local_changes.rs`):
  `both_lists_draw_their_paths_with_their_badges`, `each_list_is_in_its_paths_order`,
  `a_press_and_the_arrows_choose_a_row_of_its_list`, `the_filter_says_what_it_leaves`,
  `the_filters_count_is_of_distinct_paths_a_path_in_both_lists_once`,
  `every_kind_of_change_draws_its_badges_shape` (each painted badge's cell read pixel for pixel
  against its glyph), `the_splitter_between_the_lists_drags`, the badge rule
  `each_kind_of_change_has_forks_badge` (a unit test in `crates/cairn-ui/src/local_changes/list.rs`)
  and `every_glyph_paints_a_shape_no_other_glyph_paints` (`history/ref_glyphs.rs`), and the viewport twin
  `a_status_of_50000_paths_builds_one_viewport_filtered_or_not` (top, deep and end, filtered and
  not), named in the root `CLAUDE.md`'s virtualization invariant.
- The selection (`crates/cairn-app/src/diff/state/working.rs`):
  `a_path_in_both_lists_keeps_only_the_answer_for_the_list_chosen`,
  `a_refresh_asks_the_path_again_and_draws_its_last_diff_meanwhile`,
  `a_conflicted_path_asks_nothing_and_draws_its_notice`,
  `the_working_path_and_the_commits_file_take_the_lane_from_each_other`,
  `a_setting_asks_the_working_path_when_local_changes_is_shown`; the lists as kept
  (`crates/cairn-app/src/local_changes/state.rs`):
  `with_no_filter_a_status_is_drawn_as_it_arrives`,
  `with_a_filter_a_status_is_drawn_once_its_rows_arrive`,
  `clearing_the_filter_draws_the_newest_lists_whole`; where the selection goes
  (`crates/cairn-app/src/local_changes/next_selection.rs`, C25):
  `after_an_action_the_row_in_the_first_acted_rows_place_is_selected`,
  `after_a_refresh_the_paths_still_listed_stay_with_their_primary_and_anchor`,
  `a_vanished_selection_gives_its_place_to_the_row_that_took_it`,
  `a_filter_prunes_the_selection_to_the_rows_it_shows`,
  `nothing_selected_becomes_the_first_row_only_as_files_appear`,
  `a_status_that_keeps_the_selection_leaves_it_uncopied` (`local_changes/state.rs`), with a
  path's place in a list
  `a_path_is_given_its_place_in_a_list_listed_or_not` (`cairn-model`); the count
  (`the_count_beside_local_changes_is_of_distinct_paths`, `sidebar/pane.rs`).
- The window (`crates/cairn-app/src/local_changes/tests.rs`, headless, updates applied through
  `session::apply`):
  `local_changes_draws_its_lists_and_the_first_paths_diff_from_the_working_tree_query`,
  `a_conflicted_path_draws_its_notice_and_asks_nothing`,
  `a_path_in_both_lists_draws_each_lists_diff_and_the_answers_cannot_cross`,
  `a_refresh_that_removes_the_chosen_path_draws_no_stale_diff_under_no_row`,
  `typing_in_the_filter_asks_a_worker_and_the_lists_draw_its_answer`,
  `the_filters_count_is_the_sidebars_distinct_paths_a_path_in_both_lists_once`,
  `next_change_moves_local_changes_own_diff`, `a_status_that_could_not_be_read_is_said`,
  `a_filter_hiding_the_primary_selects_the_row_shown_in_its_place`,
  `a_refresh_moves_the_selection_and_asks_its_diff_without_a_render`; and
  `local_changes_and_all_commits_switch_the_main_region` (`sidebar/tests.rs`).
- The worker, through the real boundary (`crates/cairn-app/src/worker/refresh/tests.rs`):
  `untracked_files_under_a_new_directory_are_listed_one_per_file_and_counted`,
  `local_changes_filter_is_answered_on_a_worker_for_the_windows_own_lists` and
  `a_local_changes_filter_superseded_mid_pass_stops_and_sends_nothing`; the working-tree
  query itself, `a_working_tree_diff_answers_through_the_boundary` (`worker/diff/tests.rs`).
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
- The commit box: the subject is required, as Fork's is, a merge in progress commits with nothing
  staged, concluding the merge, as Fork's does, the cleaned `MERGE_MSG` fills an empty draft once
  per merge so a message cleared stays empty, and an amend whose staged list cannot be read is still offered (each
  the user's decision, 2026-10-09; evidence in
  `docs/research/staging-and-commit/fork-merge-and-amend-evidence.md`); Fork's limit setting, monospace toggle, Wrap paragraph at ruler, spell
  checking, autocomplete, commit template, `prepare-commit-msg` and AI drafts, sign-off, Commit
  and Push and the ⌘⇧A amend chord are not built (Commit and Push waits on push, PRD L9). The ruler
  stands at column 72 of IBM Plex Mono's advance after the field's margin.
- Amend Last Commit, and the commit chord while amending, amend at once where git logs the amend
  and no remote has `HEAD`; a commit a remote has, or one git keeps no reflog for, goes through
  the confirmation dialog first, where Fork for
  Windows warns that the commit is already pushed (its exact words are not recorded).
- A double press acts on its row alone, since its first press makes the row the selection (the
  user's decision, 2026-10-09).
- A finished line selection survives a refresh that re-reads the diff's text unchanged, where
  Fork's is dropped by every refresh (observed `ln2`); one the text changed under is let go of
  with "The file changed — line selection cleared." in the bar (R9.6, the redesign's L4, the user's kept
  deviation).
- Fork's floating actions sit at the chunk's top right; Cairn's also stand at the list's top while
  the chunk's top is scrolled above it, so a tall chunk's actions stay in reach. A selection's
  actions are drawn at its top, not at the chunk's. A press without a drag, or Escape, lets a
  selection go, and the window losing focus cancels a drag. (Each ratified by the user,
  2026-10-09.)
- The discard dialog, titled "Discard Changes" in Title Case where Fork for Windows writes
  "Discard changes" (the redesign's D3), asks in one sentence frame — "Discard all changes in
  31 files? 2 untracked files will be deleted. You can't undo this.", a chunk's "Discard this
  chunk (6 lines) in src/main.rs?" — where Fork asks "Do you want to discard all your changes
  in the selected files?" with no count but the button's (the redesign's D2; observed `dc1`).
- Stage All's double chevron sits in Unstaged's heading, as Fork for Windows draws it (Linux
  follows Fork's Windows rows); Fork for Mac draws it above the lists and flips it to Unstage
  All once nothing is left to stage. Unstage All has its own small chevron, ⇈, always drawn in
  Staged's heading, where Fork for Windows shows Unstage All only while Shift is held (the
  redesign's L2), beside the Staged button with the key held, the menu's item and the chord.
- The key held over the headings for Stage All and Unstage All is Fork for Windows' Shift on
  Linux (TrackerWin #2429, #2466) and Fork for Mac's ⌥ on macOS (the redesign's L2, superseding
  the Alt of 2026-10-09, which rested on a false Fork claim and which xfwm, openbox and Plasma 5
  take with a click to move a window); a held Alt does nothing.
- A discard asked on the staged side by the chord does nothing, as Fork's does; the menu
  offers its Discard Changes… greyed with "Staged changes can't be discarded. Unstage them
  first." beneath it (the redesign, rule 4).
- A selection mixing a submodule or a conflicted path with paths that can be discarded
  discards those, its confirmation saying in one line what is left, where Fork asks twice
  (the redesign's D1).
- A drag whose release was made outside the window, where it could not be heard, ends at the
  next press — but a press on another view's text field (the sidebar's filter) is not heard,
  since the field cancels the global pointer-down as it takes it, so a release over the other
  list after it still drops.
- The menu offers Stage or Unstage, Discard Changes…, Stage All or Unstage All and Copy Path:
  Fork's Open, External Diff, Show in Files, Blame, History, Ignore, LFS, Stash and Save as
  Patch are not built; its items name no chord, since a chord spelled in a label is a modifier
  named in a component; nothing opens the menu from the keyboard.
- A submodule's row offers no discard (Fork's Discard Submodule Changes) and says why; a
  nested repository among untracked rows is left as it is, where Fork deletes it and its
  history (the user's decision of 2026-10-10); the confirmation's default is Cancel
  (PRD L8, L24).
- The confirmation opens at the press, "Counting…", where Fork's opens once it has its words,
  and a refusal while counting is the same dialog, "Couldn't Discard Changes" (the redesign's
  rules 3 and 4).
- Ignore Whitespace is disabled in Local Changes and the diff drawn is always the exact one,
  where Fork lets it hide changes it then stages (R8.5, PRD L6).
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
