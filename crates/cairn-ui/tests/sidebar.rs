//! Headless tests for the sidebar (refs-and-status R8.1-R8.4, R8.7; criterion C8): its entries
//! and sections in Fork's order, folders split at `/`, Fork's marks on a branch, a press
//! reported for what it pressed, and one viewport built however many refs it lists.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use cairn_model::{
    AheadBehind, Disclosure, HeadState, Oid, Ref, RefKind, RefName, RefTarget, RefsSnapshot,
    SidebarRow, StashEntry, Upstream,
};
use cairn_ui::{
    BranchCounts, DETACHED_HEAD_CAPTION, GONE_CAPTION, MainView, RefGlyph,
    SIDEBAR_FILTER_PLACEHOLDER, SIDEBAR_ROW_HEIGHT, Sidebar, SidebarRefs, SidebarTarget, Trailing,
    drawn_row,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 280.;
const HEIGHT: f32 = 600.;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap_or_else(|error| panic!("{error}"))
}

fn listed(name: &str) -> Ref {
    let kind = if name.starts_with("refs/heads/") {
        RefKind::LocalBranch
    } else if name.starts_with("refs/remotes/") {
        RefKind::RemoteTracking
    } else {
        RefKind::Tag
    };
    Ref {
        name: RefName::new(name),
        kind,
        target: RefTarget::Commit(oid(1)),
        symbolic: None,
        upstream: None,
    }
}

fn snapshot(mut refs: Vec<Ref>, head: HeadState, stashes: &[&str]) -> RefsSnapshot {
    refs.sort_by(|a, b| a.name.cmp(&b.name));
    RefsSnapshot {
        refs,
        head,
        stashes: stashes
            .iter()
            .enumerate()
            .map(|(index, message)| StashEntry {
                index,
                message: (*message).to_owned(),
                commit: oid(50 + index as u8),
                base: oid(1),
            })
            .collect(),
        unreadable: 0,
    }
}

/// The rows a worker would lay out for `refs` under `disclosure`, kept with the snapshot.
fn shown(refs: RefsSnapshot, text: &str, disclosure: &Disclosure) -> SidebarRefs {
    let rows = refs
        .sidebar_rows(text, disclosure, || true)
        .unwrap_or_else(|| panic!("laying out the rows stopped"));
    SidebarRefs {
        refs: Arc::new(refs),
        rows: Arc::new(rows),
    }
}

/// A repository with every kind of entry: `main` checked out and two behind and one ahead of
/// its upstream, `topic` whose upstream is gone, `feature/login` in a folder, two remotes, a
/// tag in a folder of its own name, and two stashes.
fn every_kind() -> RefsSnapshot {
    let mut main = listed("refs/heads/main");
    main.upstream = Some(Upstream::Exists {
        name: RefName::new("refs/remotes/origin/main"),
        commit: Some(oid(2)),
    });
    let mut topic = listed("refs/heads/topic");
    topic.upstream = Some(Upstream::Gone {
        name: RefName::new("refs/remotes/origin/topic"),
    });
    snapshot(
        vec![
            main,
            topic,
            listed("refs/heads/feature/login"),
            listed("refs/remotes/origin/main"),
            listed("refs/remotes/upstream/main"),
            listed("refs/tags/v1.0"),
            listed("refs/tags/release/2.0"),
        ],
        HeadState::Branch(RefName::new("refs/heads/main")),
        &["On main: wip", "WIP on topic: 1234567 subject"],
    )
}

fn counts() -> BranchCounts {
    Arc::new(vec![(
        RefName::new("refs/heads/main"),
        AheadBehind {
            ahead: 1,
            behind: 2,
        },
    )])
}

#[derive(Clone, Copy)]
struct Fixture {
    shown: State<Option<SidebarRefs>>,
    filter: State<String>,
    notice: State<Option<String>>,
}

#[derive(Default)]
struct Heard {
    rows: Vec<SidebarRow>,
    main: Vec<MainView>,
}

fn launch(
    shown: SidebarRefs,
    local_changes: Option<usize>,
) -> (TestingRunner, Fixture, Rc<RefCell<Heard>>) {
    let heard = Rc::new(RefCell::new(Heard::default()));
    let hearing = heard.clone();
    let (mut test, fixture) = TestingRunner::new(
        move || {
            let fixture = use_consume::<Fixture>();
            let (rows, main) = (hearing.clone(), hearing.clone());
            rect()
                .width(Size::px(WIDTH))
                .height(Size::fill())
                .child(
                    Sidebar::new(fixture.shown.read().clone(), fixture.filter)
                        .counts(Some(counts()))
                        .local_changes(local_changes)
                        .notice(fixture.notice.read().clone())
                        .on_row(move |row: SidebarRow| rows.borrow_mut().rows.push(row))
                        .on_main(move |view: MainView| main.borrow_mut().main.push(view)),
                )
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || Fixture {
                shown: State::create(Some(shown)),
                filter: State::create(String::new()),
                notice: State::create(None),
            })
        },
        1.,
    );
    test.sync_and_update();
    (test, fixture, heard)
}

