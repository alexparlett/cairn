# Progress — daily-loop

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-14 — out-of-scope list reviewed against real usage and confirmed

Walked the remaining out-of-scope entries with the user after D9 moved one of them
into scope. No further changes: platform panels, issues, CI status, repository
creation, being an editor, git-flow, a tutorial, Windows, bisect and filter-repo
all hold.

Worth recording because it changes the list's status. These were my assertions
from a design principle; they are now checked against what the user actually does
in a day, which is how D6 and D9 were both found to be wrong. Reopening one should
need new information rather than a fresh opinion.

## 2026-09-14 — forge links pulled into scope; the spine's line was wrong

The user reported that "create pull request on origin" is one of their most-used
Fork context-menu commands. The spine had ruled the whole platform surface out on
the grounds that a half-implemented GitHub panel is worse than a link — which
conflated two unrelated things. Creating a pull request needs no API and no token;
it is URL construction from the branch, its upstream and the remote URL.

Recorded as D9, with the line redrawn mechanically: anything that is "open the
correct forge URL" is in scope, anything needing an API token is out. That brings a
family of siblings nearly free — open a commit, branch, tag or file in the browser,
copy a permalink to a selected line, open a compare view.

CI status stays out and now has a better reason than "expensive": it needs a
per-forge token, which would make Cairn a credential holder, and D2's premise is
that it never is. Inconsistent with a decision already taken, not just costly.

Lands in packet 6 alongside push, because the pull request is what a user wants
immediately after pushing a branch. New open question O6: how a self-hosted forge
is identified, and whether push-and-create-PR is one action or two.

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
