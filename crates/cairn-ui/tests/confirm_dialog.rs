//! Headless tests for the confirmation dialog: its words and its button are the
//! `Consequence`'s, focus starts on Cancel and stays inside it, Escape and a press outside
//! cancel, and one acknowledgement builds one token from the value drawn.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    Confirmed, Consequence, DiscardedFile, FileLoss, LineNumber, Oid, Patch, RepoPath, Selection,
};
use cairn_ui::{
    CANCEL_CAPTION, CLOSE_CAPTION, COUNTING_TEXT, ConfirmDialog, DialogAnswer, NOTHING_CHANGED,
    SHOW_FILES_CAPTION, SHOWN_FILE_HEIGHT,
};
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
    launch_answering(DialogAnswer::Ready(Rc::new(consequence)), None)
}

fn launch_answering(answer: DialogAnswer, left: Option<String>) -> (TestingRunner, Reports) {
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
                        ConfirmDialog::answering(1, TITLE, answer.clone())
                            .left(left.clone())
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

/// The prompt and the button are the `Consequence`'s own words — the files' count and kinds,
/// and the lines' count — never text typed beside it. Caught by: a dialog that writes its own
/// words, or a button that does not name the count.
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

/// Focus starts on Cancel, so a Return pressed by habit cancels and builds no token; Escape
/// cancels; a press outside the dialog cancels. Caught by: focus left behind the dialog
/// (Return acts there), Discard focused as Fork styles it, or an Escape the dialog does not
/// hear.
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

/// The dialog is modal to the keyboard — Tab and Shift-Tab move between its two answers and
/// never reach the view behind it, which hears no key while it is open. Caught by: a dialog
/// without `a11y_modal`, where Tab walks out to the window behind it.
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

/// The button builds the token from the consequence drawn — its prompt rendered from it — and
/// a second press builds no second token. Caught by: a token built from other words, or one
/// acknowledgement spent twice.
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

/// Every label's text and its place, top to bottom.
fn placed(test: &TestingRunner) -> Vec<(String, f32)> {
    let mut found = test.find_many(|node, element| {
        Label::try_downcast(element).map(|l| (l.text.to_string(), node.layout().area.min_y()))
    });
    found.sort_by(|a, b| a.1.total_cmp(&b.1));
    found
}

/// Presses the lowest label reading `caption`: a dialog's button under its title.
fn press_lowest(test: &mut TestingRunner, caption: &str) {
    let at = test
        .find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == caption)
                .map(|_| node.layout().area.center())
        })
        .into_iter()
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap_or_else(|| panic!("no label reads {caption:?}; labels: {:?}", labels(test)));
    test.click_cursor((f64::from(at.x), f64::from(at.y)));
    test.sync_and_update();
}

/// Opened at the press while the engine counts, the dialog says "Counting…", and its button,
/// greyed, reads the dialog's title; pressing it builds nothing, and focus is on Cancel, so
/// Return cancels. Caught by: a button that confirms before the count, a count shown as a
/// number, or focus off Cancel.
#[test]
fn while_counting_the_dialog_says_so_and_its_greyed_button_builds_nothing() {
    let (mut test, reports) = launch_answering(DialogAnswer::Counting, None);
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == COUNTING_TEXT), "{shown:?}");
    assert_eq!(
        shown.iter().filter(|l| *l == TITLE).count(),
        2,
        "the title and the greyed button: {shown:?}"
    );
    assert!(shown.iter().any(|l| l == CANCEL_CAPTION), "{shown:?}");
    press_lowest(&mut test, TITLE);
    assert!(
        reports.confirmed.borrow().is_empty(),
        "the greyed button confirmed"
    );
    assert_eq!(*reports.cancelled.borrow(), 0);
    press(&mut test, NamedKey::Enter);
    assert_eq!(
        *reports.cancelled.borrow(),
        1,
        "Return did not press Cancel"
    );
    assert!(reports.confirmed.borrow().is_empty());
}

/// A refusal while counting is the same dialog titled "Couldn't Discard Changes", the engine's
/// sentence and "Nothing was changed.", Close alone and focused; Return and Close let it go,
/// and nothing confirms. Caught by: a confirm or Cancel left on a refusal, the tail missing or
/// said twice, or Close not focused.
#[test]
fn a_refusal_is_the_same_dialog_with_close_alone() {
    let why = "src/a.rs changed while it was read.";
    let (mut test, reports) = launch_answering(DialogAnswer::Refused(Rc::from(why)), None);
    let shown = labels(&test);
    assert!(
        shown.iter().any(|l| l == "Couldn't Discard Changes"),
        "{shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|l| *l == format!("{why} {NOTHING_CHANGED}")),
        "{shown:?}"
    );
    assert_eq!(NOTHING_CHANGED, "Nothing was changed.");
    assert!(shown.iter().any(|l| l == CLOSE_CAPTION), "{shown:?}");
    assert!(!shown.iter().any(|l| l == CANCEL_CAPTION), "{shown:?}");
    assert!(!shown.iter().any(|l| l == TITLE), "{shown:?}");
    press(&mut test, NamedKey::Enter);
    assert_eq!(*reports.cancelled.borrow(), 1, "Return did not press Close");
    assert!(reports.confirmed.borrow().is_empty());
}

