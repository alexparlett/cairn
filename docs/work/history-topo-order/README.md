# history-topo-order

Draws the history in `git log --date-order`'s order, which is also Fork's default. No commit is
drawn above any of its children; otherwise commits are newest first, with ties in git's order.

It writes git's walk in `cairn-git` over gitoxide's commit graph, and makes it the history's order.
The history moves to a thread of its own, so the opening pass a repository without a commit-graph
needs never holds up anything else.

Spec: `docs/prd/history-topo-order.md` (the one copy of the acceptance criteria). Design:
`docs/design/history-graph.md`, `docs/design/concurrency.md`. Evidence:
`docs/research/history-topo-order/`. Base: `main`. Integration branch: `feature/history-topo-order`.
It runs before `fork-alignment` (5d), which depends on it.

| Phase | File | Builds | Criteria |
| --- | --- | --- | --- |
| 01 | `phase-01-the-walk.md` | git's date-order walk over `gix_revwalk::Graph` as `HistoryOrder::DateOrder`, stashes as synthetic tips, the commit-graph used only where git uses it, cancellable and counting, pinned against git on every fixture at every supported git, its opening pass measured on the bench | C1-C3, C10, C8 (opening pass) |
| 02 | `phase-02-into-the-history.md` | the history's order everywhere: refs, stashes, Show Lost Commits, sessions, the cold path, finds; the stash merge and reach correction retired; the tests that change with the order | C4, C5 |
| 03 | `phase-03-thread-and-window.md` | the history thread (the refs read staying on the repository thread), a reopen keeping the old rows under "Reading history… N commits", supersede and close, the bench measurements and the systems docs | C6-C9 |
| 04 | `phase-04-qa.md` | the merge bar: whole-packet `/qa`, every criterion, the dismissal audit, teardown | all |
