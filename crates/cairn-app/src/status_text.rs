//! Status text for the history list and the fetch.

use crate::fetch_state::FetchStatus;
use crate::history_state::{Progress, Status};

/// What the title bar says about the fetch, or `None` when there is nothing to say.
pub fn fetch_line(fetch: &FetchStatus) -> Option<String> {
    match fetch {
        FetchStatus::Idle => None,
        FetchStatus::Starting { remote } | FetchStatus::Running { remote, line: None } => {
            Some(format!("Fetching {remote}…"))
        }
        FetchStatus::Running {
            remote,
            line: Some(line),
        } => Some(format!("Fetching {remote}: {line}")),
        FetchStatus::Finished { remote } => Some(format!("Fetched {remote}")),
        FetchStatus::Cancelled { remote } => Some(format!("Fetch of {remote} cancelled")),
        // One line: git's whole stderr may be long, and the banner draws every frame.
        FetchStatus::Failed { remote, message } => Some(format!(
            "Fetch of {remote} failed: {}",
            why_it_failed(message)
        )),
    }
}

/// The one line of a failure worth a banner. `Error::GitFailed` carries git's whole
/// stderr, and for a fetch that is progress first — "Enumerating objects: 5, done." —
/// with the reason somewhere after it, so taking the first line told the user what git
/// had got through rather than why it stopped. git prefixes its diagnostics, so prefer
/// the first `fatal:` and then the first `error:`; a message with neither (a refusal
/// raised by Cairn itself, say) is already one sentence and its first line is right.
fn why_it_failed(message: &str) -> &str {
    let diagnostic = |marker| {
        message
            .lines()
            .map(str::trim)
            .find(move |line: &&str| line.contains(marker))
    };
    diagnostic("fatal:")
        .or_else(|| diagnostic("error:"))
        .or_else(|| message.lines().next())
        .unwrap_or_default()
}

/// What fills the list's place, or `None` when the list itself is what to draw.
pub fn placeholder(status: &Status, has_rows: bool) -> Option<String> {
    match status {
        Status::Loading => Some("Reading history…".to_owned()),
        Status::Empty => Some("No commits yet.".to_owned()),
        Status::Failed(message) if !has_rows => Some(message.clone()),
        Status::Failed(_) | Status::Ready => None,
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

    #[test]
    fn every_fetch_state_but_idle_has_its_own_sentence() {
        let remote = "origin".to_owned();
        assert_eq!(fetch_line(&FetchStatus::Idle), None);
        let sentences = [
            FetchStatus::Running {
                remote: remote.clone(),
                line: None,
            },
            FetchStatus::Running {
                remote: remote.clone(),
                line: Some("Receiving objects: 40%".to_owned()),
            },
            FetchStatus::Finished {
                remote: remote.clone(),
            },
            FetchStatus::Cancelled {
                remote: remote.clone(),
            },
            FetchStatus::Failed {
                remote,
                message: "could not read Username".to_owned(),
            },
        ]
        .map(|status| fetch_line(&status));
        for sentence in &sentences {
            assert!(
                sentence.as_ref().is_some_and(|s| s.contains("origin")),
                "{sentence:?} does not name the remote"
            );
        }
        let distinct: std::collections::BTreeSet<_> = sentences.iter().collect();
        assert_eq!(
            distinct.len(),
            sentences.len(),
            "two states say the same thing"
        );
        assert!(sentences[1].as_ref().is_some_and(|s| s.contains("40%")));
        assert!(
            sentences[4]
                .as_ref()
                .is_some_and(|s| s.contains("could not read Username"))
        );
        assert_eq!(
            fetch_line(&FetchStatus::Starting {
                remote: "origin".to_owned()
            }),
            sentences[0],
            "starting and running-without-progress say different things"
        );
    }

    /// Caught by: putting git's whole stderr into a one-line banner every frame.
    #[test]
    fn a_failure_is_said_in_one_line() {
        let line = fetch_line(&FetchStatus::Failed {
            remote: "origin".to_owned(),
            message: "first line\nsecond line\nthird".to_owned(),
        });
        assert_eq!(line.as_deref(), Some("Fetch of origin failed: first line"));
    }

    /// Caught by: showing how far git got instead of why it stopped. `Error::GitFailed`
    /// puts the whole stderr in one string, and for a fetch the progress redraws come
    /// first, so the first line is never the reason.
    #[test]
    fn a_failure_says_why_it_failed_and_not_how_far_git_got() {
        let line = fetch_line(&FetchStatus::Failed {
            remote: "origin".to_owned(),
            message: "git fetch --progress origin failed (exit status: 128): remote: \
                      Enumerating objects: 5, done.\nReceiving objects: 100%\n\
                      fatal: Authentication failed for 'http://localhost/r.git/'"
                .to_owned(),
        });
        assert_eq!(
            line.as_deref(),
            Some(
                "Fetch of origin failed: fatal: Authentication failed for \
                 'http://localhost/r.git/'"
            )
        );

        // An `error:` line is the reason when there is no `fatal:`.
        let line = fetch_line(&FetchStatus::Failed {
            remote: "origin".to_owned(),
            message: "Receiving objects: 100%\nerror: cannot lock ref 'refs/heads/main'".to_owned(),
        });
        assert_eq!(
            line.as_deref(),
            Some("Fetch of origin failed: error: cannot lock ref 'refs/heads/main'")
        );

        // And a message git did not raise keeps its first line, as before.
        assert_eq!(
            fetch_line(&FetchStatus::Failed {
                remote: "origin".to_owned(),
                message: "the prompt was refused\nand nothing asked again".to_owned(),
            })
            .as_deref(),
            Some("Fetch of origin failed: the prompt was refused")
        );
    }

    /// Caught by: giving both states the same words.
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

    /// Caught by: rewriting the engine's sentence, which names the path.
    #[test]
    fn a_failure_with_nothing_loaded_shows_its_own_sentence() {
        let message = "no git repository at /tmp/nowhere";
        assert_eq!(
            placeholder(&Status::Failed(message.to_owned()), false),
            Some(message.to_owned())
        );
    }

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

    /// Caught by: showing the list once anything has arrived.
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
