# Worktrees

Intent, not as-built. Spine: `docs/design/cairn.md`, where this is decision
**D8**.

Worktrees are first-class: list, create, switch and remove them, and show which
worktree a branch is checked out in. They are cheap on the read side, unserved by
every mainstream client, and load-bearing for anyone running several branches at
once — or several agents — which includes the way Cairn itself is developed.

In the interface they are a sidebar section, and a branch checked out in another
worktree carries a chip and a disabled checkout (`ui.md`).

That last part matters even before anything else: a client that does not know
about worktrees offers to check out a branch that is already checked out
elsewhere, and then fails confusingly.

Rejected: read-only awareness, which avoids that failure but leaves the workflow
unserved; and ignoring worktrees, which is actively unhelpful for the intended
user.
