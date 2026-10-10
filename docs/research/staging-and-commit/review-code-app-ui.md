# Review: app + UI half of staging-and-commit (cairn-app, cairn-ui, cairn-guards)

Scope read: worker/{local_lane,output_flow,pool,request,routing,epoch}.rs, local_writes.rs, activity.rs, shown_output.rs,
commit_box_state/pane.rs, create_branch.rs, local_changes_actions/pane.rs, diff_state*.rs, session.rs, shortcuts.rs,
ui: staging_gesture, local_changes_drag, edge_scroll, text_field, activity_popover; guards rosters; progress.md fix rounds.
Method: read the code and traced fix commits (git show) — not every file read end to end; line numbers are in the worktree.

## Verdict
Not "hacks everywhere". The seams (ui/app/worker partition, the Confirmed token, the single text-field policy, narrow guard
rosters) are sound and the engine-facing design is disciplined. But the **app/UI state layer** shows a recurring pattern:
a new feature copies the previous feature's bookkeeping instead of generalising it, and each QA round then adds one more
flag/handler to that copy. Four places are genuinely HIGH (will keep producing bugs). Most of the churn cost is in code
that is already well tested, so a refactor is mechanical, not risky.

Counts: HIGH 4, MEDIUM 6, LOW 4 (+ 9 good things).

---------------------------------------------------------------------------------------------------------------------
## HIGH

### H1. Who owns the file-diff lane: four independent booleans cross-reset at every site
- Where: diff_state.rs:68 (`Opening.in_lane`), :108 (`file_in_lane`), :117 (`working_in_lane`), diff_state/together.rs:56
  (`Together.in_lane`); cross-resets at diff_state.rs:139-141, 179-183, 394-398, 434 and `together_lost_lane()`.
- History: `working_in_lane` came with #83 (refs-and-status); `together.in_lane` with 59fd0ef (this packet). Each new user of
  the lane added a flag and edited every other user's reset.
- Symptom class: a diff that waits forever / is never re-asked after another view took the lane; `*_needs_asking()` predicates.
- Root cause: lane ownership is single-valued (the diff thread runs one file-diff query at a time; a newer one supersedes)
  but is stored as N independent bools, so "exactly one holder" is an invariant nobody states.
- Clean design: `lane: Option<LaneHolder>` with `enum LaneHolder { File, Working, Together, Expansion }` and one
  `take_lane(holder)`; `needs_asking(me) = holder != Some(me) && waiting`. The Request that takes the lane is built in one place.
- Cost: ~300-400 lines churn in diff_state*.rs; existing tests stay (they assert behaviour). Risk low.

### H2. Drag lifecycle implemented three times, each with its own lost-release patches
- Where: cairn-ui/src/local_changes_drag.rs:75-150 (global move + global press + global down + `on_mouse_down` + Escape),
  staging_gesture.rs:527-660 (`released`/`finish`/`on_root` + `use_drag_ends_on_focus_lost`), edge_scroll.rs:15-20,109-119
  ("a drag also ends when the button goes down again", focus lost).
