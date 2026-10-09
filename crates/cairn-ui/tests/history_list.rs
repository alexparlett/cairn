//! Headless component tests for `HistoryList`.

#[path = "../../cairn-model/tests/layout_before_compaction/mod.rs"]
mod layout_before_compaction;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cairn_model::{
    GraphRow, History, Lane, LaneAssigner, Oid, PagedCommit, RowContent, RowEdges, RowId, RowsPage,
};
use cairn_ui::accelerators::{self, Action, Os};
use cairn_ui::{HistoryList, NEW_BRANCH_CAPTION, PREFETCH_ROWS, ROW_HEIGHT, RowRender};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{KeyboardEventName, MouseEventName, PlatformEvent};
use layout_before_compaction::AssignerBeforeCompaction;

const WIDTH: f32 = 600.;
const HEIGHT: f32 = 520.;

fn oid(n: usize) -> Oid {
    let mut bytes = [0u8; 20];
    bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
    Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
}

/// A page of rows `commit n` for each `n` of `range`.
fn page(range: impl IntoIterator<Item = usize>) -> RowsPage {
    let mut page = RowsPage::new();
    for n in range {
        page.push(
            GraphRow::new(oid(n), Lane::new(0), Vec::new()),
            PagedCommit {
                parents: 1,
                subject: &format!("commit {n}"),
                author: "A",
                author_time: 0,
            },
        );
    }
    page
}

fn hold(history: &mut History, page: RowsPage) {
    history.append(page).unwrap_or_else(|full| panic!("{full}"));
}

fn rows(range: impl IntoIterator<Item = usize>) -> History {
    let mut history = History::new();
    hold(&mut history, page(range));
    history
}

/// What the list reported, in order.
#[derive(Clone, Default)]
struct Reports {
    selected: Rc<RefCell<Vec<RowId>>>,
    reached_end: Rc<RefCell<usize>>,
    actions: Rc<RefCell<Vec<Action>>>,
    new_branch: Rc<RefCell<Vec<(Oid, String)>>>,
}

#[derive(Clone)]
struct Fixture {
    rows: State<History>,
    selected: State<Option<RowId>>,
}

/// Each row draws as one label: its subject, prefixed `> ` when the list says it is selected
/// and `~ ` when it says the row is lost.
fn list(reports: Reports) -> impl Fn() -> Element + 'static {
    move || {
        let fixture = use_consume::<Fixture>();
        let mut selected = fixture.selected;
        let on_select = reports.selected.clone();
        let reached_end = reports.reached_end.clone();
        let actions = reports.actions.clone();
        let new_branch = reports.new_branch.clone();

        HistoryList::new(fixture.rows, |render: RowRender| {
            let subject = match render.content {
                RowContent::Commit(commit) => commit.summary,
                RowContent::Stash(stash) => stash.message,
            };
            let marker = if render.selected { "> " } else { "" };
            let lost = if render.lost { "~ " } else { "" };
            label()
                .height(Size::px(ROW_HEIGHT))
                .text(format!("{marker}{lost}{subject}"))
                .into()
        })
        .on_action(move |action| actions.borrow_mut().push(action))
        .on_new_branch(move |at: (Oid, String)| new_branch.borrow_mut().push(at))
        .selected(*selected.read())
        .on_select(move |id: RowId| {
            on_select.borrow_mut().push(id);
            selected.set(Some(id));
        })
        .on_reach_end(move |()| *reached_end.borrow_mut() += 1)
        .into()
    }
}

fn launch(initial: History, reports: &Reports) -> (TestingRunner, Fixture) {
    let listed = list(reports.clone());
    let (mut test, fixture) = TestingRunner::new(
        move || -> Element {
            rect()
                .expanded()
                .child(listed())
                .child(ContextMenuViewer::new())
                .into()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(|| Fixture {
                rows: State::create(initial),
                selected: State::create(None),
            })
        },
        1.,
    );
    test.sync_and_update();
    (test, fixture)
}

