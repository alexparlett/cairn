//! The one way the window puts text on the clipboard: every Copy item and copy chord — a
//! commit's id or info, a ref's name, a path, the repository's path — ends here.
//!
//! A test reads what was copied on its own thread instead, so the window's copies never write the
//! desktop's clipboard under test and a host without one still decides what a copy put there.
//! (A text field's own copy is the toolkit editor's, which writes it.)

/// Puts `text` on the clipboard. A clipboard the platform does not offer copies nothing; there
/// is nothing to lose.
pub fn copy(text: String) {
    #[cfg(not(test))]
    {
        let _ = freya::clipboard::Clipboard::set(text);
    }
    #[cfg(test)]
    COPIED.with(|copied| *copied.borrow_mut() = Some(text));
}

#[cfg(test)]
thread_local! {
    static COPIED: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// What the last copy on this thread put on the clipboard, taken.
#[cfg(test)]
pub fn take_copied() -> Option<String> {
    COPIED.with(|copied| copied.borrow_mut().take())
}

/// What Copy Full Path copies for `path`: the repository's root, as the window opened it, joined
/// to the path git writes from that root.
pub fn full_path(root: &std::path::Path, path: &cairn_model::RepoPath) -> String {
    root.join(&*path.display()).to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full path is the root and the repository's path joined by one separator, the path's
    /// folders kept. Caught by: the root left off, or the path's folders dropped.
    #[test]
    fn a_full_path_is_the_root_joined_to_the_path() {
        assert_eq!(
            full_path(
                std::path::Path::new("/srv/work/cairn"),
                &cairn_model::RepoPath::from("src/login.rs")
            ),
            "/srv/work/cairn/src/login.rs"
        );
    }
}
