//! The Cairn binary.

mod history_state;
mod repository_path;
mod status_text;
mod window;
mod worker;

use std::rc::Rc;

use cairn_model::{HistoryRow, RowId};
use freya::prelude::*;

use history_state::Progress;
use worker::{Request, Update};

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

    let opened = use_hook(|| {
        repository_path::chosen(std::env::args_os(), repository_path::working_directory())
            .display()
            .to_string()
    });

    let repository = use_hook({
        let path = opened.clone();
        move || match worker::open(&path) {
            Ok((handle, mut updates)) => {
                handle.submit(Request::OpenHistory { rows: PAGE_ROWS });
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
                        }
                    }
                    // Only if the worker did not already name a cause.
                    progress
                        .write()
                        .stream_ended("the repository worker has stopped");
                });
                Some(handle)
            }
            Err(error) => {
                progress.write().failed(error.to_string());
                None
            }
        }
    });

    let submit = repository.map(|handle: worker::RepositoryHandle| -> Rc<dyn Fn(Request)> {
        Rc::new(move |request| {
            handle.submit(request);
        })
    });

    window::window(&opened, rows, progress, selected, submit)
}
