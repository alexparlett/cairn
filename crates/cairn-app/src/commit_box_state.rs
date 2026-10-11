//! The commit box as the window keeps it (staging-and-commit R10): the draft and whether the
//! person has touched it, Amend and the draft it set aside, what the worker read for the box,
//! the commit asked and its output, and the Git Error dialog a failure opens — with, for a
//! confirmed amend git refused before amending, the token it handed back for the skip. On the
//! UI thread, so nothing here waits; what is read arrives as updates (`commit_box_pane`'s
//! `*_arrived`).
//!
//! **The draft is the window's** (R10.7): the subject and the description are two `State`s the
//! fields are bound to, kept by `LocalChangesView` for the window's life — through a refresh,
//! Local Changes hidden and shown, a failed hook and Amend's toggling. **One rule fills it**
//! (C7, Fork's developer's rule, Tracker #61): a fill — `HEAD`'s message as Amend is ticked, a
//! merge's, cherry-pick's or revert's cleaned message — goes only into an empty draft the person
//! has not touched, and any edit, clearing included, makes it theirs ([`CommitBox::touch`]).
//! Ticking Amend sets the draft aside, its touched bit with it; unticking puts it back exactly.
//! A commit that is made clears the draft it took, untouched again, and an amend unticks
//! itself; one that fails keeps it.

use std::rc::Rc;
use std::time::Instant;

use cairn_model::{Confirmed, Consequence, OperationInProgress, ScrubbedLines};
use freya::prelude::*;

use crate::output_tail::OutputTail;
use crate::shown_output::{shown_lines, spoken_lines};
use crate::worker::{CommitReads, OperationId};

/// The window's handles on the box: the draft, bound to its fields, and the rest of its state.
#[derive(Clone, Copy, PartialEq)]
pub struct CommitBoxView {
    pub subject: State<String>,
    pub description: State<String>,
    pub state: State<CommitBox>,
}

impl CommitBoxView {
    /// Each handle made by its hook, in the window's root.
    pub fn used() -> Self {
        Self {
            subject: use_state(String::new),
            description: use_state(String::new),
            state: use_state(CommitBox::default),
        }
    }

    /// Each handle made outside a component, for a test's view.
    #[cfg(test)]
    pub fn created() -> Self {
        Self {
            subject: State::create(String::new()),
            description: State::create(String::new()),
            state: State::create(CommitBox::default()),
        }
    }
}

/// A commit or an amend the box asked for, until it ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskedCommit {
    pub id: OperationId,
    pub amend: bool,
    /// The message it was asked with: what the skip commits again (R10.5), what the dialog's
    /// confirmed amend runs with, and what a commit made clears from the draft — only that.
    pub message: String,
    pub skip_hooks: bool,
}

/// The Git Error dialog a failed commit or amend opens (R10.5).
#[derive(Debug)]
pub struct GitError {
    pub serial: u64,
    /// What ran: `git commit -q -F -`.
    pub command: String,
    /// git's output, a hook's among it, ANSI sequences stripped.
    pub lines: Rc<Vec<String>>,
    /// Whether the skip is offered: on every failed commit or amend whose hooks were not skipped
    /// already (C4 — Cairn keeps no hook model, so a failure no hook caused fails the same way
    /// again, harmlessly).
    pub skip: bool,
    /// The commit that failed, for the skip to ask again.
    pub failed: AskedCommit,
    /// The dialog's sentence: Fork's, or that git made the commit and then failed (R14.3).
    pub sentence: String,
    /// git could not create `index.lock`: `Remove index.lock…` beside Close (R12.4).
    pub index_lock: bool,
    /// A confirmed amend's token git's failure handed back unspent (R1.1's option (a), the
    /// user's answer of 2026-10-10): kept until the skip spends it or Close lets it go, so the
    /// skip amends without asking again. `None` for every other failure.
    unspent: Option<Confirmed>,
}

impl GitError {
    /// The token the skip spends, taken once.
    pub fn take_unspent(&mut self) -> Option<Confirmed> {
        self.unspent.take()
    }
}

/// A draft set aside as Amend was ticked: its two fields and whether the person had touched it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Kept {
    subject: String,
    description: String,
    touched: bool,
}

