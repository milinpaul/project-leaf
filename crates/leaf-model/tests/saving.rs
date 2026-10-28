//! Tracking unsaved changes and loading snapshot + updates (E03-S03).

use leaf_model::{BlockKind, Outcome, PageCommand, PageDoc, PageOutcome, Workspace, WorkspaceCommand};

fn pages(ws: &Workspace) -> Vec<(String, Option<String>)> {
    ws.root_pages()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| ws.page(id).ok())
        .map(|meta| (meta.title, meta.icon))
        .collect()
}

fn create(ws: &mut Workspace, title: &str) {
    let index = ws.root_pages().map_or(0, |p| p.len());
    let outcome = ws.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index,
        title: title.into(),
        created_at: 1,
    });
    assert!(matches!(outcome, Ok(Outcome::PageCreated(_))));
}

fn text(page: &PageDoc) -> Vec<String> {
    page.root_blocks()
        .into_iter()
        .map(|b| page.plain_text(b).ok().flatten().unwrap_or_default())
        .collect()
}

#[test]
fn a_new_or_loaded_document_has_nothing_unsaved() {
    assert_eq!(Workspace::new().unsaved_changes().unwrap(), None);
    assert_eq!(PageDoc::new().unsaved_changes().unwrap(), None);
    let mut ws = Workspace::new();
    create(&mut ws, "a");
    let loaded = Workspace::from_bytes(&ws.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded.unsaved_changes().unwrap(), None);
}

#[test]
fn snapshot_plus_updates_rebuilds_the_workspace() {
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0); // so undo takes back only the last page
    create(&mut ws, "first");
    let snapshot = ws.to_bytes().unwrap();
    ws.mark_saved();
    assert_eq!(ws.unsaved_changes().unwrap(), None);

    let mut updates = Vec::new();
    for title in ["second", "third"] {
        create(&mut ws, title);
        updates.push(ws.unsaved_changes().unwrap().unwrap());
        ws.mark_saved();
    }
    assert!(ws.undo().unwrap());
    updates.push(ws.unsaved_changes().unwrap().expect("undo is a change too"));

    let loaded = Workspace::from_saved(&snapshot, &updates).unwrap();
    assert_eq!(pages(&loaded), pages(&ws));
    assert_eq!(pages(&loaded).len(), 2);
}

#[test]
fn a_failed_save_keeps_the_changes_for_next_time() {
    let mut page = PageDoc::new();
    let snapshot = page.to_bytes().unwrap();
    page.apply(PageCommand::InsertBlock {
        parent: None,
        index: 0,
        kind: BlockKind::Paragraph,
        text: "one".into(),
    })
    .unwrap();
    let lost = page.unsaved_changes().unwrap().unwrap();
    drop(lost); // the write failed, so mark_saved isn't called
    let id = page.root_blocks()[0];
    let outcome = page
        .apply(PageCommand::InsertText {
            id,
            pos: 3,
            text: " two".into(),
        })
        .unwrap();
    assert_eq!(outcome, PageOutcome::Changed);

    let both = page.unsaved_changes().unwrap().unwrap();
    let loaded = PageDoc::from_saved(&snapshot, &[both]).unwrap();
    assert_eq!(text(&loaded), vec!["one two"]);
    assert_eq!(loaded.check_invariants(), Ok(()));
}

#[test]
fn bytes_that_are_not_an_update_fail_to_load() {
    let snapshot = PageDoc::new().to_bytes().unwrap();
    assert!(PageDoc::from_saved(&snapshot, &[vec![1, 2, 3]]).is_err());
    assert!(Workspace::from_saved(&snapshot, &[vec![9; 8]]).is_err());
}

#[test]
fn recover_skips_damaged_updates_and_keeps_the_rest() {
    let mut ws = Workspace::new();
    let snapshot = ws.to_bytes().unwrap();
    let mut updates = Vec::new();
    for title in ["one", "two"] {
        create(&mut ws, title);
        updates.push(ws.unsaved_changes().unwrap().unwrap());
        ws.mark_saved();
    }
    // A damaged update in the middle, as when new saves were appended after a bad one.
    let mut damaged = updates[1].clone();
    let last = damaged.len() - 1;
    damaged[last] ^= 0xFF;
    damaged.truncate(last / 2);
    let updates = vec![updates[0].clone(), damaged, vec![], updates[1].clone()];

    assert!(Workspace::from_saved(&snapshot, &updates).is_err());
    let (recovered, skipped) = Workspace::recover(&snapshot, &updates).unwrap();
    assert_eq!(skipped, vec![1, 2]);
    assert_eq!(pages(&recovered), pages(&ws));

    let (page, skipped) = PageDoc::recover(&PageDoc::new().to_bytes().unwrap(), &[]).unwrap();
    assert_eq!((page.len(), skipped), (0, vec![]));
    assert!(
        PageDoc::recover(&[1, 2, 3], &[]).is_err(),
        "a damaged snapshot can't be skipped"
    );
}
