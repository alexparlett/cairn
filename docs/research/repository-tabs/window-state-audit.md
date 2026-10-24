# The window's state, per repository and per window (as built at feature/fork-alignment b539d9d)

## Summary

1. The repository's identity lives in four places: `opened` (the path as typed; main.rs:163-167, passed to `window(opened, …)`), `View::repository` and `View::repository_path` (set by `Update::Opened`, session.rs:96-100), and the `(RepositoryHandle, Updates, Replier)` triple from `worker::open`. The triple is held in main.rs's `use_hook` (169-230), the spawned task and `Closing`'s `Stage::Open`.
2. `View` (window.rs:54-130) has 33 fields. About 23 are per-repository, 4 are window-wide (`held_keys`, `zone`, `frame`, and `activity.open` as a modal) and about 10 are ambiguous: `detail_tab`, `pane_collapsed`, `pane_split`, `diff_settings`, `changes_list_width`, `show_lost`, `sidebar.width/shown`, `local.list_width/lists_split`, and the activity log.
3. `diff_settings` is not cleanly window-wide: its context is seeded from each repository's `diff.context` (`Update::ConfiguredContext` → `diff::actions::configured`, diff/actions.rs:368).
4. `Update` carries no repository tag. There is one `Updates` stream per repository, each with its own `Arc<Wake>` (a one-slot, single-waker signal) and its own epochs, and each must be driven by exactly one task.
5. `session::apply(update, view, &Worker{submit, refuse, closing})` writes one `View`. It writes window-wide fields from per-repository updates: `Started` → `frame.git` and `zone`; `ConfiguredContext` → `diff_settings`.
6. `Closing` (closing.rs) holds one `Stage`, one handle and one `Tell`. A stream's end calls `close_current_window()` (main.rs:212-215). Closing a tab needs a per-repository close that does not close the window; closing the window needs every repository closed.
7. `LocalWrites::closing` and `closing_line` are per-repository state, but the window draws them as "about the window" (window.rs:251-254).
8. Every action goes through `shortcuts::act(action, view, submit)` against the one `View` and submitter. The only window-only actions are ZoomIn/ZoomOut (via `view.frame`), Exit and About. `modal::modal`/`drawn` mix per-repository dialogs with About.
9. The menu bar, toolbar and fetch refusal take "a repository is open" from `submit.is_some()`.
10. The credential prompt is modal over the window but has no repository identity. `PromptId` is a per-acceptor counter starting at 1 (worker/askpass.rs:133-146), so ids collide across repositories. Each repository has its own `Replier`, and `window()` takes exactly one.
11. `Update::Prompt` is refused unless that repository's fetch or write is in flight (session.rs:370-376).
12. Title: the caption row shows only `CAPTION_TITLE` "Cairn" (cairn-ui caption_row.rs:20,170). The OS title is `<repo> — Cairn` via `frame::use_title`. The repository's name is drawn by the toolbar's status box (toolbar.rs:33-36) and the sidebar header (sidebar/pane.rs:77). Fork's tab strip sits under the toolbar, above the banners, i.e. between window.rs:250 and 253.
13. Process-wide singletons that are safe with several repositories: `Discovery`'s `OnceLock`, `OperationId::next`, `Confirming`'s `NEXT_SERIAL`, `History` `NEXT_SERIAL`, and empty `LazyLock` sentinels. The askpass `Channel` is per repository (`cairn-<pid>-<random8>/`), so each repository has its own `CAIRN_ASKPASS_SOCKET` and acceptor thread.
14. Not safe: `PromptId`, `Endings`/`CreateBranchState`/commit-box serials (per-state counters used as dialog `DiffKey`s), and `ActivityKey::Fetch(n)` (per log).
15. Freya's `use_side_effect` is `use_hook(|| Effect::create(cb))`, so every effect captures the first `View` and submitter forever. Affected: `use_title`, `on_focus_gained`, `use_lock_confirmation`, the find-supersede effect, the sidebar and Local Changes filter effects, `use_amend_confirmation`, and CreateBranchPane's name effect.
16. Eight components' `PartialEq` compare `submit.is_some()` only ("the submitter is the same repository's"), so they would not re-render on a tab switch to another repository's submitter.
17. `LocalChangesPane`'s `use_drop` sends `StopCounting` and drops a counting confirmation on unmount. A tab switch that unmounts it would cancel a discard count.
18. Tests: 6 literal 33-field `View` constructions (main.rs:127, window.rs:763, local_changes/tests.rs:79, sidebar/tests.rs:154, session.rs:571, window_check.rs:413), 6 `*::created()` sub-view constructors, `window(...)` called at 6 sites, and `session::apply(` at 9 sites. About 209 tests build a `View`, out of 312 render-side tests.
19. 170 render-side functions take `view: View` (local_changes/actions 22, commit_box/pane 21, create_branch 18, window 17, diff/actions 16, …). There are 13 `let View { .. } = view` destructurings.
20. Line counts: window.rs 5709 (592 production), session.rs 2054 (513 production), main.rs 238, closing.rs 205, frame.rs 207, shortcuts.rs 100, modal.rs 77, menu_bar.rs 107, toolbar.rs 121, worker/pool.rs 2711.

---


Worktree: `the feature/fork-alignment worktree`, detached at `b539d9d` ("docs(app): name every reader of the repository's name and the refresh's answers"). Scope: `crates/cairn-app/src/` on the render side. `worker/` is cited only where the render side depends on its API. All paths below are relative to `crates/cairn-app/src/` unless they are given absolute or under another crate. The root `CLAUDE.md` states the current premise: "There is no repository picker: one repository, named on the command line" (`CLAUDE.md:169-170`).

Classification key: **PER-REPO** must exist once per tab. **WINDOW** exists once per window. **AMBIGUOUS** has no evidence either way; the reason is given. Fork's behaviour on these points is unobserved: `docs/research/ui-review/fork-evidence-inventory.md:104-112` and `:438` list repository tabs (open, close, reorder, menu, dot) as having "no Fork evidence at all", beyond the strip's existence and position.

---

## 1. How the repository and its worker are opened today; where its identity lives

### 1.1 `main.rs` (238 lines)

- `main()` (main.rs:46-71):
  - `worker::Discovery::start()` (:48) finds `git` once per process, on a `cairn-discovery` thread.
  - `Closing::default()` (:49).
  - `WindowConfig::new(move || app(git.clone(), closing.clone()))` (:50-53), with `.with_title(cairn_ui::CAPTION_TITLE)` (:54), `frame::OPENING_SIZE`/`MIN_SIZE` (:55-56), `.with_app_id("cairn")` (:58) and decorations only on `Frame::Native` (:61).
  - `.with_on_close(move |_, _| closing.requested())` (:63) is the single close hook for the single window.
  - `launch(...)` with one window (:64-70).
