//! Requests and updates crossing the worker boundary. `cairn-model` vocabulary
//! only — no `cairn-git` type crosses, and errors arrive as display text.

use cairn_model::HistoryRow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Starts a scroll at `HEAD` and delivers its first `rows` rows, abandoning
    /// any walk already open.
    OpenHistory { rows: usize },
    /// The next `rows` rows. Falls back to the cold cursor when no walk is open,
    /// so it is never an error.
    MoreHistory { rows: usize },
}

/// A request is answered by a stream of these, not one reply; the history job
/// sends exactly one per request today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// More rows, in walk order, continuing where the last batch stopped.
    /// `complete` says the history has no more to give.
    Rows {
        rows: Vec<HistoryRow>,
        complete: bool,
    },
    /// `message` is display text, already rendered from the engine's error.
    Failed { message: String },
    /// A worker died. Tied to no request, so never filtered out by epoch.
    WorkerLost { message: String },
}
