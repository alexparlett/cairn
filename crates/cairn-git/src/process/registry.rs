//! A repository's invocations: those still running, and the log of those that
//! are over.
//!
//! Each open repository holds one [`Processes`], shared by every handle on it
//! ([`crate::SharedRepository`] and each worker's [`crate::Repository`]), and
//! an invocation built with `in_repository` carries a [`Registration`] into
//! it. That registration is the one place an invocation's life is booked:
//!
//! - **It enters the registry when its process is spawned**, and a spawn that
//!   fails is logged as never started and enters nothing.
//! - **It leaves when the invocation is over**, whichever thread ends it — the
//!   caller's `finish`, `records` or `collect`, a cancel, the reaper thread a
//!   drop hands it to, or the runner itself ending it — and in leaving it
//!   writes its one record to the log. The registration is consumed by that,
//!   so a second record cannot be written; and if it is ever dropped without
//!   having been, its drop writes the record (exit unknown) rather than lose
//!   the invocation.
//!
//! "Over" is when the runner concludes it: the leader reaped and the pipes
//! drained, or abandoned after the drain bound. The one exception is a drop
//! with no thread to reap on, which sends `SIGKILL` and reaps only if it
//! already can: that invocation leaves the registry then, reaped or not,
//! because nothing is left that would reap it later.
//!
//! [`Processes::end_all`] is what closing a repository runs: every invocation
//! in the registry is asked to end, the way a cancel asks, and the call waits
//! up to a bound for them to leave. It waits, so it is a worker's call, never
//! the UI thread's.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use cairn_model::{CommandExit, CommandRecord};

use super::command_log::CommandLog;
use super::group::{Group, TERMINATION_GRACE};
use super::runner::DRAIN_BOUND;

/// How long closing a repository waits for its invocations to be reaped: 3 s.
///
/// Closing ends each one the way a cancel does — `SIGTERM` to its group, then
/// `SIGKILL` once [`TERMINATION_GRACE`] (2 s) has passed — so a git that
/// ignores `SIGTERM` is reaped a little after the grace, and one whose pipe
/// is held by a process that left its group after a further [`DRAIN_BOUND`]
/// (250 ms). Three seconds is the grace and the bound with three-quarters of
/// a second to spare for a loaded machine's scheduler; anything still not
/// reaped then has had `SIGKILL` and is past what waiting can change, so the
/// window closes rather than hang on it.
pub const CLOSE_BOUND: Duration = Duration::from_secs(3);

const _: () = assert!(
    CLOSE_BOUND.as_millis() > TERMINATION_GRACE.as_millis() + DRAIN_BOUND.as_millis(),
    "CLOSE_BOUND must outlast the grace and the drain bound, or a close gives up on a process \
     the runner is still ending"
);

/// One repository's running invocations and its command log.
#[derive(Debug, Default)]
pub(crate) struct Processes {
    running: Mutex<BTreeMap<u64, Arc<Group>>>,
    /// Notified each time an invocation leaves `running`.
    left: Condvar,
    next: AtomicU64,
    /// Set by [`Processes::end_all`]: from then on an invocation is asked to
    /// end the moment it enters.
    closing: AtomicBool,
    log: Mutex<CommandLog>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Processes {
    /// Every record in the log, oldest first.
    pub(crate) fn log(&self) -> Vec<CommandRecord> {
        lock(&self.log).records()
    }

    /// How many invocations are running now.
    #[cfg(test)]
    pub(crate) fn running(&self) -> usize {
        lock(&self.running).len()
    }

    /// Asks every running invocation to end — `SIGTERM` to its group now, and
    /// the rest by the thread driving it, as a cancel does — and waits up to
    /// `bound` for all of them to be over. A write that outlasts the grace is
    /// `SIGKILL`ed, which can strand its lock files; its cancellation still
    /// lists them, to whoever drives it, but on a close nobody may be left to
    /// show them, and the next write there trips over them. Returns how many were still
    /// running when it stopped waiting: zero, unless one outlived the bound.
    /// Every invocation that enters after this is asked to end at once, since
    /// the repository is closing.
    pub(crate) fn end_all(&self, bound: Duration) -> usize {
        self.closing.store(true, Ordering::Release);
        let groups: Vec<Arc<Group>> = lock(&self.running).values().cloned().collect();
        for group in &groups {
            group.request_end();
        }
        let (running, _) = self
            .left
            .wait_timeout_while(lock(&self.running), bound, |running| !running.is_empty())
            .unwrap_or_else(PoisonError::into_inner);
        running.len()
    }

    fn enter(&self, group: &Arc<Group>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.running).insert(id, Arc::clone(group));
        if self.closing.load(Ordering::Acquire) {
            group.request_end();
        }
        id
    }

