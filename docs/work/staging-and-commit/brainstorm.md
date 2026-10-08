# Brainstorm — staging-and-commit

Locked decisions and rejected alternatives. Historical record: never
retro-edited. L1-L21 were locked by the user on 2026-10-07 and L22-L26 on 2026-10-08, after
seven evidence records under `docs/research/staging-and-commit/` were presented
with the mechanism, cost and evidence behind each option:

- `write-path-as-built.md` — the write path, the runner, the lanes and the seal as
  they stand at `36c0d5c`, and issues #16, #18, #19, #41, #44-#48.
- `staging-surface-as-built.md` — the diff model, the emitter, status, Local
  Changes and the diff view this packet adds actions to.
- `fork-staging-and-commit.md` — what Fork does, from its release notes, trackers,
  vendor docs and captures.
- `git-write-verbs.md` — every verb this packet runs, by experiment on git 2.30.9,
  2.32.7 and 2.56.0.
- `precedent-study.md` — discard safety, reflog views, operation logs and line
  staging in other clients.
- `freya-ui-apis.md` — what the linked Freya (`caa46f8`) offers for text input,
  dialogs, menus, pointer gestures and tests.
- `patch-mechanics-spike.md` — inverting a diff for unstage and discard, partial
  staging of untracked files, path quoting, and a stale patch at an offset, on git
  2.30.9 and 2.56.0.

## Locked 2026-10-07

**L1. The brief splits in two.** This packet builds the local write lane, the
seal, stage / unstage / discard by file, hunk and line (deleting untracked files
included), commit and amend, the reflog view and the operation log — the D7
milestone items, and the reflog with the first commit-level destructive operation
(program L4). A follow-on packet **5b `stash-and-ignore`** takes stash create,
snapshot, apply, pop and drop, and `.gitignore` editing, designed against the
operation log this one leaves. "Clean untracked" does not survive as a command of
its own (L8). Why: recon counted the brief as 13-15 phases in one packet, and
nothing would reach `main` until stash and ignore passed too.
Rejected: **one packet** for everything in the brief.

