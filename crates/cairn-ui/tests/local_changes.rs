//! Headless tests for Local Changes' lists (refs-and-status R9.1, R9.5; criterion C9's "draws
//! both lists with their badges" and "50,000 paths build one viewport").

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    ChangeList, ChangedEntry, ConflictKind, ConflictedEntry, LocalChanges, RepoPath, Similarity,
    StagedChange, StatusEntry, SubmoduleState, UnstagedChange, WorkingTreeStatus,
};
use cairn_ui::{
    DETAIL_ROW_HEIGHT, FILTERING, GLYPH_SIZE, LIST_HEADER_HEIGHT, LocalChangesList,
    NO_PATH_MATCHES, RefGlyph, STAGED_CAPTION, ShownFiles, UNSTAGED_CAPTION,
};
use freya::engine::prelude::{FontCollection, ImageInfo, raster_n32_premul};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 400.;
const HEIGHT: f32 = 600.;
/// C9's size.
const PATHS: usize = 50_000;

#[derive(Clone, Copy)]
struct Fixture {
    changes: State<LocalChanges>,
    unstaged: State<ShownFiles>,
    staged: State<ShownFiles>,
    filter: State<String>,
    chosen: State<Option<(ChangeList, usize)>>,
    split: State<f32>,
}

type Heard = Rc<RefCell<Vec<(ChangeList, usize)>>>;

fn launch(changes: LocalChanges) -> (TestingRunner, Fixture, Heard) {
    let heard = Heard::default();
    let hearing = heard.clone();
    let (mut test, fixture) = TestingRunner::new(
        move || {
            let fixture = use_consume::<Fixture>();
            let hearing = hearing.clone();
            let mut chosen = fixture.chosen;
            rect()
                .expanded()
                .child(
                    LocalChangesList::new(
                        fixture.changes,
                        fixture.unstaged,
                        fixture.staged,
                        fixture.filter,
                    )
                    .split(fixture.split)
                    .chosen(*fixture.chosen.read())
                    .on_choose(move |pressed: (ChangeList, usize)| {
                        hearing.borrow_mut().push(pressed);
                        chosen.set(Some(pressed));
                    }),
                )
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || Fixture {
                changes: State::create(changes),
                unstaged: State::create(ShownFiles::All),
                staged: State::create(ShownFiles::All),
                filter: State::create(String::new()),
                chosen: State::create(None),
                split: State::create(cairn_ui::LISTS_SPLIT),
            })
        },
        1.,
    );
    test.sync_and_update();
    (test, fixture, heard)
}

fn changed(
    path: &str,
    staged: Option<StagedChange>,
    unstaged: Option<UnstagedChange>,
) -> StatusEntry {
    StatusEntry::Changed(ChangedEntry {
        path: RepoPath::from(path),
        staged,
        unstaged,
        submodule: None,
    })
}

fn every_kind() -> LocalChanges {
    LocalChanges::new(WorkingTreeStatus::Listed(vec![
        changed(
            "both.rs",
            Some(StagedChange::Modified),
            Some(UnstagedChange::Modified),
        ),
        changed(
            "moved.rs",
            Some(StagedChange::Renamed {
                from: RepoPath::from("old.rs"),
                similarity: Similarity::from_percent(90),
            }),
            None,
        ),
        changed("gone.rs", None, Some(UnstagedChange::Deleted)),
        StatusEntry::Conflicted(ConflictedEntry {
            path: RepoPath::from("clash.rs"),
            kind: ConflictKind::BothModified,
            submodule: None,
        }),
        StatusEntry::Untracked(RepoPath::from("new/one.rs")),
        StatusEntry::Untracked(RepoPath::from("new/two.rs")),
    ]))
}

/// Every label drawn, as `(text, x, y)`.
fn labels(test: &TestingRunner) -> Vec<(String, f32, f32)> {
    test.find_many(|node, element| {
        Label::try_downcast(element).map(|label| {
            let area = node.layout().area;
            (label.text.to_string(), area.min_x(), area.center().y)
        })
    })
}

