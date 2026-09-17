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
//!
//! Three kinds of entry: the [`ALWAYS`] table, fixed for every invocation;
//! the [`INHERITED`] roster, copied from the parent when present; and the
//! askpass entries, which point git and ssh at Cairn's helper ([`Askpass`])
//! and, per invocation, carry the token for the operation being run — the one
//! variable that differs between two invocations, applied by
//! [`GitEnvironment::command`] because it is an invocation's property, not the
//! environment's.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use cairn_model::{AskpassToken, SOCKET_VARIABLE, TOKEN_VARIABLE};

use super::Askpass;

/// Set on every invocation, whatever the parent process has.
const ALWAYS: &[(&str, &str)] = &[
    // A GUI has no terminal, so git's own credential prompt would block forever
    // on nothing. With this, a missing credential is an error rather than a
    // frozen window.
    ("GIT_TERMINAL_PROMPT", "0"),
    // ssh consults SSH_ASKPASS only when it has no controlling terminal unless
    // told otherwise; `force` makes it ask the helper even from a Cairn
    // launched in a terminal, so a key passphrase never lands on a tty the
    // user is not looking at (evidence record; the OpenSSH floor is O5).
    ("SSH_ASKPASS_REQUIRE", "force"),
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
/// them, and that is the cost of never inheriting a git override. The two
/// exceptions, `GIT_SSL_CAINFO` and `GIT_SSL_CAPATH`, name a CA bundle and
/// nothing else (issue #18). Display (`DISPLAY`, `WAYLAND_DISPLAY`) and
/// signing (`GNUPGHOME`) variables remain open questions for the user on
/// that issue, not omissions; so does pinning `GIT_EDITOR` in [`ALWAYS`],
/// which is due with the first verb that can open an editor.
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
    // address is unset, and where Cairn's own askpass socket lives.
    "XDG_RUNTIME_DIR",
    // The running ssh-agent; without it every key asks for its passphrase.
    "SSH_AUTH_SOCK",
    // Where git writes its temporary files.
    "TMPDIR",
    // git's diagnostics reach the user in their own language; gettext reads
    // LANGUAGE first, then LC_ALL, LC_MESSAGES, LANG — and LC_CTYPE decides the
    // output charset, so a user who sets only that one still gets their
    // non-ASCII path names and messages intact.
    "LANGUAGE",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "LC_MESSAGES",
    // Transport, decided on issue #18: each configures how git reaches a
    // remote, and its absence is a "cannot connect" the user cannot diagnose
    // from Cairn. A proxy URL may carry the user's proxy credentials; they go
    // to git and nowhere else, since a `GitEnvironment` is never rendered,
    // logged or written.
    //
    // The proxy that git's HTTP transport (libcurl) reads; `http.proxy` in
    // git config overrides these (git-config(1), `http.proxy`), but a user
    // behind a corporate proxy usually has only the shell variables. Per
    // curl(1) ENVIRONMENT, the lower-case form wins when both are set, and
    // `http_proxy` exists in lower case ONLY — curl ignores `HTTP_PROXY`
    // because a CGI would set it from a request header — so that one name is
    // deliberately absent.
    "http_proxy",
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
    // A private CA, as OpenSSL reads it (openssl(7) ENVIRONMENT: the file and
    // directory its default verify paths come from), which libcurl's OpenSSL
    // backend falls back to when it was built with no CA bundle of its own and
    // nothing named one. `CURL_CA_BUNDLE`, which curl(1) documents beside
    // them, is the curl TOOL's and is read by neither libcurl nor git, so it is
    // deliberately absent: an entry that does nothing would only mislead.
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    // git's own spelling of the same two, overriding `http.sslCAInfo` and
    // `http.sslCAPath` (git-config(1)); the only `GIT_*` names on the roster,
    // and they name a CA bundle and nothing else.
    "GIT_SSL_CAINFO",
    "GIT_SSL_CAPATH",
    // Kerberos, for a remote authenticated over GSSAPI: the credential cache
    // (kinit(1), `KRB5CCNAME`) and the configuration (krb5.conf(5),
    // `KRB5_CONFIG`) the library under libcurl reads when either is not in
    // its default place.
    "KRB5CCNAME",
    "KRB5_CONFIG",
];

