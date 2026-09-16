//! The sentences the window shows about the history, as values.
//!
//! R4.3 is a rule about what a reader SEES: a list that is still loading must
//! not look like a repository with no commits in it. `history_state` decides
//! that those are different states; this decides that they are different
//! sentences, which is the half a state machine alone leaves to a screenshot.
//! Keeping it here rather than inline in the render is what lets a test fail
//! when the two arms are given the same words.

use crate::history_state::{Progress, Status};

/// What fills the list's place, or `None` when the list itself is what to draw.
///
/// `has_rows` is the reason a failure can be either: a page that failed after
/// rows were drawn leaves the rows on screen and says so in a banner, while one
/// that failed before any arrived has nothing to leave.
pub fn placeholder(status: &Status, has_rows: bool) -> Option<String> {
    match status {
        Status::Loading => Some("Reading history…".to_owned()),
        Status::Empty => Some("No commits yet.".to_owned()),
        Status::Failed(message) if !has_rows => Some(message.clone()),
        Status::Failed(_) | Status::Ready => None,
    }
}

/// How much of the history is loaded, as the title bar says it.
///
/// An ellipsis while more is coming, because "2,540 commits" and "2,540 commits
/// so far" are different claims and only one of them is true mid-scroll. A
/// history that stopped because something failed is not still coming, so it
/// gets no ellipsis either — the banner says what happened.
pub fn loaded_count(progress: &Progress) -> String {
    let loaded = progress.loaded();
    let noun = if loaded == 1 { "commit" } else { "commits" };
    let more_coming = !progress.complete() && !matches!(progress.status(), Status::Failed(_));

    if more_coming {
        format!("{loaded} {noun}…")
    } else {
        format!("{loaded} {noun}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R4.3, as the reader meets it. The bug this exists to prevent is that the
    /// two states render the same, and a test that checked only that each state
    /// has *a* sentence would not catch it — so the assertion is that they are
    /// DIFFERENT sentences, and that neither is empty.
    #[test]
    fn loading_and_an_empty_repository_do_not_say_the_same_thing() {
        let loading = placeholder(&Status::Loading, false);
        let empty = placeholder(&Status::Empty, false);

        assert!(loading.is_some(), "a loading list said nothing at all");
        assert!(empty.is_some(), "an empty repository said nothing at all");
        assert_ne!(
            loading, empty,
            "a loading list and an empty repository are shown the same words"
        );
    }

    /// A failure that never produced a row shows the failure, and the engine's
    /// sentence is passed through rather than replaced — R5.2's message names
    /// the path, and rewriting it here would lose that.
    #[test]
    fn a_failure_with_nothing_loaded_shows_its_own_sentence() {
        let message = "no git repository at /tmp/nowhere";
        assert_eq!(
            placeholder(&Status::Failed(message.to_owned()), false),
            Some(message.to_owned())
        );
    }

    /// A failure after rows arrived draws the rows, not a sentence in their
    /// place: a page that failed does not unsay the pages that worked.
    #[test]
    fn a_failure_with_rows_loaded_leaves_the_list_showing() {
        assert_eq!(
            placeholder(
                &Status::Failed("failed to read commit abc".to_owned()),
                true
            ),
            None
        );
        assert_eq!(placeholder(&Status::Ready, true), None);
    }

    /// Loading and empty are placeholders whether or not rows were ever seen,
    /// so the arms cannot be collapsed into "show the list once anything has
    /// arrived".
    #[test]
    fn loading_and_empty_are_placeholders_regardless_of_what_arrived_before() {
        assert!(placeholder(&Status::Loading, true).is_some());
        assert!(placeholder(&Status::Empty, true).is_some());
    }

    #[test]
    fn the_count_says_more_is_coming_only_while_it_is() {
        let mut mid_scroll = Progress::opening();
        mid_scroll.received(1, false, 64);
        assert_eq!(loaded_count(&mid_scroll), "64 commits…");

        let mut finished = Progress::opening();
        finished.received(1, true, 2_540);
        assert_eq!(
            loaded_count(&finished),
            "2540 commits",
            "a complete history claimed more was coming"
        );

        let mut broken = Progress::opening();
        broken.received(1, false, 64);
        broken.failed("failed to walk the history".to_owned());
        assert_eq!(
            loaded_count(&broken),
            "64 commits",
            "a history that stopped because of a failure claimed more was coming"
        );
    }

    /// One commit is a commit. A count that says "1 commits" is the kind of
    /// thing a reader notices and nobody ever fixes.
    #[test]
    fn the_count_agrees_with_itself_about_number() {
        let mut one = Progress::opening();
        one.received(1, true, 1);
        assert_eq!(loaded_count(&one), "1 commit");

        let mut none = Progress::opening();
        none.received(1, true, 0);
        assert_eq!(loaded_count(&none), "0 commits");
    }
}
