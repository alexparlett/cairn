//! What the detail pane describes: the row chosen in the history, a parent reached from the
//! Commit tab, or two commits to compare. On the UI thread; asking is a [`Request`] handed
//! to the caller's submit, never a wait.
//!
//! **Two commits**, as Fork compares them. A row pressed with the table's extending chord
//! (⌘-click, Ctrl-click) is compared with the row selected: exactly two, never three —
//! a third replaces the second, the row pressed plainly staying — and never half-selected: a
//! pressed row already in the pair leaves the other one selected alone, and a plain press
//! returns to one. The comparison is tip against tip, the lower row of the two in the list
//! the base, which the swap reverses. Each change asks what the selection now compares, so an
//! answer for one on its way when another was chosen is never drawn.

use cairn_model::{CommitSummary, History, HistoryRow, Oid, RowContent, RowId};
use freya::prelude::*;

use crate::diff::state::Answer;
use crate::window::View;
use crate::worker::{Comparison, Request};

/// What a row's changes are asked against. Named variant by variant: a row of another kind must say
/// here what it compares, or the window does not compile. A stash's row asks what `git stash show`
/// lists: the stash commit against the commit it was made on and, where the user's
/// `stash.showIncludeUntracked` says so, the untracked files it holds; its Commit tab draws the
/// stash commit's details, every parent among them.
pub fn comparison_of(id: RowId) -> Comparison {
    match id {
        RowId::Commit(oid) => Comparison::Commit(oid),
        RowId::Stash(stash) => Comparison::Stash(stash),
    }
}

/// Two rows selected to compare: the base — the lower of the two rows, the commit the
/// diff runs from — and the tip, each as the history drew it, for the header that names them,
/// and each row's identity, which a stash's row does not share with its commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub base: CommitSummary,
    pub tip: CommitSummary,
    pub base_row: RowId,
    pub tip_row: RowId,
}

impl Pair {
    /// What the pair compares: tip against tip, never against a merge base.
    pub fn comparison(&self) -> Comparison {
        Comparison::Between {
            old: self.base.id,
            new: self.tip.id,
        }
    }

    /// The row of the two that is not `one`.
    pub fn other(&self, one: Option<RowId>) -> Option<RowId> {
        let (base, tip) = (self.base_row, self.tip_row);
        match one {
            Some(id) if id == base => Some(tip),
            Some(id) if id == tip => Some(base),
            Some(_) | None => None,
        }
    }

    fn holds(&self, id: RowId) -> bool {
        id == self.base_row || id == self.tip_row
    }
}

/// What the selection compares now: the pair's comparison, or the one commit's.
pub fn selected_comparison(view: View) -> Option<Comparison> {
    match view.pair.read().as_ref() {
        Some(pair) => Some(pair.comparison()),
        None => (*view.selected.read()).map(comparison_of),
    }
}

/// The commit a loaded row draws, copied out of the history: a stash's row draws its stash
/// commit, its message the subject. Named variant by variant, as [`comparison_of`] is.
fn summary_of(row: HistoryRow<'_>) -> CommitSummary {
    match row.content() {
        RowContent::Commit(summary) => summary,
        RowContent::Stash(stash) => stash.as_commit(),
    }
}

/// Selects `id` alone and asks what it changed — through [`crate::diff::state::DiffState`], so
/// an answer for any other selection is never kept. Choosing what is already chosen asks
/// nothing new, unless its answer failed, when choosing it again is how to retry. A pair
/// selected goes: a plain press returns to one.
pub fn choose(id: RowId, view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut selected,
        mut pair,
        ..
    } = view;
    selected.set(Some(id));
    if pair.peek().is_some() {
        pair.set(None);
    }
    ask(comparison_of(id), view, submit);
}

/// The row `id`, at `index` in the loaded rows, pressed with the extending chord: it
/// and the row selected become the pair compared, the lower row the base; pressed while it is
/// one of a pair, the other stays selected alone. With nothing selected it is selected alone.
pub fn extend(id: RowId, index: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut selected,
        mut pair,
        rows,
        ..
    } = view;
    let Some(anchor) = *selected.peek() else {
        return choose(id, view, submit);
    };
    let paired = pair.peek().clone();
    if let Some(paired) = paired
        && paired.holds(id)
    {
        // Pressed again with the chord: let go of it, and the other is selected alone.
        let other = paired.other(Some(id)).unwrap_or(anchor);
        return choose(other, view, submit);
    }
    if id == anchor {
        // The one selected, pressed again with the chord: one stays one.
        return;
    }
    let made = {
        let loaded = rows.peek();
        match (loaded.position(anchor), loaded.row(index)) {
            (Some(anchor_at), Some(pressed)) if pressed.id() == id => {
                let (anchor_summary, pressed_summary) = match loaded.row(anchor_at) {
                    Some(row) => (summary_of(row), summary_of(pressed)),
                    None => return,
                };
                // The lower row in the list is the base.
                Some(if index > anchor_at {
                    Pair {
                        base: pressed_summary,
                        tip: anchor_summary,
                        base_row: id,
                        tip_row: anchor,
                    }
                } else {
                    Pair {
                        base: anchor_summary,
                        tip: pressed_summary,
                        base_row: anchor,
                        tip_row: id,
                    }
                })
            }
            _ => None,
        }
    };
    let Some(made) = made else {
        // The selection's row is not loaded: nothing to pair it with, so the press selects.
        return choose(id, view, submit);
    };
    let of = made.comparison();
    pair.set(Some(made));
    selected.set(Some(anchor));
    ask(of, view, submit);
}

