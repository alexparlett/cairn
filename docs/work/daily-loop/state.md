# State — daily-loop

The cross-session cheat sheet. Every session updates this before ending.

**Status: two of eight packets shipped (`history-graph`, `credential-prompts`);
`diff-engine` is planned and has not started. The other five are briefs.**

## The milestone

D7 in `docs/design/cairn.md`: graph, diff, stage by hunk and line, commit, branch,
fetch/pull/push. The bar is "would use this instead of Fork on an ordinary day".

## Locked decisions

L1-L7 in `brainstorm.md`. The three that most constrain implementation:

- **The diff model is patch-capable from its first commit (L2).** A display-shaped
  model makes line staging a rewrite. Packet 3 builds the patch emitter and its
  round-trip test before anything consumes it. Its own decisions are
  `docs/work/diff-engine/brainstorm.md` L1-L16; two of them change things outside
  it, D1's amendment for filter drivers and the per-lane epochs.
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
amended D1 for the filter drivers that status also runs.

## Packet status

| # | Packet | Status |
| --- | --- | --- |
| 1 | `history-graph` | **shipped** — PRD frozen, as-built in `docs/systems/history-graph.md`; work dir torn down |
| 2 | `credential-prompts` | **shipped** — PRD frozen, as-built in `docs/systems/credentials.md`; work dir torn down |
| 3 | `diff-engine` | **planned** — PRD `docs/prd/diff-engine.md` in flight; work dir `docs/work/diff-engine/`; nine phases, none started |
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
- The remote is `github.com/alexparlett/cairn`: pull requests and issues go there,
  and every pull request is merged by the user, never by a session.
- `~/Development/bench/rust` is a clone of rust-lang/rust kept for measured bars
  (`diff-engine` L12). Read it only — a `gc` or `repack` there invalidates
  recorded numbers.
