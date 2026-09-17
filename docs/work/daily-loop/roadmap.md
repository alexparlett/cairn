# Roadmap — daily-loop

Build order for the eight packets that reach D7. Order lives here, never in the
design spine. Each packet gets its own `/feature-plan` run when it starts, writing
its PRD and phases against then-current code — the briefs below carry the design
nuance that run must not lose.

| # | Packet | Status | Depends on |
| --- | --- | --- | --- |
| 1 | `history-graph` | **shipped** | — |
| 2 | `credential-prompts` | **shipped** | 1 |
| 3 | `diff-engine` | brief only | 1 |
| 4 | `refs-and-status` | brief only | 1 |
| 5 | `staging-and-commit` | brief only | 2, 3, 4 |
| 6 | `remote-sync` | brief only | 2, 4 |
| 7 | `branch-ops` | brief only | 4 |
| 8 | `worktrees` | brief only | 4 |

3 and 4 are independent of each other and of 2. 6, 7 and 8 are independent of
each other. The critical path to D7 is 1 → 2 → 3 → 5, with 4 needed before 5.

---

## 1. history-graph — shipped

`docs/prd/history-graph.md` (frozen). Landed repository opening (R5), the worker
boundary (D3), the lane assigner (D4) and the virtualised graph view. It ran
first because the dependency on `credential-prompts` is one-directional and it
exercises the gix read path that D1 rests on — which held: D1's read path is
proven, and packets 3, 4 and 5 inherit the worker boundary rather than building
one. As built: `docs/systems/history-graph.md`.

## 2. credential-prompts — shipped

