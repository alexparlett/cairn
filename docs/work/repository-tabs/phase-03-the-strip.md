# Phase 03 — Fork's tab strip

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/repository-tabs/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/repository-tabs before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/repository-tabs`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/detail/ (the detail pane's tab strip), style/metrics.rs, style/tones.rs, menu.rs, accelerators.rs and its pin, text_field.rs, the strip from 02, docs/research/repository-tabs/fork-observed-<date>.md, FT §1, §5, §6, and FR §1, §7; read state.md, this file, implementation-plan.md
        and the PRD sections and evidence named below directly; nothing else of the planning docs.
        Verify every Freya API you are not certain of in the vendored source
        (~/.cargo/git/checkouts/), never from memory.
STEP 2  Implement:
        1. The strip's look (R4.1-R4.3): crates/cairn-ui/src/tab_strip.rs, its metrics and tones
           in cairn_ui::style, equal widths to a minimum, the "+", the overflow list built with
           cairn_ui::menu; the headless test over 1, 4 and 30 tabs.
        2. The tab's controls (R4.4-R4.6), as R1 settled them: drag to reorder; and the ×, the
           middle press and the context menu drawn and reporting their intent (an EventHandler),
           which 04 wires to Close Tab.
        3. Names and marks (R4.7-R4.8): the pure names rule and its test; the mark from the
           tab's status (the waiting shape is drawn from a flag 05 sets); the status box, sidebar
           header and desktop title using the tab's name.
        4. NextTab and PreviousTab (R10.3's last two rows) in the accelerator table, heard from
           every text field, wrapping at the strip's ends; keyboard Tab never reaching the strip
           (R4.9). docs/systems/repository-tabs.md extended with the strip.
        Invariants in play: the style guards (no literal size or tone outside style/); the
        modifier guard (no literal modifier in a component); the accelerator pin (no chord twice,
        rule examples kept); the virtualization guard (no ScrollView on a render path).
        Out of scope: New Tab, Open, Close Tab, Restore and their chords (04); the waiting mark's
        cause (05).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build with several repositories open and
        drive the phase's surfaces (qa-checklist item 6). Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with the reviewers implementation-plan.md's dispatch
        table names for this phase, spawned fresh, plus the qa-checklist.md items and this file's
        QA brief. Adjudication goes to a fresh qa-confirm agent, never this session inline. Log
        dismissed findings with reasons in progress.md. Fix confirmed findings in focused
        commits. Disputed findings go to the user.
STEP 4  Acceptance: C7, C8's reorder half (`a_dragged_tab_takes_its_new_place`), C9, C10's status half, and C21's NextTab and PreviousTab rows with `next_tab_wraps_at_the_strips_ends` (docs/prd/repository-tabs.md).
STEP 5  Update state.md (interfaces introduced, test renames, validation row) and progress.md
        (newest first); save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/repository-tabs; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/repository-tabs, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when Fork's observed strip cannot be drawn within the style guard's rosters; a name cannot be shown whole within R4.3's rule at the minimum window size; or the observation contradicts T5 in a way R1 did not settle; otherwise do not stop for permission.
```

## QA brief

- Is every size and tone of the strip from cairn_ui::style, and every chord from the table?
- Does a tab dragged past the strip's ends, or onto the overflow button, land somewhere sensible, and is the new order what Ctrl+Tab follows?
- With 30 tabs at the minimum window size, is every tab one press away and every name whole somewhere?
