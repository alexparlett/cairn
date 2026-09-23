# Phase 03 — Working-tree diffs, and D1's amendment

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing.
        Verify phases 01 and 02 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-git/,
        docs/prd/diff-engine.md (R3, criteria C7 and C15),
        docs/research/diff-engine/gix-diff-api.md sections 5 and 6, and
        docs/design/engine.md ("Reads see git's form" and the residuals). Re-read the
        vendored gix-filter and gix-status source before writing against them.
STEP 2  Implement. L6 decided that a working-tree read runs the user's clean
        filter driver, exactly as `git diff` does, and that D1 says so.

        Deliverables:
        1. The queries of R3.1 through R3.5: one path's staged, unstaged or
           untracked diff; working-tree content converted to git's form through
           gix's filter pipeline, drivers included and textconv never; the index
           and attributes read fresh per query; the states of R3.4 answered
           rather than errored; and nothing written — no refreshed index, no
           object.
        2. The fixtures and tests for C7: a `text=auto` file with CRLF endings, a
           path under a configured clean filter driver (a script the fixture
           writes, so the test needs no git-lfs), a deleted file, a type change, a
           mode-only change, a conflicted path, a submodule, and a sparse index.
           Each compared against git's own output where git has one, and the
           index file checked byte-identical after every query.
        3. D1's amendment in the root CLAUDE.md, matching the wording already in
           docs/design/engine.md, including the residual it creates: the filter
           driver runs with Cairn's inherited environment plus the repository's
           paths, not with a GitEnvironment, because gix starts it. C15.

        Invariants in play: only cairn-git/src/ops/ mutates a repository — this
        phase is the one most likely to break it by accident, since gix offers
        both a refreshed-index write and a blob write on paths next to the ones
        you want; the crate seal; no unwrap or expect in shipping code.

        Out of scope: enumerating which paths changed, which is status and packet
        4's; any Local Changes screen; the worker (phase 04).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with test-coverage-auditor and
        destructive-ops-reviewer spawned fresh — this phase makes a read run a
        user-configured program, which is that reviewer's judgement even though
        no ops/ file changes. Plus the qa-checklist.md items for this phase and
        the QA brief below. Adjudication goes to qa-confirm (fresh); log
        dismissals with reasons in progress.md.
STEP 4  Acceptance: C7 and C15 pass.
STEP 5  Update state.md and progress.md. Extend docs/systems/diff.md with the
        working-tree path as built, including what it runs and with which
        environment. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if gix's pipeline cannot run a configured
driver on a read path, or if obtaining git's form of a working-tree file turns out
to require a write — either reopens L6, which is the user's decision to retake.
Otherwise do not stop for permission.
```

## QA brief

The defect class here is a diff that looks right on the machine that wrote it and
is wrong on a machine with a filter configured.

- Prove the filter actually ran. The fixture's driver should leave a distinctive
  transformation in the content, so a test can tell "the filter ran" from "the
  file happened to match".
- Prove nothing was written. Compare the index file byte for byte before and
  after, not just its modification time, and check no loose object appeared.
- The CRLF case is the one that silently regresses. A `text=auto` file with CRLF
  in the working tree must show no change at all, not every line changed.
- Read what happens when the driver fails or is missing. A filter that exits
  non-zero must surface as an error naming the path, never as an empty diff.
- Check the conflicted path answers conflicted rather than diffing against one
  stage. D6 owns resolution; this phase owns saying so.
- Read D1's amendment in both files and confirm they say the same thing. An
  amendment that lives in one of them is how the two drift.
