//! Tests for preview requests (E15-S05): noted only when allowed, for each way a bookmark is made, and
//! for "Refresh preview"; and saving a fetched preview.

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, Caret, Mark, PageCommand, PageDoc, PageOutcome, WebPreview};

use crate::{MenuEntry, PageEditor, Vertical};

const LEAF: &str = "https://leaf.app/blog";

fn page(texts: &[&str]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let ids = texts
        .iter()
        .map(|text| {
            let index = doc.root_blocks().len();
            let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind: BlockKind::Paragraph,
                text: (*text).into(),
            }) else {
                panic!("insert failed");
            };
            id
        })
        .collect();
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

fn bookmark_of(editor: &PageEditor) -> BlockId {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .find(|row| matches!(editor.doc().kind(row.id).unwrap(), BlockKind::WebBookmark { .. }))
        .unwrap()
        .id
}

#[test]
fn nothing_is_asked_for_while_previews_are_not_allowed() {
    let (mut editor, ids) = page(&[""]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.insert_web_bookmark(LEAF).unwrap();
    assert_eq!(editor.take_preview_requests(), Vec::new());
    let menu = editor.open_block_menu(bookmark_of(&editor)).unwrap().unwrap();
    assert!(
        !menu.items.iter().any(|i| i.entry == MenuEntry::RefreshPreview),
        "no Refresh offered"
    );
}

#[test]
fn every_way_of_making_a_bookmark_asks_for_its_preview_once() {
    // The slash menu's address.
    let (mut editor, ids) = page(&[""]);
    editor.set_previews_allowed(true);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.insert_web_bookmark("leaf.app/blog").unwrap();
    assert_eq!(editor.take_preview_requests(), vec![(ids[0], LEAF.to_owned())]);
    assert_eq!(editor.take_preview_requests(), Vec::new(), "taken once");

    // The offer after a paste.
    let (mut editor, ids) = page(&[""]);
    editor.set_previews_allowed(true);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.paste_text(LEAF).unwrap();
    editor.bookmark_offer_move(Vertical::Down);
    editor.bookmark_offer_apply(None).unwrap();
    assert_eq!(editor.take_preview_requests(), vec![(ids[0], LEAF.to_owned())]);

    // Turn into, from the block menu.
    let (mut editor, ids) = page(&["Leaf"]);
    editor.set_previews_allowed(true);
    editor
        .doc
        .apply(PageCommand::SetMark {
            id: ids[0],
            range: 0..4,
            mark: Mark::Link(LEAF.into()),
        })
        .unwrap();
    editor.open_block_menu(ids[0]).unwrap();
    editor.choose(MenuEntry::ToWebBookmark(LEAF.into())).unwrap();
    assert_eq!(editor.take_preview_requests(), vec![(ids[0], LEAF.to_owned())]);
}

#[test]
fn refresh_preview_asks_again_and_turning_previews_off_drops_what_waits() {
    let (mut editor, ids) = page(&[""]);
    editor.set_previews_allowed(true);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.insert_web_bookmark(LEAF).unwrap();
    editor.take_preview_requests();
    editor.open_block_menu(ids[0]).unwrap();
    assert_eq!(
        editor.choose(MenuEntry::RefreshPreview).unwrap(),
        None,
        "the menu closes"
    );
    assert_eq!(editor.take_preview_requests(), vec![(ids[0], LEAF.to_owned())]);

    editor.open_block_menu(ids[0]).unwrap();
    editor.choose(MenuEntry::RefreshPreview).unwrap();
    editor.set_previews_allowed(false);
    assert_eq!(editor.take_preview_requests(), Vec::new());
}

#[test]
fn a_fetched_preview_is_saved_off_the_undo_history_unless_the_bookmark_moved_on() {
    let (mut editor, ids) = page(&[""]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.insert_web_bookmark(LEAF).unwrap();
    let preview = WebPreview {
        title: Some("Leaf".into()),
        image: Some(AssetId::of(b"cover")),
        fetched_at: Some(1),
        ..WebPreview::default()
    };
    assert!(
        editor
            .set_web_bookmark_preview(ids[0], LEAF, preview.clone())
            .unwrap()
    );
    assert!(
        !editor
            .set_web_bookmark_preview(ids[0], "https://other.example", preview.clone())
            .unwrap()
    );
    assert!(
        editor.undo().unwrap(),
        "undo takes back the bookmark itself, not the preview"
    );
    assert_eq!(editor.doc().kind(ids[0]).unwrap(), BlockKind::Paragraph);
    assert!(
        !editor.set_web_bookmark_preview(ids[0], LEAF, preview).unwrap(),
        "no longer a bookmark"
    );
}

#[test]
fn missing_previews_are_asked_for_but_never_one_already_fetched() {
    let (mut editor, ids) = page(&["", "", ""]);
    for (block, url) in [
        (ids[0], LEAF),
        (ids[1], "https://example.com"),
        (ids[2], "https://synced.example"),
    ] {
        editor.set_caret(Caret { block, at: 0 }).unwrap();
        editor.insert_web_bookmark(url).unwrap();
    }
    // One was fetched here, one's preview arrived by sync; the first was never fetched.
    let fetched = |at| WebPreview {
        title: Some("Fetched".into()),
        fetched_at: Some(at),
        ..WebPreview::default()
    };
    editor
        .set_web_bookmark_preview(ids[1], "https://example.com", fetched(1))
        .unwrap();
    editor
        .set_web_bookmark_preview(ids[2], "https://synced.example", fetched(2))
        .unwrap();
    assert!(
        editor.has_preview_requests(),
        "they wait, so the app can ask the owner first"
    );
    assert_eq!(
        editor.take_preview_requests(),
        Vec::new(),
        "none is handed out until allowed"
    );
    assert!(
        !editor.has_preview_requests(),
        "and taking them while not allowed drops them"
    );
    editor.set_previews_allowed(true);
    editor.request_missing_previews().unwrap();
    assert_eq!(editor.take_preview_requests(), vec![(ids[0], LEAF.to_owned())]);
}
