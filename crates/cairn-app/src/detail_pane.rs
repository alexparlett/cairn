//! The detail pane under the commit list (PRD R5.1, R5.2): its strip of tabs, and the tab
//! shown. A component of its own, so an answer arriving redraws the pane and not the window.
//!
//! What it draws is decided against the selection, not against whatever the diff state
//! holds: the Commit tab draws a change set only when it is the answer for the row
//! selected now (R4.4), so a fast click never shows one commit's files under another.

use std::rc::Rc;

use cairn_model::{Oid, RowId};
use cairn_ui::accelerators::{self, Scope};
use cairn_ui::{CommitTab, DetailTab, DetailTabs, reveal_row};
use freya::prelude::*;

use crate::diff_state::{Answer, answered_changes};
use crate::window::View;
use crate::worker::Request;
use crate::{selection, shortcuts};

/// Said in the pane while no row is selected.
pub const NOTHING_SELECTED: &str = "Select a commit to see its details.";
/// Said while the selected commit's answer is on its way.
pub const READING: &str = "Reading the commit…";
/// Said in the Changes tab, whose contents arrive with the diff view.
pub const CHANGES_NOT_BUILT: &str = "The Changes tab is not built yet.";
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
                    shortcuts::act(action, view);
                }
            })
            .child(strip)
            .maybe_child((!collapsed).then(|| match tab {
                DetailTab::Commit => commit_body(view, self.submit.clone()),
                DetailTab::Changes => notice(CHANGES_NOT_BUILT, false),
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
            CommitTab::new(changes)
                .on_parent(move |parent: Oid| follow_parent(parent, view, submit.as_deref()))
                .into()
        }
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
