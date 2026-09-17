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
    socket: PathBuf,
}

impl AcceptorStop {
    pub(super) fn new(socket: PathBuf) -> Self {
        Self {
            stopping: Arc::new(AtomicBool::new(false)),
            socket,
        }
    }

    /// Flags the stop, then connects and hangs up: an empty connection is a
    /// malformed request, which is what returns `accept` to a loop that then
    /// sees the flag. Nothing else can wake a blocking `accept`.
    pub(super) fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _ = std::os::unix::net::UnixStream::connect(&self.socket);
    }

    fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
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

    #[test]
    fn a_stop_is_visible_from_a_clone_and_survives_a_socket_nobody_listens_on() {
        let stop = AcceptorStop::new(PathBuf::from("/nonexistent/cairn/askpass"));
        let seen_from = stop.clone();
        assert!(!seen_from.is_stopping());
        stop.stop();
        assert!(seen_from.is_stopping(), "the flag is not shared");
    }

    #[test]
    fn prompt_ids_are_distinct_values_the_window_can_hand_back() {
        assert_ne!(PromptId(1), PromptId(2));
        assert_eq!(PromptId(7), PromptId(7));
    }
}
