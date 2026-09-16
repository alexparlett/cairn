//! Requests and updates crossing the worker boundary.

use cairn_model::HistoryRow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Starts a walk at `HEAD`, abandoning any walk already open.
    OpenHistory { rows: usize },
    /// The next `rows` rows; falls back to the cold cursor when no walk is open.
    MoreHistory { rows: usize },
}

/// A request is answered by a stream of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
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
