---
name: review-pr
description: Review a GitHub pull request on this repo maintainer-style and post the review as a plain comment. Trigger on "review PR <number>", a PR URL, or "review that pull request". Distinct from /qa, which reviews the local working diff.
user-invocable: true
---

# /review-pr

Maintainer-style review of a GitHub PR, posted as a comment. `/qa` reviews the
local diff pre-PR; this reviews someone's (or some session's) submitted PR.

## Step 1: Gather

- `gh pr view <n>` and `gh pr diff <n>` on the canonical repo, never a fork
  clone.
- `gh pr checks <n>` for CI state; a red gate is finding number one.
- Scope the true diff against its merge base; note anything the PR description
  promises that the diff does not contain, and vice versa.

## Step 2: Domain passes

Run the reviewer dispatch table from `docs/qa-gate.md` over the diff surface:
spawn the matching domain reviewers FRESH and in parallel, plus `qa-checklist`
for the overall contract. Read their reports; you own the synthesis.

**Distinguish what the diff CHANGED from what it inherited.** Pre-existing debt
adjacent to the change is a note, not a blocking finding against this PR.

## Step 3: Confirm before you post

Independently confirm every consequential or negative finding against the code —
reviewers over-report by design. Merge duplicates; the strictest severity wins.
Drop anything you cannot evidence with a file:line.

## Step 4: Post

One plain comment (`gh pr comment`): verdict first (mergeable as-is / mergeable
after items / not mergeable), then numbered findings most severe first, each with
evidence and the concrete resolution. Praise is fine when specific; filler is
not. Never approve, request changes, or merge — the comment is the deliverable,
and merging is the user's decision alone.
