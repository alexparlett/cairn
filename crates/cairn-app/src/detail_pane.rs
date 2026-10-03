//! The detail pane under the commit list (PRD R5.1, R5.2): its strip of tabs, and the tab
//! shown. A component of its own, so an answer arriving redraws the pane and not the window.
//!
//! What it draws is decided against the selection, not against whatever the diff state
//! holds: the Commit tab draws a change set only when it is the answer for the row
//! selected now (R4.4), so a fast click never shows one commit's files under another, and
//! the Changes tab draws a file's diff only when it is the answer for the file and the
//! settings selected now.
//!
//! The Changes tab shows the diff of the file chosen in the Commit tab (phase 06): the bar
//! over it and its unified rows. Its own file list and summary are phase 07's; a file's
//! diff opening in place under its row in the Commit tab is phase 08's.

use std::rc::Rc;

use cairn_model::{ChangeStatus, ChangedFile, DiffContent, Oid, RowId};
use cairn_ui::accelerators::{self, Scope};
use cairn_ui::{
    CommitTab, DetailTab, DetailTabs, DiffHeader, ShownDiff, UnifiedDiffView, reveal_row,
};
use freya::prelude::*;

use crate::diff_state::{Answer, answered_changes, answered_file};
use crate::window::View;
use crate::worker::{FileTarget, Request};
use crate::{diff_actions, selection, shortcuts};

/// Said in the pane while no row is selected.
pub const NOTHING_SELECTED: &str = "Select a commit to see its details.";
/// Said while the selected commit's answer is on its way.
pub const READING: &str = "Reading the commit…";
/// Said in the Changes tab while no file is chosen.
pub const NO_FILE_CHOSEN: &str = "Choose a file in the Commit tab to see its diff.";
/// Said while the chosen file's diff is on its way.
pub const READING_DIFF: &str = "Reading the diff…";
/// Said for a file whose diff has no change to show and hides none: a working-tree path git
/// shows nothing for, or a file whose content did not change.
pub const NO_CHANGES_SHOWN: &str = "No changes to show.";
/// Said for a file whose every change ignoring whitespace hides (R6.7).
pub const ONLY_WHITESPACE_CHANGED: &str =
    "Every change to this file is whitespace, which is being ignored.";
/// Said in the Commit tab over a comparison of two commits, which it does not describe
/// (R7.3).
pub const NOT_ONE_COMMIT: &str = "The Commit tab describes one commit.";

pub struct DetailPane {
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
}

