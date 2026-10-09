//! View state for diffs: what the window has selected — a commit or a pair, a file, Expand
//! All — and the answer it keeps for that selection alone (PRD R4.4). On the UI thread, so
//! nothing here waits; asking is the caller's, through the [`Request`] each selection
//! returns.
//!
//! Two filters stand between a worker's answer and the window. The epoch drops an answer
//! whose query was superseded ([`crate::worker::Updates`]); this one drops an answer that
//! names anything but what is selected now — the selection cleared, or changed in a way
//! that asked nothing new — so the files of one commit are never drawn under another.

use std::collections::BTreeSet;
use std::sync::{Arc, LazyLock};

use cairn_model::ShownDiff;
use cairn_model::{
    ChangeSet, ChangeStatus, ChangedFile, Context, DiffContent, FileDiff, RenameDetection, RepoPath,
};
use cairn_ui::{Expansion, Opened, ShownFiles};

use crate::file_filter::FileFilter;
use crate::worker::{
    AllEnded, AllFrom, AllProgress, Comparison, DiffOptions, DiffQuery, ExpandQuery, ExpandedFile,
    FileQuery, FileTarget, OpenedFile, Request, Retired,
};

mod together;
mod working;

pub use together::{TogetherWanted, answered_together, answered_together_paths};
pub use working::{WorkingChoice, WorkingShown};

/// An answer the window is waiting for, has, or was told failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer<T> {
    Waiting,
    Ready(T),
    /// Display text.
    Failed(String),
}

/// Where Expand All stands, as the window keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllState {
    /// Not pressed, or collapsed again.
    Off,
    /// On its way, from where it stands — taken up from there if it is superseded.
    Running(AllFrom),
    /// Every file opened.
    Done,
    /// Its line budget spent: the files it did not reach stay collapsed.
    Stopped,
}

/// The files of the selected change set opened in place in the Commit tab (R5.3): what each
/// draws, the ones read past R2.6's limits, where Expand All stands, and the options every
/// file is asked at.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Opening {
    of: Comparison,
    /// The view's context and whitespace setting; never `load_anyway`, which is per file.
    options: DiffOptions,
    shown: Expansion,
    loaded: BTreeSet<usize>,
    all: AllState,
    /// The request for what is unanswered is the one in the file-diff lane now. The Changes
    /// tab's file shares the lane, so asking for it takes the lane from this, and the other
    /// way round; whichever lost it asks again when its tab is shown.
    in_lane: bool,
}

/// Which of the two selections sharing the file-diff lane a change of settings asks for at
/// once: the one whose tab is shown. The other is asked when its tab is shown again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asking {
    File,
    Expansion,
    /// The path chosen in Local Changes, whose view is shown in place of the history.
    Working,
}

/// The diffs `opened` holds, for a worker to free.
fn shown_of(opened: impl IntoIterator<Item = Opened>) -> Vec<ShownDiff> {
    opened
        .into_iter()
        .filter_map(|opened| match opened {
            Opened::Shown(shown) => Some(*shown),
            Opened::Reading | Opened::Failed(_) => None,
        })
        .collect()
}

fn retire(shown: Vec<ShownDiff>) -> Option<Request> {
    Retired::of(None, shown).map(Request::Retire)
}

/// The diff selection and its answers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffState {
    /// The commit or the pair selected, and what it changed — shared, so the Changes tab's
    /// filter can read it on a worker (phase 07) without a copy.
    changes: Option<(Comparison, Answer<Arc<ChangeSet>>)>,
    /// The file selected, and its diff as the view draws it, prepared on the worker that
    /// answered it — `None` inside `Ready` for a clean working-tree path.
    file: Option<(FileQuery, Answer<Option<ShownDiff>>)>,
    /// The files opened in place in the Commit tab, for the comparison selected.
    opening: Option<Opening>,
    /// The file selected's request is the one in the file-diff lane now (see `Opening`).
    file_in_lane: bool,
    /// The Changes tab's filter over the selected change set's files (phase 07).
    filter: FileFilter,
    /// Where the file selected is in the change set, when it was chosen from it: what the
    /// Changes tab's list highlights and moves from, kept rather than searched for.
    file_index: Option<usize>,
    /// The path chosen in Local Changes and its diff (refs-and-status R9.3).
    working: Option<working::Working>,
    /// The path chosen's request is the one in the file-diff lane now.
    working_in_lane: bool,
    /// How many working-tree answers have been kept: each one kept is numbered by it.
    answers: u64,
    /// The paths selected in one of Local Changes' lists, drawn together (R8.1).
    together: Option<together::Together>,
    /// How many times paths have been asked together: each ask is numbered by it.
    together_asks: u64,
}

impl DiffState {
    /// Selects a commit or a pair: its change set is awaited, and the file and Expand All
    /// that were selected go — the changes query supersedes the file-diff lane, so their
    /// answers will not come, and they were of another commit anyway.
    ///
    /// Returns what to submit, in order: the query, then — when answers were kept — a
    /// [`Request::Retire`] handing them to a worker, so a large change set is not freed on
    /// the UI thread (R2: 1.2-2.0 ms for 55,184 files).
    pub fn select_changes(&mut self, of: Comparison) -> Vec<Request> {
        let changes = self.changes.replace((of, Answer::Waiting));
        let file = self.file.take();
        let opening = self.opening.take();
        self.file_index = None;
        self.file_in_lane = false;
        // The changes query supersedes the file-diff lane, whoever's request is in it.
        self.working_in_lane = false;
        self.together_lost_lane();
        self.filter.changes_selected();
        let mut shown_diffs: Vec<ShownDiff> = Vec::new();
        if let Some((_, Answer::Ready(Some(shown)))) = file {
            shown_diffs.push(shown);
        }
        if let Some(mut opening) = opening {
            shown_diffs.extend(shown_of(opening.shown.close_all()));
        }
        let changes = changes.and_then(|(_, answer)| match answer {
            Answer::Ready(changes) => Some(changes),
            Answer::Waiting | Answer::Failed(_) => None,
        });
        let mut requests = vec![Request::Changes { of }];
        requests.extend(Retired::of(changes, shown_diffs).map(Request::Retire));
        requests
    }

