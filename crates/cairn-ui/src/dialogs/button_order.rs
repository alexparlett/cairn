//! Where a dialog's buttons sit, per platform (the user's decision E, 2026-10-09): on Linux
//! Fork for Windows' order — the primary button first, Cancel last — and on macOS Fork for
//! macOS' — Cancel first, the primary last. Data per platform, as the accelerator table's
//! chords are; which button holds focus is the dialog's own (every destructive confirmation
//! starts on Cancel, whatever the order).

use freya::prelude::*;

use crate::accelerators::Os;

/// `actions` — the primary button first, then any other action — and `cancel` (Cancel, or
/// Close where nothing is cancelled), in `platform`'s order: on Linux as given with `cancel`
/// last; on macOS `cancel` first and the actions reversed, so the primary is last.
pub fn ordered(platform: Os, actions: Vec<Element>, cancel: Element) -> Vec<Element> {
    match platform {
        Os::Linux => actions.into_iter().chain([cancel]).collect(),
        Os::MacOs => [cancel]
            .into_iter()
            .chain(actions.into_iter().rev())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caption(element: &Element) -> String {
        format!("{element:?}")
    }

    /// The order itself, apart from any dialog: Linux puts the actions first, the primary
    /// leading, and Cancel last; macOS Cancel first and the primary last. Caught by: one
    /// order for both platforms, or the actions' own order kept on macOS.
    #[test]
    fn linux_leads_with_the_primary_and_macos_ends_with_it() {
        let made = || {
            (
                vec![label().text("Primary").into(), label().text("Other").into()],
                label().text("Cancel").into(),
            )
        };
        let (actions, cancel) = made();
        let linux: Vec<String> = ordered(Os::Linux, actions, cancel)
            .iter()
            .map(caption)
            .collect();
        let (actions, cancel) = made();
        let mac: Vec<String> = ordered(Os::MacOs, actions, cancel)
            .iter()
            .map(caption)
            .collect();
        let (actions, cancel) = made();
        let (primary, other, cancel) =
            (caption(&actions[0]), caption(&actions[1]), caption(&cancel));
        assert_eq!(linux, [primary.clone(), other.clone(), cancel.clone()]);
        assert_eq!(mac, [cancel, other, primary]);
    }
}
