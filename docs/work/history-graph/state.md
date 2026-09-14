# State — history-graph

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 01 landed on `feature/history-graph`. The lane assigner exists
in `cairn-model`; nothing reads a repository yet.**

## Locked decisions

L1-L10 in `brainstorm.md`; the design-level frame is D3 and D4 in
`docs/design/cairn.md`. The three that most constrain implementation:

- Lanes are computed in `cairn-git` and travel as `cairn-model` values (L1).
- The assigner must be correct when a parent arrives before its child (L3) —
  this is normal, not corruption. See the evidence record.
- The total assigner is the floor, not an option (L9), and R1.2's stability
  covers lane INDICES only, so edges may repaint inside the loaded window (L10).

## Open questions

O2, O3 and O4 in `brainstorm.md`. **O1 is resolved**: L9 settled it before phase
01 started — a bounded reordering window can always be exceeded by larger skew,
so the total assigner is the floor rather than one of two options. Phase 01 built
that total assigner; the window refinement is deferred and unbuilt. O2 and O3 are
measurements; O4 is now a visual question rather than a structural one, thanks to
L10.

### How phase 01 answered R1.4

R1.4 requires a placement strategy and a justification for it. The assigner
places a commit that has no lane reserved for it in the leftmost free lane, and
when a child for it arrives later it draws the joining line *upward*, through a
lane that was free on every row in between, flagging every segment of that line
`out_of_order`. This is the evidence record's candidate 1 plus the rendering rule
it calls for: the line is complete rather than jumping, and the flag is what lets
phase 04 mark a link that runs backwards on screen instead of drawing time in
reverse without comment.

## New modules and interfaces introduced so far

None yet. As phases land, record here: the type or function, its crate, and the
one-line contract — so a later phase does not re-derive it from source.

| Symbol | Crate | Contract |
| --- | --- | --- |
| `Lane` | `cairn-model` | A vertical track in the graph, numbered from the left. `Lane::index()` is final once a row is emitted. |
| `EdgeKind` | `cairn-model` | Whether a segment passes a row by (`Passing`), ends at its commit (`IntoCommit`) or starts there (`OutOfCommit`). The whole geometric vocabulary; genealogy is not in it. |
| `EdgeSegment` | `cairn-model` | One piece of a connecting line clipped to one row: `from`/`to` lanes, `kind`, and `out_of_order` for a line joining a commit to a parent drawn above it. |
| `GraphRow` | `cairn-model` | One commit's line: `id`, `lane`, and every segment crossing the row. Self-contained, so drawing row N needs only row N. |
| `LaneAssigner` | `cairn-model` | Lays commits out in lanes from `(id, parent_ids)` in walk order. Pure: no gix, no I/O, no clock. |
| `LaneAssigner::push` | `cairn-model` | Lay out one commit. Returns nothing: it may add segments to rows already emitted, so rows are read back, not collected. |
| `LaneAssigner::rows` / `into_rows` | `cairn-model` | The rows so far. Lane numbers are final; edge lists may still grow. |
| `LaneAssigner::assign_all` | `cairn-model` | Lay out a whole walk in one call. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 lane assignment | landed | `scripts/gate.sh` green | `/qa` run, fresh reviewers, findings closed |
| 02 history query | not started | — | — |
| 03 worker boundary | not started | — | — |
| 04 graph view | not started | — | — |

Phase 03 additionally owes design notes here saying which parts of the worker
interface exist for fetch (`docs/prd/credential-prompts.md` R4) rather than for
the graph. Phase 04 owes R5: opening a repository from a command-line argument,
and nothing more than that.
| 05 QA | not started | — | — |

## Environment notes

- `freya` 0.5-rc and `gix` 0.87.1 are both pre-1.0. Verify every API against the
  vendored source under `~/.cargo/registry/src/`, never from memory. Freya 0.5
  uses a builder API; anything you remember from `rsx!` examples is the old one.
- `scripts/gate.sh` is the bar. Never an ad-hoc `&&` chain, never piped through
  `tail` — that masks the exit code.
- Commit explicit paths, never `git add -A`.
- A new crate, dependency or invariant needs its row in
  `crates/cairn-guards/tests/invariants.rs` in the same commit, or the gate fails.
- The repository has no remote yet. If one still does not exist when a phase ends,
  say so in the final response instead of pretending a PR was raised.
- Work in a linked worktree (`.claude/worktrees/<packet>`). A linked worktree's
  git dir is `.git/worktrees/<name>`, not `.git` — phase 01 had to fix a
  `cairn-git` test that assumed otherwise. Assert that a path IS a git directory,
  never what it is called.
