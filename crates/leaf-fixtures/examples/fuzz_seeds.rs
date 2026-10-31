//! Writes the seed corpus the fuzz targets start from (E08-S11): `fuzz_seeds <corpus folder>` makes one
//! folder per target. `just fuzz` runs it when the corpus is missing. Seeds are real Leaf data (fixture
//! pages with links, a workspace tree, a database, a saved workspace folder) plus small images, SVGs,
//! Markdown and clipboard fragments, so the fuzzer starts from inputs that get deep into the code.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a command-line tool reports on the terminal"
)]

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use leaf_core::PageId;
use leaf_fixtures::{add_links, big_workspace, save_to_folder};
use leaf_model::{
    BlockKind, CellValue, Database, DatabaseCommand, DatabaseOutcome, Outcome, PageCommand, PropertyType,
    Workspace, WorkspaceCommand,
};
use leaf_store::{DocKey, Workspace as Folder};

/// Must match `leaf_fuzz::PART`: what separates a snapshot from its updates in a `doc_bytes` input.
const PART: &[u8] = b"\0LEAF-UPDATE\0";
/// Fixed times, so the seeds are the same on every run.
const AT: i64 = 1_767_225_600_000;

fn main() -> ExitCode {
    let Some(corpus) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: fuzz_seeds <corpus folder>");
        return ExitCode::FAILURE;
    };
    match write_all(&corpus) {
        Ok(count) => {
            println!("{}: {count} seeds", corpus.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("fuzz_seeds: {err}");
            ExitCode::FAILURE
        }
    }
}

fn write_all(corpus: &Path) -> Result<usize, Box<dyn Error>> {
    let mut seeds = Seeds { corpus, count: 0 };
    doc_seeds(&mut seeds)?;
    workspace_seeds(&mut seeds)?;
    asset_seeds(&mut seeds)?;
    paste_seeds(&mut seeds)?;
    Ok(seeds.count)
}

struct Seeds<'a> {
    corpus: &'a Path,
    count: usize,
}

impl Seeds<'_> {
    fn write(&mut self, target: &str, name: &str, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        let folder = self.corpus.join(target);
        fs::create_dir_all(&folder)?;
        fs::write(folder.join(name), bytes)?;
        self.count += 1;
        Ok(())
    }
}

/// `doc_bytes`: a type byte (0 page, 1 workspace, 2 database), then a snapshot and updates.
fn doc_seeds(seeds: &mut Seeds) -> Result<(), Box<dyn Error>> {
    let mut fixture = big_workspace(7, 6, 12)?;
    add_links(&mut fixture, 2, 7)?;
    for (n, (_, page)) in fixture.pages.iter_mut().enumerate() {
        let snapshot = page.to_bytes()?;
        seeds.write(
            "doc_bytes",
            &format!("page-{n}"),
            &[&[0], snapshot.as_slice()].concat(),
        )?;
        // The same page with one later edit as an update, the way autosave stores it.
        page.mark_saved();
        page.apply(PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: BlockKind::Heading { level: 2 },
            text: "Added later".into(),
        })?;
        if let Some(update) = page.unsaved_changes()? {
            let input = [&[0], snapshot.as_slice(), PART, update.as_slice()].concat();
            seeds.write("doc_bytes", &format!("page-{n}-update"), &input)?;
        }
    }
    let (tree, database) = tree_with_database()?;
    seeds.write(
        "doc_bytes",
        "workspace",
        &[&[1], tree.to_bytes()?.as_slice()].concat(),
    )?;
    seeds.write(
        "doc_bytes",
        "database",
        &[&[2], database.to_bytes()?.as_slice()].concat(),
    )?;
    Ok(())
}

