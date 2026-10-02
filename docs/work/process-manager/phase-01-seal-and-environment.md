# Phase 01 — The seal and the environment

```
STEP 0  Pre-flight: read docs/work/process-manager/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/process-manager before editing.
        This is phase 01: first create feature/process-manager from main in its
        own worktree and push it, if it does not exist. Direct integration work
        requires an orchestrator prompt that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/ops/,
        crates/cairn-git/src/lib.rs, crates/cairn-guards/src/lib.rs and
        crates/cairn-guards/tests/invariants.rs (the process, environment,
        mutation and credential twins and their matcher self-tests),
        docs/prd/process-manager.md (R1, R2, G1-G5),
        docs/design/processes.md ("One place builds a process", "The
        environment"), and docs/research/process-manager/
        runner-and-worker-as-built.md sections 1, 3 and 6. Do not read the other
        planning docs directly.
STEP 2  Implement. L2 and L9 decided the placement, the token and the profiles;
        build to them.

        Deliverables:
        1. The module (R1.1): crates/cairn-git/src/process/, crate-private,
           holding GitBinary, GitEnvironment and Askpass, moved from ops/
           together with the existing runner (cli.rs) and its stub-git tests,
           unchanged in behaviour. Re-export from ops/ only what fetch and the
           public surface still need; nothing outside cairn-git gains access.
           Create crates/cairn-git/src/reads/ as an empty, documented module, so
           the guard can name it as a caller.
        2. The seal (R1.2): an invocation is built as a Read or a Write. A Write
           requires a WriteAuthority, defined in ops/ with a private field and a
           constructor only ops/ can call. Fetch is built as a Write. The probe
           is built as a Read. Pin it with compile_fail doctests: a Write
           without a WriteAuthority, and a WriteAuthority constructed outside
           ops/. These doctests decide only the public surface: a doctest
           compiles as another crate, so it fails on privacy whatever the seal
           does. Whether reads/ can construct a WriteAuthority is decided by the
           guard in deliverable 4 (G1, G2).
        3. The profiles (R2): ALWAYS gains ("GIT_EDITOR", "false") and
           ("GIT_SEQUENCE_EDITOR", "false"). A Read adds GIT_OPTIONAL_LOCKS=0 and
           can carry no askpass token, by construction rather than by
           convention. The builder's tests spell out both variable sets in full
           (G3). Integration tests against real git: a read of `git status` on a
           dirty fixture with a stale index leaves the index byte-identical
           (G4), and `git commit` with no message fails promptly (G5).
        4. The guards (R1.3, R1.4), rewritten in the same commit as the move:
           - every_git_invocation_disables_the_terminal_prompt follows
             environment.rs to process/ and keeps every check it has, plus the
             two new ALWAYS pins;
           - a twin that no product file outside process/ builds, spawns, waits
             on or reads a process (spawn, output, status, wait, Stdio, Child)
             or calls a GitBinary or GitEnvironment method that yields one;
           - a twin that the runner's types (GitCommand, the invocation and its
             builders) are named only in process/, ops/ and reads/, and the
             WriteAuthority constructor only in ops/ — the in-crate half of the
             seal, which a doctest cannot decide;
           - only_the_ops_module_mutates_a_repository extended to gitoxide's
             mutation APIs outside ops/ — enumerate them from the vendored gix
             0.87.1 source, never from memory, and record the list beside the
             matcher.
           Each guard keeps a nonzero-files assertion and gets matcher
           self-tests for every shape it claims (G2). Update the root CLAUDE.md:
           the repo-map row for cairn-git; the "Only cairn-git/src/ops/ mutates"
           and environment invariants, with their new twins and residuals; and
           the D1 architecture bullet, so that it no longer says reads never
           spawn a process. Document in reads/ that a read runs plumbing or
           status only — GIT_OPTIONAL_LOCKS=0 covers nothing else.

        Invariants in play: the environment invariant (its twin moves files —
        it must never pass on an empty file set); only ops/ mutates; no unsafe;
        no panic in shipping code; a change to ops/ or cairn-guards carries its
        test in the same commit.

        Out of scope: any change to how the runner reads pipes, signals or
        reaps (phase 02); the probe's and fetch's move to a new runner (phase
        03); anything in cairn-app beyond what the move forces to compile.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with gate-integrity-reviewer,
        destructive-ops-reviewer and test-coverage-auditor spawned fresh, plus
        the qa-checklist.md items for this phase and the QA brief below.
        Adjudication goes to the qa-confirm agent (fresh), never this session
        inline. Log dismissed findings with reasons in progress.md. Fix
        confirmed findings in focused fixes. Disputed findings go to the user.
STEP 4  Acceptance: G1, G2, G3, G4 and G5 pass against their pinned tests.
STEP 5  Update state.md (new symbols, validation row) and progress.md. Create
        docs/systems/git-processes.md, describing the seal, the module layout
        and the environment as built, and point docs/systems/credentials.md at
        it where it describes the environment's home; add it to the root
        CLAUDE.md Pointers. Comment on issue #18 that GIT_EDITOR and
        GIT_SEQUENCE_EDITOR are now pinned to false (its editor half), and
        close it if nothing else remains. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/process-manager; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a gitoxide mutation API cannot be told
apart from a read by name (the twin would then need a different shape); if the
token cannot seal writes without exposing a constructor outside ops/; or if
GIT_EDITOR=false breaks an existing fetch test. Otherwise do not stop for
permission.
```

## QA brief

This phase rewrites the guards that hold two of the project's invariants. The
failure mode is a guard that still passes because it checks less than before.

- For each rewritten twin, name the mutation that turned it red before this
  phase, and confirm that mutation still turns it red after. Deleting the old
  `ops/environment.rs` path from the guard must not silently scan zero files.
- Try each of the four unguarded routes from `runner-and-worker-as-built.md`
  section 3 in a scratch module outside `ops/`, and confirm each fails the gate:
  `git.command().run()`, `.stream()` with its kill, `GitCommand::new`, and
  `git.environment().command(..).output()`. Also try a `reads/` file that
  constructs a `WriteAuthority`.
- The compile_fail doctests must fail for the reason they claim. A doctest that
  fails to compile because of a typo passes as "compile_fail".
- Read the gitoxide mutation list against the vendored source. A missing method
  is a hole that looks like coverage.
- Check that the probe and fetch run with the variable set the tests spell out:
  stub git prints what it was given.