fn y_of(test: &TestingRunner, text: &str) -> Vec<f32> {
    labels(test)
        .into_iter()
        .filter(|(drawn, ..)| drawn == text)
        .map(|(_, _, y)| y)
        .collect()
}

/// The badge drawn on the row whose text is `text`, if a letter: the label left of it on its
/// line.
fn badge_of(test: &TestingRunner, y: f32, text_x: f32) -> Option<String> {
    labels(test)
        .into_iter()
        .filter(|(_, x, at)| (at - y).abs() < 1. && *x < text_x)
        .map(|(drawn, ..)| drawn)
        .next()
}

/// R9.1: Unstaged above Staged, each under its heading; a path with a staged and an unstaged
/// change in both; each row its badge and its path, a rename its source before it; a conflict
/// Fork's triangle rather than a letter; untracked files under a new directory one row each.
/// Caught by: the lists in the other order, a path in one list only, a badge another kind's,
/// a rename's source dropped, or a new directory drawn as one row.
#[test]
fn both_lists_draw_their_paths_with_their_badges() {
    let (test, _, _) = launch(every_kind());
    let unstaged = y_of(&test, UNSTAGED_CAPTION);
    let staged = y_of(&test, STAGED_CAPTION);
    assert_eq!((unstaged.len(), staged.len()), (1, 1));
    assert!(unstaged[0] < staged[0], "Unstaged is not above Staged");

    let row = |text: &str| -> Vec<(f32, Option<String>)> {
        labels(&test)
            .into_iter()
            .filter(|(drawn, ..)| drawn == text)
            .map(|(_, x, y)| (y, badge_of(&test, y, x)))
            .collect()
    };
    let both = row("both.rs");
    assert_eq!(
        both.len(),
        2,
        "a path both staged and unstaged in one list only"
    );
    assert!(
        both[0].0 < staged[0] && both[1].0 > staged[0]
            || both[1].0 < staged[0] && both[0].0 > staged[0]
    );
    for (text, badge, in_staged) in [
        ("old.rs → moved.rs", "R", true),
        ("gone.rs", "D", false),
        ("new/one.rs", "+", false),
        ("new/two.rs", "+", false),
    ] {
        let drawn = row(text);
        assert_eq!(drawn.len(), 1, "{text}");
        assert_eq!(drawn[0].1.as_deref(), Some(badge), "{text}");
        assert_eq!(
            drawn[0].0 > staged[0],
            in_staged,
            "{text} is in the wrong list"
        );
    }
    for (_, badge) in &both {
        assert_eq!(badge.as_deref(), Some("M"));
    }
    let clash = row("clash.rs");
    assert_eq!(clash.len(), 1);
    assert_eq!(clash[0].1, None, "a conflict drawn with a letter");
    assert_eq!(
        glyph_on(&test, clash[0].0),
        Some(mask_of(RefGlyph::Gone)),
        "a conflict's badge is not Fork's triangle"
    );
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

/// What `glyph` paints, as a badge's canvas would.
fn mask_of(glyph: RefGlyph) -> Vec<bool> {
    let element: Element = glyph.draw(Color::WHITE).into();
    let Element::Element { element, .. } = element else {
        panic!("a glyph is an element");
    };
    let canvas = Canvas::try_downcast(&*element).unwrap_or_else(|| panic!("a glyph is a canvas"));
    let mask = painted(&canvas.on_render);
    assert!(
        mask.iter().filter(|p| **p).count() >= 8,
        "{glyph:?} paints nothing"
    );
    mask
}

/// What the canvas drawn on the row centred at `y` paints, if one is drawn there.
fn glyph_on(test: &TestingRunner, y: f32) -> Option<Vec<bool>> {
    let canvases: Vec<(f32, CanvasElement)> = test.find_many(|node, element| {
        Canvas::try_downcast(element).map(|canvas| (node.layout().area.center().y, canvas))
    });
    canvases
        .into_iter()
        .find(|(at, _)| (at - y).abs() < 3.)
        .map(|(_, canvas)| painted(&canvas.on_render))
}

/// The user's decision (2026-10-07), TC3 and TC4: every kind of change is drawn with Fork's
/// badge, told by its shape — `M` for a modification and a type change, `+` for an added and an
/// untracked path, `D`, `R`, `C`, and, painted, the submodule's box and the conflict's triangle,
/// each badge cell holding exactly the glyph's shape. Caught by: a kind given another's badge,
/// the Commit tab's letters (`A`, `T`, `S`), or a glyph's cell drawing anything but the glyph
/// (an empty rect, the other glyph).
#[test]
fn every_kind_of_change_draws_its_badges_shape() {
    let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![
        changed("added.rs", Some(StagedChange::Added), None),
        changed(
            "copy.rs",
            Some(StagedChange::Copied {
                from: RepoPath::from("source.rs"),
                similarity: Similarity::from_percent(100),
            }),
            None,
        ),
        changed("deleted.rs", None, Some(UnstagedChange::Deleted)),
        changed("modified.rs", None, Some(UnstagedChange::Modified)),
        changed(
            "renamed.rs",
            Some(StagedChange::Renamed {
                from: RepoPath::from("was.rs"),
                similarity: Similarity::from_percent(90),
            }),
            None,
        ),
        changed("typed.rs", None, Some(UnstagedChange::TypeChanged)),
        StatusEntry::Changed(ChangedEntry {
            path: RepoPath::from("vendor"),
            staged: None,
            unstaged: Some(UnstagedChange::Modified),
            submodule: Some(SubmoduleState {
                new_commits: true,
                modified_content: false,
                untracked_content: false,
            }),
        }),
        StatusEntry::Conflicted(ConflictedEntry {
            path: RepoPath::from("xclash.rs"),
            kind: ConflictKind::BothAdded,
            submodule: None,
        }),
        StatusEntry::Untracked(RepoPath::from("zuntracked.rs")),
    ]));
    let (test, _, _) = launch(changes);
    let line = |text: &str| -> (f32, f32) {
        labels(&test)
            .into_iter()
            .find(|(drawn, ..)| drawn == text)
            .map(|(_, x, y)| (x, y))
            .unwrap_or_else(|| panic!("{text} is not drawn: {:?}", labels(&test)))
    };
    for (text, letter) in [
        ("added.rs", "+"),
        ("zuntracked.rs", "+"),
        ("source.rs → copy.rs", "C"),
        ("deleted.rs", "D"),
        ("modified.rs", "M"),
        ("was.rs → renamed.rs", "R"),
        ("typed.rs", "M"),
    ] {
        let (x, y) = line(text);
        assert_eq!(badge_of(&test, y, x).as_deref(), Some(letter), "{text}");
        assert_eq!(
            glyph_on(&test, y),
            None,
            "{text} drew a glyph besides its letter"
        );
    }
    for (text, glyph) in [
        ("vendor", RefGlyph::Submodule),
        ("xclash.rs", RefGlyph::Gone),
    ] {
        let (x, y) = line(text);
        assert_eq!(badge_of(&test, y, x), None, "{text} drew a letter");
        assert_eq!(
            glyph_on(&test, y),
            Some(mask_of(glyph)),
            "{text}'s badge is not {glyph:?}"
        );
    }
}

