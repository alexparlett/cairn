//! What Local Changes' lists and diff do when a person acts on them (staging-and-commit R8):
//! the multi-selection, Fork's five routes to stage and unstage files, the discard on the
//! unstaged side through the confirmation, and what the view says of each write it asked.
//!
//! **Which paths an action takes.** A list's action takes the paths selected in that list
//! (`cairn_ui::ListSelection`) that the lists drawn still list — each found by a binary search,
//! so a refresh that took a path away leaves it out — or, with nothing selected there, the path
//! chosen; a whole-file action names each with a rename's source beside it
//! (`LocalChanges::whole_file_paths`). Stage All and Unstage All take every row of the list,
//! gathered on the local lane from the lists drawn, never here. A drop takes the selection the
//! drag began in, or the path it began on alone; a double press, its row; a chord heard on the
//! diff, the lines a drag across it selected, or with none the path the diff shows — or the
//! paths drawn together. Every path is one `git status` listed: the window never asks
//! a write, or a discard's consequence, for a path the lists drawn do not hold.
//!
//! **After a stage or an unstage** the selection moves to the nearest path left in the list it
//! left, and its diff is asked (R8.3, `cairn_ui::nearest_remaining`): the row that slides into
//! the first acted row's place, else the nearest above it.
//!
//! **A discard** (R8.4) is only ever offered on the unstaged side, and never takes a submodule or
//! a conflicted path: a selection mixing them with paths that can be discarded asks for those
//! alone, and the confirmation says in one line what is left (`cairn_ui::left_as_they_are`, the
//! redesign's D1); a selection with nothing that can be discarded asks nothing, and the chord
//! over it does nothing, as Fork's does — the menu says why (`cairn_ui::no_discard`). What it would lose is
//! computed by the engine on the local lane, behind the writes asked before it
//! (`Request::DiscardConsequence`); the answer opens the confirmation (`Confirming`), whose
//! token asks the discard. No route reaches a discard without the dialog. The engine refuses a
//! nested repository, and anything else it cannot count, before any dialog, and the view says
//! why.
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
    ChangeKind, ChangeList, ChangeStatus, Consequence, FileDiff, LocalChanges, RepoPath, Selection,
    StagedAgainst,
};
use cairn_ui::accelerators::Action;
use cairn_ui::{
    GestureAct, GestureVerb, LineDrag, ListIntent, ListSelection, ShownFiles, discards,
    left_as_they_are, nearest_remaining, no_discard,
};
use freya::prelude::*;

use crate::confirming::Confirming;
use crate::diff_actions;
use crate::local_changes_state::{LocalChangesState, drawn_changes, shown_rows};
use crate::local_writes::{self, LocalWrites};
use crate::window::View;
use crate::worker::{LocalWrite, OperationId, Request, UnstageTarget, WriteEnding};

/// The confirmation's title, in Title Case (the redesign's D3), where Fork for Windows writes
/// "Discard changes".
pub const DISCARD_TITLE: &str = "Discard Changes";
/// Said under the lists while the engine counts what a discard would lose.
pub const READING_DISCARD: &str = "Reading what the discard would lose…";

/// What a discard asked of the engine takes: files — rows of the Unstaged list, an untracked
/// one deleted — or the lines of one diff the gesture selected (R9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discarding {
    /// The paths, and the line saying what the selection held that the discard leaves.
    Files(Vec<RepoPath>, Option<String>),
    Lines,
}

/// What the view's actions wait on, and what it says of them.
#[derive(Debug, Default)]
pub struct Acting {
    /// The discard asked of the local lane, newest, and what it takes.
    asked: Option<(OperationId, Discarding)>,
    /// What it would lose, arrived and waiting for the view to open the confirmation.
    arrived: Option<(Consequence, Discarding)>,
    /// Why the last action asked nothing, said under the lists until the next action.
    said: Option<String>,
}

