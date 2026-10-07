//! The Changes tab (PRD R5.4, R6.1, R6.8): a one-line summary of the commit, its changed
//! files on the left behind a filter, and one file's diff on the right — one file at a time,
//! as Fork shows it (Findings 5 and 19). A component of its own, mounted only while the tab
//! is shown, so the work it watches for — a filter's text, the first file to choose — is done
//! only for a tab someone is looking at.
//!
//! **What it asks, and where that runs.** Typing in the filter asks a worker which files
//! match (`DiffState::filter`, a request in the file-filter lane): the UI thread does no pass
//! over the paths. Choosing a file — pressed, or reached with ↑ or ↓ in the focused list —
//! asks its diff through the file-diff lane, which supersedes the last file's and kills its
//! `git` (`diff_actions::choose_file`). With no file chosen, the first file the list shows is
//! chosen, as Fork selects the first file by default (Finding 5).
//!
//! **What it draws, and when.** The summary and the list only for the change set of the
//! selection now; the diff only for the answer naming the file and the settings now — its
//! rows, unified or side by side as the shared setting says, or the notice of a state that is
//! not text (`cairn_ui::DiffNotice`), with Load Diff for a file past the limits.

use std::rc::Rc;

use cairn_model::{ChangeStatus, ChangedFile, ShownDiff};
use cairn_ui::{
    ChangesList, ChangesSummary, ComparisonHeader, DiffHeader, DiffNotice, DiffNoticeView, DiffView,
};
use freya::prelude::*;

use crate::detail_pane::{NOTHING_SELECTED, READING, notice};
use crate::diff_state::{Answer, answered_changes, answered_file, answered_files};
use crate::window::View;
use crate::worker::{FileTarget, Request};
use crate::{diff_actions, selection, shortcuts};

/// Said beside the list while no file is chosen.
pub const NO_FILE_CHOSEN: &str = "Choose a file to see its diff.";
/// Said while the chosen file's diff is on its way.
pub const READING_DIFF: &str = "Reading the diff…";
/// Said under the comparison's header while what two commits changed is on its way.
pub const READING_COMPARISON: &str = "Reading the comparison…";
/// The file list's share of the pane until its splitter is dragged, in percent (the user's
/// decision, 2026-10-03: about a third).
pub const LIST_WIDTH: f32 = 35.0;
/// The least a drag, or a narrowing window, leaves the file list (the user's decision).
const LIST_MIN_PIXELS: f32 = 200.0;
const DIFF_MIN_PIXELS: f32 = 240.0;

pub struct ChangesTab {
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
}

