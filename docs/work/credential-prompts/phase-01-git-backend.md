# Phase 01 — The git subprocess backend

```
STEP 0  Pre-flight: read docs/work/credential-prompts/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/credential-prompts before
        editing. This is phase 01, so first create feature/credential-prompts
        from main if it does not exist (and push it if a remote exists). Direct
        integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/,
        crates/cairn-guards/tests/invariants.rs,
        docs/prd/credential-prompts.md (requirement R1), and
        docs/design/cairn.md decision D1. Do not read the other planning docs
        directly.
STEP 2  Decide O1, then implement.

        DECIDE — O1: the minimum git version Cairn requires. Justify it by the
        features actually used, not by what is installed on the development
        machine (2.55.0). Record it in brainstorm.md under "Resolved".

        Deliverables:
        1. `cairn-git/src/ops/cli.rs`: a typed builder that constructs and runs a
           `git` invocation. This is the ONLY place in Cairn that spawns a
           process — the existing only_the_ops_module_mutates_a_repository guard
           pins that, and this is the first code it constrains. Confirm it fires
           by violating it deliberately, then reverting.
        2. An environment builder that constructs `git`'s environment
           EXPLICITLY — not the parent environment with overrides (L5). Every
           passed variable is a deliberate entry with a reason. It always
           includes GIT_TERMINAL_PROMPT=0 (L3); phase 02 adds the askpass
           variables.
        3. Output handling: prefer -z and porcelain v2 where git offers them;
           never parse human-facing output where a machine-readable form exists.
        4. Errors as cairn-git::Error variants naming what the caller must
           handle, carrying git's stderr for diagnosis. Never a bare exit code.
        5. Startup discovery: locate git, check the version against O1, fail
           loudly with a message naming the required version. Never degrade
           silently.
        6. THE GUARD for "every git invocation sets GIT_TERMINAL_PROMPT=0": the
           environment builder is the only construction path and always includes
           it. Land it in crates/cairn-guards/tests/invariants.rs with a matcher
           self-test, and add the invariant to CLAUDE.md in the same commit.

        Invariants in play: only ops/ spawns a process; every invariant gets its
        twin in the same change; no panic on a reachable path — a missing git
        binary is an expected condition, not an unwrap.

        Out of scope: the askpass helper (phase 02), any actual git operation
        beyond what the version check needs, push, and anything destructive. No
        operation in this phase takes Confirmed, because this phase performs no
        mutation — if you find yourself wanting one, you have left scope.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: run /qa over the phase diff with destructive-ops-reviewer and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to the qa-confirm
        agent (fresh), never this session inline; log dismissed findings with
        reasons in progress.md; fix confirmed findings in focused fixes;
        disputed findings go to the user.
STEP 4  Acceptance: PRD criteria B1 and B2 pass against their tests.
STEP 5  Update state.md (symbol table, O1 resolved) and progress.md. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit explicit
        paths (never git add -A), push the phase branch, and raise a PR using the
        repository template into feature/credential-prompts; never merge it. In
        explicitly declared packet mode only, commit directly onto integration
        with no per-phase PR. NEVER merge or PR to main.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the explicit-environment rule (L5)
breaks something that needs an inherited variable you cannot justify — passing
the parent environment through is a security decision, not an implementation
detail. Otherwise do not stop for permission.
```

## QA brief

- The environment builder is the phase's security surface. Enumerate in a test
  exactly which variables are passed; a test that asserts "contains
  GIT_TERMINAL_PROMPT" while the builder also leaks the parent environment passes
  and proves nothing.
- Confirm there is genuinely one construction path. A second constructor added
  "just for tests" is how the guard becomes decorative.
- The version check needs a test with a stubbed `git` on PATH, covering: absent,
  too old, and unparseable output. The third is the one people skip and the one
  that panics in the field.
- Check stderr actually reaches the error. A backend that discards git's
  diagnostics turns every failure into "git failed", which is the worst possible
  error message for a git client.
- destructive-ops-reviewer is dispatched here even though the phase mutates
  nothing: it is the reviewer that owns the mutation surface, and this phase
  builds the machinery every future mutation will use. Its judgement about the
  shape is worth more now than after ten operations are built on it.
