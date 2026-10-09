//! The paths selected in one of Local Changes' lists, their diffs drawn together
//! (staging-and-commit R8.1, the user's decision of 2026-10-09: with several files selected the
//! diff draws the selected files' diffs together, as Fork does), laid out as the Commit tab's
//! files opened in place are ([`Expansion`]), each by its place among the paths.
//!
//! **One ask, one number.** The paths are asked together on the diff thread
//! (`Request::Together`), in the order the lists show them, and every page answering names the
//! ask's number: a page of another ask is never kept. The same paths asked again — a refresh,
//! the options moved — keep what they draw until their new answers come, as the path chosen
//! does; other paths are drawn afresh. The number is also what the staging gesture's selection
//! over these diffs belongs to (R9.2): a selection made under one ask is nothing under the
//! next.
//!
//! **One lane.** The paths share the file-diff lane with the path chosen, the commit's file and
//! the files opened in place. While paths are drawn together the path chosen is never asked —
//! it stays chosen, the anchor of the lists' presses — and once they are let go of it is asked
//! again as Local Changes is shown.

use std::sync::Arc;

use cairn_model::{ChangeList, RepoPath, ShownDiff};
use cairn_ui::{Expansion, Opened};

use super::{DiffState, retire, shown_of};
use crate::worker::{
    DiffOptions, Request, TogetherEnded, TogetherFile, TogetherOutcome, TogetherQuery, WorkingSide,
    together_diffs,
};

/// Said under a path drawn together that has no diff: a conflict's notice (R9.4).
pub const CONFLICTED_TOGETHER: &str = cairn_ui::CONFLICTED;
/// Said under a path drawn together that git prints nothing for.
pub const NOTHING_TOGETHER: &str = cairn_ui::NO_CHANGES_SHOWN;

/// Said under a path drawn together that the line budget left unread.
pub const NOT_READ_TOGETHER: &str =
    "Not shown: the files above it used the diff's line budget. Choose it alone to see its diff.";

fn not_read_notice() -> String {
    NOT_READ_TOGETHER.to_owned()
}

/// The paths drawn together, and what each draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Together {
    list: ChangeList,
    /// `LocalChangesState::serial` of the lists the paths were found in.
    lists: u64,
    /// Each path, in the lists' order, and the side it is asked as — `None` for a conflict.
    entries: Vec<(RepoPath, Option<WorkingSide>)>,
    paths: Vec<RepoPath>,
    query: TogetherQuery,
    shown: Expansion,
    /// The ask is the one in the file-diff lane now.
    pub(super) in_lane: bool,
    /// Every page of the ask has arrived.
    ended: bool,
}

/// What a person's selection asks to be drawn together: the list, the lists the paths are in,
/// each path and its side, and the options — compared with what is drawn to decide whether to
/// ask anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TogetherWanted {
    pub list: ChangeList,
    pub lists: u64,
    pub entries: Vec<(RepoPath, Option<WorkingSide>)>,
    pub options: DiffOptions,
}

impl DiffState {
    /// What is drawn together now, as [`TogetherWanted`] says it — `None` when nothing is.
    pub fn together_now(&self) -> Option<TogetherWanted> {
        self.together.as_ref().map(|together| TogetherWanted {
            list: together.list,
            lists: together.lists,
            entries: together.entries.clone(),
            options: together.query.options,
        })
    }

    /// Whether `wanted` is what is drawn together now, read without copying it.
    pub fn draws_together(&self, wanted: &TogetherWanted) -> bool {
        self.together.as_ref().is_some_and(|together| {
            together.list == wanted.list
                && together.lists == wanted.lists
                && together.query.options == wanted.options
                && together.entries == wanted.entries
        })
    }

    /// The paths drawn together, and the list they are of.
    pub fn together(&self) -> Option<(ChangeList, &[RepoPath])> {
        self.together
            .as_ref()
            .map(|together| (together.list, together.paths.as_slice()))
    }

    /// The number of the ask drawn together: what a selection over its rows belongs to.
    pub fn together_drawn(&self) -> Option<u64> {
        self.together.as_ref().map(|together| together.query.asked)
    }

