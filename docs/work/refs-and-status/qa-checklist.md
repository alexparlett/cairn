# QA checklist — refs-and-status

Packet-specific acceptance beyond the repo-wide gate. The acceptance criteria
C1-C16 live in `docs/prd/refs-and-status.md` and are NOT copied here — verify
them there, against their pinned tests.

## Per-phase coverage of the PRD criteria

| Phase | PRD criteria it must satisfy |
| --- | --- |
| 01 | C1, C2, C3, and C11's refs numbers |
| 02 | C4, C5, C13, and C11's status numbers |
| 03 | C15 |
| 04 | C16 |
| 05 | C6, and C11's first-page numbers |
| 06 | C10 |
| 07 | C7 |
| 08 | C8 |
| 09 | C9, C12, and C11 complete |
| 10 | all of C1-C16, re-verified over the whole packet diff |

## Packet-specific checks

Beyond the PRD, phase 10 confirms:

- [ ] **Every parity test's oracle is git.** C1, C3, C4 and C6 compare against
      what real `git` prints in the same fixture, never against a golden file or
      against Cairn's own earlier output. A test whose expectation was pasted from
      a run of Cairn proves nothing.
- [ ] **No read writes.** The index is byte-identical after every refs, status and
      ahead/behind read; nothing calls `Outcome::write_changes`; `reads::status`
      is built with `read_invocation`.
- [ ] **`reads::status`'s argv is exactly what L1/L2 name**: no `--ignored`, no
      rename or submodule override, `--untracked-files` only as L2 says. Read the
      code, then a stub-git test that prints its argv.
- [ ] **No symbolic ref is peeled into its target's name**, anywhere the
      snapshot is built or consumed (the network lane's former `ref_tips` caller
      included).
- [ ] **A stash's index and untracked commits never become rows**, and a stash
      row's only edge is to its first parent — checked on a stash made with
      `--include-untracked`; a stash whose base no ref reaches has no row and
      pulls no commit into the graph.
- [ ] **No edge was lost to compact rows**: the equivalence test compares the
      derived edges of every row against what today's assigner retains, over
      fixtures, the Cairn checkout and the bench repository, including rows
      the assigner repainted late.
- [ ] **A deep find retains only compact, slim rows** (PRD R8.6, C16), measured
      on the bench repository; no retained row owns a heap allocation.
- [ ] **Every row draws what it drew before** the rows were slimmed — subject,
      author, date, short id, merge marker — read through the history's stores.
- [ ] **Every new `RowContent` reader names every variant**; no wildcard was
      added to satisfy the compiler.
- [ ] **Every new list is virtualized** and has its viewport twin; the
      `ScrollView` exceptions roster is still empty.
- [ ] **The refresh never waits on the UI thread**: the focus subscription and the
      Refresh action only submit.
- [ ] **A refresh that changes nothing does not reopen the history** — and one
      that changes only the stash list, or only `HEAD`'s branch, does.
- [ ] **A slow status queues neither a page nor a diff**, and a reopen frees its
      old rows off the UI thread (#52).
- [ ] **Each Fork deviation is the one the PRD names** (the generic remote
      glyph, no working-tree row) and no other crept in; each label kind is told
      apart without colour.
- [ ] **Every out-of-scope item in the PRD is filed** as an issue at teardown.
