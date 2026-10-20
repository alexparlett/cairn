//! Headless tests for the sidebar, Fork's: its header, entries and sections in Fork's order and
//! disclosure, folders split at `/` with each row its leaf name, Fork's marks on a branch, tags
//! newest first, a stash read after `On <branch>: `, a press and a right press reported for what
//! they pressed, the walk by ↑ ↓ ← →, and one viewport built however many refs it lists.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use cairn_model::{
    AheadBehind, Disclosure, HeadState, Oid, Ref, RefKind, RefName, RefTarget, RefsSnapshot,
    SidebarRow, SidebarSection, StashEntry, Upstream,
};
use cairn_ui::style::{metrics, tones};
use cairn_ui::{
    BranchCounts, COPY_BRANCH_NAME_CAPTION, COPY_REPOSITORY_PATH_CAPTION, COPY_TAG_NAME_CAPTION,
    DETACHED_HEAD_CAPTION, GearChoice, MainView, NEW_BRANCH_CAPTION, REPOSITORY_GEAR_NAME,
    RefChoice, SIDEBAR_FILTER_PLACEHOLDER, SIDEBAR_INDENT, SIDEBAR_ROW_HEIGHT, Sidebar,
    SidebarGlyph, SidebarRefs, SidebarTarget, drawn_row,
};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{MouseEventName, PlatformEvent};

const WIDTH: f32 = 300.;
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
        commit_time: None,
    }
}

fn tag(name: &str, commit_time: i64) -> Ref {
    Ref {
        commit_time: Some(commit_time),
        ..listed(name)
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
        substitutions: cairn_model::Substitutions::default(),
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

/// A repository with every kind of entry: `main` checked out and one ahead and two behind its
/// upstream, `topic` whose upstream is gone, `feature/login` in a folder with no upstream, two
/// remotes, three tags whose commits' dates are in neither name order, and two stashes, one
/// with git's `On <branch>: ` and one with git's default message.
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
    let mut origin_head = listed("refs/remotes/origin/HEAD");
    origin_head.symbolic = Some(RefName::new("refs/remotes/origin/main"));
    snapshot(
        vec![
            main,
            topic,
            origin_head,
            listed("refs/heads/feature/login"),
            listed("refs/remotes/origin/main"),
            listed("refs/remotes/upstream/main"),
            tag("refs/tags/v1.0", 200),
            tag("refs/tags/v2.0", 100),
            tag("refs/tags/release/2.0", 50),
        ],
        HeadState::Branch(RefName::new("refs/heads/main")),
        &["On main: WIP on parser", "WIP on topic: 1234567 subject"],
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
    chosen: State<Option<SidebarTarget>>,
}

#[derive(Default)]
struct Heard {
    rows: Vec<SidebarRow>,
    main: Vec<MainView>,
    gear: Vec<GearChoice>,
    menu: Vec<(SidebarRow, RefChoice)>,
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
            let (rows, main, gear, menu) = (
                hearing.clone(),
                hearing.clone(),
                hearing.clone(),
                hearing.clone(),
            );
            rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::fill())
                .child(
                    rect().width(Size::px(WIDTH)).height(Size::fill()).child(
                        Sidebar::new(fixture.shown.read().clone(), fixture.filter)
                            .name(Some("t".to_owned()))
                            .counts(Some(counts()))
                            .local_changes(local_changes)
                            .main(MainView::AllCommits)
                            .chosen(fixture.chosen.read().clone())
                            .notice(fixture.notice.read().clone())
                            .on_row(move |row: SidebarRow| rows.borrow_mut().rows.push(row))
                            .on_main(move |view: MainView| main.borrow_mut().main.push(view))
                            .on_gear(move |choice: GearChoice| {
                                gear.borrow_mut().gear.push(choice);
                            })
                            .on_ref_menu(move |chosen: (SidebarRow, RefChoice)| {
                                menu.borrow_mut().menu.push(chosen);
                            }),
                    ),
                )
                // Somewhere else to take focus.
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::fill())
                        .a11y_focusable(true),
                )
                .child(ContextMenuViewer::new())
                .into_element()
        },
        (WIDTH + 100., HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || Fixture {
                shown: State::create(Some(shown)),
                filter: State::create(String::new()),
                notice: State::create(None),
                chosen: State::create(None),
            })
        },
        1.,
    );
    test.sync_and_update();
    (test, fixture, heard)
}

