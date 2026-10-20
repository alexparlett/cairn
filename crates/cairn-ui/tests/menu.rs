//! Headless tests for the one menu component (`cairn_ui::menu`): a context menu and the menu
//! bar drawn from data, walked by the keys and the pointer as Fork's are.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use cairn_ui::accelerators::{self, Action, Os};
use cairn_ui::menu::{BarMenu, Groups, Item, MenuBar, context_menu};
use cairn_ui::style::{metrics, tones};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 800.;
const HEIGHT: f32 = 600.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Alpha,
    Bravo,
    Charlie,
    Echo,
    Exit,
    Show,
    About,
}

const BRAVO_REASON: &str = "Bravo can't be chosen now.";

/// Alpha (Refresh's chord), Bravo greyed — Charlie, ticked — Delta, a submenu holding Echo.
fn groups(tick: bool) -> Groups<Choice> {
    vec![
        vec![
            Item::new("Alpha", Choice::Alpha).chord(Action::Refresh),
            Item::new("Bravo", Choice::Bravo).greyed(Some(BRAVO_REASON.to_owned())),
        ],
        vec![Item::new("Charlie", Choice::Charlie).ticked(tick)],
        vec![Item::submenu(
            "Delta",
            vec![vec![Item::new("Echo", Choice::Echo)]],
        )],
    ]
}

/// What the test heard: each choice, and every key that reached the window behind the menu.
#[derive(Clone, Default)]
struct Heard {
    chosen: Rc<RefCell<Vec<Choice>>>,
    keys: Rc<RefCell<Vec<Key>>>,
}

/// A window with a strip at the top that opens a context menu over `groups(tick)` when pressed,
/// and a global key listener standing for the window's chords.
fn launch_context(tick: bool) -> (TestingRunner, Heard) {
    let heard = Heard::default();
    let hearing = heard.clone();
    let (mut test, ()) = TestingRunner::new(
        move || {
            let chosen = hearing.chosen.clone();
            let keys = hearing.keys.clone();
            rect()
                .expanded()
                .on_global_key_down(move |e: Event<KeyboardEventData>| {
                    keys.borrow_mut().push(e.key.clone());
                })
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(40.))
                        .on_press(move |_| {
                            let chosen = chosen.clone();
                            ContextMenu::open(context_menu(groups(tick), move |choice: Choice| {
                                chosen.borrow_mut().push(choice);
                            }));
                        }),
                )
                .child(ContextMenuViewer::new())
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    (test, heard)
}

fn open_context(test: &mut TestingRunner) {
    test.click_cursor((100., 20.));
    test.sync_and_update();
    test.sync_and_update();
}

/// Each label drawn: its text, its left, its vertical centre and its colour.
fn labels(test: &TestingRunner) -> Vec<(String, f32, f32, Option<Fill>)> {
    test.find_many(|node, element| {
        Label::try_downcast(element).map(|label| {
            let area = node.layout().area;
            (
                label.text.to_string(),
                area.min_x(),
                area.center().y,
                label.text_style_data.color.clone(),
            )
        })
    })
}

fn texts(test: &TestingRunner) -> Vec<String> {
    labels(test).into_iter().map(|(text, ..)| text).collect()
}

fn at(test: &TestingRunner, text: &str) -> (f64, f64) {
    labels(test)
        .into_iter()
        .find(|(drawn, ..)| drawn == text)
        .map(|(_, x, y, _)| (f64::from(x) + 5., f64::from(y)))
        .unwrap_or_else(|| panic!("{text} is not drawn: {:?}", texts(test)))
}

/// The labels drawn in the highlighted row's white.
fn highlighted(test: &TestingRunner) -> Vec<String> {
    labels(test)
        .into_iter()
        .filter(|(.., colour)| *colour == Some(Fill::Color(tones::SELECTED_TEXT)))
        .map(|(text, ..)| text)
        .collect()
}

fn press(test: &mut TestingRunner, named: NamedKey) {
    test.press_key(Key::Named(named));
    test.sync_and_update();
}

/// The rules drawn between groups.
fn separators(test: &TestingRunner) -> usize {
    test.find_many(|node, element| {
        Rect::try_downcast(element)
            .filter(|rect| {
                rect.style.background == Fill::Color(tones::MENU_SEPARATOR)
                    && node.layout().visible_area().height() == 1.
            })
            .map(|_| ())
    })
    .len()
}

