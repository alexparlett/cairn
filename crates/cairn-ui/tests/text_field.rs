//! Headless tests for the one key policy every text field takes (staging-and-commit R7.1,
//! C15): a field hands the window's chords and the held modifiers to the window, keeps its
//! own keys from the views around it, and — in the commit box — claims the commit chord
//! without a new line. Every chord is pressed through the accelerator table.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_ui::accelerators::{self, Action, HeldKeys, Os, Scope};
use cairn_ui::{text_field, text_field_in};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{KeyboardEventName, PlatformEvent};

const WIDTH: f32 = 600.;
const HEIGHT: f32 = 400.;

/// What was heard, in order.
#[derive(Default, Clone)]
struct Heard {
    /// The window's actions, resolved by a global listener as the window's root resolves them.
    window: Rc<RefCell<Vec<Action>>>,
    /// Local Changes' actions, resolved by the view around the field, as a list or a pane
    /// around a field would resolve them.
    around: Rc<RefCell<Vec<Action>>>,
    /// The field's own actions.
    own: Rc<RefCell<Vec<Action>>>,
    /// The keys the window heard held, kept as the window keeps them.
    held: Rc<RefCell<HeldKeys>>,
}

/// A window around a view around one field: a commit-box description (multiline, heard in
/// the commit box) when `commit_box`, a filter otherwise.
fn launch(commit_box: bool) -> (TestingRunner, Heard, State<String>) {
    let heard = Heard::default();
    let (mut test, value) = TestingRunner::new(
        {
            let heard = heard.clone();
            move || {
                let value = use_consume::<State<String>>();
                let Heard {
                    window,
                    around,
                    own,
                    held,
                } = heard.clone();
                let holding = held.clone();
                let field = if commit_box {
                    text_field_in(value, Scope::CommitBox, move |action: Action| {
                        own.borrow_mut().push(action)
                    })
                    .multiline(true)
                    .height(Size::px(120.))
                } else {
                    text_field(value)
                };
                rect()
                    .expanded()
                    .on_global_key_down(move |e: Event<KeyboardEventData>| {
                        held.borrow_mut().heard(&e, true);
                        if let Some(action) = accelerators::resolve_key(&e, Scope::Window) {
                            window.borrow_mut().push(action);
                        }
                    })
                    .on_global_key_up(move |e: Event<KeyboardEventData>| {
                        holding.borrow_mut().heard(&e, false)
                    })
                    .child(
                        rect()
                            .expanded()
                            .on_key_down(move |e: Event<KeyboardEventData>| {
                                if let Some(action) =
                                    accelerators::resolve_key(&e, Scope::LocalChanges)
                                {
                                    around.borrow_mut().push(action);
                                }
                            })
                            .child(field.width(Size::fill())),
                    )
            }
        },
        (WIDTH, HEIGHT).into(),
        |runner| runner.provide_root_context(|| State::create(String::new())),
        1.,
    );
    test.sync_and_update();
    // Into the field, so it has focus.
    test.click_cursor((20., 15.));
    test.sync_and_update();
    (test, heard, value)
}

/// The key event of `action`'s `nth` chord on this platform, pressed down.
fn press_chord(test: &mut TestingRunner, action: Action, nth: usize) {
    let chord = accelerators::chords(action, Os::current())
        .iter()
        .nth(nth)
        .unwrap_or_else(|| panic!("{action:?} has no chord {nth}"));
    let (key, code, modifiers) = chord
        .key_press()
        .unwrap_or_else(|| panic!("{action:?} is a key chord"));
    key_down(test, key, code, modifiers);
}

fn key_down(test: &mut TestingRunner, key: Key, code: Code, modifiers: Modifiers) {
    test.send_event(PlatformEvent::Keyboard {
        name: KeyboardEventName::KeyDown,
        key,
        code,
        modifiers,
    });
    test.sync_and_update();
}

/// C15, R7.1: with a filter focused, the window's chord reaches the window untyped, and the
/// extending press's held key reaches the window's `HeldKeys`; what is typed is the field's,
/// and Enter, Backspace and Delete never reach Local Changes' stage or discard around it.
/// Caught by: a field that claims every key, or one that hands its keys to the views around.
#[test]
fn a_filter_hands_the_windows_chords_and_held_keys_on_and_keeps_its_own() {
    let (mut test, heard, value) = launch(false);
    for nth in 0..accelerators::chords(Action::Refresh, Os::current())
        .iter()
        .count()
    {
        press_chord(&mut test, Action::Refresh, nth);
    }
    assert!(!heard.window.borrow().is_empty());
    assert!(
        heard
            .window
            .borrow()
            .iter()
            .all(|action| *action == Action::Refresh)
    );
    assert_eq!(value.peek().as_str(), "", "the window's chord typed");

    let (key, code, modifiers) = accelerators::chords(Action::ExtendSelection, Os::current())
        .first()
        .and_then(|chord| chord.press_hold())
        .expect("the extending chord is a press");
    key_down(&mut test, key, code, modifiers);
    assert_eq!(
        heard.held.borrow().press(),
        Some(Action::ExtendSelection),
        "the held key never reached the window"
    );

    test.write_text("abc");
    test.press_key(Key::Named(NamedKey::Backspace));
    test.sync_and_update();
    test.press_key(Key::Named(NamedKey::Enter));
    test.press_key(Key::Named(NamedKey::Delete));
    test.sync_and_update();
    assert_eq!(value.peek().as_str(), "ab");
    assert_eq!(
        heard.around.borrow().as_slice(),
        [],
        "a key typed in the field reached Local Changes' chords"
    );
}

/// C15, C16, the QA brief: in the commit box's description, Backspace deletes a character and
/// never discards, Enter makes a new line and never stages, and the commit chord commits
/// without a new line — claimed, so neither the view around nor the window acts on it; no
/// staging chord is the box's. Caught by: a description whose commit chord inserts a line
/// (Freya's multiline Enter ignores modifiers), or bare keys handed to Local Changes.
#[test]
fn in_the_commit_box_backspace_and_enter_edit_and_the_commit_chord_commits() {
    let (mut test, heard, value) = launch(true);
    test.write_text("subject");
    test.press_key(Key::Named(NamedKey::Backspace));
    test.sync_and_update();
    test.press_key(Key::Named(NamedKey::Enter));
    test.sync_and_update();
    test.write_text("body");
    assert_eq!(value.peek().as_str(), "subjec\nbody");

    press_chord(&mut test, Action::Commit, 0);
    assert_eq!(heard.own.borrow().as_slice(), [Action::Commit]);
    assert_eq!(
        value.peek().as_str(),
        "subjec\nbody",
        "the commit chord typed"
    );

    for action in [
        Action::StageOrUnstage,
        Action::StageOrUnstageAll,
        Action::Discard,
    ] {
        for nth in 0..accelerators::chords(action, Os::current()).iter().count() {
            press_chord(&mut test, action, nth);
        }
    }
    assert_eq!(
        heard.own.borrow().as_slice(),
        [Action::Commit],
        "a staging chord acted from the commit box"
    );
    assert_eq!(
        heard.around.borrow().as_slice(),
        [],
        "a key typed in the commit box reached Local Changes' chords"
    );
    assert_eq!(heard.window.borrow().as_slice(), []);
}
