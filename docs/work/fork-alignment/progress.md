# Progress — fork-alignment

Running log, newest first. Each phase's dismissed QA findings are logged here with their
reasons, for the final QA phase to audit.

## 2026-10-20 — phase 03, the toolbar, the status box and the window's regions (packet mode)

Committed onto `feature/fork-alignment` over phase 02's f998176.

- `title_bar` is now `toolbar`. No guard named the old path, so there was no guard path to move
  or to show failing.
- `cairn_ui::Toolbar`: Fork's buttons in Fork's order, each an icon over its label.
  - Quick Launch, Pull, Push, Stash (with its ⌄) and Appearance are greyed with L2's sentences
    and report nothing to a press or a key.
  - Fetch and Branch go through the menu bar's refusals and `shortcuts::act`.
  - Branch's ⌄ is a menu of New Branch… alone.
  - The sidebar toggle hides and shows the sidebar.
  - Badges on Push and Pull show the commits ahead and behind, absent at zero.
- The status box sits in the toolbar's middle, Fork's 300 px, with Branch beside it.
  - It draws the repository's name, bold, over the branch, with git's words for a detached or
    unborn `HEAD` and for "upstream gone".
  - Only its list glyph opens the popover.
  - A running operation takes the repository's place.
  - The title row's count and star are gone.
- **Stopping rule hit, and decided by the user: option A.** git's longer progress lines (about
  290-360 px) don't fit the box's 248 px of text room at any window size. The box's second line
  now shows the line up to and including its percentage.
  - A line with no percentage is kept up to its first comma, or whole if it has none. I chose
    that rule.
  - The whole line stays in the activity popover.
  - An operation name wider than the box is cut with "…". That cut is a `ui.md` row.
- The regions:
  - The banners stand under the toolbar.
  - The history and the detail pane split the height half each and grow together. A drag moves
    the proportion (`PaneSplit`).
  - Fork's "Hide commit details" icon replaces Collapse, lit while the pane is hidden.
  - The subject column gives way first as the window narrows.
- Ctrl+Shift+D is ToggleDetails on Linux, and Discard keeps Backspace and Delete. macOS keeps ⇧⌘D
  for Discard and gives ToggleDetails no chord.
- The window opens at 1024×768 and has a minimum of **880×620**, measured headlessly and in the
  real window. The toolbar needs about 780 px. At 880 px the history keeps 218 px for a subject.
  At 620 px the pane's even share (276 px) reaches Expand All. Recorded in `window-frame.md`.
