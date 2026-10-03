//! The diff thread, `cairn-diff`: the changes and file-diff lanes of one repository,
//! commits, comparisons and the working tree alike (PRD R4.2, R4.3, R4.5; packet decision
//! L8).
//!
//! It takes its own thread-local repository handle once, and keeps for the life of the
//! repository the [`DiffSession`] — gix's blob resource cache, reused across commits — and
//! [`Answers`], the commit and comparison answers already given. A working-tree answer is
//! never kept, and the engine reads the index and the attributes afresh for every
//! working-tree query.
//!
//! **Scheduling.** Each lane holds at most one request: the newest, since a request is
//! superseded by the next in its lane anyway ([`super::epoch`]). A waiting file diff is
//! served before a waiting changes query (it was asked after it — a changes query would
//! have superseded it otherwise), and a request superseded while it waited is dropped
//! unserved. The thread blocks on its queue whenever nothing is waiting; it never spins.
//!
//! **Cancellation.** A query's epoch is its [`cairn_git::Cancel`]: the engine checks it
//! between files and the runner polls it while each `git` read runs, so superseding a diff
//! ends its process group rather than letting it run out and discarding the answer. A fast
//! click through a file list therefore kills each file's one to three `git` processes in
//! turn instead of queueing them. An answer is sent only while its epoch is current, and
//! [`super::Updates::next`] checks again as it arrives, so a superseded answer is dropped
//! on arrival, never drawn.

use std::sync::mpsc::{Receiver, TryRecvError};

use cairn_git::ops::GitBinary;
use cairn_git::{
    Cancel, ChangesRequest, ContentOptions, DiffSession, Error, Repository, SharedRepository,
    WorkingTreeDiff,
};
use cairn_model::{ChangeSet, FileDiff};

use super::epoch::{Epoch, Epochs, Superseded};
use super::pool::Outbox;
use super::request::{
    Comparison, DiffOptions, DiffQuery, FileQuery, FileTarget, Update, WorkingSide,
};

/// What the diff thread is sent.
#[derive(Debug)]
pub(super) enum DiffJob {
    /// A query, numbered in its lane; boxed, since it carries a file's paths and ids.
    Query { epoch: Epoch, query: Box<DiffQuery> },
    /// The repository is closing, or every handle to it has gone: serve nothing more.
    Stop,
}

/// How many times a content query is asked when git printed lines that are not the lines
/// Cairn read ([`Error::ContentReadsDisagree`]): the file changed between the two reads,
/// which asking again settles, or a clean filter whose output differs run to run, which it
/// never will. Past this the disagreement is shown, never looped on.
pub(super) const READ_ATTEMPTS: usize = 3;

/// The diff thread's loop: runs until it is told to stop or every sender is gone.
pub(super) fn serve_diffs(
    shared: &SharedRepository,
    git: &GitBinary,
    epochs: &Epochs,
    jobs: &Receiver<DiffJob>,
    outbox: &Outbox,
) {
    // Once, at the top of the thread: each call rebuilds the object cache and the pack
    // snapshot, and `session` borrows it across turns.
    let repo = shared.to_worker();
    let mut session: Option<DiffSession<'_>> = None;
    let mut answers = Answers::default();
    let mut waiting = Waiting::default();

    while waiting.gather(jobs) {
        if epochs.is_stopping() {
            break;
        }
        let Some((epoch, query)) = waiting.next(epochs) else {
            continue; // Everything that waited was superseded; the next turn blocks.
        };
        let served = Served {
            repo: &repo,
            git,
            epochs,
            epoch,
            cancel: epochs.watch(epoch),
            outbox,
        };
        served.answer(query, &mut session, &mut answers);
    }
}

/// The newest request waiting in each lane.
#[derive(Debug, Default)]
struct Waiting {
    changes: Option<(Epoch, DiffQuery)>,
    file: Option<(Epoch, DiffQuery)>,
    stopped: bool,
}

impl Waiting {
    fn is_empty(&self) -> bool {
        self.changes.is_none() && self.file.is_none()
    }

