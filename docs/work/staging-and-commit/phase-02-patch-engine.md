# Phase 02 — The patch engine: inversions, untracked files, modes, quoting, renames

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phase 01's work is present at the integration tip.
        Direct integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-model/src/
        (patch.rs, patch_apply.rs, line_selection.rs, diff_text.rs,
        changed_file.rs, diff_rows.rs), crates/cairn-git/tests/diff/patches.rs
        (C1-C3's round trips), crates/cairn-git/src/diff/working_tree.rs and
        crates/cairn-git/src/reads/working_tree.rs (the staged side's
        --no-renames), scripts/git-floor.sh, docs/prd/staging-and-commit.md (R2,
        C3, C4, C7) and docs/research/staging-and-commit/patch-mechanics-spike.md.
        Do not read the other planning docs directly.
STEP 2  Implement.
        1. cairn-model: TextDiff::inverted, Selection::inverted and
           ChangedFile::inverted (R2.1), pure, each with tests; Selection's mode
           item, so a line selection emits no old mode/new mode and a mode
           selection emits only them (R2.4); C-quoted path lines exactly as
           git's quote_c_style (R2.5), tested for a space, a tab, a quote, a
           backslash, a newline, a control byte and invalid UTF-8; what is
           whole-file only refuses a partial selection by emitting nothing
           (R2.3).
        2. cairn-git: the staged side of the working-tree query pairs renames
           and copies as `git diff --cached` does under diff.renames (R2.6);
           unstaging lines of a staged rename emits a content-only patch at its
           new path.
        3. Tests — C3 in crates/cairn-git/tests/: for each case the PRD lists,
           stage, unstage and discard a selection of lines with real `git apply`
           (no Cairn verb yet; the test runs the command the PRD names), and
           compare the result with the reference applier's, with the mirrored
           rule's derivation applied in reverse, and with `git diff` /
           `git diff --cached` afterwards; on the host's git and on 2.30.9 and
           2.32.7. C4 and C7 likewise. Settle whether a `new file mode` patch
           applies against an intent-to-add entry (state.md's open question)
           and pin the answer.

        Invariants in play: cairn-model changes carry tests; DiffContent,
        UnifiedRow and SideBySideRow are read by naming every variant (a mode
        row, if this phase adds the row kind, is named everywhere); the staged
        query stays a read; git parity is critical (program memory).

        Out of scope: any ops/ verb (03), the stale check (03), drawing the mode
        row (08).
STEP 3  Validate: scripts/gate.sh (git-floor included). Then orchestrate this
        phase's QA in this session: /qa over the phase diff with the reviewers
        implementation-plan.md names for phase 02, spawned fresh, plus the
        qa-checklist.md items this phase covers and the QA brief below.
        Adjudication goes to qa-confirm (fresh), never this session inline; log
        dismissed findings with reasons in progress.md; fix confirmed findings
        in focused fixes; disputed findings go to the user.
STEP 4  Acceptance: C3, C4, C7.
STEP 5  Update state.md and progress.md; docs/systems/diff.md (inversion, the
        mode item, quoting, partial untracked files, the staged side's renames).
        Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if any floor git applies a case
differently from the host's; if the staged side's renames cannot match
`git diff --cached` without a new porcelain read; if a case needs -R after all.
Otherwise do not stop for permission.
```

## QA brief

The risk is a round trip that passes because both sides were derived the same
way.

- The mirrored-rule derivation must be written independently of the inversion
  (a reviewer should be able to delete either and see the other still decide).
- Each C3 case must end in a check against what `git diff` prints, not only
  against the applier.
- Quoting: apply a patch for each awkward name with real git; a test that only
  compares strings against Cairn's own quoter proves nothing.
- The mode item: stage a line of a file whose mode also changed and read the
  index entry's mode back from git.
