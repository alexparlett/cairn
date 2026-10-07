//! The sidebar as the window draws it (refs-and-status R8): `cairn_ui::Sidebar` over the rows
//! the window keeps (`sidebar_state`), each branch's counts from the last refresh, the count
//! of paths the last status listed, and what a press is doing. A component of its own, so a
//! keystroke in its filter or an answer for it redraws the sidebar and not the window.

use std::rc::Rc;

use cairn_model::{LocalChanges, SidebarRow, WorkingTreeStatus};
use cairn_ui::{MainView, Sidebar};
use freya::prelude::*;

use crate::ref_find;
use crate::window::View;
use crate::worker::{Refreshed, Request};

pub struct SidebarPane {
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
}

impl SidebarPane {
    pub fn new(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Self {
        Self { view, submit }
    }
}

// By the handles it reads: the submitter is the same repository's however often it is built.
impl PartialEq for SidebarPane {
    fn eq(&self, other: &Self) -> bool {
        self.view.sidebar == other.view.sidebar
            && self.view.refreshed == other.view.refreshed
            && self.submit.is_some() == other.submit.is_some()
    }
}

/// The count beside Local Changes: the distinct paths the last status listed (R9.2) — a path
/// in both lists once; none before a status was read, or where there is no working tree to
/// list.
pub fn local_changes_count(changes: Option<&LocalChanges>) -> Option<usize> {
    let changes = changes?;
    match changes.status() {
        WorkingTreeStatus::Listed(_) => Some(changes.paths()),
        WorkingTreeStatus::IndexUnreadable(_) | WorkingTreeStatus::NoWorkingTree => None,
    }
}

impl Component for SidebarPane {
    fn render(&self) -> impl IntoElement {
        let view = self.view;
        let asking = self.submit.clone();
        // The filter's text, asked for as it changes: the rows are laid out on a worker.
        use_side_effect(move || {
            let text = view.sidebar.filter_text.read().clone();
            let mut sidebar = view.sidebar.state;
            if sidebar.peek().text() == text {
                return;
            }
            let asked = sidebar.write().filter(&text);
            if let (Some(request), Some(submit)) = (asked, asking.as_deref()) {
                submit(request);
            }
        });

        let state = view.sidebar.state.read();
        let refreshed = view.refreshed.read();
        // A press's notice first; else why the refs could not be read, if they could not.
        let notice = state.notice().map(str::to_owned).or_else(|| {
            refreshed
                .failure(Refreshed::Refs)
                .map(|message| format!("The refs could not be read: {message}"))
        });
        let pressing = self.submit.clone();
        let mut main = view.sidebar.main;
        Sidebar::new(state.shown().cloned(), view.sidebar.filter_text)
            .counts(refreshed.ahead_behind().cloned())
            .local_changes(local_changes_count(
                refreshed.local_changes().map(|changes| &**changes),
            ))
            .main(*view.sidebar.main.read())
            .chosen(state.chosen().cloned())
            .notice(notice)
            .on_row(move |row: SidebarRow| ref_find::press(row, view, pressing.as_deref()))
            .on_main(move |chosen: MainView| main.set(chosen))
    }
}
