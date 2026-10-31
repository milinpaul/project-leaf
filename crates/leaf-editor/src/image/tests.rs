//! Tests for image insertion (E05-S07): where the block goes, the slash menu's request for a file,
//! undo, and what a paste from another app inserts.

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};

use super::{MIN_IMAGE_WIDTH, PasteKind, image_display_size, paste_kind, resized_image_width};
use crate::{Key, KeyOutcome, PageEditor};

fn page(blocks: &[(BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let ids = blocks
        .iter()
        .map(|(kind, text)| {
            let index = doc.root_blocks().len();
            let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind: kind.clone(),
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

fn para(text: &str) -> (BlockKind, &str) {
    (BlockKind::Paragraph, text)
}

fn picture() -> AssetId {
    AssetId::of(b"a picture")
}

fn image() -> BlockKind {
    BlockKind::Image {
        asset: picture(),
        width: None,
        alt: String::new(),
    }
}

fn outline(editor: &PageEditor) -> Vec<(BlockKind, String)> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| (editor.doc().kind(row.id).unwrap(), row.text.unwrap_or_default()))
        .collect()
}

fn type_keys(editor: &mut PageEditor, keys: &str) {
    for ch in keys.chars() {
        editor.insert_text(&ch.to_string()).unwrap();
    }
}

#[test]
fn an_empty_block_becomes_the_image_and_the_caret_goes_into_its_caption() {
    let (mut editor, ids) = page(&[para("Before"), para("")]);
    editor.set_caret(Caret { block: ids[1], at: 0 }).unwrap();

    let caret = editor.insert_image(picture()).unwrap();
    assert_eq!(caret, Some(Caret { block: ids[1], at: 0 }));
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Paragraph, "Before".into()), (image(), String::new())]
    );
}

#[test]
fn next_to_text_the_image_goes_below_and_the_text_is_untouched() {
    let (mut editor, ids) = page(&[para("Words"), para("After")]);
    editor.set_caret(Caret { block: ids[0], at: 2 }).unwrap();

    let caret = editor.insert_image(picture()).unwrap().unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, "Words".into()),
            (image(), String::new()),
            (BlockKind::Paragraph, "After".into()),
        ]
    );
    assert_eq!(caret.at, 0);
    assert_eq!(
        editor.caret().unwrap(),
        Some(caret),
        "the caret is in the new caption"
    );
}

#[test]
fn an_image_pasted_into_an_empty_caption_goes_below_rather_than_replacing_the_image() {
    let (mut editor, ids) = page(&[(image(), "")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.insert_image(AssetId::of(b"another")).unwrap();
    assert_eq!(outline(&editor).len(), 2);
    assert_eq!(outline(&editor)[0].0, image());
}

#[test]
fn the_caption_is_editable_text() {
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.insert_image(picture()).unwrap();
    type_keys(&mut editor, "Sunset");
    assert_eq!(outline(&editor), vec![(image(), "Sunset".into())]);
    // Enter in a caption adds a paragraph below instead of splitting it (ADR-020).
    let outcome = editor.press(Key::Enter).unwrap();
    assert!(matches!(outcome, KeyOutcome::Caret(_)));
    assert_eq!(outline(&editor)[1], (BlockKind::Paragraph, String::new()));
}

#[test]
fn inserting_is_one_undo_step() {
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    editor.insert_image(picture()).unwrap();
    assert!(editor.undo().unwrap());
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, String::new())]);
    assert!(editor.redo().unwrap());
    assert_eq!(outline(&editor), vec![(image(), String::new())]);
}

#[test]
fn with_no_caret_and_no_request_nothing_is_inserted() {
    let (mut editor, _) = page(&[para("Text")]);
    assert_eq!(editor.insert_image(picture()).unwrap(), None);
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "Text".into())]);
}

#[test]
fn choosing_image_in_the_slash_menu_asks_for_a_file_once_and_fills_that_spot() {
    let (mut editor, ids) = page(&[para(""), para("Elsewhere")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    type_keys(&mut editor, "/image");
    let menu = editor.slash_menu().unwrap().unwrap();
    assert_eq!(menu.items[0].label, "Image", "the query finds it first");
    assert!(!editor.take_image_request(), "nothing asked for yet");

    editor.press(Key::Enter).unwrap();
    assert_eq!(
        outline(&editor)[0],
        (BlockKind::Paragraph, String::new()),
        "`/image` is gone"
    );
    assert!(editor.take_image_request(), "the UI is told to show the picker");
    assert!(!editor.take_image_request(), "only once");

    // While the picker is open the caret moves elsewhere; the picture still goes where it was asked.
    editor.set_caret(Caret { block: ids[1], at: 3 }).unwrap();
    let caret = editor.insert_image(picture()).unwrap().unwrap();
    assert_eq!(caret.block, ids[0]);
    assert_eq!(outline(&editor)[0], (image(), String::new()));
    assert_eq!(outline(&editor)[1], (BlockKind::Paragraph, "Elsewhere".into()));
}

#[test]
fn a_cancelled_picker_leaves_the_page_as_it_was_without_the_query() {
    let (mut editor, ids) = page(&[para("Notes ")]);
    editor.set_caret(Caret { block: ids[0], at: 6 }).unwrap();
    type_keys(&mut editor, "/img");
    editor.slash_apply(None).unwrap();
    assert!(editor.take_image_request());
    editor.cancel_image();
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "Notes ".into())]);
    // A later paste with no new request goes at the caret, as usual.
    editor.set_caret(Caret { block: ids[0], at: 6 }).unwrap();
    editor.insert_image(picture()).unwrap();
    assert_eq!(outline(&editor)[1], (image(), String::new()));
}

