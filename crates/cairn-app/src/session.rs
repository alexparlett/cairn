//! Applying a worker's update to the view state: the one place an `Update`
//! becomes what the window draws. On the UI thread, so nothing here waits.

use std::sync::Arc;

use freya::prelude::*;

use crate::PAGE_ROWS;
use crate::history::state::{self, Progress};
use crate::operations::fetch::{FetchStatus, PromptView};
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
/// again. A prompt arriving while neither a fetch
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
            let widest = state::widest_lane(&page);
            // The first page of a walk replacing the rows held: it replaces them, in a history
            // sized for the authors they named, so its first pages do not rehash their way back
            // up; they go to a worker to free (#52).
            let reopened = progress.peek().is_reopening();
            let (loaded, held, replaced) = {
                let mut history = rows.write();
                let replaced = reopened.then(|| {
                    let sized = cairn_model::History::with_author_capacity(history.author_count());
                    std::mem::replace(&mut *history, sized)
                });
                let held = history.append(page);
                (history.len(), held, replaced)
            };
            if reopened {
                progress.set(Progress::opening());
            }
            progress.write().appended(widest, complete, loaded, held);
            if let Some(replaced) = replaced
                && !worker.closing
            {
                (worker.submit)(Request::Retire(Retired::history(replaced)));
            }
            // A find in the sidebar looks through the rows that arrived.
            crate::history::ref_find::pages_arrived(view, Some(worker.submit));
        }
        // How far the walk being opened has read: drawn as "Reading history… N commits".
        Update::Reading { commits } => progress.write().reading(commits),
        Update::Failed { message } | Update::WorkerLost { message } => {
            progress.write().failed(message);
            crate::history::ref_find::failed(view);
        }
        Update::Remotes { remotes: listed } => remotes.set(listed),
        Update::Started { git, zone } => {
            let mut found = view.frame.git;
            found.set(Some(git));
            // The zone every date is drawn in from now on; UTC, said so, when none was read.
            let mut drawn_in = view.zone;
            drawn_in
                .set(zone.map_or_else(cairn_ui::LocalZone::unreadable, cairn_ui::LocalZone::read));
        }
        Update::Opened { name, path } => {
            repository.set(Some(name));
            let mut root = view.repository_path;
            root.set(Some(path));
        }
        // The leftover lock's stat, from every refresh.
        Update::IndexLock { changed } => {
            let mut lock = view.lock;
            lock.write().stated(changed);
        }
        // What removing the stale lock would cost, read at the press.
        Update::LockConsequence { asked, outcome } => {
            crate::operations::leftover_lock::answered(view, asked, outcome);
        }
        // What a discard would lose, for the confirmation Local Changes opened at the press:
        // kept only for the discard asked last and only while that confirmation is
        // still open and counting — one let go of drops it.
        Update::CommitReads(reads) => crate::commit_box::pane::reads_arrived(*reads, view),
        Update::BranchName { name, outcome } => {
            crate::operations::create_branch::name_checked(view, name, outcome);
        }
        Update::CheckoutConsequence { asked, outcome } => {
            crate::operations::create_branch::consequence_arrived(view, asked, outcome);
        }
        Update::Amending { status, read } => {
            crate::commit_box::pane::amend_arrived(status, *read, view, worker.submit);
        }
        Update::DiscardConsequence { asked, outcome } => {
            crate::local_changes::actions::consequence_arrived(view, asked, outcome);
        }
        Update::ConfiguredContext { context } => {
            crate::diff::actions::configured(context, view, worker.submit);
        }
        Update::FetchStarted { remote } => {
            activity.log.write().fetch_started(&remote);
            fetch.write().started(remote);
        }
        // The fetch wrote: whatever waits in its lane's tap, drained once (`output_tap`).
        Update::FetchProgress { tap } => {
            let drained = tap.drain();
            let key = activity.log.peek().fetch_key();
            if let Some(key) = key {
                activity.log.write().output(key, &drained);
            }
            // The status box draws the latest redraw.
            let latest = drained
                .iter()
                .rev()
                .find_map(|lines| lines.spoken().next_back());
            if let Some(line) = latest {
                fetch.write().progressed(line.to_owned());
            }
        }
        // What an operation ran, for the activity popover.
        Update::OperationRan { by, commands } => {
            let key = match by {
                crate::worker::RanBy::Write(id) => {
                    Some(crate::operations::activity::ActivityKey::Write(id))
                }
                crate::worker::RanBy::Fetch => activity.log.peek().fetch_key(),
            };
            if let Some(key) = key {
                activity.log.write().ran(key, &commands);
            }
        }
        Update::FetchFinished { remote } => {
            activity
                .log
                .write()
                .fetch_ended(&remote, crate::operations::activity::Outcome::Succeeded);
            if !writes.peek().is_running() {
                withdraw(&mut prompt, worker);
            }
            fetch.set(FetchStatus::Finished { remote });
            refresh_after_an_operation(worker);
        }
        // A cancel opens nothing.
        Update::FetchCancelled { remote } => {
            activity.log.write().fetch_ended(
                &remote,
                crate::operations::activity::Outcome::Cancelled(
                    crate::operations::activity::FETCH_CANCELLED.to_owned(),
                ),
            );
            if !writes.peek().is_running() {
                withdraw(&mut prompt, worker);
            }
            fetch.set(FetchStatus::Cancelled { remote });
            refresh_after_an_operation(worker);
        }
        // git's failure opens its Git Error, as every operation the person started does.
        Update::FetchFailed { remote, failure } => {
            activity.log.write().fetch_ended(
                &remote,
                crate::operations::activity::Outcome::Failed(failure.said.clone()),
            );
            if !writes.peek().is_running() {
                withdraw(&mut prompt, worker);
            }
            let mut endings = view.endings;
            endings
                .write()
                .push(crate::operations::ending_dialog::of_failure(
                    &format!("Fetch '{remote}'"),
                    &failure,
                ));
            fetch.set(FetchStatus::Failed { remote });
            refresh_after_an_operation(worker);
        }
        Update::WriteStarted { id } => {
            writes.write().started(id);
            if let Some(asked) = writes.peek().running() {
                activity.log.write().write_started(asked);
            }
            crate::commit_box::pane::write_started(id, view);
        }
        // The commit box's commit's output, kept for the Git Error dialog, and every
        // write's for the activity popover.
        // Drained once from the local lane's tap, whatever waits (`output_tap`).
        Update::WriteOutput { id, tap } => {
            let drained = tap.drain();
            for lines in drained.iter() {
                crate::commit_box::pane::write_output(id, lines, view);
            }
            activity.log.write().output(
                crate::operations::activity::ActivityKey::Write(id),
                &drained,
            );
        }
        Update::WriteEnded {
            id,
            ending,
            read_again,
            mut unspent,
        } => {
            // The window closing asks nothing more of the worker.
            let asking: &dyn Fn(Request) = if worker.closing {
                &|_| {}
            } else {
                worker.submit
            };
            let the_boxs =
                crate::commit_box::pane::write_ended(id, &ending, &mut unspent, view, asking);
            crate::operations::create_branch::write_ended(view, id, &ending, asking);
            // A write that never started is named as it was asked, and a destructive one quotes
            // the prompt it was asked with however it ended.
            let asked = {
                let writes = writes.peek();
                writes
                    .queued()
                    .chain(writes.running())
                    .find(|asked| asked.id == id)
                    .cloned()
            };
            // Every ending to its one home: the commit box's Git Error is its own; any
            // other failure or refusal waits its turn in the endings' dialog, by the write's one
            // name; a success or a cancel opens nothing.
            if !the_boxs {
                let name = asked
                    .as_ref()
                    .map_or_else(|| "A write".to_owned(), |asked| asked.name.clone());
                if let Some(shown) = crate::operations::ending_dialog::of_write(&name, &ending) {
                    let mut endings = view.endings;
                    endings.write().push(shown);
                }
            }
            activity
                .log
                .write()
                .write_ended(id, &ending, asked.as_ref());
            writes.write().ended(id);
            if !fetch.peek().is_in_flight() {
                withdraw(&mut prompt, worker);
            }
            // What the write left stale and no more: a read begun before it ended is dropped
            // by the worker, so this is the one that shows it.
            if !worker.closing {
                (worker.submit)(match read_again {
                    ReadAgain::Everything => Request::Refresh,
                    ReadAgain::Status => Request::RefreshStatus,
                });
            }
        }
        // Kept for the views that draw it; the snapshot it replaces is freed
        // on a worker. A history the refs no longer draw is reopened.
        Update::Refs { snapshot, reopen } => {
            // The sidebar's rows are asked for the snapshot that arrived.
            let mut sidebar = view.sidebar.state;
            let asked = sidebar.write().refs_arrived(&snapshot);
            if !worker.closing {
                (worker.submit)(asked);
            }
            // What git substitutes as it reads a commit changed: the chosen commit's details and
            // changes are asked again, as git would answer them now.
            let substituted = refreshed
                .peek()
                .refs()
                .is_some_and(|kept| kept.substitutions != snapshot.substitutions);
            let replaced = refreshed.write().refs_arrived(snapshot);
            retire(replaced, worker);
            if substituted && !worker.closing {
                crate::history::selection::ask_again(view, Some(worker.submit));
            }
            // What the commit box reads is asked again: a merge begun, a hook added, a commit
            // made.
            if !worker.closing {
                crate::commit_box::pane::refs_arrived(view, worker.submit);
            }
            if reopen && !worker.closing {
                reopen_history(progress, *view.show_lost.peek(), worker.submit);
                // A find looking in the history it replaced looks in the new one.
                crate::history::ref_find::reopened(view, Some(worker.submit));
            }
        }
        Update::AheadBehind { counts } => {
            let replaced = refreshed.write().ahead_behind_arrived(counts);
            retire(replaced, worker);
        }
        // Kept for the sidebar's count, the commit box and Create Branch; laid out for Local
        // Changes' lists, drawn at once or once a filter's rows are here. What either lets go of
        // is freed on a worker.
        Update::Status { changes } => {
            // Every status lands in the lists' state; while Amend is ticked it asks amend's lists
            // over it, its overlay.
            let mut local = view.local.state;
            let asked = local.write().status_arrived(Arc::clone(&changes));
            let replaced = refreshed.write().status_arrived(changes);
            for request in asked {
                if !worker.closing || matches!(request, Request::Retire(_)) {
                    (worker.submit)(request);
                }
            }
            retire(replaced, worker);
            // The selection moved by the rule as the lists were drawn: its primary's
            // diff asked here, as the status arrives, never by a render effect.
            if !worker.closing {
                crate::diff::actions::show_primary(view, Some(worker.submit), false);
            }
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
            // The filter pruned the selection to the rows it shows: the primary's diff.
            if !worker.closing {
                crate::diff::actions::show_primary(view, Some(worker.submit), false);
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
        // Never drawn: Fetch is greyed while a fetch runs, so only a second press racing
        // the first's start reaches the lane's refusal, and the fetch in flight says the rest.
        Update::FetchRefused { .. } => {}
        // Answered for whoever asks; no view draws the log.
        Update::CommandLog { .. } => {}
        // A fetch's, or a local write's — a hook, a signing key, an LFS filter.
        Update::Prompt { id, text, asking } => {
            if fetch.peek().is_in_flight() || writes.peek().is_running() {
                prompt.set(Some(PromptView { id, text, asking }));
            } else {
                (worker.refuse)(id);
            }
        }
        // Each kept only for the selection it names; checked before the write,
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
                        let drawn = |diff: &crate::diff::state::DiffState| {
                            diff.shown_working().map(cairn_model::ShownDiff::content)
                        };
                        let before = drawn(&diff.peek());
                        let kept = diff.write().working_arrived(&query, answer);
                        lines_follow(view, before, drawn(&diff.peek()));
                        kept
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
        Update::DiffFailed { query, message } => {
            if diff.peek().wants(&query) {
                diff.write().failed(&query, message);
            }
        }
        Update::Superseded(retired) => retire(Some(retired), worker),
    }
}

