//! Headless tests for the commit box and the Git Error dialog: the Amend toggle reports its
//! state to assistive technology; the one button and the commit chord report the same press,
//! building no token; the counter is Fork's; a disabled box asks nothing; the line is drawn
//! whole; a commit asked stands in the button's place; an edit is reported; and the Git Error
//! dialog draws its output through a virtualizing view, the skip in its footer beside Close,
//! focus on Close.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use cairn_ui::style::{metrics, tones};
use cairn_ui::{
    AMEND_BUTTON_CAPTION, AMEND_CAPTION, Busy, CANCEL_COMMIT_CAPTION, CANCELLING, CLOSE_CAPTION,
    COMMIT_FIELD_NAME, COMMIT_OPTIONS_CAPTION, COMMIT_OPTIONS_TOOLTIP, CommitBox, CommitButton,
    GIT_ERROR_TITLE, GitErrorDialog, OUTPUT_ROW_HEIGHT, RECENT_MESSAGES_CAPTION,
    SKIP_HOOKS_CAPTION,
};
use freya::elements::rect::RectElement;
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
    launch_ready(ticked, button, subject, stopped, line, busy, true)
}

/// [`launch`], the button's press `ready` or not.
fn launch_ready(
    ticked: bool,
    button: CommitButton,
    subject: &str,
    stopped: bool,
    line: Option<&str>,
    busy: Option<Busy>,
    ready: bool,
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
                        .button(button.clone(), ready)
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

/// The Amend toggle is a check box to assistive technology that says whether it is ticked, and
/// a press reports the other state. Caught by: Freya's `Checkbox`, which sets no toggled state,
/// or a toggle that reports its own state.
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

/// Amending, the one button reads Fork's "Amend Last Commit" with nothing under it and reports
/// the press — the box builds no token; the chord's press is the window's test
/// (`commit_box/tests.rs`). Caught by: a line under the button, or the press unheard.
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

/// Fork's counter beside the subject, and its button naming the files; and a box an operation
/// in progress disables draws its one line whole — wrapped, never cut — and its button and
/// Amend ask nothing. Caught by: a disabled box that still commits or amends, or the line cut
/// to an ellipsis.
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

/// A commit asked stands in the button's place — waiting in the one form every queued write
/// uses, with no Cancel; running, "Committing 0:00" beside its Cancel, which is reported;
/// cancelled, "Cancelling…" with no Cancel — and no press of the button is heard meanwhile.
/// Caught by: a Cancel offered while queued or cancelling, or the button drawn beside.
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

    // Asked and not yet started: its verb alone, no Cancel.
    let (test, _) = launch(
        false,
        CommitButton::Commit { files: 1 },
        "x",
        false,
        None,
        Some(Busy::Running {
            verb: "Committing",
            started: None,
        }),
    );
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == "Committing"), "{shown:?}");
    assert!(
        !shown.iter().any(|l| l == CANCEL_COMMIT_CAPTION),
        "{shown:?}"
    );

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

/// A key that changes a field's text is reported as the person's edit, and one that does not
/// (an arrow) is not. Caught by: no edit reported, which would let a fill overwrite a draft the
/// person cleared.
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

/// Every control assistive technology reads as `name`, with where it is laid out.
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

/// Where the first text input of `role` is laid out.
fn input_of(test: &TestingRunner, role: AccessibilityRole) -> Area {
    test.find(|node, element| {
        Rect::try_downcast(element)
            .filter(|rect| rect.accessibility.builder.role() == role)
            .map(|_| node.layout().area)
    })
    .unwrap_or_else(|| panic!("no {role:?}"))
}

/// Rests the pointer on `at` long enough for its tooltip, and the labels drawn then.
fn hovered(test: &mut TestingRunner, at: Area) -> Vec<String> {
    let centre = at.center();
    test.move_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.poll(Duration::from_millis(50), Duration::from_millis(900));
    let shown = labels(test);
    test.move_cursor((f64::from(WIDTH - 2.), 2.));
    test.poll(Duration::from_millis(50), Duration::from_millis(400));
    shown
}

fn contains(outer: Area, inner: Area) -> bool {
    inner.min_x() >= outer.min_x()
        && inner.max_x() <= outer.max_x()
        && inner.min_y() >= outer.min_y()
        && inner.max_y() <= outer.max_y()
}

