//! A credential value: bytes that must not be shown, copied or remembered.
//!
//! [`Secret`] is the one type in Cairn that holds a credential. It has no
//! `Debug`, no `Display`, no `Clone` and no serialisation, so the compiler
//! refuses the usual routes to a log line, a panic message or a file — and a
//! type that keeps one in a field cannot derive `Debug` either, because the
//! derive needs every field to. The bytes are read through exactly one
//! method, [`Secret::expose_secret`], named so a call site is easy to find and
//! hard to write by accident, and they are zeroed when the value is dropped:
//! `zeroize` does the writes volatilely, which is what stops the compiler
//! from removing a write to memory nothing reads afterwards (decision L11 of
//! the credential-prompts packet — hand-rolled zeroing would claim a
//! protection it might not provide).
//!
//! The invariant this type carries, and its twin: CLAUDE.md, Invariants —
//! "no credential value is logged, Debug-printed, serialised or stored in
//! application state", pinned by
//! `no_credential_value_is_logged_printed_serialised_or_stored` in
//! `crates/cairn-guards/tests/invariants.rs`.

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// A credential, from the moment a user typed it until `git` has read it.
///
/// Built by moving a buffer in — never by copying one — so the allocation
/// that held the typed characters is the allocation that gets zeroed. Read it
/// with [`Secret::expose_secret`] and nothing else.
///
/// The scaffolding every refused snippet below shares compiles; each of them
/// adds exactly one line, and that line is what does not:
///
/// ```
/// let secret = cairn_model::Secret::from_string(String::from("hunter2"));
/// assert_eq!(secret.expose_secret(), b"hunter2");
/// ```
///
/// It cannot be printed (stable `rustdoc` checks that a `compile_fail` block
/// fails, not why; the one-line difference from the block above is what
/// keeps each of these honest):
///
/// ```compile_fail
/// let secret = cairn_model::Secret::from_string(String::from("hunter2"));
/// let shown = format!("{:?}", secret);
/// ```
///
/// ```compile_fail
/// let secret = cairn_model::Secret::from_string(String::from("hunter2"));
/// let shown = format!("{}", secret);
/// ```
///
/// A type that keeps one cannot derive `Debug`, so it cannot be printed
/// through a container either:
///
/// ```compile_fail
/// #[derive(Debug)]
/// struct Holder {
///     secret: cairn_model::Secret,
/// }
/// ```
///
/// And it cannot be duplicated, so there is one buffer to zero:
///
/// ```compile_fail
/// let secret = cairn_model::Secret::from_string(String::from("hunter2"));
/// let twice = secret.clone();
/// ```
#[expect(
    missing_debug_implementations,
    reason = "a credential is never rendered; the missing impl is the point (CLAUDE.md, Invariants)"
)]
pub struct Secret {
    bytes: Zeroizing<Vec<u8>>,
}

impl Secret {
    /// Takes the buffer as it is; nothing is copied, so `bytes` is the only
    /// allocation holding the value and it is the one zeroed on drop. Growth
    /// before this point is the caller's: a buffer that was reallocated while
    /// being filled left its earlier contents behind.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Zeroizing::new(bytes),
        }
    }

    /// From what a user typed. Moves the string's buffer; no copy.
    pub fn from_string(text: String) -> Self {
        Self::new(text.into_bytes())
    }

    /// The bytes, for the one place that hands them to `git`. Every call site
    /// of this method is a place a credential is in the open; the guard
    /// keeps the list of files allowed to have one.
    pub fn expose_secret(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Zeroing before the drop, for a caller that is done with the value early.
impl Zeroize for Secret {
    fn zeroize(&mut self) {
        self.bytes.zeroize();
    }
}

/// The promise the field keeps: `Zeroizing` zeroes its contents when dropped.
impl ZeroizeOnDrop for Secret {}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh value per test; tests generate credentials, never contain them.
    fn generated() -> String {
        format!(
            "generated-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        )
    }

    /// PRD B7, the drop half. What safe code can observe: the type promises
    /// to zero on drop at the type level, zeroing empties the buffer through
    /// the same path the drop takes, and construction copies nothing — so the
    /// buffer that is zeroed is the only one that ever held the value.
    #[test]
    fn a_secret_is_zeroed_on_drop_and_holds_the_only_copy() {
        fn zeroes_on_drop<T: ZeroizeOnDrop>(_: &T) {}

        let typed = generated();
        let original = typed.as_ptr();
        let mut secret = Secret::from_string(typed);
        zeroes_on_drop(&secret);
        assert_eq!(
            secret.expose_secret().as_ptr(),
            original,
            "from_string copied the buffer, leaving an unzeroed copy behind"
        );

        let bytes = vec![7u8; 32];
        let original = bytes.as_ptr();
        let from_bytes = Secret::new(bytes);
        assert_eq!(
            from_bytes.expose_secret().as_ptr(),
            original,
            "new copied the buffer, leaving an unzeroed copy behind"
        );

        assert!(!secret.is_empty());
        secret.zeroize();
        assert!(secret.is_empty(), "zeroize left the bytes in place");
        assert_eq!(secret.len(), 0);
        assert_eq!(secret.expose_secret(), b"");
    }

    #[test]
    fn the_bytes_come_back_as_typed_until_then() {
        let typed = generated();
        let secret = Secret::from_string(typed.clone());
        assert_eq!(secret.expose_secret(), typed.as_bytes());
        assert_eq!(secret.len(), typed.len());
        assert!(!secret.is_empty());
        assert!(Secret::new(Vec::new()).is_empty());
    }
}
