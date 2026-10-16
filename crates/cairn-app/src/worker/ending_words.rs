//! The one sentence a person reads for each way a write, a count or a fetch can end other than
//! as asked: worded here, from the engine's typed error, never from its `Display` — the engine
//! says why once, and the view adds at most "Nothing was changed." to a refusal. Every path is
//! drawn through `cairn_model::shown_path`, so a name holding a newline or a bidirectional
//! control cannot reshape the sentence.
//!
//! An error with no sentence of its own here — one a write never meets, or a failure of the
//! machinery beneath git — is the engine's text of it, scrubbed there ([`Error::shown`]), as one
//! sentence: capitalised, with one full stop.

use cairn_git::{CheckoutMoved, CheckoutRefusal, CommitRefusal, Error, LockRefusal, Refusal};
use cairn_model::{OperationInProgress, shown_path};

/// The sentence Fork's Git Error dialog says of any git failure, as the dialog's own constant
/// reads (the Git Error dialog's `GIT_ERROR_TEXT`; this module, on the worker side, may not name the render
/// crate — `crate::operations::ending_dialog`'s tests hold the two equal).
pub(crate) const GIT_FAILED: &str =
    "An unexpected error occurred while performing the git request.";

/// A commit refused during an operation git refuses it in, as the commit box's line says it
/// (the commit box's `operation_line`, held equal by `crate::operations::ending_dialog`'s tests).
pub(crate) fn commit_refused_in(operation: &OperationInProgress) -> String {
    let name = operation.name();
    let name = match operation {
        // "git am" is a command's name, written as git writes it.
        OperationInProgress::ApplyingPatches => name.to_owned(),
        _ => {
            let mut letters = name.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(letters).collect()
            })
        }
    };
    format!(
        "{name} is in progress. Continue or abort it with {} --continue or --abort.",
        operation.git_command()
    )
}

/// Create Branch's Discard refused during an operation, as its dialog says it
/// (`discard_refusal`, held equal by `crate::operations::ending_dialog`'s tests).
pub(crate) fn checkout_refused_in(operation: &OperationInProgress) -> String {
    let named = match operation {
        OperationInProgress::Merge => "A merge",
        OperationInProgress::CherryPick { .. } | OperationInProgress::CherryPickSequence => {
            "A cherry-pick"
        }
        OperationInProgress::Revert { .. } | OperationInProgress::RevertSequence => "A revert",
        OperationInProgress::Rebase => "A rebase",
        OperationInProgress::ApplyingPatches => "An am session",
    };
    format!("{named} is in progress. Finish or abort it first.")
}

/// `error` in one sentence, for the "Couldn't <name>" dialog or a refusal beside a control.
pub(super) fn sentence(error: &Error) -> String {
    let words = match error {
        Error::ChangedSinceRead { path } => {
            format!("{} changed after its diff was read", shown_path(path))
        }
        Error::ContentReadsDisagree { path, .. } => {
            format!("{} changed while it was read", shown_path(path))
        }
        Error::ChangedSinceConfirmed { path } | Error::LockChangedSinceConfirmed { path } => {
            format!("{} changed since you confirmed", shown_path(path))
        }
        Error::AmendChangedSinceConfirmed => {
            "HEAD changed since you confirmed the amend".to_owned()
        }
        Error::Refused { path, why } => refused(&shown_path(path), *why),
        Error::NoPaths => "No file was given".to_owned(),
        Error::CommitRefused { why } => match why {
            CommitRefusal::InProgress(operation) => return commit_refused_in(operation),
            CommitRefusal::NothingToAmend => "There is no commit to amend yet".to_owned(),
            CommitRefusal::CommitEncoding { encoding } => format!(
                "i18n.commitEncoding is {}, and Cairn writes a message only as UTF-8",
                shown_path(encoding)
            ),
            CommitRefusal::NotWhatWasConfirmed => not_what_was_confirmed(),
        },
        Error::CheckoutRefused { why } => match why {
            CheckoutRefusal::InProgress(operation) => return checkout_refused_in(operation),
            CheckoutRefusal::NotWhatWasConfirmed => not_what_was_confirmed(),
        },
        Error::CheckoutChangedSinceConfirmed { what } => match what {
            CheckoutMoved::Head => "HEAD moved since you pressed Create and Checkout".to_owned(),
            CheckoutMoved::Commit { at } => {
                format!("Commit {} can no longer be read", at.short().as_str())
            }
            CheckoutMoved::Name { name } => {
                format!("A branch named '{}' exists now", shown_path(name))
            }
        },
        Error::LockRefused { why } => match why {
            LockRefusal::NoLock => "There is no index.lock any more".to_owned(),
            LockRefusal::NotAFile => "index.lock is not a file".to_owned(),
            LockRefusal::Unreadable(kind) => format!("index.lock could not be read ({kind})"),
            LockRefusal::GitRunning(_) => "Cairn is running git in this repository".to_owned(),
            LockRefusal::NotWhatWasConfirmed => not_what_was_confirmed(),
        },
        // A working-tree file a stage's or a discard's check could not read, or a file's diff
        // that could not be made: its path C-quoted, never the engine's text.
        Error::ReadWorkingTree { path, source } => {
            format!("{} could not be read ({source})", shown_path(path))
        }
        Error::DiffFile { path, source } => {
            format!("{} could not be read ({source})", shown_path(path))
        }
        Error::LockNotRemoved { path, source } => {
            format!("{} could not be removed ({source})", shown_path(path))
        }
        // Made, and then git failed: said apart from Fork's sentence, so
        // nobody makes the commit again.
        Error::MadeButGitFailed { verb, .. } => {
            return format!(
                "The {} was made, but git failed afterwards. The history shows it.",
                if *verb == "amended" {
                    "amend"
                } else {
                    "commit"
                }
            );
        }
        // git said it made the commit, and `HEAD` is something else: said as what is known, with
        // the history to look at.
        Error::CommitUnconfirmed { verb } => {
            return if *verb == "amended" {
                "Something else moved HEAD while the amend ran, so git may have amended another \
                 commit. The history shows HEAD as it is now."
                    .to_owned()
            } else {
                "Something else moved HEAD while the commit ran. The history shows HEAD as it is \
                 now."
                    .to_owned()
            };
        }
        Error::GitUnwatched { .. } => {
            return "Cairn lost hold of git while it ran. The lists and history show anything it \
                    had already done."
                .to_owned();
        }
        Error::GitFailed { .. } => return GIT_FAILED.to_owned(),
        Error::AmendNotMade { failure, .. } => return sentence(failure),
        other => {
            let shown = other.shown().to_string();
            let mut letters = shown.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(letters).collect()
            })
        }
    };
    if words.ends_with('.') {
        words
    } else {
        format!("{words}.")
    }
}

