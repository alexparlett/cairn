# Phase 04 — Lanes, and a thread for diffs

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing.
        Verify phases 01 to 03 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/worker/ and
        crates/cairn-app/src/session.rs, docs/prd/diff-engine.md (R4 and
        criterion C8), docs/research/diff-engine/engine-and-worker-as-built.md
        section 3, and docs/research/diff-engine/gix-diff-api.md section 8 for
        what is Send and where a cancel can land. Do not read the other planning
        docs directly.
STEP 2  Implement. L8 decided the lanes and the thread; build to that.

        Deliverables:
        1. Epochs per lane (R4.1): history, changes and file diff, each
           superseding only itself, except that a new changes query also
           supersedes the file-diff lane. This replaces the single counter and
           the is-a-query predicate, both of which currently assume one query
           kind. So does serve(): its match over requests is total with no
           wildcard, the non-history arms end in continue, and the paging code
           after the match is the history path. Undo that shape deliberately.
        2. The diff thread (R4.2, R4.3, R4.5): its own repository handle, the
           tree state, a content cache reused across commits and rebuilt for each
           working-tree query, an object cache sized by gix's own helper, a
           routing table from lane to thread in place of
           WORKERS_PER_REPOSITORY, newest-request-per-lane scheduling with file
           diffs first, and yielding between files during work that spans them.
        3. The requests, updates and session state (R4.4): every answer names its
           target — the commit or pair of commits, the path, the options — and
           the window keeps an answer only for the selection it names. With the
           tests for C8, written through the real boundary the way the existing
           pool tests are.

        Invariants in play: the UI thread never waits on repository work — the
        new code lives under crates/cairn-app/src/worker/ and nothing outside it
        may name the engine or a waiting primitive; Update and Request derive
        Debug, Clone, PartialEq and Eq, so every payload must too; a match over
        RowContent names every variant.

        Out of scope: anything that draws (phases 05 to 08); new engine queries.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with responsiveness-reviewer and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items for
        this phase and the QA brief below. Adjudication goes to qa-confirm
        (fresh); log dismissals with reasons in progress.md.
STEP 4  Acceptance: C8 passes against its tests.
STEP 5  Update state.md and progress.md. Update docs/systems/history-graph.md
        where it describes one epoch counter and one worker per repository, and
        the worker bullet in the root CLAUDE.md architecture section — both
        describe the world this phase changes. D3 in docs/design/cairn.md is
        intent and already states the lanes; it needs no edit unless this phase
        changes the design. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the history session cannot stay on its
own thread under the routing table, or if any lane turns out to need the UI thread
to wait — the first reopens D3, the second breaks an invariant. Otherwise do not
stop for permission.
```

## QA brief

The failure here is silent: a diff that never arrives, or one that arrives under
the wrong commit, on a machine faster or slower than the one that wrote the code.

- State the mutation that makes each C8 test fail. "A scroll does not cancel a
  diff" is only tested if reverting to one counter turns it red.
- Read the supersession rules against the click sequences a user actually makes:
  select, scroll, select again, scroll while a file diff is loading, select a file
  then select another commit. Each must end with exactly the right answer drawn.
- Check what happens to the answer that finishes after it was superseded. It must
  be dropped on arrival, not drawn and then replaced — a flash of the wrong diff
  is the bug this design exists to prevent.
- The diff thread holds caches. Confirm the working-tree one is rebuilt per query
  and the commit one is keyed by ids that cannot go stale.
- Confirm nothing outside worker/ gained a waiting primitive or a name from the
  engine, and that the exempt worker functions the UI thread calls did not grow a
  blocking call.
- Look for a busy poll. A scheduling loop that spins between requests is worse
  than one that blocks, and the guard cannot see it inside worker/.
