//! Tests for the page commands: create, rename and icon.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use leaf_core::PageId;

use crate::{ModelError, Outcome, Workspace, WorkspaceCommand};

const T0: i64 = 1_700_000_000_000;

fn create(ws: &mut Workspace, parent: Option<PageId>, index: usize, title: &str) -> PageId {
    let command = WorkspaceCommand::CreatePage {
        parent,
        index,
        title: title.into(),
        created_at: T0,
    };
    match ws.apply(command).unwrap() {
        Outcome::PageCreated(page) => page,
        Outcome::Changed => panic!("CreatePage must report the new page"),
    }
}

fn rename(page: PageId, title: &str) -> WorkspaceCommand {
    WorkspaceCommand::RenamePage {
        page,
        title: title.into(),
    }
}

fn icon(page: PageId, icon: Option<&str>) -> WorkspaceCommand {
    WorkspaceCommand::SetPageIcon {
        page,
        icon: icon.map(Into::into),
    }
}

/// Counts Loro commits: the local-update event fires once per commit that changed something.
fn commit_counter(ws: &Workspace) -> (Arc<AtomicUsize>, loro::Subscription) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    let subscription = ws.doc.subscribe_local_update(Box::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        true
    }));
    (count, subscription)
}

#[test]
fn create_page_places_new_pages_at_the_given_index() {
    let mut ws = Workspace::new();
    let a = create(&mut ws, None, 0, "A");
    let b = create(&mut ws, None, 1, "B");
    let c = create(&mut ws, None, 0, "C");
    assert_eq!(ws.root_pages(), vec![c, a, b]);

    let child = create(&mut ws, Some(a), 0, "Child");
    let last = create(&mut ws, Some(a), 1, "Appended");
    assert_eq!(ws.children(a).unwrap(), vec![child, last]);
    assert_eq!(ws.len(), 5);

    let meta = ws.page(child).unwrap();
    assert_eq!(
        (meta.id, meta.title.as_str(), meta.created_at),
        (child, "Child", T0)
    );
    assert_eq!((meta.icon, meta.trashed_at), (None, None));
}

#[test]
fn create_page_mints_a_new_id_each_time() {
    let mut ws = Workspace::new();
    let (a, b) = (create(&mut ws, None, 0, "same"), create(&mut ws, None, 0, "same"));
    assert_ne!(a, b);
    assert!(ws.contains(a) && ws.contains(b));
}

#[test]
fn create_page_rejects_bad_parent_or_index_and_writes_nothing() {
    let mut ws = Workspace::new();
    let a = create(&mut ws, None, 0, "A");
    let (commits, _sub) = commit_counter(&ws);

    let ghost = PageId::new();
    let unknown_parent = WorkspaceCommand::CreatePage {
        parent: Some(ghost),
        index: 0,
        title: "x".into(),
        created_at: T0,
    };
    assert!(matches!(ws.apply(unknown_parent), Err(ModelError::PageNotFound(p)) if p == ghost));

    let too_far = WorkspaceCommand::CreatePage {
        parent: None,
        index: 2,
        title: "x".into(),
        created_at: T0,
    };
    assert!(matches!(
        ws.apply(too_far),
        Err(ModelError::IndexOutOfRange { index: 2, len: 1 })
    ));
    let too_far_child = WorkspaceCommand::CreatePage {
        parent: Some(a),
        index: 1,
        title: "x".into(),
        created_at: T0,
    };
    assert!(matches!(
        ws.apply(too_far_child),
        Err(ModelError::IndexOutOfRange { index: 1, len: 0 })
    ));

    assert_eq!(
        commits.load(Ordering::SeqCst),
        0,
        "rejected commands commit nothing"
    );
    assert_eq!(ws.len(), 1);
}

