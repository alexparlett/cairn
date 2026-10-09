//! The commit box as the window keeps it (staging-and-commit R10): the draft, Amend and the
//! draft it set aside, what the worker read for the box, the commit asked and its output, and
//! the Git Error dialog a failure opens. On the UI thread, so nothing here waits; what is read
//! arrives as updates (`commit_box_pane`'s `*_arrived`).
//!
//! **The draft is the window's** (R10.7): the subject and the description are two `State`s the
//! fields are bound to, kept by `LocalChangesView` for the window's life — through a refresh,
//! Local Changes hidden and shown, a failed hook and Amend's toggling. Ticking Amend sets the
//! draft aside and fills an empty one with `HEAD`'s message once it is read; unticking puts the
//! draft set aside back exactly as it was typed (R10.3). A commit that is made clears it, and an
//! amend unticks itself; one that fails keeps it.

use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Instant;

use cairn_model::{CommitHooks, Consequence, OperationInProgress};
use freya::prelude::*;

use crate::worker::{CommitReads, OperationId};

/// The most lines of a commit's output the window keeps for the Git Error dialog: the latest,
/// the oldest let go of first, so a hook that writes without end costs a bounded tail
/// (R10.5's "bounded to the retained tail").
pub const OUTPUT_LINES: usize = 10_000;
/// The most bytes of output kept, whatever the count of lines.
pub const OUTPUT_BYTES: usize = 1024 * 1024;

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
    /// The message it was asked with: what the skip commits again (R10.5).
    pub message: String,
    pub skip_hooks: bool,
}

/// What amending would replace, as last read while Amend is ticked.
#[derive(Debug, Clone)]
pub struct Amendable {
    /// Names this consequence: the amend button of another is another button.
    pub serial: u64,
    /// What the amend button confirms, or why it could not be read.
    pub consequence: Result<Rc<Consequence>, String>,
    /// Why amend's staged list could not be read, while the lists drawn stay the status's.
    pub lists_failed: Option<String>,
}

/// The Git Error dialog a failed commit opens (R10.5).
#[derive(Debug, Clone)]
pub struct GitError {
    pub serial: u64,
    /// What ran: `git commit -q -F -`.
    pub command: String,
    /// git's output, a hook's among it, ANSI sequences stripped.
    pub lines: Rc<Vec<String>>,
    /// Whether the skip is offered: a hook git would run exists, and the hooks were not skipped
    /// already.
    pub skip: bool,
    /// The commit that failed, for the skip to ask again.
    pub failed: AskedCommit,
}

/// A commit's output as it arrives, bounded to its latest [`OUTPUT_LINES`] and
/// [`OUTPUT_BYTES`].
#[derive(Debug, Default)]
pub struct OutputTail {
    lines: VecDeque<String>,
    bytes: usize,
}

