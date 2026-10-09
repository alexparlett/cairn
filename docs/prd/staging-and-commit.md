---
status: in-flight
packet: staging-and-commit
opened: 2026-10-07
---

# PRD — Staging and commit

**In flight. Authoritative while it is.** The packet's work directory is
`docs/work/staging-and-commit/`; its decisions and rejected alternatives are that
directory's `brainstorm.md`, L1-L26. Requirements below cite them by id.

Design frame: `docs/design/ui.md` (Local Changes, the staging gesture, the commit
box, the confirmation dialog, Show Lost Commits, the activity popover),
`docs/design/diff.md` (selections and patches, the inverted diff),
`docs/design/engine.md` (D1: the writes this packet runs and the reads beside
them), `docs/design/concurrency.md` (the local write lane),
`docs/design/processes.md` (prompts during a local write, the inherited roster),
`docs/design/feature-inventory.md` ("Recovery"). Program:
`docs/work/daily-loop/roadmap.md` packet 5, under program decisions L4 (the reflog
ships with the first commit-level destructive operation), L5 (closed here as L2)
and L6 (a measured bar). Evidence, all under `docs/research/staging-and-commit/`:
`write-path-as-built.md`, `staging-surface-as-built.md`,
`fork-staging-and-commit.md`, `git-write-verbs.md`, `precedent-study.md`,
`freya-ui-apis.md`, `patch-mechanics-spike.md`.

## What this packet delivers

The working tree becomes something Cairn changes, not only shows. In Local
Changes, a file, a hunk, a run of lines or a mode change can be staged, unstaged
or discarded, as Fork does it: double-click, Return, drag between the lists, the
header buttons or the context menu for files; a hovered chunk's floating buttons,
narrowed by a drag-selection, for hunks and lines. A commit box under the diff
commits what is staged, or amends `HEAD`. Every destructive operation — a discard,
deleting untracked files, an amend, removing a stale lock — is sealed behind a
confirmation that names what it costs, computed by the engine and re-checked the
moment before it runs. What a destructive operation on commits leaves behind is
recoverable from Show Lost Commits, and every operation can be read back,
with the prompt the user accepted, in the activity popover.

In build order: the seal; the patch engine's inversions; the write verbs; the
local write lane; commit and amend in the engine; the render layer's keys,
dialogs and menus; Local Changes' actions; the diff's gestures; the commit box;
Show Lost Commits; and the activity popover.

Stash and `.gitignore` are packet 5b's (`stash-and-ignore`, L1).

## Requirements

### R1 — The seal is bound to what it costs (L3)

- R1.1 `cairn_model::Confirmed` derives no `Clone` and no `Copy`, and is
  constructed only by the confirmation surfaces on a roster the guard holds: the
  confirmation dialog's component and the commit box (R10.6).
