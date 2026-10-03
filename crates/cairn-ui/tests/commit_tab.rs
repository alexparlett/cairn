//! Headless component tests for the detail pane's Commit tab and its strip.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    ChangeSet, ChangeStatus, ChangedFile, CommitDetails, FileMode, Oid, RenameDetection, RepoPath,
    Signature, Similarity, Timestamp,
};
use cairn_ui::accelerators::{self, Action, Os, Scope};
use cairn_ui::{
    AUTHOR_CAPTION, COLLAPSE_CAPTION, COMMITTER_CAPTION, CommitTab, DETAIL_ROW_HEIGHT, DetailTab,
    DetailTabs, EXPAND_CAPTION, ID_CAPTION, NO_FILES, PARENTS_CAPTION, cut_short_notice,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 480.;

fn oid(n: u64) -> Oid {
    let mut bytes = [0u8; 20];
    bytes[0] = 0xab;
    // The short id is the first seven digits: make it differ between ids.
    bytes[1] = (n % 256) as u8;
    bytes[12..20].copy_from_slice(&n.to_be_bytes());
    Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
}

fn file(n: usize) -> ChangedFile {
    let path = RepoPath::from(format!("src/file-{n:06}.rs").as_str());
    ChangedFile {
        status: ChangeStatus::Modified,
        old_path: path.clone(),
        new_path: path,
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        old_id: Some(oid(1)),
        new_id: Some(oid(2)),
    }
}

/// Blank lines before and after, CRLF lines, trailing whitespace and a tab: what git trims
/// and expands before it shows a message.
const MESSAGE: &str = "\n  \nTeach the engine to count\r\n\r\nThe first paragraph of the body.\r\n\
                       \tA tabbed line of it.  \n\n\nSigned-off-by: Ada Lovelace \
                       <ada@example.com>\n\n \n";

/// `git log --format=fuller` (git 2.56) on a commit whose message is [`MESSAGE`], each line
/// without the four spaces git indents it by.
const GIT_SHOWS: [&str; 7] = [
    "Teach the engine to count",
    "",
    "The first paragraph of the body.",
    "        A tabbed line of it.",
    "",
    "",
    "Signed-off-by: Ada Lovelace <ada@example.com>",
];

fn details(id: Oid, parents: Vec<Oid>, message: &str) -> CommitDetails {
    CommitDetails {
        id,
        parents,
        author: Signature {
            name: "Ada Lovelace".to_owned(),
            email: "ada@example.com".to_owned(),
            time: Timestamp::new(1_700_000_000, 5 * 3600 + 30 * 60),
        },
        committer: Signature {
            name: "Grace Hopper".to_owned(),
            email: "grace@example.com".to_owned(),
            time: Timestamp::new(1_700_000_000, -8 * 3600),
        },
        message: message.to_owned(),
    }
}

/// A merge with a renamed, an added and a deleted file, and whatever else `more` adds.
fn change_set(more: usize) -> ChangeSet {
    let mut files = vec![
        ChangedFile {
            status: ChangeStatus::Renamed(Similarity::from_percent(87)),
            old_path: RepoPath::from("docs/old-name.md"),
            new_path: RepoPath::from("docs/new-name.md"),
            ..file(0)
        },
        ChangedFile {
            status: ChangeStatus::Added,
            old_mode: None,
            old_id: None,
            ..file(1)
        },
        ChangedFile {
            status: ChangeStatus::Deleted,
            new_mode: None,
            new_id: None,
            ..file(2)
        },
    ];
    files.extend((3..3 + more).map(file));
    ChangeSet {
        files,
        details: Some(details(oid(77), vec![oid(10), oid(11)], MESSAGE)),
        renames: RenameDetection::default(),
    }
}

#[derive(Clone)]
struct Fixture {
    changes: State<ChangeSet>,
}

type Parents = Rc<RefCell<Vec<Oid>>>;

fn launch(initial: ChangeSet) -> (TestingRunner, Fixture, Parents) {
    let parents = Parents::default();
    let app = {
        let parents = parents.clone();
        move || {
            let fixture = use_consume::<Fixture>();
            let parents = parents.clone();
            rect()
                .expanded()
                .child(
                    CommitTab::new(fixture.changes)
                        .on_parent(move |parent: Oid| parents.borrow_mut().push(parent)),
                )
                .into_element()
        }
    };
    let (mut test, fixture) = TestingRunner::new(
        app,
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(|| Fixture {
                changes: State::create(initial),
            })
        },
        1.,
    );
    test.sync_and_update();
    test.sync_and_update();
    (test, fixture, parents)
}

