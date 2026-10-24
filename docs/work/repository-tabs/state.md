# State — repository-tabs

The cross-session cheat sheet. Every session updates it before ending.

**Status: planned, 2026-10-24. Not started.** It waits on `fork-alignment` (roadmap 5d, PR #154)
merging to `main`: every file the PRD names is as `feature/fork-alignment` (b539d9d) has it. Phase
01 needs the machine with the Windows VM for its observation.

## Locked decisions (brainstorm.md has the evidence and the rejections)

- **T1. Scope:** tabs, the Repository Manager tab (Recent and Open), Open Repository…, Close Tab,
  Restore Closed Tab, Ctrl+Tab, and the open tabs and Recent remembered across launches. Clone,
  Init and the Manager's summary card stay in `repository-manager` (5i).
- **T2. A background tab:** resident (its worker and state kept), never mounted; only the active
  tab is drawn. Nothing evicted (#4).
- **T3. Scope of state:** the layout and the diff's toggles are the window's; what a person looks
  at is the tab's (the PRD's R2.1 table). Show Lost Commits and the activity log per tab.
- **T4. Fork first:** phase 01 observes FT §17's questions; [observe] requirements are settled,
  with the user's approval, before they are built.
- **T5. Strip defaults [observe]:** × on the active and hovered tab, middle press closes, drag
  reorders, a menu of Close Tab and Close Other Tabs, an overflow list past a minimum width, the
  mark for uncommitted changes, a second shape for a waiting dialog.
- **T6. Names:** the folder, told apart by the parent folders in parentheses, worked out again on
  every open and close (Fork's stale names not copied).
- **T7. Picker:** `rfd`, made direct: no new package, its future awaited in a render-side task.
- **T8. Refresh:** focus refreshes the shown tab whole and the others' status alone; activation
  refreshes whole.
- **T9. Closing:** the last tab leaves a Manager tab; a closed tab's write finishes; quitting waits
  and names the repository.
- **T10. Duplicates:** opening an open repository shows its tab. Worktrees: two tabs, two lanes,
  until `worktrees` (8) keys the lane.
- **T11. Dialogs:** a credential prompt comes forward, titled with its tab, drawn by that tab's
  holder; every other dialog — an ending, a confirmation answered after a switch — waits on its
  tab, which is marked. A closed tab's failed write comes forward with Close alone.
- **T12. Store:** TOML through `serde`, `$XDG_STATE_HOME/cairn/tabs.toml`, written whole and
  atomically on an application thread; a `FILESYSTEM_MUTATION_EXCEPTIONS` row.
- **T13.** The packet is roadmap 5e; `file-tree-view`, `quick-launch`, `preferences` and
  `repository-manager` are 5f-5i.
- **T14.** The planning docs land on `main` directly.

## Interfaces introduced so far

None yet. Planned names (`implementation-plan.md`): `WindowView`, `TabView`, `TabId`,
`crates/cairn-app/src/tabs/holder.rs`, `worker/folder_picker.rs`, `worker/tab_store.rs`,
`crates/cairn-ui/src/tab_strip.rs`, `crates/cairn-ui/src/repository_manager.rs`, the actions
`NewTab`, `OpenRepository`, `CloseTab`, `RestoreClosedTab`, `NextTab`, `PreviousTab`, and the
guard `only_the_tab_holder_opens_a_repository`.

## [observe] settlements

None yet. Phase 01 lists each [observe] requirement here with what Fork does, what the PRD now
says, and the user's approval.

## Validation

| Phase | Gate | /qa | Notes |
| --- | --- | --- | --- |
| 01 | — | — | not started |
| 02 | — | — | |
| 03 | — | — | |
| 04 | — | — | |
| 05 | — | — | |
| 06 | — | — | |
| 07 | — | — | |
