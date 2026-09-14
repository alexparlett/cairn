---
name: ci-triage
description: Diagnose a red CI run before touching anything. Classify the failure from the first failing step's log, then apply the matching remedy. Trigger on "CI is red", "the build failed", a failing check on a PR, or a red run URL.
user-invocable: true
---

# /ci-triage

Classify first, then remedy. A blind re-run wastes a full CI cycle and, worse, can
launder a real regression as a flake. State the class you diagnosed and the
evidence before doing anything.

## Step 1: Read the actual failure

`gh run list --limit 5` to find the run; `gh run view <id> --log-failed` for the
FIRST failing step. A cancelled job counts as a failure to explain, not to ignore.

## Step 2: Classify, then remedy

- **Real failure, reproduces locally** — `scripts/gate.sh` red on the same
  commit. Fix it test-first (`extract-and-test` skill); this is not a CI problem.
- **Toolchain drift** — a format or lint step red in CI but green locally: CI
  installs a current toolchain, so a new lint or format rule can land there
  first. Update the local toolchain, reproduce, fix. Never silence the lint to
  make CI pass without the user.
- **Base moved** — the PR is red only after its base advanced: rebase, run
  `scripts/gate.sh` locally, push.
- **Lockfile desync** — lockfile conflicts or frozen-install failures after a
  base merge: regenerate the lockfile in a dedicated commit, nothing else in it.
- **Infra flake** — runner died, network timeout, cache corruption, stall with no
  test output. Re-run ONCE, loudly, quoting the failure signature in the PR or
  commit thread. If it fails again it is not a flake; treat as real.
- **Environment gap** — a tool or secret the workflow needs is absent (a
  version-pinned tool not installed, a secret not configured). Fix the workflow
  or surface the missing secret to the user; never soften the failing step.

## Step 3: Close the loop

Whatever the class, end with: the diagnosis in one line, the remedy applied, and
the green run (or the escalation to the user). A flake re-run that passes still
gets its signature recorded — flakes get exactly one sanctioned retry
(`docs/qa-gate.md`), and the record is what distinguishes the second occurrence.
