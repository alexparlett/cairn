//! Headless tests for what Local Changes' lists report when a person acts on them
//! (staging-and-commit R8.1, R8.2, R8.4, R8.7, R8.8; criterion C18's routes and drag, C8's
//! submodule row and C24's conflicted row, each at the component's boundary): every press,
//! chord, button, drag and menu item is a `ListIntent` or a choice, and the component decides
//! nothing about what it stages.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use cairn_model::{
    ChangeList, ChangedEntry, ConflictKind, ConflictedEntry, LocalChanges, RepoPath, StagedChange,
    StatusEntry, SubmoduleState, UnstagedChange, WorkingTreeStatus,
};
use cairn_ui::accelerators::{self, Action, HeldKeys, Os};
use cairn_ui::{
    COPY_PATH_CAPTION, DETAIL_ROW_HEIGHT, DISCARD_CAPTION, ListIntent, ListSelection,
    LocalChangesList, STAGE_ALL_CAPTION, STAGE_CAPTION, STAGED_CAPTION, ShownFiles,
    UNSTAGE_ALL_CAPTION, UNSTAGE_CAPTION, UNSTAGED_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{KeyboardEventName, MouseEventName, PlatformEvent};

const WIDTH: f32 = 400.;
const HEIGHT: f32 = 600.;

#[derive(Clone, Copy)]
struct Fixture {
    changes: State<LocalChanges>,
    unstaged: State<ShownFiles>,
    staged: State<ShownFiles>,
    filter: State<String>,
    chosen: State<Option<(ChangeList, usize)>>,
    selection: State<ListSelection>,
    held: State<HeldKeys>,
}

/// What the lists reported: a choice, or an intent.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Heard {
    Chose(ChangeList, usize),
    Intent(ListIntent),
}

type Log = Rc<RefCell<Vec<Heard>>>;

