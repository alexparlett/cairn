# QA checklist — process-manager

Packet-specific acceptance beyond the repo-wide gate. Acceptance criteria G1-G22
live in `docs/prd/process-manager.md` and are NOT copied here; verify them there,
against their pinned tests.

## Per-phase coverage of the PRD criteria

| Phase | PRD criteria it must satisfy |
| --- | --- |
| 01 | G1, G2, G3, G4, G5 |
| 02 | G6, G7, G8, G9, G10, G11, G12, G13, and G19's reporter |
| 03 | G17 (the engine half), G18 |
| 04 | G14, G15, G16, G17 (through the worker), G20, G21 |
| 05 | all of G1-G22, re-verified over the whole packet diff |

## Packet-specific checks

Beyond the PRD, phase 05 confirms each of these:

- [ ] **Exactly one place builds a `Command`.** Grep the product crates' `src/`,
      not just what the guard reports: `Command::new`, `process::Command` and any
      alias.
- [ ] **No process is waited on with a lock held.** Read every `try_wait`, `kill`
      and `killpg` call site. A signal must never go to a reaped pid, or to a
      group with no member believed alive (the leader reaped and every pipe
      closed).
- [ ] **No pipe is read on the thread that writes another.** Trace each
      invocation shape. A test passing at 64 KiB proves nothing about a Linux
      user at their pipe limit (two pages) or about macOS (512 bytes).
- [ ] **A cancelled write cannot be mistaken for success, nor a finished one for
      a cancel** (R4.5). A clean exit that beat the signal is success. Anything
      that exited after the signal reports cancellation, even with status 0.
      Read the ordering in the code, then confirm G12 controls it.
- [ ] **The drop path never blocks the dropping thread.** Hand-check it, because
      a test that drops on a worker thread would not notice a block.
- [ ] **The command log holds nothing from the environment.** Check its type, not
      just a test.
- [ ] **nix's features are still `process` and `signal`.** The allowlist cannot
      see features.
- [ ] **Every read in `reads/` runs plumbing or `status`.** A porcelain `diff` or
      `describe --dirty` refreshes the index despite `GIT_OPTIONAL_LOCKS=0`.
      The packet adds none, but the rule must be stated where `diff-engine`
      will read it.
- [ ] **The diff-engine path forward is real.** A read invocation can be built
      from `reads/` with a `GitBinary` the diff thread can hold, it cancels on an
      epoch, and its stdout comes back as `-z` records. Prove it with a sketch in
      a test, not a doc claim.
