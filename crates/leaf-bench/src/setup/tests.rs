#![allow(clippy::unwrap_used, reason = "tests panic on failure")]

use leaf_core::PageId;
use leaf_store::DocKey;

use super::{SavedPage, ViewedPage, caret_mid_page, typing_editor};
use crate::{BenchError, SAVED_AT, Scratch};

#[test]
fn a_saved_page_opens_with_every_block_in_its_view() {
    let saved = SavedPage::new(7, 300).unwrap();
    let mut editor = saved.open().unwrap();
    assert_eq!(editor.doc().len(), 300);
    assert_eq!(editor.view().unwrap().len(), editor.rows().unwrap().len());
}

#[test]
fn opening_a_page_that_was_never_saved_is_a_setup_error() {
    let mut saved = SavedPage::new(7, 10).unwrap();
    saved.page = PageId::new();
    let err = saved.open().err().unwrap();
    assert!(matches!(err, BenchError::Setup(_)));
    assert!(err.to_string().contains("not saved"));
}

#[test]
fn typing_lands_in_the_second_half_of_the_page() {
    let mut editor = typing_editor(7, 400).unwrap();
    let caret = editor.caret().unwrap().unwrap();
    let rows = editor.rows().unwrap();
    let index = rows.iter().position(|row| row.id == caret.block).unwrap();
    assert!(index >= rows.len() / 2);
    let before = editor.doc().plain_text(caret.block).unwrap().unwrap();
    editor.insert_text("a").unwrap();
    let after = editor.doc().plain_text(caret.block).unwrap().unwrap();
    assert_eq!(after, format!("a{before}"));
}

#[test]
fn an_opened_page_saves_only_what_was_typed() {
    let mut saved = SavedPage::new(7, 50).unwrap();
    let mut editor = saved.open().unwrap();
    caret_mid_page(&mut editor).unwrap();
    let key = DocKey::Page(saved.page);
    let store = saved.folder.store_mut();
    assert!(!store.save(key, editor.doc(), SAVED_AT).unwrap());
    editor.insert_text("a").unwrap();
    assert!(store.save(key, editor.doc(), SAVED_AT).unwrap());
    let reloaded = saved.open().unwrap();
    let caret = editor.caret().unwrap().unwrap();
    assert!(
        reloaded
            .doc()
            .plain_text(caret.block)
            .unwrap()
            .unwrap()
            .starts_with('a')
    );
}

#[test]
fn a_page_without_text_blocks_cannot_be_typed_in() {
    let err = typing_editor(7, 0).err().unwrap();
    assert!(matches!(err, BenchError::Setup(_)));
}

#[test]
fn the_scratch_folder_goes_away_with_the_saved_page() {
    let saved = SavedPage::new(7, 5).unwrap();
    let path = saved.folder.root().to_path_buf();
    assert!(path.exists());
    drop(saved);
    assert!(!path.exists());
    let fresh = Scratch::default();
    assert!(!fresh.path().exists());
}

#[test]
fn the_view_shows_a_typed_character_only_after_a_refresh() {
    let mut page = ViewedPage::new(7, 200).unwrap();
    let before = page.shown_text().unwrap().to_owned();
    page.type_char().unwrap();
    assert_eq!(page.shown_text().unwrap(), before);
    assert_eq!(page.refresh().unwrap(), 200);
    assert_eq!(page.shown_text().unwrap(), format!("a{before}"));
}

#[test]
fn an_empty_page_has_nowhere_to_type_into_its_view() {
    assert!(matches!(
        ViewedPage::new(7, 0).err().unwrap(),
        BenchError::Setup(_)
    ));
}
