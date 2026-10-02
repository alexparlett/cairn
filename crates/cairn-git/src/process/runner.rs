//! Driving one started `git` from spawn to reap: its pipes, its cancellation
//! and its outcome.
//!
//! [`super::cli::GitCommand::start`] spawns the process as the leader of a new
//! group and hands the [`Child`] here, where each pipe gets its own thread
//! (`pipes.rs`) and the caller gets an [`Invocation`]. The caller then drives
//! it on its own thread with [`Invocation::finish`], [`Invocation::records`] or
//! [`Invocation::collect`], or drops it, which ends it on a reaper thread.
//!
//! # When it is over
//!
//! When the leader has exited and its pipes are drained — or, if something
//! else still holds a pipe, [`DRAIN_BOUND`] after the exit. The exit is
//! noticed when the pipes close, which is when git exits in the common case,
//! so finishing costs no tick; the thread then reaps with `try_wait`, backing
//! off from microseconds, since the kernel makes the process a zombie a moment
//! after it closes its files. When something else holds the pipes — a process
//! git started, detached or slow to die — the exit is noticed on the tick,
//! and the holder is not waited on beyond the bound.
//!
//! # Cancelling
//!
//! Three ways, ending the process the same way (`group.rs`): the cancel signal
//! the caller passes, polled every tick; a [`KillHandle`], from any thread; and
//! dropping the invocation. A cancelled invocation waits for its group — the
//! leader reaped and its pipes closed, or the pipes abandoned [`DRAIN_BOUND`]
//! after `SIGKILL` when their holder left the group — so a cancel returns
//! within the grace plus a tick, with nothing it started left running.
//!
//! # Outcomes
//!
//! A cancel that loses the race to a clean exit is the success it was: the
//! leader exited 0 before any signal reached it. Anything else that was asked
//! to end reports cancellation, whatever its status, because a status after a
//! signal may be the signal's doing. What cancellation means depends on the
//! kind: a read reports it and nothing else, a write lists the lock files
//! present under its git directory AFTER the reap, when git has removed what
//! it was going to. A failure carries the arguments, the status and the
//! retained stderr, and a failed write the lock files present.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "fetch and the version probe move onto this runner next, and reads/ gets its \
                  first caller with diff-engine"
    )
)]

use std::process::Child;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError, sync_channel};
use std::time::{Duration, Instant};

use super::cli::{GitDirs, Kind, Output};
use super::group::{Group, KillHandle, Spawner};
use super::pipes::{self, EVENTS_BOUND, Event, Records, Tail};
use crate::{Cancel, Error};

/// How long an invocation waits for its pipes after its leader has exited,
/// when something else still holds them: 250 ms from the exit.
///
/// What it bounds is only ever that case — git exiting closes its pipes, and
/// the common case ends on that. What is still owed then is output git wrote
/// before it exited, already in the pipe, which the reader threads move in
/// well under a millisecond; 250 ms is that on a loaded machine with two
/// orders of margin, so no answer is cut short by a slow scheduler. Anything
/// arriving later comes from a process git left behind, which the invocation
/// is not waiting for: a quarter of a second is short enough that a finished
/// operation still reads as finished when it ends.
pub(crate) const DRAIN_BOUND: Duration = Duration::from_millis(250);

/// How often the driving thread looks at the cancel signal and the leader
/// while nothing arrives on the pipes.
pub(crate) const TICK: Duration = Duration::from_millis(20);

/// The first wait between `try_wait`s once the pipes have closed; doubled up
/// to [`TICK`]. The leader is normally a zombie microseconds after its pipes
/// close, so the first look or two finds it.
const FIRST_BACKOFF: Duration = Duration::from_micros(50);

/// The most queued events taken in one go before the cancel signal and the
/// leader are looked at again.
const BATCH: usize = EVENTS_BOUND;

/// A started `git` of the kind `K`. Drive it with [`Invocation::finish`],
/// [`Invocation::records`] or [`Invocation::collect`]; dropped unfinished, it
/// is ended and reaped on a reaper thread, and the drop does not block.
#[derive(Debug)]
pub(crate) struct Invocation<K: Kind> {
    group: Arc<Group>,
    driver: Option<Driver>,
    kind: K,
    arguments: String,
    dirs: Option<GitDirs>,
}

/// What a caller's stdout sink tells the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    /// The caller's ceiling was crossed: end the process and report that.
    Stop,
}

/// The receiving end of an invocation's pipes and its group, which together
/// are what drives it to its end — on the caller's thread, or the reaper's.
#[derive(Debug)]
struct Driver {
    group: Arc<Group>,
    events: Receiver<Event>,
}

/// How a driven invocation ended, before the kind gives it a meaning.
struct Ended {
    status: std::io::Result<std::process::ExitStatus>,
    signalled_while_running: bool,
    asked_to_end: bool,
    stopped: bool,
    tail: String,
}

/// Starts the pipe threads for `child` and hands back the invocation. A thread
/// that cannot start is an error that ends and reaps the process first, on
/// this thread: never a running git with nobody waiting on it.
pub(super) fn watch<K: Kind>(
    mut child: Child,
    input: Option<Vec<u8>>,
    kind: K,
    arguments: String,
    dirs: Option<GitDirs>,
    spawner: &Spawner,
) -> Result<Invocation<K>, Error> {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdin = child.stdin.take();
    let group = Arc::new(Group::new(child));
    let (sender, events) = sync_channel(EVENTS_BOUND);
    let mut invocation = Invocation {
        group: Arc::clone(&group),
        driver: Some(Driver {
            group: Arc::clone(&group),
            events,
        }),
        kind,
        arguments,
        dirs,
    };

    let started = (|| -> std::io::Result<()> {
        let missing = |pipe| std::io::Error::other(format!("git was started without {pipe}"));
        // stdin first: a writer that cannot start leaves git an empty input, never a
        // partial one, and it is ended below before it can act on more than that.
        if let Some(input) = input {
            let stdin = stdin.ok_or_else(|| missing("a stdin pipe"))?;
            let fed = Arc::clone(&group);
            group.spawn(
                "cairn-git-stdin",
                move || pipes::feed(stdin, &input, &fed),
                spawner,
            )?;
        }
        let stdout = stdout.ok_or_else(|| missing("a stdout pipe"))?;
        let stderr = stderr.ok_or_else(|| missing("a stderr pipe"))?;
        for (name, reader) in [
            (
                "cairn-git-stdout",
                Reader::Stdout(stdout, sender.clone(), Arc::clone(&group)),
            ),
            (
                "cairn-git-stderr",
                Reader::Stderr(stderr, sender.clone(), Arc::clone(&group)),
            ),
        ] {
            group.pipe_opened();
            if let Err(error) = group.spawn(name, move || reader.run(), spawner) {
                group.pipe_closed();
                return Err(error);
            }
        }
        Ok(())
    })();
    drop(sender);

    match started {
        Ok(()) => Ok(invocation),
        Err(source) => {
            // Ended here and now rather than on a reaper thread: threads are what failed.
            if let Some(driver) = invocation.driver.take() {
                driver.group.mark_ending();
                driver.run(&|| true, &mut |_| Flow::Continue, &mut |_| {});
            }
            Err(Error::GitUnwatched {
                arguments: std::mem::take(&mut invocation.arguments),
                source,
            })
        }
    }
}

