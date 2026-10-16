//! Where an operation's ending goes: one home for each kind.
//!
//! - A success says nothing: the refresh is the message.
//! - A cancel opens nothing.
//! - A git failure opens Fork's Git Error dialog — its title and sentence, `Error Details:`, the
//!   command and git's output — for every operation the person started, a fetch among them;
//!   one git refused for the lock carries `Remove index.lock…` beside Close.
//! - A refusal found while it ran — a stale patch, a file changed since it was confirmed, `HEAD`
//!   moved before an amend, closing — opens the same dialog titled "Couldn't <name>", its one
//!   sentence and "Nothing was changed." once, and no Error Details; a failure beneath git
//!   says its sentence there too, without the tail, since it may have done part of its work.
//! - A discard that left files as they were says so in one sentence there, the files behind
//!   Show files, each path as `cairn_model::shown_path` spells it.
//!
//! The commit box keeps its own Git Error, whose skip and token are its (`commit_box::state`);
//! every other ending's dialog is kept here, in [`Endings`], the oldest first — one shown at a
//! time, as one modal state decides (`crate::modal`).

use std::collections::VecDeque;
use std::rc::Rc;

use cairn_ui::{GitErrorDialog, NOTHING_CHANGED};
use freya::prelude::*;

use crate::operations::shown_output::shown_lines;
use crate::window::View;
use crate::worker::{Failure, Request, WriteEnding};

/// One ending's dialog, as the window keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub serial: u64,
    pub title: String,
    pub sentence: String,
    /// The command and git's output, for a git failure.
    pub details: Option<(String, Rc<Vec<String>>)>,
    /// The files a partly done discard left as they were.
    pub files: Option<Rc<Vec<String>>>,
    /// git could not create `index.lock`: `Remove index.lock…` is offered beside Close.
    pub remove_lock: bool,
}

/// The endings' dialogs waiting to be read, the oldest first.
#[derive(Debug, Default)]
pub struct Endings {
    waiting: VecDeque<Shown>,
    next_serial: u64,
}

