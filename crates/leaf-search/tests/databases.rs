//! Databases in search (E09-S10): rows are found by title, text and URL cells under their row's page, and the cells
//! follow edits, trash and rebuilds.
#![allow(clippy::panic, reason = "test helpers fail a test by panicking")]

mod common;

use common::{Saved, TempDir, ok, paragraph};
use leaf_core::{ColumnId, DatabaseId, PageId};
use leaf_model::{
    CellValue, Database, DatabaseCommand, DatabaseOutcome, Outcome, PageDoc, PropertyType, Workspace,
    WorkspaceCommand,
};
use leaf_search::{PROPERTY_KIND, PageHit, SearchIndex, Syncer};

const NEVER: &dyn Fn() -> bool = &|| false;

/// A database page "Plants" with rows Basil and Fern, a Notes (text), Link (URL) and Count (number) column.
struct Plants {
    saved: Saved,
    ws: Workspace,
    db: Database,
    page: PageId,
    id: DatabaseId,
    basil: PageId,
    fern: PageId,
    notes: ColumnId,
}

fn column(db: &mut Database, name: &str, kind: PropertyType) -> ColumnId {
    let index = db.column_ids().len();
    match ok(db.apply(DatabaseCommand::AddColumn {
        index,
        name: name.into(),
        kind,
    })) {
        DatabaseOutcome::ColumnCreated(id) => id,
        other => panic!("expected a column, got {other:?}"),
    }
}

fn set(db: &mut Database, row: PageId, column: ColumnId, value: CellValue) {
    ok(db.apply(DatabaseCommand::SetCell {
        row,
        column,
        value: Some(value),
    }));
}

fn plants() -> Plants {
    let saved = Saved::default();
    let mut ws = Workspace::new();
    let Outcome::DatabasePageCreated { page, database: id } =
        ok(ws.apply(WorkspaceCommand::CreateDatabasePage {
            parent: None,
            index: 0,
            title: "Plants".into(),
            created_at: 1,
        }))
    else {
        panic!("expected a database page");
    };
    let mut row = |title: &str| {
        let Outcome::PageCreated(row) = ok(ws.apply(WorkspaceCommand::CreateRowPage {
            database_page: page,
            created_at: 1,
        })) else {
            panic!("expected a row page");
        };
        ok(ws.apply(WorkspaceCommand::RenamePage {
            page: row,
            title: title.into(),
        }));
        row
    };
    let (basil, fern) = (row("Basil"), row("Fern"));
    let mut db = Database::new();
    let notes = column(&mut db, "Notes", PropertyType::Text);
    let link = column(&mut db, "Link", PropertyType::Url);
    let count = column(&mut db, "Count", PropertyType::Number);
    for (index, row) in [basil, fern].into_iter().enumerate() {
        ok(db.apply(DatabaseCommand::AddRow { row, index }));
    }
    set(&mut db, basil, notes, CellValue::Text("water daily".into()));
    set(&mut db, fern, notes, CellValue::Text("keep in the shade".into()));
    set(
        &mut db,
        fern,
        link,
        CellValue::Text("https://ferns.example/care".into()),
    );
    set(&mut db, basil, count, CellValue::Number(4242.0));
    saved.save_workspace(&ws, 1);
    saved.save_database(id, &db, 2);
    Plants {
        saved,
        ws,
        db,
        page,
        id,
        basil,
        fern,
        notes,
    }
}

fn synced(dir: &TempDir, p: &Plants) -> Syncer<Saved> {
    let mut syncer = Syncer::new(ok(SearchIndex::open(&dir.0)).0, p.saved.clone());
    ok(syncer.reconcile(NEVER));
    syncer
}

fn search(syncer: &Syncer<Saved>, query: &str) -> Vec<PageHit> {
    ok(syncer.index().search(query))
}

fn pages(hits: &[PageHit]) -> Vec<PageId> {
    hits.iter().map(|hit| hit.page).collect()
}

