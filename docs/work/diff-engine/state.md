# State — diff-engine

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 02 landed. The diff model exists in `cairn-model` and
`cairn-git` answers R2's two queries against a real repository; no component,
nothing on a worker, nothing that draws.**

## Locked decisions

L1-L16 in `brainstorm.md`; the design frame is D1, D3, D5 and D6 in
`docs/design/cairn.md`. The ones that most constrain implementation:

- **The model holds one exact answer** — both versions' lines and the exact
  changed ranges — and hunks, rows and patches are pure projections of it (L2).
  A changed line is identified by its line number on its own side, so a selection
  is presentation-independent.
- **gix computes the diff; Cairn groups it** (L3). No diff algorithm is written
  here, and gix types stop at the seam.
- **The patch always carries three lines of context**, whatever the view shows,
  and the emitter can never see the whitespace-ignoring ranges (L2, L4).
- **A working-tree read may run the user's filter driver** (L6). D1 is amended for
  it; the driver's inherited environment is a stated residual, not an oversight.
- **Queries are numbered per lane** (L8): history, changes, file diff. A changes
  query also supersedes the file-diff lane. Nothing else supersedes across lanes.
- **The layout is Fork's** (L9), down to the context buttons and the hunk header
  with no buttons on it. Where Cairn's own mockup disagrees, Fork wins and the
  mockup is superseded.
- **No per-file line counts anywhere** (L10). Fork shows none, and git spends
  407 ms counting them on the largest subject.

## Open questions

Q1-Q3 in `brainstorm.md`, lettered Q so they cannot be confused with the
program's O1-O6.

**Q3 is answered, and the answer is a large gap that is with the user.** On
`5a3292f163d` gix finds 231 rename pairs where git finds 2,774 — every one of
git's 2,543 inexact renames is missing — because gix compares `diff.renameLimit`
against the raw permutation count where git compares it against the square, and
because gix has no basename stage in front of its exhaustive one. Neither is
closable by raising the limit Cairn passes. The measurement, the cause and the
options are in the phase 02 entry of `progress.md`.

## New modules and interfaces introduced so far

Recorded as phases land: the type or function, its crate, and the one-line
contract.

Phase 01's are all in `cairn-model` and all pure: no I/O, no clock, no
dependency past the crate's allowlist. Phase 02's are in `cairn-git`, read a real
repository, and speak `cairn-model` at the boundary — no gix type reaches a
public signature. As-built prose for both: `docs/systems/diff.md`.