    /// Writes `record` and lets `id` leave, under the registry's lock held
    /// across both: so nobody can see the registry without it until the log
    /// has it, and whoever sees the registry empty sees the log complete. The
    /// lock order is the registry's, then the log's; nothing takes them the
    /// other way round.
    fn leave(&self, id: Option<u64>, record: CommandRecord) {
        let mut running = lock(&self.running);
        lock(&self.log).push(record);
        if let Some(id) = id {
            running.remove(&id);
            drop(running);
            self.left.notify_all();
        }
    }
}

/// One invocation's booking in its repository's [`Processes`]: made before
/// the spawn, entered once there is a process, and finished exactly once.
#[derive(Debug)]
pub(super) struct Registration {
    processes: Arc<Processes>,
    id: Option<u64>,
    arguments: Vec<String>,
    directory: Option<PathBuf>,
    started: SystemTime,
    clock: Instant,
    finished: bool,
}

impl Registration {
    /// Booked as starting now, with what the invocation was given. Nothing
    /// from its environment: the record has nowhere to put it.
    pub(super) fn new(
        processes: &Arc<Processes>,
        arguments: Vec<String>,
        directory: Option<PathBuf>,
    ) -> Self {
        Self {
            processes: Arc::clone(processes),
            id: None,
            arguments,
            directory,
            started: SystemTime::now(),
            clock: Instant::now(),
            finished: false,
        }
    }

    /// The process is spawned: it is running until this is finished.
    pub(super) fn enter(&mut self, group: &Arc<Group>) {
        if self.id.is_none() {
            self.id = Some(self.processes.enter(group));
        }
    }

    /// The process never started.
    pub(super) fn not_started(self) {
        self.finish(CommandExit::NotStarted, false, String::new());
    }

    /// The invocation is over: one record, and out of the registry.
    pub(super) fn finish(mut self, exit: CommandExit, cancelled: bool, stderr: String) {
        self.book(exit, cancelled, stderr);
    }

    fn book(&mut self, exit: CommandExit, cancelled: bool, stderr: String) {
        if self.finished {
            return;
        }
        self.finished = true;
        let record = CommandRecord {
            arguments: std::mem::take(&mut self.arguments),
            directory: self.directory.take(),
            started: self.started,
            duration: self.clock.elapsed(),
            exit,
            cancelled,
            stderr,
        };
        self.processes.leave(self.id.take(), record);
    }
}

impl Drop for Registration {
    /// Never the path an invocation takes — each ends by [`Registration::finish`]
    /// — but if one ever did not, it is still recorded, as an end nobody saw.
    fn drop(&mut self) {
        self.book(CommandExit::Unknown, false, String::new());
    }
}

/// How an exit status reads in the log.
pub(super) fn exit_of(status: std::process::ExitStatus) -> CommandExit {
    use std::os::unix::process::ExitStatusExt as _;
    match (status.code(), status.signal()) {
        (Some(code), _) => CommandExit::Code(code),
        (None, Some(signal)) => CommandExit::Signal(signal),
        (None, None) => CommandExit::Unknown,
    }
}