/// R9.1's order: each list in its paths' order, Unstaged's rows under its heading in that order.
/// Caught by: git's order kept (untracked after tracked).
#[test]
fn each_list_is_in_its_paths_order() {
    let (test, _, _) = launch(every_kind());
    let mut unstaged: Vec<(f32, String)> = labels(&test)
        .into_iter()
        .filter(|(text, ..)| text.ends_with(".rs") && !text.contains('→'))
        .map(|(text, _, y)| (y, text))
        .collect();
    unstaged.sort_by(|a, b| a.0.total_cmp(&b.0));
    let order: Vec<String> = unstaged.into_iter().map(|(_, text)| text).collect();
    assert_eq!(
        order,
        [
            "both.rs",
            "clash.rs",
            "gone.rs",
            "new/one.rs",
            "new/two.rs",
            "both.rs"
        ]
    );
}

/// A press is reported by its list and its row there, and ↑ and ↓ move through the rows the
/// list shows — a filter's rows stepped over — staying in the list. Caught by: a press
/// reported for the other list, a row number of the filter's answer rather than the list's,
/// or the arrows leaving the list.
#[test]
fn a_press_and_the_arrows_choose_a_row_of_its_list() {
    let (mut test, fixture, heard) = launch(every_kind());
    let mut unstaged = fixture.unstaged;
    // Unstaged: both.rs, clash.rs, gone.rs, new/one.rs, new/two.rs; the filter leaves 0, 2, 4.
    test.run_in(|| unstaged.set(ShownFiles::Filtered(vec![0, 2, 4])));
    test.sync_and_update();
    let at = labels(&test)
        .into_iter()
        .find(|(text, ..)| text == "gone.rs")
        .map(|(_, x, y)| (f64::from(x) + 5., f64::from(y)))
        .expect("gone.rs is drawn");
    test.click_cursor(at);
    test.sync_and_update();
    for key in [
        NamedKey::ArrowDown,
        NamedKey::ArrowDown,
        NamedKey::ArrowUp,
        NamedKey::ArrowUp,
        NamedKey::ArrowUp,
    ] {
        test.press_key(Key::Named(key));
        test.sync_and_update();
    }
    use ChangeList::Unstaged as U;
    assert_eq!(
        heard.borrow().as_slice(),
        [(U, 2), (U, 4), (U, 4), (U, 2), (U, 0), (U, 0)]
    );

    let staged = y_of(&test, "old.rs → moved.rs");
    test.click_cursor((40., f64::from(staged[0])));
    test.sync_and_update();
    assert_eq!(heard.borrow().last(), Some(&(ChangeList::Staged, 1)));
}

