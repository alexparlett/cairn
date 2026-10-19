//! The toolbar built from the window's state, and what each of its presses does: the status box
//! from what the last refresh answered and what runs, Push's and Pull's badges from the current
//! branch's distance from its upstream, Fetch and Branch greyed with the reasons the menu bar
//! greys their items by, and every press done as its menu item or chord does it.

use std::rc::Rc;

use cairn_model::{AheadBehind, Upstream};
use cairn_ui::accelerators::Action;
use cairn_ui::{Running, StatusBox, Toolbar, ToolbarPress, Tracking, current_branch};

use crate::operations::fetch::FetchStatus;
use crate::refresh::state::RefreshState;
use crate::window::View;
use crate::worker::Request;

/// The toolbar as the window's state says it now.
pub fn toolbar(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Toolbar {
    let open = submit.is_some();
    let tracking = tracking(&view.refreshed.read());
    let counts: Option<AheadBehind> = match tracking {
        Tracking::Counts(counts) => Some(counts),
        Tracking::Untold | Tracking::Gone => None,
    };
    // Read, not peeked: the toolbar is built on the window's render, which must draw it again
    // when the refs arrive.
    let branch_refusal = {
        let _subscribed = view.refreshed.read();
        crate::history::head::head_commit(view)
            .err()
            .map(str::to_owned)
    };
    let status = StatusBox::new(
        view.repository.read().clone(),
        view.refreshed.read().refs().map(|refs| refs.head.clone()),
    )
    .tracking(tracking)
    .running(running(view));
    Toolbar::new(status)
        .counts(counts)
        .fetch_refusal(crate::operations::fetch::refusal(view, open).map(str::to_owned))
        .branch_refusal(if open {
            branch_refusal
        } else {
            Some(crate::operations::fetch::NO_REPOSITORY.to_owned())
        })
        .on_press(move |press: ToolbarPress| pressed(press, view, submit.as_deref()))
        // Fork's: the popover hangs from where the status box is laid out.
        .on_status_sized(move |anchor: (f32, f32)| {
            let mut log = view.activity.log;
            if log.peek().anchor_moved(anchor) {
                log.write().set_anchor(anchor);
            }
        })
}

/// Does what `press` asks: Fetch and Branch as their chords do, through the same refusals; the
/// sidebar shown or hidden; the activity popover opened.
pub fn pressed(press: ToolbarPress, view: View, submit: Option<&dyn Fn(Request)>) {
    match press {
        ToolbarPress::Fetch => crate::shortcuts::act(Action::Fetch, view, submit),
        ToolbarPress::NewBranch => crate::shortcuts::act(Action::NewBranch, view, submit),
        ToolbarPress::ToggleSidebar => {
            let mut shown = view.sidebar.shown;
            shown.toggle();
        }
        ToolbarPress::Activity => view.activity.open(),
    }
}

/// Where the current branch stands against its upstream, from what the last refresh answered —
/// each as old as the answer it is read from.
fn tracking(refreshed: &RefreshState) -> Tracking {
    refreshed
        .refs()
        .and_then(|refs| {
            let branch = current_branch(&refs.head)?;
            Some(match refs.find(branch)?.upstream.as_ref()? {
                Upstream::Gone { .. } => Tracking::Gone,
                Upstream::Exists { .. } => refreshed
                    .ahead_behind_of(branch)
                    .map_or(Tracking::Untold, Tracking::Counts),
            })
        })
        .unwrap_or(Tracking::Untold)
}

/// What runs, for the status box: the local write by its one name with the writes
/// waiting behind it, and a fetch with git's latest progress line — each shown by the box once
/// it has run 250 ms.
pub fn running(view: View) -> Vec<Running> {
    let writes = view.writes.read();
    let write = writes
        .running()
        .zip(writes.running_since())
        .map(|(asked, started)| Running {
            serial: asked.id.number(),
            name: asked.name.clone(),
            started,
            waiting: writes.queued().count(),
            progress: None,
        });
    let fetch = view.fetch.read();
    let progress = match &*fetch {
        FetchStatus::Running { line, .. } => line.clone(),
        _ => None,
    };
    let fetching = fetch
        .is_in_flight()
        // Read without subscribing: the fetch's own state draws the box again as it changes.
        .then(|| view.activity.log.peek().fetch_running())
        .flatten()
        .map(|(serial, name, started)| Running {
            serial: u64::MAX - serial,
            name,
            started,
            waiting: 0,
            progress,
        });
    write.into_iter().chain(fetching).collect()
}
