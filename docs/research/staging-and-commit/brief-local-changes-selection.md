# Local Changes — selection and actions: redesign brief

Date: 2026-10-10. Read-only research over `.claude/worktrees/staging-and-commit` and Fork's
public record. Inputs: `review/ux-local-changes.md` (all), `review/code-app-ui.md` (H1, H2, M4),
`docs/research/staging-and-commit/fork-staging-and-commit.md` §1 and §8,
`docs/research/diff-engine/fork-detail-and-diff-ui.md` Findings 5 and 19, `docs/design/ui.md`
("Staging gestures", "What changes, and why"), `docs/systems/local-changes.md`, PRD
`docs/prd/staging-and-commit.md` R7.3, R7.4a, R8, R9 and its product rules, and the sources
named below. New Fork evidence was gathered on 2026-10-10 from both trackers (`gh issue view`,
`gh search issues`) and the cached release-notes text (Mac
<https://git-fork.com/releasenotes>, Windows <https://git-fork.com/releasenoteswin>).

Conventions: `Tracker #N` = `https://github.com/fork-dev/Tracker/issues/N`; `TrackerWin #N` =
`https://github.com/fork-dev/TrackerWin/issues/N`; "Mac RN x" / "Win RN x" = a release-notes
entry. Confidence: **VERIFIED** (a vendor statement, a release note, or a user report the vendor
answered without contradiction or that two independent users repeat), **INFERRED** (reasoning
over verified facts or native-control behaviour), **UNKNOWN** (looked for, not found).

## Headlines

1. **Fork does not draw several files' diffs together in Local Changes. It shows the first
   selected file's diff.** Cairn's PRD R8.1 and `ui.md` say Cairn draws them together "as Fork
   does". That premise is false (TrackerWin #786; Tracker #261, 15 Jul 2026, user, vendor
   present). The vendor has declined a combined diff in 2019 and again in July 2026. One reason
   he gives is that a selection-based combined view is error-prone. The user's decision of
   2026-10-09 (phase 07 QA 4(e)) and the files-together ratification (phase 08 QA) both rest on
   that wrong premise, so they should be asked again.
2. **On Windows, Fork turns Stage/Unstage into Stage All/Unstage All while Shift is held, not
   Alt.** TrackerWin #2429 and #2466 (2025, two users) and Win RN 2.8 ("crash on pressing
   'shift' key when changed files are not loaded yet") all say so. Cairn uses Alt on Linux
   "as Fork for Windows". Many Linux window managers take Alt for themselves, so on those
   desktops Unstage All has no visible control (review M7). Switching to Shift is the
   Fork-faithful fix. It also ends the Alt problem.
