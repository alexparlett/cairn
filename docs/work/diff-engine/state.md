# State — diff-engine

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 02's rework landed and the C6 audit is done; the content-parity
rework landed (2026-10-03) — a file's changed lines, its whitespace-ignoring lines
and each hunk's function context now come from `git diff-tree -p`, closing the
audit's F1 — and its QA (round 3) is fixed: opening refuses a bare repository git
would refuse to find, a read's `core.fsmonitor` is documented parity, Expand All
batches driver files per algorithm, and the parity and gate pins it asked for
landed. Phase 03 landed (2026-10-03) and its QA is fixed: one path's staged, unstaged and
untracked diff, answered by `git diff-index --cached`, `git diff-files` and `git
diff --no-index` run as reads, the working-tree side rebuilt from git's patch so it
is git's form of the file (clean filter driver run by git, with the read's
environment), and every R3.4 state answered. The user accepted `git diff
--no-index` for an untracked file as the one porcelain exception to the reads rule
(2026-10-03), its presentation settings pinned by `-c`. Phase 04 landed
(2026-10-03) and its QA is fixed: queries are numbered per lane (history, changes, file diff), the
changes and file-diff lanes run on a `cairn-diff` thread routed to at submit time, a
superseded diff's `git` is killed by its epoch, and every answer names the selection it
answers, which `DiffState` checks before the window keeps it; a commit's diff reads
its attributes where git does (the working tree first, nothing from `HEAD`), and the
diff thread keeps an answer only while every file git read for it is as it was — the
user's decision "stamp what git reads" — opening its handle again when the
configuration moves. Phase 05 landed (2026-10-03) and its QA is fixed (the user's
decisions recorded in `progress.md`: dates in Fork's fixed English format, and commit
encodings read through `encoding_rs` with glibc's quirks): choosing a commit asks
its changes, and the detail pane under the list — draggable, collapsible, Commit and
Changes tabs, the tab kept for the session — draws the Commit tab (author, committer,
full timestamps at their offsets, full id, parent links, whole message, a virtualised
file list with renames' both names and the cut-short notice); the accelerator table
(R8) and its guarded invariant "no component names a literal modifier" landed, and
`DiffContent` is held to the RowContent rule in production code. Phase 06 landed
(2026-10-03) and its QA is fixed (the user's decisions recorded in `progress.md`: the bar
is Fork's named glyphs lit in the accent, sizes are Fork-measured where established, a
`diff.context` edit is adopted live while the user has not moved the context, and tabs
stop at terminal widths through `unicode-width`, user-approved): a file pressed in the Commit tab has its diff drawn in the
Changes tab as the unified rows `git diff` prints (context lines from the new side,
git's end-of-file marker, hunks grouped with the user's `diff.interHunkContext`, the
whitespace-ignoring ranges under `-w`), through the virtualising view, under Fork's bar
(previous/next change, ignore whitespace with its notice only when something is hidden,
fewer/more lines never below one, entire file, side-by-side disabled until phase 07);
the context starts at the user's `diff.context`; Fork's colours are retuned tokens; IBM
Plex Mono Regular is embedded with its OFL licence. Phase 07 landed (2026-10-03; QA due):
the Changes tab is Fork's — a one-line summary, the changed files behind a filter answered
on the repository thread in a lane of its own, the first file chosen by default, one
file's diff — side-by-side is drawn (two equal columns in the one virtualising view, the
same parity bar as unified, one shared setting), every non-text state draws its notice
with Load Diff for a file past the limits, a line past the long-line limit is drawn cut
with a marker, `ShownDiff` is prepared on the diff thread (moved to `cairn-model`),
previous/next change computes in whole numbers, and the row-exhaustiveness guard covers
`UnifiedRow` and `SideBySideRow`.** The diff model exists in
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

**Phase 06 QA's four obligations to phase 07: all met (2026-10-03, `progress.md`).**
R6.9's cut landed with Load Diff (`cairn_model::drawn_bytes`, `cut_marker`;
`a_line_past_the_limit_is_drawn_cut_with_its_marker_in_both_views`); `ShownDiff` is built
on the diff thread (`worker/diff_lane.rs`, `Update::FileDiff` carries it); `step_change`
computes in whole numbers (`a_change_a_million_rows_down_is_stepped_to_exactly`); the
guard covers both row enums (`every_view_of_a_diff_row_names_every_kind_of_row`).

