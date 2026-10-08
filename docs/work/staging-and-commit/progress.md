# Progress — staging-and-commit

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

## 2026-10-08 — phase 03, C21's margin decided

The user chose C21's margin on 2026-10-08, relayed by the coordinator: a flat 50 ms per
verb, added to git's own time plus one status read (option A of the phase 03 report). From
`measured-baseline.md`'s highest sums the bars are about 93 ms to stage (42.9 + 50), 93 ms
to unstage (42.8 + 50), 76 ms to discard (25.8 + 50) and 87 ms to commit (37.4 + 50).
PRD C21 and R13.2 amended. Batched item 1 is closed; items 2-7 stay batched.
`measured-baseline.md` is evidence and not retro-edited (docs/CLAUDE.md); its section 4
already set out the 50 ms option, and the PRD records the decision.

## 2026-10-08 — phase 03, the write verbs (packet mode; stopped for C21's margin)

Built on `feature/staging-and-commit`; QA pending (the coordinator dispatches the fresh
reviewers). Stopped at R13.2's stopping rule: git's baseline exists
(`docs/research/staging-and-commit/measured-baseline.md`), and C21's margin is the user's.

What shipped (`docs/systems/staging.md`):

- `ops::stage_lines`, `unstage_lines` (`git apply --cached --whitespace=nowarn -`),
  `discard_lines` (`git apply --whitespace=nowarn -`), `stage_files` (`git add`),
  `unstage_files` with `UnstageTo::{Head, Commit(id), Nothing}` (`git reset -q`, `git
  reset -q <id>`, `git rm --cached -q`), `discard_files` (`git restore --worktree`, then
  `git clean -f --` in batches) — every one a write with `--literal-pathspecs` before the
  verb, many paths in a NUL pathspec file on stdin, the locks listed before and after on
  `Performed::locks` (R3.8), taking `Option<&AskpassToken>` for phase 04.
- The stale check (R3.7): the index read fresh with gix (`IndexNow`, once per call), the
  working tree's git form by `reads::hash_object` (`git hash-object --path=<p> -- <p>`,
  never `-w`) and its bytes hashed in process; `reads::hooks_path` (`git rev-parse
  --git-path hooks`) landed for phase 05, module-private until then.
- `discard_lines_consequence` and `discard_files_consequence` build each `Consequence`;
  `discard_lines` and `discard_files` take `Confirmed` by value, re-check before the first
  write and replaced `describe_destructive` on the roster in the same change.
- `Consequence::DiscardLines` gained `on_disk` (the bytes' hash) and `mode` (named in the
  prompt with both modes); `FileLoss`'s working-tree id is the bytes' hash.
- New errors: `Error::Refused { path, why: Refusal }`, `NoPaths`, `ChangedSinceRead`,
  `ChangedSinceConfirmed`, `ReadIndex`, `ReadWorkingTree`.
- C3 now runs through the verbs; `tests/diff/write_verbs.rs` holds C2's discard halves,
  C5, C6, C8's engine half and C9's effect; the recording `git` (`ops/recording_stub.rs`)
  holds C9's argv. `scripts/git-floor.sh` runs `ops::` too, floors 126/158/16.

Measurements and decisions:

- **`git clean`'s argv bound** (R3.5): `ops::CLEAN_ARGUMENT_BYTES`, 64 KiB per invocation,
  each path counted as its bytes, its NUL and its 8-byte pointer. Measured on this host:
  `getconf ARG_MAX` 2,097,152; `git --literal-pathspecs clean -n -- <names>` took 1,536 KiB
  of 100-byte names and the kernel refused 1,900 KiB (`E2BIG`, the pointers and the
  environment counted); one argument of 128 KiB is refused (`MAX_ARG_STRLEN`). 64 KiB is
  half Linux's smallest `ARG_MAX` (32 pages), a sixteenth of macOS's 1 MiB. A list past it
  runs as several `git clean`s after one re-check
  (`a_long_list_is_deleted_in_batches_after_one_recheck`: 700 paths, 2 invocations).
