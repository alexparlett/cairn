//! The diff's staging gesture in the window, headless (staging-and-commit R9, criterion C19's
//! window half): a hovered chunk's Stage, Unstage and Discard Changes… ask the local lane for
//! exactly its lines of the diff drawn; a drag-selection narrows them and the chords to its
//! lines, and with none the chords act on the whole file; a discard of lines confirms the
//! engine's consequence and asks the lines' discard with its token; every line of a new file is
//! discarded as the file; the mode row stages the mode alone; and nothing acts while a dialog
//! is open. Updates are applied through `session::apply`, as the worker's stream applies them.

use cairn_model::{
    ChangeStatus, ChangedFile, ChangedRange, Consequence, Context, DiffContent, DiffLine,
    DisplayOverlay, FileDiff, FileMode, LineNumber, LineSpan, Oid, Patch, RepoPath, Selection,
    ShownDiff, StagedChange, StatusEntry, TextDiff, UnstagedChange,
};
use cairn_ui::accelerators::Action;
use cairn_ui::{
    DISCARD_CHUNK_CAPTION, GestureVerb, STAGE_CHUNK_CAPTION, UNSTAGE_CHUNK_CAPTION, lines_caption,
    mode_caption,
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
            diff: Some(Box::new(ShownDiff::new(diff, query.options.context))),
        },
    );
    settle(test);
    query
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
    assert!(view.confirming.peek().is_none());
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
    let drawn = view.diff.peek().working_drawn();
    assert!(
        view.local.lines.peek().selected(drawn).is_some(),
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
        Some(&|request: Request| submitted.borrow_mut().push(request)),
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

/// The asks of paths drawn together, oldest first: each ask's number and its paths by place.
fn together_asked(submitted: &Submitted) -> Vec<(u64, Vec<(usize, String)>)> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Together(query) => Some((
                query.asked,
                query
                    .files
                    .iter()
                    .map(|file| (file.index, file.path.display().into_owned()))
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

/// R8.1 (the user's decision of 2026-10-09): with two paths selected, their diffs are asked
/// together and drawn together, each under its own row, under a bar naming how many; the
/// gesture over the second file's rows stages its lines of its own diff; and a plain press
/// back to one path lets them go and asks that path's diff alone. Caught by: the path last
/// pressed drawn alone (the interim), a gesture acting on another file's diff, or the paths
/// still drawn together once one is selected.
#[test]
fn several_paths_selected_draw_their_diffs_together() {
    let (mut test, view, submitted) = opened();
    answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    press_row(&mut test, "a.rs", 0);
    hold(&mut test, view, Action::ExtendSelection);
    press_row(&mut test, "n.rs", 0);
    let_go(&mut test, view);
    let asks = together_asked(&submitted);
    let (asked_id, files) = asks.last().cloned().expect("the paths asked together");
    assert_eq!(files, [(0, "a.rs".to_owned()), (1, "n.rs".to_owned())]);
    let shown = |diff: FileDiff| Ok(Some(Box::new(ShownDiff::new(diff, Context::default()))));
    apply(
        &mut test,
        view,
        &submitted,
        Update::Together {
            asked: asked_id,
            files: vec![
                (0, shown(forty_lines("a.rs", ChangeStatus::Modified, None))),
                (1, shown(forty_lines("n.rs", ChangeStatus::Added, None))),
            ],
            ended: Some(crate::worker::TogetherEnded::Every),
        },
    );
    settle(&mut test);
    let drawn = labels(&test);
    let headed = |path: &str| {
        test.find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == path)
                .map(|_| node.layout().area.min_x())
        })
        .into_iter()
        .filter(|x| *x > SIDEBAR_WIDTH + 300.)
        .count()
    };
    assert_eq!(headed("a.rs"), 1, "a.rs has no row of its own: {drawn:?}");
    assert_eq!(headed("n.rs"), 1, "n.rs has no row of its own: {drawn:?}");
    let bar = test.find(|_, element| {
        Paragraph::try_downcast(element).filter(|paragraph| {
            paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect::<String>()
                == "2 files"
        })
    });
    assert!(bar.is_some(), "the bar does not name two files");

    // n.rs's first rows: its own row follows a.rs's rows.
    hover_row(&mut test, "line 2");
    press(&mut test, STAGE_CHUNK_CAPTION);
    let staged = lines_asked(&submitted);
    assert_eq!(staged.len(), 1, "{staged:?}");
    assert_eq!(staged[0].0, "stage");
    assert_eq!(
        staged[0].1, "a.rs",
        "the chunk under the pointer was a.rs's first"
    );

    // One path pressed alone — another row, so no double press — nothing drawn together, its
    // diff asked alone.
    let before = asked(&submitted).len();
    let list_row = test
        .find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == "a.rs")
                .filter(|_| node.layout().area.min_x() > SIDEBAR_WIDTH)
                .map(|_| node.layout().area)
        })
        .into_iter()
        .min_by(|a, b| a.min_x().total_cmp(&b.min_x()))
        .expect("a.rs's row in the list");
    test.click_cursor((
        f64::from(list_row.min_x()) + 4.,
        f64::from(list_row.center().y),
    ));
    settle(&mut test);
    assert!(view.diff.peek().together().is_none());
    let alone = asked(&submitted);
    assert!(alone.len() > before, "the path pressed alone was not asked");
    assert_eq!(
        alone.last().map(crate::local_changes_tests::path_of),
        Some("a.rs".to_owned())
    );
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

