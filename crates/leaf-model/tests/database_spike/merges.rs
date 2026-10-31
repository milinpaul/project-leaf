//! Two devices editing the same database offline, then syncing: every case must converge (both peers end
//! up with the same table), and the result must be one a person would accept.

use loro::{Container, LoroDoc, LoroResult, LoroValue, TreeParentId, ValueOrContainer};

use super::{Db, export, sync};

/// A database with columns Name (text), Price (number), Tags (multi-select) and rows a, b, c, forked to
/// a second peer.
fn two_peers() -> LoroResult<(Db, Db)> {
    let a = Db::new(1)?;
    a.add_column("c-name", "Name", "text")?;
    a.add_column("c-price", "Price", "number")?;
    a.add_column("c-tags", "Tags", "multi_select")?;
    for row in ["row-a", "row-b", "row-c"] {
        a.add_row(row)?;
    }
    let b = a.fork(2)?;
    Ok((a, b))
}

fn converged(a: &Db, b: &Db) -> Vec<Vec<String>> {
    let table = a.table();
    assert_eq!(table, b.table(), "both peers show the same table");
    table
}

#[test]
fn edits_to_different_cells_of_a_row_both_survive_and_one_cell_settles_on_one_value() -> LoroResult<()> {
    let (a, b) = two_peers()?;
    // Both peers touch row-a's cells for the first time at once: the row map is mergeable, so neither
    // peer's map replaces the other's.
    a.set("row-a", "c-name", "Lamp")?;
    b.set("row-a", "c-price", 12.0)?;
    // The same cell on both: last writer wins, the same on both peers.
    a.set("row-b", "c-price", 1.0)?;
    b.set("row-b", "c-price", 2.0)?;
    sync(&a, &b)?;
    let table = converged(&a, &b);
    assert_eq!(table[1], ["Lamp", "12", ""]);
    assert!(table[2][1] == "1" || table[2][1] == "2", "{table:?}");
    Ok(())
}

#[test]
fn renaming_a_column_while_the_other_peer_fills_it_keeps_both() -> LoroResult<()> {
    let (a, b) = two_peers()?;
    if let Some(ValueOrContainer::Container(Container::Map(price))) = a.schema().get("c-price") {
        price.insert("name", "Cost")?;
    }
    a.doc.commit();
    b.set("row-c", "c-price", 7.0)?;
    sync(&a, &b)?;
    let table = converged(&a, &b);
    assert_eq!(table[0], ["Name", "Cost", "Tags"]);
    assert_eq!(table[3], ["", "7", ""]);
    Ok(())
}

#[test]
fn a_column_deleted_while_the_other_peer_fills_it_stays_deleted_and_the_value_is_kept_aside() -> LoroResult<()>
{
    let (a, b) = two_peers()?;
    a.delete_column("c-price")?;
    b.set("row-a", "c-price", 3.0)?;
    sync(&a, &b)?;
    let table = converged(&a, &b);
    assert_eq!(table[0], ["Name", "Tags"]);
    // Not shown, but not lost: undoing the delete on peer A would show it again.
    assert_eq!(a.raw("row-a", "c-price"), Some(LoroValue::Double(3.0)));
    Ok(())
}

#[test]
fn tags_added_on_both_peers_are_all_kept_and_a_removed_one_goes() -> LoroResult<()> {
    let (a, b) = two_peers()?;
    a.tag("row-a", "c-tags", "o-red")?;
    a.tag("row-a", "c-tags", "o-blue")?;
    sync(&a, &b)?;
    a.tag("row-a", "c-tags", "o-green")?;
    b.tag("row-a", "c-tags", "o-gold")?;
    b.untag("row-a", "c-tags", "o-red")?;
    sync(&a, &b)?;
    let table = converged(&a, &b);
    assert_eq!(table[1][2], "o-blue,o-gold,o-green");
    Ok(())
}

