//! Headless tests for Fork's Create Branch dialog (staging-and-commit R11.3; the user's
//! decisions of 2026-10-09 and 2026-10-10): its anatomy, the button's caption following the box,
//! the button and Return enabled only for a name the engine said can be created, a refusal said
//! beside the buttons behind Fork's warning glyph and worded here, "Local changes:" drawn only
//! when given in Fork's order with "Stash and reapply" greyed and ⚠ beside a chosen Discard,
//! Discard's press building the token from the consequence handed in (a confirmation surface),
//! and every change reported.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{Confirmed, Consequence, NameRefusal, Oid, OperationInProgress};
use cairn_ui::{
    BRANCH_NAME_CAPTION, BRANCH_NAME_PLACEHOLDER, CANCEL_BRANCH_CAPTION, CHECK_OUT_AFTER_CREATE,
    COMES_WITH_STASHING, CREATE_AND_CHECKOUT_CAPTION, CREATE_BRANCH_AT, CREATE_BRANCH_SUBTITLE,
    CREATE_BRANCH_TITLE, CREATE_CAPTION, CreateBranchDialog, DISCARD_LOCAL_CAPTION,
    DONT_CHANGE_CAPTION, GLYPH_SIZE, LOCAL_CHANGES_LABEL, LocalChoice, RefGlyph,
    STASH_AND_REAPPLY_CAPTION, discard_refusal, name_refusal,
};
use freya::engine::prelude::{FontCollection, ImageInfo, raster_n32_premul};
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
    waiting: Option<&'static str>,
}

/// What Discard's press confirms in these tests.
fn discarding() -> Consequence {
    Consequence::CheckoutDiscarding {
        branch: "topic".to_owned(),
        at: at(),
        head: None,
    }
}

/// What it reported.
#[derive(Default, Clone)]
struct Reports {
    checkout: Rc<RefCell<Vec<bool>>>,
    local: Rc<RefCell<Vec<LocalChoice>>>,
    created: Rc<RefCell<usize>>,
    confirmed: Rc<RefCell<Vec<Consequence>>>,
    cancelled: Rc<RefCell<usize>>,
}

fn launch(shown: Shown, refusal: Option<&str>) -> (TestingRunner, Reports) {
    launch_discarding(shown, refusal, None)
}

