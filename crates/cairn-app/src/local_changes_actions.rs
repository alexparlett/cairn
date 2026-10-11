//! What Local Changes' lists and diff do when a person acts on them (staging-and-commit R8):
//! the selection's presses, Fork's five routes to stage and unstage files, the discard on the
//! unstaged side through the confirmation, and what the view says of each write it asked.
//!
//! **Every route acts on the selection, wherever focus is** (rule 8, R8.2): the one
//! `cairn_ui::LocalSelection` the lists' state keeps. A chord on either list or on the diff,
//! the heading's button (greyed while the selection is in the other list), the menu (a
//! right-press outside the selection selects its row first, R7.4a), a double press (whose first
//! press made its row the selection) and a drag (whose row, unselected, becomes the selection
//! as it is dropped, as a native list's drag carries the row it began on) all take the
//! selected paths the lists drawn still show — each found by a binary search, so a refresh that
//! took a path away and a filter that hides one both leave it out — and nothing else; a chord
//! on the diff takes the lines a drag across it selected first, when it selected any. A
//! whole-file action names each path with a rename's source beside it
//! (`LocalChanges::whole_file_paths`). Stage All and Unstage All take every row the list shows,
//! gathered on the local lane from the lists drawn, never here. Every path is one `git status`
//! listed: the window never asks a write, or a discard's consequence, for a path the lists
//! drawn do not hold.
//!
//! **After an action** the selection moves by the one rule (`next_selection`, R8.3, L5), the
//! paths the action took leaving — the row that slides into the first acted row's place, else
//! the nearest above it — and the new primary's diff is asked at once
//! (`diff_actions::show_primary`); the status that shows the write then runs the same rule.
//!
//! **A discard** (R8.4) is only ever offered on the unstaged side, and never takes a submodule,
//! a conflicted path or a nested repository: a selection mixing them with paths that can be
//! discarded asks for those alone, and the confirmation says in one line what is left
//! (`cairn_ui::Left`, the redesign's D1 and the user's decision of 2026-10-10 on nested
//! repositories); a selection with nothing that can be discarded asks nothing, and the chord
//! over it does nothing, as Fork's does — the menu says why (`cairn_ui::no_discard`). The
//! confirmation opens at the press, counting (R7.4, rule 3), while the engine computes what the
//! discard would lose on the local lane, behind the writes asked before it
//! (`Request::DiscardConsequence`); its answer fills the same dialog, whose token asks the
//! discard, and a refusal the engine makes while counting turns it into "Couldn't Discard
//! Changes" with the engine's sentence and "Nothing was changed." (rule 4). No route reaches a
//! discard without the dialog.
//!
//! **The diff's gesture** (R9) acts on lines of the diff drawn, copied as the lane will apply
//! it: staged or unstaged by patch (`LocalWrite::StageLines`, `UnstageLines`), discarded through
//! the confirmation of what the engine says the lines' discard would lose
//! (`Request::DiscardLinesConsequence`) — but every line of a new file is the file, whose
//! discard deletes it, and is confirmed as the files' route confirms it (phase 03's
//! `Refusal::WholeFileOnly`).
//!
//! On the UI thread: an action asks and returns. While a confirmation or a credential prompt
//! is open, nothing here acts.

use std::rc::Rc;
use std::sync::Arc;

use cairn_model::{
    ChangeList, ChangeStatus, Consequence, FileDiff, LocalChanges, RepoPath, Selection,
    StagedAgainst,
};
use cairn_ui::accelerators::Action;
use cairn_ui::{
    GestureAct, GestureVerb, Left, LineDrag, ListIntent, LocalSelection, ShownFiles, kept,
    no_discard,
};
use freya::prelude::*;

use crate::confirming::Confirming;
use crate::diff_actions;
use crate::local_changes_state::{drawn_changes, shown_rows};
use crate::local_writes::{self, LocalWrites};
use crate::next_selection::Leaving;
use crate::window::View;
use crate::worker::{LocalWrite, OperationId, Request, UnstageTarget, WriteEnding};

/// The confirmation's title, in Title Case (the redesign's D3), where Fork for Windows writes
/// "Discard changes"; a refusal while counting is titled "Couldn't Discard Changes" from it.
pub const DISCARD_TITLE: &str = "Discard Changes";

/// How the view asks the worker: owned, since a discard's confirmation, opened at the press,
/// asks its write when it is confirmed, after the press has returned.
pub type Submit = Rc<dyn Fn(Request)>;

