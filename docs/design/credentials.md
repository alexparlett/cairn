# Credentials

Intent, not as-built; `docs/systems/credentials.md` describes what exists. Spine:
`docs/design/cairn.md`, where this is decision **D2**.

## Cairn holds no credential

Cairn stores no credential, integrates no keychain and implements no auth. Fetch
and push run through `git` (`engine.md`), so git invokes the user's configured
`credential.helper` and their SSH agent, and Cairn inherits whatever already works
for them — `osxkeychain` on macOS included. Anything that would need Cairn to hold
a token is out of scope for that reason (`forge-links.md`).

## Prompts reach the window

The part Cairn builds is the interactive case. A GUI has no terminal, so git's
own prompt would hang the window on nothing and ssh would ask for a passphrase on
a tty nobody is watching. Every `git` Cairn runs therefore gets an environment
Cairn built, never one it inherited: `GIT_TERMINAL_PROMPT=0` stops git falling
back to a tty, and `GIT_ASKPASS` and `SSH_ASKPASS` (with
`SSH_ASKPASS_REQUIRE=force`) name Cairn's own askpass helper. git calls the helper
with the prompt as an argument and reads the answer from its stdout; the helper
round-trips the prompt to a dialog in the running window. Evidence:
`docs/research/credential-prompts/git-credential-delegation.md`.

While Cairn runs `git`, Cairn's helper is the askpass, even for a user who has set
`GIT_ASKPASS`, `SSH_ASKPASS` or `core.askPass` for something else: a prompt must
reach the window rather than a program with no window to reach. Honouring the
user's askpass instead is rejected, because a program chosen for a terminal may
itself expect one, and a fetch that hangs on it is the failure this design exists
to prevent. `credential.helper` and the ssh-agent are untouched — git consults
them before it ever asks — so a setup that answers without prompting keeps
answering. A user who needs their own askpass program gets it through an explicit
"auth provider" setting (issue #22), not by inheritance.

## Where a secret exists

Inside the helper process, on git's stdin, and once in the application: the
dialog hands it to a worker thread, which writes it to the helper's socket and
drops it. It is never application state. The one type that holds it is
`cairn_model::Secret`, which cannot be printed, cloned or serialised and zeroes
its memory on drop; its guard is what keeps that single passage from becoming
state. Spec: `docs/prd/credential-prompts.md`.
