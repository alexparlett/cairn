# State — staging-and-commit

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 01 (the seal) built on `feature/staging-and-commit` in packet
mode, QA adjudicated and its fixes landed; it waits on three user decisions
(below) before it is done. Phases 02-12 not started.**

## Locked decisions

L1-L26 in `brainstorm.md`; the spec is `docs/prd/staging-and-commit.md`. The ones
that most constrain implementation:

- **Stash and `.gitignore` are packet 5b's** (L1). No Clean command; deleting
  untracked files is discard on untracked rows (L8).
- **No backup before a discard** (L2, closes program O3). The confirmation is the
  only barrier, so it names exactly what is lost.
- **`Confirmed` carries an engine-computed `Consequence`**, has no `Clone`, is
  built only by the dialog and the commit box, and every destructive operation
  re-checks its `Consequence` before it runs (L3).
- **The local write lane**: its own thread, FIFO, op ids, a write counter over
  status, refresh by `Invalidated`, quiet while a commit runs (L4); close waits
  for a write (L15).
- **Patches**: invert the diff and reuse the forward emitter, no `-R` (L17a);
  untracked files partially; deletions, binary, LFS, oversize, submodules, type
  changes and conflicted paths whole, and a submodule never discarded (L24); a mode change its own item; C-quoted paths; a stale check before
  every apply (L17); `--whitespace=nowarn` always (L16); staged renames paired as
  git pairs them (L18).
- **Fork's UI**: the hover-and-drag gesture with `async-io` auto-scroll (L5); no
  Ignore Whitespace in Local Changes (L6); five file routes, Fork's keys with bare
  Enter/Backspace/Delete in one scope (L7); Fork's discard rules, Cancel focused
  (L8); Fork's commit box (L9); amend confirmed by its button, a dialog only when
  pushed (L12); Show Lost Commits with `Create Branch Here…` (L13); the Activity
  popover with prompts, session only (L14).
- **Commit** is `git commit -F -`, no `--cleanup` (L10); every local write
  carries an askpass token, and four display/GPG variables join the inherited
  roster (L11), with the five identity variables beside them and no date
  variable (L26).
- **An action maps to a list of chords** per platform, amending the modifier
  invariant's "at most one" (L22).
- **One mutation without `git`**: `Remove index.lock…` deletes exactly
  `<gitdir>/index.lock` in `ops/`, and a guard holds every filesystem-mutating
  call to `ops/` (L23).
- **Conflicts and a merge follow Fork**: a conflicted row stages whole (`git add`)
  with no lines or discard; a merge pre-fills `MERGE_MSG` and commits the merge,
  Amend disabled; rebase, cherry-pick or revert disable the box (L25).

## Open questions

- C21's margin — written into the PRD by amendment once phase 03 has measured
  git's own numbers (R13.2).
- Fork's chunk-discard dialog default button and how Fork stages lines of an
  untracked file — the user may check on their own Fork; Cairn's choices (Cancel
  focused, a partial new-file patch) stand regardless.
- Whether a `new file mode` patch applies against an intent-to-add entry
  (`patch-mechanics-spike.md` E3b tested only a modification patch) — phase 02.

## New modules and interfaces

Phase 01:

- **`cairn_model::Confirmed`** (`crates/cairn-model/src/confirm.rs`): two private
  fields, `consequence: Consequence` and `prompt: String`; derives `Debug`,
  `PartialEq`, `Eq` only — no `Clone`, `Copy` or `Default`. One constructor,
  `Confirmed::by_user(consequence: Consequence) -> Self`, which renders the prompt
  from the consequence; readers `consequence(&self) -> &Consequence` (for the
  operation's re-check) and `prompt(&self) -> &str` (for the log). Six
  `compile_fail` doctests beside a passing scaffold.
- **`cairn_model::Consequence`** (`crates/cairn-model/src/consequence.rs`), plain
  data, `Clone`, built by the engine with struct literals: `DiscardLines { path,
  index: Option<Oid>, working_tree: Oid, selection: Selection }` (the selection of
  the unstaged diff the patch is built from; the counts are its own); `DiscardFiles { files:
  Vec<DiscardedFile> }`, each `DiscardedFile { path, loss: FileLoss }`, `FileLoss`
  `Modified { index: Oid, working_tree: Option<Oid>, lines: Option<usize> }` or
  `Untracked { working_tree: Oid, bytes: u64 }`; `Amend { commit: Oid, subject,
  published: Publication }`, `Publication` `Unpublished`, `Upstream(RefName)` or
  `SomeRemote`; `RemoveLock { path: PathBuf, modified: SystemTime, read_at:
  SystemTime, bytes: u64, device: u64, inode: u64 }` (the age rendered is
  `read_at - modified`, so the renderer reads no clock; mtime, size, device and
  inode are what the re-check compares). Every path and subject is escaped as git
  C-quotes a path (`quote_c_style`, `core.quotePath=false`), plus line separators
  and bidirectional controls; a file deleted in the working tree is named
  "deleted file restored". An operation derives every target from
  `confirmed.consequence()`, never from a parameter beside it.
  `Consequence::prompt()` renders the prompt (L8's words for a discard, R10.6's for
  an amend) and `Consequence::action()` the button's label (`Discard 2 Lines`,
  `Discard Changes in 3 Files`, `Amend <short>`, `Remove index.lock`). **Discarding
  files and deleting untracked files share `DiscardFiles`** — a deviation from the
  phase doc's five variants, because Fork confirms a mixed selection in one dialog
  with one prompt (L8), and one prompt is one confirmation; phase 03's operation
  for it restores the tracked files and deletes the untracked ones under one
  re-check. An untracked directory row is not modelled (status lists untracked
  files one per file; a nested repository is refused before any consequence);
  phase 03 extends the type, with a test, if it needs one. A mode change selected
  for discard is not modelled either (phase 02/03).
