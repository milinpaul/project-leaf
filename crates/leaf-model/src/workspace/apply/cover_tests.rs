//! Tests for `SetPageCover`: setting, replacing and clearing a page's cover, and that it stays with the
//! page through saving, moves and the trash.

use std::sync::atomic::Ordering;

use leaf_core::PageId;

use super::tests::{commit_counter, create};
use crate::{ModelError, Outcome, Workspace, WorkspaceCommand};

fn cover(page: PageId, cover: Option<&str>) -> WorkspaceCommand {
    WorkspaceCommand::SetPageCover {
        page,
        cover: cover.map(Into::into),
    }
}

#[test]
fn set_page_cover_sets_replaces_and_clears() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Trip");
    assert_eq!(ws.page(page).unwrap().cover, None, "a new page has no cover");

    assert_eq!(
        ws.apply(cover(page, Some("builtin:dawn"))).unwrap(),
        Outcome::Changed
    );
    assert_eq!(ws.page(page).unwrap().cover.as_deref(), Some("builtin:dawn"));
    ws.apply(cover(page, Some("builtin:forest"))).unwrap();
    assert_eq!(ws.page(page).unwrap().cover.as_deref(), Some("builtin:forest"));
    assert_eq!(ws.apply(cover(page, None)).unwrap(), Outcome::Changed);
    assert_eq!(ws.page(page).unwrap().cover, None);
}

#[test]
fn setting_the_same_cover_or_clearing_none_changes_nothing() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Trip");
    let other = create(&mut ws, None, 1, "Plain");
    ws.apply(cover(page, Some("builtin:dawn"))).unwrap();
    let (commits, _sub) = commit_counter(&ws);

    assert_eq!(
        ws.apply(cover(page, Some("builtin:dawn"))).unwrap(),
        Outcome::Unchanged
    );
    assert_eq!(ws.apply(cover(other, None)).unwrap(), Outcome::Unchanged);
    assert_eq!(commits.load(Ordering::SeqCst), 0, "no undo step for a no-op");
}

#[test]
fn set_page_cover_rejects_empty_text_and_unknown_pages() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Trip");
    ws.apply(cover(page, Some("builtin:dawn"))).unwrap();
    let (commits, _sub) = commit_counter(&ws);

    assert!(matches!(ws.apply(cover(page, Some(""))), Err(ModelError::EmptyCover(p)) if p == page));
    let ghost = PageId::new();
    assert!(matches!(
        ws.apply(cover(ghost, Some("builtin:dawn"))),
        Err(ModelError::PageNotFound(p)) if p == ghost
    ));
    assert_eq!(commits.load(Ordering::SeqCst), 0);
    assert_eq!(
        ws.page(page).unwrap().cover.as_deref(),
        Some("builtin:dawn"),
        "a rejected cover leaves the old one"
    );
    assert_eq!(
        ModelError::EmptyCover(page).to_string(),
        format!("page {page}: cover cannot be empty (use None to clear it)")
    );
}

#[test]
fn any_cover_name_is_kept_even_one_this_version_does_not_know() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Trip");
    ws.apply(cover(page, Some("image:0123abcd"))).unwrap();

    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert_eq!(
        loaded.page(page).unwrap().cover.as_deref(),
        Some("image:0123abcd")
    );
}

#[test]
fn the_cover_stays_with_the_page_through_moves_and_the_trash() {
    let mut ws = Workspace::new();
    let a = create(&mut ws, None, 0, "A");
    let b = create(&mut ws, None, 1, "B");
    ws.apply(cover(a, Some("builtin:dawn"))).unwrap();

    ws.apply(WorkspaceCommand::MovePage {
        page: a,
        new_parent: Some(b),
        index: 0,
    })
    .unwrap();
    ws.apply(WorkspaceCommand::TrashPage { page: a, at: 1 }).unwrap();
    assert_eq!(ws.page(a).unwrap().cover.as_deref(), Some("builtin:dawn"));
    ws.apply(WorkspaceCommand::RestorePage { page: a }).unwrap();
    assert_eq!(ws.page(a).unwrap().cover.as_deref(), Some("builtin:dawn"));
}
