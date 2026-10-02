//! A running invocation's process group: when it may be signalled, how it is
//! ended, and the threads that watch it.
//!
//! Every invocation's `git` is started as the leader of a new process group
//! (`CommandExt::process_group(0)`), so its group id is its pid and the group
//! holds everything it started — `ssh`, the remote helper, credential helpers,
//! hooks, filter drivers. Ending an invocation is `SIGTERM` to that group, then
//! `SIGKILL` to it once [`TERMINATION_GRACE`] has passed: git removes its lock
//! files on `SIGTERM` and cannot on `SIGKILL`, and a signal to `git` alone
//! would leave its children holding the pipes the runner reads.
//!
//! # When the group may be signalled
//!
//! Only while a member is believed alive: the leader not yet reaped, or one of
//! the pipes the runner reads still open — a pipe is held by the process it
//! was handed to, and every process git starts is in its group. A group id is
//! not reused while any member lives (POSIX XBD 4.17; Linux `kernel/pid.c`),
//! so that rule closes all but a narrow race, stated rather than claimed
//! closed: an open pipe does not prove its holder is still IN the group (it
//! may have left it, or on macOS been handed the pipe by another thread's
//! spawn), and the count lags the pipe — a reader still handing on its last
//! read counts its pipe open after the writer closed it — and if every member
//! exits between the check and the signal the id can in principle be reused. Closing that needs process handles std does not
//! offer stably. The check and the signal are made under one lock, which the
//! reap also takes, so nothing signals a pid this module has reaped unless an
//! open pipe still names the group — and nothing signals at all once the
//! invocation is over.
//!
//! # Who waits
//!
//! Nobody, under the lock. The lock covers the [`Child`] and the progress of
//! an end; every hold of it is a `try_wait`, a `killpg` or a field write, never
//! a block. The kill handle only TRIES for it (it runs on the UI thread): a
//! miss is not a lost cancel, because the request is recorded first and the
//! thread driving the invocation sends the signal itself on its next tick.

use std::io;
use std::process::{Child, ExitStatus};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;

/// How long a cancelled git gets to act on `SIGTERM` before `SIGKILL`. git's
/// handler removes its temporary and lock files and exits at once, in
/// milliseconds; two seconds is that on a loaded machine with room to spare,
/// and short enough that "cancelled" still arrives while the user is looking.
pub(crate) const TERMINATION_GRACE: Duration = Duration::from_secs(2);

/// Shared by the thread driving an invocation, its pipe threads and every
/// [`KillHandle`].
#[derive(Debug)]
pub(super) struct Group {
    leader: Mutex<Leader>,
    /// An end was asked for — by a kill handle, a drop, the query's cancel
    /// signal, a stdout ceiling or a failed stdin write. Recorded before any
    /// signal is tried, so a handle that misses the lock loses nothing.
    ending: AtomicBool,
    /// The stdout and stderr readers that have not yet seen the end of their
    /// pipe: while one is open, something in the group may still be alive.
    open_pipes: AtomicUsize,
    /// This invocation's threads still running, so a test can see each end.
    threads: AtomicUsize,
    /// Why feeding stdin failed, when it did for a reason other than git
    /// closing it.
    input_error: Mutex<Option<io::Error>>,
}

/// The leader and how far an end has got with it. One lock covers both.
#[derive(Debug)]
pub(super) struct Leader {
    child: Child,
    /// The leader's pid, which is its group's id; `None` only for a pid that
    /// does not fit a `pid_t`, impossible where git runs and handled anyway.
    group: Option<Pid>,
    status: Option<ExitStatus>,
    /// `try_wait` itself failed: the leader is gone from this process's view.
    lost: Option<io::Error>,
    /// A signal went out while the leader had not exited: anything it reports
    /// afterwards may be the signal's doing (R4.5).
    signalled_while_running: bool,
    terminated_at: Option<Instant>,
    killed_at: Option<Instant>,
    /// The invocation is over; nothing is signalled after this.
    over: bool,
}

impl Group {
    pub(super) fn new(child: Child) -> Self {
        let group = i32::try_from(child.id()).ok().map(Pid::from_raw);
        Self {
            leader: Mutex::new(Leader {
                child,
                group,
                status: None,
                lost: None,
                signalled_while_running: false,
                terminated_at: None,
                killed_at: None,
                over: false,
            }),
            ending: AtomicBool::new(false),
            open_pipes: AtomicUsize::new(0),
            threads: AtomicUsize::new(0),
            input_error: Mutex::new(None),
        }
    }

