//! A filter box's text, as every filter in the window reads it: the text anywhere in a name,
//! no wildcards, case ignored as Unicode lowercases it — the Changes tab's file filter
//! (`ChangeSet::files_matching`) and the sidebar's (`RefsSnapshot::matching`) alike.

/// How many entries a filter matches between two asks of its `keep_going`.
pub(crate) const BETWEEN_CHECKS: usize = 4_096;

/// A filter's text with its case folded, and how to find it in a name.
pub(crate) struct Folded {
    text: String,
    ascii: bool,
}

impl Folded {
    pub(crate) fn of(text: &str) -> Self {
        Self {
            text: text.to_lowercase(),
            ascii: text.is_ascii(),
        }
    }

    /// Whether the text occurs in `name`, case folded on both sides. An ASCII text in an
    /// ASCII name is compared in place; anything else is read and lowercased first, which
    /// allocates only for such a name.
    pub(crate) fn found_in(&self, name: &[u8]) -> bool {
        if self.text.is_empty() {
            return true;
        }
        if self.ascii && name.is_ascii() {
            let wanted = self.text.as_bytes();
            return name
                .windows(wanted.len())
                .any(|window| window.eq_ignore_ascii_case(wanted));
        }
        String::from_utf8_lossy(name)
            .to_lowercase()
            .contains(&self.text)
    }
}