#[derive(Debug, Default)]
pub struct CommitBox {
    amend: bool,
    /// The draft set aside as Amend was ticked, put back exactly as it is unticked (R10.3).
    kept: Option<Kept>,
    /// The person has edited the draft — typed, cleared, recalled a message — so no fill goes
    /// into it (C7).
    touched: bool,
    /// The recent message the subject holds, by its place in the list (R10.2).
    recalled: Option<usize>,
    reads: Option<CommitReads>,
    /// The recent messages, shared with the box that draws them.
    recent: Rc<Vec<String>>,
    asked: Option<AskedCommit>,
    /// Cancel was pressed for the commit asked: "Cancelling…" until it ends.
    cancelling: bool,
    started: Option<Instant>,
    /// Its commit's output as it arrives: the latest, in the one bounded tail (R10.5).
    output: OutputTail<String>,
    error: Option<GitError>,
    /// An amend at the press that must be confirmed: its consequence and the amend pressed, until
    /// the box opens the confirmation dialog on it (R10.6).
    confirming: Option<(Consequence, AskedCommit)>,
    /// The serial the next dialog takes.
    serial: u64,
}

impl CommitBox {
    pub fn is_amending(&self) -> bool {
        self.amend
    }

    pub fn recent(&self) -> &Rc<Vec<String>> {
        &self.recent
    }

    /// The operation in progress, as last read.
    pub fn operation(&self) -> Option<&OperationInProgress> {
        self.reads.as_ref()?.operation.as_ref()
    }

    pub fn asked(&self) -> Option<&AskedCommit> {
        self.asked.as_ref()
    }

    pub fn is_cancelling(&self) -> bool {
        self.cancelling
    }

    pub fn started(&self) -> Option<Instant> {
        self.started
    }

    pub fn error(&self) -> Option<&GitError> {
        self.error.as_ref()
    }

    pub fn recalled(&self) -> Option<usize> {
        self.recalled
    }

    pub fn is_touched(&self) -> bool {
        self.touched
    }

    fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    /// The person edited the draft: it is theirs, and a recalled subject is theirs too.
    pub fn touch(&mut self) {
        self.touched = true;
        self.recalled = None;
    }

    /// Whether a fill may go into the draft: it is empty and the person has not touched it (C7).
    fn fillable(&self, draft_empty: bool) -> bool {
        draft_empty && !self.touched
    }

    /// Amend ticked: the draft set aside with its touched bit, the fields left as they are.
    pub fn tick_amend(&mut self, subject: &str, description: &str) {
        if self.amend {
            return;
        }
        self.amend = true;
        self.kept = Some(Kept {
            subject: subject.to_owned(),
            description: description.to_owned(),
            touched: self.touched,
        });
        self.recalled = None;
    }

    /// Amend unticked: the draft set aside, to put back exactly as it was typed.
    pub fn untick_amend(&mut self) -> Option<(String, String)> {
        if !self.amend {
            return None;
        }
        self.amend = false;
        self.recalled = None;
        let kept = self.kept.take()?;
        self.touched = kept.touched;
        Some((kept.subject, kept.description))
    }

    /// `HEAD`'s message, read for amend's lists while Amend is ticked: to fill the draft with when
    /// it is empty and untouched (R10.3, C7).
    pub fn amend_message_arrived(&self, message: String, draft_empty: bool) -> Option<String> {
        (self.amend && self.fillable(draft_empty)).then_some(message)
    }

    /// What the box reads has arrived: kept, and — with a merge, a single cherry-pick or a
    /// single revert in progress and the draft empty and untouched — the message git prepared
    /// for it, cleaned as git's editor cleans it, to fill it with (R10.8, R6.10, C7).
    pub fn reads_arrived(&mut self, reads: CommitReads, draft_empty: bool) -> Option<String> {
        self.recent = Rc::new(reads.recent.clone().unwrap_or_default());
        let fill = match reads.operation {
            Some(operation) if operation.concluded_by_commit() => reads
                .message
                .clone()
                .ok()
                .flatten()
                .filter(|_| self.fillable(draft_empty) && !self.amend),
            Some(_) | None => None,
        };
        self.reads = Some(reads);
        fill
    }

    /// The recent message `at` is recalled into the subject (R10.2); `None` past the list. The
    /// person chose it: the draft is theirs.
    pub fn recall(&mut self, at: usize) -> Option<String> {
        let message = self.recent.get(at)?.clone();
        self.touched = true;
        self.recalled = Some(at);
        Some(message)
    }

    /// The recall let go of: ↓ from the newest back to an empty draft.
    pub fn forget_recall(&mut self) {
        self.recalled = None;
    }

    /// `ask` was asked: drawn waiting until it starts.
    pub fn commit_asked(&mut self, asked: AskedCommit) {
        self.asked = Some(asked);
        self.cancelling = false;
        self.started = None;
        self.output.clear();
        self.error = None;
    }

