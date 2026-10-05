# Brainstorm — refs-and-status

Locked decisions and rejected alternatives. Historical record: never
retro-edited. Every decision below was locked by the user on 2026-10-05, after
four evidence records under `docs/research/refs-and-status/` were presented with
the mechanism, cost and evidence behind each option.

## Locked 2026-10-05

**L1. Status comes from `git status --porcelain=v2 -z`, run as a read (closes
program O2).** A new `reads::status`, under a read's environment, cancelled by
its epoch. Exact by construction: it honours `core.fsmonitor`, the untracked
cache, sparse checkout, `status.renames` and submodule settings because it is
git. Cheaper than gix in every measured case but one on rust-lang/rust (clean,
untracked on: 27 ms against 35; 1,000 modified: 26 against 38; 10,000 untracked:
34 against 52); the exception is a tree whose every file's stat changed, 736 ms
against 135, because a read never writes the refreshed index back. Porcelain v2
exists since git 2.11, inside the 2.30 floor.
Evidence: `status-agreement-spike.md`; the index left byte-identical by this
exact read is already pinned in `crates/cairn-git/src/ops/authority.rs`
(`code-audit.md` section 5).

Rejected: **gix status.** 12 of the spike's 38 fixtures differ from git — 40
staged renames with edits shown as 80 adds and deletes (the unsquared rename
limit again), `status.renames` and `status.renameLimit` read from the `merge`
section, a conflicted path without a stage-2 entry also listed as untracked, a
lying fsmonitor hook ignored, cone-mode sparse checkout dropping an untracked
file, a sparse index erroring — and, in the API study's probe on one history, a
staged deletion nobody made under a split index beside a sparse one
(`gix-refs-and-status-api.md`, headline 6). gix also starts the clean filter and
`git-lfs filter-process` itself, from Cairn's process with its inherited
environment, outside `process/` — no `ALWAYS` table, no registry, no kill.
Program memory: a divergence from git is a critical bug, not a gap to file.
Rejected: **a hybrid** (gix index-to-worktree, git HEAD-to-index) — it still
diverges on several fixtures and doubles the code.

**L2. Untracked files are listed per file; ignored files are not listed.** Fork
runs `--untracked-files=all`, and its list shows files, not collapsed
directories, so Cairn does the same — except where the user set
`status.showUntrackedFiles=no`, which is honoured. How that setting is read so it
is git's reading is the status phase's to decide (the `reads::fetch_settings`
precedent; a second `git config` read would be a new porcelain read and needs the
user). `-uall`'s cost at scale on rust-lang/rust was not measured by the spike
(only `normal`), so the phase measures it. Ignored files: Fork hides them by
default and `--ignored` walks build directories; a "show ignored" toggle is
filed. Evidence: `fork-refs-and-status-ui.md` section 6–7.

**L3. Refs come from gix, with four parity rules.** gix agrees with
`git for-each-ref` on loose and packed refs, ordering and invalid names, at
1.7 ms for 10,501 refs unpeeled. The rules, each to be pinned by a test:
symbolic refs are never peeled (`.peeled()` renames `origin/HEAD` into a second
`origin/main`); dangling symbolic refs are hidden as git hides them; the stash
reflog is read oldest-first and reversed (the newest-first reader has a 4 KiB
buffer and stops at a long message); an upstream of `remote = .` resolves as git
resolves it (gix answers `None`). `GIT_NAMESPACE`: gix honours it from Cairn's
own environment but Cairn never hands it to `git`, so Cairn opens without it, and
honouring it is filed. Evidence: `gix-refs-and-status-api.md`.
Rejected: `git for-each-ref` as a read — no measured disagreement once the rules
hold, and D1 says a git read is argued from one.

**L4. A reftable repository is refused at open, with its reason.** gix 0.87 opens
one without complaint and then fails reading `HEAD`, so today's application
already breaks on one; refusing at open, beside dubious ownership, is the honest
answer. Reftable support is filed.

**L5. The history walks every ref; stashes are rows of their own.** Seeds: every
local branch, remote-tracking ref, tag identifying a commit, and `HEAD` — Fork's
All Commits, its default (`fork-refs-and-status-ui.md` section 2).
`HistoryRequest::from_commits` exists; the cost of seeding from thousands of tips
was never measured, so it joins the bar. A stash cannot be a tip: its index and
untracked commits would become rows, and hiding them with `with_hidden` would
hide their ancestors, which are real history. So stashes are read from the
reflog (a small list) and merged into the stream by commit time as a row kind of
their own, drawn on a short lane with one edge to the commit they were made on —
Fork's rendering (section 3). Selecting one shows `stash^1..stash`, what
`git stash show` lists. A new `RowContent` variant; the compiler and the
exhaustive-read guard name its four readers.