/// Every variable a `git` subprocess will see, but for the per-invocation
/// token. Deliberately not `Default`: there is one way to build it, and it
/// asks the parent process for the [`INHERITED`] names only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitEnvironment {
    entries: BTreeMap<String, OsString>,
}

impl GitEnvironment {
    /// `parent` answers what the launching process has for a name. It is asked
    /// about the inherited roster and nothing else, so whatever it holds under
    /// another name cannot reach `git`. `askpass` is where git and ssh send
    /// their questions; there is no environment without one.
    pub fn new(parent: impl Fn(&str) -> Option<OsString>, askpass: &Askpass) -> Self {
        let mut entries = BTreeMap::new();
        for name in INHERITED {
            if let Some(value) = parent(name) {
                entries.insert((*name).to_owned(), value);
            }
        }
        for (name, value) in ALWAYS {
            entries.insert((*name).to_owned(), OsString::from(value));
        }
        let helper = askpass.program().as_os_str().to_owned();
        entries.insert("GIT_ASKPASS".to_owned(), helper.clone());
        entries.insert("SSH_ASKPASS".to_owned(), helper);
        if let Some(socket) = askpass.socket() {
            entries.insert(SOCKET_VARIABLE.to_owned(), socket.as_os_str().to_owned());
        }
        Self { entries }
    }

    /// The variables `git` will see, by name; the per-invocation token is not
    /// among them, since it belongs to an invocation.
    pub fn variables(&self) -> impl Iterator<Item = (&str, &OsStr)> {
        self.entries
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
    }

    pub fn get(&self, name: &str) -> Option<&OsStr> {
        self.entries.get(name).map(OsString::as_os_str)
    }

