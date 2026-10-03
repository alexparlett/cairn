# State — diff-engine

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 02's rework landed and the C6 audit is done; the content-parity
rework landed (2026-10-03) — a file's changed lines, its whitespace-ignoring lines
and each hunk's function context now come from `git diff-tree -p`, closing the
audit's F1 — and its QA (round 3) is fixed: opening refuses a bare repository git
would refuse to find, a read's `core.fsmonitor` is documented parity, Expand All
batches driver files per algorithm, and the parity and gate pins it asked for
landed. Phase 03 landed (2026-10-03, QA due): one path's staged, unstaged and
untracked diff, answered by `git diff-index --cached`, `git diff-files` and `git
diff --no-index` run as reads, the working-tree side rebuilt from git's patch so it
is git's form of the file (clean filter driver run by git, with the read's
environment), and every R3.4 state answered. The user accepted `git diff
--no-index` for an untracked file as the one porcelain exception to the reads rule
(2026-10-03), its presentation settings pinned by `-c`.** The diff model exists in
`cairn-model`, and `cairn-git` answers R2's two queries: the changes query from
`git diff-tree` through the process manager (decision E, PRD R2.1, R2.2, R2.9 and C14
amended), honouring `diff.ignoreSubmodules` and `log.showRoot` as the user's `git log`
does; the content query reads both versions with gix and asks `git diff-tree -p` which
lines changed (the content-parity decision, amending L3; PRD R2.4, R2.8, R2.9, R6.4 and
C6 amended), with parity to `git diff` enforced by tests under every algorithm and
over real history. The full gate's `git-floor` step, and CI's `git floor`
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
- **git computes the diff; Cairn groups it** (L3, amended by the content-parity
  decision recorded in `progress.md`, 2026-10-03: `git diff-tree -p` answers a
  file's changed lines, its whitespace-ignoring lines and its function context; gix
  reads the blobs, decides what is not text and computes intra-line highlights). No
  diff algorithm is written here but those highlights, and gix types stop at the
  seam.
- **The patch always carries three lines of context**, whatever the view shows,
  and the emitter can never see the whitespace-ignoring ranges (L2, L4).
- **A working-tree read may run the user's filter driver** (L6). D1 is amended for
  it in `docs/design/engine.md` and the root `CLAUDE.md`. As built, git runs it —
  a child of the read's `git`, with the environment Cairn built for the read plus
  what git sets for a filter — never gix; the residuals (the inherited roster it
  still sees, a store of its own it writes, a non-required failure shown as git
  shows it) are stated there.
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

