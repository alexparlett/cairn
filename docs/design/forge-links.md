# Forge links

Intent, not as-built. Spine: `docs/design/cairn.md`, where this is decision
**D9**.

## The line

Anything that is "open the correct forge URL" is in scope. Anything that needs an
API token is out. The line is mechanical rather than a judgement call.

In: create a pull request for the current branch against its upstream default,
open a commit, branch, tag or file in the browser, copy a permalink to a selected
line, and open a compare view between two refs. It is all one mechanism — read
the branch and its upstream, read the remote URL, identify the forge, construct a
URL, hand it to the system opener. No token, no network call from Cairn, no state
to keep in sync, nothing to go stale.

Out: reviewing pull requests, reading or filing issues, CI status, and creating
repositories on a platform.

A half-implemented forge panel is worse than no panel, but a link is not a panel.
"Create pull request on origin" is one of the most-used context-menu commands in
Fork, and the thing a user wants immediately after pushing — so it is designed
together with push, and the first milestone cannot be met without it.

CI status stays out for its own reason, because it is the most tempting thing on
the far side of the line: it needs a per-forge API token, which would make Cairn a
credential holder, and `credentials.md` rests on Cairn never being one.

## Where they appear

Branch and commit context menus carry the forge items; after a push, the
notification offers *Create pull request* directly (`ui.md`).

## Which forges

github.com and gitlab.com are identifiable from the hostname; self-hosted GitLab,
Gitea and Forgejo are not. The forge table is data, not code, so adding a forge is
an entry, and an unrecognised remote gets no menu item rather than a guessed URL
and a 404.
