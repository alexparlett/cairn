//! The Cairn binary.
//!
//! Owns the window and the seam: repository work runs off the UI thread and
//! reaches the view as [`cairn_model`] values. Components never call
//! `cairn_git` themselves — this crate is the only place the two layers meet,
//! and inside it only [`worker`] may name the engine. Everything else here
//! renders, and can neither reach a repository nor wait on one.

mod history_state;
mod repository_path;
mod worker;

use cairn_model::{HistoryRow, RowContent, RowId};
use cairn_ui::{CommitRow, HistoryHeader, HistoryList, ROW_HEIGHT, RowRender};
use freya::prelude::*;

use history_state::{Progress, Status};
use worker::{Request, Update};

/// How many rows a page asks for.
///
/// A page decodes one commit object per row in its limit, and on a cold page
/// cache each of those can be a pack seek, so this is deliberately a couple of
/// screens rather than a comfortable margin.
const PAGE_ROWS: usize = 64;

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app).with_title("Cairn")));
}

fn app() -> impl IntoElement {
    use_init_theme(dark_theme);

    // The one copy of the history. Everywhere else it is a handle: the list
    // reads it and the item builder reads it, and nothing copies it to draw.
    // The window itself never reads it — everything the window decides comes
    // from `progress`, so a page arriving re-renders the list and not this.
    let mut rows = use_state(Vec::<HistoryRow>::new);
    let mut progress = use_state(Progress::opening);
    // Selection is held as the row's own identity, not its index: an index
    // means something different the moment rows arrive above it, and R4.4 asks
    // selection to survive exactly that.
    let selected = use_state(|| None::<RowId>);

    // R5: the repository containing the first command-line argument, or the
    // working directory. Resolved once, and kept so the window can name it —
    // including in the message R5.2 asks for when there is no repository there.
    let opened = use_hook(|| {
        repository_path::chosen(
            std::env::args_os().skip(1),
            repository_path::working_directory(),
        )
        .display()
        .to_string()
    });

    // Open the repository once, and drive the worker's answers from one task.
    // The task awaits, so the event loop keeps running between pages; nothing
    // on this side of the seam ever waits for a repository.
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
                                // The lanes a page needs are counted before its
                                // rows are handed over: counting them afterwards
                                // would mean walking the whole loaded history
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
                    // The stream ended. Say so only if the worker did not
                    // already say something better: it announces its own death
                    // before its channel closes, and overwriting that with a
                    // generic line loses the only sentence that named a cause.
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
    let counted = count(&progress.read());

    rect()
        .expanded()
        .theme_background()
        .child(title_bar(&opened, &counted))
        .child(HistoryHeader::new())
        // R4.3: loading, empty and a failure that never produced a row each get
        // their own sentence in the list's place. The reader is never shown a
        // blank area that could mean any of the three.
        .child(match &status {
            Status::Loading => notice("Reading history…", &opened),
            Status::Empty => notice("No commits yet.", &opened),
            Status::Failed(message) if !has_rows => notice(message.clone(), &opened),
            Status::Failed(_) | Status::Ready => {
                history(rows, lanes, selected, progress, repository)
            }
        })
        // A failure that arrived after rows were already drawn is a banner
        // under them, not a replacement for them.
        .maybe(has_rows, |el| match &status {
            Status::Failed(message) => el.child(banner(message.clone())),
            _ => el,
        })
}

/// The list itself, wired to the worker.
fn history(
    rows: State<Vec<HistoryRow>>,
    lanes: usize,
    mut selected: State<Option<RowId>>,
    mut progress: State<Progress>,
    repository: Option<worker::RepositoryHandle>,
) -> Element {
    HistoryList::new(rows, move |render: RowRender| {
        // A row is a list entry, not by definition a commit (R6.2): what to
        // draw is decided by matching its content, exhaustively and with no
        // wildcard. The row kind `refs-and-status` adds turns this into a
        // compile error here, which is the point — a view must CHOOSE what it
        // draws for a row that is not a commit, not silently draw nothing.
        // This match is what pins A9's second clause.
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
        // The list asks every time a row near the end is visible; whether that
        // is worth a request is this side's decision, and the answer is no
        // while one is in flight or the history is complete. That check IS the
        // debounce the worker boundary needs: every `submit` supersedes, so
        // asking twice throws away the page being built.
        //
        // `peek`, not `read`: this runs inside an event, and subscribing the
        // window to the progress it is about to write would loop it against its
        // own write.
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

/// How much of the history is loaded, as the title bar says it.
///
/// An ellipsis while more is coming, because "2,896 commits" and "2,896 commits
/// so far" are different claims and only one of them is true mid-scroll.
fn count(progress: &Progress) -> String {
    let loaded = progress.loaded();
    let noun = if loaded == 1 { "commit" } else { "commits" };
    if progress.complete() {
        format!("{loaded} {noun}")
    } else {
        format!("{loaded} {noun}…")
    }
}

/// The window's own line: which repository is open, and how much of it is here.
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

/// What fills the list area when there is no list to show. Always names the
/// repository, which is half of what R5.2 asks of a failure.
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

/// A failure that arrived after rows were already on screen. The rows stay; a
/// page that failed does not unsay the pages that worked.
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