fn launch_discarding(
    shown: Shown,
    refusal: Option<&str>,
    consequence: Option<Consequence>,
) -> (TestingRunner, Reports) {
    let reports = Reports::default();
    let refusal = refusal.map(str::to_owned);
    let (mut test, _) = TestingRunner::new(
        {
            let reports = reports.clone();
            move || {
                let name = use_state(|| "topic".to_owned());
                let reports = reports.clone();
                let (checkout, local, created, confirmed, cancelled) = (
                    reports.checkout.clone(),
                    reports.local.clone(),
                    reports.created.clone(),
                    reports.confirmed.clone(),
                    reports.cancelled.clone(),
                );
                CreateBranchDialog::new(1, at(), "Fix the parser", name)
                    .ready(shown.ready)
                    .refusal(refusal.clone())
                    .waiting(shown.waiting.map(str::to_owned))
                    .checkout(shown.checkout)
                    .local_changes(shown.local)
                    .discarding(consequence.clone())
                    .on_checkout(move |to: bool| checkout.borrow_mut().push(to))
                    .on_local(move |to: LocalChoice| local.borrow_mut().push(to))
                    .on_create(move |()| *created.borrow_mut() += 1)
                    .on_confirmed(move |token: Confirmed| {
                        confirmed.borrow_mut().push(token.consequence().clone())
                    })
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

/// Fork's anatomy: the title and its subtitle, the commit read only (its short id and subject), the name field
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
            waiting: None,
        },
        None,
    );
    let drawn = labels(&test);
    for caption in [
        CREATE_BRANCH_TITLE,
        CREATE_BRANCH_SUBTITLE,
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
    assert_eq!(
        CREATE_BRANCH_SUBTITLE,
        "Use '/' as a path separator to create folders"
    );

    let (test, _) = launch(
        Shown {
            ready: true,
            checkout: true,
            local: Some(LocalChoice::Keep),
            waiting: None,
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
            waiting: None,
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
            waiting: None,
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

/// Where `caption` is drawn: its label's area.
fn area_of(test: &TestingRunner, caption: &str) -> Area {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == caption)
            .map(|_| node.layout().area)
    })
    .unwrap_or_else(|| panic!("no label reads {caption:?}; labels: {:?}", labels(test)))
}

/// Which pixels of a `GLYPH_SIZE` square a canvas's callback paints.
fn painted(on_render: &RenderCallback) -> Vec<bool> {
    let side = GLYPH_SIZE as i32;
    let mut surface =
        raster_n32_premul((side, side)).unwrap_or_else(|| panic!("no raster surface"));
    let mut fonts = FontCollection::new();
    let style = TextStyleState::default();
    let mut context = CanvasContext {
        canvas: surface.canvas(),
        font_collection: &mut fonts,
        size: Size2D::new(GLYPH_SIZE, GLYPH_SIZE),
        text_style_state: &style,
    };
    on_render.call(&mut context);
    let info = ImageInfo::new_n32_premul((side, side), None);
    let stride = info.min_row_bytes();
    let mut pixels = vec![0u8; stride * side as usize];
    assert!(surface.read_pixels(&info, &mut pixels, stride, (0, 0)));
    (0..side as usize)
        .flat_map(|y| (0..side as usize).map(move |x| (x, y)))
        .map(|(x, y)| pixels.get(y * stride + x * 4 + 3).copied().unwrap_or(0) > 96)
        .collect()
}

/// Each canvas drawn, by its area, with what it paints.
fn glyphs(test: &TestingRunner) -> Vec<(Area, Vec<bool>)> {
    let canvases: Vec<(Area, CanvasElement)> = test.find_many(|node, element| {
        Canvas::try_downcast(element).map(|canvas| (node.layout().area, canvas))
    });
    canvases
        .into_iter()
        .map(|(area, canvas)| (area, painted(&canvas.on_render)))
        .collect()
}

/// What Fork's warning glyph paints: the warning triangle every refusal and conflict carries.
fn warning() -> Vec<bool> {
    let element: Element = RefGlyph::Gone.draw(Color::WHITE).into();
    let Element::Element { element, .. } = element else {
        panic!("a glyph is an element");
    };
    let canvas = Canvas::try_downcast(&*element).unwrap_or_else(|| panic!("a glyph is a canvas"));
    painted(&canvas.on_render)
}

/// Fork's refusal (USHOT TrackerWin #2472, Tracker #1911; phase 10's QA, items 19 and 20): said
/// in the buttons' row, left of them, behind the warning triangle — and no triangle without a
/// refusal. Caught by: the refusal in a row of its own above the buttons, drawn right of them,
/// or without its glyph.
#[test]
fn a_refusal_is_said_beside_the_buttons_behind_the_warning_glyph() {
    let refused = "Branch topic already exists";
    let (test, _) = launch(
        Shown {
            ready: false,
            checkout: false,
            local: None,
            waiting: None,
        },
        Some(refused),
    );
    let (said, cancel) = (
        area_of(&test, refused),
        area_of(&test, CANCEL_BRANCH_CAPTION),
    );
    assert!(
        (said.center().y - cancel.center().y).abs() < 4.,
        "the refusal at {said:?} is not in the buttons' row, at {cancel:?}"
    );
    assert!(
        said.max_x() < cancel.min_x(),
        "the refusal at {said:?} is not left of the buttons, at {cancel:?}"
    );
    let drawn = glyphs(&test);
    assert!(
        drawn.iter().any(|(area, mask)| *mask == warning()
            && (area.center().y - said.center().y).abs() < 4.
            && area.max_x() <= said.min_x()),
        "no warning glyph before the refusal: {:?}",
        drawn.iter().map(|(area, _)| area).collect::<Vec<_>>()
    );

    let (test, _) = launch(
        Shown {
            ready: true,
            checkout: false,
            local: None,
            waiting: None,
        },
        None,
    );
    assert!(
        !glyphs(&test).iter().any(|(_, mask)| *mask == warning()),
        "a warning glyph with nothing refused"
    );
}

/// The user's decision D (2026-10-09): while the name's check waits behind a running write, the
/// dialog says so where a refusal goes, beside the buttons — with no warning glyph, since
/// nothing is refused — and a refusal, when there is one, is said instead. Caught by: the
/// waiting line drawn elsewhere or behind the triangle, or drawn over a refusal.
#[test]
fn a_check_waiting_behind_a_write_is_said_beside_the_buttons() {
    let waiting = "Waiting for the commit to finish…";
    let (test, _) = launch(
        Shown {
            ready: false,
            checkout: false,
            local: None,
            waiting: Some(waiting),
        },
        None,
    );
    let (said, cancel) = (
        area_of(&test, waiting),
        area_of(&test, CANCEL_BRANCH_CAPTION),
    );
    assert!(
        (said.center().y - cancel.center().y).abs() < 4. && said.max_x() < cancel.min_x(),
        "the waiting line at {said:?} is not beside the buttons, at {cancel:?}"
    );
    assert!(
        !glyphs(&test).iter().any(|(_, mask)| *mask == warning()),
        "a warning glyph with nothing refused"
    );

    let (test, _) = launch(
        Shown {
            ready: false,
            checkout: false,
            local: None,
            waiting: Some(waiting),
        },
        Some("Branch topic already exists"),
    );
    let drawn = labels(&test);
    assert!(drawn.iter().any(|l| l == "Branch topic already exists"));
    assert!(!drawn.iter().any(|l| l == waiting), "{drawn:?}");
}

/// Fork's "Local changes:" (observed cb2, cb3; B4): Don't change, Stash and reapply, Discard, in
/// Fork's order; Stash and reapply greyed with "Comes with stashing." beside it, and a press on
/// it reports nothing; Fork's ⚠ beside Discard only while it is chosen. Caught by: the order not
/// Fork's, Stash and reapply missing or pressable, its reason unsaid, or the ⚠ drawn for Don't
/// change or missing for Discard.
#[test]
fn local_changes_are_forks_three_in_forks_order_stash_greyed_and_a_chosen_discard_warned() {
    let shown = |local| Shown {
        ready: true,
        checkout: true,
        local: Some(local),
        waiting: None,
    };
    let (mut test, reports) = launch(shown(LocalChoice::Keep), None);
    let (keep, stash, discard) = (
        area_of(&test, DONT_CHANGE_CAPTION),
        area_of(&test, STASH_AND_REAPPLY_CAPTION),
        area_of(&test, DISCARD_LOCAL_CAPTION),
    );
    assert!(
        keep.center().y < stash.center().y && stash.center().y < discard.center().y,
        "not in Fork's order: {keep:?} {stash:?} {discard:?}"
    );
    let reason = area_of(&test, COMES_WITH_STASHING);
    assert!(
        (reason.center().y - stash.center().y).abs() < 4. && reason.min_x() > stash.max_x(),
        "the reason is not beside Stash and reapply: {reason:?} {stash:?}"
    );
    assert!(
        !glyphs(&test).iter().any(|(_, mask)| *mask == warning()),
        "a warning with Don't change chosen"
    );
    click(&mut test, STASH_AND_REAPPLY_CAPTION);
    assert!(
        reports.local.borrow().is_empty(),
        "the greyed choice reported"
    );

    let (test, _) = launch(shown(LocalChoice::Discard), None);
    let discard = area_of(&test, DISCARD_LOCAL_CAPTION);
    assert!(
        glyphs(&test).iter().any(|(area, mask)| *mask == warning()
            && (area.center().y - discard.center().y).abs() < 4.
            && area.min_x() >= discard.max_x()),
        "no ⚠ beside the chosen Discard"
    );
}

/// C34 and B2: with Discard chosen, the button — and Return in the name field — IS the
/// confirmation: the dialog builds the token from the consequence it was handed, and reports it
/// rather than a create; with no consequence handed, or the name not ready, the press does
/// nothing; with Don't change it reports a create and builds no token. Caught by: a token built
/// from anything but the consequence handed in, a create reported for Discard, a token for Don't
/// change, or a press past a missing consequence.
#[test]
fn discards_press_builds_the_token_from_the_consequence_it_was_handed() {
    let discard = Shown {
        ready: true,
        checkout: true,
        local: Some(LocalChoice::Discard),
        waiting: None,
    };
    let (mut test, reports) = launch_discarding(discard, None, Some(discarding()));
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert_eq!(
        reports.confirmed.borrow().as_slice(),
        &[discarding(), discarding()],
        "the press and Return each confirm"
    );
    assert_eq!(
        *reports.created.borrow(),
        0,
        "a create reported for Discard"
    );

    let (mut test, reports) = launch_discarding(discard, None, None);
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert!(
        reports.confirmed.borrow().is_empty(),
        "confirmed with nothing to confirm"
    );
    assert_eq!(*reports.created.borrow(), 0);

    let (mut test, reports) = launch_discarding(
        Shown {
            ready: false,
            ..discard
        },
        None,
        Some(discarding()),
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert!(
        reports.confirmed.borrow().is_empty(),
        "confirmed past a refused name"
    );

    let (mut test, reports) = launch_discarding(
        Shown {
            local: Some(LocalChoice::Keep),
            ..discard
        },
        None,
        Some(discarding()),
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert!(
        reports.confirmed.borrow().is_empty(),
        "Don't change built a token"
    );
    assert_eq!(*reports.created.borrow(), 1);
}

/// The review's M3: the engine's refusal is typed and worded here — Fork's words for a name
/// taken, decision F's for `@{`, git's for one it refuses or cannot lock — and Discard's refusal
/// during an operation in the template the user chose (2026-10-10). Caught by: a refusal worded
/// otherwise, or an operation named in another's words.
#[test]
fn refusals_are_worded_by_the_view() {
    assert_eq!(
        name_refusal("test", &NameRefusal::Taken),
        "Branch test already exists"
    );
    assert_eq!(
        name_refusal("a@{b", &NameRefusal::AtBrace),
        "A branch name can't contain '@{'"
    );
    assert_eq!(
        name_refusal(
            "a..b",
            &NameRefusal::Invalid {
                reason: "'a..b' is not a valid branch name".to_owned()
            }
        ),
        "'a..b' is not a valid branch name"
    );
    assert_eq!(
        name_refusal(
            "leaf/child",
            &NameRefusal::InsideABranch {
                branch: "refs/heads/leaf".to_owned()
            }
        ),
        "'refs/heads/leaf' exists; cannot create 'refs/heads/leaf/child'"
    );
    assert_eq!(
        name_refusal(
            "folder",
            &NameRefusal::HoldsABranch {
                branch: "refs/heads/folder/inner".to_owned()
            }
        ),
        "'refs/heads/folder/inner' exists; cannot create 'refs/heads/folder'"
    );
    for (operation, said) in [
        (
            OperationInProgress::Merge,
            "A merge is in progress. Finish or abort it first.",
        ),
        (
            OperationInProgress::CherryPick { picked: None },
            "A cherry-pick is in progress. Finish or abort it first.",
        ),
        (
            OperationInProgress::CherryPickSequence,
            "A cherry-pick is in progress. Finish or abort it first.",
        ),
        (
            OperationInProgress::Revert { reverted: None },
            "A revert is in progress. Finish or abort it first.",
        ),
        (
            OperationInProgress::RevertSequence,
            "A revert is in progress. Finish or abort it first.",
        ),
        (
            OperationInProgress::Rebase,
            "A rebase is in progress. Finish or abort it first.",
        ),
        (
            OperationInProgress::ApplyingPatches,
            "An am session is in progress. Finish or abort it first.",
        ),
    ] {
        assert_eq!(discard_refusal(&operation), said);
    }
}