    /// Selects one file's diff — another file, or the same one at other options. Expand All
    /// goes: it shares the file-diff lane.
    ///
    /// Returns what to submit, in order: the query, then — when a diff was kept — a
    /// [`Request::Retire`] handing it to a worker, as [`Self::select_changes`] does: a file
    /// of fifty thousand lines is as many allocations to free.
    pub fn select_file(&mut self, query: FileQuery) -> Vec<Request> {
        let replaced = self.file.replace((query.clone(), Answer::Waiting));
        self.took_lane_for_file();
        let mut requests = vec![Request::FileDiff(query)];
        if let Some((_, Answer::Ready(Some(shown)))) = replaced {
            requests.extend(retire(vec![shown]));
        }
        requests
    }

    /// The file selected's request is now the one in the file-diff lane; whatever the
    /// expansion asked is superseded, and asked again when the Commit tab is shown.
    fn took_lane_for_file(&mut self) {
        self.file_in_lane = true;
        self.working_in_lane = false;
        self.together_lost_lane();
        if let Some(opening) = &mut self.opening {
            opening.in_lane = false;
        }
    }

    /// Whether the file selected is awaited and its request lost the lane: what the Changes
    /// tab checks, reading only, before it asks again.
    pub fn file_needs_asking(&self) -> bool {
        !self.file_in_lane && matches!(self.file, Some((_, Answer::Waiting)))
    }

    /// Whether files opened in place are awaited and their request lost the lane.
    pub fn expansion_needs_asking(&self) -> bool {
        self.opening.as_ref().is_some_and(|opening| {
            !opening.in_lane
                && (matches!(opening.all, AllState::Running(_))
                    || opening
                        .shown
                        .iter()
                        .any(|(_, opened)| matches!(opened, Opened::Reading)))
        })
    }

    /// The file selected asked again when its answer is still awaited and its request lost
    /// the lane to the files opened in place: what the Changes tab does as it is shown.
    pub fn reask_file(&mut self) -> Option<Request> {
        let query = match &self.file {
            Some((query, Answer::Waiting)) if !self.file_in_lane => query.clone(),
            Some(_) | None => return None,
        };
        self.took_lane_for_file();
        Some(Request::FileDiff(query))
    }

    /// Nothing selected: whatever is still on its way is not drawn.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "nothing clears the selection until a row can be unselected"
        )
    )]
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn changes(&self) -> Option<(Comparison, &Answer<Arc<ChangeSet>>)> {
        self.changes.as_ref().map(|(of, answer)| (*of, answer))
    }

    /// Records that the file selected is the change set's file `index`.
    pub fn chose_file_at(&mut self, index: usize) {
        self.file_index = Some(index);
    }

    /// Where the file selected is in the change set, if it was chosen from it.
    pub fn file_index(&self) -> Option<usize> {
        self.file.as_ref().and(self.file_index)
    }

    pub fn file(&self) -> Option<(&FileQuery, &Answer<Option<ShownDiff>>)> {
        self.file.as_ref().map(|(query, answer)| (query, answer))
    }

    /// The selected file's diff, when it has arrived and there is one.
    pub fn shown_file(&self) -> Option<&ShownDiff> {
        match self.file.as_ref() {
            Some((_, Answer::Ready(Some(shown)))) => Some(shown),
            Some((_, Answer::Ready(None) | Answer::Waiting | Answer::Failed(_))) | None => None,
        }
    }

    /// Where Expand All stands for the change set selected, if anything is open or asked.
    #[cfg(test)]
    pub fn all_state(&self) -> Option<AllState> {
        self.opening.as_ref().map(|opening| opening.all)
    }
}

/// The Commit tab's files opened in place (phase 08, R5.3).
impl DiffState {
    /// The opening for the change set selected now, made at `options` if there is none; `None`
    /// while no change set is here to name files of.
    fn opening_now(&mut self, options: DiffOptions) -> Option<&mut Opening> {
        let of = match &self.changes {
            Some((of, Answer::Ready(_))) => *of,
            Some((_, Answer::Waiting | Answer::Failed(_))) | None => return None,
        };
        let opening = self.opening.get_or_insert_with(|| Opening {
            of,
            options: DiffOptions {
                load_anyway: false,
                ..options
            },
            shown: Expansion::new(),
            loaded: BTreeSet::new(),
            all: AllState::Off,
            in_lane: false,
        });
        Some(opening)
    }

    /// Opens the change set's file at `index` in place, or closes it when it is open (Fork,
    /// Finding 4). Returns what to submit: the request for what is unanswered when a file
    /// opened, and a closed file's diff handed to a worker to free.
    pub fn toggle_file(&mut self, index: usize, options: DiffOptions) -> Vec<Request> {
        let Some(opening) = self.opening_now(options) else {
            return Vec::new();
        };
        if opening.shown.is_open(index) {
            opening.loaded.remove(&index);
            let closed = opening.shown.close([index]);
            return retire(shown_of(closed)).into_iter().collect();
        }
        opening.shown.set([(index, Opened::Reading)]);
        self.ask_expansion().into_iter().collect()
    }

    /// Expand All (R5.3): every file opened in the change set's order, from the first, until
    /// its line budget is spent. Files already open are kept as they are drawn and passed over:
    /// only the rest are read (the user's decision, 2026-10-04).
    pub fn expand_all(&mut self, options: DiffOptions) -> Vec<Request> {
        let Some(opening) = self.opening_now(options) else {
            return Vec::new();
        };
        opening.all = AllState::Running(AllFrom::default());
        opening.shown.set_stopped_at_budget(false);
        self.ask_expansion().into_iter().collect()
    }

    /// Collapse All: every file closed and Expand All stopped. Returns a request that names
    /// nothing to read — which supersedes whatever the lane holds for them, ending its `git`
    /// — and the diffs drawn, for a worker to free.
    pub fn collapse_all(&mut self) -> Vec<Request> {
        let Some(opening) = self.opening.as_mut() else {
            return Vec::new();
        };
        let closed = opening.shown.close_all();
        opening.loaded.clear();
        opening.all = AllState::Off;
        let mut requests = Vec::new();
        if opening.in_lane
            && let Some(Answer::Ready(changes)) = self.changes.as_ref().map(|(_, answer)| answer)
        {
            requests.push(Request::Expand(ExpandQuery {
                of: opening.of,
                changes: changes.clone(),
                options: opening.options,
                files: Vec::new(),
                all: None,
                kept_open: Vec::new(),
            }));
            opening.in_lane = false;
        }
        requests.extend(retire(shown_of(closed)));
        requests
    }

