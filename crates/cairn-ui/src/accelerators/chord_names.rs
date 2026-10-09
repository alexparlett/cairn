//! A chord spelled for a person to read — the one place Cairn does that (the user's decision,
//! 2026-10-09, amending the modifier invariant): a tooltip names an action's chord, written
//! as Fork's own shortcut lists write it (`fork-dev/Docs`, `keyboard-shortcuts-windows.md`
//! and `keyboard-shortcuts-mac.md`) — on Linux Fork's Windows form, each held key then the
//! key joined by `+` (`Ctrl+Alt+Shift+S`); on macOS the held keys' symbols run together in
//! Fork's order, then the key (`⌘⌥⇧S`, `⌘Return`, `⌫`).
//!
//! Every name is rendered from the accelerator table's data — the chord [`super::chords`]
//! lists first for the action — and never typed out whole: this file names each held key
//! once, alone, and nothing else may spell one. The guards hold both
//! (`no_component_names_a_literal_modifier`, `the_accelerator_table_holds_data_and_resolution_only`).

use freya::prelude::{Code, NamedKey};

use super::{Action, Chord, Os, Trigger, chord_modifiers, chords};

/// `action`'s first chord on `platform`, spelled for a person; `None` for an action with no
/// chord, or whose first chord a pointer press completes.
pub fn chord_name(action: Action, platform: Os) -> Option<String> {
    chords(action, platform)
        .first()
        .and_then(|chord| spelled(chord, platform))
}

fn spelled(chord: Chord, platform: Os) -> Option<String> {
    let key = key_name(chord.trigger, platform)?;
    let held = chord_modifiers(chord.held);
    // The held keys a chord names, in the order Fork writes them on the platform, each with its
    // name there: the table's own constants, written out.
    let order = match platform {
        Os::Linux => [
            (freya::prelude::Modifiers::CONTROL, "Ctrl"),
            (freya::prelude::Modifiers::ALT, "Alt"),
            (freya::prelude::Modifiers::SHIFT, "Shift"),
            (freya::prelude::Modifiers::META, "Super"),
        ],
        Os::MacOs => [
            (freya::prelude::Modifiers::META, "⌘"),
            (freya::prelude::Modifiers::ALT, "⌥"),
            (freya::prelude::Modifiers::SHIFT, "⇧"),
            (freya::prelude::Modifiers::CONTROL, "⌃"),
        ],
    };
    let names = order
        .into_iter()
        .filter(|(modifier, _)| held.contains(*modifier))
        .map(|(_, name)| name);
    let joiner = match platform {
        Os::Linux => "+",
        Os::MacOs => "",
    };
    let mut spelled: Vec<String> = names.map(str::to_owned).collect();
    spelled.push(key);
    Some(spelled.join(joiner))
}

/// What completes a chord, named as Fork names the key on `platform`.
fn key_name(trigger: Trigger, platform: Os) -> Option<String> {
    let mac = matches!(platform, Os::MacOs);
    Some(match trigger {
        Trigger::Press => return None,
        Trigger::Named(named) => match named {
            NamedKey::Enter if mac => "Return".to_owned(),
            NamedKey::Backspace if mac => "⌫".to_owned(),
            NamedKey::Delete if mac => "⌦".to_owned(),
            NamedKey::ArrowUp => "↑".to_owned(),
            NamedKey::ArrowDown => "↓".to_owned(),
            NamedKey::ArrowLeft => "←".to_owned(),
            NamedKey::ArrowRight => "→".to_owned(),
            NamedKey::Escape => "Esc".to_owned(),
            other => format!("{other:?}"),
        },
        Trigger::Physical(code) => code_name(code),
    })
}

/// A physical key's character, as its key cap shows it unshifted.
fn code_name(code: Code) -> String {
    let named = format!("{code:?}");
    if let Some(letter) = named.strip_prefix("Key") {
        return letter.to_owned();
    }
    if let Some(digit) = named.strip_prefix("Digit") {
        return digit.to_owned();
    }
    match code {
        Code::Period => ".".to_owned(),
        Code::Comma => ",".to_owned(),
        Code::Minus => "-".to_owned(),
        Code::Equal => "=".to_owned(),
        Code::Slash => "/".to_owned(),
        Code::Semicolon => ";".to_owned(),
        Code::Quote => "'".to_owned(),
        Code::BracketLeft => "[".to_owned(),
        Code::BracketRight => "]".to_owned(),
        Code::Backslash => "\\".to_owned(),
        Code::Backquote => "`".to_owned(),
        _ => named,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each of the table's chords, spelled as Fork's lists spell it, on each platform.
    /// Caught by: a name typed apart from the table (it would not follow a chord changed
    /// there), held keys out of Fork's order, the wrong joiner, or a key named as the event
    /// names it rather than as its cap reads.
    #[test]
    fn a_chord_is_named_from_the_table_as_forks_lists_write_it() {
        for (action, linux, mac) in [
            (Action::ShowLostCommits, "Ctrl+Shift+.", "⌘⇧."),
            (Action::StageOrUnstageAll, "Ctrl+Alt+Shift+S", "⌘⌥⇧S"),
            (Action::Commit, "Ctrl+Enter", "⌘Return"),
            (Action::Discard, "Backspace", "⌫"),
            (Action::ShowCommitTab, "Ctrl+Alt+1", "⌘⌥1"),
            (Action::NextChange, "Ctrl+↓", "⌘↓"),
            (Action::Refresh, "F5", "⌘R"),
        ] {
            assert_eq!(
                chord_name(action, Os::Linux).as_deref(),
                Some(linux),
                "{action:?}"
            );
            assert_eq!(
                chord_name(action, Os::MacOs).as_deref(),
                Some(mac),
                "{action:?}"
            );
        }
        // A press, and an action without a chord, have no name.
        assert_eq!(chord_name(Action::ExtendSelection, Os::Linux), None);
        assert_eq!(chord_name(Action::ToggleSideBySide, Os::MacOs), None);
    }
}
