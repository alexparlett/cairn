<!-- Local conventions for docs/ only; repo-wide rules live in the root CLAUDE.md.
Loaded on demand when files here are opened. -->

# docs/

Reference material, not auto-loaded. Agents read these on demand; nothing here is
in context by default, so a doc only helps if it is findable from the root
CLAUDE.md Pointers, this file, or a work directory's state.md.

## The tenses

Every directory answers ONE reader question; the path tells you the tense:

| Path | Question | Tense | Class |
| --- | --- | --- | --- |
| `design/` | What are we building and why? | intent | living |
| `prd/` | What exactly did we commit to for X? | commitment | living while `status: in-flight`; frozen once stamped |
| `systems/` | How does X actually work now? | truth (as-built) | living |
| `work/` | What is in flight right now? | working state | historical, deleted at teardown |
| `backlog/` | What did we defer, with NO remote to file against? | deferred | living while `status: open`; empty while the repo has a remote |
| `research/<slug>/` | What did we find when we decided? | evidence | historical, never deleted |
| `qa-gate.md` | How is work reviewed? | process contract | living |

- **`design/`** — the spine, `cairn.md` (pillars, the milestone, the map of
  feature docs, the decision index, open questions — summaries and pointers
  only), and one intent doc per feature or doctrine (`engine.md`, `diff.md`, ...).
  A program's design is one or more per-feature intent docs. When the spine
  starts carrying a feature's design, that design moves into its own doc.

  **Design describes the end state, as a cohesive whole.** Each doc reads as one
  account of how its feature works when finished, with its reasons and rejected
  alternatives where they apply — never as a log of decisions and amendments.
  When a packet changes the design, rewrite the sections it touches so the doc
  reads as if it had always been so: edit the sentences the change contradicts,
  move content to where it belongs, and split a doc that has outgrown its
  subject. Never add a paragraph after text it corrects, a new numbered decision
  entry, an "amended by packet X", "decided, not yet built", "as built by",
  "until phase N lands", or a note on what the doc used to say. The `D1`-`D9`
  ids are an index in the spine for citations, not a structure to append to.
  No dates, no phase numbers, no brainstorm or program lock ids (`L3`, `O1`,
  `Q2`), no acceptance-criterion ids (`A7`, `C15`), no struck-through answered
  items. What is built goes in `systems/`, what a packet commits to in `prd/`,
  and what is in flight in `work/`; design may POINT at each (`Spec:
  docs/prd/x.md R3`, `As built: docs/systems/x.md`), and a pointer is the only
  way it names a packet. Decision ids (`D1`), issue numbers and dependency
  versions behind evidence are fine. Twin:
  `design_docs_carry_no_point_in_time_state` in
  `crates/cairn-guards/tests/invariants.rs`, with matcher self-test
  `the_point_in_time_matcher_catches_the_shapes_it_claims`. Residual review
  obligation: the matcher reads a finite phrase list, so a sentence that dates
  itself in other words ("for now", "the first version of this doc") is the
  reviewer's to catch, and so is cohesion — a doc that reads as an original plus
  appended corrections passes every token check.
- **`prd/`** — feature specs, ONE packet each, written at that packet's decision
  lock by `/feature-plan`: requirements, product rules, and the ONE authoritative
  copy of the packet's acceptance criteria (work-dir qa-checklists point here,
  never copy). Authoritative only while its packet is in flight; teardown stamps
  it `shipped` (pointing at `systems/` for current truth), `superseded`, or
  `abandoned`. Every PRD carries a `status:` header from {in-flight, shipped,
  superseded, abandoned}.
- **`systems/`** — how each system as built actually works, created as a packet's
  phases land behavior and kept current in the SAME change that invalidates or
  refines it.
- **`work/`** — packet and program directories created by `/feature-plan`:
  brainstorm, state.md, progress.md, phase docs (packets), roadmap (programs).
  Open work is listed by `docs/work/*/state.md`.
- **`research/<slug>/`** — recon and audit reports saved IN FULL as evidence,
  keyed by the work slug that commissioned them. Evidence outlives its work
  directory (living docs cite it as the "why" behind their rules), so it is never
  deleted at teardown and never retro-edited.

**The promotion pipeline** — how findings become truth: `research/` (evidence) →
the work dir's `brainstorm.md` (decisions and rejections, each citing its
evidence) → `design/` and `prd/` (living conclusions, carrying rationale pointers
back to the evidence) → `systems/` (as-built, written only as code lands) →
teardown stamps the living docs and deletes the work dir. Data moves FORWARD only
at a decision or a landing; nothing authoritative ever cites only a conversation.

## Living docs vs historical records

- **Living docs** must stay true. Whoever invalidates one updates it in the SAME
  change; a stale living doc is a bug. They follow the anchor rule: stable paths,
  exported symbols, pinned tests; never counts or line numbers that rot.
- **Historical records** (work-dir brainstorms, progress logs, stamped PRDs) state
  what was decided or true at the time. Never retro-edit them to match later
  reality — including path moves: a frozen record's paths are era artifacts.
  Correct course in a new entry, never in the old one.

## Packet teardown graduates, git remembers

When a packet tears down, its durable content graduates before the directory is
deleted: the PRD is stamped `shipped` (or `superseded`/`abandoned`, with a pointer
to what replaced it); `systems/<system>.md` is verified current against the
as-built system and the spine's pointers updated; new invariants go into the
relevant CLAUDE.md (with their enforcement twin); unfinished items are FILED with
the `file-issue` skill — GitHub issues while a remote exists, `docs/backlog/` only
as its fallback. Everything else is deliberately discarded; git history is the
archive, so deletion loses nothing. A program tears down with its last packet:
its intent docs graduate the same way, and its work directory is deleted.

## When code and a doc disagree

Re-verify against the code first; docs rot faster than code. Then either fix the
doc (stale) or file the bug with `/file-issue` (code diverged from locked design).
Never leave the disagreement standing silently.

If you find a stale passage you cannot fix in this change, mark it in place:

```
TRAP: <what is wrong and what to trust instead>
```

so the next reader does not act on it. Removing a TRAP requires actually fixing
the passage.