    /// Blocks for a job only when none is waiting, then takes every job already queued,
    /// keeping the newest per lane. `false` when there is nothing more to serve: told to
    /// stop, or every sender gone with nothing waiting.
    fn gather(&mut self, jobs: &Receiver<DiffJob>) -> bool {
        if self.is_empty() && !self.stopped {
            match jobs.recv() {
                Ok(job) => self.take(job),
                Err(_) => return false,
            }
        }
        loop {
            match jobs.try_recv() {
                Ok(job) => self.take(job),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.is_empty() {
                        return false;
                    }
                    break;
                }
            }
        }
        !self.stopped
    }

    fn take(&mut self, job: DiffJob) {
        match job {
            DiffJob::Stop => self.stopped = true,
            DiffJob::Query { epoch, query } => match *query {
                query @ DiffQuery::Changes(_) => self.changes = Some((epoch, query)),
                query @ (DiffQuery::File(_) | DiffQuery::All { .. }) => {
                    self.file = Some((epoch, query));
                }
            },
        }
    }

    /// The file diff first, then the changes query; one superseded while it waited is
    /// dropped unserved.
    fn next(&mut self, epochs: &Epochs) -> Option<(Epoch, DiffQuery)> {
        if let Some((epoch, query)) = self.file.take()
            && epochs.is_current(epoch)
        {
            return Some((epoch, query));
        }
        if let Some((epoch, query)) = self.changes.take()
            && epochs.is_current(epoch)
        {
            return Some((epoch, query));
        }
        None
    }
}

/// One query being served, and where its answer goes.
struct Served<'a> {
    repo: &'a Repository,
    git: &'a GitBinary,
    epochs: &'a Epochs,
    epoch: Epoch,
    cancel: Superseded,
    outbox: &'a Outbox,
}

impl<'a> Served<'a> {
    fn answer(
        &self,
        query: DiffQuery,
        session: &mut Option<DiffSession<'a>>,
        answers: &mut Answers,
    ) {
        let outcome = match &query {
            DiffQuery::Changes(of) => self
                .change_set(*of, answers)
                .map(|changes| Update::Changes { of: *of, changes }),
            DiffQuery::File(asked) => {
                self.file_diff(asked, session, answers)
                    .map(|diff| Update::FileDiff {
                        query: asked.clone(),
                        diff,
                    })
            }
            DiffQuery::All { of, options } => {
                self.every_file(*of, options, session, answers)
                    .map(|diffs| Update::FileDiffs {
                        of: *of,
                        options: *options,
                        diffs,
                        complete: true,
                    })
            }
        };
        match outcome {
            Ok(update) => self.send(update),
            // Superseded: asked for by whoever superseded it, and nothing to show.
            Err(error) if was_cancelled(&error) || !self.epochs.is_current(self.epoch) => {}
            Err(error) => self.send(Update::DiffFailed {
                query,
                message: error.to_string(),
            }),
        }
    }

    /// Sent only while the query is current; [`super::Updates::next`] checks again.
    fn send(&self, update: Update) {
        if self.epochs.is_current(self.epoch) {
            self.outbox.send(Some(self.epoch), update);
        }
    }

    /// `of`'s change set, kept or asked of git; one too large to keep is still answered.
    fn change_set(&self, of: Comparison, answers: &mut Answers) -> Result<ChangeSet, Error> {
        if let Some(kept) = answers.change_set(of) {
            return Ok(kept.clone());
        }
        let changes = self.repo.changes(self.git, &request(of), &self.cancel)?;
        answers.keep_change_set(of, changes.clone());
        Ok(changes)
    }

    fn file_diff(
        &self,
        asked: &FileQuery,
        session: &mut Option<DiffSession<'a>>,
        answers: &mut Answers,
    ) -> Result<Option<FileDiff>, Error> {
        let options = content_options(&asked.options);
        match &asked.target {
            FileTarget::Committed { of, file } => {
                if let Some(kept) = answers.file_diff(asked) {
                    return Ok(Some(kept.clone()));
                }
                let session = self.session(session)?;
                let diff = asking_again(&self.cancel, || {
                    session.file_diff(self.git, &request(*of), file, &options, &self.cancel)
                })?;
                answers.keep_file_diff(asked.clone(), diff.clone());
                Ok(Some(diff))
            }
            // Never kept: the working tree moves under it.
            FileTarget::WorkingTree { path, side } => asking_again(&self.cancel, || {
                self.repo
                    .working_tree_diff(self.git, path, which(*side), &options, &self.cancel)
            }),
        }
    }

