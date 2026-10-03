//! The accelerator table (decision D5, PRD R8): every keyboard shortcut Cairn has is a
//! logical [`Action`] mapped here to at most one chord per [`Os`], and to the [`Scope`] it
//! is heard in.
//!
//! This is the one file of a render path that names a modifier, and it holds data and the
//! resolution of a press against it — never an element, and never a chord spelled out for a
//! person to read (`the_accelerator_table_holds_data_and_resolution_only`). A component that
//! wants a shortcut asks [`resolve_key`] which action a key press is, and never looks at the
//! held keys itself — the cheap half of keeping macOS reachable, since the platform's
//! command key is a row of this table rather than a literal scattered through components.
//! The guard `no_component_names_a_literal_modifier` holds every other render file to that,
//! and this module's public surface speaks actions, never modifiers, so nothing outside it
//! can branch on one by another name.
//!
//! The chords are Fork's, and only Fork's (user decision 6, 2026-10-03, from
//! `docs/research/diff-engine/fork-shortcuts.md`): previous and next change are ⌘↑/⌘↓ on
//! macOS and Ctrl+↑/Ctrl+↓ elsewhere, heard only while the detail pane has focus so a text
//! field keeps those keys; the detail tabs are ⌘⌥1/⌘⌥2 (the 3 is kept for a File Tree tab);
//! and a second commit is added to the selection with ⌘-click or Ctrl-click. Fork binds no
//! chord to the rest, so neither does Cairn: side-by-side, ignoring whitespace, more or fewer
//! lines and the entire file are actions without one, reached from the diff's header
//! (phase 06); and the previous or next file is the changed-file list's own ↑/↓, with Tab and
//! Shift-Tab moving focus between that list and the diff, as Fork does — keys of the focused
//! view, like the history list's arrows, not chords.

use freya::prelude::{Code, Key, KeyboardEventData, Modifiers, NamedKey};

