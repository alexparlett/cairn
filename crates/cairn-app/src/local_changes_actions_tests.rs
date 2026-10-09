//! Local Changes acting on files, in the window, headless (staging-and-commit R8; criteria C18,
//! C8's view half and C24's conflicted rows): each of Fork's routes asks the local lane to
//! stage or unstage exactly the paths selected, the selection then moves to the nearest path
//! left; a discard asks the engine what it would lose and opens the confirmation with it, its
//! token asking the discard; no route discards a staged change, a submodule or a conflict, and
//! the view says why; nothing acts while a dialog is open; and what each write does is drawn
//! where it was asked. Updates are applied through `session::apply`, as the worker's stream
//! applies them, and requests read back from the window's submit.

use cairn_model::{
    ChangeList, ChangedEntry, Consequence, DiscardedFile, FileLoss, Oid, RepoPath, Similarity,
    StagedChange, StatusEntry, SubmoduleState, UnstagedChange,
};
use cairn_ui::accelerators::{self, Action, Os};
use cairn_ui::{IGNORE_WHITESPACE_LABEL, STAGE_CAPTION, STAGED_CAPTION};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{MouseEventName, PlatformEvent};

use crate::confirming::Confirming;
use crate::fetch_state::PromptView;
use crate::local_changes_actions::{self, DISCARD_TITLE, READING_DISCARD};
use crate::local_changes_tests::{
    Submitted, apply, asked, changed, conflicted, labels, launch, open_local_changes, path_of,
    press_row, status, untracked,
};
use crate::window::View;
use crate::worker::{
    Done, LocalWrite, OperationId, PromptId, ReadAgain, Request, UnstageTarget, Update, WriteEnding,
};

fn settle(test: &mut TestingRunner) {
    for _ in 0..4 {
        test.sync_and_update();
    }
}

/// Unstaged: a.rs, b.rs, c.rs (untracked), d.rs; Staged: new.rs (renamed from old.rs), s.rs,
/// u.rs.
fn several() -> Vec<StatusEntry> {
    vec![
        changed("a.rs", None, Some(UnstagedChange::Modified)),
        changed("b.rs", None, Some(UnstagedChange::Modified)),
        untracked("c.rs"),
        changed("d.rs", None, Some(UnstagedChange::Deleted)),
        changed(
            "new.rs",
            Some(StagedChange::Renamed {
                from: RepoPath::from("old.rs"),
                similarity: Similarity::from_percent(90),
            }),
            None,
        ),
        changed("s.rs", Some(StagedChange::Modified), None),
        changed("u.rs", Some(StagedChange::Added), None),
    ]
}

fn opened(entries: Vec<StatusEntry>) -> (TestingRunner, View, Submitted) {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(entries));
    open_local_changes(&mut test);
    (test, view, submitted)
}

/// The writes asked, oldest first, each as its kind and paths.
fn writes(submitted: &Submitted) -> Vec<(String, Vec<String>)> {
    let names = |paths: &[RepoPath]| paths.iter().map(|p| p.display().into_owned()).collect();
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write { write, .. } => Some(match write {
                LocalWrite::StageFiles { paths } => ("stage".to_owned(), names(paths)),
                LocalWrite::UnstageFiles { paths, to } => {
                    assert_eq!(*to, UnstageTarget::Head);
                    ("unstage".to_owned(), names(paths))
                }
                LocalWrite::StageAll { changes } => (
                    "stage all".to_owned(),
                    vec![changes.len(ChangeList::Unstaged).to_string()],
                ),
                LocalWrite::UnstageAll { changes, .. } => (
                    "unstage all".to_owned(),
                    vec![changes.len(ChangeList::Staged).to_string()],
                ),
                LocalWrite::DiscardFiles(confirmed) => {
                    ("discard".to_owned(), vec![confirmed.prompt().to_owned()])
                }
                other => (format!("{other:?}"), Vec::new()),
            }),
            _ => None,
        })
        .collect()
}

fn write(kind: &str, paths: &[&str]) -> (String, Vec<String>) {
    (
        kind.to_owned(),
        paths.iter().map(|path| (*path).to_owned()).collect(),
    )
}

/// The discards' consequences asked, oldest first: their ids and paths.
fn consequences_asked(submitted: &Submitted) -> Vec<(OperationId, Vec<String>)> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::DiscardConsequence { asked, paths } => Some((
                *asked,
                paths.iter().map(|p| p.display().into_owned()).collect(),
            )),
            _ => None,
        })
        .collect()
}

