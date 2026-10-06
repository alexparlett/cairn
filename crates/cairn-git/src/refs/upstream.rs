//! A local branch's upstream, as git resolves `%(upstream)` (R1.4), read by hand from the
//! configuration (parity rules 4 and 5).
//!
//! git's rule (`set_merge` and `query_refspecs` in git's `remote.c`):
//!
//! - the remote is the LAST `branch.<name>.remote`, and the merge the FIRST
//!   `branch.<name>.merge`; no remote or no merge, no upstream;
//! - with the remote `.` — the repository itself — the upstream is the merge resolved as a
//!   ref name the way git resolves a name it is given (`main` is `refs/heads/main`); a
//!   name that resolves to no ref, or to more than one, is the upstream as written;
//! - with any other remote, the merge, LITERALLY as written, is matched against that
//!   remote's `remote.<remote>.fetch` refspecs in configuration order, a negative refspec
//!   or one with no destination skipped, and the first match's destination is the
//!   upstream; no match, no upstream.
//!
//! gix answers differently in four places, each measured against git 2.56.0: no upstream
//! for `remote = .` (it parses `.` as a URL with no refspecs; rule 4); the LAST `merge`
//! value; a short `merge` (`main`) expanded to `refs/heads/main` and mapped, where git maps
//! the literal and finds nothing; and, of two refspecs that both map the merge, not the
//! first in configuration order (rule 5). So none of gix's upstream API is used; only its
//! reading of the configuration and of refs.
//!
//! Whether the upstream exists — git's `[gone]` when it does not — is the ref resolved as
//! the snapshot resolves every ref: a symbolic upstream followed, one that resolves to
//! nothing or cannot be read gone.
//!
//! Residual: a remote defined by a file git reads in place of configuration
//! (`$GIT_DIR/remotes/<name>`, `$GIT_DIR/branches/<name>`) has fetch refspecs git reads and
//! this does not, so a branch tracking one has no upstream here; and the first of git's
//! name-resolution rules, the name itself, is tried only for a name under `refs/`, so a
//! `merge` naming a root ref such as `HEAD` with `remote = .` is taken as written.

use cairn_model::{RefName, Upstream};
use gix::bstr::{BStr, BString, ByteSlice, ByteVec};

use super::{Reading, Resolution, SYMREF_MAX_DEPTH};

/// The prefixes git tries, in order, to resolve a name it is given (`ref_rev_parse_rules`
/// in git's `refs.c`), beyond the name itself; the last one also takes a `/HEAD` suffix.
const NAME_RULES: [&str; 4] = ["refs/", "refs/tags/", "refs/heads/", "refs/remotes/"];

/// `branch`'s upstream, or `None` when it has none — no remote, no merge, or a merge the
/// remote's refspecs do not map.
pub(super) fn of(reading: &mut Reading<'_>, branch: &RefName) -> Option<Upstream> {
    let repo = reading.repo;
    let short = branch.as_str().strip_prefix("refs/heads/")?;
    let config = repo.config_snapshot();
    let config = config.plumbing();
    let short = BStr::new(short);
    let remote = config.string_by("branch", Some(short), "remote")?;
    let merge = config
        .strings_by("branch", Some(short), "merge")?
        .into_iter()
        .next()?;
    let name: BString = if remote == "." {
        resolve_name(reading, merge.as_ref()).unwrap_or(merge)
    } else {
        let fetch = config.strings_by("remote", Some(remote.as_bstr()), "fetch")?;
        fetch
            .iter()
            .find_map(|spec| tracking_destination(spec.as_ref(), merge.as_ref()))?
    };
    let upstream = RefName::new(name.to_string());
    let Ok(full) = gix::refs::FullName::try_from(name) else {
        return Some(Upstream::Gone { name: upstream });
    };
    Some(match exact(reading, &full) {
        Some(found) => match reading.resolve(&found) {
            Resolution::Listed { target, .. } => Upstream::Exists {
                name: upstream,
                commit: target.commit_id(),
            },
            Resolution::Dangling | Resolution::Unreadable => Upstream::Gone { name: upstream },
        },
        None => Upstream::Gone { name: upstream },
    })
}

/// The ref named exactly `name`, if there is one.
fn exact(reading: &mut Reading<'_>, name: &gix::refs::FullName) -> Option<gix::refs::Reference> {
    reading.cost.refs_read += 1;
    match reading.repo.try_find_reference(name.as_ref()) {
        Ok(Some(found)) if found.name() == name.as_ref() => Some(found.detach()),
        Ok(_) | Err(_) => None,
    }
}

