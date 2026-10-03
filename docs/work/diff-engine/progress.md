# Progress — diff-engine

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-10-03 — content parity: a file's changed lines come from git

The user decided after the spike (`docs/research/diff-engine/content-parity-spike.md`,
"Decided from it"): the content query's changed ranges, its whitespace-ignoring
ranges and each hunk's function context come from `git diff-tree -p` run as a read;
gix keeps the blob reads, the size, binary and LFS pre-checks, intra-line highlights
and the patch emitter. **This amends packet decision L3** ("gix computes the diff;
Cairn groups it"), which stays as written in `brainstorm.md`, a historical record;
PRD R2.4, R2.8, R2.9, R6.4 and C6 are amended inline, each marked "(amended 2026-10,
content parity, see progress.md)". C6's "crafted fixtures with unambiguous edits" is
now parity on any edit, under every algorithm and over real history. F1 of the C6
audit is closed by it.

### What shipped

- `cairn_model::FunctionContext`, carried by `DisplayOverlay`
  (`with_function_context`, `function_context`): git's text after each hunk
  header's `@@`, keyed by the line the hunk starts at on the old side, with the
  context it was read at. Display-only, so the emitter's patches are unchanged.
- `crate::reads::patches` (`crates/cairn-git/src/reads/patches.rs`): `git
  --literal-pathspecs -c diff.suppressBlankEmpty=false diff-tree -r -z --raw
  --no-abbrev -p --full-index -U<n> --no-ext-diff --no-textconv --no-color -a [-w]
  [--diff-algorithm=X] <detection> --end-of-options <old> <new> -- <paths>`, parsed
  as maximal runs, every printed line checked against gix's lines. One file, or a
  whole comparison (Expand All).
- `crate::reads::diff_attributes` (`reads/attributes.rs`): `git check-attr --stdin
  -z diff`, run only when the git in use reads driver algorithms (2.40+) and the
  configuration names one git parses.
- `crate::diff::algorithm`: `diff.algorithm` read as porcelain does
  (`Error::InvalidConfig` for a value git refuses), the drivers' algorithms from 2.40.
