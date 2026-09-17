//! A path inside a repository, in the bytes git stores.

use std::borrow::Cow;
use std::fmt;

/// A repository-relative path, as git holds it.
///
/// git promises no encoding for a path, and a patch has to carry the same bytes back or
/// it applies to a different file — so the bytes are what is stored and text is a lossy
/// reading of them.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(Vec<u8>);

impl RepoPath {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Text for a human. Borrowed when the bytes are UTF-8, so a redraw allocates nothing.
    pub fn display(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for RepoPath {
    fn from(text: &str) -> Self {
        Self(text.as_bytes().to_vec())
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

impl fmt::Debug for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: storing text and re-encoding it, which loses the byte a patch needs.
    #[test]
    fn a_path_git_cannot_spell_as_text_keeps_its_bytes() {
        let raw = vec![b'a', 0xff, b'/', b'b'];
        let path = RepoPath::new(raw.clone());
        assert_eq!(path.as_bytes(), &raw[..], "the stored bytes were rewritten");
        assert!(
            path.display().contains('\u{fffd}'),
            "a byte that is not UTF-8 read back as text without a replacement"
        );
    }

    /// Pins that text costs nothing when the bytes are already UTF-8.
    #[test]
    fn a_utf8_path_is_read_as_text_without_copying() {
        let path = RepoPath::from("crates/cairn-model/src/lib.rs");
        assert!(
            matches!(path.display(), Cow::Borrowed(_)),
            "a UTF-8 path allocated to be read"
        );
    }

    #[test]
    fn paths_order_and_compare_by_their_bytes() {
        assert!(RepoPath::from("a/b") < RepoPath::from("a/c"));
        assert_eq!(RepoPath::from("a/b"), RepoPath::new(b"a/b".to_vec()));
        assert!(RepoPath::new(Vec::new()).is_empty());
    }
}