/// `workspace_db`: a whole saved workspace database, plus a `view.json`.
fn workspace_seeds(seeds: &mut Seeds) -> Result<(), Box<dyn Error>> {
    let scratch = std::env::temp_dir().join(format!("leaf-fuzz-seeds-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let mut fixture = big_workspace(11, 8, 6)?;
    add_links(&mut fixture, 1, 11)?;
    let (tree, database) = tree_with_database()?;
    let database_id = tree
        .root_pages()?
        .into_iter()
        .find_map(|page| tree.page(page).ok()?.database);
    fixture.workspace = tree;
    {
        let mut folder: Folder = save_to_folder(&scratch, &fixture)?;
        if let Some(id) = database_id {
            folder.store_mut().save(DocKey::Database(id), &database, AT)?;
        }
    } // dropping the folder closes the database, folding its write-ahead log into the file
    seeds.write(
        "workspace_db",
        "workspace.sqlite",
        &fs::read(scratch.join("workspace.sqlite"))?,
    )?;
    let page = PageId::new();
    let view = format!(r#"{{"expanded":["{page}"],"open_page":"{page}"}}"#);
    seeds.write("workspace_db", "view.json", view.as_bytes())?;
    let _ = fs::remove_dir_all(&scratch);
    Ok(())
}

/// A small page tree holding a database page with two columns and three rows, and that database.
fn tree_with_database() -> Result<(Workspace, Database), Box<dyn Error>> {
    let mut tree = Workspace::new();
    let Outcome::PageCreated(home) = tree.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index: 0,
        title: "Home".into(),
        created_at: AT,
    })?
    else {
        return Err("no page".into());
    };
    let Outcome::DatabasePageCreated { page, .. } = tree.apply(WorkspaceCommand::CreateDatabasePage {
        parent: Some(home),
        index: 0,
        title: "Trips".into(),
        created_at: AT,
    })?
    else {
        return Err("no database page".into());
    };
    let mut database = Database::new();
    let mut columns = Vec::new();
    for (index, (name, kind)) in [("Name", PropertyType::Text), ("Days", PropertyType::Number)]
        .into_iter()
        .enumerate()
    {
        let DatabaseOutcome::ColumnCreated(column) = database.apply(DatabaseCommand::AddColumn {
            index,
            name: name.into(),
            kind,
        })?
        else {
            return Err("no column".into());
        };
        columns.push(column);
    }
    for (index, (name, days)) in [("Kyoto", 3.0), ("Tokyo", 4.0), ("Nara", 1.0)]
        .into_iter()
        .enumerate()
    {
        let Outcome::PageCreated(row) = tree.apply(WorkspaceCommand::CreateRowPage {
            database_page: page,
            created_at: AT,
        })?
        else {
            return Err("no row page".into());
        };
        database.apply(DatabaseCommand::AddRow { row, index })?;
        for (column, value) in columns
            .iter()
            .zip([CellValue::Text(name.into()), CellValue::Number(days)])
        {
            database.apply(DatabaseCommand::SetCell {
                row,
                column: *column,
                value: Some(value),
            })?;
        }
    }
    tree.apply(WorkspaceCommand::AddFavourite { page: home })?;
    Ok((tree, database))
}

/// `asset_kind`: the start of each image type Leaf stores, and SVGs it must accept or refuse.
fn asset_seeds(seeds: &mut Seeds) -> Result<(), Box<dyn Error>> {
    let samples: [(&str, &[u8]); 11] = [
        ("png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0"),
        ("jpeg", b"\xFF\xD8\xFF\xE0\0\x10JFIF\0"),
        ("gif", b"GIF89a\x01\0\x01\0\x80\0\0"),
        ("webp", b"RIFF\x1a\0\0\0WEBPVP8 "),
        ("bmp", b"BM\x3a\0\0\0\0\0\0\0\x36\0\0\0\x28\0\0\0\x01\0\0\0\x01\0"),
        ("tiff", b"II*\0\x08\0\0\0"),
        ("avif", b"\0\0\0\x1cftypavif\0\0\0\0"),
        ("heic", b"\0\0\0\x18ftypheic\0\0\0\0"),
        (
            "svg",
            br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#,
        ),
        (
            "svg-external",
            br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><image xlink:href="file:///etc/passwd"/></svg>"#,
        ),
        (
            "svg-huge",
            br#"<!-- big --><svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000" viewBox="0 0 1 1"/>"#,
        ),
    ];
    for (name, bytes) in samples {
        seeds.write("asset_kind", name, bytes)?;
    }
    Ok(())
}

/// `paste`: Markdown of every block kind, links, page names, and Leaf's own fragment JSON.
fn paste_seeds(seeds: &mut Seeds) -> Result<(), Box<dyn Error>> {
    let samples = [
        ("words", "just some words"),
        (
            "inline",
            "**bold** *italic* ~~gone~~ `code` [a link](https://example.com) and [[Itinerary]]",
        ),
        ("headings", "# One\n## Two\n### Three\n\nA paragraph.\n"),
        (
            "lists",
            "- a\n  - nested\n- b\n1. one\n2. two\n- [ ] todo\n- [x] done\n",
        ),
        ("quote", "> quoted\n> more\n\n> [!WARNING]\n> careful\n"),
        ("code", "```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n"),
        ("table", "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n"),
        ("divider", "above\n\n---\n\nbelow\n"),
        ("address", "https://example.com/a?b=1"),
        (
            "fragment-text",
            r#"{"leaf_fragment":1,"text":[{"insert":"bold","bold":true},{"insert":" plain"}]}"#,
        ),
        (
            "fragment-blocks",
            r#"{"leaf_fragment":1,"blocks":[{"type":"heading","level":1,"text":[{"insert":"Title"}]},{"type":"task","priority":"low","due":"2026-02-03","text":[{"insert":"x"}]},{"type":"paragraph","text":[{"insert":"p"}],"children":[{"type":"bulleted_list","text":[{"insert":"child"}]}]}]}"#,
        ),
    ];
    for (name, text) in samples {
        seeds.write("paste", name, text.as_bytes())?;
    }
    Ok(())
}