/// A context menu draws its groups parted by a rule, each chord read from the accelerator
/// table and right-aligned, a greyed item in the disabled tone with its reason drawn while the
/// pointer is on it, and a ticked item's ✓ in a gutter that moves every label of the menu right
/// by Fork's 18 px; the menu as wide as its widest row; a press on a greyed item chooses nothing
/// and leaves the menu open. Caught by: a chord typed rather than read from the table, groups
/// run together, a rule stretching the menu across the window, a greyed item that does not say
/// why or acts, or a gutter opened for the ticked item alone.
#[test]
fn a_menu_is_drawn_from_its_data() {
    let (mut test, heard) = launch_context(true);
    open_context(&mut test);
    let drawn = texts(&test);
    for label in ["Alpha", "Bravo", "Charlie", "Delta"] {
        assert!(drawn.iter().any(|t| t == label), "{label}: {drawn:?}");
    }
    assert_eq!(separators(&test), 2, "a rule between each two groups");
    let chord = accelerators::chord_name(Action::Refresh, Os::current())
        .unwrap_or_else(|| panic!("Refresh has a chord"));
    let (chord_x, chord_y) = at(&test, &chord);
    let (alpha_x, alpha_y) = at(&test, "Alpha");
    assert_eq!(chord_y, alpha_y, "the chord sits on its item's row");
    assert!(
        chord_x > alpha_x + 60.,
        "the chord is not right-aligned: {chord_x} against {alpha_x}"
    );
    // As wide as its widest row and no wider, rules and all: a rule that fills what the
    // window offers would stretch the menu across it.
    let widest = test
        .find_many(|node, element| {
            Rect::try_downcast(element)
                .filter(|rect| rect.style.background == Fill::Color(tones::MENU_SEPARATOR))
                .map(|_| node.layout().area.width())
        })
        .into_iter()
        .fold(0., f32::max);
    assert!(
        widest > 0. && widest < WIDTH / 2.,
        "a rule is {widest} px wide in an {WIDTH} px window"
    );
    assert!(drawn.iter().any(|t| t == "✓"), "Charlie's tick: {drawn:?}");
    let colour_of = |text: &str| {
        labels(&test)
            .into_iter()
            .find(|(drawn, ..)| drawn == text)
            .and_then(|(.., colour)| colour)
    };
    assert_eq!(colour_of("Bravo"), Some(Fill::Color(tones::DISABLED_TEXT)));
    assert!(!drawn.iter().any(|t| t == BRAVO_REASON));
    let ticked_x = at(&test, "Bravo").0;
    assert_eq!(
        at(&test, "Alpha").0,
        ticked_x,
        "every label moves with the gutter"
    );

    let bravo = at(&test, "Bravo");
    test.move_cursor(bravo);
    test.sync_and_update();
    assert!(
        texts(&test).iter().any(|t| t == BRAVO_REASON),
        "the greyed item does not say why: {:?}",
        texts(&test)
    );
    // The pointer gone from it, the reason goes too.
    let alpha = at(&test, "Alpha");
    test.move_cursor(alpha);
    test.sync_and_update();
    assert!(
        !texts(&test).iter().any(|t| t == BRAVO_REASON),
        "the reason stayed after the pointer left"
    );
    test.click_cursor(bravo);
    test.sync_and_update();
    assert!(heard.chosen.borrow().is_empty(), "a greyed item was chosen");
    assert!(texts(&test).iter().any(|t| t == "Alpha"), "the menu closed");
    press(&mut test, NamedKey::Escape);

    // The same menu with nothing ticked opens no gutter.
    let (mut test, _) = launch_context(false);
    open_context(&mut test);
    assert!(!texts(&test).iter().any(|t| t == "✓"));
    let unticked_x = at(&test, "Bravo").0;
    assert!(
        (ticked_x - unticked_x - f64::from(metrics::MENU_TICK_GUTTER)).abs() < 0.5,
        "the gutter moved the labels {} px",
        ticked_x - unticked_x
    );
}