    /// Cancel was pressed for the commit running: "Cancelling…" until git is reaped (R10.4).
    pub fn cancel_asked(&mut self) {
        if self.asked.is_some() {
            self.cancelling = true;
        }
    }

    /// The write `id` has started: when it is this box's commit, its elapsed time starts.
    pub fn started_now(&mut self, id: OperationId) {
        if self.asked.as_ref().is_some_and(|asked| asked.id == id) {
            self.started = Some(Instant::now());
        }
    }

    /// A read of the write `id`'s output: its spoken lines kept when it is this box's commit.
    pub fn output_arrived(&mut self, id: OperationId, lines: &ScrubbedLines) {
        if self.asked.as_ref().is_some_and(|asked| asked.id == id) {
            self.output.extend(spoken_lines(lines));
        }
    }

    /// The write `id` has ended: this box's commit, given back so the caller decides what its
    /// ending does to the draft; `None` for another write.
    pub fn ended(&mut self, id: OperationId) -> Option<AskedCommit> {
        let asked = self.asked.take_if(|asked| asked.id == id)?;
        self.started = None;
        self.cancelling = false;
        Some(asked)
    }

    /// A commit made: Amend unticks itself, the draft set aside let go of — it was committed or
    /// left for this one — and the recall forgotten (R10.3). `taken`: the draft is the one it
    /// committed, cleared, and untouched again.
    pub fn made(&mut self, taken: bool) {
        self.amend = false;
        self.kept = None;
        self.recalled = None;
        if taken {
            self.touched = false;
        }
    }

    /// A commit or amend failed with git's own failure: the Git Error dialog opens over its
    /// output — what streamed, or else what the engine kept of it — the skip offered on every
    /// failure whose hooks were not already skipped (R10.5 as amended, C4), with the token a
    /// confirmed amend's failure handed back, which only the skip spends.
    pub fn failed(
        &mut self,
        failed: AskedCommit,
        failure: &crate::worker::Failure,
        unspent: Option<Confirmed>,
    ) {
        let lines = if self.output.is_empty() {
            shown_lines(&failure.output)
        } else {
            self.output.iter().cloned().collect()
        };
        self.output.clear();
        let serial = self.next_serial();
        self.error = Some(GitError {
            serial,
            command: failure.command.clone().unwrap_or_default(),
            lines: Rc::new(lines),
            skip: !failed.skip_hooks,
            failed,
            sentence: failure.said.clone(),
            index_lock: failure.index_lock,
            unspent,
        });
    }

    /// An amend at the press ended asking to be confirmed: the dialog is to open on `consequence`,
    /// asking `asked`'s message again under its token (R10.6). The draft is untouched.
    pub fn needs_confirming(&mut self, consequence: Consequence, asked: AskedCommit) {
        self.confirming = Some((consequence, asked));
    }

    /// Whether a confirmation waits to be opened.
    pub fn is_confirming(&self) -> bool {
        self.confirming.is_some()
    }

    /// The confirmation to open, taken once.
    pub fn take_confirming(&mut self) -> Option<(Consequence, AskedCommit)> {
        self.confirming.take()
    }

    /// The dialog is answered: what failed — and the token it kept — for a skip to ask again, or
    /// for Close to let go of.
    pub fn close_error(&mut self) -> Option<GitError> {
        self.error.take()
    }
}

/// A message split as the box draws it (R10.2, R10.3, R10.8): the subject is its first line;
/// the description the rest, after the one blank line that separates them — so that
/// [`compose_message`] gives the text back as written.
pub fn split_message(message: &str) -> (String, String) {
    match message.split_once('\n') {
        None => (message.to_owned(), String::new()),
        Some((subject, rest)) => {
            let description = rest.strip_prefix('\n').unwrap_or(rest);
            (subject.to_owned(), description.to_owned())
        }
    }
}

