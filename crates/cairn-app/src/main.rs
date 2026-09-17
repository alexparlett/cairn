//! The Cairn binary.

mod fetch_state;
mod history_state;
mod repository_path;
mod status_text;
mod window;
mod worker;

use cairn_model::{HistoryRow, RemoteSummary, RowId};
use freya::prelude::*;

use fetch_state::{FetchStatus, PromptView};
use history_state::Progress;
use window::View;
use worker::{Reply, Request, Update};

const PAGE_ROWS: usize = 64;

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app).with_title("Cairn")));
}

fn app() -> impl IntoElement {
    use_init_theme(dark_theme);

    // The one copy of the history; this scope must not read it, only `progress`.
    let mut rows = use_state(Vec::<HistoryRow>::new);
    let mut progress = use_state(Progress::opening);
    let selected = use_state(|| None::<RowId>);
    let mut fetch = use_state(|| FetchStatus::Idle);
    let mut prompt = use_state(|| None::<PromptView>);
    let mut remotes = use_state(Vec::<RemoteSummary>::new);

    let opened = use_hook(|| {
        repository_path::chosen(std::env::args_os(), repository_path::working_directory())
            .display()
            .to_string()
    });

    let repository = use_hook({
        let path = opened.clone();
        move || match worker::open(&path) {
            Ok((handle, mut updates, answer)) => {
                handle.submit(Request::ListRemotes);
                handle.submit(Request::OpenHistory { rows: PAGE_ROWS });
                let reloading = handle.clone();
                let withdrawing = answer.clone();
                spawn(async move {
                    while let Some(update) = updates.next().await {
                        match update {
                            Update::Rows {
                                rows: page,
                                complete,
                            } => {
                                // Count before handing the rows over; afterwards it rereads the whole history.
                                let widest = history_state::widest_lane(&page);
                                let loaded = {
                                    let mut held = rows.write();
                                    held.extend(page);
                                    held.len()
                                };
                                progress.write().received(widest, complete, loaded);
                            }
                            Update::Failed { message } | Update::WorkerLost { message } => {
                                progress.write().failed(message);
                            }
                            Update::Remotes { remotes: listed } => remotes.set(listed),
                            Update::FetchStarted { remote } => fetch.write().started(remote),
                            Update::FetchProgress { remote, line } => {
                                fetch.write().progressed(&remote, line);
                            }
                            Update::FetchFinished { remote, refreshed } => {
                                withdraw(&mut prompt, &withdrawing);
                                fetch.set(FetchStatus::Finished { remote });
                                if refreshed {
                                    // The refs moved: the rows on screen are of the old ones.
                                    rows.write().clear();
                                    progress.set(Progress::opening());
                                    reloading.submit(Request::OpenHistory { rows: PAGE_ROWS });
                                }
                            }
                            Update::FetchCancelled { remote } => {
                                withdraw(&mut prompt, &withdrawing);
                                fetch.set(FetchStatus::Cancelled { remote });
                            }
                            Update::FetchFailed { remote, message } => {
                                withdraw(&mut prompt, &withdrawing);
                                fetch.set(FetchStatus::Failed { remote, message });
                            }
                            Update::Prompt { id, text } => {
                                prompt.set(Some(PromptView { id, text }))
                            }
                        }
                    }
                    // Only if the worker did not already name a cause.
                    progress
                        .write()
                        .stream_ended("the repository worker has stopped");
                });
                Some((handle, answer))
            }
            Err(error) => {
                progress.write().failed(error.to_string());
                None
            }
        }
    });

    let (submit, answer) = match repository {
        Some((handle, answer)) => (Some(handle.into_submitter()), Some(answer)),
        None => (None, None),
    };

    window::window(
        &opened,
        View {
            rows,
            progress,
            selected,
            fetch,
            prompt,
            remotes,
        },
        submit,
        answer,
    )
}

/// Takes down a dialog whose fetch has ended, refusing the prompt so the helper
/// still waiting on it is released: the git behind it is gone or failing anyway.
fn withdraw(prompt: &mut State<Option<PromptView>>, answer: &window::Replier) {
    if let Some(shown) = prompt.write().take() {
        answer(Reply::Refuse { prompt: shown.id });
    }
}
