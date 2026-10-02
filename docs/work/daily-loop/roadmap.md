# Roadmap — daily-loop

Build order for the nine packets that reach D7. Order lives here, never in the
design spine. Each packet gets its own `/feature-plan` run when it starts, writing
its PRD and phases against then-current code — the briefs below carry the design
nuance that run must not lose.

| # | Packet | Status | Depends on |
| --- | --- | --- | --- |
| 1 | `history-graph` | **shipped** | — |
| 2 | `credential-prompts` | **shipped** | 1 |
| 2a | `process-manager` | **shipped** | 2 |
| 3 | `diff-engine` | **in flight**, phases 01-02 landed; unblocked by 2a | 1, 2a |
| 4 | `refs-and-status` | brief only | 1 (and 2a if O2 picks `git status`) |
| 5 | `staging-and-commit` | brief only | 2, 2a, 3, 4 |
| 6 | `remote-sync` | brief only | 2, 2a, 4 |
| 7 | `branch-ops` | brief only | 2a, 4 |
| 8 | `worktrees` | brief only | 2a, 4 |

2a depends on 2. 3 depends on 2a, and 4 depends on neither, unless O2 sends
status to `git`. Every packet that spawns `git` depends on 2a. 6, 7 and 8 are
independent of each other. The local write lane is built by whichever of 5 and 7
lands first. The critical path to D7 is 1 → 2 → 2a → 3 → 5, with 4 needed before 5.
2a was inserted on 2026-10-02 and numbered so the other packets keep theirs.

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

## 2a. process-manager — shipped

`docs/prd/process-manager.md` (frozen), evidence `docs/research/process-manager/`.
Inserted while `diff-engine` was in flight, when its changes query hit rename parity: gix paired 231 renames where git
paired 2,774 on a large rollup, the user classed that as a critical bug, and the
changes query moved to `git diff-tree` — the first read `git` answers (D1, as
rewritten). The user asked for a proper manager for `git` processes before any
new spawn, and the runner credential-prompts built was shaped around fetch alone
(`docs/research/diff-engine/git-process-survey.md`). As built:
`docs/systems/git-processes.md`.

**Built:** one place that builds a process (`process/`), a read/write split
sealed by a token only `ops/` can make, a `reads/` module, a runner carrying
every daily-loop shape (captured cancellable stdout, stdin, bounded stderr,
process-group kill by poll, handle or drop), a registry with kill-all on close,
discovery once at startup, the network write lane with duplicate refusal, and a
command log as data. Design: `docs/design/processes.md`; the write lanes in
`concurrency.md`.

**Leaves on the doorstep:** for 3, a read invocation the diff thread can run and
cancel by epoch (proven against what shipped by `diff_engine_path_forward` in
`crates/cairn-git/src/reads/mod.rs`); for 5, stdin, the local write lane's
design, and the write-verb policies it must decide before its first verb
(issue #45, with #44 on lock lists); for 6, the network lane push joins.
Left as issues: #41-#49, and #25's remainder.

## 3. diff-engine — in flight

**In flight on `feature/diff-engine`: phases 01 and 02 landed, and the work
switched to 2a because the changes query needs a `git` process. 2a has
shipped, so it is unblocked:** `feature/diff-engine` is brought up to date with
`main` first (a shared branch, so the user's call), then the changes query is
reworked onto `git diff-tree` through `reads/`; the model, the content query and
the round-trip tests stand. Why: that branch's
`docs/work/diff-engine/progress.md` (2026-09-30) and
`docs/research/diff-engine/rename-parity-spike.md`.

`docs/prd/diff-engine.md` (in flight), work directory `docs/work/diff-engine/`,
evidence `docs/research/diff-engine/` (six records). Planned 2026-09-17 in nine
phases. **O1 is closed** (decision L1 there): both presentations, unified by
default, side-by-side as one shared setting, and Fork's context controls — and
the patch model is independent of either, which was the half of O1 that mattered.
Two decisions reach beyond this packet: D1 is amended so a working-tree read may
run the user's clean filter driver, and queries are numbered per lane so a scroll
and a diff stop cancelling each other. The brief it was planned from follows.

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

**Open:** ~~O1~~ — closed by the packet's L1, above: both, unified by default,
over a patch model that is independent of either.

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

**Inherited from packet 3, once it lands:** the single-path working-tree diff
query and the diff view itself. This packet owns the changed-path enumeration
that neither of them has, so whichever of 4 and 5 lands first wires the Local
Changes list to them. Note for O2: gix's status runs the user's clean filter
driver when it hashes a working-tree file, exactly as packet 3's diff does, so a
filtered path belongs in the agreement measurement rather than beside it.

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
`git apply --cached` is how a partial stage happens. Three things packet 3 will
leave on the doorstep. Its patch emitter and round-trip tests are built for this
packet to consume, and a patch is always emitted at three lines of context from
the exact diff, never from a whitespace-ignoring view, which is what Fork gets
wrong. `git apply --cached`
feeds its patch on stdin, which the runner gains in packet 2a; this packet adds
the local write lane it designs, and `destructive-ops-reviewer` reviews both. And
the staging affordance is undecided on purpose: Fork floats Stage and Discard
over a hovered chunk and narrows them by drag-selection — Discard on unstaged
chunks only, since Fork refuses to discard staged changes by design — while
Cairn's mockup shows header actions and a selection gutter
(`docs/research/diff-engine/fork-detail-and-diff-ui.md`).

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
