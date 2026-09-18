//! Moving an object name across the seam, in both directions.
//!
//! Both sides hold the digest itself, so neither conversion goes through hex; hex is built
//! only to name the object in an error.

use cairn_model::Oid;

use crate::Error;

/// An id of the other width is refused here: gix asserts rather than failing on one.
pub(crate) fn object_id(oid: &Oid) -> Result<gix::hash::ObjectId, Error> {
    gix::hash::ObjectId::try_from(oid.as_bytes()).map_err(|source| Error::ReadCommit {
        id: oid.to_string(),
        source: Box::new(source),
    })
}

pub(crate) fn model_id(id: &gix::hash::oid) -> Result<Oid, Error> {
    Oid::from_bytes(id.as_bytes()).map_err(|source| Error::ReadCommit {
        id: id.to_hex().to_string(),
        source: Box::new(source),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: routing either direction through hex, which loses the width check and
    /// costs an allocation on a path walked once per commit.
    #[test]
    fn an_id_survives_the_round_trip_in_both_widths() {
        for hex in [
            "1234567890abcdef1234567890abcdef12345678",
            "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        ] {
            let model = Oid::parse(hex).unwrap();
            let gix = object_id(&model).unwrap();
            assert_eq!(gix.as_bytes(), model.as_bytes());
            assert_eq!(model_id(&gix).unwrap(), model);
        }
    }
}
