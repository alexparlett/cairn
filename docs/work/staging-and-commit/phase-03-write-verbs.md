# Phase 03 — The write verbs, the stale check, and git's baseline

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-02 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/ops/
        (fetch.rs as the one verb's precedent, authority.rs, stranded_locks),
        crates/cairn-git/src/process/ (write_invocation, stdin, lock reporting),
        crates/cairn-git/src/reads/ (mod.rs and one precedent read),
        crates/cairn-guards/tests/invariants.rs (the porcelain-reads and process
        twins, phase 01's rosters), docs/prd/staging-and-commit.md (R1.4, R3,
        R13.2, C2, C5, C6, C8, C9, C22) and
        docs/research/staging-and-commit/git-write-verbs.md. Do not read the
        other planning docs directly.
STEP 2  Implement.
        1. ops/: one named operation per verb of R3.1-R3.5, each a write
           invocation with git's global --literal-pathspecs before the verb (as
           reads/patches.rs and reads/working_tree.rs pass it; never the
           GIT_LITERAL_PATHSPECS variable, which the environment twin refuses
           outside process/environment.rs), `--` or --end-of-options before
           paths, the pathspec file where the verb takes it, and
           --whitespace=nowarn on every apply; the destructive ones (discard
           lines, discard files, delete untracked) take Confirmed by value, are
           put on phase 01's roster, and re-check their Consequence before
           running (R1.4); a nested repository is refused before a Consequence
           is offered (R3.5); `git clean`'s paths go on argv, split into several
           invocations under one re-check past a bound this phase measures and
           records (R3.5); no verb discards a submodule (R3.10) or takes a patch
           or a discard for a conflicted path (R3.11); each returns Performed
           with what it invalidated. Once the real destructive operations exist,
           delete the `ops::describe_destructive` placeholder in
           crates/cairn-git/src/ops/mod.rs and its test.
        2. The stale check (R3.7): the index entry read with gix (for part of an
           untracked file, that there is none), the working tree hashed by a new
           read, which also serves R1.4's re-check of a discard of files; the two
           reads of R3.9, run inside the operation on the local lane —
           `git hash-object --path=<p> -- <p>` and
           `git rev-parse --git-path hooks` — each a named function in reads/,
           built with read_invocation (the second is used by phase 05; land it
           here so the reads arrive together). Every local outcome carries its
           present and stranded locks (R3.8).
        3. apply.ignoreWhitespace: measure, on the host's git and both floors,
           whether `apply.ignoreWhitespace=change` changes what `git apply
           --cached --whitespace=nowarn` stages from a selection; if it does, pin
           `-c apply.ignoreWhitespace=false` on every apply and amend the PRD (R3
           and its filed list) to say so. Either way C5 gains the case.
        4. Tests: C2 (discard and delete halves), C5, C6, C8 (engine, the
           submodule refusal included), C9 (--literal-pathspecs on argv, no
           GIT_LITERAL_PATHSPECS in the environment) — stub git for argv, real git
           for effect, on the host's git and the floors.
        5. git's baseline (R13.2): an #[ignore]d reporter timing git's own
           `apply --cached`, `apply -R`-equivalent discard, `commit` and one
           status read on a plain, non-shared clone of rust-lang/rust at
           c999cef531e on tmpfs (never ~/Development/bench/rust itself, which
           is cloned from but never written: GIT_OPTIONAL_LOCKS=0, check its
           .git with find -newer). Write
           docs/research/staging-and-commit/measured-baseline.md, then propose
           C21's margin to the user and amend the PRD with their answer.
        6. D1: the root CLAUDE.md D1 paragraph and repo map row for cairn-git
           name the two reads and the write verbs (C22, its half);
           docs/design/engine.md already does.

        Invariants in play: only ops/ mutates; every git subprocess runs with
        a built environment, and a read with its two variables; the
        porcelain-reads twin (none of the new reads is porcelain — do not
        loosen it); destructive operations take Confirmed by value; no
        unwrap/expect on git's bytes.

        Out of scope: the lane (04), commit and amend (05), any view.
STEP 3  Validate: scripts/gate.sh (git-floor included). Then orchestrate this
        phase's QA in this session: /qa over the phase diff with the reviewers
        implementation-plan.md names for phase 03, spawned fresh, plus the
        qa-checklist.md items this phase covers and the QA brief below.
        Adjudication goes to qa-confirm (fresh), never this session inline; log
        dismissed findings with reasons in progress.md; fix confirmed findings
        in focused fixes; disputed findings go to the user.
STEP 4  Acceptance: C2 (discard and delete), C5, C6, C8 (engine), C9, C22 (D1's
        reads and verbs); the baseline recorded and C21's margin amended into the
        PRD; `git clean`'s argv bound recorded in progress.md.
STEP 5  Update state.md and progress.md; docs/systems/git-processes.md (every
        write verb, the two reads); the staging doc (a new
        docs/systems/staging.md or a section of local-changes.md — this phase's
        call), with its row in docs/systems/README.md. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user for C21's margin once the baseline exists;
if any verb behaves differently on 2.30.9; if hash-object cannot reproduce the
diff's git form of a filtered file; if the bench repository's .git changes.
Otherwise do not stop for permission.
```

## QA brief

The risk is a destructive verb whose prompt counts one thing and whose argv
destroys another.

- For each destructive verb, read its Consequence, its argv and its effect side
  by side: does `git clean` ever get a path status did not list, or `-d` for a
  row that is not a collapsed directory?
- The re-check must read the repository, not the Consequence's own copy.
- `hash-object` must run without `-w` and leave `.git/objects` unchanged —
  assert it.
- A stale patch: the C6 test must be one where `git apply` alone would succeed
  at an offset (spike E6), or it decides nothing.