fn launch(changes: LocalChanges) -> (TestingRunner, Fixture, Log) {
    let log = Log::default();
    let hearing = log.clone();
    let (mut test, fixture) = TestingRunner::new(
        move || {
            let fixture = use_consume::<Fixture>();
            let (choosing, intending) = (hearing.clone(), hearing.clone());
            let mut chosen = fixture.chosen;
            rect()
                .expanded()
                .child(
                    LocalChangesList::new(
                        fixture.changes,
                        fixture.unstaged,
                        fixture.staged,
                        fixture.filter,
                    )
                    .chosen(*fixture.chosen.read())
                    .selection(fixture.selection)
                    .held(fixture.held)
                    .on_choose(move |pressed: (ChangeList, usize)| {
                        choosing
                            .borrow_mut()
                            .push(Heard::Chose(pressed.0, pressed.1));
                        chosen.set(Some(pressed));
                    })
                    .on_intent(move |intent: ListIntent| {
                        intending.borrow_mut().push(Heard::Intent(intent));
                    }),
                )
                .child(ContextMenuViewer::new())
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || Fixture {
                changes: State::create(changes),
                unstaged: State::create(ShownFiles::All),
                staged: State::create(ShownFiles::All),
                filter: State::create(String::new()),
                chosen: State::create(None),
                selection: State::create(ListSelection::default()),
                held: State::create(HeldKeys::default()),
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

/// Unstaged: a.rs, b.rs, c.rs, clash.rs (conflicted), sub (a submodule); Staged: s.rs, t.rs.
fn lists() -> LocalChanges {
    LocalChanges::new(WorkingTreeStatus::Listed(vec![
        changed("a.rs", None, Some(UnstagedChange::Modified)),
        changed("b.rs", None, Some(UnstagedChange::Modified)),
        StatusEntry::Untracked(RepoPath::from("c.rs")),
        StatusEntry::Conflicted(ConflictedEntry {
            path: RepoPath::from("clash.rs"),
            kind: ConflictKind::BothModified,
            submodule: None,
        }),
        StatusEntry::Changed(ChangedEntry {
            path: RepoPath::from("sub"),
            staged: None,
            unstaged: Some(UnstagedChange::Modified),
            submodule: Some(SubmoduleState {
                new_commits: true,
                modified_content: false,
                untracked_content: false,
            }),
        }),
        changed("s.rs", Some(StagedChange::Modified), None),
        changed("t.rs", Some(StagedChange::Added), None),
    ]))
}

fn labels(test: &TestingRunner) -> Vec<(String, f32, f32)> {
    test.find_many(|node, element| {
        Label::try_downcast(element).map(|label| {
            let area = node.layout().area;
            (label.text.to_string(), area.min_x(), area.center().y)
        })
    })
}

/// Where the label reading `text` is drawn: its first, a little inside its left edge.
fn at(test: &TestingRunner, text: &str) -> (f64, f64) {
    labels(test)
        .into_iter()
        .find(|(drawn, ..)| drawn == text)
        .map(|(_, x, y)| (f64::from(x) + 5., f64::from(y)))
        .unwrap_or_else(|| panic!("{text} is not drawn: {:?}", labels(test)))
}

fn texts(test: &TestingRunner) -> Vec<String> {
    labels(test).into_iter().map(|(text, ..)| text).collect()
}

fn heard(log: &Log) -> Vec<Heard> {
    log.borrow().clone()
}

/// Holds the keys of `action`'s press chord (the first press of its list), as the window
/// would hear them, through the table.
fn hold(test: &mut TestingRunner, fixture: Fixture, action: Action) {
    let Some((key, code, modifiers)) = accelerators::chords(action, Os::current())
        .iter()
        .find_map(|chord| chord.press_hold())
    else {
        panic!("{action:?} has no press");
    };
    let mut held = fixture.held;
    test.run_in(|| {
        held.write()
            .heard(&KeyboardEventData::new(key, code, modifiers), true)
    });
    test.sync_and_update();
}

fn let_go(test: &mut TestingRunner, fixture: Fixture) {
    let mut held = fixture.held;
    test.run_in(|| held.set(HeldKeys::default()));
    test.sync_and_update();
}

/// Presses each of `action`'s key chords on the focused list.
fn press_each_chord(test: &mut TestingRunner, action: Action) -> usize {
    let mut pressed = 0;
    for chord in accelerators::chords(action, Os::current()).iter() {
        if let Some((key, code, modifiers)) = chord.key_press() {
            test.send_event(PlatformEvent::Keyboard {
                name: KeyboardEventName::KeyDown,
                key,
                code,
                modifiers,
            });
            test.sync_and_update();
            pressed += 1;
        }
    }
    pressed
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

/// R8.1, R7.3: a press with the table's extending press held toggles its row, one with the
/// range press held ranges to it, and a plain press chooses; Shift+↑/↓ on the focused list
/// ranges to the row above or below the one chosen. Caught by: a press read without the held
/// keys (always a choice), the presses swapped, or Shift+↓ moving the choice.
#[test]
fn a_held_press_toggles_or_ranges_and_shift_arrows_range() {
    let (mut test, fixture, log) = launch(lists());
    use ChangeList::Unstaged as U;
    test.click_cursor(at(&test, "a.rs"));
    test.sync_and_update();
    hold(&mut test, fixture, Action::ExtendSelection);
    test.click_cursor(at(&test, "c.rs"));
    test.sync_and_update();
    let_go(&mut test, fixture);
    hold(&mut test, fixture, Action::SelectRange);
    test.click_cursor(at(&test, "clash.rs"));
    test.sync_and_update();
    let_go(&mut test, fixture);
    assert_eq!(
        heard(&log),
        [
            Heard::Chose(U, 0),
            Heard::Intent(ListIntent::Toggle(U, 2)),
            Heard::Intent(ListIntent::Range(U, 3)),
        ]
    );
    // a.rs chosen: Shift+↓ ranges to b.rs, Shift+↑ (from a.rs) to the first row.
    log.borrow_mut().clear();
    assert_eq!(press_each_chord(&mut test, Action::ExtendSelectionDown), 1);
    assert_eq!(press_each_chord(&mut test, Action::ExtendSelectionUp), 1);
    assert_eq!(
        heard(&log),
        [
            Heard::Intent(ListIntent::Range(U, 1)),
            Heard::Intent(ListIntent::Range(U, 0)),
        ]
    );
}

/// R8.2, R7.3: on a focused list each chord of stage or unstage, stage all and discard is
/// reported as that action on that list — Unstaged's and Staged's alike — and nothing else.
/// Caught by: a chord of the list heard as an arrow, one of an action's alternates unheard
/// (Return alone), or the action reported for the other list.
#[test]
fn every_chord_of_the_lists_actions_is_reported_on_its_list() {
    let (mut test, _, log) = launch(lists());
    for (row, list) in [("a.rs", ChangeList::Unstaged), ("s.rs", ChangeList::Staged)] {
        test.click_cursor(at(&test, row));
        test.sync_and_update();
        log.borrow_mut().clear();
        let mut expected = Vec::new();
        for action in [
            Action::StageOrUnstage,
            Action::StageOrUnstageAll,
            Action::Discard,
        ] {
            let pressed = press_each_chord(&mut test, action);
            assert!(pressed > 0, "{action:?} has no key");
            expected.extend(std::iter::repeat_n(
                Heard::Intent(ListIntent::Act(list, action)),
                pressed,
            ));
        }
        assert_eq!(heard(&log), expected, "{list:?}");
    }
}

/// Fork's double-click (R8.2): the second press of a double press is reported as the row's
/// stage or unstage, and its release chooses nothing more. Caught by: no double press heard,
/// or its release choosing the row again after it was acted on.
#[test]
fn a_double_press_stages_or_unstages_its_row() {
    let (mut test, _, log) = launch(lists());
    let row = at(&test, "b.rs");
    test.click_cursor(row);
    test.click_cursor(row);
    test.sync_and_update();
    assert_eq!(
        heard(&log),
        [
            Heard::Chose(ChangeList::Unstaged, 1),
            Heard::Intent(ListIntent::Double(ChangeList::Unstaged, 1)),
        ]
    );
}

/// R8.2: each list's button acts on its selection — dimmed with nothing selected — and, with
/// the table's Stage All press held, reads Stage All or Unstage All and acts on the whole list;
/// the double chevron over Unstaged is Stage All. Caught by: a button that takes focus or acts
/// with nothing selected, a held press not heard, or the chevron missing.
#[test]
fn the_buttons_stage_and_unstage_the_selection_or_with_the_press_held_everything() {
    let (mut test, fixture, log) = launch(lists());
    // Nothing selected: the buttons report nothing.
    test.click_cursor(at(&test, STAGE_CAPTION));
    test.click_cursor(at(&test, UNSTAGE_CAPTION));
    test.sync_and_update();
    assert_eq!(heard(&log), [], "a button acted with nothing selected");

    let mut selection = fixture.selection;
    test.run_in(|| {
        selection.set(ListSelection::of(
            ChangeList::Unstaged,
            RepoPath::from("b.rs"),
        ))
    });
    test.sync_and_update();
    test.click_cursor(at(&test, STAGE_CAPTION));
    test.sync_and_update();
    hold(&mut test, fixture, Action::StageOrUnstageAll);
    let drawn = texts(&test);
    assert!(
        drawn.iter().any(|t| t == STAGE_ALL_CAPTION)
            && drawn.iter().any(|t| t == UNSTAGE_ALL_CAPTION),
        "the held press did not make the buttons Stage All and Unstage All: {drawn:?}"
    );
    test.click_cursor(at(&test, UNSTAGE_ALL_CAPTION));
    test.sync_and_update();
    let_go(&mut test, fixture);
    // The double chevron, beside Unstaged's button.
    let (x, y) = at(&test, STAGE_CAPTION);
    test.click_cursor((x - 20., y));
    test.sync_and_update();
    use ChangeList::{Staged as S, Unstaged as U};
    assert_eq!(
        heard(&log),
        [
            Heard::Intent(ListIntent::Act(U, Action::StageOrUnstage)),
            Heard::Intent(ListIntent::Act(S, Action::StageOrUnstageAll)),
            Heard::Intent(ListIntent::Act(U, Action::StageOrUnstageAll)),
        ]
    );
}

/// R8.2, R7.6, C18: a press on a row dragged past the threshold and released over the other
/// list drops the path it began on, whatever happened to its row meanwhile — a status that no
/// longer lists it, rows unmounted — and its release chooses nothing; released over its own
/// list, outside both, or after Escape, nothing drops. Caught by: a drag kept by its row
/// (lost once the row goes), a drop on the list it came from, a drop after Escape, or the
/// release choosing the row under it.
#[test]
fn a_drag_drops_on_the_other_list_only_and_survives_its_row() {
    let (mut test, fixture, log) = launch(lists());
    let staged_heading = at(&test, STAGED_CAPTION);
    let into_staged = (
        staged_heading.0 + 40.,
        staged_heading.1 + DETAIL_ROW_HEIGHT as f64 * 2.,
    );
    let drag = |test: &mut TestingRunner, from: (f64, f64), to: (f64, f64)| {
        test.press_cursor(from);
        test.move_cursor((from.0, from.1 + 10.));
        test.sync_and_update();
        test.move_cursor(to);
        test.sync_and_update();
    };
    let from = at(&test, "b.rs");
    // Its row unmounted mid-drag: the status replaced by one without b.rs.
    drag(&mut test, from, into_staged);
    let mut changes = fixture.changes;
    test.run_in(|| {
        changes.set(LocalChanges::new(WorkingTreeStatus::Listed(vec![
            changed("a.rs", None, Some(UnstagedChange::Modified)),
            changed("s.rs", Some(StagedChange::Modified), None),
            changed("t.rs", Some(StagedChange::Added), None),
        ])))
    });
    test.sync_and_update();
    test.release_cursor(into_staged);
    test.sync_and_update();
    assert_eq!(
        heard(&log),
        [Heard::Intent(ListIntent::Drop {
            from: ChangeList::Unstaged,
            path: RepoPath::from("b.rs"),
        })]
    );
    log.borrow_mut().clear();

    // Released over its own list, outside both lists, or after Escape: nothing.
    let a = at(&test, "a.rs");
    drag(&mut test, a, (a.0, a.1 + 2. * DETAIL_ROW_HEIGHT as f64));
    test.release_cursor((a.0, a.1 + 2. * DETAIL_ROW_HEIGHT as f64));
    test.sync_and_update();
    // Over Staged first, then released outside both lists, over the filter (QA item 9).
    let filter = (100., 15.);
    drag(&mut test, a, into_staged);
    test.move_cursor(filter);
    test.sync_and_update();
    test.release_cursor(filter);
    test.sync_and_update();
    drag(&mut test, a, into_staged);
    test.press_key(Key::Named(NamedKey::Escape));
    test.sync_and_update();
    test.release_cursor(into_staged);
    test.sync_and_update();
    assert!(
        heard(&log)
            .iter()
            .all(|heard| !matches!(heard, Heard::Intent(ListIntent::Drop { .. }))),
        "a drag dropped where it may not: {:?}",
        heard(&log)
    );
}

/// R7.6, C18: a drag from Staged held at Unstaged's bottom edge scrolls Unstaged — the list it
/// would drop on — while the pointer stands still, its first row long unmounted, and still
/// drops there. Caught by: no scroll during a drag between the lists, or the drag lost as the
/// rows under it are replaced.
#[test]
fn a_drag_between_the_lists_scrolls_the_one_it_would_drop_on() {
    let entries: Vec<StatusEntry> = (0..400)
        .map(|n| StatusEntry::Untracked(RepoPath::from(format!("u{n:03}.rs").as_str())))
        .chain([changed("s.rs", Some(StagedChange::Modified), None)])
        .collect();
    let (mut test, _, log) = launch(LocalChanges::new(WorkingTreeStatus::Listed(entries)));
    let staged_heading = at(&test, STAGED_CAPTION);
    let from = at(&test, "s.rs");
    // Unstaged's bottom band: just above Staged's heading.
    let edge = (from.0, staged_heading.1 - DETAIL_ROW_HEIGHT as f64);
    test.press_cursor(from);
    test.move_cursor((from.0, from.1 - 10.));
    test.sync_and_update();
    test.move_cursor(edge);
    test.sync_and_update();
    test.poll(Duration::from_millis(5), Duration::from_millis(200));
    assert!(
        !texts(&test).iter().any(|t| t == "u000.rs"),
        "Unstaged did not scroll at its edge during the drag"
    );
    test.release_cursor(edge);
    test.sync_and_update();
    assert_eq!(
        heard(&log),
        [Heard::Intent(ListIntent::Drop {
            from: ChangeList::Staged,
            path: RepoPath::from("s.rs"),
        })]
    );
}

/// R8.2, R8.4, R8.7, R8.8, C8, C24: an unstaged row's menu offers Stage, Discard Changes…,
/// Stage All and Copy Path, each closing the menu as it is chosen; a submodule's and a
/// conflicted row's offer Stage and their Discard disabled, saying why; a staged row's offers
/// Unstage, Unstage All and Copy Path and no discard at all. A right-press on an unselected row
/// chooses it first. Caught by: a discard offered on Staged, for a submodule or a conflict, a
/// reason missing, or a menu left open after its item was chosen.
#[test]
fn each_rows_menu_offers_forks_items_and_no_discard_where_none_is_allowed() {
    let (mut test, _, log) = launch(lists());
    let row = at(&test, "a.rs");
    right_click(&mut test, row);
    let drawn = texts(&test);
    for item in [
        STAGE_CAPTION,
        DISCARD_CAPTION,
        STAGE_ALL_CAPTION,
        COPY_PATH_CAPTION,
    ] {
        assert!(
            drawn.iter().any(|t| t == item),
            "{item} not offered: {drawn:?}"
        );
    }
    let discard = labels(&test)
        .into_iter()
        .rfind(|(text, ..)| text == DISCARD_CAPTION)
        .map(|(_, x, y)| (f64::from(x) + 5., f64::from(y)))
        .expect("Discard Changes… in the menu");
    test.click_cursor(discard);
    test.sync_and_update();
    assert!(
        !texts(&test).iter().any(|t| t == COPY_PATH_CAPTION),
        "the menu stayed open after its item was chosen"
    );
    assert_eq!(
        heard(&log),
        [
            Heard::Chose(ChangeList::Unstaged, 0),
            Heard::Intent(ListIntent::Act(ChangeList::Unstaged, Action::Discard)),
        ]
    );

    for (row, why) in [("sub", "is a submodule"), ("clash.rs", "has a conflict")] {
        log.borrow_mut().clear();
        let pressed = at(&test, row);
        right_click(&mut test, pressed);
        let drawn = texts(&test);
        assert!(
            drawn.iter().any(|t| t.contains(why)),
            "{row}'s menu does not say why it offers no discard: {drawn:?}"
        );
        // The disabled Discard reports nothing when pressed.
        let disabled = labels(&test)
            .into_iter()
            .rfind(|(text, ..)| text == DISCARD_CAPTION)
            .map(|(_, x, y)| (f64::from(x) + 5., f64::from(y)))
            .expect("the disabled Discard Changes…");
        test.click_cursor(disabled);
        test.sync_and_update();
        assert!(
            heard(&log)
                .iter()
                .all(|heard| !matches!(heard, Heard::Intent(ListIntent::Act(_, Action::Discard)))),
            "{row}'s disabled discard acted"
        );
        test.press_key(Key::Named(NamedKey::Escape));
        test.sync_and_update();
    }

    log.borrow_mut().clear();
    let row = at(&test, "s.rs");
    right_click(&mut test, row);
    let drawn = texts(&test);
    assert!(drawn.iter().any(|t| t == UNSTAGE_CAPTION));
    assert!(drawn.iter().any(|t| t == UNSTAGE_ALL_CAPTION));
    assert!(
        !drawn.iter().any(|t| t == DISCARD_CAPTION),
        "a staged row offers a discard: {drawn:?}"
    );
    let copy = at(&test, COPY_PATH_CAPTION);
    test.click_cursor(copy);
    test.sync_and_update();
    assert_eq!(
        heard(&log),
        [
            Heard::Chose(ChangeList::Staged, 0),
            Heard::Intent(ListIntent::CopyPaths(ChangeList::Staged)),
        ]
    );
    // Both headings are still drawn under it all.
    assert!(texts(&test).iter().any(|t| t == UNSTAGED_CAPTION));
}

/// Phase 07's QA item 8: a drag ends without a drop when a press is heard while it is on — its
/// own release was made where the window could not hear it — and when the window loses focus;
/// the release over the other list after either drops nothing. Caught by: a drag kept through
/// a press, or through focus lost, which drops on the next release over the other list.
#[test]
fn a_press_heard_mid_drag_or_focus_lost_ends_the_drag_without_a_drop() {
    let (mut test, _, log) = launch(lists());
    let staged_heading = at(&test, STAGED_CAPTION);
    let into_staged = (
        staged_heading.0 + 40.,
        staged_heading.1 + DETAIL_ROW_HEIGHT as f64 * 2.,
    );
    let start = |test: &mut TestingRunner| {
        let from = at(test, "a.rs");
        test.press_cursor(from);
        test.move_cursor((from.0, from.1 + 10.));
        test.sync_and_update();
        test.move_cursor(into_staged);
        test.sync_and_update();
    };
    // A press outside every row while the drag is on: its release was lost.
    start(&mut test);
    test.press_cursor((100., 15.));
    test.sync_and_update();
    test.move_cursor(into_staged);
    test.sync_and_update();
    test.release_cursor(into_staged);
    test.sync_and_update();
    // The window losing focus mid-drag.
    start(&mut test);
    test.run_in(|| Platform::get().is_app_focused.set(false));
    // The effect that hears focus runs on the frame after the next.
    test.sync_and_update();
    test.sync_and_update();
    test.run_in(|| Platform::get().is_app_focused.set(true));
    test.sync_and_update();
    test.release_cursor(into_staged);
    test.sync_and_update();
    assert!(
        heard(&log)
            .iter()
            .all(|heard| !matches!(heard, Heard::Intent(ListIntent::Drop { .. }))),
        "a drag dropped after it ended: {:?}",
        heard(&log)
    );
}