/// `submit`, borrowed as the actions that ask at once take it.
fn now(submit: Option<&Submit>) -> Option<&dyn Fn(Request)> {
    submit.map(|submit| submit.as_ref())
}

/// The discard the view waits on: what the engine was asked under which number, and the
/// confirmation opened for it at the press, by its serial.
#[derive(Debug, Default)]
pub struct Acting {
    asked: Option<(OperationId, u64)>,
}

impl Acting {
    /// A discard was asked under `asked`, its confirmation opened counting as `serial`: any
    /// answer for an earlier ask is dropped.
    pub fn discard_asked(&mut self, asked: OperationId, serial: u64) {
        self.asked = Some((asked, serial));
    }

    /// The confirmation an answer for `asked` fills — its serial — when `asked` is the discard
    /// asked last, taken; `None` for an earlier ask, which is dropped.
    pub fn answered(&mut self, asked: OperationId) -> Option<u64> {
        self.asked
            .take_if(|(id, _)| *id == asked)
            .map(|(_, serial)| serial)
    }

    /// Whether the engine is counting a discard's loss.
    pub fn is_reading(&self) -> bool {
        self.asked.is_some()
    }

    /// The discard asked, if one waits: let go of, so its answer is dropped.
    pub fn forget_discard(&mut self) {
        self.asked = None;
    }
}

/// What the engine answered for the discard asked under `asked` (R7.4, R8.4): the confirmation
/// opened counting at the press becomes the confirmation, or — the engine refused while
/// counting — "Couldn't Discard Changes" with the engine's one sentence (rule 4, the user's
/// decision of 2026-10-10 on phase 15's Q2). An answer for an ask superseded, or for a
/// confirmation already let go of, is dropped.
pub fn consequence_arrived(view: View, asked: OperationId, outcome: Result<Consequence, String>) {
    let mut acting = view.local.acting;
    let Some(serial) = acting.write().answered(asked) else {
        return;
    };
    let mut confirming = view.confirming;
    let mut open = confirming.write();
    let Some(waiting) = open
        .as_mut()
        .filter(|c| c.serial() == serial && c.is_counting())
    else {
        return;
    };
    match outcome {
        Ok(consequence) => waiting.arrived(consequence),
        Err(why) => waiting.refused(why),
    }
}

/// What the view says under its lists (R8.6), and whether it is a failure: the write running
/// and those queued behind it, or how the last write ended when it did not do what was asked —
/// a stale patch naming its path. `None` when there is nothing to say. A discard being counted
/// says so in its own dialog, opened at the press, never here (R7.4).
pub fn acting_line(writes: &LocalWrites) -> Option<(String, bool)> {
    let queued = writes.queued().count();
    if let Some(running) = writes.running() {
        let behind = match queued {
            0 => String::new(),
            1 => " (1 more queued)".to_owned(),
            more => format!(" ({more} more queued)"),
        };
        return Some((
            format!(
                "{}…{behind}",
                crate::status_text::capitalised(&running.what)
            ),
            false,
        ));
    }
    if let Some(first) = writes.queued().next() {
        return Some((
            format!("{} (queued)", crate::status_text::capitalised(&first.what)),
            false,
        ));
    }
    let (asked, ending) = writes.last()?;
    let what = crate::status_text::capitalised(&asked.what);
    match ending {
        WriteEnding::Done(_) => None,
        WriteEnding::Stale { message, .. } => {
            Some((format!("{what}: nothing was written. {message}"), true))
        }
        WriteEnding::Refused { message } | WriteEnding::NotRun { message } => {
            Some((format!("{what}: nothing was written. {message}"), true))
        }
        WriteEnding::Failed { message, .. } => Some((
            format!(
                "{what} failed: {}",
                crate::status_text::why_it_failed(message)
            ),
            true,
        )),
        // A cancel opens nothing and says nothing here (R4.7, rule 5); an amend waiting for its
        // confirmation is the dialog's to say.
        WriteEnding::Cancelled { .. } | WriteEnding::NeedsConfirming { .. } => None,
        WriteEnding::Incomplete { kept, message, .. } => Some((
            format!(
                "{what} did not take every file — {} kept as they were: {message}",
                kept.join(", ")
            ),
            true,
        )),
    }
}