- Trace: 4b60da0 (drag + "end an edge scroll whose release the window never heard", phase 06 QA 16) -> 01942eb (a text field
  cancels the global down, so also hear the platform `mouse_down`) -> dc8a682 (line drag copies the global-down trick) ->
  local-changes docs list a *named residual* (press on another view's text field after a lost release is not heard).
- Root cause: Freya 0.5 forgets the button as the pointer leaves the window and swallows `pointer_down` in text fields; every
  drag re-derives "how do I notice my release was lost" independently. Fighting the toolkit three times, three slightly
  different ways.
- Clean design: one `PointerDrag` primitive in cairn-ui: arm -> threshold -> dragging -> end(reason {Drop, Lost, Escape,
  FocusLost}), owning the global listeners, the focus-lost effect and the edge-scroll timer; consumers supply `on_drop`.
  Lost-release rules then live (and are tested) once, including the text-field residual.
- Cost: ~600 lines moved/merged; tests/edge_scroll.rs, local_changes_actions.rs, staging_gesture.rs already pin behaviour.

### H3. git output: four bounded queues, ~six scrub sites, a hand-rolled flow-control protocol
- Where: worker/output_flow.rs (HELD_LINES 10_000 / HELD_BYTES 1 MiB + `OutputReceipt` budget), commit_box_state.rs:25-28,100-145
  (`OutputTail`, OUTPUT_LINES 10_000 / OUTPUT_BYTES 1 MiB — "the same bounds"), activity.rs:36-42 (ACTIVITY_LINES 10_000,
  ACTIVITY_BYTES 4 MiB, plus `let_go`, `streamed`/`commands`/`drawn` stores), cut by `activity::cut_line` and the engine's
  own cut; scrub in local_lane::scrubbed_error/ran_since, network_lane.rs:259-262, session.rs:110,158,195 (`shown_ending`),
  create_branch.rs:398-405, commit_box_state (ShownLines), activity. `strip_ansi` lives in commit_box_state.rs:414 and is
  imported by shown_output.rs (inverted layering).
- Trace: b34d555 (bounded reads + scrub "every drawn line") -> dd073f7 (engine says where output was cut) -> d017152 (log
  recounted per line, 256 KiB line drawn uncut) -> phase-11 QA TC5/TC8 (cut-by-length, wide lines) -> merge-bar dismissals
  "a deliberate second pass".
- Root cause: output has no owner or type. The outbox is unbounded, so the lane invented a byte-budget protocol with a
  Drop-based receipt (output_flow.rs:36-67: `Clone` returns "nothing", `PartialEq` is always true — a type that lies about
  equality so Update can derive PartialEq). Scrubbing is a property of *text*, but is done per consumer, so every new sink is
  a new chance to forget, and the code deliberately scrubs twice (lane + window). The module's own "Residual" (held lines
  stall until git's next read) is a known lost-latest-lines bug accepted.
- Clean design: (a) `ScrubbedText`/`ScrubbedLines` newtype produced exactly once, at the lane boundary, from engine output +
  its cut offsets; no UI code takes `String` from git. (b) one `BoundedTail { lines, bytes }` used by lane hold, commit tail
  and activity entry. (c) replace the receipt scheme with a shared bounded tail the window drains on a wake (latest-wins is
  then natural and the residual disappears).
- Cost: ~700-900 lines touched; highest value of the four because it is where the security-relevant invariant (no token
  drawn) currently depends on every call site remembering.

### H4. Per-write bookkeeping fans out to five holders, each matching OperationId itself
- Where: session.rs:187-230 (`WriteEnded` calls commit_box_pane::write_ended, create_branch::write_ended, activity.write_ended,
  writes.ended; plus `Acting` in local_changes_actions.rs:67-83); local_writes.rs:14-37 (`Asked`: what/name/awaited/replaces/
  cancellable/prompt copied from `LocalWrite`) -> activity.rs `Activity::new(key, name, replaces, cancellable)` copies
  again; `withdraw(prompt)` guard duplicated at 3 sites in session.rs (`!writes.is_running()` / `!fetch.is_in_flight()`).
- Trace: c9cc056 "quote a destructive write's prompt however it ended" — the token is spent by the verb so the ending can't
  carry the prompt back; fix = copy the prompt at ask time into `Asked` (a third copy of a fact `LocalWrite` already owns).
  b24a683/3893c2e: close-handling ordering across lane flag, epochs, fetch control.
- Root cause: no single ledger of "operations this session asked". Every view that cares re-keys by OperationId and
  re-derives its own copy of the metadata.
- Clean design: `LocalWrite::describe() -> WriteInfo` computed once; one `WriteLedger` (queued/running/ended) that holds
  `WriteInfo` and an `on_ended` continuation registered at ask time (the `Confirming.then` pattern already in the codebase);
  activity popover and commit box read the ledger. New write kinds then touch one match, not five.
- Cost: ~500 lines; moderate risk (ordering of commit-box vs refresh on ending is the subtle part).

---------------------------------------------------------------------------------------------------------------------
## MEDIUM

### M1. Request / Routed / LocalJob / Update / QueryLane: five-way mirror per query; cross-lane rules hand-coded in submit
- Where: worker/request.rs (Request 36 variants, Update 42), routing.rs (route/unroute/Routed — +209 lines), epoch.rs
  (15 lanes), pool.rs:623-769 `submit`, local_lane.rs:760-790 (`LocalJob`).
- Evidence: every local read added ~7 edits. `StopCounting`/`StopAmending` are Requests whose only job is bumping an epoch
  ("Nothing sent: numbering the lane is all it is for"). `Write` returns `lanes() == []` yet pool.rs:670 bumps
  `QueryLane::Amending` by hand — the one supersession outside the `lanes()` table. `defer_refresh()` (pool.rs:626) takes the
  lane mutex on the UI thread. Five arms in submit are the same shape (`if let Some(epoch) { send(LocalJob::X{..,cancel:
  watch(epoch)}) }`).
- Knock-on guard debt: CLAUDE.md's UI-thread "exempt arms" prose paragraph grows with each variant (phase 12 added
  CheckBranchName/CheckoutConsequence/LockConsequence) — a roster-by-prose that exists because `submit` does per-variant
  logic on the UI thread.
