# State — diff-engine

The cross-session cheat sheet. Every session updates this before ending.

**Status: planned. No phase has started. No code exists for this packet.**

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

None yet. As phases land, record here: the type or function, its crate, and the
one-line contract.

| Symbol | Crate | Contract |
| --- | --- | --- |
| _(none)_ | | |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 diff model | not started | — | — |
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
