//! The accelerator table (decision D5, PRD R8): every keyboard shortcut Cairn has is a
//! logical [`Action`] mapped here to one chord per [`Os`].
//!
//! This is the one file of a render path that names a modifier. A component that wants a
//! shortcut asks [`resolve_key`] which action a key press is, and never looks at the held
//! keys itself — the cheap half of keeping macOS reachable, since the platform's command
//! key is a row of this table rather than a literal scattered through components. The guard
//! `no_component_names_a_literal_modifier` holds every other render file to that, and this
//! module's public surface speaks actions, never modifiers, so nothing outside it can branch
//! on one by another name.
//!
//! Where Fork documents a chord, the table takes Fork's: previous and next change are
//! ⌘↑/⌘↓ on macOS and Ctrl+↑/Ctrl+↓ elsewhere, the detail tabs ⌘⌥1/⌘⌥2, and a second
//! commit is added to the selection with ⌘-click or Ctrl-click. Fork documents no chord for
//! the rest, so those are Cairn's own, chosen to stay clear of what the desktop and the
//! staging keys of a later packet claim.

use freya::prelude::{Code, Key, KeyboardEventData, Modifiers, NamedKey};

/// Something a shortcut does, named for what it does rather than for its keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Moves to the previous change in the diff shown.
    PreviousChange,
    /// Moves to the next change in the diff shown.
    NextChange,
    /// Moves to the previous changed file.
    PreviousFile,
    /// Moves to the next changed file.
    NextFile,
    /// Turns side-by-side on or off, for every diff view.
    ToggleSideBySide,
    /// Turns ignoring whitespace on or off.
    ToggleIgnoreWhitespace,
    /// One more line of context around each change.
    MoreLines,
    /// One line of context fewer, never below one.
    FewerLines,
    /// The whole file rather than the changes and their context.
    EntireFile,
    /// Adds a second commit to the selection, to compare the two: a press, not a key.
    ExtendSelection,
    /// Shows the detail pane's Commit tab.
    ShowCommitTab,
    /// Shows the detail pane's Changes tab.
    ShowChangesTab,
}

impl Action {
    /// Every action, in the order the table lists them.
    pub const ALL: [Action; 12] = [
        Action::PreviousChange,
        Action::NextChange,
        Action::PreviousFile,
        Action::NextFile,
        Action::ToggleSideBySide,
        Action::ToggleIgnoreWhitespace,
        Action::MoreLines,
        Action::FewerLines,
        Action::EntireFile,
        Action::ExtendSelection,
        Action::ShowCommitTab,
        Action::ShowChangesTab,
    ];
}

/// The platforms whose chords differ. Every platform but macOS takes the Linux row: Linux is
/// the target, and Windows is not a goal (D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    MacOs,
}

impl Os {
    /// The platform this build runs on.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// What completes a chord once its modifiers are held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trigger {
    /// A key that names itself, matched by the key it produces.
    Named(NamedKey),
    /// A key matched where it sits on the keyboard: with Option held, macOS turns `1` into
    /// `¡`, so a digit or a letter is matched by its position rather than its character.
    Physical(Code),
    /// A primary press of the pointer, as on a commit row.
    Press,
}

/// One chord: the modifiers held, and what completes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    held: Modifiers,
    trigger: Trigger,
}

/// The modifiers a chord is made of. Lock keys are left out, so caps lock on does not change
/// what a chord means.
fn chord_modifiers(held: Modifiers) -> Modifiers {
    held & (Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::META)
}

/// The table: `action`'s chord on `platform`. Data, so it is one match naming every action.
pub fn chord(action: Action, platform: Os) -> Chord {
    // Fork's command key: Command on macOS, Control elsewhere; its option key is Alt.
    let (command, option) = match platform {
        Os::Linux => (Modifiers::CONTROL, Modifiers::ALT),
        Os::MacOs => (Modifiers::META, Modifiers::ALT),
    };
    let both = command | option;
    let (held, trigger) = match action {
        Action::PreviousChange => (command, Trigger::Named(NamedKey::ArrowUp)),
        Action::NextChange => (command, Trigger::Named(NamedKey::ArrowDown)),
        Action::PreviousFile => (option, Trigger::Named(NamedKey::ArrowUp)),
        Action::NextFile => (option, Trigger::Named(NamedKey::ArrowDown)),
        Action::ToggleSideBySide => (both, Trigger::Physical(Code::KeyS)),
        Action::ToggleIgnoreWhitespace => (both, Trigger::Physical(Code::KeyI)),
        Action::MoreLines => (both, Trigger::Physical(Code::BracketRight)),
        Action::FewerLines => (both, Trigger::Physical(Code::BracketLeft)),
        Action::EntireFile => (both, Trigger::Physical(Code::KeyE)),
        Action::ExtendSelection => (command, Trigger::Press),
        Action::ShowCommitTab => (both, Trigger::Physical(Code::Digit1)),
        Action::ShowChangesTab => (both, Trigger::Physical(Code::Digit2)),
    };
    Chord { held, trigger }
}

impl Chord {
    /// Whether a key press of `key`, at `code`, with `held` down, is this chord.
    fn is_key(&self, key: &Key, code: Code, held: Modifiers) -> bool {
        let completed = match self.trigger {
            Trigger::Named(named) => *key == Key::Named(named),
            Trigger::Physical(physical) => code == physical,
            Trigger::Press => false,
        };
        completed && chord_modifiers(held) == self.held
    }

    /// Whether a pointer press with `held` down is this chord.
    fn is_press(&self, held: Modifiers) -> bool {
        self.trigger == Trigger::Press && chord_modifiers(held) == self.held
    }

