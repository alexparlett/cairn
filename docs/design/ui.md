# UI design

Intent, not as-built. The mockups are `docs/design/mockups/cairn-ui.html` — five
screens, example data, dark-first because the application is. Where a mockup and
a design document differ, the document is the design: the detail pane and the
diff view in particular are designed in `diff.md`, and the mockup's fixed-height
pane, tab count, `+N` file counts, banded hunk header with buttons, checkbox
column and line-selection gutter, transparent diff washes, operation-log drawer
and the discard dialog's stash-first option are not part of it.

Modelled on Fork, deliberately: it is the client the user reaches for, and its
layout is the thing worth keeping. What changes is driven by the design, not by
taste — every deviation below names the document that drives it.

## The layout model, kept from Fork

Three regions, always:

```
┌──────────────────────────────────────────────────────────────┐
│ toolbar: Fetch · Pull · Push · Stash │ repo · branch │ actions │
├───────────┬──────────────────────────────────────────────────┤
│ sidebar   │ main: history graph  OR  local changes           │
│ refs      ├──────────────────────────────────────────────────┤
│           │ detail: commit / changes  OR  diff               │
└───────────┴──────────────────────────────────────────────────┘
```

Kept, and why:

- **"Local Changes (N)" is the first sidebar entry**, with its count. It is the
  daily entry point; the number is the first thing a person wants to know.
- **The graph is the main view** when a repository opens (`history-graph.md`).
- **Staging is one screen**: unstaged and staged lists, the diff of the selected
  file, and the commit box, without navigating away.
- **Stashes appear inline in the commit list**, as well as in the sidebar: each
  stash made on a commit the graph shows is a row of its own, placed by date, on
  a short lane joined to that commit (`history-graph.md`); one whose commit no
  ref reaches is in the sidebar alone, and pressing it shows its changes. Fork's feature list calls this out for a
  reason.
- **Repository tabs** across the top. The manager behind them is open
  (`cairn.md`, "Still open"); tabs are the least-committing surface to show.
- **A detail pane below the graph**, behind a draggable splitter, with Commit and
  Changes tabs (`diff.md`).
- **Dialog buttons in the platform's order** (the user's decision E): on Linux
  Fork for Windows' — the primary button first, Cancel (or Close) last; on macOS Fork for
  macOS' — Cancel first, the primary last. Every Cairn dialog follows it: the confirmation,
  the Git Error dialog, Create Branch and the credential prompt (`cairn_ui::button_order`).
  Which button holds focus is not the order's: a destructive confirmation starts on Cancel on
  both platforms (below).