/// Every row label in the tree, as `(text, visible)`.
fn built_rows(test: &TestingRunner) -> Vec<(String, bool)> {
    test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text.contains("commit "))
            .map(|label| (label.text.to_string(), node.is_visible()))
    })
}

fn selected_rows(test: &TestingRunner) -> Vec<String> {
    built_rows(test)
        .into_iter()
        .filter_map(|(text, _)| text.strip_prefix("> ").map(str::to_owned))
        .collect()
}

fn press(test: &mut TestingRunner, key: NamedKey) {
    test.press_key(Key::Named(key));
    test.sync_and_update();
}

#[test]
fn only_a_viewport_of_rows_is_built_however_long_the_history() {
    let viewport_rows = (HEIGHT / ROW_HEIGHT).ceil() as usize;

    let within_a_viewport = |test: &TestingRunner, place: &str| {
        let count = built_rows(test).len();
        assert!(
            count >= viewport_rows && count <= viewport_rows + 2,
            "{count} rows were built for a {viewport_rows}-row viewport {place}"
        );
        count
    };

    let mut built = Vec::new();
    for length in [1_000, 100_000] {
        let (mut test, _) = launch(rows(0..length), &Reports::default());
        built.push(within_a_viewport(
            &test,
            &format!("at the top of {length} rows"),
        ));

        let deep = length - 300;
        test.scroll((100., 100.), (0., -(deep as f64 * ROW_HEIGHT as f64)));
        built.push(within_a_viewport(
            &test,
            &format!("at row {deep} of {length}"),
        ));
        assert!(
            built_rows(&test)
                .iter()
                .any(|(text, visible)| *visible && *text == format!("commit {deep}")),
            "scrolling to row {deep} did not build it"
        );
    }
    assert!(
        built.windows(2).all(|pair| pair[0] == pair[1]),
        "a longer history or a deeper scroll built a different number of rows: {built:?}"
    );
}

/// A history of `length` rows, each labelled by `per_row` tags, and the first `crowded` rows
/// by `crowd` local branches more.
fn labelled_rows(length: usize, per_row: usize, crowded: usize, crowd: usize) -> History {
    let mut history = History::new();
    for start in (0..length).step_by(1_000) {
        let mut page = RowsPage::new();
        for n in start..(start + 1_000).min(length) {
            let mut names: Vec<String> = (0..per_row)
                .map(|t| format!("refs/tags/r{n}-t{t}"))
                .collect();
            if n < crowded {
                names.extend((0..crowd).map(|b| format!("refs/heads/r{n}/b{b:05}")));
            }
            names.sort();
            let labels: Vec<cairn_model::Label<'_>> = names
                .iter()
                .map(|name| cairn_model::Label {
                    name,
                    kind: if name.starts_with("refs/heads/") {
                        cairn_model::RefKind::LocalBranch
                    } else {
                        cairn_model::RefKind::Tag
                    },
                    current: false,
                })
                .collect();
            page.push_labelled(
                GraphRow::new(oid(n), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents: 1,
                    subject: &format!("commit {n}"),
                    author: "A",
                    author_time: 0,
                },
                false,
                &labels,
            );
        }
        hold(&mut history, page);
    }
    history
}