/// Whether a dialog owns the keys and the pointer: a confirmation, a credential prompt or the
/// Git Error dialog. The window's chords are inert then (`shortcuts::act`), and so is
/// everything here.
fn dialog_open(view: View) -> bool {
    crate::shortcuts::keys_inert(view)
}

/// The rows of the selection's list it names that the lists drawn still show, sorted, with the
/// list: what every route takes. Nothing while nothing is selected.
fn selected_rows(view: View) -> Option<(ChangeList, Vec<usize>)> {
    let local = view.local.state.peek();
    let selection = local.selection()?;
    let list = selection.list();
    let lists = drawn_changes(&local);
    let shown = shown_rows(&local, list);
    let mut rows: Vec<usize> = selection
        .paths()
        .iter()
        .filter_map(|path| lists.row_of(list, path))
        .filter(|row| shown.row_of(*row).is_some())
        .collect();
    rows.sort_unstable();
    Some((list, rows))
}

/// `selection` becomes what is selected, and its primary's diff is asked; `retry` asks a
/// failed diff again.
fn make_selection(
    selection: Option<LocalSelection>,
    view: View,
    submit: Option<&dyn Fn(Request)>,
    retry: bool,
) {
    let mut local = view.local.state;
    local.write().set_selection(selection);
    diff_actions::show_primary(view, submit, retry);
}

/// The path on `row` of `list` in the lists drawn.
fn path_on(view: View, list: ChangeList, row: usize) -> Option<RepoPath> {
    let local = view.local.state.peek();
    drawn_changes(&local)
        .get(list, row)
        .map(|change| change.path.clone())
}

/// A plain press, or ↑ or ↓: the row alone is selected, primary and anchor, and its diff
/// asked — again, if it failed.
pub fn choose(list: ChangeList, row: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    if dialog_open(view) {
        return;
    }
    let Some(path) = path_on(view, list, row) else {
        return;
    };
    make_selection(Some(LocalSelection::of(list, path)), view, submit, true);
}

/// What a person did to the lists beyond choosing a row.
pub fn intent(intent: ListIntent, view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    if dialog_open(view) {
        return;
    }
    let asking = submit.as_deref();
    match intent {
        ListIntent::Toggle(list, row) => toggle(list, row, view, asking),
        ListIntent::Range(list, row) => range(list, row, view, asking),
        // The first press of a double press made its row the selection, as a plain press does;
        // the double press stages or unstages the selection.
        ListIntent::Double(list, row) => {
            choose(list, row, view, asking);
            stage_or_unstage(view, asking);
        }
        ListIntent::Act(list, action) => act(Some(list), action, view, submit.as_ref()),
        // A drag carries the selection when it began on a selected row, and otherwise the row
        // it began on, which becomes the selection as it is dropped (F26).
        ListIntent::Drop { from, path } => {
            let held = view
                .local
                .state
                .peek()
                .selection()
                .is_some_and(|selection| selection.holds(from, &path));
            if !held {
                make_selection(Some(LocalSelection::of(from, path)), view, asking, false);
            }
            stage_or_unstage(view, asking);
        }
        ListIntent::CopyPaths(_) => copy_paths(view),
    }
}

/// An action of the table heard on Local Changes' diff (R7.3): on the lines a drag across the
/// diff selected (R9.2), or, with none selected, on the selection, whole, wherever it is (rule
/// 8, R9.2 as amended) — the paths of it the lists show (`selected_rows`), as the same chord on
/// a list takes them; hovering is not selecting (Fork, Tracker #103). Return is not heard here
/// (observed `lc3`): the stage chord from the diff is ⌘S or Ctrl+S, which stages the whole
/// selection (`lc4`).
pub fn on_the_diff(action: Action, view: View, submit: Option<&Submit>) {
    if dialog_open(view) {
        return;
    }
    let content = view
        .diff
        .peek()
        .shown_working()
        .map_or(0, cairn_model::ShownDiff::content);
    let selected = view.local.lines.peek().selected(content).cloned();
    if let Some(selection) = selected {
        let list = view
            .local
            .state
            .peek()
            .selection()
            .map(LocalSelection::list);
        let verb = match (action, list) {
            (Action::StageOrUnstage, Some(ChangeList::Unstaged)) => Some(GestureVerb::Stage),
            (Action::StageOrUnstage, Some(ChangeList::Staged)) => Some(GestureVerb::Unstage),
            (Action::Discard, Some(_)) => Some(GestureVerb::Discard),
            (_, _) => None,
        };
        if let Some(verb) = verb {
            on_gesture(
                GestureAct {
                    verb,
                    selection,
                    content,
                    chunk: false,
                },
                view,
                submit,
            );
            return;
        }
    }
    act(None, action, view, submit);
}

