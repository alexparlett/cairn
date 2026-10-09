//! The one key policy every text field takes (staging-and-commit R7.1).
//!
//! Freya's `Input` claims every key but Enter, Escape, Shift and Tab: it stops the press
//! bubbling and cancels the window's global key event, so a focused field hid the window's
//! chords (F5) and the held modifiers a pointer press is resolved against (`HeldKeys`) —
//! `docs/research/staging-and-commit/freya-ui-apis.md` §2. Every field is built here instead,
//! and asks the accelerator table what each key is ([`accelerators::field_key`]): a window
//! chord or a lone modifier key reaches the window untyped; a primary+letter press that is no
//! editing binding types nothing; a chord of the field's own scope (the commit box's commit)
//! is claimed and done, without a new line; and every other key is the field's alone, kept
//! from the views around it — so a bare Enter, Backspace or Delete typed in a field never
//! reaches Local Changes' stage or discard. Nothing here names a modifier: the table decides.
//!
//! `every_text_field_takes_the_shared_key_policy` holds every render file to building its
//! fields here.

use freya::prelude::*;

use crate::accelerators::{self, Action, FieldKey, Scope};

/// A text field with no chord of its own, such as a filter: the window's chords pass through
/// it, and it keeps every other key.
pub fn text_field(value: impl Into<Writable<String>>) -> Input {
    Input::new(value).on_pre_key_down(field_keys(None, None))
}

/// A text field heard in `own` — the commit box — whose chords there it claims and reports
/// through `on_action` rather than typing them.
pub fn text_field_in(
    value: impl Into<Writable<String>>,
    own: Scope,
    on_action: impl Into<EventHandler<Action>>,
) -> Input {
    Input::new(value).on_pre_key_down(field_keys(Some(own), Some(on_action.into())))
}

/// The pre-key handler: `true` lets the field's editor read the key.
fn field_keys(
    own: Option<Scope>,
    on_action: Option<EventHandler<Action>>,
) -> Callback<Event<KeyboardEventData>, bool> {
    Callback::new(move |e: Event<KeyboardEventData>| {
        match accelerators::field_key(&e, own) {
            FieldKey::Own(action) => {
                // Claimed: nothing behind the field, the window included, acts on it too.
                e.stop_propagation();
                e.prevent_default();
                if let Some(on_action) = &on_action {
                    on_action.call(action);
                }
                false
            }
            FieldKey::Unclaimed => {
                // Not typed, and the window's global listener still hears it; no view between
                // the field and the window does.
                e.stop_propagation();
                false
            }
            FieldKey::Edit { bubbles } => {
                if !bubbles {
                    e.stop_propagation();
                }
                Input::key_down_default(e)
            }
        }
    })
}