impl Acting {
    /// A discard of `paths` was asked under `asked`, leaving what `left` says: any answer for
    /// an earlier one is dropped.
    pub fn discard_asked(
        &mut self,
        asked: OperationId,
        paths: Vec<RepoPath>,
        left: Option<String>,
    ) {
        self.asked = Some((asked, Discarding::Files(paths, left)));
        self.arrived = None;
        self.said = None;
    }

    /// A discard of a diff's lines was asked under `asked`: any answer for an earlier one is
    /// dropped.
    pub fn lines_discard_asked(&mut self, asked: OperationId) {
        self.asked = Some((asked, Discarding::Lines));
        self.arrived = None;
        self.said = None;
    }

    /// What discarding the paths asked under `asked` would lose: kept to be confirmed when it
    /// answers the discard asked last, said when the engine refused. `false` when it was not
    /// the one asked last, and is dropped.
    pub fn consequence_arrived(
        &mut self,
        asked: OperationId,
        outcome: Result<Consequence, String>,
    ) -> bool {
        let Some((_, discarding)) = self.asked.take_if(|(id, _)| *id == asked) else {
            return false;
        };
        match outcome {
            Ok(consequence) => self.arrived = Some((consequence, discarding)),
            Err(why) => self.said = Some(format!("Nothing was discarded: {why}")),
        }
        true
    }

    /// The consequence waiting to be confirmed, taken.
    pub fn take_arrived(&mut self) -> Option<(Consequence, Discarding)> {
        self.arrived.take()
    }

    pub fn has_arrived(&self) -> bool {
        self.arrived.is_some()
    }

    /// Whether the engine is counting a discard's loss.
    pub fn is_reading(&self) -> bool {
        self.asked.is_some()
    }

    /// The discard asked, if one waits: let go of, so its answer is dropped.
    pub fn forget_discard(&mut self) {
        self.asked = None;
        self.arrived = None;
    }

    fn quiet(&mut self) {
        self.said = None;
    }

    pub fn said(&self) -> Option<&str> {
        self.said.as_deref()
    }
}

/// What the view says under its lists (R8.6), and whether it is a failure: why the last action
/// asked nothing, a discard being counted, the write running and those queued behind it, or how
/// the last write ended when it did not do what was asked — a stale patch naming its path.
/// `None` when there is nothing to say.
pub fn acting_line(acting: &Acting, writes: &LocalWrites) -> Option<(String, bool)> {
    // Why the last action asked nothing is about the last thing the person did: said first.
    if let Some(said) = acting.said() {
        return Some((said.to_owned(), true));
    }
    if acting.is_reading() {
        return Some((READING_DISCARD.to_owned(), false));
    }
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

/// The rows of `list` the selection names that `lists` still lists, in the lists' order of
/// their search; or, with no selection made in `list`, the path chosen there. A selection a
/// toggle emptied takes nothing: nothing is drawn selected.
fn acted_rows(view: View, lists: &LocalChanges, list: ChangeList) -> Vec<usize> {
    let selection = view.local.selection.peek();
    let mut rows: Vec<usize> = if selection.list() == Some(list) {
        selection
            .paths()
            .iter()
            .filter_map(|path| lists.row_of(list, path))
            .collect()
    } else {
        Vec::new()
    };
    if selection.list() != Some(list) {
        let diff = view.diff.peek();
        rows.extend(
            diff.working_choice()
                .filter(|choice| choice.list == list)
                .and_then(|choice| lists.row_of(list, &choice.path)),
        );
    }
    rows.sort_unstable();
    rows
}

/// A plain press, or ↑ or ↓: the row alone is selected, and its diff asked.
pub fn choose(list: ChangeList, row: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    if dialog_open(view) {
        return;
    }
    let path = {
        let local = view.local.state.peek();
        drawn_changes(&local)
            .get(list, row)
            .map(|change| change.path.clone())
    };
    let Some(path) = path else {
        return;
    };
    let mut selection = view.local.selection;
    selection.set(ListSelection::of(list, path));
    quiet(view);
    diff_actions::choose_working(list, row, view, submit);
}

fn quiet(view: View) {
    let mut acting = view.local.acting;
    if acting.peek().said().is_some() {
        acting.write().quiet();
    }
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
        ListIntent::Double(list, row) => {
            quiet(view);
            stage_or_unstage(list, Acted::Rows(vec![row]), view, asking);
        }
        ListIntent::Act(list, action) => act(list, Acted::Selection, action, view, asking),
        ListIntent::Drop { from, path } => {
            quiet(view);
            let dragged = {
                let selection = view.local.selection.peek();
                if selection.holds(from, &path) {
                    Acted::Selection
                } else {
                    Acted::Path(path)
                }
            };
            stage_or_unstage(from, dragged, view, asking);
        }
        ListIntent::CopyPaths(list) => copy_paths(list, view),
    }
}