    /// Load Diff under the file opened in place at `index` (R6.8): it is read again past
    /// R2.6's limits, up to the load-anyway ceiling.
    pub fn load_in_place(&mut self, index: usize) -> Vec<Request> {
        let Some(opening) = self.opening.as_mut() else {
            return Vec::new();
        };
        if !opening.shown.is_open(index) || !opening.loaded.insert(index) {
            return Vec::new();
        }
        let replaced = opening.shown.set([(index, Opened::Reading)]);
        let mut requests: Vec<Request> = self.ask_expansion().into_iter().collect();
        requests.extend(retire(shown_of(replaced)));
        requests
    }

    /// The files opened in place asked again when something is unanswered and their request
    /// lost the lane to the Changes tab's file: what the Commit tab does as it is shown.
    pub fn reask_expansion(&mut self) -> Option<Request> {
        if self.opening.as_ref().is_none_or(|opening| opening.in_lane) {
            return None;
        }
        self.ask_expansion()
    }

    /// The request for every file opened by name that is not answered, and for Expand All
    /// from where it stands; `None` when nothing is awaited. Takes the lane from the file.
    fn ask_expansion(&mut self) -> Option<Request> {
        let changes = match &self.changes {
            Some((_, Answer::Ready(changes))) => changes.clone(),
            Some((_, Answer::Waiting | Answer::Failed(_))) | None => return None,
        };
        let opening = self.opening.as_mut()?;
        let files: Vec<OpenedFile> = opening
            .shown
            .iter()
            .filter(|(_, opened)| matches!(opened, Opened::Reading))
            .map(|(index, _)| OpenedFile {
                index,
                load_anyway: opening.loaded.contains(&index),
            })
            .collect();
        let all = match opening.all {
            AllState::Running(from) => Some(from),
            AllState::Off | AllState::Done | AllState::Stopped => None,
        };
        if files.is_empty() && all.is_none() {
            return None;
        }
        // Expand All passes over every file open, whatever it draws: it is kept, never read
        // again (the user's decision, 2026-10-04).
        let kept_open: Vec<usize> = match all {
            Some(_) => opening.shown.iter().map(|(index, _)| index).collect(),
            None => Vec::new(),
        };
        opening.in_lane = true;
        self.file_in_lane = false;
        self.working_in_lane = false;
        if let Some(together) = &mut self.together {
            together.in_lane = false;
        }
        Some(Request::Expand(ExpandQuery {
            of: opening.of,
            changes,
            options: opening.options,
            files,
            all,
            kept_open,
        }))
    }

    /// The settings moved: the file selected is asked again at `options` and the files opened
    /// in place at `in_place` — the Changes tab's own entire file is not theirs — each only
    /// when its options changed, the one `asking` names now, the other as soon as its tab is
    /// shown, since the two share the file-diff lane. A file read past the limits stays so.
    /// Returns what to submit: the request, and the replaced diffs for a worker to free.
    pub fn settings_changed(
        &mut self,
        options: DiffOptions,
        in_place: DiffOptions,
        working: DiffOptions,
        asking: Asking,
    ) -> Vec<Request> {
        let mut requests = Vec::new();
        let mut file_asked = false;
        if let Some((query, answer)) = &mut self.file {
            let wanted = DiffOptions {
                load_anyway: query.options.load_anyway,
                ..options
            };
            if query.options != wanted {
                query.options = wanted;
                if let Answer::Ready(Some(shown)) = std::mem::replace(answer, Answer::Waiting) {
                    requests.extend(retire(vec![shown]));
                }
                self.file_in_lane = false;
                file_asked = true;
            }
        }
        let mut expansion_asked = false;
        if let Some(opening) = &mut self.opening {
            let wanted = DiffOptions {
                load_anyway: false,
                ..in_place
            };
            if opening.options != wanted {
                opening.options = wanted;
                let open: Vec<usize> = opening.shown.iter().map(|(index, _)| index).collect();
                let replaced = opening
                    .shown
                    .set(open.into_iter().map(|index| (index, Opened::Reading)));
                requests.extend(retire(shown_of(replaced)));
                opening.in_lane = false;
                expansion_asked = true;
            }
        }
        // The path chosen in Local Changes is asked at the Changes tab's options but always
        // exact (staging-and-commit R8.5): its view is one file's diff under the same bar.
        let (working_asked, freed) =
            self.working_settings_changed(working, asking == Asking::Working);
        requests.extend(retire(freed));
        // The paths drawn together likewise, and before the path chosen, which is not asked
        // while they are drawn.
        let (together_asked, freed) =
            self.together_settings_changed(working, asking == Asking::Working);
        requests.extend(retire(freed));
        let asked = match asking {
            Asking::File if file_asked => self.reask_file(),
            Asking::Expansion if expansion_asked => self.reask_expansion(),
            Asking::Working => working_asked,
            Asking::File | Asking::Expansion => None,
        };
        let mut ordered: Vec<Request> = asked.into_iter().collect();
        ordered.extend(together_asked);
        ordered.extend(requests);
        ordered
    }

    /// A page of files opened in place arrived for `of` at `options`, which the caller has
    /// checked are the opening's. Each file is kept when it is open and read as it is to be
    /// — past the limits or not — or when Expand All on its way read it; Expand All moves on,
    /// or ends. Returns the request to free what was not kept and what was replaced.
    pub fn expansion_arrived(
        &mut self,
        files: Vec<ExpandedFile>,
        all: Option<AllProgress>,
    ) -> Option<Request> {
        let Some(opening) = self.opening.as_mut() else {
            return retire(crate::worker::expanded_diffs(files));
        };
        let running = matches!(opening.all, AllState::Running(_));
        let mut kept = Vec::new();
        let mut dropped = Vec::new();
        for file in files {
            let index = file.file.index;
            let read_as_asked = opening.loaded.contains(&index) == file.file.load_anyway;
            let wanted = if opening.shown.is_open(index) {
                read_as_asked
            } else {
                file.by_all && running && read_as_asked
            };
            if !wanted {
                dropped.extend(file.outcome.ok().map(|shown| *shown));
                continue;
            }
            kept.push((
                index,
                match file.outcome {
                    Ok(shown) => Opened::Shown(shown),
                    Err(message) => Opened::Failed(message),
                },
            ));
        }
        let replaced = opening.shown.set(kept);
        dropped.extend(shown_of(replaced));
        if let (Some(progress), true) = (all, running) {
            opening.all = match progress.ended {
                None => AllState::Running(progress.at),
                Some(AllEnded::Every) => AllState::Done,
                Some(AllEnded::Budget) => AllState::Stopped,
            };
            opening
                .shown
                .set_stopped_at_budget(opening.all == AllState::Stopped);
        }
        retire(dropped)
    }
}

