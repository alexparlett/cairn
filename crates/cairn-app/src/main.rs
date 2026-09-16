//! The Cairn binary.

mod history_state;
mod repository_path;
mod status_text;
mod worker;

use cairn_model::{HistoryRow, RowContent, RowId};
use cairn_ui::{CommitRow, HistoryHeader, HistoryList, ROW_HEIGHT, RowRender};
use freya::prelude::*;

use history_state::{Progress, Status};
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

    let status = progress.read().status().clone();
    let lanes = progress.read().lanes();
    let has_rows = progress.read().has_rows();
    let counted = status_text::loaded_count(&progress.read());

    rect()
        .expanded()
        .theme_background()
        .child(title_bar(&opened, &counted))
        .child(HistoryHeader::new())
        .child(match status_text::placeholder(&status, has_rows) {
            Some(message) => notice(message, &opened),
            None => history(rows, lanes, selected, progress, repository),
        })
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
        // No wildcard arm: a new row kind must fail to compile here.
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
        // `wants_more` debounces: every `submit` supersedes. `peek`, not `read`: reading here
        // subscribes the window to the progress it writes, and loops.
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