    /// The leader, for the thread driving the invocation. Held for a check or
    /// a signal, never across a block.
    pub(super) fn leader(&self) -> MutexGuard<'_, Leader> {
        self.leader.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The count of open pipes, read by [`Leader`]'s signalling after its reap.
    pub(super) fn pipes(&self) -> &AtomicUsize {
        &self.open_pipes
    }

    pub(super) fn pipe_opened(&self) {
        self.open_pipes.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn pipe_closed(&self) {
        self.open_pipes.fetch_sub(1, Ordering::AcqRel);
    }

    pub(super) fn ending(&self) -> bool {
        self.ending.load(Ordering::Acquire)
    }

    pub(super) fn mark_ending(&self) {
        self.ending.store(true, Ordering::Release);
    }

    /// Asks for the end without waiting for anything: records it, then sends
    /// `SIGTERM` if the lock is free. When it is not, the driving thread holds
    /// it for a moment and sends the signal itself on seeing the request.
    pub(super) fn request_end(&self) {
        self.mark_ending();
        if let Ok(mut leader) = self.leader.try_lock() {
            leader.terminate(&self.open_pipes);
        }
    }

    /// Ends the group with `SIGKILL` now, for a drop whose reaper thread could
    /// not start: no thread is left to drive a graceful end, so there is no
    /// grace, and the leader is reaped if it already can be — one that is not
    /// is a zombie until Cairn exits. A `SIGKILL` strands a write's locks, and
    /// nothing is left to report them; this is the path for a system with no
    /// threads to give. It takes the lock rather than trying for it, because
    /// no other thread would send the signal if it missed: every hold of the
    /// lock is a non-blocking `try_wait`, `killpg` or field write, so the wait
    /// is bounded by a few system calls.
    /// Hands back the leader's exit status if the reap found one.
    pub(super) fn kill_now(&self) -> Option<ExitStatus> {
        self.mark_ending();
        let mut leader = self.leader();
        leader.terminated_at.get_or_insert_with(Instant::now);
        leader.signal(Signal::SIGKILL, &self.open_pipes);
        leader.killed_at = Some(Instant::now());
        leader.reap();
        leader.status
    }

    /// A stdin write that failed for a reason other than git closing its end:
    /// git would read what it got as the whole of its input, so the process is
    /// ended BEFORE the pipe closes (the caller drops it after this returns).
    /// Runs on the stdin thread, which may wait for the lock.
    pub(super) fn input_failed(&self, error: io::Error) {
        *self
            .input_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(error);
        self.mark_ending();
        self.leader().terminate(&self.open_pipes);
    }

    pub(super) fn take_input_error(&self) -> Option<io::Error> {
        self.input_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Starts `body` on a thread of this invocation, counted while it runs.
    pub(super) fn spawn(
        self: &Arc<Self>,
        name: &str,
        body: impl FnOnce() + Send + 'static,
        spawner: &Spawner,
    ) -> io::Result<()> {
        self.threads.fetch_add(1, Ordering::AcqRel);
        let counted = Counted(Arc::clone(self));
        let started = spawner(
            name,
            Box::new(move || {
                let _counted = counted;
                body();
            }),
        );
        // A thread that never started dropped its closure unrun, and with it the
        // count it carried: `Counted`'s drop has already taken it back.
        started
    }

    /// How many of this invocation's threads are still running.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the runner's tests alone")
    )]
    pub(super) fn live_threads(&self) -> usize {
        self.threads.load(Ordering::Acquire)
    }
}

/// Holds one count in [`Group::threads`] and gives it back when dropped —
/// when the thread's body returns or unwinds, or when a thread that could
/// not start drops the closure carrying it.
struct Counted(Arc<Group>);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.threads.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Starts a named thread and lets it run detached. A parameter rather than a
/// call so a test can make a thread fail to start.
pub(super) type Spawner = dyn Fn(&str, Box<dyn FnOnce() + Send>) -> io::Result<()>;

/// A [`Spawner`] that can be kept: what a driver holds to start its reaper.
pub(super) type ThreadStarter = fn(&str, Box<dyn FnOnce() + Send>) -> io::Result<()>;

/// The real [`Spawner`].
pub(super) fn os_thread(name: &str, body: Box<dyn FnOnce() + Send>) -> io::Result<()> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(body)
        .map(drop)
}

