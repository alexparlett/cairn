//! Applying a worker's update to the view state: the one place an `Update`
//! becomes what the window draws. On the UI thread, so nothing here waits.

use std::sync::Arc;

use freya::prelude::*;

use crate::PAGE_ROWS;
use crate::fetch_state::{FetchRefusal, FetchStatus, PromptView};
use crate::history_state::{self, Progress};
use crate::window::View;
use crate::worker::{PromptId, ReadAgain, Refreshed, Request, Retired, Update, expanded_diffs};

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

/// Applies `update` to `view`. A fetch or a local write ending takes down any
/// dialog — unless the other is still in flight, whose prompt it may be — and
/// refuses its prompt, which releases the helper of a git that is gone; a
/// fetch's ending asks for a refresh, which reopens the history if the fetch
/// moved what it draws, and a write's asks for what the write says to read
/// again (staging-and-commit R4.5). A prompt arriving while neither a fetch
/// nor a write is in flight — a helper orphaned by a killed git — is refused
/// rather than shown, since the dialog could not say what asked. A refresh's
/// refs that differ from those the history was walked from reopen it, keeping
/// the selection.
pub fn apply(update: Update, view: View, worker: &Worker<'_>) {
    let View {
        mut rows,
        mut progress,
        mut fetch,
        mut prompt,
        mut remotes,
        mut refused,
        mut diff,
        mut refreshed,
        mut repository,
        mut writes,
        mut activity,
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
            // A find in the sidebar looks through the rows that arrived (R8.5).
            crate::ref_find::pages_arrived(view, Some(worker.submit));
        }
        Update::Failed { message } | Update::WorkerLost { message } => {
            progress.write().failed(message);
            crate::ref_find::failed(view);
        }
        Update::Remotes { remotes: listed } => remotes.set(listed),
        Update::Opened { name } => repository.set(Some(name)),
        Update::LocksAtOpen { locks, index_lock } => {
            // An entry of their own in the activity popover (the user's decision H).
            activity.write().locks_at_open(&locks, index_lock);
            writes.write().locks_at_open(locks);
        }
        // What removing the stale lock would cost, read at the press (the user's decision H).
        Update::LockConsequence { asked, outcome } => {
            activity.write().lock_answered(asked, outcome);
        }
        // What a discard would lose, for the confirmation Local Changes opens (R8.4): kept only
        // for the discard asked last, and only while the view that asked it is shown — one
        // that arrives once the person has moved on is dropped rather than popped up later.
        Update::CommitReads(reads) => crate::commit_box_pane::reads_arrived(*reads, view),
        Update::BranchName { name, outcome } => {
            crate::create_branch::name_checked(view, name, outcome);
        }
        Update::CheckoutConsequence { asked, outcome } => {
            crate::create_branch::consequence_arrived(view, asked, outcome);
        }
        Update::Amending { status, read } => {
            crate::commit_box_pane::amend_arrived(status, *read, view, worker.submit);
        }
        Update::DiscardConsequence { asked, outcome } => {
            let mut acting = view.local.acting;
            if *view.sidebar.main.peek() == cairn_ui::MainView::LocalChanges {
                acting.write().consequence_arrived(asked, outcome);
            } else {
                acting.write().forget_discard();
            }
        }
        Update::ConfiguredContext { context } => {
            crate::diff_actions::configured(context, view, worker.submit);
        }
        Update::FetchStarted { remote } => {
            activity.write().fetch_started(&remote);
            fetch.write().started(remote);
        }
        Update::FetchProgress { line } => {
            let line = crate::shown_output::scrubbed(&line);
            let key = activity.peek().fetch_key();
            if let Some(key) = key {
                activity.write().output(key, std::slice::from_ref(&line));
            }
            fetch.write().progressed(line);
        }
        // What an operation ran, for the activity popover (R12.1).
        Update::OperationRan {
            by,
            commands,
            lock_named,
        } => {
            let key = match by {
                crate::worker::RanBy::Write(id) => Some(crate::activity::ActivityKey::Write(id)),
                crate::worker::RanBy::Fetch => activity.peek().fetch_key(),
            };
            if let Some(key) = key {
                activity.write().ran(key, &commands, lock_named);
            }
        }
        Update::FetchFinished { remote } => {
            activity
                .write()
                .fetch_ended(&remote, crate::activity::Outcome::Succeeded);
            if !writes.peek().is_running() {
                withdraw(&mut prompt, worker);
            }
            fetch.set(FetchStatus::Finished { remote });
            refresh_after_an_operation(worker);
        }
        Update::FetchCancelled {
            remote,
            stranded_locks,
        } => {
            activity
                .write()
                .fetch_ended(&remote, crate::activity::Outcome::Cancelled);
            if !writes.peek().is_running() {
                withdraw(&mut prompt, worker);
            }
            fetch.set(FetchStatus::Cancelled {
                remote,
                stranded_locks,
            });
            refresh_after_an_operation(worker);
        }
        Update::FetchFailed { remote, message } => {
            let message = crate::shown_output::scrubbed(&message);
            activity
                .write()
                .fetch_ended(&remote, crate::activity::Outcome::Failed(message.clone()));
            if !writes.peek().is_running() {
                withdraw(&mut prompt, worker);
            }
            fetch.set(FetchStatus::Failed { remote, message });
            refresh_after_an_operation(worker);
        }
        Update::WriteStarted { id } => {
            writes.write().started(id);
            if let Some(asked) = writes.peek().running() {
                activity.write().write_started(asked);
            }
            crate::commit_box_pane::write_started(id, view);
        }
        // The commit box's commit's output, kept for the Git Error dialog (R10.5), and every
        // write's for the activity popover (R10.4, R12.1).
        Update::WriteOutput { id, lines, .. } => {
            crate::commit_box_pane::write_output(id, &lines, view);
            activity
                .write()
                .output(crate::activity::ActivityKey::Write(id), &lines);
        }
        Update::WriteEnded {
            id,
            ending,
            read_again,
        } => {
            // The window closing asks nothing more of the worker.
            let asking: &dyn Fn(Request) = if worker.closing {
                &|_| {}
            } else {
                worker.submit
            };
            // git's words in it scrubbed before anything keeps or draws them (R12.2).
            let ending = crate::shown_output::shown_ending(ending);
            crate::commit_box_pane::write_ended(id, &ending, view, asking);
            crate::create_branch::write_ended(view, id, &ending);
            // A write that never started is named as it was asked (the user's decision N).
            let name = {
                let writes = writes.peek();
                writes
                    .queued()
                    .chain(writes.running())
                    .find(|asked| asked.id == id)
                    .map(|asked| asked.name.clone())
            };
            activity.write().write_ended(id, &ending, name);
            writes.write().ended(id, ending);
            if !fetch.peek().is_in_flight() {
                withdraw(&mut prompt, worker);
            }
            // What the write left stale and no more (R4.5): a read begun before it ended is
            // dropped by the worker (R4.4), so this is the one that shows it.
            if !worker.closing {
                (worker.submit)(match read_again {
                    ReadAgain::Everything => Request::Refresh,
                    ReadAgain::Status => Request::RefreshStatus,
                });
            }
        }
        // Kept for the views that draw it (phases 07-08); the snapshot it replaces is freed
        // on a worker. A history the refs no longer draw is reopened.
        Update::Refs { snapshot, reopen } => {
            // The sidebar's rows are asked for the snapshot that arrived (R8).
            let mut sidebar = view.sidebar.state;
            let asked = sidebar.write().refs_arrived(&snapshot);
            if !worker.closing {
                (worker.submit)(asked);
            }
            let replaced = refreshed.write().refs_arrived(snapshot);
            retire(replaced, worker);
            // What the commit box reads is asked again: a merge begun, a hook added, a commit
            // made (R10).
            if !worker.closing {
                crate::commit_box_pane::refs_arrived(view, worker.submit);
            }
            if reopen && !worker.closing {
                reopen_history(rows, progress, *view.show_lost.peek(), worker.submit);
                // A find looking in the history it replaced looks in the new one.
                crate::ref_find::reopened(view, Some(worker.submit));
            }
        }
        Update::AheadBehind { counts } => {
            let replaced = refreshed.write().ahead_behind_arrived(counts);
            retire(replaced, worker);
        }
        // Kept for the title bar and the sidebar's count; laid out for Local Changes' lists,
        // drawn at once or once a filter's rows are here (R9). What either lets go of is freed
        // on a worker.
        Update::Status { changes } => {
            // While Amend is ticked, the lists drawn are amend's over this status, asked for it
            // (R10.3); otherwise the status's own.
            let amending = !worker.closing
                && crate::commit_box_pane::status_arrived_amending(&changes, view, worker.submit);
            let mut local = view.local.state;
            let asked = if amending {
                Vec::new()
            } else {
                local.write().status_arrived(Arc::clone(&changes))
            };
            let replaced = refreshed.write().status_arrived(changes);
            for request in asked {
                if !worker.closing || matches!(request, Request::Retire(_)) {
                    (worker.submit)(request);
                }
            }
            retire(replaced, worker);
        }
        Update::FilteredLocalChanges {
            changes,
            text,
            rows,
        } => {
            let mut local = view.local.state;
            let requests = local.write().filtered(changes, &text, rows);
            for request in requests {
                (worker.submit)(request);
            }
        }
        Update::RefreshFailed { what, message } => {
            // With no refs ever read, the history has nothing to walk from: say why there.
            if what == Refreshed::Refs && refreshed.peek().refs().is_none() {
                progress.write().failed(message.clone());
            }
            refreshed.write().failed(what, message);
        }
        // The sidebar's rows, the latest asked: kept with the snapshot they index, the ones
        // they replace freed on a worker.
        Update::FilteredRefs { refs, rows, .. } => {
            let mut sidebar = view.sidebar.state;
            let replaced = sidebar.write().rows_arrived(refs, rows);
            retire(replaced, worker);
        }
        // Beside the fetch in flight, which it leaves as it was; the next press clears it.
        Update::FetchRefused { remote, reason } => {
            refused.set(Some(FetchRefusal { remote, reason }));
        }
        // Answered for whoever asks; no view draws the log in this packet (PRD R8.3).
        Update::CommandLog { .. } => {}
        // A fetch's, or a local write's — a hook, a signing key, an LFS filter (R5.1).
        Update::Prompt { id, text, asking } => {
            if fetch.peek().is_in_flight() || writes.peek().is_running() {
                prompt.set(Some(PromptView { id, text, asking }));
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
            match &query.target {
                crate::worker::FileTarget::WorkingTree { .. } => {
                    // Kept only for the path chosen in Local Changes, asked as it is now.
                    let (_, freeing) = if diff.peek().wants_file(&query) {
                        diff.write().working_arrived(&query, answer)
                    } else {
                        (
                            false,
                            Retired::of(None, answer.into_iter().collect()).map(Request::Retire),
                        )
                    };
                    if let Some(request) = freeing {
                        (worker.submit)(request);
                    }
                }
                crate::worker::FileTarget::Committed { .. } => {
                    if diff.peek().wants_file(&query) {
                        diff.write().file_arrived(&query, answer);
                    } else {
                        retire(Retired::of(None, answer.into_iter().collect()), worker);
                    }
                }
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
        // A page of the paths drawn together: kept only for the ask drawn now.
        Update::Together {
            asked,
            files,
            ended,
        } => {
            let freeing = diff.write().together_arrived(asked, files, ended);
            if let Some(request) = freeing {
                (worker.submit)(request);
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

/// An operation finished (a fetch, today): the refs, ahead/behind and status are read again
/// (R10.1), and the refresh decides whether the history is reopened — unless the window is
/// closing, when the worker that would answer has stopped.
fn refresh_after_an_operation(worker: &Worker<'_>) {
    if !worker.closing {
        (worker.submit)(Request::Refresh);
    }
}

/// The refs no longer draw the history on screen (R10.4): it is reopened from them. The rows
/// it held are handed to a worker to free rather than freed here (#52, R11.3), the new
/// history is sized for as many authors as the old named so its first pages do not rehash
/// their way back up, and the selection is left as it is — it is drawn again when its row
/// arrives (R10.5). With `lost`, the walk is Show Lost Commits' (staging-and-commit R11.4): a
/// reopen like any other, off the UI thread.
pub fn reopen_history(
    mut rows: State<cairn_model::History>,
    mut progress: State<Progress>,
    lost: bool,
    submit: &dyn Fn(Request),
) {
    let replaced = {
        let mut history = rows.write();
        let sized = cairn_model::History::with_author_capacity(history.author_count());
        std::mem::replace(&mut *history, sized)
    };
    progress.set(Progress::opening());
    // The new history asked first, so its first page is not queued behind the free of the
    // old one on the repository thread (phase 06 QA, RR4).
    submit(Request::OpenHistory {
        rows: PAGE_ROWS,
        lost,
    });
    submit(Request::Retire(Retired::history(replaced)));
}

/// Takes down a dialog whose fetch or write has ended, refusing the prompt so the
/// helper still waiting on it is released: the git behind it is gone or failing
/// anyway.
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
                        history_cursor: State::create(0),
                        detail_tab: State::create(cairn_ui::DetailTab::default()),
                        pane_collapsed: State::create(false),
                        pane_height: State::create(crate::window::PANE_HEIGHT),
                        diff_settings: State::create(cairn_ui::DiffSettings::default()),
                        diff_scroll: ScrollController::new(0, 0, Vec::new()),
                        change_cursor: State::create(None),
                        refreshed: State::create(crate::refresh_state::RefreshState::default()),
                        repository: State::create(None),
                        sidebar: crate::sidebar_state::SidebarView::created(),
                        local: crate::local_changes_state::LocalChangesView::created(),
                        writes: State::create(crate::local_writes::LocalWrites::default()),
                        confirming: State::create(None),
                        show_lost: State::create(false),
                        activity: State::create(crate::activity::ActivityLog::default()),
                        branch: crate::create_branch::CreateBranchView::created(),
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

    /// Every way a fetch can end, none of them saying whether a ref moved: the refresh asked
    /// after it finds out.
    fn fetch_endings() -> [Update; 3] {
        [
            Update::FetchFinished {
                remote: "origin".to_owned(),
            },
            Update::FetchCancelled {
                remote: "origin".to_owned(),
                stranded_locks: Vec::new(),
            },
            Update::FetchFailed {
                remote: "origin".to_owned(),
                message: "some local refs could not be updated".to_owned(),
            },
        ]
    }

    /// A fetch the close itself ended may still have moved refs. Caught by: refreshing
    /// anyway — a request sent to a worker that has stopped, and a reopen that would clear
    /// the history while the window closes.
    #[test]
    fn a_fetch_ended_by_the_close_asks_for_nothing() {
        // However it ended: the close may land just as a fetch finishes or fails.
        for ending in fetch_endings() {
            let (test, view, asked) = launch(running());
            applying_while(&test, view, &asked, ending.clone(), true);
            assert_eq!(
                view.rows.read().len(),
                3,
                "the rows were cleared on the way out: {ending:?}"
            );
            assert!(
                asked.submitted.borrow().is_empty(),
                "a closing repository was asked something: {ending:?}"
            );
        }
    }

    fn running() -> FetchStatus {
        FetchStatus::Running {
            remote: "origin".to_owned(),
            line: None,
        }
    }

    /// R10.1 and R10.4, the fetch's half: every ending — finished, cancelled, failed, since
    /// one that failed or was killed may have moved refs before it stopped — asks for a
    /// refresh and does nothing else; whether the history is reopened is the refresh's to
    /// decide. The fetch's own tip comparison is gone: no ending carries whether a ref moved
    /// (the field is gone from the type), clears a row or asks for the history. Caught by:
    /// the old `reload_if` left running beside the refresh (a second reopen, rows cleared on
    /// the UI thread), or an ending that does not refresh (a fetch that moved refs leaves the
    /// graph stale).
    #[test]
    fn every_fetch_ending_asks_for_a_refresh_and_touches_no_row() {
        for ending in fetch_endings() {
            let (test, view, asked) = launch(running());
            let progress = view.progress.read().clone();
            applying(&test, view, &asked, ending.clone());
            assert_eq!(
                asked.submitted.borrow().as_slice(),
                [Request::Refresh],
                "{ending:?}"
            );
            assert_eq!(view.rows.read().len(), 3, "{ending:?} touched the rows");
            assert_eq!(
                *view.progress.read(),
                progress,
                "{ending:?} reset the history"
            );
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
        for ending in fetch_endings() {
            let (test, view, asked) = launch(running());
            let id = PromptId::for_tests(9);
            applying(
                &test,
                view,
                &asked,
                Update::Prompt {
                    id,
                    text: "Password for 'https://h/x': ".to_owned(),
                    asking: Some("origin".to_owned()),
                },
            );
            assert!(view.prompt.read().is_some(), "the prompt was not shown");
            applying(&test, view, &asked, ending);
            assert_eq!(*view.prompt.read(), None, "the dialog stayed up");
            assert_eq!(asked.refused.borrow().as_slice(), [id]);
        }
    }

    /// Caught by: showing a prompt with neither a fetch nor a write in flight, which the
    /// dialog could only attribute to "git".
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
                asking: Some("origin".to_owned()),
            },
        );
        assert_eq!(*view.prompt.read(), None);
        assert_eq!(asked.refused.borrow().as_slice(), [id]);
    }

    /// A write's id and its news, as the lane sends them.
    fn write_news(id: crate::worker::OperationId, read_again: ReadAgain) -> [Update; 2] {
        [
            Update::WriteStarted { id },
            Update::WriteEnded {
                id,
                ending: crate::worker::WriteEnding::Refused {
                    message: "nothing selected".to_owned(),
                },
                read_again,
            },
        ]
    }

    /// Staging-and-commit R4.5: a write's ending asks for what it says to read again and no
    /// more — status alone after a stage, everything after a commit (or a refresh it kept
    /// back) — once, and nothing while the window closes. Caught by: a full refresh after
    /// every write (the refs and the history read again for a stage), none at all (the
    /// write's result never drawn), or a request to a worker that has stopped.
    #[test]
    fn a_writes_ending_reads_again_what_it_says_and_no_more() {
        for (read_again, expected) in [
            (ReadAgain::Status, Request::RefreshStatus),
            (ReadAgain::Everything, Request::Refresh),
        ] {
            let (test, view, asked) = launch(FetchStatus::Idle);
            let id = crate::worker::OperationId::for_tests(3);
            for update in write_news(id, read_again) {
                applying(&test, view, &asked, update);
            }
            assert_eq!(asked.submitted.borrow().as_slice(), [expected]);
            assert!(!view.writes.read().is_running());

            let (test, view, asked) = launch(FetchStatus::Idle);
            for update in write_news(id, read_again) {
                applying_while(&test, view, &asked, update, true);
            }
            assert!(
                asked.submitted.borrow().is_empty(),
                "a closing repository was asked to read again"
            );
        }
    }

    /// R5.1: a prompt is shown while a local write runs, as during a fetch, and the write's
    /// ending takes it down — unless a fetch is in flight, whose prompt it may be; a fetch's
    /// ending leaves one up while a write runs, for the same reason. Caught by: a prompt
    /// refused during a write (a hook or a signing key can never ask), or one ending taking
    /// down the other's prompt.
    #[test]
    fn a_prompt_is_shown_while_a_write_runs_and_its_ending_takes_it_down() {
        let id = crate::worker::OperationId::for_tests(5);
        let prompt = || Update::Prompt {
            id: PromptId::for_tests(8),
            text: "Enter passphrase for key '/k': ".to_owned(),
            asking: Some("Commit".to_owned()),
        };
        let [started, ended] = write_news(id, ReadAgain::Status);

        let (test, view, asked) = launch(FetchStatus::Idle);
        applying(&test, view, &asked, started.clone());
        applying(&test, view, &asked, prompt());
        assert!(view.prompt.read().is_some(), "a write's prompt was refused");
        applying(&test, view, &asked, ended.clone());
        assert_eq!(*view.prompt.read(), None, "the dialog outlived its write");
        assert_eq!(asked.refused.borrow().as_slice(), [PromptId::for_tests(8)]);

        // A fetch in flight: the write's ending leaves the prompt, which may be the fetch's.
        let (test, view, asked) = launch(running());
        applying(&test, view, &asked, started.clone());
        applying(&test, view, &asked, prompt());
        applying(&test, view, &asked, ended);
        assert!(
            view.prompt.read().is_some(),
            "a write's ending took the fetch's prompt"
        );
        assert!(asked.refused.borrow().is_empty());

        // A write running: a fetch's ending leaves the prompt, which may be the write's.
        for ending in fetch_endings() {
            let (test, view, asked) = launch(running());
            applying(&test, view, &asked, started.clone());
            applying(&test, view, &asked, prompt());
            applying(&test, view, &asked, ending.clone());
            assert!(
                view.prompt.read().is_some(),
                "{ending:?} took a running write's prompt"
            );
        }
    }

    /// R4.9: the lock files present as the repository opened are kept for the window to name.
    #[test]
    fn the_locks_found_as_the_repository_opened_are_kept_for_the_window() {
        let lock = std::path::PathBuf::from("/r/.git/index.lock");
        let (test, view, asked) = launch(FetchStatus::Idle);
        applying(
            &test,
            view,
            &asked,
            Update::LocksAtOpen {
                locks: vec![lock.clone()],
                index_lock: true,
            },
        );
        assert_eq!(view.writes.read().locks(), [lock]);
        assert!(asked.submitted.borrow().is_empty());
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

    fn snapshot_naming(commit: Oid) -> std::sync::Arc<cairn_model::RefsSnapshot> {
        std::sync::Arc::new(cairn_model::RefsSnapshot {
            refs: vec![cairn_model::Ref {
                name: cairn_model::RefName::new("refs/heads/main"),
                kind: cairn_model::RefKind::LocalBranch,
                target: cairn_model::RefTarget::Commit(commit),
                symbolic: None,
                upstream: None,
            }],
            head: cairn_model::HeadState::Branch(cairn_model::RefName::new("refs/heads/main")),
            stashes: Vec::new(),
            unreadable: 0,
        })
    }

    /// R10.4, R10.5, R11.3 and #52, the window's half: refs that draw another history reopen
    /// it — the old rows handed to a worker to free as one `Request::Retire`, never freed on
    /// the UI thread, the new history sized for the authors the old one named, the history
    /// asked for again, and the selection left as it was; refs that draw the same history
    /// leave it, handing back only the snapshot they replace. Caught by: the old rows
    /// replaced in place (no retirement), a reopen with `History::new` (an index with no
    /// room), the selection cleared, or a reopen on a refresh that said not to.
    #[test]
    fn a_reopen_frees_the_old_rows_on_a_worker_and_keeps_the_selection() {
        use crate::history_state::Status;

        let (test, mut view, asked) = launch(FetchStatus::Idle);
        let chosen = Some(cairn_model::RowId::Commit(oid(1)));
        test.run_in(|| view.selected.set(chosen));
        applying(
            &test,
            view,
            &asked,
            Update::Refs {
                snapshot: snapshot_naming(oid(2)),
                reopen: true,
            },
        );
        let submitted: Vec<Request> = asked.submitted.borrow_mut().drain(..).collect();
        // The sidebar's rows asked for the snapshot first (refs-and-status R8), then the reopen.
        match submitted.as_slice() {
            [
                Request::FilterRefs { .. },
                Request::OpenHistory { rows, lost: false },
                Request::Retire(retired),
            ] => {
                assert_eq!(retired.replaced_rows(), Some(3), "not the old rows");
                assert_eq!(*rows, PAGE_ROWS);
            }
            other => panic!("expected the old rows retired and the history asked, got {other:?}"),
        }
        assert!(view.rows.read().is_empty(), "the old rows are still drawn");
        assert_eq!(view.progress.read().status(), &Status::Loading);
        assert_eq!(
            *view.selected.read(),
            chosen,
            "the reopen let go of the selection"
        );
        assert!(
            view.rows.read().retained().authors > History::new().retained().authors,
            "the new history was not sized for the authors the old one named"
        );

        // The same history drawn: the rows stay, and the snapshot replaced goes to a worker.
        let first = snapshot_naming(oid(2));
        test.run_in(|| {
            view.refreshed
                .write()
                .refs_arrived(std::sync::Arc::clone(&first))
        });
        applying(
            &test,
            view,
            &asked,
            Update::Refs {
                snapshot: snapshot_naming(oid(2)),
                reopen: false,
            },
        );
        match asked.submitted.borrow().as_slice() {
            [Request::FilterRefs { .. }, Request::Retire(retired)] => {
                assert_eq!(retired.refs_snapshot(), Some(&*first));
            }
            other => panic!("expected only the replaced snapshot retired, got {other:?}"),
        }
    }

    /// A refresh answered while the window closes reopens nothing: the worker that would
    /// answer has stopped. Caught by: reopening anyway, clearing the history on the way out.
    #[test]
    fn a_refresh_answered_while_closing_reopens_nothing() {
        let (test, view, asked) = launch(FetchStatus::Idle);
        applying_while(
            &test,
            view,
            &asked,
            Update::Refs {
                snapshot: snapshot_naming(oid(2)),
                reopen: true,
            },
            true,
        );
        assert_eq!(view.rows.read().len(), 3);
        assert!(
            asked.submitted.borrow().is_empty(),
            "{:?}",
            asked.submitted.borrow()
        );
    }

    /// R11.3: each refresh answer the window replaces — ahead/behind, a status — is handed
    /// to a worker to free; a failed read is said beside the answer kept, which stays; a
    /// failure to read the refs before any were read is the history's failure too, since it
    /// has nothing to walk from. Caught by: a replaced answer dropped in place, a failure
    /// that throws the kept answer away, or a first failure that leaves "Reading history…"
    /// up for good.
    #[test]
    fn a_replaced_refresh_answer_is_freed_on_a_worker_and_a_failure_keeps_the_last() {
        use cairn_model::{AheadBehind, RefName, StatusEntry, WorkingTreeStatus};

        use crate::history_state::Status;
        use crate::worker::Refreshed;

        let (test, view, asked) = launch(FetchStatus::Idle);
        applying(
            &test,
            view,
            &asked,
            Update::RefreshFailed {
                what: Refreshed::Refs,
                message: "the refs could not be listed".to_owned(),
            },
        );
        assert_eq!(
            view.progress.read().status(),
            &Status::Failed("the refs could not be listed".to_owned())
        );

        let counts = vec![(
            RefName::new("refs/heads/main"),
            AheadBehind {
                ahead: 1,
                behind: 0,
            },
        )];
        let status = WorkingTreeStatus::Listed(vec![StatusEntry::Untracked(
            cairn_model::RepoPath::from("a.txt"),
        )]);
        for _ in 0..2 {
            applying(
                &test,
                view,
                &asked,
                Update::AheadBehind {
                    counts: counts.clone(),
                },
            );
            applying(
                &test,
                view,
                &asked,
                Update::Status {
                    changes: std::sync::Arc::new(cairn_model::LocalChanges::new(status.clone())),
                },
            );
        }
        // The status replaced is let go of by both its holders — the refresh's answers and
        // Local Changes' lists — each handing its hold to a worker.
        match asked.submitted.borrow().as_slice() {
            [
                Request::Retire(counted),
                Request::Retire(drawn),
                Request::Retire(read),
            ] => {
                assert_eq!(counted.counts(), counts.as_slice());
                assert_eq!(drawn.working_tree_status(), Some(&status));
                assert_eq!(read.working_tree_status(), Some(&status));
            }
            other => panic!("expected the replaced answers retired, got {other:?}"),
        }

        applying(
            &test,
            view,
            &asked,
            Update::RefreshFailed {
                what: Refreshed::Status,
                message: "git status failed".to_owned(),
            },
        );
        let kept = view.refreshed.read();
        assert_eq!(
            kept.status(),
            Some(&status),
            "the failure threw the answer away"
        );
        assert_eq!(kept.failure(Refreshed::Status), Some("git status failed"));
        assert_eq!(
            kept.ahead_behind().map(|kept| kept.as_slice()),
            Some(counts.as_slice())
        );
        assert_eq!(kept.failure(Refreshed::AheadBehind), None);
    }

    /// C10 through the real boundary, counting reopens: the window's first refresh opens the
    /// history; a refresh that finds nothing changed asks for nothing; a stash pushed — the
    /// stash list alone changed — reopens it, and so does a checkout that moves no ref
    /// (`HEAD` onto `other`, on `HEAD`'s commit) and a branch moved; nothing changed again
    /// reopens nothing. Each reopen is counted as the window asks for it, from what the
    /// worker answered. Caught by: a comparison that leaves out the stash list or `HEAD`'s
    /// state (a step that should reopen does not), one that reopens on anything (a step that
    /// should not, does), or the old fetch comparison standing in for the refresh.
    #[test]
    fn a_refresh_reopens_for_a_stash_a_checkout_and_a_moved_ref_and_for_nothing_else() {
        use crate::worker::{Refreshable, next_update};

        let fixture = Refreshable::new("cairn-session-reopens");
        let (handle, mut updates) = fixture.open();
        let (test, view, asked) = launch(FetchStatus::Idle);
        let mut opens = 0;
        // Asks for a refresh and applies what arrives, handing on what applying asks, until
        // the refresh's refs have been applied and — if they reopened the history — its first
        // page has too. Returns how many reopens the window asked for.
        let mut refresh = || -> usize {
            handle.submit(Request::Refresh);
            let mut refs_seen = false;
            let mut reopening = false;
            let mut asked_open = 0;
            loop {
                for request in asked.submitted.borrow_mut().drain(..) {
                    if matches!(request, Request::OpenHistory { .. }) {
                        asked_open += 1;
                        reopening = true;
                    }
                    handle.submit(request);
                }
                if refs_seen && !reopening {
                    return asked_open;
                }
                let update = next_update(&mut updates);
                match &update {
                    Update::Refs { .. } => refs_seen = true,
                    Update::Rows { .. } => reopening = false,
                    _ => {}
                }
                applying(&test, view, &asked, update);
            }
        };

        opens += refresh();
        assert_eq!(opens, 1, "the first refresh did not open the history");
        assert!(!view.rows.read().is_empty());
        assert_eq!(
            refresh(),
            0,
            "a refresh that found nothing changed reopened"
        );

        fixture.push_stash(fixture.commits[2], "On main: wip");
        assert_eq!(refresh(), 1, "a changed stash list did not reopen");

        fixture.check_out("other");
        assert_eq!(refresh(), 1, "a checkout that moves no ref did not reopen");

        fixture.point("topic", fixture.commits[3]);
        assert_eq!(refresh(), 1, "a moved ref did not reopen");

        assert_eq!(refresh(), 0, "nothing changed, yet the history reopened");
        drop(handle);
    }
}
