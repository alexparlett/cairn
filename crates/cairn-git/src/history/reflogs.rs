//! What Show Lost Commits adds to a walk's tips (`docs/prd/staging-and-commit.md` R11.1,
//! R11.2): the old and the new id of every entry of `HEAD`'s reflog and of each local
//! branch's, as `git rev-list --reflog` and `git fsck` read a reflog — so the commit an
//! amend replaced is found even where the only entry naming it is the amend's own, as its
//! old id, in a log the amend itself created.
//!
//! Each log is read whole, as bytes, and each line parsed by git's own rules
//! (`show_one_reflog_ent` in git's `refs/files-backend.c`), never through gix's newest-first
//! reader, which reads through a 4 KiB window and stops for good at the first longer line —
//! a commit with a long subject writes one — nor gix's line parser, which refuses lines git
//! reads. git skips a line that does not parse, and a last line with no newline after it;
//! so does this. The null id — the old id of a log's first entry, the new id of a deletion —
//! names nothing; an id whose object is gone (pruned) or is not a commit is skipped, as
//! git's walk skips it (`handle_one_reflog_commit` in git's `revision.c`).

use std::collections::HashSet;

use cairn_model::RefName;
use gix::hash::ObjectId;

use crate::{Cancel, Error};

/// Every commit an entry of `HEAD`'s reflog or of one of `branches`' names, as its old or
/// its new id, each once, in the order the logs hold them — `HEAD`'s first, then each
/// branch's, oldest entry first. `None` once `cancel` fires, polled before each log and
/// before each id's object is looked up. A log that cannot be read is skipped, as a log
/// that is not there is.
pub(super) fn reflog_tips(
    repo: &gix::Repository,
    branches: &[RefName],
    cancel: &impl Cancel,
) -> Result<Option<Vec<ObjectId>>, Error> {
    let hex_len = repo.object_hash().len_in_hex();
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    let mut buffer = Vec::new();
    let names = std::iter::once("HEAD").chain(branches.iter().map(RefName::as_str));
    for name in names {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let Ok(name) = <&gix::refs::FullNameRef>::try_from(name) else {
            continue;
        };
        // Read whole into `buffer`; the forward iterator over it is not used.
        match repo.refs.reflog_iter(name, &mut buffer) {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => continue,
        }
        for (old, new) in entries(&buffer, hex_len) {
            for id in [old, new] {
                if !id.is_null() && seen.insert(id) {
                    ids.push(id);
                }
            }
        }
    }
    let mut commits = Vec::with_capacity(ids.len());
    for id in ids {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let is_commit = repo
            .find_header(id)
            .is_ok_and(|header| header.kind() == gix::object::Kind::Commit);
        if is_commit {
            commits.push(id);
        }
    }
    Ok(Some(commits))
}

/// The old and the new id of each entry of the reflog `log`, a whole log file's bytes, in
/// the file's order: each line git reads (`entry_ids`), and none it skips.
pub(super) fn entries(log: &[u8], hex_len: usize) -> impl Iterator<Item = (ObjectId, ObjectId)> {
    // `split_inclusive` keeps each line's newline: a last line without one is no entry.
    log.split_inclusive(|byte| *byte == b'\n')
        .filter(|line| line.ends_with(b"\n"))
        .filter_map(move |line| entry_ids(line, hex_len))
}

/// One reflog line's old and new ids, newline included, or `None` where git skips the line
/// as corrupt — its rule, `old SP new SP name <email> SP time SP tz TAB message LF`, checked
/// up to the time zone's four digits exactly as git checks it, nothing after them read.
fn entry_ids(line: &[u8], hex_len: usize) -> Option<(ObjectId, ObjectId)> {
    let old = ObjectId::from_hex(line.get(..hex_len)?).ok()?;
    let rest = line.get(hex_len..)?.strip_prefix(b" ")?;
    let new = ObjectId::from_hex(rest.get(..hex_len)?).ok()?;
    let rest = rest.get(hex_len..)?.strip_prefix(b" ")?;
    // git looks for the email's end with `strchr`, which stops at a NUL.
    let searched = rest.split(|byte| *byte == 0).next()?;
    let email_end = searched.iter().position(|byte| *byte == b'>')?;
    let rest = rest.get(email_end + 1..)?.strip_prefix(b" ")?;
    let after = nonzero_timestamp(rest)?;
    let zone = after.strip_prefix(b" ")?;
    match zone {
        [b'+' | b'-', a, b, c, d, ..]
            if [a, b, c, d].iter().all(|digit| digit.is_ascii_digit()) =>
        {
            Some((old, new))
        }
        _ => None,
    }
}

