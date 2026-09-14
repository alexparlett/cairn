//! Repository mutations.
//!
//! This module is the only place in Cairn that writes to a repository, and the
//! guard suite pins that: a mutating gitoxide call or a `git` subprocess
//! anywhere else fails the gate.
//!
//! Operations that can destroy work a user cannot recover from `git reflog`
//! alone — force push, hard reset, branch deletion, history rewrites — take a
//! [`cairn_model::Confirmed`] by value. The token cannot be forged, so the type
//! system, not a review, is what keeps a destructive path from being reached
//! without a prompt.

use cairn_model::Confirmed;

use crate::Repository;

/// A mutation that has been performed, for the operation log the UI shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performed {
    pub description: String,
    /// The prompt the user acknowledged, when the operation needed one.
    pub acknowledged: Option<String>,
}

impl Performed {
    pub(crate) fn destructive(description: impl Into<String>, confirmed: &Confirmed) -> Self {
        Self {
            description: description.into(),
            acknowledged: Some(confirmed.acknowledged().to_owned()),
        }
    }
}

/// Placeholder proving the seal compiles end to end; replaced by the first real
/// destructive operation. It performs no I/O.
pub fn describe_destructive(repo: &Repository, confirmed: Confirmed) -> Performed {
    let _ = repo.inner();
    Performed::destructive(
        format!("no-op against {}", repo.git_dir().display()),
        &confirmed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_destructive_operation_records_what_the_user_agreed_to() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let performed = describe_destructive(&repo, Confirmed::by_user("Discard 3 local commits?"));
        assert_eq!(
            performed.acknowledged.as_deref(),
            Some("Discard 3 local commits?")
        );
    }
}
