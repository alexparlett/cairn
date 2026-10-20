//! Headless tests of the status box: the repository's name over the branch, and an operation
//! running shown in their place — its one name, the writes waiting behind it, git's progress line
//! under it — only once it has run 250 ms, the wait a task on the toolkit's executor, driven here
//! by the runner's poll.

use std::time::{Duration, Instant};

use cairn_model::{HeadState, RefName};
use cairn_ui::{PROGRESS_AFTER, Running, StatusBox, Tracking, UPSTREAM_GONE, progress_shown};
use freya::prelude::*;
use freya_testing::TestingRunner;

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|label| label.text.to_string()))
}

fn area(test: &TestingRunner, text: &str) -> Area {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| node.layout().area)
    })
    .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)))
}

fn boxed(status: StatusBox) -> TestingRunner {
    let (mut test, _) = TestingRunner::new(
        move || {
            rect()
                .width(Size::px(300.))
                .height(Size::px(36.))
                .child(status.clone())
        },
        (300., 36.).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    test
}

fn main_branch() -> Option<HeadState> {
    Some(HeadState::Branch(RefName::new("refs/heads/main")))
}

/// The repository's name, bold, on the first line, the branch on the second, both centred; a gone
/// upstream said after the branch. Caught by: the two on one line, the name not bold, or the gone
/// upstream left unsaid.
#[test]
fn the_box_draws_the_name_bold_over_the_branch() {
    let test = boxed(StatusBox::new(Some("engine".to_owned()), main_branch()));
    let (name, branch) = (area(&test, "engine"), area(&test, "main"));
    assert!(name.max_y() <= branch.min_y() + 1., "{name:?} {branch:?}");
    assert!(
        (name.center().x - 150.).abs() < 10.,
        "the name is not centred: {name:?}"
    );
    let bold = test.find(|_, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == "engine")
            .map(|label| label.text_style_data.font_weight == Some(FontWeight::BOLD))
    });
    assert_eq!(bold, Some(true), "the name is not bold");
    assert!(!labels(&test).iter().any(|text| text == UPSTREAM_GONE));

    let test =
        boxed(StatusBox::new(Some("engine".to_owned()), main_branch()).tracking(Tracking::Gone));
    let (branch, gone) = (area(&test, "main"), area(&test, UPSTREAM_GONE));
    assert!(
        (gone.center().y - branch.center().y).abs() < 2. && gone.min_x() > branch.max_x(),
        "the gone upstream is not after the branch: {branch:?} {gone:?}"
    );
}

/// Nothing but the repository before 250 ms; after it, "Commit 1 file · 1 waiting" in the
/// repository's place, without anything else redrawing the box; and an operation already past
/// 250 ms as it is drawn shows at once, its name over git's progress line. Caught by: the name
/// drawn at once (a flash for every stage), never drawn because nothing wakes the box, the
/// repository kept beside the operation, or the progress line run into the name.
#[test]
fn the_status_box_shows_an_operation_only_once_it_has_run_250_ms() {
    assert!(!progress_shown(Duration::from_millis(249)));
    assert!(progress_shown(PROGRESS_AFTER));
    let started = Instant::now();
    let (mut test, _) = TestingRunner::new(
        move || {
            rect().expanded().child(
                StatusBox::new(Some("engine".to_owned()), main_branch()).running(vec![Running {
                    serial: 1,
                    name: "Commit 1 file".to_owned(),
                    started,
                    waiting: 1,
                    progress: None,
                }]),
            )
        },
        (300., 36.).into(),
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
        assert!(labels(&test).iter().any(|l| l == "engine"));
    }
    test.poll(Duration::from_millis(10), Duration::from_millis(400));
    test.sync_and_update();
    let shown = labels(&test);
    assert!(
        shown.iter().any(|l| l == "Commit 1 file · 1 waiting"),
        "not shown after 250 ms: {shown:?}"
    );
    assert!(
        !shown.iter().any(|l| l == "engine"),
        "the repository is drawn beside the operation: {shown:?}"
    );

    let long_ago = Instant::now() - Duration::from_secs(5);
    let test = boxed(
        StatusBox::new(Some("engine".to_owned()), main_branch()).running(vec![Running {
            serial: 2,
            name: "Fetch 'origin'".to_owned(),
            started: long_ago,
            waiting: 0,
            progress: Some("Receiving objects: 40%".to_owned()),
        }]),
    );
    let (name, progress) = (
        area(&test, "Fetch 'origin'"),
        area(&test, "Receiving objects: 40%"),
    );
    assert!(
        name.max_y() <= progress.min_y() + 1.,
        "the name is not over the progress line: {name:?} {progress:?}"
    );
}

/// In a box of Fork's 300 px, git's long progress line is drawn to its percentage and fits whole;
/// an operation's name is drawn whole, and only a name wider than the box — one carrying a long
/// branch name — is cut with "…" inside it, the whole name kept in the label for the popover to
/// say. Caught by: the full progress line drawn and cut, the name shortened in the data, or an
/// overlong name spilling out of the box.
#[test]
fn a_long_progress_line_is_kept_to_its_percentage_and_only_an_overlong_name_is_cut() {
    let long_ago = Instant::now() - Duration::from_secs(5);
    let running = |name: &str, progress: Option<&str>| {
        StatusBox::new(Some("engine".to_owned()), main_branch()).running(vec![Running {
            serial: 3,
            name: name.to_owned(),
            started: long_ago,
            waiting: 0,
            progress: progress.map(str::to_owned),
        }])
    };
    let test = boxed(running(
        "Fetch 'origin'",
        Some("Receiving objects:  45% (12345/27000), 12.00 MiB | 3.00 MiB/s"),
    ));
    let shown = labels(&test);
    assert!(
        shown.iter().any(|l| l == "Receiving objects: 45%"),
        "{shown:?}"
    );
    assert!(!shown.iter().any(|l| l.contains("MiB")), "{shown:?}");
    let progress = area(&test, "Receiving objects: 45%");
    assert!(progress.max_x() <= 300., "{progress:?}");
    let name = area(&test, "Fetch 'origin'");
    assert!(name.max_x() <= 300., "a short name is cut: {name:?}");

    let long = "Create branch 'feature/a-branch-name-far-too-long-for-the-status-box'";
    let test = boxed(running(long, None));
    let (at, overflow) = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == long)
                .map(|label| (node.layout().area, label.text_style_data.text_overflow))
        })
        .unwrap_or_else(|| panic!("the whole name is not the label's: {:?}", labels(&test)));
    assert!(at.max_x() <= 300., "the name spills out of the box: {at:?}");
    assert_eq!(
        overflow,
        Some(TextOverflow::Ellipsis),
        "the cut is not marked"
    );
}