/// Draws a.rs and n.rs together from the default entries, their first ask answered, and
/// returns that ask's number.
fn drawn_together(test: &mut TestingRunner, view: View, submitted: &Submitted) -> u64 {
    answer_with(
        test,
        view,
        submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    press_row(test, "a.rs", 0);
    hold(test, view, Action::ExtendSelection);
    press_row(test, "n.rs", 0);
    let_go(test, view);
    let (asked_id, _) = together_asked(submitted)
        .last()
        .cloned()
        .unwrap_or_else(|| panic!("the paths were not asked together"));
    answer_together(
        test,
        view,
        submitted,
        asked_id,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    asked_id
}

/// Answers the ask `asked` of a.rs and n.rs drawn together, a.rs with `first`.
fn answer_together(
    test: &mut TestingRunner,
    view: View,
    submitted: &Submitted,
    asked: u64,
    first: FileDiff,
) {
    let shown = |diff: FileDiff| Ok(Some(Box::new(ShownDiff::new(diff, Context::default()))));
    apply(
        test,
        view,
        submitted,
        Update::Together {
            asked,
            files: vec![
                (0, shown(first)),
                (1, shown(forty_lines("n.rs", ChangeStatus::Added, None))),
            ],
            ended: Some(crate::worker::TogetherEnded::Every),
        },
    );
    settle(test);
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

/// Phase 08 QA item 1 (critical): a selection made over files drawn together, then a re-read's
/// page replacing a file's diff under it — a refresh asks the same paths again, their old diffs
/// drawn meanwhile — is nothing: neither the discard chord nor the stage chord sends its rows'
/// old lines against the new diff. Caught by: a selection numbered by the ask alone, kept
/// across a page that replaced the diff it was made on.
#[test]
fn a_selection_over_files_drawn_together_is_nothing_once_a_page_replaces_its_diff() {
    let (mut test, view, submitted) = opened();
    drawn_together(&mut test, view, &submitted);
    // A refresh: the same status again asks the same paths under a new number, drawing the
    // old diffs meanwhile.
    apply(&mut test, view, &submitted, status(entries()));
    settle(&mut test);
    let (again, _) = together_asked(&submitted)
        .last()
        .cloned()
        .unwrap_or_else(|| panic!("the refresh did not ask again"));
    drag_over(&mut test, "line 4");
    let drawn = view.diff.peek().together_drawn().unwrap_or(0);
    assert!(
        view.local.lines.peek().selected(drawn).is_some(),
        "the drag selected nothing, so the test decides nothing"
    );
    // The re-read's page: a.rs edited elsewhere now.
    answer_together(
        &mut test,
        view,
        &submitted,
        again,
        forty_lines_edited("a.rs", ChangeStatus::Modified, None, [10, 30]),
    );
    crate::window::tests::press_chord(&mut test, Action::Discard);
    settle(&mut test);
    crate::window::tests::press_chord(&mut test, Action::StageOrUnstage);
    settle(&mut test);
    assert!(
        !any_lines_asked(&submitted),
        "old rows' lines were acted on against the re-read diff: {:?}",
        submitted.borrow()
    );
}

/// Phase 08 QA item 1, the single path: an act made under one answer — the floating action
/// built before the diff was read again — is refused once another answer is drawn, so its
/// lines never reach the lane against the new diff. Caught by: an act that trusts the number
/// it was drawn under.
#[test]
fn an_act_made_under_an_answer_no_longer_drawn_asks_nothing() {
    let (mut test, view, submitted) = opened();
    let query = answer_with(
        &mut test,
        view,
        &submitted,
        forty_lines("a.rs", ChangeStatus::Modified, None),
    );
    let old = view.diff.peek().working_drawn();
    let mut selection = Selection::empty();
    selection.select_removed(LineNumber::from_index(4));
    apply(
        &mut test,
        view,
        &submitted,
        Update::FileDiff {
            query,
            diff: Some(Box::new(ShownDiff::new(
                forty_lines_edited("a.rs", ChangeStatus::Modified, None, [10, 30]),
                Context::default(),
            ))),
        },
    );
    assert_ne!(view.diff.peek().working_drawn(), old);
    for verb in [GestureVerb::Stage, GestureVerb::Discard] {
        crate::local_changes_actions::on_gesture(
            cairn_ui::GestureAct {
                file: 0,
                verb,
                selection: selection.clone(),
                drawn: old,
                chunk: false,
            },
            view,
            Some(&|request: Request| submitted.borrow_mut().push(request)),
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
    let drawn = view.diff.peek().working_drawn();
    let selected = view
        .local
        .lines
        .peek()
        .selected(drawn)
        .map(|(_, selection)| selection.len());
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

/// The user's decision (2026-10-09): with no lines selected, the chords over files drawn
/// together act on the files read and drawn alone — never one the line budget left unread.
/// Caught by: a discard or a stage reaching a path the person was never shown.
#[test]
fn the_chords_over_files_drawn_together_take_only_the_files_drawn() {
    let (mut test, view, submitted) = launch();
    apply(
        &mut test,
        view,
        &submitted,
        status(vec![
            changed("a.rs", None, Some(UnstagedChange::Modified)),
            changed("b.rs", None, Some(UnstagedChange::Modified)),
            changed("c.rs", None, Some(UnstagedChange::Modified)),
        ]),
    );
    open_local_changes(&mut test);
    press_row(&mut test, "a.rs", 0);
    hold(&mut test, view, Action::SelectRange);
    press_row(&mut test, "c.rs", 0);
    let_go(&mut test, view);
    let (asked_id, files) = together_asked(&submitted)
        .last()
        .cloned()
        .unwrap_or_else(|| panic!("the paths were not asked together"));
    assert_eq!(files.len(), 3);
    let shown = |path: &str| {
        Ok(Some(Box::new(ShownDiff::new(
            forty_lines(path, ChangeStatus::Modified, None),
            Context::default(),
        ))))
    };
    // a.rs read; the budget spent before b.rs, so b.rs and c.rs are not.
    apply(
        &mut test,
        view,
        &submitted,
        Update::Together {
            asked: asked_id,
            files: vec![(0, shown("a.rs"))],
            ended: Some(crate::worker::TogetherEnded::Budget { next: 1 }),
        },
    );
    settle(&mut test);
    // Focus the diff, then the chords.
    let (x, y) = row(&test, "EDIT 4");
    test.click_cursor((x, y));
    settle(&mut test);
    crate::window::tests::press_chord(&mut test, Action::Discard);
    settle(&mut test);
    crate::window::tests::press_chord(&mut test, Action::StageOrUnstage);
    settle(&mut test);
    let asked: Vec<Vec<RepoPath>> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::DiscardConsequence { paths, .. } => Some(paths.clone()),
            Request::Write {
                write: LocalWrite::StageFiles { paths },
                ..
            } => Some(paths.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        asked,
        [vec![RepoPath::from("a.rs")], vec![RepoPath::from("a.rs")]]
    );
}