#[test]
fn undo_after_choosing_image_gives_back_the_typed_query() {
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(Caret { block: ids[0], at: 0 }).unwrap();
    type_keys(&mut editor, "/image");
    editor.slash_apply(None).unwrap();
    editor.insert_image(picture()).unwrap();
    assert!(editor.undo().unwrap(), "the image goes");
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, String::new())]);
    assert!(editor.undo().unwrap(), "then the query comes back");
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "/image".into())]);
}

#[test]
fn a_paste_prefers_leaf_blocks_then_text_then_a_picture() {
    assert_eq!(paste_kind(true, Some("x"), true), PasteKind::Fragment);
    assert_eq!(
        paste_kind(false, Some("cells"), true),
        PasteKind::Text,
        "a spreadsheet's text wins"
    );
    assert_eq!(paste_kind(false, None, true), PasteKind::Image, "a screenshot");
    assert_eq!(
        paste_kind(false, Some(""), true),
        PasteKind::Image,
        "empty text doesn't count"
    );
    assert_eq!(paste_kind(false, Some("x"), false), PasteKind::Text);
    assert_eq!(paste_kind(false, None, false), PasteKind::Nothing);
}

// ---- Size and resizing (E07-S08) ----

#[test]
fn a_picture_draws_at_its_natural_size_until_it_needs_to_fit_the_column() {
    assert_eq!(image_display_size(None, (400.0, 200.0), 640.0), (400.0, 200.0));
    assert_eq!(
        image_display_size(None, (1600.0, 900.0), 640.0),
        (640.0, 360.0),
        "shrunk, same shape"
    );
}

#[test]
fn a_resized_picture_draws_at_its_stored_width_shrinking_only_to_fit() {
    assert_eq!(
        image_display_size(Some(300.0), (1600.0, 800.0), 640.0),
        (300.0, 150.0)
    );
    assert_eq!(
        image_display_size(Some(900.0), (1600.0, 800.0), 640.0),
        (640.0, 320.0),
        "a narrow window shrinks it; the stored width stays for a wider one"
    );
}

#[test]
fn a_picture_with_no_size_draws_nothing() {
    assert_eq!(image_display_size(None, (0.0, 10.0), 640.0), (0.0, 0.0));
    assert_eq!(image_display_size(Some(200.0), (10.0, 0.0), 640.0), (0.0, 0.0));
}

#[test]
fn dragging_the_handle_follows_the_pointer_within_limits_in_whole_pixels() {
    assert_eq!(resized_image_width(300.0, 45.4, 640.0), 345.0);
    assert_eq!(
        resized_image_width(300.0, -1000.0, 640.0),
        MIN_IMAGE_WIDTH,
        "never smaller than the minimum"
    );
    assert_eq!(
        resized_image_width(300.0, 1000.0, 640.0),
        640.0,
        "never wider than the column"
    );
    assert_eq!(
        resized_image_width(40.0, 0.0, 30.0),
        30.0,
        "a column narrower than the minimum wins"
    );
}

#[test]
fn setting_the_width_is_one_undo_step_and_unchanged_widths_record_nothing() {
    let (mut editor, ids) = page(&[(image(), "Caption"), para("Text")]);
    editor.set_image_width(ids[0], Some(320.0)).unwrap();
    assert_eq!(
        editor.doc().kind(ids[0]).unwrap(),
        BlockKind::Image {
            asset: picture(),
            width: Some(320.0),
            alt: String::new()
        }
    );
    let before = editor.doc().to_bytes().unwrap();
    editor.set_image_width(ids[0], Some(320.0)).unwrap();
    assert_eq!(
        editor.doc().to_bytes().unwrap(),
        before,
        "the same width records nothing"
    );
    editor.set_image_width(ids[1], Some(10.0)).unwrap();
    assert_eq!(
        editor.doc().kind(ids[1]).unwrap(),
        BlockKind::Paragraph,
        "not an image: left alone"
    );

    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.doc().kind(ids[0]).unwrap(),
        image(),
        "back to its natural size"
    );
    assert!(!editor.undo().unwrap(), "that was the only step");
}

#[test]
fn the_view_carries_the_picture_and_its_width() {
    let (mut editor, ids) = page(&[(image(), "")]);
    editor.set_image_width(ids[0], Some(250.0)).unwrap();
    let row = editor.rows().unwrap().remove(0);
    assert_eq!(
        row.style,
        crate::BlockStyle::Image {
            asset: picture(),
            width: Some(250.0),
            alt: String::new()
        }
    );
}