fn texts(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

fn shows(test: &TestingRunner, text: &str) -> bool {
    texts(test).iter().any(|shown| shown == text)
}

/// Every file row built, as `(text, visible)`.
fn built_files(test: &TestingRunner) -> Vec<(String, bool)> {
    test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text.starts_with("src/file-"))
            .map(|label| (label.text.to_string(), node.is_visible()))
    })
}

/// C10, R5.3: the author and the committer, each with name, email and the full timestamp at
/// its own offset as git prints it; the full id; each parent as its short id; and the
/// files, a rename showing both names (the message, row for row:
/// `the_message_is_the_rows_git_log_shows_for_it`). Caught by: dropping the
/// committer, rendering a date in UTC or without its offset, abbreviating the id, dropping
/// a parent or the body, or a rename showing only its new name.
#[test]
fn the_commit_tab_shows_every_field_r5_3_names() {
    let (test, _, _) = launch(change_set(0));
    let shown = texts(&test);

    let expected = vec![
        AUTHOR_CAPTION.to_owned(),
        COMMITTER_CAPTION.to_owned(),
        "Ada Lovelace <ada@example.com>".to_owned(),
        "Grace Hopper <grace@example.com>".to_owned(),
        // `git log --format=fuller`'s AuthorDate and CommitDate for these moments.
        "Wed Nov 15 03:43:20 2023 +0530".to_owned(),
        "Tue Nov 14 14:13:20 2023 -0800".to_owned(),
        ID_CAPTION.to_owned(),
        oid(77).hex().as_str().to_owned(),
        PARENTS_CAPTION.to_owned(),
        oid(10).short().as_str().to_owned(),
        oid(11).short().as_str().to_owned(),
        "docs/old-name.md → docs/new-name.md".to_owned(),
        "src/file-000001.rs".to_owned(),
        "src/file-000002.rs".to_owned(),
    ];
    for text in expected {
        assert!(
            shown.contains(&text),
            "the Commit tab does not show {text:?}; it shows {shown:?}"
        );
    }
    for status in ["R", "A", "D"] {
        assert!(
            shown.iter().any(|t| t == status),
            "no {status} badge: {shown:?}"
        );
    }
    assert!(
        !shown.iter().any(|t| t == "src/file-000000.rs"),
        "the rename's new name stands alone, without its old one: {shown:?}"
    );
}

/// T3, R5.3: the message is the rows git shows for it, row for row and nothing between —
/// the blank lines before and after it gone, each line without its trailing whitespace or
/// `\r`, blank lines inside it kept, a tab expanded — between the last parent and the
/// first file. Caught by: popping only one trailing empty line, keeping the leading blank
/// lines or a `\r`, dropping a blank line inside, or drawing the tab raw.
#[test]
fn the_message_is_the_rows_git_log_shows_for_it() {
    let (test, _, _) = launch(change_set(0));
    let shown = texts(&test);
    let after_parents = shown
        .iter()
        .position(|text| *text == oid(11).short().as_str())
        .unwrap_or_else(|| panic!("no second parent: {shown:?}"))
        + 1;
    let files = shown
        .iter()
        .position(|text| text == "docs/old-name.md → docs/new-name.md")
        .unwrap_or_else(|| panic!("no first file: {shown:?}"));
    // Between them: the message, then the first file's badge.
    let message: Vec<&str> = shown[after_parents..files - 1]
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(message, GIT_SHOWS);
}