- **`ops::Performed::destructive(description, confirmed: Confirmed, invalidated)`**
  now takes the token by value and records `confirmed.prompt()` (R1.6): the record
  spends it. `ops::describe_destructive` remains the placeholder; phase 03 deletes
  it and its roster row.
- **The bounded-output helpers** `Invocation::collect` and `finish_within` are on
  `impl Invocation<Read>` alone (R4.8); a write is driven with `finish` or
  `records`. `Invocation::drive` takes `ceiling: Option<K::Ceiling>`, where
  `Kind::Ceiling` is `usize` for a read and `Infallible` for a write. `Error::GitOutputTooLarge` lost its `stranded_locks` field (only a
  read reaches it). The `#[cfg(test)]` `GitCommand::collected` gathers through
  `finish`, so tests still collect a write's output.
- **The guard's rosters** (`crates/cairn-guards/tests/invariants.rs`, in
  `destructive_operations_are_sealed_behind_the_confirmation_token`):
  `DESTRUCTIVE_OPERATIONS` (file, function) — phase 03 adds discard lines, discard
  files, phase 05 amend, phase 11 remove-lock, and 03 removes the
  `describe_destructive` row; `CONFIRMED_RECORD` (file, `Performed`,
  `destructive`: excused only inside `Performed`'s inherent impl);
  `CONFIRMED_HOLDERS` (files of the engine whose types may keep a `Confirmed` in a
  field) — empty, so a phase-04 lane request carrying a token lives in
  `cairn-app`, never in a `cairn-git` type;
  `CONFIRMATION_SURFACES` (files) — empty; phase 06 adds the dialog component,
  phase 09 the commit box. `CONSEQUENCE_BUILDERS` (`cairn-model`, `cairn-git`):
  production code anywhere else may hold a `Consequence` and call its methods but
  never spell `Consequence::` or its parts' names — **so phase 09, which must know
  whether an amend is published to decide on the dialog, adds a method to
  `Consequence` (for example `needs_force_push()`) rather than matching on
  `Publication` in `cairn-app`.**

## Carried forward from phase 01's QA (owned by the phase named)

- **Phase 03** (QA item 18): an empty `DiscardFiles` renders "the changes in 0
  files? ." and a zero-line `DiscardLines` "discard 0 lines"; "never empty" is a
  doc comment, not a check. The builder that computes either refuses empty input,
  with a test — or the type is made non-empty then.
- **Phase 03** (QA item 19): no variant expresses a mode change selected for
  discard (R2.4); add it with the operation, with a test.
- **Phase 03** (QA item 25): the working-tree blob in a `Consequence` is hashed
  through clean filters, so an edit that changes only line endings after the
  confirmation matches and is lost. Decide the deletion re-check's hashing
  (consider `git hash-object --no-filters` for it) and test it.
- **Phase 09** (QA item 21): the amend prompt joins two texts (the force-push
  warning and the "Replaces" line) that phases 06 and 09 may draw on two surfaces;
  render each part separately, and make the recorded prompt exactly what the
  surface drew.
- **Phase 11** (QA item 23): the remove-lock prompt does not say whether removing
  the lock can be undone, and says "corrupt the repository" where the index is
  what is at risk; settle the wording there.

## Waiting on the user (phase 01 is not done until these are applied)

- **QA item 6**: whether `DESTRUCTIVE_OPERATIONS` must be non-empty (the
  placeholder row counts until phase 03), or may be empty with the vacuity stated
  as a residual.
- **QA item 13**: the amend prompt's "The old commit stays in Show Lost Commits."
  (R10.6's locked wording) — left as it is until answered.
- **QA item 34a**: R1.2's "the lines and bytes per path" against
  `FileLoss::Modified` and `DiscardLines`, which carry no bytes — left as they are
  until answered.

## Validation status

| Phase | Status |
| --- | --- |
| 01 seal | QA adjudicated, confirmed fixes landed, full gate green; pending the three user decisions above (items 6, 13, 34a) — not done until they are applied |
| 02 patch engine | not started |
| 03 write verbs | not started |
| 04 local lane | not started |
| 05 commit engine | not started |
| 06 render foundations | not started |
| 07 Local Changes actions | not started |
| 08 diff gesture | not started |
| 09 commit box | not started |
| 10 lost commits | not started |
| 11 activity and measured | not started |
| 12 QA | not started |