/// Every label laid out inside the sidebar, top to bottom: its text, whether it is bold, and
/// its area.
fn drawn_labels(test: &TestingRunner) -> Vec<(String, bool, Area)> {
    let mut labels: Vec<(String, bool, Area)> = test.find_many(|node, element| {
        Label::try_downcast(element)
            .map(|label| (label, node.layout().area))
            .filter(|(_, area)| {
                area.height() > 0.
                    && area.min_y() < HEIGHT
                    && area.max_y() > 0.
                    && area.min_x() < WIDTH
            })
            .map(|(label, area)| {
                (
                    label.text.to_string(),
                    label.text_style_data.font_weight == Some(FontWeight::BOLD),
                    area,
                )
            })
    });
    labels.sort_by(|a, b| {
        a.2.min_y()
            .total_cmp(&b.2.min_y())
            .then(a.2.min_x().total_cmp(&b.2.min_x()))
    });
    labels
}

fn texts(test: &TestingRunner) -> Vec<String> {
    drawn_labels(test)
        .into_iter()
        .map(|(text, _, _)| text)
        .collect()
}

fn label_area(test: &TestingRunner, text: &str) -> Area {
    drawn_labels(test)
        .into_iter()
        .find(|(drawn, _, _)| drawn == text)
        .map(|(_, _, area)| area)
        .unwrap_or_else(|| panic!("{text} is not drawn: {:?}", texts(test)))
}

fn centre_of(test: &TestingRunner, text: &str) -> (f64, f64) {
    let centre = label_area(test, text).center();
    (f64::from(centre.x), f64::from(centre.y))
}

fn settle(test: &mut TestingRunner) {
    for _ in 0..3 {
        test.sync_and_update();
    }
}

fn right_click(test: &mut TestingRunner, at: (f64, f64)) {
    test.move_cursor(at);
    for name in [MouseEventName::MouseDown, MouseEventName::MouseUp] {
        test.send_event(PlatformEvent::Mouse {
            name,
            cursor: at.into(),
            button: Some(MouseButton::Right),
        });
        test.sync_and_update();
    }
    settle(test);
}

fn menu_items(test: &TestingRunner) -> Vec<String> {
    let mut items: Vec<(f32, String)> = test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| {
                [
                    NEW_BRANCH_CAPTION,
                    COPY_BRANCH_NAME_CAPTION,
                    COPY_TAG_NAME_CAPTION,
                    COPY_REPOSITORY_PATH_CAPTION,
                ]
                .contains(&label.text.as_ref())
            })
            .map(|label| (node.layout().area.min_y(), label.text.to_string()))
    });
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    items.into_iter().map(|(_, text)| text).collect()
}

fn press_key(test: &mut TestingRunner, key: NamedKey) {
    test.press_key(Key::Named(key));
    settle(test);
}

