# QA checklist — credential-prompts

Packet-specific acceptance beyond the repo-wide gate. The acceptance criteria
B1-B8 live in `docs/prd/credential-prompts.md` and are NOT copied here — verify
them there, against their pinned tests.

## Per-phase coverage of the PRD criteria

| Phase | PRD criteria it must satisfy |
| --- | --- |
| 01 | B1, B2 |
| 02 | B6, B7 |
| 03 | B3, B4, B5 |
| 04 | all of B1-B8, re-verified over the whole packet diff |

## Packet-specific checks

Beyond the PRD, phase 04 confirms:

- [ ] **No secret is committed.** Search the packet diff, not just the working
      tree: a fixture credential that was committed and later removed is still in
      history. If one was ever committed, say so — it is a rotation, not a revert.
- [ ] The two new invariants from `implementation-plan.md` are in `CLAUDE.md`
      WITH their named guards, and each guard has been seen to fail on a
      deliberate violation.
- [ ] `only_the_ops_module_mutates_a_repository` actually fires. Phase 01 is the
      first code that guard constrains; confirm by violating it, not by trusting
      it.
- [ ] The secret type still has no `Debug`, `Display` or `Serialize`, and nothing
      has since derived one on a type that contains it. A containing type that
      derives `Debug` defeats the whole thing.
- [ ] A cancelled or failed prompt leaves no process behind — no orphaned helper,
      no `git` waiting on a pipe nobody will write to.
- [ ] Every open question O1-O5 in `brainstorm.md` is answered with its decision
      recorded, or explicitly carried forward as a filed issue.
- [ ] `docs/systems/credentials.md` exists and describes only what was built —
      in particular, it must not describe push, which this packet does not land.