- `app(git, closing)` (main.rs:73-238):
  - `use_init_theme` (:75) is WINDOW.
  - It creates every handle of `View` with `use_state` / `use_scroll_controller` / `*View::used()` in the root scope (:78-126), then assembles `let view = View { … 33 fields … }` (:127-161).
  - `let opened = use_hook(|| repository_path::chosen(std::env::args_os(), repository_path::working_directory()).display().to_string())` (:163-167) is identity #1: the path as typed.
  - `let repository = use_hook({ … move || match worker::open(&path, &git) { … } })` (:169-230):
    - `Ok((handle, mut updates, reply))` (:174):
      - `closing.opened(handle.clone())` (:175).
      - `closing.when_requested(move || writes.write().closing())` (:177-180). This is the only `View` field the hook captures besides `progress` and `view` itself.
      - Initial requests: `Request::ListRemotes`, `Request::ConfiguredContext`, `Request::Refresh` (:182-187).
      - `submitting = handle.clone()` (:188).
      - `replying = Rc::downgrade(&reply)` (:191), commented "Weak: the task must not keep the answering end alive past the window, or the acceptor could never see it go".
      - `spawn(async move { … while let Some(update) = updates.next().await { session::apply(update, view, &session::Worker { submit: &submit, refuse: &refuse, closing: closing.is_requested() }) } … })` (:192-211).
      - On stream end: `if closing.worker_gone() { platform.close_current_window() } else { progress.write().stream_ended("the repository worker has stopped") }` (:212-221).
      - Returns `Some((handle, reply))` (:223). The strong `Replier` therefore lives in this hook for the root scope's life.
    - `Err(error)` → `progress.write().failed(…)`, `None` (:225-228).
  - `let (submit, reply) = match repository { Some((handle, reply)) => (Some(handle.into_submitter()), Some(reply)), None => (None, None) }` (:232-235).
  - `window::window(&opened, view, submit, reply)` (:237).
- `const PAGE_ROWS: usize = 64` (main.rs:39). `DIFF_FONT` (:44) is a WINDOW/process font.

### 1.2 `repository_path.rs` (89 lines)

- `pub fn chosen(arguments, working_directory) -> PathBuf` (:7-16) takes `nth(1)`. "Later arguments are ignored" (:6; test `later_arguments_are_ignored`, :65-70).
- `pub fn working_directory()` (:19-21). This is the only reader of argv.

### 1.3 The worker API the render side uses (`worker.rs` re-exports, worker.rs:47; `worker/pool.rs` 2711 lines)

- `pub fn open(path, git: &Discovery) -> Result<(RepositoryHandle, Updates, Replier), OpenError>` (pool.rs:87-92). Doc: "Drive [`Updates`] from exactly one task" (:86).
- `open_paced` (:95-295) starts seven threads per repository (pool.rs:4-27): `cairn-repository`, `-history`, `-diff`, `-refresh`, `-network`, `-local` and `-askpass`.
- The worker sends `Update::Started { git, zone }` (:206-212). `zone` is `super::zone::read()`, re-read on every open.
- The worker then sends `Update::Opened { name, path }` (:232-238).
- `Backend::open(discovery.startup(), &git)` (:245) opens this repository's askpass channel.
- `pub type Replier = Rc<dyn Fn(Reply)>` (pool.rs:65): "A plain callback, never a struct field: it holds the channel a secret travels down, and application state must not keep one."
- `pub const CLOSE_PATIENCE: Duration = 5s` (pool.rs:76), with a compile-time assert that it is greater than `CLOSE_BOUND + STOP_DEADLINE` (:78-81).
- `RepositoryHandle` (pool.rs:747-763; `Clone`). It holds the senders, `LaneState`, `Epochs` and `FetchControl`.
  - `submit(&self, Request) -> Option<Epoch>` (:783) never blocks.
  - `into_submitter(self) -> Rc<dyn Fn(Request)>` (:935-940).
  - The repository thread is "Detached; ends when the last `RepositoryHandle` drops" (:178).
  - `RepositoryJob::Close => break` (pool.rs:1243). Routing: `Request::Close => Routed::Repository(RepositoryJob::Close)` (routing.rs:288).
- `Updates { inbox: Receiver<Envelope>, wake: Arc<Wake>, epochs: Epochs }` (pool.rs:1020-1024).
  - `pub async fn next(&mut self) -> Option<Update>` (:1032). It drops stale epochs, turns big superseded answers into `Update::Superseded`, and awaits `Woken(&self.wake)` when the channel is empty.
  - `impl Drop for Updates` (:1057) calls `epochs.stop()`.
- `#[cfg(test)] pub fn idle_handle()` (pool.rs:946) is used by the `closing.rs` tests.

### 1.4 `frame.rs` (207 lines): WINDOW

- `OPENING_SIZE` (:11), `MIN_SIZE` (:19), `ZOOM_STEPS` (:23), `struct Zoom(usize)` (:27-50).
- `pub struct FrameView { zoom: State<Zoom>, about: State<bool>, git: State<Option<String>> }` (:54-59), with `used()` (:63-69) and `#[cfg(test)] created()` (:73-79).
- `zoom_refusal(view, larger)` (:83-92) and `zoom(view, larger)` → `Platform::get().set_custom_scale_factor` (:96-103).
- `title(repository: Option<&str>) -> String` gives `"<name> — Cairn"` or `"Cairn"` (:107-112).
- `use_title(view)` is `use_side_effect` → `Platform::get().set_window_title(None, title(view.repository.read()))` (:115-120).
- `ask(FrameRequest)`: drag, maximize, minimize, close, window menu (:123-139).
- `close()` → `post_callback(… renderer.request_close_window(Some(window)))`, which goes through the close hook (:143-147).
- `open_about(view)` (:150-153) and `about(view) -> Option<Element>` (:156-170).

### 1.5 `window.rs` (5709 lines; production code is lines 1-592, tests from :593)

- `pub struct View` (:54-130), `#[derive(Clone, Copy)]`. All fields are `State<_>`, `ScrollController` or `Copy` sub-views. "Handles, not values: the window subscribes to what it reads" (:51-52). `impl Debug for View` (:132-140).
- `pub fn window(opened: &str, view: View, submit: Option<Rc<dyn Fn(Request)>>, answer: Option<Replier>) -> Element` (:143-290). "`submit` and `answer` are `None` when no repository could be opened" (:142). In order, it does the following:
  - It reads `progress`, `prompt`, `confirming` and `closing_line(&view.writes)` (:149-158), and clones `submit` six times (:159-164).
  - Held-keys reset on app focus loss: `use_side_effect` (:168-173). WINDOW.
  - `crate::refresh::focus::on_focus_gained(submit.clone())` (:175).
  - `crate::frame::use_title(view)` (:177).
  - `crate::operations::leftover_lock::use_lock_confirmation(view, submit.clone())` (:179).
  - A scroll of the history supersedes a sidebar find: `use_side_effect` (:182-194).
  - `list` is the reflog strip, the reading strip, then `notice(message, opened)` or `history(view, lanes, submit)` (:196-218). `opened` is used only in the placeholder notice (:210, :515-536).
  - Root `rect()` with `.on_global_key_down` → `held_keys.heard`, then `accelerators::resolve_key(&e, Scope::Window)` → `shortcuts::act(action, view, hearing)` (:228-234). `.on_global_key_up` (:235-238).
  - `CaptionRow::new(menu_bar::menus(view, submit.is_some())).on_choose(menu_bar::choose(…)).on_frame(frame::ask)` (:241-248).
  - `crate::toolbar::toolbar(view, submit.clone())` (:250).
  - Banners: "The two banners: the leftover lock, about the repository, and closing, about the window" (:251-254). `leftover_lock::banner(view, unlocking)` (:253) and `closing_on.map(banner)` (:254).
  - `beside(SidebarPane::new(view, submit), match view.sidebar.main { AllCommits => split(list, DetailPane::new(view, submit), view), LocalChanges => LocalChangesPane::new(view, submit) }, view)` (:255-262).
  - The single modal slot, `match crate::modal::drawn(view)` (:269-286), dispatches `Prompt` → `dialog(prompt, view.prompt, answer)`, `Confirmation` → `confirmation(...)`, `CommitError` → `commit_box::pane::git_error`, `Ending` → `ending_dialog::dialog`, `CreateBranch` → `create_branch::dialog`, `Popover` → `ActivityLayer::new(view, popping)` and `About` → `frame::about(view)`.
  - `ContextMenuViewer::new()` (:288) is WINDOW, the one host.