/// Holds `action`'s press as the window hears it, through the table, until `let_go`.
fn hold(test: &mut TestingRunner, view: View, action: Action) {
    let Some((key, code, down)) = accelerators::chords(action, Os::current())
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

/// Presses `action`'s first chord where focus is, through the window tests' own helper.
fn press_chord(test: &mut TestingRunner, action: Action) {
    crate::window::tests::press_chord(test, action);
    settle(test);
}

fn press_key(test: &mut TestingRunner, key: NamedKey) {
    test.press_key(Key::Named(key));
    settle(test);
}

/// Where the label reading `text` right of the sidebar is drawn, the `nth` from the top.
fn at(test: &TestingRunner, text: &str, nth: usize) -> (f64, f64) {
    let mut found: Vec<(f32, f32)> = test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .filter(|_| node.layout().area.min_x() > crate::sidebar_state::SIDEBAR_WIDTH)
            .map(|_| {
                let area = node.layout().area;
                (area.center().y, area.min_x())
            })
    });
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (y, x) = *found
        .get(nth)
        .unwrap_or_else(|| panic!("no label {nth} reads {text}: {:?}", labels(test)));
    (f64::from(x) + 4., f64::from(y))
}

fn click(test: &mut TestingRunner, text: &str) {
    let at = at(test, text, 0);
    test.click_cursor(at);
    settle(test);
}

