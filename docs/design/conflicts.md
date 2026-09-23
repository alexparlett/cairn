# Conflicts

Intent, not as-built. Spine: `docs/design/cairn.md`, where this is decision
**D6**.

Conflicts are resolved structurally, not textually: a three-way view — ours,
result, theirs — with *use ours*, *use theirs* and *use both* per conflict region,
and *edit in your editor* as the escape hatch for the messy remainder.

The line that keeps this compatible with Cairn not being an editor: structured
resolution picks between alternatives that already exist; an editor accepts
arbitrary text. Cairn does the first and hands off the second.

Rejected: simply opening the user's editor. Fork headlines a built-in resolver,
and delegating is materially worse at the single most painful moment in git. Also
rejected: full in-app editing — the best possible experience, and an open-ended
commitment to becoming an editor.

The cost: a conflict view is real work, and the escape hatch makes the external
merge-tool integration (`feature-inventory.md`, Tier 7) a dependency rather than
a nicety.

Conflict resolution is not in the first milestone (`cairn.md`, "The first
version worth having").