- R1.2 A `Confirmed` carries a `Consequence`, an engine-computed `cairn-model`
  value naming what the operation will destroy — per operation: the paths, the
  lines per modified path and the bytes per untracked file (Fork's wording, L8),
  the blob ids the destruction is computed against, a commit's id, a lock file's
  path and age — and the prompt text rendered from it. The prompt is rendered
  from the `Consequence`, never typed beside it. (Amended 2026-10-08, the user's
  decision on phase 01's QA item 34a: "the lines and bytes per path" became
  lines per modified path and bytes per untracked file. Amended again
  2026-10-08, the user's decisions 5 and 6 on phase 03's QA: a mode change
  discarded with a whole file is named with both modes beside its lines — "1
  modified (2 lines and the mode change (100644 to 100755))" — and never reads "0
  lines"; and a file added with `git add -N`, which `git restore` leaves empty
  with its intent-to-add entry in place, is named as that — "1 new file emptied
  (20 lines)".)
- R1.3 Every destructive operation of this packet (R1.5) takes `Confirmed` by
  value, and the guard holds the roster: a destructive operation without it, or
  a roster entry with no such operation, fails.
- R1.4 Immediately before it runs, a destructive operation re-reads the state its
  `Consequence` names and refuses, writing nothing, when anything differs; the
  refusal is an outcome of its own ("changed since you confirmed"), never a
  failure of git's.
- R1.5 Destructive here: discard lines (R3.3), discard files and delete untracked
  files (R3.5), amend (R6.4), remove `index.lock` (R12.4), and Create Branch's checkout
  with "Discard" (R11.3). Not destructive: stage and unstage of files, lines or a mode
  change, commit, create a branch, and create and check out a branch keeping the changes
  (R11.3). (Create Branch's discard added 2026-10-09, the user's decision 3.)
- R1.6 `ops::Performed` records the `Consequence`'s prompt for every destructive
  operation, as today's `Performed::destructive` records the token's text.

### R2 — The patch engine (L17, L18)

- R2.1 `TextDiff`, `Selection` and `ChangedFile` gain pure inversions — old and
  new sides swapped, each change's two spans swapped, the selection's removed and
  added sets swapped, paths, modes and ids swapped and added with deleted — so
  unstage and discard emit through the existing forward rule and no `-R` is run
  (L17a).
- R2.2 What each action emits: stage lines — the unstaged diff (index to working
  tree) as it is; unstage lines — the staged diff (`HEAD` to index) inverted;
  discard lines — the unstaged diff inverted. Always at three lines of context,
  from the exact diff (R1.6 of `diff-engine`).
- R2.3 An untracked or newly added file's lines can be staged (a `new file mode`
  patch of the selected lines) and discarded (a partial deletion, emitted as a
  modification) (L17b). Deletions on either side, binary files, LFS pointers,
  files past the size limits, submodules, type changes and conflicted paths are
  whole-file only (L17c, L25); a submodule is staged and unstaged whole and never
  discarded (R3.10), and a conflicted path is staged whole and never discarded
  (R3.11).
- R2.4 A mode change is its own selectable item: `Selection` holds it apart from
  lines, and a selection of lines alone emits no `old mode`/`new mode` (L17d).
- R2.5 Every path line the emitter writes is C-quoted exactly as git's
  `quote_c_style` quotes it (L17e).
- R2.6 The staged diff pairs renames and copies as `git diff --cached` does under
  the user's configuration (L18). Unstaging lines of a staged rename emits a
  content-only patch at its new path.
- R2.7 The reference applier verifies each emitted patch against the side it
  applies to, and a second, independent derivation — the mirrored rule applied in
  reverse — must produce the same result (the tests' oracle, E1b).

### R3 — The write verbs (L8, L16, L17)

Each a named operation in `crates/cairn-git/src/ops/`, built as a write
invocation, run with git's global `--literal-pathspecs` option before the verb (the
precedent of `reads/patches.rs` and `reads/working_tree.rs`; never the
`GIT_LITERAL_PATHSPECS` variable, since an invocation's environment is built only in
`process/environment.rs` and the environment twin refuses one set anywhere else) and
`--end-of-options` or `--` before paths, many paths passed by
`--pathspec-from-file=- --pathspec-file-nul` where the verb takes it. Residual,
stated in `docs/design/engine.md`: git exports literal-pathspec mode to the hooks it
runs under these verbs (`GIT_LITERAL_PATHSPECS=1` in a `post-index-change` or
`post-checkout` hook's environment), so a hook's own globbed pathspec matches
literally.

- R3.1 Stage lines or a mode change: `git apply --cached --whitespace=nowarn -`,
  the patch on stdin.
- R3.2 Unstage lines or a mode change: `git apply --cached --whitespace=nowarn -`,
  the inverted patch on stdin.
- R3.3 Discard lines or a mode change (destructive): `git apply --whitespace=nowarn -`
  on the working tree, the inverted patch on stdin.
- R3.4 Stage files: `git add`; unstage files: `git reset -q --` (which works on an
  unborn branch); unstage a staged rename whole: `git reset -q -- <old> <new>`;
  unstage out of an amend: `git reset -q HEAD^ --` (R6.3), and out of a root
  commit's amend, which has no `HEAD^`: `git rm --cached -f -q --`. (Amended
  2026-10-09 by phase 05 and ratified by the user on 2026-10-09 (decision A): `-f`, since `git rm --cached`
  alone refuses a path whose staged content differs from both the file and `HEAD` —
  the path an amend's staged list shows edited — where the unstage drops that content
  as `git reset` would and keeps the file.)
- R3.5 Discard files (destructive): `git restore --worktree --`, never for a
  submodule (R3.10) or a conflicted path (R3.11) — an intent-to-add file is left
  empty with its entry in place, as `git restore` leaves it (R1.2); delete
  untracked files (destructive): `git clean -f -q --` the exact files status listed,
  never `-d`: status lists untracked files one per file, and the one directory it
  lists whole, a nested repository, is refused before any confirmation (L8), so
  a file added beside a confirmed one is never taken and the discard goes ahead
  for the confirmed files; a path any of whose directories is not a real
  directory — a file, or a symlink — is refused before any confirmation and by the
  re-check after one, since restoring it would replace what stands there. (Amended
  2026-10-08, the user's decision 3 on phase 03's QA, to what is built: it read
  "`-d` only for a collapsed directory row".) `git clean` takes no pathspec file, so its paths go on `argv` after `--`;
  a list past a bound is split across several invocations, all under the one
  `Consequence` re-check made before the first (R1.4). The bound is phase 03's to
  measure: `argv`'s limit for a very large selection is unverified
  (`git-write-verbs.md` §11). There is no Clean command and no Discard All. (`-q` added 2026-10-09 in phase 05, the user's decision 12 on phase 04's QA: a `git clean` left running by a second close then finishes its batch rather than dying of `SIGPIPE` at its first line.)
- R3.6 Staged changes are never discardable (L8): no verb, no gesture — with one stated
  exception, Create Branch's "Discard" before its checkout (R11.3), which discards staged and
  unstaged changes alike behind its own sealed confirmation. (Exception added 2026-10-09, the
  user's decision 3.)
- R3.7 Before every apply (R3.1-R3.3) the operation checks the content it was
  built from (L17f): for stage, the index entry's blob is the diff's old id; for
  unstage, its new id; for discard, the index entry is the old id and the working
  tree file hashed in git's form (`git hash-object --path=<p> -- <p>`, a read,
  no `-w`) is the drawn new side; for a partial discard of an untracked file,
  which has no index entry, that the index holds no entry for the path and the
  working-tree hash is the drawn new side. The index is read with gix. A stale
  patch writes nothing and reports which path moved.
- R3.8 Every local outcome carries the lock files present before and stranded
  after it (#44), and its error names them.
- R3.9 `git hash-object` (R3.7, and R1.4's re-check of a discard of lines) and
  `git rev-parse --git-path hooks` (R6.6) are reads in
  `crates/cairn-git/src/reads/`, each a named function, query plumbing under a
  read's environment, run inside the operation that needs it on the local lane
  and ended with that operation, never by a query's epoch; D1's list of reads
  git answers names them. R1.4's re-check of every discard, of lines or of files,
  also compares the working-tree file's bytes and executable bit as they are on
  disk — the bytes hashed in process with no filter (what `git hash-object
  --no-filters` gives, and a symlink as its target, which `hash-object` cannot
  give) — so an edit git's form does not show, a line ending alone, or a `chmod`
  refuses it; a discard of files compares the bytes and the executable bit alone.
  (Amended 2026-10-08 by phase 03 on phase 01's QA item 25, which left the
  deletion re-check's hashing to it, and ratified by the user on 2026-10-08,
  reworded to the bytes and the executable bit.)
- R3.10 A submodule's changes are not discardable (L24): `git restore -- <sub>`
  exits 0 and leaves the submodule's commit where it was (`git-write-verbs.md`
  §3), only `git submodule update` moves it back, and no prompt can count what is
  dirty inside it. No verb discards one, and a submodule row offers no discard by
  any route and says why.
- R3.11 A conflicted path is staged whole with `git add`, which marks it resolved,
  as Fork does (L25); it takes no line or mode patch and no discard (Fork refuses,
  TrackerWin #1859), by any verb. Resolving a conflict's content is the second
  lap's (D6).

### R4 — The local write lane (L4, L15)

- R4.1 Local writes run on a thread of their own, which requests reach directly,
  never through the repository thread.
- R4.2 Writes run one at a time in the order asked; a write asked while another
  runs is queued and drawn as queued; nothing is refused for being second.
- R4.3 Each write carries an operation id; its cancel names that id (#47's shape
  for the local lane). Only a commit or an amend is cancellable, and only while
  it runs.
- R4.4 A status read that began before the latest write ended is discarded, never
  drawn. This narrows refs-and-status R10.3 (`docs/prd/refs-and-status.md`,
  frozen; the user's decision that a running status is never superseded): such a
  status's answer is dropped and the refresh after the write draws instead, but its
  process is not ended — only closing the window ends a running status.
- R4.5 After a write, Cairn refreshes what its `Invalidated` declares and no more:
  a stage, unstage or discard refreshes status; a commit or an amend refreshes
  refs, status and the history; creating a branch refreshes refs and the history.
- R4.6 While a commit runs, Cairn starts no refresh of its own (focus, the Refresh
  chord, after another operation) and no queued index write; they run after it
  ends.
- R4.7 A cancelled or unwatched write reports that it may have taken effect; the
  refresh after it shows what did (#45 item 1).
- R4.8 The runner's bounded-output helpers are callable on reads only, at the
  type (#45 item 2).
- R4.9 The first close requested during a local write waits for it, saying which
  ("Finishing commit…"), and never kills it; a second request after
  `CLOSE_PATIENCE` closes anyway, as today, and a lock it strands is named the
  next time the repository opens (#45 item 3, #44 item 2).

### R5 — Prompts and the environment during a write (L11)

- R5.1 Every local write carries an askpass token (L11), and the window shows a
  prompt during any write, not only during a fetch.
- R5.2 `GNUPGHOME`, `DISPLAY`, `WAYLAND_DISPLAY` and `XAUTHORITY` (#18, L11), and
  the five identity variables `GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`,
  `GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL` and `EMAIL` (L26), join the
  inherited roster (`process/environment.rs`'s `INHERITED`), each with its reason
  beside it. The date variables (`GIT_AUTHOR_DATE`, `GIT_COMMITTER_DATE`) do not:
  a stale one would stamp every commit.
- R5.3 A hook that opens `/dev/tty` when Cairn was launched from a terminal stops
  rather than failing; this is stated as a residual in `docs/design/processes.md`
  and filed, not fixed (L11).

### R6 — Commit and amend in the engine (L9, L10, L11, L12)

- R6.1 Commit is `git commit -F -`, the message on stdin; no `--cleanup` is
  passed, so `commit.cleanup` and git's `-F` default decide; `--no-verify` is
  passed only from the hook failure's skip (R10.5). A non-UTF-8
  `i18n.commitEncoding` is refused before git runs, with its reason. (Amended
  2026-10-09 by phase 05 and ratified by the user on 2026-10-09 (decision F): `-q` beside it, which leaves
  out only the summary git prints once the commit is made — by analogy with the
  user's decision 12 of 2026-10-09, which gave `git clean` `-q` so an orphaned write
  has nothing to print; and no `--literal-pathspecs`, which git would export to the
  hooks.)
- R6.2 Amend is `git commit --amend -F -` with the same rules.
- R6.3 In amend mode the staged list is the index against `HEAD^` (the plumbing
  the working-tree query already runs, `git diff-index --cached`), and unstaging
  from it is R3.4's `git reset -q HEAD^ --`. A root commit's amend compares
  against the empty tree, and unstaging from it is R3.4's `git rm --cached -f -q --`,
  since there is no `HEAD^`. Amend is unavailable with no `HEAD` (an unborn
  branch) and while a merge is in progress (R6.9).
- R6.4 Amend is destructive (R1.5): its `Consequence` is `HEAD`'s id, subject,
  whether git will write the reflog entry R10.6's text depends on, and
  whether a remote already has it — from the upstream's ahead count, or, with no
  upstream or one that does not hold `HEAD`, a hidden walk of `HEAD --not --remotes`.
  (Reflog state added 2026-10-08 with R10.6, the user's decision. "Or one that does
  not hold `HEAD`" amended 2026-10-09 by phase 05 and ratified by the user on
  2026-10-09 (decision B, accepting the QA verdict that it was a defect): phase 05's
  QA item 1 found that asking the upstream alone answered "unpublished"
  for a commit another remote branch holds.)
- R6.5 Commit and amend carry an askpass token (R5.1) and stream their stderr to
  the operation log (R12.1).
- R6.6 Whether a `pre-commit` or `commit-msg` hook exists is answered from the
  hooks directory git resolves (`git rev-parse --git-path hooks`, so
  `core.hooksPath` counts); a hook counts when its file exists and is executable,
  as git runs it.
- R6.7 Recent commit messages are the messages of the latest 10 commits on the
  current branch, newest first, as Fork's are (`fork-staging-and-commit.md` §5,
  Tracker #720), read from the history on a worker; nothing is stored.
- R6.8 The commit identity is git's (L10, L26): Cairn passes no author or
  committer and reads no identity of its own, so git uses the one the user's
  terminal would — its configuration and R5.2's inherited identity variables —
  and when git has none, its own error is shown in R10.5's dialog.
- R6.9 The engine reports the operation in progress as git records it (L25): a
  merge (`MERGE_HEAD`, with git's `MERGE_MSG`), a rebase, `git am`, a cherry-pick or
  a revert. With a merge in progress, R6.1's commit is the merge commit, its message
  the user's draft; with a rebase, `git am`, a cherry-pick or a revert in progress,
  commit and amend are refused with the operation's name before git runs. (`git am`
  added 2026-10-09, the user's decision C.)

### R7 — Keys, dialogs and menus in the render layer (L5, L7, L8, L19)

- R7.1 One pre-key policy serves every text field: accelerator chords and lone
  modifier keys reach the window unclaimed; ⌘Return / Ctrl+Enter commits without
  a newline; a primary+letter chord that is not an editing binding types nothing.
  The filter fields adopt it, closing the hole where a focused field hid the
  window's chords and held modifiers.
- R7.2 The accelerator table maps an action to a list of distinct chords per
  platform, where it held at most one (L22), so Fork's alternates survive — stage
  on Return and ⌘S, discard on Backspace, Delete and Ctrl+Shift+D. The root
  `CLAUDE.md` modifier invariant ("at most one chord per platform") and its twin
  are amended with it, in phase 06. The table admits bare Enter, Backspace and
  Delete in exactly one scope — a focused file list or diff in Local Changes — and
  the pinned rule is amended to say so; nowhere else is a bare chord other than a
  function key.
- R7.3 New actions, each with Fork's chords per platform (Linux as Fork for
  Windows) and heard in its own scope only, so no list or staging chord fires while
  the commit box has focus; the window's chords still do, as from the filter fields
  (amended 2026-10-09, the user's decision on phase 06's QA item 13: it read "no chord
  but commit's fires"): in Local Changes' list-and-diff scope, stage/unstage
  the selection (Return or ⌘S; Enter or Ctrl+Shift+S), stage all / unstage all
  (⌥⇧⌘S; Ctrl+Alt+Shift+S) and discard the selection (⌫ or ⇧⌘D; Backspace, Delete
  or Ctrl+Shift+D); in the commit box, commit (⌘Return; Ctrl+Enter); in the
  history, Show Lost Commits (⌘⇧.; Ctrl+Shift+.); and, in Local Changes' lists,
  the multi-select presses (Ctrl/⌘-click, Shift-click, Shift+↑/↓).
- R7.4 A confirmation dialog: modal to assistive technology and to the keyboard
  (Tab stays inside it; the window's chords do nothing while it is open), focus
  on Cancel, its prompt and its button rendered from a `Consequence`, Escape
  cancels. Every dialog's buttons sit in the platform's order — on Linux the primary first
  and Cancel last (Fork for Windows), on macOS Cancel first and the primary last (Fork for
  macOS) — focus on Cancel on both (amended 2026-10-09, the user's decision E).
- R7.4a A right-click on a list row selects it — the history's or Local Changes' lists' — so
  the detail pane follows and its menu acts on what is shown; one inside a selection already
  made keeps the whole selection (added 2026-10-09, the user's decision C; Fork for Windows).
- R7.5 A context menu host is mounted at the window root, so every menu opens
  without the toolkit panicking for want of one.
- R7.6 Edge auto-scroll for a drag over a virtualized list, timed with
  `async-io`'s timer (L5); the drag is tracked by the list, never by a row.

### R8 — Local Changes acts (L6, L7, L8)

- R8.1 Both lists take a multi-selection. The lists stay flat (a tree view is
  #36, not built here); a collapsed untracked-directory row, where status lists
  one, acts on everything under it. (Added 2026-10-09, the user's decision on phase
  07's QA item 4(e): with several files selected the diff draws the selected files'
  diffs together, as Fork does, on the Commit tab's layout of files opened in place, each
  file under its own row with R9's gesture over its rows; built in phase 08, which reads
  them under Expand All's line budget — the files past it not drawn, each saying to choose
  it alone — a bound for the user's ratification.)
- R8.2 Files stage and unstage by double-click, R7.3's chords, a drag from one
  list to the other (one drop zone per list), the header's `Stage` / `Unstage`
  (⌥-held: Stage All / Unstage All) and the double-chevron Stage All, and the
  context menu (Stage or Unstage, Discard Changes…, Stage All, Copy Path).
  (Added 2026-10-09, the user's decisions on phase 07's QA: item 11 — with a filter on,
  Stage All and Unstage All take the rows the filter shows and no others, and every row
  with none on; item 4(d) — the double chevron sits in Unstaged's header, as Fork for
  Windows draws it, and the ⌥-held press is Alt on Linux, where xfwm, openbox and Plasma 5
  take Alt with the first button to move a window, so the press fails safe there and the
  chevron, the menu and the chord still reach Stage All and Unstage All; item 4(b) — a
  double-click acts on its own row, since its first press makes the row the selection.)
- R8.3 After a stage or unstage, the selection moves to the nearest remaining path
  in the list it left (Fork, Tracker #514).
  (Added 2026-10-09, the user's ratification on phase 07's QA item 4(a): for a selection
  with gaps, the nearest remaining path is the one that slides into the first acted
  row's place, else the nearest above it.)
- R8.4 Discard on the unstaged side only, through R7.4's dialog: Fork's words with
  counts (L8), the button naming the count. The confirmation cannot be skipped
  (L8): no modifier, setting or other route reaches a discard without the dialog.
  A selection of untracked rows deletes them; a nested repository among them
  refuses with its reason before any dialog.
  (Added 2026-10-09, the user's ratification on phase 07's QA item 4(c): a discard asked
  on the staged side — the chord on the Staged list or a staged diff — asks nothing and
  says "Staged changes can't be discarded: unstage them first.", where Fork does nothing;
  a deviation from Fork named in `docs/design/ui.md`.)
- R8.5 Ignore Whitespace is disabled in Local Changes, and the diff drawn there is
  always the exact one; the shared setting is untouched (L6).
- R8.6 A queued, running or failed write is drawn where the user acted; a stale
  patch's refusal names its path.
- R8.7 A conflicted row stages whole, by every R8.2 route, as `git add`, which
  marks it resolved (R3.11, L25); it offers no line gesture and no discard by any
  route.
- R8.8 A submodule row offers no discard by any route, and says why (R3.10, L24).

### R9 — The diff's staging gesture (L5, L17d)

- R9.1 In Local Changes' diff only, hovering a chunk outlines it and floats its
  actions: `Stage` and `Discard Changes…` in the unstaged diff, `Unstage` in the
  staged one. They take no focus, live outside the recycled rows, and follow the
  hover after the rows change.
- R9.2 A drag-selection across lines narrows the floating actions and R7.3's
  chords to the selected changed lines (`Stage 2 Lines`, `Discard 2 Lines`); with
  nothing selected the chords act on the whole file. The drag continues across
  rows the virtual list unmounts, and auto-scrolls at the edges (R7.6).
- R9.3 In side by side, a selection stays within the column it began in.
- R9.4 A mode change is drawn as a row of its own with its own actions (R2.4).
- R9.5 The Commit and Changes tabs draw no staging action.

### R10 — The commit box (L9, L11, L12)

- R10.1 Under Local Changes' diff: a subject field with a characters-left counter
  (soft at 50, red past 70), a multi-line description with a ruler at column 72,
  `Amend`, and `Commit N Files`, disabled with an empty subject, and while nothing
  is staged unless amending or concluding a merge in progress (R10.8). `Amend` is
  disabled with no `HEAD` and while a merge is in progress (R6.3). (Amended
  2026-10-09, the user's decisions on phase 09's QA: the subject is required, as
  Fork's is — Tracker #1490, TrackerWin #637 — and a merge in progress commits with
  nothing staged, concluding the merge, as Fork's has since 1.0.57 — Tracker #90;
  evidence in `docs/research/staging-and-commit/fork-merge-and-amend-evidence.md`.)
- R10.2 Recent Commit Messages (R6.7) as a menu beside the subject and ↑/↓ in an
  empty or recalled subject; a recalled message fills both fields.
- R10.3 Ticking `Amend` fills an empty draft with `HEAD`'s message and shows
  R6.3's staged list; the draft the user had is kept and returns when Amend is
  unticked; Amend unticks itself after the commit. Where amend's staged list cannot
  be read — a partial clone's missing blob, which Cairn's reads never fetch — the
  amend is still offered, and the box says the lists show what is staged against
  `HEAD`. (Added 2026-10-09, the user's decision on phase 09's QA; Fork has no
  evidence here and would likely lazy-fetch.)
- R10.4 A running commit is drawn as busy with its elapsed time and a Cancel; the
  hooks' output streams into the operation log (R12).
- R10.5 A failed commit opens Fork's `Git Error` dialog — the command and git's
  output with ANSI sequences stripped — offering `Skip pre-commit hooks and
  commit` only where R6.6 finds a `pre-commit` or `commit-msg` hook; the draft is
  kept. The output is drawn through a virtualizing view or bounded to the retained
  tail, never a `ScrollView`.
- R10.6 In amend mode the button reads `Amend <short id>` above "Replaces <short
  id> '<subject>'. The old commit stays in Show Lost Commits." when git will write
  the reflog entry that keeps the replaced commit findable, and above "Replaces
  <short id> '<subject>'. The old commit can't be recovered afterwards: this
  repository keeps no reflog." when it will not; pressing it builds the
  `Confirmed` from that text and R6.4's `Consequence`, which carries which of the
  two holds (`cairn_model::Reflog`). git writes the entry when
  `core.logAllRefUpdates` is `true` (the default with a working tree; `false` in a
  bare repository) or `always`, or, whatever it is set to, when the ref's log
  already exists, since git appends to an existing log. When a remote already
  has `HEAD`, R7.4's dialog asks first: "<short id> is already on <remote ref>.
  Sharing the amended commit needs a force push." (Amended 2026-10-08, the user's
  decision on phase 01's QA item 13: the recovery sentence is conditional on the
  reflog, never promised unconditionally.)
- R10.7 The draft survives refreshes, a failed hook and Amend's toggling, for the
  life of the window.
- R10.8 With a merge in progress (R6.9) the box fills an empty draft, once per
  merge — a message the user clears stays empty — with git's
  `MERGE_MSG` as git wrote it — its `# Conflicts:` comment lines included, left
  visible for the user to delete by hand, since under `-F` and the default cleanup
  they are committed if left — and its commit is the merge commit, as Fork's is
  (L25). During a rebase, `git am`, a cherry-pick or a revert the box is disabled and
  names the operation in progress. (Comment lines kept visible and `git am` added
  2026-10-09, the user's decisions E and C. Once per merge added 2026-10-09, the
  user's decision on phase 09's QA: Fork does not document a cleared message; its
  vendor's rule that an auto-filled message is replaced only until the user edits
  it, Tracker #61, points this way — inferred.)

### R11 — Show Lost Commits (L13)

- R11.1 A toggle (R7.3's chord, and a control in the history's toolbar area) adds
  the old and the new id of every entry of `HEAD`'s reflog and of each local branch's
  reflog to the walk's tips — as `git rev-list --reflog` and `git fsck` read a reflog
  — and draws the commits no ref reaches dimmed. The old ids are what keep an amend's
  replaced commit findable when no entry names it as its new id (a log the amend
  itself created), so the amend prompt's "The old commit stays in Show Lost Commits."
  holds whenever git writes the entry (R10.6). Residual, stated: git expires a reflog
  entry for an unreachable commit after `gc.reflogExpireUnreachable` (30 days by
  default), and from then on Show Lost Commits no longer has it. (Old ids added
  2026-10-09, the user's decision D.)
- R11.2 A reflog is read whole, never through gix's newest-first iterator, which
  stops at a line over 4 KiB.
- R11.3 Every commit row's context menu — a dimmed one's too — offers Fork's `New Branch…`,
  which opens Fork's Create Branch dialog (the user's decision, 2026-10-09, amending "A
  dimmed commit's context menu offers `Create Branch Here…`: a name, then `git branch`";
  evidence `docs/research/staging-and-commit/fork-create-branch-evidence.md`):
  - Titled "Create Branch": "Create branch at:" the commit's glyph, short id and subject,
    read only; "Branch name:" a field (placeholder "Enter branch name"); "Check out after
    create"; Cancel, and "Create" — "Create and Checkout" while the box is ticked. Return
    presses the button, Escape cancels.
  - A name is refused inline, before git runs, by git's own rules (`git check-ref-format
    --branch`, a read), when a local branch has it, when a branch's folder holds it or it
    would hold a branch (git's words), and when it holds `@{` ("A branch name can't contain
    '@{'", the user's decision F, 2026-10-09): the button disabled, the reason inside the
    dialog beside the buttons behind Fork's warning triangle, never in the Git Error dialog
    (decision B) — Fork's words for a name taken ("Branch test already exists"), git's for a
    name it refuses. While the check waits behind a running write, the same place says which
    ("Waiting for the commit to finish…", decision D). Under the title, Fork's "Use '/' as a path
    separator to create folders"; Fork's chord, Ctrl+Shift+B (⇧⌘B), opens the dialog at
    `HEAD` (Fork-settled at phase 10's QA).
  - "Check out after create" is sticky for the session (decision 1); across restarts once
    Cairn has a settings store (issue #89).
  - Unticked: `git branch -- <name> <oid>`. Ticked, while the working tree has staged,
    unstaged or conflicted changes, a "Local changes:" group: "Don't change" (the default, and
    each opening's): `git checkout -q -b <name> <oid> --`, git's refusal ("would be
    overwritten") shown in Fork's Git Error dialog with git's own words and the name kept for
    the next opening; "Discard" (decision 3): destructive (R1.5) — an engine-computed
    `Consequence` naming every staged and unstaged change to a tracked file lost and every
    untracked file the commit's tree overwrites, or deletes for being in a folder or nested
    repository where the commit holds a file, counted as the discard prompts count (lines per
    changed file, bytes per untracked file, untracked files kept said) and each such folder
    and repository named by its path — "Deleted because the branch has a file there: folder
    d/ (4 untracked files) and repository vendor/lib/ with its history", the first three
    named and the rest counted — under the title "Discard changes" and the button "Discard
    Changes and Check Out" (the user's decision A, 2026-10-09), confirmed through the
    confirmation dialog, re-checked before `git checkout -q -f -b <name> <oid> --` runs
    (R1.4), refused before any prompt during an operation in progress, over a conflicted path
    or a submodule's change; never the remembered choice. Fork's third choice, "Stash and
    reapply", waits for packet 5b, which builds stashing (decision 2: a temporary, stated
    deviation).
  - After it: the refs and the history are read again; the new branch's chip is on the commit,
    and with the checkout it is `HEAD`. Checking out enters this packet only through this
    dialog; checking out a branch is branch-ops'.
- R11.4 The toggle is a reopen of the history like any other (refs-and-status
  R10), cancellable and off the UI thread.

### R12 — The activity popover (L14, L15)

- R12.1 Fork's Activity popover, off the toolbar's status: one entry per
  operation of the session, newest first, bounded — its name, start time,
  duration, outcome, the prompt it confirmed when it had one, each `git` it ran
  (from the command log) with its stderr tail, and a recovery pointer where one
  exists (an amend points at the replaced commit, which Show Lost Commits draws).
  The list is virtualized and each stderr tail is drawn through a virtualizing
  view or bounded, never a `ScrollView`.
  Amended 2026-10-09, the user's decisions on the popover as built: it hangs from
  the status box — its left edge under the box's, an arrow pointing at it — kept
  inside the window's width, never centred (A); the right pane's order, the status
  words, the empty-state line, ×, the duration format, the command endings, the
  newest entry selected, the 200-entry / 10,000-line / 4 MiB bounds and the error
  message's three-line cut stay as built (B, C); the start time stays `HH:MM:SS
  UTC` — local time, here and in the history's date column, needs a timezone
  dependency, the user's to decide, and is filed as one issue (D); each write is
  named in Fork's imperative form (E) and the way back keeps "Show Replaced
  Commit" (F); while the popover is open the window's chords and the focused
  list's keys do nothing, as under a dialog, Escape alone closing it (I); a fetch
  refused because another runs gets no entry (J); the confirmed prompt keeps its
  four-line cut with "Show All", which shows it whole in place, scrolling with the
  pane, and collapses again (K); each `$ git …` line quotes every argument as a
  POSIX shell needs it — single quotes, an embedded `'` written `'\''`, bare when
  it needs none — so it pastes into a terminal (L); an operation whose lines the
  byte bound let go of says "Its output was let go to make room for newer
  operations." in their place (M); a write refused before it started keeps its own
  name, such as "Stage 2 files" (N); and Create Branch's wait reads "Waiting for
  the branch to be created…".
- R12.2 Credentials in a URL (`scheme://user:secret@host`) are removed from every
  stderr line before it is drawn (#46, for display).
- R12.3 Nothing about the log persists past the window (L14). It closes #41.
- R12.4 Where an outcome names a stranded `index.lock` and Cairn's registry holds
  no running `git` for that repository, the entry offers `Remove index.lock…`
  (destructive): its `Consequence` the lock's path and age, its prompt saying
  another program may still own it. git has no verb that removes a lock, so this
  is the one file Cairn deletes without `git` (L23, D1 amended): a `std` filesystem
  removal of exactly `<gitdir>/index.lock`, made in `ops/` only, after re-checking
  the lock's age and identity against its `Consequence` (R1.4), so a lock removed
  and made again since the prompt is refused.
  Amended 2026-10-09, the user's decisions G and H: the confirmation is titled
  "Remove stale lock", its button stays "Remove index.lock", and its prompt reads
  exactly "Remove <path>? It was last changed N ago and holds N bytes. Another
  program may still own it: removing a lock a running git holds can corrupt the
  index. You can't undo this action." (G). The offer and its `Consequence` are
  computed when `Remove index.lock…` is pressed, on the local lane, not once as the
  write ends — so the age is current, and the offer is not lost because some `git`
  of Cairn's happened to run as the write ended; while one runs, the button is
  drawn unpressable and says why rather than vanishing, and a refusal at the press
  is said beside it (H). A lock found as the repository opens is an entry of its
  own, "Lock files found as the repository opened", naming each lock and carrying
  the same offer where `<gitdir>/index.lock` is among them (H).
- R12.5 A guard holds every filesystem-mutating call in production code (removing,
  writing, renaming, creating or changing the permissions of a file or directory)
  to `crates/cairn-git/src/ops/` (L23), with a nonzero-files assertion and a
  matcher self-test; what writes outside any repository (the askpass channel's
  socket directory, `crates/cairn-askpass/src/channel.rs`) is an exceptions roster
  of the guard's whose rows fail when no longer needed. Packet 5b's `.gitignore`
  write reuses it. D1 in `docs/design/engine.md` and the root `CLAUDE.md` state
  the one exception to "every mutation goes through `git`" and its reason.

### R13 — Measured (L20)

- R13.1 On a plain, non-shared clone of rust-lang/rust at `c999cef531e` on tmpfs
  — never `~/Development/bench/rust` itself — warm, release build, median of
  seven: stage a hunk, unstage a hunk, discard a hunk and commit, each from the
  press until the refreshed lists are drawn; Show Lost Commits from the toggle to
  its first frame.
- R13.2 git's own time for each verb plus one status read is measured first, into
  `docs/research/staging-and-commit/measured-baseline.md`; this PRD's C21 then
  gains its margin by amendment, before any Cairn number is taken. (Amended
  2026-10-08: measured with git 2.56.0 on a tmpfs clone at `c999cef531e` — stage or
  unstage a hunk 17.4-17.8 ms, discard 0.7 ms, commit 11.0-12.3 ms, one status read
  23.7-25.1 ms — and the margin decided by the user on 2026-10-08: a flat 50 ms per
  verb, added to git's own time plus one status read.)

## Product rules

- **What is staged is what was selected, from the exact diff.** No view setting,
  no user `apply.whitespace`, no stale index changes what a patch stages.
- **A destructive operation states its cost first, and only what it will cost.**
  The prompt is computed by the engine and re-checked before the operation runs.
- **No backup is promised.** A discard says it cannot be undone, because it
  cannot (L2).
- **Fork's mechanics, every deviation named:** Cancel focused in the discard
  dialog (L8); Ignore Whitespace disabled in Local Changes (L6); prompts that count
  what is lost by kind and size, where Fork's name a count (L8); the amend button
  naming the commit it replaces (L12); the activity popover quoting each confirmed
  prompt and pointing at the way back (L14); `Remove index.lock…` (L15, L23); a
  nested repository refused before any dialog (L8); a submodule row offering no
  discard, where Fork offers `Discard Submodule Changes` (L24); the commit box
  disabled during a rebase, cherry-pick or revert, where Fork pre-fills git's
  message for a cherry-pick or revert (L25); a Show Lost Commits control in the
  history's toolbar area, where Fork has only the View menu item and the chord and
  refused a toolbar button (TrackerWin #378) — a check box at the right end of the "Graph and
  subject" heading's cell, its tooltip the chord (the user's decision, 2026-10-09); Create
  Branch's "Local changes" without "Stash and reapply" until packet 5b builds stashing, and
  its "Discard" confirmed where Fork's is not (the user's decisions 2 and 3, 2026-10-09);
  Show Lost Commits seeded from
  `HEAD`'s and the local branches' reflogs, where Fork's mode reaches every reflog
  (`git log --all --reflog`, TrackerWin #1307); and no Commit and Push until push
  exists (L9).
- **A line selection acts only on the answer it was made over** (phase 08's QA, item 1): a
  selection names rows of one diff, and once another answer is drawn under it — a refresh, a
  re-read's page — it is nothing, and no act made under the old answer reaches a write.
- **Files drawn together, as ratified by the user (2026-10-09, phase 08's QA):** they are read
  under Expand All's 50,000-line budget, the files past it not drawn, each saying to choose it
  alone; drawn together, a file offers no Load Diff, no mode row and no previous or next change
  — choosing it alone offers each; with no lines selected the chords act only on the files read
  and drawn, never one past the budget or still being read; and a discard of several files
  names them in its dialog, the first three and how many more (the user's decision,
  2026-10-09).
- **The gesture's floating actions, as ratified by the user (2026-10-09, phase 08's QA):** they
  stand at the list's top while the chunk's top is scrolled above it, where Fork's sit at the
  chunk's top right; a press without a drag, or Escape, lets a line selection go, and the window
  losing focus cancels a drag; and a selection's discard reads `Discard 2 Lines…`, its
  ellipsis saying it confirms, the confirmation's own button `Discard 2 Lines` (the user's
  decision, 2026-10-09).
- **Writes go through `git`; only `ops/` writes.** The one exception is removing a
  stale `index.lock`, which git has no verb for, done in `ops/` (R12.4). Every read
  this packet adds is a named function in `reads/`.
- **The UI thread never waits on a write.** A write is asked and its outcome
  arrives.
- **No network call of Cairn's own.** A hook may make one; Cairn does not.

## Acceptance criteria

The single authoritative copy. `docs/work/staging-and-commit/qa-checklist.md`
points here and does not restate them.

| # | Criterion | Pinned by |
| --- | --- | --- |
| C1 | `Confirmed` is neither `Clone` nor `Copy` and cannot be built outside the roster's surfaces; every rostered destructive operation takes it by value and every such operation is rostered; the prompt is rendered from the `Consequence` | `compile_fail` doctests in `cairn-model`, and a guard with a nonzero-files assertion and a matcher self-test |
| C2 | Each destructive operation refuses, writing nothing, when the state its `Consequence` names moved between confirmation and run: a discarded file edited or chmodded, a file or a symlink standing at a confirmed path's directory (an untracked file added beside a confirmed deletion is never taken, and the discard goes ahead — amended 2026-10-08, the user's decision 3), `HEAD` moved before an amend, a lock removed and recreated | integration tests in `cairn-git` against real `git` |
| C3 | For every case — modification, untracked file, intent-to-add, CRLF under `core.autocrlf`, a clean filter, an unborn branch, a mode change beside edits, a staged rename, and paths with a space, a tab, a quote, a backslash, a newline and invalid UTF-8 — staging, unstaging and discarding a selection of lines leaves the index and working tree equal to the reference applier's answer and to the mirrored-rule derivation's, and `git diff`/`git diff --cached` afterwards show exactly the unselected changes; on the host's git and on 2.30.9 and 2.32.7 (`git-floor`) | integration tests in `cairn-git` |
| C4 | A selection of lines alone stages no mode change, and a mode change stages alone | integration test |
| C5 | With `apply.whitespace=fix` and `=error` set, staging trailing-whitespace lines stages them byte for byte; with `apply.ignoreWhitespace=change` set, staging a selection stages exactly the selected lines (phase 03 measures the setting's effect first, and where it changes what is staged every `apply` pins `-c apply.ignoreWhitespace=false`, which C9 then checks) | integration test |
| C6 | A patch whose index entry or working-tree file moved after it was built writes nothing and names its path, on content where `git apply` alone would land at an offset (spike E6) | integration test |
| C7 | The staged diff of a staged rename equals `git diff --cached`'s pairing under `diff.renames` unset, `false` and `copies` | integration test against real `git` |
| C8 | File verbs: staging, unstaging (including on an unborn branch and out of an amend), discarding and deleting untracked files leave the state `git status` reports as expected; deleting names only status's paths, never passes `-d` (amended 2026-10-08, the user's decision 3), splits a list past R3.5's bound into several invocations under one re-check, and refuses a nested repository before any confirmation; staged changes offer no discard anywhere; a submodule's changes offer no discard anywhere, and its row says why, and no verb discards one | integration tests and headless tests |
| C9 | Every verb's argv is exactly what R3 names — `--literal-pathspecs` before the verb (and no `GIT_LITERAL_PATHSPECS` in its environment), `--whitespace=nowarn`, the pathspec file where used, no `-R` | stub-git tests printing argv |
| C10 | Writes queue in order and draw as queued; a stage asked during a running commit waits for it (against a stub git's long-running write in phase 04, then against real `git commit` in phase 05); a status begun before a write ended is never drawn; a write refreshes exactly what its `Invalidated` names; no refresh starts during a commit (stub, then real, as above); a cancel names its operation and reaches only that one | worker tests through the real boundary |
| C11 | A close during a commit waits and says so (stub, then real, as C10); a second close after `CLOSE_PATIENCE` closes; a stranded `index.lock` is named on the next open and in the failed write's error | worker tests, and headless tests |
| C12 | A local write whose child asks for a secret shows the prompt and is answered (an SSH-signing key with a passphrase, through the sshd fixture's key setup or a stub `ssh-keygen`; the stub's long-running write in phase 04, a real signed `git commit` in phase 05); the inherited roster holds R5.2's nine, each with its reason, and neither date variable; a commit made with `GIT_AUTHOR_EMAIL` set in Cairn's environment has that author | integration test, the process environment twin's roster |
| C13 | Commit and amend: the message reaches git byte for byte under every `commit.cleanup` value as `git commit -F` would leave it; a `pre-commit` hook's failure shows its output and the skip offers only where a hook exists (`.git/hooks`, `core.hooksPath`); the draft survives it; a non-UTF-8 `i18n.commitEncoding` is refused; amend's staged list equals `git diff --cached --name-status HEAD^`, and a root commit's amend works, unstaging from it with `git rm --cached -q --`; Amend is disabled on an unborn branch; with no identity configured, git's own error is shown; recent messages equal `git log -n 10 --format=%B HEAD`'s | integration tests in `cairn-git`, headless tests |
| C14 | Amend's button text and dialog are rendered from its `Consequence`; the dialog appears exactly when a remote has `HEAD` (an upstream at `HEAD`, behind it, none with another remote branch containing it, none at all) | integration and headless tests |
| C15 | Every text field hands accelerator chords and held modifiers to the window: with a filter field focused, F5 refreshes and a Ctrl/⌘-click extends the selection (failing first on today's code); ⌘Return / Ctrl+Enter in the description commits without a newline | headless tests |
| C16 | The accelerator table maps each action to a list of distinct chords per platform, and every chord of R7.3's lists resolves per platform; each new action resolves only in its R7.3 scope — with the commit box focused, the stage, unstage, discard and Show Lost Commits chords resolve to nothing and the commit chord commits; the pin admits bare Enter, Backspace and Delete only in Local Changes' list-and-diff scope; the root `CLAUDE.md` modifier invariant and its twin, amended in phase 06, hold a list per action | the table's tests, the modifier guard |
| C17 | The discard dialog: modal (Tab stays inside, a window chord does nothing), focus on Cancel, Escape cancels, its text and button from the `Consequence` (`Discard Changes in 3 Files`, `Discard 2 Lines`) | headless tests |
| C18 | Local Changes: each of R8.2's routes stages and unstages a multi-selection — but a double-click, which acts on its own row (amended 2026-10-09, the user's decision on phase 07's QA item 4(b)); a drag between the lists auto-scrolls and survives rows unmounting mid-drag; the selection moves to the nearest remaining path; Ignore Whitespace is disabled and the diff exact | headless tests |
| C19 | The gesture: a hovered chunk's actions stage, unstage and discard exactly that chunk; a drag-selection narrows them to its lines, across rows the virtual list unmounted; side by side keeps a selection in one column; the Commit and Changes tabs draw no action; a 10,000-line diff with the gesture builds one viewport | headless tests, and a viewport twin |
| C20 | Show Lost Commits: the commits it adds equal `git rev-list <every old and new id of every entry of HEAD's and each local branch's reflog, read from the log files> --not --branches --remotes --tags HEAD` (amended 2026-10-09, the user's decision D: old ids too, as `git rev-list --reflog` reads them), on a fixture with an amended, a reset-away and a 4 KiB-message entry, and an amend whose log it created itself (the replaced commit only an entry's old id); those rows are dimmed — their text, the graph in its colours (amended 2026-10-09: Fork-settled on Fork's evidence, applied under the user's Fork-first rule); Create Branch (R11.3, amended 2026-10-09) creates the branch `git branch` would, checks it out keeping the changes as `git checkout -b` would or refuses with git's words, and with "Discard" names every loss in a confirmation and discards exactly those; the activity popover lists each operation with its prompt, its `git`, its scrubbed stderr and its recovery pointer, and `Remove index.lock…` appears exactly when R12.4 says and removes exactly `<gitdir>/index.lock` and nothing else; the filesystem-mutation guard (R12.5) fails on a removal, write or rename outside `ops/` | integration and headless tests |
| C21 | On R13.1's clone and machine (the one `docs/research/diff-engine/measured-baseline.md` records): stage, unstage and discard a hunk and commit within git's own time plus one status read plus a flat 50 ms (the user's decision, 2026-10-08, at R13.2's amendment): about 93 ms to stage, 93 ms to unstage, 76 ms to discard and 87 ms to commit, from `docs/research/staging-and-commit/measured-baseline.md`'s highest sums (42.9, 42.8, 25.8 and 37.4 ms); Show Lost Commits' first frame recorded; `window_check` keeps every frame under 16.7 ms of UI-thread work while a hook runs and a stage lands | `#[ignore]`d reporters driven by `CAIRN_BENCH_REPO`, numbers in `progress.md` and, at teardown, in `docs/research/staging-and-commit/` |
| C22 | D1 in `docs/design/engine.md` and the root `CLAUDE.md` names R3.9's two reads, the write verbs and the one file deletion made without `git` (R12.4); the destructive-operation roster, the confirmation-surface roster, the chord lists and the bare-key scope, the gesture's viewport twin (C19) and the filesystem-mutation guard (R12.5) each have their twin named in `CLAUDE.md` | review |
| C23 | `scripts/gate.sh` passes | the gate |
| C24 | Conflicts and operations in progress (L25): staging a conflicted row runs `git add` and `git status` then reports it resolved; a conflicted row offers no line gesture and no discard by any route, and no verb takes a patch or a discard for one; with a merge in progress the engine reports it, the commit box holds `MERGE_MSG`'s text, its commit has `HEAD` and `MERGE_HEAD` as parents, and Amend is disabled; during a rebase, `git am` (added 2026-10-09, the user's decision C), a cherry-pick and a revert the engine reports each, commit and amend are refused before git runs, and the commit box is disabled and names it; the commit box holds `MERGE_MSG` with git's comment lines visible (the user's decision E) | integration tests in `cairn-git` against real `git`, headless tests |

C21 is not automated, for the reason `history-graph`'s A7 was not: a timing
assertion in CI is flaky and bound to a machine.

## Out of scope

Another packet's: stash create, Save Snapshot, apply, pop and drop, and
`.gitignore` editing (5b, `stash-and-ignore`); push, Commit and Push, and the
forge (packet 6); every other ref operation — rename, delete, checkout (packet 7);
merge, rebase, cherry-pick, revert and reset, and resolving a conflicted file
(the second lap).

Not built, as in Fork (L9): an author override, empty commits and an up-front
skip-hooks.

Not done, and filed with the `file-issue` skill at teardown: a backup before
discard (L2, should the decision be revisited); Fork's per-repository sign-off
setting and the commit box showing author and signing; pre-filling an empty draft
from `commit.template`, as Fork does (`fork-staging-and-commit.md` §5, "Template";
`git commit -F` ignores the template); wrapping a paragraph at the ruler; a
configurable subject limit; transcoding to a non-UTF-8 `i18n.commitEncoding`; a
hook reading `/dev/tty` (L11's residual); discarding a submodule's changes
(L24); a forced close orphaning a process that holds `index.lock` (#48 item 2);
pinning `apply.ignoreWhitespace` on every `apply` (measured unnecessary in phase
03, on git 2.30.9, 2.32.7 and 2.56.0 over 400 selections: it changes nothing a
fresh patch stages, only letting a patch land on context moved by whitespace,
which R3.7's stale check refuses first; C5 holds the case); a persisted operation
log;
the index refresh (#66); fetch's cancel tied to its operation (#47's remainder); a
Commit-tab "restore this file" from a lost commit; and the reflog as a list of
entries.
