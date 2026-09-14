---
name: extract-and-test
description: The mechanics behind the Modularity section of CLAUDE.md — when code is a sibling module vs a coordinator edit, and the test-first bug-fix recipe. Read before extracting a module, adding a subsystem, changing a guard rule, or fixing a bug.
user-invocable: true
---

# Extract and test

## The one decision

Does this code need the coordinator's private mutable state? If no, it is a
sibling module, every time — a new module beside the coordinator, wired in by one
line. Entry-point files are firewalls, not homes.

- Name for the behavior, not the layer: `mapgen`, `retry_policy`,
  `session_cache`; never `helpers`, `utils`, `misc`.
- Data-as-code (content tables, fixtures) is exempt from size pressure; logic is
  not.
- Extract on the rule of three, not before. Never abstract for one use or a
  hypothetical future need.

## The file ratchet (if adopted — SETUP.md §7)

Every walked file gets a pinned line-count ceiling; growth past it fails the
guard step. After extracting, lower the ceiling in the same change. A raise is
legal exactly when the baseline diff is visible and the user approves it in
review. Beware the rename reset: a move-plus-grow bypasses the reviewed-raise
path, so a disappearing baseline key in a diff is a finding.

## The test-first bug-fix recipe

1. **Reproduce with a failing test on the real code path** — not a mock of it,
   not a re-statement of the expected value. The test must fail for the reason
   the bug exists.
2. Make the SMALLEST change that turns it green.
3. State the mutation the new test now catches. If you cannot, the test decides
   nothing — rewrite it before shipping.
4. Run the affected suite, then `scripts/gate.sh --fast`.

## Traps that recur (learned the hard way)

- A helper that makes test cases convenient makes them UNIFORM: add positives
  for shapes the helper cannot produce.
- Pinning a predicate says nothing about its call sites; check the fixture can
  produce the negative through the real entry point.
- Two fixtures identical in structure can make an ordering property hold by
  construction; you need a third element arranged to break it.
- A fixture that hand-supplies what production computes asserts the fixture, not
  the code; call the real accessor and add a paired positive.
