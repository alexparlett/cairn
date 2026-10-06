//! The stash list: the `refs/stash` reflog, newest first, as `git stash list` prints it
//! (R1.5).
//!
//! gix has nothing stash-specific; a stash is an entry of an ordinary ref's reflog. Its
//! newest-first reader (`log_iter().rev()`) reads through a 4 KiB window and stops for
//! good at the first longer line — a `git stash push -m` with a long message — so the log
//! is read whole, oldest first, and reversed (parity rule 3).

use cairn_model::StashEntry;

use super::Reading;
use crate::object_id::model_id;
use crate::{Cancel, Error};

/// Every stash entry, `stash@{0}` first. An entry whose line cannot be parsed, or whose
/// commit cannot be read, is skipped and counted; the others keep the index git gives
/// them, which is their place in the reflog.
pub(super) fn list(
    reading: &mut Reading<'_>,
    cancel: &impl Cancel,
) -> Result<Vec<StashEntry>, Error> {
    let repo = reading.repo;
    reading.cost.refs_read += 1;
    let stash = match repo.try_find_reference("refs/stash") {
        Ok(Some(stash)) => stash,
        Ok(None) => return Ok(Vec::new()),
        Err(_) => {
            reading.unreadable += 1;
            return Ok(Vec::new());
        }
    };
    let mut log = stash.log_iter();
    let lines = match log.all() {
        Ok(Some(lines)) => lines,
        Ok(None) => return Ok(Vec::new()),
        Err(_) => {
            reading.unreadable += 1;
            return Ok(Vec::new());
        }
    };
    // Oldest first, as the file holds them; `None` for a line that does not parse, which
    // still takes its place in the numbering.
    let mut oldest_first = Vec::new();
    for line in lines {
        if cancel.is_cancelled() {
            return Err(Error::RefsCancelled);
        }
        reading.cost.reflog_lines_read += 1;
        oldest_first.push(line.ok().and_then(|line| {
            let commit = gix::ObjectId::from_hex(line.new_oid).ok()?;
            Some((commit, line.message.to_string()))
        }));
    }
    let mut entries = Vec::with_capacity(oldest_first.len());
    for (index, line) in oldest_first.into_iter().rev().enumerate() {
        if cancel.is_cancelled() {
            return Err(Error::RefsCancelled);
        }
        let Some((commit, message)) = line else {
            reading.unreadable += 1;
            continue;
        };
        reading.cost.objects_read += 1;
        let base = repo
            .find_commit(commit)
            .ok()
            .and_then(|found| found.parent_ids().next().map(|parent| parent.detach()));
        match (model_id(&commit), base.map(|base| model_id(&base))) {
            (Ok(commit), Some(Ok(base))) => entries.push(StashEntry {
                index,
                message,
                commit,
                base,
            }),
            _ => reading.unreadable += 1,
        }
    }
    Ok(entries)
}
