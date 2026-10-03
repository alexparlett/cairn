# Diff

How Cairn describes a change to a file today. As-built: everything here is code
that exists, and behaviour is pinned by a test named beside it.

**What exists is the model and the engine that fills it.** `cairn-model` can hold
a file diff, project it into hunks and rows, and emit a unified patch from a
selection of lines; `cairn-git` answers what a commit or a comparison changed,
what one of those changes is, line by line, and one path's staged, unstaged or
untracked diff in the working tree, against a real repository; and the
application asks those queries on a thread of their own and keeps each answer for
the selection it names ("In the application", below). The window draws a
commit's details and its changed files in the detail pane's Commit tab ("The
detail pane", below), and a chosen file's diff as unified rows in its Changes tab
("The diff view", below); nothing stages anything — the patch emitter still ships
with no caller, deliberately (program decision L2 in
`docs/work/daily-loop/brainstorm.md`), because its round-trip tests are what make
a later staging packet a feature rather than a rewrite. Which paths of a working
tree changed — status — is not here: the working-tree query answers one path it is
given. Intent for this surface is `docs/design/diff.md` and `docs/design/ui.md`, under
decisions D1 (`docs/design/engine.md`), D3 (`docs/design/concurrency.md`), D5
(`docs/design/platform.md`) and D6 (`docs/design/conflicts.md`), indexed in the
spine `docs/design/cairn.md`; the commitment it was built against is
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

`UnifiedRow` is a header, a context line, a removed line, an added line, or git's
`\ No newline at end of file` after a line that did not end (`NoNewlineAtEnd`). The
unified rows are what `git diff` prints (phase 06): a context line is the NEW side's
line — the side `xdl_emit_diff` prints it from, which differs from the old side under
`-w` (`a_context_line_is_the_new_sides_as_git_prints_it`) — and the marker follows a
removed, added or context line that did not end, a context line's end read from the new
side (`a_line_that_did_not_end_is_followed_by_gits_marker`). A file with no change to
show has no hunk at any context, the entire file included, as git prints none
(`the_entire_file_of_an_unchanged_file_is_no_hunk_as_git_prints_none`; phase 01 drew
such a file whole). `UnifiedLayout` is the owning form a view keeps: the hunks and the
index with nothing of the diff borrowed, asked for a row against the diff and overlay it
was built from (`UnifiedLayout::row`, allocation-free by
`a_row_of_a_kept_layout_costs_no_allocation_however_long_the_diff_is`).
`UnifiedLayout::shown` groups what a view shows — the overlay's whitespace-ignoring
ranges when it holds them, the exact ones otherwise — with the user's
`diff.interHunkContext` (`Hunks::of_ranges`: two changes share a hunk while no more than
twice the context plus it separates them, git's `xdl_get_hunk`;
`the_inter_hunk_context_widens_the_gap_that_merges`). It also answers each drawn
change's rows (`change_rows`, `first_change_from`, `next_change_after`,
`previous_change_before`, by search), which previous and next change move between.
`SideBySideRow` adds `Replaced`, which pairs the i-th removed line of a change
with its i-th added line; when one side is shorter, the leftover rows are
`Removed` or `Added`, and the other side is the filler. The side-by-side rows are
held to the same parity rule as the unified ones (phase 07): `SideBySideLayout`, the
owning twin of `UnifiedLayout`, groups the same ranges at the same context with the
same inter-hunk context (`SideBySideLayout::shown`), a context row's `line` is the new
side's — git's printed line, drawn in both columns
(`a_side_by_side_context_line_is_the_new_sides_in_both_columns`) — and git's end-of-file
marker is a row of its own, `NoNewlineAtEnd { old, new }`, after a change's lines with
the marker in the column of each side whose last line did not end and filler in the
other, or in both columns after a context line that ends the file
(`side_by_side_puts_gits_marker_in_the_column_of_the_side_that_did_not_end`). Read
column by column — each change's left column, then its right — the side-by-side rows
are the unified rows line for line
(`side_by_side_read_column_by_column_is_what_git_diff_prints`), and every parity test
against `git diff` reads them so too (`parity.rs`'s `side_by_side_view`, inside
`compare_commit_at`, so each comparison of the unified rows holds the side-by-side
rows to the same answer). Both layouts expose their changes as `ChangeStops`, what
previous and next change search.

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
- *Building the projection costs what the file has changes.* Closed in phase 06, and
  moved off the UI thread in phase 07: both layouts own their indexes, and
  `cairn_model::ShownDiff::new` builds them once per answer on the diff thread that
  answered it (`worker/diff_lane.rs`), never per row, per frame or per toggle; the window
  only keeps the value (`DiffState::file_arrived` takes a `ShownDiff`).
- *A change maps to its rows* (phase 06): `UnifiedLayout::change_rows` and the searches
  beside it, pinned by `changes_are_found_by_the_row_they_start_on`.
- *The row enums are `RowContent`-class, and guarded* (phase 07). The views that read
  them — `cairn_ui`'s `unified_rows::build` and `side_by_side_rows::build` — name every
  variant, and `every_view_of_a_diff_row_names_every_kind_of_row` holds every production
  reader to that with `reads_enum_partially` (self-test
  `the_diff_row_matcher_catches_the_shapes_it_claims`). What the matcher cannot see — a
  helper handing out one kind of row and read partially, a `type` alias — stays
  `qa-checklist`'s.

## What the engine answers

Three queries in `cairn-git`, and git answers all three (D1 in
`docs/design/engine.md`, "Where git answers a read" and "Reads see git's form"). The **changes query** is answered by `git diff-tree
--raw` (decision E): which paths changed, with their statuses, modes, ids and every
rename and copy pair, are git's own. The **content query** reads both versions of
one file with gix and decides there what is not text; which of its lines changed —
exactly, and ignoring whitespace — and the function context of each hunk are
`git diff-tree -p`'s (the content-parity decision, which amends packet decision
L3; `docs/research/diff-engine/content-parity-spike.md`), and Cairn groups git's
changes into hunks by git's own rule. No diff algorithm is written here but the
intra-line highlights, and no gix type appears in a public signature. The
**working-tree query** answers one path's staged, unstaged or untracked diff with
`git diff-index --cached`, `git diff-files` or `git diff --no-index`, and the side
git reads from the working tree is git's form of the file (below).

`DiffSession` (`crates/cairn-git/src/diff.rs`) holds gix's blob resource cache for
a run of content queries. Building one reads the index and the attribute stack,
which on a 62,892-entry repository is several megabytes; `Repository::file_diff` is
a one-shot session over it for a caller with a single question, and
`DiffSession::changes` is `Repository::changes` on the session's repository, which
needs no cache. Like `HistorySession` the session borrows the repository and is not
`Send`, so it lives on the worker that owns that handle. The cache is created in
`pipeline::Mode::ToGit` with no worktree roots, which is the mode that never runs a
textconv program; its attribute stack is built in git's check-in order — the working
tree's `.gitattributes` first, the index's where the working tree has none, and
`$GIT_DIR/info/attributes` and `core.attributesFile` beside them — which is where
`git diff` and `git show` of a commit read a commit's attributes, an unstaged edit
included (`binary_detection_reads_the_attributes_where_git_reads_them`). gix's own
`Repository::diff_resource_cache` reads the working tree's attributes only when a
worktree root is set, and a root also makes the cache read every resource's content
from the working tree instead of by its id, so the session builds the stack itself
(`Repository::attributes_only` with `WorktreeThenIdMapping`, `IdMapping` in a bare
repository) and hands it to `gix::diff::resource_cache` with no root: the blobs still
come from the object database. Its index is `index_or_empty`: where there is no index
file — a bare repository, or one whose index was removed — git reads no in-tree
attributes at all, never `HEAD`'s, on 2.30.9, 2.32.7 and 2.56.0, where gix's own
cache would load `HEAD`'s tree
(`where_git_reads_no_attributes_the_commit_reads_none`). `gix::diff::resource_cache` builds it with
`skip_internal_diff_if_external_is_configured` off, so a `diff.<driver>.command` in
the user's config is read and never started. Both are checked by running a diff
over a path that has a textconv *and* an external diff command configured, each a
script that writes a sentinel file first thing:
`neither_a_textconv_nor_an_external_diff_program_is_started` requires the sentinel
to be absent afterwards, and then runs the script by hand so the absence is not an
absence of a program that could never have run.

### The changes query

`Repository::changes(&GitBinary, &ChangesRequest, &impl Cancel) -> ChangeSet`
(R2.1, R2.2, R2.9, R2.10). A request names one commit — compared with its first
parent, or with the empty tree when it is a root commit (L5) — or two commits, tip
against tip and never against a merge base (R7.2). A merge is compared with its
first parent like any other commit; a combined diff is out of scope by D6. The
`GitBinary` is the one the application found at startup; the call blocks until its
process ends, so it is a worker's call.

What runs, in order (`crates/cairn-git/src/diff/changes.rs`):

1. **gix reads the commits.** Each id named is found as a commit, so a missing one
   is `Error::ReadCommit` on either side of a comparison, and no process starts
   (`a_commit_that_is_not_there_is_refused`). For one commit, its `CommitDetails`
   come from the same read, and its first parent is what it is compared with —
   or the empty tree, for a root commit and for a shallow clone's boundary commit,
   whose parents the clone does not have and which git's own `git log` shows as a
   root — so its details list no parents, as `git log --format=%P` shows none, and
   the details query (`Repository::commit_details`) agrees
   (`a_shallow_clones_boundary_commit_is_compared_as_git_log_shows_it`).
2. **The user's configuration is read, the way git reads it**
   (`crates/cairn-git/src/diff/renames.rs`). Plumbing reads neither key the way
   the user's own `git log` and `git show` do — `diff.renames` not at all, and
   `diff.renameLimit` only as `-l`'s default — so the two keys are read here, from
   the configuration gix loaded when the repository was opened: the last value
   across every file, a bare key included, parsed by git's own rules
   (`git_config_rename`, `git_parse_maybe_bool`, and `git_parse_int`'s base-0
   `strtoimax` with its `k`/`m`/`g` suffixes and its `int` range). A value git
   refuses is `Error::InvalidConfig`, because the user's own `git log` refuses to
   answer on it too. Unset, detection is renames — porcelain's default — and the
   limit is the default of the git in use: 1,000 from git 2.33, 400 before.
3. **`git diff-tree -r -z --raw --no-abbrev` answers**, in
   `crate::reads::changes` (`crates/cairn-git/src/reads/changes.rs`), as a read
   invocation: `GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`, no askpass token
   (`docs/systems/git-processes.md`). Detection is spelled out — `-M` or `-C` with
   `-l<limit>`, or `--no-renames` — so git searches exactly what the user's
   `git log` would. Neither `--textconv` nor `--ext-diff` is ever passed. The
   `-z` records are parsed as they arrive: a metadata record, then one path, or
   two for a rename or a copy. A record git does not print for two trees — `U`,
   `X`, a score where none belongs, a mode no file has, a path missing at the end —
   is `Error::UnexpectedGitOutput`, and nothing of the answer is used. git failing
   is `Error::GitFailed`, classified by its exit status, never by its stderr.
4. **The answer is sorted, by a total key** — destination path, then source path —
   so two runs of one query list the same files in the same places;
   `the_file_list_is_sorted_by_path_and_never_shuffles` runs the query twice and
   requires both, and requires the key to separate every pair.

**A submodule is listed as `diff.ignoreSubmodules` says**
(`crates/cairn-git/src/diff/submodules.rs`). Plumbing never reads that key, and with it
`all` the user's own `git log`, `git show` and `git diff` list no change to a submodule —
a gitlink added, deleted, or changed as a gitlink on both sides; never a type change —
unless the submodule's own `submodule.<name>.ignore` says otherwise. That own setting is
the configuration's, else `.gitmodules`'s; the name is the one `.gitmodules` gives the
path, the last to claim it; and `.gitmodules` is the working tree's file when there is
one, else the index's, else `HEAD`'s — never the commit being shown — and none in a bare
repository or while it is unmerged. `diff-tree` already applies a submodule's own
setting, so what differs is a submodule with none under a global `all`. Hiding those
after git answers would not match: git hides them before rename detection, so they never
count against `diff.renameLimit`, and an added gitlink can push a search `git log` runs
past the limit. So the query asks git not to queue them: `--ignore-submodules=all` when
no submodule has a setting of its own — exact, since a gitlink's own setting can then
only be unset or `all`; never otherwise, because that flag overrides a submodule's
`none`. Otherwise the first answer's hidden gitlinks are dropped, which is exact with
detection off, and with detection on the query is asked again with each excluded by an
`:(exclude,literal)` pathspec. Only `all` hides anything from a list of changed paths;
the bare key or any spelling but `all`, `dirty`, `untracked` and `none` is
`Error::InvalidConfig`, as `git log` refuses it; a submodule setting git refuses is
read by `diff-tree` itself, which then fails as `git log` does. Each rule was read from
git's source at v2.30.0 and v2.56.0 and reproduced against both. Pinned by
`a_submodule_is_listed_as_diff_ignore_submodules_shows_it` (every setting — unset,
`dirty`, `all`, a submodule's own `none` by name and by path, `untracked` by name, the
working tree's `.gitmodules`, an own `all` over a `.gitmodules` `none`, and `none` with
an own `all` — each against `git log --raw` and `git diff --raw`, with
`diff.renameLimit=1` so a gitlink counted against the limit leaves the rename unpaired),
`a_submodule_setting_git_refuses_is_refused`, `a_bare_repository_reads_no_gitmodules`,
and `a_configuration_git_refuses_is_refused`.

**A root commit is shown as `log.showRoot` says.** With the key true — git's default,
and what the bare key means — a root commit's diff is its whole content (L5); with it
false, the user's own `git log` and `git show` print no diff for a root commit, and the
query answers an empty file list with the commit's details, starting no process. A
shallow clone's boundary commit is a root for this as for everything else. The key is
read for every one-commit query, parsed as `git_config_bool`, so a value git refuses is
`Error::InvalidConfig` whether or not the commit is a root, as `git log` refuses it; a
comparison of two commits is `git diff`'s answer, which reads no `log.*` key, and is the
same under every value. Pinned by `the_root_commit_is_shown_as_log_show_root_says`
(unset, true, false, `0`, `no` and the bare key, each against `git log --raw` and
`git show --raw`, a commit with a parent and a comparison unchanged, and a shallow
boundary), with `a_configuration_git_refuses_is_refused` holding a refused value.
The configuration readers — the last value across every file, `git_config_bool`,
`git_parse_int` — are `crates/cairn-git/src/diff/git_config.rs`.

`ChangeSet` and `RenameDetection` are `cairn-model` types (`crates/cairn-model/src/change_set.rs`),
so a change set crosses the worker boundary and reaches a view as it is, and a view can
draw R2.2's notice without naming the engine.

