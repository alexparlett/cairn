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
//! before applying this one. The method is visible to `process/` alone; the
//! runner is its one caller.
//!
//! Three kinds of entry make the base: the [`ALWAYS`] table, fixed for every
//! invocation; the [`INHERITED`] roster, copied from the parent when present;
//! and the askpass entries, which point git and ssh at Cairn's helper
//! ([`Askpass`]). On top of the base, the invocation's [`Profile`] decides
//! the rest: a read adds the [`READ_ONLY`] table and can carry no askpass
//! token, because its variant has nowhere to put one; a write carries the
//! token for the operation being run when it may prompt — the one variable
//! that differs between two invocations of the same kind, applied by
//! [`GitEnvironment::command`] because it is an invocation's property, not
//! the environment's.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

use cairn_model::{AskpassToken, SOCKET_VARIABLE, TOKEN_VARIABLE};

use super::Askpass;

/// Which environment an invocation runs with on top of the base; the kind of
/// invocation decides it (`cli.rs`), never the caller.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Profile<'a> {
    /// A read: [`READ_ONLY`] applied, and no askpass token — there is no field
    /// to carry one, so a read's helper always fails closed.
    Read,
    /// A write: the base, and the operation's askpass token when it may prompt.
    /// Without one the helper is still what git runs, and it fails closed.
    Write { token: Option<&'a AskpassToken> },
}

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
    // A GUI has no terminal, and git still launches a configured `core.editor`
    // with `TERM` unset — a `vim` on a pipe, or a `code --wait` window nobody
    // asked for — so a verb that wants an editor must fail rather than wait on
    // one. `false` and not `:`, because `:` silently accepts whatever message
    // git proposed: a merge message the user never read. GIT_EDITOR is the
    // first thing git's `editor.c` consults, ahead of `core.editor`, `VISUAL`
    // and `EDITOR` (evidence:
    // `docs/research/process-manager/platform-and-git-behaviour.md` C9).
    ("GIT_EDITOR", "false"),
    // Both, because the rebase todo list's editor is `GIT_SEQUENCE_EDITOR`,
    // then `sequence.editor`, and only then GIT_EDITOR, so a user's
    // `sequence.editor` would beat the line above. The verb whose purpose is
    // the editor needs a helper of Cairn's own here instead: a change to this
    // construction and its guard, argued when that verb is designed, never a
    // per-call override.
    ("GIT_SEQUENCE_EDITOR", "false"),
];

