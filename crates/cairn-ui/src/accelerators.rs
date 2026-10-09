//! The accelerator table (decision D5, PRD R8; staging-and-commit R7): every keyboard
//! shortcut Cairn has is a logical [`Action`] mapped here to a list of distinct chords per
//! [`Os`], and to the [`Scope`] it is heard in.
//!
//! This is the one file of a render path that names a modifier, and it holds data and the
//! resolution of a press against it — never an element, and never a chord spelled out for a
//! person to read (`the_accelerator_table_holds_data_and_resolution_only`); its one child,
//! `chord_names`, spells a chord for a tooltip from this data ([`chord_name`]). A component that
//! wants a shortcut asks [`resolve_key`] which action a key press is, and never looks at the
//! held keys itself — the cheap half of keeping macOS reachable, since the platform's
//! command key is a row of this table rather than a literal scattered through components.
//! The guard `no_component_names_a_literal_modifier` holds every other render file to that,
//! and this module's public surface speaks actions, never modifiers, so nothing outside it
//! can branch on one by another name. What a focused text field does with a key is resolved
//! here too ([`field_key`]), since telling a modifier key or a primary+letter press from
//! typing names a modifier.
//!
//! The chords are Fork's, and only Fork's (user decision 6, 2026-10-03, from
//! `docs/research/diff-engine/fork-shortcuts.md`; the staging chords from
//! `docs/research/staging-and-commit/fork-staging-and-commit.md` §1-2, locked as L7 and L22):
//! previous and next change are ⌘↑/⌘↓ on macOS and Ctrl+↑/Ctrl+↓ elsewhere, heard only
//! while the detail pane has focus so a text field keeps those keys; the detail tabs are
//! ⌘⌥1/⌘⌥2 (the 3 is kept for a File Tree tab); a second commit is added to the selection
//! with ⌘-click or Ctrl-click; and Refresh is ⌘R on macOS and F5 elsewhere (Fork's own lists,
//! `fork-dev/Docs` `keyboard-shortcuts-mac.md` and `keyboard-shortcuts-windows.md`: Linux
//! takes Fork's Windows row, whose F5 is no desktop's and no other action's). In Local
//! Changes, Fork keeps alternates — stage on Return and ⌘S, discard on Backspace, Delete and
//! Ctrl+Shift+D — so an action holds a LIST of chords (L22), and the bare Enter, Backspace
//! and Delete are heard in exactly one scope, a focused file list or diff in Local Changes
//! ([`Scope::LocalChanges`]): nowhere else is a bare chord anything but a function key. Fork
//! binds no chord to the diff's toggles, so neither does Cairn: side-by-side, ignoring
//! whitespace, more or fewer lines and the entire file are actions without one, reached from
//! the diff's header; and the previous or next file is the changed-file list's own ↑/↓, with
//! Tab and Shift-Tab moving focus between that list and the diff, as Fork does — keys of the
//! focused view, like the history list's arrows, not chords.

use freya::prelude::{Code, Key, KeyboardEventData, Modifiers, NamedKey};
use freya::text_edit::EditBindings;

mod chord_names;

pub use chord_names::chord_name;

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
    /// Adds what is pressed to the selection, a press and not a key: a second commit to
    /// compare in the history, a path toggled in or out of Local Changes' list selection.
    ExtendSelection,
    /// Selects every path from the one selected to the one pressed, in Local Changes' lists:
    /// a press, not a key.
    SelectRange,
    /// Extends Local Changes' list selection to the path above.
    ExtendSelectionUp,
    /// Extends Local Changes' list selection to the path below.
    ExtendSelectionDown,
    /// Shows the detail pane's Commit tab.
    ShowCommitTab,
    /// Shows the detail pane's Changes tab.
    ShowChangesTab,
    /// Reads the refs, ahead/behind and the working tree's status again (PRD R10.1).
    Refresh,
    /// Stages what is selected in the Unstaged list or its diff, or unstages what is selected
    /// in the Staged list or its diff (staging-and-commit R8.2, R9.2).
    StageOrUnstage,
    /// Stages every unstaged path, or unstages every staged one, by the list that has focus —
    /// or, a press, by the list whose Stage or Unstage button is pressed with it held.
    StageOrUnstageAll,
    /// Discards what is selected on the unstaged side, through the confirmation (R8.4).
    Discard,
    /// Commits what is staged, from the commit box (R10).
    Commit,
    /// Shows or hides the commits only a reflog reaches (R11.1).
    ShowLostCommits,
}

impl Action {
    /// Every action, in the order the table lists them.
    pub const ALL: [Action; 19] = [
        Action::PreviousChange,
        Action::NextChange,
        Action::ToggleSideBySide,
        Action::ToggleIgnoreWhitespace,
        Action::MoreLines,
        Action::FewerLines,
        Action::EntireFile,
        Action::ExtendSelection,
        Action::SelectRange,
        Action::ExtendSelectionUp,
        Action::ExtendSelectionDown,
        Action::ShowCommitTab,
        Action::ShowChangesTab,
        Action::Refresh,
        Action::StageOrUnstage,
        Action::StageOrUnstageAll,
        Action::Discard,
        Action::Commit,
        Action::ShowLostCommits,
    ];
}

/// Where an action's chord is heard. A key press is resolved in the scope of the view that
/// hears it; a pointer press is resolved wherever it lands, by the list it lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Wherever focus is in the window — a text field included, which hands these to the
    /// window ([`field_key`]).
    Window,
    /// Only while focus is inside the detail pane or Local Changes' diff, so the same keys
    /// keep their meaning in a text field elsewhere (⌘↑ is the start of the document on
    /// macOS).
    Detail,
    /// Only while a file list or the diff in Local Changes has focus: the one scope where a
    /// bare Enter, Backspace or Delete is a chord, since no text field is in it.
    LocalChanges,
    /// Only while Local Changes' Unstaged or Staged list has focus.
    LocalChangesLists,
    /// Only while the commit box's subject or description has focus: commit's chord is heard
    /// there, and no list or staging chord fires — Backspace deletes a character and Enter
    /// makes a new line — while the window's chords still do, as from any field (R7.3, as
    /// amended by the user on 2026-10-09).
    CommitBox,
    /// Only while the history list has focus.
    History,
}