impl ChangesTab {
    pub fn new(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Self {
        Self { view, submit }
    }
}

// By the handles it reads: the submitter is the same repository's however often it is built.
impl PartialEq for ChangesTab {
    fn eq(&self, other: &Self) -> bool {
        let (one, two) = (&self.view, &other.view);
        one.selected == two.selected
            && one.pair == two.pair
            && one.diff == two.diff
            && one.diff_settings == two.diff_settings
            && one.diff_scroll == two.diff_scroll
            && one.change_cursor == two.change_cursor
            && one.filter_text == two.filter_text
            && one.changes_list_width == two.changes_list_width
            && self.submit.is_some() == other.submit.is_some()
    }
}

impl Component for ChangesTab {
    fn render(&self) -> impl IntoElement {
        let view = self.view;
        // The split's width as last laid out, which turns the list's dragged width into its
        // share. Only peeked: a measurement redraws nothing.
        let mut split_width = use_state(|| 0f32);
        let filtering = self.submit.clone();
        // The filter's text, as typed: asked of a worker when it changes. Peeked state, so
        // only a keystroke runs this.
        use_side_effect(move || {
            let text = view.filter_text.read().clone();
            let mut diff = view.diff;
            if diff.peek().filter_text() == text {
                return;
            }
            let asked = diff.write().filter(&text);
            if let (Some(request), Some(submit)) = (asked, filtering.as_deref()) {
                submit(request);
            }
        });
        // A file whose request lost the lane to the Commit tab's files opened in place (they
        // share it) is asked again as this tab is shown.
        let reasking = self.submit.clone();
        use_side_effect(move || {
            if !view.diff.read().file_needs_asking() {
                return;
            }
            let mut diff = view.diff;
            let asked = diff.write().reask_file();
            if let (Some(request), Some(submit)) = (asked, reasking.as_deref()) {
                submit(request);
            }
        });
        // With no file chosen, the first the list shows is: Fork's default (Finding 5).
        let choosing = self.submit.clone();
        use_side_effect(move || {
            let first = {
                let state = view.diff.read();
                let ready = matches!(state.changes(), Some((_, Answer::Ready(_))));
                if !ready || state.file().is_some() || !state.filter_is_settled() {
                    return;
                }
                answered_files(&state)
                    .file_at(0)
                    .filter(|index| *index < answered_changes(&state).files.len())
            };
            if let Some(first) = first {
                diff_actions::choose_file(first, view, choosing.as_deref());
            }
        });

        let Some(of) = selection::selected_comparison(view) else {
            return notice(NOTHING_SELECTED, false);
        };
        // Over two commits, both named, base then tip, with the swap (R7.3) — drawn the moment
        // the pair is set, what they changed awaited under it (the user's decision,
        // 2026-10-04); over one, its one-line summary, once its answer is here.
        let compared = view.pair.read().clone();
        let swapping = self.submit.clone();
        let comparison: Option<Element> = compared.map(|pair| {
            ComparisonHeader::new(pair.base, pair.tip)
                .on_swap(move |()| selection::swap(view, swapping.as_deref()))
                .into()
        });
        let reading = if comparison.is_some() {
            READING_COMPARISON
        } else {
            READING
        };
        let details = {
            let diff = view.diff.read();
            let Some((_, answer)) = diff.changes().filter(|(asked, _)| *asked == of) else {
                return under(comparison, notice(reading, false));
            };
            match answer {
                Answer::Waiting => return under(comparison, notice(reading, false)),
                Answer::Failed(message) => return under(comparison, notice(message.clone(), true)),
                Answer::Ready(changes) => changes.details.clone(),
            }
        };

        let current = view.diff.read().file_index();
        let choosing = self.submit.clone();
        let list = ChangesList::new(
            view.diff.into_readable().map(answered_changes, |_| true),
            view.diff.into_readable().map(answered_files, |_| true),
            view.filter_text,
        )
        .current(current)
        .on_file(move |index: usize| {
            diff_actions::choose_file(index, view, choosing.as_deref());
        });

        let mut width = view.changes_list_width;
        // Peeked: the share only matters when the split is laid out anew. Both panels are
        // proportional, so the list keeps its share as the window changes; each has a floor
        // in pixels that a drag and a narrowing window both honour.
        let list_share = *width.peek();
        let header: Option<Element> = match comparison {
            Some(comparison) => Some(comparison),
            None => details.map(|details| ChangesSummary::new(details).into()),
        };
        rect()
            .expanded()
            .content(Content::Flex)
            .maybe_child(header)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .on_sized(move |e: Event<SizedEventData>| {
                        split_width.set_if_modified(e.area.width())
                    })
                    .child(
                        ResizableContainer::new()
                            .direction(Direction::Horizontal)
                            .panel(
                                ResizablePanel::new(PanelSize::percent(list_share))
                                    .min_pixels(LIST_MIN_PIXELS)
                                    // The splitter reports the width dragged to in pixels; the
                                    // panels are laid out by share.
                                    .on_resized(move |dragged: f32| {
                                        if let Some(share) = share_of(dragged, *split_width.peek())
                                        {
                                            width.set(share);
                                        }
                                    })
                                    .child(list),
                            )
                            .panel(
                                ResizablePanel::new(PanelSize::percent(100. - list_share))
                                    .min_pixels(DIFF_MIN_PIXELS)
                                    .child(diff_side(view, self.submit.clone())),
                            ),
                    ),
            )
            .into()
    }
}

