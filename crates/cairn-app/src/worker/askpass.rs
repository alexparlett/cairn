//! The application's end of the askpass channel: a thread that accepts the
//! helper's questions, hands each to the window as a value, and writes the
//! answer back.
//!
//! One prompt at a time, because that is how git and ssh ask. The secret
//! comes back from the window as an [`Reply`], over a channel of its own —
//! never as a [`super::Request`], which derives `Debug` — and is handed to
//! the channel by reference and dropped: it is never kept, and nothing here
//! reads its bytes (`no_credential_value_is_logged_printed_serialised_or_stored`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use cairn_askpass::{Channel, Error};
use cairn_model::Secret;

use super::pool::Outbox;
use super::request::Update;

/// Names one prompt from the moment the window is told about it until it is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PromptId(u64);

impl PromptId {
    /// An id nothing issued, for a view test; production ids come from the acceptor.
    #[cfg(test)]
    pub fn for_tests(n: u64) -> Self {
        Self(n)
    }
}

/// The window's reply to a prompt. Deliberately derives nothing: it carries
/// the credential, and a `Debug` here would be a way to print it.
pub enum Reply {
    /// What the user typed, consumed by the one helper waiting on `prompt`.
    Provide { prompt: PromptId, secret: Secret },
    /// The user declined; the helper is refused and git's operation fails closed.
    Refuse { prompt: PromptId },
}

/// Unblocks and ends [`serve_prompts`] from another thread.
#[derive(Debug, Clone)]
pub(super) struct AcceptorStop {
    stopping: Arc<AtomicBool>,
    /// Set by the acceptor as it leaves its loop, on every way out.
    stopped: Arc<AtomicBool>,
    socket: PathBuf,
}

/// How long [`AcceptorStop::stop`] keeps waking the acceptor before giving up
/// on an acknowledgement — an acceptor blocked on a prompt the window still
/// holds the answering end of cannot leave until that end goes.
const STOP_DEADLINE: Duration = Duration::from_secs(1);

/// Between wake-ups. Each is one connection in the listener's backlog until the
/// acceptor takes it, so the deadline and this together must stay well under
/// that backlog (128 on Linux).
const STOP_RETRY: Duration = Duration::from_millis(25);

impl AcceptorStop {
    pub(super) fn new(socket: PathBuf) -> Self {
        Self {
            stopping: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(false)),
            socket,
        }
    }

    /// Flags the stop, then connects and hangs up: an empty connection is a
    /// malformed request, which is what returns `accept` to a loop that then
    /// sees the flag. Nothing else can wake a blocking `accept`, so the
    /// connection is retried until the acceptor acknowledges it has left
    /// ([`Self::acknowledge`]) or [`STOP_DEADLINE`] passes — a single
    /// best-effort connect that failed would otherwise leave the acceptor
    /// blocked with nothing coming, and this is the worker side, where
    /// waiting is allowed. Returns once acknowledged; a return at the
    /// deadline means the acceptor is held elsewhere (on the window's answer
    /// to a prompt) and will see the flag when it is next free.
    pub(super) fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + STOP_DEADLINE;
        loop {
            // Dropped at once: the acceptor reads the connection to its end, and an open one
            // would hold it there instead of returning it to the flag.
            let _ = std::os::unix::net::UnixStream::connect(&self.socket);
            if self.stopped() || Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(STOP_RETRY);
        }
    }

    fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    /// The acceptor has left its loop, or never entered it.
    pub(super) fn acknowledge(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    pub(super) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}

/// Acknowledges the stop when [`serve_prompts`] returns, by any path — a
/// panic included, so a stop never waits its whole deadline on a thread that
/// is already gone.
struct Leaving<'a>(&'a AcceptorStop);

impl Drop for Leaving<'_> {
    fn drop(&mut self) {
        self.0.acknowledge();
    }
}

