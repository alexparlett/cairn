# Phase 04 — Packet QA (merge bar)

The packet's last phase, run as its OWN fresh session. Not an implementation
phase: it reviews everything the packet built.

```
STEP 0  Pre-flight: read docs/work/credential-prompts/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/credential-prompts before
        editing. Verify phases 01-03 are present at the integration tip.
STEP 1  Load context via an Explore agent over the whole packet diff
        (main...feature/credential-prompts) and docs/prd/credential-prompts.md.
        Do not read the other planning docs beyond state.md and this file
        directly.
STEP 2  Run the merge bar:
        a. /qa over the WHOLE packet diff, with every reviewer in
           implementation-plan.md's dispatch table spawned fresh, in parallel:
           qa-checklist, destructive-ops-reviewer, responsiveness-reviewer,
           gate-integrity-reviewer and test-coverage-auditor. Adjudication goes
           to qa-confirm, spawned fresh.
        b. Run the ENTIRE qa-checklist.md, starting with the secret-in-history
           check — that one is not fixable later.
        c. Verify every PRD acceptance criterion B1-B8 against its pinned test.
           A criterion whose test you cannot point at is not met.
        d. AUDIT the per-phase dismissal log in progress.md. A dismissal whose
           reason no longer holds is a finding.
        e. Fix confirmed findings in focused commits.
STEP 3  Validate: scripts/gate.sh, clean.
STEP 4  Acceptance: B1-B8 all pass. This is the merge bar — everything after
        this step is conditional on it.
STEP 5  Offer teardown to the user (docs/CLAUDE.md): stamp
        docs/prd/credential-prompts.md `shipped`, verify
        docs/systems/credentials.md is current against the as-built system and
        does NOT describe push, graduate the two new invariants into CLAUDE.md
        WITH their enforcement twins, file leftovers with /file-issue — push is
        the obvious one — then delete docs/work/credential-prompts/.
STEP 6  Update state.md and progress.md. Branch authority follows the declared
        mode: in user mode teardown lands through this phase's PR into
        feature/credential-prompts and the session stops — after the user merges
        it, a resumed session raises the packet PR to main. Packet mode may raise
        that PR directly after teardown. NEVER merge it yourself.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if any secret was ever committed (that is a
rotation, not a revert), or if O4's finding means L7 is satisfied only by
accident. Otherwise do not stop for permission.
```

## QA brief

The specific risk in this packet is that everything passes and a secret leaks
anyway, because the leak paths are the ones tests do not walk.

- Search the packet HISTORY for secrets, not the working tree. `git log -p` over
  the packet diff. A fixture credential added in phase 03 and removed in a later
  commit is still there.
- Re-read the secret type's containing types. Phase 02 built it correctly; the
  question is whether phase 03 wrapped it in something that derives Debug.
- Confirm both new guards fail on a deliberate violation. Two invariants were
  promised with twins; a twin nobody has seen go red is a promise, not a guard.
- Trace the secret's whole lifetime once, by reading: dialog → channel → helper
  stdout → git stdin. Name every place it exists and confirm each is intended.
- Check that docs/systems/credentials.md does not describe push. It is the most
  natural thing in the world to document the mechanism as though its obvious next
  consumer already exists, and that is intent stated as built.
