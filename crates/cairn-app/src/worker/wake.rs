//! Waking the UI task when a worker has sent something.

use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

/// A one-slot signal: set by a worker, consumed by the task waiting on it.
#[derive(Debug, Default)]
pub struct Wake {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    signalled: bool,
    waker: Option<Waker>,
}

impl Wake {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Latches, so a wake racing a `try_recv` is not lost.
    pub fn signal(&self) {
        let waker = {
            let mut state = self.locked();
            state.signalled = true;
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    fn poll(&self, cx: &Context<'_>) -> Poll<()> {
        let mut state = self.locked();
        if state.signalled {
            state.signalled = false;
            Poll::Ready(())
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    /// Recovers from a poisoned lock: a worker panic must still wake the UI.
    fn locked(&self) -> std::sync::MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// Yields until [`Wake::signal`] is called.
#[derive(Debug)]
pub struct Woken<'a>(pub &'a Arc<Wake>);

impl Future for Woken<'_> {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.0.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Counts wakes. Implemented on `std::task::Wake`, since `unsafe` is forbidden.
    #[derive(Debug, Default)]
    struct Counter(AtomicUsize);

    impl Counter {
        fn count(&self) -> usize {
            self.0.load(Ordering::SeqCst)
        }
    }

    impl std::task::Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn a_signal_that_arrives_first_is_not_lost() {
        let wake = Wake::new();
        wake.signal();
        let waker = Waker::noop();
        let cx = Context::from_waker(waker);
        assert_eq!(wake.poll(&cx), Poll::Ready(()), "a latched signal was lost");
        assert_eq!(
            wake.poll(&cx),
            Poll::Pending,
            "the signal was consumed more than once"
        );
    }

    #[test]
    fn a_signal_that_arrives_second_wakes_the_waiting_task() {
        let wake = Wake::new();
        let counter = Arc::new(Counter::default());
        let waker = Waker::from(Arc::clone(&counter));
        let cx = Context::from_waker(&waker);

        assert_eq!(wake.poll(&cx), Poll::Pending);
        assert_eq!(counter.count(), 0);
        wake.signal();
        assert_eq!(counter.count(), 1, "the parked task was never woken");
        assert_eq!(wake.poll(&cx), Poll::Ready(()));
    }

    #[test]
    fn a_signal_from_another_thread_wakes_the_waiting_task() {
        let wake = Wake::new();
        let counter = Arc::new(Counter::default());
        let waker = Waker::from(Arc::clone(&counter));
        let cx = Context::from_waker(&waker);
        assert_eq!(wake.poll(&cx), Poll::Pending);

        let from_worker = Arc::clone(&wake);
        std::thread::spawn(move || from_worker.signal())
            .join()
            .unwrap();

        assert_eq!(counter.count(), 1);
        assert_eq!(wake.poll(&cx), Poll::Ready(()));
    }

    #[test]
    fn a_poisoned_lock_does_not_stop_the_next_wake() {
        let wake = Wake::new();
        let poisoner = Arc::clone(&wake);
        let panicked = std::thread::spawn(move || {
            let _held = poisoner.locked();
            panic!("a worker died holding the lock");
        })
        .join();
        assert!(panicked.is_err(), "the thread was supposed to panic");

        wake.signal();
        let waker = Waker::noop();
        assert_eq!(wake.poll(&Context::from_waker(waker)), Poll::Ready(()));
    }
}