/// Every label laid out inside the window, top to bottom: its text and whether it is bold.
fn drawn_labels(test: &TestingRunner) -> Vec<(String, bool)> {
    let mut labels: Vec<(f32, f32, String, bool)> = test.find_many(|node, element| {
        Label::try_downcast(element)
            .map(|label| (label, node.layout().area))
            .filter(|(_, area)| area.height() > 0. && area.min_y() < HEIGHT && area.max_y() > 0.)
            .map(|(label, area)| {
                (
                    area.min_y(),
                    area.min_x(),
                    label.text.to_string(),
                    label.text_style_data.font_weight == Some(FontWeight::BOLD),
                )
            })
    });
    labels.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    labels
        .into_iter()
        .map(|(_, _, text, bold)| (text, bold))
        .collect()
}

fn centre_of(test: &TestingRunner, text: &str) -> (f64, f64) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("{text} is not drawn"));
    (f64::from(centre.x), f64::from(centre.y))
}

/// R8.1, R8.2, R9.2: Local Changes with its count and All Commits, the filter, then Branches,
/// Remotes, Tags and Stashes in Fork's order; branches and remotes in folders split at `/`, an
/// open folder's refs under it, folders first, a closed folder's hidden; the current branch
/// bold with its counts behind then ahead; a gone upstream said; tags whole; every stash by
/// its message. Caught by: a section out of Fork's order or missing, a folder flattened, the
/// current branch drawn plain, counts or the gone upstream left out, or the count beside
/// Local Changes missing.
#[test]
fn the_sidebar_draws_its_sections_in_forks_order_with_forks_marks() {
    let mut disclosure = Disclosure::default();
    disclosure.toggle_folder("refs/heads/feature");
    disclosure.toggle_folder("refs/remotes/origin");
    let (test, _, _) = launch(shown(every_kind(), "", &disclosure), Some(3));
    let drawn: Vec<(String, bool)> = drawn_labels(&test);
    let texts: Vec<&str> = drawn.iter().map(|(text, _)| text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Local Changes (3)",
            "All Commits",
            "Branches",
            "feature",
            "login",
            "main",
            "2↓ 1↑",
            "topic",
            GONE_CAPTION,
            "Remotes",
            "origin",
            "main",
            "upstream",
            "Tags",
            "release/2.0",
            "v1.0",
            "Stashes",
            "On main: wip",
            "WIP on topic: 1234567 subject",
        ]
    );
    let bold: Vec<&str> = drawn
        .iter()
        .filter(|(text, bold)| {
            *bold && !["Branches", "Remotes", "Tags", "Stashes"].contains(&text.as_str())
        })
        .map(|(text, _)| text.as_str())
        .collect();
    assert_eq!(bold, ["main"], "only the current branch is bold");
}