- Acceptance criteria and the tests that decide them:
  - C7: `the_toolbar_is_forks_with_the_unbuilt_buttons_greyed`,
    `ahead_and_behind_are_badges_on_push_and_pull`,
    `fetch_and_branch_are_greyed_with_their_reasons`,
    `the_greyed_sentences_and_the_icons_name_are_the_specs_words`,
    `each_refresh_redraws_the_badges_on_push_and_pull`,
    `the_toolbars_branch_and_fetch_act_and_say_why_when_they_cannot`.
  - C8: `the_status_box_names_the_repository_over_the_branch`,
    `the_box_draws_the_name_bold_over_the_branch`,
    `the_status_box_shows_an_operation_only_once_it_has_run_250_ms` (kept, now checking the name
    over the progress line in the repository's place),
    `a_long_progress_line_is_kept_to_its_percentage_and_only_an_overlong_name_is_cut`,
    `a_progress_line_is_kept_to_its_percentage`,
    `the_status_box_opens_the_operations_with_their_git_and_no_token` (the glyph opens it, the
    centre does not), and `progress_and_remotes_land_in_their_states` (the line is kept whole).
  - C9: `the_detail_pane_grows_with_the_window`,
    `expand_all_is_visible_at_the_minimum_size_and_at_1024_by_1200`,
    `hiding_the_details_leaves_the_tab_strip`, `the_banners_stand_under_the_toolbar`,
    `the_subject_column_shrinks_first`, `the_window_keeps_every_region_whole_at_its_minimum_size`,
    `a_drag_against_the_panes_floor_keeps_the_proportion`,
    `the_sidebar_toggle_hides_and_shows_the_sidebar`.
  - C18 (the Ctrl+Shift+D half): the table's rows test and its pin (the new Shift+D example), and
    `the_menu_bar_holds_only_what_works_in_forks_order`.
- Tests whose expectations changed:
  - The fetch test now presses the toolbar's Fetch.
  - The tab-order test makes one Tab stop fewer: the strip's icon is no Tab stop.
  - `a_text_given_no_size_is_drawn_at_the_bodys` now measures the caption's title and the root's
    size.
  - The popover-position test allows the panel to move left to fit, since the box is centred.
  - The no-chord test presses outside the dialog on a row's date.
- Deferred check: a Lucide icon rasterizes once per icon, size, tone and scale factor (the
  `AssetCacher`). After that a re-render is a hash and a lookup (responsiveness-reviewer).

**Real window** (release build, the fixture, Hyprland, session locked; captured with `grim -T`,
driven with `hl.dsp.send_shortcut`, resized with `hl.dsp.window.resize`).
- Verified:
  - At 1024×768 the toolbar matches FS2's uw3b: the status box at x 367-667 against Fork's
    362-662, the Push badge "1", greyed labels, Branch beside the box, Appearance at the right.
  - The pane is half the main region, as in uw3.
  - Ctrl+0 selects HEAD, and the Commit tab draws down to Expand All.
  - Hyprland holds the window at 880×620 when asked for 700×500.
  - At 880×620 Expand All is visible at the pane's bottom.
  - Ctrl+Shift+D hides the pane to its strip with the icon lit blue (Fork's uw41), and brings it
    back at its share.
- Differences, and why:
  - Fork's tab strip and .gitignore banner are not drawn: no tabs (L8), no suggestion banners
    (`ui.md`).
  - Fork's Open in, Console and Work are not drawn (L2).
  - The history header and its Show Lost Commits box still stand. They overlap at 880 px, and
    phase 04 removes the header.
  - The splitters are still 4 px handles (below).
- UNVERIFIED (no pointer reaches a Wayland window from this host): tooltips, button presses, the
  Branch ⌄ menu, hover boxes and splitter drags in the real window. These are covered headlessly.
  macOS is unverified too: nothing was built for it.

**QA.** Fresh reviewers ran in the foreground: `qa-checklist`, `responsiveness-reviewer`,
`test-coverage-auditor` and `gate-integrity-reviewer`. Responsiveness and gate-integrity found
nothing; the others raised 14 findings between them. `qa-confirm` adjudicated all 14:
- Confirmed and fixed:
  - Q1: diff.md said there was no collapse chord.
  - Q2: a one-sided drag weighed pixels against the default share. Probed: at the floor only one
    panel reports. `PaneSplit` now keeps the pane's share of the measured room, pinned by
    `a_drag_against_the_panes_floor_keeps_the_proportion`.
  - Q4: the toolbar's paddings and radii now come from `style::metrics`.
  - Q6: the real window is recorded above.
  - T1: the tests now hold the pane at exactly half the room at the minimum height. Shown to fail
    with the minimum set to 880×450.
  - T2: an overlong name must stay one line, and a short one its whole width.
    - Deleting `max_lines` alone doesn't wrap in this Freya: the ellipsis keeps one line.
    - Deleting both `max_lines` and the ellipsis fails the test.
  - T3: the whole progress line is kept in the fetch state and the activity entry.
  - T4: the toolbar's Branch and Fetch reasons are tested through the window.
  - T5: the banner must also stand above the main region.
  - T6 in part: the stale menu-bar test doc is fixed. The sentences are now spelled out apart
    from the constants.
- Dismissed, with reasons:
  - Q3: Ctrl+Shift+D can't discard. Linux Discard holds only Backspace and Delete, and the
    resolver tests pin each chord to its action.
  - Q5: the status box glyph was no Tab stop before this phase either. Fork's toolbar isn't
    reached by Tab (FS2 §3), and focus at every stop is phase 08's (C19).
  - R1: the root already re-rendered per fetch progress line before this phase.
  - R2: the toolbar's handlers capture only Copy state handles and the window's one submitter.
  - T6's duplicate assertion: redundancy is not a defect.

Batched for the user (PENDING, none approved):
- Draw the splitters as Fork's 1 px rules? Freya's `ResizableHandle` has no separate hit zone,
  so a 1 px rule is also a 1 px grab target. Kept at 4 px for now.
- The minimum size 880×620 and the opening size 1024×768.
- New metrics and tones:
  - the status box's #303030 and the badge's #606060, both sampled in FS2;
  - the toolbar's 20 px icons, its 44 px least button width, its 14 px group gap.
- The status box keeps its 300 px at every width the minimum allows. Fork's shrinks and overlaps
  Push below about 800 px.
- The no-percentage rule for progress lines (up to the first comma).
- The Lucide icons chosen for Fetch, Pull, Push, Branch, Appearance and the list glyph.
- The pointer and macOS checks above, UNVERIFIED.

## 2026-10-19 — phase 02, the caption row, the menus and About Cairn (packet mode)

Committed onto `feature/fork-alignment` over phase 01's 082b3b6.

- `cairn_ui::menu`: one component that draws every menu from data. It draws groups split by
  rules, chords from `chord_name`, greyed items with their reason, the ✓ gutter and submenus.
  It walks by Fork's keys and holds the keyboard while open. The three context menus moved onto
  it with their items unchanged. A greyed item's reason is now drawn under it while the pointer
  is on it, where it used to be a second row: Freya's tooltip hides while a context menu is open.
- `cairn_ui::CaptionRow`: Fork's caption row on Linux, the same menus as a row on macOS. Each
  frame gesture goes to `crate::frame::ask` as a `FrameRequest`. Decorations are off and
  `BorderlessPlugin` handles the resize edges.
- The menu bar holds exactly R2.5's items, in Fork's order and groups (`crate::menu_bar`).
- Seven new accelerator actions; Show Lost Commits' chord now works window-wide.
- About Cairn is the last of the one modal state. `Update::GitFound` reports the git that was
  found.
- `deny.toml` pins `titlebar` and `borderless`; the lockfile gains `freya-borderless-plugin`.
- Acceptance criteria and the tests that decide them:
  - C5: `the_caption_row_draws_the_menus_the_title_and_the_controls`,
    `a_drag_on_the_caption_row_moves_the_window`.
  - C6: `the_menu_bar_holds_only_what_works_in_forks_order`, `the_menus_walk_by_key_and_hover`,
    `the_window_title_names_the_repository`, `zoom_scales_the_window_in_steps`.
  - C20 (About half): `about_cairn_names_the_version_and_the_git`.
  - C2 (borderless arm): `gate.sh --step deps`.

**Fork observed first.** `/observe-fork` checked these, all VERIFIED
(`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`):
- Hide Commit Details and Show Lost Commits show a ✓ while on, with their labels unchanged; the
  ✓ gutter shifts every label by 18 px.
- Hovering switches between menus, and ←/→ wrap. A menu opened by a press highlights nothing
  until ↓ is pressed. That differs from R9.1's literal "first item focused on open": context
  menus do highlight their first item, but the bar follows Fork.
- About Fork offers only a ✕ and ignores Escape. Cairn keeps R11.2's Close and Escape; the
  deviation row is in `ui.md`.
- The caption row maximizes on a double click and opens the system menu on a right click.

**Real window** (release build, the fixture, 1024×768). The session was screen-locked for most
of the run; the window was captured from its own surface (`grim -T`) and keys were sent with
Hyprland's `send_shortcut`.
- Verified:
  - The caption row draws the menus, "Cairn" centred and three controls, matching FS2's band of
    #1A1A1A.
  - The OS title reads `t — Cairn`.
  - Ctrl+1, Ctrl+2, Ctrl+0 and Ctrl+Shift+. act. Ctrl+= and Ctrl+- scale the window in steps:
    1.1 and 1.5 were measured from the row band.
  - Decorations, on Wayland: `WAYLAND_DEBUG=1` shows winit binding `zxdg_decoration_manager_v1`
    and sending `set_mode(client_side)` before the window first shows; Hyprland answers
    `configure(server_side)`.
  - The host's `hyprbars` draws no bar on Cairn. Its source hides the bar for a window whose
    latest request was client-side. Seen on screen once the session was unlocked: nothing above
    the caption row. `hyprctl decorations` still lists the decoration, hidden. No window rule is
    needed. An earlier note in this phase claimed a rule was needed; it misread that listing and
    is withdrawn.
  - Under X11 (Xwayland): `_MOTIF_WM_HINTS` has decorations 0, `WM_CLASS` is `cairn`, and the
    title is right. hyprbars draws no bar here either, as seen on screen.
- **UNVERIFIED** (batched for the user):
  - drag, double-press maximize, the window menu, resizing from the edges;
  - opening a menu by pointer, and hover moving along the bar;
  - a greyed reason shown on hover;
  - About Cairn and the menus' 12 px, by eye.
  
  No tool on the host moves the pointer into a Wayland window, and xdotool's pointer did not
  reach Xwayland while the session was locked.
- **UNVERIFIED**: macOS. No Apple target is installed, so nothing was compiled for macOS; the
  `Frame::Native` arm is checked only headlessly.
- Not compared shot by shot with FS1/FS2 beyond the band's tones. The toolbar and regions are
  phase 03's.

**QA.** First passes ran from the background and their reports arrived late. A second, fresh set
was run in the foreground: `qa-checklist`, `responsiveness-reviewer`, `test-coverage-auditor` and
`gate-integrity-reviewer`. `qa-confirm` adjudicated 26 raw findings; 15 were confirmed, in whole
or in part. Fixed:
- A, Q: `window-frame.md` now records the macOS gap and what headless tests cannot see.
- B, E: recorded above.
- F: the menus read `refreshed` rather than peeking it.
- G: `keys_inert`'s doc and CLAUDE.md now name About.
- I: `described`'s doc comment is back above its own function.
- K: `MenuKey::Held` is renamed `Unclaimed`, with AltGraph added.
- M: Alt added to `menu_key`'s test.
- N: the ZoomIn comment reworded.
- O: Show HEAD and New Branch… are asserted enabled once the refs are read.
- P: an unborn `HEAD`, and Fetch with no repository.
- R: the bar's Escape and Return, and no key reaching the window behind it. This needed a fix:
  the panel now closes the menu itself on Escape (`Panel::on_dismiss`), so Escape no longer
  reaches the window.
- S: a reason absent before hover and after the pointer leaves.
- T: About does not open under the popover.

Dismissed, with reasons:
- C, a greyed reason unreachable from the keyboard: R2.6 and R9.1 put the reason in a tooltip,
  and skipping disabled items is how standard menus behave. Batched as a design question.
- D, Hide Commit Details in Local Changes: the setting persists. Its tick is drawn, and the pane
  collapses when All Commits is shown again.
- H, the bar reachable under a dialog: every dialog is a Freya `Popup`, whose window-wide scrim
  covers the caption row, and the popover closes on an outside press.
- J, "fork-alignment L10" in manifests: the PRD keeps the citation after teardown, the same
  form was already in cairn-ui's manifest, and the comment guard does not read manifests.
- K in part, listing `menu_key`/`recall_step`/`field_key` among CLAUDE.md's exceptions: they
  answer what to do with a key, not which modifier is held.
- N in part, adding a Ctrl+Shift+= chord: that would be a new chord against Fork's Ctrl+=, so it
  is the user's call.
- S in part, "only the hovered item's reason": the panel keeps a single `pointed_greyed`.
- U, the Show Lost Commits chord from Local Changes: it goes through the same window-scope path
  as F5, which the table's scope tests pin.
- V, About's version compared through `version_line`: the test pins that the crate version
  reaches the dialog, and the wording is not specified.
- W, zoom refusal texts checked only for presence: R2.6 requires a reason, not its words.
- X, the empty ToggleDetails chord: phase 03 changes it.
- Y, CaptionRow cloning the menus: about twelve items, bounded.
- Z, `drag_window`'s modal loop on Windows: Windows is not a target.
- L, the untwinned "one component" sentence in CLAUDE.md: it sits in the status text and the
  repo map as a description, and the residual it supports names its reviewer. Not adjudicated by
  qa-confirm; left as written.

Batched for the user (PENDING, none approved):
- the lockfile's `freya-borderless-plugin` and the widened freya pin, with `titlebar` on
  cairn-ui rather than cairn-app;
- the `ui.md` rows for About's Close and Escape, and for no Alt access keys;
- greyed reasons drawn under the item instead of in a tooltip, and whether the keyboard should
  reach them (C);
- the zoom steps 0.5-2;
- the app id `cairn`;
- `Update::GitFound`;
- a Ctrl+Shift+= alternate for Zoom In (N);
- the pointer and macOS checks above, UNVERIFIED.

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
