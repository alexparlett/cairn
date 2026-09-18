//! Reading one commit in the detail the Commit tab shows (R1.8, R5.3).

use cairn_model::{CommitDetails, Oid, Signature, Timestamp};

use crate::object_id::{model_id, object_id};
use crate::{Error, Repository};

impl Repository {
    /// The whole record of one commit: both signatures with their offsets, every parent in
    /// git's order, and the message exactly as written.
    ///
    /// Beside [`crate::HistoryPage`]'s `CommitSummary`, not instead of it: a row of a
    /// ten-year monorepo pays for a subject and one name, and this pays for everything.
    pub fn commit_details(&self, id: &Oid) -> Result<CommitDetails, Error> {
        let commit =
            self.inner()
                .find_commit(object_id(id)?)
                .map_err(|source| Error::ReadCommit {
                    id: id.to_string(),
                    source: Box::new(source),
                })?;
        details_of(&commit, id)
    }
}

/// Shared with the changes query, which reads the same commit to find its first parent.
pub(crate) fn details_of(commit: &gix::Commit<'_>, id: &Oid) -> Result<CommitDetails, Error> {
    let read = |source: Box<dyn std::error::Error + Send + Sync>| Error::ReadCommit {
        id: id.to_string(),
        source,
    };
    let mut parents = Vec::new();
    for parent in commit.parent_ids() {
        parents.push(model_id(&parent)?);
    }
    let author = signature_of(commit.author().map_err(|e| read(Box::new(e)))?, id)?;
    let committer = signature_of(commit.committer().map_err(|e| read(Box::new(e)))?, id)?;
    // `message_raw`, not `message`: R5.3 shows the message as written, and `message`
    // splits it into a summary and a body.
    let message = commit.message_raw().map_err(|e| read(Box::new(e)))?;
    Ok(CommitDetails {
        id: *id,
        parents,
        author,
        committer,
        message: message.to_string(),
    })
}

fn signature_of(signature: gix::actor::SignatureRef<'_>, id: &Oid) -> Result<Signature, Error> {
    let time = signature.time().map_err(|source| Error::ReadCommit {
        id: id.to_string(),
        source: Box::new(source),
    })?;
    Ok(Signature {
        name: signature.name.to_string(),
        email: signature.email.to_string(),
        time: Timestamp::new(time.seconds, time.offset),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HistoryRequest;
    use cairn_model::{CommitSummary, HistoryRow, RowContent};

    /// A row's commit, with no wildcard arm: a second kind of row must fail to compile.
    fn commit_of(row: &HistoryRow) -> &CommitSummary {
        match &row.content {
            RowContent::Commit(commit) => commit,
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
        let row = page.rows.first().expect("a commit");
        let summary = commit_of(row);

        let details = repo.commit_details(&summary.id).unwrap();
        assert_eq!(details.id, summary.id);
        assert_eq!(details.parents, summary.parents);
        assert_eq!(details.author.name, summary.author_name);
        assert_eq!(details.author.email, summary.author_email);
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