/// A context menu opens with its first item highlighted; ↓ and ↑ walk the items that can be
/// chosen, skipping a greyed one and wrapping round; → opens a submenu with its first item
/// highlighted and ← closes it; the pointer highlights what it is over, one highlight with the
/// keys'; Return chooses and closes; Escape closes choosing nothing; and no key reaches the
/// window behind an open menu. Caught by: nothing highlighted on open, a greyed item walked
/// onto, a walk that stops at an end, a submenu → cannot open, two highlights at once, or a key
/// heard by the window's chords while a menu holds the keyboard.
#[test]
fn the_menus_walk_by_key_and_hover() {
    let (mut test, heard) = launch_context(false);
    open_context(&mut test);
    assert_eq!(highlighted(&test), ["Alpha"], "the first item on open");
    press(&mut test, NamedKey::ArrowDown);
    assert_eq!(
        highlighted(&test),
        ["Charlie"],
        "↓ steps over the greyed Bravo"
    );
    press(&mut test, NamedKey::ArrowDown);
    assert_eq!(highlighted(&test), ["Delta"]);
    press(&mut test, NamedKey::ArrowDown);
    assert_eq!(highlighted(&test), ["Alpha"], "↓ wraps round");
    press(&mut test, NamedKey::ArrowUp);
    assert_eq!(highlighted(&test), ["Delta"], "↑ wraps round");
    press(&mut test, NamedKey::ArrowRight);
    test.sync_and_update();
    assert!(texts(&test).iter().any(|t| t == "Echo"), "→ opened nothing");
    assert_eq!(highlighted(&test), ["Delta", "Echo"]);
    press(&mut test, NamedKey::ArrowLeft);
    test.sync_and_update();
    assert!(!texts(&test).iter().any(|t| t == "Echo"), "← left it open");
    assert_eq!(highlighted(&test), ["Delta"]);

    let charlie = at(&test, "Charlie");
    test.move_cursor(charlie);
    test.sync_and_update();
    assert_eq!(highlighted(&test), ["Charlie"], "the pointer's highlight");
    // A hover over the submenu's row opens it.
    let delta = at(&test, "Delta");
    test.move_cursor(delta);
    test.sync_and_update();
    assert!(
        texts(&test).iter().any(|t| t == "Echo"),
        "hover opened nothing"
    );
    test.move_cursor(charlie);
    test.sync_and_update();
    assert!(!texts(&test).iter().any(|t| t == "Echo"));
    press(&mut test, NamedKey::Enter);
    assert_eq!(heard.chosen.borrow().as_slice(), [Choice::Charlie]);
    assert!(
        !texts(&test).iter().any(|t| t == "Alpha"),
        "the menu stayed open"
    );
    assert!(
        heard.keys.borrow().is_empty(),
        "the window heard {:?} through the menu",
        heard.keys.borrow()
    );

    open_context(&mut test);
    press(&mut test, NamedKey::Escape);
    assert!(
        !texts(&test).iter().any(|t| t == "Alpha"),
        "Escape left it open"
    );
    assert_eq!(heard.chosen.borrow().as_slice(), [Choice::Charlie]);
}

/// A bar of File, View and Help over a strip that stands for the window's content.
fn launch_bar() -> (TestingRunner, Heard) {
    let heard = Heard::default();
    let hearing = heard.clone();
    let (mut test, ()) = TestingRunner::new(
        move || {
            let chosen = hearing.chosen.clone();
            let keys = hearing.keys.clone();
            rect()
                .expanded()
                .on_global_key_down(move |e: Event<KeyboardEventData>| {
                    keys.borrow_mut().push(e.key.clone());
                })
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(metrics::CAPTION_ROW))
                        .child(
                            MenuBar::new(vec![
                                BarMenu::new("File", vec![vec![Item::new("Exit", Choice::Exit)]]),
                                BarMenu::new(
                                    "View",
                                    vec![
                                        vec![Item::new("Show", Choice::Show)],
                                        vec![Item::new("Bravo", Choice::Bravo)],
                                    ],
                                ),
                                BarMenu::new("Help", vec![vec![Item::new("About", Choice::About)]]),
                            ])
                            .on_choose(move |choice: Choice| chosen.borrow_mut().push(choice)),
                        ),
                )
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    (test, heard)
}

fn shown(test: &TestingRunner) -> Option<&'static str> {
    let drawn = texts(test);
    [("Exit", "File"), ("Show", "View"), ("About", "Help")]
        .into_iter()
        .find(|(item, _)| drawn.iter().any(|t| t == item))
        .map(|(_, menu)| menu)
}

