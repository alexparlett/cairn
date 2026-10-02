# Worktrees

Intent, not as-built. Spine: `docs/design/cairn.md`, where this is decision
**D8**.

Worktrees are first-class: list, create, switch and remove them, and show which
worktree a branch is checked out in. They are cheap on the read side, unserved by
every mainstream client, and load-bearing for anyone running several branches at
once — or several agents — which includes the way Cairn itself is developed.

The failure they prevent matters even with read-only awareness: a client that
does not know about worktrees offers to check out a branch that is already checked
out elsewhere, and then fails confusingly. So in the interface they are a sidebar
section, and a branch checked out in another worktree carries a chip and a
disabled checkout (`ui.md`).

Writes in two worktrees of one repository share a local write lane, because refs
and objects are shared even where the index and `HEAD` are not
(`concurrency.md`). Whether staging in one worktree may run beside a commit in
another is the open question that design leaves here.

Rejected: read-only awareness alone, which avoids that failure but leaves the
workflow unserved; and ignoring worktrees, which is actively unhelpful for the intended
user.
