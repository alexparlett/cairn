---
name: qa
description: End-of-contribution judgment review. Run over the current diff before calling any change done or opening a PR. Dispatches the qa-checklist agent, fans out coverage reviewers for non-trivial changes, runs the deterministic floor, and returns READY or NOT READY. Trigger on "/qa", "review my change", "is this ready".
user-invocable: true
---

# /qa

Judgment review over the current change. The deterministic layers (Stop hook,
pre-push floor, `scripts/gate.sh`) catch what grep and compilers can catch; this
skill catches what needs reading.

## Steps

1. **Scope the diff.** Apply `docs/qa-gate.md`'s Review diff scope rule: committed
   branch changes plus staged, unstaged, and untracked work. State the union in
   one line before reviewing.
2. **Dispatch the `qa-checklist` agent** over that scope. Do not duplicate its
   work inline; wait for its report.
3. **For non-trivial changes** (more than one surface, or any change to a surface
   named in the dispatch table's domain rows), fan out a parallel coverage pass
   alongside step 2: one fresh agent each for correctness, test coverage, and
   dead code. Prompt each for COVERAGE (report every gap with confidence and
   severity), not filtering.
4. **Dispatch the domain reviewers** qa-checklist names, per the table in
   `docs/qa-gate.md`, in parallel. Spawn them fresh; never have the implementer
   review its own work.
5. **Run the deterministic floor yourself**: `scripts/gate.sh --fast` plus any
   targeted tests while iterating, then the full `scripts/gate.sh` before a ready
   verdict, so the verdict rests on green checks, not only agent reasoning.
6. **Adversarially confirm each finding via the `qa-confirm` agent, spawned
   fresh**: hand it the diff scope and every raw finding; it confirms, dismisses,
   or escalates. Never adjudicate findings against your own work inline. Log
   every dismissal with its reason (packet work: `progress.md`). Disputed or
   escalated findings go to the user.
7. **Fix confirmed findings** in focused changes (only when the surrounding task
   authorizes fixes; a pure review request is read-only), re-run the affected
   checks, and re-verify.
8. **Verdict**: READY or NOT READY (the only two verdicts; notes ride the
   summary). If the full gate did not run green, list it first under verification
   still required.

Report: scope/base, commands and outcomes, reviewers used, confirmed findings,
fixes, remaining risks, and the verdict.
