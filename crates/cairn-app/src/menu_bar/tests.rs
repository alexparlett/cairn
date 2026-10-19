//! The menu bar in the window, headless: Fork's menus holding only what Cairn does, each item
//! doing its action, Zoom In and Out, and About Cairn under the one modal state.

use std::sync::Arc;

use cairn_model::{HeadState, Oid, RefsSnapshot};
use cairn_ui::accelerators::Action;
use cairn_ui::menu::{BarMenu, ItemKind};
use cairn_ui::{MainView, NO_GIT_FOUND, licence_line, version_line};
use freya::prelude::*;
use freya_testing::TestingRunner;

use super::{BarChoice, choose, menus};
use crate::local_changes::tests::{Submitted, apply, labels, launch};
use crate::window::View;
use crate::window::tests::press_chord;
use crate::worker::{Request, Update};

/// Each menu as data a test compares: its name, and each group's items as their labels, the
/// action whose chord they show, whether they are ticked and why they are greyed.
type Shape = Vec<(
    String,
    Vec<Vec<(String, Option<Action>, bool, Option<String>)>>,
)>;

fn shape(menus: &[BarMenu<BarChoice>]) -> Shape {
    menus
        .iter()
        .map(|menu| {
            (
                menu.title.clone(),
                menu.groups
                    .iter()
                    .map(|group| {
                        group
                            .iter()
                            .map(|item| {
                                (
                                    item.label.clone(),
                                    item.chord,
                                    item.ticked,
                                    item.reason.clone(),
                                )
                            })
                            .collect()
                    })
                    .collect(),
            )
        })
        .collect()
}

fn menus_now(test: &TestingRunner, view: View, open: bool) -> Vec<BarMenu<BarChoice>> {
    test.run_in(|| menus(view, open))
}

/// The item labelled `label`'s choice.
fn choice_of(menus: &[BarMenu<BarChoice>], label: &str) -> BarChoice {
    menus
        .iter()
        .flat_map(|menu| menu.groups.iter().flatten())
        .find(|item| item.label == label)
        .and_then(|item| match &item.kind {
            ItemKind::Choose(choice) => Some(*choice),
            ItemKind::Submenu(_) => None,
        })
        .unwrap_or_else(|| panic!("no item {label}"))
}

fn chose(test: &mut TestingRunner, view: View, submitted: &Submitted, label: &str) {
    let choice = choice_of(&menus_now(test, view, true), label);
    let submitting = Submitted::clone(submitted);
    test.run_in(|| {
        choose(
            choice,
            view,
            Some(&move |request| submitting.borrow_mut().push(request)),
        );
    });
    for _ in 0..3 {
        test.sync_and_update();
    }
}

fn row(label: &str, chord: Option<Action>) -> (String, Option<Action>, bool, Option<String>) {
    (label.to_owned(), chord, false, None)
}

fn greyed(
    label: &str,
    chord: Option<Action>,
    why: &str,
) -> (String, Option<Action>, bool, Option<String>) {
    (label.to_owned(), chord, false, Some(why.to_owned()))
}

/// The last refresh's refs: `HEAD` detached at `at`.
fn head_at(test: &mut TestingRunner, view: View, at: Oid) {
    let mut refreshed = view.refreshed;
    let _ = refreshed.write().refs_arrived(Arc::new(RefsSnapshot {
        refs: Vec::new(),
        head: HeadState::Detached(at),
        stashes: Vec::new(),
        unreadable: 0,
        substitutions: cairn_model::Substitutions::default(),
    }));
    for _ in 0..3 {
        test.sync_and_update();
    }
}

