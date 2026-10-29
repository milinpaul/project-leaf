//! Writes a full-size fixture workspace to a folder, to open in Leaf or point a benchmark at:
//! `just fixture <folder> [seed]` (5,000 pages, 100,000 blocks).

use std::path::PathBuf;
use std::process::ExitCode;

use leaf_fixtures::{BIG_WORKSPACE_PAGES, BLOCKS_PER_PAGE, big_workspace, save_to_folder};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(folder) = args.next().map(PathBuf::from) else {
        eprintln!("usage: make_workspace <new folder> [seed]");
        return ExitCode::FAILURE;
    };
    let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(2026);
    let made = big_workspace(seed, BIG_WORKSPACE_PAGES, BLOCKS_PER_PAGE)
        .and_then(|fixture| save_to_folder(&folder, &fixture).map(|_| fixture.block_count()));
    match made {
        Ok(blocks) => {
            println!(
                "{}: {BIG_WORKSPACE_PAGES} pages, {blocks} blocks (seed {seed})",
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