/// Runs for the life of the channel: accepts one helper, tells the window,
/// waits for that prompt's [`Reply`], writes it back, and goes round again.
/// Ends when [`AcceptorStop::stop`] is called or the window's answering end
/// is gone; a prompt left waiting at that moment is refused, so the helper
/// and the git behind it fail closed rather than hang.
pub(super) fn serve_prompts(
    channel: Arc<Channel>,
    answers: Receiver<Reply>,
    outbox: &Outbox,
    stop: &AcceptorStop,
) {
    let _leaving = Leaving(stop);
    let mut issued = 0u64;
    loop {
        if stop.is_stopping() {
            break;
        }
        let prompt = match channel.accept() {
            Ok(prompt) => prompt,
            // A stop's wake-up, a stray connection, or a helper for an operation that
            // ended: the channel already refused it on the wire. Keep serving.
            Err(Error::Malformed | Error::UnknownToken) => continue,
            Err(_) => break,
        };
        issued += 1;
        let id = PromptId(issued);
        outbox.send(
            None,
            Update::Prompt {
                id,
                text: prompt.text().to_owned(),
            },
        );
        // Blocks on the user: this thread has nothing else to do until they decide.
        loop {
            match answers.recv() {
                Ok(Reply::Provide {
                    prompt: answered,
                    secret,
                }) if answered == id => {
                    // A helper that went away meanwhile is git's problem to report.
                    let _ = prompt.answer(&secret);
                    break;
                }
                Ok(Reply::Refuse { prompt: answered }) if answered == id => {
                    prompt.refuse();
                    break;
                }
                // An answer to a prompt that is no longer waiting; nothing to give it to.
                Ok(Reply::Provide { .. } | Reply::Refuse { .. }) => {}
                Err(_) => {
                    prompt.refuse();
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::worker::fetch_tests::RuntimeDir;

    /// A socket nobody listens on is never acknowledged: the stop gives up at its
    /// deadline, says so through `stopped`, and does not wait beyond it. The stop runs
    /// on a thread of its own, so a stop that lost its deadline is a red test here and
    /// not a stalled suite.
    #[test]
    fn a_stop_is_visible_from_a_clone_and_gives_up_on_a_socket_nobody_listens_on() {
        let stop = AcceptorStop::new(PathBuf::from("/nonexistent/cairn/askpass"));
        let seen_from = stop.clone();
        assert!(!seen_from.is_stopping());
        let (told, waited) = std::sync::mpsc::channel::<Duration>();
        let stopping = stop.clone();
        std::thread::spawn(move || {
            let started = Instant::now();
            stopping.stop();
            let _ = told.send(started.elapsed());
        });
        let waited = match waited.recv_timeout(STOP_DEADLINE * 3) {
            Ok(waited) => waited,
            Err(_) => panic!("the stop did not give up within {:?}", STOP_DEADLINE * 3),
        };
        assert!(seen_from.is_stopping(), "the flag is not shared");
        assert!(
            !stop.stopped(),
            "nothing acknowledged a stop nobody was serving"
        );
        assert!(
            waited >= STOP_DEADLINE,
            "the stop gave up after {waited:?}, before its deadline of {STOP_DEADLINE:?}"
        );
    }

    /// The deadline arm, and that the acknowledgement comes on the way OUT: an acceptor
    /// held on the window's answer to a prompt cannot leave, so a stop returns at its
    /// deadline unacknowledged; once the answering end goes, the prompt is refused, the
    /// loop is left, and only then is the stop acknowledged. An acknowledgement made on
    /// the way IN would pass the round-trip test below and fail here.
    #[test]
    fn a_stop_gives_up_on_an_acceptor_held_by_a_prompt_and_is_acknowledged_once_it_leaves() {
        let runtime = RuntimeDir::new();
        let channel = match Channel::open(&runtime.path) {
            Ok(channel) => Arc::new(channel),
            Err(error) => panic!("no channel: {error}"),
        };
        let operation = match channel.begin() {
            Ok(operation) => operation,
            Err(error) => panic!("no operation: {error}"),
        };
        let token = operation.token().clone();
        let socket = channel.socket_path().to_owned();
        let stop = AcceptorStop::new(socket.clone());
        let (answers, answered) = std::sync::mpsc::channel::<Reply>();
        let (outbox, prompts) = Outbox::watched();
        let serving = stop.clone();
        let acceptor =
            std::thread::spawn(move || serve_prompts(channel, answered, &outbox, &serving));
        // A helper asking, as git's would; it blocks until answered or refused.
        let helper = std::thread::spawn(move || cairn_askpass::ask(&socket, &token, b"Password: "));
        if prompts.recv_timeout(Duration::from_secs(10)).is_err() {
            panic!("the prompt never reached the outbox");
        }

        let started = Instant::now();
        stop.stop();
        assert!(
            !stop.stopped(),
            "acknowledged while the acceptor was still held on the window's answer"
        );
        assert!(
            started.elapsed() >= STOP_DEADLINE,
            "the stop returned after {:?}, before its deadline",
            started.elapsed()
        );

        // The window lets go: the prompt is refused and the acceptor leaves.
        drop(answers);
        assert!(acceptor.join().is_ok(), "the acceptor panicked");
        assert!(
            stop.stopped(),
            "the acceptor left its loop without acknowledging"
        );
        assert!(
            matches!(helper.join(), Ok(Err(cairn_askpass::Refusal::Declined))),
            "the helper was not refused when the acceptor left"
        );
        drop(operation);
    }

    /// Issue #21: whenever `stop` returns against a live acceptor, that acceptor has
    /// left its loop — however the wake-up and its `accept` interleaved, including a
    /// stop that lands before the thread has run at all. Repeated, since the
    /// interleaving is the scheduler's; and each wait is bounded, so a wake that fails
    /// to land is a red test here rather than a stalled suite.
    #[test]
    fn a_stop_returns_only_once_the_acceptor_has_left_its_loop() {
        let runtime = RuntimeDir::new();
        for round in 0..25 {
            let channel = match Channel::open(&runtime.path) {
                Ok(channel) => Arc::new(channel),
                Err(error) => panic!("no channel: {error}"),
            };
            let stop = AcceptorStop::new(channel.socket_path().to_owned());
            let (_answers, answered) = std::sync::mpsc::channel::<Reply>();
            let serving = stop.clone();
            let acceptor = std::thread::spawn(move || {
                let (outbox, _) = Outbox::watched();
                serve_prompts(channel, answered, &outbox, &serving);
            });
            // Odd rounds give the acceptor time to block in `accept`; even ones race it.
            if round % 2 == 1 {
                std::thread::sleep(Duration::from_millis(10));
            }
            let started = Instant::now();
            stop.stop();
            assert!(
                stop.stopped(),
                "round {round}: stop returned after {:?} without the acceptor leaving its loop",
                started.elapsed()
            );
            assert!(
                started.elapsed() < STOP_DEADLINE,
                "round {round}: the stop was acknowledged only by running out its deadline"
            );
            // Acknowledged means the thread is on its way out; joining cannot hang.
            assert!(
                acceptor.join().is_ok(),
                "round {round}: the acceptor panicked"
            );
        }
    }

    #[test]
    fn prompt_ids_are_distinct_values_the_window_can_hand_back() {
        assert_ne!(PromptId(1), PromptId(2));
        assert_eq!(PromptId(7), PromptId(7));
    }
}
