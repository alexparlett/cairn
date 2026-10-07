//! Request epochs, numbered per query lane, which double as the engine's cancel signal.
//!
//! Ten lanes (PRD R4.1, packet decision L8, phase 07's file filter, and refs-and-status
//! R11.1): the history, the walk its pages come from, the changes query, the file diff, the
//! Changes tab's filter over a change set's files, the refs snapshot, ahead/behind, the
//! working tree's status, the sidebar's filter over the refs and Local Changes' filter over
//! the status. A new query supersedes the
//! older ones in its own lane only, with one
//! exception — a changes query also supersedes the file-diff lane, since a file of the
//! commit that was selected is no file of the one that is now. So a scroll never cancels a
//! diff, a selection never cancels a scroll, and an operation, which is numbered in no
//! lane, supersedes nothing.
//!
//! The history has two numbers (refs-and-status R8.5). Its query lane is what a scroll, a
//! find in the sidebar and their stop are numbered in, each superseding the walking the last
//! one asked for. Its walk lane moves only when a walk is opened: every page of rows is
//! answered under the walk's number, never the query's, because a page a superseded find has
//! already laid out is still the next page of the same walk — dropping it would leave a hole
//! in the history the next page is appended after.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use cairn_git::Cancel;

/// A lane queries are numbered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QueryLane {
    /// Opening and paging the history walk, finding a row in it, and stopping a find.
    History,
    /// The walk the history's pages come from: moved by an open alone (`Request::OpenHistory`),
    /// so a page is dropped when the walk it belongs to was replaced, and never because a
    /// scroll or a find superseded the query that asked for it.
    Walk,
    /// What a commit, or a pair of commits, changed.
    Changes,
    /// One file's diff, or every file's (Expand All).
    FileDiff,
    /// Which of a change set's files a filter's text leaves (phase 07): a list operation,
    /// numbered so a keystroke supersedes the filter of the one before it, and nothing else.
    FileFilter,
    /// The refs snapshot a refresh reads, on the history thread (R11.2).
    Refs,
    /// How far each branch and its upstream have gone apart, for the snapshot a refresh read.
    AheadBehind,
    /// The working tree's status, as `git status` answers it. No refresh supersedes it (R10.3
    /// as amended, the user's decision of 2026-10-07): its number moves for nothing, so a
    /// status is cancelled only by a close, which stops every lane.
    Status,
    /// Which refs and stashes the sidebar's filter text leaves (R8.3).
    RefFilter,
    /// Which paths of Local Changes' two lists the view's filter text leaves (R9): numbered so
    /// a keystroke, or the lists of a status that has arrived, supersede the pass before.
    LocalChangesFilter,
}

impl QueryLane {
    /// Every lane, in the order of their counters.
    #[cfg(test)]
    pub const ALL: [Self; LANES] = [
        Self::History,
        Self::Walk,
        Self::Changes,
        Self::FileDiff,
        Self::FileFilter,
        Self::Refs,
        Self::AheadBehind,
        Self::Status,
        Self::RefFilter,
        Self::LocalChangesFilter,
    ];

    fn index(self) -> usize {
        match self {
            Self::History => 0,
            Self::Walk => 1,
            Self::Changes => 2,
            Self::FileDiff => 3,
            Self::FileFilter => 4,
            Self::Refs => 5,
            Self::AheadBehind => 6,
            Self::Status => 7,
            Self::RefFilter => 8,
            Self::LocalChangesFilter => 9,
        }
    }

    /// The lanes a new query in this one supersedes: its own, and for a changes query
    /// the file-diff lane too. Nothing else crosses.
    pub fn supersedes(self) -> &'static [Self] {
        match self {
            Self::History => &[Self::History],
            Self::Walk => &[Self::Walk],
            Self::Changes => &[Self::Changes, Self::FileDiff],
            Self::FileDiff => &[Self::FileDiff],
            Self::FileFilter => &[Self::FileFilter],
            Self::Refs => &[Self::Refs],
            Self::AheadBehind => &[Self::AheadBehind],
            Self::Status => &[Self::Status],
            Self::RefFilter => &[Self::RefFilter],
            Self::LocalChangesFilter => &[Self::LocalChangesFilter],
        }
    }
}

/// How many lanes there are: one counter each.
const LANES: usize = 10;

/// Which request a value belongs to: its lane, and its number there. Monotonic within a
/// lane, and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epoch {
    lane: QueryLane,
    number: u64,
}

/// The current epoch of every lane, shared by cloning.
#[derive(Debug, Clone, Default)]
pub struct Epochs {
    lanes: Arc<[AtomicU64; LANES]>,
    stopping: Arc<AtomicBool>,
}

