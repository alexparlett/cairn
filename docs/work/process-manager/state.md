# State — process-manager

The cross-session cheat sheet. Every session updates this before ending.

**Status: planned. No phase has started. No code exists for this packet.**
`diff-engine` is paused on `feature/diff-engine` until this packet merges.

## Locked decisions

L1-L15 are in `brainstorm.md`. The design frame is `docs/design/processes.md`,
with D1 in `engine.md` and D3 in `concurrency.md`. These constrain the
implementation most:

- **Only `process/` builds a process (L2).** Only `ops/` and `reads/` call the
  runner. A write needs a `WriteAuthority`, which only `ops/` can construct.
- **Reads run free; writes go through two lanes (L3).** This packet builds the
  network lane (fetch) and the refusal of a duplicate. The local lane is built by
  `staging-and-commit`.
- **Every `git` gets its own process group and ends `SIGTERM` → 2 s → `SIGKILL`
  to the group (L4).** Cancel comes by poll (`&impl Cancel`), by handle, or by
  drop.
- **A thread per pipe (L6).** stdout goes to the caller; a collect has a ceiling
  that errors when crossed; stderr keeps a 256 KiB tail (L7).
- **Nothing parses translated text, and the locale stays the user's (L8).**
  `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false` always; a read adds
  `GIT_OPTIONAL_LOCKS=0` and carries no token (L9).
- **No timeouts, no batch children, no retries, no lock deletion** (L5, L8, L12).

## Open questions

None of this packet's own. The questions it leaves to other packets are listed
at the end of `brainstorm.md`.

## New modules and interfaces introduced so far

None yet. As phases land, record here the type or function, its crate, and its
one-line contract.

| Symbol | Crate | Contract |
| --- | --- | --- |
| _(none)_ | | |

## Bounds fixed by phases

The PRD names these and leaves their values to the phase that builds them.
Record each value and its reason here when it is chosen.

| Bound | Phase | Value | Why |
| --- | --- | --- | --- |
| `DRAIN_BOUND` — output read after the leader exits (R3.6) | 02 | — | — |
| `CLOSE_BOUND` — wait for reaps on repository close (R6.3) | 03 | — | — |
| `LOG_ENTRIES`, `LOG_BYTES` — command log size (R8.2) | 03 | — | — |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 seal and environment | not started | — | — |
| 02 runner | not started | — | — |
| 03 engine lifecycle | not started | — | — |
| 04 application | not started | — | — |
| 05 QA | not started | — | — |

## Environment notes

- **The bench repository is a clone of rust-lang/rust at `c999cef531e` in
  `~/Development/bench/rust`.** G16 is measured there, and the harness reads
  `CAIRN_BENCH_REPO`. Read it only: no `gc`, no `repack`, no config writes.
- **Verify every std and nix API against the source that links**, never from
  memory:
  - std is under `$(rustc --print sysroot)/lib/rustlib/src/rust/library`
    (rust-src is installed for 1.97.1).
  - nix 0.31.3 is under `~/.cargo/registry/src/`.
  - `docs/research/process-manager/platform-and-git-behaviour.md` records what was
    verified, and where.
- **Don't call nix's `waitpid` on a pid std's `Child` owns.** It reaps behind
  std's back.
- **No `unsafe`.** `pre_exec`, child-side `setsid` and `PR_SET_PDEATHSIG` are all
  out.
- **Run `scripts/gate.sh` for the bar.** Never an ad-hoc `&&` chain, and never
  pipe it through `tail`.
- **Commit explicit paths**, never `git add -A`. No commit, PR or comment carries
  a Claude Code session link (root `CLAUDE.md`).
- **A new invariant or guard change needs its row and self-test in
  `crates/cairn-guards/` in the same commit.**
- The remote is `github.com/alexparlett/cairn`. Issues and pull requests go
  there, and every pull request is merged by the user, never by a session.