impl DetailPane {
    pub fn new(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Self {
        Self { view, submit }
    }
}

// By the handles it reads: the submitter is the same repository's however often it is built.
impl PartialEq for DetailPane {
    fn eq(&self, other: &Self) -> bool {
        let (one, two) = (&self.view, &other.view);
        one.selected == two.selected
            && one.diff == two.diff
            && one.rows == two.rows
            && one.detail_tab == two.detail_tab
            && one.pane_collapsed == two.pane_collapsed
            && one.history_scroll == two.history_scroll
            && one.diff_settings == two.diff_settings
            && one.diff_scroll == two.diff_scroll
            && one.change_cursor == two.change_cursor
            && self.submit.is_some() == other.submit.is_some()
    }
}

impl Component for DetailPane {
    fn render(&self) -> impl IntoElement {
        let view = self.view;
        let View {
            mut detail_tab,
            mut pane_collapsed,
            ..
        } = view;
        let tab = *detail_tab.read();
        let collapsed = *pane_collapsed.read();
        let hearing = self.submit.clone();

        let strip = DetailTabs::new(tab)
            .collapsed(collapsed)
            .on_tab(move |chosen: DetailTab| {
                detail_tab.set(chosen);
                // A tab pressed on a collapsed pane is asked to be seen.
                pane_collapsed.set(false);
            })
            .on_collapse(move |collapse: bool| pane_collapsed.set(collapse));

        rect()
            .width(Size::fill())
            .height(Size::fill())
            // The pane's own chords (previous and next change), heard only from inside it: a
            // key press reaches this from whatever in the pane has focus, and from nowhere else.
            .on_key_down(move |e: Event<KeyboardEventData>| {
                if let Some(action) = accelerators::resolve_key(&e, Scope::Detail) {
                    e.stop_propagation();
                    shortcuts::act(action, view, hearing.as_deref());
                }
            })
            .child(strip)
            .maybe_child((!collapsed).then(|| match tab {
                DetailTab::Commit => commit_body(view, self.submit.clone()),
                DetailTab::Changes => changes_body(view, self.submit.clone()),
            }))
    }
}

/// The Commit tab's body: the selected commit's answer, and only that.
fn commit_body(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Element {
    let Some(id) = *view.selected.read() else {
        return notice(NOTHING_SELECTED, false);
    };
    let of = selection::comparison_of(id);
    let diff = view.diff.read();
    let Some((_, answer)) = diff.changes().filter(|(asked, _)| *asked == of) else {
        // Whatever is kept is another selection's: it is not drawn.
        return notice(READING, false);
    };
    match answer {
        Answer::Waiting => notice(READING, false),
        Answer::Failed(message) => notice(message, true),
        Answer::Ready(changes) if changes.details.is_none() => notice(NOT_ONE_COMMIT, false),
        Answer::Ready(_) => {
            let changes = view.diff.into_readable().map(answered_changes, |_| true);
            let choosing = submit.clone();
            let mut detail_tab = view.detail_tab;
            CommitTab::new(changes)
                .on_parent(move |parent: Oid| follow_parent(parent, view, submit.as_deref()))
                .on_file(move |index: usize| {
                    diff_actions::choose_file(index, view, choosing.as_deref());
                })
                // A press shows the file's diff, in the Changes tab until phase 08 opens it
                // in place; an arrow only makes it current.
                .on_file_pressed(move |_| detail_tab.set(DetailTab::Changes))
                .into()
        }
    }
}

/// The file a diff is of, as the bar names it: the change set's own row for a committed
/// file, its path for a working-tree one.
fn file_of(target: &FileTarget) -> ChangedFile {
    match target {
        FileTarget::Committed { file, .. } => file.clone(),
        FileTarget::WorkingTree { path, .. } => ChangedFile {
            status: ChangeStatus::Modified,
            old_path: path.clone(),
            new_path: path.clone(),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        },
    }
}

/// The Changes tab's body: the chosen file's bar and its diff, for the answer naming that
/// file at the settings as they are, and only that.
fn changes_body(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Element {
    let state = view.diff.read();
    let Some((query, answer)) = state.file() else {
        return notice(NO_FILE_CHOSEN, false);
    };
    let settings = *view.diff_settings.read();
    let hiding = state.shown_file().is_some_and(ShownDiff::hides_changes);
    let header = DiffHeader::new(file_of(&query.target), settings)
        .hiding(hiding)
        .on_action(move |pressed| {
            shortcuts::act(shortcuts::of_header(pressed), view, submit.as_deref());
        });
    let body = match answer {
        Answer::Waiting => notice(READING_DIFF, false),
        Answer::Failed(message) => notice(message.clone(), true),
        Answer::Ready(None) => notice(NO_CHANGES_SHOWN, false),
        Answer::Ready(Some(shown)) => diff_body(shown, view),
    };
    rect()
        .expanded()
        .content(Content::Flex)
        .child(header)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .child(body),
        )
        .into()
}

/// One file's diff: its rows, or what stands in their place. Every state is named; the
/// non-text ones are said in a line here, and drawn in full in phase 07 (R6.8).
fn diff_body(shown: &ShownDiff, view: View) -> Element {
    match &shown.diff().content {
        DiffContent::Text { .. } if shown.row_count() == 0 => notice(
            if shown.hides_changes() {
                ONLY_WHITESPACE_CHANGED
            } else {
                NO_CHANGES_SHOWN
            },
            false,
        ),
        DiffContent::Text { .. } => {
            let current = view.change_cursor.read().and_then(|cursor| {
                shown
                    .layout()
                    .and_then(|layout| layout.change_rows(cursor.change))
            });
            UnifiedDiffView::new(
                view.diff.into_readable().map(answered_file, |_| true),
                view.diff_scroll,
            )
            .current(current)
            .into()
        }
        DiffContent::Binary { old_size, new_size } => notice(
            format!("Binary file: {old_size} bytes before, {new_size} after."),
            false,
        ),
        DiffContent::TooLarge { .. } => notice("Changes are too large to display.", false),
        DiffContent::LfsPointer { .. } => notice("A Git LFS pointer.", false),
        DiffContent::Submodule { .. } => notice("A submodule.", false),
        DiffContent::ModeChangeOnly => notice("Only the file's mode changed.", false),
        DiffContent::Conflicted => notice("The file is conflicted.", false),
        DiffContent::Unsupported { reason } => notice(reason.clone(), false),
    }
}

/// A parent link: a loaded parent is selected, its changes asked for and its row brought
/// into view; an unloaded one does nothing, since reaching it is issue #3.
fn follow_parent(parent: Oid, view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(index) = selection::loaded_row(&view.rows.peek(), parent) else {
        return;
    };
    selection::choose(RowId::Commit(parent), view, submit);
    let mut scroll = view.history_scroll;
    reveal_row(&mut scroll, index);
}

fn notice(message: impl Into<String>, alarming: bool) -> Element {
    let colours = get_theme_or_default();
    let colour = if alarming {
        colours.read().colors().error
    } else {
        colours.read().colors().text_placeholder
    };
    rect()
        .expanded()
        .center()
        .padding(12.)
        .child(label().text(message.into()).font_size(13.).color(colour))
        .into()
}
