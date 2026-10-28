//! A window saving to a real workspace folder: what is typed is there after reopening.

use std::path::{Path, PathBuf};

use gpui::{TestAppContext, VisualTestContext};
use leaf_model::{PageDoc, Workspace};
use leaf_store::DocKey;

use super::super::Shell;
use super::Storage;

fn temp_workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leaf-ui-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("w.leaf")
}

fn open_window<'a>(
    path: &Path,
    cx: &'a mut TestAppContext,
) -> (gpui::Entity<Shell>, &'a mut VisualTestContext) {
    cx.update(crate::init);
    let folder = leaf_store::Workspace::open_or_create(path).unwrap();
    let (storage, workspace) = Storage::open(folder).unwrap();
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::with_storage(storage, workspace, window, cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (shell, cx)
}

#[gpui::test]
fn switching_away_saves_everything_to_the_folder(cx: &mut TestAppContext) {
    let path = temp_workspace("blur");
    let (shell, cx) = open_window(&path, cx);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    cx.simulate_input("Plans");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("buy milk");
    let page = shell.read_with(cx, |shell, _| shell.editor().selected().unwrap());

    cx.deactivate_window();
    cx.run_until_parked();

    // Read the database directly (the window still holds the folder's lock).
    let store = leaf_store::Store::open(&path.join("workspace.sqlite")).unwrap();
    let workspace: Workspace = store.load(DocKey::Workspace).unwrap().unwrap();
    assert_eq!(workspace.page(page).unwrap().title, "Plans");
    let blocks: PageDoc = store.load(DocKey::Page(page)).unwrap().unwrap();
    let texts: Vec<String> = blocks
        .root_blocks()
        .into_iter()
        .filter_map(|b| blocks.plain_text(b).unwrap())
        .collect();
    assert_eq!(texts, vec!["buy milk"]);
    let unsaved = shell.read_with(cx, |shell, _| shell.editor().workspace().has_unsaved_changes());
    assert!(!unsaved, "everything was saved");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn a_page_saved_earlier_is_loaded_when_opened(cx: &mut TestAppContext) {
    let path = temp_workspace("load");
    let (shell, cx) = open_window(&path, cx);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    cx.simulate_input("Notes");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("first line");
    let page = shell.read_with(cx, |shell, _| shell.editor().selected().unwrap());
    shell.update(cx, |shell, _| shell.save(true));

    // A fresh editor over the same workspace document: opening the page loads its blocks from disk.
    shell.update(cx, |shell, _| {
        let workspace = Workspace::from_bytes(&shell.editor().workspace().to_bytes().unwrap()).unwrap();
        shell.editor = leaf_editor::WorkspaceEditor::new(workspace);
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(page, window, cx)));
    let texts = shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .page_editor(page)
            .unwrap()
            .rows()
            .unwrap()
            .into_iter()
            .map(|row| row.text.unwrap_or_default())
            .collect::<Vec<_>>()
    });
    assert_eq!(texts, vec!["first line"]);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
