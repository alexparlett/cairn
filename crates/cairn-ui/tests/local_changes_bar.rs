//! Headless tests for the bar over Local Changes' two lists and their headings, as Fork for
//! Windows draws them (`docs/research/ui-review/fork-window-survey-2026-10-16.md` §7, uv12a and
//! uv12b; `fork-window-survey-2-2026-10-16.md` §1c, uw6): the filter with no placeholder, then
//! Stage All, the side-by-side eye and View Mode at the bar's right; bold headings, each with its
//! bordered Stage or Unstage button greyed while nothing is selected in its list, the Staged
//! heading's ⇈ Unstage All kept and Unstaged's chevron gone; rows 20 px apart.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use cairn_model::{
    ChangeList, ChangedEntry, LocalChanges, RepoPath, StagedChange, StatusEntry, UnstagedChange,
    WorkingTreeStatus,
};
use cairn_ui::accelerators::{Action, HeldKeys};
use cairn_ui::style::{metrics, tones};
use cairn_ui::{
    FILTER_PLACEHOLDER, LIST_HEADER_HEIGHT, LISTS_BAR_HEIGHT, LOCAL_CHANGES_ROW_HEIGHT, ListIntent,
    LocalChangesList, LocalSelection, SIDE_BY_SIDE_TOOLTIP, STAGE_ALL_CAPTION, STAGE_CAPTION,
    STAGED_CAPTION, ShownFiles, UNSTAGE_ALL_CAPTION, UNSTAGE_CAPTION, UNSTAGED_CAPTION,
    VIEW_AS_LIST_CAPTION, VIEW_MODE_TOOLTIP,
};
use freya::elements::label::LabelElement;
use freya::elements::rect::RectElement;
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 400.;
const HEIGHT: f32 = 600.;

#[derive(Clone, Copy)]
struct Fixture {
    changes: State<LocalChanges>,
    selection: State<Option<LocalSelection>>,
    side_by_side: State<bool>,
}

/// What the lists reported.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Heard {
    Intent(ListIntent),
    Eye,
}

type Log = Rc<RefCell<Vec<Heard>>>;

fn launch(changes: LocalChanges) -> (TestingRunner, Fixture, Log) {
    let log = Log::default();
    let hearing = log.clone();
    let (mut test, fixture) = TestingRunner::new(
        move || {
            let fixture = use_consume::<Fixture>();
            let (intending, eyeing) = (hearing.clone(), hearing.clone());
            let filter = use_state(String::new);
            let held = use_state(HeldKeys::default);
            let unstaged = use_state(|| ShownFiles::All);
            let staged = use_state(|| ShownFiles::All);
            rect()
                .expanded()
                .child(
                    LocalChangesList::new(fixture.changes, unstaged, staged, filter)
                        .selection(fixture.selection)
                        .held(held)
                        .side_by_side(*fixture.side_by_side.read())
                        .on_intent(move |intent: ListIntent| {
                            intending.borrow_mut().push(Heard::Intent(intent));
                        })
                        .on_side_by_side(move |()| eyeing.borrow_mut().push(Heard::Eye)),
                )
                .child(ContextMenuViewer::new())
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || Fixture {
                changes: State::create(changes),
                selection: State::create(None),
                side_by_side: State::create(false),
            })
        },
        1.,
    );
    test.sync_and_update();
    (test, fixture, log)
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

/// Unstaged: a.rs, b.rs, c.rs; Staged: s.rs.
fn lists() -> LocalChanges {
    LocalChanges::new(WorkingTreeStatus::Listed(vec![
        changed("a.rs", None, Some(UnstagedChange::Modified)),
        changed("b.rs", None, Some(UnstagedChange::Modified)),
        StatusEntry::Untracked(RepoPath::from("c.rs")),
        changed("s.rs", Some(StagedChange::Modified), None),
    ]))
}

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

/// The first label reading `text`, and where it is laid out.
fn label_of(test: &TestingRunner, text: &str) -> (LabelElement, Area) {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|label| (label, node.layout().area))
    })
    .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)))
}

/// Every control assistive technology reads as `name` — a glyph button's value or a control's
/// name — with where it is laid out.
fn named(test: &TestingRunner, name: &str) -> Vec<(RectElement, Area)> {
    test.find_many(|node, element| {
        Rect::try_downcast(element)
            .filter(|rect| {
                let builder = &rect.accessibility.builder;
                builder.value() == Some(name) || builder.label() == Some(name)
            })
            .map(|rect| (rect, node.layout().area))
    })
}

/// The one control named `name`.
fn one_named(test: &TestingRunner, name: &str) -> (RectElement, Area) {
    let mut found = named(test, name);
    assert_eq!(
        found.len(),
        1,
        "{} controls are named {name:?}",
        found.len()
    );
    found.remove(0)
}