**How detection went** is `RenameDetection`: whether it was on, whether copies
were, the limit git applied (`None` for none), and `needed_limit`, which is R2.2's
"the answer says so". `RenameDetection::was_cut_short()` is true when
`diff.renameLimit` stopped git's exhaustive search — the fact git prints as
"exhaustive rename detection was skipped due to too many files" — and
`needed_limit` is then the number git's warning asks the limit to be raised to.
Both are decided from git's answer, never from that warning, which is prose in the
user's language. git skips its exhaustive stage when the sources it has left times
the destinations it has left exceeds the square of the limit
(`too_many_rename_candidates` in git's `diffcore-rename.c`), and when it skips,
what it had left are exactly the answer's unpaired paths, while when it does not
they are a superset of them — so the same inequality over the answer's counts is
exact, provided the sources are counted as the git in use counts them: the
unpaired deletions for renames from git 2.31, which culls what the exact and
basename stages paired before the check; every deletion for renames on git 2.30,
which culls nothing; and every deletion and every modified file for copies, on
every version. A limit of zero or less is no limit from git 2.33 and 32,767 before.
Each of these was read from git's source at v2.30.0, v2.31.0, v2.32.0, v2.33.0 and
v2.56.0; against the git that links,
`a_rename_limit_that_cuts_detection_short_is_reported_exactly_when_git_warns`
requires `needed_limit` to equal what git's own warning says, at limits either
side of each boundary, over renames and copies, a commit whose exact renames a
non-culling git would have counted, one paired by name ahead of the limit, and two
deletions by two additions with nothing in common at a limit of exactly 2 — a search
git runs and pairs nothing in, which is where `>` and `>=` part. The gits that link
on a developer's machine and on CI's gate are new, so CI's `git floor` job
(`scripts/git-floor.sh`) builds git 2.30 — the floor — and 2.32 — the last before
2.33 — from source and runs these tests, and the rest of
`crates/cairn-git/tests/diff/`, against each: every version branch above is decided
against the git it is for.

**Cancellation ends the process.** `cancel` is polled by the runner on every tick
while `git` runs — through rename detection too — and once it says the query was
superseded the process group is ended, the query answers `Error::ChangesCancelled`,
and the command log records the invocation as cancelled. A query already superseded
starts no process. Pinned by
`a_changes_query_superseded_by_a_newer_epoch_stops_git_and_reports_it` (an
exhaustive rename search with no limit, over enough dissimilar deletions and
additions that git is still searching when the epoch moves; the log's `cancelled` is
what a cancel that lost the race to git's own exit never says) and `a_changes_query_superseded_before_it_starts_runs_nothing`, both in
`crates/cairn-git/src/reads/mod.rs`, and `a_cancelled_changes_query_answers_cancelled`
from outside the crate.

**It writes nothing.** `the_changes_query_writes_nothing` holds the whole git
directory byte-identical across a copy search, a root commit and a comparison, with
the working tree stat-dirty and a `diff.<driver>.cachetextconv` configured, and the
textconv program never runs. That decides the outcome, not the command: between two
commits porcelain `git diff --raw` and `git log --raw`, and `--textconv` without a
patch, write nothing either. Which command runs, and that it carries no
`--textconv`, `--ext-diff` or patch flag, is pinned on the argument vector by
`the_query_is_diff_tree_and_never_runs_a_program` (`crates/cairn-git/src/reads/changes.rs`).

### The content query

`DiffSession::file_diff(&GitBinary, &ChangesRequest, &ChangedFile, &ContentOptions,
&impl Cancel) -> FileDiff` (R2.3 through R2.9), with `Repository::file_diff` the same
on a session of its own. The request is the one the file's change set answered: its
two commits — a commit and its first parent, the empty tree for a root — are what
git is given (`diff/changes.rs`'s `subject`). `ContentOptions` carries R2.6's limits,
`load_anyway`, `ignore_whitespace` and `context`, the context the view groups at
(three lines by default). It decides in this order, and the order is the point
(`crates/cairn-git/src/diff/content.rs`):

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
7. Otherwise both versions are split into lines (`split_lines`, which splits as
   git does: on `\n` alone, a `\r` kept, a last line that never ended marked), and
   **git says which changed** — unless git has only one answer: a file added,
   deleted or changed in type is one change of every line (git's own patch for a
   type change is a deletion and an addition), a side with no lines makes the other
   side's every line the change, and two sides with the same lines have none. Each
   of those starts no process, and `a_file_git_is_not_asked_about_still_reads_as_git_diff_shows_it`
   holds them — additions, deletions, emptied and filled files under a diff driver
   with an `xfuncname` and without, a rename that kept its blob (which git prints
   with no hunk) and a type change (which git prints as a deletion and an addition,
   read against git's two sections) — to `git diff`'s answer, function context
   included, which git prints none of for a hunk starting at the first line, and to
   no process started.

That the size check really precedes the read is pinned deterministically rather
than by timing: `the_size_ceiling_is_decided_before_the_content_is_read` builds a
repository whose over-limit blob is a **loose object truncated after its header**.
gix reads a loose object's header by inflating into a fixed buffer, so the size is
still readable and the content is not — answering "too large" is therefore only
possible without reading it, and asking for it anyway (`load_anyway`) fails, which
is what stops the first half from being a claim about a file that could have been
read either way.

**The read.** `crate::reads::patches` (`crates/cairn-git/src/reads/patches.rs`)
runs, as a read invocation (`GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`, no
askpass token):

```text
git --literal-pathspecs -c diff.suppressBlankEmpty=false diff-tree -r -z --raw
    --no-abbrev -p --full-index -U<n> --no-ext-diff --no-textconv --no-color -a [-w]
    [--diff-algorithm=<algorithm>] <detection> --end-of-options <old> <new> -- <new path> [<old path>]
```

- `-U<n>` is the view's context, never zero: at zero git trims the tail the two
  sides share before it diffs (`trim_common_tail`), which changes the script; at
  any context of one or more the script is the one `git diff -U3` shows. The entire
  file asks at one.
- Each change is read as a **maximal run of `-` and `+` lines**, never from the `@@`
  headers, which group changes at the context. `\ No newline at end of file` is read
  by its first byte and marks the line before it.
- **Every line git prints is checked against the lines gix read**: a removed line
  against the old side, an added one against the new, a context line against the
  new side (git prints context from it) and, unless whitespace is ignored, the old.
  A difference — bytes, newline, or a line past the end — is
  `Error::ContentReadsDisagree`, which names the file and what differed and which a
  caller answers by asking again; nothing of either read is drawn. Hunks whose gaps
  differ between the two sides are refused the same way, so a `TextDiff` built from
  git's answer always meets its own preconditions.
- The **detection pairs the file's paths**: `-M` for a rename, `-C
  --find-copies-harder` for a copy (so the source is a candidate whether or not it
  changed), `--no-renames` otherwise; both paths are the pathspec, read literally
  (`--literal-pathspecs` is a global option, before the verb). A copy whose source
  also changed answers two files; `--raw -z` in the same call says which patch is
  which, in order, and the one whose kind, paths and blobs are the change set's is
  taken.
- `-c diff.suppressBlankEmpty=false`, since plumbing reads that key and with it a
  blank context line loses its marker. `-a`, since the file is already known to be
  text by git's rules.
- **The algorithm is the user's `git diff`'s** (`crates/cairn-git/src/diff/algorithm.rs`).
  Plumbing never reads `diff.algorithm`, so it is read here — the last value, git's
  four names in any case and `default`, a value git refuses or the bare key
  `Error::InvalidConfig` — and passed as `--diff-algorithm`, always in that long
  spelling. From git 2.40 a diff driver's `diff.<driver>.algorithm` beats it for a
  path whose OLD side's `diff` attribute names the driver (`run_diff` looks the
  driver up by `one->path`), and `diff-tree` applies a driver's algorithm only when
  no `--diff-algorithm` is given — so for such a path no flag is passed. Whether a
  driver applies is asked of git itself, `crate::reads::diff_attributes`
  (`git check-attr --stdin -z diff`, the paths on stdin): the same `git_check_attr`
  `git diff` makes, so the working tree's `.gitattributes`, the index's, the
  `info` file and `core.attributesFile` are read where git reads them. It runs only
  when the git in use reads driver algorithms and the configuration names one git
  parses — never otherwise. Before 2.40 git has no such key; neither has the query.
  `diff.indentHeuristic` plumbing reads itself, so nothing is passed. Pinned by
  `every_discriminating_file_reads_as_git_diff_shows_it_under_every_configuration`
  and `a_drivers_algorithm_applies_where_git_says_the_driver_does` (a driver handed
  files by an uncommitted `.gitattributes`, with `check-attr` required to run from
  2.40 and not before), and the configuration's reading by
  `diff_algorithm_is_read_the_way_git_diff_reads_it` and
  `a_drivers_algorithm_counts_from_git_2_40_when_git_can_use_it`.
- **Ignoring whitespace** is the same read with `-w` (R2.8). git leaves a file whose
  every change is whitespace out — from its raw records too on git 2.56, from its
  patches alone on 2.30 through 2.40, which list it and print nothing — so its
  absence is that answer, no changes; a listed file is matched to its patch by the
  blobs on its `index` line (`--full-index`) so that a patch left out does not hand
  the next file's to it.
- **Function context**: the text after each header's closing `@@`, taken verbatim
  from git's own header. git finds it by searching the old side backwards from the
  line above where the hunk starts, so it depends on where a hunk starts and so on
  the context — which is why the read runs at the view's. It reaches the model as
  `FunctionContext` (below).

`cancel` is polled by the runner on every tick, so a superseded content query ends
its `git` and answers `Error::ContentCancelled`; one already superseded starts
nothing (`a_content_read_superseded_by_a_newer_epoch_stops_git_and_reports_it`,
over a `minimal` diff of two 60,000-line files that takes git seconds, and
`a_content_read_superseded_before_it_starts_runs_nothing`, both in
`crates/cairn-git/src/reads/mod.rs`). Which flags the read passes is pinned on its
argument vector (`a_file_read_is_diff_tree_with_a_patch_and_never_runs_a_program`,
`the_read_is_check_attr_of_diff_with_its_paths_on_stdin`), and that it writes
nothing and runs nothing but the repository's `core.fsmonitor` by the outcome:
`the_content_query_writes_nothing_and_runs_nothing` configures a caching
textconv, `diff.external`, a driver `command`, a clean and a smudge filter, a
driver algorithm and `core.fsmonitor`, leaves the working tree stat-dirty and
content-dirty, runs every file's query and Expand All with and without `-w`, and
requires the git directory byte-identical, `check-attr` in the command log on git
2.40 and later, and no program's mark but the fsmonitor hook's — then runs the
programs by hand. The hook runs because `diff-tree` and `check-attr` run it as
they read the index of a repository with a working tree, as the user's own
`git diff` does; the user decided to allow it as parity (`crate::reads`, "What a
read may run"). A bare repository planted to name one is refused when it is
opened (`docs/systems/git-processes.md`, "Where an invocation runs").

**Expand All, and files opened in place, a page at a time** (phase 08, R5.3; the bound
phase 02's QA asked for). `DiffSession::page(git, request, Offered { changes, files },
budget, options, cancel)` reads the files `files` names of a change set, in order, each
decided as one file is, until the page is full — `PAGE_FILES` (256) files, or
`PAGE_LINES` (20,000) lines read on it, ending at the file that crossed it — or the
`LineBudget` given is spent; the `Page` it answers holds each file read, by its index in
the change set, with its outcome, and `taken`, how many of the files offered it decided,
which is where the next page starts. **Whether a file is admitted is decided before any
of its blobs is read**, from what the files before it cost: a file costs one, for itself,
and every line of both its versions it holds — a state that is not text, refused before
its lines are split (too large on the object's header, binary, a submodule, a mode alone,
an LFS pointer), holds none — so the most a budget holds is itself and the one file that
crossed it, which R2.6's ceilings bound
(`expand_all_stops_at_its_budget_before_reading_the_next_file`: the file after the one
that spent the budget is never read, nor named to git; `a_page_ends_at_its_file_count_or_its_line_count`).
**A file that fails is that file's outcome** (phase 04 QA's R1): a blob gix cannot read,
a read git fails or one that disagrees with the lines read is an `Err` beside the other
files' answers, and only what no file is to blame for — a configuration git refuses, the
diff attributes that could not be asked, being superseded — fails the page
(`a_failing_file_is_that_files_outcome_and_the_rest_are_read`, a blob removed from the
object database). The lines of the page's text files git has to read come from as few
`git diff-tree -p` runs over **the page's paths alone** (`Scope::Paths`, literal
pathspecs, the change set's own detection, no `-a`) as the algorithms allow: one for the
files diffed with `diff.algorithm`, and one per distinct algorithm the page's diff
drivers name (git 2.40 and later), since one call cannot pass two algorithms, and one that
leaves a driver to apply its own carries it into every later file (the known limit
below). So what the engine holds is a page's blobs and a page's patch, never the
comparison's. Each text file's patch is found by its paths and checked to be the same
change between the same blobs; a file a run's answer does not hold that way — paired
otherwise, which a page's narrower paths can do, or called binary — or whose run failed
is asked about on its own. `expand_all_answers_what_each_file_answers_alone` requires
every answer, read page by page, equal to the per-file one, with and without `-w`, over
the crafted, rewrite, submodule, attribute, whitespace and discriminating fixtures, the
last with a driver algorithm; `expand_all_runs_one_diff_tree_per_comparison` holds the
runs to one per page and per driver algorithm, never one per file; and
`a_path_git_quotes_reads_as_git_diff_shows_it_alone_and_through_expand_all` and
`a_renamed_files_driver_algorithm_is_its_old_paths` hold paths git quotes and a
rename across a driver's boundary (the driver is the old path's, as in git's
`run_diff`) to `git diff` alone and through Expand All. (`DiffSession::file_diffs`, which
read every file of a change set and held the whole comparison's patch, is gone.)
On git 2.40 and later, `git show` of a whole commit carries a driver's algorithm into
every later file of the same output; Cairn answers each file as `git diff -- <path>`
does instead (see the known limits).

### The working-tree query

`Repository::working_tree_diff(&GitBinary, &RepoPath, WorkingTreeDiff, &ContentOptions,
&impl Cancel)` (and `DiffSession::working_tree_diff`, on a session's repository) in
`crates/cairn-git/src/diff/working_tree.rs`, over the read
`crate::reads::working_tree_patch` in `crates/cairn-git/src/reads/working_tree.rs`.
`WorkingTreeDiff` is `Staged` (`HEAD` against the index), `Unstaged` (the index
against the working tree) or `Untracked` (nothing against the working tree). The
answer is `Option<FileDiff>`: `None` exactly where the user's `git diff --cached
-- <path>`, `git diff -- <path>` or `git diff --no-index /dev/null <path>` prints
nothing — a clean path, one whose stat alone moved, a CRLF checkout of an LF file
under `text=auto`, a submodule its `ignore` setting hides, an intent-to-add path's
staged side.

**What runs.** One read process per question:

```text
git -c diff.suppressBlankEmpty=false <verb> -z --raw --no-abbrev -p --full-index -U<n>
    --no-ext-diff --no-textconv --no-color [-w] [--diff-algorithm=<a>]
    [--ignore-submodules=<v>] <what> -- <path>
```

with `diff-index --cached --no-renames --end-of-options <HEAD or the empty tree>`,
`diff-files --no-renames`, or `diff --no-index` and `/dev/null <path>` — `<path>`
relative to the top of the working tree, and `./-` for the path `-` — porcelain,
the one exception to a read's plumbing-only rule (accepted by the user, with the
evidence in `crates/cairn-git/src/reads/mod.rs`). Being porcelain it reads the user's
presentation settings, so the `--no-index` read also sets each back to git's default
with `-c` (`NO_INDEX_PRESENTATION` in `reads/working_tree.rs`: `diff.noprefix`,
`diff.mnemonicPrefix`, `diff.srcPrefix`, `diff.dstPrefix`, `core.quotePath`,
`diff.interHunkContext`, `diff.relative`, `diff.orderFile`,
`diff.suppressBlankEmpty` — each found by experiment to change its output on git
2.30.9, 2.32.7 or 2.56.0, the rest already decided by a flag), and keeps what decides
git's form of the file (`core.autocrlf`, `core.eol`, the attributes, the filter
drivers); `an_untracked_answer_is_the_same_under_hostile_presentation_settings` sets
all of them hostile, with colour forced and an external diff configured, and the
answer is unchanged and still `git diff --no-index`'s. The two
plumbing verbs take `:(literal)<path>` and `:(exclude,glob)<path, escaped>/**`, so a
directory of that name on the other side is not listed with it and a gitlink at the
path still is (`:(exclude,literal)<path>/` excludes the gitlink too, on every git
from 2.30.9). No `-a`: git decides what is binary, on git's form of the content. A
second run with `-w` when the view asks to ignore whitespace and the file was
modified; a `check-attr` before them when a driver may name its algorithm (2.40+),
as for a commit; nothing else. Each runs with the read environment of
`docs/systems/git-processes.md`. `diff-files` and `diff --no-index` run the path's
clean filter driver as git's child — once for the diff, once more to hash the
working tree for the patch's `index` line (a `-w` read, four times on git 2.56.0;
a raw-only read, never). A long-running `filter.<driver>.process` (what `git lfs
install` configures) is started once per read and sent `command=clean` for each,
and nothing else
(`a_long_running_filter_process_is_sent_only_clean_and_its_form_is_diffed`, a
pkt-line server in `sh` and `dd`, compared with `git diff`). The driver runs with
that environment plus what git sets for a filter (`GIT_DIR` and `GIT_WORK_TREE`
only when the repository is named to git, which it is when opened with full trust;
`GIT_EXEC_PATH`, `GIT_PREFIX`, `GIT_CONFIG_PARAMETERS`, git's exec directory first
on `PATH`), its stderr the read's bounded tail; `diff-index --cached` reads only objects. Every read of the
index runs the repository's `core.fsmonitor`, and for a submodule `diff-files` runs
`git status` inside it to say whether it is dirty. Pinned by
`a_working_tree_query_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor`
(the git directory byte-identical after every query, staged, unstaged and
untracked, with and without `-w`; no textconv, external diff, driver `command` or
smudge filter run, the clean filter and the hook run; every logged verb one of the
four) and `a_clean_filter_drivers_form_is_what_is_diffed_and_it_runs_under_git`
(the driver's recorded environment carries the read's variables and not the test
process's own).

**git's form, by construction.** Cairn never reads a working-tree file's bytes for
the diff. The side git read from the working tree is rebuilt from git's patch over
the old side (`PatchText::new_side`): every line outside a hunk is the old side's,
every context and added line the one git printed — so what Cairn holds is what git
diffed, after the clean filter, `text=auto`/`eol`/`core.autocrlf`, `ident` and a
working-tree encoding. The old side is the blob git names on its raw record, read
by gix by that id; a staged diff's new side is the index's blob, likewise. Pinned by
`a_text_auto_file_with_crlf_endings_is_no_change_and_an_edit_is_one_line` and the
clean-filter test above (an upper-casing driver: the working tree is lower case,
the lines held are upper case), each compared with `git diff`.

**The stale-read guard** is `Error::ContentReadsDisagree`, three ways: every line git
prints is checked against the side it belongs to; the rebuilt working-tree side is
hashed (`gix::objs::compute_hash`, written nowhere) and must be the id git printed
on the `index` line, which git computed from a second read of the file — so a file
that changed between git's two reads is refused, not drawn
(`content_that_changes_between_gits_reads_is_the_error_a_caller_retries`, with a
clean filter whose output changes every run); and the `-w` run must name the same
object for that side
(`a_whitespace_ignoring_read_of_other_content_is_the_error_a_caller_retries`, with a
filter stable within one git process and not across two). The id is also what the answer reports as the new side's
`new_id`: the object `git hash-object --path` would name.

**What gix decides first**, from the index read fresh for every query
(`open_index`, never the shared snapshot): a path with any stage but 0 is
`DiffContent::Conflicted`, from both the staged and the unstaged query, and git is
not asked (`a_conflicted_path_answers_conflicted`, where git itself prints `diff
--cc` and "Unmerged path"); a sparse index — one with directory entries — is
`Unsupported` with its reason (`a_sparse_index_is_unsupported_and_says_so`, skipped
on a git before 2.32, which cannot write one); an intent-to-add entry's staged diff
is `None`, because `git diff --cached` lists nothing for it where `diff-index
--cached` lists an empty file added
(`an_intent_to_add_path_is_new_unstaged_and_nothing_staged`); a bare repository is
`Unsupported`. An untracked path must be relative to the top of the working tree —
empty, absolute, or with a `.` or `..` component, it is `Error::NotAWorkTreePath`,
since `--no-index` reads whatever it is given — and must be a regular file or a
symlink by its own metadata: a named pipe (which git 2.56 waits on for a writer),
another special file or a directory is `Unsupported`, with git never started
(`an_untracked_path_outside_the_working_tree_or_not_a_file_is_refused_before_git_runs`). For these stand-in states the file carries the path and no mode or
id. And the blobs' sizes come from their headers, so a blob past R2.6's ceiling is
refused before git diffs it, git asked only for the raw record — except for a
working-tree modification, whose record cannot tell an edit from a stat or a mode
that alone moved: git is asked for its patch, under the ceiling, so a large file
merely touched is no change and one merely made executable a mode change, as `git
diff` shows them, and an edit to it is refused without its blob read; and a staged
record naming one blob under two modes is a mode change alone, `ModeChangeOnly`, as
`git diff --cached` shows it and as a commit's answers
(`a_large_file_whose_stat_or_mode_alone_moved_is_what_git_diff_shows`).

**Each R3.4 state**: a deletion, a type change (one change of every line, the new
side taken from git's addition section), a mode change alone (`ModeChangeOnly`,
told by a patch section with no hunks and no `index` line), each staged and
unstaged (`a_deleted_file_a_type_change_and_a_mode_change_answer_their_state`); a
stat-only change, which `diff-files` lists with no patch, is `None`
(`a_stat_dirty_file_is_no_change`); a submodule is `DiffContent::Submodule` with
the commit each side names and `dirty` — git's `Subproject commit <id>-dirty`
line, read from git — moved, dirty, both, and staged
(`a_submodule_answers_its_commits_and_whether_it_is_dirty_as_git_diff_shows_it`).
Plumbing reads no `diff.ignoreSubmodules`, so the query passes
`--ignore-submodules=<value>` where porcelain would apply it — for a submodule with
no `ignore` of its own in the configuration or `.gitmodules`, which beats it and
which plumbing applies itself (`diff::submodules::working_tree_ignore`); the same
test holds Cairn to `git diff` under each of `none`, `untracked`, `dirty` and
`all`, and under a submodule's own setting.

**R2.6 on the working tree** is judged on git's form, not on the file on disk: a
clean filter may make a large file small (git-lfs) or a small one large. git's
output is taken under a ceiling of twice what both sides could hold within the
limit, counting a working-tree side at the limit (`Invocation::finish_within`, the
runner's bounded stream); past it, git is ended and the answer is `TooLarge`,
measured at the larger of the file on disk and the limit's next byte. Under it,
the sizes of the held sides decide, then the line limits, then the LFS pointer
check. `the_size_ceiling_is_judged_on_gits_form_of_the_working_tree` holds a file
grown past the limit in the working tree, loading it anyway, a file a filter
shrinks (diffed in its small form, as `git diff` shows it), one a filter grows
(refused without the rest of git's output read) and an index blob past the limit.

**Failures.** `git diff --no-index` exits 1 both when the sides differ and when it
cannot read the file; the read takes status 1 as an answer only when a record for
the path came with it, and otherwise surfaces git's failure. A clean filter marked
`required` that fails, or whose program does not exist, is git's `fatal` (status
128) and `Error::GitFailed`, carrying git's diagnostic on stderr; one not required
makes git fall back to the unfiltered content, which is what Cairn shows, as `git
diff` does (`a_failing_clean_filter_is_gits_failure_with_its_diagnostic_or_what_git_shows`).

**Parity, beyond the states.**
`every_discriminating_file_reads_as_git_diff_shows_it_staged_and_unstaged` puts the
discriminating fixture's commit back into the working tree — unstaged after `reset
--mixed`, staged after `reset --soft` — and holds every changed file to `git diff`
and `git diff --cached` under the default algorithm, patience, and histogram with a
driver naming minimal, with and without `-w`, function context included;
`staged_and_unstaged_edits_read_as_git_diff_shows_them` does the same for one file
staged and edited again; `line_endings_at_the_edges_read_as_git_diff_shows_them`,
staged and unstaged, for an edit beside a last line with no newline, a change to the
final newline alone, each way, and a CRLF file kept under `-text` edited mid-file;
`an_untracked_file_is_what_git_diff_no_index_shows` for plain text, no final newline,
an empty file, a symlink, a binary file, a path holding `*` and a leading `-`, and
files named `-x` and `-` (the last given to git as `./-`, since `--no-index` reads `-`
as stdin, and its record named `-` again);
`a_staged_file_on_an_unborn_branch_and_beside_a_directory_is_that_file` for the
empty tree and a file where `HEAD` had a directory. The same suite runs on git
2.30.9 and 2.32.7 in `git-floor`.

### The display-only overlay

**Ignoring whitespace** is `git diff -w`'s second set of ranges, over the same
original lines; only which ranges are marked changes.

**Function context** is `cairn_model::FunctionContext`: what git printed after each
hunk header's `@@`, keyed by the line the hunk starts at on the old side, with the
context it was read at. Its text depends on the start alone, so the exact hunks and
the whitespace-ignoring ones share one; `FunctionContext::of(HunkHeader)` answers
`Some` — empty where git prints none — for a hunk starting where git printed one,
and `None` where it printed none, which a hunk grouped at another context than the
query's mostly gets. Pinned against `git diff` at one, three, five and eight lines
of context, with git's default rule and a driver's capturing `xfuncname`, by
`the_function_context_is_git_diffs_at_every_context`. It also carries the user's
`diff.interHunkContext` (`FunctionContext::inter_hunk_context`), read by the engine per
query as porcelain reads it (`crate::diff`'s `hunk_grouping`; plumbing never reads it),
which a view groups hunks with: git is not asked with it, since the changed ranges do
not depend on grouping and a merged hunk starts where its first part does, at a start
git printed text for. A value git refuses (`git_config_int`'s parse, or negative) for it
or for `diff.context` is `Error::InvalidConfig` for every content and working-tree
query, as the user's `git diff` refuses to run (`a_value_git_refuses_is_refused`).

**Intra-line highlighting** is always on (L4) and stays gix's: git has no
equivalent. The i-th removed line of a change is paired with its i-th added line,
which is the pairing a side-by-side view draws, and the two are diffed at word
granularity: a run of word bytes (ASCII alphanumeric, `_`, or any byte at or above
`0x80`, which keeps a multi-byte character whole), a run of spacing, or one other
byte. Myers, and no indent heuristic, on imara's own advice about character diffs.
A pair where either line is over the long-line limit is skipped (R2.7). Both sides
share one interner and one `Diff` across the file, so a file of ten thousand changed
pairs reuses two allocations.

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
  strings. A file the model has no patch for — binary, a submodule — is staged
  directly so the trees can still be compared, which proves nothing about the
  emitter; so each test's floor counts the files staged through a patch, and a
  content query that answered every file as binary fails all three.
- **C2** (`a_seeded_selection_stages_what_its_patch_says_it_does`) runs twelve
  selections per file — every line, no lines, only additions, only removals, the
  first line of every change, the last line of every change, and eight seeded ones —
  and checks the staged blob against `apply_patch` **and** what the headers claim
  against the index: the mode staged, whether the source path survives, whether
  anything is staged at all. That second half matters because the reference applier
  discards every non-`@@` line, so nothing before this checked a header end to end.
  It requires the crafted history to reach it with an added, a deleted, a modified
  and a renamed file, a mode change beside a hunk (`old mode`/`new mode` and a body
  in one patch), a file with two hunks at three lines of context, and an edit inside
  a CRLF file, whose unchanged lines are context that must keep their `\r`.
  `the_last_line_of_a_file_with_no_newline_stages_on_its_own` reaches by hand the
  edge a seeded selection may never reach.
- **C3** reverses the same patches onto the commit's tree and requires the parent's.
- **C5** (`every_crafted_commit_lists_what_git_lists` and its neighbours) compares
  with `git diff-tree -r --raw --no-abbrev` under the same config, field for field:
  a root commit, a merge against its first parent, a comparison and its swap,
  renames, copies when configured, and a limit that cuts detection short. Since
  git answers the query, what these decide is that Cairn asks git the right
  question and reads its answer whole. `the_answer_is_what_git_log_shows_under_each_configuration`
  decides the question against what the user sees: for each spelling of
  `diff.renames` (unset, the words, numbers, the bare key, `copy`) against each of
  `diff.renameLimit` (unset, `1`, `0`, `-1`, `1k`, `0x1`), every commit's list
  equals porcelain `git log --raw`'s and its `needed_limit` equals git's warning;
  `a_configuration_git_refuses_is_refused` holds a value git refuses to
  `Error::InvalidConfig`.
- **C6** (`crates/cairn-git/tests/diff/parity.rs`) compares the unified projection,
  each header with its function context, with the user's own porcelain `git diff`
  for the file's paths and the detection that pairs them (a type change against
  `git diff` of its two blobs, since git prints it as two files): the crafted
  history at one and three lines (`every_crafted_file_diff_is_the_one_git_prints`);
  a seeded fixture the four algorithms, the indent heuristic and a driver's
  algorithm are first shown to disagree on, under every configuration in the test's
  `CONFIGURATIONS` table
  (`every_discriminating_file_reads_as_git_diff_shows_it_under_every_configuration`);
  this repository's recent non-merge commits under each algorithm,
  in a clone sharing its objects so no configuration is written to the checkout
  (`this_repositorys_history_reads_as_git_diff_shows_it_under_every_algorithm`); and
  `git diff -w` over whitespace-only, real and mixed edits, with R6.7's notice
  (`ignoring_whitespace_reads_as_git_diff_w_shows_it`). No divergence is allowed in
  any. Dropping `--diff-algorithm`, or asking at `-U0`, turns them red.

### Known limits of the engine

- **The configuration is gix's view of it, as of when the handle was opened.** The
  two rename keys are read from the configuration gix loaded, as is every other key
  gix reads, `diff.algorithm` among them. A handle does not see a later edit; the
  application's diff thread opens its handle again when a configuration file moves
  ("In the application", below), and a caller holding a handle of its own must do
  the same. And gix
  reads from Cairn's own environment, which may carry `GIT_CONFIG_GLOBAL`,
  `GIT_CONFIG_COUNT` and the rest, where the `git` process's environment carries
  none of them (`docs/systems/git-processes.md`); since detection and the limit are
  passed to git spelled out, what decides the search is gix's view, which is the
  one the user's own shell has.
- **In a partial clone, a query that needs an object the clone lacks fails rather
  than fetching — from git 2.44.** A read never fetches (`GIT_NO_LAZY_FETCH=1`), so
  the query answers `Error::GitFailed` where the user's own `git log` would fetch and
  answer. In a blob-less clone (`--filter=blob:none`) that is a rename search, which
  compares contents; with detection off only trees are read, and the clone answers —
  `in_a_partial_clone_a_rename_search_fails_rather_than_fetching` pins both. A
  tree-less clone (`--filter=tree:0`) lacks the trees too, so there the query fails
  even with detection off. Git older than 2.44 ignores the variable and fetches what
  it lacks instead: a pack written into the repository and the network reached, by a
  read, and possibly once per missing object, so per query (reproduced with git 2.56
  without the variable: one `diff-tree` over a tree-less clone wrote twelve packs).
- **A submodule replaced by a directory of its own name, under a hidden-submodule
  setting with an exception, is filtered rather than excluded.** When
  `diff.ignoreSubmodules=all` hides a gitlink, some other submodule has a setting of
  its own, and rename detection is on, the hidden gitlink is excluded by a pathspec —
  but a pathspec naming `s` also excludes everything under a directory `s/`, so when
  the same commit puts files there the gitlink's row is dropped from the answer
  instead, and the search git ran still counted it against `diff.renameLimit`. Not
  pinned: it needs all four at once.
- **`.gitmodules` is parsed by gix.** A file git's own config parser refuses but gix
  reads — a syntax gix tolerates — can be answered under `diff.ignoreSubmodules=all`
  where the user's `git log` refuses the commit. A file gix cannot parse, or one
  holding a value git's submodule parser dies on, takes the cautious path, where
  `diff-tree` reads the file itself and refuses as `git log` does.
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
- **A content query costs one or two `git` processes** — two with whitespace
  ignored, one more for `check-attr` when a driver algorithm may apply — where it
  cost none; what that costs on the bench repository's largest subject, against the
  in-process diff, is measured in `docs/research/diff-engine/content-parity-spike.md`.
- **A whole commit's output under a driver algorithm is not `git show`'s.** From
  git 2.40, git applies a driver's algorithm by changing its own diff options, so in
  one `git show` or `git diff` of several files every file after one with a driver
  algorithm is diffed with that algorithm too (reproduced with 2.40.0 and 2.56.0).
  Cairn answers each file as `git diff <old> <new> -- <path>` does, which is the
  same for every file of the commit whatever its order.
- **`diff.context` and `diff.interHunkContext` are read as porcelain reads them, not
  passed to git.** The plumbing the content query runs reads neither, so
  `diff/hunk_grouping.rs` reads both (phase 06): `diff.context` is where a view's
  context starts (`Repository::configured_context`), the view's own context is the
  `-U<n>` git is asked at, and `diff.interHunkContext` rides in each answer's
  `FunctionContext` for the view to group with (`Hunks::of_ranges`, twice the context
  plus it). A value git refuses is `InvalidConfig` for every content query — one file,
  Expand All and the working tree
  (`a_grouping_git_refuses_refuses_every_content_query`) — and Expand All groups as each
  file alone does (`the_view_groups_hunks_as_the_users_git_diff_does`).
- **A diff driver named `set`, `unset` or `unspecified`** reads, through
  `git check-attr`, as that state of the attribute rather than as a driver.

## What a view may see and a patch may not

Ignoring whitespace computes a **second** set of changed ranges, for display only
(decision L4), intra-line highlighting computes byte ranges inside paired
lines, and git's function context is the text a header shows after its `@@`. All
three live in `DisplayOverlay`, beside `TextDiff` rather than inside it, so the
patch is the same bytes whatever git printed there.

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
the limit it crossed, an LFS pointer, a submodule with both commit ids and whether
a working tree's checkout of it is dirty (false wherever the new side is a commit
or the index), a mode
change only, conflicted, or unsupported with its reason. `DiffLimits` carries
R2.6's numbers (1 MiB, 50,000 lines, 2,048 bytes in a line, and the 64 MiB ceiling
on loading one anyway) where both an engine and a view can read the same ones.

A reader of `DiffContent` names every variant, as the `RowContent` invariant has a
history row read (`every_view_of_a_file_diff_names_every_state`, phase 05). Phase 07
draws each state (C11; "The diff view", below): `cairn_ui::DiffNotice::of` reads a
prepared diff by naming every state and says what stands in its place.

## In the application

The window asks for diffs through the worker boundary
(`crates/cairn-app/src/worker/`), as it asks for history pages, and never waits
on one.

**Requests and answers name their target** (PRD R4.4,
`crates/cairn-app/src/worker/request.rs`). `Request::Changes { of }` asks what a
`Comparison` — one commit, or two tip against tip — changed, and
`Update::Changes { of, changes }` answers with the `ChangeSet`.
`Request::FileDiff(FileQuery)` asks for one file's diff: a `FileTarget`
(`Committed { of, file }`, the `ChangedFile` exactly as the change set named it,
or `WorkingTree { path, side }`) and the `DiffOptions` the view chose — the
context git is asked at, whether to compute the whitespace-ignoring ranges, and
whether to load past R2.6's byte ceiling; the ceilings themselves are fixed.
`Update::FileDiff { query, diff }` answers with the same query, `diff` being
`None` where the working tree's `git diff` of the path prints nothing; since phase 07
the diff arrives prepared for the views — a boxed `cairn_model::ShownDiff`, both row
projections and the widest drawn line built on the diff thread — so the window keeps
the value and builds nothing (`DiffState::file_arrived`). A diff the window lets go of
travels back whole in a `Request::Retire`, indexes and all, and is freed on the
repository thread (`choosing_another_file_hands_the_last_diff_to_a_worker`).
`Request::Expand(ExpandQuery { of, changes, options, files, all })` asks for files opened
in place in the Commit tab (phase 08, R5.3): `changes` the window's own change set, shared,
whose files are named by index; `files` the ones opened by name and not answered yet, each
an `OpenedFile` with whether it is read past R2.6's limits (its Load Diff pressed); and
`all`, Expand All taken up where it stands — `AllFrom { next, spent }`, the first file it
has not decided and the lines it has spent — or `None`. It is answered a page at a time by
`Update::Expanded { of, options, files, all }`: each `ExpandedFile` with its index, whether
Expand All read it, and its outcome — the diff prepared for the views on the diff thread
(a boxed `ShownDiff`, as a single file's is) or its failure as display text, that file's
alone — and, for a page of Expand All, `AllProgress` (where it stands, and whether it has
ended, at the last file or with its budget spent). One that names nothing to read reads
nothing; what it is for is to supersede the one in flight (Collapse All). A failure is `Update::DiffFailed { query, message }`, naming what was
asked. A superseded query sends nothing: `ChangesCancelled`, `ContentCancelled`
and `GitReadCancelled` are not failures, and nothing is sent once the query's
epoch is no longer current.

**The Changes tab's filter** (phase 07, R5.4). `Request::FilterFiles { of, files, text }`
asks which of a change set's files hold `text` in a path; `files` is the window's own
change set, an `Arc` shared with `DiffState` rather than copied. It is numbered in a
fourth lane, `QueryLane::FileFilter`, which supersedes itself and nothing else, and is
routed to the repository thread, whose work between jobs is a bounded page, so a
keystroke never waits behind a diff. There `ChangeSet::files_matching` makes one pass
over the paths, asking the epoch every few thousand files, and answers
`Update::FilteredFiles { of, text, files }`, the matching indices in order; a filter
superseded by the next keystroke stops and sends nothing
(`a_filter_is_answered_on_a_worker_and_a_newer_one_supersedes_it`, over 55,184 paths).

**Lanes and the diff thread** (PRD R4.1-R4.3). The changes query is numbered in
the changes lane and a file diff or Expand All in the file-diff lane; a changes
query supersedes both, a file diff only the file-diff lane, and neither a page
nor a fetch (`docs/systems/history-graph.md`, "The worker boundary"). Both lanes
are served on `cairn-diff` (`worker/diff_lane.rs`), started beside the repository
thread with a thread-local handle of its own and the `GitBinary` the application
found, and reached directly from `RepositoryHandle::submit` by the routing table
(`worker/routing.rs`). Each lane holds only its newest waiting request; a waiting
file diff is served before a waiting changes query (it was asked after it, or the
changes query would have superseded it), a request superseded while it waited is
dropped unserved, and the thread blocks on its queue whenever nothing waits
(`the_newest_request_per_lane_is_served_the_file_diff_first`). One thread serves
both lanes, so a file diff asked while a changes query RUNS waits for it to end —
only a newer changes query cancels the one running — and a changes query waiting
while file diffs keep arriving waits behind each: a sustained click through files
postpones the change set of a commit selected before the clicks (which a newer
selection supersedes anyway). The query's epoch
is the `Cancel` every engine call is handed, so the runner's poll ends a
superseded read's process group — a click through a file list kills each file's
one to three `git` processes rather than queueing them
(`a_superseded_diff_kills_its_git`, a stub `git` whose `diff-tree` hangs with a
grandchild; `a_click_through_files_answers_the_last_file_only`). Files opened in
place are read a page at a time (`Served::expand`): first the files opened by name — those
read past the limits apart, since a page is read at one set of options — then Expand All
from where it stands, each page under the line budget `EXPAND_ALL_LINES`
(`worker/expand_all.rs`), each sent as soon as it is prepared, until the budget is spent or
every file is open; nothing of it is kept. The engine checks the epoch before each file it
reads, before each file it asks git about on its own, between files as it assembles a
page, and while each read runs, so a newer request in the lane — another expansion, a
file diff, a changes query — ends it at the next file or kills its read
(`expand_all_superseded_while_its_answers_are_assembled_ends_there`, in `cairn-git`;
`a_newer_request_in_the_lane_ends_an_expansion_and_kills_its_read`, a stub whose
`diff-tree -p` hangs; `expand_all_answers_every_file_in_order_through_the_boundary`;
`expand_all_stops_at_its_budget_through_the_boundary`). A
working-tree query whose reads disagree (`ContentReadsDisagree`: the file changed
between git's two reads of it) is asked again, up to `READ_ATTEMPTS` in all and
never once superseded, and then shown as a failure
(`a_disagreeing_read_is_asked_again_a_bounded_number_of_times`); a commit's
content cannot change between reads, so a disagreement there is shown at once.
A file opened in place that fails is that file's outcome, beside the others; a read git
fails for a single file, or a failure no file is to blame for in an expansion, is sent as
`DiffFailed` naming the query (`a_failed_read_is_sent_as_a_failure_naming_its_query`).

**What the thread keeps** (PRD R4.5, amended). The `DiffSession` — gix's blob
resource cache — is opened on first use and kept across commits, and answers already
given for commits and comparisons are kept, bounded by count and by size
(`crates/cairn-app/src/worker/diff_answers.rs`;
`what_is_kept_is_bounded_by_count_and_by_size`): a change set keyed by its
`Comparison`, a file's diff by its whole `FileQuery` — the comparison, both paths,
modes and blob ids, and the options
(`a_kept_answer_is_found_only_under_everything_it_was_asked_with`). The size counted
is the lines' bytes and each line's own size, the changed ranges, the overlay's
whitespace-ignoring ranges and highlights, and the function context at the 80 bytes
xdiff keeps of each — an upper bound on that last, since the overlay keeps its text
private (`the_budget_counts_ranges_and_the_overlay_too`). Working-tree answers are
never kept (`a_working_tree_answer_is_never_kept`), and neither is Expand All's; the
engine reads the index and the attributes afresh for every working-tree query.

A kept answer is kept only while everything git and gix read to give it is as it
was. Before each query the thread stamps those files
(`crates/cairn-app/src/worker/diff_freshness.rs`, over `cairn_git::DiffInputs`) —
each file's modification and change times to the nanosecond, its length, inode and
device, or that it is missing — in three tiers
(`each_tier_lets_go_of_what_its_move_makes_stale`):

- **The configuration**: every file git or gix reads it from — the system file, the
  XDG and global files as Cairn's environment names them and as `git`'s (which
  carries `HOME` and `XDG_CONFIG_HOME` but no `GIT_CONFIG_*` file variable) names
  them, `$GIT_DIR/config`, `config.worktree`, every `include.path` and
  `includeIf.*.path` target whether or not it exists or its condition holds, and
  `HEAD` where a condition names the branch
  (`every_configuration_file_git_reads_is_named_even_one_that_does_not_exist`).
  When one moves, the thread opens its repository handle again by the route the
  application opened it (`SharedRepository::reopen_for`, the same bare-repository
  check, the same registry and log;
  `opening_again_reads_the_configuration_afresh_and_refuses_another_repository`)
  and lets everything kept go; the history thread keeps its own handle. A reopen
  that fails — a configuration git would refuse, or the path now naming another
  repository (`Error::RepositoryReplaced`) — fails the query, and the next tries
  again (`a_configuration_edit_reaches_the_next_answer`: an include target created,
  `diff.algorithm` edited, `diff.renames` reaching the next change set).
- **What every path reads**: `info/attributes`, `core.attributesFile` (else the XDG
  `git/attributes`), the system `gitattributes` and the working tree's
  `.gitmodules` — and what the index holds of them, `cairn_git::StagedInputs`: every
  `.gitattributes` entry and `.gitmodules` (else `HEAD`'s), by path, stage and blob.
  The index file changes on every stat refresh, so its stamp and `HEAD` only say
  when to read `StagedInputs` again, and they are compared by value
  (`the_index_is_compared_by_what_it_holds_not_by_its_file`;
  `a_stat_only_index_refresh_keeps_what_is_kept`). When any moves, the session and
  every kept answer go.
- **What one path reads**: the working tree's `.gitattributes` in each directory
  above it. A file's diff records the directories above both its paths; a change
  set records them for every path git's rename search could pair — added, deleted,
  renamed and copied files, and modified ones when copies are searched — since an
  attribute decides whether a file is scored as text (an unstaged `-diff` turns a
  CRLF file's rename into a deletion and an addition, on git 2.30.9, 2.32.7 and
  2.56.0). A hit re-stamps them and lets the answer go when one moved; the session
  records the directories it has read and goes when one of them moved, since gix's
  attribute stack keeps the top of the tree for its life
  (`the_session_goes_when_a_directory_it_read_moved`;
  `an_unstaged_attribute_edit_reaches_a_kept_answer_and_a_new_one`).

**A stamp taken while its file could still change unseen matches nothing.** A
filesystem keeps times only so finely, so a file changed twice within that grain can
show one stamp for two contents. Each query stamps against the time it started; a
file whose modification or change time is within `SETTLING` (two seconds, FAT's
grain) of that is unsettled, an answer read under an unsettled stamp is answered and
not kept, and whatever such a stamp covers is read afresh by the next query
(`a_stamp_taken_while_its_file_may_still_change_matches_nothing`). The same rule makes
a stamp taken after a read safe — a change after the query started is unsettled —
which is how a change set's directories, known only from its answer, and a reopened
handle's configuration, stamped against when the reopen began, are trusted.

What it costs, measured on the bench repository (`~/Development/bench/rust`, release
build): a query's configuration, global and index stamps and `HEAD`, about 6 µs; a
file diff's directories, 1-4 µs; when the index or `HEAD` moved, the index read
again — about 14-20 ms for its 62,892 entries, gix's own load — and its attribute
entries compared in under 0.1 ms. A change set's directories cost more: 0.6 ms to
stamp M1's 5,590 searched paths (231 directories) and 0.14 ms to check them on a
hit, 5.4 ms and 1.5 ms for S1's 55,184 (2,490 directories), against the 35 ms the
query itself takes; the first stamp on a cold page cache took 75 ms. The object
cache is the handle's own `Repository::OBJECT_CACHE_BYTES`, not gix's tree-diff
sizing helper: the trees are compared by `git diff-tree`, so gix walks no tree here
for that cache to pay for.

**The window keeps an answer only for its selection**
(`crates/cairn-app/src/diff_state.rs`). `DiffState` holds the comparison
selected and the file selected, each with its `Answer` — waiting, ready or failed — and
the files opened in place for that comparison (below), and `session::apply` stores an
answer only when `DiffState` says it names what is selected now. Selecting a comparison
lets go of the file and the files opened in place, whose lane it supersedes, handing
what they drew to a worker to free. That is a second filter
behind the epoch: it refuses an answer whose epoch is still current but whose
selection has gone, such as one that arrives after the selection was cleared
(`an_answer_naming_another_selection_is_never_drawn`, through the real worker).
Choosing a row of the history (`crates/cairn-app/src/selection.rs`, `choose`) sets
the selection and asks `DiffState::select_changes` for its comparison, submitting the
`Request::Changes` it returns; choosing the row already chosen asks nothing new unless
its answer failed. The row's comparison is `selection::comparison_of`, a match naming
every `RowId` variant, so a row that is not a commit does not compile until it says
what it compares. Two commits compared (phase 08, R7) are chosen there too
(`selection::extend`, below).

**The file-diff lane is shared** (phase 08). The Changes tab's file and the Commit tab's
files opened in place are both asked in the file-diff lane — R4.3 puts Expand All there,
and a newer request in it supersedes it — so each asking takes the lane from the other.
`DiffState` records which holds it (`file_in_lane`, and the expansion's own), and the one
that lost it while its answer was awaited is asked again, whole, as its tab is shown: the
Changes tab's effect asks `reask_file` when `file_needs_asking`, the Commit tab's body
asks `reask_expansion` when `expansion_needs_asking`, so neither waits for good on an
answer that will not come and neither is ever half-answered
(`the_file_and_the_files_opened_in_place_take_the_lane_from_each_other`). A change of the
shared settings asks again at once only for the tab shown (`DiffState::settings_changed`
with an `Asking`), leaving the other to its tab — asking both would only have the second
end the first (`a_setting_asks_the_shown_tabs_selection_now_and_the_others_later`).

## The detail pane

As-built for PRD R5.1-R5.5 and R7 (files opened in place, Expand All and comparing two
commits from phase 08; the Changes tab's contents from phase 07). The layout is Fork's
(decision L9).

**Where it sits** (`crates/cairn-app/src/window.rs`, `split`). Below the commit list,
in a Freya `ResizableContainer`: the list a proportional panel that keeps at least
80 px, the pane a pixel panel opening at `PANE_HEIGHT` and dragged no smaller than
90 px; dragging it 24 px past that collapses it, Fork's gesture, as does the strip's
Collapse control. A collapsed pane keeps its strip of tabs under a list that takes
the rest, and opens again from the strip's Expand control, a tab pressed, or a tab's
chord; it opens at the height it was last dragged to
(`the_splitter_drags_and_the_pane_keeps_its_height`). No plain `ScrollView` is
involved: the exceptions roster stays empty.

**The Changes tab** (`crates/cairn-app/src/changes_tab.rs`, phase 07, R5.4), Fork's
(Findings 5 and 19): a one-line summary — the author's name, the short id, the author
date in the user's chosen format and the subject (`cairn_ui::ChangesSummary`,
`summary_parts`; no avatar), on a strip as tall as the diff bar's 30 px
(`the_summary_strip_is_as_tall_as_the_diff_bar`) — then, behind a draggable splitter (the list opening at 35%
of the pane, never dragged or squeezed below 200 px, its share kept for the session:
`the_file_list_opens_at_35_percent_of_the_pane_and_keeps_its_dragged_width`), the changed
files on the left under a filter field and one file's diff on the right. A component of its own, mounted only while the tab is
shown. The list (`cairn_ui::ChangesList`) is one `VirtualScrollView` of fixed rows over a
`Readable` of the change set and one of the filter's answer (`cairn_ui::ShownFiles`, every
file or the matching indices), so 55,184 files build one viewport of rows, filtered or not
(`a_list_of_55184_files_builds_one_viewport_filtered_or_not`). Typing in the field writes
the session's `View::filter_text`; an effect hands the text to `DiffState::filter`, which
asks a worker (above) and keeps the answer only for the change set and text asked last —
the last answer standing until the next arrives; for another change set, or the first
text typed, the list waits (`ShownFiles::Waiting`, "Filtering…") rather than show any
file as matched, so nothing of another change set's is ever read (`file_filter.rs`,
`the_filter_shows_the_answer_for_the_change_set_and_text_asked_last`; through the window,
`typing_in_the_filter_asks_a_worker_and_the_list_draws_its_answer`). While a filter is
active the list says "Showing N of M files" — the answer's length and the change set's,
read as they are, no walk of the list — so a file hidden by a filter kept from the last
commit is never taken for one this commit did not touch
(`an_active_filter_says_how_many_files_it_shows_of_how_many`, at 55,184 paths). The text
is kept for the session, and a change set arriving asks again with it; a file chosen
before the filter hid it stays shown on the right. Case is ignored as Unicode reads it, so
`É` finds `é` (`case_is_ignored_as_unicode_reads_it`); a known limit, accepted by the user
(2026-10-03): `str::to_lowercase` is a mapping rather than a case fold, so `İ` lowercases
to `i` and a combining dot and "istanbul" does not find `İstanbul.txt`
(`the_dotted_capital_i_is_a_known_limit`). A copy is found by either path, as a rename is
(`a_copy_is_found_by_the_path_it_was_copied_from`). With no file chosen, the first
file the list shows is chosen, as Fork selects the first file by default
(`the_changes_tab_shows_the_summary_the_files_and_the_first_files_diff`). A file pressed,
or reached with ↑ or ↓ while the list has focus — through the files it SHOWS, stopping at
either end (`the_arrows_move_through_the_files_the_list_shows`) — is chosen through
`diff_actions::choose_file`, which asks its diff in the file-diff lane: the last file's
query is superseded, its `git` killed by the epoch and its answer never drawn
(`a_file_chosen_in_the_list_supersedes_the_last_ones_diff`). The list highlights the file
chosen by the index `DiffState` records as it is chosen (`DiffState::file_index`), never
by searching the change set.

**The tabs** (`cairn_ui::DetailTabs`, `crates/cairn-ui/src/detail_tabs.rs`). Commit,
the default, and Changes, as text tabs with no count, the shown one underlined. The
tab chosen is the window's `View::detail_tab`, created once per window, so it is kept
across every selection and every collapse for the session
(`the_tab_chosen_is_kept_across_selections_and_a_collapse`). While two commits are
selected the Changes tab is shown and the Commit tab is drawn disabled and pressed for
nothing (`DetailTabs::unavailable`; R7.3, Fork's Windows build, Finding 7); the tab chosen
for the session is kept for when one commit is selected again (`detail_pane::shown_tab`).
A file pressed in the Commit tab opens in place (below) and the Commit tab stays shown —
Fork's Commit tab "does not switch to Changes" (Finding 4). The Changes tab keeps a file of
its own, chosen from its own list (the first by default): the single-file view is reached
by its tab, as in Fork; Fork's Mac 1.0.71 added buttons on the Commit tab that reveal a
file in the Changes tab, whose look is not recorded (Finding 4), so none is drawn.

**What the Commit tab draws, and when** (`crates/cairn-app/src/detail_pane.rs`). The
pane is a component of its own, so an answer arriving redraws it and not the window.
With nothing selected it says so; otherwise it draws the change set only when
`DiffState` holds the answer for the comparison of the row selected NOW — whatever it
holds for any other is not drawn, even if it is ready
(`the_commit_tab_draws_the_answer_for_the_row_selected_and_no_other`). While the
answer is on its way it says it is reading; a failure is drawn in the error colour;
a change set with no details (a comparison of two commits) is not described by this tab,
which is unavailable while two are selected anyway.

**The tab itself** (`cairn_ui::CommitTab`, `crates/cairn-ui/src/commit_tab.rs`), in
Fork's order: AUTHOR and COMMITTER in two columns, each `Name <email>` and the full
timestamp at its own offset as Fork's Windows build shows it — `14 Nov 2023 21:43:20
-00:30`: a two-digit day, the English month's abbreviation, a 24-hour time to the
second and the offset as `±HH:MM` (`date_text::long_date`, pinned against GNU `date`
by `a_timestamp_reads_as_fork_shows_it_at_its_own_offset`, Fork's own
`25 Nov 2020 01:11:30 +01:00` among its rows). The user's decision of 2026-10-03:
Fork's presentation rather than git's default format, fixed and in English, since a
localized one needs locale data no dependency of Cairn's carries; the day's padding is
inferred (Fork's one example has a two-digit day; its Windows build is .NET, whose `dd`
pads). A commit recorded before 1970 shows its true instant, also the user's decision,
since git disagrees with itself there
(`a_timestamp_before_the_epoch_reads_as_its_true_instant`);
the full 40-digit id beside SHA; each parent's 7-digit short id beside PARENTS, as a
link (no PARENTS row for a root commit); a rule; the message as `git log` shows it
(`message_lines::shown_lines`) — the blank lines before and after it gone, each line
without its trailing spaces, tabs or `\r`, blank lines inside kept, a tab expanded to
the next multiple of eight columns, the first line the subject in a larger bold face,
no line re-wrapped (`a_message_reads_line_for_line_as_git_log_shows_it`, against
`git log --format=fuller`; `the_message_is_the_rows_git_log_shows_for_it`, row for
row through the tab); a rule; git's own "exhaustive rename detection was skipped"
warning with the `diff.renameLimit` git asks for, when the search was cut short, above
the files, and nothing when a limit was set and not reached; and one row per changed
file, its status as a bare letter in a colour — `A`, `D`, `M`, `T`, `R`, `C`, as Fork
draws it, by the user's decision: git's `--name-status` letter without the similarity
score it prints after `R` and `C` (`every_status_is_its_bare_letter`) — and its path,
both paths, `old → new`, for a rename or a copy
(`the_commit_tab_shows_every_field_r5_3_names`;
`a_cut_short_rename_search_is_said_above_the_files`). No avatar, no ref chips, no
network call. The author and the committer are both drawn always, as git's `fuller`
format draws them — the user's decision, where Fork appears to omit an identical
committer. A person's `Name <email>` and a date are cut with an ellipsis where their
half of the pane is narrower than they are; nothing else in the tab is cut.

**The file list has focus of its own** (user decision 6, Fork's model). The tab takes
focus when a file row is pressed or when Tab reaches it, and draws a focus border when
reached from the keyboard; while it has focus, ↑ and ↓ move the current file — drawn
highlighted, brought into view, reported through `CommitTab::on_file` by its index in
the change set — and stop at either end
(`the_focused_file_list_moves_the_current_file_with_the_arrows`). Tab and Shift-Tab
move focus between the views that take it, Freya's own focus order: from the history,
past the strip's Collapse control, into the files, and back
(`tab_moves_the_arrows_between_the_history_and_the_commits_files`). A chord pressed
there is not an arrow (`accelerators::is_chord`). Another commit is another list, with
no file current.

**It is one virtualised list** (R5.5). Header, message lines and files are rows of one
`VirtualScrollView` at a fixed `DETAIL_ROW_HEIGHT`, so a commit touching 55,184 paths
builds one viewport of rows at the top, scrolled deep and at its very end, where the
last file is built and visible
(`only_a_viewport_of_files_is_built_however_many_the_commit_touched`). The tab is
handed a `Readable` over the window's `DiffState` (`diff_state::answered_changes`),
not a copy of the change set: each file row reads its file by index, and the header —
proportional to the message, never to the files — is built once per commit and cached
on the commit's id, the cut-short limit and whether there are files, so a write to the
diff state that leaves the commit as it was (a file's diff arriving, from phase 06)
redraws the tab without building or comparing the header again
(`the_header_is_built_once_per_commit_however_often_the_state_is_written`). A line
wider than the pane scrolls sideways rather than being cut, so a long path or message
line is never truncated; the sideways extent is the widest row built. The list is
keyed by the commit, so another commit opens at its top, and every row it built is
replaced (`another_commits_answer_replaces_every_row_of_the_last`).

**Files opened in place** (phase 08, R5.3; Fork, Finding 4). A file pressed in the
Commit tab opens its diff under its own row, and closes it when pressed again; files start
collapsed (`a_pressed_file_is_reported_and_its_diff_opens_under_its_row`;
`a_file_pressed_in_the_commit_tab_opens_its_diff_in_place`, through the window). A file's
row carries a disclosure chevron, right when closed and down when open. What an opened
file draws is more rows of the same list at the same `DETAIL_ROW_HEIGHT`: "Reading the
diff…" while it is on its way; its failure, in the error colour, that file's alone; the
notice that stands in place of rows, one line a row (`cairn_ui::notice_rows`, the same
words in the same order as the Changes tab's notice,
`every_notice_says_the_same_words_in_place_as_in_the_changes_tab`), with Load Diff under a
file past the limits, which asks that file again past them (`DiffState::load_in_place`);
or its diff, unified or side by side as the shared setting says, drawn by the same row
builders the diff view uses (`diff_view::draw_row`), at the shared context and whitespace
setting — with a row of its own above it, the bar's "Whitespace changes are hidden", when
ignoring whitespace hides a change, since an opened file has no bar to say it in
(`an_opened_file_says_it_is_being_read_or_failed_or_why_it_has_no_rows`). There is no bar
on an opened file, as Fork has none on its Commit-tab diffs (Finding 4); the settings are
the Changes tab's, shared, and a change of them asks the opened files again at them.

**The list stays one viewport however many files are open.** The opened files are a
`cairn_ui::Expansion` held in `DiffState` and handed to the tab as a `Readable`: each
opened file's index, what it draws, and its row counts in both layouts, worked out once as
it is set, with the rows the opened files before it add. A row of the list is placed by a
binary search of that table (`Expansion::item`, `position`), so building one costs
`O(log opened)` whatever the depth, and the table is built again only from the first file
a change touched (`a_table_built_a_piece_at_a_time_is_the_table_built_whole`): a page of
Expand All appended after the files open costs the page. With three files of 10,000 lines
open among 55,184, unified and side by side, the tab builds one viewport of rows at the top,
deep inside the middle file's diff and at the very end, where the last file's row sits at
the bottom of the view, so the list is exactly as long as its header, its files and every
opened file's rows (`only_a_viewport_of_rows_is_built_however_many_files_are_open`). ↑ and
↓ still move the current file, past the rows opened above it.

**Expand All** (R5.3). Above the files, right-aligned as Fork places it, Expand All —
Collapse All while any file is open (Fork turns the one into the other, Finding 4). Expand
All asks from the first file with nothing spent (`DiffState::expand_all`); each page that
arrives opens its files under their rows and moves Expand All on to where it stands, so
one superseded midway — by a file pressed, the Changes tab's file, Collapse All — is taken
up again from there; files already open are read again with the rest. It stops when every
file is open, or when its line budget, `EXPAND_ALL_LINES` (fifty thousand), is spent: then
the bar says "Expand All stopped at its line budget: N files left collapsed.", N the files
not open (`expand_all_turns_into_collapse_all_and_says_what_its_budget_left_collapsed`;
`expand_all_opens_each_pages_files_and_moves_on_until_its_budget`;
`expand_all_stops_at_its_budget_and_says_how_many_files_stay_collapsed`, through the
window). Why fifty thousand: `crates/cairn-app/src/worker/expand_all.rs` and
`docs/work/diff-engine/progress.md`. Collapse All closes every file, stops Expand All, hands
what was drawn to a worker to free, and supersedes what is in flight with an expansion
that names nothing (`collapse_all_ends_what_is_in_flight_and_frees_what_was_drawn`). A page
is kept only for the files asked as they were asked: a file closed while its page was on its
way does not open again, and a read within the limits does not answer a Load Diff
(`a_page_is_kept_only_for_the_files_asked_as_they_were_asked`); a failure no file is to
blame for fails every file still awaited, each on its own row
(`a_failed_expansion_fails_the_files_still_awaited`).

**Comparing two commits** (phase 08, R7; Fork, Finding 7). A row pressed with the
accelerator table's `ExtendSelection` chord — Ctrl-press, ⌘-press on macOS — is compared
with the row selected. A pointer press carries no modifiers in this Freya build, so the
window keeps what the keyboard says is held (`accelerators::HeldKeys`, from its global key
down and up handlers; a modifier key's own press counted before the state it sets,
`a_press_resolves_against_the_keys_the_window_heard`), and the history list resolves each
press against it, reporting a press with the chord apart (`HistoryList::on_extend`) and
drawing the second commit selected beside the first (`HistoryList::also_selected`). The
pair (`selection::Pair`) is tip against tip, never against a merge base, **the lower of the
two rows the base** whichever was pressed first (R7.2), found with one scan of the loaded
rows per press; it is asked as `Comparison::Between { old: base, new: tip }`, which the
diff thread hands git as `git diff <base> <tip>`
(`a_comparison_is_asked_with_its_base_as_the_old_side`), held to git's file list and rows
both ways round by `a_comparison_of_two_commits_reads_as_git_diff_of_the_pair_both_ways`.
The Changes tab draws it under a header naming both commits, one per line — "Base" then
"Tip", each its short id, author and subject (`cairn_ui::ComparisonHeader`) — with a swap
control at its right that asks the comparison the other way round (`selection::swap`), so
the answer changes, not just the header
(`a_modifier_click_compares_two_commits_tip_against_tip_with_the_lower_row_the_base`). Exactly
two, never half-selected (`a_comparison_is_never_left_half_selected`): a third press with
the chord replaces the second, the row pressed plainly staying; a press with the chord on
one of the pair leaves the other selected alone; on the one row selected it changes
nothing; a plain press returns to one, and the comparison's answer arriving after is never
kept.

**A replaced answer is freed off the UI thread** (R2). Dropping a change set of 55,184
files measured 1.2-2.0 ms in a release build (2026-10-03), more than a frame spares.
`DiffState::select_changes` therefore returns the query and then, when answers were
kept, a `Request::Retire` carrying them — the change set, a file's diff, Expand All's
diffs — which `selection::choose` submits after the query and the repository thread
frees, answering nothing
(`choosing_another_commit_hands_the_last_ones_answers_to_a_worker`,
`a_retired_change_set_is_freed_on_the_worker_without_an_answer`). An answer the window
never keeps goes the same way (phase 07 QA, R1): `Updates::next` hands a superseded
change set, file diff or Expand All batch back as `Update::Superseded` rather than
dropping it on the task the UI thread drives (`Update::into_retired`; a superseded
page, filter answer or failure is small and still dropped there), and `session::apply`
sends it, and any such answer naming another selection, straight on as a
`Request::Retire`, which only sends
(`a_superseded_answer_comes_back_to_be_freed_on_a_worker`,
`an_answer_the_window_will_not_keep_is_handed_to_a_worker_to_free`).

**Parent links** (`detail_pane::follow_parent`). A parent loaded in the history is
selected — its changes asked for through `selection::choose` — and its row brought
into view through the list's shared `ScrollController` (`cairn_ui::reveal_row`); a
parent not loaded does nothing visible, since reaching it is issue #3
(`a_parent_link_selects_a_loaded_parent_and_ignores_an_unloaded_one`). Finding the
parent scans the loaded rows once per press (`selection::loaded_row`), never per
frame.

## The diff view

As-built for PRD R6.1-R6.9, unified and side by side (a file opened in place under its row
in the Commit tab is drawn by the same row builders, "Files opened in place" above), with
the user's decisions of phase 06: Fork's chords
only, git parity for every row, and the user's own `diff.context` as the starting context.

**Where it is drawn, and for what** (`crates/cairn-app/src/changes_tab.rs`, `diff_side`).
A file chosen in the Changes tab — pressed or reached with ↑ or ↓ in its list, or chosen
there by default (phase 08: a press in the Commit tab opens the file in place instead) — is
asked through
`DiffState::select_file` at the session's settings, and the diff side draws the bar and
then — only for the answer naming that file at those settings — the rows, or what stands
in their place: "Reading the diff…", a failure in the error colour, or the notice of a
state that is not text ("What stands in place of rows", below)
(`the_changes_tabs_file_draws_its_diff_for_that_query_alone`). A diff
replaced by another file's, or by the same file's at other settings, is handed to the
repository thread to free, as a change set is
(`choosing_another_file_hands_the_last_diff_to_a_worker`).

**Built once per answer, on the worker** (`cairn_model::ShownDiff`, built in
`worker/diff_lane.rs`). As the diff thread answers, it prepares the answer at the context
it was asked at: both row projections (`UnifiedLayout::shown`, `SideBySideLayout::shown`,
each proportional to the changes), whether ignoring whitespace hides a change
(`DisplayOverlay::hides_a_change`, asked once, as its own doc requires), the gutter's
digits and the widest line's columns as drawn (`widest_drawn_columns`: one pass over the
drawn bytes, an upper bound — a byte is at most a column and a tab at most eight — and a
line past the long-line limit counted to its cut). The window keeps the value; toggling
side-by-side builds nothing. Measured in a release build on F1's Load Diff (1.9 MB to
5.2 MB, 334,688 unified rows), preparation took 2.4 ms on the diff thread; on a 52 MiB
file of a million changed lines, 20 ms — work the UI thread no longer does (phase 06's
obligation; it was about 39 ms at 64 MiB on the UI thread). The view is handed a
`Readable` over the window's `DiffState` (`diff_state::answered_file`), never a copy.

**A row** (`cairn_ui::DiffView`, `crates/cairn-ui/src/diff_view.rs`, drawing
`unified_rows.rs` or `side_by_side_rows.rs` as the shared setting says, each from the
pieces in `diff_row_parts.rs`). Every row is `DIFF_ROW_HEIGHT` tall, through one
`VirtualScrollView` with a fixed item size, so the view builds one viewport of rows at the
top, scrolled deep and at the end of a 1,000-line and a 100,000-line file — the same
number at each place for both — and the end is the projection's last row, so the view's
length is the projection's (`only_a_viewport_of_diff_rows_is_built_however_long_the_file`,
criterion C9 for unified rows, the twin of
`only_a_viewport_of_rows_is_built_however_long_the_history`). A unified line row is the old and the new line number, right-aligned, a one-pixel separator,
then — tinted for a change, from the separator to the row's end — a marker column
(`-`, `+`, blank) and the line. A removed line leaves the new gutter blank, an added one
the old; so a row means the same with its colour ignored
(`a_row_reads_the_same_with_its_colour_ignored`, L11). A hunk header is git's
`@@ -a,b +c,d @@` and, after a space, the function context git printed for a hunk
starting there, in muted text at the same height, with no numbers, no band and no
button (Fork, Finding 13); git's `\ No newline at end of file` is a muted row of its
own. Intra-line ranges are drawn as the paragraph's highlights in the stronger tint of
the line's side, carried from the engine's byte ranges into UTF-16 units of the drawn
text — Skia's paragraph indexes UTF-16, measured: a range splitting a surrogate pair
draws nothing (`intra_line_ranges_are_drawn_in_the_stronger_tint`,
`a_range_after_a_wide_character_lands_in_utf16_units`). What a row draws of a line's
bytes (`diff_line_text::shown_line`): a tab to the next multiple of eight columns, as a
terminal shows `git diff` (Skia draws a tab as a box, measured), the columns before it
counted as a terminal counts them — two for an emoji or a CJK ideograph, none for a
combining mark, a control's picture one — by `unicode-width` (`cairn_ui`'s `columns`, the
user's decision "terminal widths", 2026-10-03; the Commit tab's message lines use the same
rule; `a_tab_after_a_wide_or_combining_character_stops_where_a_terminal_stops`); a CRLF file's final
`\r` not drawn; another C0 control or DEL as its Unicode control picture, which IBM Plex
Mono 2.5 carries; invalid UTF-8 as one `U+FFFD` per invalid sequence.

**Side by side** (`side_by_side_rows.rs`, R6.1, Fork's Finding 11). Two columns of equal
width inside the ONE virtualising view — never two scroll views, so the exceptions roster
of plain `ScrollView`s stays empty — the old side on the left and the new on the right,
each a line-number gutter, a separator and a text area: a context line in both columns,
the i-th removed line beside the i-th added one, the shorter side's rows filler (Fork's
grey, `FILLER`), the hunk header with git's function context at the top of each column,
git's end-of-file marker in the column of the side that did not end, a `-`/`+` marker per
changed line so a row reads the same with its colour ignored
(`side_by_side_pairs_lines_fills_the_shorter_side_and_reads_without_colour`). Each column
is half the view, so both sides are on screen together as Fork's equal panes are; a line
wider than its column scrolls sideways, sliding the text of both columns together while
each gutter stays where it is — every row is laid out at the scroll's offset, with a
spacer as wide as the scroll ahead of the columns, and the view's horizontal extent is the
widest line's overflow past its column
(`side_by_side_columns_are_equal_halves_and_slide_together`). C9 for side-by-side rows:
over a 1,000-line and a 100,000-line file drawn whole, one viewport of rows at the top,
deep and at the end, the view's length the file's real row count (a header and a row per
line, counted from the file), and the last line at the end in both columns
(`only_a_viewport_of_side_by_side_rows_is_built_however_long_the_file`).

**What stands in place of rows** (`cairn_ui::DiffNotice`, `DiffNoticeView`,
`crates/cairn-ui/src/diff_notice.rs`; R6.8, C11). `DiffNotice::of` reads a prepared diff by
naming every state; each draws words that cannot be taken for another state's, git's
own where git has them (`every_state_that_is_not_text_draws_its_notice`, each state built
and looked at from a real fixture repository as well). Their parity with git is pinned end
to end by an integration test, which may run `git` itself,
`every_notice_says_what_git_diff_says_of_the_same_file`
(`crates/cairn-app/tests/notice_parity.rs`): one commit, built by real `git` with
`diff.renames=copies`, holding a mode-only change, a rename and a copy with no content
change, a rename whose mode moved, a submodule bump, a binary, an LFS pointer and a file
past the drawing limit, each read by the engine, prepared with `ShownDiff::new` and turned
into its notice, and compared with `git show` of the same file — the extended header line
for line, the `Subproject commit` lines (never "binary"), each pointer side as git prints
it, the blob sizes. What each state draws:

| State | Drawn |
| --- | --- |
| Binary | "Binary file", Fork's "Old" and "New" over each side's size in KiB and bytes (`2.0 KiB (2,048 bytes)`; bytes alone under a KiB); an absent side (added, deleted) has none |
| Too large | Fork's "Changes are too large to display", the measurement the limit fired on, and Fork's "Load Diff" while a load is offered; past the 64 MiB ceiling the file's size and the ceiling in MiB — "72.3 MiB — larger than the 64 MiB Cairn can load" (`too_large_reason`, `mib_text`) — and no button |
| Git LFS pointer | "Git LFS pointer" over each side's pointer text, the old above the new |
| Submodule | "Submodule" over git's `-Subproject commit <id>` and `+Subproject commit <id>` lines, `-dirty` after the new where the working tree's checkout has changes; never "binary" |
| Mode only | git's `old mode` and `new mode` lines, under "Mode changed" |
| Rename or copy, no content change | "Renamed without changes" ("Copied without changes") over git's `similarity index`, `rename from`/`rename to` (`copy from`/`copy to`) lines; when the mode moved too, git's mode lines first, under "Renamed, mode changed" ("Copied, mode changed") |
| Conflicted, unsupported | "Unmerged path — conflicts must be resolved before a diff can be shown"; the engine's reason |
| Text with no row | "No changes to show.", or the whitespace sentence when ignoring whitespace hides every change |

**Load Diff** (`diff_actions::load_anyway`) asks the file shown again with `load_anyway`,
which the engine honours up to the 64 MiB ceiling; while it is shown a setting changed
keeps it loaded, and another file starts unloaded
(`load_diff_asks_the_file_again_past_the_limits`). Measured on F1 (`6a6e8446b97`,
`arm_intrinsics.json`, 1.9 MB to 5.2 MB) in a release build: refused in 0.04 ms; Load
Diff answered by the engine in 189 ms (median of five), prepared in 2.4 ms, and a frame
after a scroll through its 334,688 unified (310,340 side-by-side) rows 1.4 ms (2.0 ms side
by side) headless. At the ceiling, on files built for it: 60 MiB in one line, 153 ms to
answer and microseconds a frame; 52 MiB of a million changed lines, 989 ms to answer on
the diff thread, 20 ms to prepare, 1.5 ms (3.2 ms side by side) a frame.

**A line past the long-line limit is drawn cut** (R6.9, `cairn_model::drawn_bytes`,
`cairn_ui::cut_marker`). Only a diff loaded past the limits holds one; drawn whole, a
64 MiB line would be 64 MiB of text each time its row is built. A row draws at most the
long-line limit's 2,048 bytes of a line, ending on a character — a two-, three- or
four-byte character straddling the cut left out whole — then " … N more bytes" in the
muted colour, N the bytes not drawn, a straddling character's head among them
(" … 4,192,256 more bytes" for a 4 MiB line; `the_marker_counts_every_byte_not_drawn`);
its intra-line ranges past the cut are not read (`a_cut_line_reads_only_the_ranges_before_its_cut`);
and the widest line is measured to its cut, so the horizontal extent is bounded too
(`a_line_past_the_limit_is_drawn_cut_with_its_marker_in_both_views`, a 4 MiB line in each
view; `a_line_past_the_limit_is_cut_at_the_limit_on_a_character`, at every character
width and at 2,049 bytes).

**Per frame and per row.** A frame builds the rows in view and nothing else. Per row: a
search of the layout's index, one search of the intra-line pairs per side, the line's
drawn text (at most the long-line limit's, never the file's), the line numbers formatted
and one header string for a header row. Nothing per row reads the theme: the colours are
constants. None of it grows with the file, the line or the scroll offset.

**Horizontal extent.** Unified: every row is as wide as the view, or as
`cairn_ui::content_width` (the gutters, the marker column and the widest drawn line's
columns at the font's advance, the cut marker's too when a line is cut) where that is
wider, measured once — so the horizontal scrollbar is the same whichever
rows are built (`the_horizontal_extent_is_the_widest_lines_wherever_the_view_is`). Long
lines scroll sideways, never wrap (issue #34). The gutter scrolls sideways with the
text; Fork's own behaviour there was not established. A glyph the font lacks, drawn by a
wider fallback, can run a line past the extent.

**The bar** (`cairn_ui::DiffHeader`, `crates/cairn-ui/src/diff_header.rs`), Fork's layout
(Finding 10): ↑ and ↓ for previous and next change on the left; the path in IBM Plex
Mono, the directory muted and the file name in the text colour, a rename's or a copy's
old path in a tooltip; on the right, Ignore whitespace, Fewer lines, More lines, Entire
file and Side-by-side. Each button is Fork's glyph
(`cairn_ui`'s `toggle_glyphs`, built from plain shapes, no icon font): chevrons, `⎵`,
`−`, `+` and `↕` over lines, a split rectangle. A toggle that is on draws its glyph in the
accent colour, never filled, as Fork does; each button carries its name — read by assistive
technology and shown as its tooltip — so its meaning never rests on the glyph
(`every_button_is_a_named_glyph_and_an_active_toggle_is_lit_in_the_accent`).
Fewer lines is disabled at one line, and both line buttons while the entire file is
shown; a disabled button reports nothing
(`the_bar_reports_each_button_and_holds_fewer_lines_at_one`). With whitespace ignored
the bar says "Ignoring whitespace hides some changes" exactly when the answer hides one —
never just because the toggle is on (`the_bar_says_changes_are_hidden_only_when_they_are`,
`ignoring_whitespace_hides_whitespace_only_changes_and_says_so_only_then`, R6.7) — a
deliberate deviation from Fork, which hides them silently. No button has a chord (the
user's decision, `docs/research/diff-engine/fork-shortcuts.md`); each press is mapped to
its `Action` (`shortcuts::of_header`) and done by `shortcuts::act`, the one place a chord
is done too.

**The settings** (`cairn_ui::DiffSettings`, `View::diff_settings`). One value for every
diff view, kept for the session, not across sessions (issue #29): the lines of context,
the entire file or not, whitespace ignored or not, side by side or not (unified by
default, R6.1; toggling it asks git nothing and lets go of the change last moved to, since
its rows are the other view's — `side_by_side_is_one_setting_for_every_diff_and_asks_nothing`,
which also shows the setting kept across another file and another commit). Context moves a line per click, never
below one (`context_moves_a_line_at_a_time_and_never_below_one`); the entire file is a
toggle that gives back the lines it left. It starts at the user's `diff.context`, raised
to one — read on the diff thread, whose handle is the one opened again when the
configuration moves, when the window asks at open (`Request::ConfiguredContext`,
`Repository::configured_context`) and sent again each time that handle is opened afresh,
so an edit to `diff.context` reaches a running session; taken unless the user has
already moved the context
(`the_configured_context_is_where_the_session_starts_until_the_user_moves_it`,
`the_configured_context_is_answered_through_the_boundary`,
`a_configuration_edit_mid_session_reaches_the_next_answer` — the last also showing
`diff.interHunkContext` reaching the next answer whether the context was moved or not,
since every answer reads it afresh); a value git refuses sends
nothing, and every diff then fails saying so. A setting changed asks the file shown again
through the file-diff lane — the context is the `-U<n>` git is asked at, so a new context
is a new query, superseding the one in flight and keyed apart in the thread's cache — and
the answer kept is the one naming the new options
(`the_bar_moves_the_context_a_line_at_a_time_and_asks_again_never_below_one`). The view
keeps its scroll across a setting changed, and opens at the top for another file.

**Previous and next change** (`cairn_ui::step_change`, `diff_actions::step`). The bar's
chevrons, and Fork's chords — Ctrl+↑/↓, ⌘↑/↓ on macOS — heard in `Scope::Detail`, so only
while focus is inside the detail pane: the diff (focused by a press), the files, the bar.
One step goes from the change last moved to while the view is still where that step left
it, and otherwise from the top row in view, so the first press finds the first change at
or below the top even when it is already in view (Fork's open bug is skipping it,
TrackerWin #2393). The changes are the drawn view's (`ShownDiff::stops`, unified or side
by side), and rows and pixels are whole numbers throughout, so a change a million rows
down is found from the row really at the top and put exactly one row below it
(`a_change_a_million_rows_down_is_stepped_to_exactly`; the `f32` arithmetic it replaced
lost whole rows past 2^24 pixels). The change moved to is put a row below the top and marked by a wider
separator in the accent colour; past either end nothing moves; nothing acts while the
Changes tab is hidden or the pane collapsed
(`previous_and_next_change_step_one_change_from_where_the_view_is`,
`previous_and_next_change_move_the_diff_while_the_pane_has_focus`). With the diff
focused, the plain arrows scroll a row, Page Up and Page Down a view, Home and End to
either end, and ← and → a few columns.

**Measured from Fork, or chosen by Cairn.** The user's rule is that Fork is the standard
for what a user sees, so each such value is one or the other, with its source or its
reason. Sources are `docs/research/diff-engine/fork-detail-and-diff-ui.md` (Findings by
number) and `docs/research/diff-engine/fork-shortcuts.md`.

| What | Value | Fork-measured, or Cairn-chosen |
| --- | --- | --- |
| Diff text size (`DIFF_FONT_SIZE`) | 11 px | Fork-measured: Mac's default diff font is Menlo 11 pt (Finding 17, two screenshots four years apart) with a 6.6 pt advance (Finding 24); a macOS point is a logical pixel, and Plex Mono's 0.6 em advance at 11 px is the same 6.6. Fork's Windows size and face are not established. |
| Row pitch (`DIFF_ROW_HEIGHT`) | 17 px, a hunk header the same | Fork-measured: 17 pt on the Mac (Finding 24, a vendor screenshot at 2×, May 2026). Windows' 30–31 px is at an unknown display scale, so not used. |
| Bar height (`DIFF_HEADER_HEIGHT`) | 30 px | Cairn-chosen: Fork's bar is not measured in the research; 30 lines up with the detail pane's strip above it. |
| Changes tab summary strip (`SUMMARY_HEIGHT`) | 30 px, the bar's height | User's decision, 2026-10-03 (it was 28 px), so the tab's two strips line up (`the_summary_strip_is_as_tall_as_the_diff_bar`). |
| The bar's buttons | chevrons; `⎵`; `−`, `+`, `↕` over lines; a split rectangle | Fork-measured: Finding 10 (vendor screenshots, Mac and Windows, 2025–2026), word wrap and invisibles left out (not in this packet). Drawn from plain shapes, not Fork's artwork. |
| An active toggle | its glyph in the accent, not filled | Fork-measured: the vendor, Tracker #2623 (Finding 10). |
| Tooltips and names | "Ignore whitespaces", "Decrease number of visible lines", "Increase number of visible lines", "Show entire file" | Fork-measured: Finding 10. |
| Tooltips and names | "Previous change", "Next change", "Side-by-side diff" | Cairn-chosen: Fork's are not recorded (the eye button's "Show diff side-by-side (spacebar)" belongs to the quick look, another control); the words Fork's release notes use. |
| Previous and next change chords | Ctrl+↑/↓, ⌘↑/↓, heard in the detail pane | Fork-measured: `fork-shortcuts.md`. |
| Keys in a focused diff | ↑/↓ a row, Page Up/Down a view, Home/End either end, ←/→ four columns | Cairn-chosen: Fork's diff is a text control (`NSTextView`, AvalonEdit) whose keys move a caret, and Fork documents none of its own beyond the chords above; Cairn's diff has no caret, so the keys scroll as a list's do. |
| Tab stops | eight columns, counted in terminal widths | Cairn-chosen (the user's decision "terminal widths"): what `git diff` shows in a terminal. Fork's tab width is a setting whose default is not established (Finding 17). |
| Colours | Fork's dark values, retuned | Fork-measured values (Finding 25), moved by a Cairn-chosen rule: each keeps its offset from Fork's ground, channel by channel, on Cairn's darker ground, so a tint stands out as much as in Fork (`retuned`, below). |
| `CURRENT_CHANGE` | the dark theme's `text_highlight` | Cairn-chosen: Fork uses the system accent (Finding 25), which Cairn has no platform call to read; the theme's accent, pinned equal by `the_current_change_is_the_themes_accent`. |
| Side-by-side columns | two equal columns, each half the view, one gutter each, grey filler, the hunk header at the top of each | Fork-measured: Finding 11 (equal, not resizable, the vendor; one gutter per pane; grey filler; the header repeated). Filler's grey is Fork's `#424242` (Finding 25), retuned. |
| Side-by-side sideways scroll | both columns' text slides together; each gutter stays | User decision (2026-10-03), kept as built: Fork's panes are two text controls whose sideways scroll is not established; Cairn's are one view (the no-plain-`ScrollView` invariant), so one scroll moves both. |
| Side-by-side `-`/`+` markers | a marker column per side | User decision (2026-10-03), kept as built: Fork's is an opt-in preference (Finding 12); Cairn draws it always, as in unified, so meaning never rests on colour (L11). |
| End-of-file marker side by side | a row after the change, in the column of the side that did not end | User decision (2026-10-03), kept as built: git has no side-by-side form; Fork's is not recorded. |
| Too large | "Changes are too large to display", "Load Diff" | Fork-measured: Finding 21 (Windows screenshot, TrackerWin #2245). The line under it giving the measurement is Cairn's. |
| Too large, past the 64 MiB ceiling | "72.3 MiB — larger than the 64 MiB Cairn can load": the file's size to a tenth of a MiB and the ceiling, no Load Diff, no logo | User's decision, 2026-10-03 (the sentence replaced "N bytes, more than the limit of 1,048,576 bytes; too large to load", which named the drawing limit rather than the one that refused the load). The ceiling said is `DiffLimits::LOAD_ANYWAY_BYTES`, the one every file diff is asked with. No logo kept as built: user's decision, 2026-10-03. |
| Binary | "Old"/"New" over each size in KiB and bytes | Fork-measured: Finding 22 (Windows 1.28 screenshot: the size in KB and bytes). The research does not record the sample's numbers, so it does not settle whether Fork's KB is 1,000 or 1,024 bytes: user decision (2026-10-03), labelled honestly as KiB, 1,024 bytes; bytes alone under a KiB. "Binary file" kept, user decision. |
| LFS pointer, submodule | the pointer text under "Git LFS pointer"; git's own lines under "Submodule" | User decision (2026-10-03): "Git LFS pointer" and "Submodule" kept. Fork's LFS and submodule views show downloaded content, chips and a commit graph (Finding 22), more than R6.8 asks; git's own lines are the parity answer. |
| Mode only | git's mode lines under "Mode changed" | User's decision, 2026-10-03 (it replaced "No change to the file's content"). |
| Rename with no content change | "Renamed without changes" over git's rename lines | User decision (2026-10-03). |
| Copy with no content change | "Copied without changes" over git's copy lines | User's decision, 2026-10-03: kept as built, the rename's form. |
| Rename or copy whose mode moved too | "Renamed, mode changed" ("Copied, mode changed") over git's mode lines, then its rename (copy) lines | User's decision, 2026-10-03; the copy's by analogy with the rename's. |
| Conflicted | "Unmerged path — conflicts must be resolved before a diff can be shown" | User decision (2026-10-03): git's own "Unmerged path", and why no diff is drawn. |
| Cut line marker | " … N more bytes", muted, N the bytes not drawn | User decision (2026-10-03). Fork refuses such lines rather than cutting them (Finding 21). |
| Changes tab summary | author, short id, author date, subject | Fork-measured: Finding 2 (avatar, author, abbreviated SHA, date, subject); no avatar (L9); the date in the user's chosen format. |
| Changes tab list | filter field at the top, status letter and path, first file chosen | Fork-measured: Finding 5 (filter, badge, name; first file selected by default). |
| Filter matching | the text anywhere in a path, a rename or a copy by either name, no wildcards, case ignored as Unicode reads it | Fork-measured in part: file name, extension or path expression, no wildcards (the vendor, TrackerWin #152 and Tracker #1482). Unicode case folding (`str::to_lowercase`, no dependency): user decision (2026-10-03). |
| Filter's known limit | `İ` (U+0130) lowercases to `i` and a combining dot, so "istanbul" does not find `İstanbul.txt` | User's decision, 2026-10-03: accepted, `to_lowercase` kept rather than a case-folding table (`the_dotted_capital_i_is_a_known_limit`). |
| Filter's words | "Filter" placeholder; "Filtering…" while the first answer is on its way; "No file matches the filter." | User's decision, 2026-10-03: kept as built (Cairn's words; Fork's are not recorded). |
| No file chosen | "Choose a file to see its diff." | User's decision, 2026-10-03: kept as built. |
| Diff side minimum width (`DIFF_MIN_PIXELS`) | 240 px | User's decision, 2026-10-03: kept as built. |
| Filter persistence | the text kept across commits for the session; a file chosen before the filter hid it stays shown; "Showing N of M files" whenever a filter is active | User decision (2026-10-03): the count line keeps a sticky filter from being mistaken for a commit that touched fewer files. |
| File list width | 35% of the pane until dragged, never below 200 px, its share kept for the session | User decision (2026-10-03). Fork's split is draggable (Finding 5); its default width is not established. |
| A file pressed in the Commit tab | opens its diff under its row, pressed again closes it; files start collapsed; the Commit tab stays shown | Fork-measured: Finding 4 (vendor GIF; "it does not switch to Changes"; collapsed by default, by the vendor's choice). Phase 06's press, which showed the file in the Changes tab, is replaced. |
| In-place diff's options | no bar of its own; the Changes tab's settings — context, whitespace, side-by-side — shared | Fork-measured: Finding 4 (the vendor: no header to host options; users: the Changes tab's options govern). |
| Expand All | right-aligned above the files; Collapse All while a file is open | Fork-measured: Finding 4 (Expand All turns into Collapse All). That it reads Collapse All while ANY file is open — one opened by a press as well — is Cairn-chosen: Fork's label after a single press is not recorded. |
| Expand All's budget | 50,000 lines, both versions of each file counted, and one per file | Cairn-chosen (Q2; the PRD names a line budget, not its size): R2.6's per-file line ceiling, measured against the window check — `progress.md`, phase 08. |
| What the budget says | "Expand All stopped at its line budget: N files left collapsed.", left of Collapse All | Cairn-chosen: Fork has no budget (it expands every file). |
| Expand All over files already open | reads them again with the rest, from the first file | Cairn-chosen. |
| A file row's disclosure | a chevron, right when closed, down when open | Fork-measured in part: Windows rows carry a disclosure triangle (Finding 4); a chevron of plain shapes rather than a filled triangle, and drawn on every platform (Mac rows carry none), Cairn-chosen. |
| Row pitch of an in-place diff | the Commit tab's 24 px (`DETAIL_ROW_HEIGHT`), not the Changes tab's 17 | Cairn-chosen, forced: the tab is one virtualised list of rows of one size, the invariant that keeps 55,184 files and every opened line one viewport of work; Fork's in-place pitch is not measured, and is presumably its diff's 17 pt. The alternative is the whole Commit tab at 17 px. |
| Sideways scroll of in-place diffs | the whole Commit tab scrolls sideways as one, to the widest row built | Cairn-chosen, forced as above: Fork scrolls each inline diff on its own (Finding 4); one list is one scroll. |
| An in-place diff being read, failed, or hiding whitespace | "Reading the diff…"; the failure in the error colour, that file's alone; "Whitespace changes are hidden" as a row above it | Cairn-chosen: the Changes tab's words, as rows (Fork's in-place states are not recorded). |
| Load Diff in place | under a file past the limits, as in the Changes tab | Cairn-chosen: R6.8's control, where the notice stands. |
| Reaching the Changes tab's single-file view | its tab; it keeps a file of its own (the first by default), independent of the Commit tab's current file | Fork-measured in part: the Changes tab has its own list and first file (Finding 5). Fork's Mac 1.0.71 buttons that reveal a file in the Changes tab are not drawn: their look is not recorded (Finding 4). |
| Second commit | ⌘-press on macOS, Ctrl-press elsewhere; exactly two; tip against tip | Fork-measured: Finding 7 and `fork-shortcuts.md`. |
| Which is the base | the lower row of the two | Cairn-chosen in the PRD (R7.2): Fork's Windows build orders older to newer topologically; its Mac build's order is not established. |
| A third press, or a press on one of the pair | a third replaces the second, the row pressed plainly staying; a pressed member of the pair leaves the other alone; a press with the chord on the one row selected changes nothing; with nothing selected it selects | Cairn-chosen: Fork allows only two (Finding 7) and shows a message on a third (Tracker #1321); its exact gesture semantics are not recorded. |
| The comparison's header | two lines, "Base" then "Tip", each its short id, author and subject; 60 px | Fork-measured in part: both commits named, one per line, no branch or tag labels (Finding 7); the captions and fields Cairn-chosen (meaning never on colour alone, L11, where Fork's Windows build colours the two ids). |
| The swap | a `↕` button at the header's right, named "Swap base and tip" | Fork-measured in part: a swap-direction icon at its right (Finding 7); the glyph and name Cairn-chosen. |
| The Commit tab while comparing | drawn disabled, pressed for nothing; the tab chosen for the session restored when one is selected again | Fork-measured in part: Windows disables Commit and File Tree (Finding 7); restoring the chosen tab Cairn-chosen. |

**Colours and typeface** (`cairn_ui::diff_palette`). Named tokens, never literals at a
call site. Fork's measured dark values (`docs/research/diff-engine/fork-detail-and-diff-ui.md`,
Finding 25, Windows 2023-2026) sit on Fork's `#282828`; Cairn's ground is Freya's dark
background, `rgb(20, 20, 20)`, so each is retuned by keeping its offset from the ground
channel by channel (`retuned`, computed from the Fork value written beside it):

| Token | Fork's value | Cairn's |
| --- | --- | --- |
| `REMOVED_TINT` | `#633F3E` | `rgb(79, 43, 42)` |
| `ADDED_TINT` | `#3A5C3F` | `rgb(38, 72, 43)` |
| `REMOVED_EMPHASIS` | `#9F4247` | `rgb(139, 46, 51)` |
| `ADDED_EMPHASIS` | `#388442` | `rgb(36, 112, 46)` |
| `GUTTER_SEPARATOR` | `#4B4B4B` | `rgb(55, 55, 55)` |
| `HEADER_BAR` | `#333333` | `rgb(31, 31, 31)` |
| `DIFF_TEXT` | `#DDDDDD` | kept |
| `DIFF_MUTED` (numbers, headers, the end-of-file marker) | `#A0A0A0` | kept |
| `CURRENT_CHANGE` | the system accent | the theme's `text_highlight`, `rgb(96, 145, 224)` |

`each_tint_is_forks_measured_value_moved_to_cairns_ground` pins the rule;
`the_text_reads_on_every_tint_and_an_emphasis_is_stronger_than_its_line` requires the
text at WCAG AA's 4.5:1 on the ground, every tint and every emphasis, and each emphasis
further from the ground than its line. Diff text, the bar's path, and the Commit tab's
id, parents and paths are drawn in IBM Plex Mono (R6.6, L16): Regular from IBM's release
`@ibm/plex-mono@2.5.0` (`crates/cairn-app/assets/fonts/IBMPlexMono-Regular.ttf`, with
`IBMPlexMono-LICENSE.txt`, the SIL Open Font License 1.1, beside it), embedded with
`LaunchConfig::with_font` under `DIFF_FONT_FAMILY` at 11 px; its advance is 0.6 em
(`MONO_ADVANCE_EM`, read from its `hmtx` table). The download was the user's approval;
no other weight is embedded.

**Git parity, pinned against `git diff`** (`crates/cairn-git/tests/diff/parity.rs`). The
parity tests read the view's own projection (`UnifiedRows::shown`), not a copy of it:
header, function context, every line with the side it is printed from, and the
end-of-file marker, against `git diff -U<n>` and `git diff -w -U<n>` at one, three and
eight lines over a fixture with whitespace-only edits beside real ones and unended last
lines (`the_view_draws_what_git_diff_draws_at_every_context_with_whitespace_ignored_or_not`),
and against plain `git diff` with `diff.context=5` and `diff.interHunkContext=3` set, at
the configured context and at `-U1`
(`the_view_groups_hunks_as_the_users_git_diff_does`), beside the existing comparisons
under every algorithm and over this repository's history.

## The accelerator table

As-built for PRD R8 and decision D5 (`crates/cairn-ui/src/accelerators.rs`). Every
shortcut is an `Action` mapped by `accelerators::chord(action, os)` to at most one
`Chord` per `Os` — macOS, and Linux for every other platform — and by
`accelerators::heard_in(action)` to the `Scope` it is heard in: `Window`, wherever
focus is, or `Detail`, only while focus is inside the detail pane. The table is data:
one match naming every action for the chord, one for the scope. A chord holds its
modifiers exactly (the lock keys are ignored, an extra Shift is not, for a key and a
pointer press alike), and is completed by a key that names itself (an arrow, matched by
the key), by a key where it sits (a digit, matched by its physical position, since
Option turns `1` into `¡` on macOS), or by a primary pointer press.

The chords are Fork's and only Fork's — the user's decision of 2026-10-03, from
`docs/research/diff-engine/fork-shortcuts.md`:

| Action | Linux | macOS | Heard |
| --- | --- | --- | --- |
| previous / next change | Ctrl+↑ / Ctrl+↓ | ⌘↑ / ⌘↓ | in the detail pane |
| Commit tab / Changes tab | Ctrl+Alt+1 / Ctrl+Alt+2 | ⌘⌥1 / ⌘⌥2 | anywhere |
| extend the selection to a second commit | Ctrl+press | ⌘+press | anywhere |
| toggle side-by-side, toggle ignore whitespace, more lines, fewer lines, entire file | none | none | — |

Change navigation is scoped to the pane so a text field elsewhere keeps those keys (⌘↑
is the start of the document on macOS). The 3 beside the tab chords is kept for a File
Tree tab and is nothing yet. Fork binds no chord to the four diff toggles or the entire
file, so they are actions without one, reached from the diff's header (phase 06). The
previous and next file are not chords at all: they are the focused file list's own ↑
and ↓, with Tab and Shift-Tab moving focus, as Fork does (the detail pane, above). No
collapse chord: the user's decision. Every chord holds a modifier: an unmodified key
belongs to whatever has focus.

**The contract.** A component asks `accelerators::resolve_key(event, scope)` which
action a key press is in a scope, or `accelerators::is_chord(event)` whether it is any
action's chord, and never reads the held keys itself; the module's public surface
speaks actions, scopes and chords, never a modifier a caller could branch on — but for
`Chord::key_press` and `Chord::press_hold`, which hand a chord's keys to a headless test so
it presses a chord, or holds a pointer chord's keys, through the table rather than spelling
them. A pointer press is resolved against `HeldKeys`, the keys the window heard held (a
press carries no modifiers in this Freya build): it answers which `Action` a press is,
never which key is down (phase 08). The window hears `Scope::Window` on every
key press (`on_global_key_down` on its root), and keeps `HeldKeys` from every key down and
up it hears; the detail pane hears `Scope::Detail` on
the key presses that reach it from whatever inside it has focus (its root's
`on_key_down`); both act in `crates/cairn-app/src/shortcuts.rs`, one arm per action,
and nothing acts while a credential prompt is up, since the dialog owns the keys until
it is answered (`no_accelerator_acts_while_a_credential_prompt_is_up`). The two tab
chords show their tab, opening a collapsed pane; previous and next change move the diff
(phase 06); and the extending press selects a second commit to compare (phase 08, "Comparing
two commits" above). The history list and the file list leave a
chord alone whichever scope hears it, so Ctrl+↓ is "next change", never "next commit"
(`an_accelerators_chord_does_not_move_the_selection`). Pinned by
`the_table_is_forks_chords_and_no_others` (the whole table, spelled out per platform),
`every_chord_resolves_to_its_action_in_its_scope_only`,
`chords_are_distinct_and_every_one_holds_a_modifier`,
`the_command_key_is_the_platforms_own`,
`a_chord_needs_exactly_its_modifiers_and_ignores_the_locks`,
`a_physical_chord_is_matched_by_where_the_key_sits`,
`a_chord_of_either_scope_is_a_chord`,
`the_change_chords_and_the_arrows_belong_to_the_focused_pane` (a pane hearing
`Scope::Detail` as the window's does), and `the_tab_chords_resolve_through_the_table`
through the window. That the window's detail pane hears the change chords is not
observable until phase 06 gives them something to do. That no component names a
literal modifier is the guard `no_component_names_a_literal_modifier`, and that the
table holds data and resolution only is `the_accelerator_table_holds_data_and_resolution_only`
(root `CLAUDE.md`, Invariants).

## What a commit's details carry

`CommitDetails` is R1.8: the author and the committer as separate `Signature`s,
each with a name, an email and a `Timestamp` that keeps its own offset; the whole
message, with `subject()` and `body()` reading it; and the parents in git's order,
none for a shallow clone's boundary commit, whose parents the clone lacks — read from
the shallow file by `crates/cairn-git/src/shallow.rs`, as the history walk reads it.
It sits beside `CommitSummary` rather than replacing it — a history row draws a
subject and one name, and carrying a committer, an offset and a whole message per
row of a ten-year monorepo would be paying for what no row draws.
`Timestamp::offset` spells the offset the way git writes it, `+0530` or `-0800`,
pinned by `an_offset_reads_the_way_git_writes_it`.

**Names, addresses and messages are the characters git shows** (user decision 4,
2026-10-03; `crates/cairn-git/src/commit_encoding.rs`). git converts a whole commit
from the encoding its `encoding` header names before printing any of it — no header
means UTF-8 — and prints the object's bytes unconverted when the conversion fails: a
sequence the encoding refuses anywhere in the object, or an encoding iconv does not
know. `CommitEncoding::of_commit` decides as git does, and both the details and the
history row read every field through it. ISO-8859-1 (under any name glibc's iconv
accepts, and `Latin-1`, which git renames to it) is decoded as itself; US-ASCII is
UTF-8 either way; every other encoding goes through `encoding_rs` (a dependency the
user approved on 2026-10-03), whose WHATWG tables differ from glibc's where a `Quirk`
puts glibc's reading back: a windows code page's undefined bytes refused, ISO-8859-9
and ISO-8859-11 keeping their C1 controls and TIS-620 refusing them (the WHATWG
standard reads all three as windows code pages), Shift_JIS under JIS's names reading
`0x5c` and `0x7e` as `¥` and `‾` and six JIS X 0208 codes as JIS does and refusing the
rows JIS X 0208 leaves empty, CP932 refusing the lone bytes glibc refuses, EUC-JP's six
codes, GB18030's private-use codes and lone `0x80`/`0xff`, KOI8-U's two box
drawings and Mac Roman's two characters. Measured 2026-10-03 by decoding every 1- and
2-byte sequence, and GB18030's 4-byte codes from `0x81308130` to `0x8439fe39`, with
glibc's iconv and with `encoding_rs`: after the quirks, Shift_JIS, CP932, GB18030 (over
that range), KOI8-R and KOI8-U, the ISO-8859 family, the windows code pages, IBM866 and
Mac Roman read alike. Anything else — a UTF-16 header, the WHATWG standard's
`replacement` labels, a name neither knows — reads as UTF-8, invalid sequences as
U+FFFD, as a UTF-8 terminal shows git's raw bytes. `i18n.logOutputEncoding` (and
`i18n.commitEncoding`, its fallback) changes only which bytes git writes for the same
characters — measured — so a view that draws characters has nothing to apply. Pinned
against `git log`'s `%an`, `%ae`, `%cn`, `%ce`, `%s` and `%B` on crafted commits, one
per way git reads a commit's text, by
`the_text_of_an_encoded_commit_is_the_text_git_prints`, and quirk by quirk against
`iconv` by `each_encoding_reads_as_glibc_reads_it`.

## Known limits

- **Past 2^24 pixels the virtualising view's own arithmetic is `f32`.** Cairn's previous
  and next change compute in whole numbers, but Freya's `VirtualScrollView` places rows and
  reads the scroll offset in `f32`, which holds whole pixels exactly only to 2^24 — about
  987,000 rows, reachable only by a file loaded past the limits. Where rows are placed
  past that is the toolkit's and no test pins it.
- **The filter answers for one change set at a time**, and keeps its text for the session:
  a filter typed over one commit is asked again over the next.
- **Opening or closing an early file re-places every file open after it** (phase 08). The
  Commit tab's table of opened files is rebuilt on the UI thread from the first file a
  change touched; a page of Expand All appended costs the page, but a press near the top
  with hundreds of files open below re-places them — bounded by Expand All's budget, and by
  how many files a person opens by hand.
- **The Commit tab's first draw in a session costs a frame of its own** (phase 08 window
  check): 14-15 ms of UI-thread work, headless and before paint, the first time the tab is
  drawn whatever the commit (11.5 ms over 2,828 files and over 27,592 alike) — the toolkit's
  first text of each face and fallback — and under 4 ms every time after. With paint, that
  one frame may be dropped; the window check measures paint only as an encoded snapshot.
- **A comparison's base is found by one scan of the loaded rows per press**
  (`selection::extend`), as a parent link is (`selection::loaded_row`).

- **A kept answer is as fresh as the files the thread can name.** Every file git or
  gix reads for a commit's diff is stamped before each query ("In the application"),
  with these residuals. A git built with a system configuration directory other than
  `/etc` (or `GIT_CONFIG_SYSTEM`) reads a system file gix never reads and the thread
  never stamps — the residual `docs/systems/git-processes.md` already states for
  opening; an edit to it is not seen by a kept answer, nor by gix at all. Where
  Cairn's environment names a global file (`GIT_CONFIG_GLOBAL`) other than the one
  `git`'s environment reads, both files are stamped, but the includes of the one
  only `git` reads are not named. An include path spelled with `%(prefix)` is not
  resolved, so not stamped. The process's own environment (`GIT_CONFIG_COUNT`,
  `GIT_CONFIG_PARAMETERS`) cannot change under it. A filesystem coarser than the
  two seconds `SETTLING` allows, or a file whose times are in the future, is never
  trusted rather than trusted wrongly: answers under it are not kept. And the
  history thread keeps the configuration of its first open: it reads no diff key.
- **A commit's encoding is read as glibc's iconv reads it, and only as far as was
  measured.** Where `encoding_rs` and glibc still differ, Cairn's text is not git's:
  EUC-JP sequences glibc reads and `encoding_rs` refuses (lone C1 bytes, some JIS X 0212
  codes) send the object to its raw bytes where git converts it, and some it reads that
  glibc refuses (NEC's and IBM's rows) are converted where git prints raw; GBK and
  GB2312 decode the user-defined and GBK-only codes glibc refuses; EUC-KR is read as the
  WHATWG standard's UHC, Big5 with HKSCS where glibc has private-use points, and
  ISO-2022-JP was not measured; GB18030's 4-byte codes past `0x8439fe39` were not
  compared; a UTF-16 commit reads as UTF-8 where iconv would convert it; and the parity
  is with glibc — on macOS, git's iconv is libiconv, whose tables were not compared.
- **The message's tab expansion counts a character's width from a table of Unicode's
  main wide and zero-width ranges**, not git's whole one: a tab after a wide character
  outside them lands a column or two from where `git log` puts it.
- **The working-tree query answers one path, named by its caller.** It pairs no
  rename (a path staged by `git mv` shows as added, as `git diff --cached -- <path>`
  shows it), and `Untracked` answers `git diff --no-index` for the path whatever the
  index holds: which paths are untracked is status's to say.
- **A clean filter whose output differs run to run** is refused as
  `ContentReadsDisagree` every time, since git's two reads of the file never agree.
- **A failed clean filter that is not `required` is shown as git shows it**, the
  unfiltered content diffed, without git's stderr warning, which is prose Cairn
  never parses.
- **A binary working-tree side's size is the file's on disk**, before any filter:
  git prints no content for it.
- **A sparse index is unsupported**, though git's plumbing would answer it; reading
  one is gix's to do, and this query does not yet.
- **The configuration is the handle's**, read when it was opened (`diff.algorithm`,
  `diff.ignoreSubmodules`, the drivers' algorithms); the application's diff thread
  opens its handle again when a configuration file moves. The index and the
  attributes are read fresh, by gix and by git, for every working-tree query.

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
- **No function context after the second `@@` of a patch.** git writes the
  enclosing declaration there and ignores it on apply; the emitter writes nothing,
  and the view's header rows take theirs from the overlay, which the emitter cannot
  see.
