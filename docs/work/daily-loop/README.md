# Program: daily-loop

Reaches the bar set by decision **D7**: a version of Cairn you would use instead
of Fork on an ordinary day. Graph, diff, stage by hunk and line, commit, branch,
fetch/pull/push, and the forge links (D9) — "create pull request for this branch"
is among the most-used commands in Fork, so the bar is not met without it.

A program rather than a packet — it spans eight packets, two of which are already
filed. Program design lives in `docs/design/` (`cairn.md` for the decisions,
`feature-inventory.md` for the surface); this directory holds the build order and
the briefs.

- `roadmap.md` — the packet sequence, with a brief per packet. Build order lives
  here, never in the design spine.
- `brainstorm.md` — the locked decisions and rejected alternatives behind the
  sequence.
- `state.md` — cross-session cheat sheet.
- `progress.md` — running log, newest first.

No phase docs: each packet gets its own `/feature-plan` run when it starts,
writing its PRD and phases against then-current code.

## Not in this program

Rebase, interactive rebase, conflict resolution (D6), submodules, LFS, the
repository manager, and everything in the feature inventory's Tier 4 beyond what
`revert` needs. Those are the second lap, and interactive rebase is its own
program.