/// The menus are Fork's File, View, Repository, Window and Help, in its order and groups, holding
/// exactly the items Cairn does — Hide Commit Details showing no chord until it has one — each
/// greyed with its reason when it cannot act: Show HEAD and New Branch… until the refs are read,
/// Fetch with no remote and while one runs, everything a repository answers while none is open.
/// Caught by: an item Cairn does not do, an item out of Fork's order or
/// group, a chord typed rather than the action's, or an item offered that does nothing.
#[test]
fn the_menu_bar_holds_only_what_works_in_forks_order() {
    let (mut test, view, submitted) = launch();
    let unread = crate::history::head::REFS_UNREAD;
    let expected = |fetch: (String, Option<String>)| -> Shape {
        vec![
            ("File".to_owned(), vec![vec![row("Exit", None)]]),
            (
                "View".to_owned(),
                vec![
                    vec![
                        row("Show Uncommitted Changes", Some(Action::ShowLocalChanges)),
                        row("Show All Commits", Some(Action::ShowAllCommits)),
                    ],
                    vec![row("Hide Commit Details", Some(Action::ToggleDetails))],
                    vec![greyed("Show HEAD", Some(Action::ShowHead), unread)],
                    vec![row(
                        "Show Lost Commits (Reflog)",
                        Some(Action::ShowLostCommits),
                    )],
                ],
            ),
            (
                "Repository".to_owned(),
                vec![
                    vec![row("Refresh", Some(Action::Refresh))],
                    vec![(fetch.0, Some(Action::Fetch), false, fetch.1)],
                    vec![greyed("New Branch…", Some(Action::NewBranch), unread)],
                ],
            ),
            (
                "Window".to_owned(),
                vec![vec![
                    row("Zoom In", Some(Action::ZoomIn)),
                    row("Zoom Out", Some(Action::ZoomOut)),
                ]],
            ),
            ("Help".to_owned(), vec![vec![row("About Cairn", None)]]),
        ]
    };
    let no_remote = crate::operations::fetch::NO_REMOTE.to_owned();
    assert_eq!(
        shape(&menus_now(&test, view, true)),
        expected(("Fetch".to_owned(), Some(no_remote)))
    );
    // Hide Commit Details shows the chord the table lists for it on Linux, Fork for Windows'
    // (the table's own tests spell it), and none on macOS, where its menu item stands alone.
    let toggle = |os| cairn_ui::accelerators::chords(Action::ToggleDetails, os);
    assert!(!toggle(cairn_ui::accelerators::Os::Linux).is_empty());
    assert!(toggle(cairn_ui::accelerators::Os::MacOs).is_empty());

    // With a remote, Fetch names it; while it runs, it is greyed.
    let mut remotes = view.remotes;
    remotes.set(vec![cairn_model::RemoteSummary {
        name: "origin".to_owned(),
        url: None,
    }]);
    test.sync_and_update();
    assert_eq!(
        shape(&menus_now(&test, view, true)),
        expected(("Fetch 'origin'".to_owned(), None))
    );
    chose(&mut test, view, &submitted, "Fetch 'origin'");
    assert!(
        submitted
            .borrow()
            .iter()
            .any(|request| matches!(request, Request::Fetch { remote } if remote == "origin")),
        "Fetch asked nothing"
    );
    assert_eq!(
        shape(&menus_now(&test, view, true)),
        expected((
            "Fetch 'origin'".to_owned(),
            Some(crate::operations::fetch::FETCH_RUNNING.to_owned())
        ))
    );

    // With no repository open, everything a repository answers is greyed and says so.
    let closed = shape(&menus_now(&test, view, false));
    for (menu, label) in [
        ("View", "Show HEAD"),
        ("View", "Show Lost Commits (Reflog)"),
        ("Repository", "Refresh"),
        ("Repository", "New Branch…"),
    ] {
        let reason = closed
            .iter()
            .filter(|(title, _)| title == menu)
            .flat_map(|(_, groups)| groups.iter().flatten())
            .find(|item| item.0.starts_with(label))
            .and_then(|item| item.3.clone());
        assert_eq!(
            reason.as_deref(),
            Some(super::NO_REPOSITORY),
            "{label} with no repository open"
        );
    }
    // Fetch, with a remote and none running, says no repository is open.
    let mut fetch = view.fetch;
    fetch.set(crate::operations::fetch::FetchStatus::Idle);
    test.sync_and_update();
    let fetch_reason = menus_now(&test, view, false)
        .iter()
        .flat_map(|menu| menu.groups.iter().flatten())
        .find(|item| item.label == "Fetch 'origin'")
        .and_then(|item| item.reason.clone());
    assert_eq!(
        fetch_reason.as_deref(),
        Some(crate::operations::fetch::NO_REPOSITORY)
    );
}

