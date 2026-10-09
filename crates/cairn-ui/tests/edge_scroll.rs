//! Headless tests for the edge auto-scroll of a drag over a virtualized list
//! (staging-and-commit R7.6): the list, never a row, tracks the drag; a pointer held still at
//! an edge keeps the list scrolling on the timer; away from the edges, or once released, it
//! stops.

use std::time::Duration;

use cairn_ui::{EDGE_BAND, MOST_PER_TICK, edge_step, use_edge_scroll};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 400.;
const HEIGHT: f32 = 600.;
/// Where the list sits in the window: room above and below it for the pointer to pass its
/// edges without leaving the window.
const TOP: f32 = 100.;
const LIST_HEIGHT: f32 = 300.;
const BOTTOM: f32 = TOP + LIST_HEIGHT;
const ROW: f32 = 20.;
const ROWS: usize = 10_000;

fn launch() -> TestingRunner {
    let (mut test, _) = TestingRunner::new(
        || {
            let controller = use_scroll_controller(ScrollConfig::default);
            let mut edge = use_edge_scroll(controller);
            rect()
                .expanded()
                .child(rect().width(Size::fill()).height(Size::px(TOP)))
                .child(
                    edge.on(rect()
                        .width(Size::fill())
                        .height(Size::px(LIST_HEIGHT))
                        // A press on a row starts the drag; the list, not the row, tracks it.
                        .on_pointer_down(move |_| edge.begin()))
                        .child(
                            VirtualScrollView::new_controlled(
                                |item: VirtualItem, _: &()| {
                                    label()
                                        .height(Size::px(ROW))
                                        .text(format!("row {}", item.index))
                                        .into()
                                },
                                controller,
                            )
                            .length(ROWS)
                            .item_size(ROW)
                            .expanded(),
                        ),
                )
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    test.sync_and_update();
    test
}

/// The rows drawn inside the list's viewport, by index.
fn shown(test: &TestingRunner) -> (usize, usize) {
    let rows: Vec<usize> = test.find_many(|node, element| {
        let area = node.layout().area;
        Label::try_downcast(element)
            .filter(|_| area.min_y() >= TOP - ROW && area.max_y() <= BOTTOM + ROW)
            .and_then(|label| label.text.strip_prefix("row ")?.parse().ok())
    });
    (
        *rows.iter().min().unwrap_or_else(|| panic!("no row shown")),
        *rows.iter().max().unwrap_or_else(|| panic!("no row shown")),
    )
}

fn wait(test: &mut TestingRunner, millis: u64) {
    test.poll(Duration::from_millis(5), Duration::from_millis(millis));
}

/// A step's direction and pace: none away from the edges; toward the start in the top band
/// and above it, toward the end in the bottom band and below it; faster the deeper, never
/// past the most; a viewport too short for two bands still scrolls at its edges. Caught by:
/// a band on one edge only, a sign swapped, or a pace that grows without bound.
#[test]
fn a_step_points_at_the_edge_the_pointer_is_at() {
    let (top, bottom) = (TOP, BOTTOM);
    assert_eq!(edge_step((top + bottom) / 2.0, top, bottom), 0.0);
    assert_eq!(edge_step(top + EDGE_BAND, top, bottom), 0.0);
    assert!(edge_step(top + EDGE_BAND - 1.0, top, bottom) < 0.0);
    assert!(edge_step(bottom - 1.0, top, bottom) > 0.0);
    let near = edge_step(bottom - 1.0, top, bottom);
    let past = edge_step(bottom + 10.0, top, bottom);
    assert!(near <= past, "{near} > {past}");
    assert_eq!(edge_step(bottom + 10_000.0, top, bottom), MOST_PER_TICK);
    assert_eq!(edge_step(top - 10_000.0, top, bottom), -MOST_PER_TICK);
    assert!(edge_step(top - 1.0, top, top + 10.0) < 0.0);
    assert!(edge_step(top + 11.0, top, top + 10.0) > 0.0);
}

/// QA item 15: a viewport not yet laid out (`(0, 0)`, before the list's first `on_sized`), a
/// collapsed one and an inverted one scroll nothing wherever the pointer is. Caught by: a band
/// of zero that reads every pointer as past an edge.
#[test]
fn a_viewport_with_no_height_scrolls_nothing() {
    for (top, bottom) in [(0.0, 0.0), (TOP, TOP), (BOTTOM, TOP)] {
        for y in [-1_000.0, -1.0, 0.0, TOP, BOTTOM, 1_000.0] {
            assert_eq!(
                edge_step(y, top, bottom),
                0.0,
                "a pointer at {y} over a viewport {top}..{bottom} scrolled"
            );
        }
    }
}

/// R7.6: a drag begun on a row and moved past the list's bottom keeps the list scrolling while
/// the pointer stands still there — the row it began on long unmounted — and stops once the
/// pointer is back inside; past the top it scrolls back; released, it stops. A pointer at the
/// edge with no drag scrolls nothing. Caught by: scrolling only on pointer moves, a drag kept
/// by the row (gone once it scrolls out), or a timer that outlives the drag.
#[test]
fn a_drag_held_at_an_edge_keeps_the_list_scrolling_until_it_leaves_or_ends() {
    let mut test = launch();
    assert_eq!(shown(&test).0, 0);

    // No drag: the pointer at the edge scrolls nothing.
    test.move_cursor((50., f64::from(BOTTOM) + 20.));
    wait(&mut test, 120);
    assert_eq!(
        shown(&test).0,
        0,
        "a pointer with no drag scrolled the list"
    );

    // A drag from the first row, held still below the list.
    test.press_cursor((50., f64::from(TOP) + 5.));
    test.move_cursor((50., f64::from(BOTTOM) + 20.));
    wait(&mut test, 150);
    let first = shown(&test).0;
    assert!(first > 0, "the list did not scroll at its bottom edge");
    wait(&mut test, 150);
    let later = shown(&test).0;
    assert!(
        later > first,
        "the pointer held still stopped the scroll: {first} then {later}"
    );

    // Back inside: it stops.
    test.move_cursor((50., f64::from(TOP + LIST_HEIGHT / 2.)));
    wait(&mut test, 60);
    let stopped = shown(&test).0;
    wait(&mut test, 150);
    assert_eq!(
        shown(&test).0,
        stopped,
        "the list scrolled with the pointer inside"
    );

    // Past the top: it scrolls back toward the start.
    test.move_cursor((50., f64::from(TOP) - 20.));
    wait(&mut test, 150);
    let back = shown(&test).0;
    assert!(
        back < stopped,
        "the list did not scroll back: {stopped} then {back}"
    );

    // Released at the top edge: it stops there.
    test.release_cursor((50., f64::from(TOP) - 20.));
    wait(&mut test, 60);
    let released = shown(&test).0;
    wait(&mut test, 150);
    assert_eq!(shown(&test).0, released, "the scroll outlived the drag");
}

/// Phase 06's QA item 16: a drag carried out of the window and released there may never be
/// heard, since the toolkit forgets the button as the pointer leaves. The next press — which
/// can only come once that release was made — ends the drag; so does the window losing focus.
/// A drag begun by a press is not ended by that same press. Caught by: a drag that keeps
/// scrolling after a release the window never heard, or one ended by the press that began it.
#[test]
fn a_release_the_window_never_heard_ends_the_drag_at_the_next_press_or_focus_lost() {
    let mut test = launch();
    // Begun by the press itself, held below the list: the press that began it does not end it.
    test.press_cursor((50., f64::from(TOP) + 5.));
    test.move_cursor((50., f64::from(BOTTOM) + 20.));
    wait(&mut test, 120);
    let scrolling = shown(&test).0;
    assert!(
        scrolling > 0,
        "the drag begun by its own press ended at once"
    );

    // The release made outside the window is never heard; the next press, above the list
    // where nothing begins a drag, ends it.
    test.press_cursor((50., f64::from(TOP) - 50.));
    wait(&mut test, 60);
    let ended = shown(&test).0;
    test.move_cursor((50., f64::from(BOTTOM) + 20.));
    wait(&mut test, 150);
    assert_eq!(
        shown(&test).0,
        ended,
        "a drag whose release was lost kept scrolling after the next press"
    );
    test.release_cursor((50., f64::from(TOP) - 50.));
    test.sync_and_update();

    // A drag the window loses focus during ends.
    test.press_cursor((50., f64::from(TOP) + 5.));
    test.move_cursor((50., f64::from(BOTTOM) + 20.));
    wait(&mut test, 60);
    test.run_in(|| Platform::get().is_app_focused.set(false));
    test.sync_and_update();
    wait(&mut test, 30);
    let unfocused = shown(&test).0;
    wait(&mut test, 150);
    assert_eq!(
        shown(&test).0,
        unfocused,
        "a drag kept scrolling after the window lost focus"
    );
}
