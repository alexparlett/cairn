# Phase 01 — The diff model and the patch emitter

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing;
        create and push feature/diff-engine from main first if it does not
        exist. Direct work on the integration branch requires an orchestrator
        prompt that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-model/,
        docs/prd/diff-engine.md (R1 and criterion C4) and
        docs/research/diff-engine/gix-diff-api.md sections 2 and 10 — the latter
        records what a unified patch must contain for `git apply --cached` to
        accept it, read out of git's own add-patch.c. Do not read the other
        planning docs directly.
STEP 2  Implement. L2 and L3 in brainstorm.md decided the shape; build to them
        rather than reopening them. Everything here is pure: no gix, no I/O, no
        clock, nothing beyond the crate's allowlist.

        Deliverables:
        1. The types of PRD R1.1, R1.2 and R1.8: a changed file, a file diff
           holding both versions' lines (bytes plus whether the line ended in a
           newline), the exact changed ranges, the display-only
           whitespace-ignoring ranges, the intra-line ranges, and the states a
           file that is not text answers with. Line identity is R1.3: a removed
           line by its old number, an added line by its new one. R1.8 is the
           commit's details, which the Commit tab needs and CommitSummary does
           not carry: the committer beside the author, both timestamps with their
           offsets, the whole message, and the parents.
        2. The projections of R1.4 and R1.5: hunks at a context of N lines or the
           whole file, unified rows, side-by-side rows with filler on the shorter
           side. Addressable, not materialised: a view asks for the row count and
           for row i. Hunks merge when they are within twice the context, which
           is git's rule.
        3. The patch emitter of R1.6 and R1.7, plus an in-memory reference
           applier for the tests — written from the patch format, NOT by reusing
           the emitter's own code, or C2 in phase 02 proves nothing. Three lines
           of context always. The whitespace-ignoring ranges must be unreachable
           from the emitter by construction.

        Invariants in play: cairn-model's dependency allowlist (nothing new may
        follow the diff types in); a change to cairn-model needs its test in the
        same commit; no unsafe; no unwrap, expect, todo! or dbg! in shipping
        code.

        Out of scope: any engine query (phase 02), anything that reads a
        repository, any component, the worker.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with test-coverage-auditor spawned
        fresh, plus the qa-checklist.md items for this phase and the QA brief
        below. Adjudication goes to the qa-confirm agent (fresh), never this
        session inline; log dismissed findings with reasons in progress.md; fix
        confirmed findings in focused fixes; disputed findings go to the user.
STEP 4  Acceptance: PRD criterion C4 passes against its tests, and the emitter's
        own unit tests cover the cases C2 will replay through real git.
STEP 5  Update state.md (the symbol table, with each type's one-line contract)
        and progress.md. Create docs/systems/diff.md describing the model as
        built, add its line to docs/systems/README.md's list, and update the
        cairn-model row of the root CLAUDE.md repo map. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the phase branch, and raise a
        pull request into feature/diff-engine; never merge it. NEVER merge or
        raise a pull request to main.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the model cannot express something the
patch format needs without a new dependency, or if presentation-independent line
identity (R1.3) turns out not to hold for a case the format allows — both would
reopen a decision the user already took. Otherwise do not stop for permission.
```

## QA brief

This phase's defects are invisible until packet 5 tries to stage a line, which is
exactly why the tests here have to decide something.

- The reference applier must be independent of the emitter. If both were written
  from the same mental model, C2 will agree with itself forever.
- Walk the awkward cases by hand and confirm a test exists for each: a missing
  final newline on the old side, on the new side, on both; a hunk at line 1; a
  hunk at the last line; adjacent hunks that merge at context 3 and separate at
  context 1; an empty file; a one-line file.
- Check the header arithmetic for empty ranges. A pure insertion writes the line
  before it as its old start with a zero count, and a new file writes `-0,0`.
  Getting this wrong produces a patch that applies at the wrong offset, not one
  that fails.
- Confirm the rule for a partial deletion: when some removed lines are not
  selected, the result is a modification of the file, not a deletion of it.
- Confirm R1.5 is real: ask a projection for one row of a diff with a hundred
  thousand changed lines and show that it did not build the rest. A projection
  that returns a Vec has already lost.
- Read the emitter's signature and say out loud how a caller could reach the
  whitespace-ignoring ranges. If the answer is "by being careful", it is not
  R1.7.
