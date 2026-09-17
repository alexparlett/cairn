# State — diff-engine

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 01 landed. The diff model exists in `cairn-model`; no engine
query, no component, nothing that reads a repository.**

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
program's O1-O6. Q3 (how far gix's rename detection is from git's on the largest
rollup) is the one that can reach an acceptance criterion: phase 02 measures it,
files each gap, and takes a large gap to the user.

## New modules and interfaces introduced so far

Recorded as phases land: the type or function, its crate, and the one-line
contract.

All of phase 01's are in `cairn-model` and all are pure: no I/O, no clock, no
dependency past the crate's allowlist. As-built prose: `docs/systems/diff.md`.

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

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 diff model | landed | `scripts/gate.sh` PASS | `qa-checklist`, `test-coverage-auditor` and `responsiveness-reviewer`, adjudicated by `qa-confirm`; confirmed findings fixed or recorded as residuals in `docs/systems/diff.md` |
| 02 engine, commits | not started | — | — |
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