fn press(test: &mut TestingRunner, at: Area) {
    let centre = at.center();
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    for _ in 0..3 {
        test.sync_and_update();
    }
}

/// Rests the pointer on `at` long enough for its tooltip, and the labels drawn then.
fn hovered(test: &mut TestingRunner, at: Area) -> Vec<String> {
    let centre = at.center();
    test.move_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.poll(Duration::from_millis(50), Duration::from_millis(900));
    let shown = labels(test);
    test.move_cursor((f64::from(WIDTH - 2.), f64::from(HEIGHT - 2.)));
    test.poll(Duration::from_millis(50), Duration::from_millis(400));
    shown
}

/// A heading control's look: its fill and its border's.
fn look(rect: &RectElement) -> (Fill, Option<Color>) {
    (
        rect.style.background.clone(),
        rect.style.borders.first().map(|border| border.fill),
    )
}

/// The bar across both lists' top holds, left to right, the filter field with no placeholder,
/// Stage All, the eye and View Mode, Stage All's the only double chevron outside Staged's
/// heading; the headings are bold at Fork's heading size, each its bordered Stage or Unstage
/// button greyed while nothing is selected in its list — Unstage still greyed with Unstaged's
/// row selected — the Staged heading keeping its ⇈ Unstage All; and the rows are 20 px apart.
/// Caught by: the filter's "Filter" placeholder, Stage All left in Unstaged's heading, the
/// bar's controls out of Fork's order or under a heading, a heading at the body's weight or
/// size, a button drawn unbordered or enabled with nothing selected in its list, Unstage All
/// gone, or the rows at the Changes list's 24 px.
#[test]
fn local_changes_bar_and_headings_are_forks() {
    let (mut test, fixture, _) = launch(lists());
    let (_, unstaged_heading) = label_of(&test, UNSTAGED_CAPTION);
    let (_, staged_heading) = label_of(&test, STAGED_CAPTION);
    assert!(unstaged_heading.max_y() < staged_heading.min_y());

    // The bar, over the first heading.
    let bar = [STAGE_ALL_CAPTION, SIDE_BY_SIDE_TOOLTIP, VIEW_MODE_TOOLTIP]
        .map(|name| one_named(&test, name).1);
    for area in bar {
        assert!(
            area.center().y < LISTS_BAR_HEIGHT && area.max_y() < unstaged_heading.min_y(),
            "a control of the bar stands at {area:?}, not in the bar over Unstaged's heading"
        );
    }
    assert!(
        bar[0].max_x() <= bar[1].min_x() && bar[1].max_x() <= bar[2].min_x(),
        "the bar is not Stage All, the eye and View Mode left to right: {bar:?}"
    );
    assert!(
        bar[0].min_x() > WIDTH / 2.,
        "the bar's buttons are not at its right"
    );
    let filter = test
        .find(|node, element| {
            Rect::try_downcast(element)
                .filter(|rect| rect.accessibility.builder.role() == AccessibilityRole::TextInput)
                .map(|_| node.layout().area)
        })
        .unwrap_or_else(|| panic!("no filter field"));
    assert!(
        filter.center().y < LISTS_BAR_HEIGHT && filter.max_x() <= bar[0].min_x(),
        "the filter is not in the bar left of its buttons: {filter:?}"
    );
    let placeholders = test.find_many(|_, element| {
        Paragraph::try_downcast(element).filter(|paragraph| {
            paragraph
                .spans
                .iter()
                .any(|span| span.text.as_ref() == FILTER_PLACEHOLDER)
        })
    });
    assert!(placeholders.is_empty(), "the filter draws a placeholder");

    // The headings: bold, at Fork's heading size, each its band's height.
    for caption in [UNSTAGED_CAPTION, STAGED_CAPTION] {
        let (heading, _) = label_of(&test, caption);
        assert_eq!(
            heading.text_style_data.font_weight,
            Some(FontWeight::BOLD),
            "{caption} is not bold"
        );
        assert_eq!(
            heading.text_style_data.font_size,
            Some(FontSize::from(metrics::HEADING)),
            "{caption} is not at the heading size"
        );
    }
    let heading_bands = test.find_many(|node, element| {
        Rect::try_downcast(element)
            .filter(|rect| rect.style.background == Fill::Color(tones::HEADING_BAR))
            .map(|_| node.layout().area.height())
    });
    assert_eq!(heading_bands, [LIST_HEADER_HEIGHT, LIST_HEADER_HEIGHT]);

    // Stage All is the bar's alone; the Staged heading keeps Unstage All.
    let (_, unstage_all) = one_named(&test, UNSTAGE_ALL_CAPTION);
    assert!(
        unstage_all.center().y > staged_heading.min_y()
            && unstage_all.center().y < staged_heading.max_y(),
        "Unstage All is not on the Staged heading: {unstage_all:?}"
    );

    // The buttons: bordered, greyed while nothing is selected in their list.
    let greyed = (
        Fill::Color(tones::DISABLED_FILL),
        Some(tones::DISABLED_BORDER),
    );
    let enabled = (Fill::Color(tones::BUTTON), Some(tones::BUTTON_BORDER));
    for caption in [STAGE_CAPTION, UNSTAGE_CAPTION] {
        let (button, area) = one_named(&test, caption);
        assert_eq!(look(&button), greyed, "{caption} is not greyed");
        assert_eq!(area.height(), metrics::LIST_HEADING_BUTTON);
    }
    let mut selection = fixture.selection;
    test.run_in(|| {
        selection.set(Some(LocalSelection::of(
            ChangeList::Unstaged,
            RepoPath::from("a.rs"),
        )))
    });
    test.sync_and_update();
    assert_eq!(look(&one_named(&test, STAGE_CAPTION).0), enabled);
    assert_eq!(look(&one_named(&test, UNSTAGE_CAPTION).0), greyed);

    // Rows 20 px apart.
    let (_, a) = label_of(&test, "a.rs");
    let (_, b) = label_of(&test, "b.rs");
    assert_eq!(b.center().y - a.center().y, LOCAL_CHANGES_ROW_HEIGHT);
    assert_eq!(LOCAL_CHANGES_ROW_HEIGHT, 20.);
}