    /// The diff of the path at `file` among the paths drawn together, when it has arrived.
    pub fn together_diff(&self, file: usize) -> Option<&ShownDiff> {
        match self.together.as_ref()?.shown.get(file)? {
            Opened::Shown(shown) => Some(shown),
            Opened::Reading | Opened::Failed(_) => None,
        }
    }

    /// Draws `wanted` together, asking its paths' diffs. The same paths in the same list keep
    /// what they draw until their answers come; other paths start afresh. Returns what to
    /// submit: the ask, then what was let go of, for a worker to free.
    pub fn show_together(&mut self, wanted: TogetherWanted) -> Vec<Request> {
        self.together_asks = self.together_asks.wrapping_add(1).max(1);
        let asked = self.together_asks;
        let files: Vec<TogetherFile> = wanted
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, (path, side))| {
                side.map(|side| TogetherFile {
                    index,
                    path: path.clone(),
                    side,
                })
            })
            .collect();
        let query = TogetherQuery {
            asked,
            files: Arc::new(files),
            options: wanted.options,
        };
        let same_paths = self.together.as_ref().is_some_and(|together| {
            together.list == wanted.list
                && together.entries == wanted.entries
                && together.query.options == wanted.options
        });
        let mut freed = Vec::new();
        let shown = match (same_paths, self.together.take()) {
            (true, Some(kept)) => kept.shown,
            (_, replaced) => {
                if let Some(mut replaced) = replaced {
                    freed.extend(shown_of(replaced.shown.close_all()));
                }
                let mut shown = Expansion::new();
                shown.set(wanted.entries.iter().enumerate().map(|(index, (_, side))| {
                    (
                        index,
                        match side {
                            Some(_) => Opened::Reading,
                            None => Opened::Failed(CONFLICTED_TOGETHER.to_owned()),
                        },
                    )
                }));
                shown
            }
        };
        let paths = wanted
            .entries
            .iter()
            .map(|(path, _)| path.clone())
            .collect();
        self.together = Some(Together {
            list: wanted.list,
            lists: wanted.lists,
            entries: wanted.entries,
            paths,
            query: query.clone(),
            shown,
            in_lane: true,
            ended: false,
        });
        self.took_lane_for_together();
        let mut requests = vec![Request::Together(query)];
        requests.extend(retire(freed));
        requests
    }

    /// Nothing drawn together any more — one path, or none, is selected. Returns what was
    /// drawn, for a worker to free; the path chosen is asked again as Local Changes is shown.
    pub fn let_go_of_together(&mut self) -> Vec<Request> {
        let Some(mut together) = self.together.take() else {
            return Vec::new();
        };
        retire(shown_of(together.shown.close_all()))
            .into_iter()
            .collect()
    }

    /// The paths drawn together are the file-diff lane's: the others are asked again when
    /// their view is shown.
    fn took_lane_for_together(&mut self) {
        self.working_in_lane = false;
        self.file_in_lane = false;
        if let Some(opening) = &mut self.opening {
            opening.in_lane = false;
        }
    }

    /// The ask in the file-diff lane is no longer the paths drawn together's.
    pub(super) fn together_lost_lane(&mut self) {
        if let Some(together) = &mut self.together {
            together.in_lane = false;
        }
    }

    /// Whether the paths drawn together are awaited and their ask lost the lane.
    pub fn together_needs_asking(&self) -> bool {
        self.together
            .as_ref()
            .is_some_and(|together| !together.in_lane && !together.ended)
    }

    /// The paths drawn together asked again, under a new number, when their ask lost the lane
    /// before every page came.
    pub fn reask_together(&mut self) -> Vec<Request> {
        if !self.together_needs_asking() {
            return Vec::new();
        }
        let Some(wanted) = self.together_now() else {
            return Vec::new();
        };
        self.show_together(wanted)
    }

    /// Whether a page answering the ask numbered `asked` is for what is drawn now.
    pub fn wants_together(&self, asked: u64) -> bool {
        self.together
            .as_ref()
            .is_some_and(|together| together.query.asked == asked)
    }

    /// A page of the paths drawn together arrived, for the ask drawn now: each path's answer
    /// kept by its place, and — when the page ends the ask — the paths the budget left unread
    /// said so. Returns what was replaced or not kept, for a worker to free.
    pub fn together_arrived(
        &mut self,
        asked: u64,
        files: Vec<(usize, TogetherOutcome)>,
        ended: Option<TogetherEnded>,
    ) -> Option<Request> {
        let Some(together) = self
            .together
            .as_mut()
            .filter(|together| together.query.asked == asked)
        else {
            return retire(together_diffs(files));
        };
        let kept = files.into_iter().map(|(index, outcome)| {
            (
                index,
                match outcome {
                    Ok(Some(shown)) => Opened::Shown(shown),
                    Ok(None) => Opened::Failed(NOTHING_TOGETHER.to_owned()),
                    Err(message) => Opened::Failed(message),
                },
            )
        });
        let mut freed = shown_of(together.shown.set(kept));
        if let Some(ended) = ended {
            together.ended = true;
            if let TogetherEnded::Budget { next } = ended {
                let unread: Vec<usize> = together
                    .query
                    .files
                    .iter()
                    .skip(next)
                    .map(|file| file.index)
                    .collect();
                freed.extend(shown_of(
                    together.shown.set(
                        unread
                            .into_iter()
                            .map(|index| (index, Opened::Failed(not_read_notice()))),
                    ),
                ));
            }
        }
        retire(freed)
    }

    /// The ask failed as a whole — the configuration git refuses: every path still awaited
    /// says why.
    pub(super) fn together_failed(&mut self, message: &str) {
        if let Some(together) = &mut self.together {
            let awaited: Vec<usize> = together
                .shown
                .iter()
                .filter(|(_, opened)| matches!(opened, Opened::Reading))
                .map(|(index, _)| index)
                .collect();
            together.shown.set(
                awaited
                    .into_iter()
                    .map(|index| (index, Opened::Failed(message.to_owned()))),
            );
            together.ended = true;
        }
    }

    /// The options moved: the paths drawn together are asked again at them, at once when
    /// `ask` — Local Changes is shown — and as it is shown otherwise. Returns the ask, if made,
    /// and what was drawn at the old options, for a worker to free.
    pub(super) fn together_settings_changed(
        &mut self,
        options: DiffOptions,
        ask: bool,
    ) -> (Vec<Request>, Vec<ShownDiff>) {
        let Some(together) = &mut self.together else {
            return (Vec::new(), Vec::new());
        };
        if together.query.options == options {
            return (Vec::new(), Vec::new());
        }
        let wanted = TogetherWanted {
            list: together.list,
            lists: together.lists,
            entries: together.entries.clone(),
            options,
        };
        if ask {
            return (self.show_together(wanted), Vec::new());
        }
        let freed = shown_of(together.shown.close_all());
        together.query.options = options;
        together.shown.set(
            together
                .entries
                .iter()
                .enumerate()
                .map(|(index, (_, side))| {
                    (
                        index,
                        match side {
                            Some(_) => Opened::Reading,
                            None => Opened::Failed(CONFLICTED_TOGETHER.to_owned()),
                        },
                    )
                }),
        );
        together.in_lane = false;
        together.ended = false;
        (Vec::new(), freed)
    }
}

