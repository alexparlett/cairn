# Packet: process-manager

Builds one place in Cairn that starts `git`, and a runner that can carry every
kind of invocation the daily loop needs: a parsed read a newer query can stop, a
mutation fed on stdin, a hooked mutation that runs for minutes, and a network
operation with progress. Every invocation runs in its own process group, can
always be ended, and is recorded in a command log. The packet adds no verb the
user can see. It exists so `diff-engine` can take its changes query from
`git diff-tree`, and so `staging-and-commit` can feed `git apply --cached`,
without either of them reopening the runner.

Spec: `docs/prd/process-manager.md`. Design frame: `docs/design/processes.md`,
with D1 in `engine.md` and D3 in `concurrency.md` beside it. Decisions:
`brainstorm.md` L1-L15. Evidence: `docs/research/process-manager/` (four
records), plus `docs/research/diff-engine/git-process-survey.md` and
`rename-parity-spike.md`, which commissioned it.

Integration branch: `feature/process-manager`, off `main`.

## Phases

| Phase | What it lands |
| --- | --- |
| [01](phase-01-seal-and-environment.md) | `process/` and `reads/`, the read/write types and `WriteAuthority`, the environment profiles, and the guard rewrite |
| [02](phase-02-runner.md) | the runner: process groups, a thread per pipe, stdout records, stdin, bounded stderr, the three cancels |
| [03](phase-03-engine-lifecycle.md) | fetch and the probe on the new runner with the old one deleted, the registry, the command log |
| [04](phase-04-application.md) | discovery at startup, the network lane and its refusal, kill-all on close, the log through the worker, the as-built docs |
| [05](phase-05-qa.md) | merge-bar QA over the whole packet, then teardown |

Strictly sequential. 02 needs 01's types to build the runner around, 03 moves
fetch onto 02's runner, and 04 wires what 03 exposes.

After this packet merges, `diff-engine` continues on `feature/diff-engine`. It
has to be brought up to date with `main`, which is the user's call because it is
a shared branch. Its changes query is then reworked onto `reads/`.