/// R5.5, the twin of `only_a_viewport_of_rows_is_built_however_long_the_history`: the
/// packet's own measured commit touches 55,184 paths, and the tab builds one viewport of
/// them at the top and scrolled deep. Caught by: a list that builds a row per file.
#[test]
fn only_a_viewport_of_files_is_built_however_many_the_commit_touched() {
    let viewport_rows = (HEIGHT / DETAIL_ROW_HEIGHT).ceil() as usize;
    let mut built = Vec::new();
    for files in [1_000, 55_184] {
        let (mut test, _, _) = launch(change_set(files));
        let at_top = built_files(&test).len();
        assert!(
            at_top <= viewport_rows,
            "{at_top} file rows were built for a {viewport_rows}-row viewport over {files} files"
        );

        let deep = files - 200;
        test.scroll(
            (100., 100.),
            (0., -(deep as f64 * f64::from(DETAIL_ROW_HEIGHT))),
        );
        let scrolled = built_files(&test);
        assert!(
            scrolled.len() >= viewport_rows && scrolled.len() <= viewport_rows + 2,
            "{} file rows were built scrolled deep into {files} files",
            scrolled.len()
        );
        assert!(
            scrolled
                .iter()
                .any(|(text, visible)| *visible && *text == format!("src/file-{deep:06}.rs")),
            "scrolling to file {deep} of {files} did not build it: {scrolled:?}"
        );
        built.push(scrolled.len());

        // And the very end: the last file is reachable, which a list told to be only as
        // long as the files (forgetting the header's rows) would cut off.
        test.scroll(
            (100., 100.),
            (0., -((files + 100) as f64 * f64::from(DETAIL_ROW_HEIGHT))),
        );
        let last = format!("src/file-{:06}.rs", files + 2);
        let at_end = built_files(&test);
        assert!(
            at_end
                .iter()
                .any(|(text, visible)| *visible && *text == last),
            "scrolling to the end of {files} files did not build the last, {last}: {at_end:?}"
        );
    }
    assert!(
        built.windows(2).all(|pair| pair[0] == pair[1]),
        "more files built a different number of rows: {built:?}"
    );
}

/// R5.3: a parent is a link that reports which parent it is; the tab itself reaches for
/// nothing. Caught by: every link reporting the first parent.
#[test]
fn a_parent_link_reports_its_parent() {
    let (mut test, _, parents) = launch(change_set(0));
    let second = oid(11).short().as_str().to_owned();
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == second)
                .map(|_| node.layout().area.center())
        })
        .unwrap();
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    assert_eq!(parents.borrow().as_slice(), [oid(11)]);
}

/// R2.2: a rename search `diff.renameLimit` stopped says so where the files are, with the
/// limit git asks for; one that ran to the end says nothing. A commit with no files says
/// that too. Caught by: the notice drawn always, never, or without its number.
#[test]
fn a_cut_short_rename_search_is_said_above_the_files() {
    let notice = cut_short_notice(2774);
    let (test, _, _) = launch(change_set(0));
    assert!(!shows(&test, &notice));
    assert!(!shows(&test, NO_FILES));

    let mut cut = change_set(0);
    cut.renames = RenameDetection {
        enabled: true,
        copies: false,
        limit: Some(1000),
        needed_limit: Some(2774),
    };
    let (test, _, _) = launch(cut);
    assert!(shows(&test, &notice), "{:?}", texts(&test));
    assert!(notice.contains("2774"));
    // Above the files, not among or below them.
    let top_of = |text: &str| {
        test.find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .map(|_| node.layout().area.min_y())
        })
        .unwrap_or_else(|| panic!("no label reads {text:?}"))
    };
    assert!(
        top_of(&notice) < top_of("docs/old-name.md → docs/new-name.md"),
        "the notice is not above the files"
    );

    // A search that had a limit and stayed under it was not cut short.
    let mut limited = change_set(0);
    limited.renames = RenameDetection {
        enabled: true,
        copies: false,
        limit: Some(1000),
        needed_limit: None,
    };
    let (test, _, _) = launch(limited);
    assert!(
        !texts(&test)
            .iter()
            .any(|text| text.contains("diff.renameLimit")),
        "a limit that was not reached is said to have cut the search: {:?}",
        texts(&test)
    );

    let mut empty = change_set(0);
    empty.files.clear();
    let (test, _, _) = launch(empty);
    assert!(shows(&test, NO_FILES), "{:?}", texts(&test));
}