/// The viewport twin with labelled rows: the history list still builds one viewport of
/// rows, and a row labelled by thousands of refs lays out only the chips its column holds
/// (R5.3, phase 05 QA RR3). Caught by: chips laid out for every label, or labels making the
/// list build rows past its viewport.
#[test]
fn only_a_viewport_of_labelled_rows_is_built_and_each_lays_out_a_columns_worth_of_chips() {
    let viewport_rows = (HEIGHT / ROW_HEIGHT).ceil() as usize;
    let most_chips = Rc::new(RefCell::new(0usize));
    let history = labelled_rows(100_000, 3, 40, 5_000);
    let counted = most_chips.clone();
    let (mut test, _) = TestingRunner::new(
        move || -> Element {
            let fixture = use_consume::<Fixture>();
            let counted = counted.clone();
            HistoryList::new(fixture.rows, move |render: RowRender| {
                let subject = match render.content {
                    RowContent::Commit(commit) => commit.summary,
                    RowContent::Stash(stash) => stash.message,
                };
                let mut most = counted.borrow_mut();
                *most = (*most).max(render.chips.len());
                label().height(Size::px(ROW_HEIGHT)).text(subject).into()
            })
            .into()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(|| Fixture {
                rows: State::create(history),
                selected: State::create(None),
            })
        },
        1.,
    );
    test.sync_and_update();
    test.sync_and_update();
    for place in ["top", "deep"] {
        if place == "deep" {
            test.scroll((100., 100.), (0., -(90_000. * ROW_HEIGHT as f64)));
        }
        let count = built_rows(&test).len();
        assert!(
            count >= viewport_rows && count <= viewport_rows + 2,
            "{count} rows were built for a {viewport_rows}-row viewport at the {place}"
        );
    }
    let room = cairn_ui::label_room(WIDTH, 1);
    let bound = (room / 10.).ceil() as usize + 1;
    let most = *most_chips.borrow();
    assert!(
        most > 3,
        "no crowded row laid out more than its tags: {most}"
    );
    assert!(
        most <= bound,
        "a row of 5,003 refs laid out {most} chips for {room} px (at most {bound})"
    );
}

/// C13, D5: an arrow held with a chord's modifiers is the accelerator's — "previous change",
/// "next change" — resolved through the table, not the list's "next commit", though the
/// detail pane that hears those chords does not have focus here. Caught by: the list moving
/// on any arrow whatever is held, or asking only the scope it sits in.
#[test]
fn an_accelerators_chord_does_not_move_the_selection() {
    let reports = Reports::default();
    let (mut test, _) = launch(rows(0..100), &reports);
    press(&mut test, NamedKey::ArrowDown);

    for action in [Action::NextChange, Action::PreviousChange] {
        let chord = accelerators::chords(action, Os::current()).first().unwrap();
        let (key, _, held) = chord.key_press().unwrap();
        test.press_key_with_modifiers(key, held);
        test.sync_and_update();
    }
    assert_eq!(
        reports.selected.borrow().as_slice(),
        &[RowId::Commit(oid(0))],
        "a chord moved the commit selection"
    );

    press(&mut test, NamedKey::ArrowDown);
    assert_eq!(
        reports.selected.borrow().last(),
        Some(&RowId::Commit(oid(1))),
        "the plain arrow stopped moving the selection"
    );
}

/// staging-and-commit R7.3: the history's own chord, Show Lost Commits, is reported to the
/// list's caller as its action, and moves no selection; a chord of the detail pane's scope is
/// left alone and reported as nothing. Caught by: the chord left unheard (the phase 06 carry),
/// heard as an arrow, or another scope's chord reported as the history's.
#[test]
fn the_show_lost_commits_chord_is_reported_and_moves_nothing() {
    let reports = Reports::default();
    let (mut test, _) = launch(rows(0..100), &reports);
    press(&mut test, NamedKey::ArrowDown);
    for action in [
        Action::ShowLostCommits,
        Action::NextChange,
        Action::ShowLostCommits,
    ] {
        let chord = accelerators::chords(action, Os::current()).first().unwrap();
        let (key, code, modifiers) = chord.key_press().unwrap();
        test.send_event(PlatformEvent::Keyboard {
            name: KeyboardEventName::KeyDown,
            key,
            code,
            modifiers,
        });
        test.sync_and_update();
    }
    assert_eq!(
        reports.actions.borrow().as_slice(),
        &[Action::ShowLostCommits, Action::ShowLostCommits]
    );
    assert_eq!(
        reports.selected.borrow().as_slice(),
        &[RowId::Commit(oid(0))],
        "a chord moved the commit selection"
    );
}