- **`apply.ignoreWhitespace`**: measured on 2.30.9, 2.32.7 and 2.56.0 over 400
  selections of a whitespace-heavy diff, staged with and without `-c
  apply.ignoreWhitespace=change` (and `=false`): every staged blob identical, every apply
  succeeded. The setting only lets a patch land on context that moved by whitespace (a
  stale index: exit 0 with it, exit 1 without), which the stale check refuses first. Not
  pinned; C5 holds the case; PRD's filed list amended.
- **`hash-object` reproduces the diff's git form** of a CRLF file under `core.autocrlf`
  and of a rot13-filtered file, with `--path` and with several paths at once, on 2.30.9,
  2.32.7 and 2.56.0, and writes nothing — so that stopping rule did not fire. A symlink
  is hashed in process (as its target), since `hash-object` reads the file it points to.
- **No verb behaved differently on 2.30.9** in what Cairn passes: `--end-of-options`
  before a commit after `--pathspec-from-file` is refused by 2.30's `reset` ("must come
  before non-option arguments") where 2.56 takes it, so the commit is passed as its hex
  id with no `--end-of-options` (an id cannot be an option); every verb as built passes
  the same tests on 2.30.9, 2.32.7 and 2.56.0.
- **Hashing for the re-check** (phase 01's QA item 25): every discard compares the
  file's bytes hashed with no filter, so a line ending changed after the confirmation
  refuses; a discard of lines also compares git's form (R3.7). PRD R3.9 amended — for the
  user's review.
- **The mode in a discard of lines** (QA item 19): `Consequence::DiscardLines.mode`, the
  prompt "discard 2 lines and the mode change (100644 to 100755)", the label "Discard 2
  Lines and Mode Change" / "Discard Mode Change"; a selection of the mode alone with no
  mode change is refused as nothing selected (QA item 18's zero-line prompt).
- **Every line of a new file discarded** is refused as `WholeFileOnly` and left to
  `discard_files`, whose prompt says "1 untracked file deleted (N bytes). You can't undo
  this action." (phase 02's carry-forward).
- **The rename source row** (phase 02's QA item 8): its staged diff is the rename, and
  its lines unstage at the new path, the source's entry untouched; its own whole-file
  unstage resets the source alone; both paths named unstage the rename whole.
- **R3.8 as built**: a success carries the locks before and after; a failure carries
  git's own report of those present after it (`GitFailed.present_locks`,
  `GitCancelled`/`GitUnwatched`'s `stranded_locks`), which names any lock that was there
  before and still is.
- The bench repository's `.git` was unchanged by everything: `find .git -newer <marker>`
  listed nothing before the clone, after it and after every baseline run.

Batched for the user's review — not decided:

1. **C21's margin** (the stopping rule): proposed below in the phase's report.
2. **R3.9's amendment**: a discard of files re-checks the bytes on disk hashed in process,
   not `git hash-object` (which cannot hash a symlink as git stores it and would not see a
   line ending changed under `core.autocrlf`).
3. **C2's "an untracked file added to a deletion's directory"**: Cairn never deletes a
   directory (status lists files one per file; the one directory it lists whole, a nested
   repository, is refused), so such a file is never taken and the deletion of the
   confirmed files proceeds (`a_file_added_beside_a_confirmed_deletion_is_never_taken`) —
   not refused, as C2's wording has it. A confirmed file replaced by a directory is refused.
4. **`apply.ignoreWhitespace`**: not pinned, as measured; pinning `-c
   apply.ignoreWhitespace=false` anyway would close the window between the stale check
   and `git apply` for a whitespace-only move.
5. **An intent-to-add file's discard of files** runs `git restore --worktree`, which
   leaves the file EMPTY (git's own behaviour, verified on 2.30.9 and 2.56.0) and the
   intent-to-add entry in place; the prompt counts its lines as modified. Deleting it
   instead (as an untracked file) would be two writes.
6. **A mode change in a discard of whole files** is restored without the prompt naming
   it (`FileLoss::Modified` counts lines only).
7. **A discard of files reads each tracked file's unstaged diff to count its lines** —
   exact, but one read per file selected.

## 2026-10-08 — phase 02 QA, adjudicated and fixed

Four fresh reviewers, adjudicated by a fresh `qa-confirm`. Confirmed findings fixed in
focused commits, behaviour changes test-first (each new test seen to fail before its fix):

- 1: `action_patch` returns the empty patch for an empty selection before anything else;
  an empty untracked file's stage, unstage and discard, and a `ModeChangeOnly` type
  change, wrote headers or `deleted file mode` before
  (`a_selection_of_nothing_makes_no_patch_by_any_action`).
- 2: the whole-index pairing read runs only for a path absent from `HEAD` or the index
  (`only_a_path_that_can_be_in_a_pair_asks_the_whole_index`, which counts the reads in
  the command log; it failed with the read running for a copy's modified source).
- 3: `scripts/git-floor.sh`'s floors raised to one under the runs' counts (84 and 134).
- 4: C3 decides a deletion from the case and checks the patch's header against it; an
  unborn branch's addition unstaged whole leaves no entry.
- 5: the partial-clone residual of the staged pairing stated in `docs/systems/diff.md` and
  pinned on both arms (`in_a_partial_clone_a_staged_pairing_fails_rather_than_fetching`).
- 6: the write-nothing pin asks a staged `git mv` with an edit, so both pairing reads
  run under it, and requires that they did.
- 7: C3's awkward names gain CR, DEL, BEL, BS, VT and FF.
- Optional, done: C3 gains a staged copy under `diff.renames=copies`, unstaged as content
  at the copy's path with real git (`lines_of_a_staged_copy_unstage_as_content_at_its_path`).

Carried forward to phases 03 and 07 in state.md: 8, 9 and the destructive carry list.

Batched for the user's review at the end of the packet — not decided:

- 10: PRD R2.1's text against the insertion-then-removal refinement of
  `TextDiff::inverted` (no reviewer found the refinement wrong).
- 11: whether C21 gains a selection-to-diff row for a staged rename (the cost of the
  pairing read under `diff.renameLimit=0`).

Dismissed, with the adjudicator's reasons:

- 12 (the `ContentReadsDisagree` retry is unbounded): it is not —
  `worker/diff_lane.rs`'s `READ_ATTEMPTS` is 3, and `asking_again` stops at 3 and on
  cancel.
- 13 (C4 derives its expected mode from `drawn.file.inverted()`): the `assert_ne`
  beside it is decisive whatever the expected value says; style only.

## 2026-10-08 — phase 02, the patch engine (packet mode)

Built on `feature/staging-and-commit`; full gate green (git-floor included); QA
pending — the coordinator dispatches the fresh reviewers. What shipped:
`TextDiff::inverted`, `Selection::inverted`, `ChangedFile::inverted` (R2.1);
`Selection`'s mode item (R2.4); C-quoted path lines (R2.5); `action_patch`, what
stage, unstage and discard emit, refusing part of a whole-file-only change (R2.2,
R2.3); the staged side of the working-tree query pairing renames and copies as `git
diff --cached` does (R2.6), unstaging lines of a rename as content at its new path.
C3, C4 and C7 pass against real git on 2.56.0, 2.30.9 and 2.32.7
(`crates/cairn-git/tests/diff/staging.rs`, `staged_renames.rs`); the intent-to-add
question is settled (state.md). No stopping rule fired: every floor git applied
every case as the host's did; the staged pairing is query plumbing (`diff-index`),
no porcelain read; no case needed `-R`.

Decisions taken in the phase (none is a stopping rule; the first is batched for the
user's review, as a refinement of R2.1's wording):

- **An inverted replacement is two changes, its insertion then its removal.** R2.1
  says "each change's two spans swapped". Swapped as one change, the forward rule
  leaves what stays of a partial selection inside one replacement AHEAD of what it
  restores (`B b`), where git's mirrored patch applied with `-R` — E1b, and what git's
  own `reset -p`/`checkout -p` edits do — leaves `b B`; the two oracles C3 names then
  disagree on every mixed selection. With the split, the inversion, the reference
  applier, the mirrored rule as the model states it and real `git apply -R` all agree
  on every case and seed; the unsplit inversion fails both the model's property test
  and C3 (checked by mutation). Visible cost: a whole inverted replacement is written
  `+old` before `-new`, which git applies identically. Still one forward rule, still
  no `-R`.
- **The C3 mirrored oracle is grounded in real git**: written in the test from the
  forward diff (it inverts nothing), applied in reverse both by real `git apply -R`
  on a scratch file outside any repository and by the reference applier, which must
  agree; the reference applier then reads Cairn's own patch, and git applies it.
- **The staged pairing reads the whole index once** (`git diff-index --cached --raw
  -z -M|-C -l<n> --diff-filter=RC --ita-invisible-in-index`), keeping only the
  records naming the path as they stream, then reads the pair's lines across both
  paths with the same detection; a record that moved between the two reads is
  `ContentReadsDisagree`. `--ita-invisible-in-index` was found needed: plumbing
  otherwise pairs a deleted empty file with an intent-to-add one, which `git diff
  --cached` never shows (C7's fixture holds that case; dropping the flag fails C7).
  `--diff-filter=RC` also drops an unmerged entry's `U` record, which the raw parser
  would refuse.
- **A staged rename's source path answers the rename record too**, since that
  record is the only thing `git diff --cached` shows for it; a copy's source keeps
  its own record.
- **The mode item is selected by nothing but `select_mode`**: whole-file round
  trips (`tests/diff/patches.rs`, diff-engine's C1-C3) now select it explicitly.
- **Path lines carry git's trailing tab** after a `---`/`+++` label holding a
  space (`diff.c`), so the emitted headers equal `git diff`'s byte for byte, which
  C3 compares for every awkward name.
- The root `CLAUDE.md` was not edited (state.md, carried forward).

## 2026-10-08 — phase 01, the user's three decisions applied

The user decided the three items the adjudication left with them (relayed by the
packet coordinator):

- **Item 6, the empty roster**: assert it non-empty. The guard now asserts
  `!DESTRUCTIVE_OPERATIONS.is_empty()`, its doc comment says so, and emptying the
  roster was seen to fail the guard. The placeholder row satisfies it until phase
  03.
- **Item 13, the amend prompt**: conditional wording. `Consequence::Amend` carries
  `reflog: Reflog`; the prompt says "The old commit stays in Show Lost Commits."
  only for `Reflog::Written`, and "The old commit can't be recovered afterwards:
  this repository keeps no reflog." for `NotWritten`, with full-literal tests for
  both arms. What decides it was checked against git 2.56: an amend writes the entry
  under `core.logAllRefUpdates=true` (the non-bare default), writes none under
  `false` set from the start, and still appends under `false` set after the logs
  exist; a bare repository leaves the setting unset, and git's default there is
  `false`. R10.6 and R6.4 are amended with a dated note; the engine's computation
  is phase 05's (state.md).
- **Item 34a, R1.2's bytes**: R1.2 amended to Fork's wording — lines per modified
  path and bytes per untracked file (L8) — with a dated note; no code change.

## 2026-10-08 — phase 01 QA, adjudicated and fixed

Eight fresh reviewers, adjudicated by a fresh `qa-confirm`. Confirmed findings
fixed in focused commits (`fix(model)`, `fix(git)`, `fix(guards)`, `docs(docs)`);
each guard fix was mutation-tested — the hole reopened in the real tree, the guard
seen to fail, the tree restored:

- 1: the token-file check reads every function (`function_signatures`), every
  impl header with attributes stepped over (`impl_headers`), refuses `const`,
  `static`, `macro_rules!` and `mod` items, and requires one `by_user` and one
  literal. The Secret guard's same idiom is filed as #85.
- 2: `CONFIRMED_HOLDERS` (empty) refuses an engine type that keeps a token in a field.
- 3: `renames_type` starts a statement after `}` and `{` as well as `;`.
- 4: the ceiling `drive` takes is `Kind::Ceiling`, `Infallible` for a write; a
  generic helper passing `Some(1)` no longer compiles (checked).
- 5: the R4.8 check reads code (comments out), refuses `cfg` or `ignore` on the
  pin, with a self-test (`the_bounded_output_helper_check_catches_the_shapes_it_claims`).
- 7: `RemoveLock` carries `modified`, `read_at`, `bytes`, `device`, `inode`.
- 8: `DiscardLines` carries the `Selection`; the "every target from
  `confirmed.consequence()`" contract is a residual in `CLAUDE.md` and in
  `destructive-ops-reviewer`'s check 4.
- 9: `consequence.rs`, `confirm.rs` and every `CONFIRMATION_SURFACES` file route to
  `destructive-ops-reviewer` (its scope gate, `docs/qa-gate.md`, checklist item 7).
- 10: fixture strings no longer trip `.claude/hooks/qa-stop.sh`.
- 11: state.md's phase 01 row corrected.
- 12: paths and subjects escaped as git C-quotes a path, plus line separators and
  bidirectional controls, with tests.
- 14: the surface scan must have read `cairn-ui/src` and `cairn-app/src`.
- 15: each roster self-test fixture breaks one rule and asserts that rule's
  message; disabling the by-value check fails it (checked).
- 16: `consequence.rs` is held to no `Default`/`From`/`TryFrom`/`FromStr`/
  `Deserialize`/`Decode` and one impl block.
- 17, 31, 32: residuals stated in `CLAUDE.md` and the reviewers' definitions.
- 20: the prompt tests pin literal text per variant, sum several untracked files'
  sizes, use an asymmetric selection, and cover singular hour/day, GiB/TiB, the
  label's fallback and a lock from the future.
- 24: a file deleted in the working tree is named "deleted file restored".
- 28: `.github/workflows/enforcement-review.yml` and the mockup reworded.
- 29: `CONFIRMED_RECORD` is keyed on `Performed`'s inherent impl, not the name.
- 39: `Deserialize` and `Decode` have negative self-test cases.

Carried forward to the owning phases in state.md: 18 and 19 and 25 (phase 03), 21
(phase 09), 23 (phase 11). With the user: 6, 13, 34a.

Dismissed, with the adjudicator's reasons:

- 22 (an amend re-check needs the publication): the shape already carries
  `published`; comparing it is phase 09's re-check.
- 26 (a refused operation leaves no record): R1.6 binds `Performed`, which records
  completed operations; the refusal outcome is R1.4's, phase 03's.
- 27 (added and removed lines both counted as discarded): matches Fork.
- 30 (`<Consequence>::Variant { .. }` escapes `names_a_path_into`): a qualified
  path in a struct expression is unstable, and every variant is a struct variant.
- 33 (unchecked addition could overflow): the counts are bounded by an in-memory
  diff, and only the engine builds the values.
- 34b (merging discard files and delete untracked was unapproved): R1.5 lists them
  as one requirement.
- 35 (the copy doctest cannot flip): harmless; it fails for the reason it names.
- 36 (widening the helpers to every kind is not caught): the check requires the
  declaration inside `impl Invocation<Read>`.
- 37 (a `&mut self` helper on a write escapes the pin): the declared-once check
  catches any second declaration.
- 38 (a scratch copy reported `describe_destructive` off the roster): not
  reproducible on a fresh copy or with a fresh target directory.
- 24b (a collapsed untracked directory is not modelled): the status read expands
  it with `--untracked-files=all`.

## 2026-10-08 — phase 01, the seal (packet mode)

Built on `feature/staging-and-commit`. `Confirmed` lost `Clone`, carries an
engine-computed `Consequence` and the prompt rendered from it, and has one
constructor that takes the `Consequence` alone; `Performed::destructive` spends the
token by value and records its prompt (R1.6); the bounded-output helpers moved to
`impl Invocation<Read>` (R4.8, #45 item 2). The guard was rewritten around two
rosters and the forging and route checks; the root `CLAUDE.md` invariant names
them and their residuals (C22).

Decisions taken in the phase (none is a stopping rule):

- **One `DiscardFiles` variant for discarding files and deleting untracked files**,
  not two: Fork confirms a mixed selection in one dialog with one prompt (L8), and
  one prompt must be one `Confirmed`. Four variants, not the phase doc's five.
- **R4.8's "compile_fail pin" is an in-crate type-check pin**, not a doctest: a
  doctest cannot name the crate-private `Invocation`, and stable Rust has no
  in-crate `compile_fail`. `the_bounded_output_helpers_exist_on_a_read_alone`
  (`process/runner.rs`) is a function that compiles only while a write's calls of
  the helpers' names resolve to a fallback trait (answering `Absent`) and a read's
  to the real helpers; checked by hand that widening the impl to every kind fails
  to compile (E0308 on both write calls). The runner guard requires the pin and
  the helpers' place.
- **The placeholder `describe_destructive` is the destructive roster's one row**:
  the rule "every function naming `Confirmed` is rostered" covers it, so the roster
  is not empty, and the row proves the by-value check on real code until phase 03
  replaces it.
- **The `Consequence` cannot be spelled outside the engine and the model** in
  production code (a forged one would otherwise reach a surface's `by_user`); the
  render crates hold one and ask it for its words. Phase 09 adds a method for the
  amend dialog's decision rather than matching variants.
- **Test code may build tokens** (test modules, `#[cfg(test)]` module files,
  `tests/`): it cannot ship, and phase 03's operations need tokens in their tests.
  A helper under any other `cfg`, or named for tests in production code, fails.
- `Error::GitOutputTooLarge` lost `stranded_locks`, and
  `a_write_the_runner_ends_lists_the_locks_present` its ceiling half: no write can
  cross a ceiling now.
- Each of the six `compile_fail` doctests in `confirm.rs` was checked by hand to
  fail for its own reason (E0599 `clone`, E0382 move, E0308 text, E0451 private
  fields, E0599 `default`, E0277 `From<String>`), the scaffold compiling.

## 2026-10-07 — planned

Planned with `/feature-plan` on `feature/plan-staging-and-commit`. Seven evidence
records under `docs/research/staging-and-commit/`; the user locked L1-L21 over five
rounds, asking for mechanism detail on the patch engine, which became
`patch-mechanics-spike.md` (git 2.30.9 and 2.56.0 agree on every case). The brief
was split: stash and `.gitignore` go to a new packet 5b, `stash-and-ignore`.
Program O3 closed: no backup before a discard, as Fork.

Recon found, and the plan answers: `Confirmed` derives `Clone` and its
constructor is callable from any crate (R1); the runner already feeds stdin, so
no runner change is needed for `apply` or `commit -F -`; a status begun before a
write is drawn after it today (R4.4); a focused text field likely hides the
window's chords and held modifiers today (R7.1, C15 fails first).

Side effect, reported to the user at the time: the git-verbs recon agent's GPG
experiment reached the user's real `gpg-agent` through its socket, wrote two test
private keys into `~/.gnupg/private-keys-v1.d/` and restarted the agent twice; the
agent was refused removing them, and the user was given the commands. No further
GPG experiments were run.
