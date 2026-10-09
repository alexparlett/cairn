# Roadmap — daily-loop

Build order for the ten packets that reach D7. Order lives here, never in the
design spine. Each packet gets its own `/feature-plan` run when it starts, writing
its PRD and phases against then-current code — the briefs below carry the design
nuance that run must not lose.

| # | Packet | Status | Depends on |
| --- | --- | --- | --- |
| 1 | `history-graph` | **shipped** | — |
| 2 | `credential-prompts` | **shipped** | 1 |
| 2a | `process-manager` | **shipped** | 2 |
| 3 | `diff-engine` | **shipped** | 1, 2a |
| 4 | `refs-and-status` | **shipped** (merge bar passed and torn down; lands on `main` with its packet PR) | 1, 2a (O2 picked `git status`), 3 |
| 5 | `staging-and-commit` | planned (PRD `docs/prd/staging-and-commit.md`) | 2, 2a, 3, 4 |
| 5b | `stash-and-ignore` | brief only | 5 |
| 6 | `remote-sync` | brief only | 2, 2a, 4 |
| 7 | `branch-ops` | brief only | 2a, 4 |
| 8 | `worktrees` | brief only | 2a, 4 |

2a depends on 2. 3 depends on 2a, and 4 on 1, 2a (O2 sent status to `git`) and
3 (whose working-tree query its Local Changes view opens). Every packet that
spawns `git` depends on 2a. 5b depends on 5, whose local write lane, confirmation
seal and activity popover it builds on. 6, 7 and 8 are independent of each other.
The local write lane is built by 5, which 7 then uses. The critical path to D7 is
1 → 2 → 2a → 3 → 5, with 4 needed before 5. 2a was inserted on 2026-10-02 and 5b on
2026-10-07 (staging-and-commit's L1, L21), each numbered so the other packets keep
theirs.

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

## 3. diff-engine — shipped

`docs/prd/diff-engine.md` (frozen), evidence `docs/research/diff-engine/` — among it
`measured-baseline.md` (git's numbers on the bar's subjects) and `c14-measured.md`
(Cairn's against them). Merge bar passed on 2026-10-04; the work directory was torn
down and the packet lands on `main` as one squash-merged pull request (the user's
decision). As built: `docs/systems/diff.md`; what it changed in how `git` runs and
where a repository is refused at open, `docs/systems/git-processes.md`. **O1 is
closed** (its decision L1): both presentations, unified by default, side-by-side as
one shared setting, over a patch model independent of either.

It ran in two halves. Phases 01 and 02 landed; then the changes query hit rename
parity — gix paired 231 renames where git paired 2,774 on a large rollup, which the
user classed as a critical bug — so the work paused for 2a, and the changes query,
then a file's changed lines, moved onto `git diff-tree` run as reads. Why:
`docs/research/diff-engine/rename-parity-spike.md`,
`docs/research/diff-engine/content-parity-spike.md` and
`docs/research/diff-engine/git-process-survey.md`.

**Built:** the patch-capable model in `cairn-model` (one exact answer, hunk, row
and patch projections, a selection by line identity, the emitter, and an
independent reference applier, round-tripped through real `git apply --cached`);
the engine's changes, content and single-path working-tree queries, each git's own
answer — renames, changed lines, function context, `-w`, the user's algorithm and
filter drivers — with parity to `git diff` pinned under every algorithm, over real
history and on git 2.30 and 2.32 (`git-floor`); per-lane epochs and a diff thread;
the detail pane (Commit and Changes tabs, files opened in place, Expand All under a
line budget, two commits compared tip against tip); the unified and side-by-side
diff view, Fork's bar and gutter, IBM Plex Mono embedded; the accelerator table
(D5) and its guard. Two decisions reach beyond it: D1 is amended so a read runs
`git` where gix's answer differs from git's and a working-tree read runs the clean
filter git runs; and opening refuses a repository wherever the user's own `git`
would — a bare repository found by searching, dubious ownership.

**Leaves on the doorstep:** for 4, the single-path working-tree query and the diff
view, waiting for the changed-path list this packet does not enumerate, and the
Commit tab's place for ref chips; for 5, the emitter and its round-trip tests — a
selection's patch always at three lines of context from the exact diff, never from
a whitespace-ignoring view — and a view with no line selection yet. Left as issues:
#54 (a guard over every verb a read may run), #55 (the system gitconfig a non-`/etc`
git reads), #56 (commit re-encoding beyond what was measured), #57 (Fork for
Windows' tab chords and date padding), #58 (gaps in the gate's required-test pins),
#51, #52, #53; and #29-#37, the deferred diff features, with what this packet built
toward each recorded on #29 and #31-#35.

