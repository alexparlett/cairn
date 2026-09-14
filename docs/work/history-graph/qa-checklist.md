# QA checklist — history-graph

Packet-specific acceptance beyond the repo-wide gate. The acceptance criteria
A1-A9 live in `docs/prd/history-graph.md` and are NOT copied here — verify them
there, against their pinned tests.

## Per-phase coverage of the PRD criteria

| Phase | PRD criteria it must satisfy |
| --- | --- |
| 01 | A1, A2, A3 |
| 02 | A4, A5 |
| 03 | A6 (with 04) |
| 04 | A6, A7, A8 |
| 05 | all of A1-A9, re-verified over the whole packet diff |

## Packet-specific checks

Beyond the PRD, phase 05 confirms:

- [ ] `cairn-model` still depends on nothing. Phase 01 adds types there; a stray
      dependency would defeat the seam the packet is built on.
- [ ] No `unwrap`/`expect` reachable from the assigner or the query. Both take
      whatever a real repository emits, and the clippy floor already denies them
      outside tests — confirm nothing was allowed through with an attribute.
- [ ] Every open question O1-O4 in `brainstorm.md` is either answered with its
      decision recorded, or explicitly carried forward as a filed issue. An open
      question that quietly stopped mattering is still a finding.
- [ ] The "UI thread never waits on repository work" invariant has moved out of
      the *not yet mechanically pinned* list in `CLAUDE.md`, with its guard —
      phase 03's definition of done.
- [ ] `docs/systems/history-graph.md` exists and describes only what was built.
- [ ] The A7 measurement is recorded in `progress.md` with the repository it ran
      against and the numbers observed, not as "looks smooth".
- [ ] R5 stayed minimal: a command-line argument and an error path, not the start
      of a repository picker. R5.3 parks that decision deliberately, and a picker
      landed here would pre-empt it by accident.
- [ ] The worker boundary's design notes say which parts exist for fetch
      (credential-prompts R4) rather than for the graph — phase 03's obligation,
      and the thing that stops the next packet rewriting the interface.
