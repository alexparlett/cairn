//! Headless tests for the credential dialog: what it names, what it masks,
//! and how each kind of prompt is answered or declined. The typed text is
//! generated per test; nothing here contains a credential.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_ui::CredentialPrompt;
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 800.;
const HEIGHT: f32 = 600.;
const REMOTE: &str = "origin";
const URL: &str = "https://git.example.com/ada/engine";

/// What the dialog reported, in order.
#[derive(Default, Clone)]
struct Reports {
    submitted: Rc<RefCell<Vec<String>>>,
    cancelled: Rc<RefCell<usize>>,
}

fn launch(text: &'static str) -> (TestingRunner, Reports) {
    let reports = Reports::default();
    let (mut test, _) = TestingRunner::new(
        {
            let reports = reports.clone();
            move || {
                let submitted = reports.submitted.clone();
                let cancelled = reports.cancelled.clone();
                rect().expanded().child(
                    CredentialPrompt::new(REMOTE, text)
                        .on_submit(move |answer: String| submitted.borrow_mut().push(answer))
                        .on_cancel(move |()| *cancelled.borrow_mut() += 1),
                )
            }
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    // The popup animates in; a few frames settle it and apply the auto focus.
    for _ in 0..4 {
        test.sync_and_update();
    }
    (test, reports)
}

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

/// Every span of every paragraph: where an `Input` shows what it holds.
fn spans(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| {
        Paragraph::try_downcast(element).map(|p| {
            p.spans
                .iter()
                .map(|s| s.text.to_string())
                .collect::<String>()
        })
    })
}

fn has_text_field(test: &TestingRunner) -> bool {
    test.find(|_, element| Paragraph::try_downcast(element).filter(|p| p.cursor_index.is_some()))
        .is_some()
        || !spans(test).is_empty()
}

fn click_button(test: &mut TestingRunner, caption: &str) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == caption)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no button reads {caption:?}; labels: {:?}", labels(test)));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.sync_and_update();
}

