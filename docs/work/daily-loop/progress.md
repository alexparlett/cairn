# Progress — daily-loop

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-14 — program planned from a feature analysis

Inventoried the full feature surface against Fork's published feature list
(`docs/design/feature-inventory.md`, ~69 items) and turned it into a build order.
Three decisions came out of it and went into the spine as D6, D7 and D8.

D6 revised something the spine had wrong. It said conflict resolution would open
the user's editor; Fork headlines a built-in resolver, and delegating is worst at
the moment users most need help. The middle position — structured per-region
resolution, no free-text editing — keeps "not an editor" while getting most of the
value, and it was not considered the first time.

Two structural findings shaped the sequence more than the feature list did. The
diff model has nine consumers and must be patch-capable from the start, or
line-level staging becomes a rewrite (L2). And recovery splits into two classes
with different stories: the reflog covers destroyed commits, and nothing covers a
discarded uncommitted edit — which is why the reflog view is pinned to the first
commit-level destructive operation (L4) and why the auto-stash question exists at
all (L5).

Also settled: `credential-prompts` is load-bearing for staging rather than just
early, because `git apply --cached` runs through the backend it lands (L3).

No implementation has started. Two packets are filed; six exist as briefs.