/// staging-and-commit R11.1: a row of a commit no ref reaches is drawn lost, and only that row,
/// from the history's own flag — and drawn as any other once a later page says a ref reaches
/// it after all. Caught by: the flag not handed to the row, handed to its neighbours, or read
/// from a stale copy.
#[test]
fn a_lost_row_is_drawn_lost_and_no_other() {
    let mut history = History::new();
    let mut first = page(0..2);
    for n in 2..4 {
        first.push_lost(
            GraphRow::new(oid(n), Lane::new(1), Vec::new()),
            PagedCommit {
                parents: 1,
                subject: &format!("commit {n}"),
                author: "A",
                author_time: 0,
            },
        );
    }
    hold(&mut history, first);
    hold(&mut history, page(4..6));
    let reports = Reports::default();
    let (mut test, fixture) = launch(history, &reports);
    let texts = |test: &TestingRunner| -> Vec<String> {
        built_rows(test).into_iter().map(|(text, _)| text).collect()
    };
    assert_eq!(
        texts(&test),
        [
            "commit 0",
            "commit 1",
            "~ commit 2",
            "~ commit 3",
            "commit 4",
            "commit 5"
        ]
    );
    let mut rows = fixture.rows;
    let mut reached = RowsPage::new();
    reached.reached(3, oid(3));
    test.run_in(|| hold(&mut rows.write(), reached));
    test.sync_and_update();
    test.sync_and_update();
    assert_eq!(
        texts(&test),
        [
            "commit 0",
            "commit 1",
            "~ commit 2",
            "commit 3",
            "commit 4",
            "commit 5"
        ]
    );
}

/// QA item 14, the user's decision (2026-10-09): Shift+↓ — Local Changes' list extension,
/// a chord of a scope the history list is not in — moves the commit selection as ↓ does, and
/// Shift+↑ as ↑. Caught by: a list that leaves alone every scope's chords, which swallowed
/// Shift+arrows here once Local Changes' lists had them.
#[test]
fn another_views_chord_is_the_history_lists_arrow() {
    let reports = Reports::default();
    let (mut test, _) = launch(rows(0..100), &reports);
    press(&mut test, NamedKey::ArrowDown);
    for (action, expected) in [
        (Action::ExtendSelectionDown, 1),
        (Action::ExtendSelectionDown, 2),
        (Action::ExtendSelectionUp, 1),
    ] {
        let chord = accelerators::chords(action, Os::current()).first().unwrap();
        let (key, _, held) = chord.key_press().unwrap();
        test.press_key_with_modifiers(key, held);
        test.sync_and_update();
        assert_eq!(
            reports.selected.borrow().last(),
            Some(&RowId::Commit(oid(expected))),
            "{action:?} did not move the commit selection as its arrow does"
        );
    }
}

#[test]
fn a_click_selects_that_row_and_reports_its_identity() {
    let reports = Reports::default();
    let (mut test, _) = launch(rows(0..100), &reports);

    let y = 3.5 * ROW_HEIGHT as f64;
    test.click_cursor((100., y));

    assert_eq!(
        reports.selected.borrow().as_slice(),
        &[RowId::Commit(oid(3))]
    );
    assert_eq!(selected_rows(&test), vec!["commit 3".to_owned()]);
}

#[test]
fn keys_move_the_selection_and_report_each_row_they_land_on() {
    let reports = Reports::default();
    let (mut test, _) = launch(rows(0..100), &reports);

    for key in [
        NamedKey::ArrowDown,
        NamedKey::ArrowDown,
        NamedKey::PageDown,
        NamedKey::ArrowUp,
        NamedKey::PageDown,
        NamedKey::PageUp,
    ] {
        press(&mut test, key);
    }

    assert_eq!(
        reports.selected.borrow().as_slice(),
        &[0, 1, 11, 10, 20, 10].map(|n| RowId::Commit(oid(n)))
    );
    assert_eq!(selected_rows(&test), vec!["commit 10".to_owned()]);
}

