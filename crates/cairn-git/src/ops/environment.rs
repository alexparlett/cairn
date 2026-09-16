//! The environment every `git` subprocess runs with.
//!
//! Built explicitly, never inherited: the shell that launched Cairn may carry
//! a `GIT_ASKPASS` meant for something else, a `GIT_DIR` pointing at another
//! repository, or a `GIT_CONFIG_GLOBAL` swapping the user's configuration out.
//! Every variable below is a deliberate entry with its reason beside it, and
//! the tests spell the whole set out, so adding one is a visible decision.
//!
//! This module is also the only place a [`Command`] is built, which is what
//! makes the rule structural: a process cannot exist without passing through
//! [`GitEnvironment::command`], and that clears the inherited environment
//! before applying this one.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

/// Set on every invocation, whatever the parent process has.
const ALWAYS: &[(&str, &str)] = &[
    // A GUI has no terminal, so git's own credential prompt would block forever
    // on nothing. With this, a missing credential is an error rather than a
    // frozen window.
    ("GIT_TERMINAL_PROMPT", "0"),
];

/// Copied from the parent process when present, and nothing else is. Each is
/// something `git`, or a program it runs on the user's behalf, needs to find
/// the user's own setup: a credential helper or ssh-agent that already works
/// must keep working.
///
/// Deliberately NOT here, and never will be without a decision: every `GIT_*`
/// variable (`GIT_DIR` would redirect the write, `GIT_CONFIG_GLOBAL` would swap
/// the user's configuration out, `GIT_ASKPASS` may be meant for something
/// else), which includes `GIT_SSH_COMMAND` and `GIT_SSH` — a user who sets
/// those in a shell rather than in `core.sshCommand` will find Cairn ignores
/// them, and that is the cost of never inheriting a git override. Proxy, CA
/// bundle, Kerberos, display and signing variables are open questions for the
/// user, not omissions.
const INHERITED: &[&str] = &[
    // Credential helpers, `ssh`, LFS filters and hooks are found on it.
    "PATH",
    // `~/.gitconfig`, `~/.git-credentials`, `~/.ssh/config`, `~/.ssh/known_hosts`.
    "HOME",
    // `$XDG_CONFIG_HOME/git/config` and `git/credentials`.
    "XDG_CONFIG_HOME",
    // The `credential-cache` helper keeps its socket under it.
    "XDG_CACHE_HOME",
    // The session bus, which `git-credential-libsecret` (and every other
    // Secret Service helper) needs to reach the keyring; the user the product
    // rule protects most is the one whose keyring already works.
    "DBUS_SESSION_BUS_ADDRESS",
    // Where the session bus falls back to (`$XDG_RUNTIME_DIR/bus`) when the
    // address is unset, and where Cairn's own askpass socket will live.
    "XDG_RUNTIME_DIR",
    // The running ssh-agent; without it every key asks for its passphrase.
    "SSH_AUTH_SOCK",
    // Where git writes its temporary files.
    "TMPDIR",
    // git's diagnostics reach the user in their own language; gettext reads
    // LANGUAGE first, then LC_ALL, LC_MESSAGES, LANG.
    "LANGUAGE",
    "LANG",
    "LC_ALL",
    "LC_MESSAGES",
];

/// Every variable a `git` subprocess will see. Deliberately not `Default`:
/// there is one way to build it, and it asks the parent process for the
/// [`INHERITED`] names only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitEnvironment {
    entries: BTreeMap<String, OsString>,
}

impl GitEnvironment {
    /// `parent` answers what the launching process has for a name. It is asked
    /// about the inherited roster and nothing else, so whatever it holds under
    /// another name cannot reach `git`.
    pub fn new(parent: impl Fn(&str) -> Option<OsString>) -> Self {
        let mut entries = BTreeMap::new();
        for name in INHERITED {
            if let Some(value) = parent(name) {
                entries.insert((*name).to_owned(), value);
            }
        }
        for (name, value) in ALWAYS {
            entries.insert((*name).to_owned(), OsString::from(value));
        }
        Self { entries }
    }