- **A right-click selects the row it lands on** (the user's decision C; Fork for Windows), so
  the detail pane follows and the menu acts on what is shown; a right-click inside a selection
  already made — a compared pair in the history, several paths in Local Changes' lists — keeps
  the whole selection. Every list row's menu does this.

## What changes, and why

| Change | Driven by |
| --- | --- |
| An operation that cannot be undone, or that rewrites history someone else may have, always gets the **one confirmation dialog**, whichever route asked — button or chord — and an operation that can be recovered from asks nothing. Its words are rendered from what the engine computed at the press and are the `Confirmed` prompt. | `cairn.md`, what Cairn is; `engine.md`, "The confirmation seal" |
| Every confirmation starts with focus on **Cancel**, and Escape goes back one step. Fork's discard dialog focuses its Discard button, and its own tracker asks for Cancel (Tracker #1080). | `cairn.md`, what Cairn is |
| A confirmation's prompt is **one short sentence frame** — the question naming what is discarded, the worst loss only when it is worse than changes, and "You can't undo this." — with the per-file detail behind **Show files**, where Fork asks without detail. Titles and buttons are in Title Case on both platforms, where Fork for Windows writes "Discard changes". | `engine.md`, "The confirmation seal" |
| A discard over a selection that mixes discardable files with a submodule or a conflicted file **discards what it can** and says in one line what is left; Fork asks twice. | `engine.md`, the write verbs |
| **Each kind of message has one home**: progress in the title bar's status box, a git failure in Fork's Git Error dialog, a refusal found while running in the same dialog titled "Couldn't <name>" with one sentence and no Error Details, a refusal known in advance as a greyed control with its reason beside it; success says nothing, a cancel opens no dialog, and nothing is drawn under the file lists. Fork calls every failure "unexpected", its own refusals included. | `concurrency.md`, "Operations"; `processes.md`, "Cancellation" |
| **No text a person needs is cut.** A sentence that could run long is made short instead — it names only the exception and never repeats what is on screen — and per-file detail goes behind Show files. | `cairn.md`, what Cairn is |
| The **activity popover**, Fork's Activity Manager, also quotes the prompt an operation confirmed, whole, gives each entry its duration, and offers "Show in Lost Commits" for an amend's old commit. Its times are UTC, since local time needs a timezone dependency; Fork's are local. | `engine.md`, "The confirmation seal"; `feature-inventory.md`, "Recovery" |
| **Ignore Whitespace is disabled in Local Changes**, so every change drawn there is one a gesture can stage. Fork offers it there. | `diff.md`, "Selections and patches" |
| With several files selected, the diff's bar says **how many are selected**; Fork's draws no count. | `cairn.md`, what Cairn is |
| A **visible double chevron in each list's header** — Stage All in Unstaged, Unstage All in Staged — beside the held key; Fork for Windows shows Unstage All only while Shift is held. | `platform.md` — keyboard-first, every action reachable |
| A finished **line selection survives a refresh** whose re-read draws the same text — switching to the editor and back — and is cleared, with one notice, when the text changed; Fork drops it silently. | `concurrency.md` — a refresh is frequent and must cost the person nothing |
| The floating actions **stay at the list's top** while the chunk's top is scrolled above it, so a tall chunk's actions stay in reach; Fork's sit at the chunk's top right. | `cairn.md`, what Cairn is |
| A press without a drag, or Escape, **lets a line selection go**, and the window losing focus cancels a drag; the selection's discard reads `Discard 2 Lines…`, its ellipsis saying it confirms. | `diff.md`, "Selections and patches" |
| The amend button **names the commit** it amends, and an amend asks first — in the one dialog — exactly when a remote already has the commit or git keeps no reflog to recover it from. Fork's button reads "Amend Last Commit"; Fork for Windows notes a pushed amend beside the check box and amends at once, Fork for macOS says nothing. | `engine.md`, "The confirmation seal"; `feature-inventory.md`, "Recovery" |
| A running commit has **Cancel beside it** in the box, so a hung hook is stopped where it was started; Fork's cancel is only in the Activity Manager. | `processes.md`, "Cancellation" |
| A failed commit or amend always offers **Skip pre-commit hooks and commit**, where Fork offers it where it finds a hook; hooks git finds by configuration Fork misses (Tracker #948). | `engine.md`, D1 — git decides, Cairn does not model it |
| A merge message is shown **as git's editor would leave it** — comment lines stripped by git — so what is shown is what is committed; Fork commits git's `# Conflicts:` lines, its open bug (Tracker #180). | `engine.md`, D1 |
| A leftover `index.lock` is **a state of the repository**: a banner while the file exists, no `git` of Cairn's runs and it is older than a few seconds, offering its removal — which, from there or from a Git Error, is confirmed. Fork offers the removal only in the failure, and removes unasked. | `processes.md`, "Cancellation"; `engine.md`, D1's one deletion made without `git` |
| A nested repository among the untracked rows is refused before any dialog, since deleting it deletes history no prompt can count. | `engine.md`, the write verbs |
| A submodule row's discard is greyed and says why; Fork offers `Discard Submodule Changes`. `git restore` leaves a submodule's commit where it was, and no prompt can count what is dirty inside it. | `engine.md`, the write verbs |
| During a rebase, `git am` or a sequence of cherry-picks or reverts the commit box is disabled and names git's own command to continue or abort; Fork pre-fills and commits. Continuing those operations is their own design. A single cherry-pick or revert is concluded by Commit, as in Fork. | `cairn.md`, the milestone — rewrites are the second lap |
| **Show Lost Commits has a control in the history's toolbar area** — a check box at the right end of the "Graph and subject" heading's cell, its tooltip the chord — as well as its chord; Fork has only the View menu item and the chord, and refused a toolbar button (TrackerWin #378). | `feature-inventory.md`, "Recovery" — the reflog must be visible to be usable |
| Create Branch's **"Stash and reapply" is drawn greyed**, saying it comes with stashing, where Fork's acts. | `feature-inventory.md` — stash is its own feature |
| Show Lost Commits reaches the commits `HEAD`'s and the local branches' reflogs hold; Fork's mode reaches every reflog, as `git log --all --reflog` does (TrackerWin #1307). | `history-graph.md`, "What it walks" |
| **Worktrees are a sidebar section**, as Fork's are, and a branch checked out in another worktree carries a chip as in Fork — and, unlike Fork, a disabled checkout. | `worktrees.md` |
| Pressing a ref finds its commit however deep it is; Fork holds only its newest 50,000 or 100,000 commits and does nothing for a ref past them. | `history-graph.md` — a history is however long the repository is |
| Remote-tracking labels carry a generic remote glyph wherever the forge is not known; Fork shows a forge icon on every one. | `forge-links.md` — a forge is identified only where it can be |
| Branch and commit context menus carry **forge links**: create pull request, open on the forge, copy permalink. After a push, the toast offers *Create pull request* directly. | `forge-links.md` |
| **Conflict resolution is three-way and region-level**, with *edit in your editor* as the escape hatch. Fork resolves per file and opens a resolver for the rest. | `conflicts.md` |
| The toolbar drops Fork's Appearance / Workspace / Feedback and adds *Open in terminal / editor* and a command palette. | `feature-inventory.md`, Tier 7 |
| No Accounts section in any sidebar. | `credentials.md` — Cairn holds no credential |
| A visible **loading state** distinct from an empty repository. | `concurrency.md` — every query is assumed slow |
| Linux-native chrome: no traffic lights, client-side decorations with controls at the right, keyboard-first throughout. | `platform.md` |

## The sidebar, the toolbar and Local Changes

The sidebar is Fork's, top to bottom: **Local Changes** with its count (the
number of distinct paths `git status` reports, absent when there are none),
**All Commits**, a filter box, then Worktrees, Branches, Remotes, Tags, Stashes
and Submodules. Branches and remotes are folders split at `/`; the current
branch is ✓ and bold; a branch shows its ahead and behind counts against its
upstream, or that the upstream is gone. Pressing a ref selects its commit in the
graph and scrolls to it, finding it if it is not loaded yet; it never filters.
Every section is a virtualized list, since a repository can carry tens of
thousands of tags.

The title bar names the repository — marked `*` while the working tree has
changes — the current branch, and its behind and ahead counts, as Fork's central
status box does. While an operation runs, the status box shows it instead: a
spinner and the operation's name, once it has run a quarter of a second, so a fast
stage shows nothing, and how many wait behind it; a fetch adds git's progress line,
and Fetch is greyed while one runs. Pressing the box opens the activity popover.

Pressing Local Changes puts the staging screen in the main region, as Fork lays it
out: a filter over Unstaged above Staged, each a list of paths with Fork's badges
under a header carrying its list's `Stage` or `Unstage` button and a small double
chevron — Stage All in Unstaged, Unstage All in Staged — and beside them, on the
right, the diff of the selected file with the staging gesture on it and the commit
box under it ("Staging", "The commit box", below). As built:
`docs/systems/local-changes.md`.

**Refresh** is Fork's: refs and status are re-read when the window gains focus,
after Cairn's own operations, and on the Refresh chord. After an operation only
what it changed is re-read, and while a commit's hooks run Cairn starts no refresh
of its own (`concurrency.md`, "Operations"). Nothing watches the file
system. Fork's standing complaint — that each refresh holds `index.lock` for
seconds on a large tree — does not carry over, because Cairn's reads take no
lock (`engine.md`).

Two banners can stand under the title bar, both about the repository or the window
rather than an operation: a leftover `index.lock` ("The leftover lock", below), and
a close waiting on a write — "Closing after Commit finishes. Close again to quit
now."

## Messages

Each kind of message has one home, and each operation one name — Fork's
imperative, "Stage 2 files", "Commit", "Create branch 'topic'" — used everywhere
the operation is named: the status box, "Waiting for Stage 2 files" on a control a
queued write holds, the closing banner, a dialog's title and the activity popover.

- **Progress** is the status box's.
- **A git failure** opens Fork's Git Error dialog: its title and sentence, `Error
  Details:`, the command and git's output, scrolled to its end, for every operation
  the person started.
- **A refusal Cairn finds while running** — a stale patch, a file changed since it
  was confirmed, `HEAD` moved before an amend — opens the same dialog titled
  "Couldn't <name>", with one sentence that names the path and the reason once and
  "Nothing was changed.", and no Error Details, since no git failed.
- **A refusal Cairn knows in advance** is a greyed control with its reason beside
  it, wrapped: a menu item's under it, a button's beside it. Nothing is said after a
  press that could not act.
- **Success** says nothing; the refresh is the message. **A cancel** opens no
  dialog; its entry in the popover says it was cancelled.

No text a person needs is ever cut: a sentence is kept short instead, naming only
the exception, and the detail goes behind Show files or into the Git Error dialog,
which scrolls. While any dialog or the popover is open, the window's chords and the
lists' keys do nothing.

## Staging

### Files

The selection is one value, in one list at a time, with a primary row: a press
selects a row, ⌘- or Ctrl-click toggles one in or out, Shift-click and Shift+↑/↓
extend from the anchor. The diff shows one file — the primary, the file clicked
first — with everything one file's diff has: Load Diff, the mode row, previous and
next change, the gesture. With several selected, its bar says how many. Fork shows
one file too, and its developer has declined a combined view (Tracker #261,
TrackerWin #786). Where status lists an untracked directory collapsed, its row acts
on everything under it.

Every route acts on the selection, wherever focus is. Fork's five routes, each a
toggle between the two lists: double-click a row, whose first press made it the
selection; the stage chord — Return from a list, or ⌘S on macOS; Enter from a
list, Ctrl+S or Ctrl+Shift+S on Linux, as Fork for Windows — which from the diff,
with no lines selected, stages the whole selection, while Return there does nothing,
as in Fork; a drag from one list to the other; the list header's `Stage` or
`Unstage` button, greyed while the selection is in the other list; and the context
menu, which carries Stage or Unstage, Discard Changes…, Stage All and Copy Path.
Holding Shift on Linux — Fork for Windows' key, which no window manager takes with a
click — or ⌥ on macOS turns both headers' buttons into Stage All and Unstage All,
enabled whatever is selected; the double chevrons reach the same with nothing held,
kept small, as Fork keeps its own, because changes are meant to be read before they
are staged. With a filter on, the selection keeps only the rows the filter shows,
and Stage All and Unstage All take the shown rows and no others, so nothing hidden
is ever acted on. Each list is one drop zone, never a zone per row: the virtualized
list unmounts rows mid-drag, and a row-held drag would be left holding a stale
payload; a drag near an edge scrolls the list. Discard is ⌫ or ⇧⌘D on macOS,
Backspace, Delete or Ctrl+Shift+D on Linux.

One rule says where the selection goes, after an action and after any refresh
alike, whoever moved the files — a stage, a commit made in a terminal, a filter:
the selected paths still listed stay selected, the primary kept or moved to the
nearest of them; when none is left, the row that slid into the first one's place is
selected, else the nearest above it, so a run of presses walks down the list, as
Fork's does (Tracker #514; Fork fixed a jump to the first row in 1.0.57).

An action answers every chord Fork gives it, each heard only in its own scope:
stage, unstage and discard while a file list or the diff in Local Changes has
focus, commit in the commit box, Show Lost Commits in the history, so typing in the
commit box fires nothing but commit. Bare Enter is heard only from Local Changes'
file lists, bare Backspace and Delete from those lists and the diff, never while a
text field has focus; everywhere else a chord without a modifier is a function key
(the accelerator table's contract, `docs/systems/diff.md`, "The accelerator
table").

A conflicted row is staged whole, as Fork stages it: `git add`, which marks it
resolved. It offers no line gesture and no discard — Fork refuses a discard there
too (TrackerWin #1859) — and resolving what is in it is the conflict view's
(`conflicts.md`).

### Staging gestures

Fork puts no actions on the hunk header, and neither does Cairn. Hovering a chunk
outlines it and floats its actions over it: `Stage` and `Discard Changes…` in the
unstaged diff, `Unstage` in the staged one, which offers no discard. A
drag-selection narrows the floating actions, and the stage and discard keys, to the
selected changed lines (`Stage 2 Lines`, `Discard 2 Lines…`); with no lines selected
the keys act on the whole selection of files. The drag carries on across rows the
virtual list has unmounted, and scrolls the diff when it reaches an edge. Side by
side, a selection stays in the column it began in. A mode change is a row of its own
with its own actions, never carried by a selection of lines (`diff.md`). There is no
checkbox column and no selection gutter
(`docs/research/diff-engine/fork-detail-and-diff-ui.md`).

A finished line selection belongs to the text it was made over. A refresh that
draws the same text — switching to the editor and back with the file untouched —
keeps it, its tint and its actions; one whose text changed clears it, and the diff's
bar says once "The file changed — line selection cleared.", gone at the next key or
press. No act made over rows that are no longer drawn reaches a write.

The floating controls live outside the recycled rows, take no focus, and follow the
hover when the rows under them change. They appear in Local Changes only: the
Commit and Changes tabs draw the same rows and put no action on them. In Local
Changes the Ignore Whitespace button is disabled and the diff is always the exact
one, so every drawn change maps to lines a patch can carry; the shared setting is
left as it was and still applies in the other tabs. A gesture produces a
selection, and the patch is built from the exact diff, never the displayed one
(`diff.md`, "Selections and patches"). Spec: `docs/prd/staging-and-commit.md` R8,
R9.

### Discard

Discard always asks, and the question cannot be skipped. The dialog opens at once
on the press — "Counting…" with its button greyed while the engine works out what
the discard takes — titled "Discard Changes", in one sentence: "Discard all changes
in 31 files? 2 untracked files will be deleted. You can't undo this.", with Show
files opening the list of paths and what happens to each, and the button naming the
count, `Discard Changes in 31 Files`. Lines from the diff ask "Discard 2 changed
lines in src/main.rs? You can't undo this." under `Discard 2 Lines`. Its words are
rendered from the operation's consequence and are the `Confirmed` prompt
(`engine.md`, "The confirmation seal"). Focus starts on Cancel; Escape goes back;
Tab stays inside the dialog.

Staged changes are never discardable, and there is no Discard All: select
everything, then discard. Over a staged selection the menu's Discard Changes… is
greyed, "Staged changes can't be discarded. Unstage them first.", and the chord does
nothing, as Fork's does. An untracked file is discarded by deleting it, through the
same dialog; a nested repository among the selection is refused, with its reason,
before any dialog, because deleting one deletes history no prompt can count. A
submodule or a conflicted file is never discarded: alone, its Discard is greyed and
says why; among other files, the dialog discards the rest and says in one line what
is left — "1 submodule and 1 conflicted file are left as they are." Fork has no
Clean command, and neither does Cairn: deleting untracked files is discarding them.
No backup is taken and none is promised — the confirmation is the whole protection,
as in Fork (`feature-inventory.md`, "Recovery").

### The commit box

Fork's, under the diff: a subject field with a characters-left counter — grey down
to Fork's soft limit of 50, then negative, red past 70 — and a Recent Commit
Messages menu at its right; a multi-line description with a ruler at column 72;
`Amend` at the left; and `Commit N Files` at the right, which ⌘Return or Ctrl+Enter
presses from either field without typing a newline, disabled with an empty
subject, as Fork's is, and while nothing is staged unless amending or concluding an
operation in progress. The limits are Fork's defaults; a configurable limit is
filed. Recent messages are the latest 10 commits on the current branch, read from
history, as Fork's are (Tracker #720), newest first, and never stored; ↑/↓ in an
empty or recalled subject walks them, and a recalled message fills both fields.
The draft lives for the life of the window, through refreshes, a failed hook and
Amend's toggle. Something fills the draft only while it is empty and untouched —
`HEAD`'s message, a merge's message — and any edit, clearing it included, makes it
the person's, as Fork's developer describes (Tracker #61).

At most one line stands in the box, and only when there is something to say. With a
merge in progress the box fills the draft with git's merge message as git's own
editor would leave it — its comment lines stripped by git, so what is shown is what
is committed — and commits the merge, even with nothing staged, Amend greyed under
"Merging. Committing concludes the merge; Amend is unavailable." A single
cherry-pick or revert is concluded the same way, as Fork and `git commit` conclude
it. During a rebase, `git am` or a sequence of picks or reverts the box is disabled
and its line names git's own command to continue or abort, since continuing one is
that operation's own design. On a detached `HEAD` the line reads "HEAD is detached:
this commit will be on no branch." and nothing is asked, since the commit can be
recovered.

Ticking Amend fills an empty, untouched draft with `HEAD`'s message and lists
`HEAD`'s files among the staged, as Fork does, the staged list then taken against
`HEAD`'s parent; the draft the person had returns when Amend is unticked, and Amend
unticks itself after the commit. Amend is greyed, with its reason, where there is
nothing to amend — an unborn branch — and while an operation is in progress. Where
amend's staged list cannot be read — a partial clone's missing blob, which Cairn's
reads never fetch — the amend is still offered, and the box says the lists show what
is staged against `HEAD`. The button reads `Amend 3f2a1c9`, with nothing under it,
and the chord does what it does. Pressing it asks what the amend costs, then: where
git keeps the old commit and no remote has it, it amends at once, the old commit
findable in Show Lost Commits; where a remote already has it, or the repository
keeps no reflog, the one confirmation dialog asks first — "Amend Commit", "3f2a1c9 is
already on origin/main. Amending it rewrites history others may have." or "3f2a1c9
can't be recovered after this: this repository keeps no reflog." — Cancel returning
to the box as it was.

A running commit reads "Committing 0:03" in the button's place with Cancel beside
it, its hooks' output streaming into the activity popover; a cancel opens no
dialog and keeps the draft. A failure opens Fork's `Git Error` dialog — the command
and git's output, colour sequences stripped — with `Skip pre-commit hooks and
commit` beside Close in its footer, for a commit or an amend, whatever hooks git
found; it runs once with `--no-verify`, never asks again, and is the only way to
skip hooks, as in Fork; the draft stays. Not in the box, as in Fork: an author
override, empty commits, an up-front skip-hooks, and sign-off or signing as
controls — git's own configuration for both is honoured. Commit and Push belongs
with push, and is designed with it. Spec: `docs/prd/staging-and-commit.md` R10.

### Show Lost Commits

Fork's reflog view is a mode of the graph, not a list. Its toggle — ⌘⇧. or
Ctrl+Shift+., and a check box, "Show Lost Commits", at the right end of the "Graph and
subject" heading's cell, its tooltip naming the chord, which Fork does not have — adds the old
and the new id of every entry of `HEAD`'s reflog and of each local branch's to the walk's tips
and draws the commits no ref reaches dimmed, as Fork dims them: their subject, author, id and
date in a lighter grey, their lanes, edges and chips in their own colours
(`history-graph.md`, "What it walks"); Fork's reaches every reflog. An amended-away or
reset-away commit is found there, until git expires its entry (`gc.reflogExpireUnreachable`,
30 days by default). As in Fork, there is no list of reflog entries. Spec:
`docs/prd/staging-and-commit.md` R11.

### Create Branch

Recovery is Fork's: `New Branch…` on any commit row — a dimmed one's too — or Fork's chord,
Ctrl+Shift+B (⇧⌘B), at `HEAD`, opens Fork's Create Branch dialog, under its title "Use '/' as
a path separator to create folders": the commit read only; a name checked as `git branch` and
`git checkout -b` take it, refused before git runs with the reason beside the buttons behind
Fork's warning triangle — never in the Git Error dialog — and, while the check waits behind a
running write, "Waiting for <name>"; and "Check out after create", remembered. While it is
ticked and the working tree has changes, "Local changes:" offers Fork's three, in Fork's order:
"Don't change", which checks out over the changes or shows git's refusal in the Git Error
dialog; "Stash and reapply", greyed and saying it comes with stashing; and "Discard", with
Fork's ⚠ beside it, never the remembered choice. The button stays "Create and Checkout".
Choosing Discard and pressing it — or Return — is the confirmation, as in Fork: the choice is
deliberate and the press is the acknowledgement, so no second dialog asks again. It runs Fork's
own command, a forced checkout of the new branch, which throws away staged and unstaged changes
and any untracked file in the way; what it takes is git's to decide, so Cairn predicts none of
it, and checks only that `HEAD`, the commit and the name are what they were. A Git Error, or a
refusal, opens over the dialog, which stays as it was left beneath it. Checking out a branch is
otherwise the branch operations' design. Spec: `docs/prd/staging-and-commit.md` R11.3.

### Activity

Fork's Activity Manager, opened from the title bar's status box: one entry per
operation of the session, newest first and bounded, each row its name, Fork's
result line under it — "staged", "Switched to branch 'topic'", "Already up to date"
— its start time and ⚠ on a failure. The selected entry shows its name, one of four
words — running, succeeded, failed, cancelled — with its time and, unlike Fork's,
how long it took; for a failure or a cancel, one sentence; and every `git` it ran
with its output. Unlike Fork's, it also quotes, whole, the prompt an operation
confirmed, and an amend's entry offers "Show in Lost Commits", which turns the mode
on and selects the old commit. Credentials in a URL are removed from every line of
git's before anything keeps it. When the log grows past its bound the oldest whole
entries go. Nothing in it outlives the window, as in Fork. Spec:
`docs/prd/staging-and-commit.md` R12.

### The leftover lock

A leftover `index.lock` is a state of the repository, decided by whether the file
exists now: while it does, no `git` of Cairn's is running or waiting there, and it
is older than a few seconds — so a commit in a terminal with its editor open raises
no alarm — a banner under the title bar says "index.lock is in this repository, so
git can't change the index. If no other git program is running, it was left
behind." and offers `Remove index.lock…`. It goes when the file does, however it
went. A write that fails on the lock is retried after about a second, once or
twice, as Fork retries, before it is reported; its Git Error then carries the same
`Remove index.lock…` beside Close, as Fork's does. Either way the removal is
confirmed — "Remove stale lock", naming the lock's age and size and that another
program may still own it — and is followed by a refresh, never by a retry of the
write. Spec: `docs/prd/staging-and-commit.md` R12.4.

## Palette and type

Dark-first, because the application launches with Freya's `dark_theme`. Cool
graphite neutrals with a slight blue bias, so the ground reads as chosen rather
than default. One accent — a desaturated steel blue — for HEAD, selection and the
primary action, chosen because red and green are already spoken for by diffs and
must stay semantic. Lane colours are a six-step set that stays distinguishable
under common colour-vision deficiencies, and lane identity never depends on
colour alone: the column position carries it.

Type is IBM Plex — Sans for the interface, Mono for ids, paths and diffs. One
family, so the UI and its data read as one instrument. `tabular-nums` wherever
digits align.

A light theme is a real obligation, not a toggle: a diff palette that works on a
dark ground does not survive inversion, so it needs its own design (Open, below).

## The screens

1. **History** — the default view. Graph with a feature branch off `main`, a
   stash inline, ref badges, the detail pane. The example data is Cairn's own
   log plus two invented feature commits, and is marked as such.
2. **Local changes** — the staging screen: the two lists, the diff, and the
   commit box with subject, body and amend. Its gesture is "Staging gestures",
   not the mockup's.
3. **Discard** — the destructive dialog over the history view. Its words and
   buttons are "Discard", and the record of what was confirmed is the activity
   popover ("Activity"), not the mockup's drawer.
4. **Conflict** — the three-way resolver, mid-merge, one of two conflicts.
5. **Worktrees and forge links** — the sidebar section, a branch context menu
   with the forge items, a branch held by another worktree, and the post-push
   toast.

## Open

- A light theme, with its own diff palette.
