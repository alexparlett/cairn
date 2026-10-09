//! C14's window check, measured (phase 08): the real window — `window::window`, every
//! component it draws — over the real worker and the real engine, against the repository
//! the bar names, headless through `freya-testing`, while the heaviest subjects load.
//!
//! The criterion asks for a look at the window by hand; this is the instrument beside that
//! look, not a replacement for it. Frames are paced as a 60 Hz display paces them: each
//! frame waits for whatever the worker sends until the frame is due, applies every update
//! that arrived (`session::apply`, the function the application's update task calls, timed
//! one by one: the UI thread's work per update), sends the frame's input — a scroll, where the
//! phase scrolls — and runs one `sync_and_update` (events, components, layout and
//! accessibility: everything a frame does on the UI thread but paint, which the headless
//! runner can only measure as a raster snapshot encoded to PNG, reported apart as an upper
//! bound). A frame's time is the two together — every update applied in it and its
//! `sync_and_update` — since the application applies its updates on the UI thread between
//! frames (phase 08 QA's R2). Times are wall-clock on the machine it runs on; nothing here
//! asserts one.
//!
//! Run it in a release build, warm, against the repository the bar names (read only), and —
//! for refs-and-status C12's large status — a scratch clone of it with no alternates, dirtied
//! there and never in the bench repository:
//!
//! ```text
//! GIT_OPTIONAL_LOCKS=0 git clone --quiet --local --no-hardlinks \
//!   ~/Development/bench/rust "$SCRATCH/rust"   # then modify, stage and add files in it
//! CAIRN_BENCH_REPO=~/Development/bench/rust CAIRN_SCRATCH_REPO="$SCRATCH/rust" \
//!   cargo test -p cairn-app --release -- --ignored --nocapture window_check
//! ```
//!
//! The window opens as the application opens it — the refs, ahead/behind and status read by a
//! refresh, the history walked from every ref and labelled, the sidebar's rows laid out
//! (refs-and-status C12) — and, on the scratch clone, Local Changes is shown over its status of
//! thousands of paths: its lists drawn, the first path's diff asked and drawn, a list scrolled,
//! a filter typed and a refresh landed.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use cairn_model::{DiffContent, History, Oid, RemoteSummary, RowId};
use cairn_ui::diff_palette::DIFF_FONT_FAMILY;
use cairn_ui::{DetailTab, DiffSettings, MainView};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{PlatformEvent, WheelEventName};

use crate::diff_state::{AllState, Answer, DiffState, answered_expansion};
use crate::fetch_state::FetchStatus;
use crate::history_state::Progress;
use crate::window::{PANE_HEIGHT, View, window};
use crate::worker::{
    Comparison, Discovery, EXPAND_ALL_LINES, RepositoryHandle, Request, Update, Updates, open,
    update_within,
};
use crate::{PAGE_ROWS, diff_actions, selection, session};

const WIDTH: f32 = 1440.;
const HEIGHT: f32 = 900.;
/// One frame of a 60 Hz display, which is the bar a frame is held to.
const FRAME: Duration = Duration::from_micros(16_667);
/// How long any phase may take before the check gives up on it.
const PATIENCE: Duration = Duration::from_secs(60);

/// The subjects of `measured-baseline.md` the window is checked on.
const S1: &str = "cf2dff2b1e3fa55fa5415d524200070d0d7aacfe";
const M1: &str = "5a3292f163da3327523ddec5bc44d17c2378ec37";
const F1: &str = "6a6e8446b97e8a3dfc0984660253b1ac437a445a";

