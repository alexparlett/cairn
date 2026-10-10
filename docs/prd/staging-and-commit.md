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

**Redesigned 2026-10-10.** After phases 01-11 were built, an independent review
(`review-ux-commit-and-dialogs.md`, `review-ux-activity-and-chrome.md`,
`review-ux-local-changes.md`, `review-code-engine.md`, `review-code-app-ui.md`) found the UI
had grown by patching each QA finding where it showed. A design pass (`brief-commit-and-amend.md`,
`brief-discard-and-create-branch.md`, `brief-feedback-and-activity.md`,
`brief-local-changes-selection.md`, the page `redesign-page-2026-10-10.html`) and a first-hand
look at Fork for Windows 2.21.1 (`fork-observed-2026-10-10.md`) led to the user's decisions of
2026-10-10, recorded verbatim in `redesign-decisions-2026-10-10.md` (cited below as "the
redesign", with the page's choice ids: rules 1-8, L1-L5, C1-C8, D1-D3, B1-B4, F1-F9). Rule 1 —
Fork first, checked at the source — makes `fork-observed-2026-10-10.md` win over any earlier
brief where they differ. Every requirement the redesign touches is amended in place below,
dated; the rebuild is phases 12-20 of `docs/work/staging-and-commit/`.

**Three earlier decisions are superseded, because they rested on Fork claims Fork's own record
contradicts** (the redesign; evidence `brief-local-changes-selection.md` headlines 1-2,
`brief-commit-and-amend.md` §5.2, `fork-observed-2026-10-10.md`):

- **Several files' diffs drawn together "as Fork does"** (R8.1's addition of 2026-10-09, phase
  07's QA item 4(e), and the "Files drawn together" product rule ratified at phase 08's QA):
  Fork shows one file's diff — the file clicked first — and its developer declined a combined
  view in 2019 and 2026 (fork-dev/Tracker #261, TrackerWin #786; observed, `lc2`). Superseded
  by R8.1 as amended.
- **Alt held for Stage All / Unstage All on Linux "as Fork for Windows"** (R8.2's item 4(d),
  2026-10-09): Fork for Windows uses Shift (TrackerWin #2429, #2466; Windows release notes 2.8).
  Superseded by R8.2 as amended.
- **`MERGE_MSG`'s `# Conflicts:` lines left visible and committed, as Fork does** (decision E,
  2026-10-09, R10.8 and C24): committing them is Fork's open bug since 2017 (Tracker #180), not
  its design, and git's own editor strips them. Superseded by R6.10 and R10.8 as amended.

## What this packet delivers

The working tree becomes something Cairn changes, not only shows. In Local
Changes, a file, a hunk, a run of lines or a mode change can be staged, unstaged
or discarded, as Fork does it: double-click, Return, drag between the lists, the
header buttons or the context menu for files; a hovered chunk's floating buttons,
narrowed by a drag-selection, for hunks and lines. A commit box under the diff
commits what is staged, or amends `HEAD`. Every destructive operation — a discard,
deleting untracked files, an amend a remote has or git keeps no reflog for, Create
Branch's Discard, removing a stale lock — is sealed behind a confirmation, and every
operation that can be recovered from asks nothing (the redesign, rule 2). What a
destructive operation costs is computed by the engine at the press and re-checked the
moment before it runs. What an amend leaves behind is recoverable from Show Lost Commits,
and every operation can be read back, with the prompt the user accepted, in the activity
popover, Fork's Activity Manager. Each kind of message has one home (the redesign, rule 4).

In build order: the seal; the patch engine's inversions; the write verbs; the
local write lane; commit and amend in the engine; the render layer's keys,
dialogs and menus; Local Changes' actions; the diff's gestures; the commit box;
Show Lost Commits; and the activity popover — then, from the redesign: git's output read
once; the commit engine asking git; Create Branch as Fork's; the discard prompts; one
selection; one file's diff; the commit box rebuilt; one home for each message; and Fork's
Activity Manager.

Stash and `.gitignore` are packet 5b's (`stash-and-ignore`, L1).

## Requirements

### R1 — The seal is bound to what it costs (L3)

- R1.1 `cairn_model::Confirmed` derives no `Clone` and no `Copy`, and is
  constructed only by the confirmation surfaces on a roster the guard holds: the
  confirmation dialog's component and the commit box (R10.6).
  (Amended 2026-10-10, the redesign — rule 2, C1, and the user's decision on Create Branch's
  Discard, `redesign-decisions-2026-10-10.md` §3: the roster is the confirmation dialog and the
  Create Branch dialog, whose Create and Checkout — or Return — with Discard chosen is the
  acknowledgement (R11.3). The commit box leaves it: an amend that needs confirming is confirmed
  in the dialog (R10.6). How a failed amend's skip spends a confirmation without asking again
  (R10.5) is open — see `docs/work/staging-and-commit/phase-13-commit-asks-git.md`.)
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
  (Amended 2026-10-10, the redesign — rules 6 and 7, D2: the prompt is one sentence frame,
  "<Question naming what is discarded>? [The worst loss, only when it is worse than changes.]
  You can't undo this.", and the per-file detail — each path and what happens to it, the lines,
  the bytes, a mode change — is the `Consequence`'s list behind "Show files", never in the
  sentence. The kinds above stay as that list's entries. An amend's `Consequence` carries no
  subject (R6.4); Create Branch's Discard's is fixed and generic and names no file (R11.3).)
- R1.3 Every destructive operation of this packet (R1.5) takes `Confirmed` by
  value, and the guard holds the roster: a destructive operation without it, or
  a roster entry with no such operation, fails.
- R1.4 Immediately before it runs, a destructive operation re-reads the state its
  `Consequence` names and refuses, writing nothing, when anything differs; the
  refusal is an outcome of its own ("changed since you confirmed"), never a
  failure of git's. (Amended 2026-10-10, the user's decision on Create Branch's Discard: its
  re-check is that `HEAD`, the commit and the name are unchanged, and no more — git decides what
  the forced checkout takes. A refusal is shown in the "Couldn't <name>" dialog, R14.3.)
