//! The diff thread, `cairn-diff`: the changes and file-diff lanes of one repository,
//! commits, comparisons and the working tree alike (PRD R4.2, R4.3, R4.5; packet decision
//! L8).
//!
//! It takes its own thread-local repository handle, and keeps the [`DiffSession`] — gix's
//! blob resource cache, reused across commits — and [`Answers`], the commit and comparison
//! answers already given, for as long as what they were read from stays as it was. Before
//! each query it stamps what git and gix read besides the commits
//! ([`super::diff_freshness`]): a moved configuration opens this thread's handle again
//! (the history thread keeps its own) and lets everything kept go; a moved global
//! attribute file, `.gitmodules`, or a staged attribute edit lets the session and every
//! answer go; and a moved `.gitattributes` above a kept answer's paths lets that answer
//! go when it is next found, and the session when it read that directory. An answer read
//! while a file it depends on could still change unseen is answered and not kept. A
//! working-tree answer is never kept, and the engine reads the index and the attributes
//! afresh for every working-tree query.
//!
//! **Scheduling.** Each lane holds at most one request: the newest, since a request is
//! superseded by the next in its lane anyway ([`super::epoch`]). A waiting file diff is
//! served before a waiting changes query (it was asked after it — a changes query would
//! have superseded it otherwise), and a request superseded while it waited is dropped
//! unserved. A file diff asked while a changes query RUNS waits for it to end: one thread
//! serves both lanes, and only a newer changes query, never a file diff, cancels the one
//! running. A changes query waiting while file diffs keep arriving waits behind each of
//! them — a click through files postpones the change set of a commit selected before the
//! clicks, which a newer selection then supersedes anyway. The thread blocks on its queue
//! whenever nothing is waiting; it never spins.
//!
//! **Cancellation.** A query's epoch is its [`cairn_git::Cancel`]: the engine checks it
//! between files and the runner polls it while each `git` read runs, so superseding a diff
//! ends its process group rather than letting it run out and discarding the answer. A fast
//! click through a file list therefore kills each file's one to three `git` processes in
//! turn instead of queueing them. An answer is sent only while its epoch is current, and
//! [`super::Updates::next`] checks again as it arrives, so a superseded answer is dropped
//! on arrival, never drawn.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::SystemTime;

use cairn_git::ops::GitBinary;
use cairn_git::{
    Cancel, ChangesRequest, ContentOptions, DiffSession, Error, LineBudget, Offered, Repository,
    SharedRepository, WorkingTreeDiff,
};
use cairn_model::ShownDiff;
use cairn_model::{ChangeSet, FileDiff};

use super::expand_all::EXPAND_ALL_LINES;

use super::diff_answers::{Answers, searched_paths};
use super::diff_freshness::{Dependence, Directories, Freshness, Moved, SessionReads};
use super::epoch::{Epoch, Epochs, Superseded};
use super::pool::Outbox;
use super::request::{
    AllEnded, AllFrom, AllProgress, Comparison, DiffOptions, DiffQuery, ExpandQuery, ExpandedFile,
    FileQuery, FileTarget, OpenedFile, Update, WorkingSide,
};
use super::startup::Startup;

/// What the diff thread is sent.
#[derive(Debug)]
pub(super) enum DiffJob {
    /// A query, numbered in its lane; boxed, since it carries a file's paths and ids.
    Query { epoch: Epoch, query: Box<DiffQuery> },
    /// Send `diff.context` now, and again each time the handle is opened afresh because the
    /// configuration moved: the diff thread's handle is the one that follows the
    /// configuration, so it is the one whose reading is the user's current `git diff`.
    ConfiguredContext,
    /// The repository is closing, or every handle to it has gone: serve nothing more.
    Stop,
}

/// How many times a working-tree query is asked when git printed lines that are not the
/// lines Cairn read ([`Error::ContentReadsDisagree`]): the file changed between the two
/// reads, which asking again settles, or a clean filter whose output differs run to run,
/// which it never will. Past this the disagreement is shown, never looped on. A commit's
/// content cannot change between reads, so a disagreement there is shown at once.
pub(super) const READ_ATTEMPTS: usize = 3;

/// What the thread is handed to serve with: the git it runs, the epochs it is cancelled
/// by, its queue and outbox, and how the launching environment is read, which opening the
/// repository again needs.
pub(super) struct Serving<'a> {
    pub(super) git: &'a GitBinary,
    pub(super) startup: &'a Startup,
    pub(super) epochs: &'a Epochs,
    pub(super) jobs: &'a Receiver<DiffJob>,
    pub(super) outbox: &'a Outbox,
}

