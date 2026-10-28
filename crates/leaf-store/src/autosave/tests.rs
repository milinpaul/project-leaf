//! The flush policy, driven by a fake clock (plain numbers for "now").

use std::collections::HashSet;

use leaf_core::PageId;

use super::{Autosave, DEFAULT_AUTOSAVE_MS};
use crate::DocKey;

fn set(keys: Vec<DocKey>) -> HashSet<DocKey> {
    keys.into_iter().collect()
}

#[test]
fn nothing_changed_means_nothing_due_and_no_timer() {
    let autosave = Autosave::default();
    assert_eq!(autosave.next_deadline(), None);
    assert_eq!(autosave.due(1_000_000), Vec::<DocKey>::new());
    assert_eq!(autosave.pending(), Vec::<DocKey>::new());
}

#[test]
fn a_change_falls_due_exactly_after_the_delay() {
    let mut autosave = Autosave::default();
    autosave.changed(DocKey::Workspace, 1_000);
    assert_eq!(autosave.next_deadline(), Some(1_000 + DEFAULT_AUTOSAVE_MS));
    assert_eq!(autosave.due(1_299), Vec::<DocKey>::new());
    assert_eq!(autosave.due(1_300), vec![DocKey::Workspace]);
    assert_eq!(autosave.pending(), vec![DocKey::Workspace]);
}

#[test]
fn steady_typing_cannot_postpone_the_save() {
    // A keystroke every 50 ms: the deadline stays tied to the first one.
    let mut autosave = Autosave::new(300);
    let page = DocKey::Page(PageId::new());
    for now in (0..=1_000).step_by(50) {
        autosave.changed(page, now);
        assert_eq!(autosave.next_deadline(), Some(300));
    }
    assert_eq!(autosave.due(300), vec![page]);
}

#[test]
fn each_document_has_its_own_deadline() {
    let mut autosave = Autosave::new(300);
    let (a, b) = (DocKey::Page(PageId::new()), DocKey::Page(PageId::new()));
    autosave.changed(a, 0);
    autosave.changed(b, 200);
    assert_eq!(autosave.next_deadline(), Some(300), "the earliest wins");
    assert_eq!(autosave.due(300), vec![a]);
    assert_eq!(set(autosave.due(500)), set(vec![a, b]));
    assert_eq!(set(autosave.pending()), set(vec![a, b]));

    autosave.forget(a);
    assert_eq!(autosave.next_deadline(), Some(500));
    assert_eq!(autosave.pending(), vec![b]);
}
