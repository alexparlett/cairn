//! What a refresh is asked by, besides the Refresh action (`shortcuts`) and a finished
//! operation (`session`): the window gaining focus (refs-and-status R10.1), as Fork refreshes
//! on its window's activation. Nothing watches the file system (R10.2).
//!
//! On the UI thread, so it only submits: the reads are the worker's, and a refresh asked
//! while one is in flight supersedes it lane by lane, so focus flapping never stacks them.

use std::rc::Rc;

use freya::prelude::*;

use crate::worker::Request;

/// Asks for a refresh each time the window gains focus — not as it opens, which asks its own
/// first refresh, and not when it loses focus. `submit` is `None` with no repository open.
pub fn on_focus_gained(submit: Option<Rc<dyn Fn(Request)>>) {
    let mut was_focused = None;
    use_side_effect(move || {
        let focused = *Platform::get().is_app_focused.read();
        let gained = was_focused == Some(false) && focused;
        was_focused = Some(focused);
        if gained && let Some(submit) = &submit {
            submit(Request::Refresh);
        }
    });
}