    /// The key, the physical key and the modifiers a press of this chord sends; `None` for a
    /// chord completed by the pointer. What a headless test presses to reach an action
    /// through the table rather than by spelling its keys — for tests only: a component that
    /// calls it is reading a modifier by another name.
    pub fn key_press(&self) -> Option<(Key, Code, Modifiers)> {
        match self.trigger {
            Trigger::Named(named) => Some((Key::Named(named), Code::Unidentified, self.held)),
            Trigger::Physical(code) => Some((Key::Named(NamedKey::Unidentified), code, self.held)),
            Trigger::Press => None,
        }
    }
}

/// The action a key press is on this platform, if it is one.
pub fn resolve_key(event: &KeyboardEventData) -> Option<Action> {
    resolve_key_on(Os::current(), &event.key, event.code, event.modifiers)
}

/// The action a key press is on `platform`, if it is one.
pub fn resolve_key_on(platform: Os, key: &Key, code: Code, held: Modifiers) -> Option<Action> {
    Action::ALL
        .into_iter()
        .find(|action| chord(*action, platform).is_key(key, code, held))
}

/// The action a pointer press is on `platform`, given the modifiers held when it landed.
pub fn resolve_press_on(platform: Os, held: Modifiers) -> Option<Action> {
    Action::ALL
        .into_iter()
        .find(|action| chord(*action, platform).is_press(held))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLATFORMS: [Os; 2] = [Os::Linux, Os::MacOs];

    /// C13: every action has a chord on every platform, and pressing it resolves back to that
    /// action through the table. Caught by: an action whose row the resolver skips, or two
    /// actions whose chords the resolver cannot tell apart.
    #[test]
    fn every_action_resolves_through_the_table_on_every_platform() {
        for platform in PLATFORMS {
            for action in Action::ALL {
                let chord = chord(action, platform);
                let resolved = match chord.key_press() {
                    Some((key, code, held)) => resolve_key_on(platform, &key, code, held),
                    None => resolve_press_on(platform, chord.held),
                };
                assert_eq!(
                    resolved,
                    Some(action),
                    "{action:?}'s chord on {platform:?} ({chord:?}) does not resolve to it"
                );
            }
        }
    }

    /// No two actions share a chord on one platform, and every chord holds a modifier: an
    /// unmodified key belongs to whatever has focus (the history list's arrows).
    #[test]
    fn chords_are_distinct_and_every_one_holds_a_modifier() {
        for platform in PLATFORMS {
            for (n, first) in Action::ALL.iter().enumerate() {
                let chord_of_first = chord(*first, platform);
                assert_ne!(
                    chord_modifiers(chord_of_first.held),
                    Modifiers::empty(),
                    "{first:?} on {platform:?} is a bare key"
                );
                for second in &Action::ALL[n + 1..] {
                    assert_ne!(
                        chord_of_first,
                        chord(*second, platform),
                        "{first:?} and {second:?} share a chord on {platform:?}"
                    );
                }
            }
        }
    }

    /// D5: macOS reads the command key where Linux reads Control, so the same press means
    /// different things on the two. Caught by: one row for both platforms.
    #[test]
    fn the_command_key_is_the_platforms_own() {
        let down = Key::Named(NamedKey::ArrowDown);
        assert_eq!(
            resolve_key_on(Os::Linux, &down, Code::ArrowDown, Modifiers::CONTROL),
            Some(Action::NextChange)
        );
        assert_eq!(
            resolve_key_on(Os::MacOs, &down, Code::ArrowDown, Modifiers::META),
            Some(Action::NextChange)
        );
        assert_eq!(
            resolve_key_on(Os::MacOs, &down, Code::ArrowDown, Modifiers::CONTROL),
            None,
            "Control+↓ on macOS is not the command key"
        );
        assert_eq!(
            resolve_press_on(Os::MacOs, Modifiers::META),
            Some(Action::ExtendSelection)
        );
        assert_eq!(resolve_press_on(Os::Linux, Modifiers::META), None);
    }

    /// A key without its modifiers, or with one more than its chord, is not the chord; a lock
    /// key held is ignored. Caught by: matching with `contains`, which makes Ctrl+Shift+↓
    /// "next change" and leaves Shift nothing to mean later.
    #[test]
    fn a_chord_needs_exactly_its_modifiers_and_ignores_the_locks() {
        let down = Key::Named(NamedKey::ArrowDown);
        let linux = Os::Linux;
        assert_eq!(
            resolve_key_on(linux, &down, Code::ArrowDown, Modifiers::empty()),
            None
        );
        assert_eq!(
            resolve_key_on(
                linux,
                &down,
                Code::ArrowDown,
                Modifiers::CONTROL | Modifiers::SHIFT
            ),
            None
        );
        assert_eq!(
            resolve_key_on(
                linux,
                &down,
                Code::ArrowDown,
                Modifiers::CONTROL | Modifiers::CAPS_LOCK | Modifiers::NUM_LOCK
            ),
            Some(Action::NextChange)
        );
        assert_eq!(resolve_press_on(linux, Modifiers::empty()), None);
    }

    /// A digit is matched where it sits, whatever character the platform makes of it with
    /// Option held. Caught by: matching the character, which ⌘⌥1 never produces on macOS.
    #[test]
    fn a_physical_chord_is_matched_by_where_the_key_sits() {
        let held = Modifiers::META | Modifiers::ALT;
        assert_eq!(
            resolve_key_on(Os::MacOs, &Key::Character("¡".into()), Code::Digit1, held),
            Some(Action::ShowCommitTab)
        );
        assert_eq!(
            resolve_key_on(Os::MacOs, &Key::Character("1".into()), Code::Digit3, held),
            None
        );
    }
}
