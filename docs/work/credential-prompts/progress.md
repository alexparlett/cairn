# Progress — credential-prompts

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

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