/// What the Changes tab's list shows of the selected change set: the view of the state the
/// list is handed.
pub fn answered_files(state: &DiffState) -> &ShownFiles {
    state.filter.shown()
}

/// What [`answered_changes`] hands back while no change set is kept: nothing changed, and no
/// commit.
static NO_CHANGES: ChangeSet = ChangeSet {
    files: Vec::new(),
    details: None,
    renames: RenameDetection {
        enabled: false,
        copies: false,
        limit: None,
        needed_limit: None,
    },
};

/// The change set kept for the selection, or an empty one while none is: the view of the
/// state the Commit tab is handed, which reads the files by index rather than copying them.
/// Whether the selection's answer is ready is the pane's to check before drawing it.
pub fn answered_changes(state: &DiffState) -> &ChangeSet {
    match &state.changes {
        Some((_, Answer::Ready(changes))) => changes.as_ref(),
        Some((_, Answer::Waiting | Answer::Failed(_))) | None => &NO_CHANGES,
    }
}

/// What [`answered_file`] hands back while no file's diff is kept: an empty one.
static NO_DIFF: LazyLock<ShownDiff> = LazyLock::new(|| {
    ShownDiff::new(
        FileDiff {
            file: ChangedFile {
                status: ChangeStatus::Modified,
                old_path: RepoPath::from(""),
                new_path: RepoPath::from(""),
                old_mode: None,
                new_mode: None,
                old_id: None,
                new_id: None,
            },
            content: DiffContent::ModeChangeOnly,
        },
        Context::default(),
    )
});

/// What [`answered_expansion`] hands back while nothing is open.
static NOTHING_OPEN: Expansion = Expansion::new();

/// The files opened in place for the change set selected, or none: the view of the state the
/// Commit tab is handed, which reads each file's rows by index rather than copying them.
pub fn answered_expansion(state: &DiffState) -> &Expansion {
    state
        .opening
        .as_ref()
        .map_or(&NOTHING_OPEN, |opening| &opening.shown)
}

/// The selected file's diff, or an empty one while none is kept: the view of the state the
/// diff view is handed, which reads its rows by index rather than copying them. Whether the
/// selection's answer is ready is the pane's to check before drawing it.
pub fn answered_file(state: &DiffState) -> &ShownDiff {
    state.shown_file().unwrap_or(&NO_DIFF)
}

/// The diff drawn for the path chosen in Local Changes, or an empty one while none is: the
/// view of the state Local Changes' diff view is handed. Whether there is one to draw is the
/// view's to check before drawing it.
pub fn answered_working(state: &DiffState) -> &ShownDiff {
    state.shown_working().unwrap_or(&NO_DIFF)
}

impl DiffState {
    /// The filter's text is now `text`: what to ask of a worker, if anything — nothing for
    /// the text already asked, an empty text, or a change set not yet here.
    pub fn filter(&mut self, text: &str) -> Option<Request> {
        let ready = match &self.changes {
            Some((of, Answer::Ready(changes))) => Some((*of, changes)),
            Some((_, Answer::Waiting | Answer::Failed(_))) | None => None,
        };
        self.filter.filter(text, ready)
    }

    /// The filter asked again with its text, for a change set that has just arrived.
    pub fn filter_again(&mut self) -> Option<Request> {
        let text = self.filter.text().to_owned();
        self.filter(&text)
    }

    pub fn filter_text(&self) -> &str {
        self.filter.text()
    }

    /// Whether the list shows the answer for the filter's text as it is.
    pub fn filter_is_settled(&self) -> bool {
        self.filter.is_settled()
    }

    pub fn wants_filter(&self, of: Comparison, text: &str) -> bool {
        self.wants_changes(of) && self.filter.wants(of, text)
    }

    /// A filter's answer arrived; kept only if it names the change set selected and the text
    /// asked last. Returns whether it was.
    pub fn filter_arrived(&mut self, of: Comparison, text: &str, files: Vec<u32>) -> bool {
        self.wants_changes(of) && self.filter.arrived(of, text, files)
    }

    /// Whether an answer to `asked` is for what is selected now.
    pub fn wants(&self, asked: &DiffQuery) -> bool {
        match asked {
            DiffQuery::Changes(of) => self.wants_changes(*of),
            DiffQuery::File(query) => self.wants_file(query),
            DiffQuery::Expand(asked) => self.wants_expansion(asked.of, asked.options),
            DiffQuery::Together(asked) => self.wants_together(asked.asked),
        }
    }

    pub fn wants_changes(&self, of: Comparison) -> bool {
        self.changes
            .as_ref()
            .is_some_and(|(selected, _)| *selected == of)
    }

    /// Whether an answer to `query` is for the file selected — the commit's, or the path
    /// chosen in Local Changes for a working-tree query.
    pub fn wants_file(&self, query: &FileQuery) -> bool {
        match &query.target {
            FileTarget::Committed { .. } => self
                .file
                .as_ref()
                .is_some_and(|(selected, _)| selected == query),
            FileTarget::WorkingTree { .. } => self.wants_working(query),
        }
    }

    pub fn wants_expansion(&self, of: Comparison, options: DiffOptions) -> bool {
        self.wants_changes(of)
            && self
                .opening
                .as_ref()
                .is_some_and(|opening| opening.of == of && opening.options == options)
    }

    /// `of`'s change set arrived; kept only if `of` is selected. Returns whether it was.
    pub fn changes_arrived(&mut self, of: Comparison, changes: ChangeSet) -> bool {
        match &mut self.changes {
            Some((selected, answer)) if *selected == of => {
                *answer = Answer::Ready(Arc::new(changes));
                true
            }
            Some(_) | None => false,
        }
    }

