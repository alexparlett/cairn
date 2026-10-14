//! The diff's staging gesture in the window, headless (staging-and-commit R9, criterion C19's
//! window half): a hovered chunk's Stage, Unstage and Discard Changes… ask the local lane for
//! exactly its lines of the diff drawn; a drag-selection narrows them and the chords to its
//! lines, and with none the chords act on the whole file; a discard of lines confirms the
//! engine's consequence and asks the lines' discard with its token; every line of a new file is
//! discarded as the file; the mode row stages the mode alone; nothing acts while a dialog is
//! open; with several paths selected the diff is the primary's alone, staged whole by ⌘S or
//! Ctrl+S from the diff and not by Return (C25's diff half); and a line selection is kept across
//! a refresh that re-reads the same text and let go of, said once, when the text differs (C26).
//! Updates are applied through `session::apply`, as the worker's stream applies them.

use cairn_model::{
    ChangeStatus, ChangedFile, ChangedRange, Consequence, Context, DiffContent, DiffLine,
    DisplayOverlay, FileDiff, FileMode, LineNumber, LineSpan, Oid, Patch, RepoPath, Selection,
    ShownDiff, StagedChange, StatusEntry, TextDiff, UnstagedChange,
};
use cairn_ui::accelerators::Action;
use cairn_ui::{
    DISCARD_CHUNK_CAPTION, GestureVerb, SELECTION_CLEARED, STAGE_CHUNK_CAPTION,
    UNSTAGE_CHUNK_CAPTION, lines_caption, mode_caption,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

use crate::confirming::Confirming;
use crate::local_changes_actions::DISCARD_TITLE;
use crate::local_changes_tests::{
    Submitted, apply, asked, changed, labels, launch, open_local_changes, press_row, status,
    untracked,
};
use crate::sidebar_state::SIDEBAR_WIDTH;
use crate::window::View;
use crate::worker::{FileQuery, LocalWrite, OperationId, Request, Update};

fn settle(test: &mut TestingRunner) {
    for _ in 0..4 {
        test.sync_and_update();
    }
}

/// A submitter that keeps what it is asked, as the window's own is handed to the actions.
fn submitter(submitted: &Submitted) -> crate::local_changes_actions::Submit {
    let submitted = std::rc::Rc::clone(submitted);
    std::rc::Rc::new(move |request: Request| submitted.borrow_mut().push(request))
}

/// Unstaged: a.rs, n.rs (untracked); Staged: s.rs.
fn entries() -> Vec<StatusEntry> {
    vec![
        changed("a.rs", None, Some(UnstagedChange::Modified)),
        untracked("n.rs"),
        changed("s.rs", Some(StagedChange::Modified), None),
    ]
}

/// Forty lines of `path`, `line {n}`, lines 4 and 30 replaced by `EDIT {n}`: two chunks at
/// context three. A new file is every line added.
fn forty_lines(path: &str, status: ChangeStatus, modes: Option<(FileMode, FileMode)>) -> FileDiff {
    forty_lines_edited(path, status, modes, [4, 30])
}

/// [`forty_lines`] with its two edits at `edits` instead.
fn forty_lines_edited(
    path: &str,
    status: ChangeStatus,
    modes: Option<(FileMode, FileMode)>,
    edits: [u32; 2],
) -> FileDiff {
    let new: Vec<DiffLine> = (0..40)
        .map(|n| {
            DiffLine::terminated(if edits.contains(&n) {
                format!("EDIT {n}")
            } else {
                format!("line {n}")
            })
        })
        .collect();
    let (old, changes) = if status == ChangeStatus::Added {
        (
            Vec::new(),
            vec![ChangedRange::new(LineSpan::at(0, 0), LineSpan::at(0, 40))],
        )
    } else {
        (
            (0..40)
                .map(|n| DiffLine::terminated(format!("line {n}")))
                .collect(),
            edits
                .iter()
                .map(|at| ChangedRange::new(LineSpan::at(*at, 1), LineSpan::at(*at, 1)))
                .collect(),
        )
    };
    FileDiff {
        file: ChangedFile {
            status,
            old_path: RepoPath::from(path),
            new_path: RepoPath::from(path),
            old_mode: modes.map(|(old, _)| old),
            new_mode: modes.map(|(_, new)| new),
            old_id: None,
            new_id: None,
        },
        content: DiffContent::Text {
            text: TextDiff::new(old, new, changes),
            overlay: DisplayOverlay::none(),
        },
    }
}

/// Answers the newest working-tree diff asked with `diff`, and returns its query.
fn answer_with(
    test: &mut TestingRunner,
    view: View,
    submitted: &Submitted,
    diff: FileDiff,
) -> FileQuery {
    let query = asked(submitted).last().cloned().expect("a diff asked");
    apply(
        test,
        view,
        submitted,
        Update::FileDiff {
            query: query.clone(),
            // Keyed with its question, as the diff thread keys every answer.
            diff: Some(Box::new(
                ShownDiff::new(diff, query.options.context).keyed(&query),
            )),
        },
    );
    settle(test);
    query
}

/// The content of the primary's diff drawn, 0 while none is.
fn drawn_content(view: View) -> u64 {
    view.diff
        .peek()
        .shown_working()
        .map_or(0, ShownDiff::content)
}

fn opened() -> (TestingRunner, View, Submitted) {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(entries()));
    open_local_changes(&mut test);
    (test, view, submitted)
}