impl Leader {
    /// Reaps the leader if it has exited. Never blocks.
    pub(super) fn reap(&mut self) {
        if self.status.is_some() || self.lost.is_some() {
            return;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => self.status = Some(status),
            Ok(None) => {}
            Err(error) => self.lost = Some(error),
        }
    }

    pub(super) fn exited(&self) -> bool {
        self.status.is_some() || self.lost.is_some()
    }

    fn believed_alive(&self, open_pipes: usize) -> bool {
        !self.over && (!self.exited() || open_pipes > 0)
    }

    /// Sends `signal` to the group if a member is believed alive, reaping first
    /// so an exit that has already happened is seen as one, and reading the
    /// open pipes only after that reap, so the count it decides on is never
    /// older than the reap. Whether it went out.
    ///
    /// One more window is stated rather than closed: git may exit 0 between
    /// the reap's `try_wait` and the `killpg`, and is then counted as signalled
    /// while running — a completed invocation reported as cancelled. The
    /// window is microseconds and the error is on the cautious side.
    fn signal(&mut self, signal: Signal, open_pipes: &AtomicUsize) -> bool {
        self.reap();
        let open_pipes = open_pipes.load(Ordering::Acquire);
        if !self.believed_alive(open_pipes) {
            return false;
        }
        let running = !self.exited();
        match self.group {
            Some(group) => {
                // ESRCH: the group emptied since the check; nothing to end.
                let _ = killpg(group, signal);
            }
            None if running => {
                let _ = self.child.kill();
            }
            None => {}
        }
        if running {
            self.signalled_while_running = true;
        }
        true
    }

    /// `SIGTERM` to the group, once.
    pub(super) fn terminate(&mut self, open_pipes: &AtomicUsize) {
        if self.terminated_at.is_none() && self.signal(Signal::SIGTERM, open_pipes) {
            self.terminated_at = Some(Instant::now());
        }
    }

    /// `SIGKILL` to the group once [`TERMINATION_GRACE`] has passed since the
    /// `SIGTERM`, once.
    ///
    /// When the leader was reaped before then and only an open pipe keeps the
    /// group believed alive, this widens the stated reuse race: the group's
    /// id has been free since the reap, up to the whole grace, unless a member
    /// still in the group holds it. A holder that left the group cannot be
    /// reached by either signal, and if every member of the group exited, the
    /// id could in principle be reused in those two seconds.
    pub(super) fn escalate(&mut self, open_pipes: &AtomicUsize) {
        if let Some(at) = self.terminated_at
            && self.killed_at.is_none()
            && at.elapsed() >= TERMINATION_GRACE
        {
            self.signal(Signal::SIGKILL, open_pipes);
            self.killed_at = Some(Instant::now());
        }
    }

    pub(super) fn killed_at(&self) -> Option<Instant> {
        self.killed_at
    }

    /// Ends the invocation's claim on the group: nothing signals after this,
    /// whoever asks. Hands back how the leader ended and whether a signal
    /// went out while it was running.
    pub(super) fn conclude(&mut self) -> (io::Result<ExitStatus>, bool) {
        self.over = true;
        let status = match (self.status, self.lost.take()) {
            (Some(status), _) => Ok(status),
            (None, Some(error)) => Err(error),
            (None, None) => Err(io::Error::other("the process had not exited")),
        };
        (status, self.signalled_while_running)
    }

    /// Whether a `SIGTERM` has gone out, for a test.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the runner's tests alone")
    )]
    pub(super) fn terminated(&self) -> bool {
        self.terminated_at.is_some()
    }

    /// The leader's pid, for a test that reads the process table.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the runner's tests alone")
    )]
    pub(super) fn id(&self) -> u32 {
        self.child.id()
    }
}

/// Ends an invocation from any thread, without waiting: `SIGTERM` to the
/// group now if the lock is free, and the rest — the signal when it was not,
/// `SIGKILL` after the grace, the reap — by the thread driving it. `Send`,
/// `Clone`, and never blocks the thread using it, so the UI thread's cancel
/// button can hold one. After the invocation is over it does nothing.
#[derive(Debug, Clone)]
pub(crate) struct KillHandle(pub(super) Arc<Group>);

impl KillHandle {
    pub(crate) fn kill(&self) {
        self.0.request_end();
    }
}
