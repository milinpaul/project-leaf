//! Drawing and resizing pictures in a real window over a real workspace folder (E07-S08).

use std::path::{Path, PathBuf};

use gpui::{Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, point, px, size};
use leaf_core::BlockId;
use leaf_model::BlockKind;

use super::super::{Shell, Storage};

fn temp_workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leaf-ui-image-row-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("w.leaf")
}

/// A real PNG of `width` × `height` pixels.
fn png(width: u32, height: u32) -> Vec<u8> {
    let picture = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 120, 90, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    picture.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// A window over a new workspace at `path` with a page holding the picture `bytes`. The picture
/// is not drawn or loaded yet.
fn open_with_picture<'a>(
    path: &Path,
    bytes: &[u8],
    cx: &'a mut TestAppContext,
) -> (gpui::Entity<Shell>, &'a mut VisualTestContext, BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let folder = leaf_store::Workspace::open_or_create(path).unwrap();
    let (storage, workspace) = Storage::open(folder).unwrap();
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::with_storage(storage, workspace, window, cx));
    cx.simulate_resize(size(px(1200.0), px(900.0)));
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    cx.simulate_input("Trip");
    cx.simulate_keystrokes("enter");
    let block = shell.update(cx, |shell, _| {
        let asset = shell.storage.as_mut().unwrap().import_image(bytes).unwrap();
        shell.page().unwrap().insert_image(asset).unwrap();
        let page = shell.page().unwrap();
        page.rows()
            .unwrap()
            .into_iter()
            .find(|row| matches!(page.doc().kind(row.id), Ok(BlockKind::Image { .. })))
            .unwrap()
            .id
    });
    (shell, cx, block)
}

/// Lets the picture load, then draws twice: once with it, once more with the measured room.
fn load(cx: &mut VisualTestContext) {
    draw(cx);
    cx.run_until_parked();
    draw(cx);
    draw(cx);
}

fn selector(name: &str, block: BlockId) -> &'static str {
    Box::leak(format!("{name}-{block}").into_boxed_str())
}

fn picture_size(cx: &mut VisualTestContext, block: BlockId) -> (f32, f32) {
    let bounds = cx.debug_bounds(selector("picture", block)).unwrap();
    (f32::from(bounds.size.width), f32::from(bounds.size.height))
}

fn stored_width(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> Option<f64> {
    shell.update(cx, |shell, _| {
        match shell.page().unwrap().doc().kind(block).unwrap() {
            BlockKind::Image { width, .. } => width,
            other => panic!("not an image: {other:?}"),
        }
    })
}

#[gpui::test]
fn a_placeholder_shows_until_the_picture_loads_at_its_natural_size(cx: &mut TestAppContext) {
    let path = temp_workspace("natural");
    let (_shell, cx, block) = open_with_picture(&path, &png(300, 150), cx);
    draw(cx);
    assert!(
        cx.debug_bounds(selector("image-frame", block)).is_some(),
        "loading frame first"
    );
    assert!(cx.debug_bounds(selector("picture", block)).is_none());

    load(cx);
    assert!(cx.debug_bounds(selector("image-frame", block)).is_none());
    assert_eq!(picture_size(cx, block), (300.0, 150.0));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn a_picture_wider_than_the_column_is_shrunk_to_fit_keeping_its_shape(cx: &mut TestAppContext) {
    let path = temp_workspace("wide");
    let (_shell, cx, block) = open_with_picture(&path, &png(2000, 500), cx);
    load(cx);
    let (width, height) = picture_size(cx, block);
    assert!(width < 720.0, "fits the reading column, got {width}");
    assert!(
        (height - width / 4.0).abs() < 0.5,
        "keeps 4:1, got {width} × {height}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn a_stored_width_is_used(cx: &mut TestAppContext) {
    let path = temp_workspace("stored");
    let (shell, cx, block) = open_with_picture(&path, &png(300, 150), cx);
    shell.update(cx, |shell, _| {
        shell.page().unwrap().set_image_width(block, Some(120.0)).unwrap();
    });
    load(cx);
    assert_eq!(picture_size(cx, block), (120.0, 60.0));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn a_damaged_picture_shows_a_frame_saying_so(cx: &mut TestAppContext) {
    let path = temp_workspace("broken");
    // A PNG cut short: the store takes it by its header, but it can't be decoded.
    let cut = png(300, 150)[..40].to_vec();
    let (_shell, cx, block) = open_with_picture(&path, &cut, cx);
    load(cx);
    assert!(cx.debug_bounds(selector("image-frame", block)).is_some());
    assert!(cx.debug_bounds(selector("picture", block)).is_none());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// Presses on `from` and moves right by `by` in small steps, without letting go.
fn drag_by(cx: &mut VisualTestContext, from: Point<gpui::Pixels>, by: f32) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    for step in 1..=8u8 {
        let at = point(from.x + px(by * f32::from(step) / 8.0), from.y);
        cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::none());
        draw(cx);
    }
}

#[gpui::test]
fn dragging_the_handle_resizes_live_and_saves_the_width_once_as_one_undo_step(cx: &mut TestAppContext) {
    let path = temp_workspace("resize");
    let (shell, cx, block) = open_with_picture(&path, &png(300, 150), cx);
    load(cx);
    let handle = cx.debug_bounds(selector("image-resize", block)).unwrap().center();

    drag_by(cx, handle, -100.0);
    let (width, height) = picture_size(cx, block);
    assert!((width - 200.0).abs() <= 1.0, "follows the pointer, got {width}");
    assert!((height - width / 2.0).abs() < 0.5, "keeps its shape");
    assert_eq!(stored_width(&shell, cx, block), None, "nothing saved mid-drag");

    let end = point(handle.x - px(100.0), handle.y);
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    draw(cx);
    let saved = stored_width(&shell, cx, block).unwrap();
    assert!((saved - 200.0).abs() <= 1.0, "saved {saved}");
    assert!(shell.update(cx, |shell, _| shell.blocks.resizing.is_none()));

    shell.update(cx, |shell, _| shell.page().unwrap().undo().unwrap());
    assert_eq!(stored_width(&shell, cx, block), None, "one undo puts it back");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn an_image_cant_be_dragged_wider_than_the_column(cx: &mut TestAppContext) {
    let path = temp_workspace("widest");
    let (shell, cx, block) = open_with_picture(&path, &png(300, 150), cx);
    load(cx);
    let handle = cx.debug_bounds(selector("image-resize", block)).unwrap().center();
    drag_by(cx, handle, 2000.0);
    cx.simulate_mouse_up(
        point(handle.x + px(2000.0), handle.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    draw(cx);
    let saved = stored_width(&shell, cx, block).unwrap();
    assert!(saved > 300.0 && saved < 720.0, "stops at the column, got {saved}");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn clicking_the_handle_without_moving_saves_nothing(cx: &mut TestAppContext) {
    let path = temp_workspace("click");
    let (shell, cx, block) = open_with_picture(&path, &png(300, 150), cx);
    load(cx);
    let handle = cx.debug_bounds(selector("image-resize", block)).unwrap().center();
    cx.simulate_click(handle, Modifiers::none());
    draw(cx);
    assert_eq!(stored_width(&shell, cx, block), None);
    assert_eq!(picture_size(cx, block), (300.0, 150.0));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
