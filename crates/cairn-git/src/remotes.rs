//! Listing a repository's remotes, read through gitoxide.

use cairn_model::RemoteSummary;
use gix::bstr::ByteSlice;

use crate::{Error, Repository};

impl Repository {
    /// Every configured remote, the one `git fetch` would use with no
    /// argument first when there is one (the only remote, or `origin`), the
    /// rest by name. A remote whose URL cannot be parsed is listed without one
    /// rather than hiding the rest.
    pub fn remotes(&self) -> Result<Vec<RemoteSummary>, Error> {
        let repo = self.inner();
        let default = repo.remote_default_name(gix::remote::Direction::Fetch);
        let mut names: Vec<_> = repo.remote_names().into_iter().collect();
        if let Some(default) = &default {
            names.sort_by_key(|name| name != default);
        }
        Ok(names
            .into_iter()
            .map(|name| {
                let url = repo
                    .try_find_remote(name.as_bstr())
                    .and_then(Result::ok)
                    .and_then(|remote| {
                        remote
                            .url(gix::remote::Direction::Fetch)
                            .map(|url| url.to_bstring().to_string())
                    });
                RemoteSummary {
                    name: name.to_string(),
                    url,
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Cairn checkout has an `origin`; what it points at is the developer's.
    #[test]
    fn this_checkout_lists_its_origin_with_a_url() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let remotes = repo.remotes().unwrap();
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
        assert_eq!(remotes[0].name, "origin", "the default remote is not first");
    }
}
