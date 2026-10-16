//! A confirmation the window shows, and what it does with the token (staging-and-commit
//! R7.4).
//!
//! The view that asks for a destructive operation hands the window the `Consequence` the
//! engine computed — or, for a discard, opens the confirmation at the press, counting, and
//! hands it the `Consequence` or the engine's refusal when it arrives (the redesign's rule 3)
//! — and what to do with the `Confirmed` built from it: ask the write that spends it. The
//! window draws `cairn_ui::ConfirmDialog` over everything else while one is kept, makes its own
//! chords inert (`shortcuts::act`), and lets go of it on either answer. It keeps the
//! consequence, never a token: the token exists only from the press that builds it to the
//! write that takes it.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use cairn_model::{Confirmed, Consequence};
use cairn_ui::DialogAnswer;

/// The serial the next confirmation takes: each is a different dialog, however the window
/// gets from one to the next.
static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

/// A confirmation waiting on the person. Cheap to clone — the window reads it on every render
/// — since its consequence, which may name every file of a large selection, is shared.
#[derive(Clone)]
pub struct Confirming {
    serial: u64,
    title: Rc<str>,
    answer: DialogAnswer,
    /// The one line under the prompt saying what the selection held that this leaves as it is
    /// (the redesign's D1), when it held any.
    left: Option<Rc<str>>,
    then: Rc<dyn Fn(Confirmed)>,
    /// What letting go of it unanswered does besides closing it: a count still running ended.
    let_go: Option<Rc<dyn Fn()>>,
}

impl Confirming {
    /// Ask whether to accept `consequence` under `title`, and hand the token to `then` when
    /// the dialog's button builds it.
    pub fn new(
        title: impl Into<String>,
        consequence: Consequence,
        then: impl Fn(Confirmed) + 'static,
    ) -> Self {
        Self::answering(title, DialogAnswer::Ready(Rc::new(consequence)), then)
    }

    /// Opened at the press, before the engine has counted what it would destroy (R7.4, rule
    /// 3): "Counting…", its button greyed, until [`Self::arrived`] or [`Self::refused`].
    pub fn counting(title: impl Into<String>, then: impl Fn(Confirmed) + 'static) -> Self {
        Self::answering(title, DialogAnswer::Counting, then)
    }

    fn answering(
        title: impl Into<String>,
        answer: DialogAnswer,
        then: impl Fn(Confirmed) + 'static,
    ) -> Self {
        Self {
            serial: NEXT_SERIAL.fetch_add(1, Ordering::Relaxed),
            title: Rc::from(title.into()),
            answer,
            left: None,
            then: Rc::new(then),
            let_go: None,
        }
    }

    /// With `left` said in one line under the prompt: what the selection held that the
    /// operation leaves as it is (`cairn_ui::Left`).
    pub fn leaving(mut self, left: Option<String>) -> Self {
        self.left = left.map(Rc::from);
        self
    }

    /// With `let_go` run when the person lets go of it unanswered — Cancel, Close or Escape.
    pub fn on_let_go(mut self, let_go: impl Fn() + 'static) -> Self {
        self.let_go = Some(Rc::new(let_go));
        self
    }

    /// The count arrived: the confirmation, the same dialog.
    pub fn arrived(&mut self, consequence: Consequence) {
        self.answer = DialogAnswer::Ready(Rc::new(consequence));
    }

    /// The engine refused while counting, in one sentence: the same dialog, now "Couldn't
    /// <title>" (rule 4).
    pub fn refused(&mut self, why: String) {
        self.answer = DialogAnswer::Refused(Rc::from(why));
    }

    /// Which confirmation this is; no other has the same.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn answer(&self) -> &DialogAnswer {
        &self.answer
    }

    /// The consequence drawn, once it has arrived.
    #[cfg(test)]
    pub fn consequence(&self) -> Option<&Rc<Consequence>> {
        match &self.answer {
            DialogAnswer::Ready(consequence) => Some(consequence),
            DialogAnswer::Counting | DialogAnswer::Refused(_) => None,
        }
    }

    pub fn is_counting(&self) -> bool {
        match self.answer {
            DialogAnswer::Counting => true,
            DialogAnswer::Ready(_) | DialogAnswer::Refused(_) => false,
        }
    }

    pub fn left(&self) -> Option<&str> {
        self.left.as_deref()
    }

    /// The person accepted: the token goes where the asking view said.
    pub fn confirmed(&self, token: Confirmed) {
        (self.then)(token);
    }

    /// The person let go of it unanswered.
    pub fn let_go(&self) {
        if let Some(let_go) = &self.let_go {
            let_go();
        }
    }
}

impl std::fmt::Debug for Confirming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Confirming")
            .field("serial", &self.serial)
            .field("title", &self.title)
            .field("answer", &self.answer)
            .finish_non_exhaustive()
    }
}