fn right_click(test: &mut TestingRunner, text: &str) {
    let at = at(test, text, 0);
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

/// The path selected now, as the view keeps it.
fn selected(view: View) -> Vec<String> {
    view.local
        .selection
        .peek()
        .paths()
        .iter()
        .map(|path| path.display().into_owned())
        .collect()
}

fn chosen(view: View) -> Option<String> {
    view.diff
        .peek()
        .working_choice()
        .map(|choice| choice.path.display().into_owned())
}

/// Selects `rows` of one list: the first pressed plainly, each other with the extending press.
fn choose_rows(test: &mut TestingRunner, view: View, rows: &[&str]) {
    let Some((first, rest)) = rows.split_first() else {
        return;
    };
    press_row(test, first, 0);
    hold(test, view, Action::ExtendSelection);
    for row in rest {
        press_row(test, row, 0);
    }
    let_go(test, view);
}

/// C18, R8.1-R8.3: a selection of three made by the extending press is staged by the stage
/// chord on the focused list, as one write naming exactly those paths, and the selection then
/// moves to the path that takes the first one's place, its diff asked; a range made by the
/// range press is staged by Unstaged's Stage button. Caught by: the chord acting on the path
/// chosen alone, a path the selection does not hold staged, or the selection left on paths
/// that are leaving.
#[test]
fn the_chord_and_the_button_stage_a_selection_and_the_selection_moves_on() {
    let (mut test, view, submitted) = opened(several());
    choose_rows(&mut test, view, &["a.rs", "b.rs", "c.rs"]);
    assert_eq!(selected(view), ["a.rs", "b.rs", "c.rs"]);
    press_chord(&mut test, Action::StageOrUnstage);
    assert_eq!(
        writes(&submitted),
        [write("stage", &["a.rs", "b.rs", "c.rs"])]
    );
    assert_eq!(selected(view), ["d.rs"], "R8.3: the nearest path left");
    assert_eq!(chosen(view).as_deref(), Some("d.rs"));
    assert_eq!(
        asked(&submitted).last().map(path_of).as_deref(),
        Some("d.rs"),
        "the diff of the path moved to was not asked"
    );

    press_row(&mut test, "b.rs", 0);
    hold(&mut test, view, Action::SelectRange);
    press_row(&mut test, "d.rs", 0);
    let_go(&mut test, view);
    assert_eq!(selected(view), ["b.rs", "c.rs", "d.rs"]);
    click(&mut test, STAGE_CAPTION);
    assert_eq!(
        writes(&submitted)[1..],
        [write("stage", &["b.rs", "c.rs", "d.rs"])]
    );
    assert_eq!(
        selected(view),
        ["a.rs"],
        "with nothing left below, the nearest path above"
    );
}

/// C18, R8.2: a selection dragged from Unstaged and dropped on Staged is staged whole; the
/// menu's Stage stages the selection its row is in; a double press stages its row. Caught by:
/// a drop that stages the row it began on alone, a menu acting on the row pressed rather than
/// the selection it is in, or a double press staging nothing.
#[test]
fn a_drag_the_menu_and_a_double_press_stage_what_is_selected() {
    let (mut test, view, submitted) = opened(several());
    choose_rows(&mut test, view, &["a.rs", "c.rs"]);
    let from = at(&test, "a.rs", 0);
    let staged = at(&test, STAGED_CAPTION, 0);
    let into = (staged.0 + 40., staged.1 + 40.);
    test.press_cursor(from);
    test.move_cursor((from.0, from.1 + 10.));
    test.sync_and_update();
    test.move_cursor(into);
    test.sync_and_update();
    test.release_cursor(into);
    settle(&mut test);
    assert_eq!(writes(&submitted), [write("stage", &["a.rs", "c.rs"])]);

    choose_rows(&mut test, view, &["b.rs", "d.rs"]);
    right_click(&mut test, "d.rs");
    let stage = at(&test, STAGE_CAPTION, 1);
    test.click_cursor(stage);
    settle(&mut test);
    assert_eq!(writes(&submitted)[1..], [write("stage", &["b.rs", "d.rs"])]);

    let row = at(&test, "c.rs", 0);
    test.click_cursor(row);
    test.click_cursor(row);
    settle(&mut test);
    assert_eq!(writes(&submitted)[2..], [write("stage", &["c.rs"])]);
}

/// C18, R3.4, R8.2: a staged selection unstages as `git reset` takes it — a rename's source
/// beside it, so the rename moves out whole — by the chord; Unstage All by the menu takes the
/// whole list, gathered on the local lane from the lists drawn; and the chevron's Stage All the
/// whole of Unstaged. Caught by: a rename unstaged without its source, or an All that names the
/// rows drawn on the UI thread.
#[test]
fn a_staged_selection_unstages_with_its_renames_source_and_all_takes_the_list() {
    let (mut test, view, submitted) = opened(several());
    press_row(&mut test, "old.rs → new.rs", 0);
    hold(&mut test, view, Action::ExtendSelection);
    press_row(&mut test, "u.rs", 0);
    let_go(&mut test, view);
    press_chord(&mut test, Action::StageOrUnstage);
    assert_eq!(
        writes(&submitted),
        [write("unstage", &["new.rs", "old.rs", "u.rs"])]
    );
    assert_eq!(selected(view), ["s.rs"]);

    right_click(&mut test, "s.rs");
    click(&mut test, cairn_ui::UNSTAGE_ALL_CAPTION);
    let (x, y) = at(&test, STAGE_CAPTION, 0);
    test.click_cursor((x - 20., y));
    settle(&mut test);
    assert_eq!(
        writes(&submitted)[1..],
        [write("unstage all", &["3"]), write("stage all", &["4"])]
    );
}

/// What a discard of a.rs (3 lines) and c.rs (12 bytes) would lose.
fn mixed_loss() -> Consequence {
    let oid = |n: u8| Oid::from_bytes(&[n; 20]).unwrap_or_else(|_| unreachable!());
    Consequence::DiscardFiles {
        files: vec![
            DiscardedFile {
                path: RepoPath::from("a.rs"),
                loss: FileLoss::Modified {
                    index: oid(1),
                    working_tree: Some(oid(2)),
                    executable: false,
                    lines: Some(3),
                    mode: None,
                },
            },
            DiscardedFile {
                path: RepoPath::from("c.rs"),
                loss: FileLoss::Untracked {
                    working_tree: oid(3),
                    executable: false,
                    bytes: 12,
                },
            },
        ],
    }
}

/// C18, R8.4, R1.2: the discard chord on a selection asks the engine what it would lose —
/// nothing discarded, no dialog yet, the view saying it is counting — and its answer opens the
/// confirmation drawn from it; confirming asks the discard with the token built from exactly
/// that consequence, and the selection moves on. An answer for an earlier ask is dropped.
/// Caught by: a dialog opened without the engine's consequence, a discard asked without the
/// dialog, a stale answer confirmed, or a token naming other words.
#[test]
fn a_discard_asks_what_it_would_lose_and_confirms_exactly_that() {
    let (mut test, view, submitted) = opened(several());
    choose_rows(&mut test, view, &["a.rs", "c.rs"]);
    press_chord(&mut test, Action::Discard);
    let consequences = consequences_asked(&submitted);
    assert_eq!(consequences.len(), 1);
    let (asked_id, paths) = consequences[0].clone();
    assert_eq!(paths, ["a.rs", "c.rs"]);
    assert!(
        view.confirming.peek().is_none(),
        "a dialog before the engine answered"
    );
    assert!(
        labels(&test).iter().any(|t| t == READING_DISCARD),
        "{:?}",
        labels(&test)
    );
    // An answer for another ask is not confirmed.
    apply(
        &mut test,
        view,
        &submitted,
        Update::DiscardConsequence {
            asked: OperationId::for_tests(9_999),
            outcome: Ok(mixed_loss()),
        },
    );
    assert!(
        view.confirming.peek().is_none(),
        "a stale answer opened a dialog"
    );

    apply(
        &mut test,
        view,
        &submitted,
        Update::DiscardConsequence {
            asked: asked_id,
            outcome: Ok(mixed_loss()),
        },
    );
    settle(&mut test);
    assert_eq!(
        view.confirming.peek().as_ref().map(Confirming::title),
        Some(DISCARD_TITLE)
    );
    let prompt = mixed_loss().prompt();
    assert!(labels(&test).contains(&prompt), "{:?}", labels(&test));
    assert!(
        writes(&submitted).is_empty(),
        "discarded before the dialog was answered"
    );
    click(&mut test, &mixed_loss().action());
    assert_eq!(writes(&submitted), [write("discard", &[&prompt])]);
    assert!(view.confirming.peek().is_none());
    assert_eq!(selected(view), ["b.rs"]);
}

/// Unstaged: a.rs, clash.rs (conflicted), sub (a submodule); Staged: s.rs.
fn with_a_submodule_and_a_conflict() -> Vec<StatusEntry> {
    vec![
        changed("a.rs", None, Some(UnstagedChange::Modified)),
        conflicted("clash.rs"),
        StatusEntry::Changed(ChangedEntry {
            path: RepoPath::from("sub"),
            staged: None,
            unstaged: Some(UnstagedChange::Modified),
            submodule: Some(SubmoduleState {
                new_commits: true,
                modified_content: true,
                untracked_content: false,
            }),
        }),
        changed("s.rs", Some(StagedChange::Modified), None),
    ]
}

/// C8 (views), C24, R3.6, R8.4, R8.7, R8.8: no route discards a staged change, a submodule or a
/// conflicted path — the chord asks nothing and the view says why; a conflicted row stages
/// whole, by `git add`; and a refusal the engine makes before any dialog (a nested repository)
/// is said, nothing discarded. Caught by: a discard's consequence asked for any of them, a
/// refusal kept silent, or a conflict that cannot be staged.
#[test]
fn no_discard_reaches_a_staged_change_a_submodule_or_a_conflict_and_each_says_why() {
    let (mut test, view, submitted) = opened(with_a_submodule_and_a_conflict());
    for (row, why) in [
        ("sub", "is a submodule"),
        ("clash.rs", "has a conflict"),
        ("s.rs", "Staged changes can't be discarded"),
    ] {
        press_row(&mut test, row, 0);
        for key in [NamedKey::Backspace, NamedKey::Delete] {
            press_key(&mut test, key);
        }
        press_chord(&mut test, Action::Discard);
        assert!(
            labels(&test).iter().any(|t| t.contains(why)),
            "{row}: {:?}",
            labels(&test)
        );
    }
    // A selection holding one among discardable paths refuses whole.
    choose_rows(&mut test, view, &["a.rs", "sub"]);
    press_chord(&mut test, Action::Discard);
    assert_eq!(consequences_asked(&submitted), [], "a discard was asked");
    assert!(view.confirming.peek().is_none());

    // The conflicted row stages whole — `git add`, which marks it resolved.
    press_row(&mut test, "clash.rs", 0);
    press_key(&mut test, NamedKey::Enter);
    assert_eq!(writes(&submitted), [write("stage", &["clash.rs"])]);

    // The engine's own refusal, before any dialog.
    press_row(&mut test, "a.rs", 0);
    press_chord(&mut test, Action::Discard);
    let Some((asked_id, _)) = consequences_asked(&submitted).pop() else {
        panic!("a.rs's discard was not asked");
    };
    apply(
        &mut test,
        view,
        &submitted,
        Update::DiscardConsequence {
            asked: asked_id,
            outcome: Err("a.rs is a repository nested in the working tree".to_owned()),
        },
    );
    assert!(view.confirming.peek().is_none());
    assert!(
        labels(&test)
            .iter()
            .any(|t| t.contains("nested in the working tree")),
        "{:?}",
        labels(&test)
    );
}

/// Phase 06's QA item 4, R7.4: with a confirmation open, Enter, Backspace and Delete pressed in
/// Local Changes stage and discard nothing — the dialog holds the keys, and the view acts on
/// nothing a list reports meanwhile. Caught by: the lists' chords heard behind the modal.
#[test]
fn local_changes_acts_on_nothing_while_a_confirmation_is_open() {
    let (mut test, view, submitted) = opened(several());
    press_row(&mut test, "a.rs", 0);
    let mut confirming = view.confirming;
    confirming.set(Some(Confirming::new(DISCARD_TITLE, mixed_loss(), |_| {})));
    settle(&mut test);
    for key in [NamedKey::Backspace, NamedKey::Delete, NamedKey::Enter] {
        press_key(&mut test, key);
    }
    // And whatever a list reports while one is open does nothing.
    confirming.set(Some(Confirming::new(DISCARD_TITLE, mixed_loss(), |_| {})));
    settle(&mut test);
    test.run_in(|| {
        for action in [
            Action::StageOrUnstage,
            Action::Discard,
            Action::StageOrUnstageAll,
        ] {
            local_changes_actions::intent(
                cairn_ui::ListIntent::Act(ChangeList::Unstaged, action),
                view,
                None,
            );
        }
    });
    assert_eq!(writes(&submitted), []);
    assert_eq!(consequences_asked(&submitted), []);
}

/// Phase 06's QA item 21: Backspace and Delete typed into the filter field while Local Changes
/// is shown edit the filter and discard nothing. Caught by: a bare key the field let bubble to
/// the list's chords.
#[test]
fn backspace_in_the_filter_edits_the_filter_and_discards_nothing() {
    let (mut test, view, submitted) = opened(several());
    press_row(&mut test, "a.rs", 0);
    // The filter field sits over Unstaged's heading.
    let heading = at(&test, cairn_ui::UNSTAGED_CAPTION, 0);
    test.click_cursor((
        heading.0 + 40.,
        heading.1 - f64::from(cairn_ui::LIST_HEADER_HEIGHT),
    ));
    settle(&mut test);
    test.write_text("ab");
    settle(&mut test);
    press_key(&mut test, NamedKey::Backspace);
    press_key(&mut test, NamedKey::Delete);
    press_key(&mut test, NamedKey::Enter);
    assert_eq!(*view.local.filter_text.peek(), "a");
    assert_eq!(consequences_asked(&submitted), []);
    assert_eq!(writes(&submitted), []);
}

/// Phase 06's QA item 5: a credential prompt that arrives while a confirmation is open is drawn
/// and can be answered — the confirmation set aside, kept unanswered — and the confirmation is
/// drawn again once the prompt is gone. Caught by: a prompt behind the modal no key can reach,
/// or a confirmation lost to the prompt.
#[test]
fn a_credential_prompt_sets_an_open_confirmation_aside_until_it_is_answered() {
    let (mut test, view, _) = opened(several());
    let mut confirming = view.confirming;
    confirming.set(Some(Confirming::new(DISCARD_TITLE, mixed_loss(), |_| {})));
    settle(&mut test);
    let action = mixed_loss().action();
    assert!(labels(&test).contains(&action));
    let mut prompt = view.prompt;
    prompt.set(Some(PromptView {
        id: PromptId::for_tests(4),
        text: "Enter passphrase for key '/k': ".to_owned(),
        asking: Some("Staging 1 file".to_owned()),
    }));
    settle(&mut test);
    assert!(
        labels(&test)
            .iter()
            .any(|t| t == "Staging 1 file is asking for a credential"),
        "{:?}",
        labels(&test)
    );
    assert!(
        !labels(&test).contains(&action),
        "the confirmation over the prompt"
    );
    assert!(
        view.confirming.peek().is_some(),
        "the confirmation was lost"
    );
    prompt.set(None);
    settle(&mut test);
    assert!(
        labels(&test).contains(&action),
        "the confirmation did not come back"
    );
}

/// C18, R8.5, L6: Local Changes' diff is always the exact one — asked with whitespace never
/// ignored though the shared setting ignores it, its bar's toggle disabled — and the setting is
/// left as it was. Caught by: the working-tree query asked at the shared setting, or a toggle
/// that changes it from here.
#[test]
fn local_changes_diff_is_always_exact_and_leaves_the_setting_alone() {
    let (mut test, view, submitted) = launch();
    let mut settings = view.diff_settings;
    test.run_in(|| settings.write().toggle_ignore_whitespace());
    apply(&mut test, view, &submitted, status(several()));
    open_local_changes(&mut test);
    let first = asked(&submitted);
    assert_eq!(first.len(), 1);
    assert!(
        !first[0].options.ignore_whitespace,
        "{:?}",
        first[0].options
    );
    crate::local_changes_tests::answer(&mut test, view, &submitted, &first[0], "LINE");
    let toggle = test
        .find(|node, element| {
            Rect::try_downcast(element)
                .filter(|rect| rect.accessibility.builder.value() == Some(IGNORE_WHITESPACE_LABEL))
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no ignore-whitespace toggle in Local Changes' bar"));
    test.click_cursor((f64::from(toggle.x), f64::from(toggle.y)));
    settle(&mut test);
    assert!(
        view.diff_settings.peek().ignore_whitespace(),
        "the shared setting moved"
    );
    assert_eq!(asked(&submitted).len(), 1, "a disabled toggle asked again");
}

/// R8.6, R3.7: what the writes asked from the view are doing is drawn under its lists — queued,
/// running with how many wait behind it — and a stale patch's ending names its path; one that
/// was done says nothing more. Caught by: a queued or running write not drawn, or a stale
/// ending drawn as nothing.
#[test]
fn a_write_is_drawn_queued_running_and_stale_where_it_was_asked() {
    let (mut test, view, submitted) = opened(several());
    choose_rows(&mut test, view, &["a.rs", "b.rs"]);
    press_chord(&mut test, Action::StageOrUnstage);
    assert!(
        labels(&test)
            .iter()
            .any(|t| t == "Staging 2 files (queued)"),
        "{:?}",
        labels(&test)
    );
    let id = submitted
        .borrow()
        .iter()
        .find_map(|request| match request {
            Request::Write { id, .. } => Some(*id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no write asked"));
    apply(&mut test, view, &submitted, Update::WriteStarted { id });
    assert!(
        labels(&test).iter().any(|t| t == "Staging 2 files…"),
        "{:?}",
        labels(&test)
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::Stale {
                path: "b.rs".to_owned(),
                message: "b.rs changed since it was read".to_owned(),
            },
            read_again: ReadAgain::Status,
        },
    );
    assert!(
        labels(&test)
            .iter()
            .any(|t| t.starts_with("Staging 2 files: nothing was written") && t.contains("b.rs")),
        "{:?}",
        labels(&test)
    );
    // The next write, done, leaves nothing said.
    press_row(&mut test, "c.rs", 0);
    press_chord(&mut test, Action::StageOrUnstage);
    let id = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write { id, .. } => Some(*id),
            _ => None,
        })
        .next_back()
        .unwrap_or_else(|| panic!("no second write"));
    apply(&mut test, view, &submitted, Update::WriteStarted { id });
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::Done(Done {
                description: "staged 1 file".to_owned(),
                acknowledged: None,
                locks_before: Vec::new(),
                locks_after: Vec::new(),
            }),
            read_again: ReadAgain::Status,
        },
    );
    assert!(
        !labels(&test).iter().any(|t| t.starts_with("Staging")),
        "{:?}",
        labels(&test)
    );
}

/// Phase 03's QA item 4, decided here: the engine counts any path absent from the index as
/// untracked, so the window must ask only for paths `git status` listed — a selection naming a
/// path a refresh took away (or never listed) asks for the paths the lists drawn still list,
/// and nothing when none is. Caught by: a selection's paths handed to the engine unchecked.
#[test]
fn a_discard_names_only_paths_the_lists_drawn_still_list() {
    let (mut test, view, submitted) = opened(several());
    press_row(&mut test, "a.rs", 0);
    let mut selection = view.local.selection;
    let spanning = |paths: &[&str]| {
        cairn_ui::ListSelection::spanning(
            ChangeList::Unstaged,
            RepoPath::from(paths[0]),
            paths.iter().map(|path| RepoPath::from(*path)),
        )
    };
    test.run_in(|| selection.set(spanning(&["a.rs", "gone.rs", "ignored.txt"])));
    settle(&mut test);
    press_key(&mut test, NamedKey::Backspace);
    assert_eq!(
        consequences_asked(&submitted)
            .into_iter()
            .map(|(_, paths)| paths)
            .collect::<Vec<_>>(),
        [vec!["a.rs".to_owned()]]
    );
}

/// R8.1: the extending press toggles a path out as well as in — the selection made of the
/// path chosen when none is made yet — and the path whose diff is shown, toggled out, gives
/// way to another the selection holds, so the diff never shows a path the selection left.
/// Caught by: a toggle that only adds, or a diff left on a path toggled out.
#[test]
fn the_extending_press_toggles_out_and_the_diff_follows_the_selection() {
    let (mut test, view, _) = opened(several());
    assert_eq!(
        chosen(view).as_deref(),
        Some("a.rs"),
        "the view's first path"
    );
    hold(&mut test, view, Action::ExtendSelection);
    press_row(&mut test, "c.rs", 0);
    assert_eq!(selected(view), ["a.rs", "c.rs"]);
    assert_eq!(chosen(view).as_deref(), Some("c.rs"));
    press_row(&mut test, "c.rs", 0);
    let_go(&mut test, view);
    assert_eq!(selected(view), ["a.rs"]);
    assert_eq!(chosen(view).as_deref(), Some("a.rs"));
}

/// C24, R8.7, L25: a conflicted row stages whole — `git add`, which marks it resolved — by
/// every route the person has: its double press, a drag into Staged, Unstaged's Stage button
/// and its menu's Stage, as by the chord. Caught by: a route that leaves a conflicted row out,
/// or takes it by another verb.
#[test]
fn a_conflicted_row_stages_whole_by_every_route() {
    let (mut test, _, submitted) = opened(with_a_submodule_and_a_conflict());
    let row = at(&test, "clash.rs", 0);
    test.click_cursor(row);
    test.click_cursor(row);
    settle(&mut test);
    let staged = at(&test, STAGED_CAPTION, 0);
    let into = (staged.0 + 40., staged.1 + 40.);
    test.press_cursor(row);
    test.move_cursor((row.0, row.1 + 10.));
    test.sync_and_update();
    test.move_cursor(into);
    test.sync_and_update();
    test.release_cursor(into);
    settle(&mut test);
    press_row(&mut test, "clash.rs", 0);
    click(&mut test, STAGE_CAPTION);
    right_click(&mut test, "clash.rs");
    let stage = at(&test, STAGE_CAPTION, 1);
    test.click_cursor(stage);
    settle(&mut test);
    assert_eq!(writes(&submitted), vec![write("stage", &["clash.rs"]); 4]);
    assert_eq!(consequences_asked(&submitted), []);
}

/// C18, R8.2: a staged selection unstages by every route but the double press (which takes its
/// row): dragged from Staged and dropped on Unstaged, Staged's Unstage button, and the menu's
/// Unstage — each one write naming exactly the selection. Caught by: a route that unstages the
/// row it began on alone, or stages instead.
#[test]
fn a_staged_selection_unstages_by_the_drag_the_button_and_the_menu() {
    let (mut test, view, submitted) = opened(several());
    choose_rows(&mut test, view, &["s.rs", "u.rs"]);
    let from = at(&test, "s.rs", 0);
    let into = at(&test, "a.rs", 0);
    test.press_cursor(from);
    test.move_cursor((from.0, from.1 - 10.));
    test.sync_and_update();
    test.move_cursor(into);
    test.sync_and_update();
    test.release_cursor(into);
    settle(&mut test);
    choose_rows(&mut test, view, &["s.rs", "u.rs"]);
    click(&mut test, cairn_ui::UNSTAGE_CAPTION);
    choose_rows(&mut test, view, &["s.rs", "u.rs"]);
    right_click(&mut test, "u.rs");
    let unstage = at(&test, cairn_ui::UNSTAGE_CAPTION, 1);
    test.click_cursor(unstage);
    settle(&mut test);
    assert_eq!(
        writes(&submitted),
        vec![write("unstage", &["s.rs", "u.rs"]); 3]
    );
    let row = at(&test, "s.rs", 0);
    test.click_cursor(row);
    test.click_cursor(row);
    settle(&mut test);
    assert_eq!(writes(&submitted)[3..], [write("unstage", &["s.rs"])]);
}
