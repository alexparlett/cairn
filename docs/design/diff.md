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

A file that is not diffed as text carries its state instead of lines — binary
with both sizes, too large with the limit it crossed, a Git LFS pointer, a
submodule with both commit ids, a mode change only, conflicted, or unsupported
with its reason — and the view shows that state. A too-large file is refused
before it is read, never after the window has stalled on it.

A diff shows what the user's own `git diff` shows. Which files a commit or a
comparison changed, with their renames and copies, and which lines of each
changed — under the user's algorithm, a diff driver's, and the indent heuristic,
with and without whitespace — come from `git` itself, because those are where
gix's answers differ from git's (`engine.md`, "Where git answers a read"); gix
reads both versions and decides what is not text, and Cairn groups git's changes
into hunks by git's own rule. No diff algorithm is written here but the
intra-line highlights git has no equivalent of, and gix types stop at the
seam. A working-tree diff shows what `git diff`
shows, filters included (`engine.md`, "Reads see git's form"). Spec:
`docs/prd/diff-engine.md` R1-R3. As built: `docs/systems/diff.md`.

## Selections and patches

A changed line is identified by its line number on its own side, so a
**selection** is a set of those identities, independent of presentation, context
and expansion. A mode change is an item of its own beside them: a selection of
lines carries no `old mode` or `new mode`, and a mode change stages alone, as
`git add -p` asks about it separately. The patch emitter takes a file diff and a
selection and returns a patch `git apply` accepts, always at three lines of
context whatever the view shows, with every path line C-quoted exactly as git's
own `quote_c_style` quotes it — a raw tab in a path line is a patch git refuses.

There is one emit rule, the forward one, and every action reaches it through a
diff. Staging lines applies the unstaged diff — index to working tree — to the
index (`git apply --cached`); unstaging lines applies the staged diff — `HEAD` to
index — inverted, to the index; discarding lines applies the unstaged diff
inverted, to the working tree (`git apply`). A file diff, a selection and a changed
file each have a pure inversion — the sides swapped, each change's two spans
swapped, the removed and added sets swapped, paths, modes and ids swapped and an
addition made a deletion — so no patch is applied with `-R`, and the model holds
one rule rather than a rule and its mirror. The mirror keeps one use: applied in
reverse, it is the tests' independent derivation of the same index and working
tree, beside the reference applier and real git on both floors.

Part of an untracked or newly added file can be staged — a `new file mode` patch
of the selected lines — or discarded, as a partial deletion emitted as a
modification, as Fork allows; discarding all of an untracked file deletes it
(`ui.md`, "Discard"). Some changes are whole-file only, because no patch of lines
expresses them: a deletion on either side (`git add -p`'s rule), a binary file, a
Git LFS pointer, a file past the size limits, a submodule, a type change and a
conflicted file. They are staged with `git add`, unstaged with `git reset`, which
works on an unborn branch where `git restore --staged` fails, and discarded with
`git restore --worktree` — but for two. A submodule is never discarded: `git
restore` leaves its checked-out commit where it was, only `git submodule update`
moves it back, and no prompt can count what is dirty inside it, so its row offers
no discard and says why. A conflicted file is staged whole, which marks it
resolved, and offers neither lines nor a discard; resolving its content is the
conflict view's (`conflicts.md`). Unstaging lines of a staged rename emits a
content-only patch at its new path; unstaging it whole resets both of its paths.

The staged side pairs renames and copies as `git diff --cached` does under the
user's configuration, so a staged rename is drawn and unstaged as a rename, never
as a deletion beside an addition.

What is staged is what was drawn and selected. Every `git apply` runs with
`--whitespace=nowarn`, so the user's `apply.whitespace` can neither strip
whitespace from staged lines under `fix` nor refuse them under `error` — a
deliberate departure from `git add -p`, which honours it. And before every apply
the operation checks the content its patch was built from: for a stage, that the
index entry's blob is the diff's old side; for an unstage, that it is the new side;
for a discard, that the index entry is the old side — or, for part of an untracked
file, that the index holds no entry for it — and that the working-tree file,
hashed in git's form (`git hash-object --path`, a read that runs the clean filter
the diff ran and writes no object), is the drawn new side. A stale patch writes
nothing and says which path moved; unchecked, git would apply it at an offset and
report success.

What a user stages is the exact diff, never the displayed one. Ignoring whitespace
produces a second set of ranges for display only, which the emitter cannot reach
by construction, and with whitespace ignored the view says that some changes are
hidden; in Local Changes, where the gestures are, the setting is disabled and the
exact diff is drawn (`ui.md`, "Staging gestures"). Display context, expansion and
side-by-side change what is drawn and never what the emitter reads. Spec:
`docs/prd/staging-and-commit.md` R2, R3. Evidence:
`docs/research/staging-and-commit/patch-mechanics-spike.md` and
`git-write-verbs.md` beside it.

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
  file and side-by-side, each a button with no chord, as in Fork; previous and
  next change also answer Fork's chords while the diff pane has focus. Context
  starts where the user's own `git diff` starts it — `diff.context`, three by
  default — and moves one line per click, never below one; hunks are grouped as
  their `git diff` groups them, `diff.interHunkContext` included. Context, ignore
  whitespace and side-by-side are shared by every diff view; the entire file
  belongs to the Changes tab's single file, since a diff opened in place in the
  Commit tab has no bar to turn it off. Every row is what
  `git diff` prints: a context line from the side git prints it from, and git's
  end-of-file marker as a row of its own.
- A hunk header is git's whole `@@` line — the function context git prints after
  it, by the path's diff driver, included — in muted text at normal row height,
  with no band and no buttons; staging acts on a selection, not on the header
  (`ui.md`, "Staging gestures").
- Intra-line highlighting is word-level (token granularity) and always on.
- Colours are **solid tints**, starting from Fork's measured dark values and
  retuned to Cairn's palette; intra-line ranges take stronger tints. As in Fork,
  there is no plus-and-minus column, and the line-number gutters are Fork's: small
  numbers with a gap either side of the separator. In unified the tint starts at
  the separator, and which number is blank still says removed or added without
  colour. Side by side the tint spans each column, number included, as Fork's
  panes do, so a changed pair is told from its context by its tint — the one
  place the diff lets meaning rest on colour, as Fork does.
- Every row has the same height, and a long line scrolls horizontally.
- Diff text, ids and paths are IBM Plex Mono (`ui.md`, "Palette and type").

Spec: `docs/prd/diff-engine.md` R5-R8. As built: `docs/systems/diff.md`, "The
detail pane" and "The diff view".

## Open

- Syntax highlighting in diffs.
- Whether the diff view and the conflict view (`conflicts.md`) share a component.