    /// Expand All: every file of `of`'s change set, in one engine call that checks the
    /// epoch between files and while each `git` read runs — so a newer file diff or
    /// changes query ends it at the next file or kills its read, rather than waiting
    /// behind it. Not kept: phase 08 bounds and pages it.
    fn every_file(
        &self,
        of: Comparison,
        options: &DiffOptions,
        session: &mut Option<DiffSession<'a>>,
        answers: &mut Answers,
    ) -> Result<Vec<FileDiff>, Error> {
        let changes = self.change_set(of, answers)?;
        let session = self.session(session)?;
        let options = content_options(options);
        asking_again(&self.cancel, || {
            session.file_diffs(self.git, &request(of), &changes, &options, &self.cancel)
        })
    }

    /// The session kept for the life of the repository, opened on first use; one that
    /// could not be opened is tried again on the next query that needs it.
    fn session<'s>(
        &self,
        slot: &'s mut Option<DiffSession<'a>>,
    ) -> Result<&'s mut DiffSession<'a>, Error> {
        let session = match slot.take() {
            Some(session) => session,
            None => self.repo.diff_session()?,
        };
        Ok(slot.insert(session))
    }
}

/// Asks again while git's lines and Cairn's read of the same content disagree, up to
/// [`READ_ATTEMPTS`] in all, and not once the query is superseded.
fn asking_again<T>(
    cancel: &impl Cancel,
    mut ask: impl FnMut() -> Result<T, Error>,
) -> Result<T, Error> {
    let mut attempt = 1;
    loop {
        match ask() {
            Err(Error::ContentReadsDisagree { .. })
                if attempt < READ_ATTEMPTS && !cancel.is_cancelled() =>
            {
                attempt += 1;
            }
            outcome => return outcome,
        }
    }
}

/// A query ended because it was superseded, whichever read noticed.
fn was_cancelled(error: &Error) -> bool {
    matches!(
        error,
        Error::ChangesCancelled { .. } | Error::ContentCancelled | Error::GitReadCancelled { .. }
    )
}

fn request(of: Comparison) -> ChangesRequest {
    match of {
        Comparison::Commit(id) => ChangesRequest::commit(id),
        Comparison::Between { old, new } => ChangesRequest::between(old, new),
    }
}

fn which(side: WorkingSide) -> WorkingTreeDiff {
    match side {
        WorkingSide::Staged => WorkingTreeDiff::Staged,
        WorkingSide::Unstaged => WorkingTreeDiff::Unstaged,
        WorkingSide::Untracked => WorkingTreeDiff::Untracked,
    }
}

/// R2.6's ceilings are fixed; the rest is the view's.
fn content_options(options: &DiffOptions) -> ContentOptions {
    ContentOptions {
        context: options.context,
        ignore_whitespace: options.ignore_whitespace,
        load_anyway: options.load_anyway,
        ..ContentOptions::default()
    }
}

/// The commit and comparison answers already given (R4.5), newest last, each bounded.
///
/// Keyed by everything Cairn asks the answer with: a change set by its [`Comparison`] —
/// commit ids, so the trees cannot move under it — and a file's diff by its whole
/// [`FileQuery`]: the comparison, the [`cairn_model::ChangedFile`] (both paths, modes and
/// blob ids) and the options (context, whitespace, loading past the ceiling). The
/// configuration the engine reads — `diff.algorithm` and the drivers' algorithms,
/// `diff.renames`, `diff.renameLimit`, `diff.ignoreSubmodules`, `log.showRoot` — is the
/// repository handle's, read when the repository was opened; this cache lives exactly as
/// long as that handle, so the two are refreshed together, by reopening. What `git` itself
/// reads as it runs and an answer can depend on — the attributes (a diff driver's
/// algorithm, its function-context pattern, `-diff`), `.gitmodules`, and the drivers'
/// configuration — is NOT in the key: an edit to them while the repository is open is seen
/// by the next query not already answered here, and not by one that is. Working-tree
/// answers are never kept.
#[derive(Debug, Default)]
struct Answers {
    change_sets: Vec<(Comparison, ChangeSet)>,
    file_diffs: Vec<(FileQuery, FileDiff, usize)>,
}

/// How many change sets are kept, and how many of their files in all: a commit that
/// renames tens of thousands of files is kept alone, or not at all.
const KEPT_CHANGE_SETS: usize = 16;
const KEPT_CHANGED_FILES: usize = 100_000;
/// How many file diffs are kept, and roughly how many bytes of lines in all; an answer
/// larger than the whole budget is not kept.
const KEPT_FILE_DIFFS: usize = 64;
const KEPT_DIFF_BYTES: usize = 32 * 1024 * 1024;

impl Answers {
    fn change_set(&self, of: Comparison) -> Option<&ChangeSet> {
        self.change_sets
            .iter()
            .find(|(kept, _)| *kept == of)
            .map(|(_, changes)| changes)
    }