/// What [`answered_together`] hands back while nothing is drawn together.
static NOTHING_TOGETHER_SHOWN: Expansion = Expansion::new();
/// What [`answered_together_paths`] hands back while nothing is drawn together.
static NO_PATHS: Vec<RepoPath> = Vec::new();

/// What each path drawn together draws: the view of the state the stacked view is handed.
pub fn answered_together(state: &DiffState) -> &Expansion {
    state
        .together
        .as_ref()
        .map_or(&NOTHING_TOGETHER_SHOWN, |together| &together.shown)
}

/// The paths drawn together, in the lists' order.
pub fn answered_together_paths(state: &DiffState) -> &Vec<RepoPath> {
    state
        .together
        .as_ref()
        .map_or(&NO_PATHS, |together| &together.paths)
}

#[cfg(test)]
mod tests {
    use cairn_model::{
        ChangeStatus, ChangedFile, ChangedRange, Context, DiffContent, DiffLine, DisplayOverlay,
        FileDiff, LineSpan, TextDiff,
    };

    use super::*;

    fn wanted(paths: &[&str], lists: u64) -> TogetherWanted {
        TogetherWanted {
            list: ChangeList::Unstaged,
            lists,
            entries: paths
                .iter()
                .map(|path| {
                    let side = (*path != "clash.rs").then_some(WorkingSide::Unstaged);
                    (RepoPath::from(*path), side)
                })
                .collect(),
            options: DiffOptions::default(),
        }
    }

