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
    cx.update(|cx| crate::init(None, cx));
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
    // Saving recorded when the page was edited, and that time went to disk too (E18-S01).
    let (edited_at, unstamped) = shell.read_with(cx, |shell, _| {
        let editor = shell.editor();
        let unstamped = editor.page_editor(page).unwrap().doc().has_unstamped_edits();
        (editor.workspace().page(page).unwrap().edited_at, unstamped)
    });
    assert!(!unstamped, "the typing was stamped before saving");
    assert_eq!(workspace.page(page).unwrap().edited_at, edited_at);
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

#[gpui::test]
fn deleting_a_page_for_good_removes_its_document_from_disk(cx: &mut TestAppContext) {
    let path = temp_workspace("delete");
    let (shell, cx) = open_window(&path, cx);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    cx.simulate_input("Doomed");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("some text");
    let page = shell.read_with(cx, |shell, _| shell.editor().selected().unwrap());
    cx.update(|_, cx| shell.update(cx, |shell, _| shell.save(true)));
    let store = leaf_store::Store::open(&path.join("workspace.sqlite")).unwrap();
    assert!(
        store.load::<PageDoc>(DocKey::Page(page)).unwrap().is_some(),
        "saved first"
    );

    cx.update(|_, cx| {
        shell.update(cx, |shell, cx| {
            shell.trash_page(page, cx);
            shell.delete_page_forever(page, cx);
        });
    });
    assert!(
        store.load::<PageDoc>(DocKey::Page(page)).unwrap().is_none(),
        "its blocks are gone"
    );
    let workspace: Workspace = store.load(DocKey::Workspace).unwrap().unwrap();
    assert!(
        !workspace.contains(page),
        "and the saved page tree no longer lists it"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// E06-S19: the page open when the window closes opens again with the workspace; a page in the trash
/// doesn't.
#[gpui::test]
fn the_last_open_page_opens_again(cx: &mut TestAppContext) {
    let path = temp_workspace("last-page");
    let (first, second) = {
        let (shell, cx) = open_window(&path, cx);
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        let first = shell.read_with(cx, |shell, _| shell.editor().selected().unwrap());
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
        let second = shell.read_with(cx, |shell, _| shell.editor().selected().unwrap());
        cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(first, window, cx)));
        // Closing: everything is saved and the folder's lock let go.
        shell.update(cx, |shell, _| {
            shell.save(true);
            shell.storage = None;
        });
        (first, second)
    };

    let (shell, cx) = open_window(&path, cx);
    let open = shell.read_with(cx, |shell, _| shell.editor().selected());
    assert_eq!(open, Some(first), "the page that was open last");
    assert_ne!(Some(second), open);

    // Trash it, close, reopen: nothing is opened, as before this feature.
    shell.update(cx, |shell, _| shell.editor.trash(first, 0).unwrap());
    shell.update(cx, |shell, _| {
        shell.save(true);
        shell.storage = None;
    });
    let path_again = path.clone();
    let (shell, cx) = open_window(&path_again, cx);
    assert_eq!(shell.read_with(cx, |shell, _| shell.editor().selected()), None);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
