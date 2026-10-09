//! Local Changes in the window, headless (refs-and-status R9, criterion C9): pressed in the
//! sidebar, the view draws Unstaged above Staged with their badges and the count beside the
//! entry; the first path is chosen and its diff asked through the working-tree query, and drawn
//! for that query alone; a conflicted path draws its notice and asks nothing; a path in both
//! lists draws each list's diff; a refresh that takes the chosen path away draws no diff under
//! no row; the filter is asked of a worker. Updates are applied through `session::apply`, as
//! the worker's stream applies them.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use cairn_model::{
    ChangeStatus, ChangedEntry, ChangedFile, ChangedRange, ConflictKind, ConflictedEntry, Context,
    DiffContent, DiffLine, DisplayOverlay, FileDiff, History, LineSpan, LocalChanges, RepoPath,
    ShownDiff, StagedChange, StatusEntry, TextDiff, UnstagedChange, WorkingTreeStatus,
};
use cairn_ui::accelerators::HeldKeys;
use cairn_ui::{CONFLICTED, DetailTab, DiffSettings, MainView};
use freya::prelude::*;
use freya_testing::TestingRunner;

use crate::diff_state::DiffState;
use crate::fetch_state::FetchStatus;
use crate::history_state::Progress;
use crate::refresh_state::RefreshState;
use crate::session::{self, Worker};
use crate::sidebar_state::{SIDEBAR_WIDTH, SidebarView};
use crate::window::{PANE_HEIGHT, View, window};
use crate::worker::{FileQuery, FileTarget, Request, Update, WorkingSide};

const WIDTH: f32 = 1200.;
const HEIGHT: f32 = 700.;

type Submitted = Rc<RefCell<Vec<Request>>>;

fn launch() -> (TestingRunner, View, Submitted) {
    let submitted = Submitted::default();
    let app = {
        let submitted = submitted.clone();
        move || {
            let view = use_consume::<View>();
            let submitted = submitted.clone();
            let submit: Rc<dyn Fn(Request)> =
                Rc::new(move |request| submitted.borrow_mut().push(request));
            window("/home/ada/engine", view, Some(submit), None)
        }
    };
    let (mut test, view) = TestingRunner::new(
        app,
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || View {
                rows: State::create(History::new()),
                progress: State::create(Progress::opening()),
                selected: State::create(None),
                fetch: State::create(FetchStatus::Idle),
                prompt: State::create(None),
                remotes: State::create(Vec::new()),
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
                held_keys: State::create(HeldKeys::default()),
                refreshed: State::create(RefreshState::default()),
                repository: State::create(Some("engine".to_owned())),
                sidebar: SidebarView::created(),
                local: crate::local_changes_state::LocalChangesView::created(),
                writes: State::create(crate::local_writes::LocalWrites::default()),
                confirming: State::create(None),
            })
        },
        1.,
    );
    test.sync_and_update();
    (test, view, submitted)
}

fn apply(test: &mut TestingRunner, view: View, submitted: &Submitted, update: Update) {
    let submit = {
        let submitted = Rc::clone(submitted);
        move |request| submitted.borrow_mut().push(request)
    };
    test.run_in(|| {
        session::apply(
            update,
            view,
            &Worker {
                submit: &submit,
                refuse: &|_| {},
                closing: false,
            },
        );
    });
    for _ in 0..3 {
        test.sync_and_update();
    }
}

fn changed(
    path: &str,
    staged: Option<StagedChange>,
    unstaged: Option<UnstagedChange>,
) -> StatusEntry {
    StatusEntry::Changed(ChangedEntry {
        path: RepoPath::from(path),
        staged,
        unstaged,
        submodule: None,
    })
}

fn conflicted(path: &str) -> StatusEntry {
    StatusEntry::Conflicted(ConflictedEntry {
        path: RepoPath::from(path),
        kind: ConflictKind::BothModified,
        submodule: None,
    })
}

fn untracked(path: &str) -> StatusEntry {
    StatusEntry::Untracked(RepoPath::from(path))
}

fn status(entries: Vec<StatusEntry>) -> Update {
    Update::Status {
        changes: Arc::new(LocalChanges::new(WorkingTreeStatus::Listed(entries))),
    }
}