/// R8.2, R5.5: each entry's glyph is its kind's shape — the current branch a check mark, a
/// branch whose upstream is gone Fork's warning triangle, any other branch a branch, a
/// remote-tracking ref the remote, a tag the tag, a stash the box, a folder a folder, a
/// detached `HEAD` the check mark — and a closed section or folder says so. Caught by: a kind
/// drawn with another's glyph, or a gone upstream told by colour alone.
#[test]
fn each_entry_draws_its_kinds_glyph() {
    let refs = every_kind();
    let rows = refs
        .sidebar_rows("", &Disclosure::default(), || true)
        .unwrap();
    let counts = counts();
    let drawn: Vec<(RefGlyph, String, Option<Trailing>, bool)> = rows
        .iter()
        .filter_map(|row| drawn_row(*row, &refs, Some(&counts), None))
        .map(|row| (row.glyph, row.text, row.trailing, row.bold))
        .collect();
    let of = |text: &str| {
        drawn
            .iter()
            .find(|(_, drawn, _, _)| drawn == text)
            .unwrap_or_else(|| panic!("{text} not drawn: {drawn:?}"))
            .clone()
    };
    assert_eq!(
        of("main"),
        (
            RefGlyph::Current,
            "main".to_owned(),
            Some(Trailing::Counts("2↓ 1↑".to_owned())),
            true
        )
    );
    assert_eq!(
        of("topic"),
        (
            RefGlyph::Gone,
            "topic".to_owned(),
            Some(Trailing::Gone),
            false
        )
    );
    assert_eq!(of("feature").0, RefGlyph::Folder);
    assert_eq!(of("origin").0, RefGlyph::Folder);
    assert_eq!(of("v1.0").0, RefGlyph::Tag);
    assert_eq!(of("On main: wip").0, RefGlyph::Stash);
    assert_eq!(of("Branches").0, RefGlyph::Opened);

    let mut closed = Disclosure::default();
    closed.toggle_section(cairn_model::SidebarSection::Tags);
    disclosed_as(&refs, &closed, "Tags", RefGlyph::Closed);

    let mut open = Disclosure::default();
    open.toggle_folder("refs/remotes/origin");
    let rows = refs.sidebar_rows("", &open, || true).unwrap();
    let remote = rows
        .iter()
        .filter_map(|row| drawn_row(*row, &refs, None, None))
        .find(|row| row.text == "main" && row.depth == 1)
        .unwrap_or_else(|| panic!("origin/main not drawn"));
    assert_eq!(remote.glyph, RefGlyph::Remote);
    assert_eq!(remote.trailing, None, "a remote-tracking ref has no counts");

    // A branch with no upstream: a branch, no counts.
    let mut plain = every_kind();
    plain.head = HeadState::Detached(oid(1));
    let rows = plain
        .sidebar_rows("", &Disclosure::default(), || true)
        .unwrap();
    let main = rows
        .iter()
        .filter_map(|row| drawn_row(*row, &plain, None, None))
        .find(|row| row.text == "main")
        .unwrap_or_else(|| panic!("main not drawn"));
    assert_eq!(main.glyph, RefGlyph::Branch);
    assert!(!main.bold);
    let head = drawn_row(SidebarRow::DetachedHead, &plain, None, None).unwrap();
    assert_eq!(
        (head.glyph, head.text.as_str(), head.bold),
        (RefGlyph::Current, DETACHED_HEAD_CAPTION, true)
    );
}

fn disclosed_as(refs: &RefsSnapshot, disclosure: &Disclosure, caption: &str, glyph: RefGlyph) {
    let rows = refs
        .sidebar_rows("", disclosure, || true)
        .unwrap_or_else(|| panic!("laying out the rows stopped"));
    let row = rows
        .iter()
        .filter_map(|row| drawn_row(*row, refs, None, None))
        .find(|row| row.text == caption)
        .unwrap_or_else(|| panic!("{caption} not drawn"));
    assert_eq!(row.glyph, glyph, "{caption}");
}

/// The entry chosen is drawn chosen, by identity: a ref by its name, a stash by its place and
/// commit, whatever row it lands on. Caught by: a chosen ref found by its row number, which a
/// snapshot laid out again moves.
#[test]
fn the_entry_chosen_is_drawn_chosen_by_its_name() {
    let refs = every_kind();
    let rows = refs
        .sidebar_rows("", &Disclosure::default(), || true)
        .unwrap();
    let chosen = SidebarTarget::Ref(RefName::new("refs/heads/topic"));
    let drawn_chosen: Vec<String> = rows
        .iter()
        .filter_map(|row| drawn_row(*row, &refs, None, Some(&chosen)))
        .filter(|row| row.chosen)
        .map(|row| row.text)
        .collect();
    assert_eq!(drawn_chosen, ["topic"]);
    let stash = SidebarTarget::Stash {
        index: 1,
        commit: oid(51),
    };
    let drawn_chosen: Vec<String> = rows
        .iter()
        .filter_map(|row| drawn_row(*row, &refs, None, Some(&stash)))
        .filter(|row| row.chosen)
        .map(|row| row.text)
        .collect();
    assert_eq!(drawn_chosen, ["WIP on topic: 1234567 subject"]);
}

