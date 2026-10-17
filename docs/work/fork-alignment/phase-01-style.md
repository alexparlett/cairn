# Phase 01 — the style module

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing.
        This is phase 01: if feature/fork-alignment does not exist, first create it from
        main (history-topo-order must have merged; check `git log main` for it, and stop and
        ask if it has not) in its own worktree and push it.
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src (every file that sets a font size or a colour), crates/cairn-app/src/main.rs (the launch theme), crates/cairn-ui/src/diff/palette.rs, history/lane_palette.rs, the vendored Freya theme and focus APIs (~/.cargo/git/checkouts/freya-*/caa46f8/), crates/cairn-guards/src/lib.rs and tests/invariants.rs (how a roster guard with a self-test is written), deny.toml; do not read the planning docs beyond
        state.md and this file directly (the PRD sections and evidence named below are the
        exception: read those).
STEP 2  Implement:
        1. `cairn_ui::style` (`style.rs` + `style/metrics.rs`, `style/tones.rs`,
           `style/icons.rs`) holding PRD R1.2's metrics and R1.3's tones exactly; the launch
           theme built from `dark_theme` with them (R1.4); every existing view re-toned through
           it, layout unchanged (R1.1, R1.6); no component left on Freya's 16 px default.
        2. Lucide through Freya's `icons-lucide` feature on `cairn-ui`, named only in
           `style/icons.rs` (R1.5); `deny.toml` `[[bans.features]]` for `freya`, `exact`.
        3. The focused and unfocused selection on every list: the history, the sidebar, both
           Local Changes lists, the Changes tab's files (R1.3, PRD C4).
        4. The guards `chrome_sizes_and_tones_come_from_the_style_module` and
           `every_icon_is_named_through_the_icon_module`, each with its self-test and a
           nonzero-files assertion, and both invariants added to the root CLAUDE.md with their
           twins in the same commit (implementation-plan.md, "Guards added").
        Invariants in play: each crate's allowlist (`layer_dependencies_are_allowlisted`), the ScrollView ban and the virtualization twins, the modifier guard (no chord spelled in a tooltip), the comment-history guard, a test in the same commit for any cairn-guards change.
        Out of scope: any layout or position change (phases 02-08); the caption row; dates; the history's header.
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build over the fixture
        (docs/research/ui-review/fixture-recipe.md) at 1024x768 and compare against FS1/FS2's
        shots (qa-checklist item 7). Then orchestrate this phase's QA in this session: /qa over
        the phase diff, with the reviewers implementation-plan.md's dispatch table names for
        this phase, spawned fresh, plus the qa-checklist.md items and this file's QA brief.
        Adjudication goes to a fresh qa-confirm agent, never this session inline. Log dismissed
        findings with reasons in progress.md. Fix confirmed findings in focused commits.
        Disputed findings go to the user.
STEP 4  Acceptance: C1, C2, C3, C4 (docs/prd/fork-alignment.md).
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md (newest first);
        save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/fork-alignment; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/fork-alignment, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when Freya's focus model cannot tell a list's focus without a render effect; a tone in R1.3 does not exist as a Freya theme key and would need a component override per use; the exceptions roster needs a row beyond the two palette files; or enabling `icons-lucide` adds any crate to Cargo.lock but `freya-icons` (L10 approved that one; record Lucide's ISC licence in `deny.toml` beside the pin); otherwise do not stop for permission.
```

## QA brief

- Does every tone in `style::tones` equal the PRD table, and does any view still read a stock
  Freya tone for something the table names (grep the theme reads CI listed)?
- Does the literal matcher catch the shapes it claims: an aliased `Color`, a `from_rgb` split over
  lines, a `"#…"` string? And does each `STYLE_EXCEPTIONS` row fail once it is no longer needed?
- Is the selection's focus read from Freya's focus, or tracked by hand in a way a second list
  could break?
- In the real window, are the menus, tooltips and buttons 12 px?