/// The sidebar is Fork's (FS1 §3, uw27, sb14): the repository's name bold in the header with
/// the gear; Local Changes with its count and All Commits; the filter; Branches, Remotes, Tags
/// and Stashes in Fork's order, each heading bold with no count; Tags closed as the sidebar
/// opens and every folder closed but those opened; each row its leaf name, indented 12 px per
/// level; the current branch bold with its counts at the row's right, ahead then behind; tags
/// newest first by their commits' dates; a stash read after `On <branch>: `. Caught by: a
/// section out of order, Tags drawn open, a folder's prefix drawn on its rows, an indent that is
/// not Fork's, counts behind first or not at the right, tags in name order, or a stash's
/// `On main: ` kept.
#[test]
fn the_sidebar_is_forks() {
    let mut disclosure = Disclosure::default();
    disclosure.toggle_folder("refs/heads/feature");
    disclosure.toggle_folder("refs/remotes/origin");
    let (test, _, _) = launch(shown(every_kind(), "", &disclosure), Some(3));
    assert_eq!(
        texts(&test),
        [
            "t",
            "Local Changes (3)",
            "All Commits",
            "Branches",
            "feature",
            "login",
            "main",
            "1↑ 2↓",
            "topic",
            "Remotes",
            "origin",
            "origin/HEAD",
            "main",
            "upstream",
            "Tags",
            "Stashes",
            "WIP on parser",
            "WIP on topic: 1234567 subject",
        ]
    );
    let bold: Vec<String> = drawn_labels(&test)
        .into_iter()
        .filter(|(_, bold, _)| *bold)
        .map(|(text, _, _)| text)
        .collect();
    assert_eq!(
        bold,
        ["t", "Branches", "main", "Remotes", "Tags", "Stashes"],
        "the header, the headings and the current branch are bold, and nothing else"
    );
    assert!(
        test.find(|_, element| {
            Paragraph::try_downcast(element).filter(|paragraph| {
                paragraph
                    .spans
                    .iter()
                    .any(|span| span.text.as_ref() == SIDEBAR_FILTER_PLACEHOLDER)
            })
        })
        .is_some(),
        "the filter field shows no placeholder"
    );

    // 12 px per level: a heading, a folder and a branch straight under it, a branch in it.
    let x = |text: &str| label_area(&test, text).min_x();
    let heading = x("Branches");
    let folder = x("feature");
    assert_eq!(
        x("topic"),
        folder,
        "a branch stands where a folder of its level does"
    );
    assert_eq!(x("login") - folder, SIDEBAR_INDENT, "a folder's rows");
    assert_eq!(x("origin") - heading, x("upstream") - heading);
    assert_eq!(SIDEBAR_INDENT, 12.);
    // The counts at the row's right, past the name.
    let counts = label_area(&test, "1↑ 2↓");
    assert!(
        WIDTH - counts.max_x() <= metrics::SIDEBAR_INSET + 1.,
        "the counts end {} px from the right",
        WIDTH - counts.max_x()
    );
    drop(test);

    // Tags open: newest first by their commits' dates, not by name either way.
    let mut tags_open = disclosure.clone();
    tags_open.toggle_section(SidebarSection::Tags);
    let (mut test, _, _) = launch(shown(every_kind(), "", &tags_open), Some(3));
    settle(&mut test);
    let drawn = texts(&test);
    let tags: Vec<&str> = drawn
        .iter()
        .skip_while(|text| *text != "Tags")
        .skip(1)
        .take_while(|text| *text != "Stashes")
        .map(String::as_str)
        .collect();
    assert_eq!(tags, ["v1.0", "v2.0", "release/2.0"]);
}

/// Each entry's icon is its kind's — the current branch a check mark, a branch whose upstream
/// is gone the branch with Fork's warning on it (and, when it is the current branch, the warning
/// beside the check mark), a branch with no upstream Cairn's local-only glyph, any other branch
/// and a remote-tracking ref the branch, a remote's folder its cloud, a folder under it or of
/// branches a folder, a tag the tag, a stash the box, a detached `HEAD` the check mark — and a
/// heading draws its chevron alone. Caught by: a kind drawn with another's icon, a remote's
/// folder drawn as a folder, or a gone upstream told by a count.
#[test]
fn each_entry_draws_its_kinds_glyph() {
    let refs = every_kind();
    let mut open = Disclosure::default();
    open.toggle_folder("refs/heads/feature");
    open.toggle_folder("refs/remotes/origin");
    open.toggle_section(SidebarSection::Tags);
    let rows = refs.sidebar_rows("", &open, || true).unwrap();
    let counts = counts();
    let drawn: Vec<_> = rows
        .iter()
        .filter_map(|row| drawn_row(*row, &refs, Some(&counts), None))
        .collect();
    let of = |text: &str, depth: u16| {
        drawn
            .iter()
            .find(|row| row.text == text && row.depth == depth)
            .unwrap_or_else(|| panic!("{text} not drawn at depth {depth}: {drawn:?}"))
            .clone()
    };
    let main = of("main", 0);
    assert_eq!(
        (main.glyph, main.counts.as_deref(), main.bold, main.warning),
        (Some(SidebarGlyph::Current), Some("1↑ 2↓"), true, false)
    );
    let topic = of("topic", 0);
    assert_eq!(
        (topic.glyph, topic.counts, topic.bold),
        (Some(SidebarGlyph::Gone), None, false)
    );
    assert_eq!(of("login", 1).glyph, Some(SidebarGlyph::LocalOnly));
    assert_eq!(of("feature", 0).glyph, Some(SidebarGlyph::Folder));
    assert_eq!(of("origin", 0).glyph, Some(SidebarGlyph::Remote));
    assert_eq!(of("upstream", 0).glyph, Some(SidebarGlyph::Remote));
    let remote = of("main", 1);
    assert_eq!(remote.glyph, Some(SidebarGlyph::Branch));
    assert_eq!(remote.counts, None, "a remote-tracking ref has no counts");
    assert_eq!(of("v1.0", 0).glyph, Some(SidebarGlyph::Tag));
    assert_eq!(of("WIP on parser", 0).glyph, Some(SidebarGlyph::Stash));
    let heading = of("Branches", 0);
    assert_eq!(
        (heading.glyph, heading.disclosure, heading.bold),
        (None, Some(true), true)
    );
    assert_eq!(of("Tags", 0).disclosure, Some(true));
    let closed = refs
        .sidebar_rows("", &Disclosure::default(), || true)
        .unwrap();
    let tags = closed
        .iter()
        .filter_map(|row| drawn_row(*row, &refs, None, None))
        .find(|row| row.text == "Tags")
        .unwrap_or_else(|| panic!("Tags not drawn"));
    assert_eq!(tags.disclosure, Some(false), "Tags opens closed");

    // A branch with an upstream, whoever is current, is the branch glyph.
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
    assert_eq!(main.glyph, Some(SidebarGlyph::Branch));
    assert!(!main.bold && !main.warning);
    let head = drawn_row(SidebarRow::DetachedHead, &plain, None, None).unwrap();
    assert_eq!(
        (head.glyph, head.text.as_str(), head.bold),
        (Some(SidebarGlyph::Current), DETACHED_HEAD_CAPTION, true)
    );

    // The current branch with its upstream gone: the check mark, and the warning beside it.
    let mut stranded = every_kind();
    stranded.head = HeadState::Branch(RefName::new("refs/heads/topic"));
    let rows = stranded
        .sidebar_rows("", &Disclosure::default(), || true)
        .unwrap();
    let topic = rows
        .iter()
        .filter_map(|row| drawn_row(*row, &stranded, None, None))
        .find(|row| row.text == "topic")
        .unwrap_or_else(|| panic!("topic not drawn"));
    assert_eq!(
        (topic.glyph, topic.warning, topic.bold),
        (Some(SidebarGlyph::Current), true, true)
    );
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
    let chosen_by = |chosen: &SidebarTarget| -> Vec<String> {
        rows.iter()
            .filter_map(|row| drawn_row(*row, &refs, None, Some(chosen)))
            .filter(|row| row.chosen)
            .map(|row| row.text)
            .collect()
    };
    assert_eq!(
        chosen_by(&SidebarTarget::Ref(RefName::new("refs/heads/topic"))),
        ["topic"]
    );
    assert_eq!(
        chosen_by(&SidebarTarget::Stash {
            index: 1,
            commit: oid(51),
        }),
        ["WIP on topic: 1234567 subject"]
    );
}