/// The diff thread's loop: runs until it is told to stop or every sender is gone.
pub(super) fn serve_diffs(shared: &SharedRepository, serving: &Serving<'_>) {
    let mut waiting = Waiting::default();
    // Each call rebuilds the object cache and the pack snapshot, so a handle is taken once
    // and replaced only when the configuration moves.
    let mut handle = shared.to_worker();
    let mut opened_at = shared.opened_at();
    let mut carried = None;
    loop {
        match serve_handle(&handle, opened_at, serving, &mut waiting, &mut carried) {
            Ended::Stopped => return,
            Ended::ConfigurationMoved => {
                let started = SystemTime::now();
                match shared.reopen_for(serving.git, |name| serving.startup.parent(name)) {
                    Ok(fresh) => {
                        handle = fresh;
                        opened_at = started;
                    }
                    // What the user's own git would refuse to read: the query that found it
                    // fails, and the next one tries again — the old handle's configuration
                    // is stamped afresh against its own opening, so the file that moved
                    // matches nothing and is found again.
                    Err(error) => {
                        if let Some((epoch, query)) = carried.take()
                            && serving.epochs.is_current(epoch)
                        {
                            serving.outbox.send(
                                Some(epoch),
                                Update::DiffFailed {
                                    query,
                                    message: error.to_string(),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Why serving on one handle ended.
enum Ended {
    Stopped,
    /// The configuration moved under the query carried out; the handle is to be opened
    /// again and that query served on the new one.
    ConfigurationMoved,
}

/// Serves on one handle until the thread stops or the configuration moves.
fn serve_handle(
    repo: &Repository,
    opened_at: SystemTime,
    serving: &Serving<'_>,
    waiting: &mut Waiting,
    carried: &mut Option<(Epoch, DiffQuery)>,
) -> Ended {
    let inputs = repo.diff_inputs();
    let mut freshness = Freshness::opened(&inputs, opened_at);
    let mut kept = Kept::default();
    // A handle opened afresh read the configuration afresh: the context it reads now is
    // the one the user's `git diff` shows, sent before the query that found the move.
    if waiting.context_wanted {
        send_configured_context(repo, serving.outbox);
    }
    loop {
        let (epoch, query) = match carried.take() {
            Some(carried) => carried,
            None => {
                if !waiting.gather(serving.jobs) || serving.epochs.is_stopping() {
                    return Ended::Stopped;
                }
                if std::mem::take(&mut waiting.context_due) {
                    send_configured_context(repo, serving.outbox);
                }
                match waiting.next(serving.epochs) {
                    Some(next) => next,
                    // Everything that waited was superseded; the next turn blocks.
                    None => continue,
                }
            }
        };
        if !serving.epochs.is_current(epoch) {
            continue;
        }
        let started = SystemTime::now();
        let checked = freshness.check(repo, &inputs, started);
        if kept.renew(checked.moved) {
            *carried = Some((epoch, query));
            return Ended::ConfigurationMoved;
        }
        let mut served = Served {
            repo,
            git: serving.git,
            epochs: serving.epochs,
            epoch,
            cancel: serving.epochs.watch(epoch),
            outbox: serving.outbox,
            directories: Directories::new(&inputs, started),
            settled: checked.settled,
        };
        served.answer(query, &mut kept);
    }
}

/// `diff.context` as this handle reads it. A value git refuses sends nothing: the views keep
/// the context they have, and every diff asked fails with the configuration's error, as the
/// user's `git diff` does.
fn send_configured_context(repo: &Repository, outbox: &Outbox) {
    if let Ok(context) = repo.configured_context() {
        outbox.send(None, Update::ConfiguredContext { context });
    }
}

/// What the thread keeps between queries on one handle.
#[derive(Default)]
struct Kept<'repo> {
    session: Option<DiffSession<'repo>>,
    /// The directories whose attributes `session` has read, as they were.
    reads: SessionReads,
    answers: Answers,
}

impl<'repo> Kept<'repo> {
    /// Lets go of what `moved` says is no longer what git would answer; `true` when the
    /// handle itself is to be opened again.
    fn renew(&mut self, moved: Moved) -> bool {
        match moved {
            Moved::Nothing => false,
            Moved::Answers => {
                self.let_go();
                false
            }
            Moved::Configuration => {
                self.let_go();
                true
            }
        }
    }

    fn let_go(&mut self) {
        self.session = None;
        self.reads.clear();
        self.answers = Answers::default();
    }

    /// The session, for paths whose attributes `read` stamps: kept while every directory
    /// it has read among theirs is as it was, opened on first use and again after; one
    /// that could not be opened is tried again on the next query that needs it.
    fn session_for(
        &mut self,
        repo: &'repo Repository,
        read: &Dependence,
    ) -> Result<&mut DiffSession<'repo>, Error> {
        if !self.reads.still_for(read) {
            self.session = None;
        }
        let session = match self.session.take() {
            Some(session) => session,
            None => {
                self.reads.clear();
                repo.diff_session()?
            }
        };
        self.reads.record(read);
        Ok(self.session.insert(session))
    }
}

/// The newest request waiting in each lane.
#[derive(Debug, Default)]
struct Waiting {
    changes: Option<(Epoch, DiffQuery)>,
    file: Option<(Epoch, DiffQuery)>,
    stopped: bool,
    /// The configured context was asked for: it is sent again on every handle opened.
    context_wanted: bool,
    /// The configured context is to be sent before the next query is served.
    context_due: bool,
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
            DiffJob::ConfiguredContext => {
                self.context_wanted = true;
                self.context_due = true;
            }
            DiffJob::Query { epoch, query } => match *query {
                query @ DiffQuery::Changes(_) => self.changes = Some((epoch, query)),
                query @ (DiffQuery::File(_) | DiffQuery::Expand(_)) => {
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
struct Served<'a, 'i> {
    repo: &'a Repository,
    git: &'a GitBinary,
    epochs: &'a Epochs,
    epoch: Epoch,
    cancel: Superseded,
    outbox: &'a Outbox,
    /// The working tree's `.gitattributes` as this query found them.
    directories: Directories<'i>,
    /// Every configuration and global stamp settled: an answer read now may be kept.
    settled: bool,
}

impl<'a> Served<'a, '_> {
    fn answer(&mut self, query: DiffQuery, kept: &mut Kept<'a>) {
        let outcome = match &query {
            DiffQuery::Changes(of) => self
                .change_set(*of, &mut kept.answers)
                .map(|changes| Some(Update::Changes { of: *of, changes })),
            // Prepared for the views here, on this thread, never on the UI thread: both rows'
            // indexes and a pass over the drawn bytes, 39 ms for a 64 MiB file (phase 06 QA).
            DiffQuery::File(asked) => self.file_diff(asked, kept).map(|diff| {
                Some(Update::FileDiff {
                    query: asked.clone(),
                    diff: diff.map(|diff| Box::new(ShownDiff::new(diff, asked.options.context))),
                })
            }),
            // Answered a page at a time as it is read, so its pages are sent from inside.
            DiffQuery::Expand(asked) => self.expand(asked, kept).map(|()| None),
        };
        match outcome {
            Ok(Some(update)) => self.send(update),
            Ok(None) => {}
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

    /// `of`'s change set, kept or asked of git; one too large to keep, or read while what
    /// it depends on could still change unseen, is still answered.
    fn change_set(&mut self, of: Comparison, answers: &mut Answers) -> Result<ChangeSet, Error> {
        let directories = &mut self.directories;
        if let Some(kept) = answers.change_set(of, |read| directories.still(read)) {
            return Ok(kept.clone());
        }
        let changes = self.repo.changes(self.git, &request(of), &self.cancel)?;
        let read = self.directories.of(searched_paths(&changes));
        if self.settled && read.is_settled() {
            answers.keep_change_set(of, changes.clone(), read);
        }
        Ok(changes)
    }

    fn file_diff(
        &mut self,
        asked: &FileQuery,
        kept: &mut Kept<'a>,
    ) -> Result<Option<FileDiff>, Error> {
        let options = content_options(&asked.options);
        match &asked.target {
            FileTarget::Committed { of, file } => {
                let directories = &mut self.directories;
                if let Some(diff) = kept
                    .answers
                    .file_diff(asked, |read| directories.still(read))
                {
                    return Ok(Some(diff.clone()));
                }
                let read = self.directories.of([&file.old_path, &file.new_path]);
                let session = kept.session_for(self.repo, &read)?;
                // No second asking: a commit's content cannot change between two reads.
                let diff =
                    session.file_diff(self.git, &request(*of), file, &options, &self.cancel)?;
                if self.settled && read.is_settled() {
                    kept.answers
                        .keep_file_diff(asked.clone(), diff.clone(), read);
                }
                Ok(Some(diff))
            }
            // Never kept: the working tree moves under it.
            FileTarget::WorkingTree { path, side } => asking_again(&self.cancel, || {
                self.repo
                    .working_tree_diff(self.git, path, which(*side), &options, &self.cancel)
            }),
        }
    }

    /// Files opened in place (R5.3), answered a page at a time as each is read: first the
    /// files opened by name — those read past the limits apart, since a page is read at one
    /// set of options — then, when Expand All is on its way, the rest of the change set from
    /// where it stands, until its line budget ([`EXPAND_ALL_LINES`]) is spent or every file is
    /// open. Each page is read with the engine's per-page bound and sent as soon as it is
    /// prepared, so the memory held is a page's, and a file's failure is that file's outcome.
    /// The epoch is checked between files and polled while each read runs, so a newer request
    /// in the lane — another expansion, a file diff, a changes query — ends this one at the
    /// next file or kills its read. Nothing is kept: a page is read once and handed over.
    fn expand(&mut self, asked: &ExpandQuery, kept: &mut Kept<'a>) -> Result<(), Error> {
        let changes = asked.changes.as_ref();
        let request = request(asked.of);
        let named = |load_anyway: bool| -> Vec<usize> {
            asked
                .files
                .iter()
                .filter(|file| file.load_anyway == load_anyway)
                .map(|file| file.index)
                .collect()
        };
        let all_from = asked.all.map(|all| all.next.min(changes.files.len()));
        // The directories whose attributes this reads: the files named, and every file Expand
        // All may still reach.
        let reached = asked
            .files
            .iter()
            .map(|file| file.index)
            .chain(
                all_from
                    .into_iter()
                    .flat_map(|from| from..changes.files.len()),
            )
            .filter_map(|index| changes.files.get(index))
            .flat_map(|file| [&file.old_path, &file.new_path]);
        let read = self.directories.of(reached);
        let session = kept.session_for(self.repo, &read)?;

        for load_anyway in [false, true] {
            let offered = named(load_anyway);
            let mut options = asked.options;
            options.load_anyway = load_anyway;
            let content = content_options(&options);
            let mut from = 0;
            while from < offered.len() {
                let page = session.page(
                    self.git,
                    &request,
                    Offered {
                        changes,
                        files: &offered[from..],
                    },
                    None,
                    &content,
                    &self.cancel,
                )?;
                // A page always takes a file when it has no budget; this only keeps a page
                // that took nothing from looping.
                if page.taken == 0 {
                    break;
                }
                from += page.taken;
                self.send(Update::Expanded {
                    of: asked.of,
                    options: asked.options,
                    files: prepared(page.files, asked.options, load_anyway, false),
                    all: None,
                });
            }
        }

        let (Some(start), Some(all)) = (all_from, asked.all) else {
            return Ok(());
        };
        let offered: Vec<usize> = (start..changes.files.len()).collect();
        let mut budget = LineBudget::resumed(EXPAND_ALL_LINES, all.spent);
        let content = content_options(&asked.options);
        let mut from = 0;
        loop {
            let page = if from < offered.len() && !budget.is_spent() {
                Some(session.page(
                    self.git,
                    &request,
                    Offered {
                        changes,
                        files: &offered[from..],
                    },
                    Some(&mut budget),
                    &content,
                    &self.cancel,
                )?)
            } else {
                None
            };
            let (files, taken) = match page {
                Some(page) => (page.files, page.taken),
                None => (Vec::new(), 0),
            };
            from += taken;
            let ended = if from >= offered.len() {
                Some(AllEnded::Every)
            } else if budget.is_spent() || taken == 0 {
                Some(AllEnded::Budget)
            } else {
                None
            };
            self.send(Update::Expanded {
                of: asked.of,
                options: asked.options,
                files: prepared(files, asked.options, false, true),
                all: Some(AllProgress {
                    at: AllFrom {
                        next: start + from,
                        spent: budget.spent(),
                    },
                    ended,
                }),
            });
            if ended.is_some() {
                return Ok(());
            }
        }
    }
}

/// A page's files prepared for the views on this thread, as a single file's diff is: each
/// outcome a [`ShownDiff`] at the context asked, or its failure as display text.
fn prepared(
    files: Vec<(usize, Result<FileDiff, Error>)>,
    options: DiffOptions,
    load_anyway: bool,
    by_all: bool,
) -> Vec<ExpandedFile> {
    files
        .into_iter()
        .map(|(index, outcome)| ExpandedFile {
            file: OpenedFile { index, load_anyway },
            by_all,
            outcome: outcome
                .map(|diff| Box::new(ShownDiff::new(diff, options.context)))
                .map_err(|error| error.to_string()),
        })
        .collect()
}

/// Asks again while git's lines and Cairn's read of the same working-tree content
/// disagree, up to [`READ_ATTEMPTS`] in all, and not once the query is superseded.
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use cairn_model::Oid;

    use super::*;
    use crate::worker::diff_answers::tests::{committed, text_diff};
    use crate::worker::diff_freshness::SETTLING;
    use crate::worker::epoch::QueryLane;
    use crate::worker::fetch_tests::BorrowedRepository;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    fn query(epochs: &Epochs, lane: QueryLane, n: u8) -> DiffJob {
        let of = Comparison::Commit(oid(n));
        let query = match lane {
            QueryLane::Changes => DiffQuery::Changes(of),
            QueryLane::FileDiff | QueryLane::History | QueryLane::FileFilter => {
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

    /// The design input on stale reads: a disagreement is asked again — three times in
    /// all, so a file saved once mid-read settles on the third — and then shown; never once
    /// the query is superseded. Caught by: no retry (`READ_ATTEMPTS` of one, under which a
    /// file saved mid-read shows an error), an unbounded one (a filter whose output varies
    /// loops the thread for good), or retrying a superseded query. The counts are written
    /// out, not read from the constant, so lowering it fails here.
    #[test]
    fn a_disagreeing_read_is_asked_again_a_bounded_number_of_times() {
        let disagree = || Error::ContentReadsDisagree {
            path: "a.txt".to_owned(),
            detail: "line 1".to_owned(),
        };
        let asked = Cell::new(0);
        let settled: Result<u8, Error> = asking_again(&cairn_git::CancelSignal::new(), || {
            asked.set(asked.get() + 1);
            if asked.get() < 3 {
                Err(disagree())
            } else {
                Ok(7)
            }
        });
        assert!(matches!(settled, Ok(7)), "{settled:?}");
        assert_eq!(asked.get(), 3);

        asked.set(0);
        let never: Result<u8, Error> = asking_again(&cairn_git::CancelSignal::new(), || {
            asked.set(asked.get() + 1);
            Err(disagree())
        });
        assert!(matches!(never, Err(Error::ContentReadsDisagree { .. })));
        assert_eq!(asked.get(), 3, "the retry is not bounded at three");

        asked.set(0);
        let superseded = cairn_git::CancelSignal::new();
        superseded.cancel();
        let _ = asking_again(&superseded, || -> Result<u8, Error> {
            asked.set(asked.get() + 1);
            Err(disagree())
        });
        assert_eq!(asked.get(), 1, "a superseded query was asked again");
    }

    /// T3, as the stamp design pins it: what each tier's move lets go. A moved global
    /// attribute file drops the session and every answer and keeps the handle; a moved
    /// configuration drops them and asks for the handle to be opened again; a stat-only
    /// rewrite of the index — what `git update-index --refresh` does — drops nothing.
    /// Over a fixture of this checkout's objects, with real files and the real
    /// `Freshness`, each query started past the settling time so every stamp is trusted.
    /// Caught by: a renewal that keeps the session (gix's stack holds `info/attributes`
    /// for its life), one that keeps the answers, keying the index on its file, or a
    /// moved configuration answered on the old handle.
    #[test]
    fn each_tier_lets_go_of_what_its_move_makes_stale() {
        let fixture = BorrowedRepository::new(&format!("cairn-diff-tiers-{}", std::process::id()));
        let checkout = cairn_git::SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let head = checkout.to_worker().head_id().unwrap();
        fixture.point_main_at(&head.to_string());
        let dot = fixture.fixture.path.join(".git");
        std::fs::copy(checkout.git_dir().join("index"), dot.join("index")).unwrap();

        let repo = cairn_git::Repository::discover(&fixture.fixture.path).unwrap();
        let inputs = repo.diff_inputs();
        let past = || SystemTime::now() + SETTLING + Duration::from_secs(1);
        let mut freshness = Freshness::opened(&inputs, past());
        let mut kept = Kept::default();
        fn fill<'r>(kept: &mut Kept<'r>, repo: &'r Repository) {
            kept.session_for(repo, &Dependence::default()).unwrap();
            kept.answers.keep_file_diff(
                committed(1, DiffOptions::default()),
                text_diff(1, 4),
                Dependence::default(),
            );
        }
        let held = |kept: &Kept<'_>| (kept.session.is_some(), kept.answers.counts().1);

        assert_eq!(
            freshness.check(&repo, &inputs, past()).moved,
            Moved::Nothing
        );
        fill(&mut kept, &repo);
        assert!(!kept.renew(freshness.check(&repo, &inputs, past()).moved));
        assert_eq!(
            held(&kept),
            (true, 1),
            "nothing moved, and something was let go"
        );

        // The index rewritten as a refresh writes it: a new file, the same entries.
        let bytes = std::fs::read(dot.join("index")).unwrap();
        std::fs::write(dot.join("index.lock"), &bytes).unwrap();
        std::fs::rename(dot.join("index.lock"), dot.join("index")).unwrap();
        assert!(!kept.renew(freshness.check(&repo, &inputs, past()).moved));
        assert_eq!(
            held(&kept),
            (true, 1),
            "a stat-only refresh let go of what was kept"
        );

        std::fs::create_dir_all(dot.join("info")).unwrap();
        std::fs::write(dot.join("info/attributes"), "*.rs -diff\n").unwrap();
        assert!(!kept.renew(freshness.check(&repo, &inputs, past()).moved));
        assert_eq!(
            held(&kept),
            (false, 0),
            "an info/attributes edit kept the session or answers"
        );

        fill(&mut kept, &repo);
        let mut config = std::fs::read_to_string(dot.join("config")).unwrap();
        config.push_str("[diff]\n\talgorithm = patience\n");
        std::fs::write(dot.join("config"), config).unwrap();
        assert!(
            kept.renew(freshness.check(&repo, &inputs, past()).moved),
            "a configuration edit did not ask for the handle to be opened again"
        );
        assert_eq!(held(&kept), (false, 0));
    }

    /// The session goes when a directory it read attributes from moved, and stays for one
    /// it never read. Caught by: a session kept across a `.gitattributes` edit at the top
    /// of the tree, which gix's stack never reads again.
    #[test]
    fn the_session_goes_when_a_directory_it_read_moved() {
        let fixture =
            BorrowedRepository::new(&format!("cairn-diff-session-{}", std::process::id()));
        let repo = cairn_git::Repository::discover(&fixture.fixture.path).unwrap();
        let inputs = repo.diff_inputs();
        let past = || SystemTime::now() + SETTLING + Duration::from_secs(1);
        let top = cairn_model::RepoPath::from("a.txt");
        let mut kept = Kept::default();

        let read = Directories::new(&inputs, past()).of([&top]);
        kept.session_for(&repo, &read).unwrap();
        let elsewhere = cairn_model::RepoPath::from("sub/b.txt");
        std::fs::create_dir_all(fixture.fixture.path.join("sub")).unwrap();
        std::fs::write(fixture.fixture.path.join("sub/.gitattributes"), "* -diff\n").unwrap();
        // `sub` was never read by this session; the top of the tree was, and has not moved.
        let read = Directories::new(&inputs, past()).of([&top]);
        assert!(kept.reads.still_for(&read));
        let read = Directories::new(&inputs, past()).of([&elsewhere]);
        kept.session_for(&repo, &read).unwrap();
        assert!(kept.session.is_some());

        std::fs::write(fixture.fixture.path.join(".gitattributes"), "* -diff\n").unwrap();
        let read = Directories::new(&inputs, past()).of([&top]);
        assert!(
            !kept.reads.still_for(&read),
            "the top of the tree's edit was not seen"
        );
    }

    /// R7.2: two commits are asked tip against tip with the window's base as git's old side,
    /// so `git diff <base> <tip>` is what answers (pinned against git by
    /// `a_comparison_of_two_commits_reads_as_git_diff_of_the_pair_both_ways`). Caught by: the
    /// pair handed to the engine the other way round.
    #[test]
    fn a_comparison_is_asked_with_its_base_as_the_old_side() {
        assert_eq!(
            request(Comparison::Between {
                old: oid(1),
                new: oid(2)
            }),
            ChangesRequest::between(oid(1), oid(2))
        );
        assert_eq!(
            request(Comparison::Commit(oid(3))),
            ChangesRequest::commit(oid(3))
        );
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
