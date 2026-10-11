//! Headless tests for the commit box (staging-and-commit R10) and the Git Error dialog: the
//! Amend toggle reports its state to assistive technology; the one button and the commit chord
//! report the same press, building no token; the counter is Fork's; a disabled box asks nothing;
//! the line is drawn whole; a commit asked stands in the button's place; an edit is reported;
//! and the Git Error dialog draws its output through a virtualizing view, the skip in its footer
//! beside Close, focus on Close.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use cairn_ui::{
    AMEND_BUTTON_CAPTION, AMEND_CAPTION, Busy, CANCEL_COMMIT_CAPTION, CANCELLING, CLOSE_CAPTION,
    CommitBox, CommitButton, GIT_ERROR_TITLE, GitErrorDialog, OUTPUT_ROW_HEIGHT,
    SKIP_HOOKS_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 600.;

fn settle(test: &mut TestingRunner) {
    for _ in 0..4 {
        test.sync_and_update();
    }
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

fn click(test: &mut TestingRunner, caption: &str) {
    let at = centre_of(test, caption);
    test.click_cursor(at);
    settle(test);
}

/// What the box reported.
#[derive(Default, Clone)]
struct Reports {
    amend: Rc<RefCell<Vec<bool>>>,
    commits: Rc<RefCell<usize>>,
    cancels: Rc<RefCell<usize>>,
    edits: Rc<RefCell<usize>>,
}

/// The box with Amend `ticked` and `button`, its subject `subject`, `stopped` when an operation
/// in progress disables it, `line` over it and `busy` in the button's place.
fn launch(
    ticked: bool,
    button: CommitButton,
    subject: &str,
    stopped: bool,
    line: Option<&str>,
    busy: Option<Busy>,
) -> (TestingRunner, Reports) {
    let reports = Reports::default();
    let subject = subject.to_owned();
    let line = line.map(str::to_owned);
    let (mut test, _) = TestingRunner::new(
        {
            let reports = reports.clone();
            move || {
                let typed = use_state(|| subject.clone());
                let body = use_state(String::new);
                let Reports {
                    amend,
                    commits,
                    cancels,
                    edits,
                } = reports.clone();
                rect().expanded().child(
                    CommitBox::new(typed, body)
                        .amend(ticked, true)
                        .button(button.clone(), true)
                        .stopped(stopped)
                        .line(line.clone())
                        .busy(busy.clone())
                        .on_amend(move |on: bool| amend.borrow_mut().push(on))
                        .on_commit(move |()| *commits.borrow_mut() += 1)
                        .on_cancel(move |()| *cancels.borrow_mut() += 1)
                        .on_edit(move |()| *edits.borrow_mut() += 1),
                )
            }
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    settle(&mut test);
    (test, reports)
}

/// The toggled state assistive technology reads off the Amend check box.
fn amend_toggled(test: &TestingRunner) -> Option<Toggled> {
    test.find(|_, element| {
        Rect::try_downcast(element).and_then(|rect| {
            (rect.accessibility.builder.role() == AccessibilityRole::CheckBox
                && rect.accessibility.builder.label() == Some(AMEND_CAPTION))
            .then(|| rect.accessibility.builder.toggled())
            .flatten()
        })
    })
}

/// Phase 09's step 2 (`freya-ui-apis.md` §3): the Amend toggle is a check box to assistive
/// technology that says whether it is ticked, and a press reports the other state. Caught by:
/// Freya's `Checkbox`, which sets no toggled state, or a toggle that reports its own state.
#[test]
fn the_amend_toggle_reports_its_toggled_state_to_assistive_technology() {
    for (ticked, toggled) in [(false, Toggled::False), (true, Toggled::True)] {
        let (mut test, reports) = launch(
            ticked,
            CommitButton::Commit { files: 1 },
            "x",
            false,
            None,
            None,
        );
        assert_eq!(amend_toggled(&test), Some(toggled));
        click(&mut test, AMEND_CAPTION);
        assert_eq!(*reports.amend.borrow(), [!ticked]);
    }
}

/// R10.6, C1 and C6: amending, the one button reads Fork's "Amend Last Commit" with nothing
/// under it and reports the press — the box builds no token; the chord's press is the window's
/// test (`commit_box_tests.rs`). Caught by: a line under the button, or the press unheard.
#[test]
fn amend_last_commit_reports_its_press_and_draws_no_line() {
    let (mut test, reports) = launch(true, CommitButton::Amend, "Fix", false, None, None);
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == AMEND_BUTTON_CAPTION), "{shown:?}");
    assert_eq!(
        shown
            .iter()
            .filter(|l| l.contains("Show Lost Commits") || l.contains("reflog"))
            .count(),
        0,
        "a line under the button: {shown:?}"
    );
    click(&mut test, AMEND_BUTTON_CAPTION);
    assert_eq!(*reports.commits.borrow(), 1);
}

/// R10.1, rule 4: Fork's counter beside the subject, and its button naming the files; R10.8: a
/// box an operation in progress disables draws its one line whole — wrapped, never cut — and
/// its button and Amend ask nothing. Caught by: a disabled box that still commits or amends,
/// or the line cut to an ellipsis.
#[test]
fn the_counter_is_drawn_and_a_stopped_box_asks_nothing() {
    let (test, _) = launch(
        false,
        CommitButton::Commit { files: 3 },
        &"x".repeat(88),
        false,
        None,
        None,
    );
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == "-38"), "{shown:?}");
    assert!(shown.iter().any(|l| l == "Commit 3 Files"), "{shown:?}");

    let said = cairn_ui::operation_line(Some(&cairn_model::OperationInProgress::Rebase), false)
        .unwrap_or_default();
    let (mut test, reports) = launch(
        false,
        CommitButton::Commit { files: 3 },
        "x",
        true,
        Some(&said),
        None,
    );
    assert!(labels(&test).contains(&said));
    let line = test
        .find(|_, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == said)
                .map(|label| label.max_lines)
        })
        .unwrap_or_else(|| panic!("no line"));
    assert_eq!(line, None, "the line is capped");
    click(&mut test, "Commit 3 Files");
    click(&mut test, AMEND_CAPTION);
    assert_eq!(*reports.commits.borrow(), 0);
    assert!(reports.amend.borrow().is_empty());
}

