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

## What changes, and why

| Change | Driven by |
| --- | --- |
| A destructive operation's confirmation is rendered from what the engine computed it will destroy — what is lost, how much, and whether it is recoverable — and its text IS the `Confirmed` prompt. Fork's dialogs name a count and no more. | `cairn.md`, what Cairn is; `engine.md`, "The confirmation seal" |
| The discard dialog starts with focus on **Cancel**. Fork makes Discard the default, and its own tracker asks for Cancel (Tracker #1080). | `cairn.md`, what Cairn is |
| The **activity popover**, Fork's, also quotes the prompt the user confirmed for each destructive operation and points at the way back where there is one. Fork's quotes nothing. | `engine.md`, "The confirmation seal" |
| **Ignore Whitespace is disabled in Local Changes**, so every change drawn there is one a gesture can stage. Fork offers it there. | `diff.md`, "Selections and patches" |
| The discard dialog counts what is lost by kind and size — modified files with their lines, untracked files deleted with their bytes — where Fork's names a count of files. | `engine.md`, "The confirmation seal" |
| The amend button names the commit it replaces and where the old one stays; Fork's reads `Amend`. | `engine.md`, "The confirmation seal"; `feature-inventory.md`, "Recovery" |
| A stale `index.lock` a write left behind can be removed, confirmed, from the activity popover while Cairn runs no `git` in that repository. | `processes.md`, "Cancellation"; `engine.md`, D1's one deletion made without `git` |
| A nested repository among the untracked rows is refused before any dialog, since deleting it deletes history no prompt can count. | `engine.md`, the write verbs |
| A submodule row offers no discard and says why; Fork offers `Discard Submodule Changes`. `git restore` leaves a submodule's commit where it was, and no prompt can count what is dirty inside it. | `engine.md`, the write verbs |
| During a rebase, `git am`, a cherry-pick or a revert the commit box is disabled and names the operation; Fork pre-fills git's message for a cherry-pick or revert and commits it. Continuing those operations is their own design. | `cairn.md`, the milestone — rewrites are the second lap |
| **Show Lost Commits has a control in the history's toolbar area** as well as its chord; Fork has only the View menu item and the chord, and refused a toolbar button (TrackerWin #378). | `feature-inventory.md`, "Recovery" — the reflog must be visible to be usable |
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
status box does.

Pressing Local Changes puts the staging screen in the main region, as Fork lays it
out: a filter over Unstaged above Staged, each a list of paths with Fork's badges
under a header carrying its list's `Stage` or `Unstage` button, Fork's
double-chevron Stage All above them, and beside them, on the right, the diff of the
chosen path with the staging gesture on it and the commit box under it ("Staging",
"The commit box", below). As built: `docs/systems/local-changes.md`.

**Refresh** is Fork's: refs and status are re-read when the window gains focus,
after Cairn's own operations, and on the Refresh chord. After an operation only
what it changed is re-read, and while a commit's hooks run Cairn starts no refresh
of its own (`concurrency.md`, "Operations"). Nothing watches the file
system. Fork's standing complaint — that each refresh holds `index.lock` for
seconds on a large tree — does not carry over, because Cairn's reads take no
lock (`engine.md`).

## Staging

### Files

Fork's five routes, each a toggle between the two lists: double-click a row;
Return or ⌘S on macOS, Enter or Ctrl+Shift+S on Linux, as Fork for Windows; drag
from one list to the other; the list header's `Stage` or `Unstage` button, with ⌥
held for Stage All or Unstage All, and the double-chevron Stage All, which Fork
keeps small on purpose because changes are meant to be read before they are
staged; and the context menu, which carries Stage or Unstage, Discard Changes…,
Stage All and Copy Path. Each list is one drop zone, never a zone per row: the
virtualized list unmounts rows mid-drag, and a row-held drag would be left holding
a stale payload; a drag near an edge scrolls the list. Discard is ⌫ or ⇧⌘D on
macOS, Backspace, Delete or Ctrl+Shift+D on Linux.

Both lists take a multi-selection — ⌘- or Ctrl-click, Shift-click, Shift+↑/↓ —
and where status lists an untracked directory collapsed, its row acts on
everything under it. After a stage or an unstage the selection moves to the
nearest path left in the list it left, as Fork's does (Tracker #514), so a run of
presses walks down the list. An action answers every chord Fork gives it — Return
and ⌘S both stage — and each is heard only in its own scope: stage, unstage and
discard while a file list or the diff in Local Changes has focus, commit in the
commit box, Show Lost Commits in the history, so typing in the commit box fires
nothing but commit. Bare Enter, Backspace and Delete are heard only while a file
list or the diff in Local Changes has focus, never while a text field does;
everywhere else a chord without a modifier is a function key (the accelerator
table's contract, `docs/systems/diff.md`, "The accelerator table").

A conflicted row is staged whole, as Fork stages it: `git add`, which marks it
resolved. It offers no line gesture and no discard — Fork refuses a discard there
too (TrackerWin #1859) — and resolving what is in it is the conflict view's
(`conflicts.md`).

### Staging gestures

Fork puts no actions on the hunk header, and neither does Cairn. Hovering a chunk
outlines it and floats its actions over it: `Stage` and `Discard Changes…` in the
unstaged diff, `Unstage` in the staged one, which offers no discard. A
drag-selection narrows the floating actions, and the stage and discard keys, to the
selected changed lines (`Stage 2 Lines`, `Discard 2 Lines`); with nothing selected
the keys act on the whole file. The drag carries on across rows the virtual list
has unmounted, and scrolls the diff when it reaches an edge. Side by side, a
selection stays in the column it began in. A mode change is a row of its own with
its own actions, never carried by a selection of lines (`diff.md`). There is no
checkbox column and no selection gutter (`docs/research/diff-engine/fork-detail-and-diff-ui.md`).

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

Discard always asks, and the question cannot be skipped. The dialog is Fork's, in
Fork's words with the counts the engine computed — "Do you want to discard the
changes in 3 files? 2 modified (14 lines), 1 untracked file deleted (2.1 KiB). You
can't undo this action." — and its button names the count (`Discard Changes in 3
Files`, `Discard 2 Lines`). Its words are rendered from the operation's consequence
and are the `Confirmed` prompt (`engine.md`, "The confirmation seal"). Focus starts
on Cancel; Escape cancels; Tab stays inside the dialog, and the window's chords do
nothing while it is open.

Staged changes are never discardable, and there is no Discard All: select
everything, then discard. An untracked file is discarded by deleting it, through
the same dialog; a nested repository among the selection is refused, with its
reason, before any dialog, because deleting one deletes history no prompt can
count. A submodule row offers no discard and says why: `git restore` leaves the
submodule's commit where it was, and no prompt can count what is dirty inside it.
Fork has no Clean command, and neither does Cairn: deleting untracked files
is discarding them. No backup is taken and none is promised — the confirmation is
the whole protection, as in Fork (`feature-inventory.md`, "Recovery").

### The commit box

Fork's, under the diff: a subject field with a characters-left counter — grey down
to Fork's soft limit of 50, then negative, red past 70 — and a Recent Commit
Messages menu at its right; a multi-line description with a ruler at column 72;
`Amend` at the left; and `Commit N Files` at the right, which ⌘Return or Ctrl+Enter
presses from either field without typing a newline, disabled while nothing is
staged unless amending. The limits are Fork's defaults; a configurable limit is
filed. Recent messages are the latest 10 commits on the current branch, read from
history, as Fork's are (Tracker #720), newest first, and never stored; ↑/↓ in an
empty or recalled subject walks them, and a recalled message fills both fields.
The draft lives for the life of the window, through refreshes, a failed hook and
Amend's toggle.

With a merge in progress the box fills an empty draft with git's merge message as
git wrote it — its `# Conflicts:` comment lines left visible for the user to
delete, since a message given with `-F` keeps them — and commits the merge, as
Fork's does. During a rebase, `git am`, a cherry-pick or a revert the box is
disabled and names the operation in progress, since continuing one is that
operation's own design.

Ticking Amend fills an empty draft with `HEAD`'s message and lists `HEAD`'s files
among the staged, the staged list then taken against `HEAD`'s parent; the draft the
user had returns when Amend is unticked, and Amend unticks itself after the commit.
Amend is disabled where there is nothing to amend — an unborn branch — and while a
merge is in progress.
An amend is confirmed by its button, which reads `Amend <short id>` above
"Replaces <short id> '<subject>'. The old commit stays in Show Lost Commits." —
that visible text is the `Confirmed` prompt. A dialog asks first only when a remote
already has `HEAD`: "<short id> is already on origin/main. Sharing the amended
commit needs a force push." Fork for macOS asks nothing.

A running commit is drawn busy with its elapsed time and a Cancel, its hooks'
output streaming into the activity popover. A failure opens Fork's `Git Error`
dialog — the command and git's output, colour sequences stripped — with `Skip
pre-commit hooks and commit` beside Close only where a `pre-commit` or `commit-msg`
hook exists in the hooks directory git resolves, `core.hooksPath` included; that
button is the only way to skip hooks, as in Fork, and the draft stays. Not in the
box, as in Fork: an author override, empty commits, an up-front skip-hooks, and
sign-off or signing as controls — git's own configuration for both is honoured.
Commit and Push belongs with push, and is designed with it. Spec:
`docs/prd/staging-and-commit.md` R10.

### Show Lost Commits

Fork's reflog view is a mode of the graph, not a list. Its toggle — ⌘⇧. or
Ctrl+Shift+., and a control in the history's toolbar area, which Fork does not
have — adds the old and the new id of every entry of `HEAD`'s reflog and of each
local branch's to the walk's tips and draws the commits no ref reaches dimmed, in their lanes
(`history-graph.md`, "What it walks"); Fork's reaches every reflog. An
amended-away or reset-away commit is found there, until git expires its entry
(`gc.reflogExpireUnreachable`, 30 days by default). Recovery is a dimmed commit's
`Create Branch Here…`: a name, then the branch. As in Fork, there is no list of
reflog entries. Spec: `docs/prd/staging-and-commit.md` R11.

### Activity

Fork's Activity popover, opened from the toolbar's status: one entry per operation
of the session, newest first and bounded — its name, start time, duration and
outcome, and each `git` it ran with the tail of its stderr — and, unlike Fork's,
the prompt the operation confirmed when it had one and the way back where there is
one: an amend points at the commit it replaced, which Show Lost Commits draws.
Credentials in a URL are removed from drawn stderr. Nothing in it outlives the
window, as in Fork. Where a write left an `index.lock` behind and Cairn is running
no `git` in that repository, the entry offers `Remove index.lock…`, confirmed like
any destructive operation, its prompt naming the lock's age and saying another
program may still own it. Spec: `docs/prd/staging-and-commit.md` R12.

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
