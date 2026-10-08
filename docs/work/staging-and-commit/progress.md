# Progress — staging-and-commit

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

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