/// The pair's base and tip swapped: the other diff, asked for.
pub fn swap(view: View, submit: Option<&dyn Fn(Request)>) {
    let mut pair = view.pair;
    let swapped = pair.peek().clone().map(|held| Pair {
        base: held.tip,
        tip: held.base,
        base_row: held.tip_row,
        tip_row: held.base_row,
    });
    let Some(swapped) = swapped else {
        return;
    };
    let of = swapped.comparison();
    pair.set(Some(swapped));
    ask(of, view, submit);
}

/// What the selection compares, asked again though its answer is kept: what git substitutes
/// as it reads a commit — a replace ref, a graft — changed, so the commit's details and what it
/// changed may no longer be the answer kept. While Local Changes holds the main region, whose
/// primary's diff holds the file-diff lane a changes query would take, the answer kept is
/// marked stale instead, and asked again when the history is shown ([`show_main`]).
pub fn ask_again(view: View, submit: Option<&dyn Fn(Request)>) {
    let mut diff = view.diff;
    if *view.sidebar.main.peek() == cairn_ui::MainView::LocalChanges {
        diff.write().mark_changes_stale();
        return;
    }
    let Some(of) = selected_comparison(view) else {
        return;
    };
    let requests = diff.write().select_changes(of);
    if let Some(submit) = submit {
        for request in requests {
            submit(request);
        }
    }
}

/// `chosen` shown in the main region; the history shown again asks the chosen commit's changes
/// again when what git substitutes changed while it was not ([`ask_again`]).
pub fn show_main(chosen: cairn_ui::MainView, view: View, submit: Option<&dyn Fn(Request)>) {
    let mut main = view.sidebar.main;
    main.set(chosen);
    if chosen != cairn_ui::MainView::LocalChanges && view.diff.peek().changes_stale() {
        ask_again(view, submit);
    }
}

/// Asks what `of` changed, unless it is what was asked last, its answer did not fail and what
/// git substitutes has not changed since ([`crate::diff::state::DiffState::changes_stale`]).
fn ask(of: Comparison, view: View, submit: Option<&dyn Fn(Request)>) {
    let mut diff = view.diff;
    let asked = !diff.peek().changes_stale()
        && diff
            .peek()
            .changes()
            .is_some_and(|(asked, answer)| asked == of && !matches!(answer, Answer::Failed(_)));
    if asked {
        return;
    }
    // The query, then the answers it replaces, which are freed on a worker.
    let requests = diff.write().select_changes(of);
    if let Some(submit) = submit {
        for request in requests {
            submit(request);
        }
    }
}

/// Where `parent` is in the loaded history, if it is loaded. A scan of what is loaded, run
/// once per press of a parent link and once per row pressed with the extending chord
/// ([`extend`], to find which of the two rows is lower), never per frame.
pub fn loaded_row(rows: &History, parent: Oid) -> Option<usize> {
    rows.position(RowId::Commit(parent))
}

#[cfg(test)]
mod tests {
    use cairn_model::{GraphRow, Lane, PagedCommit, RowsPage};

    use super::*;

    #[test]
    fn a_parent_is_found_where_it_is_loaded_and_nowhere_else() {
        let mut page = RowsPage::new();
        for n in 1..=5u8 {
            page.push(
                GraphRow::new(Oid::from_bytes(&[n; 20]).unwrap(), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents: 1,
                    subject: "commit",
                    author: "Ada",
                    author_time: 0,
                },
            );
        }
        let mut rows = History::new();
        rows.append(page).unwrap();
        assert_eq!(
            loaded_row(&rows, Oid::from_bytes(&[4; 20]).unwrap()),
            Some(3)
        );
        assert_eq!(loaded_row(&rows, Oid::from_bytes(&[9; 20]).unwrap()), None);
        assert_eq!(
            loaded_row(&History::new(), Oid::from_bytes(&[1; 20]).unwrap()),
            None
        );
    }
}
