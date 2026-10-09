//! Headless tests for Fork's Create Branch dialog (staging-and-commit R11.3; the user's
//! decision, 2026-10-09): its anatomy, the button's caption following the box, the button and
//! Return enabled only for a name the engine said can be created, a refusal said beside the
//! buttons, "Local changes:" drawn only when given, and every change reported.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::Oid;
use cairn_ui::{
    BRANCH_NAME_CAPTION, BRANCH_NAME_PLACEHOLDER, CANCEL_BRANCH_CAPTION, CHECK_OUT_AFTER_CREATE,
    CREATE_AND_CHECKOUT_CAPTION, CREATE_BRANCH_AT, CREATE_BRANCH_TITLE, CREATE_CAPTION,
    CreateBranchDialog, DISCARD_LOCAL_CAPTION, DONT_CHANGE_CAPTION, LOCAL_CHANGES_LABEL,
    LocalChoice,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 600.;

fn at() -> Oid {
    Oid::from_bytes(&[0xab; 20]).unwrap_or_else(|e| panic!("{e}"))
}

fn settle(test: &mut TestingRunner) {
    for _ in 0..4 {
        test.sync_and_update();
    }
}

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

fn click(test: &mut TestingRunner, caption: &str) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == caption)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no label reads {caption:?}; labels: {:?}", labels(test)));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    settle(test);
}

/// What the dialog was drawn with.
#[derive(Clone, Copy)]
struct Shown {
    ready: bool,
    checkout: bool,
    local: Option<LocalChoice>,
}

/// What it reported.
#[derive(Default, Clone)]
struct Reports {
    checkout: Rc<RefCell<Vec<bool>>>,
    local: Rc<RefCell<Vec<LocalChoice>>>,
    created: Rc<RefCell<usize>>,
    cancelled: Rc<RefCell<usize>>,
}

fn launch(shown: Shown, refusal: Option<&str>) -> (TestingRunner, Reports) {
    let reports = Reports::default();
    let refusal = refusal.map(str::to_owned);
    let (mut test, _) = TestingRunner::new(
        {
            let reports = reports.clone();
            move || {
                let name = use_state(|| "topic".to_owned());
                let reports = reports.clone();
                let (checkout, local, created, cancelled) = (
                    reports.checkout.clone(),
                    reports.local.clone(),
                    reports.created.clone(),
                    reports.cancelled.clone(),
                );
                CreateBranchDialog::new(1, at(), "Fix the parser", name)
                    .ready(shown.ready)
                    .refusal(refusal.clone())
                    .checkout(shown.checkout)
                    .local_changes(shown.local)
                    .on_checkout(move |to: bool| checkout.borrow_mut().push(to))
                    .on_local(move |to: LocalChoice| local.borrow_mut().push(to))
                    .on_create(move |()| *created.borrow_mut() += 1)
                    .on_cancel(move |()| *cancelled.borrow_mut() += 1)
                    .into_element()
            }
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    settle(&mut test);
    (test, reports)
}

/// Fork's anatomy: the title, the commit read only (its short id and subject), the name field
/// with its placeholder, the box, Cancel and the button — "Create", or "Create and Checkout"
/// with the box ticked — and "Local changes:" only when the window gives it. Caught by: a
/// caption not Fork's, the button's caption not following the box, or the local changes drawn
/// when there are none.
#[test]
fn the_dialog_is_forks_its_button_following_the_box() {
    let (test, _) = launch(
        Shown {
            ready: true,
            checkout: false,
            local: None,
        },
        None,
    );
    let drawn = labels(&test);
    for caption in [
        CREATE_BRANCH_TITLE,
        CREATE_BRANCH_AT,
        "abababa Fix the parser",
        BRANCH_NAME_CAPTION,
        CHECK_OUT_AFTER_CREATE,
        CANCEL_BRANCH_CAPTION,
        CREATE_CAPTION,
    ] {
        assert!(
            drawn.iter().any(|l| l == caption),
            "{caption:?} in {drawn:?}"
        );
    }
    assert!(!drawn.iter().any(|l| l == LOCAL_CHANGES_LABEL));
    assert!(!drawn.iter().any(|l| l == CREATE_AND_CHECKOUT_CAPTION));
    assert_eq!(BRANCH_NAME_PLACEHOLDER, "Enter branch name");

    let (test, _) = launch(
        Shown {
            ready: true,
            checkout: true,
            local: Some(LocalChoice::Keep),
        },
        None,
    );
    let drawn = labels(&test);
    for caption in [
        CREATE_AND_CHECKOUT_CAPTION,
        LOCAL_CHANGES_LABEL,
        DONT_CHANGE_CAPTION,
        DISCARD_LOCAL_CAPTION,
    ] {
        assert!(
            drawn.iter().any(|l| l == caption),
            "{caption:?} in {drawn:?}"
        );
    }
    assert!(!drawn.iter().any(|l| l == CREATE_CAPTION));
}

/// The button and Return create only for a name the engine said can be; a refusal is said
/// beside the buttons; Cancel and Escape cancel; the box and the choices report their new
/// state. Caught by: a refused name created, Return creating past a disabled button, or a
/// change not reported.
#[test]
fn only_a_ready_name_is_created_and_every_change_is_reported() {
    let (mut test, reports) = launch(
        Shown {
            ready: false,
            checkout: true,
            local: Some(LocalChoice::Keep),
        },
        Some("Branch topic already exists"),
    );
    assert!(
        labels(&test)
            .iter()
            .any(|l| l == "Branch topic already exists")
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert_eq!(*reports.created.borrow(), 0, "a refused name was created");
    click(&mut test, CHECK_OUT_AFTER_CREATE);
    click(&mut test, DISCARD_LOCAL_CAPTION);
    assert_eq!(reports.checkout.borrow().as_slice(), &[false]);
    assert_eq!(reports.local.borrow().as_slice(), &[LocalChoice::Discard]);
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    assert_eq!(*reports.cancelled.borrow(), 1, "Escape cancels");

    let (mut test, reports) = launch(
        Shown {
            ready: true,
            checkout: false,
            local: None,
        },
        None,
    );
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert_eq!(*reports.created.borrow(), 1, "Return creates a ready name");
    click(&mut test, CREATE_CAPTION);
    assert_eq!(*reports.created.borrow(), 2);
    click(&mut test, CANCEL_BRANCH_CAPTION);
    assert_eq!(*reports.cancelled.borrow(), 1);
}
