//! Headless tests of the toolbar: Fork's buttons in Fork's order, the unbuilt ones greyed with
//! the sentence that names what brings them and reporting nothing however they are pressed, Fetch
//! and Branch acting, Branch's ⌄ a menu of New Branch… alone, and the commits ahead and behind
//! as badges on Push and Pull.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use cairn_model::{AheadBehind, HeadState, RefName};
use cairn_ui::accelerators::{self, Action, Os};
use cairn_ui::style::tones;
use cairn_ui::{
    ACTIVITY_GLYPH_NAME, APPEARANCE_CAPTION, BRANCH_CAPTION, BRANCH_MENU_NAME,
    COMES_WITH_PREFERENCES, COMES_WITH_PULL_AND_PUSH, COMES_WITH_QUICK_LAUNCH, COMES_WITH_STASHING,
    FETCH_CAPTION, NEW_BRANCH_CAPTION, PULL_CAPTION, PUSH_CAPTION, QUICK_LAUNCH_CAPTION,
    SIDEBAR_TOGGLE_NAME, STASH_CAPTION, STASH_MENU_NAME, StatusBox, Toolbar, ToolbarPress,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 1024.;
const HEIGHT: f32 = 300.;

type Pressed = Rc<RefCell<Vec<ToolbarPress>>>;

fn status() -> StatusBox {
    StatusBox::new(
        Some("t".to_owned()),
        Some(HeadState::Branch(RefName::new("refs/heads/main"))),
    )
}

/// The toolbar at the window's top, `make` building it, with the context-menu host the window
/// keeps and a global key listener; every press it reports is kept.
fn launch(make: impl Fn() -> Toolbar + 'static) -> (TestingRunner, Pressed) {
    let pressed = Pressed::default();
    let hearing = pressed.clone();
    let (mut test, ()) = TestingRunner::new(
        move || {
            let hearing = hearing.clone();
            rect()
                .expanded()
                .child(make().on_press(move |press: ToolbarPress| {
                    hearing.borrow_mut().push(press);
                }))
                .child(ContextMenuViewer::new())
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    (test, pressed)
}

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|label| label.text.to_string()))
}

fn label_area(test: &TestingRunner, text: &str) -> Area {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| node.layout().area)
    })
    .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)))
}

/// The colour the label reading `text` is drawn in.
fn label_colour(test: &TestingRunner, text: &str) -> Option<Color> {
    test.find(|_, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|label| {
                label
                    .text_style_data
                    .color
                    .as_ref()
                    .and_then(Fill::as_color)
            })
    })
    .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)))
}

/// Where the button assistive technology reads as `name` is laid out.
fn named(test: &TestingRunner, name: &str) -> Area {
    test.find(|node, element| {
        Rect::try_downcast(element)
            .filter(|rect| rect.accessibility.builder.value() == Some(name))
            .map(|_| node.layout().area)
    })
    .unwrap_or_else(|| panic!("no button is named {name:?}"))
}

fn click(test: &mut TestingRunner, at: Area) {
    let centre = at.center();
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.sync_and_update();
}

