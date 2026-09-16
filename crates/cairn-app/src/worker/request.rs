//! What crosses the boundary, in both directions.
//!
//! Plain data in `cairn-model` vocabulary, naming no `cairn-git` type: a
//! component can hold an [`Update`] without the engine's surface following it
//! into the view, and an engine error arrives as the sentence to show.

use cairn_model::HistoryRow;

/// What the UI side asks a worker to do.
///
/// A second kind of work — fetch, when that packet lands — is a variant here
/// and an arm in the worker loop, changing no existing call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Start a scroll at `HEAD` and deliver its first `rows` rows. Abandons any
    /// walk already open: this is how a reload or a change of starting point
    /// arrives.
    OpenHistory { rows: usize },
    /// Deliver the next `rows` rows of the open scroll. Falls back to the cold
    /// cursor when no walk is open, so this is never an error.
    MoreHistory { rows: usize },
}

/// What a worker sends back.
///
/// A request is answered by a *stream* of these, not by one reply: a job may
/// send as many as it likes before it finishes. The history job sends exactly
/// one per request today; the shape is what lets a long-running operation
/// report progress later without every call site changing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// More rows, in walk order, continuing wherever the last batch stopped.
    /// `complete` says the history has no more to give.
    Rows {
        rows: Vec<HistoryRow>,
        complete: bool,
    },
    /// The request could not be served. `message` is the sentence to show,
    /// already rendered from the engine's error: the view must not have to
    /// understand `cairn_git::Error` to say what went wrong.
    Failed { message: String },
    /// A worker died. Not tied to any request, and so never filtered out by
    /// epoch: a pool that silently loses a thread degrades into a window that
    /// waits forever.
    WorkerLost { message: String },
}