/// Something a shortcut does, named for what it does rather than for its keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Moves to the previous change in the diff shown.
    PreviousChange,
    /// Moves to the next change in the diff shown.
    NextChange,
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
    pub const ALL: [Action; 10] = [
        Action::PreviousChange,
        Action::NextChange,
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

/// Where an action's chord is heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Wherever focus is in the window.
    Window,
    /// Only while focus is inside the detail pane — its changed-file list today, the diff
    /// from phase 06 — so the same keys keep their meaning in a text field elsewhere (⌘↑ is
    /// the start of the document on macOS).
    Detail,
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

/// The modifiers a chord is made of. The lock keys (caps, num, scroll, function and symbol
/// lock) are left out on purpose, so a lock left on does not change what a chord means.
fn chord_modifiers(held: Modifiers) -> Modifiers {
    held & (Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::META)
}

/// The table: `action`'s chord on `platform`, or `None` for an action Fork gives none. Data,
/// so it is one match naming every action.
pub fn chord(action: Action, platform: Os) -> Option<Chord> {
    // Fork's command key: Command on macOS, Control elsewhere; its option key is Alt.
    let (command, option) = match platform {
        Os::Linux => (Modifiers::CONTROL, Modifiers::ALT),
        Os::MacOs => (Modifiers::META, Modifiers::ALT),
    };
    let both = command | option;
    let (held, trigger) = match action {
        Action::PreviousChange => (command, Trigger::Named(NamedKey::ArrowUp)),
        Action::NextChange => (command, Trigger::Named(NamedKey::ArrowDown)),
        Action::ExtendSelection => (command, Trigger::Press),
        Action::ShowCommitTab => (both, Trigger::Physical(Code::Digit1)),
        Action::ShowChangesTab => (both, Trigger::Physical(Code::Digit2)),
        Action::ToggleSideBySide
        | Action::ToggleIgnoreWhitespace
        | Action::MoreLines
        | Action::FewerLines
        | Action::EntireFile => return None,
    };
    Some(Chord { held, trigger })
}

/// Where `action`'s chord is heard. One match naming every action, beside [`chord`].
pub fn heard_in(action: Action) -> Scope {
    match action {
        Action::PreviousChange | Action::NextChange => Scope::Detail,
        Action::ToggleSideBySide
        | Action::ToggleIgnoreWhitespace
        | Action::MoreLines
        | Action::FewerLines
        | Action::EntireFile
        | Action::ExtendSelection
        | Action::ShowCommitTab
        | Action::ShowChangesTab => Scope::Window,
    }
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

    /// The key event that holds a pointer chord's modifiers down, for a press to complete it;
    /// `None` for a chord completed by a key. Like [`Self::key_press`], for headless tests
    /// only, so a test holds ⌘ or Ctrl through the table rather than by spelling it.
    pub fn press_hold(&self) -> Option<(Key, Code, Modifiers)> {
        match self.trigger {
            Trigger::Press => Some((
                Key::Named(NamedKey::Unidentified),
                Code::Unidentified,
                self.held,
            )),
            Trigger::Named(_) | Trigger::Physical(_) => None,
        }
    }
}

/// The action a key press is on this platform, if it is one heard in `heard`.
pub fn resolve_key(event: &KeyboardEventData, heard: Scope) -> Option<Action> {
    resolve_key_on(
        Os::current(),
        heard,
        &event.key,
        event.code,
        event.modifiers,
    )
}

/// Whether a key press is any action's chord on this platform, in any scope: what a view
/// leaves alone rather than reading as its own key (Ctrl+↓ is "next change", never "next
/// commit", whether or not the pane that hears it has focus).
pub fn is_chord(event: &KeyboardEventData) -> bool {
    [Scope::Window, Scope::Detail]
        .into_iter()
        .any(|heard| resolve_key(event, heard).is_some())
}

/// The action a key press is on `platform`, if it is one heard in `heard`.
pub fn resolve_key_on(
    platform: Os,
    heard: Scope,
    key: &Key,
    code: Code,
    held: Modifiers,
) -> Option<Action> {
    Action::ALL.into_iter().find(|action| {
        heard_in(*action) == heard
            && chord(*action, platform).is_some_and(|chord| chord.is_key(key, code, held))
    })
}

/// The action a pointer press is on `platform`, given the modifiers held when it landed.
pub fn resolve_press_on(platform: Os, held: Modifiers) -> Option<Action> {
    Action::ALL
        .into_iter()
        .find(|action| chord(*action, platform).is_some_and(|chord| chord.is_press(held)))
}

/// What the keyboard says is held, kept from the key presses and releases the window hears,
/// so a pointer press can be resolved against the table: a press carries no modifiers in this
/// build of the toolkit, so ⌘-click and Ctrl-click (`Action::ExtendSelection`) are read from
/// this. It answers actions, never which modifier is down, so nothing that holds one can
/// branch on a modifier by another name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeldKeys(Modifiers);

impl HeldKeys {
    /// A key went down (`down`) or came up. The event's own modifiers are what is held, with
    /// the key itself counted in while it goes down and out as it comes up when it is a
    /// modifier key — a platform may report a modifier key's event before the state it
    /// changes.
    pub fn heard(&mut self, event: &KeyboardEventData, down: bool) {
        let own = match event.key {
            Key::Named(NamedKey::Control) => Modifiers::CONTROL,
            Key::Named(NamedKey::Shift) => Modifiers::SHIFT,
            Key::Named(NamedKey::Alt) => Modifiers::ALT,
            Key::Named(NamedKey::Meta) => Modifiers::META,
            _ => Modifiers::empty(),
        };
        let held = chord_modifiers(event.modifiers);
        self.0 = if down { held | own } else { held - own };
    }

    /// The action a primary pointer press is while these keys are held, on this platform.
    pub fn press(&self) -> Option<Action> {
        self.press_on(Os::current())
    }

    /// [`Self::press`], on `platform`.
    pub fn press_on(&self, platform: Os) -> Option<Action> {
        resolve_press_on(platform, self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 05's obligation to phase 08: a pointer press carries no modifiers, so the second
    /// commit of a comparison is resolved from the keys the window heard — the command key
    /// held makes a press `ExtendSelection` on each platform, and its release, another key, a
    /// lock or an extra Shift do not; the key's own press counts before its state does.
    /// Caught by: a press read with no keys held (the gesture never resolves), the other
    /// platform's command key, a release not let go of (every later click extends), or a
    /// modifier key's own press ignored until a second event.
    #[test]
    fn a_press_resolves_against_the_keys_the_window_heard() {
        let key = |key: Key, modifiers: Modifiers| KeyboardEventData {
            key,
            code: Code::Unidentified,
            modifiers,
        };
        for (platform, command, named) in [
            (Os::Linux, Modifiers::CONTROL, NamedKey::Control),
            (Os::MacOs, Modifiers::META, NamedKey::Meta),
        ] {
            let mut held = HeldKeys::default();
            assert_eq!(held.press_on(platform), None);
            // The command key's own press, reported before the state it sets.
            held.heard(&key(Key::Named(named), Modifiers::empty()), true);
            assert_eq!(held.press_on(platform), Some(Action::ExtendSelection));
            // A lock changes nothing; Shift as well is another chord.
            held.heard(
                &key(
                    Key::Named(NamedKey::CapsLock),
                    command | Modifiers::CAPS_LOCK,
                ),
                true,
            );
            assert_eq!(held.press_on(platform), Some(Action::ExtendSelection));
            held.heard(&key(Key::Named(NamedKey::Shift), command), true);
            assert_eq!(held.press_on(platform), None);
            held.heard(
                &key(Key::Named(NamedKey::Shift), command | Modifiers::SHIFT),
                false,
            );
            assert_eq!(held.press_on(platform), Some(Action::ExtendSelection));
            // Released, reported while its state still says held.
            held.heard(&key(Key::Named(named), command), false);
            assert_eq!(held.press_on(platform), None);
        }
        // What a headless test holds for the press chord resolves it, and its release lets go.
        for platform in PLATFORMS {
            let Some((key, code, modifiers)) =
                chord(Action::ExtendSelection, platform).and_then(|c| c.press_hold())
            else {
                panic!("the extending chord is a press");
            };
            let mut held = HeldKeys::default();
            held.heard(&KeyboardEventData::new(key.clone(), code, modifiers), true);
            assert_eq!(held.press_on(platform), Some(Action::ExtendSelection));
            held.heard(
                &KeyboardEventData::new(key, code, Modifiers::empty()),
                false,
            );
            assert_eq!(held.press_on(platform), None);
        }
        assert_eq!(
            chord(Action::NextChange, Os::Linux).and_then(|c| c.press_hold()),
            None
        );
        // Linux's command key on macOS is not macOS's.
        let mut held = HeldKeys::default();
        held.heard(
            &key(Key::Named(NamedKey::Control), Modifiers::CONTROL),
            true,
        );
        assert_eq!(held.press_on(Os::MacOs), None);
    }

    const PLATFORMS: [Os; 2] = [Os::Linux, Os::MacOs];
    const SCOPES: [Scope; 2] = [Scope::Window, Scope::Detail];

    fn keyed(held: Modifiers, named: NamedKey) -> Option<Chord> {
        Some(Chord {
            held,
            trigger: Trigger::Named(named),
        })
    }

    fn placed(held: Modifiers, code: Code) -> Option<Chord> {
        Some(Chord {
            held,
            trigger: Trigger::Physical(code),
        })
    }

    fn pressed(held: Modifiers) -> Option<Chord> {
        Some(Chord {
            held,
            trigger: Trigger::Press,
        })
    }

    /// T1, user decision 6: the whole table, spelled out per platform from
    /// `docs/systems/diff.md`'s table and Fork's own lists — every action's chord or its
    /// lack of one, and where it is heard. Caught by: Linux's command key or option key
    /// swapped for another modifier, a key code or a trigger swapped, an invented chord
    /// given back to an action Fork leaves unbound, or a chord heard in the wrong scope.
    #[test]
    fn the_table_is_forks_chords_and_no_others() {
        use Modifiers as M;
        let alt = M::ALT;
        let unbound = |action| (action, None, Scope::Window);
        for (platform, command) in [(Os::Linux, M::CONTROL), (Os::MacOs, M::META)] {
            let expected = [
                (
                    Action::PreviousChange,
                    keyed(command, NamedKey::ArrowUp),
                    Scope::Detail,
                ),
                (
                    Action::NextChange,
                    keyed(command, NamedKey::ArrowDown),
                    Scope::Detail,
                ),
                unbound(Action::ToggleSideBySide),
                unbound(Action::ToggleIgnoreWhitespace),
                unbound(Action::MoreLines),
                unbound(Action::FewerLines),
                unbound(Action::EntireFile),
                (Action::ExtendSelection, pressed(command), Scope::Window),
                (
                    Action::ShowCommitTab,
                    placed(command | alt, Code::Digit1),
                    Scope::Window,
                ),
                (
                    Action::ShowChangesTab,
                    placed(command | alt, Code::Digit2),
                    Scope::Window,
                ),
            ];
            assert_eq!(
                expected.map(|(action, _, _)| action),
                Action::ALL,
                "the spelled-out table and the roster disagree"
            );
            for (action, chord_of, heard) in expected {
                assert_eq!(
                    chord(action, platform),
                    chord_of,
                    "{action:?} on {platform:?}"
                );
                assert_eq!(heard_in(action), heard, "{action:?}");
            }
        }
        // Spelled once more without the helpers above, so a swapped modifier cannot hide in
        // them: Linux is Control and Alt, macOS Command and Option.
        assert_eq!(
            chord(Action::ShowChangesTab, Os::Linux).and_then(|c| c.key_press()),
            Some((
                Key::Named(NamedKey::Unidentified),
                Code::Digit2,
                Modifiers::CONTROL | Modifiers::ALT
            ))
        );
        assert_eq!(
            chord(Action::PreviousChange, Os::MacOs).and_then(|c| c.key_press()),
            Some((
                Key::Named(NamedKey::ArrowUp),
                Code::Unidentified,
                Modifiers::META
            ))
        );
    }

    /// C13: every action that has a chord resolves back to itself through the table, in its
    /// own scope and in no other. Caught by: an action whose row the resolver skips, two
    /// actions the resolver cannot tell apart, or a scope the resolver ignores.
    #[test]
    fn every_chord_resolves_to_its_action_in_its_scope_only() {
        for platform in PLATFORMS {
            for action in Action::ALL {
                let Some(chord) = chord(action, platform) else {
                    continue;
                };
                match chord.key_press() {
                    Some((key, code, held)) => {
                        for heard in SCOPES {
                            assert_eq!(
                                resolve_key_on(platform, heard, &key, code, held),
                                (heard == heard_in(action)).then_some(action),
                                "{action:?}'s chord on {platform:?} in {heard:?}"
                            );
                        }
                    }
                    None => assert_eq!(resolve_press_on(platform, chord.held), Some(action)),
                }
            }
        }
    }

    /// No two actions share a chord on one platform, and every chord holds a modifier: an
    /// unmodified key belongs to whatever has focus (the history list's arrows, the file
    /// list's, Tab).
    #[test]
    fn chords_are_distinct_and_every_one_holds_a_modifier() {
        for platform in PLATFORMS {
            let chords: Vec<(Action, Chord)> = Action::ALL
                .into_iter()
                .filter_map(|action| chord(action, platform).map(|chord| (action, chord)))
                .collect();
            for (n, (first, chord_of_first)) in chords.iter().enumerate() {
                assert_ne!(
                    chord_modifiers(chord_of_first.held),
                    Modifiers::empty(),
                    "{first:?} on {platform:?} is a bare key"
                );
                for (second, chord_of_second) in &chords[n + 1..] {
                    assert_ne!(
                        chord_of_first, chord_of_second,
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
        let detail = Scope::Detail;
        assert_eq!(
            resolve_key_on(
                Os::Linux,
                detail,
                &down,
                Code::ArrowDown,
                Modifiers::CONTROL
            ),
            Some(Action::NextChange)
        );
        assert_eq!(
            resolve_key_on(Os::MacOs, detail, &down, Code::ArrowDown, Modifiers::META),
            Some(Action::NextChange)
        );
        assert_eq!(
            resolve_key_on(
                Os::MacOs,
                detail,
                &down,
                Code::ArrowDown,
                Modifiers::CONTROL
            ),
            None,
            "Control+↓ on macOS is not the command key"
        );
        assert_eq!(
            resolve_press_on(Os::MacOs, Modifiers::META),
            Some(Action::ExtendSelection)
        );
        assert_eq!(resolve_press_on(Os::Linux, Modifiers::META), None);
    }

    /// T8: a key or a press without its modifiers, or with one more than its chord, is not
    /// the chord; a lock key held is ignored. Caught by: matching with `contains`, which
    /// makes Ctrl+Shift+↓ "next change" and Ctrl+Shift-click a second commit, and leaves
    /// Shift nothing to mean later.
    #[test]
    fn a_chord_needs_exactly_its_modifiers_and_ignores_the_locks() {
        let down = Key::Named(NamedKey::ArrowDown);
        let (linux, detail) = (Os::Linux, Scope::Detail);
        let control = Modifiers::CONTROL;
        assert_eq!(
            resolve_key_on(linux, detail, &down, Code::ArrowDown, Modifiers::empty()),
            None
        );
        assert_eq!(
            resolve_key_on(
                linux,
                detail,
                &down,
                Code::ArrowDown,
                control | Modifiers::SHIFT
            ),
            None
        );
        let locks = Modifiers::CAPS_LOCK | Modifiers::NUM_LOCK | Modifiers::SCROLL_LOCK;
        assert_eq!(
            resolve_key_on(linux, detail, &down, Code::ArrowDown, control | locks),
            Some(Action::NextChange)
        );
        assert_eq!(resolve_press_on(linux, Modifiers::empty()), None);
        assert_eq!(resolve_press_on(linux, control | Modifiers::SHIFT), None);
        assert_eq!(resolve_press_on(linux, control | Modifiers::ALT), None);
        assert_eq!(
            resolve_press_on(linux, control | Modifiers::CAPS_LOCK),
            Some(Action::ExtendSelection)
        );
    }

    /// A digit is matched where it sits, whatever character the platform makes of it with
    /// Option held; the 3 beside the tabs' is kept for a File Tree tab, so it is nothing yet.
    /// Caught by: matching the character, which ⌘⌥1 never produces on macOS.
    #[test]
    fn a_physical_chord_is_matched_by_where_the_key_sits() {
        let held = Modifiers::META | Modifiers::ALT;
        let window = Scope::Window;
        assert_eq!(
            resolve_key_on(
                Os::MacOs,
                window,
                &Key::Character("¡".into()),
                Code::Digit1,
                held
            ),
            Some(Action::ShowCommitTab)
        );
        assert_eq!(
            resolve_key_on(
                Os::MacOs,
                window,
                &Key::Character("1".into()),
                Code::Digit3,
                held
            ),
            None
        );
        for platform in PLATFORMS {
            let tabs = match platform {
                Os::Linux => Modifiers::CONTROL | Modifiers::ALT,
                Os::MacOs => held,
            };
            for heard in SCOPES {
                assert_eq!(
                    resolve_key_on(
                        platform,
                        heard,
                        &Key::Character("3".into()),
                        Code::Digit3,
                        tabs
                    ),
                    None,
                    "the File Tree's slot is taken on {platform:?}"
                );
            }
        }
    }

    /// A view asks [`is_chord`] to leave a press alone, whichever scope hears it. Caught by:
    /// asking one scope only, which hands "next change" to the history list as "next commit".
    #[test]
    fn a_chord_of_either_scope_is_a_chord() {
        let platform = Os::current();
        for action in [Action::NextChange, Action::ShowCommitTab] {
            let Some((key, code, held)) = chord(action, platform).and_then(|c| c.key_press())
            else {
                panic!("{action:?} has a key chord");
            };
            assert!(
                is_chord(&KeyboardEventData::new(key, code, held)),
                "{action:?}"
            );
        }
        assert!(!is_chord(&KeyboardEventData::new(
            Key::Named(NamedKey::ArrowDown),
            Code::ArrowDown,
            Modifiers::empty()
        )));
    }
}
