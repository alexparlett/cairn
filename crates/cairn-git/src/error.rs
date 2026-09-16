use std::path::PathBuf;

/// Everything the engine can fail with, in the caller's vocabulary.
///
/// gitoxide's error types are deliberately not re-exported: a variant here is
/// something the UI can act on, so adding one is a decision about what the UI
/// must now handle.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no git repository at {path}")]
    NotARepository { path: PathBuf },

    #[error("failed to open the repository at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The query was abandoned. `walked` is how many commits it had laid out
    /// when it stopped; the caller discards the page.
    #[error("the history query was cancelled after {walked} commits")]
    Cancelled { walked: usize },

    /// `HEAD` points at a branch that has no commits yet. A newly initialised
    /// repository has a history to show, and it is empty.
    #[error("the repository at {path} has no commits yet")]
    UnbornHead { path: PathBuf },

    /// Walking the history failed — a corrupt or missing object, usually. The
    /// list the caller has so far is still good; the rest of it is not coming.
    #[error("failed to walk the history: {source}")]
    Walk {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// One commit could not be read. Named, because the caller can show the
    /// rest of the page and say which row is missing.
    #[error("failed to read commit {id}: {source}")]
    ReadCommit {
        id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