/// R10.4 and C5: a commit asked stands in the button's place — waiting in the one form every
/// queued write uses, with no Cancel; running, "Committing 0:00" beside its Cancel, which is
/// reported; cancelled, "Cancelling…" with no Cancel — and no press of the button is heard
/// meanwhile. Caught by: a Cancel offered while queued or cancelling, or the button drawn beside.
#[test]
fn a_commit_asked_stands_in_the_buttons_place() {
    let waiting = "Waiting for staging to finish…";
    let (test, _) = launch(
        false,
        CommitButton::Commit { files: 1 },
        "x",
        false,
        None,
        Some(Busy::Waiting(waiting.to_owned())),
    );
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == waiting), "{shown:?}");
    assert!(
        !shown
            .iter()
            .any(|l| l == CANCEL_COMMIT_CAPTION || l == "Commit 1 File")
    );

    let (mut test, reports) = launch(
        false,
        CommitButton::Commit { files: 1 },
        "x",
        false,
        None,
        Some(Busy::Running {
            verb: "Committing",
            started: Some(Instant::now()),
        }),
    );
    assert!(
        labels(&test).iter().any(|l| l == "Committing 0:00"),
        "{:?}",
        labels(&test)
    );
    click(&mut test, CANCEL_COMMIT_CAPTION);
    assert_eq!(*reports.cancels.borrow(), 1);

    let (test, _) = launch(
        true,
        CommitButton::Amend,
        "x",
        false,
        None,
        Some(Busy::Cancelling),
    );
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == CANCELLING), "{shown:?}");
    assert!(
        !shown
            .iter()
            .any(|l| l == CANCEL_COMMIT_CAPTION || l == AMEND_BUTTON_CAPTION)
    );
}