| Symbol | Crate | Contract |
| --- | --- | --- |
| `RepoPath` | model | A repository-relative path in the bytes git stores; text is a lossy reading of them. |
| `FileMode` | model | Regular, executable, symlink or submodule, with the six octal digits a patch header spells. |
| `Similarity` | model | A rename's or a copy's `similarity index N%`, as a percentage that stops at a hundred. |
| `ChangeStatus` | model | Added, deleted, modified, type-changed, renamed or copied. |
| `ChangedFile` | model | One path's row in a change set: status, both paths, both modes, both ids. No line counts (L10). |
| `LineNumber` | model | A line's place on its own side, stored from zero, with `one_based()` for a gutter or a header. |
| `LineSpan` | model | A run of lines on one side; an empty span still says where it sits. |
| `DiffLine` | model | One line's bytes without its terminator, plus whether it had one. |
| `ChangedRange` | model | One contiguous difference: the lines removed and the lines added. |
| `TextDiff` | model | The one exact answer (L2): both versions as lines and the exact changed ranges. What the emitter takes, and it has no room for the whitespace-ignoring ranges. |
| `split_lines` | model | Splits content into lines the way git's diff does, keeping a last line that never ended. |
| `ByteRange` | model | A half-open run of bytes inside one line. |
| `IntraLineHighlight` | model | Where a paired removed and added line differ inside themselves. |
| `DisplayOverlay` | model | Display-only: the whitespace-ignoring ranges and the intra-line highlights. Beside `TextDiff`, never inside it (R1.7). |
| `DiffLimits` | model | R2.6's ceilings: 1 MiB, 50,000 lines, 2,048 bytes in a line, 64 MiB to load anyway. |
| `SizeLimit` | model | Which ceiling a file crossed, and what it measured. |
| `DiffContent` | model | Text, or one of the seven states that stand in place of rows. |
| `FileDiff` | model | A `ChangedFile` and what its change turned out to be. |
| `Selection` | model | A set of changed lines, each named by its number on its own side (R1.3). |
| `Context` | model | A line count (zero raised to one) or the entire file. |
| `Hunk`, `HunkHeader`, `Hunks` | model | The changed ranges grouped at a context, merging within twice it, and the header text git spells. |
| `UnifiedRow`, `UnifiedRows` | model | A unified view's rows, reached one at a time; the index counts changes, not rows. |
| `SideBySideRow`, `SideBySideRows` | model | The same, paired, with filler on the shorter side. |
| `Patch`, `emit_patch`, `PATCH_CONTEXT` | model | The unified patch a selection makes, always at three lines of context. |
| `apply_patch`, `apply_patch_in_reverse`, `PatchApplyError` | model | The reference applier, written from the format and independent of the emitter. Public so `cairn-git`'s round-trip tests can reach it. |
| `Timestamp`, `Signature`, `CommitDetails` | model | R1.8: both signatures with their offsets, the whole message, the parents. |
| `ChangesRequest` | git | What to compare: one commit against its first parent (the empty tree for a root), or two commits tip against tip. |
| `ChangeSet` | git | What a commit or a comparison changed: the files sorted by a total key, the commit's details when one commit was named, and how rename detection went. |
| `RenameDetection` | git | gix's rename and copy counters as plain numbers, and `was_cut_short()`, which is R2.2's "the answer says so". |
| `ContentOptions` | git | R2.6's limits, whether to load past them anyway, and whether to compute the whitespace-ignoring ranges. |
| `DiffSession` | git | Holds gix's resource cache for a run of queries. Borrows the repository and is not `Send`, like `HistorySession`. |
| `Repository::changes` | git | R2.1, R2.2, R2.9, R2.10, on a session of its own. Polls `Cancel` once per change. |
| `Repository::file_diff` | git | R2.3 through R2.8, on a session of its own. |
| `Repository::commit_details` | git | One commit in the detail R5.3 draws, without a changes query. |
| `Error::ChangesCancelled`, `DiffSetup`, `TreeDiff`, `DiffFile` | git | What the caller of a diff query must handle. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 diff model | landed | `scripts/gate.sh` PASS | `qa-checklist`, `test-coverage-auditor` and `responsiveness-reviewer`, adjudicated by `qa-confirm`; confirmed findings fixed or recorded as residuals in `docs/systems/diff.md` |
| 02 engine, commits | landed | `scripts/gate.sh` PASS | packet-mode: the orchestrator runs QA over the phase diff |
| 03 engine, working tree | not started | — | — |
| 04 worker lanes | not started | — | — |
| 05 detail pane | not started | — | — |
| 06 unified diff view | not started | — | — |
| 07 Changes tab and side-by-side | not started | — | — |
| 08 expansion and compare | not started | — | — |
| 09 QA | not started | — | — |

## Environment notes

- **The bench repository is a clone of rust-lang/rust at `c999cef531e` in
  `~/Development/bench/rust`**, made for this packet's measured bar (340,228
  commits, 62,892 index entries). The harness reads `CAIRN_BENCH_REPO`. Read it only:
  no `gc`, no `repack`, no config writes, or the recorded numbers stop comparing.
- `freya` 0.5-rc and `gix` 0.87.1 are pre-1.0. Verify APIs against the source
  that is actually linked, never from memory, and mind which copy that is:
  `gix` and `gix-imara-diff` are under `~/.cargo/registry/src/`, but **freya is a
  git dependency on a fork**, vendored under `~/.cargo/git/checkouts/freya-*/`
  at the rev the workspace pins. The `freya-0.5.0-rc.6` copy in the registry is a
  different version and is not what builds.
  `docs/research/diff-engine/gix-diff-api.md` and
  `docs/research/diff-engine/ui-and-app-as-built.md` record what was verified, in
  which copy, and when.
- `scripts/gate.sh` is the bar. Never an ad-hoc `&&` chain, never piped through
  `tail`.
- Commit explicit paths, never `git add -A`.
- A new dependency, crate or invariant needs its row in
  `crates/cairn-guards/tests/invariants.rs` in the same commit, or the gate fails.
- **Phase 06 needs a font file that is not in the repository yet.** Downloading it
  is the user's call: ask, with the file, its source and its size, and do not
  fetch it unprompted.
- The remote is `github.com/alexparlett/cairn`. Issues and pull requests go there;
  every pull request is merged by the user, never by a session.