- R1.5 Destructive here: discard lines (R3.3), discard files and delete untracked
  files (R3.5), amend (R6.4), remove `index.lock` (R12.4), and Create Branch's checkout
  with "Discard" (R11.3). Not destructive: stage and unstage of files, lines or a mode
  change, commit, create a branch, and create and check out a branch keeping the changes
  (R11.3). (Create Branch's discard added 2026-10-09, the user's decision 3.)
  (Amended 2026-10-10, the redesign, rule 2 and C1: an amend is destructive when a remote already
  has `HEAD` or git will write no reflog entry that keeps the old commit; an amend git logs and
  no remote has is recoverable from Show Lost Commits, asks nothing and takes no `Confirmed`
  (R6.4). `ops::amend` keeps the roster's row for the confirmed amend. Create Branch's Discard
  stays destructive, confirmed by its dialog's press — the user's decision of 2026-10-10.)
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
  user's decision 3. Amended 2026-10-10, B1 as changed after the Fork check: that discard is
  Fork's forced checkout, which also overwrites untracked files in the way — "we'd want
  untracked files discarded too".)
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
  (Amended 2026-10-10, the redesign, C4: `git rev-parse --git-path hooks` goes with R6.6 — the
  skip is offered on every failed commit or amend, so Cairn no longer asks where hooks are; and
  Create Branch's three reads become one, `git check-ref-format` (R11.3), the `--numstat` pair
  and `git ls-files --others` going with the prediction they fed. Added: `git stripspace
  --strip-comments` (R6.10) and `git config` in query form for the settings a commit and an
  amend depend on (R6.11).)
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
  refresh after it shows what did (#45 item 1). (Amended 2026-10-10, the redesign, rule 5, and
  `review-ux-activity-and-chrome.md` H1: a write the user cancelled ends `cancelled` — never
  "may have taken effect" — and a commit or amend cancelled reads `HEAD` after its `git` is
  reaped, so one that was made is reported made; a write whose `git` Cairn lost hold of ends
  `failed`, saying the lists and history show anything it had already done. The engine's
  ending carries a short sentence for people apart from the command, its output and the lock
  paths (R14.2).)
- R4.8 The runner's bounded-output helpers are callable on reads only, at the
  type (#45 item 2).
- R4.9 The first close requested during a local write waits for it, saying which
  ("Finishing commit…"), and never kills it; a second request after
  `CLOSE_PATIENCE` closes anyway, as today, and a lock it strands is named the
  next time the repository opens (#45 item 3, #44 item 2). (Amended 2026-10-10, the redesign,
  rules 4 and 5: the closing line names the write by its one name — "Closing after Commit
  finishes. Close again to quit now." — and a stranded `index.lock` is found by the lock state
  of R12.4, not by an entry of its own.)

- R4.10 git's output is read once, as whole lines (added 2026-10-10, the redesign's engine fix,
  `review-code-engine.md` H3 and M7, `review-code-app-ui.md` H3): the runner splits stdout and
  stderr alike into whole lines, never inside a character, with one line type for both;
  removes a URL's userinfo from each whole line before anything is cut or kept (R12.2); and
  keeps a bounded tail of whole lines, dropping the oldest, with "older lines dropped" a plain
  fact. Every view takes those scrubbed lines and never git's raw text. This closes the live
  defect that `process/pipes.rs` cuts a long stderr line inside a multi-byte character — the
  bug a commit's stdout was fixed for — and retires the scrubber's guess at an unseen cut.

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
  hooks.) (Amended 2026-10-10, the redesign, C4: `--no-verify` is passed from the skip offered
  on every failed commit or amend, R10.5.)
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
  (Amended 2026-10-10, the redesign, rules 2 and 3, C1, and the git-parity finding of
  `review-code-engine.md` M4: what an amend costs is read when Amend is pressed, inside the
  local-lane job that then runs it, never on a refresh. The job answers one of three: an amend
  no remote has and git logs runs at once, unconfirmed (R1.5); one a remote has, or git logs
  nowhere, ends without running git and hands back its `Consequence`, for the dialog to confirm
  and `ops::amend` to run with the token. The `Consequence` keeps `HEAD`'s id, the remote ref
  that has it and the reflog fact, and drops the subject, so a display field is never a
  freshness condition (`review-code-engine.md` M2). Whether git will log the amend is answered
  by git's own reading of `core.logAllRefUpdates` (R6.11), never by gix's configuration.)
- R6.5 Commit and amend carry an askpass token (R5.1) and stream their stderr to
  the operation log (R12.1).
- R6.6 Whether a `pre-commit` or `commit-msg` hook exists is answered from the
  hooks directory git resolves (`git rev-parse --git-path hooks`, so
  `core.hooksPath` counts); a hook counts when its file exists and is executable,
  as git runs it. (Superseded 2026-10-10, the redesign, C4: the skip is offered on every
  failed commit or amend, which catches what a file check cannot — Husky's `core.hooksPath`
  layouts, Fork's open complaint Tracker #948, and hooks declared in git's configuration — and
  where no hook ran, the skip fails the same way again, harmlessly. Cairn keeps no hook model:
  the read, `Repository::commit_hooks` and `CommitHooks` go.)
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
  added 2026-10-09, the user's decision C.) (Amended 2026-10-10, the redesign, C3 and C8: with
  a single cherry-pick or a single revert in progress, R6.1's commit concludes it, as Fork's
  and `git commit` do — the picked commit's author kept as git keeps it, `CHERRY_PICK_HEAD` or
  `REVERT_HEAD` cleared by git; a sequence of them (`.git/sequencer`), a rebase and `git am`
  still refuse commit and amend, naming git's own command to continue or abort. The engine also
  reports a detached `HEAD`, which commits — recoverable, so it asks nothing (R10.8).)
- R6.10 A merge message, or a cherry-pick's or revert's, is cleaned the way git's own editor
  cleans it before the commit box shows it (added 2026-10-10, the redesign, C2 and rule 7,
  superseding decision E of 2026-10-09): `git stripspace --strip-comments`, git reading
  `core.commentChar` itself, its input `MERGE_MSG`'s bytes on stdin — a new read in `reads/`,
  approved by the user's choice of C2, a named function with its own guard row (C32), writing
  nothing and running nothing. What the box then shows is what is committed under R6.1's rules.
