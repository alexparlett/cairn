# Progress — process-manager

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-10-02 — planned

The packet was planned with `/feature-plan` while `diff-engine` was in flight, when its
changes query hit rename parity (its 2026-09-30 progress entry). Four recon records are saved under
`docs/research/process-manager/`, and the two that commissioned the packet were
copied from the `diff-engine` branch into `docs/research/diff-engine/` so `main`
can cite them.

- L1-L4 were locked by the user. L5-L15 were presented as defaults and stood.
- The design landed in the same PR:
  - `docs/design/processes.md` (new);
  - D1 rewritten in `engine.md` so that `git` answers a read where gix diverges;
  - the write lanes in `concurrency.md`;
  - the spine's map, summary and open list.
- The PRD is `docs/prd/process-manager.md`, with five phases. No code has
  changed.
