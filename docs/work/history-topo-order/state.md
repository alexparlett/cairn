# State — history-topo-order

The cross-session cheat sheet. Every session updates it before ending.

**Status: planned, 2026-10-17. Not started.** Its base is `main`. It runs before `fork-alignment`.

## Locked decisions (brainstorm.md has the evidence and the rejections)

- **L1. Order.** `git log --date-order`, which is Fork's default. `--topo-order` is a possible
  later setting.
- **L2. The walk.** Cairn's own git date-order walk over `gix_revwalk::Graph`, with ties popping
  in insertion order. Not gix's `Topo`: its DateOrder breaks ties arbitrarily.
- **L3. The opening pass.** Accept it, visibly:
  - "Reading history… N commits";
  - cancellable;
  - the first-page bar is git's own `--date-order` time, plus 10%;
  - the memory rule is restated.
- **L4. The history thread.** The history gets a thread of its own, so filters and retires never
  wait on an open.
- **L5. The oracle.** `git log --date-order --format=%H --branches --remotes --tags HEAD`, with
  tips in a fixed order:
  - stashes grafted onto their base and fed on stdin;
  - Show Lost Commits' reflog ids fed on stdin;
  - identical at git 2.30.9, 2.32.7 and 2.56.0.
- **L6.** git's source is at `~/.cache/cairn/git-src/v2.30.9` and `v2.56.0`, read only.
- **L7.** A reopen keeps the old rows drawn and usable, with a "Reading history… N commits"
  strip, until the new first page replaces them. A first open shows the sentence.
- **L8.** Two corrections:
  - stashes are synthetic one-parent tips of the walk (after `HEAD`, before the reflog tips),
    not rows merged into the stream;
  - the refs read stays on the repository thread, which asks the history thread to reopen,
    superseding a running pass.

## Interfaces introduced so far

None yet.

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 | not started | — | — |
| 02 | not started | — | — |
| 03 | not started | — | — |
| 04 | not started | — | — |