/// R8.5, R8.7: a press is reported for what it pressed — a ref's row, a folder's, a section's,
/// Local Changes and All Commits — and the notice under the filter is drawn. Caught by: a press
/// that reports another row, or no way to switch the main region.
#[test]
fn a_press_is_reported_for_what_it_pressed() {
    let refs = every_kind();
    let (mut test, mut fixture, heard) =
        launch(shown(refs.clone(), "", &Disclosure::default()), None);
    for text in ["topic", "feature", "Tags", "Local Changes", "All Commits"] {
        test.click_cursor(centre_of(&test, text));
        test.sync_and_update();
    }
    let topic = refs
        .refs
        .iter()
        .position(|listed| listed.name.as_str() == "refs/heads/topic")
        .unwrap() as u32;
    let feature = refs
        .refs
        .iter()
        .position(|listed| listed.name.as_str() == "refs/heads/feature/login")
        .unwrap() as u32;
    assert_eq!(
        heard.borrow().rows,
        [
            SidebarRow::Ref {
                index: topic,
                depth: 0
            },
            SidebarRow::Folder {
                first: feature,
                depth: 0,
                open: false
            },
            SidebarRow::Section {
                section: cairn_model::SidebarSection::Tags,
                open: true
            },
        ]
    );
    assert_eq!(
        heard.borrow().main,
        [MainView::LocalChanges, MainView::AllCommits]
    );

    test.run_in(|| fixture.notice.set(Some("Finding v1.0…".to_owned())));
    test.sync_and_update();
    assert!(
        drawn_labels(&test)
            .iter()
            .any(|(text, _)| text == "Finding v1.0…"),
        "the notice is not drawn"
    );
    // The filter's text is the caller's to lay the rows out for.
    let field = test
        .find(|node, element| {
            Paragraph::try_downcast(element)
                .filter(|paragraph| {
                    paragraph
                        .spans
                        .iter()
                        .any(|span| span.text.as_ref() == SIDEBAR_FILTER_PLACEHOLDER)
                })
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("the filter field shows no placeholder"));
    test.click_cursor((f64::from(field.x), f64::from(field.y)));
    test.sync_and_update();
    test.write_text("log");
    test.sync_and_update();
    assert_eq!(*fixture.filter.peek(), "log");
}

/// The ref rows built, by their names' common part, and whether each is visible.
fn built_refs(test: &TestingRunner) -> Vec<(String, bool)> {
    test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text.starts_with("branch-"))
            .map(|label| (label.text.to_string(), node.is_visible()))
    })
}

fn many_branches(count: usize, folders: usize) -> RefsSnapshot {
    snapshot(
        (0..count)
            .map(|n| listed(&format!("refs/heads/team-{:02}/branch-{n:06}", n % folders)))
            .collect(),
        HeadState::Detached(oid(1)),
        &[],
    )
}

/// The rows of a list's viewport, with room for one cut at either edge.
fn within_a_viewport(test: &TestingRunner, place: &str) {
    let viewport = (HEIGHT / SIDEBAR_ROW_HEIGHT).ceil() as usize;
    let built = built_refs(test).len();
    assert!(
        built > 0 && built <= viewport + 2,
        "{built} ref rows built for a viewport of at most {viewport} {place}"
    );
}

/// R8.4, C8: a sidebar of 50,000 refs, every folder open, builds one viewport of rows at the
/// top, deep and at the end, and the end is the last ref. Caught by: a list that builds a row
/// per ref, or an end that is not the snapshot's.
#[test]
fn a_sidebar_of_50000_refs_builds_one_viewport() {
    let refs = many_branches(50_000, 50);
    let mut disclosure = Disclosure::default();
    for folder in 0..50 {
        disclosure.toggle_folder(&format!("refs/heads/team-{folder:02}"));
    }
    let shown = shown(refs, "", &disclosure);
    assert_eq!(shown.rows.len(), 50_000 + 50 + 4 + 1);
    let (mut test, _, _) = launch(shown, None);
    within_a_viewport(&test, "at the top");
    test.scroll(
        (100., 300.),
        (0., -(25_000. * f64::from(SIDEBAR_ROW_HEIGHT))),
    );
    test.sync_and_update();
    within_a_viewport(&test, "deep");
    test.scroll(
        (100., 300.),
        (0., -(60_000. * f64::from(SIDEBAR_ROW_HEIGHT))),
    );
    test.sync_and_update();
    within_a_viewport(&test, "at the end");
    assert!(
        built_refs(&test)
            .iter()
            .any(|(name, visible)| *visible && name == "branch-049999"),
        "the end of the list is not the last ref"
    );
}

/// The QA brief: folders over a flat list stay virtualized — one folder of 10,000 branches,
/// opened, builds one viewport, at its top and scrolled deep into it, and closing it builds
/// none of them. Caught by: a folder that builds every row it holds when opened.
#[test]
fn a_folder_of_10000_branches_open_builds_one_viewport() {
    let refs = many_branches(10_000, 1);
    let mut disclosure = Disclosure::default();
    disclosure.toggle_folder("refs/heads/team-00");
    let (mut test, mut fixture, _) = launch(shown(refs.clone(), "", &disclosure), None);
    within_a_viewport(&test, "with the folder open");
    test.scroll(
        (100., 300.),
        (0., -(5_000. * f64::from(SIDEBAR_ROW_HEIGHT))),
    );
    test.sync_and_update();
    within_a_viewport(&test, "deep in the folder");

    let closed = shown(refs, "", &Disclosure::default());
    test.run_in(|| fixture.shown.set(Some(closed.clone())));
    test.sync_and_update();
    assert_eq!(built_refs(&test).len(), 0, "a closed folder built its refs");
}
