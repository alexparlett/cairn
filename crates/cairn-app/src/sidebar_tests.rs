//! The sidebar in the window, headless (refs-and-status R8, criterion C8): pressing a ref
//! selects its row and brings it into view, or finds it by paging the walk — saying so while
//! it looks — and the next press or a scroll supersedes the find; a tag on a tree and a stash
//! with no row say why; the filter is asked of a worker and its answer drawn; Local Changes and
//! All Commits switch the main region. Updates are applied through `session::apply`, as the
//! worker's stream applies them.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use cairn_model::{
    GraphRow, HeadState, History, Lane, Oid, PagedCommit, Ref, RefKind, RefName, RefTarget,
    RefsSnapshot, RemoteSummary, RowId, RowsPage, SidebarRow, StashEntry,
};
use cairn_ui::accelerators::HeldKeys;
use cairn_ui::{
    DetailTab, DiffSettings, LOCAL_CHANGES_CAPTION, MainView, ROW_HEIGHT,
    SIDEBAR_FILTER_PLACEHOLDER,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

use crate::diff_state::DiffState;
use crate::fetch_state::FetchStatus;
use crate::history_state::Progress;
use crate::local_changes_pane::READING_STATUS;
use crate::ref_find::{FIND_PAGE_ROWS, finding_text, no_commit_text, not_in_graph_text};
use crate::refresh_state::RefreshState;
use crate::session::{self, Worker};
use crate::sidebar_state::{SIDEBAR_WIDTH, SidebarView};
use crate::window::{PANE_HEIGHT, View, window};
use crate::worker::{Comparison, Request, Update};

const WIDTH: f32 = 1000.;
const HEIGHT: f32 = 600.;

fn oid(n: usize) -> Oid {
    let mut bytes = [0u8; 20];
    bytes[0] = 0x10 + (n % 200) as u8;
    bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
    Oid::from_bytes(&bytes).unwrap_or_else(|error| panic!("{error}"))
}

/// Rows `range` as one page: row `n` is commit `oid(n)`, `commit n`; the rows the snapshot's
/// refs point at are labelled by them, as a walk from that snapshot labels them, row 0 `HEAD`'s.
fn page(range: std::ops::Range<usize>) -> RowsPage {
    let mut page = RowsPage::new();
    for n in range {
        let subject = format!("commit {n}");
        let name = match n {
            0 => Some("refs/heads/main"),
            5 => Some("refs/heads/near"),
            150 => Some("refs/heads/far"),
            180 => Some("refs/heads/farther"),
            _ => None,
        };
        let labels: Vec<cairn_model::Label<'_>> = name
            .map(|name| cairn_model::Label {
                name,
                kind: RefKind::LocalBranch,
                current: n == 0,
            })
            .into_iter()
            .collect();
        page.push_labelled(
            GraphRow::new(oid(n), Lane::new(0), Vec::new()),
            PagedCommit {
                parents: 1,
                subject: &subject,
                author: "Ada",
                author_time: 0,
            },
            n == 0,
            &labels,
        );
    }
    page
}

fn listed(name: &str, target: RefTarget) -> Ref {
    Ref {
        name: RefName::new(name),
        kind: if name.starts_with("refs/heads/") {
            RefKind::LocalBranch
        } else {
            RefKind::Tag
        },
        target,
        symbolic: None,
        upstream: None,
    }
}

/// `main` at row 0, `near` at row 5, `far` at row 150, `farther` at row 180, `on-tree` a tag
/// naming a tree, and a stash whose commit is no row's.
fn snapshot() -> Arc<RefsSnapshot> {
    let mut refs = vec![
        listed("refs/heads/main", RefTarget::Commit(oid(0))),
        listed("refs/heads/near", RefTarget::Commit(oid(5))),
        listed("refs/heads/far", RefTarget::Commit(oid(150))),
        listed("refs/heads/farther", RefTarget::Commit(oid(180))),
        listed(
            "refs/tags/on-tree",
            RefTarget::Tag {
                object: oid(900),
                commit: None,
            },
        ),
    ];
    refs.sort_by(|a, b| a.name.cmp(&b.name));
    Arc::new(RefsSnapshot {
        refs,
        head: HeadState::Branch(RefName::new("refs/heads/main")),
        stashes: vec![StashEntry {
            index: 0,
            message: "On main: lost".to_owned(),
            commit: oid(950),
            base: oid(951),
        }],
        unreadable: 0,
    })
}

type Submitted = Rc<RefCell<Vec<Request>>>;

/// The window over `loaded` rows, the last page `complete` or not, with the snapshot's
/// sidebar rows answered.
fn launch(loaded: usize, complete: bool) -> (TestingRunner, View, Submitted) {
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
            runner.provide_root_context(move || {
                let mut history = History::new();
                history
                    .append(page(0..loaded))
                    .unwrap_or_else(|full| panic!("{full}"));
                let mut progress = Progress::opening();
                progress.received(1, complete, loaded);
                View {
                    rows: State::create(history),
                    progress: State::create(progress),
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
                    held_keys: State::create(HeldKeys::default()),
                    refreshed: State::create(RefreshState::default()),
                    repository: State::create(Some("engine".to_owned())),
                    sidebar: SidebarView::created(),
                    local: crate::local_changes_state::LocalChangesView::created(),
                    writes: State::create(crate::local_writes::LocalWrites::default()),
                    confirming: State::create(None),
                    show_lost: State::create(false),
                    activity: State::create(crate::activity::ActivityLog::default()),
                    branch: crate::create_branch::CreateBranchView::created(),
                }
            })
        },
        1.,
    );
    let refs = snapshot();
    apply(
        &test,
        view,
        &submitted,
        Update::Refs {
            snapshot: Arc::clone(&refs),
            reopen: false,
        },
    );
    let rows = answer(&submitted);
    apply(
        &test,
        view,
        &submitted,
        Update::FilteredRefs {
            refs,
            text: String::new(),
            rows,
        },
    );
    submitted.borrow_mut().clear();
    for _ in 0..3 {
        test.sync_and_update();
    }
    (test, view, submitted)
}

