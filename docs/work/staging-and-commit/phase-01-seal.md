# Phase 01 — The seal, bound to what it costs

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. feature/staging-and-commit does not exist yet: create it from
        main and push it first. Direct integration work requires an
        orchestrator prompt that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-model/src/confirm.rs,
        crates/cairn-git/src/ops/ (mod.rs: Performed, Invalidated, the
        compile_fail doctests; authority.rs), crates/cairn-git/src/process/
        (the bounded-output helpers: collect, finish_within, and who calls
        them), crates/cairn-guards/tests/invariants.rs
        (destructive_operations_are_sealed_behind_the_confirmation_token and its
        neighbours' matcher/self-test pattern), crates/cairn-guards/src/lib.rs,
        docs/prd/staging-and-commit.md (R1, R4.8, C1) and
        docs/research/staging-and-commit/write-path-as-built.md (sections 2 and
        11). Do not read the other planning docs directly.
STEP 2  Implement.
        1. cairn-model: Confirmed loses Clone (and gains no Copy); it carries a
           Consequence and the prompt rendered from it (R1.2). The Consequence
           is plain data with a variant per destructive operation of R1.5 —
           discard lines, discard files, delete untracked files, amend, remove a
           lock — each naming what it destroys and the ids it was computed
           against. The prompt renderer lives beside it, so a prompt cannot be
           typed apart from its Consequence. compile_fail doctests for .clone()
           and for a Confirmed built without a Consequence, with a passing twin.
        2. cairn-git: Performed::destructive records the rendered prompt
           (R1.6); the bounded-output helpers become callable on read
           invocations only, at the type (R4.8, #45 item 2), with a compile_fail
           pin.
        3. cairn-guards: the destructive-operation roster (every rostered name
           in crates/cairn-git/src/ops/ takes Confirmed by value; every ops/
           function taking one is rostered; empty of real operations until
           phase 03 adds them, so the guard proves itself on its self-test and
           fails on a roster row with no function) and the confirmation-surface
           roster (Confirmed's constructor named in no production file outside
           cairn-model but the files it lists; empty here, so nothing may name
           it yet; phase 06 adds the dialog component and phase 09 the commit
           box, and a listed file that does not name the constructor fails). Strengthen
           destructive_operations_are_sealed_behind_the_confirmation_token so
           ops/mod.rs alone no longer satisfies it. Each with a nonzero-files
           assertion and a matcher self-test.
        4. The root CLAUDE.md Confirmed invariant: the rosters, their twins and
           the residuals they cannot express (a Consequence computed wrongly, a
           prompt that is rendered but misleading — destructive-ops-reviewer's).

        Invariants in play: cairn-model and cairn-guards changes carry tests in
        the same commit; enforcement at the strongest tier (the type first);
        every invariant has its twin; no unwrap/expect.

        Out of scope: any verb (03), the dialog (06), the commit box (09).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 01, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C1 (its compile-time and guard halves; the call-site halves
        complete when 03, 06 and 09 land on the rosters); C22 (the two rosters'
        twins named in the root CLAUDE.md).
STEP 5  Update state.md (new interfaces: Confirmed's shape, Consequence, the
        rosters) and progress.md; docs/systems/git-processes.md where it
        describes the seal and the bounded-output helpers. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the constructor roster cannot be
expressed without a dependency or an unsafe-adjacent trick; if a roster must
start non-empty in a way that weakens a guard; or if a Consequence variant needs
data the engine cannot read. Otherwise do not stop for permission.
```

## QA brief

The risk is a seal that looks stronger and is not.

- Try to build a `Confirmed` from `cairn-app`, from a test helper and through a
  `From`/`Default`/`Deserialize` route: each must fail to compile or fail the
  guard.
- Try to forge a matching `Consequence` by hand in a render crate: the guard
  must see the constructor named outside the roster.
- The destructive-operation roster must fail on an `ops/` function that takes
  `&Confirmed` or `Option<Confirmed>` — only by value counts.
- The prompt renderer: does any variant render a count or a path the
  `Consequence` does not carry?