/// The centre of the diff row whose text reads `text`.
fn row(test: &TestingRunner, text: &str) -> (f64, f64) {
    let found = test.find(|node, element| {
        Paragraph::try_downcast(element)
            .filter(|paragraph| {
                paragraph
                    .spans
                    .iter()
                    .map(|span| span.text.as_ref())
                    .collect::<String>()
                    == text
            })
            .map(|_| node.layout().area.center())
    });
    let centre = found.unwrap_or_else(|| panic!("no row reads {text}"));
    (f64::from(centre.x), f64::from(centre.y))
}

/// Presses the label reading `text` furthest right — the diff's, not the lists' Stage.
fn press(test: &mut TestingRunner, text: &str) {
    let found = test
        .find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .filter(|_| node.layout().area.min_x() > SIDEBAR_WIDTH)
                .map(|_| node.layout().area.center())
        })
        .into_iter()
        .max_by(|a, b| a.x.total_cmp(&b.x));
    let centre = found.unwrap_or_else(|| panic!("no {text} drawn: {:?}", labels(test)));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    settle(test);
}

fn hover(test: &mut TestingRunner, at: (f64, f64)) {
    test.move_cursor(at);
    settle(test);
}

/// Hovers the diff row reading `text`.
fn hover_row(test: &mut TestingRunner, text: &str) {
    let at = row(test, text);
    hover(test, at);
}

/// A lines write as asked: its kind, its path, the lines' numbers (removed, added) and whether
/// it holds the mode change.
type LinesAsked = (&'static str, String, (Vec<u32>, Vec<u32>), bool);

/// The lines writes asked, oldest first.
fn lines_asked(submitted: &Submitted) -> Vec<LinesAsked> {
    let numbers = |selection: &Selection| {
        (
            selection.removed().map(LineNumber::index).collect(),
            selection.added().map(LineNumber::index).collect(),
        )
    };
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write {
                write: LocalWrite::StageLines { diff, selection },
                ..
            } => Some((
                "stage",
                diff.file.new_path.display().into_owned(),
                numbers(selection),
                selection.holds_mode(),
            )),
            Request::Write {
                write: LocalWrite::UnstageLines { diff, selection },
                ..
            } => Some((
                "unstage",
                diff.file.new_path.display().into_owned(),
                numbers(selection),
                selection.holds_mode(),
            )),
            _ => None,
        })
        .collect()
}

/// R9.1, C19: hovering the first chunk of the unstaged diff floats Stage and Discard
/// Changes…, and Stage asks the local lane to stage exactly that chunk's lines of the diff
/// drawn; over the staged diff the one action, Unstage, asks to unstage its chunk. Caught by: a
/// chunk staged whole-file, the wrong diff handed to the lane, or a discard offered staged.
#[test]
fn a_hovered_chunk_stages_and_unstages_exactly_its_lines() {
    let (mut test, view, submitted) = opened();
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    hover_row(&mut test, "EDIT 4");
    let drawn = labels(&test);
    assert!(drawn.iter().any(|l| l == STAGE_CHUNK_CAPTION), "{drawn:?}");
    assert!(
        drawn.iter().any(|l| l == DISCARD_CHUNK_CAPTION),
        "{drawn:?}"
    );
    press(&mut test, STAGE_CHUNK_CAPTION);
    assert_eq!(
        lines_asked(&submitted),
        [("stage", "a.rs".to_owned(), (vec![4], vec![4]), false)]
    );

    press_row(&mut test, "s.rs", 0);
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("s.rs", ChangeStatus::Modified, None),
    );
    hover_row(&mut test, "EDIT 30");
    let drawn = labels(&test);
    assert!(
        drawn.iter().any(|l| l == UNSTAGE_CHUNK_CAPTION),
        "{drawn:?}"
    );
    assert!(
        !drawn.iter().any(|l| l == DISCARD_CHUNK_CAPTION),
        "{drawn:?}"
    );
    press(&mut test, UNSTAGE_CHUNK_CAPTION);
    assert_eq!(
        lines_asked(&submitted)[1],
        ("unstage", "s.rs".to_owned(), (vec![30], vec![30]), false)
    );
}

