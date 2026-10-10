//! Headless tests for the confirmation dialog (staging-and-commit R7.4, C17): its words and
//! its button are the `Consequence`'s, focus starts on Cancel and stays inside it, Escape and
//! a press outside cancel, and one acknowledgement builds one token from the value drawn.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    Confirmed, Consequence, DiscardedFile, FileLoss, LineNumber, Oid, Patch, RepoPath, Selection,
};
use cairn_ui::{CANCEL_CAPTION, ConfirmDialog};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 800.;
const HEIGHT: f32 = 600.;
const TITLE: &str = "Discard Changes";

fn oid(byte: u8) -> Oid {
    Oid::from_bytes(&[byte; 20]).unwrap_or_else(|e| panic!("{e}"))
}

/// Three files: two modified, one untracked — Fork's "Discard Changes in 3 Files".
fn three_files() -> Consequence {
    let modified = |path: &str, lines| DiscardedFile {
        path: RepoPath::from(path),
        loss: FileLoss::Modified {
            index: oid(1),
            working_tree: Some(oid(2)),
            executable: false,
            lines: Some(lines),
            mode: None,
        },
    };
    Consequence::DiscardFiles {
        files: vec![
            modified("src/a.rs", 4),
            modified("src/b.rs", 10),
            DiscardedFile {
                path: RepoPath::from("notes.txt"),
                loss: FileLoss::Untracked {
                    working_tree: oid(3),
                    executable: false,
                    bytes: 2150,
                },
            },
        ],
    }
}

/// Two lines of one file — Fork's "Discard 2 Lines".
fn two_lines() -> Consequence {
    let mut selection = Selection::empty();
    selection.select_removed(LineNumber::from_index(0));
    selection.select_added(LineNumber::from_index(0));
    Consequence::DiscardLines {
        path: RepoPath::from("src/lib.rs"),
        index: Some(oid(1)),
        working_tree: oid(2),
        on_disk: oid(4),
        executable: false,
        selection,
        mode: None,
        chunk: false,
        patch: Patch::empty(),
    }
}

/// What the dialog and the view behind it reported, in order.
#[derive(Default, Clone)]
struct Reports {
    confirmed: Rc<RefCell<Vec<Confirmed>>>,
    cancelled: Rc<RefCell<usize>>,
    /// Keys the focusable view behind the dialog heard.
    behind: Rc<RefCell<usize>>,
}

fn launch(consequence: Consequence) -> (TestingRunner, Reports) {
    let reports = Reports::default();
    let (mut test, _) = TestingRunner::new(
        {
            let reports = reports.clone();
            move || {
                let confirmed = reports.confirmed.clone();
                let cancelled = reports.cancelled.clone();
                let behind = reports.behind.clone();
                rect()
                    .expanded()
                    .child(
                        // A focusable view behind the dialog, focused before it opens.
                        rect()
                            .width(Size::fill())
                            .height(Size::px(40.))
                            .a11y_focusable(true)
                            .a11y_auto_focus(true)
                            .on_key_down(move |_| *behind.borrow_mut() += 1),
                    )
                    .child(
                        ConfirmDialog::new(1, TITLE, Rc::new(consequence.clone()))
                            .on_confirm(move |token: Confirmed| confirmed.borrow_mut().push(token))
                            .on_cancel(move |()| *cancelled.borrow_mut() += 1),
                    )
            }
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    // The popup animates in; a few frames settle it and apply the focus.
    for _ in 0..4 {
        test.sync_and_update();
    }
    (test, reports)
}

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

fn centre_of(test: &TestingRunner, caption: &str) -> (f64, f64) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == caption)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no label reads {caption:?}; labels: {:?}", labels(test)));
    (f64::from(centre.x), f64::from(centre.y))
}

fn press(test: &mut TestingRunner, key: NamedKey) {
    test.press_key(Key::Named(key));
    test.sync_and_update();
    test.sync_and_update();
}