/// The filter is the view's: its text written where the caller reads it, the count of paths it
/// leaves said under it, "Filtering…" until its first answer, and a filter that leaves nothing
/// saying so. Caught by: no field, every path counted before an answer, or empty lists with no
/// word.
#[test]
fn the_filter_says_what_it_leaves() {
    let (mut test, fixture, _) = launch(every_kind());
    let Fixture {
        mut filter,
        mut unstaged,
        mut staged,
        ..
    } = fixture;
    test.run_in(|| {
        filter.set("rs".to_owned());
        unstaged.set(ShownFiles::Waiting);
        staged.set(ShownFiles::Waiting);
    });
    test.sync_and_update();
    assert!(!y_of(&test, FILTERING).is_empty());
    test.run_in(|| {
        unstaged.set(ShownFiles::Filtered(vec![1]));
        staged.set(ShownFiles::Filtered(vec![0]));
    });
    test.sync_and_update();
    assert!(
        !y_of(&test, "Showing 2 of 7 files").is_empty(),
        "{:?}",
        labels(&test)
    );
    test.run_in(|| {
        unstaged.set(ShownFiles::Filtered(Vec::new()));
        staged.set(ShownFiles::Filtered(Vec::new()));
    });
    test.sync_and_update();
    assert!(!y_of(&test, NO_PATH_MATCHES).is_empty());
}

fn many(paths: usize) -> LocalChanges {
    LocalChanges::new(WorkingTreeStatus::Listed(
        (0..paths)
            .map(|n| {
                let path = format!("dir{}/file-{n:05}.rs", n % 7);
                if n % 2 == 0 {
                    changed(
                        &path,
                        Some(StagedChange::Modified),
                        Some(UnstagedChange::Modified),
                    )
                } else {
                    StatusEntry::Untracked(RepoPath::from(path.as_str()))
                }
            })
            .collect(),
    ))
}

/// The rows built, as `(path, visible)`.
fn built(test: &TestingRunner) -> Vec<(String, bool)> {
    test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text.starts_with("dir"))
            .map(|label| (label.text.to_string(), node.is_visible()))
    })
}

