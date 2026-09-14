# State — history-graph

The cross-session cheat sheet. Every session updates this before ending.

**Status: planned. No phase has started. No code exists for this packet.**

## Locked decisions

L1-L10 in `brainstorm.md`; the design-level frame is D3 and D4 in
`docs/design/cairn.md`. The three that most constrain implementation:

- Lanes are computed in `cairn-git` and travel as `cairn-model` values (L1).
- The assigner must be correct when a parent arrives before its child (L3) —
  this is normal, not corruption. See the evidence record.
- The total assigner is the floor, not an option (L9), and R1.2's stability
  covers lane INDICES only, so edges may repaint inside the loaded window (L10).

## Open questions

O2, O3 and O4 in `brainstorm.md`. O1 was closed by L9 — it turned out not to be a
choice. O2 and O3 are measurements; O4 is now a visual question rather than a
structural one, thanks to L10.

## New modules and interfaces introduced so far

None yet. As phases land, record here: the type or function, its crate, and the
one-line contract — so a later phase does not re-derive it from source.

| Symbol | Crate | Contract |
| --- | --- | --- |
| _(none)_ | | |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 lane assignment | not started | — | — |
| 02 history query | not started | — | — |
| 03 worker boundary | not started | — | — |
| 04 graph view | not started | — | — |
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
