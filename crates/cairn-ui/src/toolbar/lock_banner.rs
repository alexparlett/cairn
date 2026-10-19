//! The leftover lock's banner: drawn under the title bar while `<gitdir>/index.lock` is there,
//! no `git` of Cairn's runs or waits in the repository — the window decides those, and draws
//! this only then — and the lock is older than [`LOCK_STALE_AFTER`], which this component
//! decides: a lock younger than that is likely a running git's, and the banner waits on the
//! toolkit's executor (`async_io::Timer`, never a sleep) until it is old enough. It offers
//! `Remove index.lock…`, which the window answers with the "Remove Stale Lock" confirmation.

use std::time::{Duration, SystemTime};

use async_io::Timer;
use freya::prelude::*;

use crate::dialogs::git_error::REMOVE_LOCK_CAPTION;
use crate::style::{metrics, tones};

/// What the banner says.
pub const LOCK_BANNER_TEXT: &str = "index.lock is in this repository, so git can't change the \
                                    index. If no other git program is running, it was left behind.";
/// How old a lock must be before the banner shows: about ten seconds.
pub const LOCK_STALE_AFTER: Duration = Duration::from_secs(10);

/// Whether a lock last changed at `changed` is old enough at `now` for the banner; a lock dated
/// in the future (a clock moved) is not.
pub fn lock_is_stale(changed: SystemTime, now: SystemTime) -> bool {
    now.duration_since(changed)
        .is_ok_and(|age| age >= LOCK_STALE_AFTER)
}

/// The banner over a lock last changed at `changed`.
#[derive(Clone, PartialEq)]
pub struct LockBanner {
    changed: SystemTime,
    on_remove: EventHandler<()>,
    key: DiffKey,
}

impl LockBanner {
    pub fn new(changed: SystemTime) -> Self {
        Self {
            changed,
            on_remove: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    /// `Remove index.lock…` pressed.
    pub fn on_remove(mut self, on_remove: impl Into<EventHandler<()>>) -> Self {
        self.on_remove = on_remove.into();
        self
    }
}

impl std::fmt::Debug for LockBanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockBanner")
            .field("changed", &self.changed)
            .finish_non_exhaustive()
    }
}

impl KeyExt for LockBanner {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for LockBanner {
    fn render(&self) -> impl IntoElement {
        let changed = self.changed;
        let mut stale = use_state(move || lock_is_stale(changed, SystemTime::now()));
        // A task of this banner's scope: it ends with the banner, and wakes it once the lock is
        // old enough.
        use_hook(move || {
            if *stale.peek() {
                return;
            }
            let left = changed
                .checked_add(LOCK_STALE_AFTER)
                .and_then(|at| at.duration_since(SystemTime::now()).ok())
                .unwrap_or(Duration::ZERO);
            spawn(async move {
                Timer::after(left).await;
                stale.set(true);
            });
        });
        if !*stale.read() {
            return rect().into_element();
        }
        let on_remove = self.on_remove.clone();
        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .cross_align(Alignment::center())
            .spacing(12.)
            .padding(Gaps::new(6., 12., 6., 12.))
            .background(tones::BANNER)
            .child(
                // Wrapped, never cut.
                label()
                    .text(LOCK_BANNER_TEXT)
                    .font_size(metrics::BODY)
                    .color(tones::TEXT)
                    .width(Size::flex(1.)),
            )
            .child(
                Button::new()
                    .compact()
                    .on_press(move |_| on_remove.call(()))
                    .child(REMOVE_LOCK_CAPTION),
            )
            .into_element()
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The banner waits until the lock is about ten seconds old — a younger one is likely a
    /// running git's — and a lock dated ahead of the clock never shows it. Caught by: the age
    /// read the wrong way round, or a future time read as old.
    #[test]
    fn a_lock_is_stale_from_ten_seconds_old() {
        let now = SystemTime::now();
        assert!(!lock_is_stale(now, now));
        assert!(!lock_is_stale(now - Duration::from_secs(9), now));
        assert!(lock_is_stale(now - Duration::from_secs(10), now));
        assert!(lock_is_stale(now - Duration::from_secs(3600), now));
        assert!(!lock_is_stale(now + Duration::from_secs(60), now));
    }
}
