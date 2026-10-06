//! Applying a worker's update to the view state: the one place an `Update`
//! becomes what the window draws. On the UI thread, so nothing here waits.

use std::sync::Arc;

use freya::prelude::*;

use crate::PAGE_ROWS;
use crate::fetch_state::{FetchRefusal, FetchStatus, PromptView};
use crate::history_state::{self, Progress};
use crate::window::View;
use crate::worker::{PromptId, Request, Retired, Update, expanded_diffs};

/// What applying an update may ask of the worker: a request, and the refusal
/// of a prompt the window will not show. Two plain callbacks, never a struct
/// holding the answering end: nothing may hold that. `closing` says the
/// window has asked the repository to close, so nothing is to be asked of it
/// again.
pub struct Worker<'a> {
    pub submit: &'a dyn Fn(Request),
    pub refuse: &'a dyn Fn(PromptId),
    pub closing: bool,
}

/// Applies `update` to `view`. A fetch ending takes down any dialog and
/// refuses its prompt, which releases the helper of a git that is gone; a
/// fetch that moved refs clears the rows and asks for the history again; a
/// prompt arriving while no fetch is in flight — a helper orphaned by a
/// killed git — is refused rather than shown, since the dialog could not say
/// which remote it was for.
pub fn apply(update: Update, view: View, worker: &Worker<'_>) {
    let View {
        mut rows,
        mut progress,
        mut fetch,
        mut prompt,
        mut remotes,
        mut refused,
        mut diff,
        ..
    } = view;
    match update {
        Update::Rows {
            rows: page,
            complete,
        } => {
            // Count before handing the rows over; afterwards it rereads the whole history.
            let widest = history_state::widest_lane(&page);
            let (loaded, held) = {
                let mut history = rows.write();
                let held = history.append(page);
                (history.len(), held)
            };
            progress.write().appended(widest, complete, loaded, held);
        }
        Update::Failed { message } | Update::WorkerLost { message } => {
            progress.write().failed(message);
        }
        Update::Remotes { remotes: listed } => remotes.set(listed),
        Update::ConfiguredContext { context } => {
            crate::diff_actions::configured(context, view, worker.submit);
        }
        Update::FetchStarted { remote } => fetch.write().started(remote),
        Update::FetchProgress { line } => fetch.write().progressed(line),
        Update::FetchFinished { remote, refreshed } => {
            withdraw(&mut prompt, worker);
            fetch.set(FetchStatus::Finished { remote });
            reload_if(refreshed, rows, progress, worker);
        }
        Update::FetchCancelled {
            remote,
            refreshed,
            stranded_locks,
        } => {
            withdraw(&mut prompt, worker);
            fetch.set(FetchStatus::Cancelled {
                remote,
                stranded_locks,
            });
            reload_if(refreshed, rows, progress, worker);
        }
        Update::FetchFailed {
            remote,
            refreshed,
            message,
        } => {
            withdraw(&mut prompt, worker);
            fetch.set(FetchStatus::Failed { remote, message });
            reload_if(refreshed, rows, progress, worker);
        }
        // Beside the fetch in flight, which it leaves as it was; the next press clears it.
        Update::FetchRefused { remote, reason } => {
            refused.set(Some(FetchRefusal { remote, reason }));
        }
        // Answered for whoever asks; no view draws the log in this packet (PRD R8.3).
        Update::CommandLog { .. } => {}
        Update::Prompt { id, text } => {
            if fetch.read().is_in_flight() {
                prompt.set(Some(PromptView { id, text }));
            } else {
                (worker.refuse)(id);
            }
        }
        // Each kept only for the selection it names (PRD R4.4); checked before the write,
        // so an answer for another selection does not even wake what draws the diff. One not
        // kept is handed to a worker to free (`retire`), as a superseded one is.
        Update::Changes { of, changes } => {
            if diff.peek().wants_changes(of) {
                let filtering = {
                    let mut state = diff.write();
                    state.changes_arrived(of, changes);
                    // The filter's text, asked again for the change set that has arrived.
                    state.filter_again()
                };
                if let Some(request) = filtering {
                    (worker.submit)(request);
                }
            } else {
                retire(Retired::of(Some(Arc::new(changes)), Vec::new()), worker);
            }
        }
        Update::FilteredFiles { of, text, files } => {
            if diff.peek().wants_filter(of, &text) {
                diff.write().filter_arrived(of, &text, files);
            }
        }
        Update::FileDiff {
            query,
            diff: answer,
        } => {
            let answer = answer.map(|shown| *shown);
            if diff.peek().wants_file(&query) {
                diff.write().file_arrived(&query, answer);
            } else {
                retire(Retired::of(None, answer.into_iter().collect()), worker);
            }
        }
        Update::Expanded {
            of,
            options,
            files,
            all,
        } => {
            if diff.peek().wants_expansion(of, options) {
                let freeing = diff.write().expansion_arrived(files, all);
                if let Some(request) = freeing {
                    (worker.submit)(request);
                }
            } else {
                retire(Retired::of(None, expanded_diffs(files)), worker);
            }
        }
        Update::DiffFailed { query, message } => {
            if diff.peek().wants(&query) {
                diff.write().failed(&query, message);
            }
        }
        Update::Superseded(retired) => retire(Some(retired), worker),
    }
}

