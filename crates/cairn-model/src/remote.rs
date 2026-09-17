//! A remote as the UI needs to name it.

/// One configured remote: its name and, when the configuration gives one,
/// the URL it is fetched from. The name is what an operation is asked for;
/// the URL is shown so the user knows where a fetch will go before it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSummary {
    pub name: String,
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remote_without_a_fetch_url_is_still_a_remote() {
        let remote = RemoteSummary {
            name: "origin".to_owned(),
            url: None,
        };
        assert_eq!(remote.name, "origin");
        assert_eq!(remote.url, None);
        assert_eq!(remote.clone(), remote);
    }
}
