//! The Cairn binary.
//!
//! Owns the window and the seam: repository work runs off the UI thread and
//! reaches the view as [`cairn_model`] values. Components never call
//! `cairn_git` themselves — this crate is the only place the two layers meet,
//! and inside it only [`worker`] may name the engine. Everything else here
//! renders, and can neither reach a repository nor wait on one.

mod worker;

use std::path::PathBuf;

use cairn_model::HistoryRow;
use cairn_ui::CommitRow;
use freya::prelude::*;

use worker::{Request, Update};

/// How many rows a page asks for.
///
/// A page decodes one commit object per row in its limit, and on a cold page
/// cache each of those can be a pack seek, so this is deliberately a couple of
/// screens rather than a comfortable margin.
const PAGE_ROWS: usize = 64;

/// How many rows are loaded and drawn before the view stops asking.
///
/// The history list is not virtualised yet — phase 04 owns R4.1 — so until it
/// is, the view loads and draws a bounded number of rows rather than one
/// component per row of however much history there is. A cap is not
/// virtualisation; it is what stops an unbounded render existing meanwhile.
const ROWS_DRAWN: usize = 256;

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app).with_title("Cairn")));
}

fn app() -> impl IntoElement {
    use_init_theme(dark_theme);

    let mut rows = use_state(Vec::<HistoryRow>::new);
    let mut status = use_state(|| "Reading history...".to_owned());
    // Whether a worker has already explained itself, so the generic "stopped"
    // line cannot overwrite a message that named a cause.
    let mut reported = use_state(|| false);
    let mut selected = use_state(|| 0usize);

    // Open the repository once, and drive the worker's answers from one task.
    // The task awaits, so the event loop keeps running between pages; nothing
    // on this side of the seam ever waits for a repository.
    use_hook(move || {
        let here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        match worker::open(here) {
            Ok((handle, mut updates)) => {
                handle.submit(Request::OpenHistory { rows: PAGE_ROWS });
                spawn(async move {
                    while let Some(update) = updates.next().await {
                        match update {
                            Update::Rows {
                                rows: page,
                                complete,
                            } => {
                                let loaded = {
                                    let mut held = rows.write();
                                    held.extend(page);
                                    held.len()
                                };
                                if complete || loaded >= ROWS_DRAWN {
                                    status.set(String::new());
                                } else {
                                    // The next page of the SAME walk: the
                                    // worker still has it open, so this costs
                                    // the page rather than everything before it.
                                    handle.submit(Request::MoreHistory { rows: PAGE_ROWS });
                                }
                            }
                            Update::Failed { message } | Update::WorkerLost { message } => {
                                reported.set(true);
                                status.set(message);
                            }
                        }
                    }
                    // The stream ended. Say so only if the worker did not
                    // already say something better: it announces its own death
                    // before its channel closes, and overwriting that with a
                    // generic line loses the only sentence that named a cause.
                    if !*reported.read() {
                        status.set("the repository worker has stopped".to_owned());
                    }
                });
            }
            Err(error) => status.set(error.to_string()),
        }
    });

    rect()
        .expanded()
        .theme_background()
        .child(
            rect()
                .width(Size::fill())
                .padding(Gaps::new(10., 12., 10., 12.))
                .child(label().text("Cairn").theme_color().font_size(18.)),
        )
        .maybe(!status.read().is_empty(), |el| {
            el.child(
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(6., 12., 6., 12.))
                    .child(label().text(status.read().clone()).theme_color()),
            )
        })
        .child(
            rect()
                .expanded()
                .children(
                    rows.read()
                        .iter()
                        .take(ROWS_DRAWN)
                        .enumerate()
                        .map(|(i, row)| {
                            CommitRow::new(
                                row.commit.clone(),
                                EventHandler::new(move |()| selected.set(i)),
                            )
                            .selected(i == *selected.read())
                            .key(i)
                        }),
                ),
        )
}