/// What one phase measured.
#[derive(Default)]
struct Phase {
    frames: Vec<Duration>,
    /// Each update applied, by kind, with the UI thread's time applying it.
    applied: Vec<(&'static str, Duration)>,
    /// When each kind of update first arrived, after the phase began.
    arrived: Vec<(&'static str, Duration)>,
    /// When the phase's condition first held, after it began.
    done: Option<Duration>,
    /// The slowest frame, and what was applied before it.
    slowest: (Duration, Vec<&'static str>),
}

fn kind(update: &Update) -> &'static str {
    match update {
        Update::Rows { .. } => "rows",
        Update::Changes { .. } => "changes",
        Update::FileDiff { .. } => "file diff",
        Update::Expanded { .. } => "expanded page",
        Update::FilteredFiles { .. } => "filter",
        Update::Superseded(_) => "superseded",
        Update::DiffFailed { .. } => "diff failed",
        Update::ConfiguredContext { .. } => "configured context",
        Update::Remotes { .. } => "remotes",
        Update::Opened { .. } => "opened",
        Update::Refs { .. } => "refs",
        Update::AheadBehind { .. } => "ahead/behind",
        Update::Status { .. } => "status",
        Update::FilteredRefs { .. } => "ref filter",
        Update::FilteredLocalChanges { .. } => "local changes filter",
        Update::RefreshFailed { .. } => "refresh failed",
        Update::WriteStarted { .. } => "write started",
        Update::WriteEnded { .. } => "write ended",
        Update::Failed { .. }
        | Update::LocksAtOpen { .. }
        | Update::WriteOutput { .. }
        | Update::WorkerLost { .. }
        | Update::FetchStarted { .. }
        | Update::FetchProgress { .. }
        | Update::FetchFinished { .. }
        | Update::FetchCancelled { .. }
        | Update::FetchFailed { .. }
        | Update::FetchRefused { .. }
        | Update::Prompt { .. }
        | Update::CommandLog { .. } => "other",
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn percentile(sorted: &[Duration], at: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let index = ((sorted.len() - 1) as f64 * at).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

impl Phase {
    fn report(&self, name: &str) {
        let mut frames = self.frames.clone();
        frames.sort_unstable();
        let over = frames.iter().filter(|frame| **frame > FRAME).count();
        eprintln!("{name}");
        if let Some(done) = self.done {
            eprintln!("  answered and drawn after {:.1} ms", ms(done));
        }
        if !self.slowest.1.is_empty() || self.slowest.0 > Duration::ZERO {
            eprintln!(
                "  slowest frame {:.2} ms, after applying {:?}",
                ms(self.slowest.0),
                self.slowest.1
            );
        }
        for (kind, at) in &self.arrived {
            eprintln!("  first {kind} arrived after {:.1} ms", ms(*at));
        }
        eprintln!(
            "  frames: {} — median {:.2} ms, p99 {:.2} ms, max {:.2} ms; {} over 16.7 ms",
            frames.len(),
            ms(percentile(&frames, 0.5)),
            ms(percentile(&frames, 0.99)),
            ms(frames.last().copied().unwrap_or_default()),
            over
        );
        let mut kinds: Vec<&'static str> = self.applied.iter().map(|(kind, _)| *kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        for kind in kinds {
            let mut taken: Vec<Duration> = self
                .applied
                .iter()
                .filter(|(applied, _)| *applied == kind)
                .map(|(_, taken)| *taken)
                .collect();
            taken.sort_unstable();
            eprintln!(
                "  applying {kind}: {} updates, median {:.3} ms, max {:.3} ms",
                taken.len(),
                ms(percentile(&taken, 0.5)),
                ms(taken.last().copied().unwrap_or_default())
            );
        }
    }
}

/// What a frame sends besides the updates.
#[derive(Clone, Copy)]
enum Stimulus {
    Nothing,
    /// A wheel scroll of `dy` pixels at `(x, y)`.
    Scroll {
        x: f64,
        y: f64,
        dy: f64,
    },
}

struct Harness {
    test: TestingRunner,
    view: View,
    updates: Updates,
    handle: RepositoryHandle,
}

impl Harness {
    fn submit(&self) -> impl Fn(Request) + use<> {
        let handle = self.handle.clone();
        move |request| {
            handle.submit(request);
        }
    }

    /// Frames until `done` holds of the view, then `settle` more, sending `input` with each.
    fn pump(
        &mut self,
        input: Stimulus,
        settle: usize,
        mut done: impl FnMut(View) -> bool,
    ) -> Phase {
        let mut phase = Phase::default();
        let started = Instant::now();
        let mut after = None::<usize>;
        loop {
            let frame_started = Instant::now();
            let mut this_frame: Vec<&'static str> = Vec::new();
            // The UI thread's work this frame: every update applied, then the frame itself.
            let mut applied_in_frame = Duration::ZERO;
            loop {
                let wait = FRAME.saturating_sub(frame_started.elapsed());
                let Some(update) = update_within(&mut self.updates, wait) else {
                    break;
                };
                let kind = kind(&update);
                if !phase.arrived.iter().any(|(seen, _)| *seen == kind) {
                    phase.arrived.push((kind, started.elapsed()));
                }
                let submit = self.submit();
                let view = self.view;
                let applying = Instant::now();
                self.test.run_in(|| {
                    session::apply(
                        update,
                        view,
                        &session::Worker {
                            submit: &submit,
                            refuse: &|_| {},
                            closing: false,
                        },
                    );
                });
                let applied = applying.elapsed();
                applied_in_frame += applied;
                phase.applied.push((kind, applied));
                this_frame.push(kind);
                if frame_started.elapsed() >= FRAME {
                    break;
                }
            }
            // The input is sent, and the frame that handles it timed: the runner's own scroll
            // would handle it inside an untimed update of its own.
            let framing = Instant::now();
            if let Stimulus::Scroll { x, y, dy } = input {
                self.test.send_event(PlatformEvent::Wheel {
                    name: WheelEventName::Wheel,
                    scroll: (0., dy).into(),
                    cursor: (x, y).into(),
                    source: WheelSource::Device,
                    granularity: WheelGranularity::Pixel,
                    timestamp: Instant::now(),
                });
            }
            self.test.sync_and_update();
            let framed = applied_in_frame + framing.elapsed();
            phase.frames.push(framed);
            if std::env::var_os("CAIRN_WINDOW_CHECK_FRAMES").is_some() {
                eprintln!(
                    "    frame {} at {:.1} ms: {:.2} ms after {:?}",
                    phase.frames.len(),
                    ms(started.elapsed()),
                    ms(framed),
                    this_frame
                );
            }
            if framed > phase.slowest.0 {
                phase.slowest = (framed, this_frame);
            }
            let view = self.view;
            match after {
                None if self.test.run_in(|| done(view)) => {
                    phase.done = Some(started.elapsed());
                    after = Some(0);
                }
                Some(count) if count >= settle => return phase,
                Some(count) => after = Some(count + 1),
                None => {}
            }
            assert!(
                started.elapsed() < PATIENCE,
                "a phase of the window check did not finish in {PATIENCE:?}"
            );
        }
    }

    /// A raster snapshot of the window, encoded: an upper bound on what a frame's paint costs.
    fn paint(&mut self) -> Duration {
        let started = Instant::now();
        let _ = self.test.render();
        started.elapsed()
    }

    fn choose(&mut self, id: &str) -> Comparison {
        let oid = Oid::parse(id).unwrap_or_else(|_| panic!("{id} is not an id"));
        let submit = self.submit();
        let view = self.view;
        self.test
            .run_in(|| selection::choose(RowId::Commit(oid), view, Some(&submit)));
        Comparison::Commit(oid)
    }

    fn show(&mut self, tab: DetailTab) {
        let mut detail_tab = self.view.detail_tab;
        self.test.run_in(|| detail_tab.set(tab));
    }
}

/// The first `count` texts drawn below the history: what the pane shows, to see it is drawn.
fn shown(test: &TestingRunner, count: usize) -> Vec<String> {
    let top = f64::from(HEIGHT) - f64::from(PANE_HEIGHT);
    let mut texts: Vec<(f32, f32, String)> = test.find_many(|node, element| {
        let area = node.layout().area;
        let visible = f64::from(area.min_y()) >= top && area.min_y() < HEIGHT;
        let text = if let Some(label) = Label::try_downcast(element) {
            Some(label.text.to_string())
        } else {
            Paragraph::try_downcast(element).map(|paragraph| {
                paragraph
                    .spans
                    .iter()
                    .map(|span| span.text.as_ref())
                    .collect()
            })
        };
        text.filter(|_| visible)
            .map(|text| (area.min_y(), area.min_x(), text))
    });
    texts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    texts
        .into_iter()
        .take(count)
        .map(|(_, _, text)| text)
        .collect()
}

/// The resident memory of this process, in KiB, as the kernel counts it.
fn resident_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("VmRSS:"))
                .and_then(|value| value.trim().trim_end_matches("kB").trim().parse().ok())
        })
        .unwrap_or(0)
}