/// What the diff's staging gesture asked (R9): `verb` on the lines it names of the diff drawn,
/// in the list of the primary whose diff it is — staged into the index from the unstaged diff,
/// unstaged out of it from the staged one, or discarded from the working tree through the
/// confirmation. The selection the gesture or the chord acted on goes.
pub fn on_gesture(act: GestureAct, view: View, submit: Option<&Submit>) {
    if dialog_open(view) {
        return;
    }
    let mut lines = view.local.lines;
    if *lines.peek() != LineDrag::default() {
        lines.set(LineDrag::default());
    }
    // The primary's diff — shared, since the lane applies exactly what was drawn — and only
    // while it is the text the act was made over: a selection names that text's lines, and
    // against other text it would name others (phase 08 QA item 1, R9.6).
    let drawn = {
        let diff = view.diff.peek();
        // The primary's list, the diff drawn being the primary's.
        let list = diff
            .working_choice()
            .map(|choice| choice.list)
            .filter(|list| {
                view.local
                    .state
                    .peek()
                    .selection()
                    .is_some_and(|selection| selection.list() == *list)
            });
        let shown = diff
            .shown_working()
            .filter(|shown| shown.content() == act.content);
        list.zip(shown.map(cairn_model::ShownDiff::shared_diff))
    };
    let Some((list, diff)) = drawn else {
        return;
    };
    act_on_lines(list, diff, act.verb, act.selection, act.chunk, view, submit);
}

/// `verb` on `selection` of `diff`, the diff of a path in `list`.
fn act_on_lines(
    list: ChangeList,
    diff: Arc<FileDiff>,
    verb: GestureVerb,
    selection: Selection,
    chunk: bool,
    view: View,
    submit: Option<&Submit>,
) {
    match (verb, list) {
        (GestureVerb::Stage, ChangeList::Unstaged) => ask(
            view,
            now(submit),
            LocalWrite::StageLines { diff, selection },
        ),
        (GestureVerb::Unstage, ChangeList::Staged) => ask(
            view,
            now(submit),
            LocalWrite::UnstageLines { diff, selection },
        ),
        (GestureVerb::Discard, _) => discard_lines(list, diff, selection, chunk, view, submit),
        // Not offered: a stage of what is staged, an unstage of what is not.
        (GestureVerb::Stage, ChangeList::Staged) | (GestureVerb::Unstage, ChangeList::Unstaged) => {
        }
    }
}

/// A discard of `selection` of `diff` (R9.1, R8.4): nothing on the staged side, a submodule
/// or a conflict — the chord doing nothing, as Fork's does; every line of a new file is the
/// file, whose discard deletes it and is confirmed as a deletion by the files' route (phase 03's
/// `WholeFileOnly`); otherwise the confirmation opens at once, counting, and what the discard
/// would lose is asked of the engine (R7.4, rule 3).
fn discard_lines(
    list: ChangeList,
    diff: Arc<FileDiff>,
    selection: Selection,
    chunk: bool,
    view: View,
    submit: Option<&Submit>,
) {
    let path = diff.file.new_path.clone();
    let refused = {
        let local = view.local.state.peek();
        no_discard(drawn_changes(&local), list, [&path])
    };
    // Nothing here can be discarded: the chord does nothing, as Fork's does (R8.4, rule 4).
    if refused.is_some() {
        return;
    }
    let Some(submit) = submit.cloned() else {
        return;
    };
    let whole_new_file = diff.file.status == ChangeStatus::Added
        && diff
            .text()
            .is_some_and(|text| selection.holds_every_change(text));
    if whole_new_file {
        ask_discard_files(vec![path], None, view, submit);
        return;
    }
    let asked = OperationId::next();
    let confirming = {
        let submit = Rc::clone(&submit);
        // The lines' confirmation asks their discard; the selection stays on the path.
        Confirming::counting(DISCARD_TITLE, move |token| {
            ask(view, Some(submit.as_ref()), LocalWrite::DiscardLines(token));
        })
    };
    open_counting(confirming, asked, view, Rc::clone(&submit));
    submit(Request::DiscardLinesConsequence {
        asked,
        diff,
        selection,
        chunk,
    });
}

