//! The Cairn binary.

mod clipboard;
mod closing;
mod commit_box;
mod detail;
mod diff;
mod frame;
mod history;
mod local_changes;
mod menu_bar;
mod modal;
mod operations;
mod refresh;
mod repository_path;
mod session;
mod shortcuts;
mod sidebar;
mod toolbar;
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
use diff::state::DiffState;
use history::state::Progress;
use operations::fetch::{FetchStatus, PromptView};
use window::View;
use worker::Request;

const PAGE_ROWS: usize = 64;

/// IBM Plex Mono Regular, from IBM's release `@ibm/plex-mono@2.5.0`, under the SIL Open Font
/// Licence 1.1 beside it in `assets/fonts/`: the diff's typeface, embedded so the
/// application draws the same text wherever it runs.
const DIFF_FONT: &[u8] = include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf");

fn main() {
    // Found once, as the application starts, on a thread of its own.
    let git = worker::Discovery::start();
    let closing = Closing::default();
    let window = WindowConfig::new({
        let closing = closing.clone();
        move || app(git.clone(), closing.clone())
    })
    .with_title(cairn_ui::CAPTION_TITLE)
    .with_size(frame::OPENING_SIZE.0, frame::OPENING_SIZE.1)
    .with_min_size(frame::MIN_SIZE.0, frame::MIN_SIZE.1)
    // The Wayland app id, the same name as the X11 class winit already gives the window.
    .with_app_id("cairn")
    // Cairn draws Fork's caption row where the frame is its own (`cairn_ui::Frame`); macOS
    // keeps the desktop's frame.
    .with_decorations(cairn_ui::Frame::current() == cairn_ui::Frame::Native)
    // Closing the window closes its repository first: see `closing`.
    .with_on_close(move |_, _| closing.requested());
    launch(
        LaunchConfig::new()
            .with_font(DIFF_FONT_FAMILY, DIFF_FONT)
            // The edges and corners of a window with no frame of the desktop's resize it.
            .with_plugin(freya::borderless::BorderlessPlugin::new())
            .with_window(window),
    );
}

fn app(git: worker::Discovery, closing: Closing) -> impl IntoElement {
    // Fork Dark's tones, carried into Freya's own components (`cairn_ui::style::tones`).
    use_init_theme(cairn_ui::style::tones::theme);

    // The one copy of the history; this scope must not read it, only `progress`.
    let rows = use_state(History::new);
    let mut progress = use_state(Progress::opening);
    let selected = use_state(|| None::<RowId>);
    let fetch = use_state(|| FetchStatus::Idle);
    let prompt = use_state(|| None::<PromptView>);
    let remotes = use_state(Vec::<RemoteSummary>::new);
    let endings = use_state(crate::operations::ending_dialog::Endings::default);
    let lock = use_state(crate::operations::leftover_lock::LeftoverLock::default);
    let diff = use_state(DiffState::default);
    let history_scroll = use_scroll_controller(ScrollConfig::default);
    let history_cursor = use_state(|| 0usize);
    // Session state: the tab and the pane's shape outlive every selection.
    let detail_tab = use_state(DetailTab::default);
    let pane_collapsed = use_state(|| false);
    let pane_split = use_state(detail::split::PaneSplit::default);
    // Session state of every diff view: its settings, its scroll, the change moved to.
    let diff_settings = use_state(DiffSettings::default);
    let diff_scroll = use_scroll_controller(ScrollConfig::default);
    let change_cursor = use_state(|| None);
    // Session state of the Changes tab: its filter and its list's width.
    let filter_text = use_state(String::new);
    let changes_list_width = use_state(|| detail::changes_tab::LIST_WIDTH);
    // The second commit of a comparison, and the keys a press on a row is resolved against.
    let pair = use_state(|| None);
    let held_keys = use_state(cairn_ui::accelerators::HeldKeys::default);
    // What the last refresh answered.
    let refreshed = use_state(refresh::state::RefreshState::default);
    // What the status box and the window's title call the repository, once the worker has opened it.
    let repository = use_state(|| None::<String>);
    let repository_path = use_state(|| None::<std::path::PathBuf>);
    // The sidebar's state, for the session.
    let sidebar = sidebar::state::SidebarView::used();
    // Local Changes' lists, filter and diff, for the session.
    let local = local_changes::state::LocalChangesView::used();
    // The local writes, for the session.
    let writes = use_state(operations::local_writes::LocalWrites::default);
    // A destructive operation's confirmation, while one is open.
    let confirming = use_state(|| None::<operations::confirming::Confirming>);
    // Show Lost Commits, off as the window opens.
    let show_lost = use_state(|| false);
    // The zone dates are drawn in: UTC, said nowhere, until the worker reads the system's.
    let zone = use_state(cairn_ui::LocalZone::default);
    // Create Branch, for the session.
    let branch = operations::create_branch::CreateBranchView::used();
    // The activity popover's operations, for the session.
    let activity = operations::activity::ActivityView::used();
    // The window's frame: its zoom and About Cairn, for the session.
    let frame = frame::FrameView::used();
    let view = View {
        rows,
        progress,
        selected,
        fetch,
        prompt,
        remotes,
        endings,
        lock,
        diff,
        history_scroll,
        history_cursor,
        detail_tab,
        pane_collapsed,
        pane_split,
        diff_settings,
        diff_scroll,
        change_cursor,
        filter_text,
        changes_list_width,
        pair,
        held_keys,
        refreshed,
        repository,
        repository_path,
        sidebar,
        local,
        writes,
        confirming,
        show_lost,
        zone,
        branch,
        activity,
        frame,
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
                // The first close asked while a write runs waits for it, and says which.
                closing.when_requested(move || {
                    let mut told = writes;
                    told.write().closing();
                });
                let platform = Platform::get();
                handle.submit(Request::ListRemotes);
                handle.submit(Request::ConfiguredContext);
                // The refs, ahead/behind and status, read for the first time: the refs'
                // answer has no walk to compare with, so it asks for the history, which
                // then walks from them.
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