impl Scope {
    /// Every scope.
    pub const ALL: [Scope; 6] = [
        Scope::Window,
        Scope::Detail,
        Scope::LocalChanges,
        Scope::LocalChangesLists,
        Scope::CommitBox,
        Scope::History,
    ];
}

/// The scopes a text field can be heard in as its own ([`field_key`]), each a scope whose every
/// chord holds a modifier: a field claims its own scope's chords before typing, so a scope
/// with a bare chord — Local Changes' Backspace — would turn a typed key into an action. One
/// today, the commit box (`a_fields_own_scope_holds_no_bare_chord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldScope {
    /// The commit box's subject and description.
    CommitBox,
}

impl FieldScope {
    /// Every scope a field can be heard in.
    pub const ALL: [FieldScope; 1] = [FieldScope::CommitBox];

    /// The table's scope it is.
    pub fn heard_as(self) -> Scope {
        match self {
            FieldScope::CommitBox => Scope::CommitBox,
        }
    }
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
    /// `¡`, and Shift turns `.` into `>`, so a digit, a letter or a mark is matched by its
    /// position rather than its character.
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

/// The most chords one action holds on one platform: Fork's discard on Windows, Backspace,
/// Delete and Ctrl+Shift+D.
const MOST_CHORDS: usize = 3;

/// An action's chords on one platform, in the order the table lists them; empty for an
/// action Fork gives none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chords {
    list: [Chord; MOST_CHORDS],
    len: usize,
}

impl Chords {
    fn of<const N: usize>(chords: [(Modifiers, Trigger); N]) -> Self {
        const {
            assert!(
                N <= MOST_CHORDS,
                "an action holds more chords than MOST_CHORDS"
            )
        };
        let unused = Chord {
            held: Modifiers::empty(),
            trigger: Trigger::Press,
        };
        let mut list = [unused; MOST_CHORDS];
        for (slot, (held, trigger)) in list.iter_mut().zip(chords) {
            *slot = Chord { held, trigger };
        }
        Self { list, len: N }
    }

    /// Each chord, in the table's order.
    pub fn iter(&self) -> impl Iterator<Item = Chord> + '_ {
        self.list[..self.len].iter().copied()
    }

    /// The first chord, the one a test presses for the action; `None` when it has none.
    pub fn first(&self) -> Option<Chord> {
        self.iter().next()
    }

    /// Whether the action has no chord.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// The modifiers a chord is made of. The lock keys (caps, num, scroll, function and symbol
/// lock) are left out on purpose, so a lock left on does not change what a chord means.
fn chord_modifiers(held: Modifiers) -> Modifiers {
    held & (Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::META)
}

/// The table: `action`'s chords on `platform`, none for an action Fork gives none. Data, so
/// it is one match naming every action.
pub fn chords(action: Action, platform: Os) -> Chords {
    // Fork's command key: Command on macOS, Control elsewhere; its option key is Alt.
    let (command, option) = match platform {
        Os::Linux => (Modifiers::CONTROL, Modifiers::ALT),
        Os::MacOs => (Modifiers::META, Modifiers::ALT),
    };
    let shift = Modifiers::SHIFT;
    let bare = Modifiers::empty();
    let key = Trigger::Named;
    let at = Trigger::Physical;
    match action {
        Action::PreviousChange => Chords::of([(command, key(NamedKey::ArrowUp))]),
        Action::NextChange => Chords::of([(command, key(NamedKey::ArrowDown))]),
        Action::ExtendSelection => Chords::of([(command, Trigger::Press)]),
        Action::SelectRange => Chords::of([(shift, Trigger::Press)]),
        Action::ExtendSelectionUp => Chords::of([(shift, key(NamedKey::ArrowUp))]),
        Action::ExtendSelectionDown => Chords::of([(shift, key(NamedKey::ArrowDown))]),
        Action::ShowCommitTab => Chords::of([(command | option, at(Code::Digit1))]),
        Action::ShowChangesTab => Chords::of([(command | option, at(Code::Digit2))]),
        // Fork's Windows F5, a function key alone, on Linux; its ⌘R on macOS, the letter
        // matched where it sits.
        Action::Refresh => match platform {
            Os::Linux => Chords::of([(bare, key(NamedKey::F5))]),
            Os::MacOs => Chords::of([(command, at(Code::KeyR))]),
        },
        // Return or ⌘S on macOS, Enter or Ctrl+Shift+S on Windows (and so Linux): the bare
        // key heard only in Local Changes' scope.
        Action::StageOrUnstage => match platform {
            Os::Linux => Chords::of([
                (bare, key(NamedKey::Enter)),
                (command | shift, at(Code::KeyS)),
            ]),
            Os::MacOs => Chords::of([(bare, key(NamedKey::Enter)), (command, at(Code::KeyS))]),
        },
        // ⌥⇧⌘S / Ctrl+Alt+Shift+S on a focused list or diff; and Fork's ⌥ held over a list's
        // Stage or Unstage button, which makes it Stage All or Unstage All — a press.
        Action::StageOrUnstageAll => Chords::of([
            (command | option | shift, at(Code::KeyS)),
            (option, Trigger::Press),
        ]),
        // ⌫ or ⇧⌘D on macOS; Backspace, Delete or Ctrl+Shift+D on Windows.
        Action::Discard => match platform {
            Os::Linux => Chords::of([
                (bare, key(NamedKey::Backspace)),
                (bare, key(NamedKey::Delete)),
                (command | shift, at(Code::KeyD)),
            ]),
            Os::MacOs => Chords::of([
                (bare, key(NamedKey::Backspace)),
                (command | shift, at(Code::KeyD)),
            ]),
        },
        // ⌘Return on macOS, Ctrl+Enter on Windows.
        Action::Commit => Chords::of([(command, key(NamedKey::Enter))]),
        // ⌘⇧. on macOS, Ctrl+Shift+. on Windows: the full stop matched where it sits, since
        // Shift makes it `>`.
        Action::ShowLostCommits => Chords::of([(command | shift, at(Code::Period))]),
        Action::ToggleSideBySide
        | Action::ToggleIgnoreWhitespace
        | Action::MoreLines
        | Action::FewerLines
        | Action::EntireFile => Chords::of([]),
    }
}

