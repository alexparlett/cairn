//! Headless component tests for `HistoryList`.

#[path = "../../cairn-model/tests/layout_before_compaction/mod.rs"]
mod layout_before_compaction;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cairn_model::{
    CommitSummary, GraphRow, HistoryRow, Lane, LaneAssigner, Oid, RowContent, RowEdges, RowId,
};
use cairn_ui::accelerators::{self, Action, Os};
use cairn_ui::{HistoryList, PREFETCH_ROWS, ROW_HEIGHT, RowRender};
use freya::prelude::*;
use freya_testing::TestingRunner;
use layout_before_compaction::AssignerBeforeCompaction;

const WIDTH: f32 = 600.;
const HEIGHT: f32 = 520.;

fn oid(n: usize) -> Oid {
    let mut bytes = [0u8; 20];
    bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
    Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
}

fn row(n: usize) -> HistoryRow {
    HistoryRow {
        content: RowContent::Commit(CommitSummary {
            id: oid(n),
            parents: Vec::new(),
            summary: format!("commit {n}"),
            author_name: "A".to_owned(),
            author_email: "a@example.com".to_owned(),
            author_time: 0,
        }),
        graph: GraphRow::new(oid(n), Lane::new(0), Vec::new()),
    }
}

fn rows(range: std::ops::Range<usize>) -> Vec<HistoryRow> {
    range.map(row).collect()
}

/// What the list reported, in order.
#[derive(Clone, Default)]
struct Reports {
    selected: Rc<RefCell<Vec<RowId>>>,
    reached_end: Rc<RefCell<usize>>,
}

#[derive(Clone)]
struct Fixture {
    rows: State<Vec<HistoryRow>>,
    selected: State<Option<RowId>>,
}

/// Each row draws as one label: its subject, prefixed `> ` when the list says it is selected.
fn list(reports: Reports) -> impl Fn() -> Element + 'static {
    move || {
        let fixture = use_consume::<Fixture>();
        let mut selected = fixture.selected;
        let on_select = reports.selected.clone();
        let reached_end = reports.reached_end.clone();

        HistoryList::new(fixture.rows, |render: RowRender| {
            let RowContent::Commit(commit) = render.row.content;
            let marker = if render.selected { "> " } else { "" };
            label()
                .height(Size::px(ROW_HEIGHT))
                .text(format!("{marker}{}", commit.summary))
                .into()
        })
        .selected(*selected.read())
        .on_select(move |id: RowId| {
            on_select.borrow_mut().push(id);
            selected.set(Some(id));
        })
        .on_reach_end(move |()| *reached_end.borrow_mut() += 1)
        .into()
    }
}

fn launch(initial: Vec<HistoryRow>, reports: &Reports) -> (TestingRunner, Fixture) {
    let (mut test, fixture) = TestingRunner::new(
        list(reports.clone()),
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
        let chord = accelerators::chord(action, Os::current()).unwrap();
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

    held.write().extend(rows(65..129));
    test.sync_and_update();
    assert_eq!(
        selected_rows(&test),
        vec!["commit 11".to_owned()],
        "a page arriving below moved the selection"
    );

    held.write().insert(0, row(0));
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
    held.write().extend(rows(length..length * 2));
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
    let history: Vec<HistoryRow> = graphs
        .into_iter()
        .zip(&walk)
        .map(|(graph, (id, parents))| HistoryRow {
            content: RowContent::Commit(CommitSummary {
                id: *id,
                parents: parents.clone(),
                summary: format!("commit {}", index_of[id]),
                author_name: "A".to_owned(),
                author_email: "a@example.com".to_owned(),
                author_time: 0,
            }),
            graph,
        })
        .collect();

    let drawn: Rc<RefCell<Vec<(usize, RowEdges)>>> = Rc::default();
    let recorder = drawn.clone();
    let index_of_row = index_of.clone();
    let (mut test, _) = TestingRunner::new(
        move || -> Element {
            let fixture = use_consume::<Fixture>();
            let recorder = recorder.clone();
            let index_of = index_of_row.clone();
            HistoryList::new(fixture.rows, move |render: RowRender| {
                let RowContent::Commit(commit) = &render.row.content;
                recorder
                    .borrow_mut()
                    .push((index_of[&commit.id], render.graph.clone()));
                label()
                    .height(Size::px(ROW_HEIGHT))
                    .text(commit.summary.clone())
                    .into()
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