/// R9.2: a drag across lines narrows the stage chord to them, and with nothing selected the
/// chord stages the whole file — hovering is not selecting (Fork, Tracker #103). Caught by: the
/// chord ignoring the selection, or a hover read as one.
#[test]
fn the_chords_act_on_the_selection_and_on_the_whole_file_without_one() {
    let (mut test, view, submitted) = opened();
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    // The removed line alone: from its row to the one above the added line's.
    let (x, removed) = row(&test, "line 4");
    test.press_cursor((x, removed));
    test.move_cursor((x, removed + 3.));
    test.move_cursor((x, removed + 5.));
    test.release_cursor((x, removed + 5.));
    settle(&mut test);
    let caption = lines_caption(GestureVerb::Stage, 1);
    assert!(labels(&test).contains(&caption), "{:?}", labels(&test));
    crate::window::tests::press_chord(&mut test, Action::StageOrUnstage);
    settle(&mut test);
    assert_eq!(
        lines_asked(&submitted),
        [("stage", "a.rs".to_owned(), (vec![4], vec![]), false)]
    );

    // Nothing selected now, the pointer over a chunk: the chord stages the file.
    hover_row(&mut test, "EDIT 30");
    crate::window::tests::press_chord(&mut test, Action::StageOrUnstage);
    settle(&mut test);
    let staged_files: Vec<Vec<RepoPath>> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write {
                write: LocalWrite::StageFiles { paths },
                ..
            } => Some(paths.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(staged_files, [vec![RepoPath::from("a.rs")]]);
    assert_eq!(lines_asked(&submitted).len(), 1);
}

/// What discarding a.rs's first chunk would lose.
fn lines_loss() -> Consequence {
    let oid = |n: u8| Oid::from_bytes(&[n; 20]).unwrap_or_else(|_| unreachable!());
    let mut selection = Selection::empty();
    selection.select_removed(LineNumber::from_index(4));
    selection.select_added(LineNumber::from_index(4));
    Consequence::DiscardLines {
        path: RepoPath::from("a.rs"),
        index: Some(oid(1)),
        working_tree: oid(2),
        on_disk: oid(2),
        executable: false,
        selection,
        mode: None,
        chunk: true,
        patch: Patch::empty(),
    }
}

/// R9.1, R8.4: a chunk's Discard Changes… asks the engine what discarding its lines would lose
/// — of exactly the diff drawn — and its answer opens the confirmation, whose button asks the
/// lines' discard with its token; every line of a new file instead asks what discarding the
/// file would lose, as the files' route does, since that discard deletes it. Caught by: a
/// discard without the engine's consequence or the dialog, a new file's lines discarded by
/// patch, or the dialog's token spent on the files' verb.
#[test]
fn a_chunks_discard_confirms_its_lines_and_a_new_files_every_line_is_its_file() {
    let (mut test, view, submitted) = opened();
    let drawn = forty_lines("a.rs", ChangeStatus::Modified, None);
    answer_with(&mut test, view, &submitted, drawn.clone());
    hover_row(&mut test, "EDIT 4");
    press(&mut test, DISCARD_CHUNK_CAPTION);
    let lines_asks: Vec<(OperationId, FileDiff, Selection, bool)> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::DiscardLinesConsequence {
                asked,
                diff,
                selection,
                chunk,
            } => Some((*asked, (**diff).clone(), selection.clone(), *chunk)),
            _ => None,
        })
        .collect();
    assert_eq!(lines_asks.len(), 1, "{:?}", submitted.borrow());
    let (asked_id, diff, selection, chunk) = lines_asks[0].clone();
    assert!(
        chunk,
        "the chunk's own Discard asked as lines: its prompt names no chunk"
    );
    assert_eq!(diff, drawn, "the discard was asked of another diff");
    assert_eq!(selection.len(), 2);
    assert!(
        view.confirming
            .peek()
            .as_ref()
            .is_some_and(Confirming::is_counting),
        "the dialog did not open, counting, at the press"
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::DiscardConsequence {
            asked: asked_id,
            outcome: Ok(lines_loss()),
        },
    );
    settle(&mut test);
    assert_eq!(
        view.confirming.peek().as_ref().map(Confirming::title),
        Some(DISCARD_TITLE)
    );
    press(&mut test, &lines_loss().action());
    let discarded: Vec<String> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write {
                write: LocalWrite::DiscardLines(confirmed),
                ..
            } => Some(confirmed.prompt().to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(discarded, [lines_loss().prompt()]);

    // The untracked file: every line of it is the file.
    press_row(&mut test, "n.rs", 0);
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("n.rs", ChangeStatus::Added, None),
    );
    hover_row(&mut test, "line 10");
    press(&mut test, DISCARD_CHUNK_CAPTION);
    let file_asks: Vec<Vec<RepoPath>> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::DiscardConsequence { paths, .. } => Some(paths.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(file_asks, [vec![RepoPath::from("n.rs")]]);
    let lines_asks = submitted
        .borrow()
        .iter()
        .filter(|request| matches!(request, Request::DiscardLinesConsequence { .. }))
        .count();
    assert_eq!(
        lines_asks, 1,
        "a new file's every line was discarded by patch"
    );
}