/// Where `action`'s chords are heard. One match naming every action, beside [`chords`].
pub fn heard_in(action: Action) -> Scope {
    match action {
        Action::PreviousChange | Action::NextChange => Scope::Detail,
        Action::StageOrUnstage | Action::StageOrUnstageAll | Action::Discard => Scope::LocalChanges,
        Action::ExtendSelectionUp | Action::ExtendSelectionDown => Scope::LocalChangesLists,
        Action::Commit => Scope::CommitBox,
        Action::ShowLostCommits => Scope::History,
        // A press is resolved by the list it lands on, wherever focus is.
        Action::ExtendSelection | Action::SelectRange => Scope::Window,
        Action::ToggleSideBySide
        | Action::ToggleIgnoreWhitespace
        | Action::MoreLines
        | Action::FewerLines
        | Action::EntireFile
        | Action::ShowCommitTab
        | Action::ShowChangesTab
        | Action::Refresh => Scope::Window,
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

    /// The key event that holds a pointer chord's modifiers down, for a press to complete it —
    /// the modifier's own key going down, as a keyboard sends it; `None` for a chord completed
    /// by a key. Like [`Self::key_press`], for headless tests only, so a test holds ⌘ or Ctrl
    /// through the table rather than by spelling it.
    pub fn press_hold(&self) -> Option<(Key, Code, Modifiers)> {
        match self.trigger {
            Trigger::Press => {
                let own = [
                    (Modifiers::CONTROL, NamedKey::Control),
                    (Modifiers::META, NamedKey::Meta),
                    (Modifiers::ALT, NamedKey::Alt),
                    (Modifiers::SHIFT, NamedKey::Shift),
                ]
                .into_iter()
                .find(|(modifier, _)| *modifier == self.held)
                .map_or(NamedKey::Unidentified, |(_, named)| named);
                Some((Key::Named(own), Code::Unidentified, self.held))
            }
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

/// Whether a key press is a chord a view must leave alone rather than read as its own key: a
/// chord of the window, of the detail pane — Ctrl+↓ is "next change", never "next commit",
/// whether or not the pane that hears it has focus — or of a scope the view is in (`own`).
/// Another view's scope is not the view's concern: Shift+↓, the extension of Local Changes'
/// list selection, moves the history list as ↓ does.
pub fn is_chord(event: &KeyboardEventData, own: &[Scope]) -> bool {
    [Scope::Window, Scope::Detail]
        .iter()
        .chain(own)
        .any(|heard| resolve_key(event, *heard).is_some())
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
            && chords(*action, platform)
                .iter()
                .any(|chord| chord.is_key(key, code, held))
    })
}

/// The action a pointer press is on `platform`, given the modifiers held when it landed.
pub fn resolve_press_on(platform: Os, held: Modifiers) -> Option<Action> {
    Action::ALL.into_iter().find(|action| {
        chords(*action, platform)
            .iter()
            .any(|chord| chord.is_press(held))
    })
}

/// What a focused text field does with a key press (staging-and-commit R7.1): the one key
/// policy every field takes, through `cairn_ui::text_field`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKey {
    /// A chord of the field's own scope (the commit box's commit): the field claims it, types
    /// nothing, and does the action — so ⌘Return commits without a new line.
    Own(Action),
    /// Neither typed nor claimed: a chord the window hears wherever focus is, a modifier key
    /// the window keeps held (`HeldKeys`), or a primary+letter press that is no editing
    /// binding. The window's global listener hears it, and nothing between the field and the
    /// window does.
    Unclaimed,
    /// The field's own key — a character, the caret's keys, an editing binding, Enter, Escape,
    /// Tab — handled as the field always handles it. `bubbles` is whether the views around the
    /// field still hear it: only Shift, which a scroll view behind the field reads; every
    /// other key the field keeps from the views around it, so a bare Enter or Backspace typed
    /// in a field never reaches a list's chords.
    Edit { bubbles: bool },
}

/// [`field_key_on`] on this platform.
pub fn field_key(event: &KeyboardEventData, own: Option<FieldScope>) -> FieldKey {
    field_key_on(Os::current(), own, &event.key, event.code, event.modifiers)
}

/// What a text field heard in `own` (none for a field with no chord of its own, such as a
/// filter) does with a key press on `platform`.
pub fn field_key_on(
    platform: Os,
    own: Option<FieldScope>,
    key: &Key,
    code: Code,
    held: Modifiers,
) -> FieldKey {
    if let Some(action) =
        own.and_then(|own| resolve_key_on(platform, own.heard_as(), key, code, held))
    {
        return FieldKey::Own(action);
    }
    if resolve_key_on(platform, Scope::Window, key, code, held).is_some() {
        return FieldKey::Unclaimed;
    }
    match key {
        Key::Named(NamedKey::Shift) => FieldKey::Edit { bubbles: true },
        Key::Named(NamedKey::Control | NamedKey::Alt | NamedKey::AltGraph | NamedKey::Meta) => {
            FieldKey::Unclaimed
        }
        // A primary+letter press types its letter in the toolkit's editor unless it is one of
        // the editor's own bindings (copy, paste, undo, ...): nothing a person meant to type.
        Key::Character(_)
            if held.intersects(Modifiers::CONTROL | Modifiers::META)
                && EditBindings::default_ref().resolve(key, &held).is_none() =>
        {
            FieldKey::Unclaimed
        }
        _ => FieldKey::Edit { bubbles: false },
    }
}

/// A step through Recent Commit Messages from the commit box's subject (staging-and-commit
/// R10.2): ↑ to an older message, ↓ to a newer one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecallStep {
    Older,
    Newer,
}

