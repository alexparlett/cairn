//! The Cairn binary.

mod changes_tab;
mod closing;
mod detail_pane;
mod diff_actions;
mod diff_state;
mod fetch_state;
mod file_filter;
mod history_state;
mod ref_find;
mod refresh;
mod refresh_state;
mod repository_path;
mod row_finder;
mod selection;
mod session;
mod shortcuts;
mod sidebar_pane;
mod sidebar_state;
#[cfg(test)]
mod sidebar_tests;
mod status_text;
mod window;
#[cfg(test)]
mod window_check;
mod worker;

use std::rc::Rc;

use cairn_model::{History, RemoteSummary, RowId};
use cairn_ui::diff_palette::DIFF_FONT_FAMILY;
use cairn_ui::{DetailTab, DiffSettings};
use freya::prelude::*;

use closing::Closing;
use diff_state::DiffState;
use fetch_state::{FetchStatus, PromptView};
use history_state::Progress;
use window::View;
use worker::Request;

const PAGE_ROWS: usize = 64;

/// IBM Plex Mono Regular, from IBM's release `@ibm/plex-mono@2.5.0`, under the SIL Open Font
/// Licence 1.1 beside it in `assets/fonts/` (L16): the diff's typeface, embedded so the
/// application draws the same text wherever it runs.
const DIFF_FONT: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");

fn main() {
    // Found once, as the application starts, on a thread of its own (PRD R6.1).
    let git = worker::Discovery::start();
    let closing = Closing::default();
    let window = WindowConfig::new({
        let closing = closing.clone();
        move || app(git.clone(), closing.clone())
    })
    .with_title("Cairn")
    // Closing the window closes its repository first (PRD R6.3): see `closing`.
    .with_on_close(move |_, _| closing.requested());
    launch(
        LaunchConfig::new()
            .with_font(DIFF_FONT_FAMILY, DIFF_FONT)
            .with_window(window),
    );
}

fn app(git: worker::Discovery, closing: Closing) -> impl IntoElement {
    use_init_theme(dark_theme);

    // The one copy of the history; this scope must not read it, only `progress`.
    let rows = use_state(History::new);
    let mut progress = use_state(Progress::opening);
    let selected = use_state(|| None::<RowId>);
    let fetch = use_state(|| FetchStatus::Idle);
    let prompt = use_state(|| None::<PromptView>);
    let remotes = use_state(Vec::<RemoteSummary>::new);
    let refused = use_state(|| None);
    let diff = use_state(DiffState::default);
    let history_scroll = use_scroll_controller(ScrollConfig::default);
    let history_cursor = use_state(|| 0usize);
    // Session state: the tab and the pane's shape outlive every selection (R5.2).
    let detail_tab = use_state(DetailTab::default);
    let pane_collapsed = use_state(|| false);
    let pane_height = use_state(|| window::PANE_HEIGHT);
    // Session state of every diff view: its settings, its scroll, the change moved to.
    let diff_settings = use_state(DiffSettings::default);
    let diff_scroll = use_scroll_controller(ScrollConfig::default);
    let change_cursor = use_state(|| None);
    // Session state of the Changes tab: its filter and its list's width.
    let filter_text = use_state(String::new);
    let changes_list_width = use_state(|| changes_tab::LIST_WIDTH);
    // The second commit of a comparison, and the keys a press on a row is resolved against.
    let pair = use_state(|| None);
    let held_keys = use_state(cairn_ui::accelerators::HeldKeys::default);
    // What the last refresh answered (R10).
    let refreshed = use_state(refresh_state::RefreshState::default);
    // What the title bar calls the repository, once the worker has opened it.
    let repository = use_state(|| None::<String>);
    // The sidebar's state, for the session (refs-and-status R8).
    let sidebar = sidebar_state::SidebarView::used();
    let view = View {
        rows,
        progress,
        selected,
        fetch,
        prompt,
        remotes,
        refused,
        diff,
        history_scroll,
        history_cursor,
        detail_tab,
        pane_collapsed,
        pane_height,
        diff_settings,
        diff_scroll,
        change_cursor,
        filter_text,
        changes_list_width,
        pair,
        held_keys,
        refreshed,
        repository,
        sidebar,
    };

    let opened = use_hook(|| {
        repository_path::chosen(std::env::args_os(), repository_path::working_directory())
            .display()
            .to_string()
    });

    let repository = use_hook({
        let path = opened.clone();
        let git = git.clone();
        let closing = closing.clone();
        move || match worker::open(&path, &git) {
            Ok((handle, mut updates, reply)) => {
                closing.opened(handle.clone());
                let platform = Platform::get();
                handle.submit(Request::ListRemotes);
                handle.submit(Request::ConfiguredContext);
                // The refs, ahead/behind and status, read for the first time: the refs'
                // answer has no walk to compare with, so it asks for the history, which
                // then walks from them (R10, R11.2).
                handle.submit(Request::Refresh);
                let submitting = handle.clone();
                // Weak: the task must not keep the answering end alive past the window,
                // or the acceptor could never see it go.
                let replying = Rc::downgrade(&reply);
                spawn(async move {
                    let submit = move |request| {
                        submitting.submit(request);
                    };
                    let refuse = move |prompt| {
                        if let Some(reply) = replying.upgrade() {
                            reply(worker::Reply::Refuse { prompt });
                        }
                    };
                    while let Some(update) = updates.next().await {
                        session::apply(
                            update,
                            view,
                            &session::Worker {
                                submit: &submit,
                                refuse: &refuse,
                                closing: closing.is_requested(),
                            },
                        );
                    }
                    if closing.worker_gone() {
                        // The close the window asked for is done: every git it ran is
                        // over and its socket gone. Past the hook, which asked already.
                        platform.close_current_window();
                    } else {
                        // Only if the worker did not already name a cause.
                        progress
                            .write()
                            .stream_ended("the repository worker has stopped");
                    }
                });
                Some((handle, reply))
            }
            Err(error) => {
                progress.write().failed(error.to_string());
                None
            }
        }
    });

    let (submit, reply) = match repository {
        Some((handle, reply)) => (Some(handle.into_submitter()), Some(reply)),
        None => (None, None),
    };

    window::window(&opened, view, submit, reply)
}