- `confirmation(asking, showing)` is keyed `DiffKey::U64(serial)` (:295-315).
- `beside(...)` is the sidebar splitter, reading `view.sidebar.width` and `view.sidebar.shown` (:319-341).
- `split(list, pane, view)` is the history/pane splitter, reading `pane_collapsed` and `pane_split` (:346-397).
- `dialog(prompt, showing, answer: Option<Replier>)` (:400-428) is the credential prompt; see §5.
- `history(view, lanes, submit)` (:430-513) builds `HistoryList`. Its `on_select`/`on_extend` check `shortcuts::keys_inert(view)`, and `on_reach_end` submits `Request::MoreHistory`.

### 1.6 `session.rs` (2054 lines; production code is lines 1-513, tests from :514)

- `pub struct Worker<'a> { submit: &'a dyn Fn(Request), refuse: &'a dyn Fn(PromptId), closing: bool }` (:19-23).
- `pub fn apply(update: Update, view: View, worker: &Worker)` (:35-457). It destructures `rows, progress, fetch, prompt, remotes, diff, refreshed, repository, writes, activity` (:36-48) and matches every `Update` arm; see §3.2.
- `lines_follow` (:465-471), `retire` (:476-480), `refresh_after_an_operation` (:485-489), `pub fn reopen_history(progress, lost, submit)` (:497-503) and `withdraw(prompt, worker)` (:508-512).

### 1.7 Where the single repository's identity lives

| Representation | Where | Set by | Read by |
|---|---|---|---|
| Path as typed (`String`) | `opened`, `use_hook` in main.rs:163-167 | argv/cwd | `worker::open` (main.rs:173); `window(opened, …)`, used only in `notice(message, path)` (window.rs:210, 515) |
| `View::repository: State<Option<String>>` (window.rs:102) | root `use_state` (main.rs:107) | `Update::Opened` (session.rs:96-97) | `frame::use_title` (frame.rs:117); toolbar `StatusBox::new` (toolbar.rs:33-36); sidebar header `.name(...)` (sidebar/pane.rs:77) and its `PartialEq` (:33) |
| `View::repository_path: State<Option<PathBuf>>` (window.rs:105) | main.rs:108 | `Update::Opened` (session.rs:98-99) | Copy Repository Path (sidebar/menu.rs:34-47); Copy Full Path (detail/file_copy.rs:19; local_changes/actions.rs:670) |
| `RepositoryHandle` | main.rs's hook result; the spawned task (`submitting`); `Closing`'s `Stage::Open(handle)` (closing.rs:38) | `worker::open` | as `submit: Option<Rc<dyn Fn(Request)>>` threaded to every component |
| `Updates` | moved into the one `spawn`ed task (main.rs:192-222) | `worker::open` | `session::apply` |
| `Replier` | strong in main.rs's hook result; weak in the task (main.rs:191) | `worker::open` | `window(…, answer)` → `dialog`; `refuse` in `session::Worker` |
| "A repository is open" | `submit.is_some()` | — | `menu_bar::menus(view, open)` (window.rs:243); `toolbar(view, submit)` → `open` (toolbar.rs:19); `fetch::refusal(view, open)` (operations/fetch.rs:24-26) |

No `View` field names "which repository". The `View` is the repository, implicitly.

---

## 2. Every field of `View` and each sub-state, classified

The `View` has 33 fields (window.rs:54-130). In the "Made" column, "root" means the handle is created in `app()`'s root scope.

### 2.1 Top-level fields

| # | Field (type) | Made | Meaning | Class | Why / flag |
|---|---|---|---|---|---|
| 1 | `rows: State<History>` | main.rs:78 | the one copy of the history ("this scope must not read it") | PER-REPO | |
| 2 | `progress: State<Progress>` (history/state.rs:17-31) | :79 | walk status, lanes, loaded, reopening, reading | PER-REPO | also written by main.rs's open failure and stream end |
| 3 | `selected: State<Option<RowId>>` | :80 | selected history row | PER-REPO | |
| 4 | `fetch: State<FetchStatus>` (operations/fetch.rs:61) | :81 | Idle, Starting, Running, Cancelling, Finished, … | PER-REPO | |
| 5 | `prompt: State<Option<PromptView>>` (fetch.rs:134-141: `id: PromptId, text, asking`) | :82 | credential prompt shown | PER-REPO in origin, drawn modal over the WINDOW | see §5; `PromptId` collides across repositories |
| 6 | `remotes: State<Vec<RemoteSummary>>` | :83 | remotes, default first | PER-REPO | |
| 7 | `endings: State<Endings>` (operations/ending_dialog.rs:45-48: `waiting: VecDeque<Shown>, next_serial`) | :84 | Git Error / "Couldn't <name>" dialogs waiting | AMBIGUOUS | they belong to a repository's operations but are modal over the window; a background tab's failure has nowhere to show; `serial` keys the dialog (`DiffKey::U64`, ending_dialog.rs:171) and restarts per `Endings` |
| 8 | `lock: State<LeftoverLock>` (leftover_lock.rs:33-41: `changed, asked, ready`) | :85 | leftover `index.lock` | PER-REPO | window.rs:251 calls its banner "about the repository" |
| 9 | `diff: State<DiffState>` (diff/state.rs:107-129: `changes, file, opening, lane, filter: FileFilter, file_index, working, changes_stale`) | :86 | diff selection and answers | PER-REPO | it also holds the Changes tab's `FileFilter` (detail/file_filter.rs:22-31) |
| 10 | `history_scroll: ScrollController` | :87 | history list scroll | PER-REPO | |
| 11 | `history_cursor: State<usize>` | :88 | list cursor | PER-REPO | |
| 12 | `detail_tab: State<DetailTab>` | :90 | Commit or Changes tab, "kept for the session" | AMBIGUOUS | a session preference vs per-tab view state; Fork unobserved |
| 13 | `pane_collapsed: State<bool>` | :91 | Hide Commit Details (the View menu ticks it, menu_bar.rs:56-57; Ctrl+Shift+D) | AMBIGUOUS | a menu toggle shown ticked suggests window-wide; the layout is per tab |
| 14 | `pane_split: State<PaneSplit>` (detail/split.rs:10-15: `pane, room`) | :92 | history/pane proportion | AMBIGUOUS | splitter position; Fork per window or per tab is unknown |
| 15 | `diff_settings: State<DiffSettings>` (cairn-ui diff/settings.rs:14-26: `lines, entire_file, ignore_whitespace, side_by_side, chosen`) | :94 | "settings every diff view shares, kept for the session" | AMBIGUOUS | context is seeded from each repository's `diff.context` (`Update::ConfiguredContext` → `diff::actions::configured`, diff/actions.rs:366-370; `chosen` gates it); `change_settings` asks diffs again through one submitter (diff/actions.rs:362-363), so a window-wide value would need every repository re-asked, or each re-asked when shown |
| 16 | `diff_scroll: ScrollController` | :95 | Changes-tab diff scroll | PER-REPO | tied to the diff shown |
| 17 | `change_cursor: State<Option<ChangeCursor>>` | :96 | previous/next change position | PER-REPO | |
| 18 | `filter_text: State<String>` | :98 | Changes-tab filter text, "kept for the session" | PER-REPO | bound to `DiffState.filter`, which is per repository |
| 19 | `changes_list_width: State<f32>` | :99 | Changes-tab list share | AMBIGUOUS | a splitter |
| 20 | `pair: State<Option<Pair>>` (history/selection.rs:36-41) | :101 | comparison pair | PER-REPO | |
| 21 | `held_keys: State<HeldKeys>` | :102 | keys held, for pointer modifiers | WINDOW | keyboard state; reset on app focus loss (window.rs:168-173) |
| 22 | `refreshed: State<RefreshState>` (refresh/state.rs:44-48: `refs, ahead_behind, status`) | :104 | last refresh's answers | PER-REPO | |
| 23 | `repository: State<Option<String>>` | :107 | name | PER-REPO | identity |
| 24 | `repository_path: State<Option<PathBuf>>` | :108 | absolute root | PER-REPO | identity |
| 25 | `sidebar: SidebarView` | :110 | see §2.2 | mixed | |
| 26 | `local: LocalChangesView` | :112 | see §2.3 | mixed | |
| 27 | `writes: State<LocalWrites>` (local_writes.rs:48-56: `queued, running, closing: bool`) | :114 | local writes | PER-REPO, but its `closing` flag is a WINDOW fact | `closing_line` is drawn as "about the window" (window.rs:251-254); the close hook sets it via `Closing::when_requested` (main.rs:177-180) |
| 28 | `confirming: State<Option<Confirming>>` (operations/confirming.rs:25-35) | :116 | destructive confirmation | PER-REPO in origin, modal over the WINDOW | its `then` closure captures that repository's submitter and `writes`; `serial` comes from the process-wide `NEXT_SERIAL` (confirming.rs:20) |
| 29 | `show_lost: State<bool>` | :118 | Show Lost Commits, "kept for the session" | AMBIGUOUS | a ticked View-menu toggle (menu_bar.rs:63-65), but toggling reopens that repository's walk (history/lost_commits.rs:12-21); window-wide would mean reopening every repository's history |
| 30 | `zone: State<LocalZone>` | :120 | time zone for dates | WINDOW | the system's zone, but written by every repository's `Update::Started` (session.rs:88-95; pool.rs:206-212) |
| 31 | `branch: CreateBranchView` | :122 | see §2.4 | PER-REPO, modal | |
| 32 | `activity: ActivityView` | :124 | see §2.5 | AMBIGUOUS | |
| 33 | `frame: FrameView` | :126 | zoom, About, git (frame.rs:54-59) | WINDOW (`git` is process-wide) | `frame.git` is written by each repository's `Update::Started` |

