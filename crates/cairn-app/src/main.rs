//! The Cairn binary.
//!
//! Owns the window and the seam: repository work runs off the UI thread and
//! reaches the view as [`cairn_model`] values. Components never call
//! `cairn_git` themselves — this crate is the only place the two layers meet.

use cairn_model::CommitSummary;
use cairn_ui::CommitRow;
use freya::prelude::*;

fn main() {
    launch(LaunchConfig::new().with_window(WindowConfig::new(app).with_title("Cairn")));
}

fn app() -> impl IntoElement {
    use_init_theme(dark_theme);

    // Empty until the history query lands; the shell exists so the seam and the
    // component contract are exercised by something that actually runs.
    let commits = use_state(Vec::<CommitSummary>::new);
    let mut selected = use_state(|| 0usize);

    rect()
        .expanded()
        .theme_background()
        .child(
            rect()
                .width(Size::fill())
                .padding(Gaps::new(10., 12., 10., 12.))
                .child(label().text("Cairn").theme_color().font_size(18.)),
        )
        .child(
            rect()
                .expanded()
                .children(commits.read().iter().enumerate().map(|(i, commit)| {
                    CommitRow::new(commit.clone(), EventHandler::new(move |()| selected.set(i)))
                        .selected(i == *selected.read())
                        .key(i)
                })),
        )
}