/// The primary's diff was drawn again: a finished line selection made over
/// the text drawn `before` is kept while the text drawn `after` is the same — its content equal,
/// so it names the same lines — and is let go of when it differs, the diff's bar saying so once
/// (`cairn_ui::SELECTION_CLEARED`) until the next key or press. A selection over no text drawn
/// before — another path's, or the same at other options, each let go of as it was asked — says
/// nothing.
fn lines_follow(view: View, before: Option<u64>, after: Option<u64>) {
    let mut lines = view.local.lines;
    let live = before.is_some_and(|content| lines.peek().selected(content).is_some());
    if live && after != before {
        lines.set(cairn_ui::LineDrag::cleared_by_a_change());
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

/// An operation finished (a fetch, today): the refs, ahead/behind and status are read again,
/// and the refresh decides whether the history is reopened — unless the window is
/// closing, when the worker that would answer has stopped.
fn refresh_after_an_operation(worker: &Worker<'_>) {
    if !worker.closing {
        (worker.submit)(Request::Refresh);
    }
}

/// The refs draw another history than the one on screen: it is reopened from them, on the
/// history thread, superseding a walk still opening there. The rows held stay drawn and
/// usable, paging no further, until the new walk's first page replaces them (`apply`'s
/// `Update::Rows`), when they are handed to a worker to free; the selection is left as it is
/// — it is drawn again when its row arrives. With `lost`, the walk is Show Lost Commits': a
/// reopen like any other, off the UI thread.
pub fn reopen_history(mut progress: State<Progress>, lost: bool, submit: &dyn Fn(Request)) {
    progress.write().reopen();
    submit(Request::OpenHistory {
        rows: PAGE_ROWS,
        lost,
    });
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
                        endings: State::create(Default::default()),
                        lock: State::create(Default::default()),
                        diff: State::create(crate::diff::state::DiffState::default()),
                        filter_text: State::create(String::new()),
                        changes_list_width: State::create(crate::detail::changes_tab::LIST_WIDTH),
                        pair: State::create(None),
                        held_keys: State::create(cairn_ui::accelerators::HeldKeys::default()),
                        history_scroll: ScrollController::new(0, 0, Vec::new()),
                        history_cursor: State::create(0),
                        detail_tab: State::create(cairn_ui::DetailTab::default()),
                        pane_collapsed: State::create(false),
                        pane_split: State::create(crate::detail::split::PaneSplit::default()),
                        diff_settings: State::create(cairn_ui::DiffSettings::default()),
                        diff_scroll: ScrollController::new(0, 0, Vec::new()),
                        change_cursor: State::create(None),
                        refreshed: State::create(crate::refresh::state::RefreshState::default()),
                        repository: State::create(None),
                        repository_path: State::create(None),
                        sidebar: crate::sidebar::state::SidebarView::created(),
                        local: crate::local_changes::state::LocalChangesView::created(),
                        writes: State::create(
                            crate::operations::local_writes::LocalWrites::default(),
                        ),
                        confirming: State::create(None),
                        show_lost: State::create(false),
                        zone: State::create(cairn_ui::LocalZone::default()),
                        activity: crate::operations::activity::ActivityView::created(),
                        branch: crate::operations::create_branch::CreateBranchView::created(),
                        frame: crate::frame::FrameView::created(),
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

    /// Through the real boundary: an answer naming another selection is never
    /// drawn, even when its epoch is still current — the selection was cleared without
    /// asking anything new, so the change set arrives through the worker under a current
    /// epoch and only the window's own check can refuse it. The negative follows: asked
    /// again and selected, it is kept. Mutation that reddens it: this arm storing whatever
    /// change set arrives rather than asking `DiffState` whether it names the selection
    /// (each of `DiffState`'s two checks alone is decided by `diff::state`'s own tests).
    #[test]
    fn an_answer_naming_another_selection_is_never_drawn() {
        use crate::diff::state::Answer;
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

    /// The other three diff arms: a file's diff, Expand All's batch and a failure are
    /// each refused with nothing selected and kept once their selection is made. Synthetic
    /// answers, since what this test decides is the window's own check. Caught by: an arm
    /// that stores whatever arrives (selecting what it names first), or one that drops
    /// even its own selection's answer.
    #[test]
    fn every_diff_answer_is_kept_for_its_selection_alone() {
        use cairn_model::{
            ChangeStatus, ChangedFile, DiffContent, FileDiff, FileMode, Oid, RepoPath,
        };

        use crate::diff::state::{Answer, answered_expansion};
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

    /// An answer the window will not keep is freed on a worker, never on
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
            },
            Update::FetchFailed {
                remote: "origin".to_owned(),
                failure: crate::worker::Failure {
                    said: cairn_ui::GIT_ERROR_TEXT.to_owned(),
                    command: Some("git fetch --progress origin".to_owned()),
                    output: cairn_model::ScrubbedLines::scrubbing(
                        "error: some local refs could not be updated",
                    ),
                    locks: Vec::new(),
                    index_lock: false,
                },
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

    /// The fetch's half: every ending — finished, cancelled, failed, since one that failed or
    /// was killed may have moved refs before it stopped — asks for a refresh and does nothing
    /// else; whether the history is reopened is the refresh's to decide. No ending carries
    /// whether a ref moved, clears a row or asks for the history. Caught by: a reload of the
    /// fetch's own beside the refresh (a second reopen, rows cleared on the UI thread), or an
    /// ending that does not refresh (a fetch that moved refs leaves the graph stale).
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

    /// A fetch cancelled opens nothing, and one git failed opens its Git Error,
    /// named for the fetch, with its command. Caught by: a cancel drawn, or a fetch's failure
    /// said anywhere but its dialog.
    #[test]
    fn a_cancelled_fetch_opens_nothing_and_a_failed_one_its_git_error() {
        let (test, view, asked) = launch(running());
        let [_, cancelled, failed] = fetch_endings();
        applying(&test, view, &asked, cancelled);
        assert_eq!(
            *view.fetch.read(),
            FetchStatus::Cancelled {
                remote: "origin".to_owned(),
            }
        );
        assert!(view.endings.read().is_empty(), "a cancel opened a dialog");
        let (test, view, asked) = launch(running());
        applying(&test, view, &asked, failed);
        let endings = view.endings.read();
        let shown = endings.shown().cloned();
        assert_eq!(
            shown.as_ref().map(|shown| shown.title.as_str()),
            Some(cairn_ui::GIT_ERROR_TITLE)
        );
        assert_eq!(
            shown
                .and_then(|shown| shown.details)
                .map(|(command, _)| command),
            Some("git fetch --progress origin".to_owned())
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
                    said: "nothing selected".to_owned(),
                },
                read_again,
                unspent: crate::worker::Unspent::none(),
            },
        ]
    }

    /// A write's ending asks for what it says to read again and no
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

    /// A prompt is shown while a local write runs, as during a fetch, and the write's
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

    /// The leftover lock's stat is kept for its banner, and a refused fetch — which the
    /// greyed Fetch never asks — changes nothing. Caught by: the stat dropped between the worker
    /// and the banner, or a refusal ending the fetch in flight.
    #[test]
    fn the_lock_stat_is_kept_and_a_refused_fetch_changes_nothing() {
        let (test, view, asked) = launch(running());
        let changed = std::time::SystemTime::UNIX_EPOCH;
        applying(
            &test,
            view,
            &asked,
            Update::IndexLock {
                changed: Some(changed),
            },
        );
        assert_eq!(view.lock.read().changed(), Some(changed));
        applying(
            &test,
            view,
            &asked,
            Update::FetchRefused {
                remote: "origin".to_owned(),
                reason: "a fetch of origin is already running".to_owned(),
            },
        );
        assert_eq!(*view.fetch.read(), running(), "the fetch in flight changed");
        assert!(view.endings.read().is_empty());
        assert!(asked.submitted.borrow().is_empty());
        assert!(asked.refused.borrow().is_empty());
        applying(&test, view, &asked, Update::IndexLock { changed: None });
        assert_eq!(view.lock.read().changed(), None);
    }

    /// git's progress lines land whole — the latest in the fetch's state, for the status box to
    /// keep to its percentage as it draws it, and every one in the fetch's activity entry, for
    /// the popover — and the remotes in theirs. Caught by: a line trimmed before it is kept, so
    /// the popover could never say it whole.
    #[test]
    fn progress_and_remotes_land_in_their_states() {
        let (test, view, asked) = launch(running());
        let mut log = view.activity.log;
        test.run_in(|| log.write().fetch_started("origin"));
        let whole = "Receiving objects:  45% (12345/27000), 12.00 MiB | 3.00 MiB/s";
        applying(
            &test,
            view,
            &asked,
            Update::FetchProgress {
                tap: crate::worker::OutputTap::holding(&cairn_model::ScrubbedLines::scrubbing(
                    &format!("Receiving objects: 20%\n{whole}\n"),
                )),
            },
        );
        assert_eq!(
            *view.fetch.read(),
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: Some(whole.to_owned())
            }
        );
        let (entries, _) = view.activity.log.peek().shown();
        let kept = entries
            .iter()
            .flat_map(|entry| entry.lines.iter())
            .any(|line| *line == cairn_ui::ActivityLine::Output(whole.to_owned()));
        assert!(kept, "the activity entry does not keep the whole line");
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

    /// The user's `diff.context` is where the session's context starts; a file already shown
    /// at git's default — in the Changes tab — is asked again at it; once the user has moved
    /// the context, a later reading of the configuration does not move it back. Caught by: a
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
            options: crate::diff::actions::options(cairn_ui::DiffSettings::default()),
        };
        // Shown in the Changes tab, whose file a setting asks again at once (the Commit tab's
        // files opened in place share the lane, and the hidden one waits).
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

    /// Through the real boundary: `diff.context` and
    /// `diff.interHunkContext` edited mid-session reach the next answer — the context
    /// adopted while the user has not moved it, the grouping always — and once the user has
    /// moved the context, a `diff.context` edit is read and sent but not adopted, while the
    /// inter-hunk context still applies. Caught by: the configured context read once, on a
    /// handle the configuration's freshness never reopens.
    #[test]
    fn a_configuration_edit_mid_session_reaches_the_next_answer() {
        use cairn_model::Context;

        use crate::diff::state::Answer;
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
            test.run_in(|| crate::diff::actions::change_settings(view, Some(&submit), change));
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
            options: crate::diff::actions::options(test.run_in(|| *view.diff_settings.peek())),
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
                commit_time: None,
            }],
            head: cairn_model::HeadState::Branch(cairn_model::RefName::new("refs/heads/main")),
            stashes: Vec::new(),
            unreadable: 0,
            substitutions: cairn_model::Substitutions::default(),
        })
    }

    /// What git substitutes changed: the chosen commit's changes are asked again at once while
    /// the history is shown; while Local Changes is, nothing is asked — its primary's diff holds
    /// the file-diff lane — until the history is shown again, which asks once; and refs naming
    /// the same substitutions ask nothing. Caught by: the answer kept across a substitution
    /// shown while Local Changes was up, a changes query taking the lane from Local Changes, or
    /// a re-ask on every refresh.
    #[test]
    fn a_substitution_change_asks_the_chosen_commits_changes_again_when_the_history_is_shown() {
        use cairn_model::RowId;
        use cairn_ui::MainView;

        let (test, view, asked) = launch(FetchStatus::Idle);
        let changes_asked = || {
            asked
                .submitted
                .borrow_mut()
                .drain(..)
                .filter(|request| {
                    matches!(request, Request::Changes { of } if *of == crate::worker::Comparison::Commit(oid(1)))
                })
                .count()
        };
        let substituted = |n: u8| {
            let mut snapshot = (*snapshot_naming(oid(2))).clone();
            snapshot.substitutions =
                cairn_model::Substitutions::new(vec![(oid(1), oid(n))], vec![]);
            std::sync::Arc::new(snapshot)
        };
        let refs = |snapshot| Update::Refs {
            snapshot,
            reopen: false,
        };
        applying(&test, view, &asked, refs(snapshot_naming(oid(2))));
        test.run_in(|| {
            let submit = |request| asked.submitted.borrow_mut().push(request);
            crate::history::selection::choose(RowId::Commit(oid(1)), view, Some(&submit));
        });
        assert_eq!(changes_asked(), 1, "choosing the commit asked its changes");

        applying(&test, view, &asked, refs(substituted(3)));
        assert_eq!(changes_asked(), 1, "a replace ref added, the history shown");
        applying(&test, view, &asked, refs(substituted(3)));
        assert_eq!(changes_asked(), 0, "the same substitutions asked again");

        test.run_in(|| {
            let submit = |request| asked.submitted.borrow_mut().push(request);
            crate::history::selection::show_main(MainView::LocalChanges, view, Some(&submit));
        });
        applying(&test, view, &asked, refs(substituted(4)));
        assert_eq!(
            changes_asked(),
            0,
            "asked while Local Changes held the main region"
        );
        test.run_in(|| {
            let submit = |request| asked.submitted.borrow_mut().push(request);
            crate::history::selection::show_main(MainView::AllCommits, view, Some(&submit));
        });
        assert_eq!(
            changes_asked(),
            1,
            "the history shown again did not ask again"
        );
        test.run_in(|| {
            let submit = |request| asked.submitted.borrow_mut().push(request);
            crate::history::selection::show_main(MainView::LocalChanges, view, Some(&submit));
            crate::history::selection::show_main(MainView::AllCommits, view, Some(&submit));
        });
        assert_eq!(changes_asked(), 0, "asked again with nothing changed");

        // Back to the history another way — a ref pressed choosing the same commit — asks too.
        test.run_in(|| {
            let submit = |request| asked.submitted.borrow_mut().push(request);
            crate::history::selection::show_main(MainView::LocalChanges, view, Some(&submit));
        });
        applying(&test, view, &asked, refs(substituted(5)));
        test.run_in(|| {
            let submit = |request| asked.submitted.borrow_mut().push(request);
            let mut main = view.sidebar.main;
            main.set(MainView::AllCommits);
            crate::history::selection::choose(RowId::Commit(oid(1)), view, Some(&submit));
        });
        assert_eq!(
            changes_asked(),
            1,
            "a press choosing the commit drew the stale answer"
        );
    }

    /// A reopen keeps the old rows drawn and usable until the new walk's first page replaces
    /// them: refs that draw another history ask for the new walk alone, the rows, their status
    /// and the selection kept and no page of them asked; the walk's count, once told, is kept
    /// for the strip; its first page replaces them — the old rows handed to a worker to free as
    /// one `Request::Retire` (#52), never freed on the UI thread, the new history sized for the
    /// authors the old one named — and the selection is left as it was; refs that draw the same
    /// history leave it, handing back only the snapshot they replace. Caught by: rows emptied or
    /// retired at the ask ("Reading history…" in place of the list), a first page appended to
    /// the old rows, the old rows replaced in place (no retirement), a reopen with
    /// `History::new` (an index with no room), the selection cleared, or a reopen on a refresh
    /// that said not to.
    #[test]
    fn a_reopen_keeps_the_old_rows_until_the_new_page() {
        use crate::history::state::Status;

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
        // The sidebar's rows asked for the snapshot first, then the reopen.
        match submitted.as_slice() {
            [
                Request::FilterRefs { .. },
                Request::OpenHistory { rows, lost: false },
            ] => assert_eq!(*rows, PAGE_ROWS),
            other => panic!("expected the history asked and nothing retired, got {other:?}"),
        }
        assert_eq!(
            view.rows.read().len(),
            3,
            "the old rows were dropped at the ask"
        );
        assert_eq!(view.progress.read().status(), &Status::Ready);
        assert!(
            !view.progress.read().wants_more(),
            "the rows being replaced asked for a page"
        );
        assert_eq!(*view.selected.read(), chosen);

        applying(&test, view, &asked, Update::Reading { commits: 51_200 });
        assert_eq!(view.progress.read().read_so_far(), Some(51_200));
        assert_eq!(view.rows.read().len(), 3);

        applying(
            &test,
            view,
            &asked,
            Update::Rows {
                rows: page(5..7, "Grace"),
                complete: false,
            },
        );
        let ids: Vec<Oid> = view.rows.read().rows().map(|row| row.id().oid()).collect();
        assert_eq!(
            ids,
            vec![oid(5), oid(6)],
            "the first page did not replace the old rows"
        );
        match asked
            .submitted
            .borrow_mut()
            .drain(..)
            .collect::<Vec<_>>()
            .as_slice()
        {
            [Request::Retire(retired)] => {
                assert_eq!(retired.replaced_rows(), Some(3), "not the old rows");
            }
            other => panic!("expected the old rows retired, got {other:?}"),
        }
        assert!(!view.progress.read().is_reopening());
        assert_eq!(view.progress.read().read_so_far(), None);
        assert!(
            view.progress.read().wants_more(),
            "the new walk is not paged"
        );
        assert_eq!(
            *view.selected.read(),
            chosen,
            "the reopen let go of the selection"
        );
        assert!(
            view.rows.read().retained().authors > History::new().retained().authors,
            "the new history was not sized for the authors the old one named"
        );
        // The next page is the new walk's second, appended.
        applying(
            &test,
            view,
            &asked,
            Update::Rows {
                rows: page(7..8, "Grace"),
                complete: true,
            },
        );
        assert_eq!(view.rows.read().len(), 3);
        assert!(asked.submitted.borrow().is_empty());

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

    /// Each refresh answer the window replaces — ahead/behind, a status — is handed
    /// to a worker to free; a failed read is said beside the answer kept, which stays; a
    /// failure to read the refs before any were read is the history's failure too, since it
    /// has nothing to walk from. Caught by: a replaced answer dropped in place, a failure
    /// that throws the kept answer away, or a first failure that leaves "Reading history…"
    /// up for good.
    #[test]
    fn a_replaced_refresh_answer_is_freed_on_a_worker_and_a_failure_keeps_the_last() {
        use cairn_model::{AheadBehind, RefName, StatusEntry, WorkingTreeStatus};

        use crate::history::state::Status;
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

    /// Through the real boundary, counting reopens: the window's first refresh opens the
    /// history; a refresh that finds nothing changed asks for nothing; a stash pushed — the
    /// stash list alone changed — reopens it, and so does a checkout that moves no ref
    /// (`HEAD` onto `other`, on `HEAD`'s commit) and a branch moved; nothing changed again
    /// reopens nothing. Each reopen is counted as the window asks for it, from what the
    /// worker answered. Caught by: a comparison that leaves out the stash list or `HEAD`'s
    /// state (a step that should reopen does not), one that reopens on anything (a step that
    /// should not, does), or a fetch's own tip comparison standing in for the refresh.
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

    /// Through the real boundary, counting reopens: what git substitutes as it walks — a replace
    /// ref added and moved, `core.useReplaceRefs` turned off and on, the graft file written and
    /// removed, the shallow file written — each reopens the history though no ref moved, and a
    /// refresh that finds nothing changed reopens nothing. After each, every thread reads the
    /// configuration as it is then: the history draws the replaced commit's row as a handle
    /// opened now reads it — its own subject with replace refs off, its replacement's on — the
    /// refresh thread counts `main` against its upstream as one opened now counts it, and the
    /// local lane's Recent Commit Messages are one opened now reads; and the chosen commit's
    /// changes are asked again each time. Caught by: a comparison of the refs alone (no step
    /// reopens), a thread whose handle keeps the configuration of its first open (the row, the
    /// counts or the messages keep the replacement's with replace refs off), or the chosen
    /// commit's answer kept across a substitution.
    #[test]
    fn a_refresh_reopens_for_what_git_substitutes_and_the_walk_reads_the_configuration_now() {
        use cairn_model::{RefName, RowContent, RowId};

        use crate::worker::{Refreshable, Updates, next_update};

        let fixture = Refreshable::new("cairn-session-substitutions");
        let [newest, replaced, replacement, other] = [0, 1, 2, 3].map(|n| fixture.commits[n]);
        // `main` tracks `up`, two commits behind it: a replace ref under `main`'s first parent
        // changes what the two have in common.
        fixture.point("up", replacement);
        fixture.track("main", "up");
        let (handle, mut updates) = fixture.open();
        let (test, view, asked) = launch(FetchStatus::Idle);
        // Asks for a refresh and applies what arrives, handing on what applying asks, until the
        // refresh's refs and its ahead/behind have been applied and — if the refs reopened the
        // history — its first page has too. Returns how many reopens and how many changes
        // queries the window asked.
        let refresh = |updates: &mut Updates| -> (usize, usize) {
            handle.submit(Request::Refresh);
            let (mut refs_seen, mut counted, mut reopening) = (false, false, false);
            let (mut opens, mut changes) = (0, 0);
            loop {
                for request in asked.submitted.borrow_mut().drain(..) {
                    match request {
                        Request::OpenHistory { .. } => {
                            opens += 1;
                            reopening = true;
                        }
                        Request::Changes { .. } => changes += 1,
                        _ => {}
                    }
                    handle.submit(request);
                }
                if refs_seen && counted && !reopening {
                    return (opens, changes);
                }
                let update = next_update(updates);
                match &update {
                    Update::Refs { .. } => refs_seen = true,
                    Update::AheadBehind { .. } => counted = true,
                    Update::Rows { .. } => reopening = false,
                    _ => {}
                }
                applying(&test, view, &asked, update);
            }
        };
        // The local lane's Recent Commit Messages, asked of it directly.
        let recent = |updates: &mut Updates| -> Vec<String> {
            handle.submit(Request::CommitReads);
            loop {
                match next_update(updates) {
                    Update::CommitReads(reads) => match reads.recent {
                        Ok(recent) => return recent,
                        Err(error) => panic!("the recent messages failed: {error}"),
                    },
                    other => applying(&test, view, &asked, other),
                }
            }
        };
        let subject_drawn = |commit: Oid| -> Option<String> {
            let rows = view.rows.peek();
            let at = rows.position(RowId::Commit(commit))?;
            match rows.row(at)?.content() {
                RowContent::Commit(summary) => Some(summary.summary),
                RowContent::Stash(stash) => panic!("a commit drawn as a stash: {stash:?}"),
            }
        };
        let counted = || {
            view.refreshed
                .peek()
                .ahead_behind_of(&RefName::new("refs/heads/main"))
        };
        // Every thread's answer is the one a handle opened now gives.
        let every_thread_reads_it_now = |updates: &mut Updates, step: &str| {
            assert_eq!(
                subject_drawn(replaced),
                Some(fixture.subject_now(replaced)),
                "{step}: the history thread"
            );
            assert_eq!(
                counted(),
                fixture.ahead_behind_now("main"),
                "{step}: the refresh thread"
            );
            assert_eq!(
                recent(updates),
                fixture.recent_messages_now(),
                "{step}: the local lane"
            );
        };

        assert_eq!(
            refresh(&mut updates).0,
            1,
            "the first refresh did not open the history"
        );
        assert_eq!(
            refresh(&mut updates),
            (0, 0),
            "a refresh that found nothing changed reopened"
        );
        test.run_in(|| {
            let mut selected = view.selected;
            selected.set(Some(RowId::Commit(replaced)));
        });
        let own = fixture.subject_now(replaced);
        let stored_counts = fixture.ahead_behind_now("main");

        fixture.replace(replaced, replacement);
        assert_eq!(refresh(&mut updates), (1, 1), "a replace ref added");
        every_thread_reads_it_now(&mut updates, "a replace ref added");
        assert_ne!(
            subject_drawn(replaced),
            Some(own.clone()),
            "the replacement was not drawn"
        );

        fixture.replace(replaced, other);
        assert_eq!(refresh(&mut updates), (1, 1), "a replace ref moved");
        every_thread_reads_it_now(&mut updates, "a replace ref moved");
        let replaced_counts = counted();
        assert_ne!(
            replaced_counts, stored_counts,
            "the fixture's replace ref changes no count, so the counts decide nothing"
        );

        let replaced_messages = fixture.recent_messages_now();
        fixture.use_replace_refs(false);
        assert_ne!(
            fixture.recent_messages_now(),
            replaced_messages,
            "the fixture's replace ref changes no recent message, so the messages decide nothing"
        );
        assert_eq!(
            refresh(&mut updates),
            (1, 1),
            "core.useReplaceRefs turned off"
        );
        every_thread_reads_it_now(&mut updates, "core.useReplaceRefs turned off");
        assert_eq!(
            subject_drawn(replaced),
            Some(own.clone()),
            "the walk kept reading the replace refs the configuration turned off"
        );
        assert_eq!(counted(), stored_counts, "the counts kept the replace refs");
        fixture.use_replace_refs(true);
        assert_eq!(
            refresh(&mut updates),
            (1, 1),
            "core.useReplaceRefs turned on"
        );
        every_thread_reads_it_now(&mut updates, "core.useReplaceRefs turned on");
        assert_eq!(counted(), replaced_counts);
        fixture.unreplace(replaced);
        assert_eq!(refresh(&mut updates), (1, 1), "the replace ref deleted");
        assert_eq!(subject_drawn(replaced), Some(own));

        fixture.graft(Some((newest, &[replacement])));
        assert_eq!(refresh(&mut updates), (1, 1), "the graft file written");
        fixture.graft(None);
        assert_eq!(refresh(&mut updates), (1, 1), "the graft file removed");

        fixture.shallow(Some(newest));
        assert_eq!(refresh(&mut updates), (1, 1), "the shallow file written");
        assert_eq!(
            refresh(&mut updates),
            (0, 0),
            "nothing changed, yet the history reopened"
        );
        drop(handle);
    }
}
