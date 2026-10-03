//! View state for diffs: what the window has selected — a commit or a pair, a file, Expand
//! All — and the answer it keeps for that selection alone (PRD R4.4). On the UI thread, so
//! nothing here waits; asking is the caller's, through the [`Request`] each selection
//! returns.
//!
//! Two filters stand between a worker's answer and the window. The epoch drops an answer
//! whose query was superseded ([`crate::worker::Updates`]); this one drops an answer that
//! names anything but what is selected now — the selection cleared, or changed in a way
//! that asked nothing new — so the files of one commit are never drawn under another.

use std::sync::{Arc, LazyLock};

use cairn_model::ShownDiff;
use cairn_model::{
    ChangeSet, ChangeStatus, ChangedFile, Context, DiffContent, FileDiff, RenameDetection, RepoPath,
};
use cairn_ui::ShownFiles;

use crate::file_filter::FileFilter;
use crate::worker::{Comparison, DiffOptions, DiffQuery, FileQuery, Request, Retired};

/// An answer the window is waiting for, has, or was told failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer<T> {
    Waiting,
    Ready(T),
    /// Display text.
    Failed(String),
}

/// Expand All's files so far, in the change set's order, and whether they are all here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    pub diffs: Vec<FileDiff>,
    pub complete: bool,
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
    /// Expand All over a comparison, at some options, and what has arrived of it.
    expanded: Option<(Comparison, DiffOptions, Answer<Expanded>)>,
    /// The Changes tab's filter over the selected change set's files (phase 07).
    filter: FileFilter,
    /// Where the file selected is in the change set, when it was chosen from it: what the
    /// Changes tab's list highlights and moves from, kept rather than searched for.
    file_index: Option<usize>,
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
        let expanded = self.expanded.take();
        self.file_index = None;
        self.filter.changes_selected();
        let mut shown_diffs: Vec<ShownDiff> = Vec::new();
        if let Some((_, Answer::Ready(Some(shown)))) = file {
            shown_diffs.push(shown);
        }
        let mut diffs: Vec<FileDiff> = Vec::new();
        if let Some((_, _, Answer::Ready(Expanded { diffs: all, .. }))) = expanded {
            diffs.extend(all);
        }
        let changes = changes.and_then(|(_, answer)| match answer {
            Answer::Ready(changes) => Some(changes),
            Answer::Waiting | Answer::Failed(_) => None,
        });
        let mut requests = vec![Request::Changes { of }];
        requests.extend(Retired::of(changes, diffs, shown_diffs).map(Request::Retire));
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
        self.expanded = None;
        let mut requests = vec![Request::FileDiff(query)];
        if let Some((_, Answer::Ready(Some(shown)))) = replaced {
            requests.extend(Retired::of(None, Vec::new(), vec![shown]).map(Request::Retire));
        }
        requests
    }

    /// Asks for every file of `of`. The single file selected goes: it shares the lane.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Expand All is drawn in phase 08; its answers land here"
        )
    )]
    pub fn expand_all(&mut self, of: Comparison, options: DiffOptions) -> Request {
        self.expanded = Some((of, options, Answer::Waiting));
        self.file = None;
        self.file_index = None;
        Request::ExpandAll { of, options }
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

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Expand All is drawn in phase 08; its answers land here"
        )
    )]
    pub fn expanded(&self) -> Option<(Comparison, DiffOptions, &Answer<Expanded>)> {
        self.expanded
            .as_ref()
            .map(|(of, options, answer)| (*of, *options, answer))
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

/// The selected file's diff, or an empty one while none is kept: the view of the state the
/// diff view is handed, which reads its rows by index rather than copying them. Whether the
/// selection's answer is ready is the pane's to check before drawing it.
pub fn answered_file(state: &DiffState) -> &ShownDiff {
    state.shown_file().unwrap_or(&NO_DIFF)
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
            DiffQuery::All { of, options } => self.wants_expansion(*of, *options),
        }
    }

    pub fn wants_changes(&self, of: Comparison) -> bool {
        self.changes
            .as_ref()
            .is_some_and(|(selected, _)| *selected == of)
    }

    pub fn wants_file(&self, query: &FileQuery) -> bool {
        self.file
            .as_ref()
            .is_some_and(|(selected, _)| selected == query)
    }

    pub fn wants_expansion(&self, of: Comparison, options: DiffOptions) -> bool {
        self.expanded
            .as_ref()
            .is_some_and(|(selected, at, _)| *selected == of && *at == options)
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

    /// A batch of Expand All arrived; appended only if `of` at `options` is selected.
    pub fn expansion_arrived(
        &mut self,
        of: Comparison,
        options: DiffOptions,
        diffs: Vec<FileDiff>,
        complete: bool,
    ) -> bool {
        match &mut self.expanded {
            Some((selected, at, answer)) if *selected == of && *at == options => {
                match answer {
                    Answer::Ready(expanded) => {
                        expanded.diffs.extend(diffs);
                        expanded.complete = complete;
                    }
                    Answer::Waiting | Answer::Failed(_) => {
                        *answer = Answer::Ready(Expanded { diffs, complete });
                    }
                }
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
            DiffQuery::File(_) => {
                if let Some((_, answer)) = &mut self.file {
                    *answer = Answer::Failed(message);
                }
            }
            DiffQuery::All { .. } => {
                if let Some((_, _, answer)) = &mut self.expanded {
                    *answer = Answer::Failed(message);
                }
            }
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

    /// A new commit takes the file and Expand All with it, since the changes query
    /// supersedes their lane. Caught by: keeping the old file selected, whose answer will
    /// never come — the pane would wait for good.
    #[test]
    fn selecting_a_commit_lets_go_of_the_file_and_the_expansion() {
        let mut state = DiffState::default();
        state.select_changes(commit(1));
        state.select_file(file_of(commit(1), "a.txt"));
        state.select_changes(commit(2));
        assert_eq!(state.file(), None);

        state.expand_all(commit(2), DiffOptions::default());
        state.select_changes(commit(3));
        assert_eq!(state.expanded(), None);
    }

    /// Expand All's batches append in order until the last; a failure is shown only for
    /// the expansion asked. Caught by: a batch replacing the one before it.
    #[test]
    fn expand_all_batches_append_until_complete() {
        let mut state = DiffState::default();
        let options = DiffOptions::default();
        state.expand_all(commit(1), options);
        let diff = |path: &str| FileDiff {
            file: change_set(path).files.remove(0),
            content: cairn_model::DiffContent::ModeChangeOnly,
        };
        assert!(state.expansion_arrived(commit(1), options, vec![diff("a")], false));
        assert!(state.expansion_arrived(commit(1), options, vec![diff("b")], true));
        assert!(!state.expansion_arrived(commit(2), options, vec![diff("c")], true));
        assert_eq!(
            state.expanded(),
            Some((
                commit(1),
                options,
                &Answer::Ready(Expanded {
                    diffs: vec![diff("a"), diff("b")],
                    complete: true
                })
            ))
        );
        assert!(!state.failed(
            &DiffQuery::All {
                of: commit(2),
                options
            },
            "no".to_owned()
        ));
        assert!(state.failed(
            &DiffQuery::All {
                of: commit(1),
                options
            },
            "no".to_owned()
        ));
        assert_eq!(
            state.expanded(),
            Some((commit(1), options, &Answer::Failed("no".to_owned())))
        );
    }

    /// T4: each selection lets go of what shares its lane, one assertion per rule.
    /// Selecting a file lets Expand All go, and Expand All the file. Caught by: either
    /// assignment taken out (the pane waits for good on an answer superseded in its lane).
    #[test]
    fn a_file_and_expand_all_let_each_other_go() {
        let mut state = DiffState::default();
        state.select_changes(commit(1));
        state.expand_all(commit(1), DiffOptions::default());
        state.select_file(file_of(commit(1), "a.txt"));
        assert_eq!(state.expanded(), None, "select_file kept Expand All");

        state.expand_all(commit(1), DiffOptions::default());
        assert_eq!(state.file(), None, "expand_all kept the file");
    }

    /// T4: a batch for the selected comparison at other options is not this expansion's.
    /// Caught by: comparing the comparison alone (a batch at the old context appended to
    /// one at the new).
    #[test]
    fn an_expansion_batch_at_other_options_is_dropped() {
        let mut state = DiffState::default();
        let options = DiffOptions::default();
        let mut other = options;
        other.ignore_whitespace = true;
        state.expand_all(commit(1), options);
        let diff = FileDiff {
            file: change_set("a").files.remove(0),
            content: cairn_model::DiffContent::ModeChangeOnly,
        };
        assert!(!state.expansion_arrived(commit(1), other, vec![diff], true));
        assert_eq!(
            state.expanded(),
            Some((commit(1), options, &Answer::Waiting))
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
