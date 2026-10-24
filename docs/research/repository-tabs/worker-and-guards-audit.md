# The worker, the engine's process-wide pieces and the guards, for several repositories (as built at feature/fork-alignment b539d9d)

All paths are relative to the repository root. Worker files are under `crates/cairn-app/src/worker/`. Line numbers are as of b539d9d and rot; the symbols are the anchors.

## Summary

1. Each `worker::open` (pool.rs:87) starts 7 long-lived threads: cairn-repository, -history, -diff, -refresh, -network, -local and -askpass (askpass only when a channel opened). 10 tabs = 70 idle threads. On top of that come transient `cairn-git-stdout/-stderr/-stdin/-reaper` threads for each running git.
2. Already shared across the process: `Discovery` (one `OnceLock`, one probe), `Startup`, and `OperationId::next` (a static `AtomicU64`). Everything else is built per open: channel, socket dir, tokens, `Epochs`, `Wake`, `FetchControl`, `LaneState`, `Processes` registry and command log, zone read.
3. `PromptId` is NOT unique across repositories: each acceptor counts from 1 (askpass.rs:133-146). The window keeps one `prompt` and one modal state.
4. A prompt names only the operation ("origin", "Commit 1 file"), never the repository (`begin_for`, local/lane.rs:1605, network/lane.rs:192).
5. One fetch at a time per repository, not per app (`FetchControl` per open). One local write lane per open, but docs/design/concurrency.md:130-134 and worktrees.md:17 want it keyed by the common git dir, so two tabs on worktrees of one repository should share it. Not built.
6. Closing one repository is already worker-side and per repository: `Request::Close` arm, then `Threads::drop` on its own repository thread. The UI does not wait.
7. The app side assumes one repository: the `Closing` enum holds one handle, and main.rs:211-221 closes the window when the stream ends. A tab close needs its own end-of-stream result.
8. After Close, `serve` has returned and dropped its queue, so a later `Request::Retire` fails to send and its value is freed on the UI thread. A closed tab's History or status would be freed on the UI thread unless retired before the Close.
9. Idle cost: no worker thread polls (all block in `recv`/`accept`); nothing watches the filesystem; the UI timers run only while their component is mounted.
10. A refresh means a refs read, ahead/behind walks, a `git status` process (736 ms on a stat-dirty rust-lang/rust), an index.lock stat, maybe a full history reopen, and CommitReads. Today it runs on window focus for the one repository (window.rs:175).
11. Memory is never evicted: rows scrolled (52.7 MiB at 345k rows, issue #4), the held walk session (about 15 MB visited-set at 500k commits, or about 170 B per commit for a date-order walk without a commit-graph), up to 6 gix handles × 4 MiB object cache, command log ≤ 4 MiB, activity log ≤ 4 MiB, commit output ≤ 1 MiB, diff answers (Expand All ≤ 50k lines), status and refs snapshots.
12. `Update::Started` (git + zone) arrives once per repository but writes window-level `view.frame.git` and `view.zone` (session.rs:88-95).
13. Guard rows naming cairn-app paths: PRODUCT_SOURCE_DIRS, RENDER_SOURCE_DIRS, WORKER_DIR, ZONE_READER, CONFIRMED_HOLDERS (commit_box/state.rs GitError, local/lane.rs LocalWrite/Unspent), WINDOW_TOKEN_FIELDS, PROCESS_CALL_EXCEPTIONS (row text `history::state::Progress::status`), FONTS_DIR. FILESYSTEM_MUTATION_EXCEPTIONS names no cairn-app file; persisting tabs would need a new row (#89).
14. Not a guard row but part of the UI-thread contract: CLAUDE.md:1009-1110 and .claude/agents/responsiveness-reviewer.md list `main.rs`'s `use_hook` wiring, `Closing::*` and `session::apply`'s `withdraw` as exempt UI-thread calls.
15. `View` is built field by field in 6 places, and all of them must change: main.rs, window.rs test_view (757), session.rs tests, sidebar/tests.rs, local_changes/tests.rs, window_check.rs:413. window_check is single-handle and needs a worker-side helper (`update_within`) for any new waiting.
16. A `Replier` held as a struct field breaks the rule written at pool.rs:62-65, yet the secret guard cannot see it through the alias name `Replier`.
17. Text assuming one repository: CLAUDE.md:169-170, history-graph.md:15-19, repository_path.rs:6 ("Later arguments are ignored"), pool.rs:1143 WORKER_LOST ("Close Cairn and start it again"), and the "Closing the window closes its repository" sections in git-processes.md and credentials.md. git-processes.md:37-39 already says "each repository opens in a tab of its own".

## 1. What one `worker::open` creates

### 1.1 The call
`pool.rs:87` `open(path, &Discovery) -> Result<(RepositoryHandle, Updates, Replier), OpenError>` wraps `open_paced` (pool.rs:95). On the caller's (UI) thread it creates, with nothing touching disk:

- Channels: `jobs` (repository thread, pool.rs:103), `history_jobs` (106), `diff_jobs` (108), `refresh_jobs` (110), `local_jobs` (112), the update stream `outgoing/inbox` (114), and the credential-answer channel `answers/answered` (115).
- `LaneState::default()` (113): the local lane's write clock, its closing flag and the running write (local/lane.rs:1101-1120).
- `Wake::new()` (116, wake.rs): one wake per repository.
- `Epochs::new()` (117, epoch.rs:162): 15 lane counters and a `stopping` flag, per repository.
- `FetchControl::default()` (119, network/lane.rs:71): the one-fetch slot, per repository.
- One `Outbox` per thread (122-172). The stream ends only when the last of them drops (pool.rs:1092-1139).
- It returns a `RepositoryHandle` (pool.rs:747, `Clone`, holds every queue sender plus `lane`, `epochs`, `control`), `Updates` (pool.rs:1020, "Driven from exactly one task") and the `Replier` closure (pool.rs:290, `Rc<dyn Fn(Reply)>` around `answers`).

### 1.2 Threads per repository
The detached `cairn-repository` thread (pool.rs:179-266) does the following in order:
1. `discovery.git()`, which waits on the process-wide `OnceLock`.
2. Sends `Update::Started { git, zone: zone::read() }` (pool.rs:206-212). The zone is read again on every open (zone.rs:12).
3. `SharedRepository::discover_for` (pool.rs:218-231), which runs the ownership and bare-repository checks.
4. Sends `Update::Opened { name, path }`.
5. Builds `Reopening`.
6. `Backend::open` (startup.rs:91): one askpass `Channel` per repository, and the `git` pointed at it.
7. `WalkedRefs` and `Threads::start` (pool.rs:434-654), which spawns:

| Thread | Spawned at | Serves | Blocks on when idle |
| --- | --- | --- | --- |
| `cairn-repository` | pool.rs:179 | refs of a refresh, filters, `ListRemotes`, `CommandLog`, `Retire`, `Close`, forwards `Fetch` (`serve`, pool.rs:1172) | `jobs.recv()` (pool.rs:1186) |
| `cairn-askpass` | pool.rs:458 (only if `channel` is `Some`) | `serve_prompts` (askpass.rs:126) | `channel.accept()`; while a prompt is open, `answers.recv()` |
| `cairn-history` | pool.rs:493 | `serve_history` (history/lane.rs) | `recv()` (history/lane.rs:155); `try_recv` only while a find pages (144) |
| `cairn-diff` | pool.rs:527 | changes and file-diff lanes | `recv()` (diff/lane.rs:281) |
| `cairn-refresh` | pool.rs:559 | status, ahead/behind, index.lock stat | `recv()` (refresh/lane.rs:80) |
| `cairn-local` | pool.rs:594 | local write lane and its reads | `recv()` (local/lane.rs:1312) |
| `cairn-network` | pool.rs:621 | fetch | `recv()` (network/lane.rs:185) |

That is 7 threads, or 6 with no channel (no `$XDG_RUNTIME_DIR`). The module doc says "Seven threads per open repository" (pool.rs:4). The `Threads` doc comment (pool.rs:403) still says "The six threads the repository thread starts", which does not count the repository thread itself.

Transient threads per `git` invocation (cairn-git `process/runner.rs:214-245`, `group.rs:238`): `cairn-git-stdout` and `cairn-git-stderr` always, `cairn-git-stdin` when input is fed, and `cairn-git-reaper` when an unfinished invocation is dropped (runner.rs:788). At most about four gits run at once per repository: status on refresh, a diff read, a local write or read, and fetch. The history thread runs no git (pool.rs:721).

Process-wide threads: the UI/main thread, `cairn-discovery` (discovery.rs:58, which exits after the probe), and the toolkit's own (not audited). gix is built with `parallel` and `max-performance` (Cargo.toml:27), so some gix operations may use worker threads of their own (not audited).

With 10 tabs that is 70 resident worker threads (60 without a runtime dir), plus up to about 40 transient git pipe threads at peak, plus 10 askpass socket directories `cairn-<pid>-<rand8>` under `$XDG_RUNTIME_DIR` (channel.rs:141-145).

### 1.3 Handles to gix per repository
Each thread takes its own thread-local `Repository`. `Configured::open` (configured.rs:72) is used on the repository thread (pool.rs:1181), the history thread (history/lane.rs:110), the refresh thread (refresh/lane.rs:79) and the local lane (local/lane.rs:1311). `shared.to_worker()` is used on the diff thread (diff/lane.rs:95) and the network lane (network/lane.rs:182). Each handle gets `OBJECT_CACHE_BYTES` = 4 MiB (cairn-git repository.rs:291, 316), so 6 handles per repository. `Configured` reopens on demand only: it stats the configuration files before work and has no timer (configured.rs:1-19, 100-131).

### 1.4 routing.rs, epoch.rs, request.rs, wake.rs, zone.rs, startup.rs, configured.rs
- `routing.rs:1-36`: the table of lane → thread. `route()` runs on the caller's thread inside `RepositoryHandle::submit` (pool.rs:783-932). Every target is a per-repository queue.
- `epoch.rs`: `Epoch { lane, number }` (epoch.rs:153). Numbers are monotonic per lane *per `Epochs`*, so epochs of different repositories collide by value. Each `Updates` filters with its own `Epochs` (pool.rs:1043). `Epochs::stop` is final (epoch.rs:194).
- `request.rs`: `Request::Close` (request.rs:505) is documented as "What closing the window asks for". `Update::Started` (590) and `Update::Opened` (600) are each "Sent once … tied to no request". `Update::Prompt { id, text, asking }` (639). `MOST_LANES = 2` (509).
- `wake.rs`: one-slot latch. One per repository, one UI task each.
- `zone.rs`: `TimeZone::try_system()`, read per open.
- `startup.rs:17` `Startup` is "shared by every repository the application opens". `Backend` (startup.rs:78) is "The backend one repository's worker threads share".

## 2. Process-wide vs per repository

| Piece | Where | Today | Notes for several repositories |
| --- | --- | --- | --- |
| `git` found | discovery.rs:21-98, `Discovery::start` (49), `git()` (`OnceLock`, 80-89) | once per process, cloned into each open (pool.rs:101) | Already shared. `git_is_found_once_per_application_not_once_per_repository` (worker/tests.rs:258) opens 3 repositories *sequentially* on one Discovery; no test opens several concurrently. |
| `Startup` (env lookup, helper path) | startup.rs:18 | `Arc` inside `Discovery` | shared |
| Askpass channel and socket dir | `Backend::open` startup.rs:91-128, `Channel::open` channel.rs:131-186 | per repository: one dir `cairn-<pid>-<rand8>` (0700) and socket `askpass` (0600), removed on `Drop` (channel.rs:292-297) | N tabs = N sockets. The name is unique per open, so no collision. `CAIRN_ASKPASS_SOCKET` is set per repository's `GitEnvironment` (startup.rs:113-123). |
| Askpass tokens | channel.rs:109-117 `Live` map per Channel; `begin`/`begin_for` (194-211) | per channel | A token is good only at its own repository's socket (`UnknownToken`). `begin_for(name)`: fetch uses the remote name (network/lane.rs:192), a write uses `LocalWrite::name()` (local/lane.rs:1605). No repository name is carried. |
| Acceptor | askpass.rs:126-179; `AcceptorStop` (46-108), `STOP_DEADLINE` 1 s (56) | one thread per repository, one prompt at a time, blocks on the window's answer | `PromptId(issued)` counts from 1 per acceptor (askpass.rs:133,145-146), so ids collide across tabs. The window keeps one `prompt: State<Option<PromptView>>` (window.rs:59) and one `Modal` (modal.rs:19). session.rs:370-376 refuses a prompt unless *this view's* fetch or write is in flight. |
| `Reply` / `Replier` | askpass.rs:37, pool.rs:62-65 | per repository | pool.rs:63-64: "A plain callback, never a struct field … application state must not keep one." main.rs keeps it only in the `use_hook` tuple and a `Weak` (main.rs:186). |
| `OperationId::next` | local/lane.rs:74-81, `static NEXT: AtomicU64` | process-wide, "Never reused within the application" (72) | Already unique across tabs. |
| Network lane | network/lane.rs:66-140 `FetchControl`, `arm` refuses a second fetch (103) | one fetch at a time **per repository**, since `FetchControl` is created per open (pool.rs:119) | Several tabs can fetch concurrently. There is no app-wide limit. |
| Local write lane | `LaneState` per open (pool.rs:113) | per open | concurrency.md:130-134 and worktrees.md:17 intend one lane per *common git dir*. Two tabs on worktrees of one repository (or the same repository twice) would each get their own lane. Not built. |
| Process registry and command log | cairn-git process/registry.rs:62-75 `Processes`, one per `SharedRepository` (repository.rs:37, `Arc::default()` at 188) | per repository; log ≤ `LOG_ENTRIES` 1000 / `LOG_BYTES` 4 MiB (command_log.rs:22,34) | `end_all` ends only that repository's gits. |
| `CLOSE_BOUND` 3 s | registry.rs:52 | constant | |
| Ownership check | cairn-git ownership.rs; `Identity::of_this_process()` (233), `effective_uid` (253) reads `/proc/self/status`, fallback creates and removes a temp probe (280-291) | recomputed on every `discover_as`: each open and each `Configured::reopen` (repository.rs:67,93,205) | Process-wide in nature, nothing cached; safe to duplicate. The probe is a FILESYSTEM_MUTATION_EXCEPTIONS row. |
| Zone | zone.rs | read per open, sent in `Update::Started`, written to window-level `view.zone` (session.rs:91-94) | duplicate reads; last writer wins |
| git description | `Update::Started.git` → `view.frame.git` (session.rs:89-90) | per open into window-level `FrameView` | duplicate |
| `written_repository.rs` | worker.rs:23-24 `#[cfg(test)]` | a test fixture writing objects with `std::fs`; no git, nothing process-wide | Not relevant to production. Its doc notes "the guards scan this crate's tests, and none may build a `Command`". |
| `window_check_updates.rs` | `#[cfg(test)]`, `update_within(&mut Updates, wait)` | worker-side waiting helper for one `Updates` | A multi-tab window check needs a worker-side multi-stream equivalent: render-side files may not wait. |

## 3. Closing

### 3.1 As built (one repository = the window)
- UI: `closing.rs` `Closing(Rc<RefCell<Stage>>, Rc<RefCell<Option<Tell>>>)` has `Stage::{Unopened, Open(RepositoryHandle), Closing{since}, Gone}` (closing.rs:28-45). It holds **one handle**. `requested()` (63-83) submits `Request::Close` once and returns `KeepOpen`. A second request after `CLOSE_PATIENCE` returns `Close`.
- main.rs:155-221: `closing.opened(handle)`, `when_requested(|| writes.closing())`. On stream end, `if closing.worker_gone() { platform.close_current_window() } else { progress.stream_ended("the repository worker has stopped") }`.
- `RepositoryHandle::submit` Close arm (pool.rs:910-918): `epochs.stop()` (an atomic store), `lane.close()` (one bounded lock), then queue. The UI never waits.
- `serve` breaks on `Close` or `is_stopping` (pool.rs:1187, 1243). `jobs` is moved into `serve`, so its receiver drops when `serve` returns.
- `Threads::drop` (pool.rs:708-743) runs on the repository thread:
  1. `epochs.stop()`.
  2. Send `Stop` to history, diff and refresh; drop the network sender.
  3. `control.cancel()`, so the fetch is ended at once.
  4. Join history (its walk polls epochs between commits).
  5. `lane.close()`, send `LocalJob::Stop`, **join local** (unbounded: a commit in its hooks runs to the end, local/lane.rs:36-41).
  6. `shared.end_invocations(CLOSE_BOUND)` (repository.rs:249, registry.rs:117-129).
  7. `acceptor.stop()` (≤ `STOP_DEADLINE`; returns unacknowledged if a prompt holds the acceptor).
- `CLOSE_PATIENCE` 5 s (pool.rs:76) is const-asserted greater than `CLOSE_BOUND` + `STOP_DEADLINE` (78-81). It is a **window**-level patience in `Closing::requested`.
- `Drop for Updates` stops the epochs (pool.rs:1057-1062).
- The stream ends when every outbox drops. The acceptor leaves only once the window refuses or withdraws any open prompt (`session::apply` → `withdraw`, session.rs:508-512) or drops the `Replier`.

### 3.2 What closing ONE repository while others stay open would need (mapped, not designed)
- **Worker side: nothing waits on the UI thread.** All waiting (history join, local-lane join, reaps, acceptor stop) is on that repository's own `cairn-repository` thread. Other repositories' threads are independent: separate `Epochs`, `LaneState`, `Processes`, channel.
- **App side, which assumes one repository:**
  - `Closing` holds one `RepositoryHandle` and one `Tell` (closing.rs:28).
  - main.rs:211-221 turns "stream ended after a close was asked" into `close_current_window()`.
  - `CLOSE_PATIENCE` is consumed only by the window hook.
  - `Request::Close`'s doc says "What closing the window asks for" (request.rs:501-505).
  - A tab close needs its own end-of-stream outcome (drop the tab, not the window). A window close with N tabs needs N closes and N stream ends.
- **The closing line** ("Closing after Commit 1 file finishes. Close again to quit now.", operations/local_writes.rs:128-135, `LocalWrites::closing` at 93) lives in one view's `LocalWrites`. For a background tab waiting on a write there is no visible place to say it today.
- **Freeing a closed tab's state.** The codebase frees large answers on a worker (`Request::Retire`, pool.rs:1241). After Close, `serve` has returned and its receiver has dropped, so `self.jobs.send((epoch, Retired))` fails and the `SendError` holding the value is dropped on the UI thread (pool.rs:917). A closed tab's History, status or snapshots would therefore be freed on the UI thread unless retired *before* the Close or handed elsewhere.
- **A prompt open in the closing tab** holds its acceptor until withdrawn, and `stop()` gives up after 1 s (askpass.rs:82-94). The socket dir goes when the last `Arc<Channel>` drops (local and network lanes drop theirs first, pool.rs:609-611, 635-637).
- **`WORKER_LOST`** text: "Close Cairn and start it again to carry on." (pool.rs:1143-1144). That fits one repository per process.

## 4. Idle cost of an open repository that is not visible

- **Worker threads:** all seven block in `recv()` / `accept()`. Nothing polls. The history thread uses `try_recv` only while a find pages (history/lane.rs:144-153), and the diff thread's `try_recv` only drains after a blocking `recv` (diff/lane.rs:279-298).
- **No timers on the worker side.** The exception is the lock-retry sleep, during a refused write only (local/lane.rs:1231-1257, 50 ms slices, `LOCK_RETRIES` = 2 at 1580).
- **No filesystem watching:** "Nothing watches the file system" (refresh/focus.rs:3). Cairn starts no fsmonitor of its own. A status read under `core.fsmonitor=true` starts git's fsmonitor daemon, which outlives Cairn (CLAUDE.md, the D1 amendment). That is one daemon per working tree, outside Cairn's process.
- **Configuration following:** a stat per config file, only before work (configured.rs:9-10, 129-131).
- **UI-side timers** run only while their component is mounted:
  - commit box `Elapsed`: a 1 s loop while a commit runs (cairn-ui commit_box.rs:717-728)
  - status box: one-shot `PROGRESS_AFTER` (toolbar/status_box.rs:201)
  - lock banner: one-shot `LOCK_STALE_AFTER` (toolbar/lock_banner.rs:83)
  - edge scroll: during a drag (edge_scroll.rs:168)

  A background tab drawn by no component runs none of them. Its `Updates` task (main.rs:192) still applies every update to its view state.
- **Updates that still flow in the background:** `Update::Reading` at most every 100 ms during an opening pass (history/reading.rs), `FetchProgress` per redraw (issue #25, credentials.md), and write output.
- **Refresh on focus** (refresh/focus.rs:16-26): fires `Request::Refresh` on the window's `is_app_focused` false→true, wired once at window.rs:175 with the single `submit`. One `Request::Refresh` (pool.rs:806-816) causes:
  - The refs lane: on the repository thread, the refs snapshot is read (gix: refs, `HEAD`, stash list, upstreams, substitutions) and compared with `WalkedRefs`. `Update::Refs { reopen }` follows. A reopen is a full history open (without a commit-graph, the date-order opening pass reads every reachable commit: 1.19-1.20 s on rust-lang/rust, history-graph.md).
  - ahead/behind walks on the refresh thread.
  - Status: `git status --porcelain=v2 -z` as a process (736 ms on a stat-dirty rust-lang/rust, refresh/lane.rs:3-4). Coalesced, never superseded; only a close ends it.
  - A `stat` of `<gitdir>/index.lock` (refresh/lane.rs:24-25).
  - Window side on `Update::Refs` (session.rs:281-309): sidebar rows laid out on the repository thread, `CommitReads` on the local lane if Local Changes is the main view (commit_box/pane.rs:430-434), the selection asked again if substitutions changed.
  - A refresh is dropped while a commit runs (`defer_refresh`, pool.rs:786).
- **The same refresh after other events:** a fetch's ending (session.rs:484-489), a write's ending (`ReadAgain`, session.rs:274) and the initial open (main.rs:170).

  Today only the one repository refreshes. If every tab installs `on_focus_gained`, every tab pays the above on each focus.

## 5. Memory resident once opened (nothing is evicted)

Window state (one `View`, window.rs:54-130):
- `rows: State<History>`: linear in rows scrolled. 345,545 rows retain 52.7 MiB, process RssAnon +83 MiB; "nothing evicts: issue #4" (history-graph.md).
- `refreshed: RefreshState`: `Arc<RefsSnapshot>`, ahead/behind `BranchCounts`, `Arc<LocalChanges>` (refresh/state.rs:44-48). A status can list tens of thousands of paths (local/lane.rs:139-140).
- sidebar rows and filter rows; Local Changes lists and filter rows.
- `diff: DiffState`: change sets, file diffs, `ShownDiff`; Expand All up to `EXPAND_ALL_LINES` = 50,000 lines (worker/diff/expand_all.rs:18).
- `activity`: `ActivityLog` ≤ `LOG_BYTES` = 4 × `OUTPUT_BYTES` = 4 MiB (operations/activity.rs:53, output_tail.rs:18-20).
- commit box `StreamedOutput` tail ≤ 1 MiB; endings; `OutputTap` tails.

Worker state per repository:
- The held `HistorySession`, kept until the next `OpenHistory`. It holds gix-traverse's visited `HashSet` (about 15 MB at 500k commits, "never released while the app runs", history-graph.md). A date-order walk keeps about 170 B per commit read, which without a commit-graph is every reachable commit (history-graph.md).
- `WalkedRefs`: the latest snapshot and the one the walk was opened from (history/refs.rs:33-41).
- 6 gix thread-local handles × up to 4 MiB object cache (§1.3), plus the pack and index maps shared through the `ThreadSafeRepository`.
- Command log ≤ 1000 records / 4 MiB.

The only existing measurement point is `window_check.rs:378` `resident_kib()`, which reads VmRSS once per process.

Signs that background tabs might free state: the existing "retire to a worker" path (`Retired::of/history/refs/ahead_behind/status/sidebar`, request.rs:175) and issue #4. The history reopen machinery already drops old rows on a worker when new ones arrive (session.rs reopen; CLAUDE.md).

## 6. Guards a per-tab refactor of window state would touch

### 6.1 In `crates/cairn-guards/tests/invariants.rs`
- `PRODUCT_SOURCE_DIRS` (line 96, row `"crates/cairn-app/src"` at 100) and `RENDER_SOURCE_DIRS` (104): directory-level.
- `WORKER_DIR = "crates/cairn-app/src/worker"` (157), used by `the_ui_thread_never_waits_on_repository_work` (4983-5049), which partitions by FILE:
  - A new per-tab module outside `worker/` may not name `Receiver`, `Mutex`, `Condvar`, `JoinHandle`, a `channel`, `recv()`, `join()`, `lock()`, `wait()`, `sleep`, `park`, `block_on` or the spinning spellings (lib.rs:492-547), nor `cairn_git`, `gix` or `cairn_askpass` (5017-5032).
  - Each directory must contribute files on both sides (5037-5048).
- The UI-thread exemptions that guard relies on are prose, not a roster. CLAUDE.md ("Residual obligations") names:
  - `RepositoryHandle::submit` and its arms, `into_submitter`
  - `Closing::requested` in `closing.rs`, `Closing::when_requested` / `opened` / `is_requested`
  - `OperationId::next`, `local_writes::ask`, `LocalWrite::name`
  - the `Retired` constructors in `worker/request.rs`
  - "`worker::open`, called from `main.rs`'s `use_hook`, and the `Replier` closure"
  - `OutputTap::drain`, `Updates::next`, `Wake::poll`, `Drop for Updates`, `Discovery::start`
  - `session::apply`'s `withdraw`, `leftover_lock::ask`
  - "`main.rs`'s wiring … which no test drives", and "`crates/cairn-app/src/main.rs` holds exactly such a value today" (the `Updates` receiver, awaited)

  The same list is in `.claude/agents/responsiveness-reviewer.md:35-54`. Moving the open, the update loop or the close wiring out of main.rs into a tab module means updating both texts.
- `PROCESS_CALL_EXCEPTIONS` (1617-1625): `("crates/cairn-app/src", "status", "history::state::Progress::status, …")`.
  - The row is directory-wide and only checks that some file still calls `.status()` (1708-1718). Moving `Progress` keeps it passing, but the row's text names `history::state::Progress::status`.
  - Any new nullary `.status()` in cairn-app (a tab's `status()`) is silently excused.
  - The matcher test uses `crates/cairn-app/src/window.rs` and `view.progress.read().status()` as a fixture (3964-3975).
- `ZONE_READER = "crates/cairn-app/src/worker/zone.rs"` (6188), `the_render_layer_is_handed_its_time_zone` (6195-6237). If the zone becomes process-wide and is read once elsewhere, this path moves.
- `CONFIRMED_HOLDERS` (6728-6733):
  - `("crates/cairn-app/src/worker/local/lane.rs", "LocalWrite")`
  - `("crates/cairn-app/src/worker/local/lane.rs", "Unspent")`
  - `("crates/cairn-app/src/commit_box/state.rs", "GitError")`
  - `("crates/cairn-git/src/error.rs", "Error")`

  A row is by file *and* type; a moved file or type fails it (`confirmed_holder_violations`, 7235-7286). Any per-tab struct whose body names `Confirmed` (not `Confirming`) fails unless rostered. Holding a `GitError` or `CommitBoxView` by type is fine.
- `WINDOW_TOKEN_FIELDS` (6737-6748): `(local/lane.rs, "Unspent", "token: Option<Box<Confirmed>>,")` and `(commit_box/state.rs, "GitError", "unspent: Option<Confirmed>,")`. `the_window_holds_a_token_only_for_the_git_errors_skip` (6778-6795) finds the file by exact path in `rust_sources("crates/cairn-app/src")`.
- `destructive_operations_are_sealed_behind_the_confirmation_token` (7525-7646) requires a scanned production file under `crates/cairn-app/src/worker` (7583-7587) and under `crates/cairn-app/src` (7619-7623). Its matcher test uses `worker/local/lane.rs` and `worker/local/queued.rs` as fixtures (8107-8108).
- `CONFIRMATION_SURFACES` (6676) and `DESTRUCTIVE_OPERATIONS` (6687) name cairn-ui and cairn-git files only.
- `SECRET_HOLDERS = &[]` (8408) and the transitive container scan (8547-8593):
  - `Reply` (holds `Secret`) is a container, so a struct with a field naming `Reply` fails.
  - A field typed by the alias `Replier` is not seen (the ident differs, and aliases are not containers), so a tab struct keeping the replier would pass the guard while breaking pool.rs:62-65.
- `FILESYSTEM_MUTATION_EXCEPTIONS` (1244-1263): rows for `crates/cairn-askpass/src/channel.rs` (socket dir) and `crates/cairn-git/src/ownership.rs` (euid probe). No cairn-app row today. Its doc says "A later write outside any repository — a settings store (#89) — adds a row of its own", which applies to persisting open tabs. `OPS_FILESYSTEM_WRITES` (1269) is `ops/remove_lock.rs` only.
- `FONTS_DIR = "crates/cairn-app/assets/fonts"` (10276): unaffected.
- Fixture-only path strings (labels, not rosters): 3965/3972 (`window.rs`), 5285, 8227, 8262, 8301, 8354, 8362.

### 6.2 `crates/cairn-guards/src/lib.rs`
Generic matchers only. No cairn-app paths or symbols. The rosters relevant here are `WAITING_IDENTS` (492) and `WAITING_NULLARY_CALLS` (526), and `SYSTEM_ZONE_*` (3059-3073). The `TimeZone` type may be named elsewhere; only `try_system`, `TimeZone::system` and the like are banned outside zone.rs.

### 6.3 Other enforcement naming cairn-app paths
- `.claude/hooks/qa-stop.sh:123-129`: only `crates/cairn-app/src/worker/` may name `gix`/`cairn_git`/`cairn_askpass`, and the worker may not name `freya`/`dioxus`. Tested by `crates/cairn-guards/tests/debris_hook.rs:319-339`.
- `.claude/agents/destructive-ops-reviewer.md:72,195` names `operations/create_branch.rs` `discard_confirmed` and `worker/local/lane.rs` `LocalWrite`.

### 6.4 Construction sites a per-tab `View` split must update
`View` is built field by field in:
- crates/cairn-app/src/main.rs:120-154
- crates/cairn-app/src/window.rs:757 (`test_view`)
- crates/cairn-app/src/session.rs (tests)
- crates/cairn-app/src/sidebar/tests.rs
- crates/cairn-app/src/local_changes/tests.rs
- crates/cairn-app/src/window_check.rs:413-447

`window(opened, view, submit, answer)` (window.rs:143) and `session::apply(update, view, &Worker{submit, refuse, closing})` (session.rs:19-35) each take one repository's view and one submit.

### 6.5 window_check.rs and window_check_updates.rs
- `window_check.rs` is `#[cfg(test)]` (main.rs:20-21) and render-side, so the waiting guard scans it and it may not wait. It drives one repository:
  - `Harness { test, view, updates: Updates, handle: RepositoryHandle }` (window_check.rs:200-205)
  - `launch(path)` calls `Discovery::start()` and `open(path, &git)` (394-397)
  - it submits `ListRemotes`, `ConfiguredContext` and `Refresh` as main does (453-456)
  - it renders `window(PATH, view, submit, None)` (410)
  - it builds the `View` literal (413-447)
  - checks: `window_check` (472), `opening_pass_check` (735), `local_changes_check` (890), `writes_check` (1162); memory by `resident_kib` (378)
- `worker/window_check_updates.rs:15` `update_within(&mut Updates, Duration)` is the worker-side wait it paces frames by. Pacing several tabs needs an equivalent over several `Updates`, written on the worker side.

## 7. Text assuming one repository per process

- CLAUDE.md: "opens the repository named on its command line", "closing the window closes its repository", "There is no repository picker: one repository, named on the command line.", "`src/closing.rs` is the window's close hook, which asks the worker to close".
- docs/systems/history-graph.md:15-19 ("opens the repository containing the path named on its command line … there is no repository picker"), 691 ("closing the window does not wait…").
- docs/systems/git-processes.md:
  - 27-28 ("closing the window closes its repository")
  - 1662-1720 "Closing" (`Closing::requested`, "the window closes on the stream's end")
  - 149 and 173 ("git found once per application", correct for tabs)
  - 37-39 already says "Cairn is a multi-repository tool: each repository opens in a tab of its own, named by its path", the stated intent behind the launch-environment rule
- docs/systems/credentials.md:364 (git found once per application), 567-576 ("Closing the window closes the repository first … the socket directory removed … names are per pid and random, so nothing collides").
- docs/systems/window-frame.md:50, 251: the desktop title is `<repository> — Cairn`. frame.rs:107-116 `title(repository: Option<&str>)`.
- docs/design/concurrency.md:14-23 ("A few routed threads per repository"), 81 ("two write lanes per repository"), 115-120 ("Closing the window while a local write runs waits for it"), 130-134 (lane keyed by common dir).
- Code comments and text:
  - pool.rs:1-6 ("Seven threads per open repository"), 67-75 (`CLOSE_PATIENCE`: "the window"), 85 ("once per application … never per repository"), 1141-1144 `WORKER_LOST`
  - request.rs:501-505 (`Close`: "What closing the window asks for"), 590-604 (`Started`/`Opened` "Sent once")
  - closing.rs:1-15 ("Closing the window closes its repository")
  - discovery.rs:1-8 ("every repository the application opens takes a copy")
  - startup.rs:16
  - repository_path.rs:6 ("Later arguments are ignored.")
  - refresh/focus.rs:14-15 ("`submit` is `None` with no repository open")
  - main.rs:107-108 ("What the status box, the sidebar's header and the window's title call the repository"), 155-221 (one `worker::open` in one `use_hook`, one `spawn`ed update loop, `close_current_window` on its end)
  - history/lane.rs:1 and diff/lane.rs:1 ("of one repository": accurate per worker)
- window.rs `View` field docs say "kept for the session" for `detail_tab`, `pane_split`, `diff_settings`, `filter_text`, `show_lost`, `activity` ("the session's operations"), `zone` ("read once on the worker") and `frame` ("the `git` About names"). These are the fields where window-wide and per-repository meaning will have to be decided.
