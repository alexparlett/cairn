//! The refs a history row carries (PRD R4.3): the branches, remote-tracking refs and tags
//! that point at its commit, and whether it is `HEAD`'s, from the snapshot its walk began
//! from. A row's labels are kept beside it in its history — the names in the history's text
//! store, the labels of every labelled row in a store of their own — never in the row, so a
//! row with none costs nothing for them.

use crate::RefKind;
use crate::chunked_store::{Runs, Span};

/// One ref pointing at a row's commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label<'a> {
    /// Its full name, `refs/heads/main`, as `git log --decorate=full` names it.
    pub name: &'a str,
    pub kind: RefKind,
    /// The branch `HEAD` is on: git's `HEAD -> refs/heads/main`.
    pub current: bool,
}

impl<'a> Label<'a> {
    /// The name past its namespace, as Fork's chip spells it: `main`, `origin/main`, `v1.0`.
    pub fn short_name(&self) -> &'a str {
        crate::short_ref_name(self.name)
    }
}

/// A label as a history keeps it: its name a span of the history's text store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StoredLabel {
    pub(crate) name: Span,
    pub(crate) kind: RefKind,
    pub(crate) current: bool,
}

/// The labels of one row of a [`crate::History`], read through it.
#[derive(Clone, Copy)]
pub struct RowLabels<'h> {
    head: bool,
    stored: &'h [StoredLabel],
    text: &'h Runs<String, 16>,
}

impl<'h> RowLabels<'h> {
    pub(crate) fn new(head: bool, stored: &'h [StoredLabel], text: &'h Runs<String, 16>) -> Self {
        Self { head, stored, text }
    }

    /// Whether the row's commit is `HEAD`'s: on a branch, or detached at it.
    pub fn is_head(&self) -> bool {
        self.head
    }

    /// How many refs point at the row's commit; `HEAD` is not one of them.
    pub fn len(&self) -> usize {
        self.stored.len()
    }

    /// No ref points at the row's commit, though it may still be `HEAD`'s.
    pub fn is_empty(&self) -> bool {
        self.stored.is_empty()
    }

    /// The ref named `name` (its full name) if it points at the row's commit: a binary
    /// search, since the labels are kept bytewise by name ([`crate::RowsPage::push_labelled`]).
    pub fn find(&self, name: &str) -> Option<Label<'h>> {
        let text = self.text;
        let read = move |label: &StoredLabel| text.get(label.name).unwrap_or_default();
        self.stored
            .binary_search_by(|label| read(label).as_bytes().cmp(name.as_bytes()))
            .ok()
            .and_then(|at| self.stored.get(at))
            .map(|label| Label {
                name: read(label),
                kind: label.kind,
                current: label.current,
            })
    }

    /// Each ref pointing at the row's commit, in the snapshot's order — bytewise by full
    /// name, which puts local branches first, then remote-tracking refs, then tags.
    pub fn iter(&self) -> impl Iterator<Item = Label<'h>> + 'h {
        let text = self.text;
        self.stored.iter().map(move |label| Label {
            name: text.get(label.name).unwrap_or_default(),
            kind: label.kind,
            current: label.current,
        })
    }
}

impl std::fmt::Debug for RowLabels<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowLabels")
            .field("head", &self.head)
            .field("refs", &self.iter().collect::<Vec<_>>())
            .finish()
    }
}
