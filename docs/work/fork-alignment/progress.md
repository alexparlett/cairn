# Progress — fork-alignment

Running log, newest first. Each phase's dismissed QA findings are logged here with their
reasons, for the final QA phase to audit.

## 2026-10-19 — phase 01, the style module (packet mode)

Built on `feature/fork-alignment`, cut from `main` at 934d5e1 (history-topo-order, #142).

- `cairn_ui::style` (`metrics`, `tones`, `icons`) holds R1.2's sizes and R1.3's tones, the
  launch theme is `tones::theme()` (Freya's `dark_theme` over Fork Dark's sheet), every view is
  re-toned through it with its layout unchanged, and the body size (12 px) is set at the window
  root. Text that was 13 px is now the body's 12 px; other sizes kept and moved to `metrics`.
- A selection is drawn by its list's focus, read from Freya's `use_focus` (no render effect,
  so the stopping rule did not trigger): the history, the sidebar (now focusable, taking focus
  on a press), both Local Changes lists, the Changes tab's files and the Commit tab's files.
- Lucide through `icons-lucide`; Cargo.lock gains `freya-icons` alone (stopping rule not
  triggered); `deny.toml` pins freya's features exactly and records Lucide's ISC licence.
- Guards `chrome_sizes_and_tones_come_from_the_style_module` and
  `every_icon_is_named_through_the_icon_module`, with self-tests; `STYLE_EXCEPTIONS` holds only
  the two palette files the plan expected.
- The diff palette no longer retunes: Cairn's diff ground is Fork's `#282828`, so the bands are
  Fork Dark's values (R7.5 met here). `ADDED_EMPHASIS` stays Fork's `#388442` less 20 a channel:
  at Fork's value the text reads under WCAG AA. Chips: tag and stash by kind, branch and remote
  bordered by lane and filled with a dark shade of it.
- C1 `chrome_sizes_and_tones_come_from_the_style_module` (+ self-test); C2
  `every_icon_is_named_through_the_icon_module` (+ self-test) and `gate.sh --step deps` — the
  `borderless` arm and `freya-borderless-plugin` are phase 02's (exact refuses an allowed feature
  not enabled); C3 `the_style_module_holds_forks_metrics_and_fork_darks_tones`; C4
  `a_selection_is_blue_while_its_list_has_focus_and_grey_when_not`.

**Real window** (release build, the fixture of `fixture-recipe.md`, Hyprland, 1024×768, driven
with `wtype` and `hl.dsp.send_shortcut`): All Commits, a row selected with ↓ (Fork's blue,
white bold text, as uw3), focus moved with Tab (grey `#525252`, bold, as uw4), Create Branch by
Ctrl+Shift+B (`#2B2B2B` ground, `#0078D7` focused field, disabled Create `#343434`/`#707070`,
buttons at 12 px), the Changes tab by Ctrl+Alt+2 (file list `#212121`, chosen file grey, diff
`#282828`, added band `#3A5C3F`, hunk header `#959595`, chosen tab in blue, as uw5). Surfaces
read as Fork Dark's: chrome `#1A1A1A`, sidebar `#2C2C2D`, history `#212121`, detail `#333333`.
Differs from FS1/FS2, each a later phase's: the splitters are Freya's ~4 px handles in `#525252`
where Fork draws 1 px rules (03); the history keeps its header, 26 px rows and UTC dates (04);
dialog titles are Freya's 18 px `PopupTitle` and the dialog has no `#3BACED` border (08, R11);
the sidebar keeps its 14 px indent and has no header (05). Found in the window and fixed: a
dialog's text field drew at `PopupContent`'s 15 px. **Not verified:** menus and tooltips by eye
— no pointer reached the window (`hl.dsp.cursor.move` warps without a hover reaching it, and
`xdotool` serves XWayland only), and no menu opens from the keyboard yet; their 12 px rests on
the root size and the tooltip theme, pinned by `a_text_given_no_size_is_drawn_at_the_bodys` and
`the_launch_theme_carries_fork_darks_tones_into_freyas_components`. The minimum size was not
checked: the window has none until phase 03 sets it.

**QA.** Fresh `qa-checklist`, `responsiveness-reviewer`, `test-coverage-auditor` and
`gate-integrity-reviewer`; `qa-confirm` adjudicated 18 findings (22 raw, 4 merged). Fixed: the
invariant's claim narrowed to font sizes and tones with layout sizes stated unguarded (A); menus
take `MENU_SHADOW` (B); the icon test's comment moved (E); disabled buttons take FS2's `#383838`
border (G); a fractional-alpha tuple is caught, and the unseen routes — a computed tuple, a
`u32` or variable colour, a literal `with_a`, a size through a binding or a parameter — are
stated residuals (H, I, J); a blind-matcher pin for the size half (K); exceptions excuse tones
only (L); the residual's owner is the permanent `qa-checklist` item 1 (M); window tests pin the
history rows' focus wiring and the root's body size (N, O). Deferred: dialog titles at 18 px
(C → phase 08); C2's `borderless` (D → phase 02); after a press the sidebar holds focus with no
keys, so ↑/↓ reach no list until the history is pressed (Q → phase 05, C14, which gives the
sidebar Fork's ↑/↓).

Dismissed, with reasons:
- F, "user-approved, fork-alignment L10" in `crates/cairn-ui/Cargo.toml` is history: the
  manifest records every dependency's approval this way (its other two comments), the
  comment-history guard reads `.rs` comments only, and the meta-invariant wants each
  dependency's reason recorded.
- P, C4 never moves focus straight between Unstaged and Staged: each list reads its own focus
  node and a `LocalSelection` holds one list, so pressing the other list moves selection and
  focus together; nothing hand-tracked could carry a stale tone.
- R, the icons' `SvgViewer` cost: no view draws an icon yet; the phase that first does (03, the
  toolbar) checks it.

Batched for the user (PENDING, none approved): the lockfile's new crate `freya-icons` and the
`deny.toml` freya pin (L10 decided the crate; the pin's exact list is new); the extra FS2 tones
beyond R1.3's table; `ADDED_EMPHASIS` kept off Fork's value for contrast; the Lucide ISC notice
a package of the binary must carry beside it.

## 2026-10-16 — planned

Planned with `/feature-plan`.

Evidence in `docs/research/ui-review/`:
- two first-hand Fork for Windows 2.23.2 surveys (light, then dark), at 1024×768 over the fixture
  in `fixture-recipe.md`;
- Cairn's window over the same fixture;
- Cairn's UI as built;
- the earlier Fork evidence graded;
- the Freya chrome APIs.

The user locked S1-S5 and L1-L9 (`brainstorm.md`). They rejected the recommendation once, on the
menu bar: they chose Fork's caption row over the OS frame.

The surveys also found a git-parity bug in the graph's order. It became packet 5c,
`history-topo-order`, which runs first.

The design docs were rewritten to this end state:
- `docs/design/ui.md`: the chrome, the deviations, the sidebar, Local Changes, Show Lost Commits,
  the palette and type;
- `history-graph.md`: columns, `HEAD`'s reach, the stash lane;
- `diff.md`: the detail pane, avatars, the diff bar.

The roadmap gained 5c-5h.
