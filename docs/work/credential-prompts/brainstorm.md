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

## Open, for the phase that meets them

- **O1 (phase 01).** The minimum `git` version Cairn requires. The evidence was
  gathered against 2.55.0; the floor should be justified by the features actually
  used, not by what happened to be installed.
- **O2 (phase 02).** The helper/app channel. A unix domain socket in the user's
  runtime directory with a per-operation single-use token is the candidate, but
  the choice needs justifying: this channel carries plaintext secrets between two
  processes and its permissions are the entire security story.
- **O3 (phase 02).** Whether the secret type can zero on drop without a new
  dependency. A dependency here is a user decision, and a hand-rolled version
  that the optimiser removes is worse than none — decide with evidence.
- **O4 (phase 03).** Precedence between `GIT_ASKPASS`, `core.askPass` and
  `SSH_ASKPASS`, and whether setting them can suppress a helper the user
  configured. Unverified in the evidence record and directly load-bearing for L7.
- **O5 (phase 03).** The OpenSSH version that introduced `SSH_ASKPASS_REQUIRE`,
  and the behaviour when it is absent.