    /// A file's diff arrived, already prepared for the view on the worker that answered it;
    /// kept only if exactly `query` is selected. Returns whether it was.
    pub fn file_arrived(&mut self, query: &FileQuery, diff: Option<ShownDiff>) -> bool {
        match &mut self.file {
            Some((selected, answer)) if selected == query => {
                *answer = Answer::Ready(diff);
                true
            }
            Some(_) | None => false,
        }
    }

    /// `asked` failed; kept only if it is what is selected.
    pub fn failed(&mut self, asked: &DiffQuery, message: String) -> bool {
        if !self.wants(asked) {
            return false;
        }
        match asked {
            DiffQuery::Changes(_) => {
                if let Some((_, answer)) = &mut self.changes {
                    *answer = Answer::Failed(message);
                }
            }
            DiffQuery::File(query) => match &query.target {
                FileTarget::Committed { .. } => {
                    if let Some((_, answer)) = &mut self.file {
                        *answer = Answer::Failed(message);
                    }
                }
                FileTarget::WorkingTree { .. } => self.working_failed(message),
            },
            // What no file is to blame for — the configuration git refuses — fails every file
            // still awaited, and stops Expand All.
            DiffQuery::Expand(_) => {
                if let Some(opening) = &mut self.opening {
                    let awaited: Vec<usize> = opening
                        .shown
                        .iter()
                        .filter(|(_, opened)| matches!(opened, Opened::Reading))
                        .map(|(index, _)| index)
                        .collect();
                    opening.shown.set(
                        awaited
                            .into_iter()
                            .map(|index| (index, Opened::Failed(message.clone()))),
                    );
                    opening.all = AllState::Off;
                    opening.in_lane = false;
                }
            }
            DiffQuery::Together(_) => self.together_failed(&message),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use cairn_model::{ChangeStatus, ChangedFile, Oid, RenameDetection, RepoPath};

    use super::*;
    use crate::worker::{FileTarget, WorkingSide};

    fn commit(n: u8) -> Comparison {
        Comparison::Commit(Oid::from_bytes(&[n; 20]).unwrap())
    }

    fn change_set(path: &str) -> ChangeSet {
        ChangeSet {
            files: vec![ChangedFile {
                status: ChangeStatus::Added,
                old_path: RepoPath::from(path),
                new_path: RepoPath::from(path),
                old_mode: None,
                new_mode: Some(cairn_model::FileMode::Regular),
                old_id: None,
                new_id: Some(Oid::from_bytes(&[9; 20]).unwrap()),
            }],
            details: None,
            renames: RenameDetection::default(),
        }
    }

    fn file_of(of: Comparison, path: &str) -> FileQuery {
        FileQuery {
            target: FileTarget::Committed {
                of,
                file: change_set(path).files.remove(0),
            },
            options: DiffOptions::default(),
        }
    }

    /// R2: choosing another commit hands the change set kept for the last one — and its file
    /// and Expand All diffs — to a worker after the query, rather than dropping them on the
    /// UI thread; a selection with nothing ready retires nothing. Caught by: assigning the
    /// new selection over the old (which drops it here), or retiring before asking.
    #[test]
    fn choosing_another_commit_hands_the_last_ones_answers_to_a_worker() {
        let mut state = DiffState::default();
        assert_eq!(
            state.select_changes(commit(1)),
            [Request::Changes { of: commit(1) }],
            "nothing was kept, so nothing is retired"
        );
        assert!(state.changes_arrived(commit(1), change_set("one")));
        let requests = state.select_changes(commit(2));
        assert_eq!(requests.len(), 2, "{requests:?}");
        assert_eq!(requests[0], Request::Changes { of: commit(2) });
        match &requests[1] {
            Request::Retire(retired) => {
                assert_eq!(retired.changes(), Some(&change_set("one")));
            }
            other => panic!("the second request is not a retirement: {other:?}"),
        }
        assert_eq!(state.changes(), Some((commit(2), &Answer::Waiting)));

        // An answer still on its way, or one that failed, holds nothing to free.
        assert_eq!(
            state.select_changes(commit(3)),
            [Request::Changes { of: commit(3) }]
        );
    }

    /// R4.4: an answer naming another commit is never kept — not after the selection moved
    /// on, and not after it was cleared without asking anything new. Caught by: keeping
    /// whatever arrives (the previous commit's files drawn under the new one).
    #[test]
    fn a_change_set_naming_another_selection_is_never_kept() {
        let mut state = DiffState::default();
        assert_eq!(
            state.select_changes(commit(1)),
            [Request::Changes { of: commit(1) }]
        );
        state.select_changes(commit(2));
        assert!(!state.wants_changes(commit(1)));
        assert!(!state.wants(&DiffQuery::Changes(commit(1))));
        assert!(state.wants_changes(commit(2)));
        assert!(!state.changes_arrived(commit(1), change_set("one")));
        assert_eq!(state.changes(), Some((commit(2), &Answer::Waiting)));

        assert!(state.changes_arrived(commit(2), change_set("two")));
        assert_eq!(
            state.changes(),
            Some((commit(2), &Answer::Ready(Arc::new(change_set("two")))))
        );

        state.clear();
        assert!(!state.wants_changes(commit(2)));
        assert!(!state.changes_arrived(commit(2), change_set("two")));
        assert_eq!(state.changes(), None);
    }

    /// R4.4 for a file: the answer must name the file, its commit and its options. Caught
    /// by: comparing paths alone (the same path of another commit), or ignoring the options
    /// (an answer at the old context drawn after the context changed).
    #[test]
    fn a_file_diff_naming_another_file_commit_or_options_is_never_kept() {
        let mut state = DiffState::default();
        state.select_changes(commit(1));
        let selected = file_of(commit(1), "a.txt");
        state.select_file(selected.clone());

        let mut other_options = selected.clone();
        other_options.options.ignore_whitespace = true;
        for other in [
            file_of(commit(2), "a.txt"),
            file_of(commit(1), "b.txt"),
            other_options,
            FileQuery {
                target: FileTarget::WorkingTree {
                    path: RepoPath::from("a.txt"),
                    side: WorkingSide::Unstaged,
                },
                options: DiffOptions::default(),
            },
        ] {
            assert!(!state.wants_file(&other), "{other:?}");
            assert!(!state.file_arrived(&other, None), "{other:?}");
            assert!(!state.failed(&DiffQuery::File(other.clone()), "no".to_owned()));
        }
        assert_eq!(state.file(), Some((&selected, &Answer::Waiting)));
        assert!(state.file_arrived(&selected, None));
        assert_eq!(state.file(), Some((&selected, &Answer::Ready(None))));
    }

    /// A change set of `files` files, for the files opened in place.
    fn many(files: usize) -> ChangeSet {
        let mut set = change_set("f0");
        set.files = (0..files)
            .map(|n| change_set(&format!("f{n}")).files.remove(0))
            .collect();
        set
    }

    /// `commit(1)` selected and its change set of `files` files here.
    fn answered(files: usize) -> DiffState {
        let mut state = DiffState::default();
        state.select_changes(commit(1));
        assert!(state.changes_arrived(commit(1), many(files)));
        state
    }

    fn shown_file(index: usize) -> Box<ShownDiff> {
        Box::new(ShownDiff::new(
            FileDiff {
                file: many(index + 1).files.remove(index),
                content: DiffContent::ModeChangeOnly,
            },
            Context::default(),
        ))
    }

    fn page(indices: &[usize], by_all: bool) -> Vec<ExpandedFile> {
        indices
            .iter()
            .map(|index| ExpandedFile {
                file: OpenedFile {
                    index: *index,
                    load_anyway: false,
                },
                by_all,
                outcome: Ok(shown_file(*index)),
            })
            .collect()
    }

    fn expand_query(requests: &[Request]) -> &ExpandQuery {
        requests
            .iter()
            .find_map(|request| match request {
                Request::Expand(asked) => Some(asked),
                _ => None,
            })
            .unwrap_or_else(|| panic!("nothing was asked: {requests:?}"))
    }

    fn retired_count(requests: &[Request]) -> usize {
        requests
            .iter()
            .map(|request| match request {
                Request::Retire(retired) => retired.shown().len(),
                _ => 0,
            })
            .sum()
    }

    /// A new commit takes the file and the files opened in place with it — the changes query
    /// supersedes their lane — and hands what they drew to a worker to free. Caught by:
    /// keeping either (the pane waits for good on an answer that will never come, or draws one
    /// commit's diff under another's file), or dropping what they drew on the UI thread.
    #[test]
    fn selecting_a_commit_lets_go_of_the_file_and_the_files_opened_in_place() {
        let mut state = answered(3);
        state.select_file(file_of(commit(1), "a.txt"));
        state.toggle_file(1, DiffOptions::default());
        state.expansion_arrived(page(&[1], false), None);
        let requests = state.select_changes(commit(2));
        assert_eq!(state.file(), None);
        assert_eq!(state.all_state(), None);
        assert!(answered_expansion(&state).is_empty());
        assert_eq!(retired_count(&requests), 1, "{requests:?}");
    }

    /// R5.3, phase 02 QA's R2 at the window: Expand All asks from the first file with nothing
    /// spent; each page opens the files it read and moves Expand All on to where it stands; the
    /// page that ends it with the budget spent stops it and says so. Caught by: a page that
    /// replaces the last, progress not kept (a superseded Expand All starting again from the
    /// top), or the budget's end taken for every file opened.
    #[test]
    fn expand_all_opens_each_pages_files_and_moves_on_until_its_budget() {
        let mut state = answered(10);
        let options = DiffOptions::default();
        let asked = state.expand_all(options);
        let asked = expand_query(&asked);
        assert_eq!(asked.all, Some(AllFrom::default()));
        assert!(asked.files.is_empty());

        let at = AllFrom { next: 2, spent: 30 };
        state.expansion_arrived(page(&[0, 1], true), Some(AllProgress { at, ended: None }));
        assert_eq!(state.all_state(), Some(AllState::Running(at)));
        assert_eq!(answered_expansion(&state).len(), 2);
        assert!(!answered_expansion(&state).stopped_at_budget());

        let end = AllFrom { next: 4, spent: 60 };
        state.expansion_arrived(
            page(&[2, 3], true),
            Some(AllProgress {
                at: end,
                ended: Some(AllEnded::Budget),
            }),
        );
        assert_eq!(state.all_state(), Some(AllState::Stopped));
        let expansion = answered_expansion(&state);
        assert_eq!(expansion.len(), 4);
        assert!(expansion.stopped_at_budget());
        assert!(matches!(expansion.get(3), Some(Opened::Shown(_))));
    }

    /// The user's decision (2026-10-04): Expand All keeps the files already open — answered,
    /// failed or still being read — and asks only for the rest; what they draw is not replaced
    /// or freed, and the files Expand All reads open around them. Caught by: the files open
    /// offered to Expand All again (read anew, their diffs replaced and freed), or a page of
    /// Expand All replacing them.
    #[test]
    fn expand_all_keeps_the_files_already_open_and_asks_only_for_the_rest() {
        let mut state = answered(6);
        let options = DiffOptions::default();
        state.toggle_file(1, options);
        state.toggle_file(3, options);
        state.expansion_arrived(page(&[1, 3], false), None);
        state.toggle_file(4, options);
        let drawn = |state: &DiffState, index: usize| answered_expansion(state).get(index).cloned();
        let (one, three) = (drawn(&state, 1), drawn(&state, 3));
        assert!(matches!(one, Some(Opened::Shown(_))));

        let requests = state.expand_all(options);
        let asked = expand_query(&requests);
        assert_eq!(asked.all, Some(AllFrom::default()));
        assert_eq!(
            asked.kept_open,
            [1, 3, 4],
            "the files open are offered again"
        );
        assert_eq!(
            asked.files,
            [OpenedFile {
                index: 4,
                load_anyway: false
            }],
            "only the file still awaited is named"
        );
        assert_eq!(retired_count(&requests), 0, "a file open was let go of");
        assert_eq!(
            (drawn(&state, 1), drawn(&state, 3)),
            (one.clone(), three.clone())
        );

        let freed = state.expansion_arrived(
            page(&[0, 2, 5], true),
            Some(AllProgress {
                at: AllFrom { next: 6, spent: 9 },
                ended: Some(AllEnded::Every),
            }),
        );
        assert!(freed.is_none(), "{freed:?}");
        assert_eq!((drawn(&state, 1), drawn(&state, 3)), (one, three));
        assert_eq!(answered_expansion(&state).len(), 6);
        assert_eq!(state.all_state(), Some(AllState::Done));
    }

    /// The Changes tab's file and the files opened in place share the file-diff lane, so each
    /// asking takes it from the other — and the one that lost it is asked again, whole, when
    /// its tab is shown: never left half-answered. Caught by: a file waiting for good after an
    /// expansion superseded it, an expansion waiting for good after the file did, or an
    /// expansion asked again without the files it was waiting for.
    #[test]
    fn the_file_and_the_files_opened_in_place_take_the_lane_from_each_other() {
        let mut state = answered(6);
        let options = DiffOptions::default();
        state.toggle_file(4, options);
        assert!(!state.expansion_needs_asking());
        state.select_file(file_of(commit(1), "a.txt"));
        assert!(!state.file_needs_asking());
        assert!(
            state.expansion_needs_asking(),
            "the expansion lost the lane"
        );

        let asked = state.reask_expansion().into_iter().collect::<Vec<_>>();
        let asked = expand_query(&asked);
        assert_eq!(
            asked.files,
            [OpenedFile {
                index: 4,
                load_anyway: false
            }]
        );
        assert!(state.file_needs_asking(), "the file lost the lane");
        assert!(state.reask_expansion().is_none(), "asked twice");
        assert!(matches!(
            state.reask_file(),
            Some(Request::FileDiff(query)) if query == file_of(commit(1), "a.txt")
        ));
        assert!(!state.file_needs_asking());

        // Answered, neither needs asking whatever holds the lane.
        state.expansion_arrived(page(&[4], false), None);
        assert!(state.file_arrived(&file_of(commit(1), "a.txt"), None));
        state.select_file(file_of(commit(1), "a.txt"));
        assert!(!state.expansion_needs_asking());
    }

    /// An answer is kept only for what is asked now: a page for another comparison or at
    /// other options is not this expansion's, and a file closed while its page was on its way
    /// is not opened again by it — its diff is handed to a worker. Caught by: comparing the
    /// comparison alone (a page at the old context kept at the new), or a closed file
    /// springing open.
    #[test]
    fn a_page_is_kept_only_for_the_files_asked_as_they_were_asked() {
        let mut state = answered(6);
        let options = DiffOptions::default();
        let mut other = options;
        other.ignore_whitespace = true;
        state.toggle_file(3, options);
        assert!(state.wants_expansion(commit(1), options));
        assert!(!state.wants_expansion(commit(1), other));
        assert!(!state.wants_expansion(commit(2), options));

        let closing = state.toggle_file(3, options);
        assert!(
            closing.is_empty(),
            "nothing drawn, nothing to free: {closing:?}"
        );
        let freed = state.expansion_arrived(page(&[3], false), None);
        assert!(
            answered_expansion(&state).is_empty(),
            "the closed file opened again"
        );
        assert!(
            matches!(&freed, Some(Request::Retire(retired)) if retired.shown().len() == 1),
            "{freed:?}"
        );

        // A file read past the limits is kept only from the read past the limits.
        state.toggle_file(2, options);
        state.expansion_arrived(page(&[2], false), None);
        let loading = state.load_in_place(2);
        assert_eq!(
            expand_query(&loading).files,
            [OpenedFile {
                index: 2,
                load_anyway: true
            }]
        );
        assert_eq!(
            retired_count(&loading),
            1,
            "the replaced diff was not freed"
        );
        state.expansion_arrived(page(&[2], false), None);
        assert!(
            matches!(answered_expansion(&state).get(2), Some(Opened::Reading)),
            "a read within the limits answered a Load Diff"
        );
    }

    /// Collapse All closes every file, stops Expand All, frees what was drawn and ends what is
    /// in flight by asking for nothing in its place. Caught by: a page in flight opening files
    /// again after Collapse All, or answers dropped on the UI thread.
    #[test]
    fn collapse_all_ends_what_is_in_flight_and_frees_what_was_drawn() {
        let mut state = answered(6);
        let options = DiffOptions::default();
        state.expand_all(options);
        state.expansion_arrived(
            page(&[0, 1], true),
            Some(AllProgress {
                at: AllFrom { next: 2, spent: 9 },
                ended: None,
            }),
        );
        let requests = state.collapse_all();
        let superseding = expand_query(&requests);
        assert!(superseding.files.is_empty() && superseding.all.is_none());
        assert_eq!(retired_count(&requests), 2);
        assert_eq!(state.all_state(), Some(AllState::Off));
        state.expansion_arrived(
            page(&[2], true),
            Some(AllProgress {
                at: AllFrom { next: 3, spent: 12 },
                ended: None,
            }),
        );
        assert!(
            answered_expansion(&state).is_empty(),
            "a late page reopened files"
        );
    }

    /// A failure no file is to blame for fails every file still awaited, each on its own row,
    /// and stops Expand All; a file already drawn keeps its diff. Caught by: the failure
    /// dropped (files reading for good) or drawn over answered files.
    #[test]
    fn a_failed_expansion_fails_the_files_still_awaited() {
        let mut state = answered(4);
        let options = DiffOptions::default();
        state.toggle_file(0, options);
        state.expansion_arrived(page(&[0], false), None);
        let asked = state.toggle_file(2, options);
        let asked = expand_query(&asked).clone();
        assert!(state.failed(&DiffQuery::Expand(asked), "bad config".to_owned()));
        let expansion = answered_expansion(&state);
        assert!(matches!(expansion.get(0), Some(Opened::Shown(_))));
        assert_eq!(
            expansion.get(2),
            Some(&Opened::Failed("bad config".to_owned()))
        );
    }

    /// A setting moved: the selection whose tab is shown is asked again at once, and the other
    /// — they share the lane — when its tab is shown; what either drew is freed. Caught by:
    /// both asked (the second ending the first), neither, or the hidden one never asked again.
    #[test]
    fn a_setting_asks_the_shown_tabs_selection_now_and_the_others_later() {
        let mut state = answered(4);
        let options = DiffOptions::default();
        state.toggle_file(1, options);
        state.expansion_arrived(page(&[1], false), None);
        let chosen = file_of(commit(1), "a.txt");
        state.select_file(chosen.clone());
        assert!(state.file_arrived(&chosen, None));

        let mut wider = options;
        wider.context = Context::Lines(7);
        let requests = state.settings_changed(wider, wider, wider, Asking::Expansion);
        let asked = expand_query(&requests);
        assert_eq!(asked.options, wider);
        assert_eq!(retired_count(&requests), 1);
        assert!(
            !requests.iter().any(|r| matches!(r, Request::FileDiff(_))),
            "{requests:?}"
        );
        assert!(
            state.file_needs_asking(),
            "the file is not asked again when shown"
        );
        match state.reask_file() {
            Some(Request::FileDiff(query)) => assert_eq!(query.options, wider),
            other => panic!("{other:?}"),
        }
    }

    /// The entire file moves the Changes tab's file alone (the user's decision, 2026-10-04):
    /// the files opened in place keep their options, so nothing of theirs is asked again or
    /// freed, now or when the Commit tab is shown. Caught by: one set of options for both.
    #[test]
    fn the_entire_file_asks_the_changes_tabs_file_and_leaves_the_files_opened_in_place() {
        let mut state = answered(4);
        let options = DiffOptions::default();
        state.toggle_file(1, options);
        state.expansion_arrived(page(&[1], false), None);
        let chosen = file_of(commit(1), "a.txt");
        state.select_file(chosen.clone());
        assert!(state.file_arrived(&chosen, None));

        let entire = DiffOptions {
            context: Context::EntireFile,
            ..options
        };
        let requests = state.settings_changed(entire, options, entire, Asking::File);
        match requests.as_slice() {
            [Request::FileDiff(query)] => assert_eq!(query.options, entire),
            other => panic!("expected the file alone, asked: {other:?}"),
        }
        assert!(!state.expansion_needs_asking());
        assert_eq!(state.reask_expansion(), None);
    }

    /// Phase 08 QA's U2: an Expand All whose request failed is off — not asked again when the
    /// Commit tab is next shown, and its button Expand All once more. Caught by: the failure
    /// leaving Expand All running, which the tab then asks again, and fails again, for good.
    #[test]
    fn a_failed_expand_all_is_off_and_not_asked_again() {
        let mut state = answered(4);
        let asked = state.expand_all(DiffOptions::default());
        let asked = expand_query(&asked).clone();
        assert!(state.failed(&DiffQuery::Expand(asked), "bad config".to_owned()));
        assert_eq!(state.all_state(), Some(AllState::Off));
        assert!(!state.expansion_needs_asking());
        assert_eq!(state.reask_expansion(), None);
    }

    /// Phase 08 QA's U3, the other direction of
    /// `a_setting_asks_the_shown_tabs_selection_now_and_the_others_later`: with the Changes tab
    /// shown, a setting asks its file at once and not the files opened in place, which are
    /// asked — whole, at the new options — when the Commit tab is shown. Caught by: the
    /// expansion asked now (ending the file's read), never, or at the old options.
    #[test]
    fn a_setting_with_the_changes_tab_shown_asks_its_file_now_and_the_expansion_later() {
        let mut state = answered(4);
        let options = DiffOptions::default();
        state.toggle_file(1, options);
        state.expansion_arrived(page(&[1], false), None);
        let chosen = file_of(commit(1), "a.txt");
        state.select_file(chosen.clone());
        assert!(state.file_arrived(&chosen, None));

        let mut wider = options;
        wider.context = Context::Lines(7);
        let requests = state.settings_changed(wider, wider, wider, Asking::File);
        match requests.first() {
            Some(Request::FileDiff(query)) => assert_eq!(query.options, wider),
            other => panic!("the file was not asked first: {other:?}"),
        }
        assert!(
            !requests.iter().any(|r| matches!(r, Request::Expand(_))),
            "{requests:?}"
        );
        assert_eq!(
            retired_count(&requests),
            1,
            "the open file's diff was not freed"
        );
        assert!(!state.file_needs_asking());
        assert!(
            state.expansion_needs_asking(),
            "the files opened in place are not asked again when shown"
        );
        let asked = state.reask_expansion().into_iter().collect::<Vec<_>>();
        let asked = expand_query(&asked);
        assert_eq!(asked.options, wider);
        assert_eq!(
            asked.files,
            [OpenedFile {
                index: 1,
                load_anyway: false
            }]
        );
    }

    /// T4: a failure of the selected file is recorded as that file's answer. Caught by: the
    /// file arm of `failed` writing nothing (the pane waits for good on a failed diff).
    #[test]
    fn a_failed_file_diff_is_recorded_for_the_file() {
        let mut state = DiffState::default();
        let selected = file_of(commit(1), "a.txt");
        state.select_file(selected.clone());
        assert!(state.failed(&DiffQuery::File(selected.clone()), "git failed".to_owned()));
        assert_eq!(
            state.file(),
            Some((&selected, &Answer::Failed("git failed".to_owned())))
        );
    }

    /// R2 for a file: choosing another file — or the same at other options — hands the diff
    /// kept for the last one to a worker after the query, and the answer kept is prepared at
    /// the context it was asked at. Caught by: the replaced diff dropped on the UI thread, or
    /// rows grouped at another context than git was asked at.
    #[test]
    fn choosing_another_file_hands_the_last_diff_to_a_worker() {
        use cairn_model::{Context, DiffContent};
        let mut state = DiffState::default();
        let first = file_of(commit(1), "a.txt");
        assert_eq!(
            state.select_file(first.clone()),
            [Request::FileDiff(first.clone())],
            "nothing was kept, so nothing is retired"
        );
        let diff = FileDiff {
            file: change_set("a.txt").files.remove(0),
            content: DiffContent::ModeChangeOnly,
        };
        assert!(state.file_arrived(
            &first,
            Some(ShownDiff::new(diff.clone(), Context::default()))
        ));
        assert_eq!(
            state.shown_file().map(ShownDiff::context),
            Some(Context::default())
        );

        let mut wider = first.clone();
        wider.options.context = Context::Lines(7);
        let requests = state.select_file(wider.clone());
        assert_eq!(requests.len(), 2, "{requests:?}");
        assert_eq!(requests[0], Request::FileDiff(wider.clone()));
        match &requests[1] {
            // The diff as it was drawn, rows' indexes and all, freed off the UI thread.
            Request::Retire(retired) => assert_eq!(retired.shown().len(), 1),
            other => panic!("the second request is not a retirement: {other:?}"),
        }
        assert!(state.file_arrived(&wider, Some(ShownDiff::new(diff, Context::Lines(7)))));
        assert_eq!(
            state.shown_file().map(ShownDiff::context),
            Some(Context::Lines(7))
        );
    }
}
