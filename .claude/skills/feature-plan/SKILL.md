---
name: feature-plan
description: Plan multi-session work as a feature packet under docs/work/<packet>/ — brainstormed WITH the user, split into phases that each end with orchestrated QA, plus one final fresh QA session, each phase runnable as a fresh session from a self-contained starter prompt. Work too big for one packet becomes a PROGRAM - per-feature intent docs under docs/design/ plus a work/<program>/ directory, with build packets planned later by their own runs of this skill.
user-invocable: true
disable-model-invocation: true
---

# /feature-plan

Produces a feature packet: a self-contained planning directory that lets a series
of fresh sessions implement a large feature without re-deriving context, with QA
built into the phase structure. Design decisions belong to the user; this skill's
job is to surface options and record decisions, not to make them.

**Packet or program?** One packet = one feature, a handful of phases. Work too big
for one packet is a PROGRAM: this skill then produces per-feature intent docs
under `docs/design/` (the program's design — product rules and program-level
acceptance bars) plus `docs/work/<program>/` (brainstorm, a roadmap of
build-packet briefs carrying the design nuance, state.md, progress.md — NO phase
docs). Each build packet gets its own run of this skill when it starts, writing
its own PRD and phases against then-current code. Build order lives in the
program's roadmap, never in the design spine.

## Step 0: Pre-flight

Scan memory and `docs/` for prior decisions touching this feature. List open work
(`docs/work/*/state.md`) so the new packet does not overlap work in flight.

## Step 1: Recon

Spawn parallel Explore agents over the affected areas of the repo (do not read
large docs into the orchestrator context; have agents return structured
summaries). REQUIRED whenever the feature touches a version-sensitive third-party
API: a research agent verifying current APIs against live docs, because training
data lags current releases. Facts that cannot be verified are recorded as OPEN
questions, never guessed.

**Save the evidence.** Every substantive recon or research report (market scans,
precedent studies, code audits) is committed IN FULL to
`docs/research/<slug>/<topic>.md` as an evidence record — never just summarized
into context and discarded. The brainstorm cites these files per decision; intent
docs and PRDs carry rationale pointers back to them. Evidence survives teardown
(layer contract: `docs/CLAUDE.md`, the promotion pipeline).

## Step 2: Brainstorm with the user

Present the design space as concrete options with a recommendation each:
mechanics, scope, architecture consequences, what is explicitly OUT of scope.
Iterate until the user locks the decisions. Record every locked decision and
every rejected alternative (with the why) in `brainstorm.md`.

## Step 3: Write the PRD, then the packet

FIRST, at decision lock, write `docs/prd/<packet>.md` (layer contract:
`docs/CLAUDE.md`): the packet's spec in spec voice — requirements, product rules,
and the ONE authoritative copy of the acceptance criteria — with a
`status: in-flight` header. Every PRD is ONE packet's spec; a program's
cross-packet design goes in `docs/design/` intent docs instead, and the packet
PRD points at it for the frame. It is authoritative while the packet is in
flight; teardown stamps it and it is never edited again. The design spine gets
only a pointer. Where the packet changes the design itself, rewrite the affected
sections of the feature's doc in `docs/design/` (creating the doc if the feature
has none) so it reads as one account of the end state — edit what the change
contradicts rather than appending after it; no "amended by this packet", no "not
yet built", no dates, phases or lock ids (`docs/CLAUDE.md`, guarded by
`design_docs_carry_no_point_in_time_state`). Status stays in the PRD
and the work dir. The as-built reference `docs/systems/<system>.md` is NOT written
now: phases create and update it as behavior actually lands.

**User-mode branch rule:** create a runtime-owned phase branch from
`feature/<packet-slug>` before editing, then raise a templated pull request back
into that integration branch; never merge it. **Packet-mode exception:** only an
explicitly declared packet coordinator and the phase agents it dispatches may
commit phase work directly to `feature/<packet-slug>`.

Then create `docs/work/<packet-slug>/` containing:

- `README.md` — what this packet builds, in three sentences, plus a phase index.
- `brainstorm.md` — locked decisions and rejected alternatives from Step 2.
- `implementation-plan.md` — the technical plan; carries the packet's ONE copy of
  the review-dispatch rules (which reviewers each phase must run).
- `state.md` — the cross-session cheat sheet: locked decisions, new
  modules/interfaces introduced so far, validation status per phase. Every
  session updates it before ending.
- `progress.md` — running log, newest first.
- `qa-checklist.md` — packet-specific acceptance criteria beyond the repo-wide
  gate (points at the PRD's criteria, never copies them).
- `phase-XX-<slug>.md` — one per implementation phase, each carrying a `## QA
  brief` section its phase-end orchestrated QA runs.
- `phase-XX-qa.md` — ONE file, the packet's LAST phase under its OWN number (one
  past the final implementation phase): the merge-bar QA, run as its OWN fresh
  session.