/// Each item does what it names, as its chord does: the two views shown, the detail pane
/// collapsed and opened again and Show Lost Commits turned on — each ticked while on, its label
/// unchanged — Refresh asked of the worker, `HEAD`'s row looked for once the refs are read, and
/// About Cairn opened. Caught by: an item wired to nothing, a toggle not ticked, or a label
/// changed while it is on.
#[test]
fn each_menu_item_does_its_action() {
    let (mut test, view, submitted) = launch();
    chose(&mut test, view, &submitted, "Show Uncommitted Changes");
    assert_eq!(*view.sidebar.main.peek(), MainView::LocalChanges);
    chose(&mut test, view, &submitted, "Show All Commits");
    assert_eq!(*view.sidebar.main.peek(), MainView::AllCommits);

    let ticked = |test: &TestingRunner, label: &str| {
        menus_now(test, view, true)
            .iter()
            .flat_map(|menu| menu.groups.iter().flatten())
            .find(|item| item.label == label)
            .is_some_and(|item| item.ticked)
    };
    chose(&mut test, view, &submitted, "Hide Commit Details");
    assert!(*view.pane_collapsed.peek(), "the pane did not collapse");
    assert!(ticked(&test, "Hide Commit Details"));
    chose(&mut test, view, &submitted, "Hide Commit Details");
    assert!(!*view.pane_collapsed.peek());
    assert!(!ticked(&test, "Hide Commit Details"));

    chose(&mut test, view, &submitted, "Show Lost Commits (Reflog)");
    assert!(*view.show_lost.peek());
    assert!(ticked(&test, "Show Lost Commits (Reflog)"));
    assert!(
        submitted
            .borrow()
            .iter()
            .any(|request| matches!(request, Request::OpenHistory { lost: true, .. })),
        "the history was not walked again with the lost commits"
    );

    chose(&mut test, view, &submitted, "Refresh");
    assert!(matches!(submitted.borrow().last(), Some(Request::Refresh)));

    let head = Oid::from_bytes(&[0xab; 20]).unwrap_or_else(|e| panic!("{e}"));
    head_at(&mut test, view, head);
    // Once the refs are read, Show HEAD and New Branch… can be chosen.
    for label in ["Show HEAD", "New Branch…"] {
        assert_eq!(
            reason_now(&test, view, label),
            None,
            "{label} with HEAD read"
        );
    }
    chose(&mut test, view, &submitted, "Show HEAD");
    assert!(
        submitted
            .borrow()
            .iter()
            .any(|request| matches!(request, Request::FindRow { target, .. } if *target == head)),
        "Show HEAD did not look for HEAD's row"
    );

    // Under the activity popover — the one modal state — About Cairn does not open.
    test.run_in(|| view.activity.open());
    test.sync_and_update();
    chose(&mut test, view, &submitted, "About Cairn");
    assert!(
        !*view.frame.about.peek(),
        "About Cairn opened under the popover"
    );
    test.run_in(|| {
        let mut open = view.activity.open;
        open.set(false);
    });
    test.sync_and_update();
    chose(&mut test, view, &submitted, "About Cairn");
    assert!(*view.frame.about.peek(), "About Cairn did not open");
}

/// The reason the item labelled `label` is greyed now, if it is.
fn reason_now(test: &TestingRunner, view: View, label: &str) -> Option<String> {
    menus_now(test, view, true)
        .iter()
        .flat_map(|menu| menu.groups.iter().flatten())
        .find(|item| item.label == label)
        .and_then(|item| item.reason.clone())
}

/// With `HEAD` unborn — or naming a branch with no ref — Show HEAD and New Branch… are greyed
/// saying so, in git's word. Caught by: an unborn `HEAD` read as refs not yet read, or offered.
#[test]
fn an_unborn_head_greys_show_head_and_new_branch() {
    for head in [
        HeadState::Unborn(cairn_model::RefName::new("refs/heads/main")),
        HeadState::Branch(cairn_model::RefName::new("refs/heads/main")),
    ] {
        let (mut test, view, _) = launch();
        let mut refreshed = view.refreshed;
        let _ = refreshed.write().refs_arrived(Arc::new(RefsSnapshot {
            refs: Vec::new(),
            head,
            stashes: Vec::new(),
            unreadable: 0,
            substitutions: cairn_model::Substitutions::default(),
        }));
        test.sync_and_update();
        for label in ["Show HEAD", "New Branch…"] {
            assert_eq!(
                reason_now(&test, view, label).as_deref(),
                Some(crate::history::head::HEAD_UNBORN),
                "{label}"
            );
        }
    }
}

