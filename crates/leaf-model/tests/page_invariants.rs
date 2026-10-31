//! Property test: every page command, in any order, keeps every invariant (E02-S07).
//!
//! Random sequences of all page commands, picking real block IDs and in-range positions so most are
//! valid (invalid ones must be refused cleanly). After every step `check_invariants()` must pass, and
//! a save and load at the end must give back the same page. Run with `PROPTEST_CASES=10000` for the
//! long run; CI does that on `main`.

mod common;

use common::{command, outline, step, tables_are_grids};
use leaf_model::{BlockKind, History, PageCommand, PageDoc, PageOutcome, WebPreview};
use proptest::prelude::*;

proptest! {
    #[test]
    fn every_command_sequence_keeps_every_invariant(steps in proptest::collection::vec(step(), 1..40)) {
        let mut page = PageDoc::new();
        for step in steps {
            let Some(command) = command(step, &page) else { continue };
            // Valid commands change the page; invalid ones must be refused. Either way the rules hold.
            let result = page.apply(command.clone());
            if let Err(broken) = page.check_invariants() {
                let list: Vec<String> = broken.iter().map(ToString::to_string).collect();
                prop_assert!(false, "after {:?} (result {:?}):\n{}", command, result, list.join("\n"));
            }
            prop_assert!(tables_are_grids(&page), "after {:?} (result {:?}) a table isn't a grid", command, result);
        }
        let reloaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
        prop_assert_eq!(reloaded.check_invariants(), Ok(()));
        prop_assert_eq!(outline(&reloaded), outline(&page));
    }
}

/// The outline with web bookmark previews set aside: a fetched preview is kept off the undo history
/// (ADR-080), so it is the one thing undo doesn't take back.
fn without_previews(
    page: &PageDoc,
) -> Vec<(
    usize,
    leaf_core::BlockId,
    BlockKind,
    Option<Vec<leaf_model::TextSpan>>,
)> {
    outline(page)
        .into_iter()
        .map(|(depth, id, kind, text)| {
            let kind = match kind {
                BlockKind::WebBookmark { url, .. } => BlockKind::WebBookmark {
                    url,
                    preview: WebPreview::default(),
                },
                kind => kind,
            };
            (depth, id, kind, text)
        })
        .collect()
}

proptest! {
    /// BLOCK_MODEL invariant 7 (E02-S08): undo of command N restores the state before N, compared by
    /// `BlockId`, and redo replays it; the other invariants hold after every undo and redo. Fetched
    /// previews arrive in between, making no undo step and never undone (E15-S02).
    #[test]
    fn undo_retraces_every_command_and_redo_replays_it(steps in proptest::collection::vec(step(), 1..30)) {
        let mut page = PageDoc::new();
        page.set_undo_merge_interval(0);
        let mut history = vec![without_previews(&page)];
        for step in steps {
            let Some(command) = command(step, &page) else { continue };
            let preview = matches!(command, PageCommand::SetWebBookmarkPreview { .. });
            match page.apply(command) {
                // No change, no undo step: and nothing half-written that undo would then take back.
                Ok(PageOutcome::Unchanged) | Err(_) => {
                    prop_assert_eq!(&without_previews(&page), history.last().unwrap(), "a no-op left the page as it was");
                }
                Ok(_) if preview => {
                    prop_assert_eq!(&without_previews(&page), history.last().unwrap(), "a preview changes only the preview");
                }
                Ok(_) => history.push(without_previews(&page)),
            }
        }

        let last = history.len() - 1;
        for expected in history[..last].iter().rev() {
            let undone = matches!(page.undo().unwrap(), History::Done { .. });
            prop_assert!(undone);
            prop_assert_eq!(&without_previews(&page), expected);
            prop_assert_eq!(page.check_invariants(), Ok(()));
        }
        prop_assert_eq!(page.undo().unwrap(), History::Nothing);
        for expected in &history[1..] {
            let redone = matches!(page.redo().unwrap(), History::Done { .. });
            prop_assert!(redone);
            prop_assert_eq!(&without_previews(&page), expected);
            prop_assert_eq!(page.check_invariants(), Ok(()));
        }
        prop_assert_eq!(page.redo().unwrap(), History::Nothing);
    }
}