fn generated() -> String {
    format!(
        "generated-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    )
}

/// PRD product rule: the dialog names the remote and shows the URL git is
/// authenticating against — the prompt text itself, not a stored nickname.
#[test]
fn a_password_prompt_names_the_remote_and_the_url_and_masks_what_is_typed() {
    let text: &'static str = Box::leak(format!("Password for '{URL}': ").into_boxed_str());
    let (mut test, reports) = launch(text);
    let shown = labels(&test);
    assert!(
        shown.iter().any(|l| l.contains(REMOTE)),
        "the dialog does not name the remote: {shown:?}"
    );
    assert!(
        shown.iter().any(|l| l == text.trim_end()),
        "the dialog does not show the prompt as git asked it: {shown:?}"
    );
    assert!(
        shown.iter().any(|l| l == &format!("Password for {URL}")),
        "the dialog does not say what is wanted from where: {shown:?}"
    );
    assert!(
        has_text_field(&test),
        "a password prompt has nowhere to type"
    );

    let secret = generated();
    test.write_text(&secret);
    let visible: Vec<String> = labels(&test).into_iter().chain(spans(&test)).collect();
    assert!(
        !visible.iter().any(|t| t.contains(&secret)),
        "the typed password is drawn in the open: {visible:?}"
    );
    assert!(
        spans(&test)
            .iter()
            .any(|s| s.len() == secret.len() && s.chars().all(|c| c == '*')),
        "no masked rendering of the typed password: {:?}",
        spans(&test)
    );

    test.press_key(Key::Named(NamedKey::Enter));
    assert_eq!(
        reports.submitted.borrow().as_slice(),
        std::slice::from_ref(&secret)
    );
    assert_eq!(*reports.cancelled.borrow(), 0);
}

#[test]
fn a_username_prompt_shows_what_is_typed_and_the_button_submits_it() {
    let text: &'static str = Box::leak(format!("Username for '{URL}': ").into_boxed_str());
    let (mut test, reports) = launch(text);
    let name = generated();
    test.write_text(&name);
    assert!(
        spans(&test).iter().any(|s| s == &name),
        "a username is typed in the open, but was not drawn: {:?}",
        spans(&test)
    );
    click_button(&mut test, "Continue");
    assert_eq!(reports.submitted.borrow().as_slice(), [name]);
}

#[test]
fn a_passphrase_prompt_names_the_key_and_masks_the_passphrase() {
    let text = "Enter passphrase for key '/home/ada/.ssh/id_ed25519': ";
    let (mut test, reports) = launch(text);
    assert!(
        labels(&test)
            .iter()
            .any(|l| l == "Passphrase for the key /home/ada/.ssh/id_ed25519"),
        "{:?}",
        labels(&test)
    );
    let secret = generated();
    test.write_text(&secret);
    assert!(!spans(&test).iter().any(|s| s.contains(&secret)));
    test.press_key(Key::Named(NamedKey::Enter));
    assert_eq!(reports.submitted.borrow().as_slice(), [secret]);
}

/// ssh's host-key check is a question, not a secret: no text field, a yes.
#[test]
fn a_host_key_confirmation_has_no_text_field_and_is_answered_yes() {
    let text = "The authenticity of host 'git.example.com (203.0.113.7)' can't be established.\n\
                ED25519 key fingerprint is SHA256:abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG.\n\
                Are you sure you want to continue connecting (yes/no/[fingerprint])? ";
    let (mut test, reports) = launch(text);
    assert!(
        !has_text_field(&test),
        "a confirmation was given a text field to type a password into"
    );
    let shown = labels(&test);
    assert!(
        shown
            .iter()
            .any(|l| l.contains("SHA256:abcdefghijklmnopqrstuvwxyz")),
        "the fingerprint is not shown: {shown:?}"
    );
    assert!(
        shown.iter().any(|l| l.contains("git.example.com")),
        "{shown:?}"
    );
    // The consequence the question leaves out: ssh remembers the key for good.
    assert!(
        shown
            .iter()
            .any(|l| l.contains("known hosts") && l.contains("without asking again")),
        "the dialog does not say that accepting is permanent: {shown:?}"
    );

    // Typing lands nowhere: nothing can be submitted by Enter.
    test.write_text("no");
    test.press_key(Key::Named(NamedKey::Enter));
    assert!(reports.submitted.borrow().is_empty());

    click_button(&mut test, "Yes, connect");
    assert_eq!(reports.submitted.borrow().as_slice(), ["yes"]);
}

#[test]
fn cancel_and_escape_both_decline_without_submitting() {
    let text: &'static str = Box::leak(format!("Password for '{URL}': ").into_boxed_str());
    let (mut test, reports) = launch(text);
    test.write_text(generated());
    click_button(&mut test, "Cancel");
    assert_eq!(*reports.cancelled.borrow(), 1);

    let (mut test, reports) = launch(text);
    test.press_key(Key::Named(NamedKey::Escape));
    assert_eq!(*reports.cancelled.borrow(), 1, "Escape did not decline");
    assert!(reports.submitted.borrow().is_empty());
}

/// An unrecognised prompt is treated as a secret: masked, with the generic sentence.
#[test]
fn an_unknown_prompt_is_masked_and_stated_generically() {
    let (mut test, reports) = launch("Token: ");
    assert!(
        labels(&test)
            .iter()
            .any(|l| l == "A secret is being asked for"),
        "{:?}",
        labels(&test)
    );
    let secret = generated();
    test.write_text(&secret);
    assert!(
        !spans(&test).iter().any(|s| s.contains(&secret)),
        "an unknown prompt's answer was drawn in the open: {:?}",
        spans(&test)
    );
    test.press_key(Key::Named(NamedKey::Enter));
    assert_eq!(
        reports.submitted.borrow().as_slice(),
        std::slice::from_ref(&secret)
    );
}

/// Caught by: dropping `on_close_request`, which is what a press outside the dialog reaches.
#[test]
fn a_press_outside_the_dialog_declines() {
    let text: &'static str = Box::leak(format!("Password for '{URL}': ").into_boxed_str());
    let (mut test, reports) = launch(text);
    // The dialog is centred and DIALOG_WIDTH wide; the top-left corner is the backdrop.
    test.click_cursor((5., 5.));
    test.sync_and_update();
    assert_eq!(
        *reports.cancelled.borrow(),
        1,
        "a press outside did not decline"
    );
    assert!(reports.submitted.borrow().is_empty());
}
