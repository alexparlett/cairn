---
name: orchestrate-packet
description: Drive a whole feature packet end to end from one session — set up the integration worktree, dispatch each phase to an independent fresh agent, verify their claims, batch what needs the user, and hand back a PR. Trigger on "orchestrate <packet>", "run the packet", "build docs/work/<packet>", or any request to execute more than one phase of a planned packet in one go.
user-invocable: true
disable-model-invocation: true
---

# /orchestrate-packet

Executes a packet planned by `/feature-plan`. You are the ORCHESTRATOR: you do
not implement phases, you dispatch them, verify what comes back, and own the
boundary with the user.

**User-mode branch rule:** create a runtime-owned phase branch from
`feature/<packet-slug>` before editing, then raise a templated pull request back
into that integration branch; never merge it. **Packet-mode exception:** only an
explicitly declared packet coordinator and the phase agents it dispatches may
commit phase work directly to `feature/<packet-slug>`.

## The one rule that outranks everything

**NEVER merge into `main`.** Raise the PR, report it, stop. `main` is the user's
decision and nothing else in this skill overrides it — not a cleared blocker, not
green CI, not "finish it off", not the teardown doc's instruction to *open* the
PR. Opening is the instruction; merging is not.

The rule is the authority; the deny list is only a backstop. `.claude/settings.json`
denies the common spellings, but it binds Claude Code sessions ONLY, it cannot
enumerate every route, and this skill may dispatch merge-bar QA to another engine
by design. So the prompt text you write is what actually carries this rule —
state it imperatively in every prompt, and never as a guarantee that something
will stop the agent.

## STEP 0 — Declare the mode, in every prompt you write

Two modes, and every phase agent and QA agent must be TOLD which one it is in,
because the mode sets its escalation threshold and its reporting scope.

- **packet mode** — you build the whole packet and report once, at the end. Phase
  agents BATCH anything needing sign-off and do not stop for it. Ratchet or
  budget raises are reviewed by the user in ONE batch after QA, before the
  merge — batched review, NOT pre-approval. Nothing is approved until the user
  says so.
- **user mode** — the user starts each phase, so per-phase escalation is correct
  and a phase agent should stop and ask. This is the default unless an
  orchestrator prompt explicitly declares packet mode. Before editing, the
  session creates its own task branch from `feature/<packet-slug>`; after QA it
  commits, pushes, and raises a templated PR back into integration without
  merging it.

An agent not told its mode will escalate at the wrong grain: mid-packet
interrupts for a decision the user intends to make once, silence where they
wanted a say, or commits made with authority over the wrong branch.

## STEP 1 — Read the packet before dispatching anything

`docs/work/<packet>/`: `README.md`, `state.md`, `progress.md`,
`implementation-plan.md`, `qa-checklist.md`, and EVERY phase doc. Confirm the
packet's blockers are cleared from git, not from the doc's claim.

Watch for numbering drift: phase-doc `# Phase NN` headings can disagree with
their filenames, and in-body cross-references may use either. **Filenames are
authoritative**; tell every agent to resolve in-body references by CONTENT.
Surface the drift to the user as a doc defect; do not fix it mid-packet.

## STEP 2 — Set up the integration worktree yourself

In packet mode, create the packet's integration branch from `main` in its own
worktree and push it BEFORE dispatching phase 01:

```
git worktree add -b feature/<packet> .claude/worktrees/<packet> main
git push -u origin feature/<packet>
```

The phase-01 doc usually assigns this to phase 01. Doing it up front removes the
biggest failure mode — an agent working in the wrong tree — and costs nothing.
Tell phase 01 it is already done.

Give every agent the ABSOLUTE path in capitals, and tell it to prefix every
command with `cd <path> && ...` rather than trusting shell cwd.

## STEP 3 — Dispatch phases sequentially, each to a fresh agent

One agent per phase, strictly in order: dispatch, await completion, verify, then
dispatch the next. Never overlap them. Phases share ONE branch and build on each
other, so an agent that starts early reads a tree missing the work it depends
on — and two agents committing to the same branch at once will collide.

Because this skill explicitly declares packet mode, each phase agent works ON THE
INTEGRATION BRANCH and commits there directly. It does NOT open a per-phase PR.
Do not copy this authority into a user-started phase prompt: user mode owns a
task branch and raises a templated PR into integration. No agent merges either
kind of PR. One final integration-to-`main` PR exists per packet.