#[test]
fn rows_moved_on_one_peer_and_deleted_on_the_other_converge_without_duplicates() -> LoroResult<()> {
    let (a, b) = two_peers()?;
    // A moves row-a to the end; B deletes row-a and moves row-c to the top.
    a.rows().mov(0, 2)?;
    a.doc.commit();
    b.rows().delete(0, 1)?;
    b.rows().mov(1, 0)?;
    b.doc.commit();
    sync(&a, &b)?;
    assert_eq!(a.row_ids(), b.row_ids());
    // Finding: in a movable list the concurrent move brings the deleted row back. Both peers agree, but a
    // row someone deleted reappears, so row order can't double as row existence (see the tree test below).
    assert_eq!(a.row_ids(), ["row-c", "row-b", "row-a"]);

    // Both move the same row to different places: it ends up in one of them, once.
    let (a, b) = two_peers()?;
    a.rows().mov(0, 2)?;
    a.doc.commit();
    b.rows().mov(0, 1)?;
    b.doc.commit();
    sync(&a, &b)?;
    let rows = a.row_ids();
    assert_eq!(rows, b.row_ids());
    assert_eq!(rows.len(), 3);
    assert_eq!(rows.iter().filter(|r| *r == "row-a").count(), 1);
    Ok(())
}

#[test]
fn rows_added_on_both_peers_are_all_kept() -> LoroResult<()> {
    let (a, b) = two_peers()?;
    a.add_row("row-x")?;
    a.set("row-x", "c-name", "From A")?;
    b.add_row("row-y")?;
    b.set("row-y", "c-name", "From B")?;
    sync(&a, &b)?;
    let rows = a.row_ids();
    assert_eq!(rows, b.row_ids());
    assert_eq!(rows.len(), 5);
    assert!(rows.contains(&"row-x".to_owned()) && rows.contains(&"row-y".to_owned()));
    Ok(())
}

#[test]
fn in_a_tree_a_delete_against_a_concurrent_move_is_settled_by_loros_order_of_the_edits() -> LoroResult<()> {
    // Rows as nodes of a flat Loro tree (fractional index order), as pages and blocks already are. E09-S02
    // corrected this spike: it first tried one case only and concluded "delete wins". Loro orders the two concurrent
    // edits by device ID, so here peer 2's edit wins either way, and the tree alone can't record that a row was
    // deleted (ADR-066, "Correction").
    let names = |doc: &LoroDoc| -> Vec<String> {
        let tree = doc.get_tree("rows");
        tree.children(TreeParentId::Root)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| tree.get_meta(id).ok()?.get("id"))
            .filter_map(|v| match v {
                ValueOrContainer::Value(LoroValue::String(s)) => Some(s.to_string()),
                _ => None,
            })
            .collect()
    };
    for peer_two_deletes in [true, false] {
        let a = LoroDoc::new();
        a.set_peer_id(1)?;
        let rows = a.get_tree("rows");
        rows.enable_fractional_index(0);
        let ids: Vec<_> = (0..3)
            .map(|_| rows.create(TreeParentId::Root))
            .collect::<LoroResult<_>>()?;
        for (id, name) in ids.iter().zip(["row-a", "row-b", "row-c"]) {
            rows.get_meta(*id)?.insert("id", name)?;
        }
        a.commit();
        let b = LoroDoc::new();
        b.set_peer_id(2)?;
        b.import(&export(&a, loro::ExportMode::all_updates())?)?;

        let (mover, deleter) = if peer_two_deletes { (&a, &b) } else { (&b, &a) };
        mover.get_tree("rows").mov_to(ids[0], TreeParentId::Root, 2)?;
        mover.commit();
        deleter.get_tree("rows").delete(ids[0])?;
        deleter.commit();
        b.import(&export(&a, loro::ExportMode::all_updates())?)?;
        a.import(&export(&b, loro::ExportMode::all_updates())?)?;

        assert_eq!(names(&a), names(&b), "both peers agree either way");
        let expected: &[&str] = if peer_two_deletes {
            &["row-b", "row-c"]
        } else {
            &["row-b", "row-c", "row-a"]
        };
        assert_eq!(names(&a), expected, "peer_two_deletes={peer_two_deletes}");
    }
    Ok(())
}

#[test]
fn a_column_deleted_on_one_peer_and_moved_on_the_other_comes_back_in_the_order_but_without_a_schema()
-> LoroResult<()> {
    let (a, b) = two_peers()?;
    a.delete_column("c-price")?;
    b.columns().mov(1, 0)?;
    b.doc.commit();
    sync(&a, &b)?;
    // The same movable-list behaviour as rows: the moved ID is back in the order...
    assert!(a.column_ids().contains(&"c-price".to_owned()));
    assert_eq!(a.column_ids(), b.column_ids());
    // ...but its schema is gone, so a reader must skip order entries without a schema.
    assert!(a.schema().get("c-price").is_none());
    assert_eq!(converged(&a, &b)[0], ["?", "Name", "Tags"]);
    Ok(())
}