fn changes_ready(view: View, of: Comparison) -> bool {
    matches!(view.diff.peek().changes(), Some((asked, Answer::Ready(_))) if asked == of)
}

fn launch(path: &str) -> Harness {
    let git = Discovery::start();
    let (handle, updates, _reply) =
        open(path, &git).unwrap_or_else(|error| panic!("opening {path}: {error}"));
    let submit: Rc<dyn Fn(Request)> = {
        let handle = handle.clone();
        Rc::new(move |request| {
            handle.submit(request);
        })
    };
    let shared = Rc::new(RefCell::new(Some(submit)));
    let app = move || {
        let view = use_consume::<View>();
        window(PATH, view, shared.borrow().clone(), None)
    };
    let (mut test, view) = TestingRunner::new(
        app,
        (WIDTH, HEIGHT).into(),
        |runner| {
            runner.provide_root_context(|| View {
                rows: State::create(History::new()),
                progress: State::create(Progress::opening()),
                selected: State::create(None),
                fetch: State::create(FetchStatus::Idle),
                prompt: State::create(None),
                remotes: State::create(Vec::<RemoteSummary>::new()),
                refused: State::create(None),
                diff: State::create(DiffState::default()),
                history_scroll: ScrollController::new(0, 0, Vec::new()),
                history_cursor: State::create(0),
                detail_tab: State::create(DetailTab::default()),
                pane_collapsed: State::create(false),
                pane_height: State::create(PANE_HEIGHT),
                diff_settings: State::create(DiffSettings::default()),
                diff_scroll: ScrollController::new(0, 0, Vec::new()),
                change_cursor: State::create(None),
                filter_text: State::create(String::new()),
                changes_list_width: State::create(crate::changes_tab::LIST_WIDTH),
                pair: State::create(None),
                held_keys: State::create(cairn_ui::accelerators::HeldKeys::default()),
                refreshed: State::create(crate::refresh_state::RefreshState::default()),
                repository: State::create(None),
                sidebar: crate::sidebar_state::SidebarView::created(),
                local: crate::local_changes_state::LocalChangesView::created(),
                writes: State::create(crate::local_writes::LocalWrites::default()),
                confirming: State::create(None),
            })
        },
        1.,
    );
    test.set_fonts(HashMap::from([(DIFF_FONT_FAMILY, crate::DIFF_FONT)]));
    test.sync_and_update();
    // As the application opens a repository: the refresh's refs open the history from every
    // ref (refs-and-status C12).
    handle.submit(Request::ListRemotes);
    handle.submit(Request::ConfiguredContext);
    handle.submit(Request::Refresh);
    Harness {
        test,
        view,
        updates,
        handle,
    }
}