Every phase prompt carries: the working directory, the mode, **the
never-merge-into-`main` rule stated imperatively**, the phase doc as
authoritative, what the previous phases landed (with "read the code yourself, do
not work from this summary"), the environment notes, the escalation protocol,
the completion bar, and the report format.

**If a push is rejected** because the shared branch moved, the agent rebases
(`git pull --rebase`) and re-runs the gate. Never `git merge`, never a
force-push — both would rewrite work another phase just landed.

**Environment notes** every phase agent needs:

- `freya` (0.5-rc) and `gix` (pre-1.0) are fast-churn: verify every API against
  the vendored source under `~/.cargo/registry/src/` or current docs, never from
  memory. Freya 0.5 uses a builder API; `rsx!` examples are the old one.
- Run `scripts/gate.sh` (never an ad-hoc `&&` chain, never piped through `tail`)
  before calling a phase done. `--fast` while iterating.
- Commit with explicit paths, never `git add -A`: the build writes into `target/`
  and packet worktrees live under `.claude/worktrees/`.
- A new crate, a new dependency, or a new invariant needs its row in
  `crates/cairn-guards/tests/invariants.rs` in the same commit, or the gate fails.

## STEP 4 — Escalation: give agents one exact protocol

Agents cannot reach the user. Tell each one:

> If a STOPPING RULE triggers, a QA finding is disputed, or a decision needs the
> user, stop and return immediately with a message whose first line is exactly
> `NEEDS USER SIGN-OFF`, followed by the question, the options with your
> recommendation, and what you have already completed. Anything that is NOT a
> stopping rule: decide and proceed.

Relay those to the user VERBATIM, then feed the answer back to the live agent
with `SendMessage` — a resumed agent keeps its context; a new one starts cold.

If you told an agent something that later turns out to be wrong, correct it with
`SendMessage` immediately — especially anything about approval. An agent that
believes a raise was "approved" will write that claim into `progress.md` as fact.

## STEP 5 — Verify every phase; do not take a report at face value

After each phase, check the tree yourself: branch, clean status, commits, sync
with origin. Then spot-check the phase's central claim against the code. Agents
report honestly but at the wrong SCOPE — always require counts and claims scoped
`main...HEAD`, and state the scope beside every number you relay.

## STEP 6 — Send merge-bar QA to an independent reviewer

The final QA phase goes to a genuinely independent reviewer — a different model
or CLI — over the WHOLE packet diff. Give it the same mode declaration, the
never-merge rule stated imperatively (no deny list binds it), the known-stale
inputs it must not be misled by, and an explicit instruction to stop at
acceptance: **no teardown, no PR to `main`, and never a merge.**

Budget for two failure shapes: reviewer subagents can return a truncated process
fragment on first ask and need re-asking (a fragment is a delivery failure, not a
clean report); and a sandboxed engine may fail the gate on an environment
artifact rather than a real failure — re-run the gate yourself outside the
sandbox before believing a FAIL.

After independent QA returns READY, dispatch one fresh teardown agent in packet
mode. It performs the final phase's post-acceptance teardown, runs the final
gate, commits and pushes the teardown on integration, and reports the resulting
tip. Verify its tree and gate evidence before raising the packet PR; the
orchestrator does not fill the missing ownership by implementing teardown itself.

If a dispatched engine's harness errors, CHECK THE TREE. A "failed" call may have
executed and left uncommitted edits. Stash them with a labelled message rather
than discarding or building on them.

**CI on a packet branch.** `ci.yml` runs the gate on every push to `feature/**`,
so each packet-mode phase still gets a full CI run without a PR. An advisory PR
reviewer (if configured) needs a PR and therefore runs only on the packet's PR to
`main` — which is why each phase must dispatch its own reviewers, and why STEP 7
exists.

## STEP 7 — Before any PR: sweep advisory findings

If the advisory reviewer workflow is configured, its inline comments do not
block, so they get merged past. Sweep them:

```
gh api "repos/<owner>/<repo>/pulls/<n>/comments" --jq '.[] | "\(.path):\(.line)\n\(.body)"'
```

Check each against the CURRENT tree — many resolve themselves when later work
rewrites the file, which is luck, not process. Fix or explicitly dismiss with a
reason; never merge past an unaddressed one silently.

## STEP 8 — Hand back, do not finish

Report once: what shipped, the gate result you ran YOURSELF, any
ratchet/baseline movements with their individual reasons, the QA verdict, and
ONE consolidated list of everything needing the user — raises scoped against
`main`, disputed findings, filed leftovers.

Then raise the PR to `main` and STOP.