**The user's decisions on phase 07's Cairn-chosen behaviours (2026-10-03): applied.** The
filter folds case as Unicode reads it, keeps its text for the session, keeps a hidden chosen
file shown, and says "Showing N of M files" while active; side-by-side stays as built;
"Renamed without changes", "Unmerged path — conflicts must be resolved before a diff can be
shown", sizes in KiB; the cut marker " … N more bytes"; the file list at 35% of the pane,
never below 200 px. Recorded in `docs/systems/diff.md`'s table and `progress.md`.

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

**"Cut short" across the seam: decided in phase 04.** `ChangeSet` and
`RenameDetection` (with `was_cut_short()` and `needed_limit`) moved to `cairn-model`,
so the change set crosses the worker boundary as it is and phase 05 draws R2.2's
notice from it without naming the engine.

**For phase 05: what phase 04 left to wire.** `DiffState::select_changes`,
`select_file` and `expand_all` return the `Request` to submit; nothing calls them yet
(the commit list's selection is phase 05's to wire), and `FileTarget`/`WorkingSide`
are re-exported from `worker` for tests only until the window names them. The window
draws an `Answer` per selection: `Waiting`, `Ready`, or `Failed` with display text.

**For phase 08: Expand All answers per file (phase 04 QA, R1; deferred by the
orchestrator).** Today one file that fails — a read git fails, a disagreement —
fails the whole batch as one `DiffFailed`. Phase 08 answers each file's outcome
beside the others, so one bad file shows its own failure and the rest draw.