#[test]
fn the_selection_follows_its_row_when_rows_arrive_below_and_above_it() {
    let reports = Reports::default();
    let (mut test, fixture) = launch(rows(1..65), &reports);
    let mut held = fixture.rows;

    press(&mut test, NamedKey::ArrowDown);
    press(&mut test, NamedKey::PageDown);
    assert_eq!(selected_rows(&test), vec!["commit 11".to_owned()]);

    hold(&mut held.write(), page(65..129));
    test.sync_and_update();
    assert_eq!(
        selected_rows(&test),
        vec!["commit 11".to_owned()],
        "a page arriving below moved the selection"
    );

    // Rows only append to a history, so a row arrives above by the history being read
    // again with it: the reopen a fetch that moved refs asks for.
    held.set(rows(0..129));
    test.sync_and_update();
    assert!(
        built_rows(&test).iter().any(|(text, _)| text == "commit 0"),
        "the row that arrived above was never drawn"
    );
    assert_eq!(
        selected_rows(&test),
        vec!["commit 11".to_owned()],
        "a row arriving above moved the selection"
    );

    press(&mut test, NamedKey::ArrowDown);
    assert_eq!(
        reports.selected.borrow().last(),
        Some(&RowId::Commit(oid(12))),
        "the next key moved from the index the row used to have, not from the row"
    );
    assert_eq!(selected_rows(&test), vec!["commit 12".to_owned()]);
}

#[test]
fn a_selection_moved_off_screen_is_scrolled_into_view() {
    let reports = Reports::default();
    let (mut test, _) = launch(rows(0..1_000), &reports);

    press(&mut test, NamedKey::End);

    let shown = |test: &TestingRunner, text: &str| {
        built_rows(test)
            .into_iter()
            .find(|(built, _)| built == text)
            .map(|(_, visible)| visible)
    };
    assert_eq!(
        shown(&test, "> commit 999"),
        Some(true),
        "End selected the last row without revealing it"
    );

    press(&mut test, NamedKey::Home);
    assert_eq!(
        shown(&test, "> commit 0"),
        Some(true),
        "Home selected the first row without revealing it"
    );

    for _ in 0..3 {
        press(&mut test, NamedKey::PageDown);
    }
    assert_eq!(
        shown(&test, "> commit 30"),
        Some(true),
        "PageDown selected a row below the viewport without revealing it"
    );
}

#[test]
fn a_click_gives_the_list_the_keyboard() {
    fn app() -> Element {
        let elsewhere = use_a11y();
        rect()
            .expanded()
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(ROW_HEIGHT))
                    .a11y_id(elsewhere)
                    .a11y_focusable(true)
                    .on_press(move |_| elsewhere.request_focus()),
            )
            .child(list(consume_context::<Reports>())())
            .into()
    }

    let reports = Reports::default();
    let (mut test, _) = TestingRunner::new(
        app,
        (WIDTH, HEIGHT).into(),
        {
            let reports = reports.clone();
            move |runner| {
                runner.provide_root_context(|| reports);
                runner.provide_root_context(|| Fixture {
                    rows: State::create(rows(0..100)),
                    selected: State::create(None),
                })
            }
        },
        1.,
    );
    test.sync_and_update();

    // A focus request is applied on the frame after the one that makes it.
    test.click_cursor((100., ROW_HEIGHT as f64 / 2.));
    test.sync_and_update();
    press(&mut test, NamedKey::ArrowDown);
    assert!(
        reports.selected.borrow().is_empty(),
        "the list still had the keyboard after focus moved away"
    );

    test.click_cursor((100., 3.5 * ROW_HEIGHT as f64));
    test.sync_and_update();
    press(&mut test, NamedKey::ArrowDown);
    assert_eq!(
        reports.selected.borrow().as_slice(),
        &[2, 3].map(|n| RowId::Commit(oid(n))),
        "a click on a row did not take the keyboard back"
    );
}

