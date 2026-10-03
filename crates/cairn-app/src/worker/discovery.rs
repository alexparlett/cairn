//! Finding `git` once per application (PRD R6.1, process-manager L10).
//!
//! `git` is found and its version checked as the application starts, on a
//! thread of its own, and every repository the application opens takes a
//! copy of that one answer — pointed at its own askpass channel
//! (`GitBinary::with_environment`), never searched for or probed again. A
//! `git` that is missing or too old is refused here, once, with the version
//! Cairn needs, and each repository that asks is told so in the same words.

use std::sync::{Arc, OnceLock};

use cairn_git::ops::GitBinary;

use super::startup::Startup;

/// The application's one answer to "which `git`, and is it new enough",
/// shared by cloning. Found on the first call to [`Discovery::git`], which
/// [`Discovery::start`] makes on a thread of its own at once.
#[derive(Clone)]
pub struct Discovery(Arc<Found>);

struct Found {
    startup: Startup,
    /// The `git` found, or the refusal as the window shows it.
    answer: OnceLock<Result<GitBinary, String>>,
}

/// The path and version found, or the refusal, and nothing of the
/// environment: its inherited roster carries values such as proxy URLs, which
/// may hold the user's proxy credentials and are never rendered.
impl std::fmt::Debug for Discovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let answer = self.0.answer.get().map(|answer| {
            answer
                .as_ref()
                .map(|git| (git.path().to_owned(), git.version()))
        });
        f.debug_struct("Discovery")
            .field("answer", &answer)
            .finish_non_exhaustive()
    }
}

impl Discovery {
    /// Starts finding `git` for this process, on a `cairn-discovery` thread,
    /// and returns at once: called on the main thread as the application
    /// starts, before there is a window. It resolves the helper's path
    /// (`std::env::current_exe`, a `readlink`) and spawns; it runs no `git`.
    pub fn start() -> Self {
        Self::start_with(Startup::of_this_process())
    }

    /// [`Discovery::start`] for the launch `startup` describes.
    pub(super) fn start_with(startup: Startup) -> Self {
        let discovery = Self::new(startup);
        let finding = discovery.clone();
        // If the thread cannot be started, the first repository to ask finds git itself.
        let _ = std::thread::Builder::new()
            .name("cairn-discovery".to_owned())
            .spawn(move || {
                let _ = finding.git();
            });
        discovery
    }

    /// Nothing found yet; the first [`Discovery::git`] finds it.
    pub(super) fn new(startup: Startup) -> Self {
        Self(Arc::new(Found {
            startup,
            answer: OnceLock::new(),
        }))
    }

    /// The `git` found, finding it now if nobody has: exactly once per
    /// `Discovery`, however many threads ask, and a thread that asks while
    /// another is finding it waits for that answer. It runs `git --version`,
    /// so a worker's call, never the UI thread's.
    pub(super) fn git(&self) -> Result<&GitBinary, &str> {
        self.0
            .answer
            .get_or_init(|| {
                GitBinary::discover_with(self.0.startup.probe_environment())
                    .map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(String::as_str)
    }

    /// How the application was launched, which each repository's channel and
    /// environment are built from.
    pub(super) fn startup(&self) -> &Startup {
        &self.0.startup
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a `Debug` that renders the found `git` whole, which carries its
    /// environment — the proxy URLs among it, which may hold a credential.
    #[test]
    fn a_discovery_renders_what_was_found_and_nothing_of_the_environment() {
        let proxy = "http://ada:hunter2@proxy.example.com:3128";
        let discovery = Discovery::new(Startup::new(
            move |name| match name {
                "PATH" => std::env::var_os("PATH"),
                "https_proxy" => Some(proxy.into()),
                _ => None,
            },
            std::path::PathBuf::from("/nonexistent/cairn-askpass"),
        ));
        let found = match discovery.git() {
            Ok(git) => git.clone(),
            Err(why) => panic!("this machine's git was refused: {why}"),
        };
        assert!(
            format!("{found:?}").contains("hunter2"),
            "the case needs the proxy in the environment git was found with"
        );
        let rendered = format!("{discovery:?}");
        assert!(!rendered.contains("hunter2"), "{rendered}");
        assert!(
            rendered.contains(&found.path().display().to_string()),
            "{rendered}"
        );
    }
}