- Clean: declarative lane table (variant -> {lane, supersedes[], thread}); `submit` = bump + one channel send, with variant
  logic on the receiving thread; "stop" = a lane-bump API, not a Request.
- Cost: medium (~500 lines), mostly deletion. Do before the next verb is added.

### M2. "Ask the engine what it costs, then confirm" built four times
- Where: local_changes_actions.rs:67-145 (`Acting.asked/arrived/said`), activity.rs `ActivityLog.lock_asked/removing/
  lock_ready` + `Activity.lock_named/lock_note`, create_branch.rs:53-72 (`Opened.counting/refused/arrived`),
  commit_box_state.rs:73-82 (`Amendable{serial,consequence}` + `reading_amend`). Each has its own Request, Update and (for
  three) its own QueryLane (DiscardCount, CheckoutCount, Amending; LockConsequence has none).
- Each re-implements asked-id, newest-wins, arrived, taken, refused. `Confirming` is the shared half and is good; the
  *ask* half is not shared.
- Clean: `PendingConsequence { asked: OperationId, state: Reading | Ready(Consequence) | Refused(String) }` plus one
  `Request::Consequence(kind)` / `Update::Consequence` on one lane. Cost ~400 lines.

### M3. Amend swaps the Local Changes data source, so the Status path forks on a commit-box flag
- Where: session.rs Status arm (`status_arrived_amending` -> if amending, the status is NOT given to LocalChangesState and an
  `Amending` request goes out instead), commit_box_pane.rs:442-503, commit_box_state `amend_asked/reading_amend`; "a status
  is held twice" (refreshed + drawn) per CLAUDE.md.
- Root cause: amend is modelled as a mode that replaces the lists' source rather than a layer over the same status.
- Clean: Status always lands in LocalChangesState; amend contributes an `Option<AmendStaged>` overlay applied by
  `LocalChanges::amending` when drawing. Removes the fork, the `StopAmending` request and `Amending` lane-bump in Write.

