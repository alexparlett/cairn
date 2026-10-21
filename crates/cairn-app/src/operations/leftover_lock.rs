//! The leftover lock as the window keeps it: a state of the repository, decided by whether
//! `<gitdir>/index.lock` exists now — the refresh thread's `stat` on every refresh
//! (`Update::IndexLock`) — never by an operation's entry.
//!
//! The banner under the toolbar shows while the file is there, no `git` of Cairn's runs or
//! waits in the repository (a write running or queued, a fetch in flight), and the file is older
//! than about ten seconds (`cairn_ui::LockBanner` decides that last); it goes when a stat finds
//! the file gone, whoever removed it. Its `Remove index.lock…`, and the same button in a write's
//! Git Error where git could not create the lock, ask what the removal would cost, read at the
//! press on the local lane (`Request::LockConsequence`); the answer opens the one confirmation,
//! "Remove Stale Lock" over its prompt, whose token asks `LocalWrite::RemoveLock`; a
//! refusal at the press opens "Couldn't Remove index.lock". After the removal the window
//! refreshes, as every write's ending does, and never runs the failed write again, as Fork does.

use std::rc::Rc;
use std::time::SystemTime;

use cairn_model::Consequence;
use cairn_ui::LockBanner;
use freya::prelude::*;

use crate::operations::confirming::Confirming;
use crate::window::View;
use crate::worker::{LocalWrite, OperationId, Request};

/// What a confirmation of `Remove index.lock…` is titled, in Title Case.
pub const REMOVE_LOCK_TITLE: &str = "Remove Stale Lock";
/// The removal's one name, as its "Couldn't" dialog says it.
pub const REMOVE_LOCK_NAME: &str = "Remove index.lock";

/// The leftover lock's state.
#[derive(Debug, Default)]
pub struct LeftoverLock {
    /// When `<gitdir>/index.lock` was last changed, as the last stat found it; `None` when it
    /// was not there.
    changed: Option<SystemTime>,
    /// `Remove index.lock…` pressed: the ask its answer must name.
    asked: Option<OperationId>,
    /// What the removal would cost, read at the press and not yet put in front of the person.
    ready: Option<Consequence>,
}

impl LeftoverLock {
    /// What the last stat found.
    pub fn stated(&mut self, changed: Option<SystemTime>) {
        self.changed = changed;
    }

    pub fn changed(&self) -> Option<SystemTime> {
        self.changed
    }

    /// The press asked under `asked`.
    fn asked(&mut self, asked: OperationId) {
        self.asked = Some(asked);
    }

    /// The answer for `asked`, when it is the press's: `Some` of the answer, taken.
    fn answered(
        &mut self,
        asked: OperationId,
        outcome: Result<Consequence, String>,
    ) -> Option<Result<(), String>> {
        self.asked.take_if(|latest| *latest == asked)?;
        Some(outcome.map(|consequence| self.ready = Some(consequence)))
    }

    fn take_ready(&mut self) -> Option<Consequence> {
        self.ready.take()
    }

    fn has_ready(&self) -> bool {
        self.ready.is_some()
    }
}

/// `Remove index.lock…` pressed — on the banner, or in a write's Git Error: what the removal
/// would cost is asked of the local lane, read at the press.
pub fn ask(view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(submit) = submit else {
        return;
    };
    let asked = OperationId::next();
    let mut lock = view.lock;
    lock.write().asked(asked);
    submit(Request::LockConsequence { asked });
}

/// What the removal would cost, answering the press `asked`: kept for the confirmation
/// (`use_lock_confirmation`), or — refused at the press — "Couldn't Remove index.lock" with the
/// engine's one sentence. An answer for an earlier press is dropped.
pub fn answered(view: View, asked: OperationId, outcome: Result<Consequence, String>) {
    let mut lock = view.lock;
    let Some(answer) = lock.write().answered(asked, outcome) else {
        return;
    };
    if let Err(why) = answer {
        let mut endings = view.endings;
        endings
            .write()
            .push(crate::operations::ending_dialog::refused(
                REMOVE_LOCK_NAME,
                &why,
            ));
    }
}

/// The answer to a press of `Remove index.lock…`, put in front of the person: the confirmation
/// opens over what the engine read at the press, its token asking the local lane for the
/// removal — once no confirmation is open already. Run by the window, which holds the submitter
/// the token's ask needs.
pub fn use_lock_confirmation(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    use_side_effect(move || {
        if !view.lock.read().has_ready() || view.confirming.read().is_some() {
            return;
        }
        let mut lock = view.lock;
        let Some(consequence) = lock.write().take_ready() else {
            return;
        };
        let Some(submit) = submit.clone() else {
            return;
        };
        let mut confirming = view.confirming;
        confirming.set(Some(Confirming::new(
            REMOVE_LOCK_TITLE,
            consequence,
            move |token| {
                let mut writes = view.writes;
                crate::operations::local_writes::ask(
                    &mut writes.write(),
                    &*submit,
                    LocalWrite::RemoveLock(token),
                );
            },
        )));
    });
}

/// The banner, while the lock is there and no git of Cairn's runs or waits; its age is the
/// banner's own to decide.
pub fn banner(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Option<Element> {
    let changed = view.lock.read().changed()?;
    let busy = view.writes.read().is_busy() || view.fetch.read().is_in_flight();
    if busy {
        return None;
    }
    Some(
        // Keyed by the lock's time: another lock is another banner, deciding its age afresh.
        LockBanner::new(changed)
            .key(changed)
            .on_remove(move |()| ask(view, submit.as_deref()))
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn stale() -> Consequence {
        Consequence::RemoveLock {
            path: PathBuf::from("/r/.git/index.lock"),
            modified: SystemTime::UNIX_EPOCH,
            read_at: SystemTime::UNIX_EPOCH,
            bytes: 0,
            device: 0,
            inode: 0,
        }
    }

    /// The press's answer alone is kept: an earlier press's is dropped, a refusal is handed back
    /// to be said, and a consequence waits for the confirmation. Caught by: a stale answer
    /// opening a confirmation for a press nobody made.
    #[test]
    fn only_the_press_asked_last_is_answered() {
        let mut lock = LeftoverLock::default();
        let (first, second) = (OperationId::for_tests(1), OperationId::for_tests(2));
        lock.asked(first);
        lock.asked(second);
        assert_eq!(lock.answered(first, Ok(stale())), None);
        assert!(!lock.has_ready());
        assert_eq!(lock.answered(second, Ok(stale())), Some(Ok(())));
        assert!(lock.has_ready());
        assert!(lock.take_ready().is_some());
        lock.asked(first);
        assert_eq!(
            lock.answered(first, Err("There is no index.lock any more.".to_owned())),
            Some(Err("There is no index.lock any more.".to_owned()))
        );
        assert!(!lock.has_ready());
    }
}
