---
name: qa-checklist
description: Run the qa-checklist agent alone, in a fresh context, over the current diff. Lighter than /qa (no reviewer fan-out, no fixes) — use it for a quick contract check mid-work, or when a packet phase asks for a fresh qa-checklist pass. Trigger on "/qa-checklist" or "run the checklist".
argument-hint: [optional diff scope, e.g. a base..head range]
context: fork
agent: qa-checklist
background: false
---

Run your full QA checklist review.

Scope: $ARGUMENTS — if no scope was given, apply `docs/qa-gate.md`'s Review diff
scope rule: committed branch changes plus staged, unstaged, and untracked work.

You are spawned fresh, with no implementer context, on purpose: judge only what
the diff and the repo contracts (root `CLAUDE.md`, `docs/qa-gate.md`, local
CLAUDE.mds) actually say. Deliver the full QA CHECKLIST REPORT, including the
"Dispatch next" reviewer list, as your final message.