### M4. Selection / chosen path / together-diffs kept in sync by an effect with a loop-avoidance comment
- Where: local_changes_pane.rs:278-328 `follow_the_lists` ("a write wakes the effect again, so a decision gone wrong here
  must cost one more look, never a loop"); three representations: `view.local.selection` (ListSelection),
  `diff.working_choice()` (WorkingChoice{list,path,lists serial}), `Together{entries, lists, drawn}`; `TogetherWanted`
  compared against what is drawn to decide whether to ask.
- Trace: 4c1c891 (refresh took paths away, selection kept them, an action took them unseen), c66d65e (selection over files
  drawn together survived a re-ask; add `drawn` renumbering and carry the number on every act), 1747e77 (adding a path blanked
  the others; carry `previous` per path by hash), be847b8 (Stage All vs filter).
- Root cause: the selection is not the single source; choice and together are derived copies synchronised after the fact,
  and each stale-selection bug gets a numbering token (`drawn`, `answered`, `previous_answered`, `lists`, `answers`).
- Clean: one `Selection` owned by LocalChangesState (paths + anchor), one pure `reconcile(lists, selection, settings) ->
  Vec<Request>` called from the Status/Filter arrival in `session::apply`, not from a render effect.
- Cost: ~500 lines; the biggest long-term source of "refresh interacts with selection" bugs.

### M5. "Keys inert" and modal handling are not one mechanism; Create Branch is missing from it
- Where: shortcuts.rs:34 `keys_inert` = prompt || confirming || commit-box Git Error || activity popover open. Does NOT
  include `CreateBranchState.open` or the branch Git Error (`view.branch.state.error()`), which is a parallel
  `BranchError`/`GitError` pair (create_branch.rs:92-100 vs commit_box_state.rs:84-96). `local_changes_actions::dialog_open`
  (:218) is a one-line alias; window.rs:433/441 and lost_commits.rs:14 each call it. Each dialog separately sets
  `a11y_modal(true)`; the popover also has its own `on_global_key_down` Escape (activity_popover.rs:569).
- Not verified as a live bug (a11y_modal/Popup may swallow keys); but when a fifth dialog is added, someone must remember to
  edit `keys_inert`. Clean: a `Modal` enum/stack in window state (`Option<Modal>`), `keys_inert = modal.is_some()`.
- Cost: low-medium; also merges GitError/BranchError.

### M6. Activity log is a small database inside one struct
- activity.rs:110-300: `Activity` has three line stores (`commands`, `streamed`, interior-mutable `drawn: RefCell<Option<Rc<..>>>`
  cache inside a value held in a `State`), `let_go`, `bytes`; `ActivityLog` also holds the lock-removal state machine
  (`lock_asked`, `removing`, `lock_ready`), popover anchor, selection, fetch counters. 1344 lines (about half tests).
- Cause: d017152 (O(n) per streamed line) fixed by counters + cache; the lock offer (decision H) bolted on afterwards.
- Clean: extract `LockOffer` and reuse H3's `BoundedTail`; the popover reads a snapshot, no RefCell cache.

---------------------------------------------------------------------------------------------------------------------
## LOW
- L1. `impl Clone for LocalWrite` is `#[cfg(test)]`, 55 lines, panics for five variants (local_lane.rs:167-225); production type
  shaped by tests, and the guard roster comment (`CONFIRMED_HOLDERS`) has to mention it. Compare via Debug/`matches!` or split
  `StagingWrite` (Clone) from the token-carrying variants.
- L2. Orphaned doc comment: pool.rs:2362-2366 ("Staging-and-commit R4.9: a close marks the local lane closing…") sits on top of
  the unrelated `a_write_supersedes_the_amend_read_and_nothing_else` test; the close test below it lost its comment.
- L3. Guard-of-guard coupling: `LOCAL_CHANGES_BARE_KEYS_DECLARATION` pins the literal text `const LOCAL_CHANGES_BARE_KEYS:
  [NamedKey; 3] =` and `ACCELERATOR_PIN_BODY` pins source lines of a test; fine as a ratchet but formatting-sensitive.
  invariants.rs is now 8,896 lines (was 5,810) in one file; lib.rs 3,182. Split by invariant family before it becomes unreviewable.
- L4. Serial sprawl for identity keys (Confirming.serial, GitError.serial, BranchError.serial, Opened.serial, Amendable.serial,
  ui dialogs' `serial` as component key). Mostly justified (3f41cd6: Freya reuses dialog state across chained dialogs), but
  one `DialogId` generator would remove three `next_serial` copies.

---------------------------------------------------------------------------------------------------------------------
## Guard rosters: did they grow to fit code?
Mostly no — this is the strongest part of the packet.
- `CONFIRMED_HOLDERS`: one row, keyed by file AND type, fails if a second holder appears in that file or the row goes stale
  (983d0f3). `OPS_FILESYSTEM_WRITES`: one row (remove_lock.rs: `remove_file`). `FILESYSTEM_MUTATION_EXCEPTIONS`: two rows, each
  with a stated reason and exact kinds. `UNBOUNDED_VIEW_EXCEPTIONS` and `SECRET_HOLDERS` stay empty. `TEXT_FIELD_EXCEPTIONS`:
  one row (the accelerator table). `TEST_ONLY_CFGS`: one entry.
- Where growth is a smell rather than strength: (1) CLAUDE.md's prose "UI-thread exempt arms" list grows per Request (see M1);
  (2) the test-only Clone exception (L1); (3) guard code volume (+3,174 invariants.rs lines) is mostly matcher self-tests, but
  it is the single largest file in the packet and sits at the maintenance edge.
- No sign of rosters being loosened to let code through; several QA rounds (87cbe4d, a31ca90, 983d0f3, e38b3b6) tightened them.

## Freya workarounds that are really fighting the toolkit
1. Lost pointer release / swallowed pointer-down in text fields (H2). Three bespoke recoveries.
2. `text_field.rs`: re-implements Freya `Input`'s key policy via `on_pre_key_down` + accelerator table so chords reach the
   window. Contained (one file, one guard, `every_text_field_takes_the_shared_key_policy`) — proportionate.
3. Edge auto-scroll with `async_io::Timer` (edge_scroll.rs) because Freya scrolls nothing at a drag's edge: one new dependency,
   user-approved, pinned in deny.toml. Proportionate, but it is the third piece of the drag machinery (H2).
4. Dialog identity by `serial` (3f41cd6) and dialogs' `a11y_modal` + per-dialog Escape: toolkit state reuse; see L4/M5.
5. Popover hung from the status box via `on_sized` global area and a 45-degree rotated square as an arrow
   (activity_popover.rs:540-580): fine, ~60 lines.

## What is good (be fair)
1. Layer partition holds: `window.rs` production code is ~620 lines (rest tests); UI crates stay engine-free; guards prove it.
2. `Confirming` (confirming.rs) — a continuation that receives the `Confirmed` token and never stores it; the right shape and
   what H4 should copy.
3. `text_field.rs` + `accelerators::field_key`: one policy, one guard, no literal modifiers in components.
4. `LaneState::stamp/if_unchanged` (local_lane.rs:819-845): odd/even tick plus send-under-lock is a compact, correct answer to
   "drop a read that raced a write"; pinned by tests.
5. Scrubbing keyed to the *engine's own cut offsets* (`scrubbed_at`, TC5) rather than guessing by length is principled — it
   just needs to happen in one place (H3).
6. `Together` renumbering + carry-over by hash works and is measured (1,000 files drawn in ~27 ms, slowest frame ~10 ms).
7. Test culture: every fix round added a failing-first pin with a "Caught by:" line; virtualization twins extended to new lists;
   frame-time measurements under hook floods (window_check.rs).
8. Guard rosters are narrow, keyed, self-checking and shrink-resistant (above).
9. Dependency discipline: one new dependency (async-io), already in the lockfile, feature-pinned, user-approved.

## Suggested order if the owner wants to pay down
1. H1 (lane owner enum) — smallest, removes a bug class. 2. H3 (ScrubbedText + BoundedTail) — safety-relevant.
3. M1 + M2 together (declarative lanes, one consequence ask) before the next verb. 4. H2 (PointerDrag) before more drags.
5. H4/M4 (ledger, selection reconcile) as the staging UI settles. L-items opportunistically.