const PATH: &str = "bench";

/// C14's window check: the window over the bench repository while S1's and M1's change sets
/// load, while Expand All reads them, while F1 loads past the limits, and while each scrolls.
#[test]
#[ignore = "needs a large repository named by CAIRN_BENCH_REPO, and a release build"]
fn window_check() {
    let path = std::env::var("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO");
    eprintln!(
        "window check over {path}: {WIDTH}x{HEIGHT} at scale 1, release build: {}, Expand All \
         budget {EXPAND_ALL_LINES} lines\n",
        !cfg!(debug_assertions)
    );
    let mut harness = launch(&path);
    let history = (WIDTH as f64 / 2., 200.);
    let pane = (WIDTH as f64 / 2., f64::from(HEIGHT) - 100.);

    let opened = harness.pump(Stimulus::Nothing, 30, landed);
    opened.report(
        "opening: the refs, the first page of the decorated history, the sidebar's rows and \
         the status",
    );
    describe_landed(&harness);
    // The history scrolled with nothing loading: what a scroll costs on its own, first time
    // included, to set what follows against.
    let mut scrolled = 0;
    let alone = harness.pump(
        Stimulus::Scroll {
            x: history.0,
            y: history.1,
            dy: -240.,
        },
        0,
        move |_| {
            scrolled += 1;
            scrolled > 60
        },
    );
    alone.report("the history scrolled, 9 rows a frame, with nothing loading");

    // The Commit tab drawn the first time in the session, over the newest commit: the toolkit's
    // first text of each face and fallback costs a frame of its own once, whatever the commit
    // (measured at 11.5 ms headless over 2,828 files and over 27,592 alike), which the
    // subjects after it would otherwise carry.
    let newest = harness
        .view
        .rows
        .peek()
        .id(0)
        .unwrap_or_else(|| panic!("no history"));
    let first = {
        let submit = harness.submit();
        let view = harness.view;
        harness
            .test
            .run_in(|| selection::choose(newest, view, Some(&submit)));
        selection::comparison_of(newest)
    };
    let drawn = harness.pump(Stimulus::Nothing, 10, move |view| {
        changes_ready(view, first)
    });
    drawn.report("the Commit tab drawn the first time in the session, over the newest commit");

    for (name, subject) in [("S1 cf2dff2b1e3", S1), ("M1 5a3292f163d", M1)] {
        harness.show(DetailTab::Commit);
        // First asked in this session: the diff thread keeps no answer for it, so `git
        // diff-tree` runs while the window draws (the repository's pages warm from the
        // operating system's cache, as the bar's numbers are warm).
        let of = harness.choose(subject);
        let cold = harness.pump(
            Stimulus::Scroll {
                x: history.0,
                y: history.1,
                dy: -240.,
            },
            20,
            move |view| changes_ready(view, of),
        );
        cold.report(&format!(
            "{name}: its change set asked the first time, git diff-tree run, and drawn in the \
             Commit tab, the history scrolled every frame"
        ));
        let _ = harness.choose(F1);
        let _ = harness.pump(Stimulus::Nothing, 2, |view| {
            changes_ready(
                view,
                Comparison::Commit(Oid::parse(F1).unwrap_or_else(|_| unreachable!())),
            )
        });

        // Asked again: the answer the diff thread kept.
        let of = harness.choose(subject);
        let loading = harness.pump(
            Stimulus::Scroll {
                x: history.0,
                y: history.1,
                dy: -240.,
            },
            20,
            move |view| changes_ready(view, of),
        );
        loading.report(&format!(
            "{name}: asked again, from the answer the diff thread kept, the history scrolled \
             every frame"
        ));
        let files = harness
            .view
            .diff
            .peek()
            .changes()
            .and_then(|(_, answer)| match answer {
                Answer::Ready(changes) => Some(changes.files.len()),
                Answer::Waiting | Answer::Failed(_) => None,
            })
            .unwrap_or(0);
        eprintln!(
            "  {files} files; the pane draws {:?}",
            shown(&harness.test, 12)
        );

        let mut scrolled = 0;
        let scrolling = harness.pump(
            Stimulus::Scroll {
                x: pane.0,
                y: pane.1,
                dy: -2_400.,
            },
            0,
            move |_| {
                scrolled += 1;
                scrolled > 120
            },
        );
        scrolling.report(&format!("{name}: its files scrolled, 100 rows a frame"));

        // Back to the top of the files, where Expand All opens them.
        let _ = harness.pump(
            Stimulus::Scroll {
                x: pane.0,
                y: pane.1,
                dy: 1e9,
            },
            0,
            |_| true,
        );
        let before = resident_kib();
        let submit = harness.submit();
        let view = harness.view;
        harness
            .test
            .run_in(|| diff_actions::expand_all(true, view, Some(&submit)));
        let expanding = harness.pump(
            Stimulus::Scroll {
                x: pane.0,
                y: pane.1,
                dy: -240.,
            },
            20,
            |view| {
                matches!(
                    view.diff.peek().all_state(),
                    Some(AllState::Done | AllState::Stopped)
                )
            },
        );
        expanding.report(&format!(
            "{name}: Expand All, the Commit tab scrolled every frame"
        ));
        let (opened, stopped) = harness.test.run_in(|| {
            let state = harness.view.diff.peek();
            let expansion = answered_expansion(&state);
            (expansion.len(), expansion.stopped_at_budget())
        });
        eprintln!(
            "  {opened} files opened, {} left collapsed, stopped at the budget: {stopped}; \
             resident memory {} KiB before, {} KiB after; the pane draws {:?}",
            files.saturating_sub(opened),
            before,
            resident_kib(),
            shown(&harness.test, 16)
        );
        let mut scrolled = 0;
        let through = harness.pump(
            Stimulus::Scroll {
                x: pane.0,
                y: pane.1,
                dy: -2_400.,
            },
            0,
            move |_| {
                scrolled += 1;
                scrolled > 120
            },
        );
        through.report(&format!(
            "{name}: scrolled through the files opened, 100 rows a frame"
        ));
        eprintln!(
            "  paint, raster snapshot encoded to PNG (an upper bound): {:.2} ms\n",
            ms(harness.paint())
        );
        let submit = harness.submit();
        let view = harness.view;
        harness
            .test
            .run_in(|| diff_actions::expand_all(false, view, Some(&submit)));
    }

    // F1: refused on the byte ceiling, then Load Diff.
    harness.show(DetailTab::Changes);
    let f1 = harness.choose(F1);
    let refused = harness.pump(Stimulus::Nothing, 10, move |view| {
        changes_ready(view, f1)
            && matches!(view.diff.peek().file(), Some((_, Answer::Ready(Some(_)))))
    });
    refused.report("F1 6a6e8446b97: its change set and its one file, refused as too large");
    let submit = harness.submit();
    let view = harness.view;
    harness
        .test
        .run_in(|| diff_actions::load_anyway(view, Some(&submit)));
    let loading = harness.pump(
        Stimulus::Scroll {
            x: history.0,
            y: history.1,
            dy: -240.,
        },
        20,
        |view| {
            view.diff
                .peek()
                .shown_file()
                .is_some_and(|shown| matches!(shown.diff().content, DiffContent::Text { .. }))
        },
    );
    loading.report("F1: Load Diff, the history scrolled every frame");
    let mut scrolled = 0;
    let diff_pane = (WIDTH as f64 * 0.75, f64::from(HEIGHT) - 80.);
    let scrolling = harness.pump(
        Stimulus::Scroll {
            x: diff_pane.0,
            y: diff_pane.1,
            dy: -2_400.,
        },
        0,
        move |_| {
            scrolled += 1;
            scrolled > 120
        },
    );
    scrolling.report("F1: the loaded diff scrolled, about 140 rows a frame");
    eprintln!("  the pane draws {:?}", shown(&harness.test, 12));
    eprintln!(
        "  paint, raster snapshot encoded to PNG (an upper bound): {:.2} ms\n",
        ms(harness.paint())
    );
    drop(harness);

    local_changes_check(&path);
}

