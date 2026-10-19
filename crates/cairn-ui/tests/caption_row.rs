//! Headless tests for the window's top row (`cairn_ui::CaptionRow`): Fork's caption row where
//! Cairn draws the frame, and the same menus alone inside the desktop's frame.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_ui::menu::{BarMenu, Item};
use cairn_ui::style::metrics;
use cairn_ui::{CAPTION_TITLE, CaptionRow, Frame, FrameRequest};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{MouseEventName, PlatformEvent};

const WIDTH: f32 = 1024.;
const HEIGHT: f32 = 300.;
/// Freya's window controls, each this wide (its `titlebar_button` theme).
const CONTROL: f64 = 46.;

const MENUS: [&str; 5] = ["File", "View", "Repository", "Window", "Help"];

type Asked = Rc<RefCell<Vec<FrameRequest>>>;

fn launch(frame: Frame) -> (TestingRunner, Asked) {
    let asked = Asked::default();
    let asking = asked.clone();
    let (mut test, ()) = TestingRunner::new(
        move || {
            let asking = asking.clone();
            rect()
                .expanded()
                .child(
                    CaptionRow::new(
                        MENUS
                            .iter()
                            .map(|title| BarMenu::new(*title, vec![vec![Item::new("Item", ())]]))
                            .collect(),
                    )
                    .frame(frame)
                    .on_frame(move |request: FrameRequest| asking.borrow_mut().push(request)),
                )
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    (test, asked)
}

/// Each label drawn: its text, its left and right, and its vertical centre.
fn labels(test: &TestingRunner) -> Vec<(String, f32, f32, f32)> {
    test.find_many(|node, element| {
        Label::try_downcast(element).map(|label| {
            let area = node.layout().area;
            (
                label.text.to_string(),
                area.min_x(),
                area.max_x(),
                area.center().y,
            )
        })
    })
}

fn left_of(test: &TestingRunner, text: &str) -> Option<f32> {
    labels(test)
        .into_iter()
        .find(|(drawn, ..)| drawn == text)
        .map(|(_, x, ..)| x)
}

fn asked(asked: &Asked) -> Vec<FrameRequest> {
    std::mem::take(&mut *asked.borrow_mut())
}

fn secondary_press(test: &mut TestingRunner, at: (f64, f64)) {
    test.move_cursor(at);
    for name in [MouseEventName::MouseDown, MouseEventName::MouseUp] {
        test.send_event(PlatformEvent::Mouse {
            name,
            cursor: at.into(),
            button: Some(MouseButton::Right),
        });
        test.sync_and_update();
    }
}

/// Where Cairn draws the frame, one 26 px row holds Fork's five menus left to right at its left
/// edge, "Cairn" centred on the window, and minimize, maximize and close at its right edge, each
/// control asking the window for what it names; inside the desktop's frame the same menus are
/// drawn alone — no title, no controls. Caught by: the menus out of order or off the left edge,
/// a title off centre, a control missing or asking for another's, or a caption row drawn inside
/// a frame that already has one.
#[test]
fn the_caption_row_draws_the_menus_the_title_and_the_controls() {
    let (mut test, requests) = launch(Frame::Drawn);
    let lefts: Vec<f32> = MENUS
        .iter()
        .map(|menu| left_of(&test, menu).unwrap_or_else(|| panic!("{menu} is not drawn")))
        .collect();
    assert!(lefts[0] < 20., "File starts at {}", lefts[0]);
    assert!(
        lefts.windows(2).all(|pair| pair[0] < pair[1]),
        "the menus out of order: {lefts:?}"
    );
    let (_, title_left, title_right, title_y) = labels(&test)
        .into_iter()
        .find(|(text, ..)| text == CAPTION_TITLE)
        .unwrap_or_else(|| panic!("no title"));
    let centre = (title_left + title_right) / 2.;
    assert!(
        (centre - WIDTH / 2.).abs() < 2.,
        "the title is centred at {centre}"
    );
    assert!(
        title_y < metrics::CAPTION_ROW,
        "the title is not in the row"
    );

    let y = f64::from(metrics::CAPTION_ROW / 2.);
    let right = f64::from(WIDTH);
    for (control, request) in [
        (1., FrameRequest::Close),
        (2., FrameRequest::ToggleMaximize),
        (3., FrameRequest::Minimize),
    ] {
        test.click_cursor((right - CONTROL * control + CONTROL / 2., y));
        test.sync_and_update();
        assert_eq!(
            asked(&requests),
            [request],
            "control {control} from the right"
        );
    }

    let (mut test, requests) = launch(Frame::Native);
    for menu in MENUS {
        assert!(left_of(&test, menu).is_some(), "{menu} is not drawn");
    }
    assert!(
        left_of(&test, CAPTION_TITLE).is_none(),
        "a title inside a frame"
    );
    test.click_cursor((right - CONTROL / 2., y));
    test.click_cursor((700., y));
    test.sync_and_update();
    assert_eq!(
        asked(&requests),
        [],
        "a native frame's row asks for nothing"
    );
}

/// A press on the caption row's empty space, or on its title, asks the window to follow the
/// pointer; a double press asks it to maximize or restore; a secondary press asks for the window
/// menu where it landed; a press on a menu's name opens the menu and asks nothing of the window.
/// Caught by: a drag that does not start, a double press read as two drags, the window menu
/// asked nowhere, or a menu's name that drags the window.
#[test]
fn a_drag_on_the_caption_row_moves_the_window() {
    let (mut test, requests) = launch(Frame::Drawn);
    let y = f64::from(metrics::CAPTION_ROW / 2.);
    test.click_cursor((700., y));
    test.sync_and_update();
    assert_eq!(asked(&requests), [FrameRequest::Drag]);
    test.click_cursor((700., y));
    test.sync_and_update();
    assert_eq!(asked(&requests), [FrameRequest::ToggleMaximize]);

    secondary_press(&mut test, (300., y));
    assert_eq!(asked(&requests), [FrameRequest::WindowMenu { x: 300., y }]);

    let title = labels(&test)
        .into_iter()
        .find(|(text, ..)| text == CAPTION_TITLE)
        .map(|(_, left, ..)| f64::from(left) + 5.)
        .unwrap_or_else(|| panic!("no title"));
    test.click_cursor((title, y));
    test.sync_and_update();
    assert_eq!(
        asked(&requests),
        [FrameRequest::Drag],
        "the title drags as the row does"
    );

    let file = left_of(&test, "File").map(|x| f64::from(x) + 5.);
    let Some(file) = file else {
        panic!("File is not drawn");
    };
    test.click_cursor((file, y));
    test.sync_and_update();
    assert_eq!(asked(&requests), [], "a menu's name dragged the window");
    assert!(
        labels(&test).iter().any(|(text, ..)| text == "Item"),
        "File's menu did not open"
    );
}