/// [`recall_step_on`] for a key press.
pub fn recall_step(event: &KeyboardEventData) -> Option<RecallStep> {
    recall_step_on(&event.key, event.modifiers)
}

/// The subject's ↑ and ↓ (R10.2, Fork Tracker #587): a bare arrow, never one held with a
/// modifier — Shift+↑ selects in the field — and nothing for any other key. Not a chord of the
/// table: a field's own scope holds no bare key (`a_fields_own_scope_holds_no_bare_chord`), and
/// the subject takes the step only while it is empty or holds the message last recalled, which
/// the box decides; any other press of an arrow is the editor's.
pub fn recall_step_on(key: &Key, held: Modifiers) -> Option<RecallStep> {
    if !chord_modifiers(held).is_empty() {
        return None;
    }
    match key {
        Key::Named(NamedKey::ArrowUp) => Some(RecallStep::Older),
        Key::Named(NamedKey::ArrowDown) => Some(RecallStep::Newer),
        _ => None,
    }
}

/// What the keyboard says is held, kept from the key presses and releases the window hears,
/// so a pointer press can be resolved against the table: a press carries no modifiers in this
/// build of the toolkit, so ⌘-click and Ctrl-click (`Action::ExtendSelection`) and Shift-click
/// (`Action::SelectRange`) are read from this. It answers actions, never which modifier is
/// down, so nothing that holds one can branch on a modifier by another name: no equality to
/// compare one with, and a `Debug` that names no key.
#[derive(Clone, Copy, Default)]
pub struct HeldKeys(Modifiers);