**L6. No working-tree row in the graph.** Fork draws none (Tracker #308, open);
"Local Changes (N)" in the sidebar is the entry point. The history-graph design
and the root `CLAUDE.md` anticipated such a row; both are rewritten.
Rejected: a Sourcetree-style row at the top, which would need a non-`Oid` row in
the lane assigner and enter `HistoryList::index_of`'s full-scan fallback.

**L7. Labels on rows are Fork's.** Outlined chips between graph and subject,
lane-tinted; tags indigo with a tag glyph; current branch ✓; `HEAD` row bold;
compact labels (a remote-tracking ref at its local branch's commit shrinks to a
glyph before the branch's chip); no overflow handling — clipped at the column
edge, as Fork. Deviation: a generic remote glyph, drawn as vector shapes, in
place of Fork's forge icon until packet 6 identifies forges; no new font.
Deferred to issues: greying commits not on the current branch (reachability from
`HEAD` tracked through the walk, with the clock-skew window problem the lanes
already have) and Fork's push and pull dots (per-commit ahead/behind sets).

**L8. The sidebar is Fork's, read only.** Local Changes (N), All Commits, a
filter box, Branches (folders by `/`, ✓, ↑↓), Remotes, Tags, Stashes; Worktrees
is packet 8's and Submodules later. Every list virtualized (the `ScrollView`
roster is empty). Pressing a ref selects its commit and scrolls to it; a commit
not yet loaded is found by paging the held walk forward until it arrives,
cancellable through the epoch, saying "Finding <ref>…" — which mostly answers #3.
Double-click (checkout) is packet 7's.

**L9. Ahead/behind for every local branch with an upstream, shown on the
sidebar and in the title bar.** Two `rev_walk().with_hidden()` counts per branch,
exact by construction, computed on a worker after the refs answer as a separate
cancellable answer. The title bar carries Fork's central box: repository (`*` when
dirty), current branch, ↓behind ↑ahead; a detached `HEAD` shows its short id.

