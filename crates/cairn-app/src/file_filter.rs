//! The Changes tab's filter as the window holds it (PRD R5.4): the text, what was last asked
//! of a worker, and the files the list shows — for the change set selected now and no other.
//!
//! Matching is a pass over every path (55,184 on the largest subject), so it is never done
//! here: a text is asked of a worker ([`Request::FilterFiles`], numbered in the file-filter
//! lane so the next keystroke supersedes it) and its answer, the matching files' indices, is
//! kept only if it names the change set and the text selected now. Until it arrives the list
//! keeps showing the last answer for the same change set — a narrower or wider filter of the
//! same files — and, for another change set, nothing, so no index of one change set is ever
//! read against another.

use std::sync::Arc;

use cairn_model::ChangeSet;
use cairn_ui::ShownFiles;

use crate::worker::{Comparison, Request};

/// The filter's text and what it shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileFilter {
    text: String,
    /// The change set and the text asked of a worker last.
    asked: Option<(Comparison, String)>,
    /// The change set and the text the answer shown is for.
    answered: Option<(Comparison, String)>,
    /// What the list shows; `Filtered` with no file while the first answer for a change set
    /// is on its way.
    shown: ShownFiles,
}

impl FileFilter {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn shown(&self) -> &ShownFiles {
        &self.shown
    }

    /// Another change set was selected: what was shown was another change set's files.
    pub fn changes_selected(&mut self) {
        self.asked = None;
        self.answered = None;
        self.shown = if self.text.is_empty() {
            ShownFiles::All
        } else {
            ShownFiles::Filtered(Vec::new())
        };
    }

    /// The text is now `text`. Returns what to ask of a worker for `of`'s change set `files`,
    /// when one is ready and the text filters anything; an empty text shows every file at
    /// once, and the text already asked for asks nothing.
    pub fn filter(
        &mut self,
        text: &str,
        ready: Option<(Comparison, &Arc<ChangeSet>)>,
    ) -> Option<Request> {
        if self.text != text {
            self.text = text.to_owned();
        }
        if self.text.is_empty() {
            self.asked = None;
            self.answered = None;
            self.shown = ShownFiles::All;
            return None;
        }
        let (of, files) = ready?;
        if self
            .asked
            .as_ref()
            .is_some_and(|(asked, text)| *asked == of && *text == self.text)
        {
            return None;
        }
        self.asked = Some((of, self.text.clone()));
        Some(Request::FilterFiles {
            of,
            files: Arc::clone(files),
            text: self.text.clone(),
        })
    }

    /// Whether an answer for `text` over `of` is the one asked for last.
    pub fn wants(&self, of: Comparison, text: &str) -> bool {
        self.asked
            .as_ref()
            .is_some_and(|(asked, asked_text)| *asked == of && asked_text == text)
    }

    /// An answer arrived; kept only if it is the one asked for last. Returns whether it was.
    pub fn arrived(&mut self, of: Comparison, text: &str, files: Vec<u32>) -> bool {
        if !self.wants(of, text) {
            return false;
        }
        self.answered = self.asked.clone();
        self.shown = ShownFiles::Filtered(files);
        true
    }

    /// Whether what is shown is an answer for the text as it is — not a placeholder, and not
    /// an earlier text's answer still standing.
    pub fn is_settled(&self) -> bool {
        if self.text.is_empty() {
            return true;
        }
        self.asked.is_some() && self.answered == self.asked
    }
}

#[cfg(test)]
mod tests {
    use cairn_model::{ChangeStatus, ChangedFile, Oid, RenameDetection, RepoPath};

    use super::*;

    fn commit(n: u8) -> Comparison {
        Comparison::Commit(Oid::from_bytes(&[n; 20]).unwrap())
    }

    fn files() -> Arc<ChangeSet> {
        Arc::new(ChangeSet {
            files: ["a.rs", "b.md"]
                .map(|path| ChangedFile {
                    status: ChangeStatus::Modified,
                    old_path: RepoPath::from(path),
                    new_path: RepoPath::from(path),
                    old_mode: None,
                    new_mode: None,
                    old_id: None,
                    new_id: None,
                })
                .to_vec(),
            details: None,
            renames: RenameDetection::default(),
        })
    }

    /// R5.4: a text is asked of a worker once per change set, an answer is kept only for the
    /// change set and text asked last, the last answer stands while the next is on its way,
    /// another change set shows nothing of the last one's, and an empty text shows every file
    /// without asking. Caught by: asking again for the same text (a keystroke that changes
    /// nothing reruns a 55,184-path scan), keeping a superseded text's answer, or showing one
    /// change set's indices over another's.
    #[test]
    fn the_filter_shows_the_answer_for_the_change_set_and_text_asked_last() {
        let mut filter = FileFilter::default();
        let set = files();
        assert_eq!(filter.filter("", Some((commit(1), &set))), None);
        assert_eq!(filter.shown(), &ShownFiles::All);
        assert!(filter.is_settled());

        let asked = filter.filter("rs", Some((commit(1), &set)));
        assert!(matches!(asked, Some(Request::FilterFiles { ref text, .. }) if text == "rs"));
        assert_eq!(
            filter.filter("rs", Some((commit(1), &set))),
            None,
            "asked twice"
        );
        assert!(!filter.is_settled());
        assert!(
            !filter.arrived(commit(2), "rs", vec![1]),
            "another commit's"
        );
        assert!(
            !filter.arrived(commit(1), "r", vec![0, 1]),
            "another text's"
        );
        assert!(filter.arrived(commit(1), "rs", vec![0]));
        assert_eq!(filter.shown(), &ShownFiles::Filtered(vec![0]));
        assert!(filter.is_settled());

        // A wider text: the narrower answer stands until its own arrives.
        assert!(filter.filter("r", Some((commit(1), &set))).is_some());
        assert_eq!(filter.shown(), &ShownFiles::Filtered(vec![0]));
        assert!(!filter.is_settled());
        assert!(
            !filter.arrived(commit(1), "rs", vec![0]),
            "a superseded text's"
        );

        // Another commit: nothing of the last one's is shown, and its own is asked.
        filter.changes_selected();
        assert_eq!(filter.shown(), &ShownFiles::Filtered(Vec::new()));
        assert!(filter.filter("r", Some((commit(2), &set))).is_some());

        // Nothing ready: nothing to ask yet.
        filter.changes_selected();
        assert_eq!(filter.filter("r", None), None);

        assert_eq!(filter.filter("", None), None);
        assert_eq!(filter.shown(), &ShownFiles::All);
    }
}
