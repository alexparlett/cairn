//! Listing a repository's remotes, read through gitoxide.

use cairn_model::RemoteSummary;
use gix::bstr::ByteSlice;

use crate::Repository;

impl Repository {
    /// Every configured remote, the one `git fetch` would use with no
    /// argument first when there is one (the only remote, or `origin`), the
    /// rest by name. A remote whose URL is missing or cannot be parsed is
    /// listed without one rather than hiding the rest. A password embedded in
    /// the URL (`https://user:secret@host/`) is left out: the summary is shown
    /// and `Debug`-printed, and a credential is neither.
    pub fn remotes(&self) -> Vec<RemoteSummary> {
        let repo = self.inner();
        let default = repo.remote_default_name(gix::remote::Direction::Fetch);
        let mut names: Vec<_> = repo.remote_names().into_iter().collect();
        if let Some(default) = &default {
            names.sort_by_key(|name| name != default);
        }
        names
            .into_iter()
            .map(|name| {
                let url = repo
                    .try_find_remote(name.as_bstr())
                    .and_then(Result::ok)
                    .and_then(|remote| {
                        remote.url(gix::remote::Direction::Fetch).map(|url| {
                            let mut shown = url.clone();
                            shown.set_password(None);
                            shown.to_bstring().to_string()
                        })
                    });
                RemoteSummary {
                    name: name.to_string(),
                    url,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A repository with three remotes, built with `std::fs`: `zeta` first by name but
    /// not the default, `origin` the default, and `nowhere` with no URL at all.
    struct ThreeRemotes {
        path: PathBuf,
    }

    impl ThreeRemotes {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("cairn-three-remotes-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            for inside in ["objects/info", "objects/pack", "refs/heads", "refs/tags"] {
                std::fs::create_dir_all(path.join(inside)).unwrap();
            }
            std::fs::write(path.join("HEAD"), "ref: refs/heads/main\n").unwrap();
            std::fs::write(
                path.join("config"),
                "[core]\n\trepositoryformatversion = 0\n\tbare = true\n\
                 [remote \"zeta\"]\n\turl = https://alice:hunter2-not-a-real-secret@example.com/z.git\n\
                 \tfetch = +refs/heads/*:refs/remotes/zeta/*\n\
                 [remote \"origin\"]\n\turl = https://example.com/o.git\n\
                 \tfetch = +refs/heads/*:refs/remotes/origin/*\n\
                 [remote \"nowhere\"]\n\tfetch = +refs/heads/*:refs/remotes/nowhere/*\n",
            )
            .unwrap();
            Self { path }
        }
    }

    impl Drop for ThreeRemotes {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Caught by: dropping the sort (zeta would come first), showing the URL raw (the
    /// password would appear), or failing the whole list on the remote without a URL.
    #[test]
    fn the_default_is_first_a_password_is_left_out_and_a_missing_url_is_none() {
        let fixture = ThreeRemotes::new();
        let repo = Repository::discover(&fixture.path).unwrap();
        let remotes = repo.remotes();
        let names: Vec<&str> = remotes.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["origin", "nowhere", "zeta"]);
        assert_eq!(remotes[0].url.as_deref(), Some("https://example.com/o.git"));
        assert_eq!(
            remotes[1].url, None,
            "a remote without a URL was dropped or given one"
        );
        assert_eq!(
            remotes[2].url.as_deref(),
            Some("https://alice@example.com/z.git"),
            "the password embedded in the URL was shown"
        );
    }

    /// The Cairn checkout has an `origin`; what it points at is the developer's.
    #[test]
    fn this_checkout_lists_its_origin_with_a_url() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let remotes = repo.remotes();
        let origin = remotes
            .iter()
            .find(|remote| remote.name == "origin")
            .unwrap_or_else(|| panic!("no origin among {remotes:?}"));
        assert!(
            origin
                .url
                .as_deref()
                .is_some_and(|url| url.contains("cairn")),
            "origin's URL does not name the repository: {origin:?}"
        );
    }
}
