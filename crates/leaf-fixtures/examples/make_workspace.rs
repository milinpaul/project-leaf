//! Writes a full-size fixture workspace to a folder, to open in Leaf or point a benchmark at:
//! `just fixture <folder> [seed]` (5,000 pages, 100,000 blocks). Two more numbers give a different
//! size: `make_workspace <folder> 2026 1 10000` is one 10,000-block page, for the typing budget.

use std::path::PathBuf;
use std::process::ExitCode;

use leaf_fixtures::{BIG_WORKSPACE_PAGES, BLOCKS_PER_PAGE, big_workspace, save_to_folder};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(folder) = args.next().map(PathBuf::from) else {
        eprintln!("usage: make_workspace <new folder> [seed] [pages] [blocks per page]");
        return ExitCode::FAILURE;
    };
    let numbers: Vec<String> = args.collect();
    let number = |at: usize| numbers.get(at).and_then(|s| s.parse().ok());
    let seed = numbers.first().and_then(|s| s.parse().ok()).unwrap_or(2026);
    let pages = number(1).unwrap_or(BIG_WORKSPACE_PAGES);
    let blocks_per_page = number(2).unwrap_or(BLOCKS_PER_PAGE);
    let made = big_workspace(seed, pages, blocks_per_page)
        .and_then(|fixture| save_to_folder(&folder, &fixture).map(|_| fixture.block_count()));
    match made {
        Ok(blocks) => {
            println!(
                "{}: {pages} pages, {blocks} blocks (seed {seed})",
                folder.display()
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("could not make the workspace: {err}");
            ExitCode::FAILURE
        }
    }
}