Tally: 23 PER-REPO (1-4, 6, 8-11, 16-18, 20, 22-24, the per-repository parts of 25-27, and 31; 5 and 28 originate per repository). 4 WINDOW (21, 30, 33, and the modal role of `activity.open`). About 10 AMBIGUOUS (7, 12-15, 19, 29, 32, `sidebar.width/shown`, `local.list_width/lists_split`).

### 2.2 `SidebarView` (sidebar/state.rs:31-38; `used()` :43-52, `created()` :56-65)

| Field | Class | Note |
|---|---|---|
| `state: State<SidebarState>` (:68-82: `disclosure, shown, text, asked, head, chosen, notice`) | PER-REPO | rows of that repository's refs snapshot |
| `filter_text: State<String>` | PER-REPO | |
| `main: State<MainView>` (AllCommits or LocalChanges) | PER-REPO (likely) | it is what the main region shows; `View` menu items act on it |
| `width: State<f32>` | AMBIGUOUS | splitter |
| `shown: State<bool>` | AMBIGUOUS | toolbar `ToggleSidebar` (toolbar.rs:63-66); the toolbar is window chrome in Fork's layout |
| `finding: State<Option<Find>>` (history/ref_find.rs:35-47) | PER-REPO | |

### 2.3 `LocalChangesView` (local_changes/state.rs:46-72; `used()` :77-91, `created()` :95-109)

| Field | Class | Note |
|---|---|---|
| `state: State<LocalChangesState>` | PER-REPO | |
| `filter_text` | PER-REPO | |
| `list_width` | AMBIGUOUS | splitter |
| `lists_split` | AMBIGUOUS | "the session's, never kept beyond it" |
| `focus_unstaged`, `focus_serial` | PER-REPO (transient) | a focus request for its pane |
| `scroll: ScrollController`, `cursor` | PER-REPO | |
| `acting: State<Acting>` (local_changes/actions.rs:87-89) | PER-REPO | discard count in flight |
| `lines: State<LineDrag>` | PER-REPO | |
| `commit: CommitBoxView` (commit_box/state.rs:31-36: `subject, description, state: State<CommitBox>, output: State<StreamedOutput>`) | PER-REPO | the draft is "the window's", "kept for the window's life" (state.rs:7-9); per repository by meaning; holds `GitError` with an optional unspent `Confirmed`, guard-pinned (cairn-guards `the_window_holds_a_token_only_for_the_git_errors_skip`) |

### 2.4 `CreateBranchView` (operations/create_branch.rs:37-42; `used()` :47-52, `created()` :56-61)

- `state: State<CreateBranchState>` (:66-74: `open, checkout, asked, next_serial`) is PER-REPO. The exception is `checkout` ("sticky for the session", :68-69), which is AMBIGUOUS: sticky per window or per repository.
- `name: State<String>` is PER-REPO.

### 2.5 `ActivityView` (operations/activity.rs:284-312)

- `log: State<ActivityLog>` (:316-329: `entries, anchor, bytes, fetches, fetching, selected`) is AMBIGUOUS. It is "the session's operations".
  - Entries carry no repository name.
  - Keyed by `ActivityKey::{Write(OperationId), Fetch(u64)}` (:75-79). `OperationId` is unique per process; the fetch counter is per log.
  - `anchor` is the status box's layout position, a WINDOW fact.
  - Fork's Activity Manager scope (per window or per repository) is unobserved.
- `open: State<bool>` is WINDOW-like (modal).

### 2.6 `FrameView`

WINDOW; see §1.4.

### 2.7 Per-component local state (not in `View`; lost or reset if a tab's subtree unmounts)

- `detail/pane.rs:154`: `use_hook(RowFinder)` in `CommitBody`.
- `detail/changes_tab.rs:79`: `split_width` `use_state`.
- `local_changes/pane.rs:124`: `split_width` `use_state`.
- `local_changes/pane.rs:153-171`: `use_drop`. On unmount it sends `Request::StopCounting` and clears a counting `confirming`. A tab switch that unmounts the pane would cancel a pending discard count.
- `commit_box/pane.rs:104-108`: `use_hook` submits `Request::CommitReads` once per mount.

### 2.8 Derived, not stored

The menu bar (`menu_bar::menus`), toolbar (`toolbar::toolbar`) and modal (`modal::drawn`) hold no state of their own. They are rebuilt every render from the one `View`.

---

## 3. How updates flow from the worker to the window; closing

### 3.1 The stream