**L10. Local Changes is built here, read only.** Unstaged above Staged, flat
lists (the tree view stays #36), Fork's badges, renames shown with their source;
choosing a path opens its diff through `diff-engine`'s already-built
`working_tree_patch` (wired but dead code today); a conflicted path draws a
notice. Packet 5 adds stage, discard and the commit box to the same view.
Rejected: only the count here, the view in packet 5 — the read surface would
ship without its consumer, and the working-tree diff query would stay dead.

**L11. Refresh is Fork's: on focus, after Cairn's own operations, and on the
Refresh chord; no file-system watching.** Focus is
`Platform::get().is_app_focused` at the pinned Freya rev (`caa46f8`), verified
in the vendored `freya-winit` renderer and settable in `freya-testing`. Each
refresh re-reads refs (milliseconds); a moved tip or a changed stash list
reopens the history, which absorbs fetch's `reload_if` tip comparison. Fork's
main complaint, `index.lock` held on every focus, does not apply: Cairn's reads
take no lock. Rejected: watching with the `notify` crate — a dependency decision,
and inotify limits bite on large trees; filed. Not done: refreshing the index to
cure the every-stat-changed case, which is a write for packet 5's local lane.

**L12. The measured bar** (program L6), on `~/Development/bench/rust`, warm,
median of seven: status clean ≤ 100 ms; status with 1,000 modified and 10,000
untracked files, on a scratch clone, ≤ 250 ms; refs with every ahead/behind
≤ 100 ms; the first page of history from every ref recorded beside `HEAD`'s;
the every-stat-changed case recorded, not barred; and `window_check` frames
under 16.7 ms while it all lands. The user accepted "first page within
history-graph's A7 bar"; A7 has no number, so the PRD writes it as ≤ 200 ms (C11),
which the user may revise at the merge bar.

## Out of scope, locked with the above

Acting on any ref (7), staging (5), worktrees and submodules sections, Fork's
ref filter and hiding (#2), the tree view (#36), greying and push/pull dots (L7),
listing ignored files (L2), reftable (L4), `GIT_NAMESPACE` (L3), file-system
watching and index refresh (L11). Each is filed at teardown.

## Settled in the coverage audit, 2026-10-05

Mechanism choices made within the locks above while closing the planning pass's
coverage audit, recorded for the user to see in the planning PR:

- **A ref's find is history-lane work** (within L8): it pages the same held walk a
  scroll pages, so a scroll and a find supersede each other; no separate find
  lane.
- **A stash's base commit seeds the walk** (within L5), so its edge lands even
  when the branch it was made on is gone; a stash dated older than its base by
  clock skew is drawn directly above it.
- **A checkout that moves no ref reopens the history** (within L11): the
  snapshot comparison includes `HEAD`'s state, or ✓ and the bold row go stale.
- **Status and ahead/behind run on a third worker thread** (within D3's "a few
  routed threads"), so a slow status or a long divergence queues neither a page
  nor a diff; refs stay on the history thread.
- **A reopen frees the replaced rows off the UI thread** (#52), since refresh on
  focus makes that free frequent.
- **`stash.showIncludeUntracked` is honoured** when a stash is selected (within
  L5's "what `git stash show` lists").

## Revised after follow-up research, 2026-10-05

The user asked for evidence on two of the audit-settled choices above before
merging the plan.

- **A stash's base no longer seeds the walk.** Fork draws a stash only on a
  commit its ref walk already reached; a stash whose base no ref reaches has no
  row and is listed in the sidebar alone (vendor statements in TrackerWin #1050
  and Tracker #1283, and a screen recording in TrackerWin #1622 —
  `docs/research/refs-and-status/fork-unreachable-stash-base.md`). Walking the
  base would have matched `git log --all` and VS Code's Git Graph, but deviated
  from Fork without a reason; the user chose Fork's rule. Supersedes the "A
  stash's base commit seeds the walk" bullet above.
- **A deep find's memory is bounded, and how is the user's decision in phase
  06.** Paging the held walk is fast — about 120,000 rows a second, the oldest
  commit of rust-lang/rust in about 2.4 s, seeded from every ref as cheaply as
  from `HEAD` — but every row passed is kept (#4), about 1.4 GiB at that depth
  (`docs/research/refs-and-status/deep-find-measured.md`). The find stays as
  designed; its retained memory gets a bound before it is built.

## Locked 2026-10-05, after the deep-find measurement

**L13. Compact rows, no cap, and the deep find as designed.** The user asked for
a deep find designed not to cost memory, after Fork was researched.

Evidence: paging the held walk is fast (about 2.4 s to the oldest commit of
rust-lang/rust, cancel within about 10 µs) but a retained row costs 4.3-6.3 KB
there, 89% of it the edge segments crossing it — 105-160 open per row — so the
oldest commit holds about 1.4 GiB (`deep-find-measured.md`; the systems doc's
660 B a row came from repositories of at most 2,896 commits). Fork caps its list
at its newest 50,000 (Windows) or 100,000 (Mac) commits, loads it whole, and does
nothing for a pressed ref past it; it adopted the cap after a Chromium history
cost gigabytes, and its graph layout "calculates only the visible area"
(`fork-deep-history.md`). GitLens's Commit Graph pages a jump in, as Cairn plans.

Locked: a row keeps its id, parents, text, lane and only the lane changes at it;
the edges a drawn row crosses are derived from periodic lane snapshots, for the
drawn rows alone (Fork's technique); no cap on the history; the find pages as
planned and retains only compact rows. Estimated about 60 MB for all of
rust-lang/rust (an estimate from the measured breakdown, not a measurement); the
bar is 128 MiB (PRD C15). Compact rows are a phase of their own, 03, before stash
rows and labels are added to a row; later phases are renumbered 04-09.

Rejected: **Fork's cap** — a cap breaks "readable at scale" past it, and with
today's rows 100,000 commits still cost about 575 MB, so it bounds nothing
without compact rows. **A resident window with a re-walk beyond it** — constant
memory, but a not-resident row state and seconds-long re-walks per miss, which
only a history of millions of commits needs; filed with #4. Supersedes the
"Revised after follow-up research" bullet on bounding a deep find.

## Corrected in the revision's audit, 2026-10-05

L13's estimate of "about 60 MB" for all of rust-lang/rust was wrong. The parts of
a row that are not edges — the 216 B row struct with the outer row vector's
growth slack (108 MiB), text (27 MiB) and parent ids (14 MiB) — already come to
about 150 MiB (`deep-find-measured.md`), so compact rows land near 160 MiB:
about 9x less than 1.4 GiB, not 25x. C15's bar is set at 192 MiB accordingly;
shrinking the row struct itself is not in scope. The user was told. Also: L13's
"cancel within about 10 µs" is the maximum deep in a find (0.6-17 µs overall),
and "about 575 MB" is 575 MiB.
