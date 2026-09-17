# Phase 02 — Commit and comparison diffs in the engine

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing.
        Verify phase 01 is present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-git/ and
        crates/cairn-model/ (phase 01's types, which this phase fills),
        docs/prd/diff-engine.md (R2 and criteria C1, C2, C3, C5, C6, C14) and
        docs/research/diff-engine/gix-diff-api.md. Then re-read the vendored gix
        0.87.1 source for every API you write against: the version-sensitive API
        rule in the root CLAUDE.md is not satisfied by the research record alone.
        Do not read the other planning docs directly.
STEP 2  Implement. L3 fixed where the diff comes from: gix computes it and Cairn
        groups it. Do not write a diff algorithm.

        Deliverables:
        1. The changes query of R2.1, R2.2, R2.9 and R2.10: two commits, or one
           against its first parent, or a root commit against the empty tree;
           changed files sorted by path; renames and copies per the user's
           config, with a cut-short rename limit reported; the commit details the
           Commit tab needs; cancellation at every change. No gix type in the
           signature.
        2. The content query of R2.3 through R2.8: both versions through gix's
           resource cache in its to-git mode, with the external diff program
           never spawned; binary detection as git does it; the too-large limits,
           the size one tested before the content is read; lines tokenised with
           their terminators kept; the algorithm from config; the second,
           display-only whitespace-ignoring pass; the intra-line ranges.
        3. Fixtures with real content — the existing ones commit nothing but
           empty trees — and the tests for C1, C2, C3, C5 and C6. C1 and C2 apply
           Cairn's patches with real `git apply --cached` into a scratch index and
           compare trees; C5 and C6 compare against git's own output under the
           same config.
        4. An #[ignore]d reporter driven by CAIRN_BENCH_REPO for C14's engine
           numbers, run against ~/Development/bench/rust, with the numbers written
           into progress.md beside git's own from
           docs/research/diff-engine/measured-baseline.md. It also answers Q3:
           how far gix's rename pairs on 5a3292f163d are from git's 2,543.

        Invariants in play: any change to cairn-model — the commit details type
        of R1.8 is the one most likely to move here — needs its test in the same
        commit and nothing new in that crate's allowlist; the crate seal (no gix
        type in a public signature, and
        cairn-git never names the toolkit); only cairn-git/src/ops/ mutates a
        repository — the tests run `git apply` into a scratch index, so confirm
        the guard's scope covers test code rather than assuming it; errors are
        thiserror variants naming what the caller handles; no unwrap or expect in
        shipping code.

        Out of scope: working-tree diffs (phase 03), the worker (phase 04),
        anything that draws.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with test-coverage-auditor spawned
        fresh, plus the qa-checklist.md items for this phase and the QA brief
        below. Adjudication goes to qa-confirm (fresh); log dismissals with
        reasons in progress.md; fix confirmed findings in focused fixes.
STEP 4  Acceptance: C1, C2, C3, C5 and C6 pass against their tests, and C14's
        engine numbers are recorded.
STEP 5  Update state.md and progress.md (numbers, and Q3's answer). Extend
        docs/systems/diff.md with the engine as built. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if gix cannot honour the user's
diff.algorithm or git's indent heuristic as L3 assumes; if the rename gap against
git on 5a3292f163d is large enough to put C14 in question; or if any of this
appears to need a new dependency. Otherwise do not stop for permission.
```

## QA brief

The risk here is a test that passes because both sides of it are wrong.

- C1 must compare trees, not patch text. A patch that applies cleanly and
  produces the wrong content passes any text comparison you write.
- C2's selections must include the mean cases: every line, no lines, only
  additions, only removals, the first line of a hunk, the last line of a file
  with no trailing newline. A seed set that only ever selects interior lines
  tests nothing about the edges.
- Confirm the too-large size check really precedes the read. Instrument it or use
  a blob large enough that reading it would be obvious in the timing.
- Confirm no external program ran: configure both a textconv and an external
  diff command in a fixture and prove the diff ignored them.
- The changes query sorts what gix returns in walk order. Check the sort is total
  and stable for a rename pair, or the file list will shuffle between runs.
- Read the cancellation path. A cancelled query must stop the walk, not finish it
  and throw the answer away.