/// What follows the timestamp `strtoumax` reads at the start of `bytes` — leading white
/// space and a sign skipped, decimal digits — or `None` where it reads none, or reads zero,
/// both of which git takes for a corrupt line.
fn nonzero_timestamp(bytes: &[u8]) -> Option<&[u8]> {
    let start = bytes
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))?;
    let mut rest = bytes.get(start..)?;
    if let [b'+' | b'-', after @ ..] = rest {
        rest = after;
    }
    let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
    let (number, after) = rest.split_at(digits);
    if number.is_empty() || number.iter().all(|digit| *digit == b'0') {
        return None;
    }
    Some(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "1111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222";

    fn id(hex: &str) -> ObjectId {
        ObjectId::from_hex(hex.as_bytes()).unwrap()
    }

    fn read(log: &str) -> Vec<(ObjectId, ObjectId)> {
        entries(log.as_bytes(), 40).collect()
    }

    /// Each line git reads gives its two ids, whatever its message — a message far past
    /// gix's 4 KiB window, in the middle of the log, included — and each line git skips gives
    /// none, the lines after it still read. Caught by: a long line ending the read, a skipped
    /// line taking the rest with it, a last line without its newline read, or a rule of
    /// git's left out (each of the corrupt lines below is one git refuses).
    #[test]
    fn every_line_git_reads_is_an_entry_and_no_other() {
        let long = "x".repeat(10_000);
        let good = |old: &str, new: &str, message: &str| {
            format!("{old} {new} A U Thor <a@example.com> 1700000000 +0100\t{message}\n")
        };
        let log = [
            good(&"0".repeat(40), OLD, "branch: Created from HEAD"),
            good(OLD, NEW, &format!("commit: {long}")),
            // Corrupt, each as git's rules read it:
            format!("{OLD} {NEW} A U Thor <a@example.com> 0 +0100\tzero time\n"),
            format!("{OLD} {NEW} A U Thor a@example.com 1700000000 +0100\tno email end\n"),
            format!("{OLD} {NEW} A U Thor <a@example.com>1700000000 +0100\tno space\n"),
            format!("{OLD} {NEW} A U Thor <a@example.com> 1700000000 0100\tno sign\n"),
            format!("{OLD} {NEW} A U Thor <a@example.com> 1700000000 +01x0\tzone\n"),
            format!("{OLD}  {NEW} A U Thor <a@example.com> 1700000000 +0100\ttwo spaces\n"),
            format!(
                "{} {NEW} A U Thor <a@example.com> 1700000000 +0100\tshort\n",
                &OLD[1..]
            ),
            format!("{OLD} {NEW} A\0U <a@example.com> 1700000000 +0100\tNUL before email\n"),
            // Read, as git reads them: no tab, white space before the time, a signed time.
            format!("{NEW} {OLD} A U Thor <a@example.com>   1700000001 -0500\n"),
            format!("{NEW} {NEW} A U Thor <a@example.com> +1700000002 +0000\tsigned\n"),
            // The last line, without its newline: no entry.
            format!("{OLD} {OLD} A U Thor <a@example.com> 1700000003 +0000\tunfinished"),
        ]
        .concat();
        assert_eq!(
            read(&log),
            [
                (id(&"0".repeat(40)), id(OLD)),
                (id(OLD), id(NEW)),
                (id(NEW), id(OLD)),
                (id(NEW), id(NEW)),
            ]
        );
        assert_eq!(read(""), []);
    }
}
