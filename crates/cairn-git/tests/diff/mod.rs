//! C1, C2, C3, C5 and C6, plus the reporter behind C14.

mod repositories;
mod scratch;

mod bare_discovery;
mod bench;
mod changes;
mod content;
mod parity;
mod patches;
mod working_tree;

/// `unwrap` and `expect` are denied outside a test function, and a helper shared by several
/// tests is not one. These say the same thing with a message that names what failed, which
/// is what `tests/history.rs` already does.
pub fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

/// The `git` Cairn itself would find on this machine, found once for the whole binary: what
/// the changes query runs (decision E). Its environment is the one the application builds,
/// so it reads the same configuration the engine's gix handle does.
pub fn git() -> &'static cairn_git::ops::GitBinary {
    static GIT: std::sync::OnceLock<cairn_git::ops::GitBinary> = std::sync::OnceLock::new();
    GIT.get_or_init(|| {
        ok(
            cairn_git::ops::GitBinary::discover(&cairn_git::ops::Askpass::new(
                "/nonexistent/cairn-askpass",
                None,
            )),
            "git is found",
        )
    })
}

pub fn some<T>(value: Option<T>, what: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{what}"),
    }
}