/// Whether the window has landed what opening it reads (refs-and-status C12): the refs, a
/// page of the history walked from them, the sidebar's rows and the working tree's status.
fn landed(view: View) -> bool {
    let refreshed = view.refreshed.peek();
    view.rows.peek().len() >= PAGE_ROWS
        && refreshed.refs().is_some()
        && refreshed.status().is_some()
        && view.sidebar.state.peek().shown().is_some()
}

/// What opening landed: the refs, the labelled rows among those loaded, the status's paths.
fn describe_landed(harness: &Harness) {
    let view = harness.view;
    let (refs, rows, labelled, paths) = harness.test.run_in(|| {
        let refreshed = view.refreshed.peek();
        let history = view.rows.peek();
        let labelled = (0..history.len())
            .filter_map(|index| history.row(index))
            .filter(|row| !row.labels().is_empty())
            .count();
        (
            refreshed.refs().map_or(0, |refs| refs.refs.len()),
            history.len(),
            labelled,
            refreshed
                .local_changes()
                .map_or(0, |changes| changes.paths()),
        )
    });
    eprintln!(
        "  {refs} refs; {rows} rows loaded, {labelled} of them labelled; status lists {paths} \
         paths\n"
    );
}

/// C12's large status: Local Changes over a scratch clone of the bench repository, dirtied
/// there, with thousands of paths — its lists drawn, the first path's diff asked and drawn,
/// Unstaged scrolled, a filter typed, and a refresh landed while it is shown.
fn local_changes_check(bench: &str) {
    let scratch = std::env::var("CAIRN_SCRATCH_REPO")
        .expect("set CAIRN_SCRATCH_REPO to a scratch clone of the bench repository, dirtied");
    if let Some(refused) = scratch_refusal(Path::new(bench), Path::new(&scratch)) {
        panic!("{refused}");
    }
    let mut harness = launch(&scratch);
    let opened = harness.pump(Stimulus::Nothing, 30, landed);
    opened.report("scratch clone: opening, its large status among what lands");
    describe_landed(&harness);
    let paths = harness.test.run_in(|| {
        harness
            .view
            .refreshed
            .peek()
            .local_changes()
            .map_or(0, |changes| changes.paths())
    });
    assert!(
        paths >= 1_000,
        "the scratch clone's status lists only {paths} paths"
    );

    // Local Changes pressed: its lists drawn and the first path's diff asked and drawn.
    let mut main = harness.view.sidebar.main;
    harness.test.run_in(|| main.set(MainView::LocalChanges));
    let shown = harness.pump(Stimulus::Nothing, 20, |view| {
        view.local.state.peek().has_lists()
            && view
                .diff
                .peek()
                .working_shown()
                .is_some_and(|shown| !matches!(shown, crate::diff_state::WorkingShown::Waiting))
    });
    shown.report("Local Changes shown: both lists drawn, the first path's diff asked and drawn");
    let (unstaged, staged) = harness.test.run_in(|| {
        let state = harness.view.local.state.peek();
        let lists = crate::local_changes_state::drawn_changes(&state);
        (
            lists.len(cairn_model::ChangeList::Unstaged),
            lists.len(cairn_model::ChangeList::Staged),
        )
    });
    eprintln!(
        "  Unstaged {unstaged} rows, Staged {staged} rows; the view draws {:?}",
        visible_texts(&harness.test, 14)
    );

    let unstaged_list = (f64::from(crate::sidebar_state::SIDEBAR_WIDTH) + 120., 150.);
    let mut scrolled = 0;
    let scrolling = harness.pump(
        Stimulus::Scroll {
            x: unstaged_list.0,
            y: unstaged_list.1,
            dy: -2_400.,
        },
        0,
        move |_| {
            scrolled += 1;
            scrolled > 120
        },
    );
    scrolling.report("Unstaged scrolled, 100 rows a frame");

    let mut filter = harness.view.local.filter_text;
    harness
        .test
        .run_in(|| filter.set("cairn-new-05".to_owned()));
    let filtering = harness.pump(Stimulus::Nothing, 10, |view| {
        let state = view.local.state.peek();
        state.text() == "cairn-new-05" && state.is_settled()
    });
    filtering.report("a filter typed: its rows asked of a worker and drawn");
    harness.test.run_in(|| filter.set(String::new()));
    let _ = harness.pump(Stimulus::Nothing, 5, |view| {
        view.local.state.peek().is_settled()
    });

    // A refresh while the view is shown — what focus gained asks: the status read again, laid
    // out on the refresh thread, the lists replaced and the chosen path asked again.
    let before = harness
        .test
        .run_in(|| harness.view.local.state.peek().serial());
    harness.handle.submit(Request::Refresh);
    let refreshing = harness.pump(Stimulus::Nothing, 20, move |view| {
        view.local.state.peek().serial() > before
            && view.diff.peek().working_choice().map(|choice| choice.lists)
                == Some(view.local.state.peek().serial())
    });
    refreshing.report("a refresh landed while Local Changes is shown: the lists replaced");
    eprintln!(
        "  paint, raster snapshot encoded to PNG (an upper bound): {:.2} ms",
        ms(harness.paint())
    );
}