/// G17's engine half and R6.2: one record per invocation however it ends, and a
/// registry that holds exactly the invocations still running. Against a stub
/// `git` run in the Cairn checkout, each test with a repository handle of its
/// own, so its registry and log are its own.
#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;
    use std::time::{Duration, Instant, SystemTime};

    use cairn_model::{CommandExit, CommandRecord};

    use super::super::cli::Read;
    use super::super::runner::Invocation;
    use super::super::stub_git::{StubGit, discover_retrying};
    use super::{CLOSE_BOUND, Processes};
    use crate::process::command_log::{LOG_BYTES, LOG_ENTRIES};
    use crate::{CancelSignal, Error, Repository, SharedRepository};

    fn stub(rest: &str) -> StubGit {
        StubGit::with_git(&format!(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n{rest}"
        ))
    }

    /// Says so, then hangs in a background `sleep` it waits on; dies of `SIGTERM`.
    const HANGING: &str = "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
        sleep 30 & echo hanging >&2; wait";

    /// Ignores `SIGTERM` and hangs, so only `SIGKILL` ends it.
    const IGNORING_TERM: &str = "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
        trap '' TERM; echo hanging >&2; while :; do sleep 1; done";

    const DEADLINE: Duration = Duration::from_secs(10);

    fn repo() -> Repository {
        Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap()
    }

    fn processes(repo: &Repository) -> Arc<Processes> {
        Arc::clone(repo.processes())
    }

    fn started(stub: &StubGit, repo: &Repository) -> Invocation<Read> {
        discover_retrying(stub.environment())
            .unwrap()
            .read_invocation()
            .in_repository(repo)
            .args(["stub", "an argument"])
            .start()
            .unwrap()
    }

    fn eventually(mut condition: impl FnMut() -> bool) -> bool {
        let started = Instant::now();
        while started.elapsed() < DEADLINE {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    /// The one record the log holds once `processes` has nothing running, and
    /// that it is the only one.
    fn the_one_record(processes: &Processes) -> CommandRecord {
        assert!(
            eventually(|| processes.running() == 0),
            "{} invocations never left the registry",
            processes.running()
        );
        let mut log = processes.log();
        assert_eq!(
            log.len(),
            1,
            "one invocation, recorded {} times: {log:?}",
            log.len()
        );
        log.remove(0)
    }

    /// Drives `invocation` and cancels it the moment it says it is hanging,
    /// through its kill handle or through the signal it polls.
    fn cancelled_once_hanging(invocation: Invocation<Read>, by_handle: bool) -> Result<(), Error> {
        let signal = CancelSignal::new();
        let handle = invocation.kill_handle();
        invocation
            .finish(
                &signal,
                |_| {},
                |line| {
                    if line == "hanging" {
                        if by_handle {
                            handle.kill();
                        } else {
                            signal.cancel();
                        }
                    }
                },
            )
            .map(drop)
    }

    /// R8.1 field by field, from an invocation that finished: what it was
    /// given, where it ran, when, for how long, its status, not cancelled, and
    /// what it said on stderr — and it was in the registry from the spawn until
    /// it was over, not after. Caught by: entering at the finish rather than the
    /// spawn, leaving before the record is written, or a field left out.
    #[test]
    fn a_finished_invocation_is_logged_once_with_every_field() {
        let stub = stub("echo 'on stdout'; echo 'said this' >&2; exit 0");
        let repo = repo();
        let processes = processes(&repo);
        let before = SystemTime::now();
        let invocation = started(&stub, &repo);
        assert_eq!(
            processes.running(),
            1,
            "a spawned invocation is not in the registry"
        );
        assert!(processes.log().is_empty(), "logged before it was over");
        invocation
            .finish(&CancelSignal::new(), |_| {}, |_| {})
            .unwrap();
        let after = SystemTime::now();

        let record = the_one_record(&processes);
        assert_eq!(record.arguments, ["stub", "an argument"]);
        assert_eq!(record.directory.as_deref(), repo.workdir());
        assert!(
            before <= record.started && record.started <= after,
            "{record:?}"
        );
        assert!(
            record.duration <= after.duration_since(before).unwrap(),
            "{record:?}"
        );
        assert_eq!(record.exit, CommandExit::Code(0));
        assert!(!record.cancelled);
        assert_eq!(record.stderr, "said this");
    }

    /// A failure is recorded as what it was: its status, and what it said.
    #[test]
    fn a_failed_invocation_is_logged_once_with_its_status() {
        let stub = stub("echo 'fatal: no' >&2; exit 3");
        let repo = repo();
        let processes = processes(&repo);
        let outcome = started(&stub, &repo).finish(&CancelSignal::new(), |_| {}, |_| {});
        assert!(
            matches!(outcome, Err(Error::GitFailed { .. })),
            "{outcome:?}"
        );
        let record = the_one_record(&processes);
        assert_eq!(record.exit, CommandExit::Code(3));
        assert!(!record.cancelled);
        assert_eq!(record.stderr, "fatal: no");
    }

    /// A cancel, by the kill handle and by the polled signal, is recorded once,
    /// as cancelled, with the signal that ended it. Caught by: recording the
    /// outcome on the caller's side and the end on the runner's (twice), or
    /// reading `cancelled` off something other than what the caller was told.
    #[test]
    fn a_cancelled_invocation_is_logged_once_as_cancelled() {
        for by_handle in [true, false] {
            let stub = stub(HANGING);
            let repo = repo();
            let processes = processes(&repo);
            let outcome = cancelled_once_hanging(started(&stub, &repo), by_handle);
            assert!(
                matches!(outcome, Err(Error::GitReadCancelled { .. })),
                "{outcome:?}"
            );
            let record = the_one_record(&processes);
            assert!(record.cancelled, "by_handle {by_handle}: {record:?}");
            assert_eq!(record.exit, CommandExit::Signal(15), "{record:?}");
            assert_eq!(record.stderr, "hanging");
        }
    }

    /// A drop is recorded once, by the reaper thread, as cancelled, and the
    /// invocation leaves the registry when the reaper has reaped it. Caught by:
    /// recording at the drop (before the end is known) or never.
    #[test]
    fn a_dropped_invocation_is_logged_once_by_its_reaper() {
        let stub = stub(HANGING);
        let repo = repo();
        let processes = processes(&repo);
        let invocation = started(&stub, &repo);
        drop(invocation);
        let record = the_one_record(&processes);
        assert!(record.cancelled, "{record:?}");
        assert_eq!(record.arguments, ["stub", "an argument"]);
        assert_eq!(
            record.exit,
            CommandExit::Signal(15),
            "recorded before the reaper knew how it ended: {record:?}"
        );
    }

    /// A drop with no thread to reap on is still recorded once, as cancelled,
    /// and leaves the registry at once: nothing is left that would reap it later.
    #[test]
    fn a_drop_with_no_reaper_thread_is_logged_once() {
        fn no_thread(_: &str, _: Box<dyn FnOnce() + Send>) -> io::Result<()> {
            Err(io::Error::other("no thread"))
        }
        // Says so, leaves a marker once the line is in the pipe, then hangs.
        let stub = StubGit::with_git_from(|directory| {
            format!(
                "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                 PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
                 echo hanging >&2; touch '{}'; sleep 30",
                directory.join("spoke").display()
            )
        });
        let repo = repo();
        let processes = processes(&repo);
        let invocation = started(&stub, &repo).without_a_reaper(no_thread);
        assert!(
            eventually(|| stub.directory().join("spoke").exists()),
            "the stub never spoke"
        );
        // The line is in the pipe; the reader, blocked on it already, queues it
        // in microseconds. The margin is for a loaded machine's scheduler.
        std::thread::sleep(Duration::from_millis(200));
        drop(invocation);
        assert_eq!(
            processes.running(),
            0,
            "an abandoned invocation stayed registered"
        );
        let record = the_one_record(&processes);
        assert!(record.cancelled, "{record:?}");
        assert!(
            matches!(record.exit, CommandExit::Signal(9) | CommandExit::Unknown),
            "the SIGKILL's end, or none seen yet: {record:?}"
        );
        assert_eq!(record.stderr, "hanging", "what was queued was dropped");
    }

    /// The same path when the stub has already exited: the reap the kill makes
    /// finds the status, and the record carries it rather than `Unknown`.
    /// Linux: it waits for the stub to be a zombie. Caught by: a record that
    /// says `Unknown` whatever the reap found.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_drop_with_no_reaper_thread_records_an_exit_it_can_see() {
        fn no_thread(_: &str, _: Box<dyn FnOnce() + Send>) -> io::Result<()> {
            Err(io::Error::other("no thread"))
        }
        let stub = stub("exit 3");
        let repo = repo();
        let processes = processes(&repo);
        let invocation = started(&stub, &repo).without_a_reaper(no_thread);
        let pid = invocation.id();
        assert!(eventually(|| {
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .unwrap_or_default()
                .contains(") Z ")
        }));
        drop(invocation);
        let record = the_one_record(&processes);
        assert_eq!(record.exit, CommandExit::Code(3), "{record:?}");
    }

    /// A failed stdin write is the outcome the caller is told of, but the log
    /// records how the leader itself ended: the signal the runner sent. Caught
    /// by: recording `Unknown` because the outcome was an error.
    #[test]
    fn a_failed_stdin_write_is_logged_with_the_leaders_own_exit() {
        struct Failing;
        impl io::Write for Failing {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("the disk went away"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let stub = stub(HANGING);
        let repo = repo();
        let processes = processes(&repo);
        let invocation = started(&stub, &repo);
        super::super::pipes::feed(Failing, b"input", &invocation.group());
        let outcome = invocation.finish(&CancelSignal::new(), |_| {}, |_| {});
        assert!(
            matches!(outcome, Err(Error::GitUnwatched { .. })),
            "{outcome:?}"
        );
        let record = the_one_record(&processes);
        assert_eq!(record.exit, CommandExit::Signal(15), "{record:?}");
        assert!(!record.cancelled, "{record:?}");
    }

    /// A spawn that fails is recorded once, as never started, and never enters
    /// the registry. Caught by: recording only invocations that ran.
    #[test]
    fn an_invocation_that_never_started_is_logged_once() {
        let stub = stub("exit 0");
        let git = discover_retrying(stub.environment()).unwrap();
        std::fs::remove_file(stub.directory().join("git")).unwrap();
        let repo = repo();
        let processes = processes(&repo);
        let outcome = git
            .read_invocation()
            .in_repository(&repo)
            .arg("stub")
            .start();
        assert!(
            matches!(outcome, Err(Error::GitNotStarted { .. })),
            "{outcome:?}"
        );
        let record = the_one_record(&processes);
        assert_eq!(record.exit, CommandExit::NotStarted);
        assert_eq!(record.arguments, ["stub"]);
        assert_eq!(record.directory.as_deref(), repo.workdir());
        assert!(!record.cancelled);
        assert_eq!(record.stderr, "");
    }

    /// The outcomes the runner itself causes — a thread that could not start,
    /// a crossed ceiling — are recorded once and are not cancellations, as the
    /// caller is not told they were. Caught by: deciding `cancelled` from the
    /// end request alone, which both of these send.
    #[test]
    fn what_the_runner_ends_is_logged_once_and_not_as_a_cancel() {
        let stub = stub(HANGING);
        let repo = repo();
        let processes = processes(&repo);
        let outcome = discover_retrying(stub.environment())
            .unwrap()
            .read_invocation()
            .in_repository(&repo)
            .arg("stub")
            .start_without_threads();
        assert!(
            matches!(outcome, Err(Error::GitUnwatched { .. })),
            "{outcome:?}"
        );
        let record = the_one_record(&processes);
        assert!(!record.cancelled, "{record:?}");

        let flooding = super::tests::stub("PATH=/usr/bin:/bin; yes");
        let other = super::tests::repo();
        let processes = super::tests::processes(&other);
        let outcome = started(&flooding, &other).collect(&CancelSignal::new(), 1024, |_| {});
        assert!(
            matches!(outcome, Err(Error::GitOutputTooLarge { .. })),
            "{outcome:?}"
        );
        let record = the_one_record(&processes);
        assert!(!record.cancelled, "{record:?}");
    }

    /// A cancel that loses the race to a clean exit is recorded as the success
    /// the caller was told of, not as a cancel. Linux: it waits for the stub to
    /// be a zombie before cancelling.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_cancel_beaten_by_a_clean_exit_is_logged_as_the_success() {
        let stub = stub("echo done >&2; exit 0");
        let repo = repo();
        let processes = processes(&repo);
        let invocation = started(&stub, &repo);
        let pid = invocation.id();
        assert!(eventually(|| {
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .unwrap_or_default()
                .contains(") Z ")
        }));
        invocation.kill_handle().kill();
        invocation
            .finish(&CancelSignal::new(), |_| {}, |_| {})
            .unwrap();
        let record = the_one_record(&processes);
        assert_eq!(record.exit, CommandExit::Code(0));
        assert!(!record.cancelled, "{record:?}");
    }

    /// An invocation run in no repository is booked nowhere: there is no log
    /// it belongs to. The repository's log is untouched by it.
    #[test]
    fn an_invocation_in_no_repository_is_not_logged_in_one() {
        let stub = stub("exit 0");
        let repo = repo();
        let processes = processes(&repo);
        discover_retrying(stub.environment())
            .unwrap()
            .read_invocation()
            .arg("stub")
            .start()
            .unwrap()
            .finish(&CancelSignal::new(), |_| {}, |_| {})
            .unwrap();
        assert!(processes.log().is_empty());
        assert_eq!(processes.running(), 0);
    }

    /// R6.2 across handles: every worker handle on a repository books into the
    /// one registry and log the shared repository answers for.
    #[test]
    fn every_handle_on_a_repository_shares_its_log() {
        let stub = stub("exit 0");
        let shared = SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let git = discover_retrying(stub.environment()).unwrap();
        for _ in 0..2 {
            let worker = shared.to_worker();
            git.read_invocation()
                .in_repository(&worker)
                .arg("stub")
                .start()
                .unwrap()
                .finish(&CancelSignal::new(), |_| {}, |_| {})
                .unwrap();
        }
        assert_eq!(shared.command_log().len(), 2);
    }

    /// R6.3, the engine half: ending every invocation reaches those being
    /// driven and those dropped to a reaper, and returns once they are all
    /// reaped, well inside the bound, with each recorded once as cancelled.
    /// Caught by: ending only what is driven, waiting the whole bound whatever
    /// happens, or returning before the reaps.
    #[test]
    fn ending_every_invocation_ends_them_all_and_waits_for_their_reaps() {
        let stub = stub(HANGING);
        let shared = SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let git = discover_retrying(stub.environment()).unwrap();
        let (said, heard) = std::sync::mpsc::channel::<()>();
        let mut driving = Vec::new();
        for _ in 0..2 {
            let worker = shared.to_worker();
            let invocation = git
                .read_invocation()
                .in_repository(&worker)
                .arg("stub")
                .start()
                .unwrap();
            let said = said.clone();
            driving.push(std::thread::spawn(move || {
                invocation.finish(
                    &CancelSignal::new(),
                    |_| {},
                    |_| {
                        let _ = said.send(());
                    },
                )
            }));
        }
        let worker = shared.to_worker();
        drop(
            git.read_invocation()
                .in_repository(&worker)
                .arg("stub")
                .start()
                .unwrap(),
        );
        for _ in 0..2 {
            heard.recv_timeout(DEADLINE).unwrap();
        }

        let closing = Instant::now();
        let left = shared.end_invocations(CLOSE_BOUND);
        let took = closing.elapsed();
        assert_eq!(left, 0);
        assert!(took < Duration::from_secs(2), "the close took {took:?}");
        for thread in driving {
            let outcome = thread.join().unwrap();
            assert!(
                matches!(outcome, Err(Error::GitReadCancelled { .. })),
                "{outcome:?}"
            );
        }
        let log = shared.command_log();
        assert_eq!(log.len(), 3, "{log:?}");
        assert!(log.iter().all(|record| record.cancelled), "{log:?}");
    }

    /// The wait is bounded: a git that ignores `SIGTERM` outlives a short bound,
    /// which says so, and is reaped within `CLOSE_BOUND`, which is the grace
    /// and the drain bound with room. Caught by: an unbounded wait, a bound
    /// ignored, or a `CLOSE_BOUND` shorter than the grace.
    #[test]
    fn ending_every_invocation_waits_no_longer_than_its_bound() {
        let stub = stub(IGNORING_TERM);
        let shared = SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let worker = shared.to_worker();
        let invocation = discover_retrying(stub.environment())
            .unwrap()
            .read_invocation()
            .in_repository(&worker)
            .arg("stub")
            .start()
            .unwrap();
        let (said, heard) = std::sync::mpsc::channel::<()>();
        let driving = std::thread::spawn(move || {
            invocation.finish(
                &CancelSignal::new(),
                |_| {},
                |_| {
                    let _ = said.send(());
                },
            )
        });
        heard.recv_timeout(DEADLINE).unwrap();

        let closing = Instant::now();
        assert_eq!(shared.end_invocations(Duration::from_millis(100)), 1);
        let took = closing.elapsed();
        assert!(
            took >= Duration::from_millis(100) && took < Duration::from_secs(1),
            "a 100 ms bound waited {took:?}"
        );
        assert_eq!(shared.end_invocations(CLOSE_BOUND), 0);
        assert!(matches!(
            driving.join().unwrap(),
            Err(Error::GitReadCancelled { .. })
        ));
    }

    /// Once ending has begun, an invocation that starts afterwards is ended as
    /// soon as it starts rather than left to run in a closing repository.
    #[test]
    fn an_invocation_started_after_the_end_is_ended_at_once() {
        let stub = stub(HANGING);
        let shared = SharedRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        assert_eq!(shared.end_invocations(Duration::ZERO), 0);
        let worker = shared.to_worker();
        let started = Instant::now();
        let outcome = discover_retrying(stub.environment())
            .unwrap()
            .read_invocation()
            .in_repository(&worker)
            .arg("stub")
            .start()
            .unwrap()
            .finish(&CancelSignal::new(), |_| {}, |_| {});
        assert!(
            matches!(outcome, Err(Error::GitReadCancelled { .. })),
            "{outcome:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    /// The bounds this phase fixed, as `state.md` records them.
    #[test]
    fn the_log_and_close_bounds_have_the_values_the_packet_recorded() {
        assert_eq!(LOG_ENTRIES, 1000);
        assert_eq!(LOG_BYTES, 4 * 1024 * 1024);
        assert_eq!(CLOSE_BOUND, Duration::from_secs(3));
    }
}