**L2. A discarded uncommitted edit is protected by its confirmation alone (closes
program O3).** No snapshot, no Trash, no auto-stash — as Fork, whose maintainer
has refused a Trash and an undo for discard (`fork-staging-and-commit.md` §2).
The confirmation therefore carries the whole weight: it says what is lost, how
much, and that it cannot be undone (L3, L8).
Rejected: **a hidden snapshot under `refs/cairn/backup`** (magit wip, jj) — it
covers line and hunk discards, but every discard becomes a write that takes the
index lock, runs clean filters on dirty files, grows the object store, expires
after 30 days and shows in `git log --all` (`precedent-study.md`, closing
section, option C). **A visible stash per discard** (option D) — fills the stash
list, and a stash drop then destroys the backup. **The OS Trash** (GitHub
Desktop, option B) — whole files only, misses the common hunk and line discards
and index content, and is a filesystem write outside git that Desktop has kept
patching (#13904). **An in-app undo** (option E) — applying a stale snapshot over
newer edits is itself destructive.

**L3. `Confirmed` is bound to a `Consequence`.** Verified at `36c0d5c`
(`crates/cairn-model/src/confirm.rs`): the token derives `Clone`, `by_user` is
callable from any crate, it holds free text only, and the guard's "some `ops/`
file names `Confirmed`" check is met by `ops/mod.rs` itself
(`write-path-as-built.md` §2.5-2.6). The packet makes it:
no `Clone` or `Copy`; constructed only in a roster of confirmation surfaces (the
dialog component and the commit box, L12); carrying an engine-computed
`Consequence` — what the operation will destroy: paths, line counts, blob,
commit or stash ids — from which the prompt is rendered; and required by value,
by guard, on a roster of destructive operations, each of which re-checks the
repository against its `Consequence` immediately before it runs and refuses when
anything moved. Closes the races recon found: `stash drop` by index (git refuses
an id), a clean list stale between prompt and run, and a hunk landing at an
offset (L17).
Rejected: **tighten in place** (no `Clone`, restricted constructor, roster)
without the `Consequence` and the pre-run check.

**L4. The local write lane.** Built as designed in `docs/design/concurrency.md`,
with four rules:
- its own thread, which requests reach directly — not through the repository
  thread, where a stage would wait behind a deep find;
- FIFO, shown as queued; a second write is queued, never refused; each queued
  patch is checked against the content it was built from before it applies
  (L17f), and dropped with a note when it is stale;
- a write counter: a status read that began before the latest write ended is
  discarded rather than drawn (today it is drawn, then corrected); the refresh
  after a write is narrowed by what it declared in `Invalidated` (a stage
  refreshes status only; a commit refs, status and history);
- while a commit's hooks run, Cairn suspends its own refreshes and queued index
  writes.
Rejected: **refuse while busy** (rapid stage clicks fail) and **route through the
repository thread**.

**L5. Fork's staging gesture, with edge auto-scroll.** Hover a chunk for an
outline and floating `Stage` and `Discard Changes…` (`Unstage` in the staged
diff); a drag-selection narrows them, and the keys, to the selected lines; with
nothing selected, keys act on the whole file; in side by side a selection stays
in one column (`fork-staging-and-commit.md` §1, `fork-detail-and-diff-ui.md`
Finding 23). The floating controls live outside the recycled rows, do not take
focus, and appear only in Local Changes — the Commit tab draws the same rows and
stays action-free. Edge auto-scroll uses `async-io`'s timer: `async-io` 2.6 is
already in `Cargo.lock` through Freya, and naming it directly in a render crate
is this decision (`freya-ui-apis.md` §7).
Rejected: **scroll only while the pointer moves**, and **the mockup's selection
gutter** (not Fork).

**L6. No ignore-whitespace in the staging view.** In Local Changes the Ignore
Whitespace button is disabled and the diff is always the exact one, so every drawn
change maps to exact lines; the shared setting is untouched and still applies in
the Commit and Changes tabs. The user's answer, beyond both options offered.
Rejected: **hide hunk and line actions under `-w` and offer the toggle**, and
**map a drawn `-w` hunk to the exact changes it overlaps** (stages edits the user
never saw).

**L7. Fork's five routes for files, and Fork's keys.** Double-click a row;
Return or ⌘S (macOS), Enter or Ctrl+Shift+S (Linux, as Fork for Windows); drag
between the lists; the `Stage` / `Unstage` header buttons, ⌥-held for Stage All /
Unstage All, and Fork's double-chevron Stage All; the context menu (Stage,
Discard Changes…, Stage All, Copy Path — `Stash N Files…` and `Ignore ›` arrive
with 5b). Discard is ⌫ or ⇧⌘D (macOS), Backspace, Delete or Ctrl+Shift+D (Linux).
Multi-select by Ctrl/⌘-click, Shift-click and Shift+↑/↓; a folder acts on
everything under it; after a stage the selection moves to the nearest remaining
file (`fork-staging-and-commit.md` §1). Two settled things change:
- the accelerator table's pin (`chords_are_distinct_and_every_bare_one_is_a_function_key`)
  admits bare Enter, Backspace and Delete in one scope only — a focused file list
  or diff, never while a text field holds focus;
- refs-and-status's "the filter field is the only control above the lists" (the
  user, 2026-10-07) gives way to Fork's header buttons.
Drag uses one drop zone per list, never per row, because a row unmounted by the
virtual list mid-drag leaves a stale payload (`freya-ui-apis.md` §6).
Rejected: **all but drag**, and **no bare keys**.

**L8. Fork's discard rules, with Cancel focused.** Staged changes are never
discardable; there is no Discard All (select all, then discard); the confirmation
cannot be skipped; an untracked file is discarded by deleting it, with the same
dialog. The prompt is rendered from the `Consequence` in Fork's words with counts
— "Do you want to discard the changes in 3 files? 2 modified (14 lines), 1
untracked file deleted (2.1 KiB). You can't undo this action." — and its button
names the count (`Discard Changes in 3 Files`, `Discard 2 Lines`). Focus starts on
**Cancel**: a deliberate deviation, since Fork styles Discard as the default and
its tracker asks for this (Tracker #1080). Deleting untracked files is
`git clean -f -- <exact paths>` under literal pathspecs, the list taken from
status and never from `clean -n` (localised and quoted, `git-write-verbs.md` §4);
a collapsed directory row adds `-d`; a nested repository is refused, because
`-ff` deletes its unpushed history and the prompt cannot count it. Fork has no
Clean command, so neither does Cairn.
Rejected: **Discard focused**, as Fork styles it.

**L9. Fork's commit box.** A subject with a characters-left counter (soft at 50,
red past 70, fixed while Cairn has no settings); a multi-line description with a
ruler at 72; Recent Commit Messages as a menu and ↑/↓ in the subject — the latest 10 commits
on the current branch, read from history and never stored, as Fork's are
(Tracker #720); `Amend`, which loads HEAD's message
and lists HEAD's files among the staged (a staged list against `HEAD^`; unstaging
out of an amend is `git reset -q HEAD^ -- <path>`) and unticks itself after the
commit; `Commit N Files`, ⌘Return / Ctrl+Enter, disabled with nothing staged
unless amending; the draft kept in memory across Amend, a failed hook and
refreshes. Not built, as Fork: Commit and Push (push is packet 6), sign-off and
signing as box controls (git config is honoured; Fork's per-repository sign-off
setting is filed), author override, empty commits, an upfront skip-hooks.
Rejected: **without recent messages**.