/// The first `count` texts drawn anywhere in the window, top to bottom.
fn visible_texts(test: &TestingRunner, count: usize) -> Vec<String> {
    let mut texts: Vec<(f32, f32, String)> = test.find_many(|node, element| {
        let area = node.layout().area;
        let text = if let Some(label) = Label::try_downcast(element) {
            Some(label.text.to_string())
        } else {
            Paragraph::try_downcast(element).map(|paragraph| {
                paragraph
                    .spans
                    .iter()
                    .map(|span| span.text.as_ref())
                    .collect()
            })
        };
        text.filter(|_| area.min_x() > crate::sidebar_state::SIDEBAR_WIDTH && area.min_y() < HEIGHT)
            .map(|text| (area.min_y(), area.min_x(), text))
    });
    texts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    texts
        .into_iter()
        .take(count)
        .map(|(_, _, text)| text)
        .collect()
}

/// Why `scratch` may not stand in for a scratch clone of `bench`, or `None` when it may: it is
/// the bench, or inside it, or holds it; its git directory — `.git`, or the one a `.git` file's
/// `gitdir:` line names, a symbolic link followed — or the common directory a linked worktree's
/// `commondir` names is inside the bench; or it borrows objects through `alternates`. Read with
/// `std::fs` alone: the check runs no `git`. A read in a repository whose git directory is the
/// bench's would read, and could leave files in, the bench.
fn scratch_refusal(bench: &Path, scratch: &Path) -> Option<String> {
    let canonical = |path: &Path| {
        std::fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let (bench, scratch) = match (canonical(bench), canonical(scratch)) {
        (Ok(bench), Ok(scratch)) => (bench, scratch),
        (Err(error), _) | (_, Err(error)) => return Some(error),
    };
    if scratch.starts_with(&bench) || bench.starts_with(&scratch) {
        return Some("the scratch clone is the bench repository, inside it or holding it".into());
    }
    let dot = scratch.join(".git");
    let git_dir: PathBuf = match std::fs::read_to_string(&dot) {
        // A `.git` file: a linked worktree's, or a separate git directory's.
        Ok(file) => match file.lines().find_map(|line| line.strip_prefix("gitdir: ")) {
            Some(named) => scratch.join(named.trim()),
            None => return Some(format!("{} names no git directory", dot.display())),
        },
        Err(_) => dot,
    };
    let git_dir = match canonical(&git_dir) {
        Ok(git_dir) => git_dir,
        Err(error) => return Some(error),
    };
    let common = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(named) => match canonical(&git_dir.join(named.trim())) {
            Ok(common) => common,
            Err(error) => return Some(error),
        },
        Err(_) => git_dir.clone(),
    };
    if git_dir.starts_with(&bench) || common.starts_with(&bench) {
        return Some(format!(
            "the scratch clone's git directory ({}) is the bench repository's",
            common.display()
        ));
    }
    if common.join("objects/info/alternates").exists() {
        return Some(
            "the scratch clone borrows objects through alternates: a read there could touch the \
             bench"
                .into(),
        );
    }
    None
}