- API: `ContentOptions.context` (the view's context, default three lines);
  `DiffSession::file_diff` and `Repository::file_diff` now take `&GitBinary`, the
  `&ChangesRequest` the file came from and `&impl Cancel`; new
  `DiffSession::file_diffs(&GitBinary, &ChangesRequest, &ChangeSet, ..)` for Expand
  All; `Error::ContentCancelled` and `Error::ContentReadsDisagree { path, detail }`
  (the stale-read guard, which a caller retries).
- Deleted: `crates/cairn-git/src/diff/whitespace.rs` (Cairn's own whitespace pass)
  and the gix line diff of the content query (`exact_changes`, `changed_ranges`).
  `GitCommand::input` lost its test-only `expect(dead_code)`: `check-attr` feeds it.

### Parity evidence (git 2.56.0; the same suites on 2.30.9, 2.32.7, 2.39.5, 2.40.0)

| Suite | Compared | Divergences |
| --- | --- | --- |
| Discriminating fixture, 9 configurations (unset; myers; minimal; patience; histogram; heuristic off; histogram + heuristic off; histogram with `diff.drv.algorithm=patience`; `diff.drv.algorithm=minimal`) at `-U3` | 720 files, 3,673 hunks, 3,052 with function context | 0 |
| This repository's history, last 50 non-merge commits (all there are), each of myers, minimal, patience, histogram | 596 files and 1,157-1,158 hunks per algorithm | 0 |
| Function context at `-U1`, `-U3`, `-U5`, `-U8`, default rule and a capturing `xfuncname` | 80 files per context | 0 |
| `git diff -w`, seeded whitespace-only, mixed and real edits, and the discriminating fixture | 34 + 80 files | 0 |
| Crafted history (C6) at `-U3` and `-U1` | every text file of every commit | 0 |
| Files git is not asked about (added, deleted, emptied, filled; under a driver and not) at `-U1`, `-U3` | 16, no process started | 0 |
| Expand All against per-file, with and without `-w`, seven fixtures | at least 300 files | 0 |

Each comparison is against porcelain `git diff` for the file's paths with the
detection that pairs them, so it reads `diff.algorithm`, the drivers and the indent
heuristic as the user does. The discriminating fixture is first shown to
discriminate: each algorithm's answer differs from myers', patience's from
histogram's, the heuristic's from its absence, and — on a git that reads one — a
driver's algorithm from `diff.algorithm`.

**Mutations, each run and reverted:** dropping the `--diff-algorithm` flag turns
the discriminating suite (minimal: 2 of 80 files), the history replay (minimal: 8
of 596) and Expand All red. Asking at `-U0` turns the discriminating, history,
function-context and `-w` suites red (72 of 80, 291 of 596, 72 of 80, 17 of 34
files). The parser's unit tests are the stale-read guard's: a removed, context or
added line that differs, a lost newline and a side that ends early each refuse the
reading, and `lines_git_printed_that_were_not_read_are_the_error_a_caller_retries`
maps that to `Error::ContentReadsDisagree`.

### C14, re-measured

Same machine and repository (AMD Ryzen 7 9800X3D, 60.4 GiB, NVMe, rust-lang/rust at
`c999cef531e`); `git version 2.56.0`; `cargo test --release`, warm, one run to warm
up then the median of seven; the reporter in `crates/cairn-git/tests/diff/bench.rs`.

| Subject | Cairn now (git ranges) | min / max | Cairn before (gix ranges) | git baseline | Bar | |
| --- | --- | --- | --- | --- | --- | --- |
| F7 `3b09522c34b`, loaded | **17.513 ms** | 16.801 / 22.088 | 9.024 ms | 9.8 ms | 100 ms | MET |
| F7, refused on the byte ceiling | 0.027 ms | 0.025 / 0.032 | 0.004 ms | — | — | — |
| F1 `6a6e8446b97`, refused | 0.007 ms | 0.004 / 0.029 | 0.002 ms | — | — | — |
| F1, Load Diff | **186.859 ms** | 185.959 / 195.281 | 128.4 ms | 163.0 ms | — | recorded |
| Expand All, S7 `f0845adb0c1` (1,017 text files, one `diff-tree -p`) | **61.090 ms** | 60.823 / 62.653 | — | — | — | recorded |

The changes query is unchanged (S7 7.808 ms, S1 33.611 ms, M1 81.494 ms; M1's 2,774
pairs all git's). F7's bar is met with five times its margin; the content query now
costs git's own diff plus the gix read Cairn keeps. Expand All's 61 ms covers gix
reading and splitting all 1,017 files' blobs, git's one patch over the commit
(the spike measured 31.7 ms for the process), parsing and checking it, and the
intra-line highlights.

### Decisions made here without asking

- **git is asked at the view's context, not at `-U1`.** The function context of a
  hunk depends on where it starts, so on the context, and cannot be derived from a
  `-U1` answer (a function line between `-U1`'s start and `-U3`'s changes it). One
  read at `-U<n>` (n ≥ 1, the entire file asking at one) answers the ranges — the
  same script at every context of one or more, which the spike measured and the
  replay re-checks — and the headers for exactly the hunks the view draws. A context
  change is a new content query (phase 06). `-U0` stays refused.
- **Driver detection asks git** (`check-attr`), rather than gix's attribute stack,
  so which paths a driver covers is git's answer by construction; it runs only in
  the rare case it can matter. For a path whose driver names an algorithm no flag
  is passed, as decided; for the rest `--diff-algorithm` is always the long form.
  Behaviour reproduced on 2.30.9, 2.32.7, 2.39.5 (driver algorithm ignored, porcelain
  and plumbing alike) and 2.40.0, 2.56.0 (honoured, by the OLD path's attribute).
- **Each file is answered as `git diff -- <path>` answers it**, not as a whole
  `git show` does: from 2.40 git applies a driver's algorithm by changing its own
  options, so in one multi-file output every later file inherits it (reproduced on
  2.40.0 and 2.56.0, porcelain and plumbing). Expand All passes the algorithm
  explicitly to its one call, which switches drivers off for it, and asks about each
  driver-algorithm file alone. Recorded as a known limit in `docs/systems/diff.md`.
- **git is not asked where it has one answer**: added, deleted and type-changed
  files, a side with no lines, and two sides with the same lines (a rename that kept
  its blob). A type change is one change of every line, since git's own patch for it
  is a deletion and an addition.
- **`-w` on older gits**: 2.30 through 2.40 list a whitespace-only file and print no
  patch for it, where 2.56 leaves it out of the raw records too. The read adds
  `--full-index` and matches such a file to a patch only by the blobs on its `index`
  line; absent or unprinted under `-w` is "no changes".
- **`--literal-pathspecs` is a global option**, before the verb (`diff-tree` refuses
  it after); verified with a path holding `*` beside one the glob would match, on
  every git above. The whole-comparison read does not pass it: its only pathspecs
  are the `:(exclude,literal)` magic of the changes query's submodule rule.
- `scripts/git-floor.sh`'s test-count floors raised to 49 and 52, one under the
  runs' new counts.

### Residuals and follow-ups

- **Phase 03** (working tree): the spike's mechanism carries over —
  `diff-files -p -U<n>` and `diff-index -p -U<n> [--cached] HEAD`, with the held
  lines git's clean-filtered, EOL-converted form; `diff-files` runs the clean filter
  (spike section 3), which L6 already allows. `reads::patches` takes trees today; a
  working-tree scope is its next arm.
- **Phase 04**: every content query now starts a process and blocks on it, so it is
  a diff-thread call with its epoch; `ContentOptions.context` is part of the cache
  key R4.5 allows; `Error::ContentReadsDisagree` is retried, `ContentCancelled` is
  not an error to show.
- **Phase 06**: a context change asks the engine again, since function context
  belongs to the context it was read at (`FunctionContext::of` answers `None` for a
  hunk git printed no header at). Under `-w`, git prints a context line from the NEW
  side, which may differ from the old in its whitespace; `UnifiedRows` draws context
  from the old side and has no rows over the whitespace-ignoring ranges yet — the
  parity test builds them over the new side. `diff.context` and
  `diff.interHunkContext` are not read (a known limit).
- **Phase 08**: `DiffSession::file_diffs` is Expand All's call; its one `diff-tree`
  diffs every file git would show, including ones Cairn refuses as too large, so a
  commit holding a very large text file pays git's time for it (cancellable). The
  line budget may want to pass a subset — today it takes the whole change set.
- CI's `git floor` job runs 2.30.9 and 2.32.7, so the before-2.40 branch of the
  driver rule is decided there and the 2.40+ branch on the gate's own git; 2.39.5 and
  2.40.0 were run by hand here, not by the gate.
- QA is due: `/qa` has not reviewed this change.

## 2026-10-03 — the C6 audit, the gate's integrity, and a shallow clone's history

The C6 audit and a gate-integrity review raised findings that a fresh `qa-confirm`
adjudicated. Fixed from commit `2e15d1c` up to this entry; `scripts/gate.sh` passes,
`git-floor` included.

### The C6 audit, F1-F7

- **F1 — escalated, not fixed: gix's hunks diverge from `git diff`'s.** The content
  query computes its ranges with gix (`diff_with_slider_heuristics` in
  `crates/cairn-git/src/diff/content.rs`), and the adjudicator found answers that
  differ from `git diff -U3`'s, which the user's standing rule makes a critical
  parity bug. A spike is measuring what moving the ranges to git would cost, and the
  user decides after it. Until then the algorithm is untouched and no test
  discriminates `diff.algorithm` or the indent heuristic (R2.4): it would fail today.
- **F2 — the load-anyway ceiling.** `past_the_load_anyway_ceiling_nothing_is_offered_or_read`,
  over the truncated object with `load_anyway_bytes` either side of its size: not
  offered without `load_anyway`, refused from the header with it, offered at exactly
  the size. Mutations `loadable: true` and a `u64::MAX` load-anyway ceiling each turn
  it red.
- **F3 — both limits at exactly their value.** A side of exactly `max_lines` lines is
  inside the limit (the unit test of `crossed_line_limit`), and a file of exactly
  `max_bytes` is drawn while one byte less of ceiling refuses it
  (`a_file_exactly_at_the_byte_ceiling_is_drawn`). Each `>` turned `>=` turns its test red.
- **F4, F5 — git's binary rules.** The attributes fixture gained the `binary` macro
  (`macro.dat`), a NUL at byte 7,999 (binary) and one at byte 8,000 in a file of short
  lines (not binary), each compared with git as before; and
  `a_file_past_big_file_threshold_is_binary_as_git_says` sets
  `core.bigFileThreshold=1k` beside a text file a few KiB long. All agreed with git.
- **F6 — a binary's sizes** are compared with `git cat-file -s` for every binary file
  of both tests; `old_size: 0, new_size: 0` turns both red.
- **F7 — a pure deletion inside a file.** `crafted()` gained `delete a line`, one
  middle line of `long.txt` removed, before the two commits tests reach as `HEAD~1`
  and `HEAD`; it runs through C1, C2 (which now requires a removal-only file among
  its shapes), C3 and C6. gix's answer for it agreed with `git diff -U3`.

### Gate integrity, G1-G6

- **G1.** The two `CAIRN_REQUIRE_*` guards matched the variable on any line of the
  workflow; they now read the `gate` job's own `env:` (`job_env_entries` in
  `crates/cairn-guards/src/lib.rs`), and
  `the_workflow_env_matcher_reads_only_the_jobs_own_block` shows a workflow-level,
  another job's, a step's, and a commented-out setting each failing. Moving both
  variables into a step and the other job turned both guards red.
- **G2, G3.** `scripts/git-floor.sh` lists each filtered run first and fails under a
  floor — decided here: one under the counts when set (29 for `--lib diff:: reads::`,
  30 tests; 44 for `--test diff_engine`, 45) — and runs with `--show-output`, printing
  each run's `SKIPPED` lines and restating them all at the end. Its header names the
  two skips the floor's gits take: the partial-clone rename search below 2.44 and the
  `safe.bareRepository` case below 2.38.
- **G4 — the user decided the floor runs in the local full gate.** `scripts/gate.sh`
  with no arguments now runs `git-floor` after `test-doc`, as it runs `deps`; `--fast`
  skips both. The first run fetches and builds; the builds are cached after. Where a
  build is needed the script first checks for a C compiler (`$CC`, now also passed to
  `make`), `make` and zlib's headers, and fails naming what is missing with the
  packages to install — a fresh Debian or Ubuntu needs `build-essential zlib1g-dev`,
  which is what the CI job installs — and never skips. Twin:
  `the_local_full_gate_runs_every_step_but_the_day_loops`, with the exemption roster
  `LOCAL_FULL_GATE_EXEMPT` (`test-fast`) and the self-test
  `the_gate_sequence_matcher_catches_the_shapes_it_claims`; it failed on the gate
  before `run_git_floor` was added. Root `CLAUDE.md` and `docs/qa-gate.md` say so.
  Run here from an empty cache, the full gate fetched and built both gits and their
  diff tests passed, each run listing its two expected skips.
- **G5.** git before 2.32 ignores `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM`, so the
  fixtures ran under the machine's `~/.gitconfig` on the floor's gits. `Repo::run`,
  the scratch index's runs (`tests/diff/scratch.rs`, decided here: same hazard,
  same tests) and `tests/fixtures/mod.rs` now set `HOME` and `XDG_CONFIG_HOME` to a
  checked-empty directory and `GIT_CONFIG_NOSYSTEM=1`. Shown on 2.30.9: a home whose
  `.gitconfig` signs commits with a failing program broke eight diff tests without
  the change and none with it.
- **G6 — dismissed:** the oracle cannot reach `log.diffMerges`.

### The history graph's shallow boundary — the user decided to fix it here

The history query handed a shallow clone's boundary commit over with the parents its
object names, where `git log --format=%P` prints none. The test written for it, against
`git log --format='%H %P'` at depths 1 to 4 of one merge-shaped history, found a second
divergence: at depth 4, where a boundary commit's parent is in the clone because a
sibling branch reaches it, gix's walk left that commit out entirely — gix 0.87.1's
`rev_walk` skips the next appearance of each cut-off parent id whichever commit names
it. Decided here: both routes now walk `gix::traverse::commit::Simple` directly over an
object source that serves a boundary commit with its `parent` lines removed, git's
graft applied where the object is read (`crates/cairn-git/src/history/walk.rs`); no
commit-graph is used in a shallow repository, as git's `commit_graph_compatible`
refuses one there; an invalid `core.commitGraph` is still an error and a graph that
will not open is walked without, as gix's walk decided both. The shallow file is read
by `crates/cairn-git/src/shallow.rs`, which `commit_details` now shares. A boundary
commit is a root to the lane assigner, so no lane waits below it; the test requires
the graph to be the one git's parents lay out, and the oldest row to carry no line
past it. Turning the graft off turns it red. As built: `docs/systems/history-graph.md`,
"A shallow clone is walked as git walks it".

## 2026-10-03 — phase 02's QA: what it found, what was decided, what was fixed

QA ran over `34bc907..HEAD`, the reworked phase. Its reviewers raised nineteen raw
findings plus a coverage gap around C6; a fresh `qa-confirm` adjudicated them, most
by experiment against git 2.56, and confirmed sixteen. Dismissed: **D3**, a ceiling on
the changes query's answer — a cap would hide files git lists, and the cancel already
bounds the time; **Q2**, a test-only option to run the floor's branches — it fails
loudly rather than proving anything, and is superseded by the CI job below; and
**Q3**, that gix opens the configuration with `includes: true` under full trust,
which the adjudicator found to be no defect. The C6 audit is still to do.

Two findings needed the user, who decided on 2026-10-03: **A3, honour
`log.showRoot`**, and **Q1, prove the 2.30 floor in CI**.

What was fixed, each with its test in the same commit (commits `430ab12` to the
docs commit after `cd0c065`):

- **D1 — the repository is named to git.** `in_repository` only set the directory,
  so git's discovery read the enclosing repository for a working tree inside
  another, and refused a bare repository under `safe.bareRepository=explicit`. Each
  invocation now passes `--git-dir` and `--work-tree`. Decided here, without the
  user: an explicitly named git directory skips git's `safe.directory` check
  (reproduced with `GIT_TEST_ASSUME_DIFFERENT_OWNER=1`), so the options are given
  only for a repository gix trusts fully, which is gix's reading of the same rule; a
  less trusted one is left to discovery and to git's check. The command log and
  errors record the verb's arguments, not the location — a log is the repository's
  own. Fetch now lands in the repository Cairn opened, which was the bug's other
  half.
- **A1 — a shallow clone's boundary commit lists no parents**, as `git log
  --format=%P` shows. **The history graph has the same divergence** — gix's walk
  hands `info.parent_ids` with the parent the clone lacks
  (`crates/cairn-git/src/history.rs`, `history/session.rs`), checked on a depth-1
  clone of this checkout — and is not fixed here: the graph is history-graph's,
  the fix spans three walk sites and the lane assigner's handling of a parent that
  never arrives, and it is reported for its own change.
- **A3 — `log.showRoot`.** False means no diff for a root commit (or a shallow
  boundary), as `git log` and `git show` print none. Decided here: it is read for
  every one-commit query, since `git log` refuses an invalid value whether or not
  the commit is a root, and never for a comparison, which is `git diff`'s.
- **A2 — `diff.ignoreSubmodules`.** Decided here, against the suggested rule: a
  list filtered after git answers still diverges — git hides the gitlinks before
  rename detection, so `git log` never counts them against `diff.renameLimit`, and
  an added gitlink can push a search `git log` runs past the limit (reproduced on
  2.30 and 2.56: `git log` shows `R094`, a filtered `diff-tree` a deletion and an
  addition). So git is asked not to queue them: `--ignore-submodules=all` when no
  submodule has a setting of its own, which is exact; otherwise, because that flag
  overrides a submodule's `none`, the first answer's hidden gitlinks are excluded by
  `:(exclude,literal)` pathspecs in a second run when detection is on, and dropped
  when it is off. The semantics — the working tree's `.gitmodules`, else the
  index's, else `HEAD`'s, never the shown commit's; none in a bare repository; the
  name the last to claim a path; `ignore` values git does not know skipped — were
  read from git's source at v2.30.0 and v2.56.0 and reproduced on both. Two
  residuals are stated in `docs/systems/diff.md`.
- **T1–T4.** C1 and C3 floor on files staged through a patch (a content query
  answering everything binary now fails them, and passed before); C2 requires a
  mode change beside a hunk, two hunks and CRLF context, from three new crafted
  commits (dropping the mode lines when hunks exist, and stripping `\r` from
  context, now fail it, and passed before); the cut-short test meets the square of
  the limit (`>` turned `>=` now fails it, and passed before); and the
  write-nothing test claims only what it decides, with the command pinned on the
  argument vector.
- **Q1 — the floor in CI.** A `git-floor` gate step (`scripts/git-floor.sh`)
  builds git 2.30.9 and 2.32.7 by pinned commit and runs the diff tests on each;
  2.32 because it alone reaches the 2.31–2.32 branch. Decided here: a gate step,
  so `ci_runs_every_merge_bar_gate_step` holds CI to running it, but outside the
  local full sequence, since it fetches and builds git; run by CI as its own job,
  with the two `CAIRN_REQUIRE_*` variables moved from the workflow to the gate
  job so the floor job's skips stay skips. The oracles needed spellings git 2.30
  reads (`-m --first-parent`, a relative `--git-common-dir`, the 400 default), and
  the `safe.bareRepository` case skips before 2.38. Verified locally against both
  gits built in the scratchpad from a local clone of git's source.
- **D2, Q4, Q5, T6, A4 — docs.** The partial-clone limit now covers tree-less
  clones and what older git does; the cancellation paragraph lost its history and
  its literal counts; `crafted()`'s doc lists the commits it makes; and `state.md`
  records that "cut short" needs a `cairn-model` counterpart before phase 04 or 05
  carries it across the seam.

## 2026-10-03 — phase 02 reworked: the changes query answered by `git diff-tree`

The packet resumed on the process manager (#50) and phase 02's changes query moved
from gix to git, per decision E. `Repository::changes` now takes the `GitBinary`
the application found at startup: gix reads the commits named and the two rename
keys, and `crate::reads::changes` runs `git diff-tree -r -z --raw --no-abbrev` as a
read invocation, parsing its `-z` records into `ChangedFile`s, which are then sorted
by the same total key as before. The gix tree walk went with it — `repair_copies`,
gix's `RenameDetection` counters, the per-change cancel poll and `Error::TreeDiff`.
The content query, the model and the round trips stand. As built:
`docs/systems/diff.md`, "The changes query". `scripts/gate.sh` passes.

PRD amended, each marked inline: R2.1 (the answer is `git diff-tree`'s), R2.2
(git's defaults, the search the user's own `git log` makes, cut short decided from
the answer), R2.9 (a superseded query ends its process, rename detection included)
and C14's rename clause (git's own pairs, no gap may be filed).

### How the configuration is honoured — decided here

- **`diff-tree` reads `diff.renameLimit` but not `diff.renames`.** Verified against
  git: `git help config` says `diff.renames` affects porcelain only, and git's
  `builtin/diff-tree.c` loads `git_diff_basic_config`, which holds
  `diff.renamelimit` while `diff.renames` is in `git_diff_ui_config`; a fixture
  with `diff.renameLimit=1` and plain `diff-tree -M` printed git's limit warning.
  So detection must be passed, and it is all passed: `-M` or `-C` with
  `-l<limit>`, or `--no-renames`.
- **Read in process, from gix's loaded configuration, parsed by git's rules** —
  the last value across files, the bare key as true, `copies`/`copy`, git's
  boolean words, any integer git accepts (base 0, `k`/`m`/`g`, the `int` range).
  Not by a `git config` process: `crate::reads` admits query plumbing and `status`
  only, and that list is a user-owned rule. Two residuals follow and are stated in
  `docs/systems/diff.md`: the keys are as of the repository's opening, like every
  key gix reads; and gix sees Cairn's own `GIT_CONFIG_*` environment, which the
  `git` process does not inherit — since the flags are passed explicitly, gix's
  view, which is the user's shell's, decides.
- **The default limit and what zero means depend on the git**, read from git's
  source at v2.30.0, v2.31.0, v2.32.0, v2.33.0 and v2.56.0: 400 before 2.33 and
  1,000 from it; a limit of zero or less is 32,767 before 2.33 and none from it.
- A value git refuses is `Error::InvalidConfig`: the user's own `git log` refuses
  to answer on it too.

### "Cut short", without reading stderr — decided here

git's warning is prose in the user's language, and there is no exit status or
flag for it. But git skips its exhaustive stage exactly when the sources it has
left times the destinations it has left exceeds the limit squared
(`too_many_rename_candidates`), and when it skips, those leftovers are the
answer's unpaired paths; when it does not, they are a superset of them. So the
inequality over the answer's own counts is exact, with sources counted as the git
in use counts them: unpaired deletions for renames from 2.31 (which culls what the
exact and basename stages paired), every deletion on 2.30, every deletion and
modified file for copies on every version. `RenameDetection::needed_limit` carries
git's own "set it to at least N". Pinned against the linking git's warning, read in
the C locale by the test only, at limits either side of each boundary.

### Tests

C1, C2, C3 and C6 pass unchanged on the git-produced lists. C5 is kept and widened:
`the_answer_is_what_git_log_shows_under_each_configuration` compares with porcelain
`git log --raw` — what the user sees — over 11 spellings of `diff.renames` against
6 of `diff.renameLimit`, every commit including the root;
`a_rename_limit_that_cuts_detection_short_is_reported_exactly_when_git_warns`;
`a_configuration_git_refuses_is_refused`. New: cancellation of a running
`diff-tree` by epoch (`a_changes_query_superseded_by_a_newer_epoch_stops_git_and_reports_it`,
an exhaustive 4,000 x 4,000 search ended mid-run, the command log saying it was
ended), a query superseded before it starts (no process), the read writing nothing
(`the_changes_query_writes_nothing`: the whole git directory byte-identical, the
working tree stat-dirty, `cachetextconv` configured, the textconv never run), a
partial clone (`in_a_partial_clone_a_rename_search_fails_rather_than_fetching`), and
a shallow clone's boundary commit (below). Each new rule was mutated by hand — the
bare key read as off, the culling rule dropped, the 2.33 default dropped, the
shallow check dropped — and a named test failed each time.

### Found while building, decided without asking

- **A shallow clone's boundary commit** named a parent the clone lacks, so
  `diff-tree` failed (`fatal: bad object`) where `git log` shows it as a root. The
  query now compares it with the empty tree
  (`a_shallow_clones_boundary_commit_is_compared_as_git_log_shows_it`). Its
  `CommitDetails::parents` still lists that parent: left for a follow-up.
- **In a blob-less partial clone a rename search fails** (`GitFailed`, nothing
  fetched), where `git log` would fetch: the read's no-lazy-fetch rule wins over
  parity. With detection off the same clone answers.
- **`diff.ignoreSubmodules`** is porcelain config `diff-tree` does not read, so a
  submodule change it would hide from `git log` is listed. Not handled.

### C14, re-measured

Same machine and repository as phase 02 (AMD Ryzen 7 9800X3D, 60.4 GiB, NVMe,
rust-lang/rust at `c999cef531e`); `git version 2.56.0` (the baseline measured
2.55.0); `cargo test --release` (release profile), warm: one run to warm up, then
the median of seven; `measures_the_diff_queries_against_a_named_repository` with
`CAIRN_BENCH_REPO`, which only reads the repository.

| Subject | Cairn now (git) | min / max | Cairn before (gix) | git baseline | Bar | |
| --- | --- | --- | --- | --- | --- | --- |
| S7 `f0845adb0c1`, 1,017 files | **8.205 ms** | 7.883 / 8.465 | 1.631 ms | 7.9 ms | 100 ms | MET |
| S1 `cf2dff2b1e3`, 27,592 exact renames | **35.816 ms** | 34.166 / 36.725 | 20.658 ms | 28.6 ms | 500 ms | MET |
| M1 `5a3292f163d`, 5,602 paths | **83.476 ms** | 80.153 / 84.458 | 2.845 ms | 78.5 ms | 500 ms | MET |

The cost is git's own plus a few milliseconds of parsing and sorting; S1's 27,592
records are the most of that. **M1's rename pairs: 2,774, equal to git's** — and
the reporter now asserts every pair, source, destination and score, against
`git diff-tree -M` on the same commit, not only the count. Not cut short at the
default limit of 1,000. The content query is unchanged: F7 loaded in 9.024 ms (bar
100 ms, MET), F1 refused in 0.002 ms and loaded anyway in 128.4 ms.

### For phase 03 and 04

- Phase 04's diff thread holds a `GitBinary` copy (the application's
  `Discovery`) and calls `Repository::changes` with its epoch; the engine API is
  synchronous and testable directly, as here.
- The rename keys are read at repository open: phase 04 decides when the worker
  reopens, and that is when a config change is seen.
- `process::registry::tests::a_drop_with_no_reaper_thread_is_logged_once` failed
  once in six runs of the crate's unit tests during this phase (the record showed
  `Signal(15)` where it allows `Signal(9)` or `Unknown`), then passed five times
  running; a timing race in the process manager's own test under load, not in
  this change. Worth an issue.

## 2026-09-30 — packet PAUSED: rename parity, decision E, a process manager first

Phase 02 stopped on its rename rule: on `5a3292f163d` gix pairs 231 renames where
git pairs 2,774. The user classed a wrong rename as a critical bug, so "accept and
file" was ruled out, and asked whether gix is the right backend at all.

Two evidence records settled it, both in `docs/research/diff-engine/`:
`rename-parity-spike.md` (measured on the bench repository) and
`git-process-survey.md` (how Cairn spawns git today). What they showed:

- The cause is two defects in gix's rename tracker — the limit compared unsquared
  against its own documented contract, and no basename stage — unchanged on
  gitoxide `main`.
- Passing gix a squared limit (B) is not viable: it still skips the worst subject,
  and where it completes its pairs differ from git's.
- Falling back to git only when gix reports it was cut short (D) is rare and
  cheap and merges cleanly, but closes only about 64% of disagreeing commits:
  gix's similarity measure and first-match pairing diverge from git's even when
  it runs a full search (0.32% of recent first-parent diffs, a lower bound).
- Cairn has a careful single-invocation runner, not a process manager: no stdout
  that is both captured and cancellable, no stdin, no timeouts, no tracking or
  kill-all, no epoch-driven cancel, and a guard gap that lets `diff/` reach the
  runner without tripping any twin.

**Decided by the user:** gix stays the read backend for history, content diffs
and the model. **Option E**: the changes query — which paths changed, their
statuses, modes, ids and rename and copy pairs — comes from `git diff-tree -M`
always, exact by construction, at git's own cost (about 8–37 ms per selection,
83 ms on the worst subject, inside every C14 bar). This amends D1 (now in
`docs/design/engine.md` on `main`, after #39). And **a process manager for
spawning git is designed and built first, as its own packet**, planned with
`/feature-plan`; phase 03's working-tree reads and packet 5's staging build on it
too.

Consequences for this packet, to be re-planned when it resumes:

- Phase 02's gix changes query (`crates/cairn-git/src/diff/changes.rs`, including
  `repair_copies` and `RenameDetection`) is superseded by a git-backed one; the
  content query, the model and the round-trip tests stand. C5 already compares
  against git, so it carries over; C14's changes-query numbers must be re-measured.
- Phase 02's QA, not yet run, is deferred to the reworked phase.
- The integration branch is behind `main` by #39 (D1's move to
  `docs/design/engine.md`). Bringing it up to date means a rebase and a force
  push of a shared branch, which is the user's call.

## 2026-09-18 — phase 02: the engine answers commit and comparison diffs

`cairn-git` has R2's two queries. `Repository::changes` walks two trees — two
commits, one commit against its first parent, or a root commit against the empty
tree — and answers `ChangeSet`: the changed files sorted by a total key, the
commit's details when one commit was named, and how rename detection went.
`Repository::file_diff` turns one of those files into a `FileDiff`, through gix's
resource cache in `Mode::ToGit`. `DiffSession` holds that cache for a run of
queries; the two `Repository` methods are one-shot sessions over it. As-built
prose: `docs/systems/diff.md`. `scripts/gate.sh` passes.

### C1, C2, C3, C5 and C6

All five pass, in `crates/cairn-git/tests/diff/`. Every apply runs real `git` with
`GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY` and `GIT_ALTERNATE_OBJECT_DIRECTORIES`
pointed at a scratch directory, so the repository under test is only ever read —
including this checkout, which C1 walks.

- **C1** compares TREES, never patch text: `git write-tree` after the apply
  against `<commit>^{tree}`. Over the crafted fixtures, both rewrite fixtures and
  every non-merge commit of the Cairn checkout.
- **C2** seeds twelve selections per file — every line, no lines, only additions,
  only removals, the first line of every change, the last line of every change and
  eight pseudo-random ones — and checks the staged blob against `apply_patch`
  **and** the header effects against the index: the mode staged, whether the
  source path survives, whether anything is staged at all.
- **C3** reverses the same patches onto the commit's tree and requires the
  parent's.
- **C5** compares against `git diff-tree -r --raw --no-abbrev` under the same
  config, field for field: both modes, both ids, the status letter with its
  similarity score, and rename and copy pairs.
- **C6** compares the unified projection with `git diff -U3` of the same two
  BLOBS — two blobs rather than two commits and a path, so rename detection cannot
  change what is being compared.

### C14, measured

Machine, repository and method: `docs/research/diff-engine/measured-baseline.md`
section 1 — AMD Ryzen 7 9800X3D, 60.4 GiB, NVMe, rust-lang/rust at
`c999cef531e`. Release build, warm, five runs, `CAIRN_BENCH_REPO` set;
`measures_the_diff_queries_against_a_named_repository` in
`crates/cairn-git/tests/diff/bench.rs`. git's numbers are the baseline's own
warm medians for the same subject.

Changes query, a session built per query — what a cold worker pays:

| Subject | Cairn median | min / max | git | Bar | |
| --- | --- | --- | --- | --- | --- |
| S7 `f0845adb0c1`, 1,017 files | **1.631 ms** | 1.613 / 23.393 | 7.9 ms | 100 ms | MET |
| S1 `cf2dff2b1e3`, 55,184 paths | **20.658 ms** | 20.525 / 31.418 | 28.6 ms | 500 ms | MET |
| M1 `5a3292f163d`, 5,602 paths | **2.845 ms** | 2.844 / 4.933 | 78.5 ms | 500 ms | MET |

The same three on a session already open — what phase 04's worker will pay:
1.059 ms, 20.741 ms and 2.377 ms. So building the resource cache costs about
0.5 ms on this repository, and holding it open is worth having but is not what
the bars turn on.

Content query, F7 `3b09522c34b`,
`library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs`:

| Answer | Cairn median | min / max | git | Bar | |
| --- | --- | --- | --- | --- | --- |
| Refused: 2,532,736 bytes crosses the 1 MiB ceiling | **0.004 ms** | 0.004 / 0.006 | — | — | — |
| Loaded anyway | **8.727 ms** | 8.718 / 10.170 | 9.8 ms | 100 ms | MET |

Note for the record: this subject is 2.5 MB, so the DEFAULT content query
refuses it on R2.6's byte ceiling. The 100 ms bar is read as the time to answer
it with its lines, which is the `load_anyway` path.

F1 `6a6e8446b97`, `library/stdarch/intrinsics_data/arm_intrinsics.json`, the
too-large subject: refused in **0.002 ms** on `SizeLimit::Bytes`, with
`loadable` true; **Load Diff 127.190 ms** (126.832 / 128.810) against git's
163.0 ms with Myers. The refusal reads the object's header and never inflates
it, which is pinned separately and deterministically —
`the_size_ceiling_is_decided_before_the_content_is_read` truncates a loose
object after its header, so answering "too large" is only possible without
reading it, and asking for it anyway fails.

**Not measured, and not phase 02's to measure:** "the window stays responsive
while the two heaviest subjects load". There is no window on this branch yet.
Phase 04 or 05 owns that check by hand.

### Q3, answered: the rename gap is large, and it is the limit

**On `5a3292f163d`, gix finds 231 rename pairs where git finds 2,774.** All 231
are exact (100%); the gap is every one of git's 2,543 inexact renames — the exact
number C14 names. The reporter prints it; the cause is in the same line:

```
RenameDetection { enabled: true, copies: false, limit: 1000,
                  similarity_checks: 0,
                  renames_skipped_for_limit: 6946095, ... }
```

Zero similarity checks ran. Two findings behind that, both gix's:

1. **gix compares `diff.renameLimit` against the raw permutation count; git
   compares it against the square.** `gix_diff::rewrites::tracker`'s
   `match_pairs_of_kind` computes `permutations = num_src * num_dst` and skips the
   fuzzy stage when `permutations > rewrites.limit`. gix's own doc on
   `Rewrites::limit` says the opposite — "Defaults to 1000, meaning that only
   1000*1000 combinations can be tested" — which is git's rule. As implemented,
   gix's default behaves like git's `diff.renameLimit=31`.
2. **gix has no basename stage.** git's `diffcore-rename` pairs by file name
   before the exhaustive stage and is not governed by the limit, which is why the
   baseline found git still reporting 2,505 inexact renames at `-l1` (section 2b).
   When gix's limit is exceeded it falls back to exact matching alone.

Raising the limit Cairn passes does **not** close this at an acceptable cost: M1
needs 6,946,095 permutations, so matching git's budget (1,000,000) still skips,
and lifting the limit high enough would run millions of blob-pair similarity
diffs with none of git's cheap pre-stages in front of them. **This is phase 02's
stopping rule and goes to the user with the final report.** Filed as gaps, not
fixed here.

### Two more gix divergences, found and handled

- **A copy's source.** With `diff.renames=copies`, gix reports the copy's source
  as it is AFTER the change and stops reporting that file as modified at all. git
  reports the source as it was BEFORE — which is the version in the index, so it
  is the version a patch must be built against — and still lists the file as
  modified. `repair_copies` in `crates/cairn-git/src/diff/changes.rs` puts both
  right by reading the source path out of the old tree: one lookup per copy, and
  nothing at all when copies are off. Without it a copy's patch does not apply and
  a modified file disappears from the list. Pinned by C5's strict comparison with
  `git diff-tree -C --raw`.
- **A copy cannot be reverse-applied, by git either.** `git apply -R` of
  `copy from A / copy to B` re-creates A from B and refuses because A is still
  there. git's OWN patch for a copy fails the same way, which
  `gits_own_copy_patch_cannot_be_reversed_either` pins — so C3 undoes a copy by
  removing its destination rather than requiring of Cairn's patch what git does
  not manage with its own.

### Decisions taken without asking

- **A type change is emitted as git emits one: two file patches at one path**, and
  a selection that does not hold every change emits nothing. A single
  `diff --git` with `old mode`/`new mode` is refused by `git apply` with "wrong
  type". This is a `cairn-model` change, with its tests in the same commit.
- **An LFS pointer is detected** — `version https://git-lfs.github.com/spec/` at
  the front of a buffer of at most 1 KiB — and only when EVERY side that exists is
  one, so a file that became a pointer keeps the lines of its real side. R1.2 and
  R6.8 commit to the state and nothing else would ever produce it. It is a
  deliberate deviation from git, which shows a pointer as text.
- **`Operation::ExternalCommand` is answered as `Unsupported`, not asserted
  unreachable.** It cannot happen while the cache is built with
  `skip_internal_diff_if_external_is_configured` off, which is what
  `gix::diff::resource_cache` does; answering it means a gix that changed that
  default draws a notice instead of starting a program.
- **The engine's own line-length ceiling measures a line without its terminator**,
  which is how `DiffLine` holds one and what a view would draw. A `\r` of a CRLF
  ending is part of the line.
- **C1 reads every file with `load_anyway`**, so the emitter is exercised over real
  content rather than over whatever happens to sit under a display ceiling. The
  states that have no patch — binary, submodule, too large past 64 MiB — are
  staged directly with `update-index`, which is what keeps "yields exactly the
  commit's tree" meaningful for a commit that holds one.
- **`diff.algorithm = patience` needs no handling of Cairn's own.** gix opens a
  repository leniently by default, and `config::cache::access::diff_algorithm`
  falls back to `Histogram` for the one algorithm gix lacks, which is what R2.4
  asks for.

## 2026-09-18 — phase 01: the two escalated QA findings, decided by the user

The two findings the QA pass could not settle on its own were put to the user and
both were decided YES. They are the last of phase 01; `scripts/gate.sh` passes.

**F10 — `TextDiff::new` now states and `debug_assert!`s its real precondition.** The
doc promised only increasing, non-overlapping changes. Two more conditions were
already load-bearing and unwritten: the unchanged run between consecutive changes
(and from the start of the file to the first change) is the same length on BOTH
sides, and every range lies inside its own side's lines. `emit_patch` and the row
index each measure that run as the smaller of the two gaps, so unequal gaps drop
their difference from the body while the header is recounted from what was emitted
— an internally consistent patch that omits content its own span claims to cover,
which only real `git apply` notices.

The objection a previous session raised against this fix, and the answer the user
weighed: it cuts against the crate's "malformed input is placed rather than
rejected" precedent (`malformed_input_is_placed_rather_than_rejected` in the lane
assigner is that precedent by name). `debug_assert!` is what answers it. It does
not fire in release, so "shipping code never panics on a path a user can reach"
is untouched, while phase 02 — which builds the gix-backed PRODUCER of these
values against this contract, and runs its tests in debug — gets a loud failure
instead of a silently wrong patch. That is also why it had to land before phase 02
starts rather than at the packet's QA.

Three refusals are pinned (`an_unchanged_run_of_two_different_lengths_...`,
`a_change_reaching_past_the_end_of_its_side_...`, `changes_that_go_backwards_...`)
with `a_well_formed_diff_with_several_changes_is_accepted` as the passing twin. No
existing test violates the new preconditions — the 168-patch fixture corpus
included. One run is deliberately NOT asserted and is stated as a residual in
`docs/systems/diff.md`: the trailing run to the end of each side, because a
`TextDiff` built as a container for one side's content is a legitimate shape and
refusing it was not the decision taken.

**F1 — R1.5 is pinned by counting allocations, not by a claim.** `index_size`
measures what the index STORED; nothing measured what a lookup BUILDS. Verified by
executing the mutation: renaming `row` to `row_uncached` and giving `row` the body
`(0..self.len()).map(|n| self.row_uncached(n)).collect::<Vec<_>>().get(row).copied().flatten()`
leaves every pre-existing `cairn-model` test green — including
`one_row_of_a_hundred_thousand_lines_is_reached_through_four_index_entries` and
`many_changes_index_by_change_and_not_by_row`, the two that look like they would
catch it. `crates/cairn-model/tests/diff_row_lookup.rs` is the pin that closes it:
a single `row(k)` on a hundred-thousand-line diff, probed at every kind of piece
the index holds, for both `UnifiedRows` and `SideBySideRows`, measured to zero
allocations. It was watched go RED on that mutation (one allocation of 2.4 MB
unified and 3.2 MB side-by-side, on row 0 alone) and GREEN on its revert, before
being committed. `the_counter_sees_an_allocation_when_there_is_one` is there so a
counter that had stopped counting cannot leave the pin dead.

**New dev-dependency, `allocation-counter` 0.8.1** (MIT/Apache-2.0, no dependencies
of its own), on `cairn-model` alone. A counting global allocator is the only thing
that can decide R1.5: a timing ratio is flaky, and a counter inside `TextDiff`
would put test state in a seam type. `unsafe_code = "forbid"` is why it is a
dependency rather than hand-rolled — the lint binds Cairn's crates, a dependency
may contain `unsafe` internally, and this crate carries the `#[global_allocator]`
itself, so linking it replaces the allocator of that one test binary and of nothing
shipped. Its counters are thread-local, so `cargo test`'s parallel threads cannot
pollute each other's totals. Its row went into `TEST_ONLY_ALLOWLIST` in
`crates/cairn-guards/tests/invariants.rs`; `cargo deny` needed no exception, so
`deny.toml` is unchanged.

## 2026-09-17 — phase 01: the diff model and the patch emitter

The model of R1 exists in `cairn-model` and nothing else does: no engine query, no
component, nothing that reads a repository. Eleven modules, all pure, described as
built in `docs/systems/diff.md`. `scripts/gate.sh` passes.

**Verified against real git, outside the suite.** Before QA, every non-empty patch
the emitter produces for the whole fixture corpus — 168 of them, over both the
with-ids and without-ids passes — was run through `git apply --check --cached` and
then `git apply --cached` in a scratch repository, and each result compared
byte-for-byte against the independent oracle. 168 pass, 0 fail. The harness was
deliberately NOT committed: `cairn-model` may not spawn a process, and C1, C2 and
C3 are phase 02's to own in `cairn-git`. This is evidence that phase 02 starts from
a working emitter, not a substitute for those criteria.

**Decisions taken without asking, any of which a reviewer might have taken
differently.**

- *The reference applier is public.* C2 lives in `cairn-git`, so the oracle has to
  cross a crate boundary, and a `#[cfg(test)]` item cannot. It is pure
  `(content, patch) -> bytes` and touches no repository, so it is not a route around
  D1; `docs/systems/diff.md` says so in as many words.
- *A path is a `RepoPath` over bytes, not a `String`.* git promises no encoding, and
  a lossy path in a patch header names a different file. The cost is a type the UI
  must read through.
- *`LineNumber` stores a zero-based index and offers `one_based()`* rather than the
  model carrying two coordinate systems. One type, one conversion point.
- *The emitter matches git where git is odd.* Keeping an unterminated last line as
  context while selecting the additions after it produces a file whose run-on line is
  in the middle. That is what `git apply` makes of the same patch, so the emitter
  does not second-guess it; the reverse-apply test skips those selections and says
  why, and a view offering line selection should keep the combination out of reach.
  Worth an issue when the packet files its deferred work.
- *The `index` line is omitted for a partial selection*, because the resulting blob
  is neither the old one nor the new one and a wrong id there is a lie a three-way
  apply could act on. Real git accepts the omission; verified.
- *`DiffLimits` carries R2.6's numbers* so the engine and the view read the same
  ceilings rather than each inventing them.
- *No new CLAUDE.md invariant was added for R1.7.* The type split is the enforcement
  and the `compile_fail` doctest is its twin, which is the strongest tier available;
  promoting it to a stated invariant with a guard belongs to the packet's QA phase,
  when the whole surface exists.

**QA.** `qa-checklist`, `test-coverage-auditor` and `responsiveness-reviewer` ran
fresh over the phase diff; `qa-confirm` adjudicated, in its own context. Twenty-six
raw findings, twenty-two confirmed after merging two duplicate pairs. Seven became
test changes, each watched go red on the mutation it names before being committed;
the rest became stated residuals in `docs/systems/diff.md` and in doc comments.

Findings DISMISSED, with the adjudicator's reasons:

- **R-7**, `old_content`/`new_content` allocate without `with_capacity`: no render-path
  caller exists or is planned, the work is inherently proportional to the content and
  runs off the UI thread, so the change would be speculative.
- **R-8**, `Hunk.changes` being a public `Range` invites an O(all changes) loop: that
  field IS the hunk's contract, and both the emitter and the row index iterate it
  legitimately. The cost is inherent to walking a hunk. `Hunks` itself is index-only
  by design and was explicitly not a finding.
- **TC-9(a)**, the four zero-change fixtures making round trips assert
  `content == content`: they exercise a real path — a header-only patch fed through
  the applier must leave the content byte-identical — which is a claim, not a
  tautology.
- **QC-5** in two of its three instances: `DiffContent` genuinely is `Text` or one of
  seven other states, and "four index entries" restates the name of the test that
  pins it, so neither can rot independently. The third instance was a real rotting
  count and was fixed.
- **QC-9**, the first commit's body running to six sentences rather than four: the
  content is why and not what, and the commit already had a child on a shared
  branch. Recorded for the packet's final QA rather than rewritten.

One confirmed finding was **fixed differently from the adjudication**. QC-7 asked
that `tests/diffs/mod.rs` drop its "unwrap and expect are denied here" header and
use `expect`, on the reading that `clippy.toml`'s test exemption covers the whole
integration-test crate. It does not: clippy at `-D warnings` refuses `expect` in a
helper outside a `#[test]` function, which the gate proved. The original spelling
stands and the header comment was corrected to say precisely why.

**Two changes of unknown provenance appeared in the working tree during QA**, and
both were discarded rather than committed. No reviewer reported writing either; all
three were dispatched read-only. Code nobody can account for authoring does not
belong on a branch other phases build on, however good it looks — but both are worth
weighing deliberately, so they are described here rather than just dropped:

- About a hundred lines adding `debug_assert!` preconditions to `TextDiff::new`, with
  tests, making a malformed changed-range set panic in debug rather than be drawn
  short. That is a real choice, and it is against the crate's existing "malformed
  input is placed rather than rejected" precedent, so it is the user's to take.
- `crates/cairn-model/tests/diff_row_addressing.rs`, about 260 lines, aimed squarely
  at the confirmed gap that `index_size` measures what was stored rather than what was
  built. It reasons that the decisive pin — counting allocations under a counting
  global allocator — cannot be written here, because `GlobalAlloc` is an unsafe trait
  and `unsafe_code = "forbid"` sits at the compiler tier, and that the alternative is a
  dependency on the seam crate, which is a user decision. Its substitute is a matcher
  over a roster of the functions on the lookup path, refusing allocating spellings in
  their bodies. That reasoning matches this session's own, and the approach is a
  reasonable answer to the gap; adopting it should be a considered call, not an
  accident of the working tree.

**What phase 02 should know.** The emitter omits the `index` line on a partial
selection and writes paths unquoted, both verified acceptable to `git apply`; a
mode-only change and a hundred-percent rename correctly carry no `index` line; the
reverse-apply property does not hold for a selection whose result is not a
well-formed file, and the condition is `diffs::is_a_well_formed_file`.

## 2026-09-17 — packet planned, over three rounds with the user

Six evidence records were gathered before any decision was taken, four in
parallel and two after the user asked for them: the engine and worker as built,
the UI and app as built, the gix diff API verified against the vendored 0.87.1
source, a cross-client precedent study, then a study of Fork's detail pane and
diff view, and git's own timings on a clone of rust-lang/rust made for the
purpose. All six are in `docs/research/diff-engine/` and outlive this directory.

Three rounds: options with recommendations, then detail where the user asked for
it (the hunk source, the working-tree read, the worker threading) plus a
Fork-based layout, then the lock. Sixteen decisions are recorded as L1-L16 in
`brainstorm.md`, with every rejected alternative and the evidence behind it.

Four decisions in the PRD were added while writing it rather than chosen by the
user, and are flagged as such in `brainstorm.md`: that a whitespace-ignoring view
announces what it hides (L4), the 64 MiB ceiling on loading an over-limit file
anyway (L10), that the no-literal-modifier convention becomes a guarded invariant
when the accelerator table exists (L14), and the split of the UI work into three
phases rather than two (L15).

Two facts set the shape more than any preference did. gix computes a diff but
renders no patch Cairn can hand to `git apply`, so the grouping, the headers and
the marker for a missing final newline are Cairn's to write (L3). And a
working-tree read has to run the user's clean filter driver to see what `git diff`
sees, which amends D1 (L6) — a process Cairn causes without building its
environment, stated as a residual rather than papered over.

Nine issues were filed for what the layout deliberately leaves out: #29 to #37.

No implementation has started.
