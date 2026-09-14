# Delegating credentials to git: the mechanism, verified

Evidence record. Gathered 2026-09-14 for the `credential-prompts` packet against
**git 2.55.0**, the version installed on the development machine. Method:
`git --version` and the `git(1)` manual page on that machine. The packet must
re-check these against the minimum git version Cairn ends up requiring.

## The two environment variables that do the work

From `git(1)`, verbatim in substance:

- **`GIT_ASKPASS`** — "If this environment variable is set, then Git commands
  which need to acquire passwords or passphrases (e.g. for HTTP or IMAP
  authentication) will call this program with a suitable prompt as command-line
  argument and read the password from its STDOUT. See also the `core.askPass`
  option in git-config(1)."
- **`GIT_TERMINAL_PROMPT`** — "If this Boolean environment variable is set to
  false, git will not prompt on the terminal (e.g., when asking for HTTP
  authentication)."

Two consequences that shape the design:

1. The contract is a **process**, not a library call: prompt in on `argv`, secret
   out on stdout. Cairn's helper is therefore a second binary, and the secret
   never enters the main process's address space.
2. `GIT_TERMINAL_PROMPT=0` is what turns a hang into an error. Without it, a
   `git fetch` from a GUI with no tty can block indefinitely on a prompt nobody
   can see. It is the difference between "authentication failed" and "the app
   froze".

## Ordering, and why the helper is a fallback rather than the path

git consults `credential.helper` first. When the user already has a working
helper — `libsecret` and `store` are common on Linux, `osxkeychain` on macOS —
it answers and the askpass program is never invoked. So the common case for a
developer with a working setup involves no Cairn UI at all, which is the correct
outcome and worth stating as an acceptance criterion: **Cairn must not degrade a
setup that already works.**

Not yet verified, and the packet must establish it: the precedence between
`GIT_ASKPASS`, the `core.askPass` config value, and the older `SSH_ASKPASS`, and
whether setting one suppresses a helper the user configured.

## SSH is a separate path

Key passphrases are `ssh`'s business, not git's. `ssh` has its own `SSH_ASKPASS`,
and — unlike git — historically only consults it when there is no controlling
terminal, which is why `SSH_ASKPASS_REQUIRE=force` exists. Both need setting, and
both need testing against the OpenSSH version in play; this was NOT verified here
and is an open item for the packet, including which OpenSSH version introduced
`SSH_ASKPASS_REQUIRE`.

An agent-backed key needs no prompt at all. Same acceptance criterion as above:
an agent that works today must keep working untouched.

## What Cairn must build

- A helper binary, invoked by git with the prompt as `argv[1]`, that reaches the
  running Cairn instance, obtains the secret from a UI dialog, and writes it to
  stdout followed by a newline.
- A transport between helper and app. A unix domain socket in the user's runtime
  directory with a per-operation single-use token is the obvious candidate; the
  packet must justify the choice, because this channel carries plaintext secrets
  between two processes and its permissions are the whole security story.
- The environment Cairn hands every `git` invocation: `GIT_ASKPASS` and
  `SSH_ASKPASS` pointing at the helper, `SSH_ASKPASS_REQUIRE=force`,
  `GIT_TERMINAL_PROMPT=0`, and a deliberate decision about every other inherited
  variable rather than passing the parent environment through wholesale.

## The hygiene rules this creates

Worth promoting to invariants with enforcement twins when the packet lands:

- A credential value is never logged, never `Debug`-printed, and never stored in
  application state. The type carrying it should have no `Debug`, no `Display`,
  no `Serialize`, and should zero on drop.
- A secret never appears in a command line. `argv` is world-readable on Linux via
  `/proc`; the prompt travels on `argv`, the secret only on stdout.
- The operation log records that authentication happened, never what was sent.