/// The confirmation opened at the press, counting (R7.4, rule 3), the discard asked under
/// `asked` waiting on it; let go of unanswered, the count is ended so it stops holding the
/// local lane.
fn open_counting(confirming: Confirming, asked: OperationId, view: View, submit: Submit) {
    let mut acting = view.local.acting;
    let confirming = confirming.on_let_go(move || {
        let mut acting = view.local.acting;
        if acting.peek().is_reading() {
            acting.write().forget_discard();
            submit(Request::StopCounting);
        }
    });
    acting.write().discard_asked(asked, confirming.serial());
    let mut open = view.confirming;
    open.set(Some(confirming));
}

/// A discard of `paths`, leaving what `left` says: the confirmation opened at once, counting,
/// its token asking the discard and moving the selection on (R8.3), and what it would lose
/// asked of the engine.
fn ask_discard_files(paths: Vec<RepoPath>, left: Option<String>, view: View, submit: Submit) {
    let asked = OperationId::next();
    let confirming = {
        let submit = Rc::clone(&submit);
        let paths = paths.clone();
        Confirming::counting(DISCARD_TITLE, move |token| {
            let asking = Some(submit.as_ref());
            ask(view, asking, LocalWrite::DiscardFiles(token));
            moved_on(Leaving::of(paths.clone()), view, asking);
        })
        .leaving(left)
    };
    open_counting(confirming, asked, view, Rc::clone(&submit));
    submit(Request::DiscardConsequence { asked, paths });
}

/// `action` of the table — heard on a list, the heading, the menu or the diff — on the
/// selection, wherever focus is (rule 8). Stage All and Unstage All take `list`, the list they
/// were asked of, or the selection's when heard on the diff.
fn act(list: Option<ChangeList>, action: Action, view: View, submit: Option<&Submit>) {
    match action {
        Action::StageOrUnstage => stage_or_unstage(view, now(submit)),
        Action::StageOrUnstageAll => {
            let list = list.or_else(|| {
                view.local
                    .state
                    .peek()
                    .selection()
                    .map(LocalSelection::list)
            });
            if let Some(list) = list {
                everything(list, view, now(submit));
            }
        }
        Action::Discard => {
            if let Some((list, rows)) = selected_rows(view) {
                discard_rows(list, rows, view, submit);
            }
        }
        // No other action is heard on Local Changes' lists or diff (R7.3).
        Action::PreviousChange
        | Action::NextChange
        | Action::ToggleSideBySide
        | Action::ToggleIgnoreWhitespace
        | Action::MoreLines
        | Action::FewerLines
        | Action::EntireFile
        | Action::ExtendSelection
        | Action::SelectRange
        | Action::ExtendSelectionUp
        | Action::ExtendSelectionDown
        | Action::ShowCommitTab
        | Action::ShowChangesTab
        | Action::Refresh
        | Action::Commit
        | Action::ShowLostCommits
        | Action::NewBranch => {}
    }
}

/// Asks `write`, kept as queued.
fn ask(view: View, submit: Option<&dyn Fn(Request)>, write: LocalWrite) {
    let Some(submit) = submit else {
        return;
    };
    let mut writes = view.writes;
    local_writes::ask(&mut writes.write(), submit, write);
}

/// Stages (from Unstaged) or unstages (from Staged) the selection, whole — a conflicted path
/// by `git add`, which marks it resolved (R8.7) — and moves the selection on.
fn stage_or_unstage(view: View, submit: Option<&dyn Fn(Request)>) {
    if let Some((list, rows)) = selected_rows(view) {
        stage_or_unstage_rows(list, rows, view, submit);
    }
}

fn stage_or_unstage_rows(
    list: ChangeList,
    rows: Vec<usize>,
    view: View,
    submit: Option<&dyn Fn(Request)>,
) {
    if rows.is_empty() {
        return;
    }
    let (paths, leaving, to) = {
        let local = view.local.state.peek();
        let lists = drawn_changes(&local);
        (
            lists.whole_file_paths(list, rows.iter().copied()),
            Leaving::of(
                rows.iter()
                    .filter_map(|row| lists.get(list, *row).map(|change| change.path.clone())),
            ),
            unstage_target(lists),
        )
    };
    let write = match list {
        ChangeList::Unstaged => LocalWrite::StageFiles { paths },
        ChangeList::Staged => LocalWrite::UnstageFiles { paths, to },
    };
    ask(view, submit, write);
    moved_on(leaving, view, submit);
}

