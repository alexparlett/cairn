# State — daily-loop

The cross-session cheat sheet. Every session updates this before ending.

**Status: five packets shipped (`history-graph`, `credential-prompts`,
`process-manager`, `diff-engine`, `refs-and-status`). `refs-and-status` passed its
merge bar and was torn down on `feature/refs-and-status` (2026-10-07); it reaches
`main` with its packet pull request, merged by the user. `staging-and-commit` (5) is
planned (2026-10-07: PRD `docs/prd/staging-and-commit.md`, work directory
`docs/work/staging-and-commit/`) and next to implement, on the critical path; 5b
(`stash-and-ignore`) was inserted after it; the other four are briefs.**

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
  which is packet 5.

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
| 5 | `staging-and-commit` | **planned** — PRD `docs/prd/staging-and-commit.md` (in flight), work dir `docs/work/staging-and-commit/`, evidence `docs/research/staging-and-commit/`; O3 closed; next to implement |
| 5b | `stash-and-ignore` | brief in `roadmap.md`; depends on 5 |
| 6 | `remote-sync` | brief in `roadmap.md` |
| 7 | `branch-ops` | brief in `roadmap.md` |
| 8 | `worktrees` | brief in `roadmap.md` |

Critical path to D7: 1 → 2 → 2a → 3 → 5, with 4 needed before 5.

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
