//! Two devices editing views and boards at once, then syncing. Each test checks both peers end up the same and
//! that the result is one a person would expect.

use loro::LoroResult;

use super::{DEFAULT, Db, at, kanban, sync};

fn same(a: &Db, b: &Db) {
    assert_eq!(a.view_ids(), b.view_ids(), "same tabs");
    assert_eq!(a.rows(), b.rows(), "same row order");
    assert_eq!(
        a.cells().get_deep_value(),
        b.cells().get_deep_value(),
        "same cells"
    );
    assert_eq!(
        a.settings().get_deep_value(),
        b.settings().get_deep_value(),
        "same view settings"
    );
}

#[test]
fn views_made_on_two_devices_are_both_kept() -> LoroResult<()> {
    let a = kanban(1)?;
    let b = a.fork(2)?;
    a.add_view("v-a", "Board", "board", Some("status"))?;
    b.add_view("v-b", "Board", "board", Some("status"))?;
    sync(&a, &b)?;
    same(&a, &b);
    let tabs = a.view_ids();
    assert_eq!(tabs.len(), 3, "both, even with the same name: {tabs:?}");
    assert_eq!(
        tabs[0], DEFAULT,
        "each device made the first view's node; it reads once"
    );
    Ok(())
}

#[test]
fn a_view_deleted_on_one_device_stays_deleted_when_the_other_edits_it() -> LoroResult<()> {
    let a = kanban(1)?;
    a.add_view("v-board", "Board", "board", Some("status"))?;
    let b = a.fork(2)?;
    a.delete_view("v-board")?;
    b.set_view("v-board", "name", "Renamed")?;
    b.hide("v-board", "notes")?;
    sync(&a, &b)?;
    same(&a, &b);
    assert_eq!(a.view_ids(), [DEFAULT]);
    Ok(())
}

#[test]
fn a_tab_moved_on_one_device_and_deleted_on_the_other_stays_deleted() -> LoroResult<()> {
    let a = kanban(1)?;
    a.add_view("v-1", "One", "table", None)?;
    a.add_view("v-2", "Two", "board", Some("status"))?;
    let b = a.fork(2)?;
    a.move_view("v-2", DEFAULT)?;
    b.delete_view("v-2")?;
    sync(&a, &b)?;
    same(&a, &b);
    assert_eq!(a.view_ids(), [DEFAULT, "v-1"]);
    // And the other way round: the move after the delete in Loro's order.
    let c = kanban(3)?;
    c.add_view("v-1", "One", "table", None)?;
    c.add_view("v-2", "Two", "board", Some("status"))?;
    let d = c.fork(4)?;
    d.move_view("v-2", DEFAULT)?;
    c.delete_view("v-2")?;
    sync(&c, &d)?;
    same(&c, &d);
    assert_eq!(c.view_ids(), [DEFAULT, "v-1"]);
    Ok(())
}

#[test]
fn tabs_reordered_on_two_devices_settle_on_one_order() -> LoroResult<()> {
    let a = kanban(1)?;
    a.add_view("v-1", "One", "table", None)?;
    a.add_view("v-2", "Two", "board", Some("status"))?;
    let b = a.fork(2)?;
    a.move_view("v-2", DEFAULT)?;
    b.move_view("v-1", DEFAULT)?;
    sync(&a, &b)?;
    same(&a, &b);
    let mut tabs = a.view_ids();
    tabs.sort();
    assert_eq!(tabs, [DEFAULT, "v-1", "v-2"], "every tab once");
    Ok(())
}

#[test]
fn the_same_card_dragged_to_two_groups_ends_in_one() -> LoroResult<()> {
    let a = kanban(1)?;
    a.add_view("v-board", "Board", "board", Some("status"))?;
    let b = a.fork(2)?;
    a.move_card("r1", "status", Some("doing"), None)?;
    b.move_card("r1", "status", Some("done"), Some("r2"))?;
    sync(&a, &b)?;
    same(&a, &b);
    let board = a.board("v-board").expect("board");
    let homes: Vec<_> = board
        .iter()
        .filter(|(_, rows)| rows.iter().any(|r| r == "r1"))
        .collect();
    assert_eq!(homes.len(), 1, "one card, one group: {board:?}");
    Ok(())
}

#[test]
fn a_card_dragged_into_an_option_deleted_on_the_other_device_shows_under_no_value() -> LoroResult<()> {
    let a = kanban(1)?;
    a.add_view("v-board", "Board", "board", Some("status"))?;
    let b = a.fork(2)?;
    a.delete_option("status", "doing")?;
    b.move_card("r1", "status", Some("doing"), None)?;
    sync(&a, &b)?;
    same(&a, &b);
    let board = a.board("v-board").expect("board");
    assert!(board[0].1.iter().any(|r| r == "r1"), "under No value: {board:?}");
    assert_eq!(
        a.cell("r1", "status"),
        Some("doing".into()),
        "the value is kept, unread, in case the option comes back"
    );
    Ok(())
}

#[test]
fn an_older_device_editing_its_one_table_merges_with_a_newer_devices_views() -> LoroResult<()> {
    let newer = kanban(1)?;
    let older = newer.fork(2)?;
    // The newer device adds a board that hides notes on its cards.
    newer.add_view("v-board", "Board", "board", Some("status"))?;
    newer.hide("v-board", "notes")?;
    // The older device knows one table: it sorts it and hides status, where it always has.
    older.legacy_view().insert("sort", "status")?;
    older.hide_in_schema("status")?;
    older.doc.commit();
    sync(&newer, &older)?;
    same(&newer, &older);

    let table = newer.view(DEFAULT).expect("first view");
    assert_eq!(table.sort.as_deref(), Some("status"));
    assert_eq!(table.hidden, ["status"]);
    let board = newer.view("v-board").expect("board view");
    assert_eq!(
        board.hidden,
        ["notes"],
        "the board's own setting, untouched by the table's"
    );
    assert_eq!(
        at(&older.settings().get_deep_value(), &["v-board", "layout"]),
        Some("board".into()),
        "kept by the older device"
    );
    Ok(())
}

#[test]
fn the_first_views_tab_made_on_two_devices_reads_once_whichever_device_sorts_first() -> LoroResult<()> {
    for (pa, pb) in [(1, 2), (2, 1)] {
        let a = kanban(pa)?;
        let b = a.fork(pb)?;
        // Neither device has a node for the first view yet; each makes one. A also puts its new view first.
        a.add_view("v-a", "A", "table", None)?;
        a.move_view("v-a", DEFAULT)?;
        b.add_view("v-b", "B", "board", Some("status"))?;
        sync(&a, &b)?;
        same(&a, &b);
        // Every tab once, the same on both devices. In both peer orders A's move held, but which copy of the first
        // view's node is read depends on how the two sort; a lost tab move is the worst case (ADR 0075).
        assert_eq!(a.view_ids(), ["v-a", DEFAULT, "v-b"], "peers {pa}, {pb}");
    }
    Ok(())
}
