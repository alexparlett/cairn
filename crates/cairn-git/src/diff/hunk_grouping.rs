//! How the user's `git diff` groups a file's changes into hunks: `diff.context`, the
//! context it shows when `-U` is not given, and `diff.interHunkContext`, the extra unchanged
//! lines across which two hunks are merged into one.
//!
//! Both are porcelain's (`git_diff_ui_config` in git's `diff.c`), so `diff-tree`,
//! `diff-index` and `diff-files` — the plumbing the content query runs — never read them.
//! They are read here, as porcelain reads them. The context is what a view opens at
//! ([`crate::Repository::configured_context`]); the view's own context is the `-U<n>` git
//! is then asked at. The inter-hunk context is carried in the answer
//! ([`cairn_model::FunctionContext::inter_hunk_context`]) for the view to group hunks with
//! as the user's `git diff` groups them (`xdl_get_hunk`: merged across twice the context
//! plus it). git is not asked with it: the changed ranges do not depend on grouping, and
//! the function context a hunk carries depends only on where it starts — a merged hunk
//! starts where its first part does, at the start git printed for that part at the same
//! `-U<n>`. A value git refuses is [`Error::InvalidConfig`], since the user's `git diff`
//! refuses to run on it too: `git_config_int`'s parse (a bare key, a non-number and an
//! out-of-range value), and a negative number, which the callback rejects. Read from git's
//! source at v2.30.9 and v2.56.0, the same in both.

use super::git_config::{invalid, last_value, parse_int};
use crate::Error;

/// git's default for `diff.context`.
const DEFAULT_CONTEXT: u32 = 3;

/// The two keys, as the user's `git diff` reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Grouping {
    /// `diff.context`, or three: what `git diff` shows with no `-U`.
    pub(super) context: u32,
    /// `diff.interHunkContext`, or zero.
    pub(super) inter_hunk_context: u32,
}

impl Grouping {
    pub(super) fn read(repo: &gix::Repository) -> Result<Self, Error> {
        Self::from_config(repo.config_snapshot().plumbing())
    }

    fn from_config(file: &gix::config::File) -> Result<Self, Error> {
        Ok(Self {
            context: count(file, "context", "diff.context")?.unwrap_or(DEFAULT_CONTEXT),
            inter_hunk_context: count(file, "interhunkcontext", "diff.interHunkContext")?
                .unwrap_or(0),
        })
    }
}

/// `diff.<key>` as `git_config_int` reads it and the callback accepts it: `None` when it
/// is not set.
fn count(file: &gix::config::File, key: &str, name: &str) -> Result<Option<u32>, Error> {
    let Some(value) = last_value(file, "diff", None, key) else {
        return Ok(None);
    };
    value
        .as_deref()
        .and_then(|bytes| parse_int(bytes))
        .and_then(|number| u32::try_from(number).ok())
        .map(Some)
        .ok_or_else(|| invalid(name, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grouping(config: &str) -> Result<Grouping, Error> {
        Grouping::from_config(&gix::config::File::try_from(config).unwrap())
    }

    /// Unset is git's defaults; set is the last value given, read as git reads an `int`
    /// (case-insensitive key, octal, a unit suffix). Real git reading the same keys is
    /// `the_view_groups_hunks_as_the_users_git_diff_does`, in `tests/diff/parity.rs`.
    #[test]
    fn the_keys_read_as_porcelain_reads_them() {
        assert_eq!(
            grouping("").unwrap(),
            Grouping {
                context: 3,
                inter_hunk_context: 0
            }
        );
        assert_eq!(
            grouping("[diff]\n\tcontext = 1\n[Diff]\n\tCONTEXT = 010\n\tinterHunkContext = 2\n")
                .unwrap(),
            Grouping {
                context: 8,
                inter_hunk_context: 2
            }
        );
        assert_eq!(grouping("[diff]\n\tcontext = 0\n").unwrap().context, 0);
        assert_eq!(grouping("[diff]\n\tcontext = 1k\n").unwrap().context, 1024);
        assert_eq!(
            grouping("[diff \"x\"]\n\tcontext = 9\n").unwrap().context,
            3,
            "a driver's subsection is another key"
        );
    }

    /// What `git diff` dies on — measured with git 2.56: `abc`, the empty value, the bare
    /// key and `-1` for either key — is a refusal here too. Caught by: falling back to the
    /// default, which draws a diff the user's git refuses to.
    #[test]
    fn a_value_git_refuses_is_refused() {
        for config in [
            "[diff]\n\tcontext = abc\n",
            "[diff]\n\tcontext =\n",
            "[diff]\n\tcontext\n",
            "[diff]\n\tcontext = -1\n",
            "[diff]\n\tinterHunkContext = -1\n",
            "[diff]\n\tinterhunkcontext = 1x\n",
        ] {
            assert!(
                matches!(grouping(config), Err(Error::InvalidConfig { .. })),
                "{config:?}"
            );
        }
    }
}
