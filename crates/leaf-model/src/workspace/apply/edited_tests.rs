//! Tests for `MarkEdited` and the `edited_at` field (ADR-059): recording, the fallback for older
//! pages, undo skipping it, saving, and two peers agreeing on one time.

use std::sync::atomic::Ordering;

use leaf_core::PageId;
use loro::{ExportMode, LoroValue};
use proptest::prelude::*;

use super::tests::{T0, commit_counter, create};
use crate::page_meta::keys;
use crate::{ModelError, Outcome, Workspace, WorkspaceCommand};

fn mark(page: PageId, at: i64) -> WorkspaceCommand {
    WorkspaceCommand::MarkEdited { page, at }
}

fn rename(page: PageId, title: &str) -> WorkspaceCommand {
    WorkspaceCommand::RenamePage {
        page,
        title: title.into(),
    }
}

#[test]
fn a_new_page_was_last_edited_when_it_was_created() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Notes");
    assert_eq!(ws.page(page).unwrap().edited_at, T0);
}

#[test]
fn mark_edited_records_the_time_as_one_commit() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Notes");
    let (commits, _sub) = commit_counter(&ws);

    assert_eq!(ws.apply(mark(page, T0 + 5)).unwrap(), Outcome::Changed);
    assert_eq!(ws.page(page).unwrap().edited_at, T0 + 5);
    assert_eq!(commits.load(Ordering::SeqCst), 1);

    assert_eq!(
        ws.apply(mark(page, T0 + 5)).unwrap(),
        Outcome::Unchanged,
        "the same time again writes nothing"
    );
    assert_eq!(commits.load(Ordering::SeqCst), 1);
}

#[test]
fn an_earlier_time_is_still_recorded() {
    // A clock that moved back is still the latest edit; the model doesn't second-guess the caller.
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Notes");
    ws.apply(mark(page, T0 + 100)).unwrap();
    ws.apply(mark(page, T0 + 50)).unwrap();
    assert_eq!(ws.page(page).unwrap().edited_at, T0 + 50);
}

#[test]
fn mark_edited_on_an_unknown_page_is_an_error() {
    let mut ws = Workspace::new();
    let ghost = PageId::new();
    assert!(matches!(
        ws.apply(mark(ghost, T0)),
        Err(ModelError::PageNotFound(p)) if p == ghost
    ));
}

#[test]
fn undo_skips_the_edited_time() {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    let page = create(&mut ws, None, 0, "Notes");
    ws.apply(rename(page, "Ideas")).unwrap();
    ws.apply(mark(page, T0 + 10)).unwrap();

    assert!(ws.undo().unwrap(), "the rename is still undoable");
    let meta = ws.page(page).unwrap();
    assert_eq!(meta.title, "Notes");
    assert_eq!(meta.edited_at, T0 + 10, "undo never brings back an older time");

    ws.apply(mark(page, T0 + 20)).unwrap();
    assert!(ws.redo().unwrap(), "stamping doesn't clear the redo stack");
    assert_eq!(ws.page(page).unwrap().title, "Ideas");
    assert_eq!(ws.page(page).unwrap().edited_at, T0 + 20);
}

#[test]
fn a_stamp_alone_is_not_an_undo_step() {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    let page = create(&mut ws, None, 0, "Notes");
    ws.apply(mark(page, T0 + 10)).unwrap();
    assert!(ws.undo().unwrap(), "undo reaches the page's creation");
    assert!(!ws.contains(page));
    assert!(!ws.undo().unwrap(), "and nothing else");
}

#[test]
fn older_pages_without_the_field_read_as_created() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Old");
    let node = ws.node(page).unwrap();
    ws.tree.get_meta(node).unwrap().delete(keys::EDITED_AT).unwrap();
    ws.doc.commit();
    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded.page(page).unwrap().edited_at, T0);
}

#[test]
fn a_wrong_type_is_an_error_not_a_guess() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Odd");
    let node = ws.node(page).unwrap();
    ws.tree
        .get_meta(node)
        .unwrap()
        .insert(keys::EDITED_AT, "today")
        .unwrap();
    ws.doc.commit();
    assert!(matches!(
        ws.page(page),
        Err(ModelError::InvalidPageMeta {
            field: "edited_at",
            ..
        })
    ));
}

#[test]
fn the_time_survives_saving_and_unknown_fields_stay() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Notes");
    let node = ws.node(page).unwrap();
    ws.tree
        .get_meta(node)
        .unwrap()
        .insert("from_the_future", LoroValue::Bool(true))
        .unwrap();
    ws.doc.commit();
    ws.apply(mark(page, T0 + 42)).unwrap();

    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded.page(page).unwrap().edited_at, T0 + 42);
    let node = loaded.node(page).unwrap();
    assert!(
        loaded
            .tree
            .get_meta(node)
            .unwrap()
            .get("from_the_future")
            .is_some(),
        "unknown fields are kept (rule 5)"
    );
}

/// Sends each peer's changes to the other, as two devices syncing.
fn sync(a: &Workspace, b: &Workspace) {
    let from_a = a.doc.export(ExportMode::all_updates()).unwrap();
    let from_b = b.doc.export(ExportMode::all_updates()).unwrap();
    a.doc.import(&from_b).unwrap();
    b.doc.import(&from_a).unwrap();
}

fn peers() -> (Workspace, Workspace, PageId) {
    let mut a = Workspace::new();
    let page = create(&mut a, None, 0, "Shared");
    let b = Workspace::from_bytes(&a.to_bytes().unwrap()).unwrap();
    (a, b, page)
}

#[test]
fn a_stamp_made_after_seeing_the_other_wins() {
    let (mut a, mut b, page) = peers();
    a.apply(mark(page, T0 + 100)).unwrap();
    sync(&a, &b);
    b.apply(mark(page, T0 + 200)).unwrap();
    sync(&a, &b);
    assert_eq!(a.page(page).unwrap().edited_at, T0 + 200);
    assert_eq!(b.page(page).unwrap().edited_at, T0 + 200);
}

proptest! {
    /// Whatever each peer stamps, after syncing both read the same time, and it is one that was
    /// actually written (ADR-059: Loro keeps one of two simultaneous stamps).
    #[test]
    fn two_peers_agree_on_one_written_time(
        rounds in proptest::collection::vec(
            (proptest::collection::vec(0..1_000_i64, 0..4), proptest::collection::vec(0..1_000_i64, 0..4)),
            1..4,
        ),
    ) {
        let (mut a, mut b, page) = peers();
        let mut written = vec![T0];
        for (on_a, on_b) in rounds {
            for at in on_a {
                a.apply(mark(page, T0 + at)).unwrap();
                written.push(T0 + at);
            }
            for at in on_b {
                b.apply(mark(page, T0 + at)).unwrap();
                written.push(T0 + at);
            }
            sync(&a, &b);
            let (left, right) = (a.page(page).unwrap().edited_at, b.page(page).unwrap().edited_at);
            prop_assert_eq!(left, right);
            prop_assert!(written.contains(&left));
        }
    }
}