**L10. The message reaches git as `git commit -F -`.** On stdin, so it is never
on `argv` or in the command log; hooks get an empty stdin and the message
survives a `pre-commit` (verified, `write-path-as-built.md` probes). No
`--cleanup`: exactly `git commit -F`'s behaviour — the user's `commit.cleanup`,
else `whitespace`, which keeps `#` lines. A non-UTF-8 `i18n.commitEncoding` is
refused with its reason (filed), since transcoding needs a dependency. Identity is
git's: its own error is shown.
Rejected: **`--cleanup=strip`**, which diverges from `git commit -F`.

**L11. Hooks, signing and prompts.** A running commit shows busy and its elapsed
time, its hook output streams into the operation log, and it can be cancelled
(killing a commit in its hooks leaves no `index.lock`, `git-write-verbs.md` §10).
A failure opens Fork's `Git Error` dialog — the command and git's output, ANSI
stripped — with `Skip pre-commit hooks and commit` (`--no-verify`) only where a
`pre-commit` or `commit-msg` hook exists in the hooks directory git resolves
(`core.hooksPath` included, so Husky is seen); the draft stays. Every local write
carries an askpass token and the window shows a prompt during any write, not only
a fetch (SSH signing with an encrypted key, an LFS smudge during discard).
`GNUPGHOME`, `DISPLAY`, `WAYLAND_DISPLAY` and `XAUTHORITY` join the inherited
roster so GPG's pinentry reaches the desktop (#18; each a deliberate leak,
`destructive-ops-reviewer`'s check 9). A hook that reads `/dev/tty` hangs when
Cairn was launched from a terminal, because git sits in a background process
group of it; the fix, `setsid`, needs `unsafe` `pre_exec`, which the workspace
forbids — stated as a residual and filed.
Rejected: **leave the roster unchanged** (GPG pinentry may never appear).

**L12. Amend is confirmed by its button, and by a dialog only when pushed.** In
amend mode the button reads `Amend <short id>` above "Replaces <short id>
'<subject>'. The old commit stays in Show Lost Commits."; the `Confirmed` is built
from that visible text and bound to HEAD's id, so the commit box is the second
confirmation surface on L3's roster. A dialog appears only when HEAD is already on
a remote — "<short id> is already on origin/main. Sharing the amended commit needs
a force push." — detected from the upstream's ahead/behind already read, or, with
no upstream, a hidden walk of `HEAD --not --remotes`.
Rejected: **a dialog on every amend** (Fork for macOS asks nothing).

**L13. The reflog view is Fork's Show Lost Commits, with `Create Branch Here…`.**
A toggle (⌘⇧. / Ctrl+Shift+.) that adds every entry of `HEAD`'s and each branch's
reflog to the walk's tips and dims the commits no ref reaches; recovery is a
minimal `Create Branch Here…` (a name, then `git branch -- <name> <oid>`) that
packet 7 later reuses. An amended-away commit appears here (program L4). The
reflog is read whole, as the stash list is, because gix's reverse iterator fails
on a line over 4 KiB.
Rejected: **Copy Hash only** (recovery through a terminal contradicts program L4),
and **a dedicated reflog list** (Tower, magit — not Fork).

**L14. The operation log is Fork's Activity popover, with prompts.** Each entry:
the operation's name, start time, duration, outcome, the confirmed prompt if it
had one, the `git` it ran (from the existing command log) with its stderr tail, and
a recovery pointer where one exists (an amend points at the old commit in Show
Lost Commits). Credentials in a URL are scrubbed from drawn stderr (#46, for
display). Session only and bounded, as Fork. Closes #41.
Rejected: **persisted across sessions** (a write outside git with a storage
design of its own).