    fn shown(path: &str) -> Box<ShownDiff> {
        Box::new(ShownDiff::new(
            FileDiff {
                file: ChangedFile {
                    status: ChangeStatus::Modified,
                    old_path: RepoPath::from(path),
                    new_path: RepoPath::from(path),
                    old_mode: None,
                    new_mode: None,
                    old_id: None,
                    new_id: None,
                },
                content: DiffContent::Text {
                    text: TextDiff::new(
                        Vec::new(),
                        vec![DiffLine::terminated("x")],
                        vec![ChangedRange::new(LineSpan::at(0, 0), LineSpan::at(0, 1))],
                    ),
                    overlay: DisplayOverlay::none(),
                },
            },
            Context::default(),
        ))
    }

    fn asked(requests: &[Request]) -> Option<TogetherQuery> {
        requests.iter().find_map(|request| match request {
            Request::Together(query) => Some(query.clone()),
            _ => None,
        })
    }

    /// The paths are asked together, a conflicted one left out and drawn as its notice; a page
    /// for the ask drawn now is kept by place, one for an earlier ask is not; a budget's end
    /// says the paths left unread. Caught by: a page of another selection drawn, a conflict
    /// asked, or unread paths drawn as reading for good.
    #[test]
    fn the_paths_drawn_together_keep_only_their_own_asks_pages() {
        let mut state = DiffState::default();
        let first =
            asked(&state.show_together(wanted(&["a.rs", "clash.rs", "b.rs"], 1))).expect("an ask");
        let indices: Vec<usize> = first.files.iter().map(|file| file.index).collect();
        assert_eq!(indices, [0, 2], "the conflict was asked");
        assert_eq!(
            answered_together(&state).get(1),
            Some(&Opened::Failed(CONFLICTED_TOGETHER.to_owned()))
        );
        let second =
            asked(&state.show_together(wanted(&["a.rs", "b.rs", "c.rs"], 1))).expect("another ask");
        assert!(
            state
                .together_arrived(first.asked, vec![(0, Ok(Some(shown("a.rs"))))], None)
                .is_some()
        );
        assert_eq!(answered_together(&state).get(0), Some(&Opened::Reading));
        state.together_arrived(second.asked, vec![(0, Ok(Some(shown("a.rs"))))], None);
        assert!(state.together_diff(0).is_some());
        state.together_arrived(
            second.asked,
            vec![(1, Err("unreadable".to_owned()))],
            Some(TogetherEnded::Budget { next: 2 }),
        );
        assert_eq!(
            answered_together(&state).get(1),
            Some(&Opened::Failed("unreadable".to_owned()))
        );
        assert_eq!(
            answered_together(&state).get(2),
            Some(&Opened::Failed(not_read_notice()))
        );
        assert!(!state.together_needs_asking());
    }

    /// The same paths asked again keep what they draw until their answers come, under a new
    /// number; the lane lost before every page came asks them again; letting go frees what was
    /// drawn. Caught by: a refresh that blanks the files, or an ask lost for good.
    #[test]
    fn the_same_paths_asked_again_keep_their_diffs_meanwhile() {
        let mut state = DiffState::default();
        let first = asked(&state.show_together(wanted(&["a.rs", "b.rs"], 1))).expect("an ask");
        state.together_arrived(
            first.asked,
            vec![(0, Ok(Some(shown("a.rs")))), (1, Ok(Some(shown("b.rs"))))],
            Some(TogetherEnded::Every),
        );
        let again = asked(&state.show_together(wanted(&["a.rs", "b.rs"], 2))).expect("asked again");
        assert_ne!(again.asked, first.asked);
        assert!(
            state.together_diff(1).is_some(),
            "the refresh blanked the files"
        );
        state.together_lost_lane();
        assert!(state.together_needs_asking());
        let reasked = asked(&state.reask_together()).expect("asked again");
        assert_ne!(reasked.asked, again.asked);
        let freed = state.let_go_of_together();
        assert!(
            matches!(freed.as_slice(), [Request::Retire(retired)] if retired.shown().len() == 2)
        );
        assert!(state.together().is_none());
    }
}