/// What a worker answers the last `FilterRefs` asked.
fn answer(submitted: &Submitted) -> Vec<SidebarRow> {
    let asked = submitted
        .borrow()
        .iter()
        .rev()
        .find_map(|request| match request {
            Request::FilterRefs {
                refs,
                text,
                disclosure,
            } => Some((Arc::clone(refs), text.clone(), Arc::clone(disclosure))),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the sidebar's rows were never asked"));
    asked
        .0
        .sidebar_rows(&asked.1, &asked.2, || true)
        .unwrap_or_else(|| panic!("laying out the rows stopped"))
}

/// `update` applied as the worker's stream applies it.
fn apply(test: &TestingRunner, view: View, submitted: &Submitted, update: Update) {
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
}

fn rows_arrive(
    test: &mut TestingRunner,
    view: View,
    submitted: &Submitted,
    range: std::ops::Range<usize>,
    complete: bool,
) {
    apply(
        test,
        view,
        submitted,
        Update::Rows {
            rows: page(range),
            complete,
        },
    );
    for _ in 0..2 {
        test.sync_and_update();
    }
}

fn texts(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

/// The labels the sidebar draws, left of the main region.
fn sidebar_texts(test: &TestingRunner) -> Vec<String> {
    test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|_| node.layout().area.min_x() < SIDEBAR_WIDTH)
            .map(|l| l.text.to_string())
    })
}

/// Presses the sidebar's label reading `caption`.
fn press(test: &mut TestingRunner, caption: &str) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == caption)
                .filter(|_| node.layout().area.min_x() < SIDEBAR_WIDTH)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("the sidebar draws no {caption:?}: {:?}", texts(test)));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    for _ in 0..2 {
        test.sync_and_update();
    }
}

/// Whether the history draws `commit n` inside the window.
fn row_shown(test: &TestingRunner, n: usize) -> bool {
    let subject = format!("commit {n}");
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == subject)
            .map(|_| node.layout().area)
    })
    .is_some_and(|area| area.min_y() >= 0. && area.max_y() <= HEIGHT)
}

fn finds(submitted: &Submitted) -> Vec<Oid> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::FindRow { target, rows } => {
                assert_eq!(*rows, FIND_PAGE_ROWS);
                Some(*target)
            }
            _ => None,
        })
        .collect()
}

