use std::path::PathBuf;

/// Everything the engine can fail with, in the caller's vocabulary.
///
/// gitoxide's error types are deliberately not re-exported: a variant here is
/// something the UI can act on, and adding one is a decision about what the UI
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
}
