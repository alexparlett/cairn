use std::path::PathBuf;

/// Engine failures in the caller's vocabulary. gitoxide's error types are never
/// re-exported: a variant here is something the UI can act on.
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

    /// `walked` is how many commits were laid out before stopping; the caller
    /// discards the page.
    #[error("the history query was cancelled after {walked} commits")]
    Cancelled { walked: usize },

    /// `HEAD` points at a branch with no commits yet.
    #[error("the repository at {path} has no commits yet")]
    UnbornHead { path: PathBuf },

    /// Usually a corrupt or missing object. Rows already delivered stay good.
    #[error("failed to walk the history: {source}")]
    Walk {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Named, so the caller can show the rest of the page and say which row is
    /// missing.
    #[error("failed to read commit {id}: {source}")]
    ReadCommit {
        id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