fn stops(submitted: &Submitted) -> usize {
    submitted
        .borrow()
        .iter()
        .filter(|request| matches!(request, Request::StopFinding))
        .count()
}

fn changes_asked(submitted: &Submitted) -> Vec<Comparison> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Changes { of } => Some(*of),
            _ => None,
        })
        .collect()
}

/// R8.5: a ref whose row is loaded is selected, its changes asked, its row brought into view,
/// and no page walked. Caught by: a press that selects nothing, scrolls nowhere, or asks the
/// worker to find what is already loaded.
#[test]
fn pressing_a_loaded_ref_selects_its_row_and_brings_it_into_view() {
    let (mut test, view, submitted) = launch(200, false);
    press(&mut test, "far");
    assert_eq!(*view.selected.read(), Some(RowId::Commit(oid(150))));
    assert_eq!(changes_asked(&submitted), [Comparison::Commit(oid(150))]);
    assert!(finds(&submitted).is_empty(), "a loaded row was looked for");
    assert!(row_shown(&test, 150), "the row was not brought into view");
    // Told to the list, so its next arrow key starts at the row without a search (RR1).
    assert_eq!(*view.history_cursor.read(), 150);
}

/// R8.5, R8.6: a ref past the loaded rows is found by paging the walk — "Finding <ref>…" said
/// while it looks — and selected and brought into view when the page holding it arrives,
/// the find stopped then; the pages before it select nothing. Caught by: a press that does
/// nothing past the loaded rows (Fork's), a find never stopped, or a row looked for only in
/// the first page.
#[test]
fn pressing_a_ref_past_the_loaded_rows_finds_it_by_paging() {
    let (mut test, view, submitted) = launch(10, false);
    press(&mut test, "far");
    assert_eq!(finds(&submitted), [oid(150)]);
    assert!(
        texts(&test).contains(&finding_text("far")),
        "{:?}",
        texts(&test)
    );
    assert_eq!(*view.selected.read(), None);

    rows_arrive(&mut test, view, &submitted, 10..100, false);
    assert_eq!(
        *view.selected.read(),
        None,
        "a page without the row selected something"
    );
    assert_eq!(stops(&submitted), 0);
    rows_arrive(&mut test, view, &submitted, 100..200, false);
    assert_eq!(*view.selected.read(), Some(RowId::Commit(oid(150))));
    assert_eq!(stops(&submitted), 1, "the find was not stopped once found");
    assert!(
        !texts(&test).contains(&finding_text("far")),
        "still finding"
    );
    assert!(
        row_shown(&test, 150),
        "the found row was not brought into view"
    );
    assert_eq!(*view.history_cursor.read(), 150, "the list was not told");
}

/// The QA brief: a press during a find supersedes it, and two quick presses draw only the
/// second — the first's row, arriving first, is passed over. Caught by: a find that selects
/// whatever it was first asked for, or both in turn.
#[test]
fn two_quick_presses_draw_only_the_second() {
    let (mut test, view, submitted) = launch(10, false);
    press(&mut test, "far");
    press(&mut test, "farther");
    assert_eq!(finds(&submitted), [oid(150), oid(180)]);
    assert!(texts(&test).contains(&finding_text("farther")));

    rows_arrive(&mut test, view, &submitted, 10..170, false);
    assert_eq!(
        *view.selected.read(),
        None,
        "the superseded find's row was selected"
    );
    rows_arrive(&mut test, view, &submitted, 170..250, false);
    assert_eq!(*view.selected.read(), Some(RowId::Commit(oid(180))));
    assert_eq!(changes_asked(&submitted), [Comparison::Commit(oid(180))]);
}

