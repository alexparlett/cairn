//! What C14's window check (`crate::window_check`) needs of the worker side of the partition:
//! the next update within a frame's time. The window check paces frames by it and, being a
//! render-side file, may not wait itself; the waiting is here.

use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use super::pool::Updates;
use super::request::Update;

/// The next update if one arrives within `wait`, or `None` — when nothing came in time, or the
/// stream ended. For the window check (`window_check`), which paces frames by it and may not
/// wait itself: the waiting is here, on the worker side of the partition.
pub(crate) fn update_within(updates: &mut Updates, wait: Duration) -> Option<Update> {
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let deadline = Instant::now() + wait;
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut next = std::pin::pin!(updates.next());
    loop {
        if let Poll::Ready(next) = next.as_mut().poll(&mut cx) {
            return next;
        }
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        std::thread::park_timeout(deadline - now);
    }
}
