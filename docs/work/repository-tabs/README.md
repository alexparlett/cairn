# repository-tabs

Opens several repositories in one window, one tab each, behind Fork's tab strip, with the
Repository Manager tab, Open Repository…, Close Tab, Restore Closed Tab and Ctrl+Tab. Underneath,
it splits the window's state into what the window owns and what each tab owns, and gives every
repository its own worker, update stream and close. The open tabs and the Recent list are
remembered across launches, in Cairn's own state file.

Spec: `docs/prd/repository-tabs.md` (the one copy of the acceptance criteria). Design:
`docs/design/repository-tabs.md`. Evidence: `docs/research/repository-tabs/`. Base: `main` once
`fork-alignment` (roadmap 5d, PR #154) has merged. Integration branch: `feature/repository-tabs`.

| Phase | File | Builds | Criteria |
| --- | --- | --- | --- |
| 01 | `phase-01-observe-and-split.md` | Fork observed; `WindowView` and `TabView`; the zone and `git` read once | C1 (observation), C2, C3 (one tab) |
| 02 | `phase-02-several-tabs.md` | the tab holder and its guard, several tabs from the command line, switching, refreshes across tabs, the window checks | C4-C6, C11, C14 (paths), C25 (written) |
| 03 | `phase-03-the-strip.md` | the strip's look, ×, middle press, reorder, its menu, overflow, names, marks, Ctrl+Tab | C7-C10, C21 (tab switching) |
| 04 | `phase-04-open-and-close.md` | Open Repository… (`rfd`), the Repository Manager tab, Close Tab, closing across tabs, Restore Closed Tab, the File and Window menus | C12-C18, C21, C24 (`rfd`) |
| 05 | `phase-05-dialogs-across-tabs.md` | prompts coming forward with their tab's name, endings waiting on their tab, the status box and popover per tab | C10 (waiting), C19, C20 |
| 06 | `phase-06-remembered-tabs.md` | the tabs file (`serde`, `toml`), its filesystem row, the launch rules, Recent across launches | C12 (missing), C22-C24 |
| 07 | `phase-07-qa.md` | the merge bar: whole-packet `/qa`, every criterion, the bench measurements, teardown | C25, C26 and all |