impl Endings {
    /// The dialog to draw: the oldest unread.
    pub fn shown(&self) -> Option<&Shown> {
        self.waiting.front()
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// `shown` waits its turn, under a serial of its own.
    pub fn push(&mut self, mut shown: Shown) {
        self.next_serial += 1;
        shown.serial = self.next_serial;
        self.waiting.push_back(shown);
    }

    /// The dialog drawn was answered: the next one, if any, is drawn.
    pub fn close(&mut self, serial: u64) {
        if self
            .waiting
            .front()
            .is_some_and(|shown| shown.serial == serial)
        {
            self.waiting.pop_front();
        }
    }
}

/// A git failure, or one beneath git, of the operation called `name`.
pub fn of_failure(name: &str, failure: &Failure) -> Shown {
    match &failure.command {
        Some(command) => Shown {
            serial: 0,
            title: cairn_ui::GIT_ERROR_TITLE.to_owned(),
            sentence: failure.said.clone(),
            details: Some((command.clone(), Rc::new(shown_lines(&failure.output)))),
            files: None,
            remove_lock: failure.index_lock,
        },
        None => couldnt(name, failure.said.clone()),
    }
}

/// "Couldn't <name>", saying `sentence`.
pub fn couldnt(name: &str, sentence: String) -> Shown {
    Shown {
        serial: 0,
        title: cairn_ui::couldnt_title(name),
        sentence,
        details: None,
        files: None,
        remove_lock: false,
    }
}

/// A refusal found while `name` ran: its sentence, then "Nothing was changed." once.
pub fn refused(name: &str, said: &str) -> Shown {
    couldnt(name, format!("{said} {NOTHING_CHANGED}"))
}

/// The dialog `ending` of the write called `name` opens, or `None` for a success, a cancel and
/// an amend asking to be confirmed — whose dialog is the confirmation.
pub fn of_write(name: &str, ending: &WriteEnding) -> Option<Shown> {
    match ending {
        WriteEnding::Done(_) | WriteEnding::Cancelled { .. } => None,
        WriteEnding::NeedsConfirming { .. } => None,
        WriteEnding::Stale { said, .. }
        | WriteEnding::Refused { said }
        | WriteEnding::NotRun { said } => Some(refused(name, said)),
        WriteEnding::Failed(failure) => Some(of_failure(name, failure)),
        WriteEnding::Incomplete {
            kept,
            said,
            failure,
            ..
        } => {
            let files = Rc::new(
                kept.iter()
                    .map(|path| cairn_model::shown_path(path))
                    .collect::<Vec<_>>(),
            );
            let details = failure.as_ref().and_then(|failure| {
                failure
                    .command
                    .clone()
                    .map(|command| (command, Rc::new(shown_lines(&failure.output))))
            });
            Some(Shown {
                files: (!kept.is_empty()).then_some(files),
                details,
                remove_lock: failure.as_ref().is_some_and(|failure| failure.index_lock),
                ..couldnt(name, said.clone())
            })
        }
    }
}

/// The ending dialog drawn now, if one waits: Close lets it go and draws the next; `Remove
/// index.lock…` lets it go and asks what the removal would cost (`crate::operations::leftover_lock`).
pub fn dialog(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Option<Element> {
    let shown = view.endings.read().shown().cloned()?;
    let serial = shown.serial;
    let closing = move |()| {
        let mut endings = view.endings;
        endings.write().close(serial);
    };
    let removing = move |()| {
        let mut endings = view.endings;
        endings.write().close(serial);
        crate::operations::leftover_lock::ask(view, submit.as_deref());
    };
    Some(
        GitErrorDialog::new(serial, String::new(), Rc::new(Vec::new()))
            .details(shown.details.clone())
            .title(shown.title.clone())
            .sentence(shown.sentence.clone())
            .files(shown.files.clone())
            .remove_lock(shown.remove_lock)
            .on_remove_lock(removing)
            .on_close(closing)
            .key(DiffKey::U64(serial))
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::Done;

    fn done() -> Done {
        Done {
            description: "discarded".to_owned(),
            acknowledged: None,
            locks_before: Vec::new(),
            locks_after: Vec::new(),
            replaced: None,
            cost_after: None,
        }
    }

    fn git_failure(index_lock: bool) -> Failure {
        Failure {
            said: cairn_ui::GIT_ERROR_TEXT.to_owned(),
            command: Some("git add -- a".to_owned()),
            output: cairn_model::ScrubbedLines::scrubbing("fatal: Unable to create index.lock"),
            locks: Vec::new(),
            index_lock,
        }
    }

    /// Every ending of a write to exactly one home: a success and a cancel nothing, git's
    /// failure Fork's Git Error with its command and output (and the lock's removal where git
    /// could not create it), a refusal "Couldn't <name>" with its sentence and "Nothing was
    /// changed." once, a failure beneath git its sentence alone, and a partly done discard its
    /// sentence with the kept files, escaped, behind Show files. Caught by: a dialog for a
    /// success or a cancel, the tail on a failure that may have done part, the tail twice, or a
    /// path drawn raw.
    #[test]
    fn every_ending_has_exactly_one_home() {
        let name = "Stage 2 Files";
        assert_eq!(of_write(name, &WriteEnding::Done(done())), None);
        assert_eq!(
            of_write(
                name,
                &WriteEnding::Cancelled {
                    locks: Vec::new(),
                    ran: true,
                }
            ),
            None
        );

        let failed = of_write(name, &WriteEnding::Failed(git_failure(true))).unwrap();
        assert_eq!(failed.title, cairn_ui::GIT_ERROR_TITLE);
        assert_eq!(failed.sentence, cairn_ui::GIT_ERROR_TEXT);
        assert_eq!(
            failed
                .details
                .as_ref()
                .map(|(command, lines)| (command.as_str(), lines.len())),
            Some(("git add -- a", 1))
        );
        assert!(failed.remove_lock);
        assert!(
            !of_write(name, &WriteEnding::Failed(git_failure(false)))
                .unwrap()
                .remove_lock
        );

        let stale = of_write(
            name,
            &WriteEnding::Stale {
                path: "a".to_owned(),
                said: "a changed after its diff was read.".to_owned(),
            },
        )
        .unwrap();
        assert_eq!(stale.title, "Couldn't Stage 2 Files");
        assert_eq!(
            stale.sentence,
            "a changed after its diff was read. Nothing was changed."
        );
        assert_eq!(stale.details, None);
        assert_eq!(stale.sentence.matches(NOTHING_CHANGED).count(), 1);

        let beneath = of_write(
            name,
            &WriteEnding::Failed(Failure {
                said: "Cairn lost hold of git while it ran.".to_owned(),
                command: None,
                output: cairn_model::ScrubbedLines::new(),
                locks: Vec::new(),
                index_lock: false,
            }),
        )
        .unwrap();
        assert_eq!(beneath.title, "Couldn't Stage 2 Files");
        assert!(
            !beneath.sentence.contains(NOTHING_CHANGED),
            "{}",
            beneath.sentence
        );

        let partly = of_write(
            "Discard Changes",
            &WriteEnding::Incomplete {
                done: done(),
                kept: vec!["a\nb".to_owned(), "c".to_owned()],
                said: "2 files were left as they were; the rest were discarded.".to_owned(),
                failure: None,
            },
        )
        .unwrap();
        assert_eq!(partly.title, "Couldn't Discard Changes");
        assert_eq!(
            partly.files.as_deref().map(Vec::as_slice),
            Some([r#""a\nb""#.to_owned(), "c".to_owned()].as_slice())
        );
        assert_eq!(partly.details, None);
    }

    /// The worker side words a git failure, and a refusal during an operation, as the render
    /// crate's own constants and functions do — it may not name them (CLAUDE.md's UI-thread
    /// invariant) — so these are held equal here. Caught by: the two drifting apart.
    #[test]
    fn the_workers_words_are_the_dialogs() {
        use cairn_model::{Oid, OperationInProgress};
        assert_eq!(crate::worker::GIT_FAILED, cairn_ui::GIT_ERROR_TEXT);
        let picked = Some(Oid::from_bytes(&[1; 20]).unwrap());
        for operation in [
            OperationInProgress::Merge,
            OperationInProgress::CherryPick { picked },
            OperationInProgress::CherryPickSequence,
            OperationInProgress::Revert { reverted: None },
            OperationInProgress::RevertSequence,
            OperationInProgress::Rebase,
            OperationInProgress::ApplyingPatches,
        ] {
            assert_eq!(
                crate::worker::checkout_refused_in(&operation),
                cairn_ui::discard_refusal(&operation)
            );
            if operation.refuses_commit() {
                assert_eq!(
                    Some(crate::worker::commit_refused_in(&operation)),
                    cairn_ui::operation_line(Some(&operation), false)
                );
            }
        }
    }

    /// One dialog at a time, the oldest first, each closed by its own serial. Caught by: a newer
    /// ending replacing one unread, or a stale Close letting go of the next.
    #[test]
    fn endings_wait_their_turn() {
        let mut endings = Endings::default();
        endings.push(couldnt("Stage 1 File", "one.".to_owned()));
        endings.push(couldnt("Stage 1 File", "two.".to_owned()));
        let first = endings.shown().map(|shown| shown.serial).unwrap();
        assert_eq!(
            endings.shown().map(|shown| shown.sentence.as_str()),
            Some("one.")
        );
        endings.close(first + 1);
        assert_eq!(
            endings.shown().map(|shown| shown.sentence.as_str()),
            Some("one.")
        );
        endings.close(first);
        assert_eq!(
            endings.shown().map(|shown| shown.sentence.as_str()),
            Some("two.")
        );
    }
}