`docs/prd/credential-prompts.md` (frozen). Landed the `git` subprocess backend
(D1), the askpass helper (D2) and fetch, with the cache-invalidation contract
that D1 created written into the `ops` module docs. **Load-bearing for packet
5**, not merely early: staging calls `git apply --cached` through this backend.
Left for packet 6 by name: push (issue #16), the remote picker (#23), and the
fetch-under-prune policy (#17). As built: `docs/systems/credentials.md`.

## 3. diff-engine — brief

**Builds:** the diff model and its rendering. Commit diffs, working-tree diffs,
and comparing two arbitrary revisions. The commit details pane. Diff options:
whitespace handling, word-level intra-line diff, context lines, rename detection.

**The thing this packet must not get wrong (L2):** the model is
**patch-capable**, not display-shaped. It must emit a valid patch for an arbitrary
subset of hunks and lines — correct hunk headers, correct context — because that
is how packet 5 stages a single line. Build the patch emitter and its round-trip
test in this packet even though nothing consumes it yet; that test is the
difference between packet 5 being a feature and being a rewrite.

**Nine consumers to design against**, only two of which exist here: commit
details, working-tree changes, hunk staging, line staging, compare revisions,
conflict resolution (D6), image diffs, stash contents, interactive-rebase preview.

**Open:** O1 — side-by-side, unified, or both, and whether the patch model is
genuinely independent of presentation.

**Carries a measured bar (L6):** diffing a large commit in a large repository.

**Out:** image diffs (Tier 1, later), conflict resolution (D6, second lap),
staging of any kind.

## 4. refs-and-status — brief

**Builds:** refs enumeration (branches, remotes, tags, stashes) and working-tree
status (changed, staged, untracked, ignored, conflicted). Ref decoration on the
graph — the thing that makes the graph readable rather than a list of hashes.
Stashes shown inline in the commit list, the way Fork does it, rather than in a
side panel. The graph walks every ref by default — branches, remotes and tags —
as Fork's "All Commits" view does. Today the application walks from `HEAD` only
(`HistoryRequest::from_head` in `crates/cairn-app/src/worker/pool.rs`), so a
branch not reachable from the checkout never appears; this packet switches it to
`HistoryRequest::from_commits` over the enumerated refs.

**Open:** O2 — `gix-status` or `git status --porcelain=v2`. D1 says reads use gix,
but status is unusually exposed to `core.fsmonitor`, sparse checkout and
attributes, which is precisely the divergence class D1 warns about. Measure
agreement against `git` on a repository configured for each before committing.

**Carries a measured bar (L6):** status on a large, dirty working tree. This is
the query a client runs most often and the one most likely to feel slow.

**Out:** acting on any ref (packet 7), staging (packet 5).

## 5. staging-and-commit — brief

**Builds:** stage and unstage by file, hunk and line. Discard by file, hunk and
line. Clean untracked files. Commit and amend. `.gitignore` editing. Stash
create, apply, pop and drop.

**This is the first packet with destructive operations**, so it carries the debts
the architecture has been saving up:

- Every destructive operation takes `Confirmed` and is reviewed by
  `destructive-ops-reviewer`. This is the packet where that reviewer stops being
  theoretical.
- **The reflog view ships here (L4)** — amend and stash drop destroy committed
  work, and a client that can do that before it can show you the reflog has no
  recovery story but the terminal.
- **The operation log becomes visible here.** `ops::Performed` already records the
  prompt the user accepted; this is where a user can read it back.
- **O3, the auto-stash question (L5).** The reflog does nothing for a discarded
  uncommitted edit — there is no recovery for that class at all, which makes the
  confirmation dialog the only barrier. Decide deliberately whether Cairn
  auto-stashes first. Do not inherit a default.

**Depends on packet 3 for patch construction and packet 2 for the backend** —
`git apply --cached` is how a partial stage happens.

**Out:** merge, rebase, cherry-pick, revert, reset (all Tier 4, second lap).

## 6. remote-sync — brief

**Builds:** pull and push. Upstream tracking. Remote add, edit and remove.
Force push with `--force-with-lease` as the default and plain `--force` made hard
to reach. Push and delete tags. Prune.

**Also builds the forge links (D9), and they are not a footnote.** "Create pull
request for this branch" is one of the most-used context-menu commands in Fork, so
D7's milestone is not met without it. The group is one mechanism: read the branch
and its upstream, read the remote URL, identify the forge, construct a URL, open
it. No API token, no network call from Cairn. Siblings that come nearly free:
open a commit / branch / tag / file in the browser, copy a permalink to a selected
line, open a compare view between two refs.

Design it **with** push rather than beside it: the action a user wants immediately
after pushing a branch is the pull request, so push-and-create-PR as one gesture is
the flow to get right. Keep the forge table as DATA so adding a forge is an entry;
give an unrecognised remote **no menu item** rather than a guessed URL — github.com
and gitlab.com are identifiable by hostname, self-hosted GitLab, Gitea and Forgejo
are not.

**Open:** O4 — whether pull defaults to merge or rebase, and how visible that
choice is. It must not live only in config, where a user finds it by being
surprised.

**Destructive members:** force push, prune, delete remote tag. All take
`Confirmed`, and force push is the operation whose prompt
`destructive-ops-reviewer` will read hardest — "Force-push to origin/main?" does
not say that someone else's three commits become unreachable.

**Out:** creating or deleting repositories on a platform, *reviewing* pull
requests, issues, and CI status — all API-token work, which D9 puts on the far side
of the line. CI status specifically would make Cairn a credential holder and
contradict D2.

## 7. branch-ops — brief

**Builds:** create, rename and delete branches. Checkout and switch. Create and
delete tags.

**The non-obvious destructive case:** checkout is destructive exactly when the
working tree is dirty, which is the moment a user least expects a checkout to
cost them anything. That prompt is the one to write carefully.

**Interacts with packet 8:** refusing to check out a branch already checked out in
another worktree needs worktree awareness. Whichever of 7 and 8 lands second owns
the interaction.

**Out:** anything in Tier 4.

## 8. worktrees — brief

**Builds:** list worktrees, show which one holds a given branch, switch between
them, remove them. Decision D8 makes this first-class rather than incidental.

**The guard that matters even read-only:** a client unaware of worktrees offers to
check out a branch that is already checked out elsewhere, then fails confusingly.

**Open:** O5 — whether Cairn creates worktrees, or only manages existing ones.
Creating means choosing a path, which is a UI surface; the guard above needs only
the read side.

**Why this is in the first milestone at all**, when Fork does not have it: it is
cheap on the read side and it is the workflow this repository is developed in —
`CLAUDE.md` puts every packet in its own worktree.

---

## After D7 — the second lap, for orientation only

Not planned, not ordered, and not a commitment. Recorded so the packets above do
not accidentally foreclose them: merge with structured conflict resolution (D6),
rebase, cherry-pick, revert, reset, blame, file history, commit search, image
diffs, submodules, LFS verification, the repository manager, and interactive
rebase as its own program.