- One `Updates` per `worker::open`. Inside it: `Receiver<Envelope>`, where `Envelope { epoch: Option<Epoch>, update: Update }` (pool.rs:1078-1082, approximately), plus one `Arc<Wake>` and the repository's `Epochs`.
- `Wake` (worker/wake.rs:8-53) is "A one-slot signal". It latches `signalled` and stores one `Waker`, so one awaiting task per `Wake`. Every `Outbox` of that repository's seven threads shares it (pool.rs:122-172).
- `Updates::next` (pool.rs:1032) drops answers whose epoch is no longer current, using that repository's own epochs.
- `Update` has no repository field. Epochs, `PromptId`, the fetch counter and the dialog serials are all repository-relative.
- So N repositories today means N `Updates`, N `Wake`s and N epoch sets, each to be driven by exactly one task (pool.rs:86).
- A merged stream would need a repository tag, applied either on the render side (one task per stream, tagging as it calls `apply`) or by the worker.
- `Updates::next`'s epoch filtering must remain per repository. Merging raw envelopes across repositories would compare the wrong epochs.

### 3.2 `session::apply`: which `View` fields each `Update` writes

| Update | Writes |
|---|---|
| `Rows` | `rows`, `progress`; `ref_find::pages_arrived` (sidebar.finding, sidebar.state, selected …) |
| `Reading` | `progress` |
| `Failed` / `WorkerLost` | `progress`; `ref_find::failed` |
| `Remotes` | `remotes` |
| `Started { git, zone }` | `frame.git`, `zone` (**WINDOW fields, from a per-repository update**, session.rs:88-95) |
| `Opened { name, path }` | `repository`, `repository_path` |
| `IndexLock`, `LockConsequence` | `lock`, `confirming`, `endings` |
| `CommitReads`, `Amending` | `local.commit`, `local.state` |
| `BranchName`, `CheckoutConsequence` | `branch` |
| `DiscardConsequence` | `local.acting`, `confirming` |
| `ConfiguredContext` | `diff_settings` (**AMBIGUOUS field, per-repository config**) and asks again |
| `FetchStarted` / `Progress` / `Finished` / `Cancelled` / `Failed` | `activity.log`, `fetch`, `prompt` (withdraw), `endings`; submits `Refresh` |
| `OperationRan` | `activity.log` |
| `WriteStarted` / `WriteOutput` / `WriteEnded` | `writes`, `activity.log`, `local.commit`, `branch`, `endings`, `prompt`; submits `Refresh`/`RefreshStatus` unless `worker.closing` |
| `Refs` | `sidebar.state`, `refreshed`, selection re-ask, commit box re-read, `progress` (reopen), find |
| `AheadBehind` | `refreshed` |
| `Status`, `FilteredLocalChanges` | `local.state`, `refreshed`, `diff` (`show_primary`) |
| `RefreshFailed` | `progress`, `refreshed` |
| `FilteredRefs` | `sidebar.state` |
| `Prompt { id, text, asking }` | `prompt` if `fetch.is_in_flight() \|\| writes.is_running()`, else `worker.refuse(id)` (session.rs:370-376) |
| `Changes` / `FilteredFiles` / `FileDiff` / `Expanded` / `DiffFailed` | `diff`, `local.lines` |
| `Superseded`, `FetchRefused`, `CommandLog` | retire or nothing |

`Worker.closing` comes from `Closing::is_requested()` (main.rs:208), a window-level flag. It suppresses follow-up requests in roughly ten places in `apply`.

### 3.3 `Replier` and refusal

- `refuse` in the task upgrades the weak `Replier` (main.rs:196-200).
- `withdraw` refuses a shown prompt when its fetch or write ends (session.rs:508-512).
- The acceptor blocks on `answers.recv()` while a prompt waits (worker/askpass.rs:155-176). It is released by an answer, a refusal, or the `Replier` being dropped (`Err` → refuse and return).
- `CLAUDE.md:1075-1077` records that "the stream's end still depends on the window refusing a prompt left open".

### 3.4 Closing: `closing.rs` (205 lines)

- `pub struct Closing(Rc<RefCell<Stage>>, Rc<RefCell<Option<Tell>>>)` (:28). `enum Stage { Unopened, Open(RepositoryHandle), Closing { since }, Gone }` (:34-45).
- `opened(handle)` (:49-54) applies only while the stage is `Unopened`. It holds exactly one handle.
- `when_requested(tell)` (:58-60) holds exactly one `Tell`.
- `requested() -> CloseDecision` (:63-82). It is the window's close hook, not a repository's.
  - `Unopened | Gone` → `Close`.
  - `Open` → `handle.submit(Request::Close)`, then `Stage::Closing`, then call `tell`, then `KeepOpen`.
  - `Closing` past `CLOSE_PATIENCE` → `Close`; otherwise `KeepOpen`.
- `is_requested()` (:86-88) and `worker_gone() -> bool` (:92-97). The latter sets `Gone` and returns whether a close was asked.
- The window closes on the stream's end via `platform.close_current_window()` (main.rs:212-215), "Past the hook, which asked already".
- The invariant (`CLAUDE.md:1063-1080`) puts main.rs's wiring of this ("`Closing::opened` given the handle, the hook installed, `Closing::is_requested` passed to `session::apply`, the window closed past the hook on the stream's end") under review judgement: "which no test drives".

What "close a tab" would need, compared with "close the window", as the code stands:

- **Closing a tab.** A per-repository `Request::Close` without touching the window's `CloseDecision`. A per-repository "closing" flag in place of `Worker.closing`. That repository's stream ending must remove the tab, not call `close_current_window()`. Its `Replier` (strong, today in the root `use_hook`) must be dropped or its prompt refused so the acceptor can leave. Its `Updates` must be dropped (`Drop` stops epochs). Every `RepositoryHandle` clone must go (the repository thread is detached and ends when the last handle drops, pool.rs:178), including the one in `Closing`. A running local write is waited for (the worker ends nothing until it has), so a per-tab "closing after <write>" line is needed. Today that line is drawn as window-level (window.rs:254) from `writes.closing`.
- **Closing the window.** `Closing` today knows one handle and one `Tell`. With N repositories it must submit `Close` to each, close the window only after all N streams have ended, and keep the `CLOSE_PATIENCE` escape. `closing_line` must name the write(s) being waited on across repositories.
- The `when_requested` closure writes one `writes` (main.rs:177-180).

---

## 4. `shortcuts.rs`, `modal.rs`, `menu_bar*`, `toolbar.rs`: implicit "the repository"

### 4.1 `shortcuts.rs` (100 lines)

- `pub fn of_header(HeaderAction) -> Action` (:19-21).
- `pub fn keys_inert(view) -> bool { modal::modal(view).is_some() }` (:27-29).
- `pub fn act(action: Action, view: View, submit: Option<&dyn Fn(Request)>)` (:37-93) returns if `keys_inert`, then has one arm per `Action`. Each repository-touching arm acts on the single `view` and `submit`:
  - `ShowCommitTab`/`ShowChangesTab` → `detail_tab`, `pane_collapsed`.
  - `PreviousChange`/`NextChange` → `diff::actions::step`.
  - `ToggleIgnoreWhitespace`/`MoreLines`/`FewerLines`/`EntireFile` → `diff::actions::change_settings(view, submit, …)`. These re-ask diffs.
  - `ToggleSideBySide`.
  - `NewBranch` → `create_branch::open_at_head(view)`.
  - `Refresh` → `submit(Request::Refresh)`.
  - `ShowLostCommits` → `lost_commits::toggle`.
  - `ShowLocalChanges`, `ShowAllCommits`, `ShowHead`, `ToggleDetails`, `Fetch` → `fetch::start`.
  - Window-only: `ZoomIn`/`ZoomOut` → `frame::zoom(view, …)`.
  - Press-only, scope-only and copy actions are no-ops here.
