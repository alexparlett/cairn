//! Walking a window's Tab stops and reading whether each shows where focus is: a stop shows
//! focus when, as focus reaches it, a mark appears inside it that was not drawn a stop before —
//! the style's 1 px ring in the focused field's blue (`style::focus::ring`) or a selection's
//! band in the focused tone. A mark drawn whether or not the control has focus — a default
//! button's blue border — is drawn a stop before too, so it never passes for one.

use cairn_ui::style::tones;
use freya::prelude::*;
use freya_testing::TestingRunner;

/// An area as whole pixels, to compare marks between frames.
type Place = (i32, i32, i32, i32);

fn place(area: Area) -> Place {
    (
        area.min_x().round() as i32,
        area.min_y().round() as i32,
        area.width().round() as i32,
        area.height().round() as i32,
    )
}

/// The places of every focus mark drawn now.
fn marks(test: &TestingRunner) -> Vec<Place> {
    test.find_many(|node, element| {
        let rect = Rect::try_downcast(element)?;
        let ringed = rect
            .style
            .borders
            .iter()
            .any(|border| border.fill == tones::ACCENT);
        let band = rect.style.background == Fill::Color(tones::SELECTION_FOCUSED);
        (ringed || band).then(|| place(node.layout().area))
    })
}

/// What holds focus now, by its accessibility id, and the area of the rect that carries it —
/// `None` when no rect does.
fn focused(test: &TestingRunner) -> (AccessibilityId, Option<Area>) {
    let id = test.run_in(|| *Platform::get().focused_accessibility_id.peek());
    let area = test.find(|node, element| {
        Rect::try_downcast(element)
            .filter(|rect| rect.accessibility.a11y_id == Some(id))
            .map(|_| node.layout().area)
    });
    (id, area)
}

fn inside(mark: Place, area: Area) -> bool {
    let (x, y, width, height) = mark;
    let outer = place(area);
    x >= outer.0
        && y >= outer.1
        && x + width <= outer.0 + outer.2
        && y + height <= outer.1 + outer.3
}

/// One Tab stop: its area, and whether it showed focus as focus reached it.
#[derive(Debug)]
pub struct Stop {
    pub area: Area,
    pub shows: bool,
}

/// Presses Tab until focus comes back to the first stop it reached, at most `most` times,
/// reading each stop. Each press settles before it is read. The window's navigation is the
/// keyboard's from the first press, as the windowing layer sets it on Tab; the headless runner
/// does not, so it is set here.
pub fn walk_tab_stops(test: &mut TestingRunner, most: usize) -> Vec<Stop> {
    let mut stops: Vec<(AccessibilityId, Stop)> = Vec::new();
    let mut before = marks(test);
    test.run_in(|| {
        Platform::get()
            .navigation_mode
            .set(NavigationMode::Keyboard)
    });
    for _ in 0..most {
        test.press_key(Key::Named(NamedKey::Tab));
        for _ in 0..3 {
            test.sync_and_update();
        }
        let now = marks(test);
        let (id, area) = focused(test);
        if stops.first().is_some_and(|(first, _)| *first == id) {
            break;
        }
        // Focus on something drawn by no rect is a stop nobody can see: it shows nothing.
        let shows = area.is_some_and(|area| {
            now.iter()
                .any(|mark| !before.contains(mark) && inside(*mark, area))
        });
        stops.push((
            id,
            Stop {
                area: area.unwrap_or_default(),
                shows,
            },
        ));
        before = now;
    }
    stops.into_iter().map(|(_, stop)| stop).collect()
}
