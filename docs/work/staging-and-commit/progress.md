# Progress — staging-and-commit

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

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