/// R8.5: a scroll of the list supersedes a find — stopped, its notice gone — and the row
/// arriving after selects nothing. Caught by: a find a scroll leaves running, which jumps the
/// list away from where the user scrolled.
#[test]
fn a_scroll_of_the_list_supersedes_a_find() {
    let (mut test, view, submitted) = launch(60, false);
    press(&mut test, "far");
    assert_eq!(finds(&submitted), [oid(150)]);
    test.scroll(
        (f64::from(SIDEBAR_WIDTH) + 200., 150.),
        (0., -10. * f64::from(ROW_HEIGHT)),
    );
    for _ in 0..2 {
        test.sync_and_update();
    }
    assert_eq!(stops(&submitted), 1, "the scroll did not stop the find");
    assert!(!texts(&test).contains(&finding_text("far")));
    rows_arrive(&mut test, view, &submitted, 60..200, false);
    assert_eq!(
        *view.selected.read(),
        None,
        "a stopped find selected its row"
    );
}

/// R8.5: a row chosen in the list during a find supersedes it, and the entry pressed is let
/// go of. Caught by: a find that selects its row over the user's choice.
#[test]
fn a_row_chosen_during_a_find_supersedes_it() {
    let (mut test, view, submitted) = launch(20, false);
    press(&mut test, "far");
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == "commit 3")
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("commit 3 is not drawn"));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.sync_and_update();
    assert_eq!(stops(&submitted), 1);
    rows_arrive(&mut test, view, &submitted, 20..200, false);
    assert_eq!(*view.selected.read(), Some(RowId::Commit(oid(3))));
}

/// R8.5: a ref naming no commit — a tag on a tree — says so at once and walks nothing.
/// Caught by: a find for a commit that is not there, which pages the whole history.
#[test]
fn a_tag_on_a_tree_says_it_is_not_in_the_graph() {
    let (mut test, view, submitted) = launch(10, false);
    press(&mut test, "on-tree");
    assert!(finds(&submitted).is_empty());
    assert!(
        texts(&test).contains(&no_commit_text("on-tree")),
        "{:?}",
        texts(&test)
    );
    assert_eq!(*view.selected.read(), None);
}

/// R8.5, R4.2: a stash whose base no ref reaches is found nowhere — the find pages to the
/// walk's end — and then its changes are shown, with no row selected, and the sidebar says it
/// is not in the graph; with the history complete already it says so at once. Caught by: a
/// stash press that shows nothing, or a row drawn selected for it.
#[test]
fn a_stash_with_no_row_shows_its_changes_and_says_it_is_not_in_the_graph() {
    let (mut test, view, submitted) = launch(10, false);
    press(&mut test, "On main: lost");
    assert_eq!(finds(&submitted), [oid(950)]);
    rows_arrive(&mut test, view, &submitted, 10..30, true);
    assert!(
        texts(&test).contains(&not_in_graph_text("stash@{0}")),
        "{:?}",
        texts(&test)
    );
    assert_eq!(changes_asked(&submitted), [Comparison::Stash(oid(950))]);
    assert_eq!(view.rows.read().position(RowId::Stash(oid(950))), None);

    let (mut test, _, submitted) = launch(10, true);
    press(&mut test, "On main: lost");
    assert!(finds(&submitted).is_empty(), "a complete history was paged");
    assert!(texts(&test).contains(&not_in_graph_text("stash@{0}")));
    assert_eq!(changes_asked(&submitted), [Comparison::Stash(oid(950))]);
}