    /// The only way a process is built: `program`, this environment, nothing
    /// inherited, plus the operation's askpass token when the invocation is one
    /// that may ask. Without a token the helper is still what git runs, and it
    /// fails closed.
    pub(crate) fn command(&self, program: &Path, token: Option<&AskpassToken>) -> Command {
        let mut command = Command::new(program);
        command.env_clear();
        command.envs(&self.entries);
        if let Some(token) = token {
            command.env(TOKEN_VARIABLE, token.as_str());
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::*;

    fn names(environment: &GitEnvironment) -> Vec<&str> {
        environment.variables().map(|(name, _)| name).collect()
    }

    fn askpass() -> Askpass {
        Askpass::new(
            "/opt/cairn/cairn-askpass",
            Some(PathBuf::from("/run/user/1000/cairn-1-abc/askpass")),
        )
    }

    /// PRD B1: the whole set, spelled out. Adding a variable means editing this list.
    #[test]
    fn the_environment_is_exactly_the_deliberate_entries() {
        let environment = GitEnvironment::new(
            |name| Some(OsString::from(format!("parent-{name}"))),
            &askpass(),
        );
        assert_eq!(
            names(&environment),
            [
                "ALL_PROXY",
                "CAIRN_ASKPASS_SOCKET",
                "DBUS_SESSION_BUS_ADDRESS",
                "GIT_ASKPASS",
                "GIT_SSL_CAINFO",
                "GIT_SSL_CAPATH",
                "GIT_TERMINAL_PROMPT",
                "HOME",
                "HTTPS_PROXY",
                "KRB5CCNAME",
                "KRB5_CONFIG",
                "LANG",
                "LANGUAGE",
                "LC_ALL",
                "LC_CTYPE",
                "LC_MESSAGES",
                "NO_PROXY",
                "PATH",
                "SSH_ASKPASS",
                "SSH_ASKPASS_REQUIRE",
                "SSH_AUTH_SOCK",
                "SSL_CERT_DIR",
                "SSL_CERT_FILE",
                "TMPDIR",
                "XDG_CACHE_HOME",
                "XDG_CONFIG_HOME",
                "XDG_RUNTIME_DIR",
                "all_proxy",
                "http_proxy",
                "https_proxy",
                "no_proxy",
            ]
        );
        assert_eq!(environment.get("HOME"), Some(OsStr::new("parent-HOME")));
        assert_eq!(
            environment.get("GIT_TERMINAL_PROMPT"),
            Some(OsStr::new("0"))
        );
        assert_eq!(
            environment.get("SSH_ASKPASS_REQUIRE"),
            Some(OsStr::new("force"))
        );
        assert_eq!(
            environment.get("GIT_ASKPASS"),
            Some(OsStr::new("/opt/cairn/cairn-askpass"))
        );
        assert_eq!(
            environment.get("SSH_ASKPASS"),
            Some(OsStr::new("/opt/cairn/cairn-askpass"))
        );
        assert_eq!(
            environment.get("CAIRN_ASKPASS_SOCKET"),
            Some(OsStr::new("/run/user/1000/cairn-1-abc/askpass"))
        );
        assert_eq!(
            environment.get("CAIRN_ASKPASS_TOKEN"),
            None,
            "the token is an invocation's, not the environment's"
        );
    }

    /// PRD R3.2, R3.3: the helper is named whether or not there is a channel,
    /// so git and ssh never fall back to a terminal; without a channel there
    /// is simply no socket for it to find.
    #[test]
    fn without_a_channel_the_helper_is_still_named_and_the_socket_is_not() {
        let environment =
            GitEnvironment::new(|_| None, &Askpass::new("/opt/cairn/cairn-askpass", None));
        assert_eq!(
            names(&environment),
            [
                "GIT_ASKPASS",
                "GIT_TERMINAL_PROMPT",
                "SSH_ASKPASS",
                "SSH_ASKPASS_REQUIRE",
            ]
        );
        assert_eq!(environment.get("CAIRN_ASKPASS_SOCKET"), None);
        let without = Askpass::new("/opt/cairn/cairn-askpass", None);
        assert_eq!(without.program(), Path::new("/opt/cairn/cairn-askpass"));
        assert_eq!(without.socket(), None);
        assert_eq!(
            askpass().socket(),
            Some(Path::new("/run/user/1000/cairn-1-abc/askpass"))
        );
    }

    /// The parent is consulted for the inherited roster only; a value it holds under
    /// any other name is never even read.
    #[test]
    fn the_parent_is_asked_about_the_inherited_roster_and_nothing_else() {
        let asked = RefCell::new(BTreeSet::new());
        let environment = GitEnvironment::new(
            |name| {
                asked.borrow_mut().insert(name.to_owned());
                Some(OsString::from("x"))
            },
            &askpass(),
        );
        let asked: Vec<String> = asked.into_inner().into_iter().collect();
        assert_eq!(
            asked,
            [
                "ALL_PROXY",
                "DBUS_SESSION_BUS_ADDRESS",
                "GIT_SSL_CAINFO",
                "GIT_SSL_CAPATH",
                "HOME",
                "HTTPS_PROXY",
                "KRB5CCNAME",
                "KRB5_CONFIG",
                "LANG",
                "LANGUAGE",
                "LC_ALL",
                "LC_CTYPE",
                "LC_MESSAGES",
                "NO_PROXY",
                "PATH",
                "SSH_AUTH_SOCK",
                "SSL_CERT_DIR",
                "SSL_CERT_FILE",
                "TMPDIR",
                "XDG_CACHE_HOME",
                "XDG_CONFIG_HOME",
                "XDG_RUNTIME_DIR",
                "all_proxy",
                "http_proxy",
                "https_proxy",
                "no_proxy",
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
            "SSH_ASKPASS_REQUIRE",
            "CAIRN_ASKPASS_SOCKET",
            "CAIRN_ASKPASS_TOKEN",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "GNUPGHOME",
            "GIT_EDITOR",
            "EDITOR",
            // curl reads the proxy for plain HTTP in lower case only (httpoxy).
            "HTTP_PROXY",
            // The curl tool's, never libcurl's or git's.
            "CURL_CA_BUNDLE",
            "GIT_SSL_NO_VERIFY",
            "GIT_SSL_CERT",
            "GIT_SSL_KEY",
            "LD_PRELOAD",
        ] {
            assert!(
                !asked.iter().any(|name| name == poison),
                "{poison} was read from the parent"
            );
        }
        for poison in [
            "GIT_DIR",
            "GIT_CONFIG_GLOBAL",
            "DISPLAY",
            "HTTP_PROXY",
            "CURL_CA_BUNDLE",
            "GNUPGHOME",
            "CAIRN_ASKPASS_TOKEN",
        ] {
            assert_eq!(environment.get(poison), None, "{poison} reached git");
        }
        // The parent's askpass, whatever it was for, is replaced by Cairn's own.
        assert_eq!(
            environment.get("GIT_ASKPASS"),
            Some(OsStr::new("/opt/cairn/cairn-askpass"))
        );
    }

    /// A parent that turned the prompt on, or pointed ssh elsewhere, does not get a say.
    #[test]
    fn the_always_table_wins_whatever_the_parent_says() {
        let environment = GitEnvironment::new(
            |name| match name {
                "GIT_TERMINAL_PROMPT" => Some(OsString::from("1")),
                "SSH_ASKPASS_REQUIRE" => Some(OsString::from("never")),
                _ => None,
            },
            &askpass(),
        );
        assert_eq!(
            environment.get("GIT_TERMINAL_PROMPT"),
            Some(OsStr::new("0"))
        );
        assert_eq!(
            environment.get("SSH_ASKPASS_REQUIRE"),
            Some(OsStr::new("force"))
        );
    }

    #[test]
    fn an_absent_inherited_variable_is_left_out_rather_than_set_empty() {
        let environment = GitEnvironment::new(|_| None, &askpass());
        assert_eq!(
            names(&environment),
            [
                "CAIRN_ASKPASS_SOCKET",
                "GIT_ASKPASS",
                "GIT_TERMINAL_PROMPT",
                "SSH_ASKPASS",
                "SSH_ASKPASS_REQUIRE",
            ]
        );
        assert_eq!(environment.get("HOME"), None);
    }

    /// The token is applied by `command`, and only when given. Read back from the
    /// `Command` itself; what the child then sees is `cli.rs`'s stub test.
    #[test]
    fn the_token_is_set_on_the_invocation_and_only_when_given() {
        let environment = GitEnvironment::new(|_| None, &askpass());
        let token = AskpassToken::new("deadbeef");
        let with = environment.command(Path::new("git"), Some(&token));
        let set: BTreeMap<_, _> = with
            .get_envs()
            .map(|(k, v)| (k.to_owned(), v.map(OsStr::to_owned)))
            .collect();
        assert_eq!(
            set.get(OsStr::new("CAIRN_ASKPASS_TOKEN")),
            Some(&Some(OsString::from("deadbeef")))
        );
        assert_eq!(
            set.get(OsStr::new("GIT_ASKPASS")),
            Some(&Some(OsString::from("/opt/cairn/cairn-askpass")))
        );
        assert!(with.get_program() == "git");

        let without = environment.command(Path::new("git"), None);
        assert!(
            without
                .get_envs()
                .all(|(name, _)| name != OsStr::new("CAIRN_ASKPASS_TOKEN")),
            "a token was set for an invocation that has none"
        );
    }
}
