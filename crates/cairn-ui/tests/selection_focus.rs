//! A selected row says whether its list has focus, as Fork's lists do: Fork's blue with white
//! bold text while it has, Fork's grey with the body's text, still bold, when not — in the
//! history, the sidebar, both of Local Changes' lists and the Changes tab's files. Focus is
//! the toolkit's: a press on a row gives its list focus, a press elsewhere takes it away.

use std::sync::Arc;

use cairn_model::{
    ChangeList, ChangeSet, ChangeStatus, ChangedEntry, ChangedFile, Disclosure, FileMode, GraphRow,
    HeadState, History, Lane, LocalChanges, Oid, PagedCommit, Ref, RefKind, RefName, RefTarget,
    RefsSnapshot, RenameDetection, RepoPath, RowContent, RowId, RowsPage, StagedChange,
    StatusEntry, UnstagedChange, WorkingTreeStatus,
};
use cairn_ui::style::tones;
use cairn_ui::{
    ChangesList, CommitRow, HistoryList, LocalChangesList, LocalSelection, RowRender, ShownFiles,
    Sidebar, SidebarRefs, SidebarTarget,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 500.;
/// Where the focusable area beside each list begins: a press there takes focus from the list.
const ELSEWHERE_X: f32 = 800.;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap_or_else(|error| panic!("{error}"))
}

/// A focusable area beside the list under test, taking focus when pressed.
#[derive(PartialEq)]
struct Elsewhere;

impl Component for Elsewhere {
    fn render(&self) -> impl IntoElement {
        let id = use_a11y();
        rect()
            .width(Size::px(WIDTH - ELSEWHERE_X))
            .height(Size::fill())
            .a11y_id(id)
            .a11y_focusable(true)
            .on_press(move |_| id.request_focus())
    }
}

/// `list` at the left, the focusable area at the right.
fn beside(list: impl IntoElement) -> Element {
    rect()
        .horizontal()
        .expanded()
        .child(
            rect()
                .width(Size::px(ELSEWHERE_X))
                .height(Size::fill())
                .child(list),
        )
        .child(Elsewhere)
        .into()
}

fn launch(app: impl Fn() -> Element + 'static) -> TestingRunner {
    let (mut test, ()) = TestingRunner::new(app, (WIDTH, HEIGHT).into(), |_| {}, 1.);
    settle(&mut test);
    test
}

fn settle(test: &mut TestingRunner) {
    for _ in 0..3 {
        test.sync_and_update();
    }
}

fn press(test: &mut TestingRunner, at: (f32, f32)) {
    test.click_cursor((f64::from(at.0), f64::from(at.1)));
    settle(test);
}

fn press_elsewhere(test: &mut TestingRunner) {
    press(test, (ELSEWHERE_X + 20., HEIGHT / 2.));
}

/// The label reading `text`: where it is, its colour and whether it is bold.
fn label_of(test: &TestingRunner, text: &str) -> ((f32, f32), Option<Color>, bool) {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|label| {
                let centre = node.layout().area.center();
                (
                    (centre.x, centre.y),
                    label
                        .text_style_data
                        .color
                        .as_ref()
                        .and_then(Fill::as_color),
                    label.text_style_data.font_weight == Some(FontWeight::BOLD),
                )
            })
    })
    .unwrap_or_else(|| panic!("no label reads {text:?}"))
}

/// The selection bands drawn behind `at`, focused or not.
fn bands_at(test: &TestingRunner, at: (f32, f32)) -> Vec<Color> {
    let bands = [tones::SELECTION_FOCUSED, tones::SELECTION_UNFOCUSED];
    test.find_many(|node, element| {
        let area = node.layout().area;
        Rect::try_downcast(element)
            .filter(|_| {
                area.min_x() <= at.0
                    && at.0 <= area.max_x()
                    && area.min_y() <= at.1
                    && at.1 <= area.max_y()
            })
            .and_then(|rect| rect.style.background.as_color())
            .filter(|band| bands.contains(band))
    })
}

/// The row reading `text` is drawn selected in the tone its list's focus says: its band
/// behind it, and nothing else in either band anywhere in the window; its text in the
/// selection's tone, bold.
fn assert_selected(test: &TestingRunner, list: &str, text: &str, focused: bool) {
    let chosen = tones::selected(focused);
    let (at, colour, bold) = label_of(test, text);
    let state = if focused { "focused" } else { "unfocused" };
    assert_eq!(
        bands_at(test, at),
        vec![chosen.band],
        "{list}, {state}: the selected row's band"
    );
    assert_eq!(
        colour,
        Some(chosen.text),
        "{list}, {state}: the selected row's text"
    );
    assert!(bold, "{list}, {state}: the selected row's text is not bold");
    let other = tones::selected(!focused).band;
    let elsewhere = test.find_many(|_, element| {
        Rect::try_downcast(element)
            .and_then(|rect| rect.style.background.as_color())
            .filter(|band| *band == other)
    });
    assert!(
        elsewhere.is_empty(),
        "{list}, {state}: the other band is drawn too"
    );
}