/// The selection moved on by the rule once `leaving` leaves its list (R8.3, L5), and the new
/// primary's diff asked at once.
fn moved_on(leaving: Leaving, view: View, submit: Option<&dyn Fn(Request)>) {
    let mut local = view.local.state;
    local.write().acted(&leaving);
    diff_actions::show_primary(view, submit, false);
}

/// Stage All (from Unstaged) or Unstage All (from Staged): every row of the list — or, with a
/// filter on, every row it shows and none it hides (the user's decision, 2026-10-09) — gathered
/// on the local lane from the lists drawn. Nothing while the filter's rows are on their way.
fn everything(list: ChangeList, view: View, submit: Option<&dyn Fn(Request)>) {
    let (changes, shown) = {
        let local = view.local.state.peek();
        let Some(changes) = local.drawn_shared() else {
            return;
        };
        // The filter's rows: a copy of the indices it answered, never a walk of the paths.
        let shown = match shown_rows(&local, list) {
            ShownFiles::All => None,
            ShownFiles::Filtered(rows) => Some(rows.clone()),
            ShownFiles::Waiting => return,
        };
        (changes, shown)
    };
    if shown.as_ref().map_or(changes.len(list), Vec::len) == 0 {
        return;
    }
    let write = match list {
        ChangeList::Unstaged => LocalWrite::StageAll { changes, shown },
        ChangeList::Staged => {
            let to = unstage_target(&changes);
            LocalWrite::UnstageAll { changes, shown, to }
        }
    };
    ask(view, submit, write);
    // Every row the list shows leaves it: a selection there goes, and nothing is selected
    // until the person selects again, as Fork leaves it once the last file is staged.
    moved_on(Leaving::every(list), view, submit);
}

/// What a whole-file unstage from the lists drawn puts an entry back to (R3.4, R6.3): `HEAD`'s,
/// or — while Amend is ticked and the Staged list is amend's — `HEAD`'s parent's, so the file
/// leaves the amended commit (`git reset -q <parent> --`), or none for a root commit's amend
/// (`git rm --cached -f -q --`).
pub fn unstage_target(lists: &LocalChanges) -> UnstageTarget {
    match lists.staged_against() {
        StagedAgainst::Head => UnstageTarget::Head,
        StagedAgainst::HeadParent(Some(parent)) => UnstageTarget::Commit(parent),
        StagedAgainst::HeadParent(None) => UnstageTarget::Nothing,
    }
}

/// A ⌘- or Ctrl-press: the row toggled in or out of the selection of its list — toggled in
/// with the primary left the file clicked first (observed `lc2`), toggled out with the primary
/// moved to the nearest path still selected — and the primary's diff asked.
fn toggle(list: ChangeList, row: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(path) = path_on(view, list, row) else {
        return;
    };
    let toggled = LocalSelection::toggled(view.local.state.peek().selection(), list, path);
    make_selection(toggled, view, submit, false);
}

/// A Shift-press or Shift+↑/↓: every row the list shows from the selection's anchor to `row`,
/// `row` the primary; with no anchor in the list — nothing selected there — `row` alone.
fn range(list: ChangeList, row: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    let spanned = {
        let local = view.local.state.peek();
        let lists = drawn_changes(&local);
        let shown = shown_rows(&local, list);
        let Some(path) = lists.get(list, row).map(|change| change.path.clone()) else {
            return;
        };
        let anchor = local
            .selection()
            .filter(|selection| selection.list() == list)
            .map(|selection| selection.anchor().clone());
        let ends = anchor.as_ref().and_then(|anchor| {
            let from = shown.row_of(lists.row_of(list, anchor)?)?;
            Some((from, shown.row_of(row)?))
        });
        match (anchor, ends) {
            (Some(anchor), Some((from, to))) => {
                let (low, high) = (from.min(to), from.max(to));
                LocalSelection::spanning(
                    list,
                    anchor,
                    path,
                    (low..=high).filter_map(|drawn| {
                        let at = shown.file_at(drawn)?;
                        lists.get(list, at).map(|change| change.path.clone())
                    }),
                )
            }
            (Some(_) | None, _) => LocalSelection::of(list, path),
        }
    };
    make_selection(Some(spanned), view, submit, false);
}