**For phase 08: Expand All is unbounded in memory (QA round 3, R2; the user
decided on 2026-10-03 that phase 08 bounds it).** `DiffSession::file_diffs` reads
every changed file's two blobs into lines before it asks git anything, and holds the
whole parsed patch of each `diff-tree -p` run (every section's body bytes) until
every file is answered — so a commit that changes thousands of large text files
holds all of them at once, whatever the view shows. Phase 08 must bound it with the
line budget: decide which files the budget admits BEFORE any blob is read (the
headers give sizes without inflating), and read, ask git and answer per batch —
streaming or paging the patch per batch rather than parsing one answer for the
whole comparison — so the memory is the batch's, not the commit's.

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
| `DisplayOverlay` | model | Display-only: the whitespace-ignoring ranges, the intra-line highlights and git's function context (`with_function_context`, `function_context`). Beside `TextDiff`, never inside it (R1.7). |
| `FunctionContext` | model | git's text after each hunk header's `@@`, keyed by the hunk's old-side start, with the context it was read at; `of(HunkHeader)` answers `None` for a start git printed no header at. |
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
| `ContentOptions` | git | R2.6's limits, whether to load past them anyway, whether to compute the whitespace-ignoring ranges, and `context`, the view's context, which git is asked at. |
| `DiffSession` | git | Holds gix's resource cache for a run of content queries; its `changes` is `Repository::changes`. Borrows the repository and is not `Send`, like `HistorySession`. |
| `Repository::changes(&GitBinary, ..)` | git | R2.1, R2.2, R2.9, R2.10: gix reads the commits and the configuration — the two rename keys, `diff.ignoreSubmodules`, `log.showRoot` — and `git diff-tree` answers. Blocks on one read process (two when a hidden submodule must be excluded from a rename search); `Cancel` polled every runner tick ends it. A root commit under `log.showRoot=false` answers no files and starts no process. |
| `reads::changes`, `reads::Detection`, `reads::Submodules` | git (crate-private) | The read: `git diff-tree -r -z --raw --no-abbrev` with detection spelled out, the submodules to leave out (`--ignore-submodules=all`, or `:(exclude,literal)` pathspecs), `-z` records parsed into `ChangedFile`s, a superseded query answering `ChangesCancelled`. |
| `diff::renames` (`Configured`, `Search`) | git (crate-private) | `diff.renames` and `diff.renameLimit` parsed by git's rules, the limit the git in use applies, and the cut-short inference per git version. |
| `diff::submodules` (`Hiding`) | git (crate-private) | What `diff.ignoreSubmodules` and each submodule's own `ignore` hide from the user's `git log`, with `.gitmodules` read where git reads it. |
| `diff::git_config` | git (crate-private) | The last value of a key across the configuration, `git_config_bool`, `git_parse_int`. |
| `GitCommand::in_repository` | git (crate-private, `process/`) | Now names the repository to git — `--git-dir` and `--work-tree` ahead of the verb — for a repository gix trusts fully; one it trusts less is left to git's discovery, so `safe.directory` still decides it. |
| `SharedRepository::discover_for(path, &GitBinary, environment)`, `Error::BareRepositoryFoundBySearching`, `Error::ProtectedConfig` | git | The application's open (QA round 3, S1): a bare repository found by searching is refused where that git's `safe.bareRepository` rule refuses it, read from the launching environment's protected configuration only (`bare_discovery.rs`), before gix opens it — since every `git` run in it is then given `--git-dir`, which git never checks. `discover` applies git 2.45's rule with the process environment. |
| `DiffSession::file_diff(&GitBinary, &ChangesRequest, &ChangedFile, &ContentOptions, &impl Cancel)`, `Repository::file_diff` | git | R2.3 through R2.9: gix reads both versions and decides what is not text; `git diff-tree -p` (a read) says which lines changed, with `-w` too when asked, every printed line checked against gix's. Blocks on one or two processes (three with `check-attr`); git is not asked where it has one answer (added, deleted, type-changed, an empty side, identical lines). |
| `DiffSession::file_diffs(&GitBinary, &ChangesRequest, &ChangeSet, ..)` | git | Expand All: every file of a change set, the text files' lines from ONE `diff-tree -p` over the comparison with the change set's detection, plus one per distinct algorithm the files' drivers name (`reads::Scope::Paths`, QA round 3's R1); a file a run's answer does not hold as the change set does is asked about alone. Equal to the per-file answers; the runs counted by `expand_all_runs_one_diff_tree_per_comparison`. |
| `reads::patches`, `reads::PatchQuery`, `reads::Scope`, `reads::Algorithm`, `reads::Reading` | git (crate-private) | The patch read and its parser: maximal runs of `-`/`+` lines, git's function context per hunk, the stale-read check. |
| `reads::diff_attributes` | git (crate-private) | `git check-attr --stdin -z diff`: whether a path's diff driver is one that names an algorithm. Run only on git 2.40+ when the configuration names one. |
| `diff::algorithm` (`Algorithms`) | git (crate-private) | `diff.algorithm` as porcelain reads it, and drivers' algorithms from git 2.40. |
| `Repository::commit_details` | git | One commit in the detail R5.3 draws, without a changes query. |
| `DiffContent::Submodule { dirty }` | model | Phase 03: whether the working tree's checkout of a submodule has changes of its own (git's `-dirty`); false wherever the new side is a commit or the index. |
| `WorkingTreeDiff` | git | Phase 03, R3.1: `Staged` (`HEAD` against the index), `Unstaged` (the index against the working tree), `Untracked` (nothing against the working tree). |
| `Repository::working_tree_diff(&GitBinary, &RepoPath, WorkingTreeDiff, &ContentOptions, &impl Cancel)`, `DiffSession::working_tree_diff` | git | R3.1-R3.5 for one path: `Option<FileDiff>`, `None` where the user's `git diff [--cached]` / `git diff --no-index /dev/null` prints nothing. git computes the diff and reads the working tree; the working-tree side's lines are rebuilt from git's patch and checked against the object id git names for them; conflicted, sparse index and bare repository are stand-in states; a blob past R2.6's ceiling refused before git diffs it (a working-tree modification's patch still asked for, under the ceiling, to tell a stat or mode change alone from an edit). Blocks on one read (two with `-w`, plus `check-attr` for a driver algorithm on 2.40+). |
| `reads::working_tree_patch`, `reads::WorkingTreeQuery`, `reads::Side`, `reads::WorkingTreeAnswer` | git (crate-private) | The read: `diff-index --cached` / `diff-files` with `:(literal)<path> :(exclude,glob)<path>/**`, or `diff --no-index -- /dev/null <path>` with `NO_INDEX_PRESENTATION` pinned by `-c`; `-z --raw -p --full-index`, no `-a`; at most one record and its sections; a ceiling on stdout (`PastCeiling`); `--no-index`'s status 1 an answer only with a record. |
| `PatchText::new_side`, `new_index_id`, `submodule_targets`, `has_hunks`; `Parser::finish_listing` | git (crate-private) | The new side rebuilt from the old and git's patch, every printed line checked; the id on the `index` line; git's `Subproject commit` lines and `-dirty`; records and sections unmatched, for one path. |
| `Invocation::finish_within` | git (crate-private, `process/`) | `finish` with a stdout ceiling: the crossing chunk withheld, the process ended, `GitOutputTooLarge`; what arrived before a failed exit is already the caller's. |
| `diff::submodules::working_tree_ignore` | git (crate-private) | The `--ignore-submodules` value porcelain applies and plumbing does not read: `diff.ignoreSubmodules`, unless the submodule has an `ignore` of its own. |
| `Error::ChangesCancelled`, `ContentCancelled`, `ContentReadsDisagree`, `DiffSetup`, `DiffFile`, `UnexpectedGitOutput`, `InvalidConfig` | git | What the caller of a diff query must handle; `TreeDiff` went with the gix tree walk. `ContentReadsDisagree` is the stale-read guard: git printed lines that are not the lines gix read; ask again. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 diff model | landed | `scripts/gate.sh` PASS | `qa-checklist`, `test-coverage-auditor` and `responsiveness-reviewer`, adjudicated by `qa-confirm`; confirmed findings fixed or recorded as residuals in `docs/systems/diff.md` |
| 02 engine, commits | landed 2026-09-18; changes query reworked onto `git diff-tree` 2026-10-03 (decision E) | `scripts/gate.sh` PASS, `git-floor` included | done over the reworked phase (2026-10-03), adjudicated by `qa-confirm`; confirmed findings fixed. C6 audit done (2026-10-03, adjudicated): F2-F7 fixed; F1 closed by the content-parity rework (landed 2026-10-03, `scripts/gate.sh` PASS with `git-floor`): R2.4 and R2.8 parity enforced under every algorithm and over real history. QA of the content rework (round 3, 2026-10-03): 21 raw, 16 confirmed by `qa-confirm`, S3 dismissed, S1/R2/G4 escalated and decided by the user; **phase 02 QA round 3 fixed** — every confirmed finding fixed or recorded (R2 above, for phase 08; G4 as issue #51), `scripts/gate.sh` PASS with `git-floor` |
| 03 engine, working tree | landed 2026-10-03; QA due | `scripts/gate.sh` PASS, `git-floor` included | not yet: `/qa` has not reviewed it; the `diff --no-index` exception accepted by the user (2026-10-03) |
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
