# Implementation plan — process-manager

## Shape

```
cairn-app    worker/ ── startup: GitBinary found once ──► cloned per thread
                 │      network lane (was cairn-operations): fetch; refuses a duplicate
                 │      repository close / window close ──► registry.end_all()
                 ▼
cairn-git    ops/   ── fetch (Write, WriteAuthority) ─┐
             reads/ ── (first fn: diff-engine's)     ─┤  only callers
                                                      ▼
             process/ ── GitBinary · GitEnvironment (base + read/write profile)
                         Invocation{Read|Write} ── runner:
                           process_group(0) · thread per pipe
                           stdout → bytes | -z records | collect(ceiling)
                           stdin ← caller bytes, then closed
                           stderr → progress lines + 256 KiB tail
                           cancel: &impl Cancel poll │ KillHandle │ Drop→reaper
                           end: killpg TERM ─2s─► killpg KILL
                         Registry (per repository) · CommandLog
                                                      │
cairn-model  CommandRecord ◄──────────────────────────┘
```

## Phase order and why

1. **01, the seal and the environment**, comes first. Every later phase builds
   invocations, and they must be built through the types and checked by the
   guard from the start. The old runner moves into `process/` unchanged here, so
   the gate stays green while the new one does not exist yet.
2. **02, the runner**, is the bulk of the packet and the riskiest part: pipes,
   signals and reaping. It lands beside the old runner, tested by stub `git` and
   real `git`, before anything depends on it.
3. **03 migrates fetch and the probe**, deletes the old runner, and adds the
   registry and the log. This is the first phase where shipped behaviour runs on
   the new runner, so the credential-prompts suite is its safety net.
4. **04 wires the application**: startup discovery, the network lane and its
   refusal, kill-all on close, and the log answered through the worker. It also
   writes the as-built docs.
5. **05, QA**, runs as its own fresh session as the merge bar.

## Review dispatch per phase

The packet's one copy of the rules. Every phase ends by orchestrating its own QA:
`/qa` over the phase diff with these reviewers spawned fresh, and `qa-confirm`
(fresh) adjudicating. `qa-checklist` is always on and is not repeated below.

| Phase | Reviewers, beyond `qa-checklist` |
| --- | --- |
| 01 | `gate-integrity-reviewer` — the guards that pin the environment and the mutation seal are rewritten, and each must fail toward more coverage. `destructive-ops-reviewer` — the environment every `git` runs with changes (its check 9), and the write seal moves. `test-coverage-auditor` |
| 02 | `destructive-ops-reviewer` — what a cancel leaves behind (locks, orphans, a half-run mutation). `responsiveness-reviewer` — the kill handle is called from the UI thread and must never block. `test-coverage-auditor` — stub tests that read the process table must decide what they claim |
| 03 | `destructive-ops-reviewer` — fetch's path through `ops/` changes. `test-coverage-auditor`. `gate-integrity-reviewer` — if any guard roster changes when the old runner's names disappear |
| 04 | `responsiveness-reviewer` — the worker, shutdown and the refusal path. `test-coverage-auditor` |
| 05 | all of the above, over the whole packet diff |

## Invariants in play

- **Every `git` subprocess runs with an environment Cairn built.** Its twin is
  `every_git_invocation_disables_the_terminal_prompt`, which names
  `crates/cairn-git/src/ops/environment.rs` by path. Phase 01 moves that file, so
  the twin must move with it in the same commit. It must keep every check it has
  — exactly one `Command`, `env_clear` and `envs`, the `ALWAYS` table's pins —
  and add the new `ALWAYS` entries.
- **Only `ops/` mutates a repository.** Rewritten in phase 01. It gets a type
  seal (`WriteAuthority`) and a twin that covers the gitoxide half, which today
  has none.
- **No credential value is logged.** Phase 03's command log is new state that
  sits next to the askpass token. The token lives in the environment, and the log
  never records the environment. Check the `SECRET_HOLDERS` roster stays empty.
- **A read never refreshes the index.** `GIT_OPTIONAL_LOCKS=0` covers `status`
  only, so a read runs plumbing or `status`. The packet adds no read, but phase
  01's documentation of `reads/` must say so where `diff-engine` will read it.
- **No `unsafe`.** Process groups come from `CommandExt::process_group` and
  signals from `nix` `killpg`, both safe. `pre_exec`, `setsid` in the child and
  `PR_SET_PDEATHSIG` would need `unsafe` and are out.
- **Shipping code never panics.** Thread spawns, pipe takes and signal calls all
  fail with errors, never with `expect`.
- **The UI thread never waits on repository work.** The kill handle and the
  refusal path are called from the UI thread. `RepositoryHandle::submit` is one
  of the exempt functions the guard cannot see into.
- **Each crate depends only on its allowlist.** No new crate. nix's features stay
  `process` and `signal`. Enabling another feature passes every automated check
  (the allowlist reads names, not features), so it is the reviewer's to catch.
- **A change to `cairn-model`, `ops/` or `cairn-guards` carries a test in the same
  commit.** Phase 01 touches `ops/` and `cairn-guards`; phase 03 touches
  `cairn-model` and `ops/`.

## Risks

- **Reaping without a lock held across a blocking wait.** std's `Child::wait`
  blocks, and nix's `waitpid` must never be called on a pid std owns: it reaps
  behind std's back. Keep the as-built shape: poll `try_wait` under a short lock,
  as `cli.rs`'s `reap` does.
- **The drain bound.** Too short, and a slow writer's last output is lost. Too
  long, and a leftover child adds latency to every read. G10 pins that the bound
  holds, and G19 pins the read-path cost on a real repository.
- **Detecting exit without a tick.** A 20 ms poll tick adds up to 20 ms to every
  read, which G19 forbids. In the common case, exit is noticed when the pipes
  close. The tick paces cancel polling, and it also catches the uncommon case
  G10 tests: a grandchild holding the pipes after git has exited. For that it
  calls `try_wait` and then starts `DRAIN_BOUND`.
- **Moving a twin's file.** If phase 01 moves `environment.rs` but not the
  guard's path, the guard either fails or, worse, checks an empty file set.
  Every guard asserts a nonzero file count. Confirm it still does.
