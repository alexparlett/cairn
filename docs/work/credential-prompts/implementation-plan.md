# Implementation plan — credential-prompts

## Shape

```
cairn-app          UI prompt dialog ── names the remote and URL asking
      │                  ▲
      │                  │ (channel: O2)
      │            cairn-askpass          separate binary
      │                  ▲                 argv: prompt   stdout: secret
      │                  │
cairn-git   ops::cli ── spawns git with an explicitly built environment:
                        GIT_ASKPASS, SSH_ASKPASS, SSH_ASKPASS_REQUIRE=force,
                        GIT_TERMINAL_PROMPT=0
```

The secret's whole lifetime is: UI dialog → channel → helper stdout → git stdin.
It never exists in `cairn-git`, never in application state, never in a log.

## Phase order and why

Strictly sequential. 01 gives 02 something to serve; 03 proves both against a
real remote. Attempting 02 first produces a helper that can only be tested by
pretending to be git.

## Review dispatch per phase

The packet's one copy of the rules. Every phase runs `/qa` at its end; these are
the reviewers each phase must include beyond the always-on `qa-checklist` and the
`qa-confirm` adjudication.

| Phase | Reviewers |
| --- | --- |
| 01 | `destructive-ops-reviewer` — the phase creates the mutation backend itself, so its judgement about what counts as destructive applies to the whole surface; plus `test-coverage-auditor` |
| 02 | `destructive-ops-reviewer`, `gate-integrity-reviewer` (the phase adds invariants and guards), `test-coverage-auditor` |
| 03 | `responsiveness-reviewer` (fetch must not block the UI), `destructive-ops-reviewer`, `test-coverage-auditor` |
| 04 | all of the above over the whole packet diff |

## Invariants in play

- Only `cairn-git/src/ops/` spawns a process. The existing
  `only_the_ops_module_mutates_a_repository` guard already pins this — phase 01
  is the first code it actually constrains, so confirm it fires rather than
  assuming it does.
- Never commit secrets. This packet makes that invariant load-bearing in a new
  way: test fixtures carry credentials, so the fixtures must generate them, never
  contain them.
- Every dependency addition is a user decision (relevant to O3).

## New enforcement this packet must leave behind

Two invariants land with their twins, in the phases that create them:

1. **No credential value is logged, `Debug`-printed, serialised, or stored in
   application state.** Twin: a guard test over the secret type's impls plus a
   forbidden-token check; phase 02.
2. **Every `git` invocation sets `GIT_TERMINAL_PROMPT=0`.** Twin: a guard
   asserting the environment builder is the only construction path and always
   includes it; phase 01. An invocation that skips it hangs the app, which is
   exactly the class of bug a guard should own rather than a review.

Both go into `CLAUDE.md`'s Invariants with their guard names, in the same commit
as the guard.
