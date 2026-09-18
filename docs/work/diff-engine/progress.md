# Progress — diff-engine

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

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