    fn keep_change_set(&mut self, of: Comparison, changes: ChangeSet) {
        self.change_sets.retain(|(kept, _)| *kept != of);
        if changes.files.len() > KEPT_CHANGED_FILES {
            return;
        }
        self.change_sets.push((of, changes));
        while self.change_sets.len() > KEPT_CHANGE_SETS
            || self
                .change_sets
                .iter()
                .map(|(_, c)| c.files.len())
                .sum::<usize>()
                > KEPT_CHANGED_FILES
        {
            self.change_sets.remove(0);
        }
    }

    fn file_diff(&self, asked: &FileQuery) -> Option<&FileDiff> {
        self.file_diffs
            .iter()
            .find(|(kept, _, _)| kept == asked)
            .map(|(_, diff, _)| diff)
    }

    fn keep_file_diff(&mut self, asked: FileQuery, diff: FileDiff) {
        if !matches!(asked.target, FileTarget::Committed { .. }) {
            return;
        }
        self.file_diffs.retain(|(kept, _, _)| *kept != asked);
        let bytes = held_bytes(&diff);
        if bytes > KEPT_DIFF_BYTES {
            return;
        }
        self.file_diffs.push((asked, diff, bytes));
        while self.file_diffs.len() > KEPT_FILE_DIFFS
            || self.file_diffs.iter().map(|(_, _, b)| b).sum::<usize>() > KEPT_DIFF_BYTES
        {
            self.file_diffs.remove(0);
        }
    }
}

