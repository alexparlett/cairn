//! Headless tests for the Changes tab's list and summary (PRD R5.4, R5.5; criterion C10's
//! "the Changes tab shows the list, the filter and one file's diff").

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    ChangeSet, ChangeStatus, ChangedFile, CommitDetails, FileMode, Oid, RenameDetection, RepoPath,
    Signature, Timestamp,
};
use cairn_ui::{
    ChangesList, DETAIL_ROW_HEIGHT, FILTER_PLACEHOLDER, NO_FILE_MATCHES, ShownFiles, summary_parts,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 400.;
const HEIGHT: f32 = 500.;
/// The largest subject's file count: rust-lang/rust's S1, `cf2dff2b1e3`.
const S1_FILES: usize = 55_184;

fn file(n: usize) -> ChangedFile {
    let path = format!("dir{}/file-{n:05}.rs", n % 7);
    ChangedFile {
        status: ChangeStatus::Modified,
        old_path: RepoPath::from(path.as_str()),
        new_path: RepoPath::from(path.as_str()),
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        old_id: None,
        new_id: None,
    }
}

fn change_set(files: usize) -> ChangeSet {
    ChangeSet {
        files: (0..files).map(file).collect(),
        details: None,
        renames: RenameDetection::default(),
    }
}

#[derive(Clone, Copy)]
struct Fixture {
    changes: State<ChangeSet>,
    shown: State<ShownFiles>,
    filter: State<String>,
    current: State<Option<usize>>,
}

type Chosen = Rc<RefCell<Vec<usize>>>;

fn launch(changes: ChangeSet, shown: ShownFiles) -> (TestingRunner, Fixture, Chosen) {
    let chosen = Chosen::default();
    let heard = chosen.clone();
    let (mut test, fixture) = TestingRunner::new(
        move || {
            let fixture = use_consume::<Fixture>();
            let heard = heard.clone();
            let mut current = fixture.current;
            rect()
                .expanded()
                .child(
                    ChangesList::new(fixture.changes, fixture.shown, fixture.filter)
                        .current(*fixture.current.read())
                        .on_file(move |index: usize| {
                            heard.borrow_mut().push(index);
                            current.set(Some(index));
                        }),
                )
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || Fixture {
                changes: State::create(changes),
                shown: State::create(shown),
                filter: State::create(String::new()),
                current: State::create(None),
            })
        },
        1.,
    );
    test.sync_and_update();
    (test, fixture, chosen)
}

/// The file rows built, as `(path, visible)`.
fn built_files(test: &TestingRunner) -> Vec<(String, bool)> {
    test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text.starts_with("dir"))
            .map(|label| (label.text.to_string(), node.is_visible()))
    })
}

/// R5.4, R5.5 at the largest subject's size: the list of 55,184 files builds one viewport of
/// rows at the top, deep and at the end, and a filter's answer — every other file — builds
/// one viewport too, the files it leaves in their order, to the last of them. The filter's
/// answer is handed to the list as indices; the list scans nothing. Caught by: a list that
/// builds a row per file, or one that rebuilds every row when the filter's answer changes.
#[test]
fn a_list_of_55184_files_builds_one_viewport_filtered_or_not() {
    let (mut test, fixture, _) = launch(change_set(S1_FILES), ShownFiles::All);
    let viewport = ((HEIGHT - 40.) / DETAIL_ROW_HEIGHT).ceil() as usize;
    let within = |test: &TestingRunner, place: &str| {
        let built = built_files(test).len();
        assert!(
            built >= viewport / 2 && built <= viewport + 3,
            "{built} rows built for a viewport of about {viewport} {place}"
        );
    };
    within(&test, "at the top");
    test.scroll((100., 200.), (0., -(S1_FILES as f64) * 12.));
    within(&test, "deep");

    let mut shown = fixture.shown;
    let every_other: Vec<u32> = (0..S1_FILES as u32).step_by(2).collect();
    test.run_in(|| shown.set(ShownFiles::Filtered(every_other.clone())));
    test.sync_and_update();
    within(&test, "after the filter's answer, deep");
    test.scroll((100., 200.), (0., -(S1_FILES as f64) * 48.));
    test.sync_and_update();
    within(&test, "after the filter's answer, at the end");
    let last = file(*every_other.last().unwrap_or(&0) as usize)
        .new_path
        .display()
        .into_owned();
    assert!(
        built_files(&test)
            .iter()
            .any(|(path, visible)| *visible && *path == last),
        "the end of the filtered list is not its last file, {last}"
    );
    assert!(
        built_files(&test).iter().all(|(path, _)| path
            .trim_end_matches(".rs")
            .ends_with(['0', '2', '4', '6', '8'])),
        "a file the filter left out is drawn"
    );
}