/// C17, L8: the prompt and the button are the `Consequence`'s own words — the files' count
/// and kinds, and the lines' count — never text typed beside it. Caught by: a dialog that
/// writes its own words, or a button that does not name the count.
#[test]
fn its_words_and_its_button_are_the_consequences() {
    for (consequence, button) in [
        (three_files(), "Discard Changes in 3 Files"),
        (two_lines(), "Discard 2 Lines"),
    ] {
        assert_eq!(consequence.action(), button);
        let (test, _) = launch(consequence.clone());
        let shown = labels(&test);
        assert!(shown.iter().any(|l| l == TITLE), "{shown:?}");
        assert!(
            shown.iter().any(|l| *l == consequence.prompt()),
            "the prompt is not the consequence's: {shown:?}"
        );
        assert!(shown.iter().any(|l| l == button), "{shown:?}");
        assert!(shown.iter().any(|l| l == CANCEL_CAPTION), "{shown:?}");
    }
}

/// C17, L8: focus starts on Cancel, so a Return pressed by habit cancels and builds no token;
/// Escape cancels; a press outside the dialog cancels. Caught by: focus left behind the
/// dialog (Return acts there), Discard focused as Fork styles it, or an Escape the dialog
/// does not hear.
#[test]
fn focus_starts_on_cancel_and_escape_or_a_press_outside_cancels() {
    let (mut test, reports) = launch(three_files());
    press(&mut test, NamedKey::Enter);
    assert_eq!(
        *reports.cancelled.borrow(),
        1,
        "Return did not press Cancel"
    );
    assert!(reports.confirmed.borrow().is_empty(), "Return confirmed");
    assert_eq!(
        *reports.behind.borrow(),
        0,
        "Return reached the view behind"
    );

    let (mut test, reports) = launch(three_files());
    press(&mut test, NamedKey::Escape);
    assert_eq!(*reports.cancelled.borrow(), 1, "Escape did not cancel");
    assert!(reports.confirmed.borrow().is_empty());

    let (mut test, reports) = launch(three_files());
    test.click_cursor((5., f64::from(HEIGHT) - 5.));
    test.sync_and_update();
    assert_eq!(
        *reports.cancelled.borrow(),
        1,
        "a press outside did not cancel"
    );
    assert!(reports.confirmed.borrow().is_empty());
}

/// C17, R7.4: the dialog is modal to the keyboard — Tab and Shift-Tab move between its two
/// answers and never reach the view behind it, which hears no key while it is open. Caught
/// by: a dialog without `a11y_modal`, where Tab walks out to the window behind it.
#[test]
fn tab_stays_inside_the_dialog() {
    let (mut test, reports) = launch(three_files());
    for shift in [false, true] {
        for _ in 0..5 {
            let held = if shift {
                Modifiers::SHIFT
            } else {
                Modifiers::empty()
            };
            test.press_key_with_modifiers(Key::Named(NamedKey::Tab), held);
            test.sync_and_update();
            test.press_key(Key::Character("x".into()));
            test.sync_and_update();
        }
    }
    assert_eq!(*reports.behind.borrow(), 0, "a key reached the view behind");
    // Ten Tabs from Cancel: focus is on one of the two answers, and Return presses it.
    press(&mut test, NamedKey::Enter);
    let answered = *reports.cancelled.borrow() + reports.confirmed.borrow().len();
    assert_eq!(answered, 1, "Return pressed neither answer");
}

/// L3, R1: the button builds the token from the consequence drawn — its prompt rendered from
/// it — and a second press builds no second token. Caught by: a token built from other
/// words, or one acknowledgement spent twice.
#[test]
fn the_button_builds_one_token_from_the_consequence_drawn() {
    let consequence = two_lines();
    let (mut test, reports) = launch(consequence.clone());
    let button = centre_of(&test, "Discard 2 Lines");
    test.click_cursor(button);
    test.sync_and_update();
    test.click_cursor(button);
    test.sync_and_update();
    let confirmed = reports.confirmed.borrow();
    assert_eq!(confirmed.len(), 1, "one acknowledgement built two tokens");
    assert_eq!(*confirmed[0].consequence(), consequence);
    assert_eq!(confirmed[0].prompt(), consequence.prompt());
    assert_eq!(*reports.cancelled.borrow(), 0);
}
