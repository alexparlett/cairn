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

## Addendum, 2026-09-17: O4 settled by test — askpass does not suppress a credential helper

Phase 03 of the packet. Method: `git credential fill` on the development machine
(git 2.55.0), which runs the same `credential_fill` path `git fetch` does, with a
stub `credential.helper` script that answers `get` and records being consulted,
a stub askpass script that records every prompt it is handed, an isolated
`HOME`, `GIT_CONFIG_NOSYSTEM=1` and `GIT_TERMINAL_PROMPT=0`. Every case was
run under `env -i` with only those variables. Observed, in that order:

| Case | Configured | Who answered | Askpass invoked? |
| --- | --- | --- | --- |
| A | `credential.helper` (per invocation) and `GIT_ASKPASS` | the helper | **no** |
| B | as A, plus `core.askPass` | the helper | **no** |
| G | `credential.helper` in `~/.gitconfig` and `GIT_ASKPASS` | the helper | **no** |
| C | `GIT_ASKPASS` only | askpass, twice (`Username for 'https://host': `, then `Password for 'https://from-askpass@host': `) | yes |
| D | `core.askPass` only | askpass, as C | yes |
| D2 | `GIT_ASKPASS` and a different `core.askPass` | the `GIT_ASKPASS` program | yes |
| E | a helper answering the username only, plus `GIT_ASKPASS` | the helper for the username, askpass for the password only | yes, once |
| F | neither, `GIT_TERMINAL_PROMPT=0` | nobody: `fatal: could not read Username for 'https://host': terminal prompts disabled`, exit 128 | — |

Conclusions, each load-bearing for a locked decision:

- **Setting `GIT_ASKPASS` does not suppress a configured `credential.helper`**
  (A, B, G). The helper is consulted first and the askpass program only asked
  for what the helper left unanswered (E). L7 holds as designed and B4 is a
  test of git's own precedence, not of a Cairn workaround. The stopping rule
  in phase 03 did not trigger.
- **`GIT_ASKPASS` takes precedence over `core.askPass`** (D2), so replacing the
  variable is enough; a `core.askPass` the user configured is overridden while
  Cairn runs git, which is the accepted collateral of L5 (recorded in
  `docs/systems/credentials.md`).
- A username and a password are two askpass invocations with one environment
  (C), which is why phase 02 scoped the token to the operation.
- With `GIT_TERMINAL_PROMPT=0` and nothing answering, git fails immediately
  and names what it could not read (F): the hang L3 exists to prevent does not
  occur.

## Addendum, 2026-09-17: O5 settled — `SSH_ASKPASS_REQUIRE` is OpenSSH 8.4

Source for the version: the OpenSSH 8.4 release notes
(`https://www.openssh.org/txt/release-8.4`, released 2020-09-27) list, under
new features, allowing "some additional control over the use of ssh-askpass
via a new $SSH_ASKPASS_REQUIRE environment variable, including forcibly
enabling and disabling its use." The `ssh(1)` page on the development machine
(OpenSSH 10.5p1) documents the three values: `never`, `prefer`, and `force`
("the askpass program will be used for all passphrase input regardless of
whether DISPLAY is set").

Behaviour, by test: `ssh-keygen -y -f <key>` over a freshly generated
passphrase-protected key (it reads the passphrase through the same
`read_passphrase` path `ssh` uses), a stub askpass recording each prompt,
`env -i`, no `DISPLAY`, and the process detached from any terminal with
`setsid` where noted:

| Case | Environment | Result |
| --- | --- | --- |
| A | `SSH_ASKPASS`, `SSH_ASKPASS_REQUIRE=force` | askpass asked (`Enter passphrase for "key": `), key read |
| A2 | as A, detached | same |
| B | `SSH_ASKPASS` only, detached | askpass **not** asked; the prompt went to the (absent) terminal and failed |
| C | `SSH_ASKPASS` and `DISPLAY=:0`, detached | askpass asked |
| D | `SSH_ASKPASS_REQUIRE=force`, no `SSH_ASKPASS` | ssh tried its default askpass program and failed closed |
| E | `SSH_ASKPASS`, `SSH_ASKPASS_REQUIRE=never` | as B |
| F | as A, askpass answering wrongly | failed with "incorrect passphrase", no retry |

And against a real `sshd` started unprivileged on a loopback port with a
passphrase-protected user key and `force`: the passphrase prompt reached the
askpass (`Enter passphrase for key '<path>': `), and so did the host-key
confirmation when `StrictHostKeyChecking=ask` — as one multi-line prompt
ending `Are you sure you want to continue connecting (yes/no/[fingerprint])? `,
which the askpass answered `yes` to and ssh accepted.

Runtime consequence: with OpenSSH 8.4 or newer, `SSH_ASKPASS_REQUIRE=force`
makes every passphrase and confirmation reach Cairn's helper whatever the
terminal and display situation, and Cairn need not (and does not) pass
`DISPLAY` through. On OpenSSH older than 8.4 the variable is ignored: the
prompt goes to the controlling terminal if Cairn was launched from one (an
invisible prompt, as before this packet), and with no terminal it goes to the
askpass only if `DISPLAY` is set — which Cairn's environment does not carry —
so it fails closed instead. Cairn does not check the OpenSSH version; 8.4 is
older than every git 2.30 distribution Cairn supports (L9: Debian bullseye
ships 8.4), so this is recorded rather than enforced.
