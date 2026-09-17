//! Reading what a credential prompt asks for, off the text git or ssh put on
//! the helper's `argv[1]`.
//!
//! A prompt is prose for a person, so this is a presentation decision — which
//! input to show, and what to call the thing being authenticated — and never
//! something an operation's outcome depends on. The dialog must show the whole
//! text as well: the URL in it is the one actually being authenticated
//! against, and a rendering that substituted a stored nickname could disagree
//! with where the request is going (PRD product rule).

/// What kind of answer a prompt wants, which decides how it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// git's `Username for 'https://host': `; typed in the open.
    Username,
    /// git's `Password for 'https://user@host': `; masked.
    Password,
    /// ssh's `Enter passphrase for key '/path': `; masked.
    Passphrase,
    /// ssh's host-key check, ending `(yes/no/[fingerprint])? `: a decision, not
    /// a secret. Typing it into a password field would answer the question with
    /// the password, so it gets its own rendering and is answered `yes`.
    Confirmation,
    /// Anything else; masked, since the safe default for unknown text is to
    /// treat it as a secret.
    Other,
}

impl PromptKind {
    /// Classifies `text` by the spellings git and OpenSSH use. Unknown text is
    /// [`PromptKind::Other`].
    pub fn of(text: &str) -> Self {
        let trimmed = text.trim_start();
        if text.contains("(yes/no") {
            Self::Confirmation
        } else if trimmed.starts_with("Username") {
            Self::Username
        } else if trimmed.starts_with("Password") {
            Self::Password
        } else if text.to_ascii_lowercase().contains("passphrase") {
            Self::Passphrase
        } else {
            Self::Other
        }
    }

    /// Whether typed characters are shown as typed.
    pub fn is_shown(self) -> bool {
        matches!(self, Self::Username)
    }

    /// The answer a confirmation is given when accepted; git and ssh spell it `yes`.
    pub const ACCEPTED: &str = "yes";
}

/// What the prompt is about: the first single-quoted span, which is where git
/// puts the URL, ssh the key path, and ssh's host-key check the host. `None`
/// when the text quotes nothing.
pub fn prompt_subject(text: &str) -> Option<&str> {
    let start = text.find('\'')? + 1;
    let length = text[start..].find('\'')?;
    Some(&text[start..start + length])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spellings_git_and_ssh_use_are_told_apart() {
        for (text, kind) in [
            ("Username for 'https://host': ", PromptKind::Username),
            ("Password for 'https://u@host': ", PromptKind::Password),
            (
                "Enter passphrase for key '/home/a/.ssh/id_ed25519': ",
                PromptKind::Passphrase,
            ),
            ("Enter passphrase for \"key\": ", PromptKind::Passphrase),
            (
                "The authenticity of host 'h (1.2.3.4)' can't be established.\n\
                 ED25519 key fingerprint is SHA256:abc.\n\
                 Are you sure you want to continue connecting (yes/no/[fingerprint])? ",
                PromptKind::Confirmation,
            ),
            ("Token: ", PromptKind::Other),
            ("", PromptKind::Other),
        ] {
            assert_eq!(PromptKind::of(text), kind, "{text:?}");
        }
    }

    /// Caught by: a confirmation classified by its first word, which is `The`.
    #[test]
    fn a_confirmation_that_mentions_a_passphrase_is_still_a_confirmation() {
        let text = "Bad passphrase, try again for key: no\nAre you sure (yes/no)? ";
        assert_eq!(PromptKind::of(text), PromptKind::Confirmation);
        assert!(!PromptKind::Confirmation.is_shown());
    }

    #[test]
    fn only_a_username_is_typed_in_the_open() {
        assert!(PromptKind::Username.is_shown());
        for hidden in [
            PromptKind::Password,
            PromptKind::Passphrase,
            PromptKind::Other,
            PromptKind::Confirmation,
        ] {
            assert!(!hidden.is_shown(), "{hidden:?} would be typed in the open");
        }
        assert_eq!(PromptKind::ACCEPTED, "yes");
    }

    #[test]
    fn the_subject_is_the_first_quoted_span() {
        assert_eq!(
            prompt_subject("Password for 'https://u@host/x': "),
            Some("https://u@host/x")
        );
        assert_eq!(
            prompt_subject("Enter passphrase for key '/k': "),
            Some("/k")
        );
        assert_eq!(
            prompt_subject("The authenticity of host 'h (1.2.3.4)' can't be established."),
            Some("h (1.2.3.4)")
        );
        assert_eq!(prompt_subject("Token: "), None);
        assert_eq!(prompt_subject("half 'quoted"), None);
    }
}