/// Roughly what a file's diff holds: its lines' bytes and each line's own size.
fn held_bytes(diff: &FileDiff) -> usize {
    let line = std::mem::size_of::<cairn_model::DiffLine>();
    diff.text().map_or(0, |text| {
        text.old_lines()
            .iter()
            .chain(text.new_lines())
            .map(|l| l.bytes().len() + line)
            .sum()
    }) + std::mem::size_of::<FileDiff>()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::mpsc::channel;

    use cairn_model::{
        ChangeStatus, ChangedFile, Context, DiffContent, DiffLine, Oid, RepoPath, TextDiff,
    };

    use super::*;
    use crate::worker::epoch::QueryLane;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    fn file(n: u8) -> ChangedFile {
        ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("a.txt"),
            new_path: RepoPath::from("a.txt"),
            old_mode: Some(cairn_model::FileMode::Regular),
            new_mode: Some(cairn_model::FileMode::Regular),
            old_id: Some(oid(n)),
            new_id: Some(oid(n + 1)),
        }
    }

    fn committed(n: u8, options: DiffOptions) -> FileQuery {
        FileQuery {
            target: FileTarget::Committed {
                of: Comparison::Commit(oid(n)),
                file: file(n),
            },
            options,
        }
    }

    fn text_diff(n: u8, bytes: usize) -> FileDiff {
        FileDiff {
            file: file(n),
            content: DiffContent::Text {
                text: TextDiff::new(
                    Vec::new(),
                    vec![DiffLine::terminated(vec![b'x'; bytes])],
                    vec![cairn_model::ChangedRange::new(
                        cairn_model::LineSpan::at(0, 0),
                        cairn_model::LineSpan::at(0, 1),
                    )],
                ),
                overlay: cairn_model::DisplayOverlay::default(),
            },
        }
    }

    fn changes(files: usize) -> ChangeSet {
        ChangeSet {
            files: (0..files).map(|_| file(1)).collect(),
            details: None,
            renames: cairn_model::RenameDetection::default(),
        }
    }

    /// R4.5: a kept answer is found only under everything it was asked with. Caught by: a
    /// key that leaves out the context or the whitespace option (the view would draw an
    /// answer at another context, with function context git printed for other hunks), the
    /// file (another file's lines), or the comparison.
    #[test]
    fn a_kept_answer_is_found_only_under_everything_it_was_asked_with() {
        let mut answers = Answers::default();
        let asked = committed(1, DiffOptions::default());
        answers.keep_file_diff(asked.clone(), text_diff(1, 4));
        assert_eq!(answers.file_diff(&asked), Some(&text_diff(1, 4)));

        let mut other_context = asked.clone();
        other_context.options.context = Context::lines(10);
        let mut whitespace = asked.clone();
        whitespace.options.ignore_whitespace = true;
        let mut anyway = asked.clone();
        anyway.options.load_anyway = true;
        let mut other_file = asked.clone();
        let mut other_commit = asked.clone();
        if let FileTarget::Committed { of, file: f } = &mut other_file.target {
            f.new_id = Some(oid(99));
            let _ = of;
        }
        if let FileTarget::Committed { of, .. } = &mut other_commit.target {
            *of = Comparison::Between {
                old: oid(1),
                new: oid(2),
            };
        }
        for missed in [other_context, whitespace, anyway, other_file, other_commit] {
            assert_eq!(answers.file_diff(&missed), None, "{missed:?}");
        }
    }

    /// R4.5: working-tree answers are never kept. Caught by: keeping one, which would draw
    /// the file as it was when first asked.
    #[test]
    fn a_working_tree_answer_is_never_kept() {
        let mut answers = Answers::default();
        let asked = FileQuery {
            target: FileTarget::WorkingTree {
                path: RepoPath::from("a.txt"),
                side: WorkingSide::Unstaged,
            },
            options: DiffOptions::default(),
        };
        answers.keep_file_diff(asked.clone(), text_diff(1, 4));
        assert_eq!(answers.file_diff(&asked), None);
        assert!(answers.file_diffs.is_empty());
    }

    /// Caught by: an unbounded cache, which holds every diff ever viewed for the life of
    /// the window.
    #[test]
    fn what_is_kept_is_bounded_by_count_and_by_size() {
        let mut answers = Answers::default();
        for n in 0..(KEPT_FILE_DIFFS as u8 + 8) {
            answers.keep_file_diff(committed(n, DiffOptions::default()), text_diff(n, 1));
        }
        assert_eq!(answers.file_diffs.len(), KEPT_FILE_DIFFS);
        assert_eq!(
            answers.file_diff(&committed(0, DiffOptions::default())),
            None,
            "the oldest was not the one let go"
        );

        let mut answers = Answers::default();
        answers.keep_file_diff(
            committed(1, DiffOptions::default()),
            text_diff(1, KEPT_DIFF_BYTES + 1),
        );
        assert!(
            answers.file_diffs.is_empty(),
            "an answer past the budget was kept"
        );
        answers.keep_file_diff(
            committed(1, DiffOptions::default()),
            text_diff(1, KEPT_DIFF_BYTES / 2),
        );
        answers.keep_file_diff(
            committed(2, DiffOptions::default()),
            text_diff(2, KEPT_DIFF_BYTES / 2),
        );
        assert_eq!(answers.file_diffs.len(), 1, "the budget was not enforced");

        let mut answers = Answers::default();
        for n in 0..(KEPT_CHANGE_SETS as u8 + 2) {
            answers.keep_change_set(Comparison::Commit(oid(n)), changes(1));
        }
        assert_eq!(answers.change_sets.len(), KEPT_CHANGE_SETS);
        answers.keep_change_set(Comparison::Commit(oid(200)), changes(KEPT_CHANGED_FILES));
        assert_eq!(
            answers.change_sets.len(),
            1,
            "the file budget was not enforced"
        );
        answers.keep_change_set(
            Comparison::Commit(oid(201)),
            changes(KEPT_CHANGED_FILES + 1),
        );
        assert!(answers.change_set(Comparison::Commit(oid(201))).is_none());
    }

    fn query(epochs: &Epochs, lane: QueryLane, n: u8) -> DiffJob {
        let of = Comparison::Commit(oid(n));
        let query = match lane {
            QueryLane::Changes => DiffQuery::Changes(of),
            QueryLane::FileDiff | QueryLane::History => {
                DiffQuery::File(committed(n, DiffOptions::default()))
            }
        };
        DiffJob::Query {
            epoch: epochs.bump(lane),
            query: Box::new(query),
        }
    }

    /// R4.3: of what has queued, the newest per lane is served, the file diff first, and a
    /// request superseded while it waited is never served. Caught by: serving in arrival
    /// order (a click-through queues every file), the changes query first, or a superseded
    /// request served anyway.
    #[test]
    fn the_newest_request_per_lane_is_served_the_file_diff_first() {
        let epochs = Epochs::new();
        let (jobs, queue) = channel();
        let mut waiting = Waiting::default();
        jobs.send(query(&epochs, QueryLane::Changes, 1)).unwrap();
        for n in 2..6 {
            jobs.send(query(&epochs, QueryLane::FileDiff, n)).unwrap();
        }
        assert!(waiting.gather(&queue));
        match waiting.next(&epochs) {
            Some((_, DiffQuery::File(asked))) => {
                assert_eq!(asked, committed(5, DiffOptions::default()))
            }
            other => panic!("expected the newest file diff, got {other:?}"),
        }
        match waiting.next(&epochs) {
            Some((_, DiffQuery::Changes(of))) => assert_eq!(of, Comparison::Commit(oid(1))),
            other => panic!("expected the changes query, got {other:?}"),
        }
        assert!(waiting.next(&epochs).is_none());

        // A file diff, then a changes query that supersedes it while both wait.
        jobs.send(query(&epochs, QueryLane::FileDiff, 7)).unwrap();
        jobs.send(query(&epochs, QueryLane::Changes, 8)).unwrap();
        assert!(waiting.gather(&queue));
        match waiting.next(&epochs) {
            Some((_, DiffQuery::Changes(of))) => assert_eq!(of, Comparison::Commit(oid(8))),
            other => panic!("the superseded file diff was served: {other:?}"),
        }
    }

    /// A stop ends the loop even with requests waiting; every sender gone ends it once
    /// nothing waits. Caught by: a stop queued behind requests, which keeps the update
    /// stream — and the closing window — open until they are answered.
    #[test]
    fn a_stop_ends_the_thread_whatever_is_waiting() {
        let epochs = Epochs::new();
        let (jobs, queue) = channel();
        let mut waiting = Waiting::default();
        jobs.send(query(&epochs, QueryLane::FileDiff, 1)).unwrap();
        jobs.send(DiffJob::Stop).unwrap();
        assert!(!waiting.gather(&queue), "a stop was served after a query");

        let (jobs, queue) = channel::<DiffJob>();
        drop(jobs);
        assert!(!Waiting::default().gather(&queue));
    }

    /// The design input on stale reads: a disagreement is asked again, a bounded number of
    /// times, and then shown; never once the query is superseded. Caught by: no retry (a
    /// file saved mid-read shows an error), an unbounded one (a filter whose output varies
    /// loops the thread for good), or retrying a superseded query.
    #[test]
    fn a_disagreeing_read_is_asked_again_a_bounded_number_of_times() {
        let disagree = || Error::ContentReadsDisagree {
            path: "a.txt".to_owned(),
            detail: "line 1".to_owned(),
        };
        let asked = Cell::new(0);
        let settled: Result<u8, Error> = asking_again(&cairn_git::CancelSignal::new(), || {
            asked.set(asked.get() + 1);
            if asked.get() < READ_ATTEMPTS {
                Err(disagree())
            } else {
                Ok(7)
            }
        });
        assert!(matches!(settled, Ok(7)), "{settled:?}");
        assert_eq!(asked.get(), READ_ATTEMPTS);

        asked.set(0);
        let never: Result<u8, Error> = asking_again(&cairn_git::CancelSignal::new(), || {
            asked.set(asked.get() + 1);
            Err(disagree())
        });
        assert!(matches!(never, Err(Error::ContentReadsDisagree { .. })));
        assert_eq!(asked.get(), READ_ATTEMPTS, "the retry is not bounded");

        asked.set(0);
        let superseded = cairn_git::CancelSignal::new();
        superseded.cancel();
        let _ = asking_again(&superseded, || -> Result<u8, Error> {
            asked.set(asked.get() + 1);
            Err(disagree())
        });
        assert_eq!(asked.get(), 1, "a superseded query was asked again");
    }

    /// Caught by: a working-tree request answered for another side of the path.
    #[test]
    fn each_working_side_asks_the_engine_for_that_side() {
        assert_eq!(which(WorkingSide::Staged), WorkingTreeDiff::Staged);
        assert_eq!(which(WorkingSide::Unstaged), WorkingTreeDiff::Unstaged);
        assert_eq!(which(WorkingSide::Untracked), WorkingTreeDiff::Untracked);
    }

    /// Superseding is not failing: a cancelled read sends nothing. Caught by: a
    /// cancellation drawn as an error the user did not cause.
    #[test]
    fn a_cancelled_read_is_not_a_failure() {
        assert!(was_cancelled(&Error::ContentCancelled));
        assert!(was_cancelled(&Error::ChangesCancelled { changed: 3 }));
        assert!(was_cancelled(&Error::GitReadCancelled {
            arguments: "diff-tree".to_owned()
        }));
        assert!(!was_cancelled(&Error::ContentReadsDisagree {
            path: "a".to_owned(),
            detail: "b".to_owned()
        }));
    }
}
