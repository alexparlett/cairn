# Brainstorm — diff-engine

Locked decisions and rejected alternatives. Historical record: never retro-edited.
Evidence for every decision is under `docs/research/diff-engine/`; the program
frame is `docs/work/daily-loop/brainstorm.md` L2 (patch-capable model) and L6
(measured bar), and the brief is `docs/work/daily-loop/roadmap.md` packet 3.

## How this was decided

Four recon records were gathered first: the engine and worker as built, the UI
and app as built, the gix diff API verified against the vendored gix 0.87.1
source, and a cross-client precedent study. The user then asked for three things
before locking: more detail on the hunk source, the working-tree read and the
worker threading; a layout based on research of Fork, which the user holds as
the standard; and a measured bar on a real large repository. That produced two
more records, `fork-detail-and-diff-ui.md` and `measured-baseline.md`, the latter
against a clone of rust-lang/rust made for the purpose at
`~/Development/bench/rust`. Decisions locked over three rounds on 2026-09-17.

## Locked 2026-09-17

**L1. Both presentations, unified by default, with Fork's context controls.**
Program question O1 is closed. Side-by-side is one setting for every diff view,
kept for the session. Context defaults to three lines and moves one line per
click through more-lines and fewer-lines buttons, never below one, beside an
entire-file toggle. The user's words: "default inline optional side by side,
should show a segment around the diff, optional expand lines, or show entire
file", then "more and fewer buttons is what i want" once Fork's actual mechanism
was on the table.
Rejected: unified only, the first-round recommendation — the user wants Fork's
standard and Fork has both. Rejected: expanding a single gap between hunks,
which Fork does not have (filed, #32). Rejected: remembering the setting across
sessions now, because Cairn has no preferences store and one likely needs a
dependency (filed, #29).
Evidence: `what-clients-show.md` Parts B and C (no client defaults to
side-by-side; GitHub Desktop's split view maps back to one selection model);
`fork-detail-and-diff-ui.md` (the header toggles and the one-line minimum).

**L2. The model holds one exact answer; everything drawn or emitted is a
projection of it.** A file diff holds both versions' lines, each with whether it
ended in a newline, plus the exact changed ranges. Hunks at any context, the
entire file, unified rows, side-by-side rows and the patch are pure functions in
`cairn-model`. A changed line is identified by its line number on its own side,
so a selection survives every change of presentation. The patch always carries
three lines of context, whatever the view shows. First-round answer: "fine for
now"; refined by L3 once the user chose L1.
Rejected: a model of hunks at a fixed context — display-shaped, a new query for
every context change, and selection indices that shift when context does.
Rejected: GitHub Desktop's selection by unified-row index, which ties identity
to one presentation. Rejected: GitButler's route of splicing hunks into a blob
and writing it through the object database, which D1 forbids.
Evidence: `what-clients-show.md` Part D (four patch writers, three blob writers,
the eleven shared rules); `gix-diff-api.md` section 10 (what `git apply --cached`
requires, from git's own `add-patch.c`).

**L3. gix computes the diff; Cairn groups it into hunks and writes the patch.**
The engine loads both versions through gix's resource cache, runs gix's bundled
imara-diff with the user's `diff.algorithm` and git's indent heuristic over lines
that keep their terminators, and converts gix's changed ranges into `cairn-model`
types at the seam. Cairn writes only the grouping into hunks, the header
arithmetic and the patch text. The user asked "B will wrap the gix model right?"
— yes: the edit script is gix's, and only plain data crosses the seam, because
gix types never appear in a public signature.
Rejected: gix's own unified renderer — context is fixed per call, so every
more-lines click is a new query; fed gix's own interned input it drops the
missing-newline fact; and it writes no file headers. Rejected: running
`git diff` and parsing it, which D1 rules out for reads.
Evidence: `gix-diff-api.md` sections 2 and 10.

**L4. Diff options.** Ignore whitespace is git's `-w`, computed as a second,
display-only set of changed ranges that the patch emitter cannot reach by
construction; it is for review, never for staging. Intra-line highlighting is
token-level and always on, with no toggle. Rename detection follows the user's
config, and a rename limit that cuts detection short is reported. Added at PRD
writing and flagged to the user: the view says when ignoring whitespace hides
changes, where Fork hides them silently.
Rejected: staging from a whitespace-normalised diff with `--ignore-whitespace`,
the route behind Fork's staging failures under that toggle. Rejected: a toggle
for intra-line highlighting, which no client that has it offers.
Evidence: `what-clients-show.md` Part B; `fork-detail-and-diff-ui.md` (whitespace
policy); `gix-diff-api.md` sections 3 and 4 (neither whitespace modes nor word
diff exist in gix).

**L5. A merge commit diffs against its first parent.** The Commit tab lists every
parent; a root commit diffs against the empty tree.
Rejected: git's combined `--cc` diff — a third shape that cannot be applied as a
patch. Rejected: Tower's per-view parent setting, for lack of any demand.
Evidence: `what-clients-show.md` Part A; `fork-detail-and-diff-ui.md` (Fork runs
`git show --diff-merges=1`).

**L6. A working-tree diff runs the user's filter drivers, and D1 is amended to
say so.** An unstaged diff converts the working-tree file to git's form through
gix's filter pipeline, including a clean filter driver the user configured
(git-lfs, git-crypt, nbstripout), exactly as `git diff` does. textconv never
runs. D1 becomes: reads never spawn `git`, but converting a worktree file may run
the user's filter driver. Verified in the vendored source during planning: gix
starts the driver itself (`gix-filter`'s `spawn_driver`), with Cairn's inherited
environment plus `GIT_DIR` and `GIT_WORK_TREE` from `Repository::command_context`,
so the driver's environment is a stated residual of the environment invariant,
which stays scoped to `git` processes. User: "a is fine and scope is fine".
Scope, confirmed with it: this packet answers one path's staged, unstaged or
untracked diff and models the non-text states (deleted, type change, mode only,
conflicted, submodule, sparse index). It does not enumerate changed paths, which
is status and packet 4's, and draws no Local Changes screen; whichever of packets
4 and 5 builds that list wires it up.
Rejected: in-process conversions only, which keeps "reads never spawn" literally
true — gix skips an unconfigured driver silently — but shows LFS, git-crypt and
nbstripout users diffs `git diff` does not, and would hand packet 5 a patch built
from content their filter exists to change. Rejected: raw working-tree bytes,
which breaks every CRLF checkout.
Evidence: `gix-diff-api.md` sections 5 and 6.

**L7. Compare is two commits, tip against tip.** A modifier-click selects exactly
two; the lower row is the base; a swap control reverses them; the comparison
shows in the Changes tab.
Rejected: comparing against the merge base, which Fork does not do.
Evidence: `fork-detail-and-diff-ui.md`.

**L8. Queries are numbered per lane, and diffs get a thread of their own.** Three
lanes — history, changes, file diff — each superseding only itself, except that
a new changes query also supersedes the file-diff lane. A diff thread per
repository, with its own handle, serves the changes and file-diff lanes; an
explicit routing table from lane to thread replaces `WORKERS_PER_REPOSITORY`,
whose assertion already asks for "a routing decision". The thread serves the
newest request per lane, file diffs first, and yields between files during
multi-file work. Every answer names its target. Commit and comparison answers may
be cached by tree ids; working-tree answers never are. User: "I think thats
fine".
Rejected: serving diffs on the history thread, where the live walk borrows the
handle across turns and a diff and a page would queue behind each other.
Rejected: a pool of diff threads — a gix handle cannot be shared, each thread
needs its own caches, and only the newest selection matters. Rejected: keeping
one epoch counter, under which a selection cancels the walk in flight and the
next scroll drops the diff's answer. Rejected: epochless diffs, which could
never be superseded.
Considered, not blocking: the spine asks for the repository manager's shape
"before the worker pool in D3 has more than one consumer". The diff thread is
per-repository state and the view settings are app-wide, which fits tabs, a
sidebar of repositories and separate windows alike.
Evidence: `engine-and-worker-as-built.md` section 3; `gix-diff-api.md` section 8
(what is `Send`, and where a cancel can land).

**L9. The layout is Fork's.** The detail pane sits below the commit list behind a
splitter and collapses. Tabs are Commit, the default, and Changes; the last tab
is kept for the session. The Commit tab shows author and committer, dates, the
full id, parents as links and the message, then the changed files, whose diffs
expand in place from collapsed, with an Expand All bounded by a line budget. The
Changes tab shows a one-line summary, a filterable file list on the left and one
file's diff on the right. The diff header has previous and next change, the path,
and toggles for ignore whitespace, fewer lines, more lines, entire file and
side-by-side. A hunk header is git's `@@` line in grey at normal row height, with
no band and no buttons. Too large shows "Changes are too large to display" with
Load Diff. No avatars, because they need a network call; ref chips wait for
packet 4; a parent link selects the parent when it is loaded, and #3 covers the
rest. User: "9. Agree", with each sub-point agreed.
Three of these are Cairn's choices rather than Fork facts, because Fork's own
documents did not settle them (they are in Q1 below): that the last tab is
remembered, that one side-by-side setting is shared by every view, and that the
lower row of a pair is the base. The PRD says so at each of them.
Filed rather than built: the pane on the right (#30), a File Tree tab (#31),
visible whitespace (#33), wrap (#34), the pop-up diff (#35), tree and table
modes for the file list (#36), image diffs (#37).
Rejected, from Cairn's own mockup: the fixed 250 px pane, counts on tabs,
persistent hunk buttons on a banded header, and a checkbox column. Fork has none
of these, and the staging affordance is packet 5's to choose.
Evidence: `fork-detail-and-diff-ui.md`; `ui-and-app-as-built.md`.

**L10. A diff is paid for when it is looked at.** A changes query walks trees and
returns the list only; content is computed when a file is selected or expanded.
There are no per-file line counts anywhere, which reverses the first round's lazy
counts: Fork shows none, and git itself takes 407 ms to count lines on the
55,184-path commit. A file is too large by default above 1 MiB, above 50,000
lines, or with a line over 2,048 bytes. The line rule is Fork's, documented in
characters; bytes is Cairn's reading of it. Added at PRD
writing and flagged to the user: Load Diff works up to 64 MiB per version.
Rejected: computing every hunk up front — git itself misses 500 ms producing
whole-commit patches for the two largest subjects. Rejected: a background job
filling in line counts.
Evidence: `measured-baseline.md`; `fork-detail-and-diff-ui.md`.

**L11. Solid tints, and a marker column.** Added and removed rows take solid tints
starting from Fork's measured dark values, retuned to Cairn's ground, with
stronger tints for intra-line ranges. Unlike Fork, Cairn keeps a narrow
plus-and-minus column, so a change never rests on colour alone. User: "Colours is
fine".
Rejected: the mockup's 13% washes; colour as the only carrier of meaning, which
the graph's lanes already refuse.
Evidence: `fork-detail-and-diff-ui.md` (measured colours); `docs/design/ui.md`
(palette rules).

**L12. The measured bar is absolute, on a named repository and machine.**
rust-lang/rust at `c999cef531e`, cloned to `~/Development/bench/rust`; the
subjects and ceilings are the PRD's criterion C14; warm, release build, hardware
recorded. The rename pairs on the largest rollup are compared with git's, and
every gap is filed.
Rejected: bars stated as a ratio to git, which git's fixed 2.4 ms process cost
distorts at the small end. Rejected: a timing assertion in CI, flaky for the
reason `history-graph`'s A7 gave.
Evidence: `measured-baseline.md`.

**L13. `docs/design/ui.md` is corrected where it misdescribed Fork.** It said Fork
puts hunk actions on the hunk header. Fork outlines a hovered chunk and floats
Stage and Discard over it, and a drag-selection narrows them to lines. The
correction lands with this plan; which gesture Cairn uses is packet 5's decision.
User: "Thats fine".
Evidence: `fork-detail-and-diff-ui.md`.

**L14. The accelerator table is born here, and its rule gets a guard.** This
packet's actions resolve through a table of logical actions to per-platform
chords (D5). Added at PRD writing and flagged to the user: "no component names a
literal modifier" becomes an invariant with a guard twin in the phase that builds
the table, because a rule the table makes checkable and nobody checks is a
suggestion.
Rejected: leaving it a convention once the table exists.

**L15. Nine phases, strictly sequential.** Model; engine for commits and
comparisons; engine for the working tree; worker lanes; detail pane; unified
diff view; Changes tab, states and side-by-side; expansion and compare; final
QA. The user delegated this ("Your decision"). The UI work was first proposed as
two phases and is three, because each would otherwise have carried four large
deliverables.

**L16. IBM Plex Mono is embedded as a font file.** `docs/design/ui.md` already
chose Plex Mono for ids, paths and diffs, and Freya's default font stack on Linux
has no monospace face. The file and its OFL licence go in the repository; the
download needs the user's explicit permission when phase 06 reaches it.
Evidence: `ui-and-app-as-built.md` (fonts).

## Open, carried into the phases

Lettered Q to keep them apart from the program's O1-O6 in
`docs/work/daily-loop/brainstorm.md`, one of which (O1) this packet closed.

- **Q1.** What Fork does where its own documents are silent. Its evidence record
  lists eight such points; these are the ones a decision here rests on: whether
  the committer is hidden when it matches the author, the maximum context, how
  renames and mode changes are drawn in the diff, what multi-selection does in a
  commit's Changes tab, whether the Windows build remembers the active tab, the
  scope of the Mac build's side-by-side setting, how the Mac build orders two
  selected commits, and the exact trigger for "too large" on Mac. Where the PRD
  takes a position on one of these it is Cairn's choice and says so, not a claim
  about Fork. Settled only on the user's own Fork install.
- **Q2.** The Expand All line budget. Fixed in phase 08 against the window check,
  as a named constant with a test.
- **Q3.** How far gix's rename detection is from git's on
  `5a3292f163d`, where git finds 2,505 of its 2,543 inexact renames in a
  filename-matching pass that the rename limit does not govern. Measured in phase
  02; each gap is filed, and a gap large enough to question the bar goes to the
  user.
