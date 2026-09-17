//! The Cairn binary.

mod fetch_state;
mod history_state;
mod repository_path;
mod session;
mod status_text;
mod window;
mod worker;

use std::rc::Rc;

use cairn_model::{HistoryRow, RemoteSummary, RowId};
use freya::prelude::*;

use fetch_state::{FetchStatus, PromptView};
use history_state::Progress;
use window::View;
use worker::Request;

const PAGE_ROWS: usize = 64;

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app).with_title("Cairn")));
}

fn app() -> impl IntoElement {
    use_init_theme(dark_theme);

    // The one copy of the history; this scope must not read it, only `progress`.
    let rows = use_state(Vec::<HistoryRow>::new);
    let mut progress = use_state(Progress::opening);
    let selected = use_state(|| None::<RowId>);
    let fetch = use_state(|| FetchStatus::Idle);
    let prompt = use_state(|| None::<PromptView>);
    let remotes = use_state(Vec::<RemoteSummary>::new);
    let view = View {
        rows,
        progress,
        selected,
        fetch,
        prompt,
        remotes,
    };

    let opened = use_hook(|| {
        repository_path::chosen(std::env::args_os(), repository_path::working_directory())
            .display()
            .to_string()
    });

    let repository = use_hook({
        let path = opened.clone();
        move || match worker::open(&path) {
            Ok((handle, mut updates, reply)) => {
                handle.submit(Request::ListRemotes);
                handle.submit(Request::OpenHistory { rows: PAGE_ROWS });
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
                            },
                        );
                    }
                    // Only if the worker did not already name a cause.
                    progress
                        .write()
                        .stream_ended("the repository worker has stopped");
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