/// R9.4: a mode change is a row of its own whose action stages the mode alone — selected
/// through `Selection::select_mode`, no line with it. Caught by: a mode staged with the lines,
/// or no row for it.
#[test]
fn the_mode_row_stages_the_mode_alone() {
    let (mut test, view, submitted) = opened();
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines(
            "a.rs",
            ChangeStatus::Modified,
            Some((FileMode::Regular, FileMode::Executable)),
        ),
    );
    let mode_line = labels(&test)
        .into_iter()
        .find(|l| l.starts_with("old mode 100644"))
        .expect("a mode row");
    let found = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == mode_line)
                .map(|_| node.layout().area.center())
        })
        .expect("its place");
    hover(&mut test, (f64::from(found.x), f64::from(found.y)));
    press(&mut test, mode_caption(GestureVerb::Stage));
    assert_eq!(
        lines_asked(&submitted),
        [("stage", "a.rs".to_owned(), (vec![], vec![]), true)]
    );
}

/// Phase 06's QA item 4, for the gesture: while a confirmation is open, the chord heard on the
/// diff stages nothing, selection or not. Caught by: a gesture or chord acting behind a dialog.
#[test]
fn nothing_acts_on_lines_while_a_confirmation_is_open() {
    let (mut test, view, submitted) = opened();
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    let (x, y) = row(&test, "EDIT 4");
    test.press_cursor((x, y - 17.));
    test.move_cursor((x, y));
    test.release_cursor((x, y));
    settle(&mut test);
    assert!(
        view.local
            .lines
            .peek()
            .selected(drawn_content(view))
            .is_some(),
        "the drag selected nothing, so the test decides nothing"
    );
    let mut confirming = view.confirming;
    test.run_in(|| {
        confirming.set(Some(Confirming::new(
            DISCARD_TITLE,
            lines_loss(),
            |_token| {},
        )))
    });
    settle(&mut test);
    crate::local_changes_actions::on_the_diff(
        Action::StageOrUnstage,
        view,
        Some(&submitter(&submitted)),
    );
    settle(&mut test);
    assert!(lines_asked(&submitted).is_empty());
}

/// Holds `action`'s press as the window hears it, through the table, until `let_go`.
fn hold(test: &mut TestingRunner, view: View, action: Action) {
    let Some((key, code, down)) =
        cairn_ui::accelerators::chords(action, cairn_ui::accelerators::Os::current())
            .iter()
            .find_map(|chord| chord.press_hold())
    else {
        panic!("{action:?} has no press");
    };
    let mut held = view.held_keys;
    test.run_in(|| {
        held.write()
            .heard(&KeyboardEventData::new(key, code, down), true)
    });
    settle(test);
}