#[test]
fn the_end_is_reported_when_it_comes_into_view_and_not_on_every_frame() {
    let reports = Reports::default();
    let length = 200;
    let (mut test, fixture) = launch(rows(0..length), &reports);
    let reached = || *reports.reached_end.borrow();

    for _ in 0..10 {
        test.sync_and_update();
    }
    assert_eq!(reached(), 0, "the top of a long history reported its end");

    let far_from_end = (length - PREFETCH_ROWS - 40) as f64 * ROW_HEIGHT as f64;
    test.scroll((100., 100.), (0., -far_from_end));
    assert_eq!(
        reached(),
        0,
        "rows outside the last screen reported the end"
    );

    // Into the last rows, with the very last still below the viewport.
    test.scroll((100., 100.), (0., -30. * ROW_HEIGHT as f64));
    assert!(
        !built_rows(&test)
            .iter()
            .any(|(text, _)| *text == format!("commit {}", length - 1)),
        "the scroll went all the way to the last row"
    );
    let on_arrival = reached();
    assert!(
        on_arrival > 0,
        "coming within a screen of the end did not report it"
    );

    // Each click re-renders every visible row without bringing a new one into view.
    for n in 0..5 {
        test.click_cursor((100., (n as f64 + 0.5) * ROW_HEIGHT as f64));
        test.sync_and_update();
    }
    assert!(
        !reports.selected.borrow().is_empty(),
        "the clicks did not re-render the rows"
    );
    assert_eq!(
        reached(),
        on_arrival,
        "re-rendering rows already in view reported the end again"
    );

    let mut held = fixture.rows;
    hold(&mut held.write(), page(length..length * 2));
    test.sync_and_update();
    test.scroll((100., 100.), (0., -2. * length as f64 * ROW_HEIGHT as f64));
    assert!(
        built_rows(&test)
            .iter()
            .any(|(text, visible)| *visible && *text == format!("commit {}", length * 2 - 1)),
        "the list never grew to the page that arrived"
    );
    assert!(
        reached() > on_arrival,
        "the end of the next page was never reported"
    );
}

#[test]
fn the_list_is_outlined_only_while_it_has_keyboard_focus() {
    let (mut test, _) = launch(rows(0..100), &Reports::default());
    let outlined = |test: &TestingRunner| {
        test.find(|_, element| {
            Rect::try_downcast(element)
                .filter(|rect| rect.accessibility.builder.role() == AccessibilityRole::List)
                .map(|rect| !rect.style.borders.is_empty())
        })
        .expect("the list's container")
    };

    assert!(!outlined(&test), "a pointer-focused list was outlined");

    test.run_in(|| {
        Platform::get()
            .navigation_mode
            .set(NavigationMode::Keyboard)
    });
    // One frame recomputes the focus memo, the next renders with it.
    test.sync_and_update();
    test.sync_and_update();
    assert!(outlined(&test), "a keyboard-focused list was not outlined");
}

/// A walk that keeps several lanes open and hands some parents over before their children:
/// commit `n`'s parents are `n + 1` and, every seventh, `n + 4` and `n + 9`; every
/// thirteenth commit swaps places with the one after it.
fn braided_walk(len: usize) -> Vec<(Oid, Vec<Oid>)> {
    let mut walk: Vec<(Oid, Vec<Oid>)> = (0..len)
        .map(|n| {
            let mut parents: Vec<Oid> = [1, 4, 9]
                .into_iter()
                .take(if n % 7 == 0 { 3 } else { 1 })
                .filter(|step| n + step < len)
                .map(|step| oid(n + step))
                .collect();
            parents.dedup();
            (oid(n), parents)
        })
        .collect();
    for n in (0..len.saturating_sub(1)).step_by(13) {
        walk.swap(n, n + 1);
    }
    walk
}