- Callers:
  - Window scope: `on_global_key_down` (window.rs:228-234).
  - Detail scope: detail/pane.rs:108-110 and local_changes/pane.rs:242-244.
  - Diff bar: detail/changes_tab.rs:278 and local_changes/pane.rs:375.
  - Menu bar: menu_bar.rs:96.
  - Toolbar: toolbar.rs:61-62.
  - `Scope::LocalChanges` keys are resolved by local_changes/pane.rs:395.
  - Scopes: `cairn-ui/src/accelerators.rs:154-178`. `Action` is at `:47`.

### 4.2 `modal.rs` (77 lines)

- `pub enum Modal { Prompt, Confirmation, CommitError, Ending, CreateBranch, Popover, About }` (:18-33).
- `pub fn modal(view)` (:37-55) peeks; `pub fn drawn(view)` (:59-77) reads. The order is `prompt` → `confirming` → `local.commit.state.error()` → `endings` → `branch.state.is_up()` → `activity.open` → `frame.about`.
- Only `About` is WINDOW. The rest are per-repository states, ranked into one window-wide modal slot. `keys_inert` reads this, so any one repository's dialog must still silence the window's chords.

### 4.3 `menu_bar.rs` (107 lines) and `menu_bar/tests.rs` (470 lines)

- `pub enum BarChoice { Act(Action), Exit, About }` (:16-24). `const NO_REPOSITORY` (:27).
- `pub fn menus(view, open: bool) -> Vec<BarMenu<BarChoice>>` (:30-89).
  - File: Exit. WINDOW.
  - View: Show Uncommitted Changes, Show All Commits, Hide Commit Details (ticked by `pane_collapsed`), Show HEAD (greyed by `head::head_commit(view)`), Show Lost Commits (ticked by `show_lost`).
  - Repository: Refresh, `Fetch '<default_remote(view)>'` (greyed by `fetch::refusal(view, open)`), New Branch….
  - Window: Zoom In and Zoom Out, greyed by `frame::zoom_refusal`. WINDOW.
  - Help: About Cairn. WINDOW.
  - It subscribes to `view.refreshed` (:35-39).
- `pub fn choose(choice, view, submit)` (:92-103): `Act` → `shortcuts::act`, `Exit` → `frame::close()`, `About` → `frame::open_about` unless `keys_inert`.

### 4.4 `toolbar.rs` (121 lines)

- `pub fn toolbar(view, submit) -> Toolbar` (:18-54):
  - `open = submit.is_some()`.
  - `tracking(&view.refreshed)`.
  - `StatusBox::new(view.repository.read().clone(), refs.head)` names the repository (:33-36). `.running(running(view))`.
  - Fetch and branch refusals.
  - `on_press` → `pressed`.
  - `on_status_sized` writes `view.activity.log.anchor` (:47-53).
- `pub fn pressed(press, view, submit)` (:58-68): Fetch/NewBranch → `shortcuts::act`; `ToggleSidebar` → `view.sidebar.shown.toggle()`; `Activity` → `view.activity.open()`.
- `fn tracking` (:72-85).
- `pub fn running(view) -> Vec<Running>` (:90-121) uses `asked.id.number()` as the serial for the write and `u64::MAX - fetch serial` for the fetch.

---

## 5. Credentials and askpass on the render side

- **Arrival.** `Update::Prompt { id: PromptId, text, asking: Option<String> }`. The render side keeps it only while that repository's fetch is in flight or a write is running; otherwise it refuses at once (session.rs:370-376). Stored in `View::prompt: State<Option<PromptView>>`.
- **Modal over the window.** `Modal::Prompt` is first in `modal::drawn` (modal.rs:60-61), so `keys_inert` is true and every window chord stops. It is drawn in the window's single modal slot (window.rs:270). A credential prompt "sets an open confirmation aside" (window.rs:264-267).
- **The dialog.** `fn dialog(prompt, showing, answer: Option<Replier>)` (window.rs:400-428) builds `CredentialPrompt::new(asking, prompt.text)`. The title is "<asking> is asking for a credential" (cairn-ui dialogs/credential_prompt.rs:203-204), where `asking` is the operation's token name: the remote, or the write ("Commit"), or "git".
  - Submit → `Secret::from_string(typed)`, then `answer(Reply::Provide { prompt: id, secret })`.
  - Cancel → `Reply::Refuse`.
  - Either way, `showing.set(None)`.
- **Which repository asked?** Nothing on the render side records it.
  - `PromptView` has `id, text, asking` only.
  - `PromptId(u64)` is issued by each repository's acceptor from `let mut issued = 0u64; … issued += 1` (worker/askpass.rs:133-146). Every repository's first prompt is `PromptId(1)`.
  - The `Replier` handed to `window()` is the single repository's. With N repositories, a reply must reach that repository's `Replier`.
  - The dialog's title names the remote, not the repository; two repositories both fetching `origin` would read the same.
  - Gating ("fetch or write in flight") is per repository.
- **Guarded invariants.** The `Replier` is never a struct field (pool.rs:62-65). `no_credential_value_is_logged_printed_serialised_or_stored` (cairn-guards/tests/invariants.rs:8436). `Reply` derives nothing (askpass.rs:36-42). Any per-repository `Replier` collection must stay closure-shaped, not a stored struct field, to satisfy the doc rule.

---

## 6. The title and caption row

- **Caption row** (cairn-ui `src/caption_row.rs`, 201 lines). `CaptionRow::new(menus)`: menus at the left, `CAPTION_TITLE` = "Cairn" centred (:18-20, :164-171), the window controls at the right. `Frame::Drawn` applies on Linux and `Frame::Native` on macOS (menus alone, :137-138).
  - The doc comment says "The title at the caption row's centre: the application's name alone, as Fork's reads 'Fork'. The OS-level title names the repository" (:18-19).
  - `main.rs:54` also sets the window's initial title to `CAPTION_TITLE`.
- **OS title.** `frame::title(view.repository)` gives `<repo> — Cairn` (frame.rs:107-120; docs/systems/window-frame.md:249-253).
- **What names the repository inside the window today:** the toolbar's status box (repository name bold over the branch; toolbar.rs:33-36; cairn-ui toolbar/status_box.rs:239-240) and the sidebar's header with its gear (sidebar/pane.rs:77; menu sidebar/menu.rs:34-47).
- **Where a tab strip would sit.** Fork's own window survey puts the strip directly under the toolbar, above the notification banner. Bands: caption 0-25, toolbar 26-66, "Repository tab strip | 67-93 | ~26 | One tab per open repository, equal widths across the window … a small grey dot at its left … A '+' at the far right", banner 94-117 (`docs/research/ui-review/fork-window-survey-2026-10-16.md:36-41`). Colours: `fork-window-survey-2-2026-10-16.md:39,378`.
  - In `window()` that position falls between `.child(crate::toolbar::toolbar(view, submit.clone()))` (window.rs:250) and the two banners (window.rs:253-254).
  - In Fork's layout the caption row and toolbar sit above the tabs. The toolbar's status box still names the active repository.
  - Freya has no Tabs component, only parts (`docs/research/ui-review/freya-chrome-apis.md:267, 364`).

---

## 7. Process-wide singletons and their safety with several repositories