## 4. refs-and-status — shipped

`docs/prd/refs-and-status.md` (frozen), evidence `docs/research/refs-and-status/` —
among it `measured.md` (C11, C12, C15 and C16's numbers on rust-lang/rust). Merge bar
passed on 2026-10-07; the work directory was torn down and the packet lands on `main`
as one pull request. As built: `docs/systems/refs.md` (refs, upstreams, the stash list,
ahead/behind, the reftable refusal), `docs/systems/status.md` (status as `git status`
answers it), `docs/systems/history-graph.md` (the walk from every ref, compact and slim
rows, labels, stash rows, the title bar, refresh), `docs/systems/sidebar.md`,
`docs/systems/local-changes.md`, and what it added to `docs/systems/diff.md` and
`docs/systems/git-processes.md`. **O2 is closed** (its decision L1): status is `git
status --porcelain=v2 -z` run as a read — gix's status differed from git's on 12 of 38
fixtures, three silently, and starts clean filters outside `process/` — which made it
D1's fourth read git answers, beside the changes, content and working-tree queries; a
stash's changes became the third porcelain read (`git stash show --raw`), accepted by
the user.

**Built:** the refs snapshot under five parity rules (every upstream resolved by hand,
as git's `set_merge` does), ahead/behind as two hidden walks, the reftable refusal at
open; status read by git, twice only where an untracked directory collapsed, with C4's
oracles on git 2.30 and 2.32 (`git-floor`); compact rows whose edges are derived as
drawn, and slim rows in chunked stores (all of rust-lang/rust from every ref in
52.67 MiB, against 1.4 GiB before), with no cap on the history — a deliberate deviation
from Fork; the walk from every ref, each ref Fork's chip and each stash whose base is
walked a row of its own; the title bar's branch and counts; refresh on focus, after a
fetch and on F5/⌘R, reopening only when what the graph draws changed and freeing the old
rows on a worker (closing #52's cause); Fork's sidebar with a filter on a worker and a
find that pages the held walk; and a read-only Local Changes view wired to packet 3's
working-tree query.

**Leaves on the doorstep:** for 5, Local Changes as the view staging adds to (its lists,
the path chosen and its diff, the follow through each refresh) and status as git reports
it; for 6, the generic remote glyph the forge's icon replaces and the ahead/behind counts
push and pull move (#68 marks the commits); for 7 and 8, the refs snapshot, the sidebar's
sections and the find. Left as issues: #62-#82 — listing ignored files, reftable,
`GIT_NAMESPACE`, watching, refreshing a stale index, greying and push/pull markers, a
Submodules section, Local Changes' toolbar, and the measured or reviewed follow-ups — and
#2, #4, #35 and #36, which it built toward.

## 5. staging-and-commit — planned

PRD `docs/prd/staging-and-commit.md` (in flight), work directory
`docs/work/staging-and-commit/` (decisions L1-L21 in its `brainstorm.md`, twelve
phases), evidence `docs/research/staging-and-commit/`. Planned on 2026-10-07.

**Builds:** the local write lane (its own thread, FIFO with a queued state,
operation ids, a write counter that discards a status begun before the latest write
ended, refresh by `Invalidated`, quiet while a commit runs); the confirmation seal
bound to an engine-computed `Consequence` and re-checked before each destructive
operation runs; stage, unstage and discard by file, hunk, line and mode change, in
Fork's routes and gesture, with unstage and discard as inverted diffs through the
forward emitter; commit and amend through Fork's commit box, with hooks, the `Git
Error` dialog and its skip; Show Lost Commits (the reflog view, program L4) with
`Create Branch Here…`; and the activity popover, quoting each confirmed prompt.

**Leaves out:** stash and `.gitignore`, moved to 5b (its L1, L21). **O3 is closed**
(its L2): a discarded uncommitted edit is protected by its confirmation alone, as in
Fork — no backup, no auto-stash, no Trash, no undo. "Clean untracked" is absorbed
into discard (its L8): an untracked row is discarded by deleting it, with the same
dialog, and there is no Clean command. Push, Commit and Push and the forge stay
packet 6's; every other ref operation packet 7's; Tier 4 the second lap's.

**Inherited from packet 4** (as built, `docs/systems/local-changes.md`): the Local
Changes view, read only — Unstaged above Staged, the badges, each path's diff — which
this packet adds stage, discard and the commit box to; and status as `git status`
reports it. Its "the filter field is the only control above the lists" gives way to
Fork's header buttons (its L7).

## 5b. stash-and-ignore — brief

**Builds:** stash create through Fork's dialog (a message, and `Stage new files`,
which stages untracked files so the stash includes them); `Save Snapshot…`, a stash
that leaves the changes in place; apply through Fork's `Apply Stash` dialog, whose
`Delete stash after applying` is the difference between apply and pop; drop, which
is destructive, takes `Confirmed`, and is run by the id the prompt named, checked
against `stash@{n}` immediately before it runs; `Stash N Files…` from the file
context menu; and the context menu's `Ignore ›` submenu — `Ignore '<file>'` at
once, `Ignore All Files in '<folder>'…` and `Custom Pattern…` through Fork's `Add
Pattern to .gitignore` dialog with a live preview of the files that match — writing
the root `.gitignore` only, as Fork does. And, carried from staging-and-commit (the user's
decision 2, 2026-10-09): Create Branch's "Local changes:" gains Fork's "Stash and reapply"
(`docs/research/staging-and-commit/fork-create-branch-evidence.md` §3), which packet 5 left out
until stashing exists.

**Facts the planning research found that it must meet**
(`docs/research/staging-and-commit/git-write-verbs.md` §7 and §9,
`fork-staging-and-commit.md` §4 and §6):

- `git stash push` is not atomic: one push wrote the index six times and committed
  five ref transactions, and a kill during its `reset --hard` left the entry stored,
  the working tree half reset and a stale `index.lock`.
- "No local changes to save" exits 0 and stores nothing, so success is not a stash.
- `git stash drop` takes only `stash@{n}` — an object id is refused — and later
  entries renumber, so a drop by index races any other stash; hence the id check
  above, through the seal's pre-run re-check.
- A dropped stash is in no reflog: Show Lost Commits cannot draw it, and only
  `git fsck` finds it until `gc` prunes it. The drop's prompt must say so.
- An intent-to-add entry in the index makes `git stash push` fail ("Entry not
  uptodate. Cannot merge.") and change nothing.
- `--staged` is absent below git 2.35 (`unknown option`, 129 on 2.30), so a
  staged-only stash degrades on the floor (program memory: the floor stays 2.30);
  Fork has none either.
- A `.gitignore` write is a file write, with no git verb: where it lives in `ops/`,
  and whether D1 is amended for it, is this packet's decision.
- Appending to a `.gitignore` without a final newline joins two patterns into one
  that ignores neither; the write must add the newline first.

**Depends on 5:** the local write lane, the seal and its `Consequence`, and the
activity popover a drop's prompt is quoted in.

## 6. remote-sync — brief

**Builds:** pull and push. Upstream tracking. Remote add, edit and remove.
Force push with `--force-with-lease` as the default and plain `--force` made hard
to reach. Push and delete tags. Prune. **Commit and Push**, Fork's commit-box
gesture, left out of packet 5 to be designed with push: it is this packet's.

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

**Inherited from packet 4** (as built, `docs/systems/history-graph.md`): remote-tracking labels draw a generic
remote glyph where Fork draws the forge's icon; identifying the forge here is
what replaces it.

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
delete tags. Packet 5's Create Branch — Fork's full dialog from `New Branch…` on any commit
row, with "Check out after create" and "Local changes:" (`ops::create_branch`,
`create_branch_and_checkout`, the sealed `create_branch_discarding`) — exists to reuse: create
branch grows from it rather than beside it. Checking out enters packet 5 only through that
dialog; checkout and switch of a branch are this packet's.

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

**Why this is in the first milestone at all:** it is cheap on the read side and
it is the workflow this repository is developed in — `CLAUDE.md` puts every
packet in its own worktree. (Fork does have it: a Worktrees sidebar section since
2023, and an icon on a branch held elsewhere —
`docs/research/refs-and-status/fork-refs-and-status-ui.md`. Cairn's addition is
the disabled checkout.)

---

## After D7 — the second lap, for orientation only

Not planned, not ordered, and not a commitment. Recorded so the packets above do
not accidentally foreclose them: merge with structured conflict resolution (D6),
rebase, cherry-pick, revert, reset, blame, file history, commit search, image
diffs, submodules, LFS verification, the repository manager, and interactive
rebase as its own program; and Cairn's first persisted settings store, whose first user is
Create Branch's "Check out after create", sticky for the session until then (the user's decision
1, 2026-10-09; issue #89 — the filesystem-mutation guard will need an exceptions-roster row for
its file).
