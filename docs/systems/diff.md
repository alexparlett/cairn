# Diff

How Cairn describes a change to a file today. As-built: everything here is code
that exists, and behaviour is pinned by a test named beside it.

**What exists is the model and the engine that fills it.** `cairn-model` can hold
a file diff, project it into hunks and rows, and emit a unified patch from a
selection of lines; `cairn-git` answers what a commit or a comparison changed,
what one of those changes is, line by line, and one path's staged, unstaged or
untracked diff in the working tree, against a real repository. No
component draws one and nothing stages anything — the patch emitter still ships
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
textconv program, and `gix::diff::resource_cache` builds it with
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

**Expand All.** `DiffSession::file_diffs(.., &ChangeSet, ..)` answers every file of
a change set, decided as one file is, but asks git ONCE for the whole comparison:
`diff-tree -p` with the change set's own detection (`-M`/`-C` and the `-l` git
applied, `--ignore-submodules=all` where the user hides every gitlink), no
pathspec, and no `-a`, so a binary file costs git a line. The files whose diff
drivers name an algorithm of their own (git 2.40 and later) are asked in one more
`diff-tree -p` per distinct algorithm — that algorithm passed, the group's paths as
literal pathspecs, the same detection — since one call cannot pass two algorithms,
and one that leaves a driver to apply its own carries it into every later file (the
known limit below). Each text file's patch is found by its paths and checked to be
the same change between the same blobs; a file a run's answer does not hold that
way — paired otherwise, which a hidden submodule's place in a cut-short rename
search or a group's narrower paths can do, or called binary — is asked about on its
own. `expand_all_answers_what_each_file_answers_alone` requires every answer equal
to the per-file one, with and without `-w`, over the crafted, rewrite, submodule,
attribute, whitespace and discriminating fixtures, the last with a driver algorithm;
`expand_all_runs_one_diff_tree_per_comparison` counts the `diff-tree` runs in the
command log, which those equal answers cannot show; and
`a_path_git_quotes_reads_as_git_diff_shows_it_alone_and_through_expand_all` and
`a_renamed_files_driver_algorithm_is_its_old_paths` hold paths git quotes and a
rename across a driver's boundary (the driver is the old path's, as in git's
`run_diff`) to `git diff` alone and through Expand All.
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
`diff-files --no-renames`, or `diff --no-index` and `/dev/null <path>` — porcelain,
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
a raw-only read, never) — with that environment plus what git
sets for a filter (`GIT_DIR`, `GIT_WORK_TREE`, `GIT_EXEC_PATH`, `GIT_PREFIX`,
`GIT_CONFIG_PARAMETERS`, git's exec directory first on `PATH`), its stderr the
read's bounded tail; `diff-index --cached` reads only objects. Every read of the
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
`Unsupported`. For these stand-in states the file carries the path and no mode or
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
`required` that fails is git's `fatal` and `Error::GitFailed`, whose message names
the path; one not required makes git fall back to the unfiltered content, which is
what Cairn shows, as `git diff` does
(`a_failing_clean_filter_is_an_error_naming_the_path_or_what_git_shows`).

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
`the_function_context_is_git_diffs_at_every_context`.

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

- **The configuration is gix's view of it, as of when the repository was opened.**
  The two rename keys are read from the configuration gix loaded, so a change the
  user makes while Cairn has the repository open is not seen until it is opened
  again — as for every other key gix reads, `diff.algorithm` among them. And gix
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
- **`diff.context` and `diff.interHunkContext` are not read.** The view's context is
  its own setting (R6.3), and hunks merge at twice the context, as `git diff` does
  when neither key is set. A user who sets either sees `git diff` group hunks
  otherwise than Cairn; the changed lines are the same.
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

**Residual, stated rather than implied:** nothing guards that a reader of
`DiffContent` names every variant the way the `RowContent` invariant does for a
history row. A wildcard arm over it would compile and draw nothing for a state
added later. Today the model itself is the only reader; when the views land, C11
("every R6.8 state draws its notice") is what decides it, and whether that
deserves a guard of its own is a question for the packet's QA phase.

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

## Known limits

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
- **The configuration is the handle's**, read when the repository was opened
  (`diff.algorithm`, `diff.ignoreSubmodules`, the drivers' algorithms); the index
  and the attributes are read fresh, by gix and by git, for every query.

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