/// The line saying what a mixed selection leaves is drawn under the prompt, never part of the
/// prompt the token records. Caught by: the line left out, drawn above the prompt, or folded
/// into the token's words.
#[test]
fn what_is_left_is_one_line_under_the_prompt_and_not_in_the_token() {
    let consequence = three_files();
    let left = "1 submodule and 1 conflicted file are left as they are.";
    let (mut test, reports) = launch_answering(
        DialogAnswer::Ready(Rc::new(consequence.clone())),
        Some(left.to_owned()),
    );
    let order: Vec<String> = placed(&test).into_iter().map(|(text, _)| text).collect();
    let prompt_at = order.iter().position(|t| *t == consequence.prompt());
    let left_at = order.iter().position(|t| t == left);
    assert!(
        matches!((prompt_at, left_at), (Some(p), Some(l)) if l > p),
        "{order:?}"
    );
    press_lowest(&mut test, &consequence.action());
    let confirmed = reports.confirmed.borrow();
    assert_eq!(confirmed.len(), 1);
    assert_eq!(confirmed[0].prompt(), consequence.prompt());
    assert!(!confirmed[0].prompt().contains("left as"));
}

/// A discard of several files puts "▸ Show files" under its prompt; pressed, the glyph turns
/// to ▾, the caption stays, and each file is listed with what happens to it; one file and
/// lines list nothing. Caught by: the detail in the sentence, a caption that changes, a list
/// shown unasked, or one where what is taken is already on screen.
#[test]
fn show_files_discloses_each_file_and_what_happens_to_it() {
    let (mut test, _) = launch(three_files());
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == SHOW_FILES_CAPTION), "{shown:?}");
    assert!(shown.iter().any(|l| l == "▸"), "{shown:?}");
    assert!(
        !shown.iter().any(|l| l == "src/a.rs"),
        "listed unasked: {shown:?}"
    );
    let disclosure = centre_of(&test, SHOW_FILES_CAPTION);
    test.click_cursor(disclosure);
    test.sync_and_update();
    test.sync_and_update();
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == SHOW_FILES_CAPTION), "{shown:?}");
    assert!(shown.iter().any(|l| l == "▾"), "{shown:?}");
    for (path, what) in [
        ("src/a.rs", "4 lines"),
        ("src/b.rs", "10 lines"),
        ("notes.txt", "Deleted"),
    ] {
        assert!(shown.iter().any(|l| l == path), "{path}: {shown:?}");
        assert!(shown.iter().any(|l| l == what), "{what}: {shown:?}");
    }
    // Moved away first, so the second press is not read as a double press.
    test.move_cursor((5., 5.));
    test.sync_and_update();
    let disclosure = centre_of(&test, SHOW_FILES_CAPTION);
    test.click_cursor(disclosure);
    test.sync_and_update();
    test.sync_and_update();
    assert!(
        !labels(&test).iter().any(|l| l == "src/a.rs"),
        "it did not close: {:?}",
        labels(&test)
    );

    for nothing in [two_lines(), {
        let Consequence::DiscardFiles { files } = three_files() else {
            unreachable!()
        };
        Consequence::DiscardFiles {
            files: files[..1].to_vec(),
        }
    }] {
        let (test, _) = launch(nothing);
        assert!(
            !labels(&test).iter().any(|l| l == SHOW_FILES_CAPTION),
            "{:?}",
            labels(&test)
        );
    }
}

/// A path wider than the dialog is listed whole, its row as wide as its words and the list
/// scrolling sideways to it, as every other list of paths does — never cut at the dialog's
/// edge. Caught by: a row held to the list's width, the path squeezed into what is left beside
/// its loss.
#[test]
fn show_files_lists_a_path_wider_than_the_dialog_whole_and_scrolls_to_it() {
    let long = format!("src/{}/deep.rs", "a-folder-whose-name-runs-on/".repeat(12));
    let Consequence::DiscardFiles { mut files } = three_files() else {
        unreachable!()
    };
    files[0].path = RepoPath::from(long.as_str());
    let (mut test, _) = launch(Consequence::DiscardFiles { files });
    let disclosure = centre_of(&test, SHOW_FILES_CAPTION);
    test.click_cursor(disclosure);
    test.sync_and_update();
    test.sync_and_update();
    let area_of = |test: &TestingRunner, text: &str| {
        test.find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .map(|_| node.layout().area)
        })
        .unwrap_or_else(|| panic!("no label reads {text:?}; labels: {:?}", labels(test)))
    };
    let dialog = area_of(&test, SHOW_FILES_CAPTION);
    let path = area_of(&test, &long);
    let what = area_of(&test, "4 lines");
    assert!(
        path.width() > 600.,
        "the path is {} wide, held to the dialog: {path:?}",
        path.width()
    );
    assert!(
        what.min_x() >= path.max_x(),
        "its loss at {what:?} is drawn over the path at {path:?}"
    );
    // Scrolled sideways, the row moves under the viewport and its loss comes into view.
    test.scroll(
        (f64::from(dialog.min_x() + 40.), f64::from(path.center().y)),
        (-(f64::from(path.width()) * 2.), 0.),
    );
    test.sync_and_update();
    test.sync_and_update();
    let scrolled = area_of(&test, &long);
    assert!(
        scrolled.min_x() < path.min_x() - 100.,
        "the list does not scroll sideways: the path was at {path:?}, now {scrolled:?}"
    );
}

