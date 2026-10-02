# Phase 05 — Merge-bar QA, then teardown

```
STEP 0  Pre-flight: read docs/work/process-manager/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/process-manager before editing.
        Verify phases 01 to 04 are present at the integration tip and every
        phase PR is merged into it.
STEP 1  Load context via an Explore agent over the whole packet diff
        (git diff main...feature/process-manager), docs/prd/process-manager.md,
        docs/work/process-manager/qa-checklist.md and progress.md (the dismissal
        log), and docs/design/processes.md. Do not read the other planning docs
        directly.
STEP 2  Review, do not build:
        1. Run /qa over the whole packet diff with every reviewer named in
           implementation-plan.md, spawned fresh: gate-integrity-reviewer,
           destructive-ops-reviewer, responsiveness-reviewer and
           test-coverage-auditor.
        2. Run the ENTIRE qa-checklist.md, including the packet-specific checks.
        3. Verify every PRD acceptance criterion, G1-G22, against its pinned
           test, and re-run the G19 reporter. A criterion pinned by a test that
           does not decide it is a finding.
        4. AUDIT the per-phase dismissal log in progress.md. A dismissal whose
           reason no longer holds is a finding.
        5. Fix confirmed findings in focused commits. Adjudication goes to the
           qa-confirm agent (fresh). Disputed findings go to the user.
        Invariants in play: all of them, since this is the merge bar.
        Out of scope: new features; anything the PRD lists as another packet's.
STEP 3  Validate: scripts/gate.sh on the integration tip.
STEP 4  Acceptance: G1-G22 all pass, the qa-checklist is fully ticked, and no
        confirmed finding is open. This is the merge bar: everything after it is
        conditional on passing.
STEP 5  Update state.md and progress.md. Then offer packet teardown per
        docs/CLAUDE.md:
        - stamp docs/prd/process-manager.md shipped, pointing at
          docs/systems/git-processes.md;
        - verify docs/systems/git-processes.md is current against the code and
          named in the root CLAUDE.md Pointers;
        - confirm the new invariants are in the root CLAUDE.md with their twins;
        - update docs/work/daily-loop/roadmap.md and state.md (process-manager
          shipped, diff-engine unblocked);
        - file leftovers with the file-issue skill (the command-log view if not
          yet filed; #25's remainder);
        - delete docs/work/process-manager/.
        Research records stay.
STEP 6  Branch authority follows the declared mode.
        - User mode: commit explicit paths, push the phase branch, raise its PR
          into feature/process-manager, and stop. After the user merges it, a
          resumed session verifies the integration tip and raises the packet PR
          from feature/process-manager to main.
        - Explicitly declared packet mode: commit teardown onto integration and
          raise the packet PR to main directly.
        The USER merges every PR.
STEP 7  Final response: the verdict, every finding and its outcome, what is
        deferred and where it was filed, and that diff-engine can resume (it
        needs feature/diff-engine brought up to date with main — the user's
        call, as it is a shared branch).
STOPPING RULES: stop and ask the user if any acceptance criterion fails and the
fix would change a locked decision; if G19 or G20 cannot be met; or before
deleting the work directory. Otherwise do not stop for permission.
```

## QA brief

The merge bar for code that every later packet will run its `git` through.

- Read the packet as a whole, not phase by phase. The failure here is
  integration: a seal from phase 01 that phase 03's migration quietly widened, or
  a registry from phase 03 that phase 04's close does not actually drain.
- Re-run the four unguarded routes from
  `docs/research/process-manager/runner-and-worker-as-built.md` section 3 in a
  scratch module, and confirm each now fails the gate.
- Confirm `diff-engine`'s resume path: a read built in `reads/`, run by the diff
  thread with its own `GitBinary` copy, cancelled by an epoch, answering `-z`
  records. If that sketch does not compile against what shipped, the packet did
  not deliver its reason for existing.
