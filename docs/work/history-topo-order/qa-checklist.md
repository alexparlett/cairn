# QA checklist — history-topo-order

Packet-specific checks beyond the repo-wide gate and `docs/qa-gate.md`. The acceptance criteria
are in `docs/prd/history-topo-order.md` (C1-C9), not copied here.

1. **git's algorithm, cited.** Every step of the walk cites `revision.c` or `prio-queue.c` at
   v2.30.9 and v2.56.0 (`~/.cache/cairn/git-src/`), and follows GT §5.1. A step that departs from
   git says why, and a test shows that the order is unchanged by it.
2. **The oracle is git's, not Cairn's.** Each parity test runs real git through the oracle
   command of PRD R1 and L5, never a hand-written expected list. The fixtures cover R5.1 in full,
   with and without a commit-graph, and the `git-floor` step runs them.
3. **The bench, untouched.** Every `git` run on `~/Development/bench/rust` uses
   `GIT_OPTIONAL_LOCKS=0`; a commit-graph is written only in a copy, never with alternates. A
   `find <bench>/.git -newer <marker>` before and after is empty and its stderr is shown. The
   memory `bench-is-read-only-even-metadata` applies.
4. **Cancellation.** The opening pass polls between commits. A superseded open and a close stop
   within one commit's work: check the poll is on the opening pass's own loop and not only on
   page production.
5. **The assigner and its oracle are untouched.**
   - `crates/cairn-model/tests/support/layout_before_compaction.rs` is unedited and its
     fingerprint test passes.
   - The skew and equivalence tests still feed out-of-order walks.
   - `CommitTime` and `GraphOrder` are unchanged.
6. **Tests changed by design** say why in their name or body. None is deleted to make room. Each
   rewritten one still fails when the behaviour it pins is broken.
7. **Responsiveness.** Nothing on a render path waits on the history thread. The count updates at
   most once per 100 ms. A filter keystroke during an opening pass is answered in its usual time
   (measured, not asserted only by reading).
