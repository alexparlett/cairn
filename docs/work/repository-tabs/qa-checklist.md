# QA checklist — repository-tabs

Packet-specific checks beyond the repo-wide gate and `docs/qa-gate.md`. The acceptance criteria
are in `docs/prd/repository-tabs.md` (C1-C26), not copied here. Each phase runs the items its file
names and the ones its diff touches; phase 07 runs them all.

1. **Fork evidence, cited.** Everything a phase draws as Fork's — the strip, the Manager, the
   menus' items and order, the chords — is in `docs/research/repository-tabs/fork-tabs-evidence.md`
   as VERIFIED, or in the packet's own observation file, never from memory or a tracker. Each
   [observe] requirement built was settled first, with the user's approval logged in
   `progress.md`.
2. **One repository's state stays its own.** No update, answer, prompt, ending, refresh or chord
   of one tab reads or writes another tab's state or the window's but where R2.1 says the window
   owns it. Look for every `session::apply`-style write and every captured `submit`.
3. **Nothing waits, nothing is freed on the UI thread.** Opening, switching, closing and quitting
   with several tabs wait on nothing on the UI thread; a closed tab's answers are retired to its
   worker. The UI-thread residual text in the root `CLAUDE.md` and
   `.claude/agents/responsiveness-reviewer.md` names the new UI-thread calls (the holder's open,
   the store's channel, the picker's spawn) and drops the gone ones.
4. **The seals still hold across tabs.** A confirmation, a Git Error's unspent token and a
   credential prompt are each the tab's that asked: no token or answer crosses tabs, and the holder
   keeps the `Replier` in its hook, never a field.
5. **Virtualization.** Every new list — the Recent list, the overflow menu, a strip of many tabs —
   builds one viewport or is bounded by the window's width, and the twins in the root `CLAUDE.md`
   still hold for whichever tab is shown.
6. **The real window.** Run the release build with at least five repositories open (two sharing a
   folder name, one that fails to open, one with a commit hook that sleeps), switch, close during a
   commit, quit during a write, relaunch. Record what was driven in the phase's progress entry and
   what needed the pointer.
7. **No regression behind the tabs.** Every existing behavioural suite stays green unchanged but
   for how it builds its state: staging routes and their selection, the commit box, Create Branch,
   the confirmation seal, Show Lost Commits, the activity popover, the status box, fetch and its
   prompt, closing with a write running.
8. **Words.** Every new string — the Manager's, the greyed toolbar's reason, the closing line, a
   prompt's title, a failed open's tab — follows `ui.md`'s rules: Fork's words where Fork has them,
   one short sentence, Title Case for titles and buttons, no text a person needs cut.
9. **The store is Cairn's alone.** The tabs file holds paths and nothing read from a repository;
   it is written only by its one excepted file; a bad file is never fatal.