/// An action of the table heard on Local Changes' diff (R7.3): on the lines a drag across the
/// diff selected (R9.2), or, with none selected, on the path the diff shows, whole, in the list
/// it was chosen from — hovering is not selecting (Fork, Tracker #103).
pub fn on_the_diff(action: Action, view: View, submit: Option<&dyn Fn(Request)>) {
    if dialog_open(view) {
        return;
    }
    let (drawn, together) = {
        let diff = view.diff.peek();
        match diff.together() {
            Some((list, _)) => (diff.together_drawn().unwrap_or(0), Some(list)),
            None => (diff.working_drawn(), None),
        }
    };
    let selected = view
        .local
        .lines
        .peek()
        .selected(drawn)
        .map(|(file, selection)| (file, selection.clone()));
    if let Some((file, selection)) = selected {
        let list = together.or_else(|| view.diff.peek().working_choice().map(|choice| choice.list));
        let verb = match (action, list) {
            (Action::StageOrUnstage, Some(ChangeList::Unstaged)) => Some(GestureVerb::Stage),
            (Action::StageOrUnstage, Some(ChangeList::Staged)) => Some(GestureVerb::Unstage),
            (Action::Discard, Some(_)) => Some(GestureVerb::Discard),
            (_, _) => None,
        };
        if let Some(verb) = verb {
            on_gesture(
                GestureAct {
                    file,
                    verb,
                    selection,
                    drawn,
                    chunk: false,
                },
                view,
                submit,
            );
            return;
        }
    }
    // The paths drawn together, whole — those read and drawn, never one still being read or
    // one the line budget left unread (the user's decision, 2026-10-09).
    if let Some(list) = together {
        let paths = view.diff.peek().together_read_paths();
        act(list, Acted::Paths(paths), action, view, submit);
        return;
    }
    let Some((list, path)) = view
        .diff
        .peek()
        .working_choice()
        .map(|choice| (choice.list, choice.path.clone()))
    else {
        return;
    };
    act(list, Acted::Path(path), action, view, submit);
}