/// Whether a button under `at` lights its hover box while the pointer rests on it: a rect
/// over `at`'s centre in the hover tone.
fn lit_when_hovered(test: &mut TestingRunner, at: Area) -> bool {
    let centre = at.center();
    test.move_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.sync_and_update();
    let lit = test
        .find(|node, element| {
            Rect::try_downcast(element)
                .filter(|_| node.layout().area.contains(centre))
                .and_then(|rect| rect.style.background.as_color())
                .filter(|tone| *tone == tones::MENU_HOVER)
        })
        .is_some();
    test.move_cursor((f64::from(WIDTH - 2.), f64::from(HEIGHT - 2.)));
    test.sync_and_update();
    lit
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

/// Fork's buttons left to right — the sidebar toggle, Quick Launch, Fetch, Pull, Push, Stash and
/// its ⌄, the status box, Branch and its ⌄, Appearance — and none of Fork's Open in, Console or
/// Work; each greyed one's tooltip its sentence, verbatim, and no press or key on it reporting
/// anything; Fetch's tooltip its name over its chord from the table, Branch's none; Fetch,
/// Branch, the sidebar toggle and the status box's glyph each reporting their press; Branch's ⌄
/// a menu of New Branch… alone, with its chord. Caught by: a button out of Fork's order, an
/// unplanned button drawn, a greyed button that acts, says another sentence or is drawn in the
/// enabled tone or lighting its hover box, a chord typed rather than read from the table, or New
/// Worktree… offered.
#[test]
fn the_toolbar_is_forks_with_the_unbuilt_buttons_greyed() {
    let (mut test, pressed) = launch(|| Toolbar::new(status()));
    let order = [
        named(&test, SIDEBAR_TOGGLE_NAME),
        label_area(&test, QUICK_LAUNCH_CAPTION),
        label_area(&test, FETCH_CAPTION),
        label_area(&test, PULL_CAPTION),
        label_area(&test, PUSH_CAPTION),
        label_area(&test, STASH_CAPTION),
        named(&test, STASH_MENU_NAME),
        label_area(&test, "t"),
        label_area(&test, BRANCH_CAPTION),
        named(&test, BRANCH_MENU_NAME),
        label_area(&test, APPEARANCE_CAPTION),
    ];
    for pair in order.windows(2) {
        assert!(
            pair[0].max_x() <= pair[1].min_x() + 0.5,
            "out of Fork's order: {:?} then {:?}",
            pair[0],
            pair[1]
        );
    }
    let shown = labels(&test);
    for absent in ["Open in", "Console", "Work"] {
        assert!(
            !shown.iter().any(|text| text == absent),
            "{absent} is drawn: {shown:?}"
        );
    }
    // Every label fits the toolbar's band.
    for at in &order {
        assert!(at.max_y() <= 41. + 0.5 && at.max_x() <= WIDTH, "{at:?}");
    }

    let greyed = [
        (
            label_area(&test, QUICK_LAUNCH_CAPTION),
            COMES_WITH_QUICK_LAUNCH,
        ),
        (label_area(&test, PULL_CAPTION), COMES_WITH_PULL_AND_PUSH),
        (label_area(&test, PUSH_CAPTION), COMES_WITH_PULL_AND_PUSH),
        (label_area(&test, STASH_CAPTION), COMES_WITH_STASHING),
        (named(&test, STASH_MENU_NAME), COMES_WITH_STASHING),
        (
            label_area(&test, APPEARANCE_CAPTION),
            COMES_WITH_PREFERENCES,
        ),
    ];
    // A greyed button is drawn disabled — its caption in the disabled tone — where an enabled
    // one's caption is the toolbar's.
    for caption in [
        QUICK_LAUNCH_CAPTION,
        PULL_CAPTION,
        PUSH_CAPTION,
        STASH_CAPTION,
        APPEARANCE_CAPTION,
    ] {
        assert_eq!(
            label_colour(&test, caption),
            Some(tones::DISABLED_TEXT),
            "{caption} is not drawn disabled"
        );
    }
    assert_eq!(
        label_colour(&test, FETCH_CAPTION),
        Some(tones::TOOLBAR_TEXT),
        "Fetch is drawn disabled"
    );
    let fetch = label_area(&test, FETCH_CAPTION);
    assert!(
        lit_when_hovered(&mut test, fetch),
        "Fetch does not light its hover box"
    );
    for (at, sentence) in greyed {
        assert!(
            !lit_when_hovered(&mut test, at),
            "a greyed button lit its hover box at {at:?}"
        );
        assert!(
            hovered(&mut test, at).iter().any(|text| text == sentence),
            "no tooltip reads {sentence:?} over {at:?}"
        );
        click(&mut test, at);
        for key in [Key::Named(NamedKey::Enter), Key::Character(" ".into())] {
            test.press_key(key);
            test.sync_and_update();
        }
        assert!(
            pressed.borrow().is_empty(),
            "a greyed button reported {:?}",
            pressed.borrow()
        );
        assert!(
            !labels(&test).iter().any(|text| text == NEW_BRANCH_CAPTION),
            "a greyed button opened a menu"
        );
    }

    let chord = accelerators::chord_name(Action::Fetch, Os::current())
        .unwrap_or_else(|| panic!("Fetch has a chord"));
    let at = label_area(&test, FETCH_CAPTION);
    let tip = hovered(&mut test, at);
    assert!(
        tip.iter().filter(|text| *text == FETCH_CAPTION).count() >= 2 && tip.contains(&chord),
        "Fetch's tooltip is not its name over {chord:?}: {tip:?}"
    );
    let before = labels(&test).len();
    let at = label_area(&test, BRANCH_CAPTION);
    let after = hovered(&mut test, at).len();
    assert_eq!(before, after, "Branch has a tooltip");

    for (at, press) in [
        (label_area(&test, FETCH_CAPTION), ToolbarPress::Fetch),
        (label_area(&test, BRANCH_CAPTION), ToolbarPress::NewBranch),
        (
            named(&test, SIDEBAR_TOGGLE_NAME),
            ToolbarPress::ToggleSidebar,
        ),
        (named(&test, ACTIVITY_GLYPH_NAME), ToolbarPress::Activity),
    ] {
        click(&mut test, at);
        assert_eq!(pressed.borrow_mut().drain(..).collect::<Vec<_>>(), [press]);
    }
    // The status box's centre opens nothing.
    let at = label_area(&test, "t");
    click(&mut test, at);
    assert!(pressed.borrow().is_empty(), "{:?}", pressed.borrow());

    let at = named(&test, BRANCH_MENU_NAME);

    click(&mut test, at);
    let menu = labels(&test);
    assert!(
        menu.iter().any(|text| text == NEW_BRANCH_CAPTION),
        "{menu:?}"
    );
    assert!(!menu.iter().any(|text| text.starts_with("New Worktree")));
    let new_branch_chord = accelerators::chord_name(Action::NewBranch, Os::current())
        .unwrap_or_else(|| panic!("New Branch has a chord"));
    assert!(menu.contains(&new_branch_chord), "{menu:?}");
    test.press_key(Key::Named(NamedKey::Enter));
    test.sync_and_update();
    assert_eq!(
        pressed.borrow_mut().drain(..).collect::<Vec<_>>(),
        [ToolbarPress::NewBranch]
    );
}

/// Fetch and Branch greyed with the reasons they are handed, their tooltips those reasons, and
/// neither reporting a press; Branch's ⌄ greyed with it. Caught by: a refused Fetch that still
/// fetches, or a reason not shown.
#[test]
fn fetch_and_branch_are_greyed_with_their_reasons() {
    let (mut test, pressed) = launch(|| {
        Toolbar::new(status())
            .fetch_refusal(Some("A fetch is running.".to_owned()))
            .branch_refusal(Some("The refs have not been read yet.".to_owned()))
    });
    for (at, reason) in [
        (label_area(&test, FETCH_CAPTION), "A fetch is running."),
        (
            label_area(&test, BRANCH_CAPTION),
            "The refs have not been read yet.",
        ),
    ] {
        assert!(
            hovered(&mut test, at).iter().any(|text| text == reason),
            "no tooltip reads {reason:?}"
        );
        click(&mut test, at);
    }
    let at = named(&test, BRANCH_MENU_NAME);
    click(&mut test, at);
    assert!(pressed.borrow().is_empty(), "{:?}", pressed.borrow());
    assert!(
        !labels(&test).iter().any(|text| text == NEW_BRANCH_CAPTION),
        "a greyed ⌄ opened its menu"
    );
}

/// The commits ahead are a badge on Push's icon and those behind on Pull's, absent at zero and
/// when nothing is told, drawn though both buttons are greyed. Caught by: the two swapped, a zero
/// drawn, or a badge outside its button.
#[test]
fn ahead_and_behind_are_badges_on_push_and_pull() {
    let counted = |ahead, behind| Some(AheadBehind { ahead, behind });
    let (test, _) = launch(move || Toolbar::new(status()).counts(counted(1, 18)));
    for (count, button) in [("1", PUSH_CAPTION), ("18", PULL_CAPTION)] {
        let (badge, caption) = (label_area(&test, count), label_area(&test, button));
        assert!(
            badge.max_y() <= caption.min_y()
                && badge.min_x() >= caption.min_x() - 10.
                && badge.max_x() <= caption.max_x() + 20.,
            "{count:?} is not over {button}: {badge:?} {caption:?}"
        );
    }

    let digits = |test: &TestingRunner| {
        labels(test)
            .into_iter()
            .filter(|text| text.chars().all(|c| c.is_ascii_digit()))
            .collect::<Vec<_>>()
    };
    let (test, _) = launch(move || Toolbar::new(status()).counts(counted(0, 4)));
    assert_eq!(
        digits(&test),
        ["4"],
        "a zero is drawn, or the four is missing"
    );
    let push = label_area(&test, PUSH_CAPTION);
    let four = label_area(&test, "4");
    assert!(four.max_x() < push.min_x(), "the behind count is on Push");

    let (test, _) = launch(move || Toolbar::new(status()).counts(counted(0, 0)));
    assert!(digits(&test).is_empty(), "{:?}", digits(&test));
    let (test, _) = launch(move || Toolbar::new(status()));
    assert!(digits(&test).is_empty(), "{:?}", digits(&test));
}

/// The words the greyed buttons and the pane's icon say are the spec's and Fork's, spelled here
/// apart from the constants the views draw. Caught by: a sentence reworded in the code alone.
#[test]
fn the_greyed_sentences_and_the_icons_name_are_the_specs_words() {
    assert_eq!(COMES_WITH_QUICK_LAUNCH, "Comes with Quick Launch.");
    assert_eq!(COMES_WITH_PULL_AND_PUSH, "Comes with pull and push.");
    assert_eq!(COMES_WITH_STASHING, "Comes with stashing.");
    assert_eq!(COMES_WITH_PREFERENCES, "Comes with Preferences.");
    assert_eq!(cairn_ui::HIDE_DETAILS_TOOLTIP, "Hide commit details");
}
