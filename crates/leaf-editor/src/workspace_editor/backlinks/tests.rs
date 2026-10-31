//! Tests for the backlinks list's view model (E10-S07).

use leaf_core::{BlockId, PageId};
use leaf_model::Workspace;

use super::{BacklinkRow, BacklinkSource, backlinks_label};
use crate::WorkspaceEditor;

const NOW: i64 = 1_700_000_000_000;

fn source(page: PageId) -> BacklinkSource {
    BacklinkSource {
        page,
        block: BlockId::new(),
        before: "Flights for ".into(),
        after: " are booked".into(),
    }
}

#[test]
fn rows_show_the_linking_pages_as_they_are_now() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let japan = editor.new_page(NOW).unwrap();
    editor.rename(japan, "Trip to Japan").unwrap();
    editor.set_icon(japan, Some("🗾")).unwrap();
    let budget = editor.new_page(NOW).unwrap();
    editor.rename(budget, "Budget").unwrap();
    editor.set_icon(budget, Some("💴")).unwrap();
    let untitled = editor.new_page(NOW).unwrap();
    let trashed = editor.new_page(NOW).unwrap();
    editor.trash(trashed, NOW).unwrap();

    let found = vec![
        source(budget),
        source(untitled),
        source(trashed),
        source(PageId::new()),
    ];
    let rows = editor.backlinks(japan, found.clone());
    assert_eq!(
        rows[0],
        BacklinkRow {
            page: budget,
            block: found[0].block,
            icon: Some("💴".into()),
            title: "Budget".into(),
            before: "Flights for ".into(),
            link: "🗾 Trip to Japan".into(),
            after: " are booked".into(),
        }
    );
    assert_eq!(rows[1].title, "Untitled");
    assert_eq!(rows.len(), 2, "pages in the trash or gone are left out");
}

#[test]
fn the_line_counts_backlinks_and_is_absent_for_none() {
    assert_eq!(backlinks_label(0), None);
    assert_eq!(backlinks_label(1).as_deref(), Some("1 backlink"));
    assert_eq!(backlinks_label(3).as_deref(), Some("3 backlinks"));
}