fn history_app() -> Element {
    let rows = use_hook(|| {
        let mut page = RowsPage::new();
        for n in 0..5u8 {
            page.push(
                GraphRow::new(oid(n), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents: 1,
                    subject: &format!("commit {n}"),
                    author: "A",
                    author_time: 0,
                },
            );
        }
        let mut history = History::new();
        history.append(page).unwrap_or_else(|full| panic!("{full}"));
        State::create(history)
    });
    beside(
        HistoryList::new(rows, |render: RowRender| match render.content {
            RowContent::Commit(commit) => CommitRow::new(commit, render.graph, render.lanes)
                .selected(render.selected)
                .focused(render.focused)
                .into(),
            RowContent::Stash(stash) => {
                CommitRow::new(stash.as_commit(), render.graph, render.lanes)
                    .selected(render.selected)
                    .focused(render.focused)
                    .into()
            }
        })
        .selected(Some(RowId::Commit(oid(2)))),
    )
}

fn sidebar_app() -> Element {
    let shown = use_hook(|| {
        let listed = |name: &str| Ref {
            name: RefName::new(name),
            kind: RefKind::LocalBranch,
            target: RefTarget::Commit(oid(1)),
            symbolic: None,
            upstream: None,
            commit_time: None,
        };
        let refs = RefsSnapshot {
            refs: vec![listed("refs/heads/main"), listed("refs/heads/topic")],
            head: HeadState::Branch(RefName::new("refs/heads/main")),
            stashes: Vec::new(),
            unreadable: 0,
            substitutions: cairn_model::Substitutions::default(),
        };
        let rows = refs
            .sidebar_rows("", &Disclosure::default(), || true)
            .unwrap_or_else(|| panic!("laying out the rows stopped"));
        SidebarRefs {
            refs: Arc::new(refs),
            rows: Arc::new(rows),
        }
    });
    let filter = use_state(String::new);
    beside(
        Sidebar::new(Some(shown), filter)
            .chosen(Some(SidebarTarget::Ref(RefName::new("refs/heads/topic")))),
    )
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

fn local_changes_app() -> Element {
    let changes = use_state(|| {
        LocalChanges::new(WorkingTreeStatus::Listed(vec![
            changed("staged.rs", Some(StagedChange::Modified), None),
            changed("unstaged.rs", None, Some(UnstagedChange::Modified)),
        ]))
    });
    let unstaged = use_state(|| ShownFiles::All);
    let staged = use_state(|| ShownFiles::All);
    let filter = use_state(String::new);
    let mut selection = use_state(|| None::<LocalSelection>);
    beside(
        LocalChangesList::new(changes, unstaged, staged, filter)
            .selection(selection)
            .on_choose(move |(list, row): (ChangeList, usize)| {
                let path = changes.peek().get(list, row).map(|c| c.path.clone());
                selection.set(path.map(|path| LocalSelection::of(list, path)));
            }),
    )
}

fn changes_app() -> Element {
    let changes = use_state(|| {
        let file = |path: &str| ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from(path),
            new_path: RepoPath::from(path),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: None,
            new_id: None,
        };
        ChangeSet {
            files: vec![file("first.rs"), file("second.rs")],
            details: None,
            renames: RenameDetection::default(),
        }
    });
    let shown = use_state(|| ShownFiles::All);
    let filter = use_state(String::new);
    beside(ChangesList::new(changes, shown, filter).current(Some(1)))
}

/// Each list's selected row is Fork's blue with white bold text while the list has focus, and
/// Fork's grey with the body's text, still bold, once a press elsewhere takes focus from it;
/// a press on the row gives the list focus back. Caught by: a selection drawn in one tone
/// whatever the focus, focus tracked by hand so the band does not follow the toolkit's, a list
/// that never takes focus when pressed, or the text left in the body's tone on the blue.
#[test]
fn a_selection_is_blue_while_its_list_has_focus_and_grey_when_not() {
    // The history takes focus as it opens.
    let mut test = launch(history_app);
    assert_selected(&test, "the history", "commit 2", true);
    press_elsewhere(&mut test);
    assert_selected(&test, "the history", "commit 2", false);
    let (row, ..) = label_of(&test, "commit 2");
    press(&mut test, row);
    assert_selected(&test, "the history", "commit 2", true);

    // The sidebar has no focus until one of its rows is pressed.
    let mut test = launch(sidebar_app);
    assert_selected(&test, "the sidebar", "topic", false);
    let (row, ..) = label_of(&test, "topic");
    press(&mut test, row);
    assert_selected(&test, "the sidebar", "topic", true);
    press_elsewhere(&mut test);
    assert_selected(&test, "the sidebar", "topic", false);

    // Each of Local Changes' lists, chosen by a press.
    for (list, path) in [("Unstaged", "unstaged.rs"), ("Staged", "staged.rs")] {
        let mut test = launch(local_changes_app);
        let (row, ..) = label_of(&test, path);
        press(&mut test, row);
        assert_selected(&test, list, path, true);
        press_elsewhere(&mut test);
        assert_selected(&test, list, path, false);
    }

    // The Changes tab's files, the chosen file drawn grey until the list is pressed.
    let mut test = launch(changes_app);
    assert_selected(&test, "the Changes tab's files", "second.rs", false);
    let (row, ..) = label_of(&test, "second.rs");
    press(&mut test, row);
    assert_selected(&test, "the Changes tab's files", "second.rs", true);
    press_elsewhere(&mut test);
    assert_selected(&test, "the Changes tab's files", "second.rs", false);
}