/// The viewport twin of the root `CLAUDE.md`'s virtualization invariant: "Show files" over
/// a discard of 50,000 files builds one viewport of rows, at the top and scrolled to the end,
/// the last file there. Caught by: a list that builds a row per file.
#[test]
fn show_files_over_50000_paths_builds_one_viewport() {
    const FILES: usize = 50_000;
    let files: Vec<DiscardedFile> = (0..FILES)
        .map(|n| DiscardedFile {
            path: RepoPath::from(format!("src/file{n:05}.rs").as_str()),
            loss: FileLoss::Modified {
                index: oid(1),
                working_tree: Some(oid(2)),
                executable: false,
                lines: Some(n),
                mode: None,
            },
        })
        .collect();
    let (mut test, _) = launch(Consequence::DiscardFiles { files });
    let disclosure = centre_of(&test, SHOW_FILES_CAPTION);
    test.click_cursor(disclosure);
    test.sync_and_update();
    test.sync_and_update();
    let built = |test: &TestingRunner| -> Vec<String> {
        labels(test)
            .into_iter()
            .filter(|l| l.starts_with("src/file"))
            .collect()
    };
    let viewport = (200. / SHOWN_FILE_HEIGHT).ceil() as usize;
    let at_top = built(&test);
    assert!(
        !at_top.is_empty() && at_top.len() <= viewport + 3,
        "{} rows built at the top for a viewport of {viewport}",
        at_top.len()
    );
    assert_eq!(at_top.first().map(String::as_str), Some("src/file00000.rs"));
    let list = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == "src/file00000.rs")
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no first row"));
    test.scroll(
        (f64::from(list.x), f64::from(list.y)),
        (0., -(FILES as f64) * f64::from(SHOWN_FILE_HEIGHT) * 2.),
    );
    test.sync_and_update();
    test.sync_and_update();
    let at_end = built(&test);
    assert!(
        !at_end.is_empty() && at_end.len() <= viewport + 3,
        "{} rows built at the end for a viewport of {viewport}",
        at_end.len()
    );
    assert!(
        at_end.iter().any(|l| l == "src/file49999.rs"),
        "the end is not the last file: {at_end:?}"
    );
}

/// Whether assistive technology reads the button labelled `caption` as disabled; `None` when no
/// button carries that label.
fn button_disabled(test: &TestingRunner, caption: &str) -> Option<bool> {
    test.find(|_, element| {
        Rect::try_downcast(element).and_then(|rect| {
            (rect.accessibility.builder.role() == AccessibilityRole::Button
                && rect.accessibility.builder.label() == Some(caption))
            .then(|| rect.accessibility.builder.is_disabled())
        })
    })
}

/// While the engine counts, the greyed button is disabled to assistive technology as well as
/// to the eye, and once the count fills the same dialog its caption's button is not. Caught
/// by: the disabled flag dropped from the greyed answer (a screen reader would offer a button
/// that does nothing), or kept on the confirming one.
#[test]
fn the_greyed_button_is_disabled_to_assistive_technology_until_the_count_arrives() {
    let (mut test, answer) = {
        let (test, answer) = TestingRunner::new(
            move || {
                let answer = use_consume::<State<DialogAnswer>>();
                rect()
                    .expanded()
                    .child(ConfirmDialog::answering(1, TITLE, answer.read().clone()))
                    .into_element()
            },
            (WIDTH, HEIGHT).into(),
            |runner| runner.provide_root_context(|| State::create(DialogAnswer::Counting)),
            1.,
        );
        (test, answer)
    };
    for _ in 0..4 {
        test.sync_and_update();
    }
    assert_eq!(
        button_disabled(&test, TITLE),
        Some(true),
        "{:?}",
        labels(&test)
    );
    assert_eq!(button_disabled(&test, CANCEL_CAPTION), Some(false));

    let consequence = three_files();
    let mut answer = answer;
    test.run_in(|| answer.set(DialogAnswer::Ready(Rc::new(consequence.clone()))));
    for _ in 0..2 {
        test.sync_and_update();
    }
    assert_eq!(
        button_disabled(&test, &consequence.action()),
        Some(false),
        "{:?}",
        labels(&test)
    );
    assert_eq!(
        button_disabled(&test, TITLE),
        None,
        "the greyed button outlived the count"
    );
}
