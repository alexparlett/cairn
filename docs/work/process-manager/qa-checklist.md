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

- [x] **Exactly one place builds a `Command`.** Grep the product crates' `src/`,
      not just what the guard reports: `Command::new`, `process::Command` and any
      alias.
- [x] **No process is waited on with a lock held.** Read every `try_wait`, `kill`
      and `killpg` call site. A signal must never go to a reaped pid, or to a
      group with no member believed alive (the leader reaped and every pipe
      closed).
- [x] **No pipe is read on the thread that writes another.** Trace each
      invocation shape. A test passing at 64 KiB proves nothing about a Linux
      user at their pipe limit (two pages) or about macOS (512 bytes).
- [x] **A cancelled write cannot be mistaken for success, nor a finished one for
      a cancel** (R4.5). A clean exit that beat the signal is success. Anything
      that exited after the signal reports cancellation, even with status 0.
      Read the ordering in the code, then confirm G12 controls it.
- [x] **The drop path never blocks the dropping thread.** Hand-check it, because
      a test that drops on a worker thread would not notice a block.
- [x] **The command log holds nothing from the environment.** Check its type, not
      just a test.
- [x] **nix's features are still `process` and `signal`.** The allowlist cannot
      see features.
- [x] **Every read in `reads/` runs plumbing or `status`.** A porcelain `diff` or
      `describe --dirty` refreshes the index despite `GIT_OPTIONAL_LOCKS=0`.
      The packet adds none, but the rule must be stated where `diff-engine`
      will read it.
- [x] **The diff-engine path forward is real.** A read invocation can be built
      from `reads/` with a `GitBinary` the diff thread can hold, it cancels on an
      epoch, and its stdout comes back as `-z` records. Prove it with a sketch in
      a test, not a doc claim.

## Evidence (phase 05)

1. `Command` is named in production only in `process/environment.rs` (import and the one `Command::new`); `cli.rs` names `CommandExt` and `Stdio`; no alias, no `build.rs` (qa-checklist reviewer's grep of every `crates/*/src`).
2. The one `try_wait` is `group.rs`, the `killpg` calls are in `Leader::signal`; every lock hold is a `try_wait`, `killpg` or field write; `Leader::signal` reaps, then checks `open_pipes` and `believed_alive` under one lock, and `over` blocks any later signal (`a_kill_after_the_invocation_is_over_signals_nothing`).
3. A thread per pipe, the driver only receives on a bounded channel; the 5 MiB, 64 MiB and 1 MiB tests exceed any pipe, so pipe size decides nothing.
4. `Ended::cancelled` decides by whether a signal reached a running leader; G12's three tests control both orderings (one waits for the zombie), each killed by its mutation.
5. `Driver::drop` is `request_end` (try-lock plus `killpg`) and a reaper-thread spawn; the no-thread fallback's bounded wait is stated; `dropping_*` tests pass and fail under a blocking drop.
6. `CommandRecord` has exactly the R8.1 fields, destructured exhaustively by `the_record_holds_exactly_what_r8_1_lists`; `a_fetch_with_a_token_is_logged_once_without_the_token_or_the_environment` plants five values.
7. Exactly `process` and `signal` (`cargo tree -e features -i nix`), and now pinned by `deny.toml` `[[bans.features]]`.
8. `reads/` has no function; `reads/mod.rs` states plumbing or `status` only, now also no `--textconv`/`--ext-diff`; destructive-ops check 10 and the dispatch rows now name `reads/`.
9. `diff_engine_path_forward` in `reads/mod.rs` (commit `d0a8e19`): a read built there from a cloned `GitBinary`, run on a thread, cancelled by an epoch, answering `-z` records; committed as a permanent pin.