| Singleton | Where | Scope | Safe with N repositories? |
|---|---|---|---|
| `Discovery(Arc<Found{startup, answer: OnceLock<Result<GitBinary,String>>}>)` | worker/discovery.rs:20-26, 78-87; `start()` main.rs:48 | one per process, cloned into each `open` | Yes, by design ("every repository the application opens takes a copy", discovery.rs:3-8). Each repository runs `GitBinary::with_environment` with its own askpass (startup.rs:117-118) |
| `Startup` (parent env lookup, helper path) | worker/startup.rs:17-21 | process; "shared by every repository" (:13-16) | Yes |
| Askpass `Channel` / `CAIRN_ASKPASS_SOCKET` | `Backend::open` per repository (startup.rs:86-125); `Channel::open` makes `XDG_RUNTIME_DIR/cairn-<pid>-<random8>/askpass` (cairn-askpass/src/channel.rs:129-183) | per repository: own socket, own `cairn-askpass` acceptor thread, own token table | Yes; the random suffix avoids collisions. Each repository's git env names its own socket |
| `PromptId` counter | acceptor-local `issued` (worker/askpass.rs:133-146) | per repository | No: ids collide at the window across repositories |
| `OperationId::next()` (`static NEXT: AtomicU64`) | worker/local/lane.rs:74-90; called by `local_writes::ask` (local_writes.rs:142-147) | process | Yes: unique across repositories, "Never reused within the application" |
| `Confirming` `static NEXT_SERIAL: AtomicU64` | operations/confirming.rs:20 | process | Yes |
| `Endings.next_serial`, `CreateBranchState.next_serial`, commit box `next_serial` | ending_dialog.rs:47,62-63; create_branch.rs:72,99-100; commit_box/state.rs:204,343 | per state | No, if dialogs from several repositories share the one modal slot: used as `DiffKey::U64(serial)` (ending_dialog.rs:171; commit_box/pane.rs:527) |
| `ActivityKey::Fetch(n)` / `ActivityLog.fetches` | operations/activity.rs:75-79, 316-329 | per log | No, if the log is merged across repositories |
| `History` `NEXT_SERIAL` | cairn-model/src/history.rs:172 | process | Yes (serials stay unique) |
| `LazyLock`/`static` empty sentinels (`NO_CHANGES`, `NO_ROWS`, `NO_DIFF`, `NOTHING_OPEN`) | local_changes/state.rs:157-161; diff/state.rs:557-598 | process, immutable | Yes |
| `clipboard::COPIED` `thread_local!` | clipboard.rs:20-22 | `#[cfg(test)]` only | n/a |
| cairn-ui `LocalZone` `NEXT_READ` | cairn-ui/src/local_time.rs:29 | process | Yes |
| Toolkit window singletons: `Platform::get()` title, scale factor, `is_app_focused`, `close_current_window`, `request_close_window` | frame.rs:102,118,124,144; refresh/focus.rs:19; window.rs:170; main.rs:181,215 | window | They stay window-wide. `on_focus_gained` refreshes the one submitter |
| Epochs, `Wake`, `LaneState`, `FetchControl` | pool.rs:113-119 | per repository | Yes |
| Worker threads | pool.rs:4-27 | 7 per repository | Yes, at a cost of 7N threads |

Freya hook semantics affect every one of these once there is more than one `View`. `use_side_effect` is `use_hook(|| Effect::create(callback))` (freya-core `lifecycle/effect.rs:98-100`, vendored at `/root/.cargo/git/checkouts/freya-23bd2b0bd50361d3/caa46f8/`), and `use_state` is `use_hook(|| State::create(init()))` (`lifecycle/state.rs:703-705`). A hook's closure, and whatever it captured (`view`, `submit`), is fixed at the first render of its scope. `State::create` attaches to the current scope; `create_in_scope` exists (`state.rs:544-565`).

Effects that capture `view` and/or `submit` this way:

- `frame::use_title` (frame.rs:115-120)
- `refresh::focus::on_focus_gained` (refresh/focus.rs:16-26)
- `leftover_lock::use_lock_confirmation` (leftover_lock.rs:111-135)
- the find-supersede effect (window.rs:182-194)
- the held-keys reset (window.rs:168-173; WINDOW, harmless)
- `SidebarPane` filter effect (sidebar/pane.rs:54)
- `LocalChangesPane` effects (local_changes/pane.rs:127, 143, 174)
- `use_amend_confirmation` (commit_box/pane.rs:278)
- `CreateBranchPane` name effect (create_branch.rs:475)
- `CommitBody` re-ask (detail/pane.rs:157)
- `ChangesTab` effects (detail/changes_tab.rs:83, 97, 109)

Eight components implement `PartialEq` as "By the handles it reads: the submitter is the same repository's however often it is built" and compare only `submit.is_some()`. Unless their `View` handles differ, they will not re-render for another repository's submitter:

- `DetailPane` (detail/pane.rs:52-68)
- `CommitBody` (:136-146)
- `ChangesTab` (detail/changes_tab.rs:58-70)
- `CommitBoxPane` (commit_box/pane.rs:51-57)
- `ActivityLayer` (operations/activity.rs:701-707)
- `CreateBranchPane` (create_branch.rs:460)
- `LocalChangesPane` (local_changes/pane.rs:99-107)
- `SidebarPane` (sidebar/pane.rs:28-34)

---

## 8. Tests: how the window state is built; refactor surface

Literal `View { … }` constructions (all 33 fields each), 6 in total:

1. `main.rs:127-161` (production)
2. `window.rs:757-801`, `fn test_view(initial, progress, fetch, prompt) -> View`, provided via `runner.provide_root_context` in `launch_at` (:714-753) and consumed by `use_consume::<View>()` (:727)
3. `local_changes/tests.rs:79-112`, in `launch_with_theme` (:57-118) behind `pub(crate) fn launch()` (:44) and `launch_counted()` (:51)
4. `sidebar/tests.rs:154-188`, in `fn launch(loaded, complete)` (:131)
5. `session.rs:571-607`, in `fn launch(fetch)` (:562-614), which draws no window (`rect`) and calls `apply` through `run_in`
6. `window_check.rs:413-447`, in `fn launch(path) -> Harness` (:394-460), with a real `worker::open` (`#[ignore]`d measurements)

Sub-view test constructors, each called from builders 2-6: `SidebarView::created` (sidebar/state.rs:56), `LocalChangesView::created` (local_changes/state.rs:95), which calls `CommitBoxView::created` (commit_box/state.rs:51), `ActivityView::created` (activity.rs:296), `CreateBranchView::created` (create_branch.rs:56), `FrameView::created` (frame.rs:73).

Call sites:

- `window(` is called at 6 sites: main.rs:237; window.rs:738, 5547; local_changes/tests.rs:71; sidebar/tests.rs:141; window_check.rs:406.
- `session::apply(` is called at 9 sites: main.rs 1, window.rs 4, local_changes/tests.rs 2, sidebar/tests.rs 1, window_check.rs 1. session.rs's own tests call it through `applying`/`applying_while` (62 uses).
- window.rs test helpers: `launch(` ×66, `launch_with(` ×8, `launch_sized(` ×7, `launch_at(` ×3, `test_view(` ×3.
- `press_chord` (window.rs tests) is shared, with 19 uses across 7 other test files.
- `local_changes::tests::{launch, launch_counted, apply, …}` is imported by commit_box/tests.rs, local_changes/actions/tests/{files,gesture}.rs, menu_bar/tests.rs, operations/tests.rs and operations/create_branch/tests.rs.
- Per-file uses (grep, which includes definitions):

  | File | `launch` | `opened` | `apply` |
  |---|---|---|---|
  | commit_box/tests | 1 | 29 | 47 |
  | files | 2 | 22 | 14 |
  | gesture | 4 | 9 | 9 |
  | menu_bar | 6 | — | 1 |
  | operations | 12 | — | 27 |
  | create_branch | 15 | — | 33 |
  | local_changes/tests | 18 | — | 26 |
  | sidebar | 17 | — | 7 |
  | session | 25 | — | 62 |

