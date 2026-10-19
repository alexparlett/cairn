//! Reading one commit in the detail the Commit tab shows.

use cairn_model::{CommitDetails, Oid, Signature, Timestamp};
use gix::objs::Find as _;

use crate::commit_encoding::CommitEncoding;
use crate::history::walk::Grafted;
use crate::object_id::object_id;
use crate::{Error, Repository};

impl Repository {
    /// The whole record of one commit: both signatures with their offsets, every parent in
    /// git's order, and the message exactly as written — in the characters git shows, read
    /// through the encoding the commit names (`crate::commit_encoding`).
    ///
    /// Beside a [`crate::HistoryPage`]'s rows, not instead of them: a row of a ten-year
    /// monorepo keeps a subject, a name and a parent count, and this pays for everything —
    /// every parent, both addresses, the whole message.
    pub fn commit_details(&self, id: &Oid) -> Result<CommitDetails, Error> {
        details_of(&Grafted::read(self.inner())?, id)
    }
}

/// `id`'s details as `git show -s` and `git log` read the commit, shared with the changes
/// query, which reads the same commit to find its first parent, and Recent Commit Messages.
///
/// The commit is read through `objects` (`Grafted`), as git reads it: a commit a replace ref
/// replaces is its replacement's author, committer, message and parents — git looks the
/// replacement up wherever it reads an object (`lookup_replace_object` in `odb.c`'s
/// `do_oid_object_info_extended` and `object.c`'s `parse_object_with_flags`), unless
/// `core.useReplaceRefs` is false; and the parents are the ones git shows: a shallow clone's
/// boundary commit names parents the clone does not have, and git's own `git log` reads the
/// shallow file and shows none (`%P` is empty), so neither does this, and a commit the graft
/// file grafts shows the graft's parents (`parse_commit_buffer` in `commit.c`; `crate::grafts`).
pub(crate) fn details_of(objects: &Grafted<'_>, id: &Oid) -> Result<CommitDetails, Error> {
    let read = |source: Box<dyn std::error::Error + Send + Sync>| Error::ReadCommit {
        id: id.to_string(),
        source,
    };
    let mut buffer = Vec::new();
    let (decoded, bytes) = read_commit(objects, id, &mut buffer)?;
    let parents = decoded
        .parents()
        .map(|parent| crate::object_id::model_id(&parent))
        .collect::<Result<Vec<_>, Error>>()?;
    let encoding = CommitEncoding::of_decoded(&decoded, bytes);
    let author = signature_of(
        decoded.author().map_err(|e| read(Box::new(e)))?,
        encoding,
        id,
    )?;
    let committer = signature_of(
        decoded.committer().map_err(|e| read(Box::new(e)))?,
        encoding,
        id,
    )?;
    // The raw message, not `message()`: the Commit tab shows the message as written, and
    // `message()` splits it into a summary and a body.
    Ok(CommitDetails {
        id: *id,
        parents,
        author,
        committer,
        message: encoding.text(decoded.message),
    })
}

/// `id` read as a commit through `objects` into `buffer` — git's commit, through a replace ref
/// and a graft — decoded, beside the bytes it was decoded from. One that is missing, or not a
/// commit, is [`Error::ReadCommit`], as is a replacement git would die on.
pub(crate) fn read_commit<'b>(
    objects: &Grafted<'_>,
    id: &Oid,
    buffer: &'b mut Vec<u8>,
) -> Result<(gix::objs::CommitRef<'b>, &'b [u8]), Error> {
    let read = |source: Box<dyn std::error::Error + Send + Sync>| Error::ReadCommit {
        id: id.to_string(),
        source,
    };
    let data = objects
        .try_find(&object_id(id)?, buffer)
        .map_err(read)?
        .ok_or_else(|| read(Box::new(NotACommit(*id))))?;
    if data.kind != gix::objs::Kind::Commit {
        return Err(read(Box::new(NotACommit(*id))));
    }
    let decoded = gix::objs::CommitRef::from_bytes(data.data, data.object_hash)
        .map_err(|e| read(Box::new(e)))?;
    Ok((decoded, data.data))
}

/// What reading a commit's details fails with where `id` names no commit as git reads it.
#[derive(Debug, thiserror::Error)]
#[error("{0} is not a commit in this repository")]
pub(crate) struct NotACommit(pub(crate) Oid);

/// `signature` as git prints it: its name and address in the commit's encoding.
fn signature_of(
    signature: gix::actor::SignatureRef<'_>,
    encoding: CommitEncoding,
    id: &Oid,
) -> Result<Signature, Error> {
    let time = signature.time().map_err(|source| Error::ReadCommit {
        id: id.to_string(),
        source: Box::new(source),
    })?;
    Ok(Signature {
        name: encoding.text(signature.name),
        email: encoding.text(signature.email),
        time: Timestamp::new(time.seconds, time.offset),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HistoryRequest;
    use cairn_model::{CommitSummary, History, RowContent};

    /// The first row's commit, with no wildcard arm: a second kind of row must fail to
    /// compile.
    fn first_commit(history: &History) -> CommitSummary {
        match history.row(0).expect("a commit").content() {
            RowContent::Commit(commit) => commit,
            RowContent::Stash(stash) => panic!("a walk from HEAD drew a stash: {stash:?}"),
        }
    }

    /// Against this checkout: whatever the head commit is, the details agree with the
    /// summary the history walk built from the same object, and carry what the summary
    /// does not.
    #[test]
    fn the_details_of_a_commit_agree_with_its_summary_and_carry_more() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let page = repo
            .history(&HistoryRequest::from_head(1), &crate::CancelSignal::new())
            .unwrap();
        let mut history = History::new();
        history.append(page.rows).unwrap();
        let summary = first_commit(&history);

        let details = repo.commit_details(&summary.id).unwrap();
        assert_eq!(details.id, summary.id);
        assert_eq!(details.parents.len(), summary.parent_count);
        assert_eq!(details.author.name, summary.author_name);
        assert!(
            !details.author.email.is_empty(),
            "the details carry the author's address, which a row does not"
        );
        assert_eq!(details.author.time.seconds, summary.author_time);
        assert_eq!(
            details.subject(),
            summary.summary,
            "the details' subject is not the summary the list draws"
        );
        assert!(
            !details.committer.name.is_empty(),
            "a commit always records a committer, which CommitSummary does not carry"
        );
        assert!(
            details.message.starts_with(&summary.summary),
            "the whole message must begin with its own subject"
        );
    }

    #[test]
    fn an_id_that_names_no_commit_is_refused() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let missing = Oid::parse("1234567890abcdef1234567890abcdef12345678").unwrap();
        let error = repo.commit_details(&missing).unwrap_err();
        assert!(matches!(error, Error::ReadCommit { .. }), "got {error:?}");
    }
}