/// A discard of `rows` of `list` (R8.4, the redesign's D1 and the user's decision of
/// 2026-10-10 on phase 15's Q6): nothing asked — the chord doing nothing, as Fork's does — on
/// the staged side or where every row is a submodule, a conflict or a nested repository;
/// otherwise the rows that can be discarded asked, its confirmation opened at once with one
/// line saying what the rest leave.
fn discard_rows(list: ChangeList, rows: Vec<usize>, view: View, submit: Option<&Submit>) {
    if rows.is_empty() {
        return;
    }
    let (paths, left) = {
        let local = view.local.state.peek();
        let lists = drawn_changes(&local);
        let named: Vec<&RepoPath> = rows
            .iter()
            .filter_map(|row| lists.get(list, *row).map(|change| change.path))
            .collect();
        if no_discard(lists, list, named).is_some() {
            return;
        }
        let mut left = Left::default();
        let taken: Vec<usize> = rows
            .iter()
            .copied()
            .filter(|row| match lists.get(list, *row).as_ref().map(kept) {
                Some(None) => true,
                Some(Some(kind)) => {
                    left.count(kind);
                    false
                }
                None => false,
            })
            .collect();
        (lists.whole_file_paths(list, taken), left.text())
    };
    let Some(submit) = submit.cloned() else {
        return;
    };
    ask_discard_files(paths, left, view, submit);
}

/// Copy Path: the paths selected, one per line, to the clipboard.
fn copy_paths(view: View) {
    let Some((list, rows)) = selected_rows(view) else {
        return;
    };
    let text = {
        let local = view.local.state.peek();
        let lists = drawn_changes(&local);
        rows.into_iter()
            .filter_map(|row| {
                lists
                    .get(list, row)
                    .map(|change| change.path.display().into_owned())
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    if !text.is_empty() {
        // A clipboard the platform does not offer copies nothing; there is nothing to lose.
        let _ = freya::clipboard::Clipboard::set(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::Done;

    /// R7.4, R8.4: an answer is for the confirmation opened for the discard asked last, and
    /// one for an earlier ask is dropped; letting go of the ask drops its answer too. Caught by:
    /// a stale consequence filling a newer confirmation.
    #[test]
    fn only_the_discard_asked_last_is_answered() {
        let mut acting = Acting::default();
        let (first, second) = (OperationId::for_tests(1), OperationId::for_tests(2));
        acting.discard_asked(first, 11);
        acting.discard_asked(second, 12);
        assert_eq!(acting.answered(first), None);
        assert!(acting.is_reading());
        assert_eq!(acting.answered(second), Some(12));
        assert!(!acting.is_reading());
        assert_eq!(acting.answered(second), None, "answered twice");
        acting.discard_asked(first, 13);
        acting.forget_discard();
        assert_eq!(acting.answered(first), None);
    }

    /// R8.6: the line under the lists says the write running and how many are queued behind
    /// it, and a write that did not do what was asked —
    /// naming a stale patch's path — and nothing for a write that was done. Caught by: a
    /// queued write not drawn, or a stale ending drawn as success.
    #[test]
    fn the_line_says_what_runs_waits_and_failed() {
        let mut writes = LocalWrites::default();
        assert_eq!(acting_line(&writes), None);
        let stage = |path: &str| LocalWrite::StageFiles {
            paths: vec![RepoPath::from(path)],
        };
        let (one, two) = (OperationId::for_tests(1), OperationId::for_tests(2));
        writes.asked(one, &stage("a"));
        writes.asked(two, &stage("b"));
        assert_eq!(
            acting_line(&writes),
            Some(("Staging 1 file (queued)".to_owned(), false))
        );
        writes.started(one);
        assert_eq!(
            acting_line(&writes),
            Some(("Staging 1 file… (1 more queued)".to_owned(), false))
        );
        writes.ended(
            one,
            WriteEnding::Done(Done {
                description: "staged 1 file".to_owned(),
                acknowledged: None,
                locks_before: Vec::new(),
                locks_after: Vec::new(),
                replaced: None,
                cost_after: None,
            }),
        );
        writes.started(two);
        writes.ended(
            two,
            WriteEnding::Stale {
                path: "src/b.rs".to_owned(),
                message: "src/b.rs changed since it was read".to_owned(),
            },
        );
        let Some((line, failed)) = acting_line(&writes) else {
            panic!("a stale write said nothing");
        };
        assert!(failed);
        assert!(line.contains("src/b.rs"), "{line}");
    }
}