3. **In Fork, the selection lives in one list at a time, and the other list's button is
   greyed** (TrackerWin #2429 screenshots: an unstaged file selected greys `Unstage All`).
   **Since Win 2.7, a held Stage All / Unstage All is enabled whatever is selected**
   (TrackerWin #2466, vendor: fixed in 2.7).
4. **After a stage, Fork moves the selection to the nearest file left** (vendor, Tracker #514).
   It is the same rule for the last row (Mac RN 1.0.59 "Set selection at the end of file list
   after staging last file") and for a directory (Mac RN 2.17). Fork does not jump to the first
   row: Mac RN 1.0.57 fixed exactly that bug. Fork keeps a file selected whenever files exist
   (vendor, Tracker #2333). **What Fork does when a file disappears because of an outside
   change, and whether it keeps a line selection across a refresh, is UNKNOWN.**
5. In Cairn, one concept — "what is selected" — is stored as three values: `ListSelection`, the
   diff's `WorkingChoice`, and `Together`. They are kept in step by a render effect (`Follow`,
   five cases) plus fallbacks in every action. The redesign replaces them with one `Selection`
   that has a primary row, and one pure `next_selection` rule that runs after an action and
   after a refresh alike.

---

## 1. Fork's behaviour, step by step

### 1.1 Selecting

| # | Claim | Source | Confidence |
|---|---|---|---|
| F1 | Both lists take multiple selection with ⌘/Ctrl-click and Shift-click (and ⌘A). | Tracker #2332 (user, 2025: "cmd + A or shift + clicks"); TrackerWin #2338 (user: click, shift+click, ctrl+click) | VERIFIED |
| F2 | The selection is in one list at a time. With an unstaged file selected, the Staged list's button is greyed, and the other way round. | TrackerWin #2429 (user screenshots, Feb 2025) | VERIFIED |
| F3 | There is always a selected file. Fork selects one when files appear. | vendor, Tracker #2333 (as recorded in `fork-staging-and-commit.md` §1) | VERIFIED |
| F4 | The first file is selected by default. | Win RN 1.17 "Select first item in file list by default" | VERIFIED |
| F5 | A refresh does not reset the selection to the first row: Mac RN 1.0.57 fixed "Selection if file list view always jumps to first item in list mode" (sic). | Mac RN 1.0.57 | VERIFIED (that it was a bug); INFERRED (that the selection is kept) |
| F6 | A multi-selection survives switching away and back. Fork even tidies it on focus regain: it unselected the folders and the files stayed selected. | Tracker #2332 (user, Apr 2025, with screenshots; the vendor replied without contradicting) | INFERRED (from the user's text; screenshots not viewed) |
| F7 | In a tree, a selected folder acts on everything under it, even rows that look deselected. | vendor, TrackerWin #1348, #2682; TrackerWin #2338 → #1887 | VERIFIED (does not apply to Cairn's flat lists) |
| F8 | If a large file's diff loads after a smaller file was chosen, the large file's diff can be drawn under the small file's row. This is an old race. | TrackerWin #562 (2019, open) | VERIFIED (a bug, not a model) |

### 1.2 What the diff shows

| # | Claim | Source | Confidence |
|---|---|---|---|
| F9 | **With several files selected, only the first selected file's diff shows.** | TrackerWin #786 (user, May 2020, open): "only the changes from the first selected files will appear"; Tracker #261 (user filiphr, 15 Jul 2026, Mac flat list): "only the first selected file is show in the diff view" — the vendor commented on the same day and did not contradict it | VERIFIED |
| F10 | "First" means either the topmost in list order or the first one clicked. Which one is not recorded. | — | UNKNOWN |
| F11 | The vendor declines a combined multi-file diff in Changes: performance, memory, the minimap, complexity (2019). In 2026 he adds Windows' lack of a variable-height virtual list, and that **selection-based multi-view is unclear and error-prone: a newly changed file off-screen won't be selected**. | Tracker #261, vendor 2019-10-26 and 2026-07-14 (comments 546602774, 4969799416, 4970927305) | VERIFIED |
| F12 | Fork has no line budget in Local Changes because it never draws more than one file. The Commit tab's stacked view is collapsed by default. | `fork-detail-and-diff-ui.md` Findings 4, 19 | VERIFIED |
| F13 | The diff keeps its scroll position across a chunk discard and per file. | Mac RN 1.0.81 "Don't reset scroll position on chunk discard"; Win RN 1.33, 2.8; Mac RN 1.0.95; Win RN 1.20 | VERIFIED |
| F14 | A large file that was loaded anyway goes back to its "too large" placeholder after every stage or discard. In other words, Fork re-reads the diff after each write. | Tracker #2513 (open) | VERIFIED |

### 1.3 What each route acts on

| # | Route | Acts on | Source | Confidence |
|---|---|---|---|---|
| F15 | Return / ⌘S (Mac), Enter / Ctrl+Shift+S (Win) | The selected files, or the selected lines when the diff has a line selection. The vendor said in 2017 that the shortcut "will work for both files and text selection". | DOCS shortcut lists; vendor, Tracker #54 (comment 312039091) | VERIFIED |
| F16 | Stage/discard keys | The selection in whichever view is active. | Win RN 1.30 "Apply stage/discard hotkeys to active selection" | VERIFIED (wording); INFERRED (that "active" means the focused view's selection) |
| F17 | Return heard in the diff with no lines selected and **several files** selected | Not recorded. Since the diff shows one file, it is either that file or the list's selection. | — | UNKNOWN |
| F18 | Hovering a chunk is not selecting it. The chords act on the whole file unless lines are selected. | Tracker #103 | VERIFIED |
| F19 | Header `Stage` / `Unstage` | The selected rows in that list. Greyed when nothing is selected there. | VSHOT Tracker #1754; TrackerWin #2429 | VERIFIED |
| F20 | Held modifier on the header button | **Mac: ⌥** (Mac RN 1.0.23 "Show Stage All/Unstage All buttons on Option key press"). **Windows: Shift** (TrackerWin #2429, #2466; Win RN 2.8 shift-key crash fix). Since Win 2.7 both All buttons are enabled while it is held, whatever is selected. | as cited | VERIFIED |
| F21 | Stage All stages every file, not just the selection. Mac RN 1.0.52 fixed the opposite bug. | Mac RN 1.0.52; Tracker #64 | VERIFIED |
| F22 | Double chevron: on Mac it sits above the lists and flips to "unstage all" once everything is staged (blog 1.0.70 GIF). On Windows it is only in the Unstaged header (Win RN 1.18; USHOT TrackerWin #1348). The vendor keeps it hard to hit on purpose (TrackerWin #2565). | as cited | VERIFIED |
| F23 | Double-click a row: stages or unstages it. | Mac RN 1.0.20; Tracker #970, #974 | VERIFIED |
| F24 | Double-click while several rows are selected | Native list controls collapse a selection to the row on its first click, so it would act on that one row. | — | INFERRED |
| F25 | Drag between lists: drags files or folders. | Mac RN 1.0.31, 1.0.95; Win RN 1.40 | VERIFIED |
| F26 | Drag of an unselected row while others are selected | Native drag carries the selection if the row is in it, otherwise the row alone. | — | INFERRED |
| F27 | Context menu: `Stage`, `Discard Changes…`, `Stage All`, … on the unstaged side. The staged side offers Unstage and Unstage All; its full contents are not recorded. | USHOT Tracker #2095; VSHOT TrackerWin #326; USER Tracker #1152 | VERIFIED / partly UNKNOWN |
| F28 | Discard acts on the whole selection. With a tree, deselected children of a selected folder go too. | TrackerWin #2338/#1887 | VERIFIED |

### 1.4 Where the selection goes

| # | Event | Fork | Source | Confidence |
|---|---|---|---|---|
| F29 | After a stage or unstage | The nearest remaining file in the list it left. The vendor calls review → stage → repeat the common pattern. | vendor, Tracker #514 (460979499); Mac RN 1.0.55 fix | VERIFIED |
| F30 | After staging the last row of the list | The new last row, i.e. the nearest file above. | Mac RN 1.0.59 | VERIFIED |
| F31 | After staging a directory | The next available file. | Mac RN 2.17 | VERIFIED |
| F32 | After staging the only file left | Nothing is selected and the diff goes blank. Switching automatically to Staged is an open request. | Tracker #1663 | VERIFIED |
| F33 | Users have asked for the selection to follow the file into Staged since 2019 (open, steady upvotes). The vendor prefers nearest-remaining. | Tracker #514, #939 | VERIFIED (demand exists) |
| F34 | Staging the row at index 0 once lost keyboard focus; fixed in Win 2.16. | Win RN 2.16 | VERIFIED |
| F35 | After a refresh on focus, while the file is still listed | The selection is kept (F5, F6). | — | INFERRED |
| F36 | After an outside change removes the selected file | Not recorded. F3 says some file stays selected; which one is not recorded. | — | UNKNOWN |
| F37 | After a commit | Fork stays on Local Changes. | vendor, TrackerWin #118 | VERIFIED |

### 1.5 Line selections

| # | Claim | Source | Confidence |
|---|---|---|---|
| F38 | A drag across text selects lines. The floating buttons and the chords narrow to those lines. Lines can be selected on one side of side-by-side only. | `fork-detail-and-diff-ui.md` Finding 23; Tracker #1985 | VERIFIED |
| F39 | Fork refreshes when it regains focus. | GitClient 1.0.1 RN; Win RN 1.44, 1.55 (fixes for missed refreshes) | VERIFIED |
| F40 | Whether a line selection survives that refresh. | No tracker issue or release note found (searched "selected lines", "select lines", "selection lost/cleared/resets", "line selection lost") | UNKNOWN |
| F41 | Selecting lines in an inactive Fork window garbles the selection. | TrackerWin #2537 (2025, open) | VERIFIED (a bug; it shows a line selection is a native text selection, not a model) |

---

## 2. Cairn today, step by step, and where it diverges

### 2.1 The state

The selection is held in **three places**:

- `view.local.selection: ListSelection` (`crates/cairn-ui/src/list_selection.rs`): one list, the
  sorted paths and an anchor. It can be empty while still belonging to a list ("toggled out the
  last").
- `DiffState::working_choice(): WorkingChoice { list, path, lists serial }`
  (`crates/cairn-app/src/diff_state/working.rs`): the path whose diff is drawn. When no selection
  is made in a list, this is what actions fall back to.
- `DiffState.together: Together` (`crates/cairn-app/src/diff_state/together.rs`): the paths
  drawn together, with `TogetherWanted` compared against it on every render.

Line selections are identified by **answer number**: `working_drawn` (`answered` /
`previous_answered`) or `together_drawn`, renumbered on each answer kept
(`staging_gesture.rs` `phase_under`).

### 2.2 Gestures

| Step | Cairn | Fork | Diverges? |
|---|---|---|---|
| Plain press / ↑↓ | `choose`: `ListSelection::of` plus `choose_working` | same | no |
| ⌘/Ctrl-click | `toggle`. If there is no selection in this list, the base is built from the *chosen path* (`(false, Some(chosen))`). Toggling out the shown path moves the chosen path to "another the selection holds". Toggling out the last leaves an empty-but-owned selection while the chosen path is still drawn. | native toggle; nothing selected → diff blank (INFERRED) | yes, in mechanism: two states reconciled by hand |
| Shift-click / Shift+↑↓ | `range`. The anchor falls back to the chosen path. | same | mechanism only |
| Several selected | Their diffs are drawn **together**, read in order under Expand All's 50,000-line budget. Files past it say "Not shown: the files above it used the diff's line budget. Choose it alone to see its diff." The bar fakes a `ChangedFile` named "N files" (`together_side`). There is no Load Diff, no mode row and no prev/next. | **The first selected file's diff only** (F9) | **yes. The PRD claims parity and is wrong.** |

### 2.3 What each route acts on

| Route | Cairn acts on | Same as the list's selection? |
|---|---|---|
| Double-click | its own row (the first press already made it the selection) | yes, in effect |
| Chord, list focused | `Acted::Selection` → `acted_rows`: the selection's paths still listed, **or the chosen path when the selection is in the other list or empty** | usually; the fallback acts on a row that is not drawn selected |
| Chord, diff focused, lines selected | those lines (the gesture) | n/a |
| Chord, diff focused, no lines, one file | the chosen path | yes |
| Chord, diff focused, no lines, several files | **`together_read_paths()`: only files read and drawn**, never one past the budget or still being read | **no. 40 selected might stage 12 (review H3).** Discard's dialog then names only those. |
| Drag | the selection if the drag began inside it, else the row it began on | yes |
| Header Stage/Unstage | `Act(list, StageOrUnstage)` → same `acted_rows` fallback | usually |
| Header with **Alt** held | Stage All / Unstage All | Fork Windows uses **Shift** (F20). Alt never arrives under xfwm, openbox or Plasma 5. |
| Double chevron | Stage All, in Unstaged's header only | Fork Windows (F22). There is **no visible Unstage All when Alt is taken** (review M7). |
| Context menu | the selection (a right-press outside it chooses that row first, R7.4a) | yes |
| Stage All / Unstage All with a filter | only the rows shown (user decision, 2026-10-09) | Fork: all (F21). A recorded deviation. |

So the action's **scope depends on which view has focus** (list or diff), on whether the files
are drawn together, and on how far the together read has got.

### 2.4 Where the selection goes

- **After Cairn's own stage or unstage** (`stage_or_unstage` → `next_after` → `move_to`): the
  nearest remaining row in that list, measured from the first acted row (R8.3, 4(a)). This is
  computed **at the action, before status arrives**. With none left, the selection is empty.
  Matches Fork (F29–F32).
- **After Stage All / Unstage All** (`everything`): the selection is cleared outright.
- **After a refresh** (`follow_the_lists`, a render effect, `Follow::{Keep, ReAsk, ChooseFirst,
  LetGo, Nothing}`): this follows **only the chosen path**. If it is still listed, it is asked
  again. If it is gone, the **first** row shown (Unstaged before Staged) is chosen and the
  selection is reset to it. That is a different rule from R8.3 (review L4, M4). Other selected
  paths that vanish stay in `ListSelection` and are filtered out where they are acted on. So
  **a path that vanishes and later reappears comes back selected unseen** (INFERRED from
  `acted_rows`'s `filter_map(row_of)` and `together_wanted`).
- The effect carries a loop-avoidance comment ("a write wakes the effect again…"). Each fix for a
  stale selection has added another counter (`drawn`, `answered`, `previous_answered`, `lists`).

### 2.5 Line selections and refresh

A line selection is bound to the answer number. A focus-gain refresh re-reads the diff,
`working_arrived` keeps the new answer under a new number, and the selection becomes "nothing".
It is dropped silently, so Return then stages the whole file (review M4). This is safe — no stale
rows reach a write — but the person loses the selection with no notice. Fork: UNKNOWN (F40).

### 2.6 The bar for several files

`together_side` builds `RepoPath::from(format!("{count} files"))` and passes it to
`header_file(None, ..)`, which builds a `Modified` `ChangedFile`. Prev/next change do nothing.
Load Diff and the mode row disappear (ratified, `ui.md`).

### 2.7 Divergences, summarised

| Divergence | Recorded? |
|---|---|
| Several files drawn together, line budget, chords narrowed to files drawn | Recorded as a user decision, but **claimed "as Fork does", which is false (F9)** |
| Alt on Linux for Stage All / Unstage All | Recorded (4(d)), but as "Fork for Windows", whose key is **Shift (F20)** |
| No visible Unstage All when Alt is taken | Recorded as a residual (fails safe). Fork Windows has Shift. |
| Selection jumps to the first row when the chosen path vanishes | Not recorded (L4). Fork fixed the same jump in Mac 1.0.57. |
| A line selection dropped by any refresh | Recorded as a product rule (phase 08 QA item 1). Fork: unknown. |
| Chord scope depends on focus | Not recorded as a deviation (H3) |
| Vanished paths stay in the selection | Not recorded (a latent bug) |

---

## 3. Proposed clean model

### 3.1 One selection

```text
LocalSelection {
    list: ChangeList,            // the selection lives in one list (Fork, F2)
    paths: SortedSet<RepoPath>,  // never empty while the value exists
    primary: RepoPath,           // ∈ paths: the row whose diff is shown, the focus ring
    anchor: RepoPath,            // ∈ paths: where Shift-ranges extend from
}
state: Option<LocalSelection>    // None = nothing selected (only when no row is shown, F32)
```

- `LocalChangesState` owns it, in one place. There is no `WorkingChoice` separate from it: the
  diff query is *derived* from `primary` and the lists as they are drawn. `Together` and
  `TogetherWanted` are gone (see 3.2).
- Press = `{row}`, which becomes primary and anchor. ⌘/Ctrl-click toggles. Toggled in, the row
  becomes primary. Toggling out the primary moves primary to the nearest remaining selected path.
  Toggling out the last selected path gives `None` (the diff says "No file selected"); there is
  no hidden chosen path. Shift-click / Shift+↑↓ makes the range anchor..row, with the row as
  primary.
- A press in the other list starts a selection there (F2).
- The header buttons read `state.list`. The other list's `Stage` / `Unstage` is greyed. Both All
  forms are enabled while the held key is down (F2, F20, Win 2.7).

### 3.2 What the diff shows

**Recommended: Fork's model.** The diff shows the **primary** file's diff, as a single file
always is: Load Diff, the mode row, prev/next change and the gesture all work. With more than one
file selected, the bar keeps the primary's ordinary file header and adds a counted suffix:
**`2 of 40 selected`**. There is no fabricated `ChangedFile`. ↑/↓ in the list moves the
selection; Tab moves focus to the diff. There is no line budget and no "Not shown…" notice,
because nothing is drawn but one file. (Option 4.1 B/C below keeps a combined view if the user
still wants one.)

### 3.3 Every route acts on the selection

The rule is one sentence: **an action acts on the selected lines if the diff has a line
selection, and otherwise on the selected files — wherever it is heard.**

| Route | Acts on |
|---|---|
| Return / ⌘S / Ctrl+Shift+S, list or diff focused | the line selection if any, else every path in `state.paths` |
| Backspace / Delete / ⇧⌘D / Ctrl+Shift+D | the same, through the confirmation (which names the files) |
| Double-click | the selection. Its first press made the row the selection, so in practice it acts on that row (F23, F24). No special case is needed. |
| Drag | the selection. The press that starts a drag on an unselected row selects it, as native lists do (F26), so a drag never carries anything other than the selection. |
| Header `Stage` / `Unstage` | the selection, if it is in that list; greyed otherwise |
| Header with **Shift** held (Linux, Fork for Windows) / ⌥ (macOS) | Stage All / Unstage All, enabled whatever is selected |
| Double chevron (both headers, see 4.4) | Stage All / Unstage All |
| Context menu | the selection (R7.4a: a right-press outside the selection selects that row first) |
| ⌥⇧⌘S / Ctrl+Alt+Shift+S | all of that list |

The "files drawn" set no longer exists, so no route narrows the selection. Mixed discards (a
submodule among modified files, review M3) should be handled with one rule — refuse the whole
discard and count what is refused, e.g. "1 of 31 can't be discarded" — but that belongs to the
discard flow's brief.

### 3.4 One rule for where the selection goes next

```text
next_selection(sel, before: &Rows, after: &Rows, leaving: &Set<RepoPath>) -> Option<LocalSelection>
  kept = sel.paths ∩ after.listed(sel.list) − leaving
  if kept non-empty:
      primary = sel.primary if kept ∋ it, else the kept path nearest the old primary
      anchor  = sel.anchor  if kept ∋ it, else primary
      → Some(kept, primary, anchor)
  else:
      i = position in `before` (shown order, through the filter) of sel's first row
      row = after.shown(sel.list)[i] if it exists, else the row nearest above (i−1, …)
      → Some({row}) if a row exists in sel.list, else None (the diff goes blank: F32)
```

- **After an action**, call it at once with `after = before − acted` and `leaving = acted`, so the
  next file's diff is asked immediately (Fork's review → stage → repeat rhythm, F29–F31). When the
  status arrives, call it again with the real lists. If the write did what it said, that second
  call changes nothing.
- **After a refresh** (focus gain, a fetch, an outside commit), call it with the old and new lists
  and `leaving = ∅`. Vanished paths are pruned, so nothing comes back selected unseen. When every
  selected path vanished, the row that slid into the first one's place is selected — **the same
  place R8.3 puts it after a stage**. This replaces `Follow::ChooseFirst`.
- **Stage All / Unstage All** go through the same rule. Every row leaves, so the result is
  `None`, unless a filter kept some rows, in which case the row in the first acted row's place is
  selected.
- It is a pure function in `cairn-app` (or `cairn-model`), called from `session::apply` when the
  status or filter rows arrive, not from a render effect. That removes the loop hazard. The diff
  query is then derived from `primary`: if the primary and its listing are the same as last time,
  the diff is asked again only when the status serial moved (the file may have changed while
  still listed, as `ReAsk` does now).
- **The filter** (choice 4.5): recommended — changing the filter prunes the selection to the rows
  it shows, through the same function with `after = shown rows`. That keeps "what you see is what
  you act on", the rule the user chose for Stage All.

### 3.5 A line selection is kept while the diff text is unchanged

- `ShownDiff::new` (diff thread) computes a `content: u64` hash of what the gesture's rows are
  built from: the path, the side, the options and the patch text.
- `LineDrag` records `content`, not an answer number. `selected(content)` and
  `on_gesture(act.content)` refuse an act unless the drawn diff's `content` equals it. Equal
  content means identical rows, so the existing safety (phase 08 QA item 1: no old rows reach a
  write) still holds, and the engine's stale-patch refusal still backs it up.
- A refresh whose re-read is byte-identical keeps the selection, its tint and its floating
  actions. One whose text differs drops the selection, and the bar says once: **"The file
  changed — line selection cleared."** It disappears at the next press or key.
- A drag still in progress is cancelled when focus is lost, as today. A *finished* selection is
  not.

### 3.6 A visible Unstage All on every platform

- The held key is **Shift on Linux** (Fork for Windows, F20) and ⌥ on macOS (Fork for Mac).
  Window managers do not take Shift+click, so the morph works everywhere. While held, both
  headers read `Stage All` / `Unstage All`, enabled whatever is selected (Win 2.7).
- Plus (choice 4.4) a small double chevron in **each** header: ⇊ in Unstaged, ⇈ in Staged. It is
  as small as Fork keeps its own, so it stays "hard to hit" (TrackerWin #2565).

### 3.7 Storyboards (recommended options)

**(a) Select three files and stage them by chord from the diff.**
1. Unstaged lists `a.rs`, `b.rs`, `c.rs`, `d.rs`. `a.rs` is selected (the first file by default,
   F4). The diff shows `a.rs`, with bar `a.rs  M`.
2. Shift-click `c.rs`. Rows a–c are highlighted; `c.rs` is primary (focus ring). The diff shows
   `c.rs`. The bar reads `c.rs  M · 3 of 3 selected`. Unstaged's `Stage` is enabled and Staged's
   `Unstage` is greyed.
3. Click into the diff (no lines dragged) and press Return.
4. Three files are staged: `git add -- a.rs b.rs c.rs` is asked. The selection moves at once to
   `d.rs`, the row in `a.rs`'s place, and `d.rs`'s diff is asked. The activity status says
   "Stage 3 files". Focus stays in the diff.
5. The status arrives: Unstaged = `d.rs`, Staged = `a.rs b.rs c.rs`. `next_selection` keeps
   `d.rs`; nothing changes.

**(b) Select 40 files where not all diffs can be drawn.**
1. Unstaged has 300 files. Click row 10, then Shift-click row 49. Forty rows are highlighted and
   row 49 is primary.
2. The diff shows row 49's file (2,000 lines, read as any single file is, with Load Diff past the
   limits). The bar reads `src/x.rs  M · 40 of 40 selected`. There is no budget and no "Not
   shown…" notice.
3. ⌘/Ctrl-click row 30: it toggles out. "39 of 39 selected", and the diff is unchanged (the
   primary is still row 49).
4. Backspace: the confirmation names the first three files and "36 more", and counts what is
   lost. Cancel is focused. The count covers all 39, wherever focus was.
5. Return instead: all 39 are staged, and the selection moves to the row in row 10's place.

**(c) Drag-select lines, switch to another app and back.**
1. `a.rs` is shown. Drag across six changed lines. They are tinted and float `Stage 6 Lines` /
   `Discard 6 Lines…`.
2. Alt-Tab to the editor. No drag is in progress, so nothing is cancelled. Do not edit `a.rs`.
3. Return to Cairn. A refresh runs: status, then `a.rs`'s diff is re-read. Its `content` is
   equal, so the six lines stay tinted, the floating actions stay, and Return stages those six
   lines.
4. Variant: you edited `a.rs` in the editor. The re-read's `content` differs. The tint goes, and
   the bar says "The file changed — line selection cleared." until the next press or key. Return
   would now stage the whole file, and the bar has said so.

**(d) A file vanishes from the list because it was committed in a terminal.**
1. Unstaged: `a b c d`. `b` and `c` are selected, with `c` primary.
2. In a terminal: `git commit -am …` covering `b` and `c` only (say `a` and `d` are untracked).
3. Focus returns. The status arrives: Unstaged = `a d`. `next_selection` finds nothing kept, so
   the row in `b`'s old position (index 1) is `d`. `{d}` is selected and `d`'s diff is asked.
   This is the same place a stage of `b` and `c` would have left it.
4. Variant: only `c` was committed. `kept = {b}`, so `b` becomes primary and the diff shows `b`.
   No first-row jump.
5. Variant: everything was committed. Both lists are empty, the selection is `None`, the diff
   says "No local changes".

**(e) Unstage All on Linux (window manager takes Alt+click).**
1. Staged has 12 files. The selection is in Unstaged (`d.rs`), so Staged's `Unstage` is greyed.
2. Hold Shift. Unstaged's button reads `Stage All` and Staged's reads `Unstage All`, both enabled.
3. Click `Unstage All`. All 12 rows (or only the rows the filter shows) are unstaged. The
   selection, which was in Unstaged, keeps `d.rs` (it is still listed). The 12 files appear in
   Unstaged.
4. Equivalent without a modifier: Staged's ⇈ chevron (choice 4.4), the context menu's `Unstage
   All`, or Ctrl+Alt+Shift+S with Staged focused.

---

## 4. Choices and unknowns

### 4.1 What the diff shows for several files (Fork VERIFIED: the first file only)

| Option | What the person experiences | Cost |
|---|---|---|
| **A. Fork's: the primary's diff, plus "k of n selected" in the bar** | The diff behaves the same whatever is selected. Actions act on the selection, and the confirmation names it. There is no side-by-side review of several files. | Removes `together.rs`, `stacked_diff.rs`, `Request::Together` / `Update::Together` and the worker's together read, `NOT_READ_TOGETHER` and the fake header. Some loss of something already built. |
| B. Today's stacked view, fixed: no read budget. Files are read lazily as they scroll into view (the stacked list is already virtualised; reading is the cost, not drawing), and the actions act on the selection, never on "files drawn". | You can read a .cpp and its .h together (TrackerWin #786's wish). | Keeps the Together state and its lane holder. Adds read-on-scroll paging. Loses Load Diff, the mode row and prev/next unless those are rebuilt per file. A deviation from Fork that has to be recorded. |
| C. A, plus an explicit "Show selected together" toggle in the bar that switches to B | Fork by default, with a combined view when asked for | The costs of both A and B |

**Recommendation: A.** It is what Fork does (F9). The vendor's 2026 reasoning (F11) is exactly
the hazard the review found (H3). And it removes the largest source of patches (the together
lane, the renumbering, the budget). The decision of 2026-10-09 (phase 07 QA 4(e)) was taken as
"as Fork does". That premise is false, so the user should be asked again with this evidence.

### 4.2 What a diff-focused chord acts on when several files are selected and no lines are (Fork UNKNOWN, F17)

| Option | Experience | Cost |
|---|---|---|
| **A. The whole selection** | One scope everywhere. The bar's "k of n selected" warns, and a discard confirms with names. | none |
| B. Only the primary file shown | Matches what is visible. But the same key acts differently from the list than from the diff — today's H3 problem in a smaller form. | a focus-dependent branch |

**Recommendation: A.** One rule. Fork's "active selection" (Win 1.30) most plausibly means the
list's selection whenever there are no lines.

### 4.3 Where the selection goes after an outside change removes it (Fork UNKNOWN, F36)

| Option | Experience | Cost |
|---|---|---|
| **A. The same as after a stage: the row in the first removed row's place, else the nearest above** | The same thing happens whoever moved the file. | one function |
| B. The first row (today) | It jumps to the top of a long list. Fork fixed this jump in 1.0.57. | a second rule |
| C. Nothing selected | A blank diff after every terminal commit. Contradicts F3. | — |

**Recommendation: A.**

### 4.4 A visible Unstage All

| Option | Experience | Cost |
|---|---|---|
| **A. Shift-held morph (Fork for Windows) on Linux, ⌥ on macOS** | Fork-faithful. Works under every window manager. Still a hidden mode. | change `HeldKeys::press` and the doc rows |
| B. A plus a ⇈ chevron in the Staged header | Visible on every platform. A small deviation from Fork for Windows (it has only the Unstaged chevron); Fork Mac has a flipping chevron. | one glyph, one ui.md row |
| C. A single flipping chevron (Fork Mac) | Ambiguous when both lists have rows | — |

**Recommendation: A + B.** The task asks for a *visible* Unstage All on every platform, and B is
the smallest visible control. It deliberately mirrors the Unstaged chevron Fork already draws.

### 4.5 The filter and the selection

| Option | Experience | Cost |
|---|---|---|
| **A. Changing the filter prunes the selection to shown rows. If none are left, the first shown row is selected.** | Nothing hidden is ever acted on. Consistent with the Stage All filter decision. | none |
| B. Keep hidden paths selected (today keeps the chosen path's diff drawn) | You can act on rows you can't see | a "hidden but selected" state |

**Recommendation: A.** Fork has a filter, but how it treats the selection is UNKNOWN.

### 4.6 A line selection on refresh (Fork UNKNOWN, F40)

| Option | Experience | Cost |
|---|---|---|
| **A. Keep while the content hash is equal; otherwise clear, and say so once** | Alt-Tab costs nothing. A real change is announced. | the hash on the diff thread; replaces the `answered` counters |
| B. Today: dropped by any refresh, silently | Lost work after every focus switch | — |
| C. Try to map the selection onto the new diff by line content | Surprising partial survivals | complex and risky (could stage the wrong lines) |

**Recommendation: A.**

### 4.7 After staging, follow the file into Staged? (Tracker #514 demand; vendor said no)

Keep nearest-remaining (Fork, R8.3, 4(a)). Note only that users ask for this.

### Recorded decisions the clean model would revisit

| Decision (PRD) | Why revisit |
|---|---|
| R8.1 addition, 2026-10-09 (phase 07 QA 4(e)): several files drawn together "as Fork does" | The premise is false (F9, F11). Recommend 4.1 A. |
| Product rule "Files drawn together, as ratified by the user (2026-10-09, phase 08's QA)": line budget; chords act only on files read and drawn; no Load Diff / mode row / prev-next | Becomes moot under A. Under B, the "chords act only on files drawn" part must go (H3). |
| R8.2 addition, 2026-10-09 (4(d)): Alt on Linux; chevron only in Unstaged | Fork for Windows is Shift (F20). Add the Staged chevron (4.4 B). |
| Product rule "A line selection acts only on the answer it was made over" (phase 08 QA item 1) | Keep the safety, but the identity becomes content, not answer number (4.6 A). |
| R8.3 / 4(a) nearest remaining, measured from the first acted row | **Kept**, and extended to refreshes (4.3 A). |
| R8.2 4(b) a double-click acts on its own row | **Kept**. It now follows from "routes act on the selection". |
| Item 11 (2026-10-09) Stage All with a filter takes the shown rows | **Kept**, and extended to the selection (4.5 A). |
| R7.4a decision C: a right-click selects its row | **Kept.** |

---

## 5. What the redesign removes from today's code, roughly

Line counts are of whole files today. Removal is partial where noted.

- **`crates/cairn-app/src/diff_state/together.rs` (690 lines): all of it** under 4.1 A. Also:
  `Request::Together` / `Update::Together` / `TogetherQuery` / `TogetherOutcome` in
  `worker/request.rs`; `DiffLane::together` in `worker/diff_lane.rs` (roughly 100–150 lines); the
  together arms in `diff_state.rs` and `session.rs`; and `Together.in_lane`, one of the four lane
  booleans (code review H1).
- **`crates/cairn-ui/src/stacked_diff.rs` (297 lines)**: only Local Changes uses it, so all of it
  goes. Its tests in `crates/cairn-ui/tests/staging_gesture.rs` go too.
- `local_changes_pane.rs`: `together_wanted`, `draw_together`, `together_side` (the fake "N
  files" `ChangedFile`), the `Follow` enum, `follow` and the `follow_the_lists` effect — about
  250 of 913 lines. They are replaced by the pure `next_selection` (about 60 lines plus tests)
  called from `session::apply`.
- `local_changes_actions.rs`: `acted_rows`' chosen-path fallback; `toggle`'s synthesis from the
  chosen path; `range`'s anchor fallback; `chosen_is`; `on_the_diff`'s `together_read_paths`
  branch; `move_to` and `everything`'s separate selection resets (folded into `next_selection`)
  — about 150 of 923 lines.
- `diff_state/working.rs`: `WorkingChoice` as independent state (its `list` / `path` come from the
  selection's primary); the `answered` / `previous_answered` counters and `working_drawn`, replaced
  by `ShownDiff::content`. About 80 of 587 lines change; the query and answer plumbing stays.
- `list_selection.rs`: the "empty but still the list's" state; `ListSelection` becomes
  `LocalSelection` with a primary (171 lines, rewritten at about the same size).
- `staging_gesture.rs`: `drawn: u64` becomes `content: u64` — a rename in kind, not a removal.
- `accelerators.rs`: the Stage All press resolves from Shift on Linux instead of Alt. A small
  change, plus its tests.
- `ui.md` rows that go: "drawn together under a line budget…", "Drawn together, a file offers no
  Load Diff…". Rows that are added: "a Staged-side ⇈ Unstage All" (if 4.4 B), "a line selection
  survives a refresh that leaves the diff identical" (only if Fork is shown not to keep it). The
  `local-changes.md` sections "Several paths, drawn together" and most of "A refresh follows the
  path" are rewritten.
- Tests that pin removed behaviour — for example
  `the_chords_over_files_drawn_together_take_only_the_files_drawn` and
  `a_selection_over_files_drawn_together_is_nothing_once_a_page_replaces_its_diff` — are deleted.
  New ones pin `next_selection` (after an action, after a refresh, when a path vanishes, with
  gaps, with a filter) and content-keyed line selections.

Net, roughly: **−1,300 to −1,500 lines** of production code under 4.1 A, and the end of the
three-way selection sync that the review traced through QA rounds 07 and 08.

## Open questions only the owner's Fork (Mac) can settle quickly

1. With several files selected, is the "first" file the topmost or the first clicked (F10)?
2. With focus in the diff, several files selected and no lines, what does Return stage (F17)?
3. After `git commit` in a terminal removes the selected file, which row does Fork select (F36)?
4. Does a drag-selected line range survive switching apps and back with the file untouched (F40)?
5. Does Fork's Staged context menu list `Unstage All` (F27)?
