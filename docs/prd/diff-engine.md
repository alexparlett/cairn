---
status: in-flight
packet: diff-engine
opened: 2026-09-17
---

# PRD — Diff engine

**In flight. Authoritative while the packet is open.** Teardown stamps this file
and points at `docs/systems/diff.md`; until then this is the one copy of what the
packet commits to.

Design frame: `docs/design/cairn.md` decisions **D1** (amended by this packet for
filter drivers on a working-tree read), **D3** (the worker pool, which this packet
splits into lanes), **D5** (the accelerator table, born here) and **D6**
(conflicts stay out); `docs/design/ui.md` for the layout. Program:
`docs/work/daily-loop/roadmap.md` packet 3, under program decisions L2 (the model
is patch-capable) and L6 (a measured bar). Decisions and rejected alternatives:
`docs/work/diff-engine/brainstorm.md` L1-L16. Evidence, all under
`docs/research/diff-engine/`: `gix-diff-api.md` (the gix API, verified against
the vendored source), `engine-and-worker-as-built.md`, `ui-and-app-as-built.md`,
`what-clients-show.md` (a cross-client precedent study),
`fork-detail-and-diff-ui.md` (Fork, the layout this packet follows) and
`measured-baseline.md` (git's own timings on the repository the bar names).

## What this packet delivers

The ability to read what a commit changed, what two commits differ by, and what a
working-tree file differs from its staged and committed versions, drawn the way
Fork draws it, from a model that can already emit the patch a later packet will
stage a single line with.

Three layers, in build order. A diff model in `cairn-model` whose hunks, rows and
patches are all projections of one exact answer. Engine queries in `cairn-git`
that compute that answer through gix. And a detail pane and diff view in
`cairn-ui` and `cairn-app`, fed through a worker lane of their own so that neither
a scroll nor a diff can cancel the other.

The patch emitter ships with nothing consuming it. That is deliberate (program
L2): its round-trip tests against `git apply` are the difference between packet 5
being a feature and being a rewrite.

## Requirements

### R1 — A patch-capable diff model

- R1.1 A **changed file** describes one path in the change set of a commit, a
  comparison or a working tree: its status (added, deleted, modified, renamed or
  copied with its similarity, type changed), its old and new paths, its old and
  new modes (regular, executable, symlink, submodule) and its old and new object
  ids. It carries no line counts (L10).
- R1.2 A **file diff** holds both versions of one file as lines, each line its
  bytes plus whether it ended in a newline, and the exact changed ranges between
  them (L2). When whitespace is ignored it also holds a second set of changed
  ranges, for display only (L4), and it holds the intra-line highlight ranges of
  paired lines (R2.7). A file that is not diffed as text carries its state
  instead: binary with both sizes, too large with the limit it crossed (R2.6), a
  Git LFS pointer, a submodule with old and new commit ids, a mode change only,
  conflicted, or unsupported with its reason.
- R1.3 A changed line is identified by its line number on its own side: a removed
  line by its old line number, an added line by its new one. A **selection** is a
  set of those identities, so it is independent of presentation, context and
  expansion.
- R1.4 Hunks, unified rows and side-by-side rows are pure projections of a file
  diff and a context setting, which is a line count or the entire file.
  Side-by-side pairs the i-th removed line of a changed range with its i-th added
  line and pads the shorter side with filler rows.
- R1.5 A projection is addressable: a view obtains the row count and any one row
  without constructing every row, so a file of any length costs one viewport of
  work per frame.
- R1.6 The **patch emitter** takes a file diff and a selection and returns a
  unified patch that `git apply --cached` accepts against the old version and that
  applies exactly the selected changes. An unselected removed line becomes
  context, an unselected added line is dropped, hunk headers are recounted, a
  hunk with no selected change is omitted, and context is always three lines
  whatever the view shows. It writes the headers git needs for an added, deleted,
  renamed or mode-changed file, and the `\ No newline at end of file` marker for an
  unterminated last line on either side, keeping the marker with its line when
  that line becomes context. The same patch applied in reverse removes exactly the
  selected changes.
- R1.7 The emitter reads only the exact changed ranges. The whitespace-ignoring
  ranges are unreachable from it by construction, not by convention (L4).
- R1.8 A **commit's details** is its own model type, because the Commit tab needs
  what `CommitSummary` does not carry: the committer beside the author, both
  timestamps with their offsets, the whole message rather than its subject, and
  the parents. Extending `CommitSummary` instead is allowed; either way the change
  obeys `cairn-model`'s rules, which is a test in the same commit and nothing new
  in its dependency allowlist.
- R1.9 All of R1 is pure: no gix, no I/O, no clock, and nothing beyond the crate's
  dependency allowlist.

### R2 — The engine answers commit and comparison diffs

- R2.1 A **changes query** takes two commits, or one commit meaning against its
  first parent (the empty tree for a root commit, L5), and returns the changed
  files sorted by path. For one commit it also returns the details the Commit tab
  shows (R5.3). No gix type appears in its signature.
- R2.2 Renames and copies are detected as the user's `diff.renames` and
  `diff.renameLimit` configure them, with gix's defaults otherwise. When the limit
  cuts rename detection short, the answer says so.
- R2.3 A **content query** takes one changed file and the display options and
  returns its file diff. Both versions are read in git's form through gix's
  resource cache in its to-git mode: no textconv program and no external diff
  program runs on any read.
- R2.4 **gix computes the diff** (L3): the algorithm is the user's
  `diff.algorithm` (one gix lacks falls back to histogram), with git's indent
  heuristic, over lines that keep their terminators. Cairn converts gix's changed
  ranges into model types at the seam and implements no diff algorithm of its own.
- R2.5 Binary detection follows git: the `diff` and `binary` attributes,
  `core.bigFileThreshold`, and a NUL byte in the first 8,000 bytes.
- R2.6 A file is **too large** to diff by default when either version exceeds
  1 MiB or 50,000 lines, or holds a line longer than 2,048 bytes (L10). The line
  rule is Fork's, whose own limit is documented in characters; bytes is Cairn's
  reading of it, identical for ASCII and stricter for anything else, and the
  fixtures encode bytes. The size
  limit is tested before the content is read, and the limits apply to git's form
  of the content. A caller may ask to load it anyway up to 64 MiB per version;
  above that the placeholder offers no load.
- R2.7 **Intra-line highlighting** is computed by the engine for paired removed
  and added lines, at token granularity (words, whitespace runs, punctuation), and
  is always on (L4). It is skipped for a pair where either line is over the
  long-line limit.
- R2.8 **Ignoring whitespace** computes the second set of changed ranges by
  comparing lines with all whitespace removed, which is git's `-w`. The lines drawn
  are always the original bytes.
- R2.9 A query can be cancelled: a tree walk checks at every change, and work that
  spans files checks between files. Rename detection and a single file's content
  diff run to completion, bounded by the rename limit and by R2.6.
- R2.10 Failures are `cairn_git::Error` variants naming what the caller must
  handle.

### R3 — The engine answers working-tree diffs of one path

- R3.1 Given a path, the engine answers its **staged** diff (HEAD against the
  index), its **unstaged** diff (the index against the working tree) or an
  **untracked** file's diff (nothing against the working tree), each as a file
  diff under R1.
- R3.2 Working-tree content is converted to git's form through gix's filter
  pipeline: line endings, `ident`, working-tree encoding, and the clean filter
  driver that the path's attributes name and the user's config defines, exactly
  as `git diff` converts it (L6, D1 as amended). No textconv program runs.
- R3.3 The index and the attributes are read fresh for every working-tree query.
  Nothing about a working tree is cached across queries.
- R3.4 A deleted file, a type change, a mode-only change, a conflicted path (no
  diff until D6), a submodule (its commit ids and whether it is dirty) and a sparse
  index (unsupported, and said so) each answer their state, never an error and
  never an empty diff.
- R3.5 A working-tree query writes nothing: no refreshed index, no object.
- R3.6 This packet does not enumerate which paths changed, which is status and
  packet 4's, and does not draw a Local Changes screen. Whichever of packets 4 and
  5 builds the changed-file list wires it to R3 and R6 (L6).

### R4 — Diff work runs on a lane of its own

- R4.1 Every query belongs to a **lane** (history, changes, file diff) and carries
  an epoch numbered per lane. A new query supersedes older queries in its own lane
  only, with one exception: a new changes query also supersedes the file-diff lane
  (L8). A scroll never cancels a diff, a selection never cancels a scroll, and a
  fetch still supersedes nothing.
- R4.2 Each open repository has a **diff thread** with its own repository handle,
  serving the changes and file-diff lanes, including working-tree requests. An
  explicit routing table from lane to thread replaces `WORKERS_PER_REPOSITORY`.
- R4.3 The diff thread serves the newest request in each lane, file diffs first.
  Work that spans files, such as Expand All, runs one file at a time and yields to
  a newer request between files.
- R4.4 Every answer names the target it answers (the commit or pair of commits,
  the path, the options), and the window draws an answer only against the
  selection it names.
- R4.5 Answers for commits and comparisons may be cached, keyed by tree or blob ids
  and options, because they never go stale. Working-tree answers are never cached.
- R4.6 The UI thread never waits on any of this; the existing invariant holds
  unchanged.

### R5 — The detail pane

- R5.1 The pane sits below the commit list, behind a draggable splitter, and can
  be collapsed (L9). A right-hand position is issue #30.
- R5.2 It has two tabs, **Commit** (the default) and **Changes**. The tab last
  chosen is kept for the session — Fork's Mac build does this, and whether its
  Windows build does was not established, so this is Cairn's choice. A File Tree
  tab is issue #31.
- R5.3 The **Commit tab** shows the author and the committer, each with name,
  email and full timestamp with its offset; the full commit id; each parent as a
  short id that selects that parent when it is loaded (reaching an unloaded one is
  issue #3); and the whole message. Below them is the changed-file list, a renamed
  file showing both names. A file's diff expands in place, and files start
  collapsed. **Expand All** expands files in list order until a total line budget
  is spent, then says how many files stay collapsed. The tab shows no avatar and
  makes no network call; ref chips wait for packet 4's ref enumeration.
- R5.4 The **Changes tab** shows a one-line summary, the changed-file list on the
  left with a filter, and one file's diff on the right.
- R5.5 Every list in the pane is virtualised: a commit touching 55,184 paths lays
  out one viewport of rows.

### R6 — The diff view

- R6.1 Unified by default. **Side-by-side** is one setting for every diff view,
  kept for the session (L1). One shared setting is Cairn's choice: Fork's Windows
  build shares it, and the scope of the Mac build's could not be established.
  Remembering it across sessions is issue #29.
- R6.2 The header holds previous-change and next-change controls, the path with
  its filename emphasised, and toggles for **ignore whitespace**, **fewer lines**,
  **more lines**, **entire file** and **side-by-side**.
- R6.3 Context defaults to three lines and moves by one line per click, never
  below one; **entire file** shows every line. Context is one setting for every
  diff view, kept for the session. Expanding a single gap is issue #32.
- R6.4 A unified row carries an old and a new line-number gutter and a
  plus-or-minus marker column; a side-by-side row carries a number gutter per
  side. A hunk header row shows git's `@@ -a,b +c,d @@` in muted text at normal
  row height, with no band and no buttons. Every row has the same height, and a
  long line scrolls horizontally rather than wrapping (wrap is issue #34).
- R6.5 Added and removed rows take **solid tints**, starting from Fork's measured
  dark values and retuned to Cairn's ground; intra-line ranges take stronger tints.
  The tint starts after the gutters. A change is never shown by colour alone: the
  marker column and the blank gutter carry it too (L11).
- R6.6 Diff text, ids and paths render in IBM Plex Mono, embedded as a font file
  with its licence (L16).
- R6.7 With whitespace ignored, whitespace-only changes are hidden and the view
  says that some are.
- R6.8 A file diff that is not text shows its state (R1.2) in place of rows: both
  sizes for a binary; "Changes are too large to display" with a **Load Diff**
  control for too large; the pointer text and a label for an LFS pointer; old and
  new commit ids for a submodule; old and new modes for a mode-only change; both
  names for a rename with no content change.
- R6.9 In a diff loaded past the limit, a line longer than the long-line limit is
  drawn truncated, with a visible marker.

### R7 — Comparing two commits

- R7.1 A modifier-click on a second commit row selects exactly two commits; a
  plain click returns to one.
- R7.2 The comparison is tip against tip, not against a merge base. The lower row
  in the list is the base, and a swap control reverses the two (L7). Fork's
  Windows build orders the pair automatically and offers the swap; what its Mac
  build does was not established, so the ordering rule is Cairn's.
- R7.3 A comparison shows in the Changes tab under a header naming both commits;
  the Commit tab is unavailable while two commits are selected.

### R8 — Keyboard

- R8.1 An **accelerator table** maps each logical action to a per-platform chord
  (D5). No component names a literal modifier.
- R8.2 This packet's actions: previous and next change, previous and next file,
  toggle side-by-side, toggle ignore whitespace, more lines, fewer lines, entire
  file, extend the selection to a second commit, and switch tab.
- R8.3 "No component names a literal modifier" becomes an invariant in
  `CLAUDE.md`, with its guard twin, in the phase that builds the table (L14).

## Product rules

- **What a user will stage is the exact diff, never the displayed one.** Display
  context, expansion, side-by-side and ignore-whitespace change what is drawn and
  never what the emitter reads.
- **A working-tree diff shows what `git diff` shows.** Its content has been
  through the user's filters; a diff that skipped them would one day stage content
  the filter exists to change.
- **Nothing in this packet writes to a repository.** Every query here is a read,
  including the ones that run a filter driver.
- **An answer is drawn only for the selection it was computed for.** A fast click
  never draws the previous commit's files under the new commit's header.
- **A too-large file is refused before it is read**, never after the window has
  stalled on it.
- **Meaning never rests on colour alone**, the rule the graph's lanes already
  follow.
- **Hidden changes are announced.** Ignoring whitespace says that it is hiding
  something; Fork hides them silently, and that is a deliberate deviation.
- **No network call.** No avatar and no remote lookup, for any part of the pane.

## Acceptance criteria

The single authoritative copy. `docs/work/diff-engine/qa-checklist.md` points
here and does not restate them.

| # | Criterion | Pinned by |
| --- | --- | --- |
| C1 | For every non-merge commit of the Cairn checkout and of the crafted fixtures, the patch with every line selected, applied with `git apply --cached` to the parent's tree, yields exactly the commit's tree | integration test in `cairn-git` running real `git`; plus an `#[ignore]`d reporter over a named repository, with results recorded in `progress.md` |
| C2 | Seeded random line selections, applied the same way, give exactly what an independent in-memory applier gives, over fixtures covering: a missing final newline on each side, CRLF content, an added file, a deleted file, a rename with edits, a mode change, an empty file, a one-line file, and adjacent hunks | property-style test in `cairn-git` over a fixed set of seeds |
| C3 | The same patches applied with `--reverse --cached` onto the commit restore the parent's content | integration test |
| C4 | A selection survives unified rows, side-by-side rows, every context size and entire-file mode unchanged, and the emitter's output depends on none of them | model unit tests |
| C5 | The changes query agrees with git's own detection under the same config on fixtures: paths, statuses, rename and copy pairs and modes, including a merge commit against its first parent, a root commit, copies when configured, and a rename limit that cuts detection short; and the commit details it returns match git's for the same commit, including the committer, both timestamps with their offsets, and the whole message | integration tests comparing against `git` |
| C6 | On crafted fixtures with unambiguous edits, the content query's unified projection equals `git diff -U3`; binary detection agrees with git for the `-diff` attribute, the `binary` attribute and a NUL byte; each too-large limit fires, and the size limit fires before the content is read | integration tests |
| C7 | Staged, unstaged and untracked diffs of a path equal `git diff --cached`, `git diff` and `git diff --no-index` on fixtures including a `text=auto` file with CRLF endings and a path under a configured clean filter driver; every R3.4 state is answered; the index file is byte-identical after every query | integration tests |
| C8 | A scroll does not cancel a diff and a diff does not cancel a scroll; a changes query supersedes the file-diff lane; a superseded answer is never drawn; an answer naming another selection is never drawn | worker tests through the real boundary |
| C9 | The diff view over a 1,000-line and a 100,000-line file builds one viewport of rows, at the top and scrolled deep, in unified and in side-by-side | headless `freya-testing` test, the twin of `only_a_viewport_of_rows_is_built_however_long_the_history` |
| C10 | The Commit tab shows every R5.3 field, expands a file in place, and stops Expand All at its budget with the notice; the Changes tab shows the list, the filter and one file's diff; the chosen tab is kept | headless tests |
| C11 | The header toggles behave as R6.2 and R6.3 say, including the one-line minimum; ignore-whitespace hides whitespace-only changes and says so; intra-line ranges are drawn; every R6.8 state draws its notice | headless tests |
| C12 | A modifier-click selects two commits, the comparison is tip against tip with the lower row as base, and swap reverses it | headless test |
| C13 | Every R8.2 action resolves through the accelerator table, and no component names a literal modifier | unit test, plus the guard from R8.3 |
| C14 | On rust-lang/rust at `c999cef531e`, on the machine recorded in `measured-baseline.md`, warm, in a release build: the changes query finishes within 100 ms on `f0845adb0c1`, 500 ms on `cf2dff2b1e3` and 500 ms on `5a3292f163d`; the content query finishes within 100 ms on `3b09522c34b`; `6a6e8446b97` answers too large without reading its content, and its Load Diff time is recorded; the rename pairs on `5a3292f163d` match git's, or each gap is filed; the window stays responsive while the two heaviest subjects load | an `#[ignore]`d reporter driven by `CAIRN_BENCH_REPO` for the engine numbers, and a check by hand for the window, all recorded in `progress.md` |
| C15 | D1's amendment is in `docs/design/cairn.md` and `CLAUDE.md`, and the filter driver's inherited environment is stated there as a residual | review |
| C16 | `scripts/gate.sh` passes | the gate |

C14 is deliberately not automated, for the reason `history-graph`'s A7 was not: a
timing assertion in CI is flaky and bound to a machine. The machine is an AMD
Ryzen 7 9800X3D with 60 GiB of memory, an NVMe disk and git 2.55.0, and the
subjects are chosen and justified in `measured-baseline.md`.

## Out of scope

Filed, not done: remembering diff view settings across sessions (#29), the detail
pane on the right (#30), a File Tree tab (#31), expanding context between
individual hunks (#32), visible whitespace (#33), line wrap (#34), a pop-up
side-by-side diff (#35), tree and table modes for the changed-file list (#36) and
image diffs (#37).

Another packet's: listing which working-tree paths changed and the Local Changes
screen (packet 4 or 5); ref chips on the Commit tab (packet 4); any staging,
unstaging or discarding, and the subprocess runner's stdin support that
`git apply --cached` needs (packet 5); combined merge diffs and conflict
resolution (D6, the second lap); jumping to a parent that is not loaded (#3).

Not planned: syntax highlighting, which stays open in the spine, and avatars,
which would need a network call.
