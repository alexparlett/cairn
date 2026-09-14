# Brainstorm — credential-prompts

Locked decisions and rejected alternatives. Historical record: never retro-edited.

## Locked 2026-09-14

**L1. Cairn implements no authentication and stores no credential.** git already
does this correctly, via the user's `credential.helper` and ssh-agent. Rejected:
a Cairn keychain integration — it would duplicate working infrastructure, ship a
second place for secrets to leak, and break the user's existing setup. Cites:
`docs/design/cairn.md` D2.

**L2. The askpass helper is a separate binary.** Not a design preference: git's
contract is a process, invoked with the prompt on `argv` and read from stdout
(verified against git 2.55.0). Rejected: nothing — there is no in-process
alternative. The upside is real, though: the secret never enters the main
process's address space. Cites: the evidence record.

**L3. `GIT_TERMINAL_PROMPT=0` on every invocation.** Without it, a GUI with no tty
hangs indefinitely on an invisible prompt. It is the difference between
"authentication failed" and "the app froze". Cites: the evidence record.

**L4. The subprocess backend lands in this packet.** An askpass helper with no
`git` invocation to serve is untestable. Rejected: a separate backend packet
first — it would ship an interface with no consumer.

**L5. The environment handed to `git` is built explicitly, never inherited
wholesale.** Every passed variable is a deliberate entry. Rejected: passing the
parent environment through and overriding a few keys — that inherits whatever the
launching shell had, including an askpass the user set for something else.

**L6. A secret never travels on `argv`.** `/proc` makes `argv` world-readable on
Linux. The prompt goes on `argv`; the secret comes back only on stdout.

**L7. "Do not degrade a working setup" is a product rule with a regression
test.** A user with libsecret, osxkeychain or a loaded ssh-agent must see no new
dialog. This is the rule an otherwise-correct implementation is most likely to
break, because a helper that overrides rather than falls back passes every other
criterion. Cites: PRD product rules, criterion B4.

**L8. Push is deliberately not in this packet.** It is the obvious next consumer
of the backend, and it is destructive — it needs `Confirmed` and the
`destructive-ops-reviewer`, which is a different conversation from "can we
authenticate at all".

## Locked 2026-09-14, second pass

Decided with the user when the packet's open questions were reviewed, before any
code was written.

**L9. Minimum git version is 2.30.** The technical floor is lower — the binding
feature is porcelain v2 status at 2.11; `GIT_ASKPASS`, `GIT_TERMINAL_PROMPT` and
`-z` are all far older. 2.30 is Debian bullseye's version, comfortably past
everything Cairn uses, and anything older is on a distro whose Rust toolchain
would struggle with a 2024-edition binary anyway. This is a support policy, so it
belongs to the user and not to a phase. Rejected: 2.11 (a 2016 git, tested
forever for no user) and 2.45 (excludes Debian bookworm at 2.39 for convenience
Cairn has not earned). Closes O1.

**L10. The channel is a unix socket in `$XDG_RUNTIME_DIR`, and its threat model is
stated honestly: it protects against OTHER users, not against same-user
processes.** 0700 directory, 0600 socket, not the Linux abstract namespace (which
carries no permissions), single-use token handed to the helper via the
environment rather than `argv` (L6). Same-user isolation is not achievable — a
process running as the user can read our environment through `/proc` — and it is
also not worth pursuing, because that same process could read
`~/.git-credentials` or query the ssh-agent directly. Writing the limit down is
the point: a threat model that overclaims is worse than one that is narrow.
Rejected: `SO_PEERCRED` peer verification (raises the bar against a racing
same-user process, but is defeatable by a determined one and buys complexity for
a boundary that cannot hold) and an inherited pre-opened fd (cleaner in
principle, but depends on git preserving inherited fds across askpass invocation,
which is unverified). Closes O2.

**L11. `zeroize` is added as a dependency for the secret type.** Hand-rolled
zeroing can be optimised away, so a type doing it by hand would claim a
protection it may not provide — worse than claiming nothing. `zeroize` is small,
has no transitive dependencies, and documents the compiler-fence guarantee that
makes the write actually happen. Per the dependency invariant this was the user's
call, and it was taken deliberately. The dependency is added in phase 02, in the
same commit as the code that uses it and the allowlist row in
`crates/cairn-guards/tests/invariants.rs`. Rejected: hand-rolling, and dropping
the zeroing requirement in favour of relying on the helper being a short-lived
separate process. Closes O3.

## Open, for the phase that meets them

- **O4 (phase 03).** Precedence between `GIT_ASKPASS`, `core.askPass` and
  `SSH_ASKPASS`, and whether setting them can suppress a helper the user
  configured. Unverified in the evidence record and directly load-bearing for L7.
- **O5 (phase 03).** The OpenSSH version that introduced `SSH_ASKPASS_REQUIRE`,
  and the behaviour when it is absent.
