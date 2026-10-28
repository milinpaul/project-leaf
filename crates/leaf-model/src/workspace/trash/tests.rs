//! Tests for trash, restore and permanent delete.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use leaf_core::PageId;

use crate::{ModelError, Outcome, Workspace, WorkspaceCommand};

fn create(ws: &mut Workspace, parent: Option<PageId>, title: &str) -> PageId {
    let index = match parent {
        Some(p) => ws.children(p).unwrap().len(),
        None => ws.root_pages().unwrap().len(),
    };
    let command = WorkspaceCommand::CreatePage {
        parent,
        index,
        title: title.into(),
        created_at: 0,
    };
    match ws.apply(command).unwrap() {
        Outcome::PageCreated(page) => page,
        other => panic!("expected a new page, got {other:?}"),
    }
}

fn trash(ws: &mut Workspace, page: PageId, at: i64) -> Result<Outcome, ModelError> {
    ws.apply(WorkspaceCommand::TrashPage { page, at })
}

fn restore(ws: &mut Workspace, page: PageId) -> Result<Outcome, ModelError> {
    ws.apply(WorkspaceCommand::RestorePage { page })
}

fn delete(ws: &mut Workspace, page: PageId) -> Result<Outcome, ModelError> {
    ws.apply(WorkspaceCommand::DeletePagePermanently { page })
}

#[test]
fn trashing_hides_the_page_and_everything_inside_it() {
    let mut ws = Workspace::new();
    let (a, b) = (create(&mut ws, None, "A"), create(&mut ws, None, "B"));
    let inside = create(&mut ws, Some(b), "Inside B");

    assert_eq!(trash(&mut ws, b, 10).unwrap(), Outcome::Changed);
    assert_eq!(ws.root_pages().unwrap(), vec![a]);
    assert_eq!(
        ws.trash().unwrap(),
        vec![b],
        "only pages trashed on their own are listed"
    );
    assert!(ws.is_in_trash(b).unwrap() && ws.is_in_trash(inside).unwrap());
    assert!(!ws.is_in_trash(a).unwrap());
    assert_eq!(ws.page(b).unwrap().trashed_at, Some(10));
    assert_eq!(
        ws.page(inside).unwrap().trashed_at,
        None,
        "hidden by its parent, not trashed itself"
    );
}

#[test]
fn restore_puts_the_page_back_where_it_was() {
    let mut ws = Workspace::new();
    let [a, b, c] = ["A", "B", "C"].map(|t| create(&mut ws, None, t));
    trash(&mut ws, b, 10).unwrap();
    assert_eq!(ws.root_pages().unwrap(), vec![a, c]);

    assert_eq!(restore(&mut ws, b).unwrap(), Outcome::Changed);
    assert_eq!(
        ws.root_pages().unwrap(),
        vec![a, b, c],
        "back in the middle, not at the end"
    );
    assert_eq!(ws.page(b).unwrap().trashed_at, None);
    assert_eq!(ws.trash().unwrap(), Vec::<PageId>::new());
}

#[test]
fn restore_goes_to_the_top_level_when_the_parent_is_in_the_trash() {
    let mut ws = Workspace::new();
    let (parent, other) = (create(&mut ws, None, "Parent"), create(&mut ws, None, "Other"));
    let child = create(&mut ws, Some(parent), "Child");
    trash(&mut ws, child, 10).unwrap();
    trash(&mut ws, parent, 20).unwrap();
    assert_eq!(
        ws.trash().unwrap(),
        vec![parent, child],
        "most recently trashed first"
    );

    restore(&mut ws, child).unwrap();
    assert_eq!(
        ws.root_pages().unwrap(),
        vec![other, child],
        "the child surfaces at the end of the top level"
    );
    assert_eq!(ws.trash().unwrap(), vec![parent]);
    assert!(!ws.is_in_trash(child).unwrap());
}

