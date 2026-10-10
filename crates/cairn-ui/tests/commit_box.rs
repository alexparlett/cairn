//! Headless tests for the commit box (staging-and-commit R10) and the dialogs it opens: the
//! Amend toggle reports its state to assistive technology; the amend button confirmed in place
//! is the confirmation surface's own, building one token from the consequence it draws; the
//! counter is Fork's; a disabled box asks nothing; and the Git Error dialog draws its output
//! through a virtualizing view, offering the skip only where it is told a hook exists.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{Confirmed, Consequence, Oid, Publication, Reflog};
use cairn_ui::{
    AMEND_BUTTON_CAPTION, AMEND_CAPTION, AmendButton, CLOSE_CAPTION, CommitBox, CommitButton,
    GIT_ERROR_TITLE, GitErrorDialog, OUTPUT_ROW_HEIGHT, SKIP_HOOKS_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 600.;

fn amend() -> Consequence {
    Consequence::Amend {
        commit: Oid::from_bytes(&[0xab; 20]).unwrap_or_else(|e| panic!("{e}")),
        published: Publication::Unpublished,
        reflog: Reflog::Written,
    }
}

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
    tokens: Rc<RefCell<Vec<Confirmed>>>,
}

/// The box with Amend `ticked` and `button`, its subject `subject`, `stopped` when the box is
/// disabled by an operation in progress.
fn launch(
    ticked: bool,
    button: CommitButton,
    subject: &str,
    stopped: Option<&str>,
) -> (TestingRunner, Reports) {
    let reports = Reports::default();
    let subject = subject.to_owned();
    let stopped = stopped.map(str::to_owned);
    let (mut test, _) = TestingRunner::new(
        {
            let reports = reports.clone();
            move || {
                let typed = use_state(|| subject.clone());
                let body = use_state(String::new);
                let Reports {
                    amend,
                    commits,
                    tokens,
                } = reports.clone();
                rect().expanded().child(
                    CommitBox::new(typed, body)
                        .amend(ticked, true)
                        .button(button.clone(), true)
                        .stopped(stopped.clone())
                        .on_amend(move |on: bool| amend.borrow_mut().push(on))
                        .on_commit(move |()| *commits.borrow_mut() += 1)
                        .on_confirmed(move |token: Confirmed| tokens.borrow_mut().push(token)),
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
        let (mut test, reports) = launch(ticked, CommitButton::Commit { files: 1 }, "x", None);
        assert_eq!(amend_toggled(&test), Some(toggled));
        click(&mut test, AMEND_CAPTION);
        assert_eq!(*reports.amend.borrow(), [!ticked]);
    }
}

/// R10.6 and C1: the amend button confirmed in place is the confirmation surface's own — its
/// label Fork's "Amend Last Commit", its token built from that consequence and recording the
/// prompt rendered from it — and one consequence buys one token, however often it is pressed;
/// the box itself reports no commit for it. Caught by: a token per press, a label typed apart
/// from the consequence, or the press reported as a plain commit.
#[test]
fn the_amend_button_in_place_builds_one_token_from_the_consequence_it_draws() {
    let consequence = Rc::new(amend());
    let (mut test, reports) = launch(
        true,
        CommitButton::AmendInPlace {
            serial: 7,
            consequence: consequence.clone(),
        },
        "Fix the parser",
        None,
    );
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == AMEND_BUTTON_CAPTION), "{shown:?}");
    click(&mut test, AMEND_BUTTON_CAPTION);
    click(&mut test, AMEND_BUTTON_CAPTION);
    let tokens = reports.tokens.borrow();
    assert_eq!(tokens.len(), 1, "one consequence bought two tokens");
    assert_eq!(tokens[0].consequence(), &*consequence);
    assert_eq!(tokens[0].prompt(), consequence.prompt());
    assert_eq!(*reports.commits.borrow(), 0);
}

/// R10.6, L12: an amend a remote has is not confirmed by the box's button — the press is
/// reported, for the window to open the dialog — and builds no token. Caught by: the in-place
/// button drawn for a published amend.
#[test]
fn a_published_amends_button_asks_rather_than_confirms() {
    let consequence = Rc::new(Consequence::Amend {
        commit: Oid::from_bytes(&[0xab; 20]).unwrap_or_else(|e| panic!("{e}")),
        published: Publication::SomeRemote,
        reflog: Reflog::Written,
    });
    let (mut test, reports) = launch(
        true,
        CommitButton::AmendAsking {
            serial: 3,
            consequence,
        },
        "Fix the parser",
        None,
    );
    click(&mut test, &format!("{AMEND_BUTTON_CAPTION}…"));
    assert!(reports.tokens.borrow().is_empty());
    assert_eq!(*reports.commits.borrow(), 1);
}

/// R1.1 and the user's decision (2026-10-09), its line cut back by phase 13 (staging-and-commit
/// R6.4 as amended; phase 18 rebuilds the box): the amend button confirmed in place refuses a
/// consequence that must be confirmed — a remote has the commit, or git keeps no reflog — which
/// the dialog confirms, and builds nothing while not ready. Caught by: a pushed or unlogged
/// commit amended by the box's button, or a press heard while the box is not ready.
#[test]
fn the_amend_button_refuses_a_pushed_consequence_and_builds_nothing_unready() {
    let published = Consequence::Amend {
        commit: Oid::from_bytes(&[0xab; 20]).unwrap_or_else(|e| panic!("{e}")),
        published: Publication::SomeRemote,
        reflog: Reflog::Written,
    };
    let unlogged = Consequence::Amend {
        commit: Oid::from_bytes(&[0xab; 20]).unwrap_or_else(|e| panic!("{e}")),
        published: Publication::Unpublished,
        reflog: Reflog::NotWritten,
    };
    for (consequence, ready) in [(published, true), (unlogged, true), (amend(), false)] {
        let tokens: Rc<RefCell<Vec<Confirmed>>> = Rc::default();
        let (mut test, _) = TestingRunner::new(
            {
                let tokens = tokens.clone();
                move || {
                    let tokens = tokens.clone();
                    let confirmed = use_state(|| None::<u64>);
                    rect().expanded().child(
                        AmendButton::new(1, Rc::new(consequence.clone()), confirmed)
                            .ready(ready)
                            .on_confirmed(move |token: Confirmed| tokens.borrow_mut().push(token)),
                    )
                }
            },
            (WIDTH, HEIGHT).into(),
            |_| {},
            1.,
        );
        settle(&mut test);
        click(&mut test, AMEND_BUTTON_CAPTION);
        assert!(
            tokens.borrow().is_empty(),
            "ready {ready}: a token was built"
        );
    }
}

/// The user's decision (2026-10-09): a failed amend's skip is drawn in the Git Error dialog
/// with the prompt the amend was confirmed with, and one press builds one token from that
/// consequence, the dialog closing as it does; the commit's own skip is not drawn beside it.
/// Caught by: a skip confirming other words, or two tokens from one press.
#[test]
fn a_failed_amends_skip_confirms_the_prompt_it_draws_once() {
    let tokens: Rc<RefCell<Vec<Confirmed>>> = Rc::default();
    let closed: Rc<RefCell<usize>> = Rc::default();
    let consequence = Rc::new(amend());
    let (mut test, _) = TestingRunner::new(
        {
            let (tokens, closed, consequence) =
                (tokens.clone(), closed.clone(), consequence.clone());
            move || {
                let (tokens, closed) = (tokens.clone(), closed.clone());
                rect().expanded().child(
                    GitErrorDialog::new(1, "git commit -q --amend -F -", Rc::new(Vec::new()))
                        .skip(true)
                        .skip_amend(consequence.clone(), move |token: Confirmed| {
                            tokens.borrow_mut().push(token)
                        })
                        .on_close(move |()| *closed.borrow_mut() += 1),
                )
            }
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    settle(&mut test);
    assert!(
        labels(&test).iter().any(|l| *l == consequence.prompt()),
        "{:?}",
        labels(&test)
    );
    let skips = labels(&test)
        .iter()
        .filter(|l| *l == SKIP_HOOKS_CAPTION)
        .count();
    assert_eq!(skips, 1, "the commit's skip drawn beside the amend's");
    click(&mut test, SKIP_HOOKS_CAPTION);
    click(&mut test, SKIP_HOOKS_CAPTION);
    let tokens = tokens.borrow();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].prompt(), consequence.prompt());
    assert_eq!(*closed.borrow(), 1);
}

/// R10.1: Fork's counter beside the subject, and its button naming the files; R10.8: a box an
/// operation in progress disables names it, and its button and Amend ask nothing. Caught by:
/// a disabled box that still commits or amends.
#[test]
fn the_counter_is_drawn_and_a_stopped_box_asks_nothing() {
    let (test, _) = launch(
        false,
        CommitButton::Commit { files: 3 },
        &"x".repeat(88),
        None,
    );
    let shown = labels(&test);
    assert!(shown.iter().any(|l| l == "-38"), "{shown:?}");
    assert!(shown.iter().any(|l| l == "Commit 3 Files"), "{shown:?}");

    let said = "Committing is unavailable while a rebase is in progress.";
    let (mut test, reports) = launch(false, CommitButton::Commit { files: 3 }, "x", Some(said));
    assert!(labels(&test).iter().any(|l| l == said));
    click(&mut test, "Commit 3 Files");
    click(&mut test, AMEND_CAPTION);
    assert_eq!(*reports.commits.borrow(), 0);
    assert!(reports.amend.borrow().is_empty());
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
    assert_eq!(*answers.skipped.borrow(), 1);
}
