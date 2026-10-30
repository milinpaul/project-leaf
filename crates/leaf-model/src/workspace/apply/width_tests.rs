//! Tests for `SetPageFullWidth`: switching a page between the reading column and the whole width,
//! undo, saving, and that it isn't an edit.

use std::sync::atomic::Ordering;

use leaf_core::PageId;

use super::tests::{commit_counter, create};
use crate::{ModelError, Outcome, Workspace, WorkspaceCommand};

fn width(page: PageId, full_width: bool) -> WorkspaceCommand {
    WorkspaceCommand::SetPageFullWidth { page, full_width }
}

#[test]
fn a_page_starts_in_the_reading_column_and_switches_both_ways() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Notes");
    let other = create(&mut ws, None, 1, "Other");
    assert!(!ws.page(page).unwrap().full_width, "a new page is not full width");

    assert_eq!(ws.apply(width(page, true)).unwrap(), Outcome::Changed);
    assert!(ws.page(page).unwrap().full_width);
    assert!(!ws.page(other).unwrap().full_width, "only that page changes");
    assert_eq!(ws.apply(width(page, false)).unwrap(), Outcome::Changed);
    assert!(!ws.page(page).unwrap().full_width);
}

#[test]
fn setting_the_same_width_changes_nothing() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Notes");
    let wide = create(&mut ws, None, 1, "Wide");
    ws.apply(width(wide, true)).unwrap();
    let (commits, _sub) = commit_counter(&ws);

    assert_eq!(ws.apply(width(page, false)).unwrap(), Outcome::Unchanged);
    assert_eq!(ws.apply(width(wide, true)).unwrap(), Outcome::Unchanged);
    assert_eq!(commits.load(Ordering::SeqCst), 0, "no undo step for a no-op");
}

#[test]
fn an_unknown_page_is_an_error() {
    let mut ws = Workspace::new();
    let ghost = PageId::new();
    assert!(matches!(
        ws.apply(width(ghost, true)),
        Err(ModelError::PageNotFound(p)) if p == ghost
    ));
}

#[test]
fn switching_is_one_undo_step_and_not_an_edit() {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    let page = create(&mut ws, None, 0, "Notes");
    let edited = ws.page(page).unwrap().edited_at;

    ws.apply(width(page, true)).unwrap();
    assert_eq!(ws.page(page).unwrap().edited_at, edited, "layout is not an edit");
    assert!(ws.undo().unwrap());
    assert!(!ws.page(page).unwrap().full_width, "undo puts the column back");
    assert!(ws.page(page).is_ok(), "and only that: the page is still there");
}

#[test]
fn the_width_survives_saving_moves_and_the_trash() {
    let mut ws = Workspace::new();
    let a = create(&mut ws, None, 0, "A");
    let b = create(&mut ws, None, 1, "B");
    ws.apply(width(a, true)).unwrap();
    ws.apply(WorkspaceCommand::MovePage {
        page: a,
        new_parent: Some(b),
        index: 0,
    })
    .unwrap();
    ws.apply(WorkspaceCommand::TrashPage { page: a, at: 1 }).unwrap();
    ws.apply(WorkspaceCommand::RestorePage { page: a }).unwrap();

    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert!(loaded.page(a).unwrap().full_width);
    assert!(!loaded.page(b).unwrap().full_width);
}
