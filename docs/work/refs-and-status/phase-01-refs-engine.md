# Phase 01 — The refs snapshot and ahead/behind in the engine

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        This is phase 01: if feature/refs-and-status does not exist, first
        create it from main (the packet's declared base, with the planning PR
        merged), in a git worktree of its own as the root CLAUDE.md requires
        of a packet's branch, and push it. Direct integration work requires an orchestrator
        prompt that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/refs.rs,
        crates/cairn-git/src/repository.rs (open and its refusals),
        crates/cairn-git/src/lib.rs, crates/cairn-model/src/ (Oid, RefName),
        crates/cairn-app/src/worker/network_lane.rs (ref_tips' one caller),
        crates/cairn-git/tests/fixtures/, docs/prd/refs-and-status.md (R1, R2,
        C1, C2, C3, C11) and docs/research/refs-and-status/gix-refs-and-status-api.md
        sections on refs, upstream, stash and worktrees. Then re-read the
        vendored gix 0.87.1 source (~/.cargo/registry/src/*/gix-0.87.1/,
        gix-ref) for every API you write against; the research record does not
        satisfy the version-sensitive API rule. Do not read the other planning
        docs directly.
STEP 2  Implement.
        1. cairn-model: the snapshot's vocabulary — a ref (full name, kind:
           local, remote-tracking, tag; the commit it identifies; an annotated
           tag's object id; a symbolic ref's target name), HEAD's state (branch,
           detached at, unborn on), a local branch's upstream (name, exists or
           gone), a stash entry (index, message, commit, base), ahead/behind
           counts, and a count of refs skipped as unreadable. Plain data, with
           unit tests in the same commit. RefName stays what it is unless a
           test shows it must change.
        2. cairn-git: the refs query of R1 through gix, under the four parity
           rules of L3 (never peel a symbolic ref; hide a dangling one; stash
           reflog oldest-first then reversed; `remote = .` by hand), in
           for-each-ref's order, cancellable, no gix type at the boundary.
           Rebuild Repository::ref_tips on the snapshot so the network lane's
           before/after comparison sees symbolic refs and tag objects as the
           snapshot does (phase 06 replaces that comparison with the refresh's;
           ops/fetch.rs's contract doc names ref_tips, so keep it true). Report
           the query's cost (R1.7). Open without GIT_NAMESPACE (R1.8).
        3. cairn-git: refuse a reftable repository at open (R1.9) beside the
           dubious-ownership refusal, with a reason the window already draws.
        4. cairn-git: ahead/behind (R2) — two walks with the other side hidden
           per branch with an existing upstream, cancellable between branches
           and within a walk.
        5. Tests: C1 and C3 against real git in fixtures (git for-each-ref,
           git symbolic-ref, git rev-parse, git stash list, %(upstream),
           git rev-list --left-right --count), including GIT_NAMESPACE set in
           the test's own environment and a cancelled ahead/behind; C2 with its CAIRN_REQUIRE_*
           variable, gate probe and twin (implementation-plan.md, "New
           enforcement"). An #[ignore]d reporter driven by CAIRN_BENCH_REPO for
           C11's refs numbers on ~/Development/bench/rust (read only) and on a
           generated 10,000-ref fixture; numbers into progress.md.

        Invariants in play: cairn-model is plain data and every change needs its
        test in the same commit; the crate seal; only ops/ mutates (nothing here
        writes — a reflog read is a read); a test that skips must be required
        wherever its host can serve it (C2); no unwrap/expect in shipping code;
        errors are thiserror variants naming what the caller handles.

        Out of scope: status (02), compact and slim rows (03, 04), the history walk and labels (05), the worker
        lanes (06), anything that draws.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 01, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C1, C2 and C3 pass against their tests; C11's refs numbers
        are recorded.
STEP 5  Update state.md (new modules and interfaces, validation status) and
        progress.md; create docs/systems/refs.md and its README row; update
        docs/systems/git-processes.md where it lists what open refuses. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if gix disagrees with git for-each-ref on
anything the four parity rules do not cover (git parity is critical — never file
it and move on); if reftable cannot be detected before gix fails; if
ahead/behind on the bench repository threatens C11's 100 ms; or if anything here
seems to need a new dependency. Otherwise do not stop for permission.
```

## QA brief

The risk is a parity test that agrees with itself.

- Every expectation in C1 and C3 must come from running git in the same fixture
  at test time. A list typed into the test from a run of Cairn passes when both
  are wrong.
- `origin/HEAD` is the trap: check it is listed as itself, naming
  `refs/remotes/origin/main`, and that `origin/main` is listed exactly once.
- The stash fixture needs both a message over 4 KiB and more than one entry
  after it; a reader that stops at the long line passes a fixture where the long
  message is the oldest.
- A tag on a tree and a tag chain: check the first identifies no commit and the
  second its final commit, with the outer tag object's id kept.
- Read the cancellation: a cancelled ahead/behind must stop its walk, not finish
  it and drop the answer.
- Confirm the reftable refusal happens before any gix ref read, and that the
  required-test pin really fails when the variable is set and the host can make
  a reftable repository but the test skipped.
