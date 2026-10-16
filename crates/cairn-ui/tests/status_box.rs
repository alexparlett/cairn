//! Headless test of the status box's progress: an operation running is shown — its one name,
//! the writes waiting behind it — only once it has run 250 ms, the wait a task on the toolkit's
//! executor, driven here by the runner's poll.

use std::time::{Duration, Instant};

use cairn_ui::{PROGRESS_AFTER, Running, StatusBox, progress_shown};
use freya::prelude::*;
use freya_testing::TestingRunner;

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|label| label.text.to_string()))
}

/// Nothing before 250 ms, "Commit 1 file · 1 waiting" after, without anything else redrawing
/// the box; and an operation already past 250 ms as it is drawn shows at once. Caught by: the
/// name drawn at once (a flash for every stage), or never drawn because nothing wakes the box.
#[test]
fn the_status_box_shows_an_operation_only_once_it_has_run_250_ms() {
    assert!(!progress_shown(Duration::from_millis(249)));
    assert!(progress_shown(PROGRESS_AFTER));
    let started = Instant::now();
    let (mut test, _) = TestingRunner::new(
        move || {
            rect().expanded().child(
                StatusBox::new(Some("engine".to_owned()), None).running(vec![Running {
                    serial: 1,
                    name: "Commit 1 file".to_owned(),
                    started,
                    waiting: 1,
                    progress: None,
                }]),
            )
        },
        (800., 60.).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    if started.elapsed() < PROGRESS_AFTER {
        assert!(
            !labels(&test).iter().any(|l| l.starts_with("Commit")),
            "shown before 250 ms: {:?}",
            labels(&test)
        );
    }
    test.poll(Duration::from_millis(10), Duration::from_millis(400));
    test.sync_and_update();
    assert!(
        labels(&test)
            .iter()
            .any(|l| l == "Commit 1 file · 1 waiting"),
        "not shown after 250 ms: {:?}",
        labels(&test)
    );

    let long_ago = Instant::now() - Duration::from_secs(5);
    let (mut test, _) = TestingRunner::new(
        move || {
            rect().expanded().child(
                StatusBox::new(Some("engine".to_owned()), None).running(vec![Running {
                    serial: 2,
                    name: "Fetch 'origin'".to_owned(),
                    started: long_ago,
                    waiting: 0,
                    progress: Some("Receiving objects: 40%".to_owned()),
                }]),
            )
        },
        (800., 60.).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    assert!(
        labels(&test)
            .iter()
            .any(|l| l == "Fetch 'origin'  Receiving objects: 40%"),
        "{:?}",
        labels(&test)
    );
}