#[test]
fn rows_are_found_by_title_and_by_their_text_and_url_cells() {
    let (dir, p) = (TempDir::new(), plants());
    let syncer = synced(&dir, &p);
    assert_eq!(pages(&search(&syncer, "basil")), [p.basil], "by title");
    assert_eq!(
        pages(&search(&syncer, "plants")),
        [p.page],
        "the database page by its title"
    );

    let hits = search(&syncer, "shade");
    assert_eq!(pages(&hits), [p.fern], "by a text cell, under the row");
    let cell = &hits[0].blocks[0];
    assert_eq!(cell.kind, PROPERTY_KIND);
    assert_eq!(
        cell.block, None,
        "no block to scroll to: the row opens at its properties"
    );
    assert!(cell.excerpt.text.contains("shade"));
    assert!(!hits[0].title_matched);

    assert_eq!(
        pages(&search(&syncer, "ferns example")),
        [p.fern],
        "by a URL cell"
    );
    assert!(search(&syncer, "4242").is_empty(), "numbers aren't text");
}

#[test]
fn an_edited_cell_is_found_by_its_new_text_only() {
    let (dir, mut p) = (TempDir::new(), plants());
    let mut syncer = synced(&dir, &p);
    set(
        &mut p.db,
        p.basil,
        p.notes,
        CellValue::Text("water weekly".into()),
    );
    p.saved.save_database(p.id, &p.db, 3);
    ok(syncer.reconcile(NEVER));
    assert_eq!(pages(&search(&syncer, "weekly")), [p.basil]);
    assert_eq!(pages(&search(&syncer, "daily")), [], "edited away");
    assert_eq!(
        pages(&search(&syncer, "shade")),
        [p.fern],
        "the other cell is untouched"
    );

    ok(p.db.apply(DatabaseCommand::SetCell {
        row: p.basil,
        column: p.notes,
        value: None,
    }));
    p.saved.save_database(p.id, &p.db, 4);
    ok(syncer.reconcile(NEVER));
    assert!(search(&syncer, "weekly").is_empty(), "an emptied cell is gone");
}

#[test]
fn a_row_pages_own_text_keeps_its_cells() {
    let (dir, p) = (TempDir::new(), plants());
    let mut syncer = synced(&dir, &p);
    let mut doc = PageDoc::new();
    paragraph(&mut doc, "repot in spring");
    p.saved.save_page(p.fern, &doc, 5);
    ok(syncer.reconcile(NEVER));
    assert_eq!(pages(&search(&syncer, "repot")), [p.fern]);
    assert_eq!(pages(&search(&syncer, "shade")), [p.fern], "its cells stay");
}

#[test]
fn trashed_rows_and_databases_are_not_found_and_a_rebuild_finds_cells_again() {
    let (dir, mut p) = (TempDir::new(), plants());
    let mut syncer = synced(&dir, &p);
    ok(p.ws.apply(WorkspaceCommand::TrashPage { page: p.fern, at: 6 }));
    p.saved.save_workspace(&p.ws, 6);
    ok(syncer.reconcile(NEVER));
    assert!(
        search(&syncer, "shade").is_empty(),
        "a trashed row's cells go with it"
    );
    assert_eq!(pages(&search(&syncer, "daily")), [p.basil]);

    ok(syncer.rebuild(NEVER));
    assert_eq!(
        pages(&search(&syncer, "daily")),
        [p.basil],
        "a rebuild reads the cells again"
    );
    assert_eq!(pages(&search(&syncer, "shade")), [], "still trashed");

    ok(p.ws.apply(WorkspaceCommand::TrashPage { page: p.page, at: 7 }));
    p.saved.save_workspace(&p.ws, 7);
    ok(syncer.reconcile(NEVER));
    assert!(
        search(&syncer, "daily").is_empty(),
        "the database and its rows went to the Trash"
    );
    assert!(
        ok(syncer.index().indexed_databases()).is_empty(),
        "the database is forgotten"
    );
}
