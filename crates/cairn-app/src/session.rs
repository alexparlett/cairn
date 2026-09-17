//! Applying a worker's update to the view state: the one place an `Update`
//! becomes what the window draws. On the UI thread, so nothing here waits.

use freya::prelude::*;

use crate::PAGE_ROWS;
use crate::fetch_state::{FetchStatus, PromptView};
use crate::history_state::{self, Progress};
use crate::window::View;
use crate::worker::{PromptId, Request, Update};

/// What applying an update may ask of the worker: a request, and the refusal
/// of a prompt the window will not show. Two plain callbacks, never a struct
/// holding the answering end: nothing may hold that.
pub struct Worker<'a> {
    pub submit: &'a dyn Fn(Request),
    pub refuse: &'a dyn Fn(PromptId),
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
        ..
    } = view;
    match update {
        Update::Rows {
            rows: page,
            complete,
        } => {
            // Count before handing the rows over; afterwards it rereads the whole history.
            let widest = history_state::widest_lane(&page);
            let loaded = {
                let mut held = rows.write();
                held.extend(page);
                held.len()
            };
            progress.write().received(widest, complete, loaded);
        }
        Update::Failed { message } | Update::WorkerLost { message } => {
            progress.write().failed(message);
        }
        Update::Remotes { remotes: listed } => remotes.set(listed),
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
        Update::Prompt { id, text } => {
            if fetch.read().is_in_flight() {
                prompt.set(Some(PromptView { id, text }));
            } else {
                (worker.refuse)(id);
            }
        }
    }
}

/// The refs moved: the rows on screen are of the old ones.
fn reload_if(
    refreshed: bool,
    mut rows: State<Vec<cairn_model::HistoryRow>>,
    mut progress: State<Progress>,
    worker: &Worker<'_>,
) {
    if !refreshed {
        return;
    }
    rows.write().clear();
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

    use cairn_model::{CommitSummary, EdgeSegment, GraphRow, HistoryRow, Lane, Oid, RowContent};
    use freya_testing::TestingRunner;

    use super::*;
    use crate::history_state::Status;

    fn row(n: u8) -> HistoryRow {
        let mut bytes = [0u8; 20];
        bytes[19] = n;
        let id = Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"));
        HistoryRow {
            content: RowContent::Commit(CommitSummary {
                id,
                parents: Vec::new(),
                summary: format!("commit {n}"),
                author_name: "Ada".to_owned(),
                author_email: "ada@example.com".to_owned(),
                author_time: 0,
            }),
            graph: GraphRow {
                id,
                lane: Lane::new(0),
                edges: vec![EdgeSegment::passing(Lane::new(0))],
            },
        }
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
                        rows: State::create((0..3).map(row).collect()),
                        progress: State::create(progress),
                        selected: State::create(None),
                        fetch: State::create(fetch),
                        prompt: State::create(None),
                        remotes: State::create(Vec::new()),
                    }
                })
            },
            1.,
        );
        test.sync_and_update();
        (test, view, asked)
    }

    fn applying(test: &TestingRunner, view: View, asked: &Rc<Asked>, update: Update) {
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
                },
            );
        });
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
}