/// The tab draws the answer it is handed now: a second commit's answer replaces the first's
/// everywhere, rows already built included. Caught by: a list that keeps the rows it built
/// because their count did not change.
#[test]
fn another_commits_answer_replaces_every_row_of_the_last() {
    let (mut test, fixture, _) = launch(change_set(0));
    let mut next = change_set(0);
    next.details = Some(details(oid(78), vec![oid(12)], "Another subject\n"));
    for (n, file) in next.files.iter_mut().enumerate() {
        file.status = ChangeStatus::Modified;
        file.old_path = RepoPath::from(format!("lib/other-{n}.rs").as_str());
        file.new_path = file.old_path.clone();
    }
    let mut changes = fixture.changes;
    changes.set(next);
    test.sync_and_update();
    test.sync_and_update();

    let shown = texts(&test);
    assert!(shown.iter().any(|t| t == "Another subject"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "lib/other-1.rs"), "{shown:?}");
    for stale in [
        "Teach the engine to count",
        "docs/old-name.md → docs/new-name.md",
        "src/file-000001.rs",
    ] {
        assert!(
            !shown.iter().any(|t| t == stale),
            "{stale:?} of the previous commit is still drawn: {shown:?}"
        );
    }
}

/// What the focus tests saw: the files made current, and the detail pane's actions heard.
#[derive(Default, Clone)]
struct Heard {
    files: Rc<RefCell<Vec<usize>>>,
    actions: Rc<RefCell<Vec<Action>>>,
}

const ELSEWHERE: &str = "Elsewhere";

/// The tab inside a pane that hears the detail pane's chords as the window's does — on a
/// key press that reaches it from whatever inside has focus — after a focusable control
/// standing for the rest of the window.
fn launch_focus(initial: ChangeSet) -> (TestingRunner, Heard) {
    let heard = Heard::default();
    let app = {
        let heard = heard.clone();
        move || {
            let fixture = use_consume::<Fixture>();
            let (files, actions) = (heard.files.clone(), heard.actions.clone());
            rect()
                .expanded()
                .child(Button::new().child(ELSEWHERE))
                .child(
                    rect()
                        .expanded()
                        .on_key_down(move |e: Event<KeyboardEventData>| {
                            if let Some(action) = accelerators::resolve_key(&e, Scope::Detail) {
                                e.stop_propagation();
                                actions.borrow_mut().push(action);
                            }
                        })
                        .child(
                            CommitTab::new(fixture.changes)
                                .on_file(move |index: usize| files.borrow_mut().push(index)),
                        ),
                )
                .into_element()
        }
    };
    let (mut test, _) = TestingRunner::new(
        app,
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(|| Fixture {
                changes: State::create(initial),
            })
        },
        1.,
    );
    test.sync_and_update();
    test.sync_and_update();
    (test, heard)
}

fn press_key(test: &mut TestingRunner, key: NamedKey) {
    test.press_key(Key::Named(key));
    test.sync_and_update();
}

fn press_action(test: &mut TestingRunner, action: Action) {
    // A named key: its code is not what the table matches it by.
    let Some((key, _, modifiers)) =
        accelerators::chord(action, Os::current()).and_then(|chord| chord.key_press())
    else {
        panic!("{action:?} has no key chord");
    };
    test.press_key_with_modifiers(key, modifiers);
    test.sync_and_update();
}

/// User decision 6, Fork's previous and next file: a pressed file row is the current file
/// and gives the tab focus, and while it has focus ↑ and ↓ move the current file — stopping
/// at either end, and bringing a file below the viewport into view. Caught by: arrows that
/// move nothing, run past the ends, or leave the current file out of sight.
#[test]
fn the_focused_file_list_moves_the_current_file_with_the_arrows() {
    let more = 40;
    let last = more + 2;
    let (mut test, heard) = launch_focus(change_set(more));
    click_label(&mut test, "src/file-000001.rs");
    assert_eq!(heard.files.borrow().as_slice(), [1]);

    press_key(&mut test, NamedKey::ArrowDown);
    for _ in 0..5 {
        press_key(&mut test, NamedKey::ArrowUp);
    }
    assert_eq!(heard.files.borrow().as_slice(), [1, 2, 1, 0, 0, 0, 0]);

    for _ in 0..last + 5 {
        press_key(&mut test, NamedKey::ArrowDown);
    }
    assert_eq!(heard.files.borrow().last(), Some(&last));
    let name = format!("src/file-{last:06}.rs");
    assert!(
        built_files(&test)
            .iter()
            .any(|(text, visible)| *visible && *text == name),
        "the last file was made current out of sight: {:?}",
        built_files(&test)
    );
}

