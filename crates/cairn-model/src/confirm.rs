/// Proof that a human was shown a specific consequence and accepted it.
///
/// Destructive repository operations (`cairn_git::ops`) take one of these by
/// value. The field is private and the only constructor records the exact
/// prompt text that was acknowledged, so a code path cannot reach a history
/// rewrite without having put words in front of the user first — and a review
/// can read those words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmed {
    acknowledged: String,
}

impl Confirmed {
    /// Build the token from the prompt the user actually saw and accepted.
    ///
    /// Call this at the acknowledgement site — the handler for the confirm
    /// button — never in the engine, and never with text the user was not
    /// shown.
    pub fn by_user(acknowledged_prompt: impl Into<String>) -> Self {
        Self {
            acknowledged: acknowledged_prompt.into(),
        }
    }

    /// The prompt text, for the operation log.
    pub fn acknowledged(&self) -> &str {
        &self.acknowledged
    }
}
