# State — daily-loop

The cross-session cheat sheet. Every session updates this before ending.

**Status: program planned. Two of its eight packets are filed; neither has
started. No code exists for any of them.**

## The milestone

D7 in `docs/design/cairn.md`: graph, diff, stage by hunk and line, commit, branch,
fetch/pull/push. The bar is "would use this instead of Fork on an ordinary day".

## Locked decisions

L1-L7 in `brainstorm.md`. The three that most constrain implementation:

- **The diff model is patch-capable from its first commit (L2).** A display-shaped
  model makes line staging a rewrite. Packet 3 builds the patch emitter and its
  round-trip test before anything consumes it.
- **`credential-prompts` is load-bearing for staging (L3)**, because
  `git apply --cached` runs through its backend. It is not merely early.
- **The reflog view ships with the first commit-level destructive operation (L4)**,
  which is packet 5.

## Open questions

O1-O6 in `brainstorm.md`, each assigned to the packet that meets it. O2
(`gix-status` versus `git status --porcelain=v2`) is the one that could reach back
into D1, because status is the most divergence-prone read there is.

## Packet status

| # | Packet | Status |
| --- | --- | --- |
| 1 | `history-graph` | **shipped** — PRD frozen, as-built in `docs/systems/history-graph.md`; work dir torn down |
| 2 | `credential-prompts` | filed — `docs/work/credential-prompts/` |
| 3 | `diff-engine` | brief in `roadmap.md` |
| 4 | `refs-and-status` | brief in `roadmap.md` |
| 5 | `staging-and-commit` | brief in `roadmap.md` |
| 6 | `remote-sync` | brief in `roadmap.md` |
| 7 | `branch-ops` | brief in `roadmap.md` |
| 8 | `worktrees` | brief in `roadmap.md` |

Critical path to D7: 1 → 2 → 3 → 5, with 4 needed before 5.

## Environment notes

- `freya` 0.5-rc and `gix` 0.87.1 are pre-1.0. Verify APIs against the vendored
  source under `~/.cargo/registry/src/`, never from memory.
- `scripts/gate.sh` is the bar. Never an ad-hoc `&&` chain, never piped through
  `tail`.
- Commit explicit paths, never `git add -A`.
- The repository has no remote yet, so the PR step in every packet's phase prompts
  has nowhere to go. Say so rather than pretending.
