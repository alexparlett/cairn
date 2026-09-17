# Phase 09 — Packet QA (merge bar)

The packet's last phase, run as its OWN fresh session. Not an implementation
phase: it reviews everything the packet built.

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing.
        Verify phases 01 to 08 are present at the integration tip.
STEP 1  Load context via an Explore agent over the whole packet diff
        (main...feature/diff-engine) and docs/prd/diff-engine.md. Do not read the
        other planning docs beyond state.md and this file directly.
STEP 2  Run the merge bar:
        a. /qa over the WHOLE packet diff, with every reviewer in
           implementation-plan.md's dispatch table spawned fresh, in parallel:
           qa-checklist, test-coverage-auditor, responsiveness-reviewer,
           gate-integrity-reviewer and destructive-ops-reviewer. Adjudication
           goes to qa-confirm, spawned fresh.
        b. Run the ENTIRE qa-checklist.md.
        c. Verify every PRD acceptance criterion C1-C16 against its pinned test.
           A criterion whose test you cannot point at is not met.
        d. AUDIT the per-phase dismissal log in progress.md. A dismissal whose
           reason no longer holds is a finding.
        e. Fix confirmed findings in focused commits.
STEP 3  Validate: scripts/gate.sh, clean.
STEP 4  Acceptance: C1-C16 all pass. This is the merge bar — everything after
        this step is conditional on it.
STEP 5  Offer teardown to the user (docs/CLAUDE.md): stamp
        docs/prd/diff-engine.md `shipped`, verify docs/systems/diff.md is current
        against the as-built system and describes no screen that does not exist,
        graduate the two new invariants into CLAUDE.md WITH their enforcement
        twins, update docs/work/daily-loop/roadmap.md and state.md for the
        packet, file leftovers with /file-issue and update any of #29 to #37 the
        packet touched, then delete docs/work/diff-engine/.
STEP 6  Update state.md and progress.md. Branch authority follows the declared
        mode: in user mode teardown lands through this phase's pull request into
        feature/diff-engine and the session stops — after the user merges it, a
        resumed session raises the packet pull request to main. Packet mode may
        raise that pull request directly after teardown. NEVER merge it yourself.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a C14 ceiling is missed, if any
criterion has no test that decides it, or if a dismissal from an earlier phase
turns out to have been wrong in a way that needs a decision rather than a fix.
Otherwise do not stop for permission.
```

## QA brief

The packet's specific risk is that everything renders beautifully and the patch
model is subtly wrong, because nothing in this packet consumes the emitter. The
consumer arrives one packet later, when the cost of being wrong is a rewrite.

- Re-read the round-trip tests with fresh eyes. Do they compare trees? Do the
  selections reach the edges? Is the reference applier still independent of the
  emitter, or did a later phase refactor them together?
- Trace one line from disk to screen and back to a patch: the bytes gix loaded,
  the line the model holds, the row the view drew, the selection identity, the
  patch line. Name every place the content is copied or transformed, and confirm
  each is intended.
- Confirm the working-tree path still runs the filter and still writes nothing,
  after four UI phases have touched the code around it.
- Confirm no answer can be drawn against the wrong selection under any click
  order you can construct.
- Confirm the two new guards fail on a deliberate violation, and that the
  exceptions roster for unbounded views is still empty.
- Read docs/systems/diff.md against the code. It is the first document a packet-5
  session will read, and a sentence describing something that does not exist will
  send that session building against it.
