# Progress — history-topo-order

Running log, newest first. Each phase's dismissed QA findings are logged here with their
reasons, for the final QA phase to audit.

## 2026-10-17 — planned

Planned with `/feature-plan`, on the bug `fork-alignment`'s planning found: with tied commit
times Cairn draws a parent above its child.

Evidence in `docs/research/history-topo-order/`:
- git's order and algorithm, cited from git's source at v2.30.9 and v2.56.0, which the user
  approved fetching;
- gitoxide's walks;
- Cairn's walk as built.

The user locked L1-L7 (`brainstorm.md`); L8 records two corrections a coverage audit found:
- date order;
- Cairn's own walk;
- accept the opening pass, visibly;
- a history thread;
- the oracle;
- the git source fetched for citations;
- a reopen keeping the old rows.

`docs/design/history-graph.md` and `concurrency.md` were rewritten to the end state.
