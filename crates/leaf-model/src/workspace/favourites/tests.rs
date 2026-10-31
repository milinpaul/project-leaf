//! Tests for favourites (E18-S02, ADR-060): order, repeats, the trash, deleting, unknown entries
//! and saving.

use leaf_core::PageId;
use loro::LoroValue;

use super::FAVOURITES;
use crate::{ModelError, Outcome, Workspace, WorkspaceCommand};

fn create(ws: &mut Workspace, parent: Option<PageId>) -> PageId {
    let command = WorkspaceCommand::CreatePage {
        parent,
        index: 0,
        title: String::new(),
        created_at: 0,
    };
    match ws.apply(command).unwrap() {
        Outcome::PageCreated(page) => page,
        other => panic!("expected a new page, got {other:?}"),
    }
}

fn add(ws: &mut Workspace, page: PageId) -> Result<Outcome, ModelError> {
    ws.apply(WorkspaceCommand::AddFavourite { page })
}

fn remove(ws: &mut Workspace, page: PageId) -> Result<Outcome, ModelError> {
    ws.apply(WorkspaceCommand::RemoveFavourite { page })
}

#[test]
fn favourites_keep_the_order_they_were_added_in() {
    let mut ws = Workspace::new();
    let [a, b, c] = [(); 3].map(|()| create(&mut ws, None));
    assert_eq!(ws.favourites().unwrap(), Vec::<PageId>::new());
    for page in [b, c, a] {
        assert_eq!(add(&mut ws, page).unwrap(), Outcome::Changed);
    }
    assert_eq!(ws.favourites().unwrap(), vec![b, c, a]);
    assert!(ws.is_favourite(a));

    remove(&mut ws, c).unwrap();
    add(&mut ws, c).unwrap();
    assert_eq!(ws.favourites().unwrap(), vec![b, a, c], "re-starring goes last");
}

#[test]
fn adding_twice_or_removing_a_non_favourite_changes_nothing() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None);
    add(&mut ws, page).unwrap();
    assert_eq!(add(&mut ws, page).unwrap(), Outcome::Unchanged);
    assert_eq!(ws.favourites().unwrap(), vec![page]);

    assert_eq!(remove(&mut ws, page).unwrap(), Outcome::Changed);
    assert!(!ws.is_favourite(page));
    assert_eq!(remove(&mut ws, page).unwrap(), Outcome::Unchanged);
}

#[test]
fn unknown_and_trashed_pages_are_refused() {
    let mut ws = Workspace::new();
    let ghost = PageId::new();
    assert!(matches!(add(&mut ws, ghost), Err(ModelError::PageNotFound(p)) if p == ghost));
    assert!(matches!(remove(&mut ws, ghost), Err(ModelError::PageNotFound(p)) if p == ghost));

    let parent = create(&mut ws, None);
    let child = create(&mut ws, Some(parent));
    ws.apply(WorkspaceCommand::TrashPage { page: parent, at: 1 })
        .unwrap();
    for page in [parent, child] {
        assert!(matches!(add(&mut ws, page), Err(ModelError::PageInTrash(_))));
    }
}

#[test]
fn the_trash_hides_a_favourite_and_restoring_brings_it_back_in_place() {
    let mut ws = Workspace::new();
    let parent = create(&mut ws, None);
    let child = create(&mut ws, Some(parent));
    let other = create(&mut ws, None);
    for page in [child, other, parent] {
        add(&mut ws, page).unwrap();
    }

    ws.apply(WorkspaceCommand::TrashPage { page: parent, at: 1 })
        .unwrap();
    assert_eq!(
        ws.favourites().unwrap(),
        vec![other],
        "the child goes with its parent"
    );
    assert!(ws.is_favourite(parent), "still starred, just hidden");

    ws.apply(WorkspaceCommand::RestorePage { page: parent }).unwrap();
    assert_eq!(ws.favourites().unwrap(), vec![child, other, parent]);
}