/// Added to a read's environment, on top of the base.
const READ_ONLY: &[(&str, &str)] = &[
    // `git status` refreshes the index's stat information and writes it back
    // when it can take the lock; with this it never takes the lock, so looking
    // at a repository never rewrites its index behind the user's back — nor
    // holds `index.lock` while the user's own `git commit` needs it. Only
    // `status` honours it: porcelain `diff` and `describe --dirty` refresh the
    // index anyway, which is why a read runs plumbing or `status` — or `diff
    // --no-index`, which reads no index — and nothing else (`crate::reads`; evidence:
    // `docs/research/process-manager/platform-and-git-behaviour.md` C3).
    ("GIT_OPTIONAL_LOCKS", "0"),
    // In a partial clone, asking for an object only the promisor remote holds
    // fetches it: a pack written and the network reached, from a read. With
    // this, git answers that the object is missing instead. Git older than
    // 2.44 ignores the variable, and the floor is 2.30, so on such a git a read
    // can still lazy-fetch: carrying no askpass token, it fails closed only
    // where the promisor needs a prompt, and one a configured credential
    // helper or the ssh agent answers fetches. That is a constraint the reads in `crate::reads`
    // design around, not one this line removes (decided by the user,
    // 2026-10-02: keep the floor, set the variable).
    ("GIT_NO_LAZY_FETCH", "1"),
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
/// them, and that is the cost of never inheriting a git override. The
/// exceptions are named one by one: `GIT_SSL_CAINFO` and `GIT_SSL_CAPATH`,
/// which name a CA bundle and nothing else (issue #18), and `GIT_AUTHOR_NAME`,
/// `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME` and `GIT_COMMITTER_EMAIL` — with
/// `EMAIL`, which is no `GIT_*` name, the five identity variables of
/// staging-and-commit L26, which name who a commit is by. Never
/// the date variables (`GIT_AUTHOR_DATE`, `GIT_COMMITTER_DATE`): a stale one
/// left in a shell would stamp every commit Cairn makes with it (R5.2).
/// `GIT_EDITOR` and `EDITOR` are not inherited either: [`ALWAYS`] pins the
/// editor to `false`.
///
/// Every local write runs the user's hooks, filters and signing programs with
/// this environment too (staging-and-commit R5), so it is what a `pre-commit`
/// hook and `gpg` see; each entry is a deliberate leak of the user's
/// environment, `destructive-ops-reviewer`'s check 9.
///
/// One roster for every invocation, a read's as a write's: a read's clean
/// filter or fsmonitor hook sees the display, signing and identity variables
/// too. Decided by the user on 2026-10-09: that is what the user's own `git`
/// gives those programs from a shell, so a filter or hook behaves under Cairn
/// as it does there, and a second roster for reads would be a second list to
/// keep right for no program that needs less.
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
    // Signing and the desktop, decided on #18 by staging-and-commit L11 (R5.2): a commit
    // under `commit.gpgSign` runs `gpg`, whose agent asks for a key's passphrase through
    // a pinentry on the user's desktop.
    //
    // GnuPG's home, when the user keeps their keys anywhere but `~/.gnupg`: without it
    // `gpg` finds no secret key and every signed commit fails.
    "GNUPGHOME",
    // The X display a graphical pinentry (and an `ssh-askpass` the user configured
    // themselves) opens its window on; without it the passphrase is never asked for.
    "DISPLAY",
    // The Wayland display, the same for a pinentry built for Wayland.
    "WAYLAND_DISPLAY",
    // Where the X cookie a pinentry presents to the display lives when it is not
    // `~/.Xauthority`: a path, never the cookie itself; without it the display refuses
    // the pinentry's connection.
    "XAUTHORITY",
    // Identity, staging-and-commit L26 (R5.2): the terminal's identity is the one Cairn
    // commits with, so a per-project identity set in the shell (direnv) is honoured
    // exactly as the user's own `git commit` honours it, ahead of `user.name`.
    //
    // The author's name a commit records, over `user.name` and `author.name`.
    "GIT_AUTHOR_NAME",
    // The author's email a commit records, over `user.email` and `author.email`.
    "GIT_AUTHOR_EMAIL",
    // The committer's name, over `user.name` and `committer.name`.
    "GIT_COMMITTER_NAME",
    // The committer's email, over `user.email` and `committer.email`.
    "GIT_COMMITTER_EMAIL",
    // git's fallback for an email no configuration sets, before it guesses one from the
    // host name.
    "EMAIL",
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

    /// The base every invocation sees, by name. A read's [`READ_ONLY`] entries
    /// and a write's token are not among them: they belong to an invocation.
    pub fn variables(&self) -> impl Iterator<Item = (&str, &OsStr)> {
        self.entries
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
    }

    pub fn get(&self, name: &str) -> Option<&OsStr> {
        self.entries.get(name).map(OsString::as_os_str)
    }

    /// The only way a process is built: `program`, this environment, nothing
    /// inherited, plus what `profile` adds — [`READ_ONLY`] for a read, the
    /// operation's askpass token for a write that may ask. Visible to
    /// `process/` alone: the runner is the one caller.
    pub(super) fn command(&self, program: &Path, profile: Profile<'_>) -> Command {
        let mut command = Command::new(program);
        command.env_clear();
        command.envs(&self.entries);
        match profile {
            Profile::Read => {
                command.envs(READ_ONLY.iter().copied());
            }
            Profile::Write { token: Some(token) } => {
                command.env(TOKEN_VARIABLE, token.as_str());
            }
            Profile::Write { token: None } => {}
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
                "DISPLAY",
                "EMAIL",
                "GIT_ASKPASS",
                "GIT_AUTHOR_EMAIL",
                "GIT_AUTHOR_NAME",
                "GIT_COMMITTER_EMAIL",
                "GIT_COMMITTER_NAME",
                "GIT_EDITOR",
                "GIT_SEQUENCE_EDITOR",
                "GIT_SSL_CAINFO",
                "GIT_SSL_CAPATH",
                "GIT_TERMINAL_PROMPT",
                "GNUPGHOME",
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
                "WAYLAND_DISPLAY",
                "XAUTHORITY",
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
        assert_eq!(environment.get("GIT_EDITOR"), Some(OsStr::new("false")));
        assert_eq!(
            environment.get("GIT_SEQUENCE_EDITOR"),
            Some(OsStr::new("false"))
        );
        for read_only in ["GIT_OPTIONAL_LOCKS", "GIT_NO_LAZY_FETCH"] {
            assert_eq!(
                environment.get(read_only),
                None,
                "{read_only} is a read's, not every invocation's"
            );
        }
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
                "GIT_EDITOR",
                "GIT_SEQUENCE_EDITOR",
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
                "DISPLAY",
                "EMAIL",
                "GIT_AUTHOR_EMAIL",
                "GIT_AUTHOR_NAME",
                "GIT_COMMITTER_EMAIL",
                "GIT_COMMITTER_NAME",
                "GIT_SSL_CAINFO",
                "GIT_SSL_CAPATH",
                "GNUPGHOME",
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
                "WAYLAND_DISPLAY",
                "XAUTHORITY",
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
            // A stale date would stamp every commit (staging-and-commit R5.2).
            "GIT_AUTHOR_DATE",
            "GIT_COMMITTER_DATE",
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
            "GIT_AUTHOR_DATE",
            "GIT_COMMITTER_DATE",
            "HTTP_PROXY",
            "CURL_CA_BUNDLE",
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

    /// A parent that turned the prompt on, pointed ssh elsewhere or named an
    /// editor does not get a say. The editor variables are never even asked
    /// about (the roster test above); the parent offers them here anyway, so a
    /// constructor that started reading them would be caught by its values.
    #[test]
    fn the_always_table_wins_whatever_the_parent_says() {
        let environment = GitEnvironment::new(
            |name| match name {
                "GIT_TERMINAL_PROMPT" => Some(OsString::from("1")),
                "SSH_ASKPASS_REQUIRE" => Some(OsString::from("never")),
                "GIT_EDITOR" | "GIT_SEQUENCE_EDITOR" => Some(OsString::from("vim")),
                _ => None,
            },
            &askpass(),
        );
        for (name, value) in [
            ("GIT_TERMINAL_PROMPT", "0"),
            ("SSH_ASKPASS_REQUIRE", "force"),
            ("GIT_EDITOR", "false"),
            ("GIT_SEQUENCE_EDITOR", "false"),
        ] {
            assert_eq!(environment.get(name), Some(OsStr::new(value)), "{name}");
        }
    }

    #[test]
    fn an_absent_inherited_variable_is_left_out_rather_than_set_empty() {
        let environment = GitEnvironment::new(|_| None, &askpass());
        assert_eq!(
            names(&environment),
            [
                "CAIRN_ASKPASS_SOCKET",
                "GIT_ASKPASS",
                "GIT_EDITOR",
                "GIT_SEQUENCE_EDITOR",
                "GIT_TERMINAL_PROMPT",
                "SSH_ASKPASS",
                "SSH_ASKPASS_REQUIRE",
            ]
        );
        assert_eq!(environment.get("HOME"), None);
    }

    /// What `command` sets, read back from the `Command` itself: after
    /// `env_clear`, every variable the child will see is one of these. What
    /// the child then actually sees is `cli.rs`'s and `ops/authority.rs`'s stub
    /// tests.
    fn applied(command: &Command) -> BTreeMap<String, String> {
        command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map_or_else(
                        || "<removed>".to_owned(),
                        |v| v.to_string_lossy().into_owned(),
                    ),
                )
            })
            .collect()
    }

    /// The parent of the two profile tests below: two inherited variables, so
    /// the base carries the roster's kind of entry as well as the fixed ones.
    fn profiled() -> GitEnvironment {
        GitEnvironment::new(
            |name| match name {
                "HOME" => Some(OsString::from("/home/someone")),
                "LANG" => Some(OsString::from("en_GB.UTF-8")),
                _ => None,
            },
            &askpass(),
        )
    }

    fn spelled(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    /// PRD G3, a read: the base, the editor pinned to `false`, optional locks
    /// and lazy fetching off, and no token — spelled out variable by variable,
    /// so adding one to either profile is an edit here.
    #[test]
    fn a_read_is_the_base_with_optional_locks_and_lazy_fetch_off_and_no_token() {
        let read = profiled().command(Path::new("git"), Profile::Read);
        assert_eq!(
            applied(&read),
            spelled(&[
                ("CAIRN_ASKPASS_SOCKET", "/run/user/1000/cairn-1-abc/askpass"),
                ("GIT_ASKPASS", "/opt/cairn/cairn-askpass"),
                ("GIT_EDITOR", "false"),
                ("GIT_NO_LAZY_FETCH", "1"),
                ("GIT_OPTIONAL_LOCKS", "0"),
                ("GIT_SEQUENCE_EDITOR", "false"),
                ("GIT_TERMINAL_PROMPT", "0"),
                ("HOME", "/home/someone"),
                ("LANG", "en_GB.UTF-8"),
                ("SSH_ASKPASS", "/opt/cairn/cairn-askpass"),
                ("SSH_ASKPASS_REQUIRE", "force"),
            ])
        );
        assert!(read.get_program() == "git");
    }

    /// PRD G3, a write: the same base with the editor pinned, no read-only
    /// variable, and the token exactly when the operation was given one.
    #[test]
    fn a_write_is_the_base_with_its_token_only_when_given() {
        let base = [
            ("CAIRN_ASKPASS_SOCKET", "/run/user/1000/cairn-1-abc/askpass"),
            ("GIT_ASKPASS", "/opt/cairn/cairn-askpass"),
            ("GIT_EDITOR", "false"),
            ("GIT_SEQUENCE_EDITOR", "false"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("HOME", "/home/someone"),
            ("LANG", "en_GB.UTF-8"),
            ("SSH_ASKPASS", "/opt/cairn/cairn-askpass"),
            ("SSH_ASKPASS_REQUIRE", "force"),
        ];
        let environment = profiled();

        let without = environment.command(Path::new("git"), Profile::Write { token: None });
        assert_eq!(applied(&without), spelled(&base));

        let token = AskpassToken::new("deadbeef");
        let with = environment.command(
            Path::new("git"),
            Profile::Write {
                token: Some(&token),
            },
        );
        let mut expected = spelled(&base);
        expected.insert("CAIRN_ASKPASS_TOKEN".to_owned(), "deadbeef".to_owned());
        assert_eq!(applied(&with), expected);
    }
}