/// R8.3: typing in the sidebar's filter asks a worker for the rows (never laid out here), and
/// the answer drawn is the worker's. Caught by: a filter run on the UI thread, or an answer
/// that is not drawn.
#[test]
fn typing_in_the_sidebars_filter_asks_a_worker_and_draws_its_answer() {
    let (mut test, view, submitted) = launch(10, false);
    let field = test
        .find(|node, element| {
            Paragraph::try_downcast(element)
                .filter(|paragraph| {
                    paragraph
                        .spans
                        .iter()
                        .any(|span| span.text.as_ref() == SIDEBAR_FILTER_PLACEHOLDER)
                })
                .filter(|_| node.layout().area.min_x() < SIDEBAR_WIDTH)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no filter field in the sidebar"));
    test.click_cursor((f64::from(field.x), f64::from(field.y)));
    test.sync_and_update();
    test.write_text("far");
    for _ in 0..3 {
        test.sync_and_update();
    }
    let texts_asked: Vec<String> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::FilterRefs { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts_asked.last().map(String::as_str),
        Some("far"),
        "{texts_asked:?}"
    );
    assert!(
        sidebar_texts(&test).contains(&"near".to_owned()),
        "drawn before the answer"
    );

    let rows = answer(&submitted);
    let refs = view
        .sidebar
        .state
        .peek()
        .shown()
        .map(|shown| Arc::clone(&shown.refs))
        .unwrap_or_else(|| panic!("no rows kept"));
    apply(
        &test,
        view,
        &submitted,
        Update::FilteredRefs {
            refs,
            text: "far".to_owned(),
            rows,
        },
    );
    for _ in 0..2 {
        test.sync_and_update();
    }
    let drawn = sidebar_texts(&test);
    assert!(drawn.contains(&"far".to_owned()) && drawn.contains(&"farther".to_owned()));
    assert!(
        !drawn.contains(&"near".to_owned()),
        "the answer was not drawn: {drawn:?}"
    );
    // The rows replaced went to a worker to free, with their snapshot.
    assert!(
        submitted.borrow().iter().any(|request| matches!(
            request,
            Request::Retire(retired) if retired.sidebar_rows().is_some()
        )),
        "the replaced rows were freed on the UI thread"
    );
}

/// R8.7: Local Changes puts its view in the main region and All Commits the history back; a
/// press of a ref returns to the history. Caught by: entries that switch nothing.
#[test]
fn local_changes_and_all_commits_switch_the_main_region() {
    let (mut test, view, _) = launch(10, false);
    assert!(texts(&test).contains(&"commit 0".to_owned()));
    press(&mut test, LOCAL_CHANGES_CAPTION);
    assert_eq!(*view.sidebar.main.read(), MainView::LocalChanges);
    let drawn = texts(&test);
    assert!(drawn.contains(&READING_STATUS.to_owned()), "{drawn:?}");
    assert!(
        !drawn.contains(&"commit 0".to_owned()),
        "the history is still drawn"
    );
    press(&mut test, "All Commits");
    assert!(texts(&test).contains(&"commit 0".to_owned()));
    press(&mut test, LOCAL_CHANGES_CAPTION);
    press(&mut test, "near");
    assert_eq!(*view.sidebar.main.read(), MainView::AllCommits);
    assert_eq!(*view.selected.read(), Some(RowId::Commit(oid(5))));
}

/// R8.5: while a find looks, the list's end coming into view asks no page of its own — one
/// would supersede the find. Caught by: the list asking for more as a find's short pages
/// arrive, which stops the find after its first page.
#[test]
fn a_find_is_not_superseded_by_the_list_reaching_its_end() {
    let (mut test, view, submitted) = launch(10, false);
    press(&mut test, "far");
    submitted.borrow_mut().clear();
    rows_arrive(&mut test, view, &submitted, 10..14, false);
    let asked: Vec<Request> = submitted
        .borrow()
        .iter()
        .filter(|request| !matches!(request, Request::Retire(_)))
        .cloned()
        .collect();
    assert!(asked.is_empty(), "the list asked {asked:?} during a find");
}

/// R8.5, R10.4: a refresh that reopens the history while a find looks asks the find again of
/// the new walk, after the open, and the new history's pages are looked through from its
/// first row. Caught by: a find lost to a reopen (the press does nothing), or one that looks
/// past the new history's rows by the count the old one had.
#[test]
fn a_reopen_during_a_find_asks_it_again_of_the_new_walk() {
    let (mut test, view, submitted) = launch(10, false);
    press(&mut test, "far");
    submitted.borrow_mut().clear();
    let mut moved = (*snapshot()).clone();
    moved.unreadable = 0;
    moved
        .refs
        .push(listed("refs/heads/zz", RefTarget::Commit(oid(7))));
    apply(
        &test,
        view,
        &submitted,
        Update::Refs {
            snapshot: Arc::new(moved),
            reopen: true,
        },
    );
    let asked: Vec<&'static str> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::OpenHistory { .. } => Some("open"),
            Request::FindRow { target, .. } if *target == oid(150) => Some("find"),
            _ => None,
        })
        .collect();
    assert_eq!(asked, ["open", "find"]);
    // The new walk's first page holds the row.
    rows_arrive(&mut test, view, &submitted, 140..160, false);
    assert_eq!(*view.selected.read(), Some(RowId::Commit(oid(150))));
}
