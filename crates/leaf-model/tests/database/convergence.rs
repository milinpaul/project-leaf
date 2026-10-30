//! Two devices editing the same database at once end up with the same database once they exchange changes,
//! and every invariant holds on the merged one.

use leaf_model::Database;
use proptest::prelude::*;

use super::{edit, outline, steps};

fn sync(a: &mut Database, b: &mut Database) -> Result<(), leaf_model::ModelError> {
    let from_a = a.to_bytes()?;
    let from_b = b.to_bytes()?;
    a.import_changes(&from_b)?;
    b.import_changes(&from_a)
}

proptest! {
    #[test]
    fn two_peers_editing_at_once_converge_and_keep_every_invariant(
        setup in steps(20),
        rounds in proptest::collection::vec((steps(15), steps(15)), 1..4),
    ) {
        let mut a = Database::new();
        edit(&mut a, setup);
        let mut b = Database::from_bytes(&a.to_bytes().unwrap()).unwrap();
        for (on_a, on_b) in rounds {
            edit(&mut a, on_a);
            edit(&mut b, on_b);
            sync(&mut a, &mut b).unwrap();
            prop_assert_eq!(outline(&a), outline(&b), "both devices show the same database");
            for db in [&a, &b] {
                if let Err(broken) = db.check_invariants() {
                    prop_assert!(false, "after merging: {:?}", broken);
                }
            }
        }
    }
}