**L15. Write policies (#45, #44, #19, #47).** A cancelled or unwatched write says
"may have taken effect" and the refresh shows the truth; the bounded-output
helpers become read-only at the type; the first close during a local write waits
and says so ("Finishing commit…") and never kills it, a second close after
`CLOSE_PATIENCE` forces it and a lock left behind is named on the next open; lock
files are carried on every local outcome and named in its error; a confirmed
`Remove index.lock` is offered only while Cairn runs no `git` in that repository,
its `Consequence` the path and the lock's age, its prompt saying another program
may own it; the local lane's cancel is keyed by operation id (fetch's stays as
filed in #47); only a commit (or an amend) is cancellable; #66's index refresh stays deferred.
Rejected: **no lock removal**.

**L16. Every `git apply` runs with `--whitespace=nowarn`.** The user's
`apply.whitespace` would otherwise strip whitespace from staged lines under `fix`
or refuse under `error` (verified on 2.30-2.56, `git-write-verbs.md` §1): what is
staged is what was drawn and selected. A deliberate departure from `git add -p`,
which honours the setting.
Rejected: **honour the config**, as `git add -p` does.

**L17. The patch engine.** All verified on git 2.30.9 and 2.56.0
(`patch-mechanics-spike.md`):
- **a.** Unstage and discard **invert the diff** and reuse the forward emitter —
  one rule, no `-R`: stage lines is the unstaged diff applied with
  `git apply --cached`; unstage lines is the staged diff inverted, applied with
  `git apply --cached`; discard lines is the unstaged diff inverted, applied with
  `git apply` to the working tree. `TextDiff`, `Selection` and `ChangedFile`
  gain pure inversions. The mirrored rule applied with `-R` gives the identical
  index (E1b) and is the tests' independent oracle, beside the reference applier
  and real-git round trips on both floors.
- **b.** Part of an untracked or new file can be staged (a `new file mode` patch
  of the selected lines, E3) or discarded (a partial deletion emitted as a
  modification, E4), as Fork allows; discarding all of one is L8's delete.
- **c.** Whole-file only: deletions on both sides (`git add -p`'s rule), binary,
  LFS pointers, files past the limits, submodules and type changes — staged with
  `git add`, unstaged with `git reset -q --` (which works on an unborn branch,
  where `restore --staged` fails), discarded with `git restore --worktree --`. A
  staged rename unstages by lines as a content-only patch at its new path, and
  whole as `git reset -q -- <old> <new>`.
- **d.** A mode change is its own selectable item — `Selection` gains it, the diff
  draws it as a row with its own actions, and a line selection does not carry it —
  as `git add -p` asks it separately.
- **e.** Path lines are C-quoted exactly as git's `quote_c_style` quotes them; a
  raw tab is refused by `git apply` (E5).
- **f.** Before every apply the operation checks the content it was built from:
  for stage, the index entry's blob is the diff's `old_id`; for unstage, its
  `new_id`; for discard, the index entry is `old_id` and the working-tree file
  hashed in git's form (`git hash-object --path=<p> -- <p>`, a read without `-w`,
  which runs the clean filter the diff ran) equals the drawn new side. The index
  is read with gix. A stale patch writes nothing and says why; without this git
  applies it at an offset and exits 0 (E6, E6b).
Rejected: **a mirrored emit rule applied with `-R`** (a second rule in the model),
**untracked files whole only**, **today's rule that any partial patch carries the
mode change**, and **an index-only check** for discard.

**L18. Staged renames are drawn as git draws them.** The staged diff runs with
`--no-renames` today, so a staged rename draws as a whole-file addition where
`git diff --cached` pairs it — a parity bug (program memory: a divergence from git
is critical). The staged side follows git's rename detection; unstaging lines
inside a rename changes content only (L17c).
Rejected: **refuse line-level unstage of a rename and file the parity gap**.

**L19. One key policy for every text field.** A shared pre-key handler in
`cairn-ui` asks the accelerator table, hands accelerator chords and lone modifier
keys through to the window unclaimed, claims ⌘Return / Ctrl+Enter for commit
without inserting a newline, and stops a primary+letter chord typing its letter.
It closes the hole recon inferred from Freya's dispatch (`freya-ui-apis.md` §2):
a focused field cancels the window's key event, so the filter fields hide F5 and
the held modifiers from `HeldKeys` today — confirmed by a failing test first.

**L20. A measured bar against git's own.** On a plain, non-shared clone of
rust-lang/rust on tmpfs — never the bench itself, whose bytes and timestamps are
the baseline (program memory): stage, unstage and discard a hunk and commit, each
from the press until the refreshed lists are drawn; Show Lost Commits from the
toggle to its first frame; no frame over 16.7 ms while a hook runs. Against git's
own time for the same verb plus one status read, measured first and recorded in
`docs/research/staging-and-commit/measured-baseline.md`; the margin is written
into the PRD once that baseline exists.
Rejected: **no measured bar** (writes are not read surfaces).

**L21. The roadmap gains 5b.** `stash-and-ignore`, depending on 5: stash create,
Save Snapshot, apply, pop, drop and `.gitignore`. Rejected: **folding them into
packet 7**.

## Locked 2026-10-08, after the coverage audit

A fresh audit of the planning pass found five behaviours the decisions above left
open or contradicted; the user locked each.

**L22. An action maps to a list of chords.** Fork binds stage to Return and ⌘S,
and discard on Windows to Backspace, Delete and Ctrl+Shift+D; the accelerator
table's contract (`chord(action, platform) -> Option<Chord>`, and the root
`CLAUDE.md` invariant "at most one chord per platform") would drop Fork's
alternates. The table maps an action to a list of distinct chords per platform,
and the invariant and its twin are amended with it.
Rejected: **one chord per action**, dropping Fork's alternates.