/// A pipe reader and what it needs, so both run through one spawn.
enum Reader {
    Stdout(
        std::process::ChildStdout,
        std::sync::mpsc::SyncSender<Event>,
        Arc<Group>,
    ),
    Stderr(
        std::process::ChildStderr,
        std::sync::mpsc::SyncSender<Event>,
        Arc<Group>,
    ),
}

impl Reader {
    fn run(self) {
        match self {
            Self::Stdout(pipe, events, group) => pipes::read_stdout(pipe, &events, &group),
            Self::Stderr(pipe, events, group) => pipes::read_stderr(pipe, &events, &group),
        }
    }
}

impl<K: Kind> Invocation<K> {
    /// A handle that ends this invocation from any thread, without blocking it.
    pub(crate) fn kill_handle(&self) -> KillHandle {
        KillHandle(Arc::clone(&self.group))
    }

    /// Waits for the end, handing stdout to `stdout` as it arrives and each
    /// non-blank stderr line to `progress`; `cancel` is polled every tick. The
    /// `Output` carries the retained stderr and no stdout — it went to `stdout`.
    pub(crate) fn finish(
        self,
        cancel: &impl Cancel,
        mut stdout: impl FnMut(&[u8]),
        mut progress: impl FnMut(&str),
    ) -> Result<Output, Error> {
        self.drive(
            cancel,
            &mut |chunk| {
                stdout(chunk);
                Flow::Continue
            },
            &mut progress,
            None,
        )
        .map(|stderr| Output::new(Vec::new(), stderr))
    }

    /// As [`Invocation::finish`], with stdout split into the NUL-terminated
    /// records of a `-z` format, each handed to `record` as soon as it is whole.
    /// A last record without its NUL is handed on only if the invocation
    /// succeeded, since a cancelled one may have been cut off inside it.
    pub(crate) fn records(
        self,
        cancel: &impl Cancel,
        mut record: impl FnMut(&[u8]),
        mut progress: impl FnMut(&str),
    ) -> Result<Output, Error> {
        let mut splitter = Records::default();
        let outcome = self.drive(
            cancel,
            &mut |chunk| {
                splitter.push(chunk, &mut record);
                Flow::Continue
            },
            &mut progress,
            None,
        );
        if outcome.is_ok() {
            splitter.finish(&mut record);
        }
        outcome.map(|stderr| Output::new(Vec::new(), stderr))
    }

    /// Waits for the end and hands back the whole of stdout — or
    /// [`Error::GitOutputTooLarge`] if git writes more than `ceiling` bytes, in
    /// which case it is ended at once and nothing it wrote is returned.
    pub(crate) fn collect(
        self,
        cancel: &impl Cancel,
        ceiling: usize,
        mut progress: impl FnMut(&str),
    ) -> Result<Output, Error> {
        let mut collected = Vec::new();
        let stderr = self.drive(
            cancel,
            &mut |chunk| {
                if collected.len() + chunk.len() > ceiling {
                    return Flow::Stop;
                }
                collected.extend_from_slice(chunk);
                Flow::Continue
            },
            &mut progress,
            Some(ceiling),
        )?;
        Ok(Output::new(collected, stderr))
    }

    /// The leader's pid, for a test that reads the process table.
    #[cfg(test)]
    pub(crate) fn id(&self) -> u32 {
        self.group.leader().id()
    }

    /// The group, for a test that watches its threads and signals.
    #[cfg(test)]
    fn group(&self) -> Arc<Group> {
        Arc::clone(&self.group)
    }

    fn drive(
        mut self,
        cancel: &impl Cancel,
        stdout: &mut dyn FnMut(&[u8]) -> Flow,
        progress: &mut dyn FnMut(&str),
        ceiling: Option<usize>,
    ) -> Result<String, Error> {
        let arguments = std::mem::take(&mut self.arguments);
        let Some(driver) = self.driver.take() else {
            return Err(Error::GitUnwatched {
                arguments,
                source: std::io::Error::other("the invocation was already driven"),
            });
        };
        let ended = driver.run(&|| cancel.is_cancelled(), stdout, progress);
        if ended.stopped
            && let Some(ceiling) = ceiling
        {
            return Err(Error::GitOutputTooLarge { arguments, ceiling });
        }
        let status = match ended.status {
            Ok(status) => status,
            Err(source) => return Err(Error::GitUnwatched { arguments, source }),
        };
        // A cancel that lost the race to a clean exit is that exit's success (R4.5).
        let beaten_by_a_clean_exit = status.success() && !ended.signalled_while_running;
        if ended.asked_to_end && !beaten_by_a_clean_exit {
            // After the reap: what git removed on its way out is not reported.
            return Err(self.kind.cancelled(arguments, self.dirs.as_ref()));
        }
        if !status.success() {
            return Err(Error::GitFailed {
                arguments,
                status,
                stderr: ended.tail,
                present_locks: self.kind.present_locks(self.dirs.as_ref()),
            });
        }
        Ok(ended.tail)
    }
}

impl<K: Kind> Drop for Invocation<K> {
    /// Ends an invocation dropped before it finished: the end is asked for at
    /// once (`SIGTERM` if the lock is free) and driven to the reap on a thread
    /// of its own, so this returns without waiting. If even that thread cannot
    /// start, the group is sent `SIGKILL` now and reaped if it already can be;
    /// a leader that outlives that is a zombie until Cairn exits, the one
    /// thing left when the system has no threads to give.
    fn drop(&mut self) {
        let Some(driver) = self.driver.take() else {
            return;
        };
        let group = Arc::clone(&driver.group);
        group.request_end();
        let reaped = group.spawn(
            "cairn-git-reaper",
            move || {
                driver.run(&|| true, &mut |_| Flow::Continue, &mut |_| {});
            },
            &super::group::os_thread,
        );
        if reaped.is_err() {
            group.kill_now();
        }
    }
}