    /// The variables `git` will see, by name.
    pub fn variables(&self) -> impl Iterator<Item = (&str, &OsStr)> {
        self.entries
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
    }

    pub fn get(&self, name: &str) -> Option<&OsStr> {
        self.entries.get(name).map(OsString::as_os_str)
    }

    /// The only way a process is built: `program`, this environment, nothing inherited.
    pub(crate) fn command(&self, program: &Path) -> Command {
        let mut command = Command::new(program);
        command.env_clear();
        command.envs(&self.entries);
        command
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeSet;

    use super::*;

    fn names(environment: &GitEnvironment) -> Vec<&str> {
        environment.variables().map(|(name, _)| name).collect()
    }

    /// PRD B1: the whole set, spelled out. Adding a variable means editing this list.
    #[test]
    fn the_environment_is_exactly_the_deliberate_entries() {
        let environment =
            GitEnvironment::new(|name| Some(OsString::from(format!("parent-{name}"))));
        assert_eq!(
            names(&environment),
            [
                "DBUS_SESSION_BUS_ADDRESS",
                "GIT_TERMINAL_PROMPT",
                "HOME",
                "LANG",
                "LANGUAGE",
                "LC_ALL",
                "LC_MESSAGES",
                "PATH",
                "SSH_AUTH_SOCK",
                "TMPDIR",
                "XDG_CACHE_HOME",
                "XDG_CONFIG_HOME",
                "XDG_RUNTIME_DIR",
            ]
        );
        assert_eq!(environment.get("HOME"), Some(OsStr::new("parent-HOME")));
        assert_eq!(
            environment.get("GIT_TERMINAL_PROMPT"),
            Some(OsStr::new("0"))
        );
    }

    /// The parent is consulted for the inherited roster only; a value it holds under
    /// any other name is never even read.
    #[test]
    fn the_parent_is_asked_about_the_inherited_roster_and_nothing_else() {
        let asked = RefCell::new(BTreeSet::new());
        let environment = GitEnvironment::new(|name| {
            asked.borrow_mut().insert(name.to_owned());
            Some(OsString::from("x"))
        });
        let asked: Vec<String> = asked.into_inner().into_iter().collect();
        assert_eq!(
            asked,
            [
                "DBUS_SESSION_BUS_ADDRESS",
                "HOME",
                "LANG",
                "LANGUAGE",
                "LC_ALL",
                "LC_MESSAGES",
                "PATH",
                "SSH_AUTH_SOCK",
                "TMPDIR",
                "XDG_CACHE_HOME",
                "XDG_CONFIG_HOME",
                "XDG_RUNTIME_DIR",
            ]
        );
        for poison in [
            "GIT_ASKPASS",
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_CONFIG_GLOBAL",
            "GIT_SSH_COMMAND",
            "GIT_SSH",
            "SSH_ASKPASS",
            "DISPLAY",
            "LD_PRELOAD",
        ] {
            assert!(
                !asked.iter().any(|name| name == poison),
                "{poison} was read from the parent"
            );
            assert_eq!(environment.get(poison), None, "{poison} reached git");
        }
    }

    /// A parent that turned the prompt on does not get a say.
    #[test]
    fn the_terminal_prompt_is_off_whatever_the_parent_says() {
        let environment = GitEnvironment::new(|name| {
            (name == "GIT_TERMINAL_PROMPT").then(|| OsString::from("1"))
        });
        assert_eq!(
            environment.get("GIT_TERMINAL_PROMPT"),
            Some(OsStr::new("0"))
        );
    }

    #[test]
    fn an_absent_inherited_variable_is_left_out_rather_than_set_empty() {
        let environment = GitEnvironment::new(|_| None);
        assert_eq!(names(&environment), ["GIT_TERMINAL_PROMPT"]);
        assert_eq!(environment.get("HOME"), None);
    }
}