/// The message a commit is asked with: the subject, then — when there is one — a blank line
/// and the description, as typed. git's own cleanup (`commit.cleanup`, `-F`'s default) is
/// git's to apply (R6.1).
pub fn compose_message(subject: &str, description: &str) -> String {
    if description.is_empty() {
        subject.to_owned()
    } else {
        format!("{subject}\n\n{description}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A message split into the two fields composes back unchanged — one with `#` lines among it
    /// (a person may type them; `git commit -F` keeps them), a recalled one or `HEAD`'s. Caught
    /// by: the comment lines stripped, the blank separator kept in the description, or a body's own
    /// blank lines folded.
    #[test]
    fn a_message_split_into_the_fields_composes_back_unchanged() {
        let merge = "Merge branch 'feature'\n\n# Conflicts:\n#\ta.txt\n#\tb.txt\n";
        let (subject, description) = split_message(merge);
        assert_eq!(subject, "Merge branch 'feature'");
        assert_eq!(description, "# Conflicts:\n#\ta.txt\n#\tb.txt\n");
        assert_eq!(compose_message(&subject, &description), merge);
        for message in [
            "Fix the parser\n\nIt read past the end.\n\nTwice.",
            "One line",
            "Subject\n\n  indented body",
        ] {
            let (subject, description) = split_message(message);
            assert_eq!(
                compose_message(&subject, &description),
                message,
                "{message:?}"
            );
        }
        assert_eq!(compose_message("Only a subject", ""), "Only a subject");
    }

    /// R10.5's bound: the output kept is the latest lines of the one bounded tail, however many
    /// a hook writes. Caught by: an unbounded buffer, or the newest lines dropped rather than
    /// the oldest.
    #[test]
    fn the_output_kept_is_its_latest_bounded_tail() {
        use crate::output_tail::OUTPUT_LINES;
        let id = OperationId::for_tests(1);
        let asked = AskedCommit {
            id,
            amend: false,
            message: "m".to_owned(),
            skip_hooks: false,
        };
        let mut state = CommitBox::default();
        state.commit_asked(asked.clone());
        let lines: Vec<String> = (0..OUTPUT_LINES + 5).map(|n| format!("line {n}")).collect();
        state.output_arrived(id, &ScrubbedLines::scrubbing(&lines.join("\n")));
        state.ended(id);
        state.failed(
            asked,
            &crate::worker::Failure {
                said: "failed".to_owned(),
                command: None,
                output: ScrubbedLines::new(),
                locks: Vec::new(),
                index_lock: false,
            },
            None,
        );
        let shown = state.error().map(|error| error.lines.clone());
        let shown = shown.as_deref().map(Vec::as_slice).unwrap_or_default();
        assert_eq!(shown.len(), OUTPUT_LINES);
        assert_eq!(shown.first().map(String::as_str), Some("line 5"));
        assert_eq!(
            shown.last().map(String::as_str),
            Some(format!("line {}", OUTPUT_LINES + 4).as_str())
        );
    }

    /// R10.3, C7 and the QA brief's first case: a draft typed, Amend ticked and unticked comes
    /// back exactly with its touched bit; `HEAD`'s message fills only an empty, untouched draft,
    /// and a fill the person cleared stays cleared. Caught by: a typed draft filled, a cleared
    /// fill filled again, or the touched bit lost across Amend.
    #[test]
    fn amend_sets_the_draft_aside_and_fills_only_an_empty_untouched_one() {
        let mut state = CommitBox::default();
        state.touch();
        state.tick_amend("Typed subject", "typed body\n");
        assert!(state.is_amending());
        assert_eq!(
            state.amend_message_arrived("HEAD's".to_owned(), false),
            None
        );
        assert_eq!(
            state.untick_amend(),
            Some(("Typed subject".to_owned(), "typed body\n".to_owned()))
        );
        assert!(state.is_touched(), "the draft's touched bit was lost");

        // Typed and then cleared: empty, but the person's — no fill.
        state.tick_amend("", "");
        assert_eq!(
            state.amend_message_arrived("HEAD's".to_owned(), true),
            None,
            "a cleared draft was filled"
        );
        let _ = state.untick_amend();

        // An untouched empty draft is filled; cleared after, it stays cleared.
        let mut state = CommitBox::default();
        state.tick_amend("", "");
        assert_eq!(
            state.amend_message_arrived("HEAD's".to_owned(), true),
            Some("HEAD's".to_owned())
        );
        state.touch();
        assert_eq!(state.amend_message_arrived("HEAD's".to_owned(), true), None);
        assert_eq!(state.untick_amend(), Some((String::new(), String::new())));
        assert!(!state.is_touched(), "the set-aside draft came back touched");
        // Not amending: no amend fill at all.
        assert_eq!(state.amend_message_arrived("HEAD's".to_owned(), true), None);
    }

    /// R10.8 and C7: a merge fills an empty, untouched draft with the message git prepared,
    /// cleaned (R6.10), and never a draft the person touched — a fill they cleared stays cleared
    /// — nor while amending; a single cherry-pick fills the same way, and a rebase never; a
    /// commit made of the draft makes it untouched again. Caught by: a draft overwritten on every
    /// refresh, a cleared fill filled again, or a refusing operation's message filled.
    #[test]
    fn a_merge_fills_an_empty_untouched_draft() {
        let reads = |operation| CommitReads {
            operation,
            message: Ok(Some("Merge branch 'x'\n".to_owned())),
            recent: Ok(vec!["Last\n".to_owned()]),
        };
        let merge = || Some(OperationInProgress::Merge);
        let mut state = CommitBox::default();
        assert_eq!(
            state.reads_arrived(reads(merge()), false),
            None,
            "a typed draft was filled"
        );
        assert_eq!(
            state.reads_arrived(reads(merge()), true).as_deref(),
            Some("Merge branch 'x'\n")
        );
        assert_eq!(state.recent().as_slice(), ["Last\n"]);
        state.touch();
        assert_eq!(
            state.reads_arrived(reads(merge()), true),
            None,
            "a cleared fill was filled again"
        );
        state.made(true);
        assert!(state.reads_arrived(reads(merge()), true).is_some());
        let mut state = CommitBox::default();
        let pick = Some(OperationInProgress::CherryPick { picked: None });
        assert!(
            state.reads_arrived(reads(pick), true).is_some(),
            "a pick's message"
        );
        let mut state = CommitBox::default();
        assert_eq!(
            state.reads_arrived(reads(Some(OperationInProgress::Rebase)), true),
            None,
            "a rebase's message filled"
        );
        let mut state = CommitBox::default();
        state.tick_amend("", "");
        assert_eq!(
            state.reads_arrived(reads(merge()), true),
            None,
            "filled amending"
        );
    }

    /// R10.5 as C4 amended it: the skip is offered on every failed commit — no hook model —
    /// and never for one whose hooks were already skipped; the dialog shows what streamed, or
    /// else the engine's kept output, ANSI stripped. Caught by: a skip withheld for want of a
    /// hook Cairn found, or offered again after the skip (an endless `--no-verify` loop).
    #[test]
    fn a_failure_offers_the_skip_unless_it_was_skipped() {
        let asked = |skip_hooks| AskedCommit {
            id: OperationId::for_tests(3),
            amend: false,
            message: "m".to_owned(),
            skip_hooks,
        };
        let mut state = CommitBox::default();
        state.commit_asked(asked(false));
        state.output_arrived(
            OperationId::for_tests(3),
            &ScrubbedLines::scrubbing("\u{1b}[31mlint\u{1b}[0m failed"),
        );
        state.output_arrived(
            OperationId::for_tests(4),
            &ScrubbedLines::scrubbing("another write's line"),
        );
        let failed = state.ended(OperationId::for_tests(3));
        assert_eq!(failed, Some(asked(false)));
        state.failed(
            asked(false),
            &crate::worker::Failure {
                said: cairn_ui::GIT_ERROR_TEXT.to_owned(),
                command: Some("git commit -q -F -".to_owned()),
                output: ScrubbedLines::scrubbing("kept"),
                locks: Vec::new(),
                index_lock: false,
            },
            None,
        );
        let error = state.error();
        assert_eq!(
            error.map(|error| error.lines.as_slice().to_vec()),
            Some(vec!["lint failed".to_owned()])
        );
        assert_eq!(
            error.map(|error| error.skip),
            Some(true),
            "a failure, yet no skip"
        );
        state.failed(
            asked(false),
            &crate::worker::Failure {
                said: cairn_ui::GIT_ERROR_TEXT.to_owned(),
                command: Some("git commit -q -F -".to_owned()),
                output: ScrubbedLines::scrubbing("a\nb"),
                locks: Vec::new(),
                index_lock: false,
            },
            None,
        );
        let error = state.error();
        assert_eq!(error.map(|error| error.skip), Some(true));
        assert_eq!(
            error.map(|error| error.lines.as_slice().to_vec()),
            Some(vec!["a".to_owned(), "b".to_owned()])
        );
        state.failed(
            asked(true),
            &crate::worker::Failure {
                said: cairn_ui::GIT_ERROR_TEXT.to_owned(),
                command: Some("git commit -q --no-verify -F -".to_owned()),
                output: ScrubbedLines::scrubbing("x"),
                locks: Vec::new(),
                index_lock: false,
            },
            None,
        );
        assert_eq!(state.error().map(|error| error.skip), Some(false));
    }
}