impl Driver {
    /// Drives the invocation to its end on this thread. See the module docs for
    /// when that is.
    fn run(
        self,
        cancel: &dyn Fn() -> bool,
        stdout: &mut dyn FnMut(&[u8]) -> Flow,
        progress: &mut dyn FnMut(&str),
    ) -> Ended {
        let Driver { group, events } = self;
        let mut tail = Tail::default();
        let mut stopped = false;
        let mut pipes_closed = false;
        let mut exit_noticed: Option<Instant> = None;
        let mut backoff = FIRST_BACKOFF;
        // The tick is part of the bound: the exit is noticed at most a tick late.
        let drain = DRAIN_BOUND.saturating_sub(TICK);

        let mut handle = |event: Event, stopped: &mut bool| match event {
            Event::Stdout(chunk) => {
                if !*stopped && stdout(&chunk) == Flow::Stop {
                    *stopped = true;
                    group.mark_ending();
                }
            }
            Event::Line(line) => {
                let text = line.trim_end();
                if !text.is_empty() {
                    progress(text);
                }
                tail.push(&line);
            }
        };

        loop {
            if pipes_closed {
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(TICK);
            } else {
                let wait = exit_noticed.map_or(TICK, |at| {
                    drain
                        .saturating_sub(at.elapsed())
                        .clamp(Duration::ZERO, TICK)
                });
                match events.recv_timeout(wait) {
                    Ok(event) => {
                        handle(event, &mut stopped);
                        for _ in 0..BATCH {
                            match events.try_recv() {
                                Ok(event) => handle(event, &mut stopped),
                                Err(TryRecvError::Empty) => break,
                                Err(TryRecvError::Disconnected) => {
                                    pipes_closed = true;
                                    break;
                                }
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => pipes_closed = true,
                }
            }

            if cancel() {
                group.mark_ending();
            }
            let ending = group.ending();
            let open = group.open_pipes();
            let mut leader = group.leader();
            if ending {
                leader.terminate(open);
                leader.escalate(open);
            }
            leader.reap();
            if !leader.exited() {
                continue;
            }
            if pipes_closed {
                break;
            }
            let abandon = if ending {
                // Ended: the group was signalled while its pipes were open; what still
                // holds them after SIGKILL left the group, and gets the drain bound.
                leader
                    .killed_at()
                    .is_some_and(|at| at.elapsed() >= DRAIN_BOUND)
            } else {
                exit_noticed.get_or_insert_with(Instant::now).elapsed() >= drain
            };
            if abandon {
                drop(leader);
                // What is already queued is the leader's, written before it exited.
                for _ in 0..EVENTS_BOUND {
                    match events.try_recv() {
                        Ok(event) => handle(event, &mut stopped),
                        Err(_) => break,
                    }
                }
                break;
            }
        }

        // Read under the lock that concludes: a kill after this signals nothing and
        // counts for nothing, since the invocation was over before it.
        let (status, signalled_while_running, asked_to_end) = {
            let mut leader = group.leader();
            let asked_to_end = group.ending();
            let (status, signalled) = leader.conclude();
            (status, signalled, asked_to_end)
        };
        // Dropping the receiver ends a reader still on a pipe at its next send.
        drop(events);
        let input = group.take_input_error();
        Ended {
            status: match input {
                Some(error) => Err(error),
                None => status,
            },
            signalled_while_running,
            asked_to_end,
            stopped,
            tail: tail.into_text(),
        }
    }
}

/// Against a stub `git` that does what each test needs, and real `git` where a
/// criterion is about git itself. Linux reads the process table to see that
/// nothing an invocation started is left alive or unreaped.
#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use super::super::cli::{Read, TERMINATION_GRACE};
    use super::super::group::os_thread;
    use super::super::pipes;
    use super::super::stub_git::{StubGit, discover_retrying};
    use super::{DRAIN_BOUND, Invocation, TICK};
    use crate::ops::{Askpass, GitBinary};
    use crate::{CancelSignal, Error};

    /// Answers `--version`, then runs `rest` for anything else.
    fn stub(rest: &str) -> StubGit {
        StubGit::with_git(&format!(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n{rest}"
        ))
    }

    fn started(stub: &StubGit) -> Invocation<Read> {
        discover_retrying(stub.environment())
            .unwrap()
            .read_invocation()
            .arg("stub")
            .start()
            .unwrap()
    }

    fn never() -> CancelSignal {
        CancelSignal::new()
    }

    /// A stub that hangs in a background `sleep` — a grandchild holding stdout and
    /// stderr, as `ssh` or a hook would — after saying so. A system without `sleep`
    /// exits 99 before saying anything, so no cancel fires on a stub that died alone.
    const HANGING_WITH_A_GRANDCHILD: &str = "PATH=/usr/bin:/bin; \
        command -v sleep >/dev/null || exit 99; \
        sleep 30 & echo hanging >&2; wait";

    /// On `SIGTERM`: says so, ends its grandchild (quietly: the group signal may
    /// have ended it first), and exits as git does after removing its locks. The
    /// trap runs at once because the shell is in `wait`.
    const ENDING_ON_TERM: &str = "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
        sleep 30 & child=$!; \
        trap 'echo terminated >&2; kill $child 2>/dev/null; exit 143' TERM; \
        echo hanging >&2; wait $child";

    /// Ignores `SIGTERM` and hangs in one-second sleeps, so only `SIGKILL` ends it.
    const IGNORING_TERM: &str = "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
        trap '' TERM; echo hanging >&2; while :; do sleep 1; done";

    /// How long a test waits for something that should take at most the grace and
    /// a tick, before calling it hung: margin for a loaded machine.
    const DEADLINE: Duration = Duration::from_secs(10);

    /// Runs `body` on a thread and fails the test if it is not done by `deadline`,
    /// so a runner that deadlocks fails here instead of hanging the suite.
    fn within<T: Send + 'static>(
        deadline: Duration,
        body: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(body());
        });
        finished
            .recv_timeout(deadline)
            .unwrap_or_else(|_| panic!("not done within {deadline:?}: the runner hung"))
    }

    /// Polls `condition` until it holds or `deadline` passes.
    fn eventually(deadline: Duration, mut condition: impl FnMut() -> bool) -> bool {
        let started = Instant::now();
        loop {
            if condition() {
                return true;
            }
            if started.elapsed() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Every process in group `group`, zombies included: what is alive or unreaped.
    #[cfg(target_os = "linux")]
    fn group_members(group: u32) -> Vec<u32> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir("/proc").unwrap().flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };
            // After the command's closing parenthesis: state, ppid, pgrp.
            let Some((_, rest)) = stat.rsplit_once(')') else {
                continue;
            };
            if rest.split_whitespace().nth(2).and_then(|f| f.parse().ok()) == Some(group) {
                found.push(pid);
            }
        }
        found
    }

    #[cfg(target_os = "linux")]
    fn reaped(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    }

    /// The whole group gone: the leader reaped and no member alive or a zombie.
    #[cfg(target_os = "linux")]
    fn group_gone(pid: u32) -> bool {
        reaped(pid) && group_members(pid).is_empty()
    }

    /// Ends a group a test left running on purpose.
    #[cfg(target_os = "linux")]
    fn end_group(pid: u32) {
        let pid = i32::try_from(pid).unwrap();
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        );
    }

    /// R3.1: the leader of a new group. Caught by: dropping `process_group(0)`,
    /// after which the stub is in this test process's group and every group
    /// signal below would go nowhere — or, worse, here.
    #[cfg(target_os = "linux")]
    #[test]
    fn every_invocation_leads_a_process_group_of_its_own() {
        let stub = stub(HANGING_WITH_A_GRANDCHILD);
        let invocation = started(&stub);
        let pid = invocation.id();
        assert!(
            eventually(DEADLINE, || group_members(pid).len() >= 2),
            "the stub's group {pid} never held the stub and its sleep: {:?}",
            group_members(pid)
        );
        assert!(!group_members(pid).contains(&std::process::id()));
        drop(invocation);
        assert!(eventually(DEADLINE, || group_gone(pid)));
    }

    /// G6, the stub half: over 5 MiB of `-z` records, with stderr written all the
    /// while by another process in the group, arrive whole and in order, and every
    /// stderr line reaches the progress callback. Caught by: reading the two pipes
    /// on one thread (the stub blocks on whichever is full, and `within` fails), a
    /// splitter that loses a record across a chunk boundary, or a dropped chunk.
    #[test]
    fn five_mib_of_records_beside_continuous_stderr_arrive_whole_and_in_order() {
        const RECORDS: usize = 400_000;
        const LINES: usize = 3_000;
        let (records, lines) = within(DEADLINE * 3, || {
            let stub = stub(&format!(
                "PATH=/usr/bin:/bin; command -v seq >/dev/null || exit 99; \
                 (i=0; while [ $i -lt {LINES} ]; do echo \"progress $i\" >&2; i=$((i+1)); done) & \
                 seq -f 'record-%07g' 1 {RECORDS} | tr '\\n' '\\0'; wait"
            ));
            let mut records = Vec::with_capacity(RECORDS);
            let mut lines = Vec::new();
            started(&stub)
                .records(
                    &never(),
                    |record| records.push(String::from_utf8_lossy(record).into_owned()),
                    |line| lines.push(line.to_owned()),
                )
                .unwrap();
            (records, lines)
        });
        assert_eq!(records.len(), RECORDS);
        let bytes: usize = records.iter().map(|record| record.len() + 1).sum();
        assert!(
            bytes >= 5 * 1024 * 1024,
            "only {bytes} bytes: not the volume G6 asks for"
        );
        for (n, record) in records.iter().enumerate() {
            assert_eq!(
                record,
                &format!("record-{:07}", n + 1),
                "record {n} out of order"
            );
        }
        let expected: Vec<String> = (0..LINES).map(|n| format!("progress {n}")).collect();
        assert_eq!(lines, expected);
    }

    /// G6, the ceiling: a collect told to take less than git writes fails with the
    /// ceiling's own error, which carries no output, and the process is ended
    /// rather than left writing to nobody. Caught by: truncating to the ceiling and
    /// returning success, or reporting the crossing as a cancel.
    #[test]
    fn a_collect_over_its_ceiling_is_refused_whole_and_the_process_ended() {
        let stub = stub(
            "PATH=/usr/bin:/bin; command -v seq >/dev/null || exit 99; \
             seq -f 'record-%07g' 1 400000 | tr '\\n' '\\0'",
        );
        let invocation = started(&stub);
        let pid = invocation.id();
        let outcome = within(DEADLINE, move || {
            invocation.collect(&never(), 1024 * 1024, |_| {})
        });
        match outcome {
            Err(Error::GitOutputTooLarge { arguments, ceiling }) => {
                assert_eq!(arguments, "stub");
                assert_eq!(ceiling, 1024 * 1024);
            }
            other => panic!("expected the ceiling error, got {other:?}"),
        }
        #[cfg(target_os = "linux")]
        assert!(eventually(DEADLINE, || group_gone(pid)));
        let _ = pid;

        // The same output under a ceiling it fits is collected whole.
        let output = within(DEADLINE, move || {
            started(&stub).collect(&never(), 8 * 1024 * 1024, |_| {})
        })
        .unwrap();
        assert_eq!(output.records().count(), 400_000);
    }

    /// Deterministic bytes that are not a repeating pattern a pipe could fold.
    fn noise(length: usize) -> Vec<u8> {
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 24) as u8
            })
            .collect()
    }

    /// G7, the stub half: 64 MiB written to stdin while the process writes stdout
    /// and stderr all the while completes, every byte back in order. Caught by:
    /// writing stdin on the thread that reads stdout (the stub's `cat` blocks on a
    /// full stdout, the write on a full stdin, and `within` fails), or closing
    /// stdin early.
    #[test]
    fn sixty_four_mib_of_stdin_beside_busy_stdout_and_stderr_completes() {
        let input = noise(64 * 1024 * 1024);
        let expected_length = input.len();
        let first = input[..4096].to_vec();
        let last = input[input.len() - 4096..].to_vec();
        let (output, lines) = within(DEADLINE * 6, move || {
            let stub = stub(
                "PATH=/usr/bin:/bin; command -v cat >/dev/null || exit 99; \
                 (i=0; while [ $i -lt 2000 ]; do echo \"progress $i\" >&2; i=$((i+1)); done) & \
                 cat; wait",
            );
            let mut lines = 0usize;
            let output = discover_retrying(stub.environment())
                .unwrap()
                .read_invocation()
                .arg("stub")
                .input(input)
                .start()
                .unwrap()
                .collect(&never(), 65 * 1024 * 1024, |_| lines += 1)
                .unwrap();
            (output, lines)
        });
        assert_eq!(output.stdout().len(), expected_length);
        assert_eq!(&output.stdout()[..4096], first.as_slice());
        assert_eq!(&output.stdout()[expected_length - 4096..], last.as_slice());
        assert_eq!(lines, 2000);
    }

    /// G7, end of input and id parity, with real `git`: `hash-object --stdin` over
    /// 64 MiB returns, so it saw the end of its input, and its id is git's id for
    /// those bytes — computed here by gitoxide, which shares no code with git.
    /// Caught by: stdin never closed (git waits, `within` fails) or a byte lost.
    #[test]
    fn sixty_four_mib_through_hash_object_gives_gits_own_id() {
        let input = noise(64 * 1024 * 1024);
        let expected =
            gix::objs::compute_hash(gix::hash::Kind::Sha1, gix::objs::Kind::Blob, &input)
                .unwrap()
                .to_string();
        let output = within(DEADLINE * 6, move || {
            let git =
                GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None)).unwrap();
            // Outside any repository, so the hash is SHA-1 whatever this checkout uses.
            let outside = std::env::temp_dir();
            git.read_invocation()
                .arg("-C")
                .arg(&outside)
                .args(["hash-object", "--stdin"])
                .input(input)
                .start()
                .unwrap()
                .collect(&never(), 1024, |_| {})
                .unwrap()
        });
        assert_eq!(output.stdout_text().trim(), expected);
    }

    /// Two of the three ways to cancel; the third, dropping, needs no driving.
    #[derive(Debug, Clone, Copy)]
    enum Cancelling {
        ByHandle,
        BySignal,
    }

    /// Drives `invocation`, cancelling it the moment the stub says it is hanging,
    /// and hands back the outcome, what the stub said, and how long the end took.
    fn cancel_once_hanging(
        invocation: Invocation<Read>,
        how: Cancelling,
    ) -> (Result<super::Output, Error>, Vec<String>, Duration) {
        within(DEADLINE, move || {
            let signal = CancelSignal::new();
            let handle = invocation.kill_handle();
            let mut seen = Vec::new();
            let mut at = None;
            let outcome = invocation.finish(
                &signal,
                |_| {},
                |line| {
                    seen.push(line.to_owned());
                    if line == "hanging" && at.is_none() {
                        at = Some(Instant::now());
                        match how {
                            Cancelling::ByHandle => handle.kill(),
                            Cancelling::BySignal => signal.cancel(),
                        }
                    }
                },
            );
            (outcome, seen, at.map_or(Duration::ZERO, |at| at.elapsed()))
        })
    }

    /// What every cancel of the grandchild stub must show: reported as a read's
    /// cancellation, the stub really was hanging, the end came well inside the
    /// grace (the stub dies of `SIGTERM`), and nothing of the group is left alive
    /// or unreaped.
    fn assert_ended_whole(
        outcome: &Result<super::Output, Error>,
        seen: &[String],
        took: Duration,
        pid: u32,
    ) {
        assert!(
            matches!(outcome, Err(Error::GitReadCancelled { arguments }) if arguments == "stub"),
            "{outcome:?}"
        );
        assert_eq!(
            seen,
            ["hanging"],
            "the stub was dying by itself, so the cancel decided nothing"
        );
        assert!(
            took < TERMINATION_GRACE + TICK,
            "the cancel took {took:?}: the grace plus a tick is the most it may"
        );
        #[cfg(target_os = "linux")]
        assert!(
            group_gone(pid),
            "the cancel left the group {pid} alive or unreaped: {:?}",
            group_members(pid)
        );
        let _ = pid;
    }

    /// G8, by the kill handle. Caught by: signalling the leader alone (its `sleep`
    /// lives on holding stderr, so the group is not gone and the cancel waits for
    /// the grace), not reaping, or reporting the cancel as a failure.
    #[test]
    fn a_kill_handle_ends_the_whole_group_and_reaps_it() {
        let stub = stub(HANGING_WITH_A_GRANDCHILD);
        let invocation = started(&stub);
        let pid = invocation.id();
        let (outcome, seen, took) = cancel_once_hanging(invocation, Cancelling::ByHandle);
        assert_ended_whole(&outcome, &seen, took, pid);
    }

    /// G8, by a superseded cancel signal, polled on the tick. Caught by: a driver
    /// that never polls the signal (the stub hangs for 30 s and `within` fails).
    #[test]
    fn a_superseded_cancel_signal_ends_the_whole_group_and_reaps_it() {
        let stub = stub(HANGING_WITH_A_GRANDCHILD);
        let invocation = started(&stub);
        let pid = invocation.id();
        let (outcome, seen, took) = cancel_once_hanging(invocation, Cancelling::BySignal);
        assert_ended_whole(&outcome, &seen, took, pid);
    }

    /// G8, by dropping it: the drop returns at once, even though the stub is still
    /// running, and the reaper thread ends and reaps the group within the grace
    /// plus a tick; then every thread the invocation started has ended. Caught by:
    /// a drop that waits for the process, or one that leaves it running or a zombie.
    #[cfg(target_os = "linux")]
    #[test]
    fn dropping_an_unfinished_invocation_ends_and_reaps_its_group_without_blocking() {
        let stub = stub(HANGING_WITH_A_GRANDCHILD);
        let invocation = started(&stub);
        let pid = invocation.id();
        let group = invocation.group();
        assert!(
            eventually(DEADLINE, || group_members(pid).len() >= 2),
            "the stub never started its grandchild"
        );
        let dropping = Instant::now();
        drop(invocation);
        let took = dropping.elapsed();
        assert!(
            took < Duration::from_millis(50),
            "the drop blocked for {took:?}"
        );
        assert!(
            eventually(TERMINATION_GRACE + TICK, || group_gone(pid)),
            "the dropped invocation's group {pid} outlived the grace: {:?}",
            group_members(pid)
        );
        assert!(
            eventually(DEADLINE, || group.live_threads() == 0),
            "{} of the invocation's threads outlived it",
            group.live_threads()
        );
    }

    /// The drop never waits, even for a process that will take the whole grace to
    /// end: a stub that ignores `SIGTERM` keeps the reaper busy for two seconds,
    /// and the drop has returned long before. Caught by: ending the process on the
    /// dropping thread (the drop takes the grace), which a stub that dies of
    /// `SIGTERM` at once, as in the test above, would not show.
    #[cfg(target_os = "linux")]
    #[test]
    fn dropping_never_waits_for_a_process_that_outlasts_sigterm() {
        let stub = stub(IGNORING_TERM);
        let invocation = started(&stub);
        let pid = invocation.id();
        assert!(
            eventually(DEADLINE, || group_members(pid).len() >= 2),
            "the stub never started its sleep"
        );
        let dropping = Instant::now();
        drop(invocation);
        let took = dropping.elapsed();
        assert!(
            took < Duration::from_millis(50),
            "the drop blocked for {took:?}"
        );
        assert!(
            !group_gone(pid),
            "the stub ended at once, so this decided nothing about waiting"
        );
        assert!(
            eventually(TERMINATION_GRACE + Duration::from_secs(2), || group_gone(
                pid
            )),
            "the reaper never ended the group {pid}: {:?}",
            group_members(pid)
        );
    }

    /// G9, `SIGTERM` first: a stub that traps it says so and ends well inside the
    /// grace, so it was not `SIGKILL`ed (which runs no trap). Caught by: `SIGKILL`
    /// first, or no signal to a process that is waiting rather than running.
    #[test]
    fn a_cancel_sends_sigterm_first_and_a_process_that_acts_on_it_is_not_killed() {
        let stub = stub(ENDING_ON_TERM);
        let invocation = started(&stub);
        let (outcome, seen, took) = cancel_once_hanging(invocation, Cancelling::ByHandle);
        assert!(
            matches!(outcome, Err(Error::GitReadCancelled { .. })),
            "{outcome:?}"
        );
        assert_eq!(
            seen,
            ["hanging", "terminated"],
            "the stub did not report SIGTERM"
        );
        assert!(
            took < TERMINATION_GRACE,
            "ended after {took:?}: it was not SIGTERM that ended it"
        );
    }

    /// G9, the escalation: a stub that ignores `SIGTERM` is `SIGKILL`ed once the
    /// grace has passed, and not before. Caught by: no escalation (`within` fails)
    /// or escalating early.
    #[test]
    fn a_process_that_ignores_sigterm_is_killed_after_the_grace() {
        let stub = stub(IGNORING_TERM);
        let invocation = started(&stub);
        let pid = invocation.id();
        let (outcome, seen, took) = cancel_once_hanging(invocation, Cancelling::ByHandle);
        assert!(
            matches!(outcome, Err(Error::GitReadCancelled { .. })),
            "{outcome:?}"
        );
        assert_eq!(seen, ["hanging"]);
        assert!(
            took >= TERMINATION_GRACE,
            "killed after {took:?}, inside the grace"
        );
        assert!(
            took < TERMINATION_GRACE + Duration::from_secs(2),
            "{took:?}"
        );
        #[cfg(target_os = "linux")]
        assert!(eventually(DEADLINE, || group_gone(pid)));
        let _ = pid;
    }

    /// G10: a stub that exits while its grandchild holds stdout and stderr returns
    /// within `DRAIN_BOUND` of the exit — what it wrote before exiting included —
    /// and the grandchild, which nobody cancelled, is left alone. Caught by:
    /// waiting for the pipes to close (30 s, so `within` fails), or noticing the
    /// exit only on a cancel.
    #[cfg(target_os = "linux")]
    #[test]
    fn an_exit_with_a_grandchild_holding_the_pipes_returns_within_the_drain_bound() {
        let stub = stub(
            "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
             sleep 30 & echo 'last words' >&2; printf 'answer\\0'; exit 0",
        );
        let invocation = started(&stub);
        let pid = invocation.id();
        let begun = Instant::now();
        let (output, records) = within(DEADLINE, move || {
            let mut records = Vec::new();
            let output = invocation
                .records(&never(), |record| records.push(record.to_vec()), |_| {})
                .unwrap();
            (output, records)
        });
        let took = begun.elapsed();
        assert_eq!(records, [b"answer".to_vec()]);
        assert_eq!(output.stderr(), "last words");
        // The stub exits within milliseconds of starting; scheduling slack beside that.
        assert!(
            took < DRAIN_BOUND + Duration::from_millis(150),
            "returned {took:?} after the start, past the drain bound"
        );
        assert!(
            !group_members(pid).is_empty(),
            "the grandchild was ended, though nothing was cancelled"
        );
        end_group(pid);
    }

    /// G11, a failure: the arguments, the status and what stderr said; a read names
    /// no locks.
    #[test]
    fn a_failure_carries_the_arguments_the_status_and_stderr() {
        let stub = stub("echo 'fatal: not a thing' >&2; exit 128");
        let outcome = within(DEADLINE, move || {
            started(&stub).finish(&never(), |_| {}, |_| {})
        });
        match outcome {
            Err(Error::GitFailed {
                arguments,
                status,
                stderr,
                present_locks,
            }) => {
                assert_eq!(arguments, "stub");
                assert_eq!(status.code(), Some(128));
                assert_eq!(stderr, "fatal: not a thing");
                assert!(present_locks.is_empty());
            }
            other => panic!("expected the failure, got {other:?}"),
        }
    }

    /// G12, a cancel after a clean exit is the success it was. The exit is
    /// observed (a zombie) before the cancel. Caught by: deciding by the flag alone.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_cancel_after_a_clean_exit_is_reported_as_success() {
        let stub = stub("echo done >&2; exit 0");
        let invocation = started(&stub);
        let pid = invocation.id();
        assert!(
            eventually(DEADLINE, || std::fs::read_to_string(format!(
                "/proc/{pid}/stat"
            ))
            .is_ok_and(|stat| stat.contains(") Z "))),
            "the stub never exited"
        );
        invocation.kill_handle().kill();
        let output = invocation.finish(&never(), |_| {}, |_| {}).unwrap();
        assert_eq!(output.stderr(), "done");
    }

    /// G12, the other side: a cancelled stub that exits 0 after the signal is a
    /// cancel, not a success — its status may be the signal's doing. Caught by:
    /// deciding by the exit status alone, as the old runner did.
    #[test]
    fn a_cancelled_process_that_exits_zero_after_the_signal_is_reported_cancelled() {
        let stub = stub(
            "PATH=/usr/bin:/bin; command -v sleep >/dev/null || exit 99; \
             sleep 30 & child=$!; trap 'kill $child 2>/dev/null; exit 0' TERM; \
             echo hanging >&2; wait $child",
        );
        let (outcome, seen, _) = cancel_once_hanging(started(&stub), Cancelling::ByHandle);
        assert_eq!(seen, ["hanging"]);
        assert!(
            matches!(outcome, Err(Error::GitReadCancelled { .. })),
            "{outcome:?}"
        );
    }

    /// G12, nonzero: a cancelled stub that exits with a failure of its own is a
    /// cancel, not that failure.
    #[test]
    fn a_cancelled_process_that_exits_nonzero_is_reported_cancelled() {
        let stub = stub(ENDING_ON_TERM);
        let (outcome, _, _) = cancel_once_hanging(started(&stub), Cancelling::BySignal);
        assert!(
            matches!(outcome, Err(Error::GitReadCancelled { .. })),
            "{outcome:?}"
        );
    }

    /// Nothing signals after the reap: the pid is the system's to reuse by then.
    /// Caught by: a kill handle that signals without the believed-alive check.
    #[test]
    fn a_kill_after_the_invocation_is_over_signals_nothing() {
        let stub = stub("echo done >&2; exit 0");
        let invocation = started(&stub);
        let handle = invocation.kill_handle();
        let group = invocation.group();
        invocation.finish(&never(), |_| {}, |_| {}).unwrap();
        handle.kill();
        assert!(
            !group.leader().terminated(),
            "a SIGTERM went to a group whose leader was reaped and whose pipes were closed"
        );
    }

    /// The kill handle never waits: with the lock held, as the driver holds it for a
    /// poll, it returns at once and sends nothing — and the driver, seeing the
    /// request, sends the `SIGTERM` itself. Caught by: `lock()` in place of
    /// `try_lock()` (the kill blocks until the test lets go), or a request lost.
    #[test]
    fn a_kill_that_misses_the_lock_returns_at_once_and_the_driver_finishes_it() {
        let stub = stub(ENDING_ON_TERM);
        let invocation = started(&stub);
        let handle = invocation.kill_handle();
        let group = invocation.group();
        let (outcome, seen) = within(DEADLINE, move || {
            let mut seen = Vec::new();
            let outcome = invocation.finish(
                &never(),
                |_| {},
                |line| {
                    seen.push(line.to_owned());
                    if line == "hanging" {
                        let held = group.leader();
                        let killing = Instant::now();
                        handle.kill();
                        assert!(
                            killing.elapsed() < Duration::from_millis(50),
                            "the kill waited"
                        );
                        assert!(!held.terminated(), "the kill took a lock that was held");
                    }
                },
            );
            (outcome, seen)
        });
        assert!(
            matches!(outcome, Err(Error::GitReadCancelled { .. })),
            "{outcome:?}"
        );
        assert_eq!(
            seen,
            ["hanging", "terminated"],
            "the driver never sent the SIGTERM"
        );
    }

    /// G13: 1 MiB of stderr in lines reaches the progress callback line by line,
    /// and what is retained is at most 256 KiB and ends with the last line. Caught
    /// by: retaining everything, or dropping lines under volume.
    #[test]
    fn a_mib_of_stderr_is_forwarded_whole_and_retained_as_a_bounded_tail() {
        const LINES: usize = 21_000;
        let stub = stub(&format!(
            "PATH=/usr/bin:/bin; command -v seq >/dev/null || exit 99; \
             seq -f 'stderr line %06g padded to fifty bytes or so....' 1 {LINES} >&2"
        ));
        let (output, lines) = within(DEADLINE, move || {
            let mut lines = Vec::new();
            let output = started(&stub)
                .finish(&never(), |_| {}, |line| lines.push(line.to_owned()))
                .unwrap();
            (output, lines)
        });
        let bytes: usize = lines.iter().map(|line| line.len() + 1).sum();
        assert!(bytes >= 1024 * 1024, "only {bytes} bytes of stderr");
        assert_eq!(lines.len(), LINES);
        for (n, line) in lines.iter().enumerate() {
            assert_eq!(
                line,
                &format!("stderr line {:06} padded to fifty bytes or so....", n + 1)
            );
        }
        let tail = output.stderr();
        assert!(
            tail.len() <= pipes::TAIL_BYTES,
            "retained {} bytes",
            tail.len()
        );
        assert!(
            tail.len() > pipes::TAIL_BYTES / 2,
            "retained only {} bytes",
            tail.len()
        );
        assert!(tail.ends_with(&format!(
            "stderr line {LINES:06} padded to fifty bytes or so...."
        )));
    }

    /// Thread hygiene: every thread an invocation starts has ended once it has
    /// finished, with stdin, stdout and stderr all in use. Caught by: a reader
    /// that outlives its pipe, or a writer that never closes stdin.
    #[test]
    fn every_thread_an_invocation_starts_ends_with_it() {
        let stub = stub("PATH=/usr/bin:/bin; cat; echo said >&2");
        let names = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = Arc::clone(&names);
        let spawner = move |name: &str, body: Box<dyn FnOnce() + Send>| {
            recorded.lock().unwrap().push(name.to_owned());
            os_thread(name, body)
        };
        let invocation = discover_retrying(stub.environment())
            .unwrap()
            .read_invocation()
            .input(b"fed".to_vec())
            .start_with(&spawner)
            .unwrap();
        let group = invocation.group();
        assert_eq!(
            *names.lock().unwrap(),
            ["cairn-git-stdin", "cairn-git-stdout", "cairn-git-stderr"],
            "stdin, stdout and stderr each get a thread of their own"
        );
        let output = within(DEADLINE, move || invocation.collect(&never(), 1024, |_| {})).unwrap();
        assert_eq!(output.stdout(), b"fed");
        assert!(
            eventually(Duration::from_secs(2), || group.live_threads() == 0),
            "{} threads outlived the invocation",
            group.live_threads()
        );
    }

    /// A thread that cannot start is an error, and the process it was to watch is
    /// ended and reaped first — never left running with nobody waiting on it.
    /// Caught by: returning the error with the process still running (the old
    /// runner's leak).
    #[cfg(target_os = "linux")]
    #[test]
    fn a_thread_that_cannot_start_ends_and_reaps_the_process() {
        let stub = StubGit::with_git_from(|directory| {
            format!(
                "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                 echo $$ > '{}'; {HANGING_WITH_A_GRANDCHILD}",
                directory.join("pid").display()
            )
        });
        let pid_file = stub.directory().join("pid");
        let git = discover_retrying(stub.environment()).unwrap();
        // The stdout reader, then the stderr reader: the first and the second thread.
        for fail_at in 0..2 {
            let _ = std::fs::remove_file(&pid_file);
            let calls = Arc::new(AtomicUsize::new(0));
            let counted = Arc::clone(&calls);
            let file = pid_file.clone();
            let spawner = move |name: &str, body: Box<dyn FnOnce() + Send>| {
                if counted.fetch_add(1, Ordering::SeqCst) == fail_at {
                    // Fail only once the stub is really running, so the end is decisive.
                    assert!(
                        eventually(DEADLINE, || std::fs::read_to_string(&file)
                            .is_ok_and(|pid| pid.ends_with('\n'))),
                        "the stub never started"
                    );
                    return Err(io::Error::other(format!("no thread for {name}")));
                }
                os_thread(name, body)
            };
            let begun = Instant::now();
            let outcome = git.read_invocation().arg("stub").start_with(&spawner);
            let took = begun.elapsed();
            let Err(Error::GitUnwatched { arguments, source }) = outcome else {
                panic!("expected the thread failure, got {outcome:?}");
            };
            assert_eq!(arguments, "stub");
            assert!(source.to_string().starts_with("no thread for"), "{source}");
            assert!(
                took < TERMINATION_GRACE,
                "the stub took {took:?} to end: not by SIGTERM"
            );
            let pid: u32 = std::fs::read_to_string(&pid_file)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            assert!(
                reaped(pid),
                "thread {fail_at} failed and the stub {pid} was not reaped"
            );
            assert!(
                eventually(DEADLINE, || group_members(pid).is_empty()),
                "thread {fail_at} failed and the stub's group was left: {:?}",
                group_members(pid)
            );
        }
    }

    /// A stdin write that fails other than by git closing its end ends the process
    /// and is an error — git would otherwise take what it got as the whole input.
    /// git closing stdin early is its choice, and no error.
    #[test]
    fn a_failed_stdin_write_ends_the_process_and_a_closed_stdin_does_not() {
        struct Failing(io::ErrorKind);
        impl io::Write for Failing {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(self.0))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let stub = stub(HANGING_WITH_A_GRANDCHILD);
        let invocation = started(&stub);
        let group = invocation.group();
        pipes::feed(Failing(io::ErrorKind::BrokenPipe), b"input", &group);
        assert!(
            !group.ending(),
            "git closing its stdin is not a reason to end it"
        );
        pipes::feed(Failing(io::ErrorKind::Other), b"input", &group);
        assert!(group.ending());
        let outcome = within(DEADLINE, move || {
            invocation.finish(&never(), |_| {}, |_| {})
        });
        assert!(
            matches!(&outcome, Err(Error::GitUnwatched { source, .. }) if source.kind() == io::ErrorKind::Other),
            "{outcome:?}"
        );
    }

    /// A last record without its NUL is handed on when the invocation succeeded.
    #[test]
    fn a_last_record_without_its_terminator_still_counts_on_success() {
        let stub = stub("printf 'a\\0b'");
        let mut records = Vec::new();
        started(&stub)
            .records(&never(), |record| records.push(record.to_vec()), |_| {})
            .unwrap();
        assert_eq!(records, [b"a".to_vec(), b"b".to_vec()]);
    }

    /// R4.2 at the type level: the handle crosses threads and is shared.
    #[test]
    fn the_kill_handle_is_send_and_clone() {
        fn send_and_clone<T: Send + Sync + Clone + 'static>() {}
        send_and_clone::<super::KillHandle>();
    }

    /// G19's reporter: the runner's cost over a bare `std::process::Command` for
    /// the same read, on the bench repository. Not a check — a timing assertion is
    /// bound to a machine — so it reports, and `progress.md` records what it said.
    /// Run in release: `CAIRN_BENCH_REPO=<rust checkout> cargo test --release -p
    /// cairn-git g19 -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement for G19, driven by CAIRN_BENCH_REPO; run by hand in release"]
    fn g19_reports_the_runners_overhead_over_a_bare_command() {
        const RUNS: usize = 40;
        let Some(repository) = std::env::var_os("CAIRN_BENCH_REPO") else {
            eprintln!("G19: CAIRN_BENCH_REPO is not set; nothing measured");
            return;
        };
        let git = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None)).unwrap();
        // M1 against its first parent, as the measured baseline compares it: a merge
        // alone prints nothing to diff-tree. Then the floor — a commit against itself,
        // an empty diff — where the process is all there is, so the overhead shows.
        for (what, arguments) in [
            (
                "M1",
                [
                    "diff-tree",
                    "-r",
                    "-M",
                    "-z",
                    "--raw",
                    "5a3292f163d^1",
                    "5a3292f163d",
                ],
            ),
            (
                "floor",
                [
                    "diff-tree",
                    "-r",
                    "-M",
                    "-z",
                    "--raw",
                    "c999cef531e",
                    "c999cef531e",
                ],
            ),
        ] {
            let through_the_runner = || {
                let started = Instant::now();
                let output = git
                    .read_invocation()
                    .arg("-C")
                    .arg(&repository)
                    .args(arguments)
                    .start()
                    .unwrap()
                    .collect(&never(), 1 << 30, |_| {})
                    .unwrap();
                (started.elapsed(), output.stdout().len())
            };
            let bare = || {
                let started = Instant::now();
                let output = std::process::Command::new(git.path())
                    .arg("-C")
                    .arg(&repository)
                    .args(arguments)
                    .output()
                    .unwrap();
                assert!(output.status.success());
                (started.elapsed(), output.stdout.len())
            };
            for _ in 0..3 {
                through_the_runner();
                bare();
            }
            let (mut runner, mut command) = (Vec::new(), Vec::new());
            let mut bytes = 0;
            for _ in 0..RUNS {
                let (took, length) = through_the_runner();
                runner.push(took);
                bytes = length;
                let (took, length) = bare();
                command.push(took);
                assert_eq!(length, bytes, "the two paths read different answers");
            }
            runner.sort();
            command.sort();
            let median = |times: &[Duration]| times[times.len() / 2];
            eprintln!(
                "G19 {what}: {RUNS} runs each, {bytes} bytes of output; runner median {:?} min \
                 {:?}; bare Command median {:?} min {:?}; overhead at the median {:?}, at the \
                 min {:?}",
                median(&runner),
                runner[0],
                median(&command),
                command[0],
                median(&runner).saturating_sub(median(&command)),
                runner[0].saturating_sub(command[0]),
            );
        }
    }
}
