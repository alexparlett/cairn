As-built descriptions, one per system that exists; see docs/CLAUDE.md for the
contract. Each is kept current in the same change that invalidates it.

- `history-graph.md` — how the history view reads, lays out and draws a
  repository: the worker boundary, the history session, the lane assigner and
  the virtualized list.
- `credentials.md` — the `git` subprocess backend and its environment, the
  askpass helper and its channel, the secret type, fetch end to end, and the
  decisions the `credential-prompts` packet locked.
