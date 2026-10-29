//! Inserting images in a real window over a real workspace folder (E05-S07): a pasted picture and a
//! picked file are stored in `assets/` and become an image block; text on the clipboard wins.

use std::path::{Path, PathBuf};

use gpui::{ClipboardItem, Image, ImageFormat, TestAppContext, VisualTestContext};
use leaf_core::AssetId;
use leaf_model::BlockKind;

use super::super::{Shell, Storage};

fn temp_workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leaf-ui-image-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("w.leaf")
}

/// A tiny real PNG.
fn png() -> Vec<u8> {
    let picture = image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 120, 90, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    picture.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A window over a new workspace at `path`, with a page titled "Trip" and the caret in an empty
/// paragraph below the title.
fn open_window<'a>(
    path: &Path,
    cx: &'a mut TestAppContext,
) -> (gpui::Entity<Shell>, &'a mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let folder = leaf_store::Workspace::open_or_create(path).unwrap();
    let (storage, workspace) = Storage::open(folder).unwrap();
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::with_storage(storage, workspace, window, cx));
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    cx.simulate_input("Trip");
    cx.simulate_keystrokes("enter");
    redraw(cx);
    (shell, cx)
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn blocks(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Vec<(BlockKind, String)> {
    shell.update(cx, |shell, _| {
        let page = shell.page().unwrap();
        page.rows()
            .unwrap()
            .into_iter()
            .map(|row| (page.doc().kind(row.id).unwrap(), row.text.unwrap_or_default()))
            .collect()
    })
}

fn image_of(bytes: &[u8]) -> BlockKind {
    BlockKind::Image {
        asset: AssetId::of(bytes),
        width: None,
        alt: String::new(),
    }
}

#[gpui::test]
fn a_pasted_screenshot_is_stored_and_becomes_an_image_with_an_editable_caption(cx: &mut TestAppContext) {
    let path = temp_workspace("paste");
    let (shell, cx) = open_window(&path, cx);
    let bytes = png();
    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
        ImageFormat::Png,
        bytes.clone(),
    )));

    cx.simulate_keystrokes("secondary-v");
    redraw(cx);
    assert_eq!(
        blocks(&shell, cx),
        vec![(image_of(&bytes), String::new())],
        "the empty block became the image"
    );
    let stored = path.join("assets").join(AssetId::of(&bytes).to_string());
    assert_eq!(
        std::fs::read(&stored).unwrap(),
        bytes,
        "the picture is in assets/"
    );
    // `debug_bounds` takes a `'static` selector; leaking one short string in a test is fine.
    let frame: &'static str = Box::leak(format!("image-{}", first_block(&shell, cx)).into_boxed_str());
    assert!(cx.debug_bounds(frame).is_some(), "the image frame is drawn");

    cx.simulate_input("Harbour at dusk");
    assert_eq!(
        blocks(&shell, cx)[0].1,
        "Harbour at dusk",
        "typing goes into the caption"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

fn first_block(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> leaf_core::BlockId {
    shell.update(cx, |shell, _| shell.page().unwrap().rows().unwrap()[0].id)
}

#[gpui::test]
fn text_on_the_clipboard_wins_over_a_picture_of_it(cx: &mut TestAppContext) {
    let path = temp_workspace("text-wins");
    let (shell, cx) = open_window(&path, cx);
    let mut item = ClipboardItem::new_string("A1 B1".into());
    item.entries.push(gpui::ClipboardEntry::Image(Image::from_bytes(
        ImageFormat::Png,
        png(),
    )));
    cx.write_to_clipboard(item);

    cx.simulate_keystrokes("secondary-v");
    assert_eq!(blocks(&shell, cx), vec![(BlockKind::Paragraph, "A1 B1".into())]);
    assert!(
        !path.join("assets").exists() || std::fs::read_dir(path.join("assets")).unwrap().next().is_none()
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn a_copied_file_that_isnt_a_picture_pastes_as_its_path(cx: &mut TestAppContext) {
    let path = temp_workspace("not-image");
    let notes = path.parent().unwrap().join("notes.txt");
    std::fs::write(&notes, "plain words").unwrap();
    let (shell, cx) = open_window(&path, cx);
    cx.write_to_clipboard(ClipboardItem {
        entries: vec![gpui::ClipboardEntry::ExternalPaths(gpui::ExternalPaths(
            vec![notes.clone()].into(),
        ))],
    });

    cx.simulate_keystrokes("secondary-v");
    assert_eq!(
        blocks(&shell, cx),
        vec![(BlockKind::Paragraph, notes.display().to_string())]
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn the_slash_menu_image_entry_opens_the_file_picker_and_inserts_the_chosen_file(cx: &mut TestAppContext) {
    let path = temp_workspace("slash");
    let picture = path.parent().unwrap().join("harbour.png");
    let bytes = png();
    std::fs::write(&picture, &bytes).unwrap();
    let (shell, cx) = open_window(&path, cx);

    cx.simulate_input("/image");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(cx.did_prompt_for_paths(), "the file picker is open");
    assert_eq!(
        blocks(&shell, cx),
        vec![(BlockKind::Paragraph, String::new())],
        "`/image` is gone"
    );

    let chosen = picture.clone();
    cx.simulate_path_prompt_response(move |options| {
        assert!(options.files && !options.directories && !options.multiple);
        Some(vec![chosen])
    });
    cx.run_until_parked();
    assert_eq!(blocks(&shell, cx), vec![(image_of(&bytes), String::new())]);
    assert!(path.join("assets").join(AssetId::of(&bytes).to_string()).exists());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn closing_the_file_picker_inserts_nothing(cx: &mut TestAppContext) {
    let path = temp_workspace("cancel");
    let (shell, cx) = open_window(&path, cx);
    cx.simulate_input("/image");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.simulate_path_prompt_response(|_| None);
    cx.run_until_parked();
    assert_eq!(blocks(&shell, cx), vec![(BlockKind::Paragraph, String::new())]);
    // A later paste is not taken for the cancelled request: it goes at the caret as usual.
    let bytes = png();
    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
        ImageFormat::Png,
        bytes.clone(),
    )));
    cx.simulate_keystrokes("secondary-v");
    assert_eq!(blocks(&shell, cx), vec![(image_of(&bytes), String::new())]);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
