# Brainstorm — daily-loop

Locked decisions and rejected alternatives. Historical record: never retro-edited.

## Locked 2026-09-14

**L1. The milestone is the daily loop, not a read-only explorer.** Design decision
D7. Rejected: shipping a viewer first — it arrives sooner but replaces nothing,
and the argument that it would validate the gitoxide read bet does not hold,
because the `history-graph` packet validates that anyway. Also rejected: read plus
commit with no remote, which defers credential work into a milestone that still
sends you back to Fork every day.

**L2. The diff model is patch-capable from its first commit.** Not a display
model with staging bolted on later. Staging a single line means constructing a
patch for an arbitrary subset of hunks and lines — correct headers, correct
context — and handing it to `git apply --cached`. A display-only model makes
line staging a rewrite rather than a feature. Cites:
`docs/design/feature-inventory.md`, "Why diff is the one to get right".

**L3. `credential-prompts` moves ahead of the diff work.** Its phase 01 is the
`git` subprocess backend, and staging needs that backend for
`git apply --cached`. It was already second in the sequence for its own reasons;
this makes it load-bearing rather than merely early.

**L4. The reflog view ships with the first commit-level destructive operation.**
Not later. Fork sells the reflog as "restore lost commits", and a client that can
destroy commits before it can show you the reflog has no recovery story except
"open a terminal" — which contradicts the whole point of the `Confirmed` design.

**L5. Uncommitted work is a separate recovery problem and the staging packet owns
it.** The reflog covers destroyed commits and does nothing for a discarded edit,
so for that class the confirmation dialog is the only barrier that exists.
Whether Cairn auto-stashes before destructive working-tree operations is parked in
the spine's "Still open" and the staging packet must meet it deliberately rather
than inherit a default. Cites: `feature-inventory.md`, "Recovery".

**L6. Speed gets a measured bar per read surface.** D7's milestone is worthless if
it is slow, and "we chose gitoxide" is not a measurement. Every packet landing a
Tier 0 or Tier 1 surface carries an acceptance criterion in the shape of
`history-graph`'s A7: a named real repository, recorded numbers.

**L7. Forge links are in the milestone; forge APIs are not.** Design decision D9.
"Create pull request for this branch" is among the most-used Fork context-menu
commands, so D7's bar is not met without it — and it costs no API token, because it
is URL construction. It lands in packet 6 alongside push, since the PR is what a
user wants immediately after pushing. Rejected: leaving the whole platform surface
out, which was the spine's original position and conflated opening a link with
shipping a panel. Also rejected: pulling CI status in on the same reasoning — it
needs a token, which would make Cairn a credential holder and contradict D2.

## Open, for the packet that meets them

- **O1 (diff-engine).** Side-by-side, unified, or both — and if both, whether the
  patch-construction model is genuinely independent of the presentation. Fork
  advertises side-by-side; unified is what patches are.
- **O2 (refs-and-status).** Whether status comes from `gix-status` or from
  `git status --porcelain=v2`. D1 says reads use gix, but status is unusually
  sensitive to `core.fsmonitor`, sparse checkout and attribute handling — the
  exact divergence class D1 warns about. Measure agreement against `git` before
  committing to gix here.
- **O3 (staging-and-commit).** The auto-stash question from L5.
- **O4 (remote-sync).** Whether pull defaults to merge or rebase, and how visible
  the choice is. It must not be buried in config where a user discovers it by
  being surprised.
- **O5 (worktrees).** Whether Cairn can create a worktree, or only manage existing
  ones. Creating means choosing a path, which is a UI surface; the guard against
  checking out an already-checked-out branch needs only the read side.
- **O6 (remote-sync).** How a self-hosted forge is identified. A config key, a git
  config convention, or a heuristic — and whether push-and-create-PR is one action
  or two. The forge table being data rather than code is settled (L7); how Cairn
  learns which entry applies to a given remote is not.
