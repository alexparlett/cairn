---
status: in-flight
packet: refs-and-status
opened: 2026-10-05
---

# PRD — Refs and status

**In flight. Authoritative while it is.** The packet's work directory is
`docs/work/refs-and-status/`; its decisions and rejected alternatives are that
directory's `brainstorm.md`, L1-L14. Requirements below cite them by id.

Design frame: `docs/design/history-graph.md` (what the graph walks, its labels and
its stash rows), `docs/design/ui.md` (the sidebar, the toolbar, Local Changes),
`docs/design/engine.md` (D1: status is the fourth read git answers),
`docs/design/concurrency.md` (the lanes this packet adds). Program:
`docs/work/daily-loop/roadmap.md` packet 4, under program decision L6 (a measured
bar); it closes the program's open question O2. Evidence, all under
`docs/research/refs-and-status/`:

- `code-audit.md` — what this packet builds on, as built;
- `gix-refs-and-status-api.md` — gix 0.87.1's refs, reflog, worktree and status
  APIs, read from the vendored source and probed against git 2.56.0;
- `status-agreement-spike.md` — gix status against `git status`, 38 fixtures on
  git 2.56.0 and 2.30.9, and both tools' cost on rust-lang/rust;
- `fork-refs-and-status-ui.md` — Fork's labels, stash rows, sidebar, Local
  Changes and refresh, as published;
- `fork-unreachable-stash-base.md` — what Fork does with a stash whose base no
  ref reaches;
- `deep-find-measured.md` — finding a ref's commit by paging the held walk on
  rust-lang/rust: time, retained memory, cancel;
- `fork-deep-history.md` — Fork's commit cap, its layout of the visible area
  alone, and what other clients do with a history too long to hold.

## What this packet delivers

A graph that names what it shows. The history walks every branch, remote-tracking
ref and tag, draws each ref as Fork's label on the commit it points at, and draws
each stash made on a commit it shows as a row of its own. A row keeps only what
it cannot derive, so a history of any depth stays cheap to scroll and to search. A sidebar lists the refs and the working tree's
count of changes, and pressing a ref finds its commit. A toolbar names the
current branch and how far it is from its upstream. And a read-only Local Changes
view lists what `git status` says has changed, staged and unstaged, opening any
file's diff through the working-tree query `diff-engine` built.

Four layers, in build order: refs, ahead/behind and status in the engine, each
exactly what git says; compact rows whose edges are derived as they are drawn; the history walk seeded
from every ref, with labels and stash rows; the worker lanes that carry the new answers and the refresh that
re-asks them; and the views that draw them.

Nothing in it writes to a repository. Staging, committing and every ref operation
are later packets'.

## Requirements

### R1 — The engine enumerates refs as git does (L3, L4)

- R1.1 One query answers a snapshot of the repository's refs: every local branch,
  every remote-tracking ref, every tag, the stash list, and `HEAD`'s state —
  on a branch (named), detached (at a commit) or unborn (on a branch with no
  commit). Enumeration is gix's, in process.
- R1.2 Each ref carries its full name, its kind, and the commit it identifies. An
  annotated tag carries both its tag object's id and the commit it peels to; a
  tag chain is followed to its end; a tag that peels to something other than a
  commit is listed, and identifies no commit.
- R1.3 A symbolic ref (`refs/remotes/origin/HEAD`) is never peeled into its
  target's name: it is listed as itself, naming the ref it points at. A symbolic
  ref whose target does not exist is not listed, as `git for-each-ref` does not
  list it. A ref whose name is invalid is skipped, as git skips it; a ref that
  cannot be read is skipped and counted, never a failure of the whole snapshot.
- R1.4 Each local branch carries its upstream as git resolves it
  (`branch.<name>.remote` and `.merge`), including a local upstream
  (`remote = .`), and whether that upstream ref exists ("gone" when it does not).
- R1.5 The stash list is the `refs/stash` reflog, newest first, each entry with
  its index (`stash@{n}`), its message, its commit and the commit it was made on
  (its first parent). The reflog is read so that no message length truncates or
  ends it (`gix-refs-and-status-api.md`: gix's newest-first reader stops at a
  line over 4 KiB).