/// Stage All reports the lists' Stage All on Unstaged, the eye its press and View Mode opens a
/// menu of "View as List" alone, ticked, whose choice reports nothing; each says Fork's words
/// as its tooltip, and the eye says whether the diff is side by side. Caught by: Stage All
/// reported on Staged, the eye unheard or its state not said, View Mode offering a mode Cairn
/// does not draw, or a tooltip not Fork's.
#[test]
fn the_bars_controls_report_their_presses_and_say_what_they_are() {
    let (mut test, fixture, log) = launch(lists());
    for (name, tooltip) in [
        (STAGE_ALL_CAPTION, STAGE_ALL_CAPTION),
        (SIDE_BY_SIDE_TOOLTIP, SIDE_BY_SIDE_TOOLTIP),
        (VIEW_MODE_TOOLTIP, VIEW_MODE_TOOLTIP),
    ] {
        let at = one_named(&test, name).1;
        let shown = hovered(&mut test, at);
        assert!(
            shown.iter().any(|label| label == tooltip),
            "{name}'s tooltip is not {tooltip:?}: {shown:?}"
        );
    }
    let toggled = |test: &TestingRunner| one_named(test, SIDE_BY_SIDE_TOOLTIP).0;
    assert_eq!(
        toggled(&test).accessibility.builder.toggled(),
        Some(Toggled::False)
    );
    let mut side_by_side = fixture.side_by_side;
    test.run_in(|| side_by_side.set(true));
    test.sync_and_update();
    assert_eq!(
        toggled(&test).accessibility.builder.toggled(),
        Some(Toggled::True)
    );

    let at = one_named(&test, STAGE_ALL_CAPTION).1;
    press(&mut test, at);
    let at = one_named(&test, SIDE_BY_SIDE_TOOLTIP).1;
    press(&mut test, at);
    let before = labels(&test);
    let at = one_named(&test, VIEW_MODE_TOOLTIP).1;
    press(&mut test, at);
    let opened: Vec<String> = labels(&test)
        .into_iter()
        .filter(|label| !before.contains(label) && label != "✓")
        .collect();
    assert_eq!(
        opened,
        [VIEW_AS_LIST_CAPTION],
        "View Mode's menu: {opened:?}"
    );
    let (_, item) = label_of(&test, VIEW_AS_LIST_CAPTION);
    let ticks = test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == "✓")
            .map(|_| node.layout().area.center().y)
    });
    assert!(
        ticks.iter().any(|y| (y - item.center().y).abs() < 1.),
        "View as List is not ticked"
    );
    press(&mut test, item);
    assert_eq!(
        *log.borrow(),
        [
            Heard::Intent(ListIntent::Act(
                ChangeList::Unstaged,
                Action::StageOrUnstageAll
            )),
            Heard::Eye,
        ]
    );
}

/// With Unstaged empty Stage All is greyed and its press reports nothing. Caught by: Stage All
/// pressable over an empty list.
#[test]
fn stage_all_is_greyed_while_unstaged_shows_no_row() {
    let (mut test, _, log) = launch(LocalChanges::new(WorkingTreeStatus::Listed(vec![changed(
        "s.rs",
        Some(StagedChange::Modified),
        None,
    )])));
    let (stage_all, at) = one_named(&test, STAGE_ALL_CAPTION);
    assert!(stage_all.accessibility.builder.is_disabled());
    press(&mut test, at);
    assert_eq!(*log.borrow(), []);
}