/// C9, R9.5: a status of 50,000 paths — 50,000 rows in Unstaged and 25,000 in Staged — builds
/// one viewport of each list at the top, deep and at the end, and so does a filter's answer of
/// every other row, to the last row it leaves. Caught by: a list that builds a row per path,
/// or one that rebuilds every row when the filter's answer changes.
#[test]
fn a_status_of_50000_paths_builds_one_viewport_filtered_or_not() {
    let changes = many(PATHS);
    assert_eq!(changes.len(ChangeList::Unstaged), PATHS);
    assert_eq!(changes.len(ChangeList::Staged), PATHS / 2);
    let last_unstaged = changes
        .get(ChangeList::Unstaged, PATHS - 1)
        .map(|change| change.path.display().into_owned())
        .unwrap_or_default();
    let (mut test, fixture, _) = launch(changes);
    // Each list has about half the height under the filter and the two headings.
    let viewport = ((HEIGHT - 40. - 2. * LIST_HEADER_HEIGHT) / DETAIL_ROW_HEIGHT).ceil() as usize;
    let within = |test: &TestingRunner, place: &str| {
        let built = built(test).len();
        assert!(
            built >= viewport / 2 && built <= viewport + 6,
            "{built} rows built for two lists sharing a viewport of about {viewport} {place}"
        );
    };
    within(&test, "at the top");
    let unstaged_list = (100., 120.);
    let staged_list = (100., f64::from(HEIGHT) - 60.);
    test.scroll(unstaged_list, (0., -(PATHS as f64) * 12.));
    test.scroll(staged_list, (0., -(PATHS as f64) * 6.));
    test.sync_and_update();
    within(&test, "deep");

    let mut unstaged = fixture.unstaged;
    let every_other: Vec<u32> = (0..PATHS as u32).step_by(2).collect();
    test.run_in(|| unstaged.set(ShownFiles::Filtered(every_other.clone())));
    test.sync_and_update();
    within(&test, "after the filter's answer, deep");
    test.scroll(unstaged_list, (0., -(PATHS as f64) * 48.));
    test.sync_and_update();
    within(&test, "after the filter's answer, at the end");
    let last = fixture
        .changes
        .read()
        .get(
            ChangeList::Unstaged,
            *every_other.last().unwrap_or(&0) as usize,
        )
        .map(|change| change.path.display().into_owned())
        .unwrap_or_default();
    assert!(
        built(&test)
            .iter()
            .any(|(path, visible)| *visible && *path == last),
        "the end of the filtered list is not its last row, {last}"
    );
    test.run_in(|| unstaged.set(ShownFiles::All));
    test.sync_and_update();
    test.scroll(unstaged_list, (0., -(PATHS as f64) * 48.));
    test.sync_and_update();
    within(&test, "unfiltered, at the end");
    assert!(
        built(&test)
            .iter()
            .any(|(path, visible)| *visible && *path == last_unstaged),
        "the end of Unstaged is not its last row, {last_unstaged}"
    );
}

/// The user's decision (2026-10-07): a draggable splitter between Unstaged and Staged, the
/// share dragged to written where the caller keeps it. Caught by: two fixed halves (the Staged
/// heading does not move), or a drag that keeps nothing.
#[test]
fn the_splitter_between_the_lists_drags() {
    let (mut test, fixture, _) = launch(every_kind());
    let before = y_of(&test, STAGED_CAPTION)[0];
    let handle = test
        .find(|node, element| {
            let area = node.layout().area;
            Rect::try_downcast(element)
                .filter(|_| area.height() == ResizableContext::HANDLE_SIZE && area.width() > 300.)
                .map(|_| f64::from(area.center().y))
        })
        .expect("a splitter between the lists");
    test.press_cursor((200., handle));
    test.move_cursor((200., handle + 60.));
    test.sync_and_update();
    test.move_cursor((200., handle + 100.));
    test.sync_and_update();
    test.release_cursor((200., handle + 100.));
    test.sync_and_update();
    let after = y_of(&test, STAGED_CAPTION)[0];
    assert!(
        after > before + 60.,
        "the Staged heading moved from {before} to {after}"
    );
    assert!(
        *fixture.split.read() > cairn_ui::LISTS_SPLIT + 10.,
        "the share dragged to was not kept: {}",
        fixture.split.read()
    );
}
