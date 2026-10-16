//! The discard ask's own tests; acting on files and on lines in the window are `files`' and
//! `gesture`'s.

#[cfg(test)]
mod files;
#[cfg(test)]
mod gesture;

use super::*;

/// An answer is for the confirmation opened for the discard asked last, and one for an earlier
/// ask is dropped; letting go of the ask drops its answer too. Caught by: a stale consequence
/// filling a newer confirmation.
#[test]
fn only_the_discard_asked_last_is_answered() {
    let mut acting = Acting::default();
    let (first, second) = (OperationId::for_tests(1), OperationId::for_tests(2));
    acting.discard_asked(first, 11);
    acting.discard_asked(second, 12);
    assert_eq!(acting.answered(first), None);
    assert!(acting.is_reading());
    assert_eq!(acting.answered(second), Some(12));
    assert!(!acting.is_reading());
    assert_eq!(acting.answered(second), None, "answered twice");
    acting.discard_asked(first, 13);
    acting.forget_discard();
    assert_eq!(acting.answered(first), None);
}