impl std::fmt::Debug for HeldKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeldKeys").finish_non_exhaustive()
    }
}

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

    /// R10.2: a bare ↑ or ↓ in the subject is a step through the recent messages, and an arrow
    /// held with any modifier — Shift selecting, the command key's previous and next change —
    /// or any other key is none. Caught by: a step taken on Shift+↑, which would recall over a
    /// selection the person was making.
    #[test]
    fn only_a_bare_arrow_steps_through_the_recent_messages() {
        let up = Key::Named(NamedKey::ArrowUp);
        let down = Key::Named(NamedKey::ArrowDown);
        assert_eq!(
            recall_step_on(&up, Modifiers::empty()),
            Some(RecallStep::Older)
        );
        assert_eq!(
            recall_step_on(&down, Modifiers::empty()),
            Some(RecallStep::Newer)
        );
        // A lock left on is no modifier.
        assert_eq!(
            recall_step_on(&up, Modifiers::CAPS_LOCK),
            Some(RecallStep::Older)
        );
        for held in [
            Modifiers::SHIFT,
            Modifiers::CONTROL,
            Modifiers::META,
            Modifiers::ALT,
        ] {
            assert_eq!(recall_step_on(&up, held), None, "{held:?}");
            assert_eq!(recall_step_on(&down, held), None, "{held:?}");
        }
        assert_eq!(
            recall_step_on(&Key::Named(NamedKey::Enter), Modifiers::empty()),
            None
        );
    }

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
            // Shift alone is the range press, on both platforms.
            held.heard(&key(Key::Named(NamedKey::Shift), Modifiers::empty()), true);
            assert_eq!(held.press_on(platform), Some(Action::SelectRange));
            held.heard(&key(Key::Named(NamedKey::Shift), Modifiers::SHIFT), false);
            assert_eq!(held.press_on(platform), None);
        }
        // What a headless test holds for each press chord resolves it, and its release lets go.
        for platform in PLATFORMS {
            for action in [Action::ExtendSelection, Action::SelectRange] {
                let Some((key, code, modifiers)) = chords(action, platform)
                    .first()
                    .and_then(|c| c.press_hold())
                else {
                    panic!("{action:?} is a press");
                };
                let mut held = HeldKeys::default();
                held.heard(&KeyboardEventData::new(key.clone(), code, modifiers), true);
                assert_eq!(held.press_on(platform), Some(action));
                // The modifier's own key, so a field hands it on as a keyboard sends it.
                held = HeldKeys::default();
                held.heard(
                    &KeyboardEventData::new(key.clone(), code, Modifiers::empty()),
                    true,
                );
                assert_eq!(held.press_on(platform), Some(action), "{key:?}");
                held.heard(
                    &KeyboardEventData::new(key, code, Modifiers::empty()),
                    false,
                );
                assert_eq!(held.press_on(platform), None);
            }
        }
        // ⌥ held over a list's Stage or Unstage button: Stage All or Unstage All (R8.2), on
        // both platforms, and with nothing else held only.
        for platform in PLATFORMS {
            let Some((key, code, modifiers)) = chords(Action::StageOrUnstageAll, platform)
                .iter()
                .find_map(|c| c.press_hold())
            else {
                panic!("Stage All has no press on {platform:?}");
            };
            let mut held = HeldKeys::default();
            held.heard(&KeyboardEventData::new(key, code, modifiers), true);
            assert_eq!(held.press_on(platform), Some(Action::StageOrUnstageAll));
            held.heard(&key_of(NamedKey::Shift, Modifiers::ALT), true);
            assert_eq!(
                held.press_on(platform),
                None,
                "⌥⇧ is no press of the table's"
            );
        }
        assert_eq!(
            chords(Action::NextChange, Os::Linux)
                .first()
                .and_then(|c| c.press_hold()),
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

    /// A named key going down with `modifiers` already held.
    fn key_of(named: NamedKey, modifiers: Modifiers) -> KeyboardEventData {
        KeyboardEventData::new(Key::Named(named), Code::Unidentified, modifiers)
    }

    fn keyed(held: Modifiers, named: NamedKey) -> Chord {
        Chord {
            held,
            trigger: Trigger::Named(named),
        }
    }

    fn placed(held: Modifiers, code: Code) -> Chord {
        Chord {
            held,
            trigger: Trigger::Physical(code),
        }
    }

    fn pressed(held: Modifiers) -> Chord {
        Chord {
            held,
            trigger: Trigger::Press,
        }
    }

    /// T1, user decision 6, and staging-and-commit R7.2-R7.3 (L7, L22): the whole table,
    /// spelled out per platform from `docs/systems/diff.md`'s table and Fork's own lists —
    /// every action's chords in order, or its lack of one, and where it is heard. Caught by:
    /// Linux's command key or option key swapped for another modifier, a key code or a
    /// trigger swapped, an invented chord given back to an action Fork leaves unbound, one of
    /// Fork's alternates dropped (stage on Return alone, discard without Delete), or a chord
    /// heard in the wrong scope.
    #[test]
    fn the_table_is_forks_chords_and_no_others() {
        use Modifiers as M;
        let (alt, shift, none) = (M::ALT, M::SHIFT, M::empty());
        let unbound = |action| (action, Vec::new(), Scope::Window);
        for (platform, command) in [(Os::Linux, M::CONTROL), (Os::MacOs, M::META)] {
            let expected: [(Action, Vec<Chord>, Scope); 19] = [
                (
                    Action::PreviousChange,
                    vec![keyed(command, NamedKey::ArrowUp)],
                    Scope::Detail,
                ),
                (
                    Action::NextChange,
                    vec![keyed(command, NamedKey::ArrowDown)],
                    Scope::Detail,
                ),
                unbound(Action::ToggleSideBySide),
                unbound(Action::ToggleIgnoreWhitespace),
                unbound(Action::MoreLines),
                unbound(Action::FewerLines),
                unbound(Action::EntireFile),
                (
                    Action::ExtendSelection,
                    vec![pressed(command)],
                    Scope::Window,
                ),
                (Action::SelectRange, vec![pressed(shift)], Scope::Window),
                (
                    Action::ExtendSelectionUp,
                    vec![keyed(shift, NamedKey::ArrowUp)],
                    Scope::LocalChangesLists,
                ),
                (
                    Action::ExtendSelectionDown,
                    vec![keyed(shift, NamedKey::ArrowDown)],
                    Scope::LocalChangesLists,
                ),
                (
                    Action::ShowCommitTab,
                    vec![placed(command | alt, Code::Digit1)],
                    Scope::Window,
                ),
                (
                    Action::ShowChangesTab,
                    vec![placed(command | alt, Code::Digit2)],
                    Scope::Window,
                ),
                (
                    Action::Refresh,
                    match platform {
                        Os::Linux => vec![keyed(none, NamedKey::F5)],
                        Os::MacOs => vec![placed(command, Code::KeyR)],
                    },
                    Scope::Window,
                ),
                (
                    Action::StageOrUnstage,
                    match platform {
                        Os::Linux => vec![
                            keyed(none, NamedKey::Enter),
                            placed(command | shift, Code::KeyS),
                        ],
                        Os::MacOs => {
                            vec![keyed(none, NamedKey::Enter), placed(command, Code::KeyS)]
                        }
                    },
                    Scope::LocalChanges,
                ),
                (
                    Action::StageOrUnstageAll,
                    vec![placed(command | alt | shift, Code::KeyS), pressed(alt)],
                    Scope::LocalChanges,
                ),
                (
                    Action::Discard,
                    match platform {
                        Os::Linux => vec![
                            keyed(none, NamedKey::Backspace),
                            keyed(none, NamedKey::Delete),
                            placed(command | shift, Code::KeyD),
                        ],
                        Os::MacOs => vec![
                            keyed(none, NamedKey::Backspace),
                            placed(command | shift, Code::KeyD),
                        ],
                    },
                    Scope::LocalChanges,
                ),
                (
                    Action::Commit,
                    vec![keyed(command, NamedKey::Enter)],
                    Scope::CommitBox,
                ),
                (
                    Action::ShowLostCommits,
                    vec![placed(command | shift, Code::Period)],
                    Scope::History,
                ),
            ];
            assert_eq!(
                expected.each_ref().map(|(action, _, _)| *action),
                Action::ALL,
                "the spelled-out table and the roster disagree"
            );
            for (action, chords_of, heard) in expected {
                assert_eq!(
                    chords(action, platform).iter().collect::<Vec<_>>(),
                    chords_of,
                    "{action:?} on {platform:?}"
                );
                assert_eq!(heard_in(action), heard, "{action:?}");
            }
        }
        // Spelled once more without the helpers above, so a swapped modifier cannot hide in
        // them: Linux is Control and Alt, macOS Command and Option.
        assert_eq!(
            chords(Action::ShowChangesTab, Os::Linux)
                .first()
                .and_then(|c| c.key_press()),
            Some((
                Key::Named(NamedKey::Unidentified),
                Code::Digit2,
                Modifiers::CONTROL | Modifiers::ALT
            ))
        );
        assert_eq!(
            chords(Action::PreviousChange, Os::MacOs)
                .first()
                .and_then(|c| c.key_press()),
            Some((
                Key::Named(NamedKey::ArrowUp),
                Code::Unidentified,
                Modifiers::META
            ))
        );
    }

    /// C13, C16: every chord of every action's list resolves back to that action through the
    /// table, in its own scope and in no other. Caught by: an action whose row the resolver
    /// skips, a chord of a list after its first that the resolver never reads, two actions
    /// the resolver cannot tell apart, or a scope the resolver ignores.
    #[test]
    fn every_chord_resolves_to_its_action_in_its_scope_only() {
        for platform in PLATFORMS {
            for action in Action::ALL {
                for chord in chords(action, platform).iter() {
                    match chord.key_press() {
                        Some((key, code, held)) => {
                            for heard in Scope::ALL {
                                assert_eq!(
                                    resolve_key_on(platform, heard, &key, code, held),
                                    (heard == heard_in(action)).then_some(action),
                                    "{action:?}'s chord {chord:?} on {platform:?} in {heard:?}"
                                );
                            }
                        }
                        None => assert_eq!(resolve_press_on(platform, chord.held), Some(action)),
                    }
                }
            }
        }
    }

    /// C16, R7.3: with the commit box focused, the stage, unstage, stage-all, discard and
    /// Show Lost Commits chords resolve to nothing, and the commit chord commits — on each
    /// platform, every chord of each list. Caught by: a staging chord heard in every scope, or
    /// the commit chord heard outside the box.
    #[test]
    fn in_the_commit_box_only_the_commit_chord_resolves() {
        for platform in PLATFORMS {
            for action in Action::ALL {
                for chord in chords(action, platform).iter() {
                    let Some((key, code, held)) = chord.key_press() else {
                        continue;
                    };
                    let expected = (action == Action::Commit).then_some(Action::Commit);
                    assert_eq!(
                        resolve_key_on(platform, Scope::CommitBox, &key, code, held),
                        expected,
                        "{action:?}'s {chord:?} in the commit box on {platform:?}"
                    );
                }
            }
            let enter = Key::Named(NamedKey::Enter);
            let backspace = Key::Named(NamedKey::Backspace);
            for key in [&enter, &backspace] {
                assert_eq!(
                    resolve_key_on(
                        platform,
                        Scope::CommitBox,
                        key,
                        Code::Unidentified,
                        Modifiers::empty()
                    ),
                    None,
                    "{key:?} alone in the commit box on {platform:?}"
                );
            }
        }
    }

    /// One row of a table as the pin reads it: an action, where it is heard, and its chords on
    /// one platform.
    type Row = (Action, Scope, Vec<Chord>);

    /// The table as the pin reads it, on `platform`.
    fn rows_of(platform: Os) -> Vec<Row> {
        Action::ALL
            .into_iter()
            .map(|action| {
                (
                    action,
                    heard_in(action),
                    chords(action, platform).iter().collect(),
                )
            })
            .collect()
    }

    /// The bare keys a chord may be in Local Changes' list-and-diff scope (L7): Fork's stage and
    /// discard keys.
    const LOCAL_CHANGES_BARE_KEYS: [NamedKey; 3] =
        [NamedKey::Enter, NamedKey::Backspace, NamedKey::Delete];

    /// What breaks the pin in `rows`: a chord listed twice — in one action's list, or shared by
    /// two actions, whatever their scopes — and a bare chord that is no function key, but for
    /// Enter, Backspace and Delete in Local Changes' list-and-diff scope.
    fn pin_violations(rows: &[Row]) -> Vec<String> {
        let mut found = Vec::new();
        let every: Vec<(Action, Scope, Chord)> = rows
            .iter()
            .flat_map(|(action, where_heard, list)| {
                list.iter().map(|chord| (*action, *where_heard, *chord))
            })
            .collect();
        for (n, (action, where_heard, chord)) in every.iter().enumerate() {
            let function_key = matches!(
                chord.trigger,
                Trigger::Named(
                    NamedKey::F1
                        | NamedKey::F2
                        | NamedKey::F3
                        | NamedKey::F4
                        | NamedKey::F5
                        | NamedKey::F6
                        | NamedKey::F7
                        | NamedKey::F8
                        | NamedKey::F9
                        | NamedKey::F10
                        | NamedKey::F11
                        | NamedKey::F12
                )
            );
            let local_changes_key = *where_heard == Scope::LocalChanges
                && matches!(chord.trigger, Trigger::Named(named) if LOCAL_CHANGES_BARE_KEYS.contains(&named));
            if chord_modifiers(chord.held).is_empty() && !function_key && !local_changes_key {
                found.push(format!(
                    "{action:?} in {where_heard:?} is a bare {chord:?}, no function key"
                ));
            }
            for (second, _, chord_of_second) in &every[n + 1..] {
                if chord == chord_of_second {
                    found.push(format!("{action:?} and {second:?} both hold {chord:?}"));
                }
            }
        }
        found
    }

    /// R7.2 (L7, L22): no chord is listed twice on one platform — not within one action's list,
    /// and not by two actions, whatever their scopes — and every chord holds a modifier but a
    /// function key's or, in Local Changes' list-and-diff scope alone, a bare Enter, Backspace
    /// or Delete: an unmodified key belongs to whatever has focus (the history list's arrows,
    /// the file list's, Tab, a letter typed in a filter or the commit box), a function key
    /// types nothing in any of them — Fork's Windows Refresh is F5 alone — and the one scope
    /// with bare keys is one no text field is in. The rule is checked over the table, then
    /// shown to fail on the shapes it claims to refuse, so a rule that went blind fails too.
    #[test]
    fn chords_are_distinct_and_every_bare_one_is_a_function_key_or_local_changes_own() {
        for platform in PLATFORMS {
            let rows = rows_of(platform);
            assert_eq!(pin_violations(&rows), Vec::<String>::new(), "{platform:?}");
            // The table holds bare keys only where the rule admits them, so the admission is
            // in use and the shapes below are its edges.
            assert!(
                rows.iter()
                    .any(|(_, where_heard, list)| *where_heard == Scope::LocalChanges
                        && list.iter().any(|c| chord_modifiers(c.held).is_empty())),
                "no bare chord in Local Changes on {platform:?}: the admission reads nothing"
            );

            // A bare Enter added to any other scope fails, whichever action holds it.
            for where_heard in Scope::ALL {
                if where_heard == Scope::LocalChanges {
                    continue;
                }
                for named in LOCAL_CHANGES_BARE_KEYS {
                    let mut moved = rows.clone();
                    let (_, heard, list) = moved
                        .iter_mut()
                        .find(|(action, _, _)| *action == Action::Refresh)
                        .unwrap();
                    *heard = where_heard;
                    list.push(keyed(Modifiers::empty(), named));
                    assert!(
                        pin_violations(&moved)
                            .iter()
                            .any(|v| v.contains("is a bare")),
                        "a bare {named:?} in {where_heard:?} passed on {platform:?}"
                    );
                }
            }
            // A bare key other than the three, even in Local Changes, fails.
            let mut letter = rows.clone();
            letter
                .iter_mut()
                .find(|(action, _, _)| *action == Action::Discard)
                .unwrap()
                .2
                .push(placed(Modifiers::empty(), Code::KeyD));
            assert!(
                !pin_violations(&letter).is_empty(),
                "a bare D in Local Changes"
            );
            // A bare press fails anywhere.
            let mut press = rows.clone();
            press
                .iter_mut()
                .find(|(action, _, _)| *action == Action::SelectRange)
                .unwrap()
                .2 = vec![pressed(Modifiers::empty())];
            assert!(!pin_violations(&press).is_empty(), "a bare press");
            // A chord listed twice for one action fails.
            let mut twice = rows.clone();
            let (_, _, list) = twice
                .iter_mut()
                .find(|(action, _, _)| *action == Action::StageOrUnstage)
                .unwrap();
            let again = list[1];
            list.push(again);
            assert!(
                pin_violations(&twice)
                    .iter()
                    .any(|v| v.contains("StageOrUnstage and StageOrUnstage")),
                "a chord listed twice for one action passed on {platform:?}"
            );
            // Two actions sharing a chord fail, even in different scopes.
            let mut shared = rows.clone();
            let commit = chords(Action::Commit, platform).first().unwrap();
            shared
                .iter_mut()
                .find(|(action, _, _)| *action == Action::ShowLostCommits)
                .unwrap()
                .2
                .push(commit);
            assert!(
                pin_violations(&shared)
                    .iter()
                    .any(|v| v.contains("Commit and ShowLostCommits")),
                "two actions sharing a chord passed on {platform:?}"
            );
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
        let enter = Key::Named(NamedKey::Enter);
        assert_eq!(
            resolve_key_on(
                Os::MacOs,
                Scope::CommitBox,
                &enter,
                Code::Enter,
                Modifiers::CONTROL
            ),
            None,
            "Control+Return on macOS does not commit"
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
    /// Shift nothing to mean later — and makes Ctrl+Enter a bare Enter's stage.
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
        let enter = Key::Named(NamedKey::Enter);
        let local = Scope::LocalChanges;
        assert_eq!(
            resolve_key_on(linux, local, &enter, Code::Enter, Modifiers::empty()),
            Some(Action::StageOrUnstage)
        );
        assert_eq!(
            resolve_key_on(linux, local, &enter, Code::Enter, control),
            None,
            "Ctrl+Enter is the commit's, never the bare Enter's stage"
        );
        assert_eq!(
            resolve_key_on(linux, local, &enter, Code::Enter, Modifiers::NUM_LOCK),
            Some(Action::StageOrUnstage)
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
    /// Option held; the 3 beside the tabs' is kept for a File Tree tab, so it is nothing yet;
    /// and Show Lost Commits' full stop is matched where it sits, since Shift makes it `>`.
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
        assert_eq!(
            resolve_key_on(
                Os::Linux,
                Scope::History,
                &Key::Character(">".into()),
                Code::Period,
                Modifiers::CONTROL | Modifiers::SHIFT
            ),
            Some(Action::ShowLostCommits)
        );
        for platform in PLATFORMS {
            let tabs = match platform {
                Os::Linux => Modifiers::CONTROL | Modifiers::ALT,
                Os::MacOs => held,
            };
            for heard in Scope::ALL {
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

    /// A view asks [`is_chord`] to leave a press alone when the window, the detail pane or a
    /// scope it is in hears it — and only then (QA item 14, the user's decision of
    /// 2026-10-09). Caught by: asking one scope only, which hands "next change" to the history
    /// list as "next commit"; or asking every scope, which swallows Local Changes' Shift+↓ in
    /// every other list.
    #[test]
    fn a_chord_of_the_window_the_pane_or_the_views_own_scope_is_a_chord() {
        let platform = Os::current();
        let event = |action: Action| {
            let Some((key, code, held)) =
                chords(action, platform).first().and_then(|c| c.key_press())
            else {
                panic!("{action:?} has a key chord");
            };
            KeyboardEventData::new(key, code, held)
        };
        for action in [Action::NextChange, Action::ShowCommitTab, Action::Refresh] {
            assert!(is_chord(&event(action), &[]), "{action:?}");
        }
        for (action, own) in [
            (Action::Discard, Scope::LocalChanges),
            (Action::Commit, Scope::CommitBox),
            (Action::ShowLostCommits, Scope::History),
            (Action::ExtendSelectionDown, Scope::LocalChangesLists),
        ] {
            assert!(is_chord(&event(action), &[own]), "{action:?} in {own:?}");
            assert!(
                !is_chord(&event(action), &[]),
                "{action:?} is left alone by a view outside {own:?}"
            );
        }
        assert!(!is_chord(
            &KeyboardEventData::new(
                Key::Named(NamedKey::ArrowDown),
                Code::ArrowDown,
                Modifiers::empty()
            ),
            &Scope::ALL
        ));
    }

    /// QA item 11: a field claims its own scope's chords before it types, so no scope a field
    /// can be heard in may hold a bare chord on any platform — or a Backspace typed there would
    /// be Local Changes' discard. Caught by: a `FieldScope` added for a scope with a bare key.
    #[test]
    fn a_fields_own_scope_holds_no_bare_chord() {
        for platform in PLATFORMS {
            for own in FieldScope::ALL {
                for action in Action::ALL
                    .into_iter()
                    .filter(|action| heard_in(*action) == own.heard_as())
                {
                    for chord in chords(action, platform).iter() {
                        assert!(
                            !chord_modifiers(chord.held).is_empty(),
                            "{action:?}'s {chord:?} is bare in {own:?} on {platform:?}"
                        );
                    }
                }
            }
        }
    }

    /// R7.1, C15: what a focused text field does with each kind of key. A window chord and a
    /// lone Control, Alt or Command are neither typed nor kept from the window; Shift is the
    /// field's and still reaches the views around it; a primary+letter press that is no
    /// editing binding types nothing; the editing bindings, typing, Enter and Backspace are
    /// the field's alone — so a bare Enter or Backspace never reaches Local Changes' chords —
    /// and in the commit box the commit chord is the field's own action, while no staging
    /// chord is. Caught by: a field that claims every key (the window's chords and held keys
    /// lost), one that hands everything on (Backspace discarding, ⌘R typing `r`), or one that
    /// hands its own scope's chord to the window.
    #[test]
    fn a_text_field_hands_on_the_windows_chords_and_keeps_its_own_keys() {
        for (platform, command) in [
            (Os::Linux, Modifiers::CONTROL),
            (Os::MacOs, Modifiers::META),
        ] {
            let field = |own, key: Key, code, held| field_key_on(platform, own, &key, code, held);
            let none = Modifiers::empty();
            let character = |c: &str| Key::Character(c.into());
            // Every window chord, every chord of its list, passes unclaimed, in either field.
            for action in Action::ALL
                .into_iter()
                .filter(|a| heard_in(*a) == Scope::Window)
            {
                for chord in chords(action, platform).iter() {
                    if let Some((key, code, held)) = chord.key_press() {
                        for own in [None, Some(FieldScope::CommitBox)] {
                            assert_eq!(
                                field(own, key.clone(), code, held),
                                FieldKey::Unclaimed,
                                "{action:?} on {platform:?} in {own:?}"
                            );
                        }
                    }
                }
            }
            for modifier in [NamedKey::Control, NamedKey::Alt, NamedKey::Meta] {
                assert_eq!(
                    field(None, Key::Named(modifier), Code::Unidentified, none),
                    FieldKey::Unclaimed,
                    "{modifier:?} on {platform:?}"
                );
            }
            assert_eq!(
                field(None, Key::Named(NamedKey::Shift), Code::ShiftLeft, none),
                FieldKey::Edit { bubbles: true }
            );
            // A primary+letter that is no editing binding types nothing; the bindings are the
            // field's.
            assert_eq!(
                field(None, character("r"), Code::KeyR, command | Modifiers::SHIFT),
                FieldKey::Unclaimed
            );
            assert_eq!(
                field(None, character("q"), Code::KeyQ, command),
                FieldKey::Unclaimed
            );
            for letter in ["a", "c", "x", "v", "z", "y"] {
                assert_eq!(
                    field(None, character(letter), Code::Unidentified, command),
                    FieldKey::Edit { bubbles: false },
                    "{letter} with the command key on {platform:?}"
                );
            }
            // Typing, Shift's capitals and Option's characters are the field's.
            assert_eq!(
                field(None, character("r"), Code::KeyR, none),
                FieldKey::Edit { bubbles: false }
            );
            assert_eq!(
                field(None, character("R"), Code::KeyR, Modifiers::SHIFT),
                FieldKey::Edit { bubbles: false }
            );
            assert_eq!(
                field(None, character("é"), Code::KeyE, Modifiers::ALT),
                FieldKey::Edit { bubbles: false }
            );
            // Enter, Backspace and Delete are the field's, never Local Changes' stage or
            // discard — and the commit box's commit is its own.
            for own in [None, Some(FieldScope::CommitBox)] {
                for named in [NamedKey::Enter, NamedKey::Backspace, NamedKey::Delete] {
                    assert_eq!(
                        field(own, Key::Named(named), Code::Unidentified, none),
                        FieldKey::Edit { bubbles: false },
                        "{named:?} in {own:?} on {platform:?}"
                    );
                }
            }
            let (key, code, held) = chords(Action::Commit, platform)
                .first()
                .and_then(|c| c.key_press())
                .unwrap();
            assert_eq!(
                field(Some(FieldScope::CommitBox), key.clone(), code, held),
                FieldKey::Own(Action::Commit)
            );
            assert_eq!(
                field(None, key, code, held),
                FieldKey::Edit { bubbles: false },
                "a filter has no commit"
            );
            // The staging chords are not the commit box's.
            for action in [
                Action::StageOrUnstage,
                Action::StageOrUnstageAll,
                Action::Discard,
                Action::ShowLostCommits,
            ] {
                // Stage All's press, ⌥ over a list's button, is no key a field hears.
                for (key, code, held) in chords(action, platform)
                    .iter()
                    .filter_map(|c| c.key_press())
                {
                    assert!(
                        !matches!(
                            field(Some(FieldScope::CommitBox), key, code, held),
                            FieldKey::Own(_)
                        ),
                        "{action:?} acts from the commit box on {platform:?}"
                    );
                }
            }
        }
    }
}