**For phase 08: Expand All's lane is ready to page.** `Update::FileDiffs` carries
`complete`, and `DiffState::expansion_arrived` appends batches; today the diff thread
sends one batch, complete, from one `DiffSession::file_diffs` call.

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
| `ChangeSet` | model (moved from git in phase 04) | What a commit or a comparison changed: the files sorted by a total key, the commit's details when one commit was named, and how rename detection went. |
| `RenameDetection` | model (moved from git in phase 04) | Whether detection was on and found copies, the limit git applied, and `needed_limit` / `was_cut_short()`: R2.2's "the answer says so", decided from git's answer and the limit, never from stderr. |
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
| `reads::working_tree_patch`, `reads::WorkingTreeQuery`, `reads::Side`, `reads::WorkingTreeAnswer` | git (crate-private) | The read: `diff-index --cached` / `diff-files` with `:(literal)<path> :(exclude,glob)<path>/**`, or `diff --no-index -- /dev/null <path>` (`<path>` work-tree-relative, `reads::work_tree_relative`; `./-` for `-`) with `NO_INDEX_PRESENTATION` pinned by `-c`; `-z --raw -p --full-index`, no `-a`; at most one record and its sections; a ceiling on stdout (`PastCeiling`); `--no-index`'s status 1 an answer only with a record. |
| `PatchText::new_side`, `new_index_id`, `submodule_targets`, `has_hunks`; `Parser::finish_listing` | git (crate-private) | The new side rebuilt from the old and git's patch, every printed line checked; the id on the `index` line; git's `Subproject commit` lines and `-dirty`; records and sections unmatched, for one path. |
| `Invocation::finish_within` | git (crate-private, `process/`) | `finish` with a stdout ceiling: the crossing chunk withheld, the process ended, `GitOutputTooLarge`; what arrived before a failed exit is already the caller's. |
| `diff::submodules::working_tree_ignore` | git (crate-private) | The `--ignore-submodules` value porcelain applies and plumbing does not read: `diff.ignoreSubmodules`, unless the submodule has an `ignore` of its own. |
| `QueryLane`, `Epoch`, `Epochs::bump(lane)`, `Superseded` | app (`worker/epoch.rs`) | Phase 04, R4.1: epochs numbered per lane; `QueryLane::supersedes` is the rule (a changes query also supersedes the file-diff lane); `Superseded` is the `Cancel` for walks and diff reads alike. |
| `route`, `Routed`, `RepositoryJob`, `Page`, `thread_of` | app (`worker/routing.rs`) | Phase 04, R4.2: the routing table, applied in `submit`; history to `cairn-repository`, changes and file diff to `cairn-diff`. `thread_of`/`Thread` are the lanes' half, test-only. |
| `serve_diffs`, `Serving`, `DiffJob`, `READ_ATTEMPTS`, `Kept` | app (`worker/diff_lane.rs`) | Phase 04, R4.2-R4.5: the diff thread; newest request per lane, file diff first, blocks when idle; a disagreeing working-tree read asked again up to three times (a commit's never); kept state renewed per what moved (QA: the handle reopened when the configuration moves). |
| `Answers`, `searched_paths`, `held_bytes` | app (`worker/diff_answers.rs`) | Phase 04 QA: commit/comparison answers kept, bounded, keyed by everything asked, each with the `Dependence` it read; a hit checks it; the budget counts ranges and the overlay. |
| `Stamp`, `SETTLING`, `Dependence`, `Directories`, `SessionReads`, `Freshness`, `Moved`, `Checked` | app (`worker/diff_freshness.rs`) | Phase 04 QA, "stamp what git reads": every file a kept answer depends on stamped before each query, three tiers (configuration → reopen; global and staged → drop all; per-path `.gitattributes` → drop that answer, and the session when it read the directory); a stamp within two seconds of its file's change matches nothing. |
| `DiffInputs`, `StagedInputs`, `Repository::diff_inputs`, `staged_inputs`, `head_id` | git | Phase 04 QA: the files outside the object database a commit's diff reads (configuration with includes and missing files, global attribute files, `.gitmodules`, per-directory `.gitattributes`) and the index's attribute and `.gitmodules` entries by value. |
| `SharedRepository::reopen_for`, `opened_at`, `Error::RepositoryReplaced` | git | Phase 04 QA: a worker handle opened afresh by the application's route, sharing the registry and log; another repository at the path is refused. |
| `Comparison`, `WorkingSide`, `DiffOptions`, `FileTarget`, `FileQuery`, `DiffQuery` | app (`worker/request.rs`) | Phase 04, R4.4: what a diff request names and its answer names back. |
| `Request::Changes`/`FileDiff`/`ExpandAll`, `Update::Changes`/`FileDiff`/`FileDiffs`/`DiffFailed`, `Request::lane()` | app (`worker/request.rs`) | Phase 04: the diff boundary; `lane()` replaced `is_query()`. A superseded query sends nothing. |
| `DiffState`, `Answer`, `Expanded` | app (`src/diff_state.rs`) | Phase 04, R4.4: the diff selection in `View::diff`; an answer is kept only when it names what is selected now. |
| `cairn_ui::accelerators` (`Action`, `Os`, `Scope`, `Chord`, `chord`, `heard_in`, `resolve_key`, `resolve_key_on`, `is_chord`, `resolve_press_on`) | ui | Phase 05, R8: the accelerator table; at most one chord per action per platform (Fork's only, user decision 6), each heard in a `Scope` (`Window`, or `Detail` inside the detail pane); `Chord::key_press` is for headless tests only. |
| `CommitTab`, `DETAIL_ROW_HEIGHT`, `cut_short_notice`, `status_letter`, `file_text` | ui (`commit_tab.rs`) | Phase 05, R5.3/R5.5: the Commit tab as one fixed-row virtualised list over a `Readable<ChangeSet>`, header cached per commit; `on_parent`; focusable, ↑/↓ moving the current file, `on_file`. |
| `message_lines::shown_lines` | ui (private) | Phase 05 QA: a message's lines as `git log` shows them — blank lines around it dropped, trailing whitespace trimmed, tabs expanded. |
| `CommitEncoding`, `Quirk` | git (crate-private, `commit_encoding.rs`) | Phase 05 QA: how git reads a commit's text — its `encoding` header, ISO-8859-1 by hand and the rest through `encoding_rs` with glibc's quirks, git's whole-object fallback to bytes. |
| `Request::Retire`, `Retired` | app (`worker/request.rs`) | Phase 05 QA, R2: answers the window let go of, freed on the repository thread; `DiffState::select_changes` returns the query and, when answers were kept, this. |
| `DetailTab`, `DetailTabs`, `DETAIL_STRIP_HEIGHT` | ui (`detail_tabs.rs`) | Phase 05, R5.2: the strip, its tabs and the collapse control. |
| `HistoryList::controller`, `reveal_row` | ui (`history_list.rs`) | Phase 05: the list scrolled by a shared controller, so a parent link reveals its row; a key the table resolves is left alone. |
| `date_text::long_date` | ui (private) | Phase 05 QA: Fork's Windows date, `25 Nov 2020 01:11:30 +01:00`, at the recorded offset (replaced `git_default`). |
| `selection::{comparison_of, choose, loaded_row}` | app (`src/selection.rs`) | Phase 05: a row chosen asks its changes through `DiffState`; `comparison_of` names every `RowId`. |
| `DetailPane`, `NOTHING_SELECTED`, `READING`, `CHANGES_NOT_BUILT`, `NOT_ONE_COMMIT` | app (`src/detail_pane.rs`) | Phase 05: the pane, drawing an answer only for the selection now. |
| `shortcuts::act` | app (`src/shortcuts.rs`) | Phase 05: what each `Action` does; the diff actions are placed and act from phases 06-08. |
| `View::{history_scroll, detail_tab, pane_collapsed, pane_height}`, `window::PANE_HEIGHT`, `diff_state::answered_changes` | app | Phase 05: session state of the pane; the change set as the Commit tab reads it. |
| `names_a_literal_modifier`, `MODIFIER_IDENTS`, `MODIFIER_METHODS`, `MODIFIER_TEXT`, `reads_enum_partially` | guards | Phase 05: the R8.3 matcher; the RowContent matcher generalised to `DiffContent`. |
| `spells_a_chord`, `production_char_literals`, `names_an_element`, `ELEMENT_BUILDERS` | guards | Phase 05 QA: chord text in string and char literals with escapes read; the accelerator table held to data and resolution. |
| `UnifiedRow::NoNewlineAtEnd`, `UnifiedLayout` (`exact`, `shown`, `row`, `change_rows`, `first_change_from`, `next_change_after`, `previous_change_before`), `UnifiedRows::shown`, `DrawnRanges`, `Hunks::of_ranges`, `FunctionContext::with_inter_hunk_context`/`inter_hunk_context` | model | Phase 06: the unified rows as `git diff [-w]` prints them (context from the new side, the end-of-file marker, no hunk without a change), an owning layout a view keeps across frames, git's grouping with `diff.interHunkContext`. |
| `Repository::configured_context`, `diff::hunk_grouping::Grouping` | git | Phase 06: `diff.context` (raised to one) and `diff.interHunkContext` read as porcelain reads them; a value git refuses is `InvalidConfig` for every content query; the inter-hunk context carried in each answer's function context. |
| `ShownDiff`, `UnifiedDiffView`, `DIFF_ROW_HEIGHT`, `NO_NEWLINE_AT_END`, `ChangeCursor`, `step_change` | ui (`diff_view.rs`) | Phase 06: one file's diff prepared once per answer; its rows through `VirtualScrollView` at a fixed height; previous/next change from the view or the change last moved to. Phase 07: `ShownDiff` moved to the model, `UnifiedDiffView` became `DiffView`, `step_change` takes `ChangeStops` in whole numbers. |
| `DiffHeader`, `HeaderAction`, `*_LABEL`, `HIDDEN_CHANGES_NOTICE`, `DIFF_HEADER_HEIGHT` | ui (`diff_header.rs`) | Phase 06: Fork's bar, buttons only, side-by-side disabled; the notice only when the answer hides a change. Phase 06 QA: each button a glyph named with Fork's tooltip where recorded (`*_LABEL`, replacing `*_CAPTION`), lit in the accent while on. |
| `toggle_glyphs::Glyph` | ui (private) | Phase 06 QA: Fork's glyphs from plain shapes, one colour each. |
| `columns::columns` | ui (private) | Phase 06 QA: a character's terminal columns (`unicode-width`), the one rule the diff's and the message's tab stops use. |
| `DiffSettings` | ui (`diff_settings.rs`) | Phase 06: the shared session settings — lines (never below one), entire file, ignore whitespace — starting at the configured context until the user moves it. |
| `diff_palette` (`REMOVED_TINT`, `ADDED_TINT`, `*_EMPHASIS`, `DIFF_TEXT`, `DIFF_MUTED`, `GUTTER_SEPARATOR`, `HEADER_BAR`, `CURRENT_CHANGE`, `GROUND`, `retuned`, `DIFF_FONT_FAMILY`, `DIFF_FONT_SIZE`, `MONO_ADVANCE_EM`) | ui | Phase 06: Fork's measured dark values retuned to Cairn's ground, and IBM Plex Mono. |
| `shown_line`, `ShownLine`, `TAB_WIDTH`, `widest_columns` | ui (`diff_line_text.rs`) | Phase 06: a line's bytes as a row draws them (tabs to eight, control pictures, a CRLF `\r` hidden), its intra-line byte ranges carried to UTF-16 units. |
| `CommitTab::on_file_pressed` | ui | Phase 06: a press distinct from an arrow. |
| `Request::ConfiguredContext`, `Update::ConfiguredContext`, `Routed::ConfiguredContext`, `DiffJob::ConfiguredContext` | app (`worker/`) | Phase 06: `diff.context`; nothing sent for a refused value. Phase 06 QA (T7): read on the diff thread, whose handle the configuration's freshness reopens, and sent again on every reopen (`RepositoryJob::ConfiguredContext` gone). |
| `embedded_font_violations`, `EMBEDDED_FONTS` | guards | Phase 06 QA (C2): the fonts directory is exactly the roster, each font beside its licence. |
| `diff_actions` (`choose_file`, `change_settings`, `configured`, `step`, `options`), `shortcuts::of_header`, `View::{diff_settings, diff_scroll, change_cursor}`, `diff_state::answered_file`, `DiffState::shown_file` | app | Phase 06: choosing a file, the shared settings re-asking through the lane, previous/next change; `select_file` now returns the query and a `Retire` for the replaced diff. |
| `detail_pane::{NO_FILE_CHOSEN, READING_DIFF, NO_CHANGES_SHOWN, ONLY_WHITESPACE_CHANGED}` | app | Phase 06: the Changes tab's sentences (replacing `CHANGES_NOT_BUILT`). Phase 07: moved to `changes_tab` and `cairn_ui::diff_notice`. |
| `Error::ChangesCancelled`, `ContentCancelled`, `ContentReadsDisagree`, `DiffSetup`, `DiffFile`, `UnexpectedGitOutput`, `InvalidConfig`, `NotAWorkTreePath` | git | What the caller of a diff query must handle; `TreeDiff` went with the gix tree walk. `ContentReadsDisagree` is the stale-read guard: git printed lines that are not the lines gix read; ask again. `NotAWorkTreePath`: an untracked path that is empty, absolute or has a `.`/`..` component, refused before anything runs. |
| `SideBySideLayout`, `SideBySideRows::shown`, `SideBySideRow::NoNewlineAtEnd { old, new }`, `ChangeStops`, `UnifiedLayout::stops` | model (`diff_rows.rs`) | Phase 07: side-by-side rows as `git diff` prints them — the unified grouping, a context line from the new side in both columns, git's marker in the column of the side that did not end; the changes previous/next change search, for either layout. |
| `ShownDiff`, `drawn_bytes`, `widest_drawn_columns`, `LINE_CUT_BYTES`, `TAB_STOP` | model (`diff_shown.rs`) | Phase 07: one answer prepared for both views, built on the diff thread; R6.9's cut at the long-line limit on a character; the widest drawn line. |
| `ChangeSet::files_matching` | model | Phase 07: the Changes tab's filter — the text in a path, a rename by either name, case folded as Unicode reads it (user decision) — stopping when told. |
| `DiffView`, `content_width`, `text_width` | ui (`diff_view.rs`) | Phase 07: one file's diff, unified or side by side (`.side_by_side`); the pixel widths of a prepared diff. |
| `unified_rows::build`, `side_by_side_rows::{build, Columns}`, `diff_row_parts` | ui (private) | Phase 07: a unified row; a side-by-side row of two equal columns sliding together; the shared pieces (number, separator, marker, line text with its cut marker). |
| `DiffNotice`, `DiffNoticeView`, `TOO_LARGE_TO_DISPLAY`, `LOAD_DIFF_CAPTION`, `BINARY_FILE`, `LFS_POINTER`, `SUBMODULE`, `NO_CONTENT_CHANGE`, `CONFLICTED`, `NO_CHANGES_SHOWN`, `ONLY_WHITESPACE_CHANGED`, `OLD_SIDE`, `NEW_SIDE`, `header_lines`, `size_text`, `too_large_reason`, `subproject_line` | ui (`diff_notice.rs`) | Phase 07, R6.8: what stands in place of rows, every state named; Load Diff reported through `on_load`. |
| `cut_marker`, `ShownLine::cut` | ui (`diff_line_text.rs`) | Phase 07, R6.9: the muted " … N more bytes" after a cut line, N the bytes not drawn (user decision). |
| `ShownFiles::Waiting`, `FILTERING`, `filter_count`, `RENAMED_WITHOUT_CHANGES`, `COPIED_WITHOUT_CHANGES` | ui | Phase 07, the user's decisions: the filter's first answer awaited; "Showing N of M files"; the rename notice's title. |
| `ChangesList`, `ChangesSummary`, `summary_parts`, `FILTER_PLACEHOLDER`, `NO_FILE_MATCHES`, `SUMMARY_HEIGHT` | ui (`changes_list.rs`) | Phase 07, R5.4: the filtered, virtualised file list with ↑/↓, and Fork's one-line summary. |
| `ShownFiles` | ui (`file_filter.rs`) | Phase 07: every file, or a filter's indices; rows to files and back. |
| `DiffSettings::{side_by_side, toggle_side_by_side}`, `HeaderAction::SideBySide`, `diff_palette::FILLER` | ui | Phase 07, R6.1: the shared setting, the bar's button enabled, Fork's filler grey retuned. |
| `QueryLane::FileFilter`, `Request::FilterFiles`, `Update::FilteredFiles`, `RepositoryJob::Filter`, `Retired::of(changes, diffs, shown)` | app (`worker/`) | Phase 07: the filter's lane, on the repository thread; `Update::FileDiff` carries a boxed `ShownDiff`; a retired change set is an `Arc`. |
| `ChangesTab`, `LIST_WIDTH`, `NO_FILE_CHOSEN`, `READING_DIFF` | app (`changes_tab.rs`) | Phase 07: the Changes tab — summary, list, diff side — with the effects that ask the filter and choose the first file. |
| `FileFilter`, `DiffState::{filter, filter_again, filter_arrived, wants_filter, filter_text, filter_is_settled, chose_file_at, file_index}`, `diff_state::answered_files` | app | Phase 07: the filter as the window keeps it, inside `DiffState`, for the change set selected now. |
| `diff_actions::{load_anyway, toggle_side_by_side}`, `View::{filter_text, changes_list_width}` | app | Phase 07: Load Diff; the side-by-side toggle; the tab's session state. |
| `every_view_of_a_diff_row_names_every_kind_of_row`, `the_diff_row_matcher_catches_the_shapes_it_claims`, `every_production_view_names_every_variant` | guards | Phase 07: `reads_enum_partially` over `UnifiedRow` and `SideBySideRow`. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 diff model | landed | `scripts/gate.sh` PASS | `qa-checklist`, `test-coverage-auditor` and `responsiveness-reviewer`, adjudicated by `qa-confirm`; confirmed findings fixed or recorded as residuals in `docs/systems/diff.md` |
| 02 engine, commits | landed 2026-09-18; changes query reworked onto `git diff-tree` 2026-10-03 (decision E) | `scripts/gate.sh` PASS, `git-floor` included | done over the reworked phase (2026-10-03), adjudicated by `qa-confirm`; confirmed findings fixed. C6 audit done (2026-10-03, adjudicated): F2-F7 fixed; F1 closed by the content-parity rework (landed 2026-10-03, `scripts/gate.sh` PASS with `git-floor`): R2.4 and R2.8 parity enforced under every algorithm and over real history. QA of the content rework (round 3, 2026-10-03): 21 raw, 16 confirmed by `qa-confirm`, S3 dismissed, S1/R2/G4 escalated and decided by the user; **phase 02 QA round 3 fixed** — every confirmed finding fixed or recorded (R2 above, for phase 08; G4 as issue #51), `scripts/gate.sh` PASS with `git-floor` |
| 03 engine, working tree | landed 2026-10-03; **phase 03 QA fixed** 2026-10-03 | `scripts/gate.sh` PASS, `git-floor` included | done (2026-10-03): 19 raw, 16 confirmed by `qa-confirm`, QC3 dismissed; every confirmed finding fixed test-first (`progress.md`); the `diff --no-index` exception accepted by the user (2026-10-03); a full read-verb roster guard is a candidate follow-up for the user |
| 04 worker lanes | landed 2026-10-03; **phase 04 QA fixed** 2026-10-03 | `scripts/gate.sh` PASS, `git-floor` included | done (2026-10-03): 17 raw, 15 confirmed plus escalations; the user decided freshness ("stamp what git reads"); every confirmed finding fixed test-first, Expand All's per-file outcomes deferred to phase 08 — `progress.md` |
| 05 detail pane | landed 2026-10-03; **phase 05 QA fixed** 2026-10-03 | `scripts/gate.sh` PASS, `git-floor` included | done (2026-10-03): 26 raw, 15 confirmed by `qa-confirm`, R3/G5 and one more dismissed; every confirmed finding fixed test-first and the user's six decisions applied (`progress.md`): dates in Fork's fixed English format, encodings through `encoding_rs` (user-approved) |
| 06 unified diff view | landed 2026-10-03; **phase 06 QA fixed** 2026-10-03 | `scripts/gate.sh` PASS, `git-floor` included | done (2026-10-03): 15 raw plus 1 found, 14 confirmed by `qa-confirm`, C4 dismissed; every confirmed finding fixed test-first or recorded for phase 07, and the user's four decisions applied (`progress.md`), `unicode-width` user-approved |
| 07 Changes tab and side-by-side | landed 2026-10-03, **QA due** | `scripts/gate.sh` PASS, `git-floor` included | due (the orchestrator runs it) |
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
- **`unicode-width` 0.2.2** (MIT OR Apache-2.0, `default-features = false`) is
  `cairn-ui`'s, user-approved 2026-10-03 ("terminal widths"), allowlisted in
  `crates/cairn-guards/tests/invariants.rs`.
- **The diff font is in the repository** (user-approved download, 2026-10-03):
  `crates/cairn-app/assets/fonts/IBMPlexMono-Regular.ttf` (173,052 bytes, SHA-256
  `7c6fbddca4b700be918f5f6183d9bd4464fa427fe435f0b480d77fe2bb8c5a43`) with
  `IBMPlexMono-LICENSE.txt` (SIL OFL 1.1, 4,456 bytes, SHA-256
  `7e6b2818edbd8f6a01ae80641cc8f16a51080d08fb4e532be3a0b6f74adb07da`), from
  `ibm-plex-mono.zip` of IBM/plex's release `@ibm/plex-mono@2.5.0`. Another weight or
  face is another download for the user to approve.
- The remote is `github.com/alexparlett/cairn`. Issues and pull requests go there;
  every pull request is merged by the user, never by a session.