#[test]
fn a_trashed_favourite_can_still_be_unstarred() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None);
    add(&mut ws, page).unwrap();
    ws.apply(WorkspaceCommand::TrashPage { page, at: 1 }).unwrap();
    assert_eq!(remove(&mut ws, page).unwrap(), Outcome::Changed);
    ws.apply(WorkspaceCommand::RestorePage { page }).unwrap();
    assert_eq!(ws.favourites().unwrap(), Vec::<PageId>::new());
}

#[test]
fn deleting_for_good_removes_the_favourites_and_undo_brings_them_back() {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);
    let parent = create(&mut ws, None);
    let child = create(&mut ws, Some(parent));
    let other = create(&mut ws, None);
    for page in [child, other] {
        add(&mut ws, page).unwrap();
    }
    ws.apply(WorkspaceCommand::TrashPage { page: parent, at: 1 })
        .unwrap();
    ws.apply(WorkspaceCommand::DeletePagePermanently { page: parent })
        .unwrap();
    assert_eq!(ws.favourites().unwrap(), vec![other]);
    assert!(!ws.is_favourite(child));
    assert!(
        !ws.doc
            .get_map(FAVOURITES)
            .get(&child.to_string())
            .is_some_and(|v| { !matches!(v, loro::ValueOrContainer::Value(LoroValue::Null)) }),
        "the entry itself is gone"
    );

    assert!(ws.undo().unwrap());
    assert!(ws.undo().unwrap());
    assert_eq!(ws.favourites().unwrap(), vec![child, other]);
}

#[test]
fn entries_this_version_does_not_understand_are_skipped_but_kept() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None);
    let odd = create(&mut ws, None);
    add(&mut ws, page).unwrap();
    let map = ws.doc.get_map(FAVOURITES);
    map.insert("not-a-page", 5_i64).unwrap();
    map.insert(&odd.to_string(), "pinned-by-a-newer-version").unwrap();
    ws.doc.commit();

    assert_eq!(ws.favourites().unwrap(), vec![page]);
    assert!(!ws.is_favourite(odd));
    add(&mut ws, page).unwrap();
    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    let map = loaded.doc.get_map(FAVOURITES);
    assert!(map.get("not-a-page").is_some(), "kept (rule 5)");
    assert!(map.get(&odd.to_string()).is_some(), "kept (rule 5)");
    assert_eq!(loaded.favourites().unwrap(), vec![page]);
}

#[test]
fn a_favourite_whose_page_is_gone_is_left_out() {
    // What a device is left with when another deleted the page while it starred it.
    let mut ws = Workspace::new();
    let page = create(&mut ws, None);
    ws.doc
        .get_map(FAVOURITES)
        .insert(&PageId::new().to_string(), 1_i64)
        .unwrap();
    ws.doc.commit();
    add(&mut ws, page).unwrap();
    assert_eq!(ws.favourites().unwrap(), vec![page]);
}

#[test]
fn equal_positions_are_ordered_by_page_id() {
    let mut ws = Workspace::new();
    let a = create(&mut ws, None);
    let b = create(&mut ws, None);
    let map = ws.doc.get_map(FAVOURITES);
    map.insert(&a.to_string(), 1_i64).unwrap();
    map.insert(&b.to_string(), 1_i64).unwrap();
    ws.doc.commit();
    let mut expected = vec![a, b];
    expected.sort_unstable();
    assert_eq!(ws.favourites().unwrap(), expected);
}

#[test]
fn favourites_survive_saving() {
    let mut ws = Workspace::new();
    let [a, b] = [(); 2].map(|()| create(&mut ws, None));
    add(&mut ws, b).unwrap();
    add(&mut ws, a).unwrap();
    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded.favourites().unwrap(), vec![b, a]);
}

#[test]
fn imported_changes_rebuild_the_page_index() {
    let mut a = Workspace::new();
    let mut b = Workspace::from_bytes(&a.to_bytes().unwrap()).unwrap();
    let page = create(&mut a, None);
    add(&mut a, page).unwrap();
    b.import_changes(&a.to_bytes().unwrap()).unwrap();
    assert!(b.contains(page));
    assert_eq!(b.favourites().unwrap(), vec![page]);
    assert!(b.import_changes(b"not loro").is_err());
}