fn let_go(test: &mut TestingRunner, view: View) {
    let mut held = view.held_keys;
    test.run_in(|| held.set(cairn_ui::accelerators::HeldKeys::default()));
    settle(test);
}

/// Phase 02's QA item 8, the lines half: where `status.renames` lists a staged rename's source
/// as a row of its own — `old.rs`, staged as deleted — whose staged diff git pairs into the
/// rename, the gesture over that row's diff unstages its lines at the path the rename has now,
/// `new.rs`: the diff drawn is the one handed to the lane, whose patch names the new path.
/// Caught by: lines unstaged at the source's path, which the index no longer holds.
#[test]
fn a_renames_source_row_unstages_its_lines_at_the_new_path() {
    let (mut test, view, submitted) = launch();
    apply(
        &mut test,
        view,
        &submitted,
        status(vec![changed("old.rs", Some(StagedChange::Deleted), None)]),
    );
    open_local_changes(&mut test);
    let mut renamed = forty_lines("new.rs", ChangeStatus::Modified, None);
    renamed.file.status = ChangeStatus::Renamed(cairn_model::Similarity::from_percent(90));
    renamed.file.old_path = RepoPath::from("old.rs");
    answer_with(&mut test, view, &submitted, renamed);
    hover_row(&mut test, "EDIT 4");
    press(&mut test, UNSTAGE_CHUNK_CAPTION);
    assert_eq!(
        lines_asked(&submitted),
        [("unstage", "new.rs".to_owned(), (vec![4], vec![4]), false)]
    );
}

/// Drags over the diff row reading `text` and the row after it.
fn drag_over(test: &mut TestingRunner, text: &str) {
    let (x, y) = row(test, text);
    test.press_cursor((x, y));
    test.move_cursor((x, y + 8.));
    test.move_cursor((x, y + 17.));
    test.release_cursor((x, y + 17.));
    settle(test);
}

/// Whether any lines were asked of the lane: a stage, an unstage, or a discard's consequence.
fn any_lines_asked(submitted: &Submitted) -> bool {
    !lines_asked(submitted).is_empty()
        || submitted
            .borrow()
            .iter()
            .any(|request| matches!(request, Request::DiscardLinesConsequence { .. }))
}

/// Phase 08 QA item 1, R9.6: an act made over one text — the floating action built before the
/// diff was read again — is refused once a diff of other text is drawn, so its lines never
/// reach the lane against the new diff. Caught by: an act that trusts the content it was drawn
/// over.
#[test]
fn an_act_made_under_an_answer_no_longer_drawn_asks_nothing() {
    let (mut test, view, submitted) = opened();
    let query = answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    let old = drawn_content(view);
    let mut selection = Selection::empty();
    selection.select_removed(LineNumber::from_index(4));
    apply(
        &mut test,
        view,
        &submitted,
        Update::FileDiff {
            query: query.clone(),
            diff: Some(Box::new(
                ShownDiff::new(
                    forty_lines_edited("a.rs", ChangeStatus::Modified, None, [10, 30]),
                    Context::default(),
                )
                .keyed(&query),
            )),
        },
    );
    assert_ne!(drawn_content(view), old);
    for verb in [GestureVerb::Stage, GestureVerb::Discard] {
        crate::local_changes_actions::on_gesture(
            cairn_ui::GestureAct {
                verb,
                selection: selection.clone(),
                content: old,
                chunk: false,
            },
            view,
            Some(&submitter(&submitted)),
        );
    }
    settle(&mut test);
    assert!(!any_lines_asked(&submitted), "{:?}", submitted.borrow());
}

/// Phase 08 QA item 11: part of a new file's lines is not the file — a drag over two of its
/// forty lines asks what discarding those lines would lose, never the file's deletion — and,
/// lines a drag selected, never as a chunk (phase 15: the prompt names lines). Caught by: a new
/// file's every discard routed to the files' route, or a line selection worded as a chunk.
#[test]
fn part_of_a_new_files_lines_is_discarded_as_lines() {
    let (mut test, view, submitted) = opened();
    press_row(&mut test, "n.rs", 0);
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("n.rs", ChangeStatus::Added, None),
    );
    drag_over(&mut test, "line 10");
    let selected = view
        .local
        .lines
        .peek()
        .selected(drawn_content(view))
        .map(Selection::len);
    assert_eq!(selected, Some(2), "the drag did not select two lines");
    crate::window::tests::press_chord(&mut test, Action::Discard);
    settle(&mut test);
    let (lines, files) = submitted
        .borrow()
        .iter()
        .fold((0, 0), |(lines, files), r| match r {
            Request::DiscardLinesConsequence { .. } => (lines + 1, files),
            Request::DiscardConsequence { .. } => (lines, files + 1),
            _ => (lines, files),
        });
    assert_eq!((lines, files), (1, 0));
    assert!(
        submitted.borrow().iter().all(|r| match r {
            Request::DiscardLinesConsequence { chunk, .. } => !chunk,
            _ => true,
        }),
        "a drag's lines asked as the chunk"
    );
}

