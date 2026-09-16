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
const INHERITED: &[&str] = &[
    // Credential helpers, `ssh`, LFS filters and hooks are found on it.
    "PATH",
    // `~/.gitconfig`, `~/.git-credentials`, `~/.ssh/config`, `~/.ssh/known_hosts`.
    "HOME",
    // `$XDG_CONFIG_HOME/git/config` and `git/credentials`.
    "XDG_CONFIG_HOME",
    // The `credential-cache` helper keeps its socket under it.
    "XDG_CACHE_HOME",
    // The running ssh-agent; without it every key asks for its passphrase.
    "SSH_AUTH_SOCK",
    // Where git writes its temporary files.
    "TMPDIR",
    // git's diagnostics reach the user in their own language.
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
                "GIT_TERMINAL_PROMPT",
                "HOME",
                "LANG",
                "LC_ALL",
                "LC_MESSAGES",
                "PATH",
                "SSH_AUTH_SOCK",
                "TMPDIR",
                "XDG_CACHE_HOME",
                "XDG_CONFIG_HOME",
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
                "HOME",
                "LANG",
                "LC_ALL",
                "LC_MESSAGES",
                "PATH",
                "SSH_AUTH_SOCK",
                "TMPDIR",
                "XDG_CACHE_HOME",
                "XDG_CONFIG_HOME",
            ]
        );
        for poison in [
            "GIT_ASKPASS",
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_CONFIG_GLOBAL",
            "GIT_SSH_COMMAND",
            "SSH_ASKPASS",
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