/// The menu bar is Fork's: a press on a name opens its menu with nothing highlighted, ↓
/// highlights the first item, → and ← move to the menu beside it and wrap round, the pointer
/// over another name while one is open opens that one, a press on the open menu's name closes
/// it, a press elsewhere closes it, and a choice is reported and closes the menu. Caught by: a
/// first item highlighted on a press, a menu that cannot be left by the arrows or the pointer,
/// or a press on the open name that opens it again.
#[test]
fn the_menu_bar_opens_moves_and_closes_as_forks() {
    let (mut test, heard) = launch_bar();
    assert_eq!(shown(&test), None);
    let view = at(&test, "View");
    test.click_cursor(view);
    test.sync_and_update();
    assert_eq!(shown(&test), Some("View"));
    assert!(highlighted(&test).is_empty(), "highlighted on open");
    press(&mut test, NamedKey::ArrowDown);
    assert_eq!(highlighted(&test), ["Show"]);
    press(&mut test, NamedKey::ArrowRight);
    test.sync_and_update();
    assert_eq!(shown(&test), Some("Help"));
    assert!(highlighted(&test).is_empty(), "highlighted after a move");
    press(&mut test, NamedKey::ArrowRight);
    test.sync_and_update();
    assert_eq!(shown(&test), Some("File"), "→ wraps round");
    press(&mut test, NamedKey::ArrowLeft);
    test.sync_and_update();
    assert_eq!(shown(&test), Some("Help"), "← wraps round");

    test.move_cursor(view);
    test.sync_and_update();
    assert_eq!(shown(&test), Some("View"), "the pointer switches menus");
    test.click_cursor(view);
    test.sync_and_update();
    assert_eq!(shown(&test), None, "a press on the open name closes it");

    let file = at(&test, "File");
    test.click_cursor(file);
    test.sync_and_update();
    assert_eq!(shown(&test), Some("File"));
    test.click_cursor((600., 400.));
    test.sync_and_update();
    assert_eq!(shown(&test), None, "a press outside closes it");

    // Escape closes the open menu, choosing nothing; ↓ and Return choose from it.
    test.click_cursor(view);
    test.sync_and_update();
    press(&mut test, NamedKey::Escape);
    assert_eq!(shown(&test), None, "Escape left the menu open");
    test.click_cursor(view);
    test.sync_and_update();
    press(&mut test, NamedKey::ArrowDown);
    press(&mut test, NamedKey::Enter);
    assert_eq!(heard.chosen.borrow().as_slice(), [Choice::Show]);
    assert_eq!(shown(&test), None, "Return left the menu open");
    assert!(
        heard.keys.borrow().is_empty(),
        "the window heard {:?} through the bar's menu",
        heard.keys.borrow()
    );
    heard.chosen.borrow_mut().clear();

    // With none open, the pointer over a name opens nothing.
    let help = at(&test, "Help");
    test.move_cursor(help);
    test.sync_and_update();
    assert_eq!(shown(&test), None);
    test.click_cursor(help);
    test.sync_and_update();
    let about = at(&test, "About");
    test.click_cursor(about);
    test.sync_and_update();
    assert_eq!(heard.chosen.borrow().as_slice(), [Choice::About]);
    assert_eq!(shown(&test), None, "a choice closes the menu");
    test.poll(Duration::from_millis(10), Duration::from_millis(20));
}

/// A context menu is as wide as its widest row, chord and all: a long chord stands inside the
/// menu, right-aligned on its row, and the rule spans the same width. Caught by: a chord drawn
/// past the menu's edge (a row laid out wider than the menu that holds it), or a rule as wide as
/// the window.
#[test]
fn a_long_chord_stands_inside_the_menu() {
    let (mut test, ()) = TestingRunner::new(
        || {
            rect()
                .expanded()
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(40.))
                        .on_press(|_| {
                            ContextMenu::open(context_menu(
                                vec![
                                    vec![
                                        Item::new("Stage All", Choice::Alpha)
                                            .chord(Action::StageOrUnstageAll),
                                    ],
                                    vec![Item::new("Copy", Choice::Bravo)],
                                ],
                                |_: Choice| {},
                            ));
                        }),
                )
                .child(ContextMenuViewer::new())
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    open_context(&mut test);
    let chord = accelerators::chord_name(Action::StageOrUnstageAll, Os::current())
        .unwrap_or_else(|| panic!("Stage All has a chord"));
    // The label reading `text` in `colour`: a copy of the chord holds its room unseen.
    let area_of = |text: &str, colour: Color| {
        test.find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| {
                    label.text == text && label.text_style_data.color == Some(Fill::Color(colour))
                })
                .map(|_| node.layout().area)
        })
        .unwrap_or_else(|| panic!("{text} is not drawn: {:?}", texts(&test)))
    };
    let rule = test
        .find(|node, element| {
            Rect::try_downcast(element)
                .filter(|rect| rect.style.background == Fill::Color(tones::MENU_SEPARATOR))
                .map(|_| node.layout().visible_area())
        })
        .unwrap_or_else(|| panic!("no rule is drawn"));
    let caption = area_of("Stage All", tones::SELECTED_TEXT);
    let chord = area_of(&chord, tones::MENU_CHORD);
    assert!(
        (chord.max_x() - (rule.max_x() - metrics::MENU_INSET)).abs() < 0.5,
        "the chord ends at {}, not at the menu's {} less its inset",
        chord.max_x(),
        rule.max_x()
    );
    assert!(
        chord.min_x() >= caption.max_x() + metrics::MENU_CHORD_GAP - 0.5,
        "the chord crowds its caption"
    );
    assert!(
        rule.width() < WIDTH / 2.,
        "the rule is {} px wide",
        rule.width()
    );
}