/// The files staged whole, oldest first.
fn files_staged(submitted: &Submitted) -> Vec<Vec<RepoPath>> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write {
                write: LocalWrite::StageFiles { paths },
                ..
            } => Some(paths.clone()),
            _ => None,
        })
        .collect()
}

/// Every paragraph's text, its spans joined: the diff's bar draws its path as one.
fn paragraphs(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| {
        Paragraph::try_downcast(element).map(|paragraph| {
            paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect::<String>()
        })
    })
}

/// C25's diff half (R8.1 as amended, R7.3 as amended; observed `lc2`-`lc4`): c.rs pressed, then
/// a.rs added with the extending press — the diff is c.rs's, the file clicked first, under its
/// own bar, and nothing is asked for a.rs or for both, nor counted; from the diff, with no lines
/// selected, Return does nothing and Ctrl+S (⌘S) stages the whole selection, the paths in the
/// lists' order; the selection then moves to b.rs, the row in the first acted row's place, and
/// its diff is asked. Caught by: the path last toggled shown, a count or a second file drawn, a
/// bare Return heard on the diff, the chord taking the primary alone, or the selection left on
/// paths the action took.
#[test]
fn several_paths_selected_show_the_primarys_diff_and_its_chord_stages_them_all() {
    let (mut test, view, submitted) = launch();
    apply(
        &mut test,
        view,
        &submitted,
        status(
            ["a.rs", "b.rs", "c.rs", "d.rs"]
                .into_iter()
                .map(|path| changed(path, None, Some(UnstagedChange::Modified)))
                .collect(),
        ),
    );
    open_local_changes(&mut test);
    press_row(&mut test, "c.rs", 0);
    let asked_for_c = asked(&submitted).len();
    hold(&mut test, view, Action::ExtendSelection);
    press_row(&mut test, "a.rs", 0);
    let_go(&mut test, view);
    let paths = view
        .local
        .state
        .peek()
        .selection()
        .map(|selection| selection.paths().to_vec());
    assert_eq!(
        paths,
        Some(vec![RepoPath::from("a.rs"), RepoPath::from("c.rs")]),
        "the extending press did not select both"
    );
    assert_eq!(
        asked(&submitted)
            .iter()
            .map(crate::local_changes_tests::path_of)
            .collect::<Vec<_>>()
            .last()
            .map(String::as_str),
        Some("c.rs"),
        "the diff is not the file clicked first's"
    );
    assert_eq!(
        asked(&submitted).len(),
        asked_for_c,
        "a path added to the selection asked a diff"
    );
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("c.rs", ChangeStatus::Modified, None),
    );
    let drawn = paragraphs(&test);
    assert!(drawn.iter().any(|p| p == "c.rs"), "c.rs's bar: {drawn:?}");
    let counted = labels(&test)
        .into_iter()
        .chain(drawn)
        .filter(|text| text.contains("selected") || text.contains("2 files"))
        .collect::<Vec<_>>();
    assert!(
        counted.is_empty(),
        "the diff counted the selection: {counted:?}"
    );

    // Focus in the diff, no lines selected: Return does nothing.
    let (x, y) = row(&test, "EDIT 30");
    test.click_cursor((x, y));
    settle(&mut test);
    assert_eq!(view.local.lines.peek().selected(drawn_content(view)), None);
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert!(
        files_staged(&submitted).is_empty() && lines_asked(&submitted).is_empty(),
        "Return in the diff staged: {:?}",
        submitted.borrow()
    );
    // Ctrl+S (⌘S) stages the whole selection.
    let before = asked(&submitted).len();
    crate::window::tests::press_chord(&mut test, Action::StageOrUnstage);
    settle(&mut test);
    assert_eq!(
        files_staged(&submitted),
        [vec![RepoPath::from("a.rs"), RepoPath::from("c.rs")]]
    );
    assert_eq!(
        view.local
            .state
            .peek()
            .selection()
            .map(|selection| selection.primary().clone()),
        Some(RepoPath::from("b.rs")),
        "the selection is not the row in the first acted row's place"
    );
    assert_eq!(
        asked(&submitted)[before..]
            .iter()
            .map(crate::local_changes_tests::path_of)
            .collect::<Vec<_>>(),
        ["b.rs"]
    );
}

