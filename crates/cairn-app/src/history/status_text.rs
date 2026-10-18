//! Status text for the history list.

use crate::history::state::{Progress, Status};

/// What fills the list's place, or `None` when the list itself is what to draw: while a walk
/// opens with no rows to draw, its sentence — "Reading history… N commits" once the worker has
/// said how far it has read.
pub fn placeholder(progress: &Progress) -> Option<String> {
    let has_rows = progress.has_rows();
    let reading = progress.read_so_far();
    match progress.status() {
        Status::Loading => Some(reading_text(reading)),
        Status::Empty | Status::Failed(_) if !has_rows && reading.is_some() => {
            Some(reading_text(reading))
        }
        Status::Empty => Some("No commits yet.".to_owned()),
        Status::Failed(message) if !has_rows => Some(message.clone()),
        Status::Failed(_) | Status::Ready => None,
    }
}

/// The strip over the rows while a walk replacing them reads ("Reading history… N commits"),
/// once the worker has said how far it has read; `None` with no rows drawn, where the
/// placeholder says it instead.
pub fn reading_strip(progress: &Progress) -> Option<String> {
    let reading = progress.read_so_far()?;
    progress.has_rows().then(|| reading_text(Some(reading)))
}

/// "Reading history…", and how many commits once that is known.
fn reading_text(reading: Option<usize>) -> String {
    match reading {
        None => "Reading history…".to_owned(),
        Some(1) => "Reading history… 1 commit".to_owned(),
        Some(commits) => format!("Reading history… {commits} commits"),
    }
}

/// How much of the history is loaded, as the title bar says it.
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

    /// A progress in `status`, with `loaded` rows held where the status holds any.
    fn at(status: &Status, loaded: usize) -> Progress {
        let mut progress = Progress::opening();
        match status {
            Status::Loading => {}
            Status::Empty => progress.received(1, true, 0),
            Status::Ready => progress.received(1, false, loaded.max(1)),
            Status::Failed(message) => {
                progress.received(1, false, loaded);
                progress.failed(message.clone());
            }
        }
        progress
    }

    /// Caught by: giving both states the same words.
    #[test]
    fn loading_and_an_empty_repository_do_not_say_the_same_thing() {
        let loading = placeholder(&at(&Status::Loading, 0));
        let empty = placeholder(&at(&Status::Empty, 0));

        assert!(loading.is_some(), "a loading list said nothing at all");
        assert!(empty.is_some(), "an empty repository said nothing at all");
        assert_ne!(
            loading, empty,
            "a loading list and an empty repository are shown the same words"
        );
    }

    /// Caught by: rewriting the engine's sentence, which names the path.
    #[test]
    fn a_failure_with_nothing_loaded_shows_its_own_sentence() {
        let message = "no git repository at /tmp/nowhere";
        assert_eq!(
            placeholder(&at(&Status::Failed(message.to_owned()), 0)),
            Some(message.to_owned())
        );
    }

    #[test]
    fn a_failure_with_rows_loaded_leaves_the_list_showing() {
        assert_eq!(
            placeholder(&at(
                &Status::Failed("failed to read commit abc".to_owned()),
                40
            )),
            None
        );
        assert_eq!(placeholder(&at(&Status::Ready, 40)), None);
    }

    /// A first open says it is reading, and how far once the worker has said — only then,
    /// since the worker says nothing of a pass shorter than 250 ms; a reopen over rows draws them
    /// under a strip saying the same, and over none says it in their place. Caught by: a count
    /// shown nowhere, a count drawn in place of the rows a reopen keeps, or "1 commits".
    #[test]
    fn the_history_says_it_is_reading_during_a_long_open() {
        let mut opening = Progress::opening();
        assert_eq!(placeholder(&opening), Some("Reading history…".to_owned()));
        assert_eq!(reading_strip(&opening), None);
        opening.reading(1);
        assert_eq!(
            placeholder(&opening),
            Some("Reading history… 1 commit".to_owned())
        );
        opening.reading(204_800);
        assert_eq!(
            placeholder(&opening),
            Some("Reading history… 204800 commits".to_owned())
        );

        let mut reopening = at(&Status::Ready, 40);
        reopening.reopen();
        assert_eq!(placeholder(&reopening), None, "the rows kept were hidden");
        assert_eq!(
            reading_strip(&reopening),
            None,
            "a short reopen said it was reading"
        );
        reopening.reading(51_200);
        assert_eq!(placeholder(&reopening), None, "the rows kept were hidden");
        assert_eq!(
            reading_strip(&reopening),
            Some("Reading history… 51200 commits".to_owned())
        );

        let mut over_nothing = at(&Status::Empty, 0);
        over_nothing.reopen();
        assert_eq!(
            placeholder(&over_nothing),
            Some("No commits yet.".to_owned())
        );
        over_nothing.reading(300);
        assert_eq!(
            placeholder(&over_nothing),
            Some("Reading history… 300 commits".to_owned())
        );
        assert_eq!(reading_strip(&over_nothing), None);
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

    /// Caught by: "1 commits".
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
