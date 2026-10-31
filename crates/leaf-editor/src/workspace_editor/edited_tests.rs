//! Tests for recording when pages were edited (E18-S01, ADR-059): what counts as an edit, and that
//! undo is one.

use leaf_core::PageId;
use leaf_model::{PageCommand, PageDoc, Workspace};

use super::WorkspaceEditor;
use crate::{BuiltinCover, Caret};

const NOW: i64 = 1_700_000_000_000;

fn edited_at(editor: &WorkspaceEditor, page: PageId) -> i64 {
    editor.workspace().page(page).unwrap().edited_at
}

/// A new page, opened, with the caret in its empty paragraph.
fn open_new(editor: &mut WorkspaceEditor) -> PageId {
    let page = editor.new_page(NOW).unwrap();
    let open = editor.open_page().unwrap().unwrap();
    let block = open.doc().root_blocks()[0];
    open.set_caret(Caret { block, at: 0 }).unwrap();
    page
}

fn type_text(editor: &mut WorkspaceEditor, text: &str) {
    editor.open_page().unwrap().unwrap().insert_text(text).unwrap();
}

#[test]
fn opening_a_page_is_not_an_edit() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = open_new(&mut editor);
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW, "still the creation time");
}

#[test]
fn loading_a_saved_page_is_not_an_edit() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    // Saved empty, so loading it also adds the paragraph to type in.
    editor.insert_page(page, PageDoc::new()).unwrap();
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW);
}

#[test]
fn typing_is_recorded_once_per_stamp() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = open_new(&mut editor);
    type_text(&mut editor, "Hello");
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW + 1_000);

    editor.stamp_edits(NOW + 2_000).unwrap();
    assert_eq!(
        edited_at(&editor, page),
        NOW + 1_000,
        "nothing new, nothing recorded"
    );
}

#[test]
fn undo_and_redo_are_edits_too() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = open_new(&mut editor);
    type_text(&mut editor, "Hello");
    editor.stamp_edits(NOW + 1_000).unwrap();

    assert!(editor.open_page().unwrap().unwrap().undo().unwrap());
    editor.stamp_edits(NOW + 2_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW + 2_000);

    assert!(editor.open_page().unwrap().unwrap().redo().unwrap());
    editor.stamp_edits(NOW + 3_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW + 3_000);
}

#[test]
fn title_icon_and_cover_changes_are_edits() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();

    editor.rename(page, "Plans").unwrap();
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW + 1_000);

    editor.set_icon(page, Some("🌱")).unwrap();
    editor.stamp_edits(NOW + 2_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW + 2_000);

    editor.set_cover(page, Some(BuiltinCover::ALL[0])).unwrap();
    editor.stamp_edits(NOW + 3_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW + 3_000);
}

#[test]
fn unchanged_titles_icons_and_covers_are_not_edits() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    editor.rename(page, "").unwrap();
    editor.set_icon(page, None).unwrap();
    editor.set_cover(page, None).unwrap();
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW);
}

#[test]
fn moving_or_trashing_a_page_is_not_an_edit() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    editor.trash(page, NOW + 500).unwrap();
    editor.restore(page).unwrap();
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert_eq!(edited_at(&editor, page), NOW);
}

#[test]
fn each_edited_page_gets_its_own_time_and_others_keep_theirs() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let quiet = editor.new_page(NOW).unwrap();
    let busy = open_new(&mut editor);
    editor.rename(quiet, "Quiet").unwrap();
    type_text(&mut editor, "Busy");
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert_eq!(edited_at(&editor, quiet), NOW + 1_000);
    assert_eq!(edited_at(&editor, busy), NOW + 1_000);

    type_text(&mut editor, "!");
    editor.stamp_edits(NOW + 2_000).unwrap();
    assert_eq!(edited_at(&editor, quiet), NOW + 1_000);
    assert_eq!(edited_at(&editor, busy), NOW + 2_000);
}

#[test]
fn a_page_deleted_before_stamping_is_skipped() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = open_new(&mut editor);
    type_text(&mut editor, "Gone soon");
    editor.rename(page, "Gone").unwrap();
    editor.trash(page, NOW + 500).unwrap();
    editor.delete_forever(page).unwrap();
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert!(!editor.workspace().contains(page));
}

#[test]
fn stamping_writes_through_commands_and_is_saved() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = open_new(&mut editor);
    editor.workspace().mark_saved();
    type_text(&mut editor, "Hi");
    editor.stamp_edits(NOW + 1_000).unwrap();
    assert!(editor.workspace().has_unsaved_changes());
    let reloaded = Workspace::from_bytes(&editor.workspace().to_bytes().unwrap()).unwrap();
    assert_eq!(reloaded.page(page).unwrap().edited_at, NOW + 1_000);
}

#[test]
fn the_page_doc_counts_raw_commands_as_edits() {
    // Any change to the blocks counts, however the editor made it.
    let mut doc = PageDoc::new();
    assert!(!doc.has_unstamped_edits());
    doc.apply(PageCommand::InsertBlock {
        parent: None,
        index: 0,
        kind: leaf_model::BlockKind::Paragraph,
        text: "x".into(),
    })
    .unwrap();
    assert!(doc.has_unstamped_edits());
    doc.mark_stamped();
    assert!(!doc.has_unstamped_edits());
    doc.mark_saved();
    assert!(
        !doc.has_unstamped_edits(),
        "saving and stamping are tracked apart"
    );
}
