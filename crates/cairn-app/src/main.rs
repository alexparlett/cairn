//! The Cairn binary: the window, and the wiring between engine and view.
//!
//! The only crate where the two layers meet, and inside it only [`worker`] may
//! name the engine.

mod history_state;
mod repository_path;
mod status_text;
mod worker;

use cairn_model::{HistoryRow, RowContent, RowId};
use cairn_ui::{CommitRow, HistoryHeader, HistoryList, ROW_HEIGHT, RowRender};
use freya::prelude::*;

use history_state::{Progress, Status};
use worker::{Request, Update};

/// How many rows a page asks for. One commit object is decoded per row, each a
/// possible pack seek on a cold cache, so this is a couple of screens rather
/// than a comfortable margin.
const PAGE_ROWS: usize = 64;

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app).with_title("Cairn")));
}

fn app() -> impl IntoElement {
    use_init_theme(dark_theme);

    // The one copy of the history; everywhere else it is a handle. This scope
    // never reads it — everything it decides comes from `progress` — so a page
    // costs a title bar and a header, not a pass over the history.
    let mut rows = use_state(Vec::<HistoryRow>::new);
    let mut progress = use_state(Progress::opening);
    // Identity, not index: an index means something else the moment rows arrive
    // above it, and R4.4 asks selection to survive that.
    let selected = use_state(|| None::<RowId>);

    // R5. Kept so the window can name it, including in R5.2's message when
    // there is no repository there.
    let opened = use_hook(|| {
        repository_path::chosen(std::env::args_os(), repository_path::working_directory())
            .display()
            .to_string()
    });

    // Opened once, and the answers driven from one task. The task awaits, so
    // the event loop keeps running between pages.
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
                                // Counted before the rows are handed over:
                                // afterwards walks the whole loaded history
                                // again for every page.
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
                    // Only if the worker did not already say something better:
                    // a generic line would overwrite the named cause.
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

    let status = progress.read().status().clone();
    let lanes = progress.read().lanes();
    let has_rows = progress.read().has_rows();
    let counted = status_text::loaded_count(&progress.read());

    rect()
        .expanded()
        .theme_background()
        .child(title_bar(&opened, &counted))
        .child(HistoryHeader::new())
        // R4.3: loading, empty and a failure that produced no row each get their
        // own sentence, so a blank area never means any of the three.
        .child(match status_text::placeholder(&status, has_rows) {
            Some(message) => notice(message, &opened),
            None => history(rows, lanes, selected, progress, repository),
        })
        // A failure after rows were drawn is a banner under them, not a
        // replacement for them.
        .maybe(has_rows, |el| match &status {
            Status::Failed(message) => el.child(banner(message.clone())),
            _ => el,
        })
}

fn history(
    rows: State<Vec<HistoryRow>>,
    lanes: usize,
    mut selected: State<Option<RowId>>,
    mut progress: State<Progress>,
    repository: Option<worker::RepositoryHandle>,
) -> Element {
    HistoryList::new(rows, move |render: RowRender| {
        // Matched exhaustively and with no wildcard (R6.2), so the row kind
        // `refs-and-status` adds is a compile error here rather than a row
        // silently not drawn. Pins A9's second clause.
        match render.row.content {
            RowContent::Commit(commit) => CommitRow::new(commit, render.row.graph, render.lanes)
                .selected(render.selected)
                .into(),
        }
    })
    .lanes(lanes)
    .selected(*selected.read())
    .on_select(move |id: RowId| selected.set(Some(id)))
    .on_reach_end(move |()| {
        // The list asks every time a row near the end is visible; `wants_more`
        // is the debounce, since every `submit` supersedes and asking twice
        // throws away the page being built. `peek`, not `read`: subscribing the
        // window to the progress it is about to write would loop it.
        if !progress.peek().wants_more() {
            return;
        }
        if let Some(handle) = &repository {
            handle.submit(Request::MoreHistory { rows: PAGE_ROWS });
            progress.write().asked();
        }
    })
    .into()
}

/// Which repository is open, and how much of it is here.
fn title_bar(path: &str, counted: &str) -> Element {
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .cross_align(Alignment::center())
        .spacing(10.)
        .padding(Gaps::new(8., 12., 8., 12.))
        .child(label().text("Cairn").theme_color().font_size(16.))
        .child(
            label()
                .text(path.to_owned())
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .width(Size::flex(1.))
                .font_size(13.)
                .color(get_theme_or_default().read().colors().text_secondary),
        )
        .child(
            label()
                .text(counted.to_owned())
                .max_lines(1)
                .font_size(13.)
                .color(get_theme_or_default().read().colors().text_placeholder),
        )
        .into()
}

/// What fills the list area when there is no list. The sentence is
/// [`status_text::placeholder`]'s.
fn notice(message: impl Into<String>, path: &str) -> Element {
    let message = message.into();
    rect()
        .expanded()
        .center()
        .spacing(6.)
        .child(label().text(message).theme_color().font_size(14.))
        .child(
            label()
                .text(path.to_owned())
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(12.)
                .color(get_theme_or_default().read().colors().text_placeholder),
        )
        .into()
}

/// A failure after rows were already on screen: the rows stay, since a page that
/// failed does not unsay the pages that worked.
fn banner(message: String) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::px(ROW_HEIGHT))
        .cross_align(Alignment::center())
        .padding(Gaps::new(0., 12., 0., 12.))
        .background(get_theme_or_default().read().colors().surface_tertiary)
        .child(
            label()
                .text(message)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(13.)
                .color(get_theme_or_default().read().colors().error),
        )
        .into()
}
