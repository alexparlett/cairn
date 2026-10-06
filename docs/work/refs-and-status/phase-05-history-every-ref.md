# Phase 05 — The history from every ref, labelled, with stash rows

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        Verify phases 01-04 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/history*
        (HistoryRequest, HistorySession, walk_tips), crates/cairn-model/src/
        (CommitSummary, HistoryRow, RowContent, RowId, GraphRow, LaneAssigner),
        the RowContent readers the compiler will name (crates/cairn-app/src/window.rs,
        selection.rs, crates/cairn-ui's HistoryRow::id and HistoryList),
        crates/cairn-guards/tests/invariants.rs
        (every_view_of_a_row_names_every_kind_of_row), docs/systems/history-graph.md,
        docs/prd/refs-and-status.md (R4, C6, C11) and
        docs/research/refs-and-status/code-audit.md sections 1-3. Re-read the
        vendored gix revwalk source for what you call. Do not read the other
        planning docs directly.
STEP 2  Implement.
        1. cairn-git: a history request seeded from phase 01's snapshot — every
           local branch, remote-tracking ref, commit-identifying tag and HEAD,
           nothing about a stash, tags on trees filtered before walk_tips —
           taken from the same snapshot the labels come from.
        2. cairn-model and cairn-git: a stash row kind (a new RowContent
           variant), merged into the stream by commit time on the history
           thread but never after its base (R4.2's clock-skew rule), and only
           for a stash whose base the walk reaches (Fork's rule, R4.2): settle
           how that is known before the row's date comes up — a reachability
           check per stash from the seeds, or another mechanism — and measure
           it on the bench repository against C11; laid out
           with its first parent as its only parent; every row
           carries its labels (R4.3) from the snapshot. The assigner must not
           await a stash's index or untracked commit.
        3. Every RowContent reader the compiler names handles the stash variant
           by naming it — a stash is selectable, so selection.rs and the window
           treat it as R6.2 will need (its pair for the changes query is
           stash^1..stash) — never by a wildcard.
        4. Tests for C6: the walked commit set against
           `git rev-list --branches --remotes --tags HEAD` (a stash whose
           branch was deleted among the fixtures, with no row), labels against
           `git log --decorate=full`, stash rows from stashes made with and
           without --include-untracked; assigner tests for a stash row's lane
           and edge. Extend the bench reporter for C11's first page from every
           ref, recorded beside HEAD's.

        Invariants in play: RowContent read by naming every variant (the guard
        and its residuals in the root CLAUDE.md); cairn-model plain data with
        tests; the crate seal; no history-sized work added to a render path
        (HistoryList::index_of's fallback must stay unreachable — stash rows
        append, they never arrive above a row).

        Out of scope: drawing chips or stash rows (07), the worker's refresh and
        reopen (06), the sidebar's find (08). The application may keep walking
        from HEAD until phase 06 wires the snapshot, if that keeps the gate
        green; say which in state.md.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 05, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C6 passes; C11's first-page numbers are recorded.
STEP 5  Update state.md and progress.md; docs/systems/history-graph.md (seeds,
        labels, stash rows, the assigner's first non-commit row). Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if knowing whether a stash's base is
reached costs more than C11's first-page bar allows; if a stash row cannot be laid out without
keying the assigner by something other than Oid; if the first page from every ref
on the bench repository is far past C11's 200 ms; or if a reader of RowContent
seems to need a wildcard. Otherwise do not stop for permission.
```

## QA brief

- The commit-set comparison must cover the whole walk, not one page; a fixture
  small enough to walk to the end makes that cheap.
- A stash made with `--include-untracked` has three parents. Check neither the
  index commit nor the untracked commit appears, and that a fixture where a
  branch DOES reach the index commit still shows it (as a commit, once).
- Two stashes on one commit: two rows, two lanes, both joined to that commit.
- A stash whose branch was deleted: no row, and none of its base's otherwise
  unreachable commits in the graph. A stash whose base is reached only deep in
  history: its row still appears, at its date or directly above its base. A
  stash whose commit date is older than its base's (set `GIT_COMMITTER_DATE`):
  drawn directly above it.
- A stash older than the first page's oldest commit must still appear when the
  page reaching its date arrives — check the merge is by date across pages, not
  only within the first.
- Grep every match over `RowContent` added or touched; a `_ =>` arm the guard
  missed is a finding.
