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

use cairn_model::{Confirmed, Consequence};

/// A confirmation waiting on the person.
#[derive(Clone)]
pub struct Confirming {
    title: String,
    consequence: Consequence,
    then: Rc<dyn Fn(Confirmed)>,
}

impl Confirming {
    /// Ask whether to accept `consequence` under `title`, and hand the token to `then` when
    /// the dialog's button builds it.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "phase 07's discard and phase 09's amend ask for one"
        )
    )]
    pub fn new(
        title: impl Into<String>,
        consequence: Consequence,
        then: impl Fn(Confirmed) + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            consequence,
            then: Rc::new(then),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn consequence(&self) -> &Consequence {
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
            .field("title", &self.title)
            .field("consequence", &self.consequence)
            .finish_non_exhaustive()
    }
}
