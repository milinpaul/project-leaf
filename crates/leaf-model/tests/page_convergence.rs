//! Property test: two devices editing the same page at the same time end up with the same page once
//! they exchange changes, and every invariant holds on the merged page (E02-S09).
//!
//! Each case starts both peers from one shared page, then runs rounds: each peer applies its own
//! random commands (chosen against its own copy, so they are valid there), then the peers swap
//! changes. After every swap both pages must be identical, compared block by block with formatting,
//! pass `check_invariants()`, and survive a save and load. Run with `PROPTEST_CASES=10000` for the
//! long run.

mod common;

use common::{Step, command, outline, step};
use leaf_model::PageDoc;
use proptest::prelude::*;

/// Applies `steps` to `page`; refused commands are fine, they just change nothing.
fn edit(page: &mut PageDoc, steps: Vec<Step>) {
    for step in steps {
        if let Some(command) = command(step, page) {
            let _ = page.apply(command);
        }
    }
}

/// Swaps changes both ways, as two devices syncing.
fn sync(a: &mut PageDoc, b: &mut PageDoc) -> Result<(), leaf_model::ModelError> {
    let from_a = a.to_bytes()?;
    let from_b = b.to_bytes()?;
    a.import_changes(&from_b)?;
    b.import_changes(&from_a)
}

fn steps(max: usize) -> impl Strategy<Value = Vec<Step>> {
    proptest::collection::vec(step(), 0..max)
}

proptest! {
    #[test]
    fn two_peers_editing_at_once_converge_and_keep_every_invariant(
        setup in steps(12),
        rounds in proptest::collection::vec((steps(15), steps(15)), 1..4),
    ) {
        let mut a = PageDoc::new();
        edit(&mut a, setup);
        let mut b = PageDoc::from_bytes(&a.to_bytes().unwrap()).unwrap();
        for (on_a, on_b) in rounds {
            edit(&mut a, on_a);
            edit(&mut b, on_b);
            sync(&mut a, &mut b).unwrap();
            prop_assert_eq!(outline(&a), outline(&b), "both devices show the same page");
            for page in [&a, &b] {
                if let Err(broken) = page.check_invariants() {
                    let list: Vec<String> = broken.iter().map(ToString::to_string).collect();
                    prop_assert!(false, "after merging:\n{}", list.join("\n"));
                }
            }
        }
        let reloaded = PageDoc::from_bytes(&a.to_bytes().unwrap()).unwrap();
        prop_assert_eq!(reloaded.check_invariants(), Ok(()));
        prop_assert_eq!(outline(&reloaded), outline(&a));
    }
}