**Phase sizing:** one phase = one logical slice, 2 to 4 deliverables. When in
doubt, split. **Ordering:** foundations first; every implementation phase ends by
orchestrating its own QA (fresh reviewer subagents, the `qa-confirm` agent
adjudicating findings, dismissals logged in `progress.md` — contract:
`docs/qa-gate.md` "Packet phase QA"); the packet's final QA phase is a separate
fresh session; each phase is a separate session. **Branch flow (root CLAUDE.md):**
the packet gets ONE long-lived integration branch, `feature/<packet-slug>`. Every
phase prompt names its mode and branch authority. User mode is the default when
the user starts one phase: create a session-owned task branch from integration
before editing, then after QA commit, push, and raise a templated PR back into
integration without merging it. Packet mode applies only when
`/orchestrate-packet` explicitly drives the whole packet: phases commit directly
onto integration with no per-phase PR; CI runs the gate on every push to
`feature/**`. Nothing reaches `main` until final QA passes. In user-mode final
QA, its phase PR must merge into integration before a resumed session raises the
packet PR to `main`; packet mode may raise that PR directly after teardown. Every
PR is left for the USER to merge after human review. Planning docs (this skill's
output and plan revisions) land on `main` via their own PR. The final QA phase
offers packet teardown per `docs/CLAUDE.md`: stamp the PRD `shipped`, verify
`docs/systems/<system>.md` is current against the as-built system, graduate
invariants into CLAUDE.md with their enforcement twins, file leftovers with the
`file-issue` skill, then delete the work directory; git history is the archive.
A program tears down with its last packet: its intent docs are stamped and
pointed at `docs/systems/`, and its work directory is deleted.

## Step 3½: Coverage review before closing

Before ending any planning pass (packet or program), dispatch a FRESH agent to
audit both directions: every locked decision reached its authoritative home
(intent doc, PRD, spine, roadmap — not just the brainstorm), and every doc claim
matches reality in TENSE (nothing unbuilt stated as built; the as-built
`systems/` docs describe only current code). Fix every finding before closing;
the audit prompt enumerates the session's decisions explicitly.

## Step 4: Starter prompts

Each phase file opens with a self-contained starter prompt a fresh session can
run:

```
STEP 0  Pre-flight: read docs/work/<packet>/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/<packet-slug> before editing.
        For phase 01, first create and push integration from the packet's
        declared base if it does not exist. Later phases verify all earlier
        phase work is present at the integration tip. Direct integration work
        requires an orchestrator prompt that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over <named paths>; do not read the
        planning docs beyond state.md and this phase file directly.
STEP 2  Implement: <deliverables, exhaustively listed>. Invariants in play:
        <the specific CLAUDE.md invariants this phase can break>.
        Out of scope: <explicit list>.
STEP 3  Validate: <commands>. Then orchestrate this phase's QA in this
        session: run /qa over the phase diff (reviewers from
        implementation-plan.md, spawned fresh) plus the qa-checklist.md
        items this phase covers and this file's QA brief. Adjudication goes
        to the qa-confirm agent (fresh), never this session inline; log
        dismissed findings with reasons in progress.md; fix confirmed
        findings in focused fixes; disputed findings go to the user.
STEP 4  Acceptance: <criteria from qa-checklist.md this phase satisfies>.
STEP 5  Update state.md and progress.md; save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase
        branch, and raise a PR using the repository template into
        feature/<packet-slug>; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration
        branch with no per-phase PR. NEVER merge or PR to main — teardown
        raises that one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when <the decisions this phase must not
make alone>; otherwise do not stop for permission.
```

The final QA phase is the packet's last numbered phase (`phase-<N+1>-qa.md` after
implementation phase N), same shape, with STEP 2 replaced by: run `/qa` over the
whole packet diff, run the ENTIRE `qa-checklist.md`, verify every PRD acceptance
criterion against its pinned test, AUDIT the per-phase dismissal log in
`progress.md` (a dismissal whose reason no longer holds is a finding), and fix
confirmed findings in focused commits.

It also KEEPS acceptance at STEP 4, ahead of the state update and the teardown
offer — the one place the order above is deliberately not applied. Acceptance is
the merge bar there, and everything after it is conditional on passing: teardown
is gated on it. In user mode, teardown lands through the final phase PR into
integration and the session stops; after the user merges it, a resumed session
verifies integration and raises the packet PR to `main`. Packet mode may raise
the packet PR immediately after teardown on integration.
