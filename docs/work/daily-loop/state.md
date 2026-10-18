# State — daily-loop

The cross-session cheat sheet. Every session updates this before ending.

**Status: seven packets shipped (`history-graph`, `credential-prompts`,
`process-manager`, `diff-engine`, `refs-and-status`, `staging-and-commit`,
`history-topo-order`). `history-topo-order` (5c) passed its merge bar and was torn down on
`feature/history-topo-order` (2026-10-18); it reaches `main` with its packet pull request,
merged by the user, with the decisions it made in flight listed for the user's review in
`docs/research/history-topo-order/packet-record.md`. The critical path is built; D7 still needs 6
(`remote-sync`, its pull and push) and 7 (`branch-ops`, its branch operations), which are
independent of each other, beside 5b (`stash-and-ignore`, which builds on 5) and 8; all
four are briefs. Inserted 2026-10-16 ahead of them: 5c (`history-topo-order`, shipped)
and 5d (`fork-alignment`, planned, next: `docs/prd/fork-alignment.md`, `docs/work/fork-alignment/`),
with 5e-5h (`file-tree-view`, `quick-launch`, `preferences`, `repository-manager`) briefs after
5d (`roadmap.md`).**

## The milestone

D7 in `docs/design/cairn.md`: graph, diff, stage by hunk and line, commit, branch,
fetch/pull/push. The bar is "would use this instead of Fork on an ordinary day".

## Locked decisions

L1-L7 in `brainstorm.md`. The three that most constrain implementation:

- **The diff model is patch-capable from its first commit (L2).** A display-shaped
  model makes line staging a rewrite. Packet 3 builds the patch emitter and its
  round-trip test before anything consumes it. Its own decisions (L1-L16, its
  brainstorm, deleted at teardown) are in `docs/design/diff.md` and
  `docs/prd/diff-engine.md`; two of them change things outside it, D1's amendment
  for filter drivers and the per-lane epochs.
- **`credential-prompts` is load-bearing for staging (L3)**, because
  `git apply --cached` runs through its backend. It is not merely early.
- **The reflog view ships with the first commit-level destructive operation (L4)**,
  which is packet 5: Show Lost Commits shipped with amend.

## Open questions

O1-O6 in `brainstorm.md`, each assigned to the packet that meets it. **O1 is
closed** by `diff-engine` (its L1): both presentations, unified by default, over a
patch model independent of either. O2 (`gix-status` versus
`git status --porcelain=v2`) is the one that could reach back into D1, because
status is the most divergence-prone read there is — and `diff-engine` has since
amended D1 for the filter drivers that status also runs. **O2 is closed** by
`refs-and-status` (its L1, 2026-10-05): `git status --porcelain=v2 -z`, run as a
read, after a 38-fixture agreement spike. **O3 is closed** by `staging-and-commit`
(its L2, 2026-10-07): a discarded uncommitted edit is protected by its confirmation
alone, as in Fork — no backup, no auto-stash.

## Packet status

| # | Packet | Status |
| --- | --- | --- |
| 1 | `history-graph` | **shipped** — PRD frozen, as-built in `docs/systems/history-graph.md`; work dir torn down |
| 2 | `credential-prompts` | **shipped** — PRD frozen, as-built in `docs/systems/credentials.md`; work dir torn down |
| 2a | `process-manager` | **shipped** — PRD frozen, as-built in `docs/systems/git-processes.md`; work dir torn down; leftovers #41-#49 and #25 |
| 3 | `diff-engine` | **shipped** — PRD frozen, as-built in `docs/systems/diff.md`; work dir torn down; lands on `main` with its packet PR (squash merge); leftovers #54-#58, with #51-#53 |
| 4 | `refs-and-status` | **shipped** — PRD frozen, as-built in `docs/systems/refs.md`, `status.md`, `sidebar.md`, `local-changes.md` and `history-graph.md`; work dir torn down; numbers in `docs/research/refs-and-status/measured.md`; O2 closed for `git status`; leftovers #62-#82 |
| 5 | `staging-and-commit` | **shipped** — PRD frozen, as-built in `docs/systems/staging.md`, `local-changes.md`, `git-processes.md` and `history-graph.md`; work dir torn down; numbers in `docs/research/staging-and-commit/measured.md`; O3 closed (no backup before a discard); leftovers #90-#133 |
| 5c | `history-topo-order` | **shipped** — PRD frozen, as-built in `docs/systems/history-graph.md` (and the threads in `git-processes.md`); work dir torn down; numbers in `docs/research/history-topo-order/measured.md`, decisions pending the user's review in `packet-record.md` beside it; leftovers #139-#141 |
| 5b | `stash-and-ignore` | brief in `roadmap.md`; depends on 5 |
| 6 | `remote-sync` | brief in `roadmap.md` |
| 7 | `branch-ops` | brief in `roadmap.md` |
| 8 | `worktrees` | brief in `roadmap.md` |

Critical path to D7: 1 → 2 → 2a → 3 → 5, with 4 needed before 5 — built; D7 then needs 6 and 7.

## Environment notes

- `freya` 0.5-rc and `gix` 0.87.1 are pre-1.0. Verify APIs against the vendored
  source under `~/.cargo/registry/src/`, never from memory.
- `scripts/gate.sh` is the bar. Never an ad-hoc `&&` chain, never piped through
  `tail`.
- Commit explicit paths, never `git add -A`.
- The remote is `github.com/alexparlett/cairn`: pull requests and issues go there,
  and every pull request is merged by the user, never by a session.
- `~/Development/bench/rust` is a clone of rust-lang/rust kept for measured bars
  (`diff-engine` L12). Read it only — a `gc` or `repack` there invalidates
  recorded numbers.
