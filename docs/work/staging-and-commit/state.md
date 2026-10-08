# State — staging-and-commit

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 01 (the seal) done on `feature/staging-and-commit` in packet
mode: QA adjudicated, every confirmed fix and the user's three decisions of
2026-10-08 applied, full gate green. Phase 02 (the patch engine) done in packet
mode: QA adjudicated, confirmed fixes applied, full gate green; items 10-11 batched for
the user's review at the end of the packet. Phase 03 (the write verbs) built in packet
mode: QA adjudicated, confirmed fixes and the user's four decisions (2, 3, 5, 6) applied,
C21's margin decided (a flat 50 ms) and amended, full gate green. Phase 04 (the local
lane) built in packet mode, gate green, QA pending; two items batched for the user
(progress.md's phase 04 entry). Phases 05-12 not started.**

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

- ~~C21's margin~~ — decided by the user on 2026-10-08: a flat 50 ms per verb over
  git's own time plus one status read (≈93 ms stage, ≈93 ms unstage, ≈76 ms discard,
  ≈87 ms commit); PRD C21 and R13.2 amended. Phase 11 measures Cairn against these.
- Fork's chunk-discard dialog default button and how Fork stages lines of an
  untracked file — the user may check on their own Fork; Cairn's choices (Cancel
  focused, a partial new-file patch) stand regardless.