/// Hands answers the window will not keep to a worker to free, rather than freeing them here
/// on the UI thread: a file's prepared diff is up to 64 MiB in both layouts, a change set
/// tens of thousands of files. `Request::Retire` only sends.
fn retire(retired: Option<Retired>, worker: &Worker<'_>) {
    if let Some(retired) = retired {
        (worker.submit)(Request::Retire(retired));
    }
}

/// The refs moved: the rows on screen are of the old ones — unless the window
/// is closing, when the history stays as it is: the worker that would answer
/// has stopped, and clearing every row on the way out is work for nothing.
fn reload_if(
    refreshed: bool,
    mut rows: State<cairn_model::History>,
    mut progress: State<Progress>,
    worker: &Worker<'_>,
) {
    if !refreshed || worker.closing {
        return;
    }
    rows.set(cairn_model::History::new());
    progress.set(Progress::opening());
    (worker.submit)(Request::OpenHistory { rows: PAGE_ROWS });
}

/// Takes down a dialog whose fetch has ended, refusing the prompt so the helper
/// still waiting on it is released: the git behind it is gone or failing anyway.
fn withdraw(prompt: &mut State<Option<PromptView>>, worker: &Worker<'_>) {
    if let Some(shown) = prompt.write().take() {
        (worker.refuse)(shown.id);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use cairn_model::{GraphRow, History, Lane, Oid, PagedCommit, RowsPage};
    use freya_testing::TestingRunner;

    use super::*;
    use crate::history_state::Status;

    fn oid(n: u8) -> Oid {
        let mut bytes = [0u8; 20];
        bytes[19] = n;
        Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
    }

    /// A page of rows `commit n` for each `n` of `ids`, each by `author`.
    fn page(ids: impl IntoIterator<Item = u8>, author: &str) -> RowsPage {
        let mut page = RowsPage::new();
        for n in ids {
            page.push(
                GraphRow::new(oid(n), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents: 1,
                    subject: &format!("commit {n}"),
                    author,
                    author_time: 0,
                },
            );
        }
        page
    }

    fn history_of(page: RowsPage) -> History {
        let mut history = History::new();
        history.append(page).unwrap_or_else(|full| panic!("{full}"));
        history
    }

    /// What the worker was asked, and which prompts were refused (never a secret).
    #[derive(Default)]
    struct Asked {
        submitted: RefCell<Vec<Request>>,
        refused: RefCell<Vec<PromptId>>,
    }

    /// A view with three rows loaded and a fetch in flight, inside a runner so the
    /// states have a runtime; `apply` runs through `run_in`.
    fn launch(fetch: FetchStatus) -> (TestingRunner, View, Rc<Asked>) {
        let asked = Rc::new(Asked::default());
        let (mut test, view) = TestingRunner::new(
            rect,
            (100., 100.).into(),
            move |runner| {
                runner.provide_root_context(|| {
                    let mut progress = Progress::opening();
                    progress.received(1, false, 3);
                    View {
                        rows: State::create(history_of(page(0..3, "Ada"))),
                        progress: State::create(progress),
                        selected: State::create(None),
                        fetch: State::create(fetch),
                        prompt: State::create(None),
                        remotes: State::create(Vec::new()),
                        refused: State::create(None),
                        diff: State::create(crate::diff_state::DiffState::default()),
                        filter_text: State::create(String::new()),
                        changes_list_width: State::create(crate::changes_tab::LIST_WIDTH),
                        pair: State::create(None),
                        held_keys: State::create(cairn_ui::accelerators::HeldKeys::default()),
                        history_scroll: ScrollController::new(0, 0, Vec::new()),
                        detail_tab: State::create(cairn_ui::DetailTab::default()),
                        pane_collapsed: State::create(false),
                        pane_height: State::create(crate::window::PANE_HEIGHT),
                        diff_settings: State::create(cairn_ui::DiffSettings::default()),
                        diff_scroll: ScrollController::new(0, 0, Vec::new()),
                        change_cursor: State::create(None),
                    }
                })
            },
            1.,
        );
        test.sync_and_update();
        (test, view, asked)
    }

    fn applying(test: &TestingRunner, view: View, asked: &Rc<Asked>, update: Update) {
        applying_while(test, view, asked, update, false);
    }

    fn applying_while(
        test: &TestingRunner,
        view: View,
        asked: &Rc<Asked>,
        update: Update,
        closing: bool,
    ) {
        let submit = {
            let asked = Rc::clone(asked);
            move |request| asked.submitted.borrow_mut().push(request)
        };
        let refuse = {
            let asked = Rc::clone(asked);
            move |prompt| asked.refused.borrow_mut().push(prompt)
        };
        test.run_in(|| {
            apply(
                update,
                view,
                &Worker {
                    submit: &submit,
                    refuse: &refuse,
                    closing,
                },
            );
        });
    }

    /// C8, R4.4, through the real boundary: an answer naming another selection is never
    /// drawn, even when its epoch is still current — the selection was cleared without
    /// asking anything new, so the change set arrives through the worker under a current
    /// epoch and only the window's own check can refuse it. The negative follows: asked
    /// again and selected, it is kept. Mutation that reddens it: this arm storing whatever
    /// change set arrives rather than asking `DiffState` whether it names the selection
    /// (each of `DiffState`'s two checks alone is decided by `diff_state`'s own tests).
    #[test]
    fn an_answer_naming_another_selection_is_never_drawn() {
        use crate::diff_state::Answer;
        use crate::worker::{Comparison, changes_answer, checkout, commits};

        let (handle, mut updates) = checkout();
        let of = Comparison::Commit(commits(&handle, &mut updates, 1)[0]);
        let (test, mut view, asked) = launch(FetchStatus::Idle);

        for request in test.run_in(|| view.diff.write().select_changes(of)) {
            handle.submit(request);
        }
        test.run_in(|| view.diff.write().clear());
        applying(&test, view, &asked, changes_answer(&mut updates, of));
        assert_eq!(
            test.run_in(|| view.diff.peek().changes().map(|(of, _)| of)),
            None,
            "a change set was drawn with nothing selected"
        );

        for request in test.run_in(|| view.diff.write().select_changes(of)) {
            handle.submit(request);
        }
        applying(&test, view, &asked, changes_answer(&mut updates, of));
        let kept = test.run_in(|| {
            view.diff
                .peek()
                .changes()
                .map(|(of, answer)| (of, matches!(answer, Answer::Ready(_))))
        });
        assert_eq!(
            kept,
            Some((of, true)),
            "the answer for the selection was not kept"
        );
        drop(handle);
    }

    /// T5, the other three diff arms: a file's diff, Expand All's batch and a failure are
    /// each refused with nothing selected and kept once their selection is made. Synthetic
    /// answers, since what is decided here is the window's own check. Caught by: an arm
    /// that stores whatever arrives (selecting what it names first), or one that drops
    /// even its own selection's answer.
    #[test]
    fn every_diff_answer_is_kept_for_its_selection_alone() {
        use cairn_model::{
            ChangeStatus, ChangedFile, DiffContent, FileDiff, FileMode, Oid, RepoPath,
        };

        use crate::diff_state::{Answer, answered_expansion};
        use crate::worker::{
            AllEnded, AllFrom, AllProgress, Comparison, DiffOptions, DiffQuery, ExpandedFile,
            FileQuery, FileTarget, OpenedFile,
        };

        let of = Comparison::Commit(Oid::from_bytes(&[1; 20]).unwrap());
        let file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("a.txt"),
            new_path: RepoPath::from("a.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Executable),
            old_id: Some(Oid::from_bytes(&[2; 20]).unwrap()),
            new_id: Some(Oid::from_bytes(&[2; 20]).unwrap()),
        };
        let diff = FileDiff {
            file: file.clone(),
            content: DiffContent::ModeChangeOnly,
        };
        let query = FileQuery {
            target: FileTarget::Committed { of, file },
            options: DiffOptions::default(),
        };
        let options = DiffOptions::default();
        let (test, mut view, asked) = launch(FetchStatus::Idle);
        let file_update = || Update::FileDiff {
            query: query.clone(),
            diff: Some(Box::new(cairn_model::ShownDiff::new(
                diff.clone(),
                query.options.context,
            ))),
        };
        let page = || Update::Expanded {
            of,
            options,
            files: vec![ExpandedFile {
                file: OpenedFile {
                    index: 0,
                    load_anyway: false,
                },
                by_all: true,
                outcome: Ok(Box::new(cairn_model::ShownDiff::new(
                    diff.clone(),
                    options.context,
                ))),
            }],
            all: Some(AllProgress {
                at: AllFrom { next: 1, spent: 1 },
                ended: Some(AllEnded::Every),
            }),
        };

        applying(&test, view, &asked, file_update());
        assert!(
            !test.run_in(|| view.diff.peek().file().is_some()),
            "an answer was kept with nothing selected"
        );
        test.run_in(|| view.diff.write().select_file(query.clone()));
        applying(&test, view, &asked, file_update());
        assert_eq!(
            test.run_in(|| view.diff.peek().file().map(|(_, a)| a.clone())),
            Some(Answer::Ready(Some(cairn_model::ShownDiff::new(
                diff.clone(),
                query.options.context
            )))),
            "the selected file's diff was not kept"
        );

        test.run_in(|| view.diff.write().clear());
        applying(&test, view, &asked, page());
        assert!(
            test.run_in(|| answered_expansion(&view.diff.peek()).is_empty()),
            "an answer was kept with nothing selected"
        );
        test.run_in(|| {
            let mut state = view.diff.write();
            state.select_changes(of);
            state.changes_arrived(
                of,
                cairn_model::ChangeSet {
                    files: vec![diff.file.clone()],
                    details: None,
                    renames: cairn_model::RenameDetection::default(),
                },
            );
            state.expand_all(options)
        });
        applying(&test, view, &asked, page());
        assert!(
            test.run_in(|| matches!(
                answered_expansion(&view.diff.peek()).get(0),
                Some(cairn_ui::Opened::Shown(_))
            )),
            "the selected expansion's page was not kept"
        );

        test.run_in(|| view.diff.write().clear());
        let failure = || Update::DiffFailed {
            query: DiffQuery::File(query.clone()),
            message: "git failed".to_owned(),
        };
        applying(&test, view, &asked, failure());
        assert!(
            !test.run_in(|| view.diff.peek().file().is_some()),
            "an answer was kept with nothing selected"
        );
        test.run_in(|| view.diff.write().select_file(query.clone()));
        applying(&test, view, &asked, failure());
        assert_eq!(
            test.run_in(|| view.diff.peek().file().map(|(_, a)| a.clone())),
            Some(Answer::Failed("git failed".to_owned())),
            "the selected file's failure was not kept"
        );
    }

    /// R1 (phase 07 QA): an answer the window will not keep is freed on a worker, never on
    /// the UI thread — a file's prepared diff is up to 64 MiB in both layouts, a change set
    /// 55,184 files. One the epoch filter caught arrives as `Update::Superseded` and is
    /// handed straight back; one naming another selection is handed back from its own arm.
    /// Both go as a `Request::Retire`, which only sends. Caught by: either dropped in place.
    #[test]
    fn an_answer_the_window_will_not_keep_is_handed_to_a_worker_to_free() {
        use cairn_model::{
            ChangeSet, ChangeStatus, ChangedFile, DiffContent, FileDiff, FileMode, Oid,
            RenameDetection, RepoPath, ShownDiff,
        };

        use crate::worker::{Comparison, DiffOptions, FileQuery, FileTarget, Retired};

        let of = Comparison::Commit(Oid::from_bytes(&[1; 20]).unwrap());
        let file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("a.txt"),
            new_path: RepoPath::from("a.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Executable),
            old_id: Some(Oid::from_bytes(&[2; 20]).unwrap()),
            new_id: Some(Oid::from_bytes(&[2; 20]).unwrap()),
        };
        let diff = FileDiff {
            file: file.clone(),
            content: DiffContent::ModeChangeOnly,
        };
        let shown = ShownDiff::new(diff.clone(), DiffOptions::default().context);
        let query = FileQuery {
            target: FileTarget::Committed { of, file },
            options: DiffOptions::default(),
        };
        let changes = ChangeSet {
            files: Vec::new(),
            details: None,
            renames: RenameDetection::default(),
        };
        let retired = |asked: &Asked| -> Vec<Retired> {
            asked
                .submitted
                .borrow_mut()
                .drain(..)
                .map(|request| match request {
                    Request::Retire(retired) => retired,
                    other => panic!("expected a retirement, got {other:?}"),
                })
                .collect()
        };
        let (test, view, asked) = launch(FetchStatus::Idle);

        // Caught by the epoch filter: handed back as it came.
        let stale = Retired::of(None, vec![shown.clone()])
            .unwrap_or_else(|| unreachable!("a diff is something"));
        applying(&test, view, &asked, Update::Superseded(stale.clone()));
        assert_eq!(retired(&asked), [stale]);

        // Naming nothing selected: each payload handed back from its own arm.
        applying(
            &test,
            view,
            &asked,
            Update::FileDiff {
                query: query.clone(),
                diff: Some(Box::new(shown.clone())),
            },
        );
        let handed = retired(&asked);
        assert_eq!(handed.len(), 1, "an unwanted file diff was not retired");
        assert_eq!(handed[0].shown(), std::slice::from_ref(&shown));

        applying(
            &test,
            view,
            &asked,
            Update::Changes {
                of,
                changes: changes.clone(),
            },
        );
        let handed = retired(&asked);
        assert_eq!(handed.len(), 1, "an unwanted change set was not retired");
        assert_eq!(handed[0].changes(), Some(&changes));

        applying(
            &test,
            view,
            &asked,
            Update::Expanded {
                of,
                options: DiffOptions::default(),
                files: vec![crate::worker::ExpandedFile {
                    file: crate::worker::OpenedFile {
                        index: 0,
                        load_anyway: false,
                    },
                    by_all: true,
                    outcome: Ok(Box::new(shown.clone())),
                }],
                all: None,
            },
        );
        let handed = retired(&asked);
        assert_eq!(
            handed.len(),
            1,
            "an unwanted page of files opened in place was not retired"
        );
        assert_eq!(handed[0].shown(), std::slice::from_ref(&shown));

        // A clean working-tree path's answer holds nothing to free, so nothing is sent.
        applying(&test, view, &asked, Update::FileDiff { query, diff: None });
        assert!(retired(&asked).is_empty());
    }

    /// A fetch the close itself ended may still have moved refs. Caught by: reloading
    /// anyway — every row cleared on the UI thread and a request sent to a worker that has
    /// stopped, so the history vanishes behind "Reading history…" while the window closes.
    #[test]
    fn a_fetch_ended_by_the_close_leaves_the_history_as_it_is() {
        // However it ended: the close may land just as a fetch finishes or fails.
        for ending in [
            Update::FetchFinished {
                remote: "origin".to_owned(),
                refreshed: true,
            },
            Update::FetchCancelled {
                remote: "origin".to_owned(),
                refreshed: true,
                stranded_locks: Vec::new(),
            },
            Update::FetchFailed {
                remote: "origin".to_owned(),
                refreshed: true,
                message: "some local refs could not be updated".to_owned(),
            },
        ] {
            let (test, view, asked) = launch(running());
            applying_while(&test, view, &asked, ending.clone(), true);
            assert_eq!(
                view.rows.read().len(),
                3,
                "the rows were cleared on the way out: {ending:?}"
            );
            assert!(
                asked.submitted.borrow().is_empty(),
                "a closing repository was asked for its history again: {ending:?}"
            );
        }
    }

    fn running() -> FetchStatus {
        FetchStatus::Running {
            remote: "origin".to_owned(),
            line: None,
        }
    }

    /// Caught by: not clearing, not resetting, or not asking again — the graph would keep
    /// the pre-fetch refs.
    #[test]
    fn a_fetch_that_moved_refs_clears_the_rows_and_asks_for_the_history_again() {
        let (test, view, asked) = launch(running());
        applying(
            &test,
            view,
            &asked,
            Update::FetchFinished {
                remote: "origin".to_owned(),
                refreshed: true,
            },
        );
        assert!(view.rows.read().is_empty(), "the old rows stayed");
        assert_eq!(view.progress.read().status(), &Status::Loading);
        assert_eq!(
            asked.submitted.borrow().as_slice(),
            [Request::OpenHistory { rows: PAGE_ROWS }]
        );
        assert_eq!(
            *view.fetch.read(),
            FetchStatus::Finished {
                remote: "origin".to_owned()
            }
        );
    }

    /// Caught by: reloading on every fetch, which loses the reader's place for nothing.
    #[test]
    fn a_fetch_that_moved_nothing_leaves_the_rows_alone() {
        let (test, view, asked) = launch(running());
        applying(
            &test,
            view,
            &asked,
            Update::FetchFinished {
                remote: "origin".to_owned(),
                refreshed: false,
            },
        );
        assert_eq!(view.rows.read().len(), 3);
        assert!(asked.submitted.borrow().is_empty());
    }

    /// Caught by: a cancelled or failed fetch that moved refs before it stopped leaving
    /// the graph stale.
    #[test]
    fn a_cancelled_or_failed_fetch_that_moved_refs_reloads_too() {
        for ending in [
            Update::FetchCancelled {
                remote: "origin".to_owned(),
                refreshed: true,
                stranded_locks: Vec::new(),
            },
            Update::FetchFailed {
                remote: "origin".to_owned(),
                refreshed: true,
                message: "some local refs could not be updated".to_owned(),
            },
        ] {
            let (test, view, asked) = launch(running());
            applying(&test, view, &asked, ending);
            assert!(view.rows.read().is_empty());
            assert_eq!(asked.submitted.borrow().len(), 1);
        }
    }

    /// Caught by: dropping the paths between the worker and the banner, which is the one
    /// hop nothing else pins.
    #[test]
    fn a_cancelled_fetch_hands_the_lock_files_it_found_to_the_banner() {
        let lock = std::path::PathBuf::from("/r/.git/refs/remotes/origin/main.lock");
        let (test, view, asked) = launch(running());
        applying(
            &test,
            view,
            &asked,
            Update::FetchCancelled {
                remote: "origin".to_owned(),
                refreshed: false,
                stranded_locks: vec![lock.clone()],
            },
        );
        assert_eq!(
            *view.fetch.read(),
            FetchStatus::Cancelled {
                remote: "origin".to_owned(),
                stranded_locks: vec![lock],
            }
        );
    }

    /// Caught by: deleting the withdrawal — the dialog stays up after the fetch ended and
    /// the helper behind it waits until the user notices.
    #[test]
    fn a_fetch_ending_takes_the_dialog_down_and_refuses_its_prompt() {
        for ending in [
            Update::FetchFinished {
                remote: "origin".to_owned(),
                refreshed: false,
            },
            Update::FetchCancelled {
                remote: "origin".to_owned(),
                refreshed: false,
                stranded_locks: Vec::new(),
            },
            Update::FetchFailed {
                remote: "origin".to_owned(),
                refreshed: false,
                message: "no".to_owned(),
            },
        ] {
            let (test, view, asked) = launch(running());
            let id = PromptId::for_tests(9);
            applying(
                &test,
                view,
                &asked,
                Update::Prompt {
                    id,
                    text: "Password for 'https://h/x': ".to_owned(),
                },
            );
            assert!(view.prompt.read().is_some(), "the prompt was not shown");
            applying(&test, view, &asked, ending);
            assert_eq!(*view.prompt.read(), None, "the dialog stayed up");
            assert_eq!(asked.refused.borrow().as_slice(), [id]);
        }
    }

    /// Caught by: showing a prompt with no fetch in flight, which the dialog could only
    /// attribute to "git".
    #[test]
    fn a_prompt_with_no_fetch_in_flight_is_refused_not_shown() {
        let (test, view, asked) = launch(FetchStatus::Idle);
        let id = PromptId::for_tests(4);
        applying(
            &test,
            view,
            &asked,
            Update::Prompt {
                id,
                text: "Password for 'https://h/x': ".to_owned(),
            },
        );
        assert_eq!(*view.prompt.read(), None);
        assert_eq!(asked.refused.borrow().as_slice(), [id]);
    }

    /// PRD R7.2, the hop between the worker and the window: a refusal lands in the view
    /// for the banner, and the fetch in flight is left as it was. Caught by: dropping the
    /// update, or letting it end or replace the fetch that is running.
    #[test]
    fn a_refused_fetch_is_kept_for_the_window_and_the_fetch_in_flight_is_untouched() {
        let (test, view, asked) = launch(running());
        applying(
            &test,
            view,
            &asked,
            Update::FetchRefused {
                remote: "origin".to_owned(),
                reason: "a fetch of origin is already running".to_owned(),
            },
        );
        assert_eq!(
            *view.refused.read(),
            Some(FetchRefusal {
                remote: "origin".to_owned(),
                reason: "a fetch of origin is already running".to_owned(),
            })
        );
        assert_eq!(*view.fetch.read(), running(), "the fetch in flight changed");
        assert!(asked.submitted.borrow().is_empty());
        assert!(asked.refused.borrow().is_empty());
    }

    #[test]
    fn progress_and_remotes_land_in_their_states() {
        let (test, view, asked) = launch(running());
        applying(
            &test,
            view,
            &asked,
            Update::FetchProgress {
                line: "Receiving objects: 40%".to_owned(),
            },
        );
        assert_eq!(
            *view.fetch.read(),
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: Some("Receiving objects: 40%".to_owned())
            }
        );
        applying(
            &test,
            view,
            &asked,
            Update::Remotes {
                remotes: vec![cairn_model::RemoteSummary {
                    name: "origin".to_owned(),
                    url: None,
                }],
            },
        );
        assert_eq!(view.remotes.read().len(), 1);
    }

    /// Phase 06: the user's `diff.context` is where the session's context starts; a file
    /// already shown at git's default — in the Changes tab — is asked again at it; once the user has moved the
    /// context, a later reading of the configuration does not move it back. Caught by: a
    /// context fixed at three, or one the configuration overrides after the user chose.
    #[test]
    fn the_configured_context_is_where_the_session_starts_until_the_user_moves_it() {
        use cairn_model::{ChangeStatus, ChangedFile, Context, FileMode, Oid, RepoPath};

        use crate::worker::{Comparison, FileQuery, FileTarget};

        let (test, mut view, asked) = launch(FetchStatus::Idle);
        let file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("a.txt"),
            new_path: RepoPath::from("a.txt"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: None,
            new_id: None,
        };
        let query = FileQuery {
            target: FileTarget::Committed {
                of: Comparison::Commit(Oid::from_bytes(&[1; 20]).unwrap()),
                file,
            },
            options: crate::diff_actions::options(cairn_ui::DiffSettings::default()),
        };
        // Shown in the Changes tab, whose file a setting asks again at once (phase 08: the
        // Commit tab's files opened in place share the lane, and the hidden one waits).
        test.run_in(|| view.detail_tab.set(cairn_ui::DetailTab::Changes));
        test.run_in(|| view.diff.write().select_file(query.clone()));

        applying(
            &test,
            view,
            &asked,
            Update::ConfiguredContext {
                context: Context::Lines(5),
            },
        );
        assert_eq!(
            test.run_in(|| view.diff_settings.peek().context()),
            Context::Lines(5)
        );
        let mut again = query.clone();
        again.options.context = Context::Lines(5);
        assert_eq!(
            asked.submitted.borrow().as_slice(),
            [Request::FileDiff(again)]
        );

        test.run_in(|| {
            view.diff_settings.write().more_lines();
        });
        applying(
            &test,
            view,
            &asked,
            Update::ConfiguredContext {
                context: Context::Lines(9),
            },
        );
        assert_eq!(
            test.run_in(|| view.diff_settings.peek().context()),
            Context::Lines(6),
            "the configuration moved a context the user chose"
        );
        assert_eq!(asked.submitted.borrow().len(), 1);
    }

    /// Phase 06 QA (T7), through the real boundary: `diff.context` and
    /// `diff.interHunkContext` edited mid-session reach the next answer — the context
    /// adopted while the user has not moved it, the grouping always — and once the user has
    /// moved the context, a `diff.context` edit is read and sent but not adopted, while the
    /// inter-hunk context still applies. Caught by: the configured context read once, on a
    /// handle the configuration's freshness never reopens.
    #[test]
    fn a_configuration_edit_mid_session_reaches_the_next_answer() {
        use cairn_model::Context;

        use crate::diff_state::Answer;
        use crate::worker::{Configurable, FileQuery, FileTarget, next_update};

        let repository = Configurable::new("diff-context-edit");
        let (handle, mut updates) = repository.open();
        let (test, mut view, asked) = launch(FetchStatus::Idle);
        // The file is the Changes tab's, shown, so a setting asks it again at once.
        test.run_in(|| view.detail_tab.set(cairn_ui::DetailTab::Changes));

        // Applies updates, handing on whatever applying them asked, until `done` holds of
        // the state and what arrived; returns what arrived.
        let mut pump = |done: &dyn Fn(&[Update]) -> bool| -> Vec<Update> {
            let mut seen = Vec::new();
            loop {
                for request in asked.submitted.borrow_mut().drain(..) {
                    handle.submit(request);
                }
                if done(&seen) {
                    return seen;
                }
                let update = next_update(&mut updates);
                seen.push(update.clone());
                applying(&test, view, &asked, update);
            }
        };
        // The inter-hunk context of the answer shown, when it is the one asked at `context`
        // with whitespace `ignoring` or not.
        let shown_at = |context: Context, ignoring: bool| -> Option<u32> {
            test.run_in(|| {
                let state = view.diff.peek();
                match state.file() {
                    Some((query, Answer::Ready(Some(shown))))
                        if query.options.context == context
                            && query.options.ignore_whitespace == ignoring =>
                    {
                        shown
                            .diff()
                            .overlay()
                            .map(|overlay| overlay.function_context().inter_hunk_context())
                    }
                    _ => None,
                }
            })
        };
        let configured = |seen: &[Update]| -> Vec<Context> {
            seen.iter()
                .filter_map(|update| match update {
                    Update::ConfiguredContext { context } => Some(*context),
                    _ => None,
                })
                .collect()
        };
        let settings = || test.run_in(|| view.diff_settings.peek().context());
        let change = |change: fn(&mut cairn_ui::DiffSettings) -> bool| {
            let submit = |request| asked.submitted.borrow_mut().push(request);
            test.run_in(|| crate::diff_actions::change_settings(view, Some(&submit), change));
        };

        // At open: git's default, since nothing is configured.
        asked
            .submitted
            .borrow_mut()
            .push(Request::ConfiguredContext);
        let seen = pump(&|seen| !configured(seen).is_empty());
        assert_eq!(configured(&seen), [Context::Lines(3)]);
        let query = FileQuery {
            target: FileTarget::Committed {
                of: repository.of,
                file: repository.file.clone(),
            },
            options: crate::diff_actions::options(test.run_in(|| *view.diff_settings.peek())),
        };
        let mut diff = view.diff;
        let asked_first = test.run_in(|| diff.write().select_file(query));
        asked.submitted.borrow_mut().extend(asked_first);
        pump(&|_| shown_at(Context::Lines(3), false).is_some());
        assert_eq!(shown_at(Context::Lines(3), false), Some(0));

        // Edited while the context is unmoved: the next answer, asked for anything, follows.
        repository.configure("[diff]\n\tcontext = 5\n\tinterHunkContext = 2\n");
        change(|settings| {
            settings.toggle_ignore_whitespace();
            true
        });
        let seen = pump(&|_| shown_at(Context::Lines(5), true).is_some());
        assert!(configured(&seen).contains(&Context::Lines(5)), "{seen:?}");
        assert_eq!(settings(), Context::Lines(5));
        assert_eq!(shown_at(Context::Lines(5), true), Some(2));

        // Moved by the user: a later diff.context is read and sent, and not adopted; the
        // inter-hunk context still applies.
        change(cairn_ui::DiffSettings::more_lines);
        pump(&|_| shown_at(Context::Lines(6), true).is_some());
        repository.configure("[diff]\n\tcontext = 7\n\tinterHunkContext = 4\n");
        change(|settings| {
            settings.toggle_ignore_whitespace();
            true
        });
        let seen = pump(&|_| shown_at(Context::Lines(6), false) == Some(4));
        assert!(configured(&seen).contains(&Context::Lines(7)), "{seen:?}");
        assert_eq!(
            settings(),
            Context::Lines(6),
            "the configuration moved a context the user chose"
        );
        drop(handle);
    }
}