/// `name` resolved to one ref as git resolves a name it is given (`repo_dwim_ref`): each
/// rule's candidate that resolves to an object counts, and the ref it resolves to — the
/// end of a symbolic chain — is the answer when exactly one does (or, with
/// `core.warnAmbiguousRefs` off, the first that does). `None` when none does, or more than
/// one: git then keeps the name as written.
fn resolve_name(reading: &mut Reading<'_>, name: &BStr) -> Option<BString> {
    let warn_ambiguous = reading
        .repo
        .config_snapshot()
        .plumbing()
        .boolean("core.warnAmbiguousRefs")
        .ok()
        .flatten()
        .unwrap_or(true);
    let mut candidates: Vec<BString> = Vec::new();
    if name.starts_with(b"refs/") {
        candidates.push(name.to_owned());
    }
    for prefix in NAME_RULES {
        let mut candidate = BString::from(prefix);
        candidate.push_str(name);
        candidates.push(candidate);
    }
    let mut head = BString::from("refs/remotes/");
    head.push_str(name);
    head.push_str("/HEAD");
    candidates.push(head);

    let mut found = None;
    let mut count = 0;
    for candidate in candidates {
        let Ok(full) = gix::refs::FullName::try_from(candidate) else {
            continue;
        };
        if let Some(end) = symbolic_end(reading, full) {
            count += 1;
            if found.is_none() {
                found = Some(end);
            }
            if !warn_ambiguous {
                break;
            }
        }
    }
    (count == 1 || (!warn_ambiguous && count > 0))
        .then_some(found)
        .flatten()
        .map(|end| end.as_bstr().to_owned())
}

/// The name at the end of `name`'s symbolic chain, when the chain ends at an object.
fn symbolic_end(
    reading: &mut Reading<'_>,
    name: gix::refs::FullName,
) -> Option<gix::refs::FullName> {
    let mut current = exact(reading, &name)?;
    for _ in 0..=SYMREF_MAX_DEPTH {
        match current.target {
            gix::refs::Target::Object(_) => return Some(current.name),
            gix::refs::Target::Symbolic(next) => current = exact(reading, &next)?,
        }
    }
    None
}

/// The destination one fetch refspec maps `merge` to, as git's `query_refspecs` maps it: a
/// negative refspec, or one with no destination, maps nothing; a pattern maps a name that
/// starts and ends as its source does, the middle carried into the destination's `*`;
/// anything else maps only its source, byte for byte.
fn tracking_destination(spec: &BStr, merge: &BStr) -> Option<BString> {
    let spec = spec.strip_prefix(b"+").unwrap_or(spec);
    if spec.starts_with(b"^") {
        return None;
    }
    let colon = spec.rfind_byte(b':')?;
    let (source, destination) = (&spec[..colon], &spec[colon + 1..]);
    if destination.is_empty() {
        return None;
    }
    let Some(star) = source.find_byte(b'*') else {
        return (source == merge.as_bytes()).then(|| destination.into());
    };
    let (prefix, suffix) = (&source[..star], &source[star + 1..]);
    if merge.len() < prefix.len() + suffix.len()
        || !merge.starts_with(prefix)
        || !merge.ends_with(suffix)
    {
        return None;
    }
    let middle = &merge[prefix.len()..merge.len() - suffix.len()];
    let into = destination.find_byte(b'*')?;
    let mut mapped = BString::from(&destination[..into]);
    mapped.push_str(middle);
    mapped.push_str(&destination[into + 1..]);
    Some(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn maps(spec: &str, merge: &str) -> Option<String> {
        tracking_destination(spec.into(), merge.into()).map(|name| name.to_string())
    }

    /// git's `query_refspecs` and `match_name_with_pattern`, case by case. Caught by: a
    /// short name expanded before matching, a negative or destination-less refspec taken,
    /// a pattern matched by its prefix alone, or the split made at the first colon.
    #[test]
    fn a_refspec_maps_the_merge_literally_as_git_maps_it() {
        let glob = "+refs/heads/*:refs/remotes/origin/*";
        assert_eq!(
            maps(glob, "refs/heads/main").as_deref(),
            Some("refs/remotes/origin/main")
        );
        assert_eq!(
            maps(glob, "refs/heads/a/b").as_deref(),
            Some("refs/remotes/origin/a/b")
        );
        assert_eq!(maps(glob, "main"), None, "a short merge was expanded");
        assert_eq!(
            maps("refs/heads/main:refs/remotes/first/main", "refs/heads/main").as_deref(),
            Some("refs/remotes/first/main")
        );
        assert_eq!(
            maps(
                "refs/heads/main:refs/remotes/first/main",
                "refs/heads/mainline"
            ),
            None
        );
        assert_eq!(maps("^refs/heads/main", "refs/heads/main"), None);
        assert_eq!(
            maps("refs/heads/main", "refs/heads/main"),
            None,
            "no destination"
        );
        assert_eq!(
            maps("refs/heads/main:", "refs/heads/main"),
            None,
            "an empty destination"
        );
        assert_eq!(
            maps(
                "refs/heads/pre-*-post:refs/remotes/o/x-*-y",
                "refs/heads/pre-mid-post"
            )
            .as_deref(),
            Some("refs/remotes/o/x-mid-y")
        );
        assert_eq!(
            maps(
                "refs/heads/pre-*-post:refs/remotes/o/*",
                "refs/heads/pre-post"
            ),
            None
        );
        assert_eq!(
            maps("refs/heads/a*:refs/remotes/o/*", "refs/heads/a").as_deref(),
            Some("refs/remotes/o/")
        );
    }
}
