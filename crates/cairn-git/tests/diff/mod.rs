//! C1, C2, C3, C5 and C6, plus the reporter behind C14.

mod repositories;
mod scratch;

mod bench;
mod changes;
mod content;
mod patches;

/// `unwrap` and `expect` are denied outside a test function, and a helper shared by several
/// tests is not one. These say the same thing with a message that names what failed, which
/// is what `tests/history.rs` already does.
pub fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

pub fn some<T>(value: Option<T>, what: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{what}"),
    }
}
