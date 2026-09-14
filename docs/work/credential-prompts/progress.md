# Progress — credential-prompts

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-14 — three open questions closed with the user

Reviewing the packet's open questions closed the three that were genuinely the
user's to take, before any code was written: minimum git version (2.30, a support
policy), the channel's threat model (protects against other users, not same-user
processes — accepted with the limit written down rather than papered over), and
`zeroize` as an accepted dependency (hand-rolled zeroing can be optimised away,
so doing it by hand would claim a protection it might not provide).

Recorded as L9, L10 and L11. O4 and O5 remain, and both are settled by test or
lookup in phase 03 rather than by decision.

## 2026-09-14 — packet planned

Filed from the design decisions locked the same day (`docs/design/cairn.md` D1,
D2). The mechanism was verified against git 2.55.0's own manual on the
development machine rather than recalled: `GIT_ASKPASS` takes the prompt on
`argv` and reads the secret from the program's stdout, and `GIT_TERMINAL_PROMPT`
is what prevents a tty fallback. Those two facts set the whole shape — a separate
helper binary, and a secret that never enters the main process.

Two things were deliberately NOT verified and are carried as open questions: the
precedence between `GIT_ASKPASS`, `core.askPass` and a configured
`credential.helper` (O4), and the OpenSSH version that introduced
`SSH_ASKPASS_REQUIRE` (O5). O4 can invalidate the packet's central product rule,
so it is settled by test, not by reading.

No implementation has started.
