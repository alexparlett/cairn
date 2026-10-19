//! What git reads in place of what is stored as it walks the history: the object each replace
//! ref names in place of the one it replaces, and the parents each grafted commit — a shallow
//! clone's boundary, or one the graft file names — is shown with in place of its own. A history
//! walked under one set of substitutions is not the history walked under another, though no
//! ref moved, so a refresh compares them beside the refs (`RefsSnapshot::walks_as`).

use crate::Oid;

/// The substitutions git applies to a walk, as one read found them: each pair and each graft
/// in id order, so that two reads of the same files compare equal whatever order they listed
/// them in, or why they could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Substitutions {
    /// Each object a replace ref replaces and the object its ref names, by the replaced id.
    /// Empty where git reads no replace ref (`core.useReplaceRefs` false).
    replacements: Vec<(Oid, Oid)>,
    /// Each grafted commit and the parents git shows for it, by the commit's id: none at a
    /// shallow boundary.
    grafts: Vec<(Oid, Vec<Oid>)>,
    /// Why the substitutions could not be read — a replace ref that names nothing, two for one
    /// object, a graft file that cannot be read — where the walk fails as git does.
    unreadable: Option<String>,
}

impl Substitutions {
    /// The substitutions read: each replaced object and its replacement, and each grafted
    /// commit and the parents git shows for it, in any order.
    pub fn new(mut replacements: Vec<(Oid, Oid)>, mut grafts: Vec<(Oid, Vec<Oid>)>) -> Self {
        replacements.sort_unstable();
        grafts.sort_unstable();
        Self {
            replacements,
            grafts,
            unreadable: None,
        }
    }

    /// Substitutions that could not be read, and why.
    pub fn unreadable(reason: String) -> Self {
        Self {
            replacements: Vec::new(),
            grafts: Vec::new(),
            unreadable: Some(reason),
        }
    }

    /// Each replaced object and its replacement, by the replaced id.
    pub fn replacements(&self) -> &[(Oid, Oid)] {
        &self.replacements
    }

    /// Each grafted commit and the parents git shows for it, by the commit's id.
    pub fn grafts(&self) -> &[(Oid, Vec<Oid>)] {
        &self.grafts
    }

    /// Why they could not be read, if they could not.
    pub fn unreadable_reason(&self) -> Option<&str> {
        self.unreadable.as_deref()
    }

    /// Whether git substitutes nothing: no replace ref it reads, no graft, nothing unreadable.
    pub fn is_empty(&self) -> bool {
        self.replacements.is_empty() && self.grafts.is_empty() && self.unreadable.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    /// Two reads of the same files are equal whatever order each listed them in, and a change
    /// to any one pair, graft or parent — or an unreadable read — is not. Caught by: the order
    /// a read listed them in kept (a refresh reopening on every read of a `HashMap`), or a
    /// graft's parents compared as a set.
    #[test]
    fn substitutions_compare_by_what_they_substitute_and_not_by_the_order_read() {
        let read = Substitutions::new(
            vec![(oid(1), oid(2)), (oid(3), oid(4))],
            vec![(oid(5), vec![oid(6), oid(7)]), (oid(8), vec![])],
        );
        let again = Substitutions::new(
            vec![(oid(3), oid(4)), (oid(1), oid(2))],
            vec![(oid(8), vec![]), (oid(5), vec![oid(6), oid(7)])],
        );
        assert_eq!(read, again);
        assert!(!read.is_empty());
        assert!(Substitutions::default().is_empty());
        assert_eq!(read.replacements().first(), Some(&(oid(1), oid(2))));

        for changed in [
            Substitutions::new(
                vec![(oid(1), oid(9)), (oid(3), oid(4))],
                vec![(oid(5), vec![oid(6), oid(7)]), (oid(8), vec![])],
            ),
            Substitutions::new(
                vec![(oid(3), oid(4))],
                vec![(oid(5), vec![oid(6), oid(7)]), (oid(8), vec![])],
            ),
            Substitutions::new(
                vec![(oid(1), oid(2)), (oid(3), oid(4))],
                vec![(oid(5), vec![oid(7), oid(6)]), (oid(8), vec![])],
            ),
            Substitutions::new(
                vec![(oid(1), oid(2)), (oid(3), oid(4))],
                vec![(oid(5), vec![oid(6), oid(7)])],
            ),
            Substitutions::unreadable("duplicate replace ref".to_owned()),
        ] {
            assert_ne!(changed, read, "{changed:?}");
        }
        let broken = Substitutions::unreadable("duplicate replace ref".to_owned());
        assert!(!broken.is_empty());
        assert_eq!(broken.unreadable_reason(), Some("duplicate replace ref"));
    }
}