/// A status's diff of `path`: one added line reading `line`.
fn diff_of(path: &str, line: &str, context: Context) -> Box<ShownDiff> {
    Box::new(ShownDiff::new(
        FileDiff {
            file: ChangedFile {
                status: ChangeStatus::Modified,
                old_path: RepoPath::from(path),
                new_path: RepoPath::from(path),
                old_mode: None,
                new_mode: None,
                old_id: None,
                new_id: None,
            },
            content: DiffContent::Text {
                text: TextDiff::new(
                    Vec::new(),
                    vec![DiffLine::terminated(line.as_bytes().to_vec())],
                    vec![ChangedRange::new(LineSpan::at(0, 0), LineSpan::at(0, 1))],
                ),
                overlay: DisplayOverlay::default(),
            },
        },
        context,
    ))
}

fn answer(
    test: &mut TestingRunner,
    view: View,
    submitted: &Submitted,
    query: &FileQuery,
    line: &str,
) {
    apply(
        test,
        view,
        submitted,
        Update::FileDiff {
            query: query.clone(),
            diff: Some(diff_of(&path_of(query), line, query.options.context)),
        },
    );
}

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

/// The diff rows drawn: every paragraph's text.
fn paragraphs(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| {
        Paragraph::try_downcast(element).map(|paragraph| {
            paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect()
        })
    })
}

fn path_of(query: &FileQuery) -> String {
    match &query.target {
        FileTarget::WorkingTree { path, .. } => path.display().into_owned(),
        FileTarget::Committed { file, .. } => file.new_path.display().into_owned(),
    }
}

fn side_of(query: &FileQuery) -> Option<WorkingSide> {
    match &query.target {
        FileTarget::WorkingTree { side, .. } => Some(*side),
        FileTarget::Committed { .. } => None,
    }
}

/// The working-tree diffs asked, oldest first.
fn asked(submitted: &Submitted) -> Vec<FileQuery> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::FileDiff(query) => Some(query.clone()),
            _ => None,
        })
        .collect()
}

/// Presses the sidebar's Local Changes entry, whatever its count.
fn open_local_changes(test: &mut TestingRunner) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text.starts_with("Local Changes"))
                .filter(|_| node.layout().area.min_x() < SIDEBAR_WIDTH)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("the sidebar draws no Local Changes: {:?}", labels(test)));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    for _ in 0..3 {
        test.sync_and_update();
    }
}

/// Presses the row of the list reading `text`, the `nth` drawn from the top.
fn press_row(test: &mut TestingRunner, text: &str, nth: usize) {
    let mut found: Vec<(f32, f32)> = test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .filter(|_| node.layout().area.min_x() > SIDEBAR_WIDTH)
            .map(|_| {
                let area = node.layout().area;
                (area.center().y, area.min_x())
            })
    });
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (y, x) = *found
        .get(nth)
        .unwrap_or_else(|| panic!("no row {nth} reads {text}: {:?}", labels(test)));
    test.click_cursor((f64::from(x) + 4., f64::from(y)));
    for _ in 0..3 {
        test.sync_and_update();
    }
}

fn every_kind() -> Vec<StatusEntry> {
    vec![
        changed(
            "both.rs",
            Some(StagedChange::Modified),
            Some(UnstagedChange::Modified),
        ),
        changed("added.rs", Some(StagedChange::Added), None),
        conflicted("clash.rs"),
        untracked("new/one.rs"),
        untracked("new/two.rs"),
    ]
}