- R1.6 Order within each kind is `git for-each-ref`'s.
- R1.7 The query is cancellable by its epoch and reports the cost it paid.
- R1.8 `GIT_NAMESPACE` in Cairn's own environment is not honoured: the refs
  Cairn shows are the refs the `git` it runs sees, and it runs `git` without that
  variable.
- R1.9 A repository whose refs live in reftable (`extensions.refStorage` set to
  anything but `files`) is refused at open with a reason saying so, where it is
  refused for dubious ownership today. gix 0.87 opens such a repository and then
  fails reading `HEAD`; reftable support is not built here (Out of scope).

### R2 — Ahead and behind (L9)

- R2.1 For each local branch with an upstream that exists, the engine answers how
  many commits the branch has that its upstream lacks, and the reverse — exactly
  what `git rev-list --left-right --count <branch>...<upstream>` answers. gix
  has no API for it; it is two walks with the other side hidden.
- R2.2 It is a query of its own, answered after the refs snapshot it belongs to,
  cancellable between branches and within a walk, so a long divergence never
  holds up the refs.

### R3 — Status is what `git status` says (L1, L2)

- R3.1 The working tree's status is read by running `git status --porcelain=v2
  -z` as a read invocation, a new named function in `crates/cairn-git/src/reads/`,
  under a read's environment and cancelled by its epoch (the process group is
  ended). gix's status is not used: it differed from git on 12 of the spike's 38
  fixtures (`status-agreement-spike.md`), on a split index beside a sparse one it
  reported a staged deletion nobody made (`gix-refs-and-status-api.md`), and it
  starts clean filters outside `process/`.
- R3.2 The answer, in `cairn-model` types, carries per path: its staged change
  (added, modified, deleted, renamed or copied with its source, type changed),
  its unstaged change (modified, deleted, type changed), intent-to-add, a
  submodule's state (new commits, modified content, untracked content), its
  conflict kind (each of git's seven: both deleted, added by us, deleted by them,
  added by them, deleted by us, both added, both modified), or untracked. Paths
  are bytes as git gives them; no path is lost to its encoding.
- R3.3 Renames and copies are whatever the user's configuration makes `git
  status` report (`status.renames`, `status.renameLimit`, `diff.renames`); Cairn
  passes nothing that overrides them.
- R3.4 Untracked files are listed one per file (git's `--untracked-files=all`,
  Fork's choice), except where the user's configuration sets
  `status.showUntrackedFiles=no`, where none are listed. The setting is read as
  git reads it; how is phase 02's to decide against `reads::fetch_settings`'s
  precedent, and a new porcelain read needs the user.
- R3.5 Ignored files are not listed.
- R3.6 Submodules are reported as the user's `submodule.<name>.ignore` and
  `diff.ignoreSubmodules` make `git status` report them.
- R3.7 A state git cannot answer — a sparse index on a git that cannot read one
  (git 2.30 cannot) — is answered as that state, never as an empty or partial
  list.
- R3.8 A status read writes no index, object, ref or configuration. Residuals,
  accepted as parity with the user's own `git status`: under a split index git
  updates `sharedindex.*`'s mtime, under a sparse index the mtime of loose tree
  objects; the fsmonitor and the clean filter run as D1 already allows. Because a
  read never writes the refreshed index back, a tree whose every file's stat
  changed stays slow to read (736 ms on rust-lang/rust) until something
  refreshes the index; Cairn does not, in this packet.

### R4 — The history walks every ref, with stash rows (L5)

- R4.1 The history is seeded from every local branch, every remote-tracking ref,
  every tag that identifies a commit, and `HEAD` — Fork's All Commits — from the
  same snapshot R1 answers. Nothing about a stash is a seed: not `refs/stash`,
  and not the commit a stash was made on.
- R4.2 Each stash whose base — the commit it was made on — is a commit the walk
  reaches is a row of its own, merged into the stream by its commit time but
  never after its base (a stash dated older than its base, by clock skew, is
  drawn directly above it), with one edge: to that commit. A stash whose base no
  seed reaches (its branch deleted, its commit rebased or amended away, made on
  a detached `HEAD` that moved on) has no row in the graph, as in Fork
  (`fork-unreachable-stash-base.md`); it stays in the sidebar's Stashes (R8.5).
  How the walk knows a stash's base is reached before the row's date comes up is
  phase 05's to settle within C11. A stash's index and untracked commits never
  become rows (unless some ref reaches them otherwise). Several stashes on one
  commit each take their own row and lane.
- R4.3 Every row carries the refs that point at its commit (R1's labels:
  branches, remote-tracking refs, tags, and whether it is `HEAD`'s commit), from
  the snapshot the walk started from.
- R4.4 A row that is not a commit is a new `RowContent` variant read by naming
  every variant, as the invariant requires; the lane assigner lays a stash row
  out without keying it as a commit the walk can reach.
- R4.5 There is no working-tree row (L6). Fork draws none; the sidebar's Local
  Changes is the entry point.
- R4.6 A row keeps only what it cannot derive (L13): its commit's id, parents,
  subject, author and date, its lane, and the lanes that start, end, merge or
  fork at it — never every lane passing through it. The edges a drawn row
  crosses are derived from periodic full lane snapshots, advanced through the
  rows' changes, for the rows drawn alone; they are exactly
  the edges the lane assigner computes today, repaints included. Today 89% of a
  retained row is the edges crossing it, 4.3-6.3 KB a row on rust-lang/rust
  (`deep-find-measured.md`); the bar is C15.
- R4.7 A retained row is slim (L14): its commit id once (not again on its graph
  row), a parent count in place of its parents (the list draws only whether a
  commit is a merge; the Commit tab reads parents from the details query), its
  subject as an offset into a text store the history shares, its author as an
  index into an author table the history shares, its date, its lane and its lane
  changes — and no author email, which the list never draws. No retained row
  allocates on its own; the history's stores grow in fixed chunks, never by
  doubling. A row is read through the history that holds its stores, and each
  page from the worker carries its own text and any authors new to the history.
  The bar is C16.

### R5 — Labels on rows (L7)

- R5.1 Each ref is a chip between the graph and the subject, Fork's: outlined,
  tinted with its row's lane colour; a tag indigo with a tag glyph; a
  remote-tracking ref with a remote glyph (a forge's icon is packet 6's); the
  current branch marked ✓. The `HEAD` row's subject is bold.
- R5.2 Compact labels: a remote-tracking ref at the same commit as the local
  branch that tracks it shrinks to its glyph in front of that branch's chip. A
  remote-tracking ref with no local branch there keeps its own chip.
- R5.3 Labels that do not fit are clipped at the column's edge, as Fork clips
  them: no wrap, no elision, no "+N".
- R5.4 A stash row draws a `stash@{n}` chip with a stash glyph and the stash's
  message as its subject.
- R5.5 Each kind of label is told apart by its glyph and shape, never by colour
  alone.

### R6 — The detail pane for refs and stashes

- R6.1 The Commit tab shows a REFS row above the id, listing the refs that point
  at the commit, as Fork does; a commit with none shows no row.
- R6.2 Selecting a stash row shows its message, author and date in the Commit tab
  and, in the Changes tab, what it changed against the commit it was made on —
  the changes query of that pair, which is what `git stash show` lists — and,
  where the user's `stash.showIncludeUntracked` is set, the untracked files the
  stash holds, as `git stash show` then lists them.

### R7 — The title bar names the branch (L9)

- R7.1 The title bar shows the repository's name, marked `*` while status reports
  any change, the current branch, and its behind and ahead counts (↓n ↑m) when it
  has an upstream; a detached `HEAD` shows its short id, an unborn branch its name
  and that it has no commit.

### R8 — The sidebar (L8)

- R8.1 A sidebar left of the history, behind a draggable splitter, in Fork's
  order: Local Changes with its count, All Commits, a filter box, Branches,
  Remotes, Tags, Stashes. Worktrees (packet 8) and Submodules are not built.
- R8.2 Branches and remote-tracking refs are grouped into folders by `/`; the
  current branch is marked ✓ and bold; a branch with an upstream shows its
  ahead and behind counts; an upstream that is gone is shown as gone.
- R8.3 The filter narrows every section to names containing its text, answered
  on a worker for a snapshot of any size.
- R8.4 Every list in the sidebar is virtualized; a snapshot of tens of thousands
  of refs builds one viewport.
- R8.5 Pressing a ref or a stash selects its row and scrolls it into view. A row
  not yet loaded is found by paging the held walk forward until it arrives,
  saying so while it looks. A find is history-lane work — it pages the same
  held walk a scroll pages — so the next press or scroll supersedes it. A ref
  whose commit is not in the walk (a tag on a tree) says so. A stash with no row
  (R4.2) shows its changes in the detail pane with no row selected, saying it is
  not in the graph.
- R8.6 A find retains what scrolling to its target would, and no more: R4.6's
  compact rows. On rust-lang/rust paging to the oldest commit takes about 2.4 s
  and, with today's rows, holds about 1.4 GiB (`deep-find-measured.md`); with
  compact, slim rows it is held to C16. There is no cap on the history (L13). This
  deviates from Fork on purpose: Fork holds only its newest 50,000 or 100,000
  commits and does nothing when a pressed ref is past them
  (`fork-deep-history.md`).
- R8.7 Pressing Local Changes shows the Local Changes view in the main region;
  pressing All Commits shows the history.

### R9 — Local Changes, read only (L10)

- R9.1 Unstaged above Staged, each a flat list of paths with Fork's badges: a
  modified file, an added or untracked one, a deleted one, a renamed or copied one
  with its source, a type change, a submodule, a conflicted path. A path with
  both staged and unstaged changes is in both lists.
- R9.2 The count beside Local Changes is the number of distinct paths status
  reports; with no change, no count is shown.
- R9.3 Choosing a path shows its diff through the working-tree query
  (`diff-engine` R3): its staged diff from Staged, its unstaged or untracked diff
  from Unstaged, in the diff view with its bar and settings. The first path is
  chosen when the view opens.
- R9.4 A conflicted path shows a notice in place of a diff: resolution is the
  second lap's (D6).
- R9.5 Both lists are virtualized.
- R9.6 Nothing in the view stages, discards or commits; packet 5 adds that to it.

### R10 — Refresh (L11)

- R10.1 Refs, ahead/behind and status are re-read when the window gains focus,
  when a Cairn operation finishes (a fetch, today), and on the Refresh action,
  whose chord is Fork's for each platform, in the accelerator table.
- R10.2 Nothing watches the file system.
- R10.3 A refresh supersedes the refresh before it, lane by lane; an answer is
  drawn only against the refresh it answers.
- R10.4 When the refs snapshot differs from the one the history was walked from —
  any ref moved, appeared or went, `HEAD`'s state changed (a checkout that moves
  no ref still moves ✓ and the bold row), or the stash list changed — the history
  is reopened; otherwise it is left alone. Ahead/behind counts are not part of
  the comparison. This replaces the fetch's own tip comparison.
- R10.5 A reopened history keeps the selected commit selected if it arrives
  again as the walk pages.

### R11 — Every new answer runs on a worker (L11)

- R11.1 Refs, ahead/behind, status and the sidebar filter are queries, each in a
  lane of its own with its own epoch; none supersedes another lane, and the
  existing lanes keep their one crossing (a changes query supersedes the file
  diff). A ref's find is a history-lane query (R8.5).
- R11.2 Refs are read on the history thread, which reopens the walk from them.
  Status and ahead/behind run on a third thread of their own, so neither a
  history page nor a diff queues behind a slow status (736 ms on a stat-dirty
  rust-lang/rust) or a long divergence.
- R11.3 A superseded answer large enough to cost a frame (a refs snapshot, a
  status) is freed off the UI thread, and so are the history rows a reopen
  replaces — which today `session::reload_if` clears on the UI thread (#52), and
  which a reopen on focus would otherwise free far more often.
- R11.4 The UI thread never waits on any of this; the invariant holds unchanged.

## Product rules

- **Status is git's.** Cairn shows what `git status` shows, because it is what
  `git status` printed. A path git lists is listed; a path it does not, is not.
- **Refs are git's.** A ref `git for-each-ref` lists is listed with its target;
  one it does not, is not.
- **Fork's layout, every deviation named.** Two deviations: a generic remote
  glyph until packet 6 knows the forge; and no cap on the history — a pressed ref
  is found however deep it is, where Fork stops at its newest 50,000 or 100,000
  commits and does nothing. (No working-tree row follows Fork, which has none.)
- **Nothing in this packet writes to a repository.** Every query here is a read.
- **Meaning never rests on colour alone.** A label's kind is its glyph and shape.
- **An answer is drawn only for the refresh or selection it was computed for.**
- **No network call.** Ahead and behind compare local refs; nothing fetches.

## Acceptance criteria

The single authoritative copy. `docs/work/refs-and-status/qa-checklist.md` points
here and does not restate them.

| # | Criterion | Pinned by |
| --- | --- | --- |
| C1 | The refs snapshot equals `git for-each-ref`'s (names, kinds, targets, a symbolic ref's target, an annotated tag's object and peeled commit) and `HEAD`'s state equals `git symbolic-ref`/`git rev-parse`'s, on fixtures covering loose and packed refs, a packed ref with a peeled line, lightweight, annotated and chained tags, a tag on a tree, `origin/HEAD`, a dangling symbolic ref, an unreadable ref, an invalid name, detached and unborn `HEAD`, and a linked worktree; the stash list equals `git stash list` with forty entries and a message over 4 KiB; each upstream equals `%(upstream)`, including `remote = .` and a gone upstream; with `GIT_NAMESPACE` set in Cairn's own environment the snapshot is still `git for-each-ref`'s without it; the query reports its cost | integration tests in `cairn-git` against real `git` |
| C2 | A reftable repository is refused at open with its reason; a `files` repository opens | integration test, skipped where the host's git cannot make a reftable repository and required wherever it can (`CAIRN_REQUIRE_*`, the gate probing as the test does) |
| C3 | Ahead and behind equal `git rev-list --left-right --count` for a branch ahead, behind, diverged, equal, with a local upstream, and with a gone upstream (no counts); a cancelled query stops its walk rather than finishing it | integration test |
| C4 | Status parsed from git equals independent oracles — `git diff --cached --name-status` under the same rename config, `git diff --name-status`, `git ls-files --others --exclude-standard`, `git ls-files -u` and the submodule's own state — on fixtures covering every R3.2 kind, all seven conflict kinds, renames and copies under each `status.renames` value, intent-to-add, type and mode changes, submodules under each ignore setting, `status.showUntrackedFiles=no`, nested untracked directories listed per file, names with spaces, newlines and invalid UTF-8, and an unborn branch; the index is byte-identical after every read; under git 2.30.9 and 2.32.7 as well as the host's (`git-floor`), where a sparse index answers R3.7's state | integration tests |
| C5 | A superseded status read ends its process group: the repository's registry holds no running invocation after the cancel | integration test through the runner |
| C6 | The commits the history walks equal `git rev-list --branches --remotes --tags HEAD`, on a fixture with a branch `HEAD` cannot reach, a tag on a tree, and a stash whose branch was deleted (no row for it, and none of its base's otherwise unreachable commits); every row's labels equal what `git log --decorate=full` names for that commit, `refs/stash` aside; each stash whose base is walked is one row, with one edge, to its first parent, and its index and untracked commits are not rows; a stash dated older than its base is drawn above it | integration and assigner tests |
| C7 | Rows draw each label kind with its glyph, compact labels, clipping, ✓ and the bold `HEAD` subject; a stash row draws its chip and message; the Commit tab draws REFS; selecting a stash lists what `git stash show --name-status` lists, with `stash.showIncludeUntracked` unset and set | headless tests |
| C8 | The sidebar draws its sections in order, groups by `/`, marks the current branch, shows ahead/behind and gone; the filter narrows on a worker; pressing a ref selects its row, finding one beyond the loaded pages and cancelling when a press or scroll supersedes it; a tag on a tree says so; a stash with no row shows its changes and says so; a deep find retains only compact, slim rows (C16); a sidebar of 50,000 refs builds one viewport | headless and worker tests |
| C9 | Local Changes draws both lists with their badges and the count by R9.2; choosing a path draws its diff from the working-tree query; a conflicted path draws its notice; 50,000 paths build one viewport | headless tests |
| C10 | Focus, the Refresh action and a finished fetch each re-read refs and status; a moved ref, a changed stash list and a checkout that moves no ref each reopen the history, and an unchanged snapshot does not; the selection survives a reopen; a superseded refresh is never drawn; no new lane supersedes another, and the changes-to-file-diff crossing is unchanged; a slow status queues neither a page nor a diff; superseded snapshots, statuses and a reopen's replaced rows are freed off the UI thread | worker tests through the real boundary, and headless tests with focus set |
| C11 | On rust-lang/rust at `c999cef531e` (`~/Development/bench/rust`, never written), on the machine recorded in `docs/research/diff-engine/measured-baseline.md`, warm, release build, median of seven: status on a clean tree within 100 ms; status with 1,000 modified and 10,000 untracked files (on a scratch clone) within 250 ms; the refs snapshot with every ahead/behind within 100 ms, and a 10,000-ref fixture's recorded; the first page of history seeded from every ref within 200 ms, recorded beside `HEAD`'s; status with every file's stat changed recorded, not barred | an `#[ignore]`d reporter driven by `CAIRN_BENCH_REPO`, numbers in `progress.md` and, at teardown, in `docs/research/refs-and-status/` |
| C12 | `window_check` keeps every frame under 16.7 ms of UI-thread work while the decorated history, the sidebar, the refs and a large status land | the `#[ignore]`d `window_check`, numbers recorded |
| C13 | D1 in `docs/design/engine.md` and the root `CLAUDE.md` names status as a read git answers, with R3.8's residuals | review |
| C14 | `scripts/gate.sh` passes | the gate |
| C15 | Every row's derived edges equal the edges today's lane assigner computes for it, repaints included, over the crafted fixtures, the Cairn checkout and every ref of the bench repository; a find of rust-lang/rust's oldest commit at `c999cef531e`, in a release build, takes no more than 10% longer than with today's rows (2.4 s) | an equivalence test in `cairn-model`/`cairn-git`, and the `#[ignore]`d reporter driven by `CAIRN_BENCH_REPO`, numbers recorded |
| C16 | Every row draws the same subject, author, date, short id and merge marker as before the rows were slimmed, over the crafted fixtures and the Cairn checkout; paging the whole of rust-lang/rust at `c999cef531e` from every ref, in a release build, retains at most 64 MiB of rows and the stores they read (1.4 GiB today; about 160 MiB with compact edges alone); no retained row owns a heap allocation | headless and model tests, and the `#[ignore]`d reporter driven by `CAIRN_BENCH_REPO`, numbers recorded |

C11, C12, and the measured halves of C15 and C16 are not automated, for the reason `history-graph`'s A7 was not: a
timing assertion in CI is flaky and bound to a machine.

## Out of scope

Another packet's: checking out, creating, renaming or deleting a ref (packet 7);
staging, discarding, committing and stash operations (packet 5); worktrees in the
sidebar and the guard against checking out a branch held elsewhere (packet 8);
forge icons on remote labels (packet 6).

Not done, and filed with the `file-issue` skill at teardown: listing ignored files; reftable repositories; honouring
`GIT_NAMESPACE`; watching the file system; refreshing the index so a
stat-dirty tree reads fast again (a write, for the local write lane); greying
commits not on the current branch, and Fork's push and pull dots; Fork's branch
filter and hiding refs (#2); a Submodules section; the changed-file tree view
(#36); a resident bound on rows for histories of millions of commits, with a
re-walk beyond it (#4).