#[test]
fn permanent_delete_removes_the_subtree_and_reports_every_page() {
    let mut ws = Workspace::new();
    let keep = create(&mut ws, None, "Keep");
    let gone = create(&mut ws, None, "Gone");
    let child = create(&mut ws, Some(gone), "Child");
    let grandchild = create(&mut ws, Some(child), "Grandchild");
    trash(&mut ws, gone, 10).unwrap();

    let outcome = delete(&mut ws, gone).unwrap();
    assert_eq!(
        outcome,
        Outcome::PagesDeleted(vec![gone, child, grandchild]),
        "parents before children"
    );
    for page in [gone, child, grandchild] {
        assert!(!ws.contains(page));
        assert!(matches!(ws.page(page), Err(ModelError::PageNotFound(p)) if p == page));
    }
    assert_eq!(ws.len(), 1);
    assert_eq!(ws.root_pages().unwrap(), vec![keep]);
    assert_eq!(ws.trash().unwrap(), Vec::<PageId>::new());
}

#[test]
fn a_page_inside_a_trashed_page_can_be_deleted_on_its_own() {
    let mut ws = Workspace::new();
    let parent = create(&mut ws, None, "Parent");
    let child = create(&mut ws, Some(parent), "Child");
    trash(&mut ws, parent, 10).unwrap();
    assert_eq!(
        delete(&mut ws, child).unwrap(),
        Outcome::PagesDeleted(vec![child])
    );
    assert!(ws.contains(parent));
}

#[test]
fn trash_rules_refuse_and_write_nothing() {
    let mut ws = Workspace::new();
    let (live, binned) = (create(&mut ws, None, "Live"), create(&mut ws, None, "Binned"));
    let inside = create(&mut ws, Some(binned), "Inside");
    trash(&mut ws, binned, 10).unwrap();

    let commits = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&commits);
    let _sub = ws.doc.subscribe_local_update(Box::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        true
    }));

    let in_trash = |r: Result<Outcome, ModelError>, page: PageId| matches!(r, Err(ModelError::PageInTrash(p)) if p == page);
    assert!(in_trash(trash(&mut ws, binned, 11), binned), "already trashed");
    assert!(
        in_trash(trash(&mut ws, inside, 11), inside),
        "inside a trashed page"
    );
    let under_trash = WorkspaceCommand::CreatePage {
        parent: Some(binned),
        index: 0,
        title: "x".into(),
        created_at: 0,
    };
    assert!(
        in_trash(ws.apply(under_trash), binned),
        "no new pages inside the trash"
    );
    let move_out = WorkspaceCommand::MovePage {
        page: inside,
        new_parent: None,
        index: 0,
    };
    assert!(in_trash(ws.apply(move_out), inside), "restore first, then move");
    let move_in = WorkspaceCommand::MovePage {
        page: live,
        new_parent: Some(binned),
        index: 0,
    };
    assert!(in_trash(ws.apply(move_in), binned), "no moving into the trash");

    let not_in_trash = |r: Result<Outcome, ModelError>, page: PageId| matches!(r, Err(ModelError::NotInTrash(p)) if p == page);
    assert!(not_in_trash(restore(&mut ws, live), live));
    assert!(
        not_in_trash(restore(&mut ws, inside), inside),
        "only pages trashed on their own are restored"
    );
    assert!(
        not_in_trash(delete(&mut ws, live), live),
        "only pages in the trash can be deleted for good"
    );

    let ghost = PageId::new();
    assert!(matches!(
        trash(&mut ws, ghost, 1),
        Err(ModelError::PageNotFound(_))
    ));
    assert!(matches!(ws.is_in_trash(ghost), Err(ModelError::PageNotFound(_))));

    assert_eq!(commits.load(Ordering::SeqCst), 0);
    ws.apply(WorkspaceCommand::RenamePage {
        page: binned,
        title: "Renamed".into(),
    })
    .unwrap();
    assert_eq!(
        ws.page(binned).unwrap().title,
        "Renamed",
        "renaming in the trash is allowed"
    );
}

#[test]
fn trash_errors_explain_themselves() {
    let page = PageId::new();
    assert_eq!(
        ModelError::PageInTrash(page).to_string(),
        format!("page {page} is in the trash")
    );
    assert_eq!(
        ModelError::NotInTrash(page).to_string(),
        format!("page {page} is not in the trash")
    );
}