/// Drags over the removed line 4 alone: from its row to just above the added line's, ending
/// `nudge` pixels across — the floating actions hide while the pointer stands where the last
/// action was pressed, and a click in a test moves no pointer.
fn select_line_4(test: &mut TestingRunner, nudge: f64) {
    let (x, removed) = row(test, "line 4");
    test.press_cursor((x, removed));
    test.move_cursor((x, removed + 3.));
    test.move_cursor((x + nudge, removed + 5.));
    test.release_cursor((x + nudge, removed + 5.));
    settle(test);
}

/// C26 through the window (R9.6, L4; the QA brief): a line selection over a.rs, then a refresh
/// — the status read again, which reads the primary again, its old diff drawn meanwhile — whose
/// re-read is the same text in every byte keeps the selection, its tint and its action, and an
/// act then stages exactly the lines selected; the same once more with a re-read that differs
/// lets the selection go, says "The file changed — line selection cleared." once in the bar,
/// and the next key takes the words away, nothing acted on. Caught by: a selection dropped by
/// every refresh (Fork's behaviour, the kept deviation), one kept over other text, the notice
/// never said, or said for good.
#[test]
fn a_line_selection_is_kept_across_a_refresh_of_the_same_text_and_cleared_once_otherwise() {
    let (mut test, view, submitted) = opened();
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    select_line_4(&mut test, 0.);
    let caption = lines_caption(GestureVerb::Stage, 1);
    assert!(labels(&test).contains(&caption), "{:?}", labels(&test));

    let before = asked(&submitted).len();
    apply(&mut test, view, &submitted, status(entries()));
    settle(&mut test);
    assert_eq!(
        asked(&submitted).len(),
        before + 1,
        "the refresh did not read a.rs again"
    );
    assert!(labels(&test).contains(&caption), "the refresh blanked it");
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    let drawn = labels(&test);
    assert!(
        drawn.contains(&caption),
        "the same text dropped it: {drawn:?}"
    );
    assert!(!drawn.iter().any(|l| l == SELECTION_CLEARED), "{drawn:?}");
    press(&mut test, &caption);
    assert_eq!(
        lines_asked(&submitted),
        [("stage", "a.rs".to_owned(), (vec![4], vec![]), false)]
    );

    select_line_4(&mut test, 7.);
    assert!(labels(&test).contains(&caption));
    apply(&mut test, view, &submitted, status(entries()));
    settle(&mut test);
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines_edited("a.rs", ChangeStatus::Modified, None, [4, 31]),
    );
    let drawn = labels(&test);
    assert!(!drawn.contains(&caption), "kept over other text: {drawn:?}");
    assert_eq!(
        drawn.iter().filter(|l| *l == SELECTION_CLEARED).count(),
        1,
        "{drawn:?}"
    );
    assert_eq!(view.local.lines.peek().selected(drawn_content(view)), None);
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    assert!(
        !labels(&test).iter().any(|l| l == SELECTION_CLEARED),
        "the words outlived the next key"
    );
    assert_eq!(lines_asked(&submitted).len(), 1, "{:?}", submitted.borrow());
}

/// Forty lines, `line {n}`, each of `edits` reading `{word} {n}` instead.
fn forty_lines_text(edits: &[u32], word: &str) -> String {
    (0..40)
        .map(|n| {
            if edits.contains(&n) {
                format!("{word} {n}\n")
            } else {
                format!("line {n}\n")
            }
        })
        .collect()
}