/// C9, R9.2, R9.3: the count beside Local Changes is the distinct paths status lists; pressed,
/// the view draws both lists; the first path — Unstaged's first row — is chosen and its diff
/// asked through the working-tree query as its list says, and its answer drawn; an answer for
/// any other path or side is not. Caught by: no path chosen as the view opens, a query asked
/// as the wrong side, or an answer drawn that the query chosen does not name.
#[test]
fn local_changes_draws_its_lists_and_the_first_paths_diff_from_the_working_tree_query() {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(every_kind()));
    assert!(
        labels(&test).contains(&"Local Changes (5)".to_owned()),
        "{:?}",
        labels(&test)
    );
    assert!(
        asked(&submitted).is_empty(),
        "a diff asked with the view hidden"
    );
    open_local_changes(&mut test);
    assert_eq!(*view.sidebar.main.read(), MainView::LocalChanges);
    let drawn = labels(&test);
    for text in [
        "Unstaged",
        "Staged",
        "both.rs",
        "added.rs",
        "clash.rs",
        "new/one.rs",
        "new/two.rs",
    ] {
        assert!(
            drawn.contains(&text.to_owned()),
            "{text} not drawn: {drawn:?}"
        );
    }
    let first = asked(&submitted);
    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(
        (path_of(&first[0]), side_of(&first[0])),
        ("both.rs".to_owned(), Some(WorkingSide::Unstaged)),
        "the first path is not Unstaged's first row, asked as unstaged"
    );
    assert_eq!(
        first[0].options,
        crate::diff_actions::options(DiffSettings::default())
    );

    let mut staged = first[0].clone();
    staged.target = FileTarget::WorkingTree {
        path: RepoPath::from("both.rs"),
        side: WorkingSide::Staged,
    };
    answer(&mut test, view, &submitted, &staged, "STAGED LINE");
    assert!(!paragraphs(&test).iter().any(|t| t.contains("STAGED LINE")));
    answer(&mut test, view, &submitted, &first[0], "UNSTAGED LINE");
    assert!(
        paragraphs(&test)
            .iter()
            .any(|t| t.contains("UNSTAGED LINE")),
        "{:?}",
        paragraphs(&test)
    );

    // An untracked path is asked as untracked.
    press_row(&mut test, "new/two.rs", 0);
    let last = asked(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    assert_eq!(
        (path_of(&last), side_of(&last)),
        ("new/two.rs".to_owned(), Some(WorkingSide::Untracked))
    );
}

/// R9.4: a conflicted path draws its notice in place of a diff and asks nothing. Caught by: a
/// query asked for a conflict, or a reading notice left up for good.
#[test]
fn a_conflicted_path_draws_its_notice_and_asks_nothing() {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(every_kind()));
    open_local_changes(&mut test);
    let before = asked(&submitted).len();
    press_row(&mut test, "clash.rs", 0);
    assert_eq!(
        asked(&submitted).len(),
        before,
        "a conflict asked for a diff"
    );
    assert!(
        labels(&test).contains(&CONFLICTED.to_owned()),
        "{:?}",
        labels(&test)
    );
}