/// The QA brief's scroll: deep, then back to the top. Every row the list draws, each time it
/// draws it, carries exactly the edges the assigner retained for it before compaction.
/// Caught by: deriving a row from a snapshot other than its own nearest, or from state a
/// previous row's derivation left behind.
#[test]
fn rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew() {
    let length = 3_000;
    let walk = braided_walk(length);
    let mut before_assigner = AssignerBeforeCompaction::new();
    let mut before = Vec::new();
    for (id, parents) in &walk {
        before.extend(before_assigner.push(*id, parents.clone()));
    }
    before.extend(before_assigner.into_rows());

    let graphs = LaneAssigner::assign_all(walk.iter().cloned());
    let index_of: HashMap<Oid, usize> = walk
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (*id, index))
        .collect();
    let mut history = History::new();
    // Pages of the application's 64 rows, as the worker hands them over.
    for chunk in graphs.chunks(64) {
        let mut paged = RowsPage::new();
        for graph in chunk {
            paged.push(
                graph.clone(),
                PagedCommit {
                    parents: walk[index_of[&graph.id]].1.len(),
                    subject: &format!("commit {}", index_of[&graph.id]),
                    author: "A",
                    author_time: 0,
                },
            );
        }
        hold(&mut history, paged);
    }

    let drawn: Rc<RefCell<Vec<(usize, RowEdges)>>> = Rc::default();
    let recorder = drawn.clone();
    let index_of_row = index_of.clone();
    let (mut test, _) = TestingRunner::new(
        move || -> Element {
            let fixture = use_consume::<Fixture>();
            let recorder = recorder.clone();
            let index_of = index_of_row.clone();
            HistoryList::new(fixture.rows, move |render: RowRender| {
                let (id, subject) = match &render.content {
                    RowContent::Commit(commit) => (commit.id, commit.summary.clone()),
                    RowContent::Stash(stash) => (stash.id, stash.message.clone()),
                };
                recorder
                    .borrow_mut()
                    .push((index_of[&id], render.graph.clone()));
                label().height(Size::px(ROW_HEIGHT)).text(subject).into()
            })
            .into()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(|| Fixture {
                rows: State::create(history),
                selected: State::create(None),
            })
        },
        1.,
    );
    test.sync_and_update();
    let at_the_top = drawn.borrow().len();

    let deep = 2_500.0 * ROW_HEIGHT as f64;
    test.scroll((100., 100.), (0., -deep));
    test.scroll((100., 100.), (0., deep));

    let drawn = drawn.borrow();
    for (index, graph) in drawn.iter() {
        let old = &before[*index];
        assert_eq!(
            graph,
            &RowEdges {
                lane: old.lane,
                edges: old.edges.clone(),
            },
            "row {index} drew other edges than the assigner retained for it"
        );
    }
    let times_drawn = |index: usize| drawn.iter().filter(|(at, _)| *at == index).count();
    assert!(at_the_top > 0, "nothing was drawn at the top");
    assert!(
        (0..5).all(|index| times_drawn(index) >= 2),
        "the top rows were not drawn again after the scroll back"
    );
    assert!(
        drawn.iter().any(|(index, _)| *index > 2_000),
        "the scroll never reached deep rows"
    );
    assert!(
        drawn
            .iter()
            .any(|(_, graph)| graph.edges.iter().any(|edge| edge.out_of_order)),
        "no drawn row carried a repainted line, so the late lines decided nothing"
    );
}

