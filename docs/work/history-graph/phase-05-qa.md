# Phase 05 — Packet QA (merge bar)

The packet's last phase, run as its OWN fresh session. Not an implementation
phase: it reviews everything the packet built.

```
STEP 0  Pre-flight: read docs/work/history-graph/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/history-graph before editing.
        Verify phases 01-04 are present at the integration tip.
STEP 1  Load context via an Explore agent over the whole packet diff
        (main...feature/history-graph) and docs/prd/history-graph.md. Do not read
        the other planning docs beyond state.md and this file directly.
STEP 2  Run the merge bar:
        a. /qa over the WHOLE packet diff, with every reviewer in
           implementation-plan.md's dispatch table spawned fresh, in parallel:
           qa-checklist, test-coverage-auditor, responsiveness-reviewer, and
           gate-integrity-reviewer (phase 03 touched the enforcement layer).
           Adjudication goes to qa-confirm, spawned fresh.
        b. Run the ENTIRE qa-checklist.md.
        c. Verify every PRD acceptance criterion A1-A8 against its pinned test.
           A criterion whose test you cannot point at is not met.
        d. AUDIT the per-phase dismissal log in progress.md. A dismissal whose
           reason no longer holds is a finding.
        e. Fix confirmed findings in focused commits.
STEP 3  Validate: scripts/gate.sh, clean.
STEP 4  Acceptance: A1-A8 all pass. This is the merge bar — everything after
        this step is conditional on it.
STEP 5  Offer teardown to the user (docs/CLAUDE.md): stamp
        docs/prd/history-graph.md `shipped`, verify docs/systems/history-graph.md
        is current against the as-built system, graduate any new invariants into
        CLAUDE.md WITH their enforcement twins, file leftovers with /file-issue,
        then delete docs/work/history-graph/. Git history is the archive.
STEP 6  Update state.md and progress.md. Branch authority follows the declared
        mode: in user mode teardown lands through this phase's PR into
        feature/history-graph and the session stops — after the user merges it, a
        resumed session raises the packet PR to main. Packet mode may raise that
        PR directly after teardown. NEVER merge it yourself.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if A7's measurement is bad enough to
question D4, or if an open question O1-O4 was never actually resolved — shipping
a packet whose central decision was made implicitly is worse than shipping late.
Otherwise do not stop for permission.
```

## QA brief

The specific risk in this packet: the four phases each touch a different crate,
so an integration defect has no single owner and every phase's QA could pass
while the whole is wrong.

- Trace one commit end to end: gix walk → assigner → worker → rendered row. Any
  field that loses meaning in transit is a finding.
- Re-check the crate seal by reading the manifests, not by trusting the guard —
  then confirm the guard would have caught a violation, by making one.
- Confirm the "not yet mechanically pinned" list in CLAUDE.md actually shrank.
  Phase 03 owed a guard; a phase that shipped without paying that debt is the
  exact failure the meta-invariant exists to prevent.
- Check docs/systems/history-graph.md describes only what exists. Intent stated
  as built is a docs-tier finding, and this is the packet's last chance to catch
  it.