/// Zoom In and Out, by chord or menu, scale the window through the toolkit's scale factor a step
/// at a time, as far as the last step each way, where the item is greyed saying so. Caught by: a
/// zoom with no step, one past either end, or an item left offered at the end.
#[test]
fn zoom_scales_the_window_in_steps() {
    let (mut test, view, submitted) = launch();
    let factor = |view: View| view.frame.zoom.peek().factor();
    assert_eq!(factor(view), 1.0);
    press_chord(&mut test, Action::ZoomIn);
    assert_eq!(factor(view), 1.1);
    chose(&mut test, view, &submitted, "Zoom In");
    assert_eq!(factor(view), 1.25);
    for _ in 0..10 {
        press_chord(&mut test, Action::ZoomIn);
    }
    assert_eq!(
        factor(view),
        *crate::frame::ZOOM_STEPS.last().unwrap_or(&0.)
    );
    let reason_of = |test: &TestingRunner, label: &str| {
        menus_now(test, view, true)
            .iter()
            .flat_map(|menu| menu.groups.iter().flatten())
            .find(|item| item.label == label)
            .and_then(|item| item.reason.clone())
    };
    assert!(
        reason_of(&test, "Zoom In").is_some(),
        "Zoom In offered at the end"
    );
    assert!(reason_of(&test, "Zoom Out").is_none());
    for _ in 0..20 {
        press_chord(&mut test, Action::ZoomOut);
    }
    assert_eq!(factor(view), crate::frame::ZOOM_STEPS[0]);
    assert!(
        reason_of(&test, "Zoom Out").is_some(),
        "Zoom Out offered at the end"
    );
    assert!(reason_of(&test, "Zoom In").is_none());
}

/// About Cairn names Cairn, this build's version and licence, and the `git` found as the worker
/// described it — or that none has been found — and while it is open the window's chords do
/// nothing; Escape closes it. Caught by: a version or `git` left out, About drawn under another
/// dialog's keys, or a dialog Escape cannot close.
#[test]
fn about_cairn_names_the_version_and_the_git() {
    let (mut test, view, submitted) = launch();
    chose(&mut test, view, &submitted, "About Cairn");
    let drawn = labels(&test);
    assert!(drawn.iter().any(|t| t == cairn_ui::ABOUT_TITLE));
    assert!(
        drawn
            .iter()
            .any(|t| *t == version_line(env!("CARGO_PKG_VERSION"))),
        "{drawn:?}"
    );
    assert!(
        drawn
            .iter()
            .any(|t| *t == licence_line(env!("CARGO_PKG_LICENSE")))
    );
    assert!(drawn.iter().any(|t| t == NO_GIT_FOUND), "{drawn:?}");
    test.press_key(Key::Named(NamedKey::Escape));
    test.sync_and_update();
    assert!(!*view.frame.about.peek(), "Escape left About open");

    let described = "git 2.47.1, at /usr/bin/git".to_owned();
    apply(
        &mut test,
        view,
        &submitted,
        Update::GitFound {
            described: described.clone(),
        },
    );
    chose(&mut test, view, &submitted, "About Cairn");
    assert!(labels(&test).contains(&described));
    let before = submitted.borrow().len();
    press_chord(&mut test, Action::Refresh);
    assert_eq!(
        submitted.borrow().len(),
        before,
        "a chord acted behind About"
    );
}

/// Where a label reading `text` is drawn, a little inside its left edge.
fn at(test: &TestingRunner, text: &str) -> (f64, f64) {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| {
                let area = node.layout().area;
                (f64::from(area.min_x()) + 4., f64::from(area.center().y))
            })
    })
    .unwrap_or_else(|| panic!("{text} is not drawn"))
}

/// The window draws the menu bar in its top row, over everything else, and an item chosen from
/// it there acts: View's Show Uncommitted Changes shows Local Changes. Caught by: a bar the
/// window does not draw, drawn under the title row, or whose choices reach nothing.
#[test]
fn the_window_draws_the_menu_bar_and_its_choices_act() {
    let (mut test, view, _) = launch();
    let view_menu = at(&test, "View");
    assert!(
        view_menu.1 < f64::from(cairn_ui::style::metrics::CAPTION_ROW),
        "the menus are not in the top row: {view_menu:?}"
    );
    test.click_cursor(view_menu);
    test.sync_and_update();
    let item = at(&test, "Show Uncommitted Changes");
    test.click_cursor(item);
    for _ in 0..3 {
        test.sync_and_update();
    }
    assert_eq!(*view.sidebar.main.peek(), MainView::LocalChanges);
    assert!(
        !labels(&test).iter().any(|t| t == "Show All Commits"),
        "the menu stayed open"
    );
}
