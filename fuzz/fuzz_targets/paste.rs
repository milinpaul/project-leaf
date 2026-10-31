//! Text pasted from another app (Markdown, links, `[[page]]` names, anything) and Leaf's own clipboard
//! fragment JSON, pasted into a small page. The input's first byte picks where: a paragraph's middle, a code
//! block, or the start of a list item. The page must keep its rules after the paste, and undo must work.
#![no_main]

use std::sync::OnceLock;

use leaf_core::BlockId;
use leaf_editor::{Fragment, PageEditor};
use leaf_fuzz::walk_page;
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};
use libfuzzer_sys::fuzz_target;

/// Where each paste goes: block number and position in its text.
const SPOTS: [(usize, usize); 3] = [(0, 6), (1, 3), (2, 0)];

/// The saved bytes of a page with a paragraph, a code block and a list item, and their IDs. Built once:
/// loading it is much faster than making it again for every input.
fn page() -> Option<&'static (Vec<u8>, Vec<BlockId>)> {
    static PAGE: OnceLock<Option<(Vec<u8>, Vec<BlockId>)>> = OnceLock::new();
    PAGE.get_or_init(|| {
        let mut doc = PageDoc::new();
        let mut ids = Vec::new();
        let blocks = [
            (BlockKind::Paragraph, "Hello world"),
            (
                BlockKind::Code {
                    language: "rust".into(),
                },
                "fn main() {}",
            ),
            (BlockKind::BulletedList, "item"),
        ];
        for (index, (kind, text)) in blocks.into_iter().enumerate() {
            let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind,
                text: text.into(),
            }) else {
                return None;
            };
            ids.push(id);
        }
        Some((doc.to_bytes().ok()?, ids))
    })
    .as_ref()
}

fuzz_target!(|data: &[u8]| {
    leaf_fuzz::allow_caught_panics();
    let Some((bytes, ids)) = page() else {
        return;
    };
    let Ok(doc) = PageDoc::from_bytes(bytes) else {
        return;
    };
    let (block, at) = SPOTS[usize::from(data.first().copied().unwrap_or(0)) % SPOTS.len()];
    let mut editor = PageEditor::new(doc);
    if editor
        .set_caret(Caret {
            block: ids[block],
            at,
        })
        .is_err()
    {
        return;
    }
    let text = String::from_utf8_lossy(data);
    match Fragment::from_json(&text) {
        Some(fragment) => drop(editor.paste(&fragment)),
        None => drop(editor.paste_text(&text)),
    }
    if let Err(problems) = editor.doc().check_invariants() {
        panic!("paste broke the page: {problems:?}");
    }
    walk_page(editor.doc());
    let _ = editor.undo();
    walk_page(editor.doc());
});
