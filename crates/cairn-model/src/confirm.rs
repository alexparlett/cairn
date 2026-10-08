use crate::Consequence;

/// Proof that a person was shown what an operation will destroy and accepted
/// it.
///
/// Destructive repository operations (`cairn_git::ops`) take one of these by
/// value. It carries the engine-computed [`Consequence`] and the prompt
/// rendered from it, so the operation re-checks exactly what the person was
/// told and the operation log quotes exactly what they read. The fields are
/// private and the one constructor takes a `Consequence`, never text: a
/// prompt cannot be typed apart from what it describes. It is neither `Clone`
/// nor `Copy`, so one confirmation buys one operation.
///
/// Where it may be built is the guard's: only the confirmation surfaces on
/// its roster name the constructor in production code
/// (`destructive_operations_are_sealed_behind_the_confirmation_token`).
///
/// The scaffold the refused blocks below share compiles; each adds one line,
/// and that line is what does not (stable `rustdoc` checks that a
/// `compile_fail` block fails, not why, so the one-line difference is what
/// keeps each of them honest, and the guard requires each block to be exactly
/// the scaffold plus its line):
///
/// ```
/// fn scaffold(consequence: cairn_model::Consequence) {
///     let confirmed = cairn_model::Confirmed::by_user(consequence);
///     let _ = confirmed.prompt();
/// }
/// ```
///
/// One confirmation cannot be duplicated, by `Clone` or by `Copy`:
///
/// ```compile_fail
/// fn scaffold(consequence: cairn_model::Consequence) {
///     let confirmed = cairn_model::Confirmed::by_user(consequence);
///     let _ = confirmed.prompt();
///     let _ = confirmed.clone();
/// }
/// ```
///
/// ```compile_fail
/// fn scaffold(consequence: cairn_model::Consequence) {
///     let confirmed = cairn_model::Confirmed::by_user(consequence);
///     let _ = confirmed.prompt();
///     let spent = confirmed; let _ = confirmed.prompt();
/// }
/// ```
///
/// And none is built without a `Consequence` — from text, by a literal, by
/// `Default` or by a conversion:
///
/// ```compile_fail
/// fn scaffold(consequence: cairn_model::Consequence) {
///     let confirmed = cairn_model::Confirmed::by_user(consequence);
///     let _ = confirmed.prompt();
///     let _ = cairn_model::Confirmed::by_user(String::from("Discard 3 files?"));
/// }
/// ```
///
/// ```compile_fail
/// fn scaffold(consequence: cairn_model::Consequence) {
///     let confirmed = cairn_model::Confirmed::by_user(consequence);
///     let _ = confirmed.prompt();
///     let _ = cairn_model::Confirmed { consequence: unreachable!(), prompt: String::new() };
/// }
/// ```
///
/// ```compile_fail
/// fn scaffold(consequence: cairn_model::Consequence) {
///     let confirmed = cairn_model::Confirmed::by_user(consequence);
///     let _ = confirmed.prompt();
///     let _ = cairn_model::Confirmed::default();
/// }
/// ```
///
/// ```compile_fail
/// fn scaffold(consequence: cairn_model::Consequence) {
///     let confirmed = cairn_model::Confirmed::by_user(consequence);
///     let _ = confirmed.prompt();
///     let _: cairn_model::Confirmed = String::from("Discard 3 files?").into();
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Confirmed {
    consequence: Consequence,
    prompt: String,
}

impl Confirmed {
    /// The token for the `consequence` the person was shown and accepted, its
    /// prompt rendered from it here.
    ///
    /// Call this at the acknowledgement site — the handler for the confirm
    /// button of a surface on the guard's roster — with the `Consequence`
    /// whose prompt that surface drew, never in the engine.
    pub fn by_user(consequence: Consequence) -> Self {
        let prompt = consequence.prompt();
        Self {
            consequence,
            prompt,
        }
    }

    /// What the person accepted losing, for the operation's re-check.
    pub fn consequence(&self) -> &Consequence {
        &self.consequence
    }

    /// The prompt the person read, rendered from [`Confirmed::consequence`],
    /// for the operation log.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;

    /// Caught by: a prompt typed or rendered apart from the consequence it carries.
    #[test]
    fn the_prompt_is_the_one_rendered_from_the_consequence() {
        let consequence = Consequence::RemoveLock {
            path: PathBuf::from("/repo/.git/index.lock"),
            age: Duration::from_secs(90),
            bytes: 0,
        };
        let expected = consequence.prompt();
        let confirmed = Confirmed::by_user(consequence.clone());
        assert_eq!(confirmed.prompt(), expected);
        assert_eq!(confirmed.consequence(), &consequence);
    }
}
