---
name: file-issue
description: Capture a bug, feature request, or design question as a house-format issue. Files to GitHub via gh when a remote exists, otherwise to docs/backlog/. Use when the user reports a problem or idea that should not be fixed in the current change, or when review surfaces out-of-scope work worth keeping.
user-invocable: true
---

# /file-issue

Turn a report or idea into a well-formed issue instead of a drive-by TODO. One
issue per problem; if the input bundles several, say so and file them separately.

## Step 1: Classify

- **Bug** — behavior diverges from design or invariant.
- **Feature request** — new capability or content.
- **Design question** — a decision the user must make; the issue frames options,
  it does not decide.
- **Harness/tooling** — gates, hooks, reviewers, scripts.

## Step 2: Compose, house format

Always these sections, in this order:

```
## Problem
<what is wrong or missing, one paragraph, no solutioning>

## Expected behavior
<what should happen instead; cite the design doc or CLAUDE.md invariant if one applies>

## Evidence
<file:symbol anchors, failing command output, or the review finding — enough for a
fresh session to reproduce or locate it>

## Proposed direction (optional)
<options with trade-offs; for design questions, ALWAYS this section, with a
recommendation>
```

Title: imperative, specific, no ticket-speak. Label with the classification.

## Step 3: File

- Remote exists: `gh issue create` with title, body, and labels. Report the URL.
- No remote: write `docs/backlog/<slug>.md` with a `status: open` header and the
  same body. Backlog entries are reviewed for promotion whenever a remote
  appears.

## Rules

- Never file an issue as a substitute for fixing something in scope of the
  current change — issues are for OUT-of-scope work worth keeping.
- A QA finding that touches the in-flight work goes to the user as a question,
  not to the tracker.
- Anchor rule applies: stable paths and symbols, no line numbers.