/// RR1: the list keeps where its selection sits in the hint its caller gives it, and reads it
/// there first — so a row its caller chose (a pressed ref's, a parent link's), told to the
/// hint, is moved from at once rather than searched for among every loaded row. A press is
/// written to the caller's hint; and with one commit drawn twice — rows 5 and 500, a
/// history no walk makes, built so the two ways of finding it disagree — the hint at 500
/// moves an arrow key to 501, where a search from the top would find 5 and move to 6. Caught
/// by: the list keeping a hint of its own and ignoring the caller's.
#[test]
fn the_list_moves_from_the_row_its_callers_hint_names() {
    #[derive(Clone)]
    struct Hinted {
        rows: State<History>,
        selected: State<Option<RowId>>,
        cursor: State<usize>,
    }
    let reports = Reports::default();
    let on_select = reports.selected.clone();
    let mut history = rows(0..5);
    hold(&mut history, page([500]));
    hold(&mut history, page(6..500));
    hold(&mut history, page([500]));
    hold(&mut history, page(501..600));
    let (mut test, hinted) = TestingRunner::new(
        move || {
            let hinted = use_consume::<Hinted>();
            let mut selected = hinted.selected;
            let on_select = on_select.clone();
            HistoryList::new(hinted.rows, |render: RowRender| {
                let subject = match render.content {
                    RowContent::Commit(commit) => commit.summary,
                    RowContent::Stash(stash) => stash.message,
                };
                label().height(Size::px(ROW_HEIGHT)).text(subject).into()
            })
            .selected(*selected.read())
            .cursor(hinted.cursor)
            .on_select(move |id: RowId| {
                on_select.borrow_mut().push(id);
                selected.set(Some(id));
            })
            .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(|| Hinted {
                rows: State::create(history),
                selected: State::create(None),
                cursor: State::create(0),
            })
        },
        1.,
    );
    test.sync_and_update();

    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == "commit 3")
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("commit 3 is not drawn"));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.sync_and_update();
    assert_eq!(
        *hinted.cursor.peek(),
        3,
        "a press did not reach the caller's hint"
    );

    let (mut selected, mut cursor) = (hinted.selected, hinted.cursor);
    test.run_in(|| {
        selected.set(Some(RowId::Commit(oid(500))));
        cursor.set(500);
    });
    test.sync_and_update();
    reports.selected.borrow_mut().clear();
    press(&mut test, NamedKey::ArrowDown);
    assert_eq!(
        reports.selected.borrow().as_slice(),
        [RowId::Commit(oid(501))],
        "the arrow moved from the row a search found, not the one the hint named"
    );
}

fn right_click(test: &mut TestingRunner, at: (f64, f64)) {
    test.move_cursor(at);
    for name in [MouseEventName::MouseDown, MouseEventName::MouseUp] {
        test.send_event(PlatformEvent::Mouse {
            name,
            cursor: at.into(),
            button: Some(MouseButton::Right),
        });
        test.sync_and_update();
    }
    test.sync_and_update();
}

/// The user's decision (2026-10-09): every commit row's context menu offers Fork's "New
/// Branch…", and choosing it reports that row's commit — a lost row's as any other's; a
/// stash's row offers none. Caught by: the menu on lost rows alone (R11.3 as it read before),
/// the wrong commit reported, or a stash's row offering a branch at its stash commit.
#[test]
fn every_commit_rows_menu_offers_new_branch_at_its_commit() {
    let mut history = rows(0..3);
    let mut lost = RowsPage::new();
    lost.push_lost(
        GraphRow::new(oid(3), Lane::new(1), Vec::new()),
        PagedCommit {
            parents: 1,
            subject: "commit 3",
            author: "A",
            author_time: 0,
        },
    );
    lost.push_stash(
        GraphRow::new(oid(4), Lane::new(2), Vec::new()),
        cairn_model::PagedStash {
            index: 0,
            base: oid(0),
            message: "commit 4 stash",
            author: "A",
            author_time: 0,
        },
    );
    hold(&mut history, lost);
    let reports = Reports::default();
    let (mut test, _) = launch(history, &reports);
    let row_at = |n: usize| (100., (n as f64 + 0.5) * f64::from(ROW_HEIGHT));
    let menu_offered = |test: &TestingRunner| {
        test.find(|_, element| {
            Label::try_downcast(element).filter(|l| l.text == NEW_BRANCH_CAPTION)
        })
        .is_some()
    };
    for n in [1usize, 3] {
        right_click(&mut test, row_at(n));
        assert!(menu_offered(&test), "row {n} offered no menu");
        let item = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|l| l.text == NEW_BRANCH_CAPTION)
                    .map(|_| node.layout().area.center())
            })
            .unwrap_or_else(|| panic!("no item"));
        test.click_cursor((f64::from(item.x), f64::from(item.y)));
        test.sync_and_update();
        assert_eq!(
            reports.new_branch.borrow().last(),
            Some(&(oid(n), format!("commit {n}"))),
            "row {n}"
        );
        assert!(!menu_offered(&test), "the menu stayed open");
    }
    right_click(&mut test, row_at(4));
    assert!(!menu_offered(&test), "a stash's row offered New Branch");
    assert_eq!(reports.new_branch.borrow().len(), 2);
}