#[test]
fn rename_page_changes_only_the_title() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Draft");
    ws.apply(icon(page, Some("📝"))).unwrap();

    assert_eq!(ws.apply(rename(page, "Final")).unwrap(), Outcome::Changed);
    let meta = ws.page(page).unwrap();
    assert_eq!((meta.title.as_str(), meta.icon.as_deref()), ("Final", Some("📝")));

    ws.apply(rename(page, "")).unwrap();
    assert_eq!(ws.page(page).unwrap().title, "", "an empty title is allowed");

    let ghost = PageId::new();
    assert!(matches!(ws.apply(rename(ghost, "x")), Err(ModelError::PageNotFound(p)) if p == ghost));
}

#[test]
fn set_page_icon_sets_replaces_and_clears() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Ideas");

    assert_eq!(ws.apply(icon(page, Some("💡"))).unwrap(), Outcome::Changed);
    assert_eq!(ws.page(page).unwrap().icon.as_deref(), Some("💡"));
    ws.apply(icon(page, Some("🌱"))).unwrap();
    assert_eq!(ws.page(page).unwrap().icon.as_deref(), Some("🌱"));
    ws.apply(icon(page, None)).unwrap();
    assert_eq!(ws.page(page).unwrap().icon, None);
    ws.apply(icon(page, None)).unwrap();
    assert_eq!(ws.page(page).unwrap().icon, None, "clearing twice is fine");
}

#[test]
fn set_page_icon_rejects_empty_text_and_unknown_pages() {
    let mut ws = Workspace::new();
    let page = create(&mut ws, None, 0, "Ideas");
    ws.apply(icon(page, Some("💡"))).unwrap();
    let (commits, _sub) = commit_counter(&ws);

    assert!(matches!(ws.apply(icon(page, Some(""))), Err(ModelError::EmptyIcon(p)) if p == page));
    let ghost = PageId::new();
    assert!(matches!(ws.apply(icon(ghost, Some("💡"))), Err(ModelError::PageNotFound(p)) if p == ghost));

    assert_eq!(commits.load(Ordering::SeqCst), 0);
    assert_eq!(
        ws.page(page).unwrap().icon.as_deref(),
        Some("💡"),
        "a rejected icon leaves the old one"
    );
}

#[test]
fn each_command_is_exactly_one_commit() {
    let mut ws = Workspace::new();
    let (commits, _sub) = commit_counter(&ws);
    let count = || commits.load(Ordering::SeqCst);

    let a = create(&mut ws, None, 0, "A");
    assert_eq!(count(), 1);
    create(&mut ws, Some(a), 0, "A1");
    assert_eq!(count(), 2);
    ws.apply(rename(a, "Renamed")).unwrap();
    assert_eq!(count(), 3);
    ws.apply(icon(a, Some("🍃"))).unwrap();
    assert_eq!(count(), 4);
    ws.apply(icon(a, None)).unwrap();
    assert_eq!(count(), 5);

    assert!(ws.apply(icon(a, Some(""))).is_err());
    ws.apply(rename(a, "After an error")).unwrap();
    assert_eq!(
        count(),
        6,
        "a failed command leaves nothing for the next commit to sweep up"
    );
}

#[test]
fn command_results_survive_save_and_load() {
    let mut ws = Workspace::new();
    let a = create(&mut ws, None, 0, "A");
    let b = create(&mut ws, None, 0, "B");
    let child = create(&mut ws, Some(a), 0, "Child");
    ws.apply(rename(a, "Alpha")).unwrap();
    ws.apply(icon(child, Some("🌿"))).unwrap();

    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded.root_pages(), vec![b, a]);
    assert_eq!(loaded.children(a).unwrap(), vec![child]);
    assert_eq!(loaded.page(a).unwrap().title, "Alpha");
    assert_eq!(loaded.page(child).unwrap().icon.as_deref(), Some("🌿"));
}

#[test]
fn new_errors_explain_themselves() {
    let page = PageId::new();
    assert_eq!(
        ModelError::IndexOutOfRange { index: 3, len: 1 }.to_string(),
        "index 3 is past the end (1 items)"
    );
    assert_eq!(
        ModelError::EmptyIcon(page).to_string(),
        format!("page {page}: icon cannot be empty (use None to clear it)")
    );
}
