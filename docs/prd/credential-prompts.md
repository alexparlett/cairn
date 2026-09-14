---
status: in-flight
packet: credential-prompts
opened: 2026-09-14
---

# PRD — Credential prompts

The authoritative spec for the `credential-prompts` packet while it is in flight.
Design frame: `docs/design/cairn.md`, decisions **D1** (writes go through the
`git` binary) and **D2** (credentials delegated to git entirely). Evidence:
`docs/research/credential-prompts/git-credential-delegation.md`.

## What this packet delivers

The ability to run an authenticated `git` operation from Cairn without Cairn ever
holding a credential: the subprocess backend that D1 requires, the askpass helper
binary that lets git ask a GUI for a secret, and one real operation — `fetch` —
wired end to end to prove the whole path.

The subprocess backend is in this packet because nothing else can be: an askpass
helper with no `git` invocation to serve is untestable.

## Requirements

### R1 — A typed `git` subprocess backend

- R1.1 `cairn-git/src/ops/cli.rs` builds and runs `git` invocations. It is the
  only place in Cairn that spawns a process, which the existing
  `only_the_ops_module_mutates_a_repository` guard already pins.
- R1.2 The environment handed to `git` is constructed explicitly, never inherited
  wholesale. Every variable passed through is a deliberate entry.
- R1.3 Machine-readable output formats are used where git offers them (`-z`,
  porcelain v2). Human-facing output is never parsed on a path where a
  machine-readable form exists.
- R1.4 Failures surface as `cairn-git::Error` variants naming what the caller must
  handle, carrying git's stderr for diagnosis — never a bare exit code.
- R1.5 Cairn locates `git` and checks its version once at startup, and fails
  loudly with a message naming the minimum version. It never degrades silently.

### R2 — An askpass helper that never leaks

- R2.1 A second binary, invoked by git with the prompt as `argv[1]`, which obtains
  the secret from the running Cairn UI and writes it to stdout.
- R2.2 The helper reaches the app over a channel whose access control is stated
  and justified. It carries plaintext secrets between two processes; its
  permissions are the security story.
- R2.3 A secret never appears in any process's `argv`, since `/proc` makes that
  world-readable on Linux.
- R2.4 The type carrying a secret has no `Debug`, no `Display`, no `Serialize`,
  and zeroes on drop.
- R2.5 A prompt Cairn cannot service — no UI, user cancels — fails the operation
  cleanly. It never hangs and never retries silently.

### R3 — Every `git` invocation is prompt-safe

- R3.1 `GIT_TERMINAL_PROMPT=0` on every invocation, so a missing credential is an
  error rather than an invisible hang on a tty that does not exist.
- R3.2 `GIT_ASKPASS` points at the helper.
- R3.3 `SSH_ASKPASS` points at the helper and `SSH_ASKPASS_REQUIRE=force` is set,
  with the OpenSSH version that introduced it established and recorded — the
  evidence record flags this as unverified.

### R4 — Fetch, end to end

- R4.1 A fetch runs against an HTTPS remote requiring credentials, and against an
  SSH remote requiring a key passphrase.
- R4.2 A fetch against a remote already served by the user's credential helper or
  ssh-agent completes **with no Cairn prompt at all**.
- R4.3 Fetch progress is visible and the UI stays responsive throughout.

## Product rules

- **Cairn must not degrade a setup that already works.** A user with `libsecret`,
  `osxkeychain` or a loaded ssh-agent sees no change in behaviour and no new
  dialog. This is the rule most likely to be broken by a plausible-looking
  implementation, because a helper that overrides rather than falls back would
  pass every other criterion here.
- Cairn never offers to remember a credential. Persistence is the user's
  `credential.helper`, configured by them, and Cairn does not write that config.
- A credential dialog states which remote and which URL is asking. A prompt that
  does not say who wants the password trains people to type it into anything.
- No credential value reaches the operation log, a panic message, a crash report,
  or a `tracing` event at any level.

## Acceptance criteria

The single authoritative copy. `docs/work/credential-prompts/qa-checklist.md`
points here and does not restate them.

| # | Criterion | Pinned by |
| --- | --- | --- |
| B1 | A `git` invocation runs with an explicitly constructed environment; a test enumerates exactly which variables are passed | unit test over the environment builder |
| B2 | Startup rejects a missing or too-old `git` with a message naming the required version | integration test with a stubbed `git` on `PATH` |
| B3 | A fetch from an HTTPS remote needing a password prompts once and succeeds | integration test against a local HTTP remote with a fixed credential |
| B4 | A fetch from a remote already served by a credential helper produces **no** prompt | integration test with a stub helper; this is the regression that protects the product rule |
| B5 | Cancelling a prompt fails the operation cleanly, with no hang and no retry | integration test |
| B6 | No secret appears in `argv`, in any log line, or in a `Debug` rendering | a guard test in `cairn-guards` plus the type's missing impls |
| B7 | The secret-carrying type has no `Debug`/`Display`/`Serialize` and zeroes on drop | compile-fail test plus a drop test |
| B8 | `scripts/gate.sh` passes | the gate |

B3 and B4 need a local git remote fixture rather than a network. Standing one up
is part of the work, not an assumption.

## Out of scope

Filed, not done: push (the next operation to use this backend, deliberately
separate because it is destructive and needs `Confirmed`), clone, submodule
credentials, proxy configuration, GPG commit signing, and any Cairn-owned
credential storage — which D2 rules out permanently, not just for this packet.