impl Epochs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Supersedes whatever `lane` (and the lanes it supersedes) has in flight, and names
    /// the replacement. The lanes a changes query supersedes besides its own are bumped
    /// FIRST, so a file diff never sees its lane current under a newer changes query.
    pub fn bump(&self, lane: QueryLane) -> Epoch {
        for other in lane.supersedes() {
            if *other != lane {
                self.counter(*other).fetch_add(1, Ordering::AcqRel);
            }
        }
        Epoch {
            lane,
            number: self.counter(lane).fetch_add(1, Ordering::AcqRel) + 1,
        }
    }

    pub fn current(&self, lane: QueryLane) -> Epoch {
        Epoch {
            lane,
            number: self.counter(lane).load(Ordering::Acquire),
        }
    }

    pub fn is_current(&self, epoch: Epoch) -> bool {
        !self.is_stopping() && self.current(epoch.lane) == epoch
    }

    /// Stops everything, for good.
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
    }

    pub fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }

    pub fn watch(&self, mine: Epoch) -> Superseded {
        Superseded {
            mine,
            epochs: self.clone(),
        }
    }

    fn counter(&self, lane: QueryLane) -> &AtomicU64 {
        &self.lanes[lane.index()]
    }
}

/// The [`Cancel`] the engine polls: once per commit on a walk, and every runner tick while
/// a `git` read runs — so superseding a diff ends its process, it does not merely discard
/// its answer.
#[derive(Debug, Clone)]
pub struct Superseded {
    mine: Epoch,
    epochs: Epochs,
}

impl Cancel for Superseded {
    fn is_cancelled(&self) -> bool {
        !self.epochs.is_current(self.mine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epochs_rise_and_are_never_reused() {
        let epochs = Epochs::new();
        for lane in QueryLane::ALL {
            let before = epochs.current(lane);
            let first = epochs.bump(lane);
            let second = epochs.bump(lane);
            assert!(
                first > before,
                "{lane:?}: the first request did not advance"
            );
            assert!(
                second > first,
                "{lane:?}: {second:?} did not follow {first:?}"
            );
            assert_eq!(epochs.current(lane), second);
            assert!(
                !epochs.is_current(first),
                "{lane:?}: the superseded request was still reported current"
            );
        }
    }

    #[test]
    fn a_request_is_cancelled_by_the_one_that_supersedes_it() {
        let epochs = Epochs::new();
        for lane in QueryLane::ALL {
            let mine = epochs.bump(lane);
            let cancel = epochs.watch(mine);
            assert!(
                !cancel.is_cancelled(),
                "the current request reported itself abandoned"
            );
            epochs.bump(lane);
            assert!(
                cancel.is_cancelled(),
                "{lane:?}: superseding a request left its work running"
            );
        }
    }

    /// PRD R4.1, the whole table: each lane supersedes itself, a changes query the
    /// file-diff lane as well, and nothing else crosses. Caught by: one counter for every
    /// lane (a scroll cancels a diff), or a changes query that leaves the file diff of the
    /// previous commit current.
    #[test]
    fn each_lane_supersedes_itself_and_a_changes_query_the_file_diff_too() {
        for asked in QueryLane::ALL {
            let epochs = Epochs::new();
            let before: Vec<Epoch> = QueryLane::ALL
                .iter()
                .map(|lane| epochs.bump(*lane))
                .collect();
            epochs.bump(asked);
            for (lane, earlier) in QueryLane::ALL.iter().zip(before) {
                let superseded = !epochs.is_current(earlier);
                let expected = asked.supersedes().contains(lane);
                assert_eq!(
                    superseded,
                    expected,
                    "a {asked:?} query {} the {lane:?} lane",
                    if superseded {
                        "superseded"
                    } else {
                        "left current"
                    }
                );
            }
        }
        assert_eq!(
            QueryLane::Changes.supersedes(),
            [QueryLane::Changes, QueryLane::FileDiff]
        );
        assert_eq!(QueryLane::History.supersedes(), [QueryLane::History]);
        assert_eq!(QueryLane::FileDiff.supersedes(), [QueryLane::FileDiff]);
        assert_eq!(QueryLane::FileFilter.supersedes(), [QueryLane::FileFilter]);
        for lane in [
            QueryLane::Walk,
            QueryLane::Refs,
            QueryLane::AheadBehind,
            QueryLane::Status,
            QueryLane::RefFilter,
            QueryLane::LocalChangesFilter,
        ] {
            assert_eq!(
                lane.supersedes(),
                [lane],
                "{lane:?} crosses into another lane"
            );
        }
    }

    #[test]
    fn stopping_cancels_the_current_request_of_every_lane() {
        let epochs = Epochs::new();
        for lane in QueryLane::ALL {
            let mine = epochs.bump(lane);
            let cancel = epochs.watch(mine);
            epochs.stop();
            assert!(
                cancel.is_cancelled(),
                "{lane:?}: shutdown left work running"
            );
            assert!(
                !epochs.is_current(mine),
                "a stopping pool has no current work"
            );
        }
    }

    #[test]
    fn the_epoch_is_shared_across_threads() {
        let epochs = Epochs::new();
        let mine = epochs.bump(QueryLane::FileDiff);
        let cancel = epochs.watch(mine);
        let other = epochs.clone();
        std::thread::spawn(move || other.bump(QueryLane::Changes))
            .join()
            .unwrap();
        assert!(
            cancel.is_cancelled(),
            "a supersession from another thread was not visible"
        );
    }
}
