# State — diff-engine

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 02's rework landed, its QA findings are fixed and the C6 audit is
done (2026-10-03), except F1: gix's hunks diverge from `git diff`'s, a spike is
measuring what moving the content query's ranges to git costs, and the user decides
after it. Phase 03 next.** The diff model exists in `cairn-model`,
and `cairn-git` answers R2's two queries: the changes query from `git diff-tree`
through the process manager (decision E, PRD R2.1, R2.2, R2.9 and C14 amended),
honouring `diff.ignoreSubmodules` and `log.showRoot` as the user's `git log` does,
the content query from gix. The full gate's `git-floor` step, and CI's `git floor`
job, run the diff tests on git 2.30 and 2.32 built from source. Why decision E, and the evidence: the 2026-09-30 and
2026-10-03 entries in `progress.md`,
`docs/research/diff-engine/rename-parity-spike.md` and
`docs/research/diff-engine/git-process-survey.md`.

## Locked decisions

L1-L16 in `brainstorm.md`; the design frame is `docs/design/diff.md`,
with D1, D3, D5 and D6 in `engine.md`, `concurrency.md`, `platform.md` and
`conflicts.md` beside it. The ones that most constrain implementation:

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

**For phases 04 and 05: "cut short" has no `cairn-model` type yet.** `ChangeSet`
and `RenameDetection` are `cairn-git` types, and `cairn-ui` may not name
`cairn-git`. Whatever crosses the seam to draw R2.2's notice — that the rename
search was cut short, and the `needed_limit` to raise `diff.renameLimit` to — needs
a `cairn-model` counterpart, which the phase that carries a change set across the
worker boundary (04) or draws it (05) must decide.

**Q3 is answered and closed by decision E.** gix paired 231 renames on
`5a3292f163d` where git pairs 2,774; the changes query now asks git, and the bench
reporter requires every one of M1's 2,774 pairs to equal `git diff-tree`'s.

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
| `RenameDetection` | git | Whether detection was on and found copies, the limit git applied, and `needed_limit` / `was_cut_short()`: R2.2's "the answer says so", decided from git's answer and the limit, never from stderr. |
| `ContentOptions` | git | R2.6's limits, whether to load past them anyway, and whether to compute the whitespace-ignoring ranges. |
| `DiffSession` | git | Holds gix's resource cache for a run of content queries; its `changes` is `Repository::changes`. Borrows the repository and is not `Send`, like `HistorySession`. |
| `Repository::changes(&GitBinary, ..)` | git | R2.1, R2.2, R2.9, R2.10: gix reads the commits and the configuration — the two rename keys, `diff.ignoreSubmodules`, `log.showRoot` — and `git diff-tree` answers. Blocks on one read process (two when a hidden submodule must be excluded from a rename search); `Cancel` polled every runner tick ends it. A root commit under `log.showRoot=false` answers no files and starts no process. |
| `reads::changes`, `reads::Detection`, `reads::Submodules` | git (crate-private) | The read: `git diff-tree -r -z --raw --no-abbrev` with detection spelled out, the submodules to leave out (`--ignore-submodules=all`, or `:(exclude,literal)` pathspecs), `-z` records parsed into `ChangedFile`s, a superseded query answering `ChangesCancelled`. |
| `diff::renames` (`Configured`, `Search`) | git (crate-private) | `diff.renames` and `diff.renameLimit` parsed by git's rules, the limit the git in use applies, and the cut-short inference per git version. |
| `diff::submodules` (`Hiding`) | git (crate-private) | What `diff.ignoreSubmodules` and each submodule's own `ignore` hide from the user's `git log`, with `.gitmodules` read where git reads it. |
| `diff::git_config` | git (crate-private) | The last value of a key across the configuration, `git_config_bool`, `git_parse_int`. |
| `GitCommand::in_repository` | git (crate-private, `process/`) | Now names the repository to git — `--git-dir` and `--work-tree` ahead of the verb — for a repository gix trusts fully; one it trusts less is left to git's discovery, so `safe.directory` still decides it. |
| `Repository::file_diff` | git | R2.3 through R2.8, on a session of its own. |
| `Repository::commit_details` | git | One commit in the detail R5.3 draws, without a changes query. |
| `Error::ChangesCancelled`, `DiffSetup`, `DiffFile`, `UnexpectedGitOutput`, `InvalidConfig` | git | What the caller of a diff query must handle; `TreeDiff` went with the gix tree walk. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 diff model | landed | `scripts/gate.sh` PASS | `qa-checklist`, `test-coverage-auditor` and `responsiveness-reviewer`, adjudicated by `qa-confirm`; confirmed findings fixed or recorded as residuals in `docs/systems/diff.md` |
| 02 engine, commits | landed 2026-09-18; changes query reworked onto `git diff-tree` 2026-10-03 (decision E) | `scripts/gate.sh` PASS, `git-floor` included | done over the reworked phase (2026-10-03), adjudicated by `qa-confirm`; confirmed findings fixed. C6 audit done (2026-10-03, adjudicated): F2-F7 fixed; **F1 open** — gix's hunks diverge from `git diff -U3`, awaiting the spike and the user's decision; no R2.4 test until then |
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
  `tail`. Its full run includes `git-floor`, which builds git 2.30.9 and 2.32.7
  into `~/.cache/cairn/git-floor` the first time (network, a C compiler, make and
  zlib's headers) and reuses them after; `CAIRN_GIT_FLOOR_CACHE` points it at
  another directory holding `git-2.30.9/` and `git-2.32.7/` prefixes. CI runs the
  step as its own job.
- Commit explicit paths, never `git add -A`.
- A new dependency, crate or invariant needs its row in
  `crates/cairn-guards/tests/invariants.rs` in the same commit, or the gate fails.
- **Phase 06 needs a font file that is not in the repository yet.** Downloading it
  is the user's call: ask, with the file, its source and its size, and do not
  fetch it unprompted.
- The remote is `github.com/alexparlett/cairn`. Issues and pull requests go there;
  every pull request is merged by the user, never by a session.