/// The QA brief: a path both staged and unstaged shows, chosen in each list, that list's diff;
/// an answer for the list chosen before, arriving late, is not drawn. Caught by: the two rows
/// asking one query, or answers compared by path (the unstaged diff drawn under the staged
/// row).
#[test]
fn a_path_in_both_lists_draws_each_lists_diff_and_the_answers_cannot_cross() {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(every_kind()));
    open_local_changes(&mut test);
    let unstaged = asked(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    assert_eq!(side_of(&unstaged), Some(WorkingSide::Unstaged));
    answer(&mut test, view, &submitted, &unstaged, "UNSTAGED LINE");

    // both.rs again, in Staged: the second row reading it.
    press_row(&mut test, "both.rs", 1);
    let staged = asked(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    assert_eq!(
        (path_of(&staged), side_of(&staged)),
        ("both.rs".to_owned(), Some(WorkingSide::Staged))
    );
    assert!(
        !paragraphs(&test)
            .iter()
            .any(|t| t.contains("UNSTAGED LINE")),
        "the unstaged diff is drawn under the staged row"
    );
    answer(&mut test, view, &submitted, &unstaged, "UNSTAGED LINE");
    assert!(
        !paragraphs(&test)
            .iter()
            .any(|t| t.contains("UNSTAGED LINE")),
        "a late answer for the other list was drawn"
    );
    answer(&mut test, view, &submitted, &staged, "STAGED LINE");
    assert!(paragraphs(&test).iter().any(|t| t.contains("STAGED LINE")));
}

/// The QA brief: a refresh whose status no longer lists the path chosen draws no diff under no
/// row — the first path is chosen in its place and asked — and a late answer for the path
/// gone is not drawn; a refresh that still lists it asks it again, its diff drawn meanwhile.
/// Caught by: the gone path's diff left drawn, or its late answer kept.
#[test]
fn a_refresh_that_removes_the_chosen_path_draws_no_stale_diff_under_no_row() {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(every_kind()));
    open_local_changes(&mut test);
    press_row(&mut test, "new/one.rs", 0);
    let chosen = asked(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    assert_eq!(path_of(&chosen), "new/one.rs");
    answer(&mut test, view, &submitted, &chosen, "ONE LINE");
    assert!(paragraphs(&test).iter().any(|t| t.contains("ONE LINE")));

    // Still listed: asked again, its diff drawn until the new one comes.
    let before = asked(&submitted).len();
    apply(&mut test, view, &submitted, status(every_kind()));
    let again = asked(&submitted);
    assert_eq!(again.len(), before + 1, "the path was not asked again");
    assert_eq!(again.last(), Some(&chosen));
    assert!(
        paragraphs(&test).iter().any(|t| t.contains("ONE LINE")),
        "the refresh blanked the diff of a path still listed"
    );

    // Gone: the first path chosen in its place.
    let mut without = every_kind();
    without.retain(|entry| entry.path().as_bytes() != b"new/one.rs");
    apply(&mut test, view, &submitted, status(without));
    assert!(!labels(&test).contains(&"new/one.rs".to_owned()));
    assert!(
        !paragraphs(&test).iter().any(|t| t.contains("ONE LINE")),
        "the diff of a path no longer listed is drawn under no row"
    );
    let first = asked(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    assert_eq!(path_of(&first), "both.rs");
    answer(&mut test, view, &submitted, &chosen, "ONE LINE");
    assert!(
        !paragraphs(&test).iter().any(|t| t.contains("ONE LINE")),
        "a late answer for the path gone was drawn"
    );

    // Nothing listed: no path chosen, and the view says so.
    apply(&mut test, view, &submitted, status(Vec::new()));
    assert!(
        labels(&test).contains(&crate::local_changes_pane::NO_LOCAL_CHANGES.to_owned()),
        "{:?}",
        labels(&test)
    );
}

/// R9's filter: typing asks a worker with the window's own lists, shared; until the answer the
/// lists say they are filtering; the answer draws the rows it names and no others. Caught by:
/// filtering on the UI thread (rows dropped before an answer), or copied lists.
#[test]
fn typing_in_the_filter_asks_a_worker_and_the_lists_draw_its_answer() {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(every_kind()));
    open_local_changes(&mut test);
    let mut text = view.local.filter_text;
    test.run_in(|| text.set("new".to_owned()));
    for _ in 0..3 {
        test.sync_and_update();
    }
    let (lists, asked_text) = submitted
        .borrow()
        .iter()
        .rev()
        .find_map(|request| match request {
            Request::FilterLocalChanges { changes, text } => {
                Some((Arc::clone(changes), text.clone()))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("the filter was not asked of a worker"));
    assert_eq!(asked_text, "new");
    let held = view
        .refreshed
        .read()
        .local_changes()
        .cloned()
        .unwrap_or_else(|| panic!("no status kept"));
    assert!(Arc::ptr_eq(&lists, &held), "the lists were copied");
    assert!(labels(&test).contains(&cairn_ui::FILTERING.to_owned()));

    let rows = lists
        .matching("new", || true)
        .unwrap_or_else(|| unreachable!("never told to stop"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::FilteredLocalChanges {
            changes: lists,
            text: "new".to_owned(),
            rows,
        },
    );
    let drawn = labels(&test);
    assert!(drawn.contains(&"new/one.rs".to_owned()) && drawn.contains(&"new/two.rs".to_owned()));
    assert!(
        !drawn.contains(&"clash.rs".to_owned()) && !drawn.contains(&"added.rs".to_owned()),
        "{drawn:?}"
    );
    assert!(
        drawn.contains(&"Showing 2 of 5 files".to_owned()),
        "{drawn:?}"
    );
}

/// The user's decision (2026-10-07): under Local Changes' filter, "Showing N of M files" counts
/// distinct paths, as the sidebar's "Local Changes (N)" does — a path with a staged and an
/// unstaged change, in both lists, is one file — through the real filter pass a worker runs and
/// the window keeping its answer. Caught by: counting rows ("Showing 2 of 6 files"), or
/// counting paths on one side only.
#[test]
fn the_filters_count_is_the_sidebars_distinct_paths_a_path_in_both_lists_once() {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(every_kind()));
    open_local_changes(&mut test);
    assert!(labels(&test).contains(&"Local Changes (5)".to_owned()));
    let mut text = view.local.filter_text;
    test.run_in(|| text.set("both".to_owned()));
    for _ in 0..3 {
        test.sync_and_update();
    }
    let lists = view
        .refreshed
        .read()
        .local_changes()
        .cloned()
        .unwrap_or_else(|| panic!("no status kept"));
    let rows = lists
        .matching("both", || true)
        .unwrap_or_else(|| unreachable!("never told to stop"));
    assert_eq!(
        rows.unstaged.len() + rows.staged.len(),
        2,
        "both.rs in both lists"
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::FilteredLocalChanges {
            changes: lists,
            text: "both".to_owned(),
            rows,
        },
    );
    let drawn = labels(&test);
    assert_eq!(
        drawn.iter().filter(|text| *text == "both.rs").count(),
        2,
        "{drawn:?}"
    );
    assert!(
        drawn.contains(&"Showing 1 of 5 files".to_owned()),
        "{drawn:?}"
    );
}

/// A diff of `path` with a change every ten of its `lines` lines.
fn many_changes(path: &str, lines: u32, context: Context) -> Box<ShownDiff> {
    let old: Vec<DiffLine> = (0..lines)
        .map(|k| DiffLine::terminated(format!("line {k}")))
        .collect();
    let new: Vec<DiffLine> = (0..lines)
        .map(|k| {
            DiffLine::terminated(if k % 10 == 5 {
                format!("LINE {k}")
            } else {
                format!("line {k}")
            })
        })
        .collect();
    let changes = (0..lines / 10)
        .map(|k| ChangedRange::new(LineSpan::at(k * 10 + 5, 1), LineSpan::at(k * 10 + 5, 1)))
        .collect();
    let mut shown = diff_of(path, "", context);
    let file = shown.diff().file.clone();
    *shown = ShownDiff::new(
        FileDiff {
            file,
            content: DiffContent::Text {
                text: TextDiff::new(old, new, changes),
                overlay: DisplayOverlay::none(),
            },
        },
        context,
    );
    shown
}

/// R6.2 in Local Changes: next and previous change move its diff — its own scroll, not the
/// Changes tab's — heard from inside the view, as the detail pane hears them. Caught by: a
/// chord unheard with the detail pane gone, or one that moves the Changes tab's diff.
#[test]
fn next_change_moves_local_changes_own_diff() {
    use cairn_ui::accelerators::Action;

    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(every_kind()));
    open_local_changes(&mut test);
    let query = asked(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::FileDiff {
            query: query.clone(),
            diff: Some(many_changes("both.rs", 200, query.options.context)),
        },
    );
    // Inside the diff, right of the lists, under the bar.
    test.click_cursor((f64::from(WIDTH) - 200., 300.));
    test.sync_and_update();
    for _ in 0..3 {
        crate::window::tests::press_chord(&mut test, Action::NextChange);
    }
    let (_, local_y): (i32, i32) = view.local.scroll.into();
    let (_, changes_y): (i32, i32) = view.diff_scroll.into();
    assert!(local_y < 0, "next change did not move Local Changes' diff");
    assert_eq!(changes_y, 0, "the Changes tab's diff moved");
    assert_eq!(
        view.local
            .cursor
            .read()
            .as_ref()
            .map(|cursor| cursor.change),
        Some(2)
    );
}

/// The handed-over `RefreshState::failure(Refreshed::Status)`: a status that could not be read
/// is said — in place of the lists before any was read, over the lists kept after. Caught by:
/// a failure said nowhere ("Reading…" for good), or the lists kept thrown away.
#[test]
fn a_status_that_could_not_be_read_is_said() {
    use crate::local_changes_pane::status_failure;
    use crate::worker::Refreshed;

    let (mut test, view, submitted) = launch();
    let failed = || Update::RefreshFailed {
        what: Refreshed::Status,
        message: "git status failed".to_owned(),
    };
    apply(&mut test, view, &submitted, failed());
    open_local_changes(&mut test);
    let said = status_failure("git status failed");
    assert!(labels(&test).contains(&said), "{:?}", labels(&test));

    apply(&mut test, view, &submitted, status(every_kind()));
    assert!(
        !labels(&test).contains(&said),
        "a failure said over a status read since"
    );
    apply(&mut test, view, &submitted, failed());
    let drawn = labels(&test);
    assert!(drawn.contains(&said), "{drawn:?}");
    assert!(
        drawn.contains(&"clash.rs".to_owned()),
        "the lists kept were thrown away"
    );
}

/// The rows of the lists drawn highlighted: a list row's rect in the chosen colour.
fn highlighted_rows(test: &TestingRunner) -> usize {
    let chosen = test.run_in(|| get_theme_or_default().read().colors().surface_secondary);
    test.find_many(|node, element| {
        let area = node.layout().area;
        Rect::try_downcast(element)
            .filter(|_| {
                area.min_x() > SIDEBAR_WIDTH && area.height() == cairn_ui::DETAIL_ROW_HEIGHT
            })
            .filter(|rect| rect.style.background.as_color() == Some(chosen))
            .map(|_| ())
    })
    .len()
}

/// Phase 09 QA's RR-note, the Changes tab's rule: a filter that hides the path chosen leaves it
/// chosen — its diff drawn, no row highlighted — and a refresh asks it again; only a status
/// that no longer lists it chooses another. Caught by: the path let go of, or another chosen,
/// when the filter hides it; or the hidden path not asked again on a refresh.
#[test]
fn a_filter_hiding_the_path_chosen_keeps_it_chosen_and_drawn() {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(every_kind()));
    open_local_changes(&mut test);
    let chosen = asked(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    assert_eq!(path_of(&chosen), "both.rs");
    answer(&mut test, view, &submitted, &chosen, "UNSTAGED LINE");
    assert_eq!(
        highlighted_rows(&test),
        1,
        "the path chosen is not highlighted"
    );

    let mut text = view.local.filter_text;
    test.run_in(|| text.set("new".to_owned()));
    for _ in 0..3 {
        test.sync_and_update();
    }
    let filter = |submitted: &Submitted| {
        submitted
            .borrow()
            .iter()
            .rev()
            .find_map(|request| match request {
                Request::FilterLocalChanges { changes, text } => {
                    Some((Arc::clone(changes), text.clone()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("the filter was not asked"))
    };
    let (lists, typed) = filter(&submitted);
    let rows = lists
        .matching(&typed, || true)
        .unwrap_or_else(|| unreachable!("never told to stop"));
    let before = asked(&submitted).len();
    apply(
        &mut test,
        view,
        &submitted,
        Update::FilteredLocalChanges {
            changes: lists,
            text: typed,
            rows,
        },
    );
    assert!(
        !labels(&test).contains(&"both.rs".to_owned()),
        "the filter left both.rs"
    );
    assert_eq!(asked(&submitted).len(), before, "another path was chosen");
    assert!(
        paragraphs(&test)
            .iter()
            .any(|t| t.contains("UNSTAGED LINE")),
        "the diff of the path chosen was let go of"
    );
    assert_eq!(
        highlighted_rows(&test),
        0,
        "a row is highlighted for a path not shown"
    );

    // A refresh, the filter on: the new lists' rows asked, then the hidden path asked again.
    apply(&mut test, view, &submitted, status(every_kind()));
    let (lists, typed) = filter(&submitted);
    let rows = lists
        .matching(&typed, || true)
        .unwrap_or_else(|| unreachable!("never told to stop"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::FilteredLocalChanges {
            changes: lists,
            text: typed,
            rows,
        },
    );
    assert_eq!(
        asked(&submitted).pop(),
        Some(chosen),
        "the hidden path was not asked again"
    );
    assert!(
        paragraphs(&test)
            .iter()
            .any(|t| t.contains("UNSTAGED LINE"))
    );
}
