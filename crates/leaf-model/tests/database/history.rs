//! Undo and redo walk back and forth through exactly the states the commands went through.

use leaf_model::{Database, DatabaseOutcome};
use proptest::prelude::*;

use super::{command, outline, steps};

proptest! {
    #[test]
    fn undo_walks_back_through_every_state_and_redo_walks_forward(steps in steps(30)) {
        let mut db = Database::new();
        db.set_undo_merge_interval(0);
        let mut states = vec![outline(&db)];
        for step in steps {
            if let Some(command) = command(step, &db)
                && matches!(db.apply(command), Ok(outcome) if outcome != DatabaseOutcome::Unchanged)
            {
                states.push(outline(&db));
            }
        }
        for expected in states.iter().rev().skip(1) {
            prop_assert!(db.undo().unwrap());
            prop_assert_eq!(&outline(&db), expected);
            prop_assert_eq!(db.check_invariants(), Ok(()));
        }
        prop_assert!(!db.undo().unwrap(), "nothing left to undo");
        for expected in states.iter().skip(1) {
            prop_assert!(db.redo().unwrap());
            prop_assert_eq!(&outline(&db), expected);
        }
    }
}
