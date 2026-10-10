//! A confirmation the window shows, and what it does with the token (staging-and-commit
//! R7.4).
//!
//! The view that asks for a destructive operation hands the window the `Consequence` the
//! engine computed and what to do with the `Confirmed` built from it — ask the write that
//! spends it. The window draws `cairn_ui::ConfirmDialog` over everything else while one is
//! kept, makes its own chords inert (`shortcuts::act`), and lets go of it on either answer.
//! It keeps the consequence, never a token: the token exists only from the press that
//! builds it to the write that takes it.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use cairn_model::{Confirmed, Consequence};

/// The serial the next confirmation takes: each is a different dialog, however the window
/// gets from one to the next.
static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

/// A confirmation waiting on the person. Cheap to clone — the window reads it on every render
/// — since its consequence, which may name every file of a large selection, is shared.
#[derive(Clone)]
pub struct Confirming {
    serial: u64,
    title: Rc<str>,
    consequence: Rc<Consequence>,
    /// The one line under the prompt saying what the selection held that this leaves as it is
    /// (the redesign's D1), when it held any.
    left: Option<Rc<str>>,
    then: Rc<dyn Fn(Confirmed)>,
}

impl Confirming {
    /// Ask whether to accept `consequence` under `title`, and hand the token to `then` when
    /// the dialog's button builds it.
    pub fn new(
        title: impl Into<String>,
        consequence: Consequence,
        then: impl Fn(Confirmed) + 'static,
    ) -> Self {
        Self {
            serial: NEXT_SERIAL.fetch_add(1, Ordering::Relaxed),
            title: Rc::from(title.into()),
            consequence: Rc::new(consequence),
            left: None,
            then: Rc::new(then),
        }
    }

    /// With `left` said in one line under the prompt: what the selection held that the
    /// operation leaves as it is (`cairn_ui::left_as_they_are`).
    pub fn leaving(mut self, left: Option<String>) -> Self {
        self.left = left.map(Rc::from);
        self
    }

    pub fn left(&self) -> Option<&str> {
        self.left.as_deref()
    }

    /// Which confirmation this is; no other has the same.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn consequence(&self) -> &Rc<Consequence> {
        &self.consequence
    }

    /// The person accepted: the token goes where the asking view said.
    pub fn confirmed(&self, token: Confirmed) {
        (self.then)(token);
    }
}

impl std::fmt::Debug for Confirming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Confirming")
            .field("serial", &self.serial)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}