- ~~Whether a `new file mode` patch applies against an intent-to-add entry~~ —
  settled in phase 02: it does, on git 2.30.9, 2.32.7 and 2.56.0, leaving a real
  entry with the lines selected (`a_new_file_patch_applies_over_an_intent_to_add_entry`,
  `crates/cairn-git/tests/diff/staging.rs`), so an intent-to-add path stages through
  the same rule as an untracked one.

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
  published: Publication, reflog: Reflog }`, `Publication` `Unpublished`,
  `Upstream(RefName)` or `SomeRemote`, `Reflog` `Written` or `NotWritten` (the
  prompt promises "stays in Show Lost Commits" only for `Written`, and otherwise
  says the old commit can't be recovered because the repository keeps no reflog —
  R10.6 as amended); `RemoveLock { path: PathBuf, modified: SystemTime, read_at:
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

Phase 02 (`docs/systems/diff.md`, "Stage, unstage and discard" and "A staged
rename or copy is paired as `git diff --cached` pairs it"):

- **`cairn_model::action_patch(PatchAction, &FileDiff, &Selection) -> Patch`**
  (`crates/cairn-model/src/action_patch.rs`), `PatchAction` `Stage | Unstage |
  Discard`: what each verb of R3.1-R3.3 carries. Give it the diff the selection was
  made on — the staged `FileDiff` for `Unstage`, the unstaged or untracked one
  otherwise — exactly as the engine answered it. Stage is `emit_patch` as drawn;
  unstage and discard are `emit_patch` over the inversion; apply with `git apply
  --cached --whitespace=nowarn -` (stage, unstage) or `git apply --whitespace=nowarn
  -` (discard), never `-R`. Empty patch = nothing to apply (nothing selected, part
  of a whole-file-only change, or a state that is not text): phase 03's verb must
  refuse an empty patch rather than run `git apply` on nothing, and route whole-file
  changes to its file verbs.
- **Inversions**: `TextDiff::inverted` (a replacement inverts into its insertion
  then its removal — see Decisions in progress.md), `Selection::inverted`,
  `ChangedFile::inverted` (a copy inverts to its destination deleted).
- **`Selection`'s mode item**: `select_mode`, `unselect_mode`, `holds_mode`;
  `with_every_change` selects lines only; `len` counts lines only; `is_empty` is
  false for the mode alone. A whole-file stage by patch must select the mode too.
- **Quoting**: `emit_patch` writes every path line as git's `quote_c_style`
  (`crates/cairn-model/src/c_quote.rs`, crate-private), with git's trailing tab on a
  `---`/`+++` label holding a space.
- **`crate::reads::staged_pairing`** (`crates/cairn-git/src/reads/working_tree.rs`)
  and `WorkingTreeQuery::paired`: the staged side of `Repository::working_tree_diff`
  now answers a staged rename or copy as git pairs it (status `Renamed`/`Copied`,
  both paths), one more read process under rename detection. A paired read that
  moved between its two reads is `ContentReadsDisagree` (the diff lane asks again).
- **Test harness for later phases**: `crates/cairn-git/tests/diff/staging.rs` builds
  each C3 case and checks an apply against three oracles; phase 03's verbs can
  reuse its cases (`edited`, `Case`) and oracles (`forward_rule`, `mirrored_rule`,
  `left_out`) to test through the real operations.

Phase 03 (`docs/systems/staging.md`):

- **The verbs** (`cairn_git::ops`, each `-> Result<Performed, Error>`, each taking
  `token: Option<&AskpassToken>` last, for phase 04's lane): `stage_lines(git, repo, diff,
  &Selection, token)` and `unstage_lines(..)` (give them the diff the selection was made on:
  unstaged/untracked for stage, staged for unstage); `stage_files(git, repo, &[RepoPath],
  token)`; `unstage_files(git, repo, &[RepoPath], &UnstageTo, token)` with `UnstageTo::Head`
  (`git reset -q`), `UnstageTo::Commit(Oid)` (out of an amend: `HEAD`'s parent) and
  `UnstageTo::Nothing` (out of a root commit's amend: `git rm --cached -q`, which git refuses
  where the staged content differs from both the file and `HEAD` — phase 05's to meet);
  `discard_lines(git, repo, Confirmed, token)` (it applies the patch the `Consequence`
  carries — no diff is passed) and `discard_files(git, repo, Confirmed, token)`, which
  answers `Error::DiscardIncomplete { performed, kept, failure }` when it did not take
  every confirmed file (the lane must log `performed`, whose prompt the user accepted).
- **The builders**: `discard_lines_consequence(git, repo, &FileDiff, Selection)` and
  `discard_files_consequence(git, repo, &[RepoPath])` — computed on a worker (they run
  `git`), each refusing before any prompt with `Error::Refused { path, why: Refusal }`
  (`NothingSelected`, `WholeFileOnly`, `Conflicted`, `Submodule`, `NestedRepository`,
  `NotAFile`, `Obstructed`, `NoUnstagedChange`, `NotWhatWasConfirmed`) or `Error::NoPaths`. A selection of
  every line of a new file is `WholeFileOnly`: phase 08 routes it to `discard_files`.
- **Outcomes to draw**: `Error::ChangedSinceRead { path }` (a stale patch, R3.7) and
  `Error::ChangedSinceConfirmed { path }` (R1.4), never git's failure; `Performed::locks()`
  (`Locks { before, after }`, R3.8); `Invalidated`: stage/unstage → index, discard lines →
  working tree, discard files → index and working tree.
- `ops::CLEAN_ARGUMENT_BYTES` (64 KiB): `git clean`'s paths per invocation.
- `crate::reads::hash_object` (used by the stale check) and `reads/hooks_path.rs`'s
  `hooks_path` — **phase 05**: re-export it from `reads/mod.rs` (it is module-private and
  `expect(dead_code)` until then).
- `Consequence::DiscardLines` gained `on_disk: Oid`, `executable: bool`, `mode:
  Option<(FileMode, FileMode)>` and `patch: Patch`; `FileLoss::Modified` gained
  `executable` and `mode`, `FileLoss::Untracked` `executable`, and `FileLoss::Emptied` is new
  (an intent-to-add file, which the discard leaves empty); every working-tree id is the bytes
  on disk hashed with no filter.

Phase 04 (`docs/systems/git-processes.md`, "The local write lane"):

- **`crate::worker::LocalWrite`** (`crates/cairn-app/src/worker/local_lane.rs`): `StageLines {
  diff: Box<FileDiff>, selection }`, `UnstageLines { .. }`, `StageFiles { paths }`,
  `UnstageFiles { paths, to: UnstageTarget }` (`Head`, `Commit(Oid)`, `Nothing`),
  `DiscardLines(Confirmed)`, `DiscardFiles(Confirmed)`, and `#[cfg(test)] HeldCommit {
  remote }`; `what()` names it ("staging 1 file", "commit"). **Phase 05 adds `Commit` and
  `Amend`**: `is_commit` true, `read_again` `Everything`, and `perform` calls
  `lane.install(id, Box::new(move || cancel.cancel()))` once git runs — the tests that drive
  `HeldCommit` are the ones to re-run against `git commit`.
- **Requests**: `Request::Write { id: OperationId, write }`, `Request::CancelWrite { id }`,
  `Request::RefreshStatus` (status alone). `Request` is no longer `Clone` (a test-only
  `Clone` refuses a destructive write). Ask a write with `local_writes::ask(&mut writes,
  submit, write)`, which takes the id and keeps it queued.
- **Updates**: `WriteStarted { id }`, `WriteEnded { id, ending: WriteEnding, read_again:
  ReadAgain }`; `Opened { name, locks }` (the lock files as the repository opened).
  `WriteEnding::{Done(Done), Stale { path, message }, Refused, Failed { message, locks },
  MayHaveTakenEffect { message, locks }, Incomplete { done, kept, message }, NotRun }`;
  `Done { description, acknowledged, locks_before, locks_after }`.
- **Window state**: `View::writes: State<LocalWrites>` (`crates/cairn-app/src/local_writes.rs`):
  `queued()`, `running()`, `last()`, `locks()`, `closing_on()`; `Closing::when_requested`.
- **`cairn_git::SharedRepository::lock_files`**: every `*.lock` under the git directories.
- **The environment twin** reads `INHERITED`: `INHERITED_PINS` (R5.2's nine, each with a
  comment of its own) and `INHERITED_NEVER` (no `*_DATE`).

## Carried forward from phase 04 (owned by the phase named)

- **Phase 05**: `Commit`/`Amend` in `LocalWrite` (above); the `expect(dead_code)` on
  `LaneState::install` and `Request::CancelWrite` go with them; re-run C10/C11/C12's
  commit-dependent tests against `git commit` with a slow hook.
- **Phase 07**: ask writes through `local_writes::ask`; draw a write queued and its outcome
  (`LocalWrites::queued`, `last`); decide where `discard_*_consequence` is asked (a worker's
  call; the local lane orders it after the writes ahead of it).
- **Phase 09**: a Cancel the commit box draws is for the running commit (`CancelWrite` of a
  queued one does nothing, R4.3).
- **Phase 11**: the activity popover reads `WriteEnding`/`Done`; measure the open's lock
  listing (`SharedRepository::lock_files`, a walk of `refs/`) on the bench repository.
- **The user** (batched, progress.md's phase 04 entry): the second close during a write
  (design says end it as a cancel; built per PRD R4.9, the window closes and the write runs
  on); a prompt carries no owner when a fetch and a write ask at once.

## Carried forward from phase 03 (owned by the phase named)

- **Phase 04** (done: the lane runs every verb with its write's token; the consequences
  carried to phase 07): the lane calls the verbs above on its own thread; the verbs take the
  write's token; `discard_*_consequence` are reads the window asks for before a dialog.
- **Phase 05**: `hooks_path` re-export; `UnstageTo::Commit`/`Nothing` for amend; the root
  amend's `git rm --cached` refusal; C13's root-commit case.
- **Phase 07** (phase 03's QA item 4): `discard_files_consequence` accepts any path
  absent from the index as untracked, so an ignored file or a path inside a nested
  repository reaches `git clean -f`, which leaves it (the outcome now names it as kept).
  Phase 07 either refuses a path `git status` did not list before any prompt, or states in
  `docs/systems/staging.md` that the caller owns that, with a test of the `Absent` arm.
- **Phase 07**: the dialog's `Consequence` from `discard_files_consequence` (a nested
  repository, a submodule, a conflicted path and a staged-only path are refused by it
  before any dialog); the rename source row's whole-file unstage resets the source alone,
  both paths unstage the rename; an intent-to-add file's discard leaves it empty and says
  so (the user's decision 5).
- **Phase 08**: `discard_lines_consequence` takes the `Selection` by value; a mode-only
  selection where the diff has a mode change is allowed and named.
- **Phase 11 / C22**: the root `CLAUDE.md` still has to name the one file deletion made
  without `git` (R12.4) when it exists.
- **Phase 12 / the user**: items 4 and 7 batched in progress.md's phase 03 entry (the
  rest decided by the user on 2026-10-08).

## Carried forward from phase 02 (owned by the phase named)

- **Phase 03** (done: `DiscardLines.mode`, named in the prompt): `Consequence::DiscardLines` counts `Selection::len`, which counts
  lines only, so a selection holding the mode change renders no word for it (and a
  mode-only one "Discard 0 Lines"); this is phase 01's item 19, now concrete. Phase
  03's discard verb decides how a mode selected for discard is confirmed, with a test.
- **Phase 03** (done: refused as `WholeFileOnly`, left to `discard_files`): a whole selection of an untracked or intent-to-add file's lines
  discards as a `deleted file mode` patch, which `git apply` honours by deleting the
  file — but L8 routes deleting an untracked file to `git clean` under
  `DiscardFiles`. The verb (or phase 07/08's gesture) must route an all-lines
  discard of an added file to the file verb, or confirm it as a deletion.
- **Phase 03 / C22** (done): the root `CLAUDE.md` (repo map's `cairn-model` and `cairn-git`
  rows, and D1's list of reads) does not yet name `action_patch`, the inversions or
  `reads::staged_pairing`; `docs/design/engine.md` and `docs/systems/diff.md` do. Not
  edited by the phase agent (an instruction file); for the user or C22's update.
- **Phase 08**: drawing the mode row (R9.4) selects through `Selection::select_mode`.

From phase 02's QA (adjudicated 2026-10-08):

- **Phases 03 and 07** (QA item 8; the engine's half done, `a_rename_sources_row_unstages_its_lines_at_the_new_path`): a rename's SOURCE path is paired too
  (`reads::working_tree::names`): where the user's `status.renames` differs from
  `diff.renames`, status lists the deleted source as a row of its own while its staged
  diff is the rename, and an unstage built from it acts at the NEW path. The verb and the
  gesture decide which row offers what, and on which path, with a test.
- **Phase 03 / C22** (QA item 9; done): the root `CLAUDE.md` repo map and D1's list of reads
  must name `action_patch`, the inversions and `reads::staged_pairing` (the C22 item
  above); phase 03's QA checks it.
- **Phase 03, the destructive verbs** (QA carry list; each done — the patch from the confirmed diff, checked against the `Consequence`; no `--recount`/`--3way`/`--unidiff-zero`; the file verb's prompt says the file is deleted and can't be undone):
  - emit the patch from the exact `FileDiff` the user acknowledged, never from a re-read;
  - no `--recount`, `--3way` or `--unidiff-zero` on any apply;
  - a whole discard of an untracked file must say it cannot be recovered;
  - an all-lines discard of an added file goes to the file verb (the item above).

## Carried forward from phase 01's QA (owned by the phase named)

- **Phase 03** (QA item 18; done: the builders refuse, `NoPaths` and `NothingSelected`): an empty `DiscardFiles` renders "the changes in 0
  files? ." and a zero-line `DiscardLines` "discard 0 lines"; "never empty" is a
  doc comment, not a check. The builder that computes either refuses empty input,
  with a test — or the type is made non-empty then.
- **Phase 03** (QA item 19; done: `DiscardLines.mode`): no variant expresses a mode change selected for
  discard (R2.4); add it with the operation, with a test.
- **Phase 03** (QA item 25; done: the bytes on disk, hashed with no filter, re-checked): the working-tree blob in a `Consequence` is hashed
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

## The user's decisions on phase 01's QA (2026-10-08), applied

- **Item 6**: `DESTRUCTIVE_OPERATIONS` is never empty, and the guard asserts it.
  **Phase 03** (done) must replace the `describe_destructive` row with its real
  operations in the same change that deletes the placeholder, or the guard fails.
- **Item 13**: R10.6 amended — the amend prompt's recovery sentence is
  conditional on the reflog (`Consequence::Amend`'s `reflog: Reflog`). Phase 01
  defines the field and both renderings. **Phase 05** computes it in the engine
  (R6.4, amended): `Written` when git will write the entry — `core.logAllRefUpdates`
  `true` (the default with a working tree; `false` when bare) or `always`, or the
  ref's log file already existing, since git appends to an existing log (checked
  with git 2.56 in phase 01) — read as git reads the setting, with a test for each
  arm against real git, including a bare repository and a log that exists under
  `false`. Phase 10 should confirm that what Show Lost Commits walks (C20: the
  reflogs of `HEAD` and each local branch) is the entry this decides.
- **Item 34a**: R1.2 amended to Fork's wording, lines per modified path and bytes
  per untracked file (L8); no code change.

## Validation status

| Phase | Status |
| --- | --- |
| 01 seal | done — QA adjudicated, all confirmed fixes and the user's three decisions (items 6, 13, 34a) applied, full gate green |
| 02 patch engine | done — QA adjudicated, confirmed fixes applied, full gate green; items 10-11 batched for the user |
| 03 write verbs | done — QA adjudicated, fixes and the user's decisions applied, full gate green |
| 04 local lane | built, gate green, QA pending |
| 05 commit engine | not started |
| 06 render foundations | not started |
| 07 Local Changes actions | not started |
| 08 diff gesture | not started |
| 09 commit box | not started |
| 10 lost commits | not started |
| 11 activity and measured | not started |
| 12 QA | not started |
