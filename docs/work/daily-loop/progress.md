# Progress — daily-loop

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-10-18 — history-topo-order shipped and torn down

Packet 5c passed its merge bar (every C1-C10 met, each test checked to fail under a mutation;
C8 on the bench: the first 64 rows in 1,183 ms against git's 1,257 ms without a commit-graph,
the whole history equal to git's `--date-order` row for row; full gate green). A CI failure on
the integration branch — a worker test reading the opening pass's count where it expected a
page on a slow runner — was fixed in its tests (`Pacing::UNTOLD`). Torn down on
`feature/history-topo-order`: PRD `docs/prd/history-topo-order.md` stamped shipped; as built in
`docs/systems/history-graph.md`; the decisions made in flight, PENDING the user's review, each
phase's QA adjudication and the merge bar's numbers kept in
`docs/research/history-topo-order/packet-record.md`; no new invariant (the packet changed the
routing account and the UI-thread guard's stated residuals in the root `CLAUDE.md`, under their
existing twins); leftovers filed as #139-#141. Not filed, for the user: the gitoxide
replace-refs issue, and whether #128 is closed. Next: 5d, `fork-alignment`.

## 2026-10-17 — history-topo-order planned

Packet 5c planned with `/feature-plan` (PRD `docs/prd/history-topo-order.md`, work dir
`docs/work/history-topo-order/`, evidence `docs/research/history-topo-order/`). The user chose
`git log --date-order` (Fork's default) over `--topo-order`; Cairn's own walk of git's algorithm
over gitoxide's commit graph, since gitoxide's date order breaks ties arbitrarily; to accept the
whole-history opening pass a repository without a commit-graph needs, visibly; and a history
thread of its own. The research found gitoxide 0.87.1 does have a topological walk, which the
history-graph research had said it lacked, and that 14% of the bench's rows are reordered by
repaints today.

## 2026-10-16 — fork-alignment planned; 5c-5h inserted

The UI review the user planned after packet 5 was run as `/feature-plan` with the
`/observe-fork` skill: two first-hand surveys of Fork for Windows 2.23.2 (light and dark)
and one of Cairn's window, over one fixture, plus a code inventory, an audit of the earlier
Fork evidence and a Freya API check (`docs/research/ui-review/`). Packet 5d,
`fork-alignment`, is planned (PRD `docs/prd/fork-alignment.md`, work dir
`docs/work/fork-alignment/`, decisions S1-S5 and L1-L9 in its brainstorm), with
`docs/design/ui.md`, `history-graph.md` and `diff.md` rewritten to its end state. The
surveys found a git-parity bug — a parent drawn above its child when commit times tie —
which the user made its own packet, 5c `history-topo-order`, to run first. The user split
the file tree, Quick Launch, Preferences and the repository manager into packets of their
own, 5e-5h, all briefs. Fork for Windows in the VM is now 2.23.2 and runs its own git
2.50.1, not the 2.21.1 the earlier observations record.

## 2026-10-14 — staging-and-commit shipped and torn down

Packet 5 passed its merge bar (every C1-C35 met, C21 within its bars on three fresh bench
clones; full gate green), after a redesign the user approved on 2026-10-10 that phases
12-20 rebuilt. Torn down on `feature/staging-and-commit`: PRD
`docs/prd/staging-and-commit.md` stamped shipped; as built in `docs/systems/staging.md`,
`local-changes.md`, `git-processes.md` and `history-graph.md`; C21's numbers in
`docs/research/staging-and-commit/measured.md`; the invariants it added graduated into the
root `CLAUDE.md`; leftovers filed as #90-#133, and #41, #46, #47, #48, #66 and #87 commented. O3 stays closed (no backup before a
discard); program L4 is met (Show Lost Commits shipped with amend). 5b's brief was checked
against what the packet left and updated in `roadmap.md`.

## 2026-10-07 — staging-and-commit planned; O3 closed; 5b inserted

Packet 5 planned with `/feature-plan`: PRD `docs/prd/staging-and-commit.md`, work
directory `docs/work/staging-and-commit/` (L1-L21, twelve phases), evidence in seven
records under `docs/research/staging-and-commit/`. O3 closed by its L2: a discarded
uncommitted edit is protected by its confirmation alone, as Fork does it, after a
precedent study weighed a hidden snapshot, a stash per discard, the OS Trash and an
in-app undo. The brief split in two (its L1, L21): stash and `.gitignore` moved to a
new packet 5b, `stash-and-ignore`, which depends on 5, and "clean untracked" was
absorbed into discard. The living design docs were rewritten to the decisions in
the same change.

## 2026-10-07 — refs-and-status shipped and torn down

Packet 4 passed its merge bar (every C1-C16 met; full gate green) and the user accepted
its batched decisions as built. Torn down on `feature/refs-and-status`: PRD
`docs/prd/refs-and-status.md` stamped shipped; as-built in `docs/systems/refs.md`,
`status.md`, `sidebar.md`, `local-changes.md` and `history-graph.md`; measured numbers
in `docs/research/refs-and-status/measured.md`; leftovers filed as #62-#82. O2 stays
closed for `git status`. Next on the critical path: `staging-and-commit`.

## 2026-10-05 — refs-and-status planned; O2 closed

Packet 4 planned with `/feature-plan`: PRD `docs/prd/refs-and-status.md`, work
directory `docs/work/refs-and-status/`, evidence `docs/research/refs-and-status/`.
O2 closed for `git status --porcelain=v2 -z` as a read, on a 38-fixture agreement
spike (gix differed on 12, three silently, and runs clean filters outside
`process/`). The packet now depends on 2a. The Fork study corrected two program
records: Fork has had a worktree sidebar since 2023, and draws no working-tree
row in the graph, so Cairn draws none.

## 2026-09-14 — out-of-scope list reviewed against real usage and confirmed

Walked the remaining out-of-scope entries with the user after D9 moved one of them
into scope. No further changes: platform panels, issues, CI status, repository
creation, being an editor, git-flow, a tutorial, Windows, bisect and filter-repo
all hold.

Worth recording because it changes the list's status. These were my assertions
from a design principle; they are now checked against what the user actually does
in a day, which is how D6 and D9 were both found to be wrong. Reopening one should
need new information rather than a fresh opinion.

## 2026-09-14 — forge links pulled into scope; the spine's line was wrong

The user reported that "create pull request on origin" is one of their most-used
Fork context-menu commands. The spine had ruled the whole platform surface out on
the grounds that a half-implemented GitHub panel is worse than a link — which
conflated two unrelated things. Creating a pull request needs no API and no token;
it is URL construction from the branch, its upstream and the remote URL.

Recorded as D9, with the line redrawn mechanically: anything that is "open the
correct forge URL" is in scope, anything needing an API token is out. That brings a
family of siblings nearly free — open a commit, branch, tag or file in the browser,
copy a permalink to a selected line, open a compare view.

CI status stays out and now has a better reason than "expensive": it needs a
per-forge token, which would make Cairn a credential holder, and D2's premise is
that it never is. Inconsistent with a decision already taken, not just costly.

Lands in packet 6 alongside push, because the pull request is what a user wants
immediately after pushing a branch. New open question O6: how a self-hosted forge
is identified, and whether push-and-create-PR is one action or two.

## 2026-09-14 — program planned from a feature analysis

Inventoried the full feature surface against Fork's published feature list
(`docs/design/feature-inventory.md`, ~69 items) and turned it into a build order.
Three decisions came out of it and went into the spine as D6, D7 and D8.

D6 revised something the spine had wrong. It said conflict resolution would open
the user's editor; Fork headlines a built-in resolver, and delegating is worst at
the moment users most need help. The middle position — structured per-region
resolution, no free-text editing — keeps "not an editor" while getting most of the
value, and it was not considered the first time.

Two structural findings shaped the sequence more than the feature list did. The
diff model has nine consumers and must be patch-capable from the start, or
line-level staging becomes a rewrite (L2). And recovery splits into two classes
with different stories: the reflog covers destroyed commits, and nothing covers a
discarded uncommitted edit — which is why the reflog view is pinned to the first
commit-level destructive operation (L4) and why the auto-stash question exists at
all (L5).

Also settled: `credential-prompts` is load-bearing for staging rather than just
early, because `git apply --cached` runs through the backend it lands (L3).

No implementation has started. Two packets are filed; six exist as briefs.