impl OutputTail {
    /// One line more, ANSI sequences stripped; the oldest let go of past the bounds.
    pub fn push(&mut self, line: &str) {
        let line = strip_ansi(line);
        self.bytes += line.len();
        self.lines.push_back(line);
        while self.lines.len() > OUTPUT_LINES || (self.bytes > OUTPUT_BYTES && self.lines.len() > 1)
        {
            if let Some(dropped) = self.lines.pop_front() {
                self.bytes -= dropped.len();
            }
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.bytes = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The lines kept, oldest first.
    pub fn lines(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }
}

#[derive(Debug, Default)]
pub struct CommitBox {
    amend: bool,
    /// The draft set aside as Amend was ticked, put back exactly as it is unticked (R10.3).
    kept: Option<(String, String)>,
    /// Amend was ticked over an empty draft: `HEAD`'s message fills it once read, unless the
    /// person has typed meanwhile.
    fill_with_head: bool,
    /// The recent message the subject holds, by its place in the list (R10.2).
    recalled: Option<usize>,
    reads: Option<CommitReads>,
    /// The recent messages, shared with the box that draws them.
    recent: Rc<Vec<String>>,
    /// The merge whose message filled the draft: it fills an empty draft once per merge.
    merge_filled: bool,
    /// While Amend is ticked: what it would replace, once read; `reading` while a newer read
    /// is on its way.
    amendable: Option<Amendable>,
    reading_amend: bool,
    asked: Option<AskedCommit>,
    started: Option<Instant>,
    output: OutputTail,
    error: Option<GitError>,
    /// The serial the next consequence or dialog takes.
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

    /// The hooks git would run, as last read; none known when the read failed.
    pub fn hooks(&self) -> CommitHooks {
        self.reads
            .as_ref()
            .and_then(|reads| reads.hooks.as_ref().ok().copied())
            .unwrap_or_default()
    }

    pub fn amendable(&self) -> Option<&Amendable> {
        self.amendable.as_ref()
    }

    pub fn is_reading_amend(&self) -> bool {
        self.reading_amend
    }

    pub fn asked(&self) -> Option<&AskedCommit> {
        self.asked.as_ref()
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

    fn next_serial(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }

    /// Amend ticked: the draft set aside, and whether `HEAD`'s message is to fill it — only an
    /// empty one (R10.3).
    pub fn tick_amend(&mut self, subject: &str, description: &str) {
        if self.amend {
            return;
        }
        self.amend = true;
        self.kept = Some((subject.to_owned(), description.to_owned()));
        self.fill_with_head = subject.is_empty() && description.is_empty();
        self.amendable = None;
        self.reading_amend = true;
        self.recalled = None;
    }

    /// Amend unticked: the draft set aside, to put back exactly as it was typed.
    pub fn untick_amend(&mut self) -> Option<(String, String)> {
        if !self.amend {
            return None;
        }
        self.amend = false;
        self.fill_with_head = false;
        self.amendable = None;
        self.reading_amend = false;
        self.recalled = None;
        self.kept.take()
    }

    /// A newer amend read is on its way: the last one's consequence stays drawn meanwhile.
    pub fn amend_asked(&mut self) {
        self.reading_amend = true;
    }

    /// What amending would replace has been read: the consequence numbered afresh, and — when
    /// Amend was ticked over an empty draft that is still empty — `HEAD`'s message to fill it
    /// with.
    pub fn amend_arrived(
        &mut self,
        consequence: Result<Consequence, String>,
        message: Result<String, String>,
        lists_failed: Option<String>,
        draft_empty: bool,
    ) -> Option<String> {
        let serial = self.next_serial();
        self.amendable = Some(Amendable {
            serial,
            consequence: consequence.map(Rc::new),
            lists_failed,
        });
        self.reading_amend = false;
        let fill = std::mem::take(&mut self.fill_with_head);
        match message {
            Ok(message) if fill && draft_empty => Some(message),
            Ok(_) | Err(_) => None,
        }
    }

    /// What the box reads has arrived: kept, and — with a merge newly in progress and the draft
    /// empty — git's `MERGE_MSG` to fill it with, as git wrote it (R10.8, the user's decision E).
    pub fn reads_arrived(&mut self, reads: CommitReads, draft_empty: bool) -> Option<String> {
        self.recent = Rc::new(reads.recent.clone().unwrap_or_default());
        let fill = match &reads.operation {
            Some(OperationInProgress::Merge { message }) => {
                let first = !self.merge_filled;
                self.merge_filled = true;
                message
                    .clone()
                    .filter(|_| first && draft_empty && !self.amend)
            }
            Some(
                OperationInProgress::Rebase
                | OperationInProgress::ApplyingPatches
                | OperationInProgress::CherryPick
                | OperationInProgress::Revert,
            )
            | None => {
                self.merge_filled = false;
                None
            }
        };
        self.reads = Some(reads);
        fill
    }

    /// The recent message `at` is recalled into the subject (R10.2); `None` past the list.
    pub fn recall(&mut self, at: usize) -> Option<String> {
        let message = self.recent.get(at)?.clone();
        self.recalled = Some(at);
        Some(message)
    }

    /// The person typed: a recalled subject is theirs now.
    pub fn forget_recall(&mut self) {
        self.recalled = None;
    }

    /// `ask` was asked: drawn waiting until it starts.
    pub fn commit_asked(&mut self, asked: AskedCommit) {
        self.asked = Some(asked);
        self.started = None;
        self.output.clear();
        self.error = None;
    }

    /// The write `id` has started: when it is this box's commit, its elapsed time starts.
    pub fn started_now(&mut self, id: OperationId) {
        if self.asked.as_ref().is_some_and(|asked| asked.id == id) {
            self.started = Some(Instant::now());
        }
    }

    /// A line of the write `id`'s output: kept when it is this box's commit.
    pub fn output_arrived(&mut self, id: OperationId, line: &str) {
        if self.asked.as_ref().is_some_and(|asked| asked.id == id) {
            self.output.push(line);
        }
    }

    /// The write `id` has ended: this box's commit, given back so the caller decides what its
    /// ending does to the draft; `None` for another write.
    pub fn ended(&mut self, id: OperationId) -> Option<AskedCommit> {
        let asked = self.asked.take_if(|asked| asked.id == id)?;
        self.started = None;
        Some(asked)
    }

    /// A commit made: Amend unticks itself, the draft set aside let go of — it was committed or
    /// left for this one — and the recall forgotten (R10.3).
    pub fn made(&mut self) {
        self.amend = false;
        self.kept = None;
        self.fill_with_head = false;
        self.amendable = None;
        self.reading_amend = false;
        self.recalled = None;
    }

    /// A commit failed with git's own failure: the Git Error dialog opens over its output —
    /// what streamed, or else what the engine kept of it — the skip offered only where a hook
    /// exists and it was not already skipped (R10.5).
    pub fn failed(&mut self, failed: AskedCommit, command: String, kept: &str) {
        let lines = if self.output.is_empty() {
            kept.lines().map(strip_ansi).collect()
        } else {
            self.output.lines()
        };
        self.output.clear();
        let serial = self.next_serial();
        self.error = Some(GitError {
            serial,
            command,
            lines: Rc::new(lines),
            skip: self.hooks().skippable() && !failed.skip_hooks,
            failed,
        });
    }

    /// The dialog is answered: what failed, for a skip to ask again.
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

/// `line` without its terminal control sequences (R10.5; Fork Tracker #1218 prints them raw):
/// every CSI (`ESC [ … final`), OSC (`ESC ] … BEL` or `ESC \`) and two-byte escape, and every
/// other control character but a tab — a carriage return a progress meter writes among them.
pub fn strip_ansi(line: &str) -> String {
    let mut shown = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                // CSI: parameters and intermediates, ended by a byte in `@`..=`~`.
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                // OSC: ended by BEL or ST (`ESC \`).
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Any other escape: intermediates (` `..=`/`) and one final character, such as
                // a character set chosen (`ESC ( B`).
                Some(' '..='/') => {
                    for c in chars.by_ref() {
                        if !(' '..='/').contains(&c) {
                            break;
                        }
                    }
                }
                Some(_) | None => {}
            },
            '\t' => shown.push(c),
            c if c.is_control() => {}
            c => shown.push(c),
        }
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R10.8 and the user's decision E: `MERGE_MSG` as git wrote it — its `# Conflicts:` lines
    /// among it — is what the two fields hold, and the message they compose is the file's text
    /// unchanged; so is any subject-and-body message, a recalled one or `HEAD`'s. Caught by: the
    /// comment lines stripped, the blank separator kept in the description, or a body's own
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

    /// R10.5: a hook's coloured output reads as its text — colours, cursor moves and window
    /// titles gone, a carriage return gone, a tab kept. Caught by: an escape's tail left in
    /// (`[31m`), or an OSC swallowing the text after it.
    #[test]
    fn ansi_sequences_and_control_characters_are_stripped() {
        assert_eq!(
            strip_ansi("\u{1b}[1;31merror:\u{1b}[0m lint failed"),
            "error: lint failed"
        );
        assert_eq!(
            strip_ansi("\u{1b}]0;title\u{7}after\u{1b}]8;;url\u{1b}\\link"),
            "afterlink"
        );
        assert_eq!(strip_ansi("50%\r100%\tdone\u{1b}(B"), "50%100%\tdone");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    /// R10.5's bound: the output kept is the latest lines, however many a hook writes. Caught
    /// by: an unbounded buffer, or the newest lines dropped rather than the oldest.
    #[test]
    fn the_output_kept_is_its_latest_bounded_tail() {
        let mut tail = OutputTail::default();
        for n in 0..OUTPUT_LINES + 5 {
            tail.push(&format!("line {n}"));
        }
        let lines = tail.lines();
        assert_eq!(lines.len(), OUTPUT_LINES);
        assert_eq!(lines.first().map(String::as_str), Some("line 5"));
        assert_eq!(
            lines.last().map(String::as_str),
            Some(format!("line {}", OUTPUT_LINES + 4).as_str())
        );
        let mut wide = OutputTail::default();
        let long = "x".repeat(OUTPUT_BYTES / 2 + 1);
        for _ in 0..3 {
            wide.push(&long);
        }
        assert_eq!(wide.lines().len(), 1, "past the byte bound");
    }

    /// R10.3, the QA brief's first case: a draft typed, Amend ticked and unticked comes back
    /// exactly; an empty draft is filled with `HEAD`'s message, and only if still empty when it
    /// arrives. Caught by: the draft lost to `HEAD`'s message, or a typed amend message
    /// overwritten by the read arriving late.
    #[test]
    fn amend_sets_the_draft_aside_and_fills_only_an_empty_one() {
        let mut state = CommitBox::default();
        state.tick_amend("Typed subject", "typed body\n");
        assert!(state.is_amending());
        assert_eq!(
            state.amend_arrived(Err("x".to_owned()), Ok("HEAD's".to_owned()), None, false),
            None,
            "a typed draft was filled"
        );
        assert_eq!(
            state.untick_amend(),
            Some(("Typed subject".to_owned(), "typed body\n".to_owned()))
        );

        state.tick_amend("", "");
        assert_eq!(
            state.amend_arrived(Err("x".to_owned()), Ok("HEAD's".to_owned()), None, true),
            Some("HEAD's".to_owned())
        );
        // A later read never fills again.
        assert_eq!(
            state.amend_arrived(Err("x".to_owned()), Ok("HEAD's".to_owned()), None, true),
            None
        );
        assert_eq!(state.untick_amend(), Some((String::new(), String::new())));

        // Typed over meanwhile: the read finds the draft no longer empty.
        state.tick_amend("", "");
        assert_eq!(
            state.amend_arrived(Err("x".to_owned()), Ok("HEAD's".to_owned()), None, false),
            None
        );
    }

    /// R10.8: a merge fills an empty draft with `MERGE_MSG` once, as git wrote it, and never
    /// over a draft or while amending; a merge ended and another begun fills again. Caught by:
    /// a draft overwritten on every refresh during a merge.
    #[test]
    fn a_merge_fills_an_empty_draft_once() {
        let reads = |operation| CommitReads {
            operation,
            hooks: Ok(CommitHooks::default()),
            recent: Ok(vec!["Last\n".to_owned()]),
        };
        let merge = || {
            Some(OperationInProgress::Merge {
                message: Some("Merge branch 'x'\n\n# Conflicts:\n#\ta\n".to_owned()),
            })
        };
        let mut state = CommitBox::default();
        assert_eq!(
            state.reads_arrived(reads(merge()), false),
            None,
            "a typed draft was filled"
        );
        let mut state = CommitBox::default();
        assert_eq!(
            state.reads_arrived(reads(merge()), true).as_deref(),
            Some("Merge branch 'x'\n\n# Conflicts:\n#\ta\n")
        );
        assert_eq!(
            state.reads_arrived(reads(merge()), true),
            None,
            "filled twice"
        );
        assert_eq!(state.recent().as_slice(), ["Last\n"]);
        let _ = state.reads_arrived(reads(None), true);
        assert!(state.reads_arrived(reads(merge()), true).is_some());
    }

    /// R10.5: the skip is offered only where a hook git would run exists, and never for a
    /// commit whose hooks were already skipped; the dialog shows what streamed, or else the
    /// engine's kept output, ANSI stripped. Caught by: a skip offered with no hook, or offered
    /// again after the skip (an endless `--no-verify` loop).
    #[test]
    fn a_failure_offers_the_skip_only_where_a_hook_exists() {
        let asked = |skip_hooks| AskedCommit {
            id: OperationId::for_tests(3),
            amend: false,
            message: "m".to_owned(),
            skip_hooks,
        };
        let mut state = CommitBox::default();
        state.commit_asked(asked(false));
        state.output_arrived(OperationId::for_tests(3), "\u{1b}[31mlint\u{1b}[0m failed");
        state.output_arrived(OperationId::for_tests(4), "another write's line");
        let failed = state.ended(OperationId::for_tests(3));
        assert_eq!(failed, Some(asked(false)));
        state.failed(asked(false), "git commit -q -F -".to_owned(), "kept");
        let error = state.error().cloned();
        assert_eq!(
            error.as_ref().map(|error| error.lines.as_slice().to_vec()),
            Some(vec!["lint failed".to_owned()])
        );
        assert_eq!(
            error.map(|error| error.skip),
            Some(false),
            "no hook, yet a skip"
        );

        let _ = state.reads_arrived(
            CommitReads {
                operation: None,
                hooks: Ok(CommitHooks {
                    pre_commit: true,
                    commit_msg: false,
                }),
                recent: Ok(Vec::new()),
            },
            false,
        );
        state.failed(asked(false), "git commit -q -F -".to_owned(), "a\nb");
        let error = state.error().cloned();
        assert_eq!(error.as_ref().map(|error| error.skip), Some(true));
        assert_eq!(
            error.map(|error| error.lines.as_slice().to_vec()),
            Some(vec!["a".to_owned(), "b".to_owned()])
        );
        state.failed(
            asked(true),
            "git commit -q --no-verify -F -".to_owned(),
            "x",
        );
        assert_eq!(state.error().map(|error| error.skip), Some(false));
    }
}
