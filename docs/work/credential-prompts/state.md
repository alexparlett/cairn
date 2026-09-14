# State — credential-prompts

The cross-session cheat sheet. Every session updates this before ending.

**Status: planned. No phase has started. No code exists for this packet.**

## Locked decisions

L1-L8 in `brainstorm.md`; the design-level frame is D1 and D2 in
`docs/design/cairn.md`. The three that most constrain implementation:

- Cairn implements no authentication and stores no credential (L1).
- The askpass helper is a separate binary because git's contract is a process:
  prompt on `argv`, secret on stdout (L2).
- A user whose credential helper or ssh-agent already works must see no new
  dialog (L7) — and B4 is the regression test that protects it.

## Open questions

O1-O5 in `brainstorm.md`. O4 (askpass vs. credential-helper precedence) is the
one that can invalidate L7, so phase 03 must settle it with a test rather than a
reading.

## New modules and interfaces introduced so far

None yet. As phases land, record here: the type or function, its crate, and the
one-line contract.

| Symbol | Crate | Contract |
| --- | --- | --- |
| _(none)_ | | |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 git backend | not started | — | — |
| 02 askpass helper | not started | — | — |
| 03 fetch end to end | not started | — | — |
| 04 QA | not started | — | — |

## Environment notes

- Development machine has git 2.55.0. The evidence record was gathered against
  it; O1 sets the actual required floor, which will be lower.
- `freya` 0.5-rc and `gix` 0.87.1 are pre-1.0. Verify APIs against the vendored
  source under `~/.cargo/registry/src/`, never from memory.
- `scripts/gate.sh` is the bar. Never an ad-hoc `&&` chain, never piped through
  `tail`.
- Commit explicit paths, never `git add -A`.
- **Test fixtures must generate their credentials, never contain them.** This
  packet is the one most likely to commit a secret by accident.
- A new crate, dependency or invariant needs its row in
  `crates/cairn-guards/tests/invariants.rs` in the same commit, or the gate fails.
- The repository has no remote yet. If one still does not exist when a phase ends,
  say so in the final response instead of pretending a PR was raised.
