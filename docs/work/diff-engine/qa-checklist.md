# QA checklist — diff-engine

Packet-specific acceptance beyond the repo-wide gate. The acceptance criteria
C1-C16 live in `docs/prd/diff-engine.md` and are NOT copied here — verify them
there, against their pinned tests.

## Per-phase coverage of the PRD criteria

| Phase | PRD criteria it must satisfy |
| --- | --- |
| 01 | C4 |
| 02 | C1, C2, C3, C5, C6, and C14's engine numbers |
| 03 | C7, C15 |
| 04 | C8 |
| 05 | C10 (the Commit tab's fields and the kept tab), C13 |
| 06 | C9 (unified), C11 (the toggles and the intra-line ranges) |
| 07 | C9 (side-by-side), C10 (the Changes tab), C11 (the non-text states) |
| 08 | C10 (expansion in place and Expand All), C12, and C14's window check |
| 09 | all of C1-C16, re-verified over the whole packet diff |

## Packet-specific checks

Beyond the PRD, phase 09 confirms:

- [ ] **No gix type reaches a public signature.** Read every `pub` item added to
      `cairn-git`, not just the ones the seam guard can see.
- [ ] **No read path writes.** Nothing outside `ops/` writes a refreshed index, a
      blob or an object — `write_changes` and `worktree_file_to_object` are the two
      that are easy to call by accident.
- [ ] **No program runs on a read except a filter driver.** A fixture configuring
      both a `textconv` and an external `diff.<driver>.command` must leave no trace
      after a diff.
- [ ] **The emitter cannot reach the whitespace-ignoring ranges**, and the patch
      carries three lines of context whatever the display context was. Check the
      signature, then check a test proves it.
- [ ] **A partial selection's patch was applied by real git**, not compared to a
      golden string. A golden patch test passes when both the emitter and the
      expectation are wrong.
- [ ] **The too-large check runs before the content is read.** A fixture with a
      blob far over the limit must answer without the read happening.
- [ ] **Lanes supersede exactly as specified.** State the mutation that makes C8
      fail; if you cannot, C8 decides nothing.
- [ ] **The exceptions roster is still empty** and no render file names a plain
      `ScrollView`. Three lists arrived in this packet.
- [ ] **The marker column is present in both views** and no state is carried by
      colour alone.
- [ ] **Both new twins have been seen to fail** on a deliberate violation: the
      literal-modifier guard, and the headless test that pins one viewport of diff
      rows. A twin nobody has watched go red is a promise, not a guard.
- [ ] **No new dependency slipped in.** The font is a file with its licence, not a
      crate; `gate.sh --step deps` still passes.
- [ ] **`docs/systems/diff.md` describes only what was built** — no Local Changes
      screen, no staging, no ref chips. Documenting the obvious next consumer as
      though it exists is the failure mode here.
- [ ] **D1's amendment reads the same in the spine and in `CLAUDE.md`**, and the
      filter driver's inherited environment is stated in both.
- [ ] **The measured numbers are recorded with their hardware, commit ids and
      build profile**, and each rename gap against git is filed.
- [ ] **Q1-Q3 in `brainstorm.md` are answered or carried as filed issues**, and
      every issue from #29 to #37 that this packet turned out to touch is updated.