/// Fork's box (uv12c, uw36a): one bordered field holding the subject over a rule over the
/// description, the Recent Commit Messages glyph at the subject's right end inside it; under it
/// Amend at the left and the split button at the right — its caption, then its ⌄ beside it, a
/// control of its own, greyed and saying it comes with pull and push whether the caption can be
/// pressed or not, its press reporting nothing. Caught by: two separate fields, the glyph
/// outside the field, the ⌄ missing, enabled, or silent, or a press on it committing.
#[test]
fn the_commit_box_is_one_field_and_a_split_button() {
    for ready in [false, true] {
        let subject = if ready { "Fix" } else { "" };
        let (mut test, reports) = launch_ready(
            false,
            CommitButton::Commit { files: 1 },
            subject,
            false,
            None,
            None,
            ready,
        );
        let (field_rect, field) = one_named(&test, COMMIT_FIELD_NAME);
        assert_eq!(
            field_rect.style.borders.first().map(|border| border.fill),
            Some(tones::RULE),
            "the field is not bordered"
        );
        let subject_area = input_of(&test, AccessibilityRole::TextInput);
        let description = input_of(&test, AccessibilityRole::MultilineTextInput);
        assert!(
            contains(field, subject_area) && contains(field, description),
            "the subject and the description are not parts of one field: {field:?}"
        );
        assert!(subject_area.max_y() <= description.min_y());
        assert_eq!(subject_area.height(), metrics::COMMIT_SUBJECT);
        let rules = test.find_many(|node, element| {
            let area = node.layout().area;
            Rect::try_downcast(element)
                .filter(|rect| rect.style.background == Fill::Color(tones::RULE))
                .filter(|_| area.height() == 1. && area.width() > field.width() / 2.)
                .map(|_| area.center().y)
        });
        assert!(
            rules
                .iter()
                .any(|y| *y >= subject_area.max_y() && *y <= description.min_y()),
            "no rule between the subject and the description: {rules:?}"
        );
        let (_, recent) = one_named(&test, RECENT_MESSAGES_CAPTION);
        assert!(
            contains(field, recent)
                && recent.min_x() >= subject_area.max_x()
                && recent.center().y > subject_area.min_y()
                && recent.center().y < subject_area.max_y(),
            "Recent Commit Messages is not at the subject's right end: {recent:?}"
        );

        let (_, amend) = one_named(&test, AMEND_CAPTION);
        let caption = "Commit 1 File";
        let (_, button) = one_named(&test, caption);
        let (options, chevron) = one_named(&test, COMMIT_OPTIONS_CAPTION);
        assert!(
            amend.max_x() < button.min_x(),
            "Amend is not left of the button"
        );
        assert!(
            chevron.min_x() >= button.max_x() - 1.
                && (chevron.center().y - button.center().y).abs() < 1.,
            "the ⌄ is not beside the caption: {button:?} {chevron:?}"
        );
        assert!(
            field.max_y() <= button.min_y(),
            "the button is not under the field"
        );
        assert!(
            options.accessibility.builder.is_disabled(),
            "the ⌄ is not greyed"
        );
        let shown = hovered(&mut test, chevron);
        assert!(
            shown.iter().any(|label| label == COMMIT_OPTIONS_TOOLTIP),
            "the ⌄ does not say why it is greyed (ready: {ready}): {shown:?}"
        );
        assert_eq!(COMMIT_OPTIONS_TOOLTIP, "Comes with pull and push.");
        let centre = chevron.center();
        test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
        settle(&mut test);
        assert_eq!(*reports.commits.borrow(), 0, "the ⌄ committed");
        click(&mut test, caption);
        assert_eq!(
            *reports.commits.borrow(),
            usize::from(ready),
            "the caption's press (ready: {ready})"
        );
    }
}

/// Fork's counter appears only once the subject has text, and the subject is bold then: with
/// none, no figure is drawn and nothing is bold; with "x", "49" stands between the subject and
/// Recent Commit Messages. Caught by: the counter's "50" over an empty subject, the counter
/// after the glyph, or the typed subject at the body's weight.
#[test]
fn the_counter_and_the_bold_subject_appear_once_the_subject_has_text() {
    let bold_round = |test: &TestingRunner, subject: Area| {
        test.find_many(|node, element| {
            Rect::try_downcast(element)
                .filter(|rect| rect.text_style_data.font_weight == Some(FontWeight::BOLD))
                .filter(|_| contains(node.layout().area, subject))
        })
        .len()
    };
    let (test, _) = launch(
        false,
        CommitButton::Commit { files: 1 },
        "",
        false,
        None,
        None,
    );
    let shown = labels(&test);
    assert!(
        !shown.iter().any(|l| l == "50"),
        "a counter while empty: {shown:?}"
    );
    let subject = input_of(&test, AccessibilityRole::TextInput);
    assert_eq!(bold_round(&test, subject), 0, "the empty subject is bold");

    let (test, _) = launch(
        false,
        CommitButton::Commit { files: 1 },
        "x",
        false,
        None,
        None,
    );
    let subject = input_of(&test, AccessibilityRole::TextInput);
    let counter = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == "49")
                .map(|_| node.layout().area)
        })
        .unwrap_or_else(|| panic!("no counter once typed: {:?}", labels(&test)));
    let (_, recent) = one_named(&test, RECENT_MESSAGES_CAPTION);
    assert!(
        counter.min_x() >= subject.max_x() && counter.max_x() <= recent.min_x(),
        "the counter is not between the subject and the glyph"
    );
    assert!(
        bold_round(&test, subject) >= 1,
        "the typed subject is not bold"
    );
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

/// A hook's output of a hundred thousand lines builds one viewport's rows — through a
/// virtualizing view, never a `ScrollView` — opened at its end, where the failure is said; and
/// the skip is drawn only where it is offered. Caught by: a dialog laying out every line, or
/// the skip drawn whatever the hooks.
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

/// The skip sits in the dialog's footer beside Close — the same answer component, on the same
/// row — and focus starts on Close, so a Return pressed by habit closes rather than skips the
/// hooks. Caught by: the skip drawn in the body, or focus on the skip.
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
