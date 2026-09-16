use std::path::PathBuf;

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

    /// `walked` is how many commits were laid out before stopping.
    #[error("the history query was cancelled after {walked} commits")]
    Cancelled { walked: usize },

    /// `HEAD` points at a branch with no commits yet.
    #[error("the repository at {path} has no commits yet")]
    UnbornHead { path: PathBuf },

    /// Usually a corrupt or missing object.
    #[error("failed to walk the history: {source}")]
    Walk {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("failed to read commit {id}: {source}")]
    ReadCommit {
        id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