/// The file list's share of the split, in percent, once dragged to `list_px` in a split
/// `split_px` wide (its handle included): Freya's splitter reports the width dragged to in
/// pixels, while both panels are laid out by share. `None` while the split is unmeasured.
pub(crate) fn share_of(list_px: f32, split_px: f32) -> Option<f32> {
    let room = split_px - ResizableContext::HANDLE_SIZE;
    (room > 0. && list_px.is_finite()).then(|| (list_px / room * 100.).clamp(0., 100.))
}

/// What the tab draws in place of its list and diff — the answer awaited, or why it failed —
/// under the comparison's header when two commits are compared.
fn under(header: Option<Element>, body: Element) -> Element {
    match header {
        None => body,
        Some(header) => rect()
            .expanded()
            .content(Content::Flex)
            .child(header)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(body),
            )
            .into(),
    }
}

/// The file a diff is of, as the bar names it: the change set's own row for a committed
/// file, its path for a working-tree one.
fn file_of(target: &FileTarget) -> ChangedFile {
    match target {
        FileTarget::Committed { file, .. } => file.clone(),
        FileTarget::WorkingTree { path, .. } => ChangedFile {
            status: ChangeStatus::Modified,
            old_path: path.clone(),
            new_path: path.clone(),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        },
    }
}

/// The diff side: the chosen file's bar and its diff, for the answer naming that file at the
/// settings as they are, and only that.
fn diff_side(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Element {
    let state = view.diff.read();
    let Some((query, answer)) = state.file() else {
        return notice(NO_FILE_CHOSEN, false);
    };
    let settings = *view.diff_settings.read();
    let hiding = state.shown_file().is_some_and(ShownDiff::hides_changes);
    let acting = submit.clone();
    let header = DiffHeader::new(file_of(&query.target), settings)
        .hiding(hiding)
        .on_action(move |pressed| {
            shortcuts::act(shortcuts::of_header(pressed), view, acting.as_deref());
        });
    let body = match answer {
        Answer::Waiting => notice(READING_DIFF, false),
        Answer::Failed(message) => notice(message.clone(), true),
        Answer::Ready(None) => DiffNoticeView::new(DiffNotice::NothingShown {
            hides_changes: false,
        })
        .into(),
        Answer::Ready(Some(shown)) => diff_body(shown, view, submit, settings.side_by_side()),
    };
    rect()
        .expanded()
        .content(Content::Flex)
        .child(header)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .child(body),
        )
        .into()
}

/// One file's diff: its rows, or the notice that stands in their place (R6.8).
fn diff_body(
    shown: &ShownDiff,
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
    side_by_side: bool,
) -> Element {
    if let Some(stands_in) = DiffNotice::of(shown) {
        return DiffNoticeView::new(stands_in)
            .on_load(move |()| diff_actions::load_anyway(view, submit.as_deref()))
            .into();
    }
    let current = view.change_cursor.read().and_then(|cursor| {
        shown
            .stops(side_by_side)
            .and_then(|stops| stops.rows(cursor.change))
    });
    DiffView::new(
        view.diff.into_readable().map(answered_file, |_| true),
        view.diff_scroll,
    )
    .side_by_side(side_by_side)
    .current(current)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The splitter's pixels become a share of the room the two panels split, the handle
    /// left out; an unmeasured split, or a width that is not a number, keeps the share as
    /// it was; and no width is turned into a share past the whole. Caught by: the pixels
    /// stored as the share (the diff side laid out to nothing on the next commit).
    #[test]
    fn a_dragged_width_becomes_a_share_of_the_split() {
        let split = 800. + ResizableContext::HANDLE_SIZE;
        assert_eq!(share_of(280., split), Some(35.));
        assert_eq!(share_of(400., split), Some(50.));
        assert_eq!(share_of(0., split), Some(0.));
        assert_eq!(share_of(2000., split), Some(100.));
        assert_eq!(share_of(-5., split), Some(0.));
        assert_eq!(share_of(280., 0.), None);
        assert_eq!(share_of(280., ResizableContext::HANDLE_SIZE), None);
        assert_eq!(share_of(f32::NAN, split), None);
    }
}