/// What the diff's staging gesture asked (R9): `verb` on the lines it names of the diff drawn,
/// in the list the diff was chosen from — staged into the index from the unstaged diff,
/// unstaged out of it from the staged one, or discarded from the working tree through the
/// confirmation. The selection the gesture or the chord acted on goes.
pub fn on_gesture(act: GestureAct, view: View, submit: Option<&dyn Fn(Request)>) {
    if dialog_open(view) {
        return;
    }
    let mut lines = view.local.lines;
    if *lines.peek() != LineDrag::default() {
        lines.set(LineDrag::default());
    }
    quiet(view);
    // The diff the act was over: one of the paths drawn together, by its place, or the path
    // chosen — shared, since the lane applies exactly what was drawn — and only while it is
    // still the answer the act was made under: a selection names that answer's lines, and
    // against a re-read's diff it would name others (phase 08 QA item 1).
    let drawn = {
        let diff = view.diff.peek();
        match diff.together() {
            Some((list, _)) => diff
                .together_diff(act.file)
                .filter(|_| diff.together_drawn() == Some(act.drawn))
                .map(|shown| (list, shown.shared_diff())),
            None => {
                let list = diff.working_choice().map(|choice| choice.list);
                let shown = diff
                    .shown_working()
                    .filter(|_| act.file == 0 && diff.working_drawn() == act.drawn);
                list.zip(shown.map(cairn_model::ShownDiff::shared_diff))
            }
        }
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
    submit: Option<&dyn Fn(Request)>,
) {
    match (verb, list) {
        (GestureVerb::Stage, ChangeList::Unstaged) => {
            ask(view, submit, LocalWrite::StageLines { diff, selection })
        }
        (GestureVerb::Unstage, ChangeList::Staged) => {
            ask(view, submit, LocalWrite::UnstageLines { diff, selection })
        }
        (GestureVerb::Discard, _) => discard_lines(list, diff, selection, chunk, view, submit),
        // Not offered: a stage of what is staged, an unstage of what is not.
        (GestureVerb::Stage, ChangeList::Staged) | (GestureVerb::Unstage, ChangeList::Unstaged) => {
        }
    }
}

/// A discard of `selection` of `diff` (R9.1, R8.4): refused in the view for the staged side, a
/// submodule or a conflict; every line of a new file is the file, whose discard deletes it and
/// is confirmed as a deletion by the files' route (phase 03's `WholeFileOnly`); otherwise what
/// it would lose asked of the engine, whose answer opens the confirmation.
fn discard_lines(
    list: ChangeList,
    diff: Arc<FileDiff>,
    selection: Selection,
    chunk: bool,
    view: View,
    submit: Option<&dyn Fn(Request)>,
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
    let Some(submit) = submit else {
        return;
    };
    let asked = OperationId::next();
    let mut acting = view.local.acting;
    let whole_new_file = diff.file.status == ChangeStatus::Added
        && diff
            .text()
            .is_some_and(|text| selection.holds_every_change(text));
    if whole_new_file {
        acting
            .write()
            .discard_asked(asked, vec![path.clone()], None);
        submit(Request::DiscardConsequence {
            asked,
            paths: vec![path],
        });
    } else {
        acting.write().lines_discard_asked(asked);
        submit(Request::DiscardLinesConsequence {
            asked,
            diff,
            selection,
            chunk,
        });
    }
}

/// Which rows an action takes.
enum Acted {
    /// The selection in the list, or the path chosen there.
    Selection,
    /// These rows of the list.
    Rows(Vec<usize>),
    /// This path alone, if the list lists it.
    Path(RepoPath),
    /// These paths, those the list lists.
    Paths(Vec<RepoPath>),
}

impl Acted {
    fn rows(self, view: View, lists: &LocalChanges, list: ChangeList) -> Vec<usize> {
        match self {
            Self::Selection => acted_rows(view, lists, list),
            Self::Rows(rows) => rows,
            Self::Path(path) => lists.row_of(list, &path).into_iter().collect(),
            Self::Paths(paths) => {
                let mut rows: Vec<usize> = paths
                    .iter()
                    .filter_map(|path| lists.row_of(list, path))
                    .collect();
                rows.sort_unstable();
                rows
            }
        }
    }
}

fn act(
    list: ChangeList,
    acted: Acted,
    action: Action,
    view: View,
    submit: Option<&dyn Fn(Request)>,
) {
    quiet(view);
    match action {
        Action::StageOrUnstage => stage_or_unstage(list, acted, view, submit),
        Action::StageOrUnstageAll => everything(list, view, submit),
        Action::Discard => discard(list, acted, view, submit),
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

/// Stages (from Unstaged) or unstages (from Staged) the rows `acted` names, whole — a
/// conflicted path by `git add`, which marks it resolved (R8.7) — and moves the selection on.
fn stage_or_unstage(list: ChangeList, acted: Acted, view: View, submit: Option<&dyn Fn(Request)>) {
    let (paths, next, to) = {
        let local = view.local.state.peek();
        let lists = drawn_changes(&local);
        let rows = acted.rows(view, lists, list);
        if rows.is_empty() {
            return;
        }
        (
            lists.whole_file_paths(list, rows.iter().copied()),
            next_after(&local, list, &rows),
            unstage_target(lists),
        )
    };
    let write = match list {
        ChangeList::Unstaged => LocalWrite::StageFiles { paths },
        ChangeList::Staged => LocalWrite::UnstageFiles { paths, to },
    };
    ask(view, submit, write);
    move_to(list, next, view, submit);
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
    let mut selection = view.local.selection;
    selection.set(ListSelection::default());
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

/// The row of `list` the selection moves to once `acted` — rows of the lists drawn, sorted —
/// leave it (R8.3): the nearest left in the order the list shows, through the filter.
fn next_after(local: &LocalChangesState, list: ChangeList, acted: &[usize]) -> Option<usize> {
    let lists = drawn_changes(local);
    let shown: &ShownFiles = shown_rows(local, list);
    let len = shown.len(lists.len(list));
    let first = acted
        .iter()
        .filter_map(|row| shown.row_of(*row))
        .min()
        .unwrap_or(0);
    let drawn = nearest_remaining(len, first, |drawn| {
        shown
            .file_at(drawn)
            .is_some_and(|row| acted.binary_search(&row).is_ok())
    })?;
    shown.file_at(drawn)
}

/// The selection moved to `next` of `list` and its diff asked — or, with nothing left there,
/// let go of, as Fork leaves the diff blank once the last file is staged.
fn move_to(list: ChangeList, next: Option<usize>, view: View, submit: Option<&dyn Fn(Request)>) {
    match next {
        Some(row) => {
            let path = {
                let local = view.local.state.peek();
                drawn_changes(&local)
                    .get(list, row)
                    .map(|change| change.path.clone())
            };
            if let Some(path) = path {
                let mut selection = view.local.selection;
                selection.set(ListSelection::of(list, path));
                diff_actions::choose_working(list, row, view, submit);
            }
        }
        None => {
            let mut selection = view.local.selection;
            selection.set(ListSelection::default());
        }
    }
}

/// A ⌘- or Ctrl-press: the row toggled in or out of the selection of its list — a selection
/// made of the path chosen when none is made yet — and, toggled in, its diff asked.
fn toggle(list: ChangeList, row: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    quiet(view);
    let (path, chosen) = {
        let local = view.local.state.peek();
        let Some(path) = drawn_changes(&local)
            .get(list, row)
            .map(|change| change.path.clone())
        else {
            return;
        };
        let chosen = view
            .diff
            .peek()
            .working_choice()
            .filter(|choice| choice.list == list)
            .map(|choice| choice.path.clone());
        (path, chosen)
    };
    let mut selection = view.local.selection;
    let base = {
        let held = selection.peek();
        match (held.list() == Some(list), chosen) {
            (false, Some(chosen)) => ListSelection::of(list, chosen),
            (true, _) | (false, None) => held.clone(),
        }
    };
    let toggled = base.toggled(list, path.clone());
    let added = toggled.holds(list, &path);
    // Toggled in, its diff is shown; the path shown toggled out, another the selection holds.
    let shown = if added {
        Some(row)
    } else if chosen_is(view, list, &path) {
        let local = view.local.state.peek();
        toggled
            .paths()
            .first()
            .and_then(|other| drawn_changes(&local).row_of(list, other))
    } else {
        None
    };
    selection.set(toggled);
    if let Some(row) = shown {
        diff_actions::choose_working(list, row, view, submit);
    }
}

/// Whether the path the diff shows is `path`, chosen from `list`.
fn chosen_is(view: View, list: ChangeList, path: &RepoPath) -> bool {
    view.diff
        .peek()
        .working_choice()
        .is_some_and(|choice| choice.list == list && choice.path == *path)
}

/// A Shift-press or Shift+↑/↓: every row the list shows from the selection's anchor — or the
/// path chosen — to `row`, and `row`'s diff asked.
fn range(list: ChangeList, row: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    quiet(view);
    let spanned = {
        let local = view.local.state.peek();
        let lists = drawn_changes(&local);
        let shown = shown_rows(&local, list);
        let Some(path) = lists.get(list, row).map(|change| change.path.clone()) else {
            return;
        };
        let selection = view.local.selection.peek();
        let anchor = selection
            .anchor()
            .filter(|_| selection.list() == Some(list))
            .cloned()
            .or_else(|| {
                view.diff
                    .peek()
                    .working_choice()
                    .filter(|choice| choice.list == list)
                    .map(|choice| choice.path.clone())
            });
        let ends = anchor.as_ref().and_then(|anchor| {
            let from = shown.row_of(lists.row_of(list, anchor)?)?;
            Some((from, shown.row_of(row)?))
        });
        match (anchor, ends) {
            (Some(anchor), Some((from, to))) => {
                let (low, high) = (from.min(to), from.max(to));
                ListSelection::spanning(
                    list,
                    anchor,
                    (low..=high).filter_map(|drawn| {
                        let at = shown.file_at(drawn)?;
                        lists.get(list, at).map(|change| change.path.clone())
                    }),
                )
            }
            (Some(_) | None, _) => ListSelection::of(list, path),
        }
    };
    let mut selection = view.local.selection;
    selection.set(spanned);
    diff_actions::choose_working(list, row, view, submit);
}

/// A discard of the rows `acted` names (R8.4, the redesign's D1): nothing asked — the chord
/// doing nothing, as Fork's does — on the staged side or where every row is a submodule or a
/// conflict; otherwise what discarding the rows that can be discarded would lose asked of the
/// engine, whose answer opens the confirmation, with one line saying what the rest leave.
fn discard(list: ChangeList, acted: Acted, view: View, submit: Option<&dyn Fn(Request)>) {
    let (paths, left) = {
        let local = view.local.state.peek();
        let lists = drawn_changes(&local);
        let rows = acted.rows(view, lists, list);
        if rows.is_empty() {
            return;
        }
        let named: Vec<&RepoPath> = rows
            .iter()
            .filter_map(|row| lists.get(list, *row).map(|change| change.path))
            .collect();
        if no_discard(lists, list, named).is_some() {
            return;
        }
        let (mut submodules, mut conflicted) = (0, 0);
        let taken: Vec<usize> = rows
            .iter()
            .copied()
            .filter(
                |row| match lists.get(list, *row).map(|change| change.kind) {
                    Some(kind) if discards(kind) => true,
                    Some(ChangeKind::Submodule) => {
                        submodules += 1;
                        false
                    }
                    Some(_) => {
                        conflicted += 1;
                        false
                    }
                    None => false,
                },
            )
            .collect();
        (
            lists.whole_file_paths(list, taken),
            left_as_they_are(submodules, conflicted),
        )
    };
    let Some(submit) = submit else {
        return;
    };
    let asked = OperationId::next();
    let mut acting = view.local.acting;
    acting.write().discard_asked(asked, paths.clone(), left);
    submit(Request::DiscardConsequence { asked, paths });
}

/// The consequence that arrived, put in front of the person: the confirmation opened, its
/// token asking the discard and moving the selection on (R8.3). Run by the view while it is
/// shown.
pub fn confirm_arrived(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    let mut acting = view.local.acting;
    let Some((consequence, discarding)) = acting.write().take_arrived() else {
        return;
    };
    let mut confirming = view.confirming;
    let (paths, left) = match discarding {
        Discarding::Files(paths, left) => (paths, left),
        // The lines' confirmation asks their discard; the selection stays on the path.
        Discarding::Lines => {
            confirming.set(Some(Confirming::new(
                DISCARD_TITLE,
                consequence,
                move |token| ask(view, submit.as_deref(), LocalWrite::DiscardLines(token)),
            )));
            return;
        }
    };
    confirming.set(Some(
        Confirming::new(DISCARD_TITLE, consequence, move |token| {
            let next = {
                let local = view.local.state.peek();
                let lists = drawn_changes(&local);
                let mut rows: Vec<usize> = paths
                    .iter()
                    .filter_map(|path| lists.row_of(ChangeList::Unstaged, path))
                    .collect();
                rows.sort_unstable();
                next_after(&local, ChangeList::Unstaged, &rows)
            };
            ask(view, submit.as_deref(), LocalWrite::DiscardFiles(token));
            move_to(ChangeList::Unstaged, next, view, submit.as_deref());
        })
        .leaving(left),
    ));
}

/// Copy Path: the paths the selection names in `list`, one per line, to the clipboard.
fn copy_paths(list: ChangeList, view: View) {
    let text = {
        let local = view.local.state.peek();
        let lists = drawn_changes(&local);
        acted_rows(view, lists, list)
            .into_iter()
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

    /// R8.4: an answer for the discard asked last is kept to confirm, a refusal is said, and an
    /// answer for an earlier ask is dropped. Caught by: a stale consequence confirmed, or a
    /// refusal kept silent.
    #[test]
    fn only_the_discard_asked_last_is_confirmed_and_a_refusal_is_said() {
        let mut acting = Acting::default();
        let (first, second) = (OperationId::for_tests(1), OperationId::for_tests(2));
        acting.discard_asked(first, vec![RepoPath::from("a")], None);
        acting.discard_asked(second, vec![RepoPath::from("b")], None);
        assert!(!acting.consequence_arrived(first, Err("x".to_owned())));
        assert!(acting.is_reading());
        assert!(acting.consequence_arrived(second, Err("sub is a repository".to_owned())));
        assert!(!acting.is_reading());
        assert_eq!(
            acting.said(),
            Some("Nothing was discarded: sub is a repository")
        );
        assert!(!acting.has_arrived());
    }

    /// R8.6: the line under the lists says the write running and how many are queued behind
    /// it, a discard being counted, a refusal, and a write that did not do what was asked —
    /// naming a stale patch's path — and nothing for a write that was done. Caught by: a
    /// queued write not drawn, or a stale ending drawn as success.
    #[test]
    fn the_line_says_what_runs_waits_and_failed() {
        let acting = Acting::default();
        let mut writes = LocalWrites::default();
        assert_eq!(acting_line(&acting, &writes), None);
        let stage = |path: &str| LocalWrite::StageFiles {
            paths: vec![RepoPath::from(path)],
        };
        let (one, two) = (OperationId::for_tests(1), OperationId::for_tests(2));
        writes.asked(one, &stage("a"));
        writes.asked(two, &stage("b"));
        assert_eq!(
            acting_line(&acting, &writes),
            Some(("Staging 1 file (queued)".to_owned(), false))
        );
        writes.started(one);
        assert_eq!(
            acting_line(&acting, &writes),
            Some(("Staging 1 file… (1 more queued)".to_owned(), false))
        );
        writes.ended(
            one,
            WriteEnding::Done(Done {
                description: "staged 1 file".to_owned(),
                acknowledged: None,
                locks_before: Vec::new(),
                locks_after: Vec::new(),
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
        let Some((line, failed)) = acting_line(&acting, &writes) else {
            panic!("a stale write said nothing");
        };
        assert!(failed);
        assert!(line.contains("src/b.rs"), "{line}");
    }
}
