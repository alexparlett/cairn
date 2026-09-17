//! Requests and updates crossing the worker boundary.

use cairn_model::{HistoryRow, RemoteSummary};

use super::askpass::PromptId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Starts a walk at `HEAD`, abandoning any walk already open.
    OpenHistory { rows: usize },
    /// The next `rows` rows; falls back to the cold cursor when no walk is open.
    MoreHistory { rows: usize },
    /// The configured remotes, answered by [`Update::Remotes`].
    ListRemotes,
    /// Fetches `remote` (a configured name or a URL) on the operations
    /// thread; its progress and outcome arrive as the `Fetch*` updates.
    Fetch { remote: String },
    /// Kills the fetch in flight, if any.
    CancelFetch,
}

impl Request {
    /// A query is answered under an epoch and superseded by the next one; an
    /// operation is not — a scroll must not cancel a fetch, nor a fetch a scroll.
    pub fn is_query(&self) -> bool {
        match self {
            Self::OpenHistory { .. } | Self::MoreHistory { .. } => true,
            Self::ListRemotes | Self::Fetch { .. } | Self::CancelFetch => false,
        }
    }
}

/// A request is answered by a stream of these. Everything about a fetch or a
/// prompt is tied to no epoch, so a scroll cannot make it vanish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// `complete` says the history has no more to give.
    Rows {
        rows: Vec<HistoryRow>,
        complete: bool,
    },
    /// `message` is display text, already rendered from the engine's error.
    Failed {
        message: String,
    },
    /// A worker died. Tied to no request, so never filtered out by epoch.
    WorkerLost {
        message: String,
    },
    /// The default remote first, when there is one.
    Remotes {
        remotes: Vec<RemoteSummary>,
    },
    /// git is running; a `CancelFetch` from here on has something to kill.
    FetchStarted {
        remote: String,
    },
    /// One redraw of git's own progress meter, for the one fetch in flight.
    FetchProgress {
        line: String,
    },
    /// `refreshed` says a ref moved: the history on screen is of the old
    /// ones and must be asked for again. On every ending, since a fetch that
    /// failed or was killed may have moved some refs before it stopped.
    FetchFinished {
        remote: String,
        refreshed: bool,
    },
    FetchCancelled {
        remote: String,
        refreshed: bool,
    },
    FetchFailed {
        remote: String,
        refreshed: bool,
        message: String,
    },
    /// git or ssh is asking, through the helper: `text` is the prompt as
    /// given, and `id` is what the answer must name.
    Prompt {
        id: PromptId,
        text: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: an operation numbered like a query, which a scroll would then supersede
    /// (the fetch never runs) or which would supersede a page (the scroll stalls).
    #[test]
    fn queries_are_numbered_and_operations_are_not() {
        for query in [
            Request::OpenHistory { rows: 1 },
            Request::MoreHistory { rows: 1 },
        ] {
            assert!(query.is_query(), "{query:?}");
        }
        for operation in [
            Request::ListRemotes,
            Request::Fetch {
                remote: "origin".to_owned(),
            },
            Request::CancelFetch,
        ] {
            assert!(!operation.is_query(), "{operation:?}");
        }
    }
}
