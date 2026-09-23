# Diff

Intent, not as-built. Spine: `docs/design/cairn.md`. Layout around it: `ui.md`.
Fork's own pane and diff view, studied as the reference:
`docs/research/diff-engine/fork-detail-and-diff-ui.md`.

## Why diff is the one to get right

After the graph, the diff is the most load-bearing thing Cairn builds. Commit
details, working-tree changes, hunk staging, line staging, compare revisions,
conflict resolution, image diffs, stash contents and interactive-rebase preview
all consume it.

And the trap: a diff model built for display makes line-level staging impossible
without a rewrite. Staging one line means *constructing a patch* and handing it to
`git apply --cached`, which is what `git add -p` does internally. Fork headlines
line-by-line staging, and it is the feature that most demands the foundation be
right the first time, so the model is patch-capable from the start.

## The model holds one exact answer

A file diff holds both versions of the file as lines and the exact changed ranges
between them. Hunks, unified rows, side-by-side rows and patches are all pure
projections of that one answer, and a projection is addressable — a view asks for
the row count and any single row without building every row, so a file of any
length costs one viewport of work per frame.

A changed line is identified by its line number on its own side, so a
**selection** is a set of those identities, independent of presentation, context
and expansion. The patch emitter takes a file diff and a selection and returns a
patch `git apply --cached` accepts, always with three lines of context whatever
the view shows, and the same patch in reverse removes exactly the selection.

What a user stages is the exact diff, never the displayed one. Ignoring
whitespace produces a second set of ranges for display only, which the emitter
cannot reach by construction. Display context, expansion and side-by-side change
what is drawn and never what the emitter reads. Hidden changes are announced:
with whitespace ignored, the view says that some changes are hidden.

A file that is not diffed as text carries its state instead of lines — binary
with both sizes, too large with the limit it crossed, a Git LFS pointer, a
submodule with both commit ids, a mode change only, conflicted, or unsupported
with its reason — and the view shows that state. A too-large file is refused
before it is read, never after the window has stalled on it.

gix computes the diff; Cairn groups it. No diff algorithm is written here, and gix
types stop at the seam (`engine.md`). A working-tree diff shows what `git diff`
shows, filters included (`engine.md`, "Reads see git's form"). Spec:
`docs/prd/diff-engine.md` R1-R3.

## The detail pane

- It sits below the commit list by default, as in both of Fork's builds, behind a
  draggable splitter, and collapses. Putting it to the right is a user preference
  (issue #30, which needs somewhere to keep preferences, issue #29), so the pane's
  components never assume their width.
- Two tabs: **Commit**, the default, and **Changes**; the last one used is kept
  for the session. Fork's third tab, File Tree, is issue #31.
- **Commit** shows author and committer with full timestamps, the full commit id,
  parents as links, the message and chips for the refs pointing at the commit;
  then the changed files, whose diffs expand in place from collapsed, with an
  Expand All bounded by a line budget. No avatars: Cairn makes no network call
  for the pane.
- **Changes** shows a one-line summary, the changed files on the left with a
  filter, and one file's diff on the right.
- Selecting a second commit with a modifier-click compares the two, tip against
  tip, in the Changes tab, with a control to swap which is the base.
- No per-file line counts anywhere, as in Fork: they cost a full blob read per
  file, which on a large commit is hundreds of milliseconds of git's own time
  (`docs/research/diff-engine/measured-baseline.md`).

## The diff view

- **Unified by default, side-by-side as one setting** shared by every diff view.
  The patch model is independent of either.
- The header carries previous and next change, the path with its filename
  emphasised, and toggles for ignore whitespace, fewer lines, more lines, entire
  file and side-by-side. Context starts at three lines and moves one line per
  click, never below one.
- A hunk header is git's `@@` line in muted text at normal row height, with no
  band and no buttons; staging acts on a selection, not on the header
  (`ui.md`, "Staging gestures").
- Intra-line highlighting is word-level (token granularity) and always on.
- Colours are **solid tints**, starting from Fork's measured dark values and
  retuned to Cairn's palette; intra-line ranges take stronger tints, and the tint
  starts after the gutters. A narrow plus-and-minus column stays, which Fork has
  no equivalent of: meaning never rests on colour alone.
- Every row has the same height, and a long line scrolls horizontally.
- Diff text, ids and paths are IBM Plex Mono (`ui.md`, "Palette and type").

Spec: `docs/prd/diff-engine.md` R5-R8.

## Open

- Syntax highlighting in diffs.
- Whether the diff view and the conflict view (`conflicts.md`) share a component.
