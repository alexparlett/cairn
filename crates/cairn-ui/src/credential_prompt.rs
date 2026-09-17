//! The dialog that answers a credential prompt from git or ssh.
//!
//! It says WHICH remote is asking and shows the prompt text itself — the URL
//! in it is the one being authenticated against, not a nickname that could
//! disagree with where the request goes (PRD product rule). What it asks for
//! decides how it asks: a username is typed in the open, a password or
//! passphrase masked, and ssh's host-key check is a question with a yes and
//! a no rather than a text field, so a password is never typed into it.
//!
//! The typed text leaves this component through `on_submit` as the `String`
//! the input held, moved rather than copied; the caller wraps it in
//! `cairn_model::Secret` at once, which is the first Cairn-owned type it
//! reaches. Freya's `Input` keeps its own editing buffers, which nothing
//! zeroes — a limit of the toolkit, recorded in `docs/systems/credentials.md`.

use cairn_model::{PromptKind, prompt_subject};
use freya::prelude::*;

/// Width of the dialog, wide enough for a URL to stay on one line.
const DIALOG_WIDTH: f32 = 520.0;

pub struct CredentialPrompt {
    remote: String,
    text: String,
    on_submit: EventHandler<String>,
    on_cancel: EventHandler<()>,
    key: DiffKey,
}

impl CredentialPrompt {
    /// `remote` is the remote the running operation was asked for; `text` is
    /// the prompt exactly as git or ssh handed it to the helper.
    pub fn new(remote: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            remote: remote.into(),
            text: text.into(),
            on_submit: EventHandler::new(|_| {}),
            on_cancel: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    /// What the user typed (or `yes` for an accepted confirmation), once.
    pub fn on_submit(mut self, on_submit: impl Into<EventHandler<String>>) -> Self {
        self.on_submit = on_submit.into();
        self
    }

    /// The user declined: Cancel, Escape, or a press outside the dialog.
    pub fn on_cancel(mut self, on_cancel: impl Into<EventHandler<()>>) -> Self {
        self.on_cancel = on_cancel.into();
        self
    }
}

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for CredentialPrompt {
    fn eq(&self, other: &Self) -> bool {
        self.remote == other.remote && self.text == other.text && self.key == other.key
    }
}

impl std::fmt::Debug for CredentialPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialPrompt")
            .field("remote", &self.remote)
            .field("text", &self.text)
            .finish_non_exhaustive()
    }
}

impl KeyExt for CredentialPrompt {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for CredentialPrompt {
    fn render(&self) -> impl IntoElement {
        let mut typed = use_state(String::new);
        let kind = PromptKind::of(&self.text);
        let colours = get_theme_or_default();
        let secondary = colours.read().colors().text_secondary;

        let on_cancel = self.on_cancel.clone();
        let cancel = move |_| on_cancel.call(());
        let on_submit = self.on_submit.clone();
        let submit = EventHandler::new(move |()| {
            // Moved out, not cloned: the buffer that held the characters is the one
            // the caller's `Secret` will zero.
            let answer = std::mem::take(&mut *typed.write());
            on_submit.call(answer);
        });
        let on_accept = self.on_submit.clone();
        let accept = move |_| on_accept.call(PromptKind::ACCEPTED.to_owned());

        let mut buttons = PopupButtons::new().child(
            Button::new()
                .on_press({
                    let on_cancel = self.on_cancel.clone();
                    move |_| on_cancel.call(())
                })
                .child("Cancel"),
        );
        let mut content = PopupContent::new().child(
            label()
                .text(asking(kind, &self.text))
                .width(Size::fill())
                .font_size(14.),
        );
        if kind == PromptKind::Confirmation {
            content = content.child(
                label()
                    .text(self.text.trim_end().to_owned())
                    .width(Size::fill())
                    .font_size(12.)
                    .color(secondary),
            );
            // What accepting costs, which the question itself does not say: ssh writes
            // the key to known_hosts and never asks about this host again, so a yes
            // here is a standing trust decision rather than a one-time answer.
            content = content.child(
                label()
                    .text(ACCEPTING_IS_PERMANENT.to_owned())
                    .width(Size::fill())
                    .font_size(12.)
                    .color(secondary),
            );
            buttons = buttons.child(
                Button::new()
                    .filled()
                    .on_press(accept)
                    .child("Yes, connect"),
            );
        } else {
            content = content
                .child(
                    label()
                        .text(self.text.trim_end().to_owned())
                        .width(Size::fill())
                        .font_size(12.)
                        .color(secondary),
                )
                .child(
                    Input::new(typed)
                        .width(Size::fill())
                        .auto_focus(true)
                        .mode(if kind.is_shown() {
                            InputMode::Shown
                        } else {
                            InputMode::new_password()
                        })
                        .on_submit({
                            let submit = submit.clone();
                            move |_: String| submit.call(())
                        }),
                );
            buttons = buttons.child(
                Button::new()
                    .filled()
                    .on_press(move |_| submit.call(()))
                    .child("Continue"),
            );
        }

        Popup::new()
            .width(Size::px(DIALOG_WIDTH))
            .on_close_request(cancel)
            .child(PopupTitle::new(format!(
                "{} is asking for a credential",
                self.remote
            )))
            .child(content)
            .child(buttons)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// Said beside every host-key question, because the question is asked once and
/// answered for good: ssh appends the key to `known_hosts` and consults it silently
/// from then on. Stated here rather than left to the user to know.
const ACCEPTING_IS_PERMANENT: &str =
    "Accepting adds this key to your known hosts. ssh will trust it from now on \
     without asking again.";

/// The one-line statement of what is wanted and from where.
fn asking(kind: PromptKind, text: &str) -> String {
    let subject = prompt_subject(text);
    match (kind, subject) {
        (PromptKind::Username, Some(url)) => format!("Username for {url}"),
        (PromptKind::Password, Some(url)) => format!("Password for {url}"),
        (PromptKind::Passphrase, Some(key)) => format!("Passphrase for the key {key}"),
        (PromptKind::Confirmation, Some(host)) => {
            format!("The host {host} is not yet known. Connect to it anyway?")
        }
        (PromptKind::Confirmation, None) => "Connect anyway?".to_owned(),
        (PromptKind::Username, None) => "Username".to_owned(),
        (PromptKind::Password, None) => "Password".to_owned(),
        (PromptKind::Passphrase, None) => "Passphrase".to_owned(),
        (PromptKind::Other, _) => "A secret is being asked for".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_of_prompt_is_stated_with_its_subject() {
        let cases = [
            ("Username for 'https://h/x': ", "Username for https://h/x"),
            (
                "Password for 'https://u@h/x': ",
                "Password for https://u@h/x",
            ),
            (
                "Enter passphrase for key '/k': ",
                "Passphrase for the key /k",
            ),
            (
                "The authenticity of host 'h (1.2.3.4)' can't be established.\n\
                 Are you sure you want to continue connecting (yes/no/[fingerprint])? ",
                "The host h (1.2.3.4) is not yet known. Connect to it anyway?",
            ),
            ("Token: ", "A secret is being asked for"),
        ];
        for (text, expected) in cases {
            assert_eq!(asking(PromptKind::of(text), text), expected, "{text:?}");
        }
    }

    #[test]
    fn a_prompt_that_quotes_nothing_is_still_stated() {
        for (text, expected) in [
            ("Username: ", "Username"),
            ("Password: ", "Password"),
            ("passphrase please", "Passphrase"),
            ("continue (yes/no)? ", "Connect anyway?"),
        ] {
            assert_eq!(asking(PromptKind::of(text), text), expected, "{text:?}");
        }
    }
}