- R6.11 The settings a commit and an amend depend on are git's answers (added 2026-10-10, the
  redesign's git-parity fixes, `review-code-engine.md` M4): `core.logAllRefUpdates`, for R6.4's
  reflog fact, and `i18n.commitEncoding`, for R6.1's refusal, are read with `git config` in
  query form through the one named config read `reads/` has (`reads::fetch_settings`,
  generalised), so a linked worktree's `includeIf`, the system file and trust are read as
  git reads them; gix's configuration is not asked either.

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
  (Amended 2026-10-10, applied as Fork under rule 1, observed in Fork for Windows 2.21.1,
  `fork-observed-2026-10-10.md` `lc3`, `lc4`: with focus in the diff and no lines selected,
  Return does nothing, and Ctrl+S — ⌘S on macOS — stages the whole selection, as it does from
  the list; so Linux's stage chords gain Ctrl+S, and bare Return and Enter are heard from the
  file lists alone, never the diff — the bare-key scope narrows to the lists for Enter, while
  Backspace and Delete stay heard from the lists and the diff. This amends the root
  `CLAUDE.md` modifier invariant's bare-key rule and its pin, toward fewer bare chords.)
- R7.4 A confirmation dialog: modal to assistive technology and to the keyboard
  (Tab stays inside it; the window's chords do nothing while it is open), focus
  on Cancel, its prompt and its button rendered from a `Consequence`, Escape
  cancels. Every dialog's buttons sit in the platform's order — on Linux the primary first
  and Cancel last (Fork for Windows), on macOS Cancel first and the primary last (Fork for
  macOS) — focus on Cancel on both (amended 2026-10-09, the user's decision E).
  (Amended 2026-10-10, the redesign, rule 2, rule 6, D2 and D3: it is the one confirmation
  dialog every operation that cannot be undone or rewrites shared history uses, whichever
  route asked — button or chord; titles and buttons are in Title Case ("Discard Changes",
  "Amend Commit"); its prompt is short and never cut, and where several paths are named a
  "Show files" disclosure under it opens a virtualized list of them; it opens at once on the
  press, saying "Counting…" with its button greyed while the engine computes the
  `Consequence`; Escape and Cancel go back one step, to whatever was open beneath it.
  Kept deviation: Cancel has focus where Fork's discard dialog focuses its Discard button,
  observed `dc1` — L8 and rule 2.)
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
  (Superseded 2026-10-10 — the 2026-10-09 addition rested on a false Fork claim; the redesign,
  rule 8 and L1, and applied as Fork under rule 1, observed `lc2`: the selection is one value,
  in one list at a time, with a primary row. The diff shows one file — the primary, which is
  the file clicked first, not the topmost and not the one last toggled — with everything a
  single file's diff has: Load Diff, the mode row, previous and next change, the gesture. No
  files are drawn together, there is no line budget and no "not shown" notice. With more than
  one file selected the diff's bar keeps the primary's own header and adds "k of n selected"
  (L1 as answered; Fork draws no count — the user's to confirm, see
  `docs/work/staging-and-commit/state.md`). Toggling the primary out moves it to the nearest
  selected path; toggling the last one out leaves nothing selected and the diff says "No file
  selected".)
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
  (Amended 2026-10-10, the redesign, L2, superseding item 4(d)'s Alt, which rested on a false
  Fork claim — Fork for Windows' key is Shift, TrackerWin #2429, #2466: the held key is Shift
  on Linux and ⌥ on macOS, which no window manager takes with a click; while it is held both
  headers read Stage All and Unstage All, enabled whatever is selected (Fork for Windows 2.7,
  TrackerWin #2466); and a visible double chevron sits in each header — ⇊ Stage All in
  Unstaged, ⇈ Unstage All in Staged — a deviation from Fork for Windows, which hides Unstage
  All until the key is held. Every route acts on the selection, wherever focus is (rule 8):
  the list's own button is greyed while the selection is in the other list. The filter prunes
  the selection to the rows it shows (L3), so nothing hidden is acted on.)
- R8.3 After a stage or unstage, the selection moves to the nearest remaining path
  in the list it left (Fork, Tracker #514).
  (Added 2026-10-09, the user's ratification on phase 07's QA item 4(a): for a selection
  with gaps, the nearest remaining path is the one that slides into the first acted
  row's place, else the nearest above it.)
  (Amended 2026-10-10, the redesign, L5, and observed in Fork for Windows 2.21.1, `lc4`, `lc5`:
  one rule decides where the selection goes, after an action and after any refresh alike —
  selected paths still listed stay selected, the primary kept or moved to the nearest of them;
  when none is left, the row that took the first one's place is selected, else the nearest
  above it — whoever moved the files: a stage, a commit made in a terminal, a filter. It is a
  pure function run as the status or the filter's rows arrive, never from a render effect.)
- R8.4 Discard on the unstaged side only, through R7.4's dialog: Fork's words with
  counts (L8), the button naming the count. The confirmation cannot be skipped
  (L8): no modifier, setting or other route reaches a discard without the dialog.
  A selection of untracked rows deletes them; a nested repository among them
  refuses with its reason before any dialog.
  (Added 2026-10-09, the user's ratification on phase 07's QA item 4(c): a discard asked
  on the staged side — the chord on the Staged list or a staged diff — asks nothing and
  says "Staged changes can't be discarded: unstage them first.", where Fork does nothing;
  a deviation from Fork named in `docs/design/ui.md`.)
  (Amended 2026-10-10, the redesign, rules 4 and 6, D1, D2 and D3: the dialog is R7.4's,
  titled "Discard Changes", in the one sentence frame — "Discard all changes in 31 files? 2
  untracked files will be deleted. You can't undo this." — with "Show files" and the button
  "Discard Changes in 31 Files"; lines from the diff read "Discard 2 changed lines in
  src/main.rs? You can't undo this." under the button "Discard 2 Lines". A selection mixing
  discardable rows with a submodule or a conflicted path discards what can be, the dialog
  saying in one line what is left ("1 submodule and 1 conflicted file are left as they are.")
  and its button counting only what it discards, where Fork asks twice (D1). A refusal known
  in advance is a greyed control with its reason beside it, wrapped, never cut — the menu's
  Discard Changes… greyed over a staged selection, "Staged changes can't be discarded. Unstage
  them first." — and the discard chord over such a selection does nothing, as Fork's does,
  since rule 4 leaves no line under the lists to say it in; this replaces item 4(c)'s line.)
- R8.5 Ignore Whitespace is disabled in Local Changes, and the diff drawn there is
  always the exact one; the shared setting is untouched (L6).
- R8.6 A queued, running or failed write is drawn where the user acted; a stale
  patch's refusal names its path. (Superseded 2026-10-10, the redesign, rule 4: there is no
  line under the lists. A running or queued write shows in the status box (R14.1), a git
  failure in the Git Error dialog, and a refusal found while running — a stale patch, naming
  its path once — in the "Couldn't <name>" dialog (R14.3).)
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
  (Amended 2026-10-10, the redesign, rule 8: with no lines selected, the chords act on every
  selected file — the selection — wherever focus is.)
- R9.6 A finished line selection is kept while the diff's text is unchanged (added 2026-10-10,
  the redesign, L4 — a deviation from Fork, which drops it silently on a refresh, observed
  `ln2`): a refresh whose re-read draws byte-identical rows — switching to the editor and back
  with the file untouched — keeps the selection, its tint and its floating actions; one whose
  text differs clears it, and the diff's bar says once "The file changed — line selection
  cleared.", gone at the next key or press. A selection is keyed to the content of the diff it
  was made over, so no act made over other rows reaches a write (the product rule below); a drag
  still in progress is cancelled when the window loses focus.
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
  (Amended 2026-10-10, the redesign, rule 4, C3 and C8: a control greyed carries its reason
  beside it, wrapped — `Amend` greyed during a merge under the one operation line "Merging.
  Committing concludes the merge; Amend is unavailable."; and the button also concludes a single
  cherry-pick or revert with nothing staged (R6.9).)
- R10.2 Recent Commit Messages (R6.7) as a menu beside the subject and ↑/↓ in an
  empty or recalled subject; a recalled message fills both fields.
- R10.3 Ticking `Amend` fills an empty draft with `HEAD`'s message and shows
  R6.3's staged list; the draft the user had is kept and returns when Amend is
  unticked; Amend unticks itself after the commit. Where amend's staged list cannot
  be read — a partial clone's missing blob, which Cairn's reads never fetch — the
  amend is still offered, and the box says the lists show what is staged against
  `HEAD`. (Added 2026-10-09, the user's decision on phase 09's QA; Fork has no
  evidence here and would likely lazy-fetch.)
  (Amended 2026-10-10, the redesign, C7: every fill — `HEAD`'s message here, `MERGE_MSG` in
  R10.8 — goes only into an empty draft the person has not touched; any edit, clearing
  included, makes the draft theirs, one rule replacing the once-per-merge flag (Fork's
  developer's rule, Tracker #61). Ticking Amend shows the last commit's files among the staged,
  as Fork does, observed `amd1`.)
- R10.4 A running commit is drawn as busy with its elapsed time and a Cancel; the
  hooks' output streams into the operation log (R12). (Amended 2026-10-10, the redesign, C5
  and rules 4-5: the button reads "Committing 0:03" (or "Amending") with Cancel beside it —
  kept where Fork has only the Activity Manager's ×, so a hung hook is stopped where it was
  started; a commit queued behind another write reads "Waiting for <name>", the one form every
  queued write uses; a cancel opens no dialog and keeps the draft, the button reading
  "Cancelling…" until git is reaped (R4.7).)
- R10.5 A failed commit opens Fork's `Git Error` dialog — the command and git's
  output with ANSI sequences stripped — offering `Skip pre-commit hooks and
  commit` only where R6.6 finds a `pre-commit` or `commit-msg` hook; the draft is
  kept. The output is drawn through a virtualizing view or bounded to the retained
  tail, never a `ScrollView`. (Amended 2026-10-10, the redesign, C4: `Skip pre-commit hooks
  and commit` is offered on every failed commit or amend, in the dialog's footer in the
  platform's order beside Close, one button component for both; it runs once, with
  `--no-verify`, and never asks a second time — an amend that was confirmed is not confirmed
  again (how its token is spent is open, see R1.1). No prompt text is drawn in the dialog.)
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
  (Superseded 2026-10-10, the redesign, rules 2, 3 and 6, C1 and C6 — and with it the merge
  bar's W1, the three-line cap that could cut "can't be recovered": in amend mode the button
  reads `Amend <short id>` ("Amend 3f2a1c9"), with no line under it, and the commit chord does
  exactly what the button does. Pressing it asks the engine what the amend costs (R6.4): an amend
  git logs and no remote has runs at once — the old commit stays in Show Lost Commits; one a
  remote has, or one git keeps no reflog for, opens R7.4's dialog, titled "Amend Commit", in
  one or two fixed sentences naming the id once — "3f2a1c9 is already on origin/main. Amending
  it rewrites history others may have." and "3f2a1c9 can't be recovered after this: this
  repository keeps no reflog." — with Amend and Cancel, Cancel returning to the box unchanged.
  Kept deviations from Fork: Fork for Windows notes a pushed amend inline and amends at once
  with no dialog (observed `amd1`-`amd3`), Fork for macOS says nothing (C1); and Fork's button
  reads "Amend Last Commit" (observed `amd2`) where Cairn's names the commit (C6 as answered,
  the user's to confirm against the observation).)
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
  (Amended 2026-10-10, the redesign, C2, C3, C7 and C8, superseding decision E and the
  once-per-merge flag: the box fills an empty, untouched draft with `MERGE_MSG` cleaned by R6.10
  — no `# Conflicts:` block, no scissors section — so what it shows is what is committed; a
  single cherry-pick or revert fills and concludes the same way, its line reading
  "Cherry-picking a1b2c3d. Committing concludes it; Amend is unavailable."; during a rebase,
  `git am` or a sequence of picks or reverts the box is disabled under one line naming git's
  own command ("A rebase is in progress. Continue or abort it with git rebase --continue or
  --abort."); and on a detached `HEAD` one line reads "HEAD is detached: this commit will be on
  no branch.", asking nothing (rule 2: the commit is recoverable).)

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
  - (Amended 2026-10-10 — the redesign's B1 as changed after the Fork check, B2, B3 and B4, and
    the user's decision on how Discard is confirmed, `redesign-decisions-2026-10-10.md` §3,
    observed `cb2`-`cb4`, `am1`; superseding decision A's title, button and prompt, the
    W3 question of the merge bar, and the counted `Consequence` of decision 3:) "Discard" is
    Fork's. The group reads "Local changes:" Don't change / Stash and reapply / Discard, in
    Fork's order; "Stash and reapply" is drawn greyed with "Comes with stashing." beside it
    until packet 5b builds stashing (B4); Discard chosen shows ⚠ beside it, as Fork's does; the
    button stays "Create and Checkout" (B3). With Discard chosen, pressing Create and Checkout —
    or Return in the dialog — IS the acknowledgement: the Create Branch dialog is a
    confirmation surface on the roster (R1.1), it builds the `Confirmed` on that press, and no
    second dialog opens (B2; the user: "Clicking the button when I've already selected discard
    is the confirmation. Asking again will just annoy the user."). Rule 2 holds: the
    confirmation is the deliberate choice of the radio plus the press, as in Fork. The
    `Consequence` is fixed and generic and names no file — the operation discards local changes
    and any untracked files in the way, then checks out <name> at <commit> — so nothing is
    predicted of what git deletes (rule 7). It runs Fork's command, `git checkout -q --no-track
    -f -b <name> <oid> --` (Fork's `--force`, with Cairn's `-q` and `--`), which also overwrites
    untracked files in the way (B1). Its re-check is that `HEAD`, the commit and the name are
    unchanged. It is still refused before git runs during an operation in progress, over a
    conflicted path and over a submodule's change, each said as a greyed choice with its reason
    (rule 4), and it is never the remembered choice. The dialog stays open beneath a Git Error
    or a "Couldn't Create branch '<name>'" dialog, so Close returns to it as it was left; while
    it is open the window's keys are inert, as under any dialog.
  - (Amended 2026-10-10, the git-parity finding of `review-code-engine.md` M3:) the name is
    checked as `git checkout -b` and `git branch` take it, which phase 14 establishes against
    real git — `git check-ref-format --branch` resolves `@{-N}` to a previous branch's name, so
    it is the wrong oracle where those verbs take the name literally; the refusal is typed
    (invalid, taken, a folder holds it, it holds a branch) and worded by the view, decision F's
    "A branch name can't contain '@{'" kept. While the check waits behind a running write the
    dialog says "Waiting for <name>" (rule 5).
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
  (Amended 2026-10-10, the redesign — rules 4-6, F3, F4, F7, F8, and applied as Fork under rule 1,
  observed `am1`, `am2` — superseding B's status words, C's three-line cut, F's "Show Replaced
  Commit", K's four-line cut and Show All, M's let-go line, and every lock note of H:) the
  popover is Fork's Activity Manager. A row is the operation's one name with Fork's result
  line under it ("Switched to branch 'topic'", "staged", "Already up to date", "stage
  failed"), its start time as `HH:MM:SS UTC` (F7: kept in UTC everywhere while local time waits
  on the filed timezone issue — a deviation from Fork's local `HH:MM`) and ⚠ on a failure, a
  distinct shape on a cancel. The right pane's header is the name, then one of four status
  words — running, succeeded, failed, cancelled — with the time, and the duration (F8, which
  Fork lacks; TrackerWin #2179 asks for it); then, for a failed or cancelled operation, one
  sentence, never cut; then the prompt the operation confirmed, whole and wrapped, only for an
  operation that ran (F3); then every `$ git …` line with its output, one append-only list,
  each command ending in one form (`exit code 1`, `killed by signal 9`, `cancelled`). An amend's
  way back reads "Show in Lost Commits", which turns the mode on and selects the commit (F4). The
  byte bound evicts whole oldest operations; only one larger than the bound alone is cut, its
  first line "Earlier output not kept." A fetch's progress shows in the status box, with Fetch
  greyed while a fetch runs, so no fetch is refused for being second (F9). Fork's All / User /
  Background tabs are not drawn (the user's to confirm, `state.md`).
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
  (Amended 2026-10-10, the redesign, F5, F6 and decision G kept — superseding H's offer on
  entries, its blocked state, its notes and the entry found at open: a leftover lock is one
  state of the repository, decided by whether `<gitdir>/index.lock` exists now — a `stat` on
  every refresh, on open, on focus, after each write and on Refresh. A banner under the title
  bar shows exactly while the file exists, no `git` of Cairn's runs or is queued in the
  repository, and the file is older than about ten seconds (F6): "index.lock is in this
  repository, so git can't change the index. If no other git program is running, it was left
  behind." with `Remove index.lock…`; it goes when the file does, however it went. As Fork
  does, observed `lk2`, a write that fails because git could not create `index.lock` carries
  the same `Remove index.lock…` in its Git Error dialog beside Close. Both open the one
  confirmation, decision G's "Remove stale lock" unchanged, its `Consequence` read at the
  press; after the removal Cairn refreshes and does not retry the write, as Fork does (`lk3`).
  Before reporting a write that failed on a lock, Cairn waits about a second and retries it
  once or twice (F5; Fork retries, Tracker #1303, observed `lk2`). Kept deviations from Fork:
  the banner, where Fork shows nothing until a write fails (`lk1`), and the confirmation, where
  Fork removes the lock unasked (`lk3`) — the user's F6 and G.)
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
- R13.3 The rebuilt packet is measured again at its merge bar, on the same clone and bars
  (added 2026-10-10: the redesign rewrites the paths C21 measured — the selection, the diff's
  query, the output's line handling and the amend's read at the press), and an amend's
  press-time read is recorded beside them.

### R14 — One home for each message (added 2026-10-10, the redesign, rules 4-6, F1, F2)

Evidence: `brief-feedback-and-activity.md` §3, `review-ux-activity-and-chrome.md`,
`review-ux-local-changes.md` H1, H2, `review-ux-commit-and-dialogs.md` H3, M7.

- R14.1 Progress shows in the title bar's status box: a spinner and the running operation's
  name, shown only once it has run 250 ms (F1), with " · 1 waiting" while writes queue behind
  it; a fetch adds git's progress line (F9). A control a running write holds is greyed and says
  "Waiting for <name>".
- R14.2 Each operation has one name — Fork's imperative, "Stage 2 files", "Commit", "Create
  branch 'topic'" — the only wording of it, used by the status box, a wait, the closing line,
  a dialog's title and the activity popover (rule 5). An ending carries a short sentence for
  people apart from its command, output and lock paths, and the engine's error text is never
  drawn as it stands; the engine says why once, and the view adds at most one tail, "Nothing
  was changed.", true of every refusal.
- R14.3 A git failure opens Fork's Git Error dialog — its title, its sentence, `Error Details:`,
  the command and git's output — for every operation the person started. A refusal Cairn finds
  while running (a stale patch, a file changed since it was confirmed, `HEAD` moved before an
  amend, closing) opens the same dialog titled "Couldn't <name>" with one sentence and no Error
  Details, since no git failed (F2). A partly done discard says so there in one sentence, the
  kept files behind "Show files".
- R14.4 A refusal Cairn knows in advance is a greyed control with its reason beside it — a
  menu item's reason wrapped under it, a button's beside it — never a message after a press
  that could not act.
- R14.5 Success says nothing: the refresh is the message. A cancel opens no dialog.
- R14.6 There is no line under the file lists and no banner for an operation; the two banners
  are the leftover lock (R12.4) and closing (R4.9), about the repository and the window.
- R14.7 No text a person needs is cut (rule 6): a sentence that could run long is made short
  instead — it names the exception only and never repeats what is on screen — and per-file
  detail goes behind "Show files"; git's own text is shown only where it scrolls, the Git Error
  dialog and the popover.
- R14.8 While any dialog is open — a confirmation, Git Error, "Couldn't <name>", Create Branch,
  a credential prompt — or the popover is, the window's chords and the lists' keys do nothing:
  one modal state decides it.

## Product rules

The design rules the user approved on 2026-10-10 (`redesign-decisions-2026-10-10.md` §2) are
this packet's product rules from then on; the rules before them are amended to agree, each
change dated.

- **Fork first, checked at the source** (rule 1). Where Fork's behaviour is known, Cairn does
  the same, and every "as Fork does" cites the issue, release note or first-hand observation it
  comes from; `fork-observed-2026-10-10.md` wins over an earlier brief where they differ.
- **One confirmation rule** (rule 2). An operation that cannot be undone, or that rewrites
  history someone else may have, always gets the one confirmation dialog — Cancel focused,
  Escape back one step — or, for Create Branch's Discard, the deliberate choice of the radio
  plus the press (the user's decision, 2026-10-10). An operation that can be recovered from asks
  nothing. A button and its chord always do the same thing.
- **Ask the cost at the press** (rule 3). What an operation destroys is computed when it is
  pressed, never on every refresh before it; no control has a "reading" state.
- **One home for each kind of message** (rule 4; R14).
- **One name per operation, four status words** (rule 5; R14.2, R12.1).
- **Short text, never cut** (rule 6; R14.7).
- **Let git decide** (rule 7). Cairn runs git and shows its refusal rather than predicting what
  git would do, and the message shown is the message committed.
- **One selection, one scope** (rule 8). Files are selected in one list; the diff shows one
  file; every route acts on what is selected, wherever focus is.
- **What is staged is what was selected, from the exact diff.** No view setting,
  no user `apply.whitespace`, no stale index changes what a patch stages.
- **A destructive operation states its cost first, and only what it will cost.**
  The prompt is computed by the engine and re-checked before the operation runs. (Amended
  2026-10-10: Create Branch's Discard states its cost as a fixed sentence and leaves what the
  forced checkout takes to git, rule 7 and the user's decision on its confirmation.)
- **No backup is promised.** A discard says it cannot be undone, because it
  cannot (L2).
- **Fork's mechanics, every deviation named** (amended 2026-10-10 to the redesign): Cancel
  focused in every confirmation, where Fork focuses its Discard button (L8, rule 2; observed
  `dc1`); prompts in one sentence frame ending "You can't undo this.", their per-file detail
  behind "Show files", where Fork's ask without it (D2), and Title Case titles where Fork for
  Windows writes "Discard changes" (D3); a mixed selection's discard taking what it can and
  saying what is left, where Fork asks twice (D1); Ignore Whitespace disabled in Local Changes
  (L6); a visible ⇈ Unstage All in Staged's header, where Fork for Windows shows it only while
  Shift is held (L2); "k of n selected" in the diff's bar, where Fork draws no count (L1, the
  user's to confirm); a finished line selection kept across a refresh that leaves the diff's text
  unchanged, where Fork drops it (L4, the user's kept deviation); the amend dialog when a
  remote has `HEAD` or git keeps no reflog, where Fork for Windows notes a pushed amend inline
  and amends at once and Fork for macOS says nothing (C1, the user's kept deviation); the amend
  button naming the commit, where Fork's reads "Amend Last Commit" (C6); Cancel beside a running
  commit, where Fork has only the Activity Manager's × (C5); the commit box disabled during a
  rebase, `git am` or a sequence of picks, naming git's command, where Fork pre-fills and commits
  (L25 as narrowed by C3); the activity popover quoting each confirmed prompt (F3), its "Show in
  Lost Commits" (F4), its duration (F8) and its times in `HH:MM:SS UTC` where Fork's are local
  `HH:MM` (F7); a leftover `index.lock` shown as a banner while it exists, where Fork says nothing
  until a write fails, and its removal confirmed, where Fork's is not (F6 and G, the user's kept
  deviations); a nested repository refused before any dialog (L8); a submodule row offering no
  discard, where Fork offers `Discard Submodule Changes` (L24); a Show Lost Commits control in
  the history's toolbar area, where Fork has only the View menu item and the chord and refused a
  toolbar button (TrackerWin #378) — a check box at the right end of the "Graph and subject"
  heading's cell, its tooltip the chord (the user's decision, 2026-10-09); Create Branch's "Stash
  and reapply" greyed until packet 5b builds stashing (B4); Show Lost Commits seeded from
  `HEAD`'s and the local branches' reflogs, where Fork's mode reaches every reflog (`git log --all
  --reflog`, TrackerWin #1307); and no Commit and Push until push exists (L9). (Before
  2026-10-10 this list also named prompts counting by kind and size, the amend's line, Create
  Branch's own Discard confirmation and the commit box refusing a cherry-pick or revert; the
  redesign replaced each.)
- **A line selection acts only on the diff it was made over** (phase 08's QA, item 1; amended
  2026-10-10, L4): a selection names rows of one diff by its content, and once a diff whose text
  differs is drawn under it, it is nothing, and no act made over the old rows reaches a write; a
  re-read that draws the same text keeps it (R9.6).
- **Files drawn together** (ratified 2026-10-09 at phase 08's QA): superseded 2026-10-10 — the
  premise was false (Fork shows one file); the diff shows the primary file alone (R8.1), and a
  discard of several files names them behind "Show files" (R8.4).
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
points here and does not restate them. C25-C34 were added, and the rows the redesign touches
amended in place, on 2026-10-10 (`redesign-decisions-2026-10-10.md`).

| # | Criterion | Pinned by |
| --- | --- | --- |
| C1 | `Confirmed` is neither `Clone` nor `Copy` and cannot be built outside the roster's surfaces; every rostered destructive operation takes it by value and every such operation is rostered; the prompt is rendered from the `Consequence` (Amended 2026-10-10, the redesign: the confirmation-surface roster is the confirmation dialog and the Create Branch dialog, the commit box off it; `ops::amend` stays on the destructive roster for the confirmed amend, and the unconfirmed amend R6.4 runs takes no token.) | `compile_fail` doctests in `cairn-model`, and a guard with a nonzero-files assertion and a matcher self-test |
| C2 | Each destructive operation refuses, writing nothing, when the state its `Consequence` names moved between confirmation and run: a discarded file edited or chmodded, a file or a symlink standing at a confirmed path's directory (an untracked file added beside a confirmed deletion is never taken, and the discard goes ahead — amended 2026-10-08, the user's decision 3), `HEAD` moved before an amend, a lock removed and recreated (Amended 2026-10-10: Create Branch's Discard refuses when `HEAD`, the commit or the name moved, and checks nothing else, the user's decision on its confirmation; an amend's re-check no longer compares the subject.) | integration tests in `cairn-git` against real `git` |
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
| C13 | Commit and amend: the message reaches git byte for byte under every `commit.cleanup` value as `git commit -F` would leave it; a `pre-commit` hook's failure shows its output and the skip offers only where a hook exists (`.git/hooks`, `core.hooksPath`); the draft survives it; a non-UTF-8 `i18n.commitEncoding` is refused; amend's staged list equals `git diff --cached --name-status HEAD^`, and a root commit's amend works, unstaging from it with `git rm --cached -q --`; Amend is disabled on an unborn branch; with no identity configured, git's own error is shown; recent messages equal `git log -n 10 --format=%B HEAD`'s (Amended 2026-10-10, C4: the skip is offered on every failed commit or amend, in the footer for both, and a skipped confirmed amend is not confirmed again; the hooks-path read, `CommitHooks` and their pins are gone; `i18n.commitEncoding` is refused as `git config` answers it — R6.11; a cancelled commit that git had already made is reported made, one it had not is `cancelled` with the draft kept — R4.7.) | integration tests in `cairn-git`, headless tests; C32 for the config read |
| C14 | Amend's button text and dialog are rendered from its `Consequence`; the dialog appears exactly when a remote has `HEAD` (an upstream at `HEAD`, behind it, none with another remote branch containing it, none at all) (Superseded 2026-10-10 — C1, C6 and rule 3: the button reads `Amend <short id>` with no line under it, button and chord identical; at the press the engine reads the cost inside the job that runs it, and an amend git logs and no remote has runs with no dialog; the dialog "Amend Commit" appears exactly when a remote has `HEAD` (an upstream at `HEAD`, behind it, none with another remote branch containing it) or git will keep no reflog (`core.logAllRefUpdates` false and no log yet, as `git config` answers), in the fixed sentences of R10.6, Cancel returning to the box unchanged; no amend read is asked on a status's arrival.) | integration and headless tests; worker tests that no amend cost read runs before the press |
| C15 | Every text field hands accelerator chords and held modifiers to the window: with a filter field focused, F5 refreshes and a Ctrl/⌘-click extends the selection (failing first on today's code); ⌘Return / Ctrl+Enter in the description commits without a newline | headless tests |
| C16 | The accelerator table maps each action to a list of distinct chords per platform, and every chord of R7.3's lists resolves per platform; each new action resolves only in its R7.3 scope — with the commit box focused, the stage, unstage, discard and Show Lost Commits chords resolve to nothing and the commit chord commits; the pin admits bare Enter, Backspace and Delete only in Local Changes' list-and-diff scope; the root `CLAUDE.md` modifier invariant and its twin, amended in phase 06, hold a list per action | the table's tests, the modifier guard |
| C17 | The discard dialog: modal (Tab stays inside, a window chord does nothing), focus on Cancel, Escape cancels, its text and button from the `Consequence` (`Discard Changes in 3 Files`, `Discard 2 Lines`) (Amended 2026-10-10, D1-D3, rule 6: titled "Discard Changes"; the one sentence frame, the worst loss only when it is worse than changes; "Show files" over a virtualized list of the paths for a selection of several; a mixed selection discarding what it can, one line saying what is left and the button counting only that; the dialog open at once on the press with "Counting…" and its button greyed until the `Consequence` arrives; no text cut.) | headless tests; a viewport twin for "Show files" over 50,000 paths |
| C18 | Local Changes: each of R8.2's routes stages and unstages a multi-selection — but a double-click, which acts on its own row (amended 2026-10-09, the user's decision on phase 07's QA item 4(b)); a drag between the lists auto-scrolls and survives rows unmounting mid-drag; the selection moves to the nearest remaining path; Ignore Whitespace is disabled and the diff exact (Amended 2026-10-10, rule 8, L2, L3, L5: every route acts on the selection wherever focus is; Stage All and Unstage All by Shift held on Linux and ⌥ on macOS, enabled whatever is selected, and by ⇊ and ⇈ in the headers; the selection after an action is C25's.) | headless tests |
| C19 | The gesture: a hovered chunk's actions stage, unstage and discard exactly that chunk; a drag-selection narrows them to its lines, across rows the virtual list unmounted; side by side keeps a selection in one column; the Commit and Changes tabs draw no action; a 10,000-line diff with the gesture builds one viewport (Amended 2026-10-10: with no lines selected the chords act on every selected file, wherever focus is; the 10,000-line twin stays and `files_drawn_together_build_one_viewport` goes with the view it pinned; a line selection across a refresh is C26's.) | headless tests, and a viewport twin |
| C20 | Show Lost Commits: the commits it adds equal `git rev-list <every old and new id of every entry of HEAD's and each local branch's reflog, read from the log files> --not --branches --remotes --tags HEAD` (amended 2026-10-09, the user's decision D: old ids too, as `git rev-list --reflog` reads them), on a fixture with an amended, a reset-away and a 4 KiB-message entry, and an amend whose log it created itself (the replaced commit only an entry's old id); those rows are dimmed — their text, the graph in its colours (amended 2026-10-09: Fork-settled on Fork's evidence, applied under the user's Fork-first rule); Create Branch (R11.3, amended 2026-10-09) creates the branch `git branch` would, checks it out keeping the changes as `git checkout -b` would or refuses with git's words, and with "Discard" names every loss in a confirmation and discards exactly those; the activity popover lists each operation with its prompt, its `git`, its scrubbed stderr and its recovery pointer, and `Remove index.lock…` appears exactly when R12.4 says and removes exactly `<gitdir>/index.lock` and nothing else; the filesystem-mutation guard (R12.5) fails on a removal, write or rename outside `ops/` (Amended 2026-10-10: Create Branch's Discard is C34's; the activity popover is C29's and `Remove index.lock…` C30's.) | integration and headless tests |
| C21 | On R13.1's clone and machine (the one `docs/research/diff-engine/measured-baseline.md` records): stage, unstage and discard a hunk and commit within git's own time plus one status read plus a flat 50 ms (the user's decision, 2026-10-08, at R13.2's amendment): about 93 ms to stage, 93 ms to unstage, 76 ms to discard and 87 ms to commit, from `docs/research/staging-and-commit/measured-baseline.md`'s highest sums (42.9, 42.8, 25.8 and 37.4 ms); Show Lost Commits' first frame recorded; `window_check` keeps every frame under 16.7 ms of UI-thread work while a hook runs and a stage lands (Amended 2026-10-10, R13.3: measured again at the merge bar over the rebuilt packet, an amend's press-time read recorded beside the four.) | `#[ignore]`d reporters driven by `CAIRN_BENCH_REPO`, numbers in `progress.md` and, at teardown, in `docs/research/staging-and-commit/` |
| C22 | D1 in `docs/design/engine.md` and the root `CLAUDE.md` names R3.9's two reads, the write verbs and the one file deletion made without `git` (R12.4); the destructive-operation roster, the confirmation-surface roster, the chord lists and the bare-key scope, the gesture's viewport twin (C19) and the filesystem-mutation guard (R12.5) each have their twin named in `CLAUDE.md` (Amended 2026-10-10: D1 and the root `CLAUDE.md` also name `git stripspace --strip-comments`, the config read R6.11 widens, Create Branch's forced checkout as Fork runs it and its one read, and no longer name `git rev-parse --git-path hooks`, the `--numstat` pair or `git ls-files --others`; the two rosters' changed rows and the narrowed bare-key scope have their twins named.) | review |
| C23 | `scripts/gate.sh` passes | the gate |
| C24 | Conflicts and operations in progress (L25): staging a conflicted row runs `git add` and `git status` then reports it resolved; a conflicted row offers no line gesture and no discard by any route, and no verb takes a patch or a discard for one; with a merge in progress the engine reports it, the commit box holds `MERGE_MSG`'s text, its commit has `HEAD` and `MERGE_HEAD` as parents, and Amend is disabled; during a rebase, `git am` (added 2026-10-09, the user's decision C), a cherry-pick and a revert the engine reports each, commit and amend are refused before git runs, and the commit box is disabled and names it; the commit box holds `MERGE_MSG` with git's comment lines visible (the user's decision E) (Amended 2026-10-10, superseding decision E and narrowing L25 — C2, C3, C8: the commit box holds `MERGE_MSG` cleaned of git's comment lines as `git stripspace --strip-comments` leaves it, and what it shows is what is committed; a single cherry-pick or revert is concluded by Commit (C33); a rebase, `git am` and a sequence still refuse, naming git's command; a detached `HEAD` commits under one line and no dialog.) | integration tests in `cairn-git` against real `git`, headless tests |
| C25 | One selection (added 2026-10-10, rule 8, L1, L3, L5; R8.1-R8.3): the selection is one value in one list with a primary; the diff shows the primary — the file clicked first, then Ctrl/⌘-click another, observed `lc2` — with Load Diff, the mode row and previous and next change; "k of n selected" in its bar while several are selected (unless the user drops it); Ctrl+S (⌘S) with focus in the diff and no lines selected stages the whole selection, and Return there does nothing (observed `lc3`, `lc4`); after an action, a refresh, an outside commit (observed `lc5`) or a filter, one pure rule gives the next selection — kept paths kept, else the row in the first one's place, else the nearest above — tested after an action, after a refresh, a path vanished, with gaps and with a filter; no file is ever acted on that the filter hides | unit tests of the rule, headless and window tests |
| C26 | A finished line selection is kept across a refresh whose re-read draws the same text (switching apps and back), and cleared with one notice, "The file changed — line selection cleared.", when the text differs; no act made over rows of another content reaches a write (added 2026-10-10, L4; R9.6) | headless tests, and a window test through a real refresh |
| C27 | Stage All and Unstage All (added 2026-10-10, L2; R8.2): while Shift is held on Linux (⌥ on macOS) both headers read Stage All and Unstage All, enabled whatever is selected; ⇊ in Unstaged and ⇈ in Staged reach them with no key held; a held Alt does nothing; with a filter on each takes the rows shown | headless tests, the accelerator table's tests |
| C28 | One home for each message (added 2026-10-10, rules 4-6, F1, F2; R14): a running write's name shows in the status box only after 250 ms and with " · N waiting" behind it; a git failure opens Git Error; a refusal found while running opens "Couldn't <name>" with one sentence, "Nothing was changed." once, and no Error Details; success shows nothing; a cancel opens no dialog; nothing is drawn under the file lists; a refusal known in advance is a greyed control with its reason, wrapped; no text a person needs is cut (no line cap on such a label); each operation's one name is the only wording of it; while any dialog or the popover is open — Create Branch included — the window's chords and the lists' keys do nothing | headless and window tests |
| C29 | Fork's Activity Manager (added 2026-10-10, F3, F4, F7, F8, F9; R12.1): each row its name, Fork's result line, `HH:MM:SS UTC` and ⚠ on a failure; the header its name, one of running, succeeded, failed, cancelled, the time and the duration; a failed or cancelled entry's one sentence; the confirmed prompt whole and wrapped, only for an operation that ran; "Show in Lost Commits" turning the mode on and selecting the replaced commit; whole oldest entries evicted at the byte bound, an entry larger alone cut under "Earlier output not kept."; a fetch's progress in the status box with Fetch greyed while it runs; the popover's two lists still one viewport each | headless tests, the popover's viewport twin |
| C30 | The leftover lock (added 2026-10-10, F5, F6, G; R12.4): the banner shows exactly while `<gitdir>/index.lock` exists, no `git` of Cairn's runs or waits and the file is older than about ten seconds, and goes when the file does, whoever removed it; a write that fails because git could not create `index.lock` is retried about a second later once or twice before it is reported, and its Git Error then carries `Remove index.lock…`; both routes open "Remove stale lock" with decision G's prompt, and the removal is followed by a refresh, never a retry of the write | worker tests against real `git`, headless tests |
| C31 | git's output read once (added 2026-10-10; R4.10): stdout and stderr are split by one line type into whole lines, never inside a character — a multi-byte character straddling the stderr piece limit arrives whole (failing first on `process/pipes.rs` as it was) — each line scrubbed of a URL's userinfo before it is kept or cut, the tail dropping whole lines with a plain "older lines dropped"; no view draws git's text but through the scrubbed lines | engine tests in `cairn-git`, and a guard or type that no render file takes git's raw `String` |
| C32 | git's answers, not Cairn's (added 2026-10-10, the four git-parity fixes; R6.10, R6.11, R11.3): a `MERGE_MSG` with `# Conflicts:`, a scissors line and `core.commentChar` set is shown and committed exactly as `git stripspace --strip-comments` leaves it; the stripspace read writes nothing and runs nothing, and its guard row (`"stripspace"` only in its `reads/` file, once, `--strip-comments` its one option) has a matcher self-test; `core.logAllRefUpdates` and `i18n.commitEncoding` are read as `git config` answers them, a linked worktree's `includeIf` included, with no gix configuration read left for either; a branch name is accepted or refused as `git branch` and `git checkout -b` take it — `@{-1}`, `@{`, `a..b`, a taken name, a folder clash — on the host's git and both floors | integration tests against real `git`, `git-floor`, the porcelain-read guard |
| C33 | A single cherry-pick or revert in progress is concluded by Commit (added 2026-10-10, C3; R6.9): the commit equals what `git commit -F` would make — the picked author kept, `CHERRY_PICK_HEAD` or `REVERT_HEAD` gone — with the box filled from the cleaned message; a sequence (`.git/sequencer`), a rebase and `git am` still refuse commit and amend before git runs, the box naming git's own command | integration tests in `cairn-git` against real `git`, headless tests |
| C34 | Create Branch's Discard (added 2026-10-10, B1-B4 and the user's decision on its confirmation; R11.3): with Discard chosen, Create and Checkout or Return builds the `Confirmed` in the Create Branch dialog — on the confirmation-surface roster — and no second dialog opens; the argv is exactly `checkout -q --no-track -f -b <name> <oid> --`; staged, unstaged and untracked-in-the-way changes are gone afterwards as Fork's command leaves them; the re-check refuses, writing nothing, when `HEAD`, the commit or the name moved, and checks nothing else; ⚠ beside Discard; "Stash and reapply" greyed with "Comes with stashing."; the button reads "Create and Checkout"; the dialog stays open beneath a Git Error or "Couldn't …" dialog and Close returns to it as left; `checkout_discarding_consequence`'s prediction, its loss kinds and `reads::untracked_paths` and `reads::change_lines` are gone | integration tests against real `git`, stub-git argv tests, headless tests, the seal guard's rosters |

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

Not done in this packet, added 2026-10-10 with the redesign, and filed at teardown: the review's
refactors the rebuild does not rewrite anyway (`review-code-engine.md` H1's one witness of a
path's state, M1's per-operation consequence types, M5, M6 and the L items;
`review-code-app-ui.md` H2's one pointer-drag primitive, H4's write ledger, M1's declarative
lane table, M2's one consequence ask, M6 and the L items) — each phase doc of the rebuild says
which it takes in passing; Fork's Activity Manager's All / User / Background tabs; and Fork's
"Stash and reapply", packet 5b's.