/// R5.4, Fork's model (user decision 6): a pressed file is chosen and gives the list focus;
/// then ↓ and ↑ move to the next and previous file the list SHOWS — a filter's answer is
/// stepped over by its rows — reporting each by its index in the change set, and stop at
/// either end. Caught by: arrows that step through the change set rather than the list, or a
/// press that reports a row number rather than the file's index.
#[test]
fn the_arrows_move_through_the_files_the_list_shows() {
    let (mut test, _, chosen) = launch(change_set(40), ShownFiles::Filtered(vec![3, 7, 20]));
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == file(7).new_path.display())
                .map(|_| node.layout().area.center())
        })
        .expect("file 7 is shown");
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
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
    assert_eq!(chosen.borrow().as_slice(), [7, 20, 20, 7, 3, 3]);
}

/// The filter field is the list's, and what is typed is the filter's text — the caller's to
/// ask about; a filter that leaves nothing says so. Caught by: no field, or a list drawn
/// empty with no word.
#[test]
fn the_filter_field_takes_the_text_and_an_empty_answer_says_so() {
    let (mut test, fixture, _) = launch(change_set(10), ShownFiles::All);
    let field = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == FILTER_PLACEHOLDER)
                .map(|_| node.layout().area.center())
        })
        .or_else(|| {
            test.find(|node, element| {
                Paragraph::try_downcast(element)
                    .filter(|paragraph| {
                        paragraph
                            .spans
                            .iter()
                            .any(|span| span.text.as_ref() == FILTER_PLACEHOLDER)
                    })
                    .map(|_| node.layout().area.center())
            })
        })
        .expect("the filter field shows its placeholder");
    test.click_cursor((f64::from(field.x), f64::from(field.y)));
    test.sync_and_update();
    test.write_text("rs");
    test.sync_and_update();
    assert_eq!(*fixture.filter.read(), "rs");

    let mut shown = fixture.shown;
    test.run_in(|| shown.set(ShownFiles::Filtered(Vec::new())));
    test.sync_and_update();
    let labels: Vec<String> =
        test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()));
    assert!(labels.iter().any(|l| l == NO_FILE_MATCHES), "{labels:?}");
}

/// Fork's summary line (Finding 2): who wrote the commit, its short id, the date at its own
/// offset in the user's chosen format, and the subject. Caught by: the committer's name or
/// date in the author's place, a full id, or the message's body.
#[test]
fn the_summary_is_author_short_id_date_and_subject() {
    let signature = |name: &str, seconds| Signature {
        name: name.to_owned(),
        email: format!("{name}@example.com"),
        time: Timestamp::new(seconds, -30 * 60),
    };
    let id = Oid::from_bytes(&[0xab; 20]).unwrap();
    let details = CommitDetails {
        id,
        parents: Vec::new(),
        author: signature("Ada", 1_700_000_000),
        committer: signature("Grace", 1_800_000_000),
        message: "Subject line\n\nBody text\n".to_owned(),
    };
    assert_eq!(
        summary_parts(&details),
        [
            "Ada".to_owned(),
            "abababa".to_owned(),
            "14 Nov 2023 21:43:20 -00:30".to_owned(),
            "Subject line".to_owned(),
        ]
    );
}
