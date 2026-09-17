//! What the askpass helper and the `git` environment agree on.
//!
//! The helper (`cairn-askpass`) and the environment builder (`cairn-git`)
//! never see each other; these names are the whole of their contract, so a
//! rename on one side cannot silently strand the other.

use std::fmt;

/// The helper binary, installed beside `cairn`; `GIT_ASKPASS` and
/// `SSH_ASKPASS` point at it.
pub const HELPER_PROGRAM: &str = "cairn-askpass";

/// The environment variable naming the socket the helper connects to.
pub const SOCKET_VARIABLE: &str = "CAIRN_ASKPASS_SOCKET";

/// The environment variable carrying the token for the operation the helper
/// serves. In the environment and never on `argv`, which `/proc` makes
/// world-readable (decision L6).
pub const TOKEN_VARIABLE: &str = "CAIRN_ASKPASS_TOKEN";

/// The token that ties a helper invocation to one operation Cairn is running.
///
/// Issued by the channel when an operation begins, handed to `git` in its
/// environment, presented back by the helper, and dead once the operation
/// ends. It is an authorisation to be *asked*, not a credential, and a
/// process running as the same user can read it from `/proc` regardless — the
/// channel's own docs state that limit — so it is not a [`crate::Secret`].
/// Its `Debug` still shows nothing, so a token cannot reach a log by accident.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AskpassToken(String);

impl AskpassToken {
    /// From the characters the channel generated, or the ones the helper
    /// found in its environment. Any value is a well-formed token; whether it
    /// is a live one is the channel's decision.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AskpassToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AskpassToken(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_round_trips_its_value_but_does_not_print_it() {
        let value = format!("token-{}", std::process::id());
        let token = AskpassToken::new(value.clone());
        assert_eq!(token.as_str(), value);
        assert_eq!(format!("{token:?}"), "AskpassToken(..)");
        assert_eq!(token, AskpassToken::new(value));
    }

    /// The names are the contract with the helper; a change is a wire change.
    #[test]
    fn the_variable_names_are_the_ones_the_helper_reads() {
        assert_eq!(SOCKET_VARIABLE, "CAIRN_ASKPASS_SOCKET");
        assert_eq!(TOKEN_VARIABLE, "CAIRN_ASKPASS_TOKEN");
        assert_eq!(HELPER_PROGRAM, "cairn-askpass");
    }
}