/// C7: a key that changes a field's text is reported as the person's edit, and one that does
/// not (an arrow) is not. Caught by: no edit reported, which would let a fill overwrite a draft
/// the person cleared.
#[test]
fn an_edit_in_a_field_is_reported() {
    let (mut test, reports) = launch(
        false,
        CommitButton::Commit { files: 1 },
        "",
        false,
        None,
        None,
    );
    let at = test
        .find(|node, element| {
            Paragraph::try_downcast(element)
                .filter(|paragraph| {
                    paragraph
                        .spans
                        .iter()
                        .map(|span| span.text.as_ref())
                        .collect::<String>()
                        == cairn_ui::SUBJECT_PLACEHOLDER
                })
                .map(|_| {
                    let area = node.layout().area;
                    (f64::from(area.min_x() + 4.), f64::from(area.center().y))
                })
        })
        .unwrap_or_else(|| panic!("no subject field"));
    test.click_cursor(at);
    settle(&mut test);
    test.press_key(Key::Named(NamedKey::ArrowLeft));
    settle(&mut test);
    assert_eq!(*reports.edits.borrow(), 0, "an arrow was an edit");
    test.write_text("a");
    settle(&mut test);
    assert!(*reports.edits.borrow() >= 1, "typing was no edit");
}

/// What the Git Error dialog reported.
#[derive(Default, Clone)]
struct Answers {
    skipped: Rc<RefCell<usize>>,
    closed: Rc<RefCell<usize>>,
}

fn launch_error(lines: usize, skip: bool) -> (TestingRunner, Answers) {
    let answers = Answers::default();
    let output: Rc<Vec<String>> = Rc::new((0..lines).map(|n| format!("hook line {n}")).collect());
    let (mut test, _) = TestingRunner::new(
        {
            let answers = answers.clone();
            move || {
                let Answers { skipped, closed } = answers.clone();
                rect().expanded().child(
                    GitErrorDialog::new(1, "git commit -q -F -", output.clone())
                        .skip(skip)
                        .on_skip(move |()| *skipped.borrow_mut() += 1)
                        .on_close(move |()| *closed.borrow_mut() += 1),
                )
            }
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    settle(&mut test);
    (test, answers)
}

/// R10.5: a hook's output of a hundred thousand lines builds one viewport's rows — through a
/// virtualizing view, never a `ScrollView` — opened at its end, where the failure is said;
/// and the skip is drawn only where it is offered. Caught by: a dialog laying out every line,
/// or the skip drawn whatever the hooks.
#[test]
fn the_git_error_draws_one_viewport_of_output_and_the_skip_only_where_offered() {
    let (test, _) = launch_error(100_000, true);
    let built = labels(&test)
        .iter()
        .filter(|label| label.starts_with("hook line "))
        .count();
    assert!(built > 0, "no line of output was drawn");
    assert!(
        built <= (240. / OUTPUT_ROW_HEIGHT) as usize + 4,
        "{built} lines of output built for one viewport"
    );
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == GIT_ERROR_TITLE), "{shown:?}");
    assert!(shown.iter().any(|l| l == "git commit -q -F -"));
    assert!(
        shown.iter().any(|l| l == "hook line 99999"),
        "the output did not open at its end: {shown:?}"
    );
    assert!(shown.iter().any(|l| l == SKIP_HOOKS_CAPTION));

    let (mut test, answers) = launch_error(3, false);
    assert!(!labels(&test).iter().any(|l| l == SKIP_HOOKS_CAPTION));
    click(&mut test, CLOSE_CAPTION);
    assert_eq!(*answers.closed.borrow(), 1);
    assert_eq!(*answers.skipped.borrow(), 0);

    let (mut test, answers) = launch_error(3, true);
    click(&mut test, SKIP_HOOKS_CAPTION);
    click(&mut test, SKIP_HOOKS_CAPTION);
    assert_eq!(*answers.skipped.borrow(), 1, "the skip ran twice");
}

/// R10.5 and C4: the skip sits in the dialog's footer beside Close — the same answer component,
/// on the same row — and focus starts on Close, so a Return pressed by habit closes rather than
/// skips the hooks. Caught by: the skip drawn in the body, or focus on the skip.
#[test]
fn the_skip_sits_in_the_footer_beside_close_and_focus_starts_on_close() {
    let (mut test, answers) = launch_error(3, true);
    let skip = centre_of(&test, SKIP_HOOKS_CAPTION);
    let close = centre_of(&test, CLOSE_CAPTION);
    assert!(
        (skip.1 - close.1).abs() < 1.,
        "not one row: {skip:?} {close:?}"
    );
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert_eq!(*answers.closed.borrow(), 1, "Return did not close");
    assert_eq!(*answers.skipped.borrow(), 0, "Return skipped the hooks");
}
