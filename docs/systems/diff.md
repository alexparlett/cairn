# Diff

How Cairn describes a change to a file today. As-built: everything here is code
that exists, and behaviour is pinned by a test named beside it.

**What exists so far is the model and nothing else.** `cairn-model` can hold a
file diff, project it into hunks and rows, and emit a unified patch from a
selection of lines. No engine query computes one yet, no component draws one, and
nothing stages anything — the patch emitter ships with no caller, deliberately
(program decision L2 in `docs/work/daily-loop/roadmap.md`), because its round-trip
tests are what make a later staging packet a feature rather than a rewrite. Intent
for this surface is `docs/design/cairn.md` (decisions D1, D3, D5, D6) and
`docs/design/ui.md`; the commitment it was built against is
`docs/prd/diff-engine.md`, in flight.

## One exact answer, and projections of it

The load-bearing shape (decision L2 of `docs/work/diff-engine/brainstorm.md`) is
that a file diff holds **one** exact answer and everything else is derived from it
on demand.

`cairn_model::TextDiff` is that answer: both versions of the file as
`DiffLine`s — each line its bytes without the terminator, plus whether it had
one — and the exact `ChangedRange`s between them, each naming a run of removed
lines and the run of added lines that replaced it. Either run may be empty, and an
empty run still says where it sits, which is what an insertion's `-N,0` header is
built from. `split_lines` splits content the way git's own diff does, and
`splitting_and_rejoining_returns_the_same_bytes` pins that a file survives the
round trip, a missing final newline and a CRLF ending included.

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

- *`index_size` measures what was stored, not what was built.* It returns the index's
  own entry count, so a projection that also materialised every row into a `Vec`
  would keep every assertion above green. The `Copy` pin says a row owns nothing;
  neither says the collection was not built. Criterion C9's headless count is what
  decides it, and it belongs to the phase that draws the view.
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
  `a_header_spells_what_git_spells` holds the seven shapes, taken from `git diff`.
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

They take **one file's** patch, which is what `emit_patch` produces.

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