/// Hands the worker what the window asked and applies what it answers, until `done` holds of
/// the window and what arrived; returns what arrived. The waiting is the worker's
/// (`next_update`), never this file's.
fn pump(
    test: &mut TestingRunner,
    view: View,
    submitted: &Submitted,
    worker: (
        &crate::worker::RepositoryHandle,
        &mut crate::worker::Updates,
    ),
    done: impl Fn(&TestingRunner, &[Update]) -> bool,
) -> Vec<Update> {
    let (handle, updates) = worker;
    let mut seen = Vec::new();
    loop {
        let asked: Vec<Request> = submitted.borrow_mut().drain(..).collect();
        for request in asked {
            handle.submit(request);
        }
        if done(test, &seen) {
            return seen;
        }
        let update = crate::worker::next_update(updates);
        seen.push(update.clone());
        apply(test, view, submitted, update);
    }
}

/// Whether the primary's diff drawn has a line reading `line` on its new side.
fn drawn_reads(view: View, line: &str) -> bool {
    view.diff
        .peek()
        .shown_working()
        .and_then(ShownDiff::text)
        .is_some_and(|(text, _)| text.new_lines().iter().any(|new| new.text() == line))
}

/// Whether a working-tree diff arrived after the last status in `seen`.
fn read_again_after_a_status(seen: &[Update]) -> bool {
    seen.iter()
        .rposition(|update| matches!(update, Update::Status { .. }))
        .is_some_and(|status| {
            seen[status..]
                .iter()
                .any(|update| matches!(update, Update::FileDiff { .. }))
        })
}

/// C26 through a real refresh (merge bar, phase 21, item 12): the window over a real worker,
/// a.rs staged as forty lines and two of them edited in the working tree. A line selected in
/// its unstaged diff is kept across a `Request::Refresh` whose status and re-read diff are the
/// same text — the answer keyed by the diff thread itself — and, once a.rs is edited again and
/// refreshed, let go of, "The file changed — line selection cleared." said exactly once.
/// Caught by: a working-tree answer keyed to its query rather than its text, end to end
/// through the real diff lane (the selection then outlives the edit, or is dropped by every
/// refresh).
#[test]
fn a_line_selection_survives_a_real_refresh_of_the_same_text_and_clears_once_otherwise() {
    let repository = crate::worker::StagedFile::new("cairn-c26-window", "a.rs", &{
        forty_lines_text(&[], "line")
    });
    repository.write("a.rs", &forty_lines_text(&[4, 30], "EDIT"));
    let (handle, mut updates) = repository.open();
    let (mut test, view, submitted) = launch();
    let caption = lines_caption(GestureVerb::Stage, 1);

    handle.submit(Request::Refresh);
    pump(
        &mut test,
        view,
        &submitted,
        (&handle, &mut updates),
        |_, seen| {
            seen.iter()
                .any(|update| matches!(update, Update::Status { .. }))
        },
    );
    open_local_changes(&mut test);
    pump(
        &mut test,
        view,
        &submitted,
        (&handle, &mut updates),
        |_, _| drawn_content(view) != 0,
    );
    settle(&mut test);
    select_line_4(&mut test, 0.);
    assert!(labels(&test).contains(&caption), "{:?}", labels(&test));
    let first = drawn_content(view);

    // Refreshed with a.rs untouched: the same text, re-read through the diff lane.
    handle.submit(Request::Refresh);
    pump(
        &mut test,
        view,
        &submitted,
        (&handle, &mut updates),
        |_, seen| read_again_after_a_status(seen),
    );
    settle(&mut test);
    assert_eq!(drawn_content(view), first, "the same text drew another");
    let drawn = labels(&test);
    assert!(
        drawn.contains(&caption),
        "a refresh of the same text dropped it: {drawn:?}"
    );
    assert!(!drawn.iter().any(|l| l == SELECTION_CLEARED), "{drawn:?}");

    // Edited, then refreshed: other text, so the selection goes, said once.
    repository.write("a.rs", &forty_lines_text(&[4, 31], "EDITED"));
    handle.submit(Request::Refresh);
    pump(
        &mut test,
        view,
        &submitted,
        (&handle, &mut updates),
        |_, seen| read_again_after_a_status(seen) && drawn_reads(view, "EDITED 31"),
    );
    assert_ne!(drawn_content(view), first, "other text drew the same key");
    settle(&mut test);
    let drawn = labels(&test);
    assert!(!drawn.contains(&caption), "kept over other text: {drawn:?}");
    assert_eq!(
        drawn.iter().filter(|l| *l == SELECTION_CLEARED).count(),
        1,
        "{drawn:?}"
    );
    assert_eq!(view.local.lines.peek().selected(drawn_content(view)), None);
    drop(handle);
}