/// A press is reported for what it pressed — a ref's row, a folder's, a heading's, Local
/// Changes and All Commits — and the notice under the filter is drawn. Caught by: a press that
/// reports another row, or no way to switch the main region.
#[test]
fn a_press_is_reported_for_what_it_pressed() {
    let refs = every_kind();
    let (mut test, mut fixture, heard) =
        launch(shown(refs.clone(), "", &Disclosure::default()), None);
    for text in ["topic", "feature", "Tags", "Local Changes", "All Commits"] {
        test.click_cursor(centre_of(&test, text));
        test.sync_and_update();
    }
    let index_of = |name: &str| {
        refs.refs
            .iter()
            .position(|listed| listed.name.as_str() == name)
            .unwrap() as u32
    };
    assert_eq!(
        heard.borrow().rows,
        [
            SidebarRow::Ref {
                index: index_of("refs/heads/topic"),
                depth: 0
            },
            SidebarRow::Folder {
                first: index_of("refs/heads/feature/login"),
                depth: 0,
                open: false
            },
            SidebarRow::Section {
                section: SidebarSection::Tags,
                open: false
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
        texts(&test).contains(&"Finding v1.0…".to_owned()),
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

/// A right press on a ref reports its row as a press does — the window selects its commit — and
/// opens Fork's working items for it (FS1 §8): a branch's or a remote-tracking ref's New
/// Branch… then Copy Branch Name, a tag's New Branch… then Copy Tag Name, each choice reported
/// with the row. A right press on a stash reports it and opens nothing; on a folder or a heading
/// it does neither. Caught by: a right press that leaves the commit where it was, a menu with
/// another row's items, a choice reported for the wrong row, or a menu on a folder.
#[test]
fn a_right_press_on_a_ref_selects_its_commit() {
    let refs = every_kind();
    let mut tags_open = Disclosure::default();
    tags_open.toggle_section(SidebarSection::Tags);
    let (mut test, _, heard) = launch(shown(refs.clone(), "", &tags_open), None);
    let index_of = |name: &str| {
        refs.refs
            .iter()
            .position(|listed| listed.name.as_str() == name)
            .unwrap() as u32
    };
    let topic = SidebarRow::Ref {
        index: index_of("refs/heads/topic"),
        depth: 0,
    };
    let at = centre_of(&test, "topic");
    right_click(&mut test, at);
    assert_eq!(heard.borrow().rows, [topic], "the right press chose no ref");
    assert_eq!(
        menu_items(&test),
        [NEW_BRANCH_CAPTION, COPY_BRANCH_NAME_CAPTION]
    );
    test.click_cursor(centre_of_label(&test, COPY_BRANCH_NAME_CAPTION));
    settle(&mut test);
    assert_eq!(heard.borrow().menu, [(topic, RefChoice::CopyName)]);
    assert!(menu_items(&test).is_empty(), "the menu stayed open");

    let tag = SidebarRow::Ref {
        index: index_of("refs/tags/v2.0"),
        depth: 0,
    };
    let at = centre_of(&test, "v2.0");
    right_click(&mut test, at);
    assert_eq!(heard.borrow().rows.last(), Some(&tag));
    assert_eq!(
        menu_items(&test),
        [NEW_BRANCH_CAPTION, COPY_TAG_NAME_CAPTION]
    );
    test.click_cursor(centre_of_label(&test, NEW_BRANCH_CAPTION));
    settle(&mut test);
    assert_eq!(
        heard.borrow().menu.last(),
        Some(&(tag, RefChoice::NewBranch))
    );

    let pressed = heard.borrow().rows.len();
    let at = centre_of(&test, "WIP on parser");
    right_click(&mut test, at);
    assert_eq!(
        heard.borrow().rows.last(),
        Some(&SidebarRow::Stash { index: 0 })
    );
    assert!(menu_items(&test).is_empty(), "a stash's row opened a menu");
    for text in ["origin", "Branches"] {
        let at = centre_of(&test, text);
        right_click(&mut test, at);
        assert!(menu_items(&test).is_empty(), "{text} opened a menu");
    }
    assert_eq!(
        heard.borrow().rows.len(),
        pressed + 1,
        "a right press on a folder or heading was reported"
    );
}

/// The label reading `text` anywhere in the window — a menu's item, drawn over the sidebar.
fn centre_of_label(test: &TestingRunner, text: &str) -> (f64, f64) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("{text} is not drawn"));
    (f64::from(centre.x), f64::from(centre.y))
}

/// The gear's menu holds Copy Repository Path alone, and choosing it is reported. Caught by: a
/// gear with no menu, or one whose choice goes nowhere.
#[test]
fn the_gear_copies_the_repository_path() {
    let (mut test, _, heard) = launch(shown(every_kind(), "", &Disclosure::default()), None);
    let gear = test
        .find(|node, element| {
            Rect::try_downcast(element)
                .filter(|rect| rect.accessibility.builder.value() == Some(REPOSITORY_GEAR_NAME))
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("the header draws no gear"));
    test.click_cursor((f64::from(gear.x), f64::from(gear.y)));
    settle(&mut test);
    assert_eq!(menu_items(&test), [COPY_REPOSITORY_PATH_CAPTION]);
    test.click_cursor(centre_of_label(&test, COPY_REPOSITORY_PATH_CAPTION));
    settle(&mut test);
    assert_eq!(heard.borrow().gear, [GearChoice::CopyRepositoryPath]);
}

/// The bands drawn behind `at` in the selection's tones.
fn bands_at(test: &TestingRunner, at: (f64, f64)) -> Vec<Color> {
    let bands = [tones::SELECTION_FOCUSED, tones::SELECTION_UNFOCUSED];
    let (x, y) = (at.0 as f32, at.1 as f32);
    test.find_many(|node, element| {
        let area = node.layout().area;
        Rect::try_downcast(element)
            .filter(|_| {
                area.min_x() <= x && x <= area.max_x() && area.min_y() <= y && y <= area.max_y()
            })
            .and_then(|rect| rect.style.background.as_color())
            .filter(|band| bands.contains(band))
    })
}

/// Once a press gives the sidebar focus its keys are Fork's (sb22-sb72): ↓ and ↑ walk every
/// row — a ref or stash walked to reported at once, so its commit is selected, a heading or a
/// folder only holding the walk, drawn selected in the focused blue — and never reach All
/// Commits or Local Changes; ← closes an open heading; Return does nothing. Caught by: a heading
/// skipped, a ref walked to and not chosen, a heading's walk reported as a press (which would
/// close it), the walk reaching Local Changes, or Return taken.
#[test]
fn the_sidebar_walks_by_key_as_forks_tree() {
    let refs = every_kind();
    let mut disclosure = Disclosure::default();
    disclosure.toggle_folder("refs/heads/feature");
    let (mut test, mut fixture, heard) = launch(shown(refs.clone(), "", &disclosure), None);
    let index_of = |name: &str| {
        refs.refs
            .iter()
            .position(|listed| listed.name.as_str() == name)
            .unwrap() as u32
    };
    let main = SidebarRow::Ref {
        index: index_of("refs/heads/main"),
        depth: 0,
    };
    let topic = SidebarRow::Ref {
        index: index_of("refs/heads/topic"),
        depth: 0,
    };
    let login = SidebarRow::Ref {
        index: index_of("refs/heads/feature/login"),
        depth: 1,
    };
    // The window chooses what is pressed or walked to, as `ref_find` does.
    let choose = |test: &mut TestingRunner, fixture: &mut Fixture, row: SidebarRow| {
        let chosen = SidebarTarget::of(row, &refs);
        test.run_in(|| fixture.chosen.set(chosen));
        settle(test);
    };
    test.click_cursor(centre_of(&test, "main"));
    settle(&mut test);
    choose(&mut test, &mut fixture, main);
    heard.borrow_mut().rows.clear();

    press_key(&mut test, NamedKey::ArrowDown);
    assert_eq!(heard.borrow().rows, [topic], "↓ chose the next ref");
    choose(&mut test, &mut fixture, topic);
    press_key(&mut test, NamedKey::ArrowDown);
    assert_eq!(
        heard.borrow().rows,
        [topic],
        "walking onto the Remotes heading reported it"
    );
    assert_eq!(
        bands_at(&test, centre_of(&test, "Remotes")),
        [tones::SELECTION_FOCUSED],
        "the heading walked to is not drawn selected"
    );
    assert!(
        bands_at(&test, centre_of(&test, "topic")).is_empty(),
        "the ref walked past is still drawn selected"
    );
    press_key(&mut test, NamedKey::Enter);
    assert_eq!(heard.borrow().rows, [topic], "Return did something");

    // Back up past the branches to the heading, never further.
    for _ in 0..8 {
        press_key(&mut test, NamedKey::ArrowUp);
        if let Some(row) = heard.borrow().rows.last().copied() {
            choose(&mut test, &mut fixture, row);
        }
    }
    let rows = heard.borrow().rows.clone();
    assert_eq!(rows, [topic, topic, main, login], "{rows:?}");
    assert!(heard.borrow().main.is_empty(), "the walk reached an entry");
    assert_eq!(
        bands_at(&test, centre_of(&test, "Branches")),
        [tones::SELECTION_FOCUSED]
    );

    // ← on the open heading closes it.
    press_key(&mut test, NamedKey::ArrowLeft);
    assert_eq!(
        heard.borrow().rows.last(),
        Some(&SidebarRow::Section {
            section: SidebarSection::Branches,
            open: true
        }),
        "← did not close the open heading"
    );
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

/// The rows of a list's viewport, with room for one cut at either edge, each Fork's 22 px: two
/// rows built one above the other stand one row's height apart.
fn within_a_viewport(test: &TestingRunner, place: &str) {
    assert_eq!(SIDEBAR_ROW_HEIGHT, 22.);
    let viewport = (HEIGHT / SIDEBAR_ROW_HEIGHT).ceil() as usize;
    let built = built_refs(test).len();
    assert!(
        built > 0 && built <= viewport + 2,
        "{built} ref rows built for a viewport of at most {viewport} {place}"
    );
    let mut tops: Vec<f32> = test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text.starts_with("branch-"))
            .map(|_| node.layout().area.min_y())
    });
    tops.sort_by(f32::total_cmp);
    let pitches: Vec<f32> = tops.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(
        !pitches.is_empty() && pitches.iter().all(|pitch| *pitch == SIDEBAR_ROW_HEIGHT),
        "the rows {place} are not 22 px apart: {pitches:?}"
    );
}

/// A sidebar of 50,000 refs at 22 px, every folder open, builds one viewport of rows at the top,
/// deep and at the end, and the end is the last ref. Caught by: a list that builds a row per
/// ref, rows at another height, or an end that is not the snapshot's.
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

/// Folders over a flat list stay virtualized — one folder of 10,000 branches, opened, builds
/// one viewport of 22 px rows, at its top and scrolled deep into it, and closing it builds none
/// of them. Caught by: a folder that builds every row it holds when opened.
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