**L23. D1 is amended for one file deletion: a stale `index.lock`.** git has no
verb that removes a lock, so `Remove index.lock…` (L15) deletes exactly
`<gitdir>/index.lock`, in `ops/` only, after re-checking the lock's age and
identity against its `Consequence`. A guard holds filesystem-mutating calls to
`crates/cairn-git/src/ops/`, which packet 5b's `.gitignore` write will reuse.
Rejected: **drop the removal**, leaving the error to name the lock and #19 open.

**L24. A submodule's changes are not discardable.** `git restore -- <sub>` exits
0 and leaves the submodule's commit where it is (`git-write-verbs.md` §3,
verified); only `git submodule update` moves it back, and a prompt cannot count
what is dirty inside the submodule. A submodule row offers no discard and says
why; filed. Rejected: **`git submodule update --checkout` as a destructive
operation** whose prompt cannot state its cost.

**L25. Conflicts and a merge in progress follow Fork.** Staging a conflicted row
is whole-file only and marks it resolved (`git add`); a conflicted row offers no
line gesture and no discard (Fork refuses, TrackerWin #1859). With a merge in
progress the commit box pre-fills git's `MERGE_MSG` and its commit is the merge
commit; Amend is disabled. During a rebase, cherry-pick or revert the commit box
is disabled and names the operation. Resolving a conflict's content stays the
second lap's (D6). Rejected: **refuse every action on a conflicted row and
disable committing during any operation**.

**L26. The commit identity is the terminal's.** `GIT_AUTHOR_NAME`,
`GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL` and `EMAIL` join the
inherited roster, so a per-project identity (direnv) is the one Cairn commits
with; the date variables do not, since a stale one would stamp every commit.
Rejected: **leave them out and file the divergence**.

## Settled in passing

- Destructive operations in this packet, each on L3's roster: discard lines,
  discard files, delete untracked files, amend, remove `index.lock`. Not
  destructive: stage and unstage (files, lines, a mode change), commit, create a
  branch. Unstaging a staged version the working tree no longer holds leaves that
  blob unreachable; Fork does not confirm it either — a residual for
  `destructive-ops-reviewer`, not a confirmation.
- Linux follows Fork for Windows' chords, as refs-and-status settled for Refresh.
- Fork's post-stage selection (the nearest remaining file) is evidenced
  (Tracker #514); the chunk-discard dialog's default button and how Fork stages
  lines of an untracked file are not — the user may check them on their own Fork.
- Fork pre-fills an empty commit box from `commit.template`
  (`fork-staging-and-commit.md` §5, "Template"), and `git commit -F` ignores the
  template; L9 does not build the pre-fill, so it is filed, not built.
