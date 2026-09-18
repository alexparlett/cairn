# Diff

How Cairn describes a change to a file today. As-built: everything here is code
that exists, and behaviour is pinned by a test named beside it.

**What exists is the model and the engine that fills it.** `cairn-model` can hold
a file diff, project it into hunks and rows, and emit a unified patch from a
selection of lines; `cairn-git` answers what a commit or a comparison changed and
what one of those changes is, line by line, against a real repository. No
component draws one and nothing stages anything — the patch emitter still ships
with no caller, deliberately (program decision L2 in
`docs/work/daily-loop/brainstorm.md`), because its round-trip tests are what make
a later staging packet a feature rather than a rewrite. **Working-tree diffs are
not here**: everything below reads trees and blobs from the object database.
Intent for this surface is `docs/design/cairn.md` (decisions D1, D3, D5, D6) and
`docs/design/ui.md`; the commitment it was built against is
`docs/prd/diff-engine.md`, in flight.

## One exact answer, and projections of it

The load-bearing shape (packet decision L2 in
`docs/work/diff-engine/brainstorm.md`, which is a different decision from the
program's L2 above) is that a file diff holds **one** exact answer and everything
else is derived from it on demand.

`cairn_model::TextDiff` is that answer: both versions of the file as
`DiffLine`s — each line its bytes without the terminator, plus whether it had
one — and the exact `ChangedRange`s between them, each naming a run of removed
lines and the run of added lines that replaced it. Either run may be empty, and an
empty run still says where it sits, which is what an insertion's `-N,0` header is
built from. `split_lines` splits content the way git's own diff does, and
`splitting_and_rejoining_returns_the_same_bytes` pins that a file survives the
round trip, a missing final newline and a CRLF ending included.

**What `TextDiff::new` requires of its producer is stated and checked**, because
every projection already depends on it and none of them can see it broken.
Changes arrive in increasing order and do not overlap; the unchanged run between
two consecutive changes — and the run from the start of the file to the first
change — is the same length on both sides; and every range lies inside its own
side's lines. `emit_patch` and the row index each measure that run as the
*smaller* of the two gaps, which loses nothing only while they are equal: unequal
gaps drop their difference from the body while the hunk header is recounted from
the lines actually emitted, so the result is an internally consistent patch that
omits content its own span claims to cover — and real `git apply` is the first
thing in the world to notice. All three conditions are `debug_assert!`ed, so a
producer fails loudly under `cargo test` while a release build keeps the crate's
existing behaviour: malformed input drawn short, never a panic in front of a user.
`an_unchanged_run_of_two_different_lengths_is_refused_in_a_debug_build`,
`a_change_reaching_past_the_end_of_its_side_is_refused_in_a_debug_build` and
`changes_that_go_backwards_are_refused_in_a_debug_build` pin the three refusals,
with `a_well_formed_diff_with_several_changes_is_accepted` as the passing twin
that stops them holding against an assertion which fired on everything.

**Residual, stated rather than implied:** the *trailing* run — from the end of the
last change to the end of each side — is not asserted. It has the same defect
shape, but a `TextDiff` is also legitimately built as a container for one side's
content alone (`splitting_and_rejoining_returns_the_same_bytes` does exactly
that), and refusing that shape was not part of the decision taken. Whether a
producer's two sides end consistently with each other is a review obligation for
the phase that builds the producer.

Three things are projections of that answer and hold no state of their own:

- **Hunks** (`Hunks::of(text, context)`), which group the changed ranges at a
  context setting. Two changes share a hunk when no more than twice the context
  separates them, which is git's own rule and exactly when their context runs
  would touch; `adjacent_changes_merge_at_one_context_and_separate_at_another` and
  `the_merging_rule_is_twice_the_context_inclusive` pin both sides of it.
  `Context::EntireFile` is one hunk holding the whole file.
- **Rows** (`UnifiedRows`, `SideBySideRows`), what a view draws.
- **The patch** (`emit_patch`), what `git apply` takes.

## A changed line is named by its own line number

`Selection` is a set of changed lines, a removed one named by its old line number
and an added one by its new one (R1.3). It records nothing about hunks, context or
which view was on screen, which is why the same selection means the same thing in
every projection — the acceptance criterion C4, pinned by
`a_selection_and_its_patch_are_the_same_in_every_view`
(`crates/cairn-model/tests/diff_patch.rs`). That test walks every fixture at six
contexts in both views, checks each projection offers every changed line, builds a
selection by clicking every one of them in each view, and requires the patch to be
the same bytes each time.

`LineNumber` is the one type both readings go through: it is stored counting from
zero, which is how the lines are indexed, and `one_based()` is the number a gutter
and a hunk header show. Having one type rather than two coordinate systems is what
keeps the off-by-one out of the header arithmetic.

## Rows are reached one at a time

A file is however long somebody's file is, so neither projection builds a row
until it is asked for one (R1.5). What each builds up front is an index of the
diff's *changes*: a hunk header, a run of context, a change, in order. A row
number becomes an index entry and an offset by binary search, so the cost is in
how many separate changes a file has and never in how far down the reader is.
`UnifiedRows::index_size` reports that footprint, and
`one_row_of_a_hundred_thousand_lines_is_reached_through_four_index_entries` pins
it: a hundred-thousand-line file with one change at entire-file context is four
index entries and a hundred thousand and two rows.

A row borrows its line from the diff. Both row types are `Copy`, which a type
holding a `Vec` cannot be, so a row that started carrying its own bytes would stop
compiling — `a_row_owns_nothing_it_would_have_to_allocate` is that pin, and
`a_row_points_at_the_diffs_own_line` checks the borrow is the diff's own line and
not a copy.

`UnifiedRow` is a header, a context line, a removed line or an added line.
`SideBySideRow` adds `Replaced`, which pairs the i-th removed line of a change
with its i-th added line; when one side is shorter, the leftover rows are
`Removed` or `Added`, and the other side is the filler.

**Residuals, stated rather than implied, for the phases that draw this.** These are
model-level pins and each has a hole a mutation walks through:

- *`index_size` measures what was stored; what a LOOKUP builds is measured separately,
  and now pinned.* `index_size` returns the index's own entry count, so a projection
  that also materialised every row into a `Vec` would keep every assertion above green —
  the `Copy` pin says a row owns nothing, and neither says the collection was not built.
  `crates/cairn-model/tests/diff_row_lookup.rs` closes that hole at this level: it counts
  a single `row(k)` on a hundred-thousand-line diff under a counting global allocator
  (`allocation-counter`, a dev-dependency of `cairn-model` alone, which carries the
  `#[global_allocator]` itself and counts per thread) and requires zero allocations, for
  both projections, with `the_counter_sees_an_allocation_when_there_is_one` as the pin
  that the counter is still counting. The mutation it was written against — `row`
  collecting every row and indexing the result — leaves every other model test green and
  turns these two red. What no model-level count can see is how many rows the drawn
  component asks for per frame: that is criterion C9's headless count, and it still
  belongs to the phase that draws the view.
- *The binary search is unpinned.* `row(i)` is cheap because the index is searched
  rather than scanned; rewriting that search as a linear scan gives identical
  answers and keeps every test green, while turning a handful of comparisons per row
  into one per index entry. Nothing here measures comparisons, so keeping the search
  a search is a review obligation, not a guarded fact.
- *Building the projection costs what the file has changes, and it borrows.* `new`
  walks every hunk and every change up front, and the type holds a reference to the
  diff, so it cannot be kept in a view's state and the path of least resistance is
  rebuilding it inside the per-row builder — which would pay that cost per row per
  frame. Build it once per file and context, off the UI thread, and hand it over;
  whether the model needs an owning form is the view phase's call, not something to
  design before there is a caller.
- *Neither projection maps a change to a row number or back*, although the index
  computes exactly that. The previous-change and next-change controls of R6.2 will
  want it; adding it before there is a caller would be inventing an API for a
  hypothetical, so it is recorded here for the phase that builds those controls.
- *The row enums are `RowContent`-class.* `UnifiedRow` and `SideBySideRow` are drawn
  per row by matching on them, so a wildcard arm compiles and silently draws nothing
  the day an expansion row or a marker row lands. No guard covers them — the
  row-content twin reads that one type's spelling — and nothing reads them yet. The
  first view phase should either widen that guard to them or say why not.

## What the engine answers

Two queries in `cairn-git`, both reads, neither spawning a process. gix computes
both diffs and Cairn only groups what it returns (decision L3): no diff algorithm
is written here, and no gix type appears in a public signature.

`DiffSession` (`crates/cairn-git/src/diff.rs`) holds gix's blob resource cache for
a run of queries. Building one reads the index and the attribute stack, which on a
62,892-entry repository is several megabytes; `Repository::changes` and
`Repository::file_diff` are one-shot sessions over it for a caller with a single
question. Like `HistorySession` it borrows the repository and is not `Send`, so it
lives on the worker that owns that handle. The cache is created in
`pipeline::Mode::ToGit` with no worktree roots, which is the mode that never runs a
textconv program, and `gix::diff::resource_cache` builds it with
`skip_internal_diff_if_external_is_configured` off, so a `diff.<driver>.command` in
the user's config is read and never started. Both are checked by running a diff
over a path that has a textconv *and* an external diff command configured, each a
script that writes a sentinel file first thing:
`neither_a_textconv_nor_an_external_diff_program_is_started` requires the sentinel
to be absent afterwards, and then runs the script by hand so the absence is not an
absence of a program that could never have run.

### The changes query

`Repository::changes(&ChangesRequest, &impl Cancel) -> ChangeSet` (R2.1, R2.2,
R2.9, R2.10). A request names one commit — compared with its first parent, or with
the empty tree when it is a root commit (L5) — or two commits, tip against tip and
never against a merge base (R7.2). A merge is compared with its first parent like
any other commit; a combined diff is out of scope by D6.

The answer holds the changed files, the commit's `CommitDetails` when one commit
was named, and a `RenameDetection` reporting gix's counters as plain numbers.
`RenameDetection::was_cut_short()` is R2.2's "the answer says so": it is true when
`diff.renameLimit` stopped the search, which is the fact git prints as "exhaustive
rename detection was skipped due to too many files".

**The file list is sorted here, because gix does not sort it.** gix emits
modifications in traversal order and rename pairs as its tracker finds them. The
key is total — destination path, then source path — so two runs of one query list
the same files in the same places;
`the_file_list_is_sorted_by_path_and_never_shuffles` runs the query twice and
requires both, and requires the key to separate every pair.

**Cancellation stops the walk.** `Cancel` is polled once per change, inside gix's
callback, and a poll that answers yes returns `ControlFlow::Break`, which ends the
traversal rather than letting it finish and discarding the answer. Whether it
broke is recorded on Cairn's side and checked before gix's own error, because gix
reports a break as a failure. `a_cancelled_changes_query_stops_walking` compares
the files collected before the break with the whole answer.
**The gap, stated:** rename detection runs its similarity comparisons *between*
those callbacks, so a superseded query on a rename-heavy commit finishes that
phase before the break is seen. It is bounded by `diff.renameLimit` and by
nothing else.

**A copy's source is put back where git has it.** With `diff.renames=copies`, gix
reports a copy's source as it is AFTER the change and stops reporting that file as
modified at all; git reports the source as it was BEFORE — which is the version in
the index, so it is the version a patch must be built against — and still lists the
file as modified. `repair_copies` reads the source path out of the old tree and
corrects both: one lookup per copy, and nothing at all when copies are not
configured. Without it a copy's patch does not apply and a modified file vanishes
from the list. The similarity percentage stays gix's, and gix measured it against
the other version of the source; where the repaired source is byte-identical to the
copy — git's `C100` — the percentage is set from that fact instead.

### The content query

`Repository::file_diff(&ChangedFile, &ContentOptions) -> FileDiff` (R2.3 through
R2.8). It decides in this order, and the order is the point:

1. **A submodule** answers its two commit ids. gix's blob platform refuses the mode
   outright, so this is settled before anything is asked of it.
2. **A mode change alone** — the same blob on both sides, a different mode — answers
   `ModeChangeOnly` without reading the content. On a commit that renames 27,592
   files this is the difference between a file list and inflating every blob.
3. **The size ceiling, before the content is read** (R2.6). An object's header
   carries its size, so `repo.find_header` decides it without inflating anything.
4. Both sides are set on the resource cache and `prepare_diff` decides **binary**
   the way git does — the `diff` and `binary` attributes, `core.bigFileThreshold`,
   and a NUL byte in the first 8,000 bytes (R2.5).
5. **The line ceilings** — 50,000 lines, 2,048 bytes in a line — are measured over
   git's form of the content, without splitting it into lines.
6. **A Git LFS pointer** is recognised when every side that exists begins
   `version https://git-lfs.github.com/spec/` and is at most 1 KiB.
7. Otherwise the file is diffed as text.

That the size check really precedes the read is pinned deterministically rather
than by timing: `the_size_ceiling_is_decided_before_the_content_is_read` builds a
repository whose over-limit blob is a **loose object truncated after its header**.
gix reads a loose object's header by inflating into a fixed buffer, so the size is
still readable and the content is not — answering "too large" is therefore only
possible without reading it, and asking for it anyway (`load_anyway`) fails, which
is what stops the first half from being a claim about a file that could have been
read either way.

**The tokens keep their terminators.** `gix::diff::blob::sources::byte_lines` is
what the exact diff is computed over, so a last line that lost its newline, and a
CRLF ending that became LF, are changes — exactly as git sees them. gix's own
`interned_input()` strips terminators and would lose both. `split_lines` splits the
same buffer the same way, so there is one `DiffLine` per token on each side and
every range lands inside its own side.

**The algorithm is the user's, and so is the indent heuristic.** `prepare_diff`
resolves `diff.<driver>.algorithm`, then `diff.algorithm`, then gix's default, and
`diff_with_slider_heuristics` is `Diff::compute` followed by `postprocess_lines`,
which is git's `--indent-heuristic`. `diff.algorithm = patience`, the one algorithm
gix lacks, needs no handling of Cairn's own: gix opens a repository leniently and
falls back to histogram itself, which is what R2.4 asks for.

**What `TextDiff::new` requires of its producer, gix gives for free.** Its
`HunkIter` advances both sides over the same unchanged tokens, so the unchanged run
between two changes is the same length on both sides by construction, and a hunk's
range cannot reach past its side. Nothing is converted or clamped at the seam; the
`debug_assert!`s have not fired over any fixture, the Cairn checkout's own history,
or the bench repository.

### The display-only overlay

**Ignoring whitespace** compares lines with every ASCII whitespace byte removed,
which is git's `-w`, and the key is one token per line — so a range over the keys
indexes the original lines one for one and nothing has to be mapped back. The lines
drawn are always the original bytes; only which ranges are marked changes. A blank
line keeps its place with an empty key rather than being dropped, which is what
stops every later range from shifting by one.

**Intra-line highlighting** is always on (L4). The i-th removed line of a change is
paired with its i-th added line, which is the pairing a side-by-side view draws, and
the two are diffed at word granularity: a run of word bytes (ASCII alphanumeric,
`_`, or any byte at or above `0x80`, which keeps a multi-byte character whole), a
run of spacing, or one other byte. Myers, and no indent heuristic, on imara's own
advice about character diffs. A pair where either line is over the long-line limit
is skipped (R2.7). Both sides share one interner and one `Diff` across the file, so
a file of ten thousand changed pairs reuses two allocations.

### What decides the engine, and what it decides against

Real `git`, never the model against itself. The tests live in
`crates/cairn-git/tests/diff/` and every apply runs with `GIT_INDEX_FILE`,
`GIT_OBJECT_DIRECTORY` and `GIT_ALTERNATE_OBJECT_DIRECTORIES` pointed at a scratch
directory, so the repository under test — including this checkout — is only ever
read.

- **C1** (`every_crafted_commit_round_trips_through_real_git_apply`,
  `every_rewrite_commit_round_trips_through_real_git_apply`,
  `every_commit_of_this_repository_round_trips_through_real_git_apply`) applies
  every file's patch, with every line selected, to the parent's tree and compares
  the **tree** `git write-tree` gives with the commit's own. Not the patch text: a
  patch that applies cleanly and stages the wrong bytes passes any comparison of
  strings.
- **C2** (`a_seeded_selection_stages_what_its_patch_says_it_does`) runs twelve
  selections per file — every line, no lines, only additions, only removals, the
  first line of every change, the last line of every change, and eight seeded ones —
  and checks the staged blob against `apply_patch` **and** what the headers claim
  against the index: the mode staged, whether the source path survives, whether
  anything is staged at all. That second half matters because the reference applier
  discards every non-`@@` line, so nothing before this checked a header end to end.
  `the_last_line_of_a_file_with_no_newline_stages_on_its_own` reaches by hand the
  edge a seeded selection may never reach.
- **C3** reverses the same patches onto the commit's tree and requires the parent's.
- **C5** (`every_crafted_commit_lists_what_git_lists` and its neighbours) compares
  with `git diff-tree -r --raw --no-abbrev` under the same config, field for field.
- **C6** (`every_crafted_file_diff_is_the_one_git_prints`) compares the unified
  projection with `git diff -U3` of the same two **blobs** — two blobs rather than
  two commits and a path, so rename detection cannot change what is compared.

### Known limits of the engine

- **gix finds far fewer renames than git once the limit bites.** On
  `5a3292f163d`, git's largest rollup, gix finds 231 rename pairs where git finds
  2,774, and runs zero similarity checks. Two causes, both gix's:
  `gix_diff::rewrites::tracker` compares `diff.renameLimit` against the raw
  permutation count where git compares it against the square (gix's own doc on
  `Rewrites::limit` states git's rule, which the code does not implement), and gix
  has no basename stage in front of its exhaustive one, so when the limit is
  exceeded it falls back to exact matching alone. Raising the limit Cairn passes
  does not close it at an acceptable cost. The measurement and the options are in
  the phase 02 entry of `docs/work/diff-engine/progress.md`.
- **A copy cannot be reverse-applied — by git either.** `git apply -R` of
  `copy from A / copy to B` re-creates A from B and refuses because A is still
  there. `gits_own_copy_patch_cannot_be_reversed_either` pins that git's own patch
  fails the same way, which is why C3 undoes a copy by removing its destination.
- **A type change reads as one file and writes as two.** The changed-file list
  reports git's `T` for one path, and the content query answers a text diff of the
  old bytes against the new. Its patch is two file patches at that path (see below),
  and it is all or nothing.
- **An LFS pointer is a deliberate deviation from git**, which shows a pointer as
  ordinary text. R1.2 and R6.8 commit to the state, and nothing else would ever
  produce it.
- **`Operation::ExternalCommand` is answered, not asserted unreachable.** It cannot
  happen while the cache is built as above; answering it as `Unsupported` means a
  gix that changed that default draws a notice instead of starting a program.

## What a view may see and a patch may not

Ignoring whitespace computes a **second** set of changed ranges, for display only
(decision L4), and intra-line highlighting computes byte ranges inside paired
lines. Both live in `DisplayOverlay`, beside `TextDiff` rather than inside it.

That separation is R1.7, and it is the whole reason the two are different types:
`emit_patch` takes a `&TextDiff`, which has no room for either, so reaching the
whitespace-ignoring ranges from the emitter does not compile rather than being
something a reviewer has to notice. The `compile_fail` doctest on `TextDiff` in
`crates/cairn-model/src/diff_text.rs` pins it, with a passing twin that differs by
its last line alone. (That pin is why the full gate has a `test-doc` step.)

`DisplayOverlay::hides_a_change` answers whether ignoring whitespace swallowed a
change that is really there, which is the notice R6.7 asks a view to show. It reads
both sets of ranges, so it is a question to ask once per overlay and keep. A
highlight is found from either side by search rather than by scan, so a row can
ask for its own tinting without walking the file.

## The patch

`emit_patch(file, text, selection)` returns a `Patch`: bytes for
`git apply --cached`. It takes no context and no view — three lines of context,
always (R1.6) — and there is no argument it could take one through.

The rules, each read out of git's own `add-patch.c` and `apply.c` and matched by
hand against `git diff` output. The tests named below pin the shapes; what proves
them against real `git apply` is criteria C1-C3, which land with the engine in
`cairn-git`:

- An unselected removed line becomes context; an unselected added line is dropped.
  `an_unselected_removal_becomes_context_and_an_unselected_addition_is_dropped`.
- Counts are recounted from the lines actually emitted, and a hunk's new start is
  its old start shifted by the net lines of every hunk emitted before it. A hunk
  with nothing selected is left out, which moves the later hunks' new sides and
  only their new sides:
  `a_hunk_with_nothing_selected_is_left_out_and_the_next_hunks_new_side_shifts`.
- An empty range in a header names the line *before* it and a count of one is left
  out, so a new file writes `-0,0` and a one-line edit writes `@@ -1 +1 @@`.
  `a_header_spells_what_git_spells` holds every shape in that roster, each taken
  from `git diff`.
- `\ No newline at end of file` follows the line it belongs to, whatever marker
  that line ended up with — including a removal that a selection turned into
  context: `the_marker_follows_a_removal_that_was_turned_into_context`.
- **A deletion with some removals left out is a modification of the file**, not a
  deletion of it: no `deleted file mode`, and the new side is the path rather than
  `/dev/null`. `a_deletion_with_a_removal_left_out_is_a_modification`. An addition
  stays an addition however little of it is selected.
- The `index` line is written when the blob moved and left out when only the mode
  or the path did, which is what git does; a patch built from *part* of a
  selection leaves it out, because the content it makes is neither blob and a
  wrong id there is a lie a three-way apply could act on.
  `the_index_line_follows_gits_own_rule`, `a_partial_selection_claims_no_blob_id`.
- Paths go out as the bytes git stores (`RepoPath`), so a patch names the file git
  names even when the path is not text.
- **A type change is two file patches at one path**, the old kind deleted and the
  new kind added, which is what `git diff` writes and the only form `git apply`
  takes: one `diff --git` carrying `old mode`/`new mode` is refused with "wrong
  type", because the preimage git would patch is not the kind the new mode names.
  A type change is all or nothing — half of a file becoming a symbolic link is not
  a state a repository can hold, and the second section would land on a path the
  first had left in place — so a selection that does not hold every change emits
  nothing. `a_type_change_is_written_as_a_deletion_and_an_addition` and
  `a_type_change_that_is_not_wholly_selected_emits_nothing`, with C1 and C3 proving
  both directions against real `git apply`.

## The reference applier

`apply_patch` and `apply_patch_in_reverse` (`crates/cairn-model/src/patch_apply.rs`)
read the unified-diff format and know nothing about the emitter's internals. That
is the point: they exist so a round trip compares two independent readings of the
same rule, and sharing code between them would make them agree with each other
whatever they both got wrong.

They are strict. Every line a patch claims is already there is checked against the
content, its newline included — in both directions, so a missing marker over a
line that never ended is refused as loudly as a marker over one that did — and
every hunk's counts are checked against the lines it carries. An oracle that
accepts anything decides nothing, and
`a_header_that_does_not_match_its_lines_is_refused` and
`a_missing_marker_is_refused_over_a_line_that_never_ended` pin that it does not.

They take **one file's** patch, which is what `emit_patch` produces for every
status but a type change — that one is two sections, and the applier reads the
second as a hunk out of order. What a type change's patch must DO is staged and
checked against the index instead, in C2.

They are **a test oracle and never a write path.** They are public because the
round-trip tests that need them live in another crate, but they are pure
`(content, patch) -> bytes` and touch no repository: every mutation Cairn performs
goes through the `git` binary under `cairn-git::ops`, per decision D1, and nothing
here changes that. A later phase may put them behind `#[doc(hidden)]` or a
test-support feature once there is a caller to gate against.

The round-trip tests judge the emitter against *two* other paths: the applier, and
`diffs::expected_result` in `crates/cairn-model/tests/diffs/mod.rs`, which states
the selection rule with no hunks, headers, context or counts in it at all.
`a_patch_gives_what_the_selection_means_over_every_awkward_case` runs a seeded
sweep of selections over a corpus covering a missing final newline on each side and
on both, a hunk at the first line and at the last, adjacent hunks that merge and
hunks that do not, an empty file, a one-line file, CRLF content, an added file, a
deleted file, a rename with edits and a mode change — each of those twice, once
with blob ids and once without.
`a_patch_for_one_selection_does_not_give_what_another_one_means` is the negative
that keeps the comparison honest.

**Residual for the view phases:** `TextDiff`, `DiffContent` and `FileDiff` derive
`Clone` and `PartialEq`, which a file's worth of lines makes expensive — a toolkit
that compares component props by value on every parent render would pay it every
frame. `cairn_ui::HistoryList` already met this and hand-wrote a `PartialEq` that
compares the collection as a handle rather than by content; the diff view should
follow it rather than passing a diff by value.

## The states that are not text

`DiffContent` is what a file's diff turned out to be: `Text`, or one of the seven
states that stand in place of rows (R1.2) — binary with both sizes, too large with
the limit it crossed, an LFS pointer, a submodule with both commit ids, a mode
change only, conflicted, or unsupported with its reason. `DiffLimits` carries
R2.6's numbers (1 MiB, 50,000 lines, 2,048 bytes in a line, and the 64 MiB ceiling
on loading one anyway) where both an engine and a view can read the same ones.

**Residual, stated rather than implied:** nothing guards that a reader of
`DiffContent` names every variant the way the `RowContent` invariant does for a
history row. A wildcard arm over it would compile and draw nothing for a state
added later. Today the model itself is the only reader; when the views land, C11
("every R6.8 state draws its notice") is what decides it, and whether that
deserves a guard of its own is a question for the packet's QA phase.

## What a commit's details carry

`CommitDetails` is R1.8: the author and the committer as separate `Signature`s,
each with a name, an email and a `Timestamp` that keeps its own offset; the whole
message, with `subject()` and `body()` reading it; and the parents in git's order.
It sits beside `CommitSummary` rather than replacing it — a history row draws a
subject and one name, and carrying a committer, an offset and a whole message per
row of a ten-year monorepo would be paying for what no row draws.
`Timestamp::offset` spells the offset the way git writes it, `+0530` or `-0800`,
pinned by `an_offset_reads_the_way_git_writes_it`.

## Known limits

- **A kept last line that never ended, with lines added after it, makes a file
  whose run-on line is in the middle.** A selection that leaves the removal of an
  unterminated last line out while selecting the additions after it produces
  exactly that, because the format says the marker belongs to the line in front of
  it and `git apply` reads it the same way. The emitter matches git rather than
  second-guessing it; the reverse of such a patch has nothing well formed to work
  on, so `the_same_patch_backwards_undoes_exactly_what_it_did` skips those
  selections and says why. A view that offers line selection should keep the
  combination out of reach.
- **A path is written unquoted.** git quotes an unusual path in its own output and
  `git apply` accepts either form, so this is not a correctness problem for the
  paths git can carry; a path holding a newline or a tab is not handled.
- **No function context after the second `@@`.** git writes the enclosing
  declaration there and ignores it on apply; Cairn writes nothing.