/// Phase 09 QA's TC2: the window check's guard refuses every scratch that would read the
/// bench's git directory — the bench itself, a directory inside it, a linked worktree of it, a
/// `.git` symbolic link to it, a clone borrowing objects — and accepts a clone of its own.
/// Built with `std::fs` in the temporary directory; no `git` runs. Caught by: a guard that
/// reads only `<scratch>/.git/objects/info/alternates` (a linked worktree passes).
#[test]
fn the_scratch_guard_refuses_whatever_shares_the_benchs_git_directory() {
    let root = std::env::temp_dir().join(format!("cairn-scratch-guard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let make = |path: &Path| std::fs::create_dir_all(path).unwrap_or_else(|e| panic!("{e}"));
    let write =
        |path: &Path, text: &str| std::fs::write(path, text).unwrap_or_else(|e| panic!("{e}"));
    let bench = root.join("bench");
    make(&bench.join(".git/objects/info"));
    make(&bench.join(".git/worktrees/linked"));
    write(&bench.join(".git/worktrees/linked/commondir"), "../..\n");

    let clone = root.join("clone");
    make(&clone.join(".git/objects/info"));
    assert_eq!(
        scratch_refusal(&bench, &clone),
        None,
        "a clone of its own was refused"
    );

    let linked = root.join("linked");
    make(&linked);
    write(
        &linked.join(".git"),
        &format!(
            "gitdir: {}\n",
            bench.join(".git/worktrees/linked").display()
        ),
    );
    let linked_relative = root.join("linked-relative");
    make(&linked_relative);
    write(
        &linked_relative.join(".git"),
        "gitdir: ../bench/.git/worktrees/linked\n",
    );
    let symlinked = root.join("symlinked");
    make(&symlinked);
    std::os::unix::fs::symlink(bench.join(".git"), symlinked.join(".git"))
        .unwrap_or_else(|e| panic!("{e}"));
    let borrowing = root.join("borrowing");
    make(&borrowing.join(".git/objects/info"));
    write(
        &borrowing.join(".git/objects/info/alternates"),
        &format!("{}\n", bench.join(".git/objects").display()),
    );
    let inside = bench.join("sub");
    make(&inside);
    for (name, scratch) in [
        ("the bench", &bench),
        ("a directory inside it", &inside),
        ("a linked worktree", &linked),
        ("a linked worktree named relatively", &linked_relative),
        ("a .git symbolic link", &symlinked),
        ("a clone borrowing objects", &borrowing),
    ] {
        assert!(
            scratch_refusal(&bench, scratch).is_some(),
            "{name} was accepted"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}