/// User decision 6: previous and next change are heard only while the detail pane has
/// focus, and pressing one moves no file; Tab moves focus into the file list and Shift-Tab
/// out of it again, after which its arrows are not the list's. Caught by: change chords
/// heard wherever focus is (they would take ⌘↑ from a text field), a file list that reads
/// a chord as its arrow, or a tab that Tab cannot reach.
#[test]
fn the_change_chords_and_the_arrows_belong_to_the_focused_pane() {
    let (mut test, heard) = launch_focus(change_set(5));
    click_label(&mut test, "src/file-000001.rs");
    press_action(&mut test, Action::NextChange);
    press_action(&mut test, Action::PreviousChange);
    assert_eq!(
        heard.actions.borrow().as_slice(),
        [Action::NextChange, Action::PreviousChange]
    );
    assert_eq!(
        heard.files.borrow().as_slice(),
        [1],
        "a chord moved the file"
    );

    click_label(&mut test, ELSEWHERE);
    press_action(&mut test, Action::NextChange);
    press_key(&mut test, NamedKey::ArrowDown);
    assert_eq!(
        heard.actions.borrow().len(),
        2,
        "a change chord was heard with focus outside the pane"
    );
    assert_eq!(
        heard.files.borrow().as_slice(),
        [1],
        "the arrows moved an unfocused list"
    );

    // Back with Tab, out with Shift-Tab.
    press_key(&mut test, NamedKey::Tab);
    test.sync_and_update();
    press_key(&mut test, NamedKey::ArrowDown);
    // The current file is where it was left.
    assert_eq!(
        heard.files.borrow().as_slice(),
        [1, 2],
        "Tab did not reach the file list"
    );
    test.press_key_with_modifiers(Key::Named(NamedKey::Tab), Modifiers::SHIFT);
    test.sync_and_update();
    test.sync_and_update();
    press_key(&mut test, NamedKey::ArrowDown);
    assert_eq!(
        heard.files.borrow().as_slice(),
        [1, 2],
        "Shift-Tab did not leave the file list"
    );
}

#[derive(Clone)]
struct Strip {
    tab: State<DetailTab>,
    collapsed: State<bool>,
}

fn launch_strip() -> (TestingRunner, Strip) {
    let app = || {
        let strip = use_consume::<Strip>();
        let (mut tab, mut collapsed) = (strip.tab, strip.collapsed);
        DetailTabs::new(*tab.read())
            .collapsed(*collapsed.read())
            .on_tab(move |chosen: DetailTab| tab.set(chosen))
            .on_collapse(move |collapse: bool| collapsed.set(collapse))
            .into_element()
    };
    let (mut test, strip) = TestingRunner::new(
        app,
        (WIDTH, 60.).into(),
        |runner| {
            runner.provide_root_context(|| Strip {
                tab: State::create(DetailTab::default()),
                collapsed: State::create(false),
            })
        },
        1.,
    );
    test.sync_and_update();
    (test, strip)
}

fn click_label(test: &mut TestingRunner, caption: &str) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == caption)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no label reads {caption:?}: {:?}", texts(test)));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.sync_and_update();
}

/// R5.2: Commit is the default and both tabs report themselves; the strip's control asks to
/// collapse an open pane and to open a collapsed one. Caught by: Changes as the default, a
/// tab reporting the other, or a control that only ever collapses.
#[test]
fn the_strip_reports_its_tabs_and_its_collapse() {
    let (mut test, strip) = launch_strip();
    assert_eq!(*strip.tab.read(), DetailTab::Commit);

    click_label(&mut test, "Changes");
    assert_eq!(*strip.tab.read(), DetailTab::Changes);
    click_label(&mut test, "Commit");
    assert_eq!(*strip.tab.read(), DetailTab::Commit);

    click_label(&mut test, COLLAPSE_CAPTION);
    assert!(*strip.collapsed.read());
    click_label(&mut test, EXPAND_CAPTION);
    assert!(!*strip.collapsed.read());
}
