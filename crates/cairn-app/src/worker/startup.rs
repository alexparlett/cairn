//! What the worker builds before it serves anything: the askpass channel,
//! the environment every `git` runs with, and the found `git` itself.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cairn_askpass::Channel;
use cairn_git::ops::{Askpass, GitBinary, GitEnvironment};

/// What `open` is given: how to read the launching environment, and where the
/// helper is. Both are decided on the calling thread and used on the worker's,
/// because opening the channel is filesystem work.
pub(super) struct Startup {
    /// Asked about the environment's inherited roster and nothing else.
    parent: ParentLookup,
    helper: PathBuf,
}

/// How the launching environment is read, by name.
type ParentLookup = Box<dyn Fn(&str) -> Option<OsString> + Send>;

impl Startup {
    pub(super) fn new(
        parent: impl Fn(&str) -> Option<OsString> + Send + 'static,
        helper: PathBuf,
    ) -> Self {
        Self {
            parent: Box::new(parent),
            helper,
        }
    }

    /// This process's environment and the helper installed beside its executable.
    pub(super) fn of_this_process() -> Self {
        Self::new(
            |name| std::env::var_os(name),
            helper_beside_this_executable(),
        )
    }
}

/// The helper installed beside this executable; a bare name, which git and ssh
/// search `PATH` for, if where this executable is cannot be known.
pub(super) fn helper_beside_this_executable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_owned))
        .map_or_else(
            || PathBuf::from(cairn_model::HELPER_PROGRAM),
            |directory| directory.join(cairn_model::HELPER_PROGRAM),
        )
}

/// The backend the worker threads share.
pub(super) struct Backend {
    pub(super) git: GitBinary,
    /// `None` when no channel could be opened; `prompting` says why.
    pub(super) channel: Option<Arc<Channel>>,
    /// `Err` names why no prompt can be answered in this session — the helper
    /// is not built beside the executable, or there is no runtime directory
    /// to put a socket in. Fetching still works where a credential helper or
    /// agent answers (L7); a prompt fails closed, and the failure says this.
    pub(super) prompting: Result<(), String>,
}

impl Backend {
    /// Opens the channel, builds the environment around it, and finds `git`.
    /// A missing or too-old `git` is the one refusal; everything about
    /// prompting degrades to a named reason instead.
    pub(super) fn open(startup: &Startup) -> Result<Self, cairn_git::Error> {
        let channel = match (startup.parent)("XDG_RUNTIME_DIR") {
            Some(runtime) => Channel::open(Path::new(&runtime))
                .map(Arc::new)
                .map_err(|error| error.to_string()),
            None => Err(
                "XDG_RUNTIME_DIR is not set, so there is nowhere to put the askpass \
                         socket"
                    .to_owned(),
            ),
        };
        let helper = if startup.helper.is_absolute() && !startup.helper.is_file() {
            Err(format!(
                "the askpass helper is not at {} (build it with `cargo build -p cairn-askpass`)",
                startup.helper.display()
            ))
        } else {
            Ok(())
        };
        let prompting = helper.and_then(|()| channel.as_ref().map(|_| ()).map_err(Clone::clone));
        let askpass = Askpass::new(
            startup.helper.clone(),
            channel
                .as_ref()
                .ok()
                .map(|channel| channel.socket_path().to_owned()),
        );
        let git = GitBinary::discover_with(GitEnvironment::new(&startup.parent, &askpass))?;
        Ok(Self {
            git,
            channel: channel.ok(),
            prompting,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GIT_ASKPASS names the helper installed beside this executable. The bare-name
    /// fallback runs only when `current_exe` fails, which a test cannot make happen.
    #[test]
    fn the_helper_is_named_beside_this_executable() {
        let helper = helper_beside_this_executable();
        let this = std::env::current_exe().unwrap();
        assert_eq!(helper.parent(), this.parent());
        assert_eq!(
            helper.file_name().and_then(|n| n.to_str()),
            Some(cairn_model::HELPER_PROGRAM)
        );
    }

    /// The channel's socket is what git is told to have the helper connect to.
    #[test]
    fn a_runtime_directory_gives_the_environment_a_socket_and_prompting() {
        let runtime = crate::worker::fetch_tests::RuntimeDir::new();
        let helper = crate::worker::fetch_tests::built_helper();
        let startup = Startup::new(
            {
                let runtime = runtime.path.clone();
                move |name| match name {
                    "PATH" => std::env::var_os("PATH"),
                    "XDG_RUNTIME_DIR" => Some(runtime.clone().into_os_string()),
                    _ => None,
                }
            },
            helper.clone(),
        );
        let backend = match Backend::open(&startup) {
            Ok(backend) => backend,
            Err(error) => panic!("no backend: {error}"),
        };
        assert_eq!(backend.prompting, Ok(()));
        let socket = backend
            .channel
            .as_ref()
            .map(|channel| channel.socket_path().to_owned());
        assert!(
            socket
                .as_ref()
                .is_some_and(|s| s.starts_with(&runtime.path)),
            "the socket {socket:?} is not under the runtime directory"
        );
        assert_eq!(
            backend
                .git
                .environment()
                .get(cairn_model::SOCKET_VARIABLE)
                .map(|s| s.to_owned()),
            socket.map(PathBuf::into_os_string)
        );
        assert_eq!(
            backend.git.environment().get("GIT_ASKPASS"),
            Some(helper.as_os_str())
        );
    }

    /// Without a runtime directory there is no channel, git is still found and
    /// still pointed at the helper (so a prompt fails closed), and the reason is kept.
    #[test]
    fn without_a_runtime_directory_git_is_still_found_and_the_reason_is_named() {
        let startup = Startup::new(
            |name| (name == "PATH").then(|| std::env::var_os("PATH")).flatten(),
            crate::worker::fetch_tests::built_helper(),
        );
        let backend = match Backend::open(&startup) {
            Ok(backend) => backend,
            Err(error) => panic!("no backend: {error}"),
        };
        assert!(backend.channel.is_none());
        assert!(
            backend
                .prompting
                .as_ref()
                .is_err_and(|why| why.contains("XDG_RUNTIME_DIR")),
            "{:?}",
            backend.prompting
        );
        assert_eq!(
            backend.git.environment().get(cairn_model::SOCKET_VARIABLE),
            None
        );
        assert!(backend.git.environment().get("GIT_ASKPASS").is_some());
    }

    /// A helper that is not built is named as the reason, with the command that builds it.
    #[test]
    fn a_missing_helper_is_named_with_how_to_build_it() {
        let runtime = crate::worker::fetch_tests::RuntimeDir::new();
        let startup = Startup::new(
            {
                let runtime = runtime.path.clone();
                move |name| match name {
                    "PATH" => std::env::var_os("PATH"),
                    "XDG_RUNTIME_DIR" => Some(runtime.clone().into_os_string()),
                    _ => None,
                }
            },
            PathBuf::from("/nonexistent/cairn/cairn-askpass"),
        );
        let backend = match Backend::open(&startup) {
            Ok(backend) => backend,
            Err(error) => panic!("no backend: {error}"),
        };
        assert!(
            backend
                .prompting
                .as_ref()
                .is_err_and(|why| why.contains("cargo build -p cairn-askpass")),
            "{:?}",
            backend.prompting
        );
        assert!(
            backend.channel.is_some(),
            "the channel is independent of the helper"
        );
    }
}