/// Why a write verb refused a path, naming it once.
fn refused(path: &str, why: Refusal) -> String {
    match why {
        Refusal::NothingSelected => "Nothing is selected".to_owned(),
        Refusal::WholeFileOnly => {
            format!("{path} is staged, unstaged or discarded only as a whole file")
        }
        Refusal::Conflicted => format!("{path} is conflicted, so it can only be staged whole"),
        Refusal::Submodule => {
            format!("{path} is a submodule; submodule changes can't be discarded")
        }
        Refusal::NestedRepository => {
            format!("{path} is a nested repository; nested repositories can't be discarded")
        }
        Refusal::NotAFile => format!("{path} is not a file"),
        Refusal::Obstructed => format!("A directory on {path} is a file or a symlink"),
        Refusal::NoUnstagedChange => format!("{path} has no unstaged change to discard"),
        Refusal::NotWhatWasConfirmed => not_what_was_confirmed(),
    }
}

fn not_what_was_confirmed() -> String {
    "What was asked is not what was confirmed".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every path in a sentence is C-quoted where it could reshape it — a newline, a
    /// right-to-left override — and each sentence is one, with one stop and no engine tail.
    /// Caught by: the raw path drawn, the engine's Display drawn, or a stop missing.
    #[test]
    fn a_sentence_names_its_path_escaped_and_ends_once() {
        for (path, shown) in [
            ("src/a.rs", "src/a.rs"),
            ("a\nb", r#""a\nb""#),
            ("a\u{202e}b", r#""a\342\200\256b""#),
        ] {
            let said = sentence(&Error::ChangedSinceConfirmed {
                path: path.to_owned(),
            });
            assert_eq!(said, format!("{shown} changed since you confirmed."));
            assert!(
                !said.contains('\n') && !said.contains('\u{202e}'),
                "{said:?}"
            );
            let said = sentence(&Error::Refused {
                path: path.to_owned(),
                why: Refusal::NotAFile,
            });
            assert_eq!(said, format!("{shown} is not a file."));
            let said = sentence(&Error::ReadWorkingTree {
                path: path.to_owned(),
                source: std::io::Error::other("denied"),
            });
            assert_eq!(said, format!("{shown} could not be read (denied)."));
            assert!(
                !said.contains('\n') && !said.contains('\u{202e}'),
                "{said:?}"
            );
            let said = sentence(&Error::DiffFile {
                path: path.to_owned(),
                source: Box::new(std::io::Error::other("corrupt")),
            });
            assert_eq!(said, format!("{shown} could not be read (corrupt)."));
        }
        assert_eq!(sentence(&Error::NoPaths), "No file was given.");
        assert_eq!(
            sentence(&Error::CommitRefused {
                why: CommitRefusal::NothingToAmend
            }),
            "There is no commit to amend yet."
        );
        assert_eq!(
            sentence(&Error::CheckoutRefused {
                why: CheckoutRefusal::InProgress(cairn_model::OperationInProgress::ApplyingPatches)
            }),
            "An am session is in progress. Finish or abort it first."
        );
        assert_eq!(
            sentence(&Error::CheckoutChangedSinceConfirmed {
                what: CheckoutMoved::Name {
                    name: "topic".to_owned()
                }
            }),
            "A branch named 'topic' exists now."
        );
        let other = sentence(&Error::CommitCancelledBeforeRunning);
        assert_eq!(other, "The commit was cancelled before git ran.");
    }

    /// A commit git made and then failed, and one git made while something else moved `HEAD`,
    /// each say what is known — never Fork's "unexpected error", which would have the person
    /// commit again. Caught by: the engine's text drawn, or the two sharing words.
    #[test]
    fn a_commit_made_then_failed_or_unconfirmed_says_so() {
        use std::os::unix::process::ExitStatusExt as _;
        let failure = Box::new(Error::GitFailed {
            arguments: "commit -q -F -".to_owned(),
            status: std::process::ExitStatus::from_raw(128 << 8),
            stderr: cairn_model::ScrubbedLines::new(),
            present_locks: Vec::new(),
        });
        assert_eq!(
            sentence(&Error::MadeButGitFailed {
                verb: "committed",
                failure
            }),
            "The commit was made, but git failed afterwards. The history shows it."
        );
        assert!(
            sentence(&Error::CommitUnconfirmed { verb: "amended" })
                .contains("git may have amended another commit")
        );
        assert!(
            sentence(&Error::CommitUnconfirmed { verb: "committed" })
                .starts_with("Something else moved HEAD while the commit ran.")
        );
    }
}