Tests that build a `View`: window.rs 72, session.rs 20, sidebar/tests 14, local_changes/tests 13, local_changes/actions/tests 1, files 21, gesture 11, commit_box/tests 22, menu_bar/tests 6, operations/tests 12, create_branch/tests 13, window_check 4. That is about 209 of the 312 `#[test]`s on the render side.

Direct `view.<field>` references in test code (grep): window.rs 119, session.rs 72, window_check.rs 49, commit_box/tests 44, files 30, sidebar/tests 25, menu_bar/tests 16, operations/tests 15, local_changes/tests 14, gesture 12, create_branch/tests 9.

Production coupling:

- 170 functions take `view: View`: local_changes/actions 22, commit_box/pane 21, operations/create_branch 18, window 17, diff/actions 16, history/ref_find 11, history/selection 9, detail/pane 8, local_changes/pane 6, frame 5, session 4, leftover_lock 4, changes_tab 4, toolbar 3, fetch 3, activity 3, and 2 or fewer in the rest.
- 30 production files import `crate::window::View`.
- Non-test `view.<field>` reads by field: `local` 80, `sidebar` 35, `diff` 27, `refreshed` 15, `progress` 12, `confirming` 11, `writes` 10, `endings` 8, `rows` 7, `diff_settings` 7, `lock` 6, `pair`/`history_scroll`/`held_keys`/`activity` 5 each, `zone`/`show_lost`/`selected`/`repository`/`prompt`/`fetch` 4 each, `repository_path`/`history_cursor`/`frame` 3 each, and 2 or fewer for the rest.
- 13 `let View { …, .. } = view` destructurings: detail/pane.rs:75; history/selection.rs:89, 105; shortcuts.rs:39; window.rs:347; session.rs:36; diff/actions.rs:47, 61, 121, 278, 316, 329, 398.
- `View` must stay `Copy` (window.rs:53): every sub-view is `#[derive(Clone, Copy)]` of handles.

---

## 9. Line counts

Render side (`crates/cairn-app/src/`):

| File | Lines |
|---|---|
| window.rs | 5709 (production 1-592) |
| session.rs | 2054 (production 1-513) |
| window_check.rs | 1620 |
| operations/activity.rs | 1505 |
| commit_box/tests.rs | 1417 |
| diff/state.rs | 1382 |
| local_changes/actions/tests/files.rs | 1330 |
| local_changes/tests.rs | 1215 |
| operations/tests.rs | 1189 |
| operations/create_branch/tests.rs | 1129 |
| local_changes/actions/tests/gesture.rs | 1050 |
| sidebar/tests.rs | 834 |
| local_changes/state.rs | 735 |
| local_changes/actions.rs | 693 |
| commit_box/state.rs | 652 |
| diff/state/working.rs | 621 |
| commit_box/pane.rs | 572 |
| operations/create_branch.rs | 521 |
| local_changes/pane.rs | 487 |
| menu_bar/tests.rs | 470 |
| history/state.rs | 455 |
| local_changes/next_selection.rs | 446 |
| diff/actions.rs | 425 |
| history/ref_find.rs | 351 |
| detail/changes_tab.rs | 348 |
| operations/ending_dialog.rs | 346 |
| history/selection.rs | 295 |
| detail/pane.rs | 282 |
| sidebar/state.rs | 270 |
| operations/local_writes.rs | 259 |
| operations/fetch.rs | 247 |
| main.rs | 238 |
| operations/output_tail.rs | 211 |
| frame.rs | 207 |
| closing.rs | 205 |
| detail/file_filter.rs | 201 |
| operations/leftover_lock.rs | 194 |
| sidebar/menu.rs | 159 |
| refresh/state.rs | 151 |
| operations/confirming.rs | 147 |
| history/status_text.rs | 142 |
| sidebar/pane.rs | 123 |
| toolbar.rs | 121 |
| menu_bar.rs | 107 |
| shortcuts.rs | 100 |
| repository_path.rs | 89 |
| detail/split.rs | 85 |
| modal.rs | 77 |
| worker.rs | 56 |
| clipboard.rs | 52 |
| refresh/focus.rs | 26 |
| **All render-side files** | **32043** |

Worker API used by the render side:

| File | Lines |
|---|---|
| worker/pool.rs | 2711 |
| worker/request.rs | 1044 |
| worker/askpass.rs | 322 |
| worker/startup.rs | 252 |
| worker/wake.rs | 152 |
| worker/discovery.rs | 128 |
| worker/zone.rs | 27 |

cairn-ui chrome:

| File | Lines |
|---|---|
| accelerators.rs | 1964 |
| sidebar.rs | 986 |
| toolbar/status_box.rs | 383 |
| toolbar.rs | 344 |
| dialogs/credential_prompt.rs | 280 |
| caption_row.rs | 201 |
| diff/settings.rs | 199 |
| menu/bar.rs | 175 |

`cairn-askpass/src/channel.rs`: 377 lines.

---

## What must change, as the code stands (noted, not designed)

1. **Opening a repository.** `repository_path::chosen` takes one argument; `opened`, the `worker::open` hook and the update task exist once in `app()`. Each must exist once per repository, created after the first render, with no fixed hook count.
2. **The state.** `View`'s 23 per-repository fields (and the per-repository parts of `sidebar`, `local`, `writes` and `branch`) must exist per tab. `held_keys`, `zone` and `frame` (and the modal/`ContextMenuViewer` hosting) are window-wide. A decision is owed on each AMBIGUOUS field in §2.
3. **Updates.** There must be N streams, each driven by its own task, or a tag added to each update. `session::apply`'s writes of `frame.git`, `zone` and `diff_settings` from per-repository updates need a rule.
4. **Closing.** `Closing`, `Worker.closing`, `LocalWrites::closing`/`closing_line`, and the stream-end → `close_current_window()` path all assume one repository per window. Closing a tab and closing the window become separate paths.
5. **Modal and keys.** `modal::modal`/`drawn` and `keys_inert` rank per-repository dialogs in one window slot. Which repository's dialog is on top, and whether a background tab's dialog takes over the window's keys, is undecided.
6. **Credential prompt.** It needs a repository identity, a per-repository `Replier`, a tagged `PromptId`, and a title or wording that says which repository asked.
7. **Hooks and equality.** Every `use_side_effect`/`use_hook` that captures `view`/`submit`, and the eight `PartialEq` impls that compare `submit.is_some()`, assume one repository for the window's life.
8. **Repository identity on screen.** The status box, sidebar header, OS title and Copy (Full) Repository Path read the one `View`. A tab strip would go between the toolbar (window.rs:250) and the banners (window.rs:253), as Fork places it.
9. **Tests.** Six 33-field literal builders and about 209 tests reach `View` directly. The six builders and the `launch` helpers are the change points.
